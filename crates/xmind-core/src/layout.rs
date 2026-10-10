use crate::col::{Col, WHITE};
use crate::geo::{cubic, cubic_tangent, dash_polyline, flatten_cubic, Rect, V2};
use crate::model::{Arrows, Sheet, Side, Structure, Topic};
use crate::scene::{HitRect, HitTarget, NodeSide, Prim, RelCurve, Scene};
use crate::text::{line_height, wrap_text, TextMeasurer};
use crate::theme::{self, Theme};
use std::collections::HashMap;

/// Build the renderable scene of one sheet.
/// `edit` = (topic_id, live_text) while the GUI is editing that topic, so the
/// layout reflows live.
pub fn build_scene(sheet: &Sheet, theme_id: &str, m: &dyn TextMeasurer, edit: Option<(&str, &str)>) -> Scene {
    build_scene_root(sheet, theme_id, m, edit, None)
}

/// Same, but rendering only the subtree of `root_id` (used by drill-down).
pub fn build_scene_root(
    sheet: &Sheet,
    theme_id: &str,
    m: &dyn TextMeasurer,
    edit: Option<(&str, &str)>,
    root_id: Option<&str>,
) -> Scene {
    let th = theme::theme_by_id(theme_id);
    let mut lx = Layout::new(th, m, edit);
    let drilling = root_id.is_some() && root_id != Some(sheet.root.id.as_str());
    let root_topic = match root_id.and_then(|id| sheet.root.find(id)) {
        Some(t) => t,
        None => &sheet.root,
    };
    let root_struct = root_topic.eff_structure(Structure::LogicRight);
    let root_branch = th.root_fill;
    lx.measure(root_topic, 0, root_struct, root_branch);
    lx.place(root_topic, 0.0, 0.0, 0, root_struct);
    // detached (floating) topics only belong to the real sheet root
    if !drilling {
        for (i, d) in sheet.root.detached.iter().enumerate() {
            let b = theme::branch_color(th, i);
            lx.measure(d, 1, d.eff_structure(Structure::LogicRight), b);
            let base = d
                .abs_pos
                .unwrap_or((320.0 + (i % 3) as f32 * 280.0, -60.0 + (i / 3) as f32 * 220.0));
            let bh = lx.blocks[&d.id].1;
            lx.place(d, base.0, base.1 - bh / 2.0, 1, Structure::LogicRight);
        }
    }
    // shift everything into positive space
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    for r in lx.rects.values() {
        min_x = min_x.min(r.x);
        min_y = min_y.min(r.y);
    }
    let (dx, dy) = (40.0 - min_x, 40.0 - min_y);
    if dx.is_finite() && dy.is_finite() {
        for r in lx.rects.values_mut() {
            r.x += dx;
            r.y += dy;
        }
    }
    let mut scene = Scene { bg: th.bg, ..Default::default() };
    scene.rects = lx.rects.clone();
    lx.sheet = Some(sheet);
    lx.draw_all(&mut scene, root_struct, root_branch);
    let mut b: Option<Rect> = None;
    for r in scene.rects.values() {
        b = Some(match b {
            Some(u) => u.union(r),
            None => *r,
        });
    }
    for c in &scene.rels {
        for p in [c.p0, c.p1] {
            let r = Rect::new(p.x, p.y, 0.0, 0.0);
            b = Some(match b {
                Some(u) => u.union(&r),
                None => r,
            });
        }
    }
    scene.bounds = b.map(|r| r.inflate(30.0)).unwrap_or(Rect::new(0.0, 0.0, 400.0, 300.0));
    scene
}

fn badge_rect(r: Rect, dir: NodeSide) -> Rect {
    match dir {
        NodeSide::Right => Rect::new(r.right() + 3.0, r.cy() - 8.0, 16.0, 16.0),
        NodeSide::Left => Rect::new(r.x - 19.0, r.cy() - 8.0, 16.0, 16.0),
        NodeSide::Down => Rect::new(r.cx() - 8.0, r.bottom() + 3.0, 16.0, 16.0),
        NodeSide::Up => Rect::new(r.cx() - 8.0, r.y - 19.0, 16.0, 16.0),
    }
}

struct Layout<'a> {
    th: &'static Theme,
    m: &'a dyn TextMeasurer,
    sizes: HashMap<String, (f32, f32)>,
    blocks: HashMap<String, (f32, f32)>,
    badge_dir: HashMap<String, NodeSide>,
    rects: HashMap<String, Rect>,
    boundary_hits: Vec<HitRect>,
    edit: Option<(&'a str, &'a str)>,
    sheet: Option<&'a Sheet>,
}

fn max_w(level: usize) -> f32 {
    match level {
        0 => 380.0,
        1 => 280.0,
        _ => 230.0,
    }
}

fn boxed_st(st: Structure) -> bool {
    matches!(
        st,
        Structure::OrgDown | Structure::OrgUp | Structure::Tree | Structure::Timeline | Structure::Fishbone | Structure::Matrix
    )
}

/// Unified content metrics so measure and draw can never disagree.
struct Content {
    lines: Vec<String>,
    lh: f32,
    text_w: f32,
    mk_size: f32,
    mk_w: f32,
    ic_n: bool,
    ic_l: bool,
    img_wh: Option<(f32, f32)>,
    labels_w: f32,
    has_labels: bool,
    px: f32,
    py: f32,
    inner_w: f32,
    inner_h: f32,
}

impl<'a> Layout<'a> {
    fn new(th: &'static Theme, m: &'a dyn TextMeasurer, edit: Option<(&'a str, &'a str)>) -> Self {
        Layout {
            th,
            m,
            sizes: HashMap::new(),
            blocks: HashMap::new(),
            badge_dir: HashMap::new(),
            rects: HashMap::new(),
            boundary_hits: Vec::new(),
            edit,
            sheet: None,
        }
    }

    fn text_of<'t>(&'t self, t: &'t Topic) -> &'t str {
        match self.edit {
            Some((id, live)) if *id == t.id => live,
            _ => &t.text,
        }
    }

    fn eff(&self, t: &Topic, cur: Structure) -> Structure {
        t.structure.unwrap_or(cur)
    }

