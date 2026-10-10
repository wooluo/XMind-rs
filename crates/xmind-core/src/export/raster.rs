//! Headless raster rendering (PNG) and the shared font machinery used by all
//! exporters. Text is rendered through ab_glyph with a system CJK font so the
//! result matches the GUI closely.

use crate::col::Col;
use crate::model::Doc;
use crate::scene::{Prim, Scene};
use crate::text::TextMeasurer;
use crate::XRResult;
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use tiny_skia::{
    FillRule, Paint, PathBuilder, Path as TSPath, Pixmap, Shader, Stroke, Transform,
};

pub struct FontBook {
    pub family: String,
    font: Option<FontVec>,
}

impl FontBook {
    /// Discover a system font, preferring CJK-capable families.
    /// Fonts that load but cannot produce outlines (e.g. some protected
    /// macOS asset-cache fonts) are rejected automatically.
    pub fn load() -> FontBook {
        let candidates = [
            "PingFang SC",
            "Hiragino Sans GB",
            "Microsoft YaHei",
            "Noto Sans CJK SC",
            "Noto Sans SC",
            "Source Han Sans SC",
            "WenQuanYi Micro Hei",
            "Arial Unicode MS",
            "Segoe UI",
            "Helvetica Neue",
            "DejaVu Sans",
        ];
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        // pass 1: require CJK + latin outlines; pass 2: latin only
        for require_cjk in [true, false] {
            for want in candidates {
                for face in db.faces() {
                    let hit = face.families.iter().any(|(name, _)| name == want);
                    if !hit {
                        continue;
                    }
                    let path = match &face.source {
                        fontdb::Source::File(p) => Some(p.clone()),
                        #[allow(unreachable_patterns)]
                        fontdb::Source::SharedFile(p, _) => Some(p.clone()),
                        _ => None,
                    };
                    let Some(p) = path else { continue };
                    let Ok(bytes) = std::fs::read(&p) else { continue };
                    let Ok(f) = FontVec::try_from_vec_and_index(bytes, face.index) else { continue };
                    let can = |ch: char| {
                        let mut g = f.as_scaled(PxScale::from(16.0)).scaled_glyph(ch);
                        g.position = ab_glyph::point(0.0, 20.0);
                        f.outline_glyph(g).is_some()
                    };
                    let ok = can('A') && (!require_cjk || can('你'));
                    if ok {
                        return FontBook { family: want.to_string(), font: Some(f) };
                    }
                }
            }
        }
        FontBook { family: String::new(), font: None }
    }

    pub fn available(&self) -> bool {
        self.font.is_some()
    }
}

impl TextMeasurer for FontBook {
    fn line_width(&self, text: &str, size: f32, _bold: bool) -> f32 {
        let Some(font) = &self.font else {
            return text.chars().count() as f32 * 0.6 * size;
        };
        let scaled = font.as_scaled(PxScale::from(size));
        text.chars().map(|ch| scaled.h_advance(font.glyph_id(ch))).sum::<f32>() + 1.0
    }
}

const MAX_SIDE: u32 = 12000;

pub fn render_png(scene: &Scene, doc: &Doc, scale: f32) -> XRResult<Vec<u8>> {
    let book = FontBook::load();
    render_png_with(scene, doc, scale, &book)
}

pub fn render_png_with(scene: &Scene, doc: &Doc, scale: f32, book: &FontBook) -> XRResult<Vec<u8>> {
    let b = scene.bounds;
    let mut w = (b.w * scale).ceil().max(8.0) as u32;
    let mut h = (b.h * scale).max(8.0).ceil() as u32;
    let s = if w > MAX_SIDE || h > MAX_SIDE {
        let s = MAX_SIDE as f32 / w.max(h) as f32;
        w = (w as f32 * s) as u32;
        h = (h as f32 * s) as u32;
        scale * s
    } else {
        scale
    };
    let mut pm = Pixmap::new(w, h).ok_or_else(|| crate::XRErr::Other("画布过大".into()))?;
    // background
    pm.fill(tcol(scene.bg));
    draw_prims(&mut pm, &scene.prims, doc, book, s, b.x, b.y);
    pm.encode_png().map_err(|e| crate::XRErr::Other(format!("png encode: {e}")))
}

fn tcol(c: Col) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(c.0, c.1, c.2, c.3)
}

fn tpaint(c: Col) -> Paint<'static> {
    let mut p = Paint::default();
    p.shader = Shader::SolidColor(tcol(c));
    p.anti_alias = true;
    p
}

fn tstroke(w: f32) -> Stroke {
    Stroke {
        width: w.max(0.5),
        ..Default::default()
    }
}

