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

/// Descent/em of the installed system font. egui's galley bottom edge sits
/// `descent` below the glyph baseline, so painting with a bottom anchor must
/// shift down by this much to land on the true baseline.
pub fn descent_ratio() -> f32 {
    static R: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *R.get_or_init(|| {
        find_cjk_font()
            .and_then(|sf| {
                use ab_glyph::Font as _;
                ab_glyph::FontVec::try_from_vec_and_index(sf.bytes, sf.index).ok().map(|f| {
                    let upm = f.units_per_em().unwrap_or(1000.0);
                    (f.descent_unscaled() / upm).abs().clamp(0.0, 0.5)
                })
            })
            .unwrap_or(0.12)
    })
}

/// Install the discovered CJK font as a fallback for proportional + monospace.
pub fn install(ctx: &Context) -> Option<String> {
    let sf = find_cjk_font()?;
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("sys-cjk".into(), FontData { font: Cow::Owned(sf.bytes), index: sf.index, tweak: Default::default() });
    // Put the system font FIRST in Proportional so Latin+CJK both come from
    // it (matches the headless export renderer). egui's font fallback for
    // proportional text doesn't reliably reach the appended CJK font, but
    // using it as the primary works.
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "sys-cjk".into());
    fonts.families.entry(FontFamily::Monospace).or_default().push("sys-cjk".into());
    ctx.set_fonts(fonts);
    Some(sf.family)
}

/// A tiny re-export shim so we don't duplicate the fontdb dependency logic.
mod fontdb_lite {
    pub use fontdb::*;
}