    fn content_boxed(&self, t: &Topic, level: usize, branch: Col, boxed: bool) -> Content {
        let res = theme::resolve(self.th, level, branch, t.style.as_ref(), boxed);
        let text = self.text_of(t);
        let lines = wrap_text(self.m, text, res.size, res.bold, max_w(level));
        let lh = line_height(res.size);
        let text_w = lines
            .iter()
            .map(|l| self.m.line_width(l, res.size, res.bold))
            .fold(0.0f32, f32::max);
        let text_h = lines.len() as f32 * lh;

        let mk_size = (res.size * 1.15).clamp(14.0, 20.0);
        let mk_w = if t.markers.is_empty() {
            0.0
        } else {
            t.markers.len() as f32 * mk_size + (t.markers.len() - 1) as f32 * 4.0
        };
        let mk_gap = if t.markers.is_empty() { 0.0 } else { 8.0 };

        let ic_n = t.note.as_ref().map(|n| !n.trim().is_empty()).unwrap_or(false);
        let ic_l = t.link.is_some();
        let icons_w = (if ic_n { 20.0 } else { 0.0 }) + (if ic_l { 20.0 } else { 0.0 });
        let icons_gap = if icons_w > 0.0 { 6.0 } else { 0.0 };

        let img_wh = t.image.as_ref().map(|im| {
            let mut w = im.w.min(150.0);
            let mut h = im.h * (w / im.w.max(1.0));
            if h > 110.0 {
                h = 110.0;
                w = im.w * (h / im.h.max(1.0));
            }
            (w.max(8.0), h.max(8.0))
        });

        let lsize = (res.size - 3.0).max(10.0);
        let mut labels_w = 0.0;
        for (i, l) in t.labels.iter().enumerate() {
            if i > 0 {
                labels_w += 6.0;
            }
            labels_w += self.m.line_width(l, lsize, false) + 12.0;
        }
        let has_labels = !t.labels.is_empty();

        let row_w = mk_w + mk_gap + text_w + icons_gap + icons_w;
        let inner_w = row_w.max(labels_w).max(img_wh.map(|(w, _)| w).unwrap_or(0.0));
        let mut inner_h = text_h + if has_labels { 26.0 } else { 0.0 };
        if let Some((_, ih)) = img_wh {
            inner_h += ih + 4.0;
        }
        let (px, py) = if res.shape == crate::model::Shape::Underline {
            (6.0, 3.0)
        } else if level == 0 {
            (18.0, 12.0)
        } else {
            (12.0, 7.0)
        };
        Content {
            lines,
            lh,
            text_w,
            mk_size,
            mk_w,
            ic_n,
            ic_l,
            img_wh,
            labels_w,
            has_labels,
            px,
            py,
            inner_w,
            inner_h,
        }
    }

    fn box_size(&self, t: &Topic, level: usize, branch: Col, boxed: bool) -> (f32, f32) {
        let c = self.content_boxed(t, level, branch, boxed);
        (c.inner_w + c.px * 2.0, c.inner_h + c.py * 2.0)
    }

    // ---------- measuring ----------

    fn child_branch(&self, t_branch: Col, level: usize, i: usize) -> Col {
        if level == 0 {
            theme::branch_color(self.th, i)
        } else {
            t_branch
        }
    }

    fn measure(&mut self, t: &Topic, level: usize, st: Structure, branch: Col) {
        let (w, h) = self.box_size(t, level, branch, boxed_st(st));
        self.sizes.insert(t.id.clone(), (w, h));
        let has_visible = t.expanded && !t.children.is_empty();

        match st {
            Structure::LogicRight | Structure::LogicLeft | Structure::Map => {
                let mut block_h = h;
                let mut block_w = w;
                if has_visible {
                    match st {
                        Structure::Map => {
                            let mut hr = 0.0f32;
                            let mut hl = 0.0f32;
                            let mut wr = 0.0f32;
                            let mut wl = 0.0f32;
                            let mut nr = 0usize;
                            let mut nl = 0usize;
                            for (i, c) in t.children.iter().enumerate() {
                                let cs = c.side.unwrap_or(if i % 2 == 0 { Side::Right } else { Side::Left });
                                let cb = self.child_branch(branch, level, i);
                                self.measure(c, level + 1, self.eff(c, Structure::LogicRight), cb);
                                let (bw, bh) = self.blocks[&c.id];
                                if cs == Side::Right {
                                    hr += bh;
                                    wr = wr.max(bw);
                                    nr += 1;
                                } else {
                                    hl += bh;
                                    wl = wl.max(bw);
                                    nl += 1;
                                }
                            }
                            hr += theme::vgap(level) * nr.saturating_sub(1) as f32;
                            hl += theme::vgap(level) * nl.saturating_sub(1) as f32;
                            block_h = block_h.max(hr).max(hl);
                            block_w = wl
                                + (if wl > 0.0 { theme::hgap(level) } else { 0.0 })
                                + w
                                + (if wr > 0.0 { theme::hgap(level) } else { 0.0 })
                                + wr;
                        }
                        _ => {
                            let mut sum = 0.0f32;
                            let mut cw_max = 0.0f32;
                            for (i, c) in t.children.iter().enumerate() {
                                let cb = self.child_branch(branch, level, i);
                                self.measure(c, level + 1, self.eff(c, st), cb);
                                sum += self.blocks[&c.id].1;
                                cw_max = cw_max.max(self.blocks[&c.id].0);
                                if i + 1 < t.children.len() {
                                    sum += theme::vgap(level);
                                }
                            }
                            block_h = block_h.max(sum);
                            block_w = w + theme::hgap(level) + cw_max;
                        }
                    }
                    // summaries extend the block
                    let mut sw = 0.0f32;
                    for s in &t.summaries {
                        self.measure(&s.topic, (level + 2).max(2), Structure::LogicRight, Col::rgb(100, 116, 139));
                        sw = sw.max(theme::hgap(2) + 26.0 + self.blocks[&s.topic.id].0);
                    }
                    block_w += sw;
                }
                self.blocks.insert(t.id.clone(), (block_w, block_h));
            }
            Structure::OrgDown | Structure::OrgUp | Structure::Tree => {
                let mut block_w = w;
                let mut block_h = h;
                if has_visible {
                    let mut sum = 0.0;
                    let mut mh = 0.0f32;
                    for (i, c) in t.children.iter().enumerate() {
                        let cb = self.child_branch(branch, level, i);
                        self.measure(c, level + 1, self.eff(c, st), cb);
                        sum += self.blocks[&c.id].0;
                        if i + 1 < t.children.len() {
                            sum += 28.0;
                        }
                        mh = mh.max(self.blocks[&c.id].1);
                    }
                    block_w = block_w.max(sum);
                    block_h = h + 36.0 + mh;
                }
                self.blocks.insert(t.id.clone(), (block_w, block_h));
            }
            Structure::Timeline => {
                let mut block_w = w + 60.0;
                let mut block_h = h;
                if has_visible {
                    for (i, c) in t.children.iter().enumerate() {
                        let cb = self.child_branch(branch, level, i);
                        self.measure(c, level + 1, self.eff(c, Structure::OrgDown), cb);
                        block_w += self.blocks[&c.id].0 + 44.0;
                        block_h = block_h.max(self.blocks[&c.id].1);
                    }
                }
                self.blocks.insert(t.id.clone(), (block_w, block_h));
            }
            Structure::Fishbone => {
                let mut block_w = w + 260.0;
                let mut block_h = h.max(200.0);
                if has_visible {
                    let n = t.children.len();
                    let mut max_h = 0.0f32;
                    for (i, c) in t.children.iter().enumerate() {
                        let cb = self.child_branch(branch, level, i);
                        self.measure(c, 1, self.eff(c, Structure::LogicRight), cb);
                        max_h = max_h.max(self.blocks[&c.id].1);
                    }
                    block_w = w + 60.0 + n.div_ceil(2) as f32 * 170.0 + 140.0;
                    block_h = (140.0 + 40.0 + max_h).max(h) * 2.0 + 40.0;
                }
                self.blocks.insert(t.id.clone(), (block_w, block_h));
            }
            Structure::Matrix => {
                let mut block_w = w;
                let mut block_h = h;
                if has_visible {
                    for (i, c) in t.children.iter().enumerate() {
                        self.measure_deep_boxes(c, 1, self.child_branch(branch, level, i));
                    }
                    let (hw, cw, rh) = self.matrix_metrics(t);
                    block_w = hw + 16.0 + cw.iter().sum::<f32>() + cw.len() as f32 * 10.0;
                    block_h = h + 40.0 + rh.iter().sum::<f32>() + rh.len() as f32 * 10.0;
                }
                self.blocks.insert(t.id.clone(), (block_w, block_h));
            }
        }
    }

