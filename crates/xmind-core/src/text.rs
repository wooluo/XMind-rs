/// Font-metric abstraction so layout is identical on screen (egui) and in
/// headless exporters (ab_glyph).
pub trait TextMeasurer {
    /// Width of a single line of text at the given font size.
    fn line_width(&self, text: &str, size: f32, bold: bool) -> f32;
}

/// Line height factor shared by every renderer so vertical rhythm matches.
pub const LINE_H: f32 = 1.42;

pub fn line_height(size: f32) -> f32 {
    size * LINE_H
}

/// Fixed-width mock measurer (used by tests and as last-resort fallback).
pub struct MockMeasurer {
    pub char_w: f32,
}

impl Default for MockMeasurer {
    fn default() -> Self {
        MockMeasurer { char_w: 0.62 }
    }
}

impl TextMeasurer for MockMeasurer {
    fn line_width(&self, text: &str, size: f32, _bold: bool) -> f32 {
        text.chars().count() as f32 * self.char_w * size
    }
}

pub fn is_cjk_char(ch: char) -> bool {
    matches!(ch as u32,
        0x2E80..=0x9FFF | 0x3000..=0x303F | 0xFF00..=0xFFEF
        | 0xAC00..=0xD7AF | 0x2000..=0x206F)
}

/// Split text into wrapping units: CJK chars stand alone; latin words stay
/// together.
fn units(par: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut cur_cjk = false;
    for ch in par.chars() {
        let is_ws = ch.is_whitespace();
        let is_cjk = matches!(ch as u32,
            0x2E80..=0x9FFF | 0x3000..=0x303F | 0xFF00..=0xFFEF | 0x3040..=0x30FF
            | 0xAC00..=0xD7AF | 0x2000..=0x206F /* general punct */);
        if is_ws {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur_cjk = false;
            continue;
        }
        if cur.is_empty() {
            cur.push(ch);
            cur_cjk = is_cjk;
        } else if is_cjk || cur_cjk {
            // CJK breaks between every char
            out.push(std::mem::take(&mut cur));
            cur.push(ch);
            cur_cjk = is_cjk;
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Greedy word wrap, shared by all measurers.
pub fn wrap_text(m: &dyn TextMeasurer, text: &str, size: f32, bold: bool, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for par in text.split('\n') {
        if par.trim().is_empty() {
            lines.push(String::new());
            continue;
        }
        let us = units(par);
        let mut line = String::new();
        let mut line_w = 0.0;
        for u in us {
            let uw = m.line_width(&u, size, bold);
            let needs_space = !line.is_empty() && {
                let a = line.chars().last().unwrap();
                let b = u.chars().next().unwrap();
                !(is_cjk_char(a) && is_cjk_char(b))
            };
            let sep_w = if needs_space { m.line_width(" ", size, bold) } else { 0.0 };
            if !line.is_empty() && line_w + sep_w + uw > max_w {
                lines.push(line.clone());
                line.clear();
                line_w = 0.0;
                line.push_str(&u);
                line_w += uw;
            } else {
                if needs_space {
                    line.push(' ');
                    line_w += sep_w;
                }
                line.push_str(&u);
                line_w += uw;
            }
        }
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_cjk_and_latin() {
        let m = MockMeasurer { char_w: 0.5 };
        // 10 chars * 0.5 * 10px = 50px; max 25 => 5 chars/line
        let lines = wrap_text(&m, "一二三四五六七八九十", 10.0, false, 25.0);
        assert_eq!(lines.len(), 2);
        let lines = wrap_text(&m, "hello world foo", 10.0, false, 60.0);
        assert_eq!(lines, vec!["hello world", "foo"]);
    }
}
