//! System CJK font discovery + egui font installation.
//! Mirrors xmind-core's FontBook logic so screen and export use the same face.

use egui::{Context, FontData, FontDefinitions, FontFamily};
use std::borrow::Cow;

pub struct SysFont {
    pub family: String,
    pub bytes: Vec<u8>,
    pub index: u32,
}

const CANDIDATES: &[&str] = &[
    "Hiragino Sans GB", // on macOS this one reliably outlines with ab_glyph
    "PingFang SC",
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

pub fn find_cjk_font() -> Option<SysFont> {
    let mut db = fontdb_lite::Database::new();
    db.load_system_fonts();
    for want in CANDIDATES {
        for face in db.faces() {
            let hit = face.families.iter().any(|(name, _)| name == want);
            if !hit {
                continue;
            }
            let path = match &face.source {
                fontdb_lite::Source::File(p) => Some(p.clone()),
                #[allow(unreachable_patterns)]
                fontdb_lite::Source::SharedFile(p, _) => Some(p.clone()),
                _ => None,
            };
            let Some(p) = path else { continue };
            let Ok(bytes) = std::fs::read(&p) else { continue };
            // validate: must produce outlines (protected asset-cache fonts don't)
            if ab_glyph::FontVec::try_from_vec_and_index(bytes.clone(), face.index).map(|f| can_outline(&f)).unwrap_or(false) {
                return Some(SysFont { family: want.to_string(), bytes, index: face.index });
            }
        }
    }
    None
}

fn can_outline(f: &ab_glyph::FontVec) -> bool {
    use ab_glyph::{Font, PxScale, ScaleFont};
    for ch in ['A', '你'] {
        let mut g = f.as_scaled(PxScale::from(16.0)).scaled_glyph(ch);
        g.position = ab_glyph::point(0.0, 20.0);
        if f.outline_glyph(g).is_none() {
            return false;
        }
    }
    true
}

/// Install the discovered CJK font as a fallback for proportional + monospace.
pub fn install(ctx: &Context) -> Option<String> {
    let sf = find_cjk_font()?;
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("sys-cjk".into(), FontData { font: Cow::Owned(sf.bytes), index: sf.index, tweak: Default::default() });
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push("sys-cjk".into());
    }
    ctx.set_fonts(fonts);
    Some(sf.family)
}

/// A tiny re-export shim so we don't duplicate the fontdb dependency logic.
mod fontdb_lite {
    pub use fontdb::*;
}