    /// Measure a topic and every descendant as standalone boxes (matrix rows).
    fn measure_deep_boxes(&mut self, t: &Topic, level: usize, branch: Col) {
        let (w, h) = self.box_size(t, level, branch, true);
        self.sizes.insert(t.id.clone(), (w, h));
        self.blocks.insert(t.id.clone(), (w, h));
        for (i, c) in t.children.iter().enumerate() {
            self.measure_deep_boxes(c, level + 1, self.child_branch(branch, level, i));
        }
    }

    fn matrix_metrics(&self, t: &Topic) -> (f32, Vec<f32>, Vec<f32>) {
        let cols = t.children.iter().map(|c| c.children.len()).max().unwrap_or(0);
        let mut header_w = 0.0f32;
        for c in &t.children {
            header_w = header_w.max(self.sizes[&c.id].0);
        }
        let mut col_w = vec![90.0f32; cols];
        for c in &t.children {
            for (j, g) in c.children.iter().enumerate() {
                col_w[j] = col_w[j].max(self.sizes[&g.id].0 + 16.0);
            }
        }
        let rows: Vec<f32> = t
            .children
            .iter()
            .map(|c| {
                let mut rh = self.sizes[&c.id].1;
                for g in &c.children {
                    // include deep stacks
                    let mut dh = self.sizes[&g.id].1;
                    for d in &g.children {
                        dh += self.sizes[&d.id].1 + 2.0;
                    }
                    rh = rh.max(dh + 12.0);
                }
                rh
            })
            .collect();
        (header_w, col_w, rows)
    }

    // ---------- placement ----------

    fn set_rect(&mut self, t: &Topic, x: f32, y: f32) {
        let (w, h) = self.sizes[&t.id];
        self.rects.insert(t.id.clone(), Rect::new(x, y, w, h));
    }

    fn place(&mut self, t: &Topic, x: f32, y: f32, level: usize, st: Structure) {
        match t.structure.unwrap_or(st) {
            Structure::LogicRight => self.place_h(t, x, y, level, Side::Right),
            Structure::LogicLeft => self.place_h(t, x, y, level, Side::Left),
            Structure::Map => self.place_map(t, x, y),
            Structure::OrgDown | Structure::Tree => self.place_v(t, x, y, level, st, false),
            Structure::OrgUp => self.place_v(t, x, y, level, st, true),
            Structure::Timeline => self.place_timeline(t, x, y),
            Structure::Fishbone => self.place_fishbone(t, x, y),
            Structure::Matrix => self.place_matrix(t, x, y),
        }
    }

    fn place_h(&mut self, t: &Topic, x: f32, y: f32, level: usize, side: Side) {
        let (ox, oy) = t.offset.unwrap_or((0.0, 0.0));
        let (x, y) = (x + ox, y + oy);
        let (w, h) = self.sizes[&t.id];
        let bh = self.blocks[&t.id].1;
        let own_y = y + (bh - h) / 2.0;
        if !t.children.is_empty() {
            self.badge_dir.insert(
                t.id.clone(),
                if side == Side::Right { NodeSide::Right } else { NodeSide::Left },
            );
        }
        if t.expanded && !t.children.is_empty() {
            let mut cy = y;
            if side == Side::Right {
                let cx = x + w + theme::hgap(level);
                for c in t.children.iter() {
                    self.place(c, cx, cy, level + 1, Structure::LogicRight);
                    cy += self.blocks[&c.id].1 + theme::vgap(level);
                }
            } else {
                // children to the left; frontier = right edge of each child block
                let mut cx = x - theme::hgap(level);
                for c in t.children.iter() {
                    let bw = self.blocks[&c.id].0;
                    cx -= bw;
                    self.place(c, cx + bw, cy, level + 1, Structure::LogicLeft);
                    cx -= theme::vgap(level);
                    cy += self.blocks[&c.id].1 + theme::vgap(level);
                }
            }
            self.place_summaries(t, level, side);
        }
        let own_x = if side == Side::Right { x } else { x - w };
        self.set_rect(t, own_x, own_y);
    }

    fn place_summaries(&mut self, t: &Topic, level: usize, side: Side) {
        if t.summaries.is_empty() {
            return;
        }
        for s in &t.summaries {
            let end = s.end.min(t.children.len().saturating_sub(1));
            let start = s.start.min(end);
            if start > end {
                continue;
            }
            let mut bb: Option<Rect> = None;
            for c in &t.children[start..=end] {
                if let Some(r) = self.rects.get(&c.id) {
                    bb = Some(match bb {
                        Some(u) => u.union(r),
                        None => *r,
                    });
                }
            }
            let bbox = bb.unwrap_or_else(|| Rect::new(0.0, 0.0, 10.0, 10.0));
            let ymid = bbox.cy();
            let (sw, sh) = self.sizes[&s.topic.id];
            let sb = self.blocks[&s.topic.id];
            let (sx, children_edge) = if side == Side::Right {
                (bbox.right() + 26.0, bbox.right() + 26.0 + sw + theme::hgap(2))
            } else {
                (bbox.x - 26.0 - sw, bbox.x - 26.0 - sw - theme::hgap(2))
            };
            self.rects.insert(format!("sum-{}", s.id), bbox);
            self.set_rect(&s.topic, sx, ymid - sh / 2.0);
            self.place_children_h(&s.topic, children_edge, ymid - sb.1 / 2.0, (level + 2).max(2), side);
        }
    }

