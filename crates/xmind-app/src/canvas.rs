//! The infinite canvas: scene rendering, hit-testing, selection, dragging,
//! in-place editing, context menu, find highlights, presentation overlay.

use crate::editor::{DragMode, Editor, View};
use crate::i18n::{tr, Lang};
use egui::{Align2, Color32, FontId, Pos2, Rect as ERect, Sense, Stroke, Vec2};
use std::collections::HashMap;
use xmind_core::col::Col;
use xmind_core::geo::{flatten_cubic, dist_to_polyline, Rect, V2};
use xmind_core::marker::Marker;
use xmind_core::model::{Side, Structure};
use xmind_core::scene::{HitTarget, Prim, Scene};
use xmind_core::text::{FontMetrics, TextMeasurer};

pub struct EguiMeasurer<'a> {
    pub ctx: &'a egui::Context,
}

impl TextMeasurer for EguiMeasurer<'_> {
    fn metrics(&self) -> FontMetrics {
        FontMetrics { ascent: crate::fonts::ascent_ratio(), descent: crate::fonts::descent_ratio() }
    }

    fn line_width(&self, text: &str, size: f32, _bold: bool) -> f32 {
        self.ctx
            .fonts(|f| f.layout_no_wrap(text.to_string(), FontId::proportional(size), Color32::WHITE))
            .size()
            .x
    }
}

fn c32(c: Col) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0, c.1, c.2, c.3)
}

fn pos2(x: f32, y: f32) -> Pos2 {
    Pos2::new(x, y)
}

fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

enum Hit {
    Topic(String),
    Badge(String),
    Boundary { parent: String, id: String },
    Rel(String),
}

fn hit_test(scene: &Scene, w: V2) -> Option<Hit> {
    for (id, r) in &scene.badges {
        if r.contains(w) {
            return Some(Hit::Badge(id.clone()));
        }
    }
    let mut best: Option<(f32, &str)> = None;
    for h in &scene.hits {
        if let HitTarget::Topic(id) = &h.target {
            if h.rect.contains(w) {
                let area = h.rect.w * h.rect.h;
                if best.map(|(a, _)| area < a).unwrap_or(true) {
                    best = Some((area, id));
                }
            }
        }
    }
    if let Some((_, id)) = best {
        return Some(Hit::Topic(id.to_string()));
    }
    for rc in &scene.rels {
        let pts = flatten_cubic(rc.p0, rc.c1, rc.c2, rc.p1, 24);
        if dist_to_polyline(w, &pts) < 7.0 {
            return Some(Hit::Rel(rc.id.clone()));
        }
    }
    for h in &scene.hits {
        if let HitTarget::Boundary { parent, id } = &h.target {
            if h.rect.contains(w) {
                return Some(Hit::Boundary { parent: parent.clone(), id: id.clone() });
            }
        }
    }
    None
}

