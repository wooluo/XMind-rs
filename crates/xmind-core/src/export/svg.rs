use crate::model::Doc;
use crate::scene::{Prim, Scene};

const FONT_STACK: &str = "PingFang SC, Hiragino Sans GB, Microsoft YaHei, Noto Sans CJK SC, sans-serif";

pub fn render(scene: &Scene, doc: &Doc, _scale: f32) -> String {
    let b = scene.bounds;
    let mut w = b.w.max(8.0);
    let mut h = b.h.max(8.0);
    let (w, h) = {
        if w > 16000.0 || h > 16000.0 {
            let s = 16000.0 / w.max(h);
            w *= s;
            h *= s;
        }
        (w, h)
    };
    let mut out = String::with_capacity(64 * 1024);
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"0 0 {w:.0} {h:.0}\">\n"
    ));
    out.push_str(&format!(
        "<rect x=\"0\" y=\"0\" width=\"{w:.0}\" height=\"{h:.0}\" fill=\"{}\"/>\n",
        hex(scene.bg)
    ));
    for p in &scene.prims {
        write_prim(&mut out, p, b.x, b.y, doc);
    }
    out.push_str("</svg>\n");
    out
}

pub fn wrap_html(scene: &Scene, doc: &Doc, title: &str) -> String {
    let svg = render(scene, doc, 1.0);
    let esc = title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    format!(
        "<!DOCTYPE html>\n<html lang=\"zh\">\n<head>\n<meta charset=\"utf-8\">\n<title>{esc}</title>\n<style>body{{margin:0;background:#fff;display:flex;justify-content:center;}} svg{{max-width:100%;height:auto;}}</style>\n</head>\n<body>\n{svg}\n</body>\n</html>\n"
    )
}

fn hex(c: crate::col::Col) -> String {
    format!("#{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

fn rgba(c: crate::col::Col) -> String {
    if c.3 >= 255 {
        hex(c)
    } else {
        format!("rgba({},{},{},{:.2})", c.0, c.1, c.2, c.3 as f32 / 255.0)
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn write_prim(out: &mut String, p: &Prim, ox: f32, oy: f32, doc: &Doc) {
    match p {
        Prim::Rect { r, rounding, fill, stroke } => {
            let style = style_attr(fill, stroke, 1.0);
            out.push_str(&format!(
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"{:.1}\" {style}/>\n",
                r.x - ox,
                r.y - oy,
                r.w,
                r.h,
                rounding
            ));
        }
        Prim::Poly { pts, closed, fill, stroke } => {
            if pts.is_empty() {
                return;
            }
            let mut d = String::new();
            for (i, pt) in pts.iter().enumerate() {
                if i == 0 {
                    d.push_str(&format!("M{:.1} {:.1}", pt.x - ox, pt.y - oy));
                } else {
                    d.push_str(&format!(" L{:.1} {:.1}", pt.x - ox, pt.y - oy));
                }
            }
            if *closed {
                d.push_str(" Z");
            }
            out.push_str(&format!("<path d=\"{d}\" {} {style2}/>\n", "", style2 = style_attr(fill, stroke, 1.0)));
        }
        Prim::Circle { c, r, fill, stroke } => {
            out.push_str(&format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" {}/>\n",
                c.x - ox,
                c.y - oy,
                r,
                style_attr(fill, stroke, 1.0)
            ));
        }
        Prim::Text { p, text, size, color, bold, italic, center } => {
            let anchor = if *center { "middle" } else { "start" };
            let weight = if *bold { " font-weight=\"bold\"" } else { "" };
            let style = if *italic { " font-style=\"italic\"" } else { "" };
            out.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"{:.1}\" fill=\"{}\" font-family=\"{FONT_STACK}\" text-anchor=\"{anchor}\"{weight}{style}>{}</text>\n",
                p.x - ox,
                p.y - oy,
                size,
                hex(*color),
                esc(text)
            ));
        }
        Prim::Tris { tris, color } => {
            let mut d = String::new();
            for [a, b, c] in tris {
                d.push_str(&format!(
                    "M{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1} Z ",
                    a.x - ox, a.y - oy, b.x - ox, b.y - oy, c.x - ox, c.y - oy
                ));
            }
            out.push_str(&format!("<path d=\"{d}\" fill=\"{}\"/>\n", rgba(*color)));
        }
        Prim::Image { r, asset } => {
            if let Some(a) = doc.assets.get(asset) {
                let b64 = base64(&a.data);
                out.push_str(&format!(
                    "<image x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" href=\"data:image/{};base64,{b64}\"/>\n",
                    r.x - ox,
                    r.y - oy,
                    r.w,
                    r.h,
                    a.kind
                ));
            }
        }
    }
}

fn style_attr(fill: &Option<crate::col::Col>, stroke: &Option<(f32, crate::col::Col)>, _sw: f32) -> String {
    let mut s = String::new();
    match fill {
        Some(c) => s.push_str(&format!("fill=\"{}\" ", rgba(*c))),
        None => s.push_str("fill=\"none\" "),
    }
    if let Some((w, c)) = stroke {
        s.push_str(&format!("stroke=\"{}\" stroke-width=\"{:.2}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" ", rgba(*c), w));
    }
    s
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}