    fn place_children_h(&mut self, t: &Topic, edge: f32, y: f32, level: usize, side: Side) {
        let mut cy = y;
        if side == Side::Right {
            let cx = edge;
            for c in t.children.iter() {
                self.place(c, cx, cy, level, Structure::LogicRight);
                cy += self.blocks[&c.id].1 + theme::vgap((level - 1).max(0));
            }
        } else {
            let mut cx = edge;
            for c in t.children.iter() {
                let bw = self.blocks[&c.id].0;
                cx -= bw;
                self.place(c, cx + bw, cy, level, Structure::LogicLeft);
                cx -= theme::vgap((level - 1).max(0));
                cy += self.blocks[&c.id].1 + theme::vgap((level - 1).max(0));
            }
        }
    }

    fn place_map(&mut self, t: &Topic, x: f32, y: f32) {
        let (ox, oy) = t.offset.unwrap_or((0.0, 0.0));
        let (x, y) = (x + ox, y + oy);
        let (w, h) = self.sizes[&t.id];
        let bh = self.blocks[&t.id].1;
        let own_y = y + (bh - h) / 2.0;
        if !t.children.is_empty() {
            self.badge_dir.insert(t.id.clone(), NodeSide::Right);
        }
        self.set_rect(t, x, own_y);
        if t.expanded && !t.children.is_empty() {
            let mut cy_r = y;
            let mut cy_l = y;
            let rx = x + w + theme::hgap(0);
            let lw = t
                .children
                .iter()
                .enumerate()
                .filter(|(i, c)| c.side.unwrap_or(if i % 2 == 0 { Side::Right } else { Side::Left }) == Side::Left)
                .map(|(_, c)| self.blocks[&c.id].0)
                .fold(0.0, f32::max);
            let mut cx_l = x - theme::hgap(0) - lw;
            for (i, c) in t.children.iter().enumerate() {
                let cs = c.side.unwrap_or(if i % 2 == 0 { Side::Right } else { Side::Left });
                if cs == Side::Right {
                    self.place(c, rx, cy_r, 1, Structure::LogicRight);
                    cy_r += self.blocks[&c.id].1 + theme::vgap(0);
                } else {
                    let bw = self.blocks[&c.id].0;
                    self.place(c, cx_l + bw, cy_l, 1, Structure::LogicLeft);
                    cx_l -= bw + theme::vgap(0);
                    cy_l += self.blocks[&c.id].1 + theme::vgap(0);
                }
            }
            self.place_summaries(t, 0, Side::Right);
        }
    }

    fn place_v(&mut self, t: &Topic, x: f32, y: f32, level: usize, st: Structure, up: bool) {
        let (ox, oy) = t.offset.unwrap_or((0.0, 0.0));
        let (x, y) = (x + ox, y + oy);
        let (w, h) = self.sizes[&t.id];
        let bw = self.blocks[&t.id].0;
        let own_x = x + (bw - w) / 2.0;
        if !t.children.is_empty() {
            self.badge_dir.insert(t.id.clone(), if up { NodeSide::Up } else { NodeSide::Down });
        }
        if t.expanded && !t.children.is_empty() {
            let mut cx = x;
            for c in t.children.iter() {
                let cst = self.eff(c, st);
                let (cbw, cbh) = self.blocks[&c.id];
                if up {
                    self.place(c, cx, y - 36.0 - cbh, level + 1, cst);
                } else {
                    self.place(c, cx, y + h + 36.0, level + 1, cst);
                }
                cx += cbw + 28.0;
            }
        }
        self.set_rect(t, own_x, y);
    }

    fn place_timeline(&mut self, t: &Topic, x: f32, y: f32) {
        let (ox, oy) = t.offset.unwrap_or((0.0, 0.0));
        let (x, y) = (x + ox, y + oy);
        let (w, h) = self.sizes[&t.id];
        let bh = self.blocks[&t.id].1;
        let own_y = y + (bh - h) / 2.0;
        if !t.children.is_empty() {
            self.badge_dir.insert(t.id.clone(), NodeSide::Right);
        }
        self.set_rect(t, x, own_y);
        if t.expanded && !t.children.is_empty() {
            let line_y = own_y + h / 2.0;
            let mut cx = x + w + 60.0;
            for (i, c) in t.children.iter().enumerate() {
                let (cw, ch) = self.sizes[&c.id];
                let cy = line_y - ch / 2.0;
                self.set_rect(c, cx, cy);
                self.badge_dir.insert(c.id.clone(), if i % 2 == 0 { NodeSide::Down } else { NodeSide::Up });
                if c.expanded && !c.children.is_empty() {
                    let below = i % 2 == 0;
                    let mut sy = if below { cy + ch + 14.0 } else { cy - 14.0 };
                    for g in &c.children {
                        let (gw, _gh) = self.sizes[&g.id];
                        let gbh = self.blocks[&g.id].1;
                        let gx = cx + (cw - gw).max(0.0) / 2.0;
                        if below {
                            self.place(g, gx, sy, 2, Structure::OrgDown);
                            sy += gbh + 8.0;
                        } else {
                            self.place(g, gx, sy - gbh, 2, Structure::OrgUp);
                            sy -= gbh + 8.0;
                        }
                    }
                }
                cx += self.blocks[&c.id].0 + 44.0;
            }
        }
    }

    fn place_fishbone(&mut self, t: &Topic, x: f32, y: f32) {
        let (w, h) = self.sizes[&t.id];
        let _ = w;
        let bh = self.blocks[&t.id].1;
        let line_y = y + bh / 2.0;
        let own_y = line_y - h / 2.0;
        let head_x = x + self.blocks[&t.id].0 - w;
        self.set_rect(t, head_x, own_y);
        if t.expanded && !t.children.is_empty() {
            for (i, c) in t.children.iter().enumerate() {
                let up = i % 2 == 0;
                let rib_x = x + 80.0 + (i / 2) as f32 * 170.0;
                let rib_end = V2::new(rib_x + 120.0, line_y + if up { -150.0 } else { 150.0 });
                let (cw, ch) = self.sizes[&c.id];
                self.set_rect(c, rib_end.x - cw / 2.0, rib_end.y + if up { -ch - 6.0 } else { 6.0 });
                let mut k = 0.35f32;
                for g in &c.children {
                    let gp = V2::new(rib_x + (rib_end.x - rib_x) * k, line_y + (rib_end.y - line_y) * k);
                    let (gw, gh) = self.sizes[&g.id];
                    let ox = 8.0;
                    let oy = if up { -gh - 12.0 } else { 12.0 };
                    self.rects.insert(g.id.clone(), Rect::new(gp.x + ox, gp.y + oy, gw, gh));
                    let mut dy = 0.0f32;
                    for d in &g.children {
                        let (dw, dh) = self.sizes[&d.id];
                        self.rects.insert(d.id.clone(), Rect::new(gp.x + ox + gw + 10.0, gp.y + oy + dy, dw, dh));
                        dy += dh + 3.0;
                    }
                    k = (k + 0.28).min(1.05);
                }
            }
        }
    }