fn draw_prims(
    pm: &mut Pixmap,
    prims: &[Prim],
    doc: &Doc,
    book: &FontBook,
    scale: f32,
    ox: f32,
    oy: f32,
) {
    let tr = |x: f32, y: f32| (x - ox, y - oy);
    for p in prims {
        match p {
            Prim::Rect { r, rounding, fill, stroke } => {
                let (x, y) = tr(r.x, r.y);
                if let Some(path) = rounded_rect(x * scale, y * scale, r.w * scale, r.h * scale, rounding * scale) {
                    paint_path(pm, &path, fill, stroke, scale);
                }
            }
            Prim::Poly { pts, closed, fill, stroke } => {
                let mut pb = PathBuilder::new();
                for (i, pt) in pts.iter().enumerate() {
                    let (x, y) = tr(pt.x, pt.y);
                    if i == 0 {
                        pb.move_to(x * scale, y * scale);
                    } else {
                        pb.line_to(x * scale, y * scale);
                    }
                }
                if *closed {
                    pb.close();
                }
                if let Some(path) = pb.finish() {
                    paint_path(pm, &path, fill, stroke, scale);
                }
            }
            Prim::Circle { c, r, fill, stroke } => {
                let (cx, cy) = tr(c.x, c.y);
                if let Some(path) = PathBuilder::from_circle(cx * scale, cy * scale, r * scale) {
                    paint_path(pm, &path, fill, stroke, scale);
                }
            }
            Prim::Text { p, text, size, color, bold, .. } => {
                let (x, y) = tr(p.x, p.y);
                draw_text(pm, book, text, *size, *color, *bold, x * scale, y * scale, scale);
            }
            Prim::Tris { tris, color } => {
                let mut pb = PathBuilder::new();
                for [a, b, c] in tris {
                    let (ax, ay) = tr(a.x, a.y);
                    let (bx, by) = tr(b.x, b.y);
                    let (cx, cy) = tr(c.x, c.y);
                    pb.move_to(ax * scale, ay * scale);
                    pb.line_to(bx * scale, by * scale);
                    pb.line_to(cx * scale, cy * scale);
                    pb.close();
                }
                if let Some(path) = pb.finish() {
                    pm.fill_path(&path, &tpaint(*color), FillRule::Winding, Transform::identity(), None);
                }
            }
            Prim::Image { r, asset } => {
                if let Some(a) = doc.assets.get(asset) {
                    if let Ok(img) = image::load_from_memory(&a.data) {
                        let rgba = img.to_rgba8();
                        let (x, y) = tr(r.x, r.y);
                        blit(pm, &rgba, x * scale, y * scale, r.w * scale, r.h * scale);
                    }
                }
            }
        }
    }
}

fn paint_path(pm: &mut Pixmap, path: &TSPath, fill: &Option<Col>, stroke: &Option<(f32, Col)>, scale: f32) {
    if let Some(c) = fill {
        pm.fill_path(path, &tpaint(*c), FillRule::Winding, Transform::identity(), None);
    }
    if let Some((w, c)) = stroke {
        pm.stroke_path(path, &tpaint(*c), &tstroke(w * scale), Transform::identity(), None);
    }
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<TSPath> {
    let r = r.clamp(0.0, w.min(h) / 2.0);
    let mut pb = PathBuilder::new();
    if r <= 0.01 {
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
        pb.close();
        return pb.finish();
    }
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

fn draw_text(
    pm: &mut Pixmap,
    book: &FontBook,
    text: &str,
    size: f32,
    color: Col,
    bold: bool,
    x: f32,
    baseline_y: f32,
    scale: f32,
) {
    let Some(font) = &book.font else { return };
    let scaled = font.as_scaled(PxScale::from(size * scale));
    let mut pen_x = x;
    for ch in text.chars() {
        let mut glyph = scaled.scaled_glyph(ch);
        glyph.position = ab_glyph::point(pen_x, baseline_y);
        if let Some(og) = font.outline_glyph(glyph) {
            // px_bounds() is absolute (glyph.position included)
            let bounds = og.px_bounds();
            let off_x = bounds.min.x as f32;
            let off_y = bounds.min.y as f32;
            og.draw(|gx, gy, cov| {
                if cov <= 0.003 {
                    return;
                }
                let px = off_x.floor() as i32 + gx as i32;
                let py = off_y.floor() as i32 + gy as i32;
                blend_pixel(pm, px, py, color, cov.min(1.0));
                if bold && cov > 0.3 {
                    blend_pixel(pm, px + 1, py, color, cov.min(1.0) * 0.7);
                }
            });
        }
        pen_x += scaled.h_advance(scaled.scaled_glyph(ch).id);
    }
}

fn blend_pixel(pm: &mut Pixmap, x: i32, y: i32, color: Col, cov: f32) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    if x < 0 || y < 0 || x >= w || y >= h {
        return;
    }
    let idx = (y as usize * pm.width() as usize + x as usize) * 4;
    let d = pm.data_mut();
    // premultiplied source-over
    let sa = cov * (color.3 as f32 / 255.0);
    let da = d[idx + 3] as f32 / 255.0;
    let f = |src: u8, dst: u8| {
        let s = src as f32 / 255.0 * sa;
        let o = s + dst as f32 / 255.0 * (1.0 - sa);
        (o * 255.0).round().min(255.0) as u8
    };
    d[idx] = f(color.0, d[idx]);
    d[idx + 1] = f(color.1, d[idx + 1]);
    d[idx + 2] = f(color.2, d[idx + 2]);
    let out_a = sa + da * (1.0 - sa);
    d[idx + 3] = (out_a * 255.0).round().min(255.0) as u8;
}

fn blit(pm: &mut Pixmap, img: &image::RgbaImage, x: f32, y: f32, w: f32, h: f32) {
    let (iw, ih) = img.dimensions();
    if iw == 0 || ih == 0 || w <= 0.0 || h <= 0.0 {
        return;
    }
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = (x + w).ceil() as i32;
    let y1 = (y + h).ceil() as i32;
    for py in y0..y1 {
        for px in x0..x1 {
            if px < 0 || py < 0 || px >= pm.width() as i32 || py >= pm.height() as i32 {
                continue;
            }
            let sx = ((px as f32 - x) / w * iw as f32) as u32 % iw;
            let sy = ((py as f32 - y) / h * ih as f32) as u32 % ih;
            let p = img.get_pixel(sx.min(iw - 1), sy.min(ih - 1));
            let c = Col(p.0[0], p.0[1], p.0[2], p.0[3]);
            blend_pixel(pm, px, py, c, 1.0);
        }
    }
}