pub fn show(ui: &mut egui::Ui, ed: &mut Editor, lang: Lang) {
    let size = ui.available_size();
    let canvas = ui.allocate_rect(ERect::from_min_size(ui.cursor().min, size), Sense::click_and_drag());
    let rect = canvas.rect;
    let painter = ui.painter().clone();
    let ctx = ui.ctx().clone();

    // ---------- scene (cached on doc/selection-independent state) ----------
    let theme_id = ed.doc.sheet().theme.clone().unwrap_or_else(|| ed.doc.theme.clone());
    let drill_top = ed.drill.last().cloned().unwrap_or_default();
    let edit_buf = ed.editing.as_ref().map(|(_, b)| b.clone()).unwrap_or_default();
    let key = (ed.scene_ver, ed.doc.current, drill_top, edit_buf);
    if ed.scene.is_none() || ed.scene_key != key {
        let measurer = EguiMeasurer { ctx: &ctx };
        let sheet = ed.doc.sheet();
        let edit = ed.editing.as_ref().map(|(id, buf)| (id.as_str(), buf.as_str()));
        let scene = xmind_core::layout::build_scene_root(
            sheet,
            &theme_id,
            &measurer,
            edit,
            ed.drill.last().map(|s| s.as_str()),
        );
        ed.scene = Some(scene);
        ed.scene_key = key;
        // preload image textures
        ed.tex_cache.clear();
        let assets: Vec<(String, Vec<u8>)> = ed
            .doc
            .assets
            .iter()
            .map(|(k, a)| (k.clone(), a.data.clone()))
            .collect();
        for (k, data) in assets {
            if let Ok(img) = image::load_from_memory(&data) {
                let rgba = img.to_rgba8();
                let sz = (rgba.width(), rgba.height());
                let ci = egui::ColorImage::from_rgba_unmultiplied([sz.0 as usize, sz.1 as usize], rgba.as_raw());
                let handle = ctx.load_texture(format!("img-{k}"), ci, egui::TextureOptions::default());
                ed.tex_cache.insert(k, handle);
            }
        }
    }
    let mut scene_owned = std::mem::take(&mut ed.scene).expect("scene");
    let scene = &scene_owned;

    // ---------- wheel / zoom / pan ----------
    let hovered = canvas.hovered();
    {
        let zoom = ctx.input(|i| i.zoom_delta());
        let scroll = ctx.input(|i| i.smooth_scroll_delta);
        if hovered && zoom != 1.0 {
            if let Some(p) = canvas.hover_pos() {
                ed.view.zoom_at((p.x, p.y), zoom);
            }
        }
        if hovered {
            ed.view.pan_x += scroll.x;
            ed.view.pan_y += scroll.y;
        }
    }

    let v = ed.view;
    let w2s = |x: f32, y: f32| -> Pos2 {
        let (sx, sy) = v.to_screen(x, y);
        pos2(sx, sy)
    };
    let rect_of = |r: &Rect| -> ERect {
        ERect::from_min_max(w2s(r.x, r.y), w2s(r.right(), r.bottom()))
    };

    // ---------- background + content ----------
    painter.rect_filled(rect, 0.0, c32(scene.bg));
    draw_prims(&painter, scene, &v, &ed.tex_cache);

    // ---------- overlays ----------
    let sel_color = Color32::from_rgb(29, 122, 243);
    for id in &ed.sel {
        if let Some(r) = scene.rect_of(id) {
            painter.rect_stroke(rect_of(&r).expand(3.0 * v.zoom), 6.0 * v.zoom, Stroke::new(2.5, sel_color));
        }
    }
    if let Some((pid, bid)) = &ed.sel_boundary {
        for h in &scene.hits {
            if let HitTarget::Boundary { parent, id } = &h.target {
                if parent == pid && id == bid {
                    painter.rect_stroke(rect_of(&h.rect), 8.0, Stroke::new(2.5, Color32::from_rgb(99, 102, 241)));
                }
            }
        }
    }
    if let Some(rid) = &ed.sel_rel {
        for rc in &scene.rels {
            if &rc.id == rid {
                let pts: Vec<Pos2> = flatten_cubic(rc.p0, rc.c1, rc.c2, rc.p1, 24)
                    .into_iter()
                    .map(|p| w2s(p.x, p.y))
                    .collect();
                painter.add(egui::Shape::line(pts, Stroke::new(4.0, Color32::from_rgba_unmultiplied(99, 102, 241, 110))));
            }
        }
    }
    if let Some(f) = &ed.find {
        for (i, id) in f.matches.iter().enumerate() {
            if let Some(r) = scene.rect_of(id) {
                let c = if i == f.idx { Color32::from_rgb(245, 158, 11) } else { Color32::from_rgb(250, 204, 21) };
                painter.rect_stroke(rect_of(&r).expand(5.0 * v.zoom), 6.0, Stroke::new(2.0, c));
            }
        }
    }
    if let Some(src) = &ed.pending_rel {
        if let Some(r) = scene.rect_of(src) {
            painter.rect_stroke(rect_of(&r).expand(6.0 * v.zoom), 8.0, Stroke::new(2.5, Color32::from_rgb(236, 72, 153)));
        }
    }

    // presentation: dim everything, redraw current subtree on top
    if let Some(cur) = ed.current_present_id() {
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(15, 18, 25, 135));
        let mut keep: Vec<&str> = Vec::new();
        if let Some(t) = ed.doc.sheet().find(&cur) {
            keep.push(t.id.as_str());
            t.walk(&mut |x| keep.push(x.id.as_str()));
        }
        let ranges: Vec<(usize, usize)> = keep.iter().filter_map(|id| scene.node_prims.get(*id)).cloned().collect();
        for (i, prim) in scene.prims.iter().enumerate() {
            if ranges.iter().any(|(s, e)| i >= *s && i < *e) {
                draw_prim(&painter, prim, &v, &ed.tex_cache);
            }
        }
        painter.text(
            pos2(rect.center().x, rect.bottom() - 12.0),
            Align2::CENTER_BOTTOM,
            tr(lang, "exit_present"),
            FontId::proportional(14.0),
            Color32::from_gray(205),
        );
    }

    // drill breadcrumb
    if !ed.drill.is_empty() {
        let mut x = rect.left() + 12.0;
        let chain = drill_chain(ed);
        for (i, (id, title)) in chain.iter().enumerate() {
            let short: String = title.chars().take(12).collect();
            let label = if i == 0 { format!("⌂ {short}") } else { format!("› {short}") };
            let galley = ctx.fonts(|f| f.layout_no_wrap(label, FontId::proportional(13.0), Color32::from_rgb(51, 65, 85)));
            let w = galley.size().x + 16.0;
            let cr = ERect::from_min_size(pos2(x, rect.top() + 10.0), vec2(w, 24.0));
            painter.rect_filled(cr, 8.0, Color32::from_rgba_unmultiplied(255, 255, 255, 225));
            painter.galley(pos2(cr.left() + 8.0, cr.center().y - galley.size().y / 2.0), galley, Color32::WHITE);
            x = cr.right() + 6.0;
            let _ = id;
        }
    }

    if ed.pending_rel.is_some() {
        painter.text(
            pos2(rect.center().x, rect.bottom() - 12.0),
            Align2::CENTER_BOTTOM,
            tr(lang, "click_target"),
            FontId::proportional(14.0),
            Color32::from_rgb(219, 39, 119),
        );
    }

    // ---------- interactions ----------
    let pointer = canvas.interact_pointer_pos();
    let world = pointer.map(|p| {
        let (x, y) = v.to_world(p.x, p.y);
        V2::new(x, y)
    });

    if canvas.drag_started() {
        let over_topic = world.and_then(|w| match hit_test(scene, w) {
            Some(Hit::Topic(id)) => Some(id),
            _ => None,
        });
        let mods = ctx.input(|i| i.modifiers);
        match over_topic {
            Some(id) => {
                if !ed.sel.contains(&id) {
                    ed.select_topic(&id, mods.ctrl || mods.shift);
                }
                ed.drag_mode = DragMode::MoveTopics;
                let w = world.unwrap();
                ed.drag_origin = (w.x, w.y);
                ed.drag_last = (w.x, w.y);
                ed.drag_moved = false;
            }
            _ => {
                ed.drag_mode = if mods.shift { DragMode::RectSelect } else { DragMode::Pan };
                let p = pointer.unwrap();
                ed.drag_origin = (p.x, p.y);
                ed.drag_last = (p.x, p.y);
            }
        }
    }

    match ed.drag_mode {
        DragMode::Pan => {
            if canvas.dragged() {
                if let Some(p) = pointer {
                    let d = p - pos2(ed.drag_last.0, ed.drag_last.1);
                    ed.view.pan_x += d.x;
                    ed.view.pan_y += d.y;
                    ed.drag_last = (p.x, p.y);
                }
            }
            if canvas.drag_stopped() {
                ed.drag_mode = DragMode::None;
            }
        }
        DragMode::MoveTopics => {
            if canvas.dragged() {
                if let Some(w) = world {
                    let dx = w.x - ed.drag_last.0;
                    let dy = w.y - ed.drag_last.1;
                    if (w.x - ed.drag_origin.0).abs() + (w.y - ed.drag_origin.1).abs() > 3.0 {
                        ed.drag_moved = true;
                    }
                    if ed.drag_moved {
                        let ids = ed.sel.clone();
                        for id in ids {
                            if let Some(t) = ed.doc.sheet_mut().find_mut(&id) {
                                if let Some(abs) = t.abs_pos {
                                    t.abs_pos = Some((abs.0 + dx, abs.1 + dy));
                                } else {
                                    let o = t.offset.unwrap_or((0.0, 0.0));
                                    t.offset = Some((o.0 + dx, o.1 + dy));
                                }
                            }
                        }
                        ed.scene_ver += 1;
                    }
                    ed.drag_last = (w.x, w.y);
                }
            }
            if canvas.drag_stopped() {
                if let (true, Some(w)) = (ed.drag_moved, world) {
                    let dragged = ed.sel.clone();
                    if let Some(Hit::Topic(target)) = hit_test(scene, w) {
                        if !dragged.contains(&target) {
                            if let Some(first) = dragged.first().cloned() {
                                // target must not be inside the dragged subtree
                                let inside = ed
                                    .doc
                                    .sheet()
                                    .find(&first)
                                    .map(|t| t.find(&target).is_some())
                                    .unwrap_or(false);
                                if inside {
                                    ed.toast = Some(("不能移动到自己的子主题下".into(), 2.0));
                                } else if ed.parent_info(&target).map(|(p, _)| p) == Some(first.clone()) {
                                    ed.toast = Some(("不能移动到自己的子主题下".into(), 2.0));
                                } else {
                                    ed.move_subtree(&first, &target, None);
                                }
                            }
                        }
                    }
                }
                ed.drag_mode = DragMode::None;
                ed.drag_moved = false;
            }
        }
        DragMode::RectSelect => {
            let a = pos2(ed.drag_origin.0, ed.drag_origin.1);
            if canvas.dragged() {
                let b = pointer.unwrap_or(a);
                let sel_rect = ERect::from_two_pos(a, b);
                painter.rect_stroke(sel_rect, 2.0, Stroke::new(1.5, sel_color));
                let (x0, y0) = ed.view.to_world(sel_rect.left(), sel_rect.top());
                let (x1, y1) = ed.view.to_world(sel_rect.right(), sel_rect.bottom());
                let wr = Rect::new(x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs());
                let mut ids = Vec::new();
                for (id, r) in &scene.rects {
                    if r.intersect(&wr) {
                        ids.push(id.clone());
                    }
                }
                ed.sel = ids;
            }
            if canvas.drag_stopped() {
                ed.drag_mode = DragMode::None;
            }
        }
        DragMode::None => {}
    }

    // click / double-click
    if let Some(w) = world {
        if canvas.clicked() || canvas.drag_stopped() {
            if ed.pending_rel.is_some() {
                match hit_test(scene, w) {
                    Some(Hit::Topic(t)) => {
                        ed.complete_relationship(&t);
                    }
                    _ => ed.pending_rel = None,
                }
            } else {
                match hit_test(scene, w) {
                    Some(Hit::Badge(id)) => ed.toggle_expand(&id),
                    Some(Hit::Topic(id)) => {
                        let extend = ctx.input(|i| i.modifiers.ctrl || i.modifiers.shift);
                        ed.select_topic(&id, extend);
                    }
                    Some(Hit::Rel(rid)) => {
                        ed.sel.clear();
                        ed.sel_boundary = None;
                        ed.sel_rel = Some(rid);
                    }
                    Some(Hit::Boundary { parent, id }) => {
                        ed.sel.clear();
                        ed.sel_rel = None;
                        ed.sel_boundary = Some((parent, id));
                    }
                    None => {
                        if !ctx.input(|i| i.modifiers.ctrl) {
                            ed.sel.clear();
                            ed.sel_rel = None;
                            ed.sel_boundary = None;
                        }
                    }
                }
            }
        }
        if canvas.double_clicked() {
            if let Some(Hit::Topic(id)) = hit_test(scene, w) {
                if ctx.input(|i| i.modifiers.command) {
                    if let Some(url) = ed.topic(&id).and_then(|t| t.link.clone()) {
                        ctx.output_mut(|o| o.open_url = Some(egui::OpenUrl::new_tab(url)));
                    }
                } else {
                    ed.start_editing(&id);
                }
            }
        }
    }

    // zoom keys
    if hovered && !ed.editing.is_some() {
        let (plus, minus, zero) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Equals) && i.modifiers.command,
                i.key_pressed(egui::Key::Minus) && i.modifiers.command,
                i.key_pressed(egui::Key::Num0) && i.modifiers.command,
            )
        });
        if plus {
            ed.view.zoom_at((rect.center().x, rect.center().y), 1.2);
        }
        if minus {
            ed.view.zoom_at((rect.center().x, rect.center().y), 1.0 / 1.2);
        }
        if zero {
            ed.view.zoom = 1.0;
        }
    }

    // topic keyboard (when not editing text and no other widget has focus)
    if ed.editing.is_none() && !ctx.wants_keyboard_input() && !ctx.is_using_pointer() {
        keyboard(&ctx, ed);
    }

    // ---------- in-place edit ----------
    if let Some((id, buf)) = ed.editing.clone() {
        if let Some(r) = scene.rect_of(&id) {
            let sr = rect_of(&r);
            let sz = (16.0 * ed.view.zoom).clamp(11.0, 30.0);
            let edit_rect = ERect::from_min_max(
                pos2(sr.left() + 4.0, sr.top() + 1.0),
                pos2(sr.right() - 4.0, sr.bottom() - 1.0),
            );
            let mut buf2 = buf;
            let out = ui.put(
                edit_rect,
                egui::TextEdit::singleline(&mut buf2)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(edit_rect.width()),
            );
            let eid = id.clone();
            ed.editing = Some((eid, buf2));
            if !out.has_focus() && !out.lost_focus() {
                out.request_focus();
            }
            let (enter, esc, tab) = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::Enter),
                    i.key_pressed(egui::Key::Escape),
                    i.key_pressed(egui::Key::Tab),
                )
            });
            if esc {
                ed.cancel_editing();
            } else if tab {
                ed.commit_editing();
                if let Some(n) = ed.add_child(&id, "") {
                    ed.select_topic(&n, false);
                    ed.start_editing(&n);
                }
            } else if enter {
                ed.commit_editing();
            } else if out.lost_focus() {
                // 点击其他地方 = 确认输入（Tab 会转移焦点，已在上面先处理）
                ed.commit_editing();
            }
        }
    }

    // ---------- context menu ----------
    let scene_ptr = &scene_owned as *const Scene;
    let scene_owned_ref: &Scene = unsafe { &*scene_ptr };
    ed.scene = Some(std::mem::take(&mut scene_owned));
    let _ = scene_owned_ref;
    canvas.context_menu(|ui| context_menu(ui, ed, lang));

    // toast
    if let Some((msg, t)) = ed.toast.clone() {
        let dt = ctx.input(|i| i.unstable_dt.min(0.1)) as f64;
        let t = t - dt;
        if t <= 0.0 {
            ed.toast = None;
        } else {
            ed.toast = Some((msg.clone(), t));
            let galley = ctx.fonts(|f| f.layout_no_wrap(msg, FontId::proportional(13.0), Color32::WHITE));
            let w = galley.size().x + 24.0;
            let r = ERect::from_center_size(pos2(rect.center().x, rect.top() + 30.0), vec2(w, 26.0));
            painter.rect_filled(r, 13.0, Color32::from_rgba_unmultiplied(30, 35, 45, 235));
            painter.galley(pos2(r.left() + 12.0, r.center().y - galley.size().y / 2.0), galley, Color32::WHITE);
        }
    }
}