    fn place_matrix(&mut self, t: &Topic, x: f32, y: f32) {
        let (w, h) = self.sizes[&t.id];
        let _ = w;
        self.set_rect(t, x, y);
        if !(t.expanded && !t.children.is_empty()) {
            return;
        }
        let (header_w, col_w, rows) = self.matrix_metrics(t);
        let mut ry = y + h + 40.0;
        for (i, c) in t.children.iter().enumerate() {
            let row_h = rows[i];
            let (cw, ch) = self.sizes[&c.id];
            self.set_rect(c, x, ry + (row_h - ch) / 2.0);
            let mut cx = x + header_w + 16.0;
            for (j, g) in c.children.iter().enumerate() {
                self.rects.insert(g.id.clone(), Rect::new(cx, ry, col_w[j] - 10.0, row_h));
                let mut dy = ry + 6.0;
                for d in &g.children {
                    let (dw, dh) = self.sizes[&d.id];
                    self.rects.insert(d.id.clone(), Rect::new(cx + 8.0, dy, (col_w[j] - 26.0).max(dw), dh));
                    dy += dh + 2.0;
                }
                cx += col_w[j] + 10.0;
            }
            ry += row_h + 10.0;
        }
    }

    // ---------- drawing ----------

    fn draw_all(&mut self, scene: &mut Scene, root_struct: Structure, root_branch: Col) {
        let sheet = self.sheet.expect("sheet");
        let root = &sheet.root;
        self.draw_structure_extras(scene, root, root_struct);
        self.draw_boundaries(scene, root, root_struct);
        self.draw_connectors(scene, root, root_struct, 0, root_branch, None);
        self.draw_topic(scene, root, 0, root_struct, root_branch, None);
        self.draw_relationships(scene, sheet);
        for (id, r) in &self.rects {
            if id.starts_with("sum-") {
                continue;
            }
            scene.hits.push(HitRect { rect: *r, target: HitTarget::Topic(id.clone()) });
        }
        for (id, dir) in &self.badge_dir {
            if let Some(r) = self.rects.get(id) {
                scene.badges.insert(id.clone(), badge_rect(*r, *dir));
            }
        }
        for (id, dir) in &self.badge_dir {
            if let Some(r) = self.rects.get(id) {
                scene.hits.push(HitRect {
                    rect: badge_rect(*r, *dir),
                    target: HitTarget::Badge(id.clone()),
                });
            }
        }
        for h in self.boundary_hits.drain(..) {
            scene.hits.push(h);
        }
    }

    fn draw_structure_extras(&mut self, scene: &mut Scene, t: &Topic, st: Structure) {
        match st {
            Structure::Timeline => {
                if t.expanded && !t.children.is_empty() {
                    let rr = self.rects[&t.id];
                    let last = self.rects[&t.children.last().unwrap().id];
                    let line_y = rr.cy();
                    let end_x = last.right() + 26.0;
                    let ac = accent(self.th);
                    scene.prims.push(Prim::Poly {
                        pts: vec![V2::new(rr.right() + 8.0, line_y), V2::new(end_x, line_y)],
                        closed: false,
                        fill: None,
                        stroke: Some((3.0, ac)),
                    });
                    scene.prims.push(Prim::Poly {
                        pts: vec![
                            V2::new(end_x, line_y),
                            V2::new(end_x - 10.0, line_y - 6.0),
                            V2::new(end_x - 10.0, line_y + 6.0),
                        ],
                        closed: true,
                        fill: Some(ac),
                        stroke: None,
                    });
                }
            }
            Structure::Fishbone => {
                let rr = self.rects[&t.id];
                let line_y = rr.cy();
                let x0 = (rr.x - 160.0).max(10.0);
                let ac = accent(self.th);
                scene.prims.push(Prim::Poly {
                    pts: vec![V2::new(x0, line_y), V2::new(rr.x - 6.0, line_y)],
                    closed: false,
                    fill: None,
                    stroke: Some((3.0, ac)),
                });
                if t.expanded {
                    for (i, c) in t.children.iter().enumerate() {
                        let up = i % 2 == 0;
                        let cr = self.rects[&c.id];
                        let rib_start = V2::new(cr.cx() - 120.0, line_y);
                        let rib_end = V2::new(cr.cx(), line_y + if up { -150.0 } else { 150.0 });
                        let bcol = if self.is_root(t) { theme::branch_color(self.th, i) } else { Col::rgb(120, 130, 145) };
                        scene.prims.push(Prim::Poly {
                            pts: vec![rib_start, rib_end],
                            closed: false,
                            fill: None,
                            stroke: Some((2.2, bcol)),
                        });
                    }
                }
            }
            _ => {}
        }
        for c in t.children.iter().chain(t.detached.iter()) {
            self.draw_structure_extras(scene, c, self.eff(c, st));
        }
    }

    fn is_root(&self, t: &Topic) -> bool {
        self.sheet.map(|s| s.root.id == t.id).unwrap_or(false)
    }

