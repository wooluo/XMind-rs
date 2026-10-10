use crate::col::{Col, WHITE};
use crate::geo::{star_pts, V2};
use crate::scene::Prim;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ColKey {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Black,
}

impl ColKey {
    pub fn color(&self) -> Col {
        match self {
            ColKey::Red => Col::rgb(239, 68, 68),
            ColKey::Orange => Col::rgb(249, 115, 22),
            ColKey::Yellow => Col::rgb(245, 158, 11),
            ColKey::Green => Col::rgb(34, 197, 94),
            ColKey::Blue => Col::rgb(59, 130, 246),
            ColKey::Purple => Col::rgb(139, 92, 246),
            ColKey::Black => Col::rgb(31, 41, 55),
        }
    }
    pub fn xmind_name(&self) -> &'static str {
        match self {
            ColKey::Red => "red",
            ColKey::Orange => "orange",
            ColKey::Yellow => "yellow",
            ColKey::Green => "green",
            ColKey::Blue => "blue",
            ColKey::Purple => "purple",
            ColKey::Black => "black",
        }
    }
    pub fn from_xmind(s: &str) -> Option<ColKey> {
        Some(match s {
            "red" => ColKey::Red,
            "orange" => ColKey::Orange,
            "yellow" => ColKey::Yellow,
            "green" => ColKey::Green,
            "blue" => ColKey::Blue,
            "purple" => ColKey::Purple,
            "black" | "gray" | "grey" => ColKey::Black,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Face {
    Laugh,
    Smile,
    Neutral,
    Sad,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Sym {
    Attention,
    Question,
    Idea,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Marker {
    Priority(u8), // 1..=9
    Task(f32),    // 0..=100 percent
    Flag(ColKey),
    Star(ColKey),
    People(ColKey),
    Heart(ColKey),
    Face(Face),
    Symbol(Sym),
}

pub fn priority_color(p: u8) -> Col {
    match p {
        1 => Col::rgb(239, 68, 68),
        2 => Col::rgb(249, 115, 22),
        3 => Col::rgb(245, 158, 11),
        4 => Col::rgb(234, 179, 8),
        5 => Col::rgb(132, 204, 22),
        6 => Col::rgb(34, 197, 94),
        7 => Col::rgb(59, 130, 246),
        8 => Col::rgb(139, 92, 246),
        _ => Col::rgb(107, 114, 128),
    }
}

impl Marker {
    /// Round-trip with XMind marker ids, best effort.
    pub fn to_xmind_id(&self) -> String {
        match self {
            Marker::Priority(p) => format!("priority-{p}"),
            Marker::Task(t) => {
                let n = (t / 12.5).round() as i32; // 0..8
                let s = match n {
                    0 => "task-start",
                    1 => "task-oct",
                    2 => "task-quarter",
                    3 => "task-3oct",
                    4 => "task-half",
                    5 => "task-5oct",
                    6 => "task-3quar",
                    7 => "task-7oct",
                    _ => "task-done",
                };
                s.to_string()
            }
            Marker::Flag(c) => format!("flag-{}", c.xmind_name()),
            Marker::Star(c) => format!("star-{}", c.xmind_name()),
            Marker::People(c) => format!("people-{}", c.xmind_name()),
            Marker::Heart(c) => format!("heart-{}", c.xmind_name()),
            Marker::Face(f) => match f {
                Face::Laugh => "symbol-laugh".into(),
                Face::Smile => "symbol-smile".into(),
                Face::Neutral => "symbol-poker-face".into(),
                Face::Sad => "symbol-cry".into(),
            },
            Marker::Symbol(s) => match s {
                Sym::Attention => "symbol-attention".into(),
                Sym::Question => "symbol-question".into(),
                Sym::Idea => "symbol-idea".into(),
            },
        }
    }

    pub fn from_xmind_id(id: &str) -> Option<Marker> {
        let id = id.trim();
        if let Some(n) = id.strip_prefix("priority-") {
            return n.parse::<u8>().ok().filter(|p| (1..=9).contains(p)).map(Marker::Priority);
        }
        let task = match id {
            "task-start" => Some(0.0),
            "task-oct" => Some(12.5),
            "task-quarter" => Some(25.0),
            "task-3oct" => Some(37.5),
            "task-half" => Some(50.0),
            "task-5oct" => Some(62.5),
            "task-3quar" => Some(75.0),
            "task-7oct" => Some(87.5),
            "task-done" | "task-full" => Some(100.0),
            _ => None,
        };
        if let Some(t) = task {
            return Some(Marker::Task(t));
        }
        let parts: Vec<&str> = id.splitn(2, '-').collect();
        if parts.len() == 2 {
            let name = parts[1];
            if let Some(c) = ColKey::from_xmind(name) {
                return match parts[0] {
                    "flag" => Some(Marker::Flag(c)),
                    "star" => Some(Marker::Star(c)),
                    "people" => Some(Marker::People(c)),
                    "heart" => Some(Marker::Heart(c)),
                    _ => None,
                };
            }
        }
        Some(match id {
            "symbol-laugh" => Marker::Face(Face::Laugh),
            "symbol-smile" => Marker::Face(Face::Smile),
            "symbol-poker-face" | "symbol-neutral" => Marker::Face(Face::Neutral),
            "symbol-cry" | "symbol-sad" => Marker::Face(Face::Sad),
            "symbol-attention" => Marker::Symbol(Sym::Attention),
            "symbol-question" => Marker::Symbol(Sym::Question),
            "symbol-idea" => Marker::Symbol(Sym::Idea),
            _ => return None,
        })
    }

    /// Draw the marker centered at `c`, fitting in a box of side `s`.
    /// Returns extra text primitives (priority number, "!"/"?"...).
    pub fn prims(&self, c: V2, s: f32) -> Vec<Prim> {
        let mut ps = Vec::new();
        let r = s / 2.0;
        match self {
            Marker::Priority(p) => {
                ps.push(Prim::Circle { c, r, fill: Some(priority_color(*p)), stroke: Some((1.0, WHITE)) });
                ps.push(Prim::Text {
                    p: V2::new(c.x, c.y + s * 0.36),
                    text: p.to_string(),
                    size: s * 0.78,
                    color: WHITE,
                    bold: true,
                    italic: false,
                    center: true,
                });
            }
            Marker::Task(t) => {
                if *t >= 100.0 {
                    ps.push(Prim::Circle { c, r, fill: Some(Col::rgb(34, 197, 94)), stroke: Some((1.2, WHITE)) });
                    let k = r * 0.45;
                    ps.push(Prim::Poly {
                        pts: vec![
                            V2::new(c.x - k, c.y),
                            V2::new(c.x - k * 0.2, c.y + k * 0.8),
                            V2::new(c.x + k, c.y - k * 0.7),
                        ],
                        closed: false,
                        fill: None,
                        stroke: Some((s * 0.14 + 1.0, WHITE)),
                    });
                } else {
                    // pie: gray base, green progress wedge
                    ps.push(Prim::Circle { c, r, fill: Some(Col::rgb(229, 231, 235)), stroke: Some((1.2, WHITE)) });
                    let mut pts = vec![c];
                    let n = 20;
                    let end = -std::f32::consts::FRAC_PI_2 + (*t / 100.0) * std::f32::consts::TAU;
                    for i in 0..=n {
                        let a = -std::f32::consts::FRAC_PI_2 + (end + std::f32::consts::FRAC_PI_2) * i as f32 / n as f32;
                        pts.push(V2::new(c.x + a.cos() * r, c.y + a.sin() * r));
                    }
                    pts.push(c);
                    ps.push(Prim::Poly { pts, closed: true, fill: Some(Col::rgb(34, 197, 94)), stroke: None });
                }
            }
            Marker::Flag(col) => {
                let col_c = col.color();
                ps.push(Prim::Poly {
                    pts: vec![
                        V2::new(c.x - r * 0.45, c.y - r),
                        V2::new(c.x - r * 0.45, c.y + r),
                    ],
                    closed: false,
                    fill: None,
                    stroke: Some((s * 0.12, Col::rgb(87, 83, 78))),
                });
                ps.push(Prim::Poly {
                    pts: vec![
                        V2::new(c.x - r * 0.45, c.y - r * 0.95),
                        V2::new(c.x + r, c.y - r * 0.55),
                        V2::new(c.x - r * 0.45, c.y - r * 0.15),
                    ],
                    closed: true,
                    fill: Some(col_c),
                    stroke: Some((0.8, col_c.darken(0.7))),
                });
            }
            Marker::Star(col) => {
                let c0 = col.color();
                let pts = star_pts(c, r, 5, 0.45);
                ps.push(Prim::Poly { pts, closed: true, fill: Some(c0), stroke: Some((0.8, WHITE)) });
            }
            Marker::People(col) => {
                let c0 = col.color();
                ps.push(Prim::Circle { c: V2::new(c.x, c.y - r * 0.35), r: r * 0.38, fill: Some(c0), stroke: None });
                let mut body = Vec::new();
                for i in 0..=12 {
                    let a = std::f32::consts::PI + std::f32::consts::PI * i as f32 / 12.0;
                    body.push(V2::new(c.x + a.cos() * r * 0.72, c.y + r * 0.15 - a.sin() * r * 0.62));
                }
                body.push(V2::new(c.x + r * 0.72, c.y + r));
                body.push(V2::new(c.x - r * 0.72, c.y + r));
                ps.push(Prim::Poly { pts: body, closed: true, fill: Some(c0), stroke: None });
            }
            Marker::Heart(col) => {
                let c0 = col.color();
                let mut pts = Vec::new();
                for i in 0..=24 {
                    let t = i as f32 / 24.0 * std::f32::consts::TAU;
                    let x = 16.0 * t.sin().powi(3);
                    let y = 13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (t).cos();
                    pts.push(V2::new(c.x + x / 17.0 * r, c.y - y / 17.0 * r));
                }
                ps.push(Prim::Poly { pts, closed: true, fill: Some(c0), stroke: Some((0.8, c0.darken(0.7))) });
            }
            Marker::Face(f) => {
                let (bg, fg) = match f {
                    Face::Laugh => (Col::rgb(34, 197, 94), WHITE),
                    Face::Smile => (Col::rgb(132, 204, 22), WHITE),
                    Face::Neutral => (Col::rgb(148, 163, 184), WHITE),
                    Face::Sad => (Col::rgb(239, 68, 68), WHITE),
                };
                ps.push(Prim::Circle { c, r, fill: Some(bg), stroke: Some((1.0, WHITE)) });
                let ey = c.y - r * 0.2;
                for dx in [-r * 0.35, r * 0.35] {
                    ps.push(Prim::Circle { c: V2::new(c.x + dx, ey), r: r * 0.1, fill: Some(fg), stroke: None });
                }
                let my = c.y + r * 0.3;
                let mouth = match f {
                    Face::Laugh => vec![
                        V2::new(c.x - r * 0.45, my - r * 0.05),
                        V2::new(c.x - r * 0.2, my + r * 0.35),
                        V2::new(c.x + r * 0.2, my + r * 0.35),
                        V2::new(c.x + r * 0.45, my - r * 0.05),
                    ],
                    Face::Smile => vec![
                        V2::new(c.x - r * 0.42, my),
                        V2::new(c.x - r * 0.15, my + r * 0.28),
                        V2::new(c.x + r * 0.15, my + r * 0.28),
                        V2::new(c.x + r * 0.42, my),
                    ],
                    Face::Neutral => vec![V2::new(c.x - r * 0.35, my + r * 0.1), V2::new(c.x + r * 0.35, my + r * 0.1)],
                    Face::Sad => vec![
                        V2::new(c.x - r * 0.4, my + r * 0.3),
                        V2::new(c.x - r * 0.1, my - r * 0.05),
                        V2::new(c.x + r * 0.1, my - r * 0.05),
                        V2::new(c.x + r * 0.4, my + r * 0.3),
                    ],
                };
                ps.push(Prim::Poly { pts: mouth, closed: false, fill: None, stroke: Some((s * 0.1 + 0.8, fg)) });
            }
            Marker::Symbol(sym) => {
                let (bg, ch) = match sym {
                    Sym::Attention => (Col::rgb(245, 158, 11), "!"),
                    Sym::Question => (Col::rgb(59, 130, 246), "?"),
                    Sym::Idea => (Col::rgb(250, 204, 21), "i"),
                };
                if matches!(sym, Sym::Attention) {
                    ps.push(Prim::Poly {
                        pts: vec![
                            V2::new(c.x, c.y - r),
                            V2::new(c.x + r * 0.9, c.y + r * 0.75),
                            V2::new(c.x - r * 0.9, c.y + r * 0.75),
                        ],
                        closed: true,
                        fill: Some(bg),
                        stroke: Some((1.0, bg.darken(0.7))),
                    });
                } else {
                    ps.push(Prim::Circle { c, r, fill: Some(bg), stroke: Some((1.0, WHITE)) });
                }
                ps.push(Prim::Text {
                    p: V2::new(c.x, c.y + r * 0.38),
                    text: ch.into(),
                    size: s * 0.8,
                    color: if matches!(sym, Sym::Idea) { Col::rgb(120, 53, 15) } else { WHITE },
                    bold: true,
                    italic: false,
                    center: true,
                });
            }
        }
        ps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xmind_marker_roundtrip() {
        for m in [
            Marker::Priority(3),
            Marker::Task(50.0),
            Marker::Task(100.0),
            Marker::Flag(ColKey::Red),
            Marker::Star(ColKey::Blue),
            Marker::People(ColKey::Green),
            Marker::Heart(ColKey::Purple),
            Marker::Face(Face::Laugh),
            Marker::Symbol(Sym::Question),
        ] {
            assert_eq!(Marker::from_xmind_id(&m.to_xmind_id()), Some(m), "{m:?}");
        }
    }
}