fn drill_chain(ed: &Editor) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let root_id = ed.doc.sheet().root.id.clone();
    let mut cur = root_id;
    loop {
        let title = ed.doc.sheet().find(&cur).map(|t| t.text.clone()).unwrap_or_default();
        out.push((cur.clone(), title));
        if cur == *ed.drill.last().unwrap() {
            break;
        }
        let mut next = None;
        if let Some(t) = ed.doc.sheet().find(&cur) {
            for d in &ed.drill {
                if t.find(d).is_some() {
                    next = Some(d.clone());
                    break;
                }
            }
        }
        match next {
            Some(n) if n != cur => cur = n,
            _ => break,
        }
    }
    out
}

fn draw_prims(painter: &egui::Painter, scene: &Scene, v: &View, tex: &HashMap<String, egui::TextureHandle>) {
    for p in &scene.prims {
        draw_prim(painter, p, v, tex);
    }
}

fn draw_prim(painter: &egui::Painter, prim: &Prim, v: &View, tex: &HashMap<String, egui::TextureHandle>) {
    let w2s = |x: f32, y: f32| -> Pos2 {
        let (sx, sy) = v.to_screen(x, y);
        pos2(sx, sy)
    };
    let zoom = v.zoom;
    match prim {
        Prim::Rect { r, rounding, fill, stroke } => {
            let rect = ERect::from_min_max(w2s(r.x, r.y), w2s(r.right(), r.bottom()));
            let rr = rounding * zoom;
            if let Some(f) = fill {
                painter.rect_filled(rect, rr, c32(*f));
            }
            if let Some((w, c)) = stroke {
                painter.rect_stroke(rect, rr, Stroke::new(w.max(0.8) * zoom, c32(*c)));
            }
        }
        Prim::Poly { pts, closed: _, fill, stroke } => {
            let screen: Vec<Pos2> = pts.iter().map(|p| w2s(p.x, p.y)).collect();
            if screen.len() >= 2 {
                if let Some(f) = fill {
                    painter.add(egui::Shape::convex_polygon(screen.clone(), c32(*f), Stroke::NONE));
                }
                if let Some((w, c)) = stroke {
                    painter.add(egui::Shape::line(screen, Stroke::new(w.max(0.7) * zoom, c32(*c))));
                }
            }
        }
        Prim::Circle { c, r, fill, stroke } => {
            let center = w2s(c.x, c.y);
            let rr = r * zoom;
            if let Some(f) = fill {
                painter.circle_filled(center, rr, c32(*f));
            }
            if let Some((w, s)) = stroke {
                painter.circle_stroke(center, rr, Stroke::new(w.max(0.7) * zoom, c32(*s)));
            }
        }
        Prim::Text { p, text, size, color, bold, italic: _, center } => {
            let pos = w2s(p.x, p.y);
            let sz = size * zoom;
            let col = c32(*color);
            // p.y is the glyph baseline. egui places a galley's baseline at
            // `top + ascent`, so anchor by the real font ascent — no
            // bottom-anchor guesswork, pixel-exact like the exporter.
            let ascent = crate::fonts::ascent_ratio() * sz;
            let galley = painter
                .ctx()
                .fonts(|f| f.layout_no_wrap(text.clone(), FontId::proportional(sz), col));
            let x = if *center { pos.x - galley.size().x * 0.5 } else { pos.x };
            let top = pos2(x, pos.y - ascent);
            painter.galley(top, galley, col);
            if *bold {
                let off = 0.35 * zoom.max(0.3);
                let galley = painter
                    .ctx()
                    .fonts(|f| f.layout_no_wrap(text.clone(), FontId::proportional(sz), col));
                painter.galley(top + Vec2::new(off, 0.0), galley, col);
            }
        }
        Prim::Tris { tris, color } => {
            for [a, b, c] in tris {
                painter.add(egui::Shape::convex_polygon(
                    vec![w2s(a.x, a.y), w2s(b.x, b.y), w2s(c.x, c.y)],
                    c32(*color),
                    Stroke::NONE,
                ));
            }
        }
        Prim::Image { r, asset } => {
            if let Some(h) = tex.get(asset) {
                let rect = ERect::from_min_max(w2s(r.x, r.y), w2s(r.right(), r.bottom()));
                painter.image(h.id(), rect, ERect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            }
        }
    }
}

// keyboard ---------------------------------------------------------------

fn keyboard(ctx: &egui::Context, ed: &mut Editor) {
    // presentation mode: only stepping + exit
    if ed.presentation.is_some() {
        let (fwd, back, esc) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::Space),
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if esc {
            ed.presentation = None;
        } else if fwd {
            ed.present_step(true);
        } else if back {
            ed.present_step(false);
        }
        center_on_current(ctx, ed);
        return;
    }

    let events: Vec<egui::Event> = ctx.input(|i| i.events.clone());
    for ev in events {
        let egui::Event::Key { key, pressed: true, .. } = ev else { continue };
        match key {
            egui::Key::Tab => {
                if let Some(id) = ed.primary().cloned() {
                    if let Some(n) = ed.add_child(&id, "") {
                        ed.select_topic(&n, false);
                        ed.start_editing(&n);
                    }
                } else {
                    let root = ed.doc.sheet().root.id.clone();
                    ed.select_topic(&root, false);
                }
            }
            egui::Key::Enter => {
                if let Some(id) = ed.primary().cloned() {
                    if ed.doc.sheet().root.id == id {
                        if let Some(n) = ed.add_child(&id, "") {
                            ed.select_topic(&n, false);
                            ed.start_editing(&n);
                        }
                    } else if let Some(n) = ed.add_sibling(&id, "") {
                        ed.select_topic(&n, false);
                        ed.start_editing(&n);
                    }
                }
            }
            egui::Key::F2 | egui::Key::Space => {
                if let Some(id) = ed.primary().cloned() {
                    ed.start_editing(&id);
                }
            }
            egui::Key::Delete | egui::Key::Backspace => {
                if !ed.sel.is_empty() || ed.sel_rel.is_some() || ed.sel_boundary.is_some() {
                    ed.delete_selected();
                }
            }
            egui::Key::ArrowUp => nav_sibling(ed, -1),
            egui::Key::ArrowDown => nav_sibling(ed, 1),
            egui::Key::ArrowLeft => nav_parent(ed),
            egui::Key::ArrowRight => nav_child(ed),
            egui::Key::Escape => {
                if ed.pending_rel.take().is_some() {
                } else if !ed.drill.is_empty() {
                    ed.drill.pop();
                    ed.scene_ver += 1;
                } else if ed.find.take().is_some() {
                } else {
                    ed.sel.clear();
                }
            }
            _ => {}
        }
    }
}