    fn draw_boundaries(&mut self, scene: &mut Scene, t: &Topic, st: Structure) {
        if !t.boundaries.is_empty() && t.expanded {
            for b in &t.boundaries {
                let end = b.end.min(t.children.len().saturating_sub(1));
                let start = b.start.min(end);
                if start > end {
                    continue;
                }
                let mut bb: Option<Rect> = None;
                for c in &t.children[start..=end] {
                    if let Some(r) = self.rects.get(&c.id) {
                        bb = Some(match bb {
                            Some(u) => u.union(r),
                            None => *r,
                        });
                    }
                }
                if let Some(bbox) = bb {
                    let color = b.color.unwrap_or(Col::rgb(99, 102, 241));
                    let area = bbox.inflate(12.0);
                    scene.prims.push(Prim::Rect {
                        r: area,
                        rounding: 14.0,
                        fill: Some(color.with_alpha(26)),
                        stroke: Some((1.4, color.with_alpha(150))),
                    });
                    if let Some(label) = &b.label {
                        if !label.trim().is_empty() {
                            let lw = self.m.line_width(label, 12.0, true) + 14.0;
                            let chip = Rect::new(area.x + 8.0, area.y - 9.0, lw, 18.0);
                            scene.prims.push(Prim::Rect { r: chip, rounding: 9.0, fill: Some(color), stroke: None });
                            scene.prims.push(Prim::Text {
                                p: V2::new(chip.cx(), chip.y + 12.5),
                                text: label.clone(),
                                size: 12.0,
                                color: color.text_on(),
                                bold: true,
                                italic: false,
                                center: true,
                            });
                        }
                    }
                    self.boundary_hits.push(HitRect {
                        rect: area,
                        target: HitTarget::Boundary { parent: t.id.clone(), id: b.id.clone() },
                    });
                }
            }
        }
        for c in t.children.iter().chain(t.detached.iter()) {
            self.draw_boundaries(scene, c, self.eff(c, st));
        }
    }

    fn draw_connectors(&mut self, scene: &mut Scene, t: &Topic, st: Structure, level: usize, branch: Col, side: Option<Side>) {
        if t.expanded {
            for (i, c) in t.children.iter().enumerate() {
                let cst = self.eff(c, st);
                let cb = self.child_branch(branch, level, i);
                let lw = theme::connector_width(level + 1);
                if !matches!(st, Structure::Timeline | Structure::Fishbone | Structure::Matrix) {
                    let pr = self.rects[&t.id];
                    let cr = self.rects[&c.id];
                    let go_left = matches!(cst, Structure::LogicLeft) || side == Some(Side::Left);
                    match cst {
                        Structure::LogicRight | Structure::LogicLeft | Structure::Map => {
                            let (p0, p1) = if go_left {
                                (V2::new(pr.x, pr.cy()), V2::new(cr.right(), cr.cy()))
                            } else {
                                (V2::new(pr.right(), pr.cy()), V2::new(cr.x, cr.cy()))
                            };
                            let mx = (p0.x + p1.x) / 2.0;
                            let col = cb;
                            let pts = flatten_cubic(p0, V2::new(mx, p0.y), V2::new(mx, p1.y), p1, 16);
                            scene.prims.push(Prim::Poly { pts, closed: false, fill: None, stroke: Some((lw, col)) });
                        }
                        Structure::OrgDown | Structure::Tree => {
                            let p0 = V2::new(pr.cx(), pr.bottom());
                            let p1 = V2::new(cr.cx(), cr.y);
                            let my = (p0.y + p1.y) / 2.0;
                            scene.prims.push(Prim::Poly {
                                pts: vec![p0, V2::new(p0.x, my), V2::new(p1.x, my), p1],
                                closed: false,
                                fill: None,
                                stroke: Some((lw.min(2.6), cb)),
                            });
                        }
                        Structure::OrgUp => {
                            let p0 = V2::new(pr.cx(), pr.y);
                            let p1 = V2::new(cr.cx(), cr.bottom());
                            let my = (p0.y + p1.y) / 2.0;
                            scene.prims.push(Prim::Poly {
                                pts: vec![p0, V2::new(p0.x, my), V2::new(p1.x, my), p1],
                                closed: false,
                                fill: None,
                                stroke: Some((lw.min(2.6), cb)),
                            });
                        }
                        _ => {}
                    }
                }
                let cs = if st == Structure::Map {
                    Some(c.side.unwrap_or(if i % 2 == 0 { Side::Right } else { Side::Left }))
                } else if matches!(cst, Structure::LogicLeft) {
                    Some(Side::Left)
                } else {
                    side
                };
                self.draw_connectors(scene, c, cst, level + 1, cb, cs);
            }
            // summary braces
            for s in &t.summaries {
                let end = s.end.min(t.children.len().saturating_sub(1));
                let start = s.start.min(end);
                if start > end {
                    continue;
                }
                if let Some(bbox) = self.rects.get(&format!("sum-{}", s.id)).copied() {
                    let grey = Col::rgb(100, 116, 139);
                    let sr = self.rects[&s.topic.id];
                    let left = sr.x < bbox.x;
                    let x = if left { bbox.x } else { bbox.right() };
                    let d = if left { -1.0 } else { 1.0 };
                    let (a, mid, b2) = (bbox.y, bbox.cy(), bbox.bottom());
                    let pts = vec![
                        V2::new(x, a),
                        V2::new(x + 10.0 * d, a),
                        V2::new(x + 10.0 * d, a + 14.0),
                        V2::new(x + 16.0 * d, mid - 8.0),
                        V2::new(x + 16.0 * d, mid),
                        V2::new(x + 16.0 * d, mid + 8.0),
                        V2::new(x + 10.0 * d, b2 - 14.0),
                        V2::new(x + 10.0 * d, b2),
                        V2::new(x, b2),
                    ];
                    scene.prims.push(Prim::Poly { pts, closed: false, fill: None, stroke: Some((2.0, grey)) });
                    let tip_x = if left { sr.right() } else { sr.x };
                    scene.prims.push(Prim::Poly {
                        pts: vec![V2::new(x + 16.0 * d, mid), V2::new(tip_x, sr.cy())],
                        closed: false,
                        fill: None,
                        stroke: Some((2.0, grey)),
                    });
                    let sside = if left { Some(Side::Left) } else { None };
                    self.draw_connectors(scene, &s.topic, Structure::LogicRight, (level + 2).max(2), grey, sside);
                }
            }
        }
        for d in t.detached.iter() {
            self.draw_connectors(scene, d, Structure::LogicRight, level + 1, branch, Some(Side::Right));
        }
    }

