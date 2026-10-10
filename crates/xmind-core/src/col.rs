/// Simple RGBA color, renderer independent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Col(pub u8, pub u8, pub u8, pub u8);

pub const WHITE: Col = Col(255, 255, 255, 255);
pub const BLACK: Col = Col(17, 24, 39, 255);
pub const TRANSPARENT: Col = Col(0, 0, 0, 0);

impl Col {
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Col(r, g, b, 255)
    }
    pub fn from_hex(s: &str) -> Option<Col> {
        let s = s.trim().trim_start_matches('#');
        if s.len() == 6 {
            let v = u32::from_str_radix(s, 16).ok()?;
            Some(Col(
                ((v >> 16) & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                (v & 0xff) as u8,
                255,
            ))
        } else if s.len() == 8 {
            let v = u64::from_str_radix(s, 16).ok()?;
            Some(Col(
                ((v >> 24) & 0xff) as u8,
                ((v >> 16) & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                (v & 0xff) as u8,
            ))
        } else {
            None
        }
    }
    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }
    pub fn with_alpha(&self, a: u8) -> Col {
        Col(self.0, self.1, self.2, a)
    }
    pub fn alpha_f(&self) -> f32 {
        self.3 as f32 / 255.0
    }
    /// Blend `self` onto `base` using self's alpha.
    pub fn blend_over(&self, base: Col) -> Col {
        let a = self.alpha_f();
        let f = |s: u8, b: u8| ((s as f32 * a + b as f32 * (1.0 - a)).round() as u8).min(255);
        Col(f(self.0, base.0), f(self.1, base.1), f(self.2, base.2), 255)
    }
    /// Multiply towards black (factor 0..1).
    pub fn darken(&self, f: f32) -> Col {
        Col(
            (self.0 as f32 * f) as u8,
            (self.1 as f32 * f) as u8,
            (self.2 as f32 * f) as u8,
            self.3,
        )
    }
    pub fn lighten(&self, f: f32) -> Col {
        let g = |c: u8| ((c as f32 + (255.0 - c as f32) * f).round() as u8).min(255);
        Col(g(self.0), g(self.1), g(self.2), self.3)
    }
    pub fn is_dark(&self) -> bool {
        let lum = 0.299 * self.0 as f32 + 0.587 * self.1 as f32 + 0.114 * self.2 as f32;
        lum < 140.0
    }
    /// Pick readable text color (black/white) for this background.
    pub fn text_on(&self) -> Col {
        if self.is_dark() { WHITE } else { BLACK }
    }
}