fn center_on_current(ctx: &egui::Context, ed: &mut Editor) {
    if let (Some(cur), Some(scene)) = (ed.current_present_id(), ed.scene.as_ref()) {
        if let Some(r) = scene.rect_of(&cur) {
            let size = ctx.screen_rect().size();
            ed.view.center_on((size.x, size.y), r, Some(1.4));
        }
    }
}

fn nav_sibling(ed: &mut Editor, dir: i32) {
    let Some(id) = ed.primary().cloned() else { return };
    let Some((pid, idx)) = ed.parent_info(&id) else {
        let sheet = ed.doc.sheet();
        if !sheet.root.children.is_empty() {
            let t = if dir > 0 { &sheet.root.children[0] } else { sheet.root.children.last().unwrap() };
            let tid = t.id.clone();
            ed.select_topic(&tid, false);
        }
        return;
    };
    if idx >= 10_000 {
        return;
    }
    let children: Vec<String> = ed
        .doc
        .sheet()
        .find(&pid)
        .map(|p| p.children.iter().map(|c| c.id.clone()).collect())
        .unwrap_or_default();
    let ni = idx as i32 + dir;
    if ni >= 0 && (ni as usize) < children.len() {
        ed.select_topic(&children[ni as usize], false);
    } else {
        let Some((gp, gidx)) = ed.parent_info(&pid) else { return };
        if gidx < 10_000 {
            let siblings: Vec<String> = ed
                .doc
                .sheet()
                .find(&gp)
                .map(|p| p.children.iter().map(|c| c.id.clone()).collect())
                .unwrap_or_default();
            let gi = gidx as i32 + dir;
            if gi >= 0 && (gi as usize) < siblings.len() {
                let mut cur = siblings[gi as usize].clone();
                loop {
                    let next = ed.doc.sheet().find(&cur).and_then(|t| t.children.first().map(|c| c.id.clone()));
                    match next {
                        Some(n) if n != id => cur = n,
                        _ => break,
                    }
                }
                ed.select_topic(&cur, false);
            }
        }
    }
}