    fn draw_topic(
        &mut self,
        scene: &mut Scene,
        t: &Topic,
        level: usize,
        st: Structure,
        branch: Col,
        side: Option<Side>,
    ) {
        let governing = t.structure.unwrap_or(st);
        let res = theme::resolve(self.th, level, branch, t.style.as_ref(), boxed_st(governing));
        let c = self.content_boxed(t, level, branch, boxed_st(governing));
        let r = match self.rects.get(&t.id) {
            Some(r) => *r,
            None => return,
        };
        scene.mark_node_start(&t.id);
        // background shape
        match res.shape {
            crate::model::Shape::Ellipse => {
                let mut pts = Vec::new();
                for i in 0..=36 {
                    let a = i as f32 / 36.0 * std::f32::consts::TAU;
                    pts.push(V2::new(r.cx() + a.cos() * r.w / 2.0, r.cy() + a.sin() * r.h / 2.0));
                }
                scene.prims.push(Prim::Poly { pts, closed: true, fill: res.fill, stroke: res.stroke });
            }
            crate::model::Shape::Diamond => {
                scene.prims.push(Prim::Poly {
                    pts: vec![V2::new(r.cx(), r.y), V2::new(r.right(), r.cy()), V2::new(r.cx(), r.bottom()), V2::new(r.x, r.cy())],
                    closed: true,
                    fill: res.fill,
                    stroke: res.stroke,
                });
            }
            crate::model::Shape::Underline => {
                let uw = c.text_w.max(12.0).min(r.w);
                scene.prims.push(Prim::Poly {
                    pts: vec![V2::new(r.cx() - uw / 2.0, r.bottom()), V2::new(r.cx() + uw / 2.0, r.bottom())],
                    closed: false,
                    fill: None,
                    stroke: Some((2.0, branch)),
                });
            }
            _ => {
                let radius = if res.shape == crate::model::Shape::Capsule {
                    r.h / 2.0
                } else if res.shape == crate::model::Shape::Rect {
                    0.0
                } else {
                    res.radius
                };
                scene.prims.push(Prim::Rect { r, rounding: radius, fill: res.fill, stroke: res.stroke });
            }
        }
        if matches!(governing, Structure::Matrix) && level >= 2 {
            scene.prims.push(Prim::Rect { r, rounding: 4.0, fill: None, stroke: Some((1.0, Col::rgb(226, 232, 240))) });
        }
        // content
        let row_w = c.mk_w + (if c.mk_w > 0.0 { 8.0 } else { 0.0 }) + c.text_w
            + if c.ic_n || c.ic_l { 6.0 } else { 0.0 }
            + (if c.ic_n { 20.0 } else { 0.0 }) + (if c.ic_l { 20.0 } else { 0.0 });
        let start_x = r.cx() - row_w / 2.0;
        let start_y = r.y + c.py + c.img_wh.map(|(_, h)| h + 4.0).unwrap_or(0.0);
        let text_h = c.lines.len() as f32 * c.lh;
        if !t.markers.is_empty() {
            let my = start_y + text_h / 2.0;
            for (i, mk) in t.markers.iter().enumerate() {
                let center = V2::new(start_x + c.mk_size / 2.0 + i as f32 * (c.mk_size + 4.0), my);
                for p in mk.prims(center, c.mk_size) {
                    scene.prims.push(p);
                }
            }
        }
        let tx = start_x + c.mk_w + if c.mk_w > 0.0 { 8.0 } else { 0.0 };
        for (i, line) in c.lines.iter().enumerate() {
            scene.prims.push(Prim::Text {
                p: V2::new(tx, start_y + i as f32 * c.lh + res.size * 0.8),
                text: line.clone(),
                size: res.size,
                color: res.text,
                bold: res.bold,
                italic: res.italic,
                center: false,
            });
        }
        // icons
        let mut ix = tx + c.text_w + 6.0;
        let iy = start_y + text_h / 2.0;
        if c.ic_n {
            let b = Rect::new(ix, iy - 8.0, 14.0, 16.0);
            scene.prims.push(Prim::Rect { r: b, rounding: 3.0, fill: Some(WHITE), stroke: Some((1.2, Col::rgb(148, 163, 184))) });
            for k in 1..=3 {
                let y = b.y + 3.5 + k as f32 * 3.4;
                scene.prims.push(Prim::Poly {
                    pts: vec![V2::new(b.x + 3.0, y), V2::new(b.right() - 3.0, y)],
                    closed: false,
                    fill: None,
                    stroke: Some((1.1, Col::rgb(148, 163, 184))),
                });
            }
        }
        if c.ic_l {
            for dx in [0.0, 6.0] {
                scene.prims.push(Prim::Rect {
                    r: Rect::new(ix + dx, iy - 7.0, 10.0, 14.0),
                    rounding: 5.0,
                    fill: None,
                    stroke: Some((1.4, Col::rgb(59, 130, 246))),
                });
            }
        }
        // image
        if let Some(im) = &t.image {
            if let Some((iw, ih)) = c.img_wh {
                let ir = Rect::from_center(r.cx(), r.y + c.py + ih / 2.0, iw, ih);
                scene.prims.push(Prim::Image { r: ir, asset: im.asset.clone() });
            }
        }
        // labels
        if c.has_labels {
            let lsize = (res.size - 3.0).max(10.0);
            let mut lx = r.cx() - c.labels_w / 2.0;
            let ly = r.bottom() - c.py - 19.0;
            for (i, l) in t.labels.iter().enumerate() {
                if i > 0 {
                    lx += 6.0;
                }
                let w = self.m.line_width(l, lsize, false) + 12.0;
                let chip = Rect::new(lx, ly, w, 18.0);
                let chip_c = branch.lighten(0.55);
                scene.prims.push(Prim::Rect { r: chip, rounding: 8.0, fill: Some(chip_c), stroke: None });
                scene.prims.push(Prim::Text {
                    p: V2::new(chip.cx(), chip.cy() + lsize * 0.36),
                    text: l.clone(),
                    size: lsize,
                    color: branch.darken(0.55),
                    bold: false,
                    italic: false,
                    center: true,
                });
                lx += w;
            }
        }
        // collapse badge
        if let Some(dir) = self.badge_dir.get(&t.id).copied() {
            let br = badge_rect(r, dir);
            let center = V2::new(br.cx(), br.cy());
            let bcol = if level == 0 { accent(self.th) } else { branch };
            scene.prims.push(Prim::Circle { c: center, r: 7.5, fill: Some(WHITE), stroke: Some((1.5, bcol)) });
            let k = 3.4;
            scene.prims.push(Prim::Poly {
                pts: vec![V2::new(center.x - k, center.y), V2::new(center.x + k, center.y)],
                closed: false,
                fill: None,
                stroke: Some((1.5, bcol)),
            });
            if !t.expanded {
                scene.prims.push(Prim::Poly {
                    pts: vec![V2::new(center.x, center.y - k), V2::new(center.x, center.y + k)],
                    closed: false,
                    fill: None,
                    stroke: Some((1.5, bcol)),
                });
            }
        }
        scene.mark_node_end(&t.id);

        // recurse
        if t.expanded {
            for (i, ch) in t.children.iter().enumerate() {
                let cst = self.eff(ch, st);
                let cb = self.child_branch(branch, level, i);
                let cs = if st == Structure::Map {
                    Some(ch.side.unwrap_or(if i % 2 == 0 { Side::Right } else { Side::Left }))
                } else if side.is_some() {
                    side
                } else {
                    None
                };
                self.draw_topic(scene, ch, level + 1, cst, cb, cs);
            }
            for s in &t.summaries {
                self.draw_topic(scene, &s.topic, (level + 2).max(2), Structure::LogicRight, Col::rgb(100, 116, 139), None);
            }
        }
        for (i, d) in t.detached.iter().enumerate() {
            let db = self.child_branch(branch, level, i);
            self.draw_topic(scene, d, level + 1, Structure::LogicRight, db, None);
        }
    }

    fn draw_relationships(&mut self, scene: &mut Scene, sheet: &Sheet) {
        for rel in &sheet.relationships {
            let (ra, rb) = match (self.rects.get(&rel.a), self.rects.get(&rel.b)) {
                (Some(a), Some(b)) => (*a, *b),
                _ => continue,
            };
            let ca = V2::new(ra.cx(), ra.cy());
            let cb = V2::new(rb.cx(), rb.cy());
            let p0 = clip_to_rect(ca, cb, ra);
            let p1 = clip_to_rect(cb, ca, rb);
            let mx = (p0.x + p1.x) / 2.0;
            let c1 = V2::new(mx, p0.y);
            let c2 = V2::new(mx, p1.y);
            let color = rel.color.unwrap_or(Col::rgb(100, 116, 139));
            let stroke = (1.6, color);
            if rel.dashed {
                let pts = flatten_cubic(p0, c1, c2, p1, 40);
                for (a, b) in dash_polyline(&pts, 7.0, 5.0) {
                    scene.prims.push(Prim::Poly { pts: vec![a, b], closed: false, fill: None, stroke: Some(stroke) });
                }
            } else {
                let pts = flatten_cubic(p0, c1, c2, p1, 16);
                scene.prims.push(Prim::Poly { pts, closed: false, fill: None, stroke: Some(stroke) });
            }
            let tan_end = norm(cubic_tangent(p0, c1, c2, p1, 1.0));
            if matches!(rel.arrows, Arrows::End | Arrows::Both) {
                push_arrow(scene, p1, tan_end, color);
            }
            if matches!(rel.arrows, Arrows::Both) {
                let tan_start = norm(cubic_tangent(p0, c1, c2, p1, 0.0));
                push_arrow(scene, p0, V2::new(-tan_start.x, -tan_start.y), color);
            }
            if let Some(label) = &rel.label {
                if !label.trim().is_empty() {
                    let mid = cubic(p0, c1, c2, p1, 0.5);
                    let lw = self.m.line_width(label, 12.0, false) + 14.0;
                    let chip = Rect::from_center(mid.x, mid.y, lw, 20.0);
                    scene.prims.push(Prim::Rect { r: chip, rounding: 10.0, fill: Some(WHITE), stroke: Some((1.0, color)) });
                    scene.prims.push(Prim::Text {
                        p: V2::new(chip.cx(), chip.cy() + 4.2),
                        text: label.clone(),
                        size: 12.0,
                        color: Col::rgb(51, 65, 85),
                        bold: false,
                        italic: false,
                        center: true,
                    });
                }
            }
            scene.rels.push(RelCurve { id: rel.id.clone(), p0, c1, c2, p1 });
        }
    }
}

// helpers ----------------------------------------------------------------

fn norm(v: V2) -> V2 {
    let l = v.x.hypot(v.y).max(0.0001);
    V2::new(v.x / l, v.y / l)
}

fn accent(th: &Theme) -> Col {
    th.root_fill
}

fn push_arrow(scene: &mut Scene, tip: V2, dir: V2, col: Col) {
    let n = V2::new(-dir.y, dir.x);
    let a = V2::new(tip.x - dir.x * 10.0 + n.x * 4.5, tip.y - dir.y * 10.0 + n.y * 4.5);
    let b = V2::new(tip.x - dir.x * 10.0 - n.x * 4.5, tip.y - dir.y * 10.0 - n.y * 4.5);
    scene.prims.push(Prim::Poly { pts: vec![tip, a, b], closed: true, fill: Some(col), stroke: None });
}

fn clip_to_rect(from: V2, to: V2, r: Rect) -> V2 {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let mut t = f32::INFINITY;
    if dx > 0.0 {
        t = t.min((r.right() - from.x) / dx);
    } else if dx < 0.0 {
        t = t.min((r.x - from.x) / dx);
    }
    if dy > 0.0 {
        t = t.min((r.bottom() - from.y) / dy);
    } else if dy < 0.0 {
        t = t.min((r.y - from.y) / dy);
    }
    if !t.is_finite() {
        return from;
    }
    V2::new(from.x + dx * t, from.y + dy * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::MockMeasurer;

    fn sheet_tree(children: usize, depth: usize) -> Sheet {
        let mut sh = Sheet::new("s", "中心主题");
        fn add(t: &mut Topic, n: usize, depth: usize) {
            for i in 0..n {
                let mut c = Topic::new(format!("主题 {i}"));
                if depth > 1 {
                    add(&mut c, n, depth - 1);
                }
                t.insert_child(None, c);
            }
        }
        if depth > 0 {
            add(&mut sh.root, children, depth);
        }
        sh
    }

    #[test]
    fn logic_right_no_overlap() {
        for depth in 1..=3 {
            let sh = sheet_tree(3, depth);
            let scene = build_scene(&sh, "classic-blue", &MockMeasurer::default(), None);
            let rects: Vec<&Rect> = scene.rects.values().collect();
            for i in 0..rects.len() {
                for j in (i + 1)..rects.len() {
                    assert!(
                        !rects[i].intersect(rects[j]),
                        "overlap at depth {depth}: {:?} vs {:?}",
                        rects[i],
                        rects[j]
                    );
                }
            }
            assert!(scene.rects.contains_key(&sh.root.id));
        }
    }

    #[test]
    fn org_down_no_overlap() {
        let mut sh = sheet_tree(3, 3);
        sh.root.structure = Some(Structure::OrgDown);
        let scene = build_scene(&sh, "business", &MockMeasurer::default(), None);
        let rects: Vec<&Rect> = scene.rects.values().collect();
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                assert!(!rects[i].intersect(rects[j]), "org overlap: {:?} {:?}", rects[i], rects[j]);
            }
        }
    }

    #[test]
    fn map_and_timeline_render() {
        let mut sh = sheet_tree(4, 2);
        sh.root.structure = Some(Structure::Map);
        let s1 = build_scene(&sh, "classic-blue", &MockMeasurer::default(), None);
        assert_eq!(s1.rects.len(), 1 + 4 + 16);
        let mut sh2 = sheet_tree(3, 2);
        sh2.root.structure = Some(Structure::Timeline);
        let s2 = build_scene(&sh2, "vitality", &MockMeasurer::default(), None);
        assert!(s2.rects.len() >= 4);
    }
}