fn nav_parent(ed: &mut Editor) {
    if let Some(id) = ed.primary().cloned() {
        if let Some((pid, _)) = ed.parent_info(&id) {
            ed.select_topic(&pid, false);
        }
    }
}

fn nav_child(ed: &mut Editor) {
    if let Some(id) = ed.primary().cloned() {
        if let Some(t) = ed.doc.sheet().find(&id) {
            if !t.expanded && !t.children.is_empty() {
                ed.toggle_expand(&id);
            } else if let Some(c) = t.children.first() {
                let cid = c.id.clone();
                ed.select_topic(&cid, false);
            }
        }
    }
}

// context menu ------------------------------------------------------------

fn context_menu(ui: &mut egui::Ui, ed: &mut Editor, lang: Lang) {
    let root_id = ed.doc.sheet().root.id.clone();
    let id = ed.primary().cloned().unwrap_or_else(|| root_id.clone());
    let is_root = id == root_id;
    ui.menu_button(tr(lang, "insert"), |ui| {
        if ui.button(format!("{} (Tab)", tr(lang, "child"))).clicked() {
            if let Some(n) = ed.add_child(&id, "") {
                ed.select_topic(&n, false);
                ed.start_editing(&n);
            }
            ui.close_menu();
        }
        if ui.add_enabled(!is_root, egui::Button::new(format!("{} (Enter)", tr(lang, "sibling")))).clicked() {
            if let Some(n) = ed.add_sibling(&id, "") {
                ed.select_topic(&n, false);
                ed.start_editing(&n);
            }
            ui.close_menu();
        }
        if ui.add_enabled(!is_root, egui::Button::new(tr(lang, "parent"))).clicked() {
            if let Some(n) = ed.insert_parent_for(&id, "上级主题") {
                ed.select_topic(&n, false);
            }
            ui.close_menu();
        }
        ui.separator();
        if ui.button(tr(lang, "relationship")).clicked() {
            ed.begin_relationship(&id);
            ui.close_menu();
        }
        if ui.add_enabled(ed.sel.len() >= 2, egui::Button::new(tr(lang, "boundary"))).clicked() {
            if !ed.add_boundary_from_selection() {
                ed.toast = Some(("请选择同一父主题下的多个同级主题".into(), 2.5));
            }
            ui.close_menu();
        }
        if ui.add_enabled(!ed.sel.is_empty(), egui::Button::new(tr(lang, "summary"))).clicked() {
            if !ed.add_summary_from_selection() {
                ed.toast = Some(("请选择同一父主题下的多个同级主题".into(), 2.5));
            }
            ui.close_menu();
        }
        ui.separator();
        if ui.button(tr(lang, "floating")).clicked() {
            if let Some(n) = ed.add_floating((420.0, 80.0)) {
                ed.select_topic(&n, false);
                ed.start_editing(&n);
            }
            ui.close_menu();
        }
    });
    ui.menu_button(tr(lang, "marker"), |ui| {
        for (name, ms) in crate::editor::marker_groups() {
            ui.menu_button(name, |ui| {
                for m in ms {
                    if ui.button(marker_label(&m)).clicked() {
                        ed.toggle_marker(&id, m);
                        ui.close_menu();
                    }
                }
            });
        }
        if ui.button(tr(lang, "remove_marker")).clicked() {
            ed.clear_markers(&id);
            ui.close_menu();
        }
    });
    ui.menu_button(tr(lang, "style"), |ui| {
        ui.label(tr(lang, "fill"));
        ui.horizontal(|ui| {
            for c in crate::editor::swatches() {
                if ui.add(egui::Button::new("  ").fill(c32(c))).clicked() {
                    ed.with_style(&id, |s| s.fill = Some(c));
                }
            }
        });
        ui.separator();
        ui.label(tr(lang, "text"));
        ui.horizontal(|ui| {
            for c in crate::editor::swatches() {
                if ui.add(egui::Button::new(egui::RichText::new("A").color(c32(c)))).clicked() {
                    ed.with_style(&id, |s| s.text = Some(c));
                }
            }
        });
        ui.separator();
        if ui.button(tr(lang, "auto")).clicked() {
            ed.with_style(&id, |s| {
                s.fill = None;
                s.text = None;
                s.stroke = None;
                s.shape = None;
                s.font_size = None;
                s.bold = None;
            });
            ui.close_menu();
        }
    });
    ui.menu_button(tr(lang, "structure"), |ui| {
        for st in [
            Structure::LogicRight,
            Structure::LogicLeft,
            Structure::Map,
            Structure::OrgDown,
            Structure::OrgUp,
            Structure::Tree,
            Structure::Timeline,
            Structure::Fishbone,
            Structure::Matrix,
        ] {
            if ui.button(st.name_zh()).clicked() {
                ed.set_structure(&id, Some(st));
                ui.close_menu();
            }
        }
    });
    if !is_root {
        let sheet_struct = ed.doc.sheet().root.eff_structure(Structure::LogicRight);
        let l1 = ed.parent_info(&id).map(|(p, _)| p) == Some(root_id);
        if sheet_struct == Structure::Map && l1 {
            ui.menu_button("方向", |ui| {
                if ui.button(tr(lang, "side_left")).clicked() {
                    ed.set_side(&id, Some(Side::Left));
                    ui.close_menu();
                }
                if ui.button(tr(lang, "side_right")).clicked() {
                    ed.set_side(&id, Some(Side::Right));
                    ui.close_menu();
                }
            });
        }
    }
    ui.separator();
    if ui.add_enabled(!is_root, egui::Button::new(format!("{} (Del)", tr(lang, "delete")))).clicked() {
        ed.delete_selected();
        ui.close_menu();
    }
    ui.separator();
    let expanded = ed.topic(&id).map(|t| t.expanded).unwrap_or(true);
    if ui.button(if expanded { "收起" } else { "展开" }).clicked() {
        ed.toggle_expand(&id);
        ui.close_menu();
    }
    if ui.button(tr(lang, "drill")).clicked() {
        ed.drill.push(id.clone());
        ed.scene_ver += 1;
        ed.view = crate::editor::View::new();
        ui.close_menu();
    }
}

fn marker_label(m: &Marker) -> String {
    match m {
        Marker::Priority(p) => format!("P{p}"),
        Marker::Task(t) => format!("进度 {}%", *t as i32),
        Marker::Flag(c) => format!("旗帜 {:?}", c),
        Marker::Star(c) => format!("星 {:?}", c),
        Marker::People(c) => format!("人 {:?}", c),
        Marker::Heart(_) => "心".into(),
        Marker::Face(f) => format!("{f:?}"),
        Marker::Symbol(s) => format!("{s:?}"),
    }
}
