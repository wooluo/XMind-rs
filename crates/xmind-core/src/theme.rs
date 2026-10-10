use crate::col::{Col, BLACK, WHITE};
use crate::model::{Shape, TopicStyle};

/// Fully resolved visual style for one node.
#[derive(Clone, Debug)]
pub struct Resolved {
    pub shape: Shape,
    pub fill: Option<Col>,
    pub text: Col,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub stroke: Option<(f32, Col)>,
    pub radius: f32,
    /// color of the connector coming into this node
    pub branch: Col,
}

pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub bg: Col,
    pub palette: &'static [Col],
    pub root_fill: Col,
    pub root_text: Col,
    pub root_size: f32,
    pub l1_size: f32,
    pub deep_size: f32,
    /// level>=2 nodes drawn as plain text + colored underline
    pub deep_underline: bool,
    pub deep_text: Col,
    /// l1 nodes are paper-white with colored stroke (business style)
    pub l1_paper: bool,
    pub dark: bool,
}

const fn c(r: u8, g: u8, b: u8) -> Col {
    Col(r, g, b, 255)
}

pub static THEMES: &[Theme] = &[
    Theme {
        id: "classic-blue",
        name: "经典蓝",
        bg: c(245, 248, 252),
        palette: &[c(29, 122, 243), c(22, 163, 74), c(245, 158, 11), c(239, 68, 68), c(139, 92, 246), c(6, 182, 212), c(236, 72, 153), c(100, 116, 139)],
        root_fill: c(29, 122, 243),
        root_text: WHITE,
        root_size: 24.0,
        l1_size: 16.0,
        deep_size: 14.0,
        deep_underline: true,
        deep_text: c(31, 41, 55),
        l1_paper: false,
        dark: false,
    },
    Theme {
        id: "fresh-green",
        name: "清新绿",
        bg: c(247, 252, 245),
        palette: &[c(34, 197, 94), c(14, 165, 233), c(249, 115, 22), c(168, 85, 247), c(225, 29, 72), c(20, 184, 166)],
        root_fill: c(22, 163, 74),
        root_text: WHITE,
        root_size: 24.0,
        l1_size: 16.0,
        deep_size: 14.0,
        deep_underline: true,
        deep_text: c(20, 53, 45),
        l1_paper: false,
        dark: false,
    },
    Theme {
        id: "sakura",
        name: "樱花粉",
        bg: c(255, 247, 250),
        palette: &[c(219, 39, 119), c(245, 158, 11), c(139, 92, 246), c(14, 165, 233), c(16, 185, 129)],
        root_fill: c(219, 39, 119),
        root_text: WHITE,
        root_size: 24.0,
        l1_size: 16.0,
        deep_size: 14.0,
        deep_underline: true,
        deep_text: c(80, 45, 65),
        l1_paper: false,
        dark: false,
    },
    Theme {
        id: "dark-night",
        name: "暗夜",
        bg: c(20, 22, 26),
        palette: &[c(79, 139, 240), c(52, 211, 153), c(251, 191, 36), c(248, 113, 113), c(167, 139, 250), c(34, 211, 238)],
        root_fill: c(39, 43, 51),
        root_text: c(226, 232, 240),
        root_size: 24.0,
        l1_size: 16.0,
        deep_size: 14.0,
        deep_underline: true,
        deep_text: c(209, 213, 219),
        l1_paper: false,
        dark: true,
    },
    Theme {
        id: "business",
        name: "商务蓝",
        bg: WHITE,
        palette: &[c(30, 58, 138), c(3, 105, 161), c(21, 128, 61), c(180, 83, 9), c(124, 45, 18)],
        root_fill: c(30, 58, 138),
        root_text: WHITE,
        root_size: 22.0,
        l1_size: 15.0,
        deep_size: 13.5,
        deep_underline: false,
        deep_text: c(17, 24, 39),
        l1_paper: true,
        dark: false,
    },
    Theme {
        id: "elegant-gray",
        name: "素雅灰",
        bg: c(250, 250, 250),
        palette: &[c(55, 65, 81), c(107, 114, 128), c(75, 85, 99), c(156, 163, 175)],
        root_fill: c(17, 24, 39),
        root_text: WHITE,
        root_size: 23.0,
        l1_size: 15.0,
        deep_size: 14.0,
        deep_underline: true,
        deep_text: c(31, 41, 55),
        l1_paper: false,
        dark: false,
    },
    Theme {
        id: "vitality",
        name: "活力橙",
        bg: c(255, 251, 245),
        palette: &[c(249, 115, 22), c(234, 179, 8), c(34, 197, 94), c(59, 130, 246), c(236, 72, 153)],
        root_fill: c(234, 88, 12),
        root_text: WHITE,
        root_size: 24.0,
        l1_size: 16.0,
        deep_size: 14.0,
        deep_underline: true,
        deep_text: c(50, 40, 30),
        l1_paper: false,
        dark: false,
    },
    Theme {
        id: "violet",
        name: "紫罗兰",
        bg: c(250, 247, 255),
        palette: &[c(109, 40, 217), c(37, 99, 235), c(219, 39, 119), c(8, 145, 178), c(101, 163, 13)],
        root_fill: c(109, 40, 217),
        root_text: WHITE,
        root_size: 24.0,
        l1_size: 16.0,
        deep_size: 14.0,
        deep_underline: true,
        deep_text: c(45, 35, 65),
        l1_paper: false,
        dark: false,
    },
    Theme {
        id: "pure",
        name: "纯白",
        bg: WHITE,
        palette: &[c(17, 24, 39), c(55, 65, 81), c(75, 85, 99), c(107, 114, 128)],
        root_fill: c(17, 24, 39),
        root_text: WHITE,
        root_size: 23.0,
        l1_size: 15.0,
        deep_size: 14.0,
        deep_underline: true,
        deep_text: c(31, 41, 55),
        l1_paper: false,
        dark: false,
    },
];

pub fn theme_by_id(id: &str) -> &'static Theme {
    THEMES.iter().find(|t| t.id == id).unwrap_or(&THEMES[0])
}

/// Branch color for a level-1 child at index `idx`.
pub fn branch_color(th: &Theme, idx: usize) -> Col {
    th.palette[idx % th.palette.len()]
}

/// Resolve the effective style of a topic.
/// `level`: 0 = root, 1 = first level ...
/// `branch`: branch color inherited from the level-1 ancestor.
pub fn resolve(th: &Theme, level: usize, branch: Col, style: Option<&TopicStyle>, structure_out: bool) -> Resolved {
    let st = style;
    let default = |shape: Shape, fill: Option<Col>, text: Col, size: f32, bold: bool, stroke: Option<(f32, Col)>, radius: f32| Resolved {
        shape,
        fill,
        text,
        size,
        bold,
        italic: false,
        stroke,
        radius,
        branch,
    };
    let mut r = if level == 0 {
        default(Shape::Capsule, Some(th.root_fill), th.root_text, th.root_size, true, None, th.root_size)
    } else if level == 1 {
        if th.l1_paper {
            default(Shape::Round, Some(WHITE), th.dark.then_some(WHITE).unwrap_or(BLACK), th.l1_size, true, Some((2.0, branch)), 8.0)
        } else {
            default(Shape::Round, Some(branch), branch.text_on(), th.l1_size, true, None, 8.0)
        }
    } else if th.deep_underline {
        default(Shape::Underline, None, th.deep_text, th.deep_size, false, None, 0.0)
    } else {
        default(Shape::Round, Some(branch.lighten(0.86)), th.deep_text, th.deep_size, false, None, 6.0)
    };
    if let Some(s) = st {
        if let Some(f) = s.fill {
            r.fill = Some(f);
        }
        if let Some(t) = s.text {
            r.text = t;
        }
        if let Some(sc) = s.stroke {
            r.stroke = Some((s.stroke_w.unwrap_or(1.5), sc));
        } else if s.stroke_w.is_some() {
            if let Some((_w, c)) = r.stroke {
                r.stroke = Some((s.stroke_w.unwrap(), c));
            }
        }
        if let Some(fs) = s.font_size {
            r.size = fs;
        }
        if let Some(b) = s.bold {
            r.bold = b;
        }
        if let Some(i) = s.italic {
            r.italic = i;
        }
        if let Some(sh) = s.shape {
            r.shape = sh;
        }
    }
    if structure_out {
        // org/timeline/matrix children look like boxes regardless of theme
        if r.shape == Shape::Underline {
            r.shape = Shape::Round;
            r.fill = Some(if th.dark { c(39, 43, 51) } else { WHITE });
            r.stroke = Some((1.4, branch));
        }
    }
    if r.shape == Shape::Capsule {
        r.radius = r.size * 0.8;
    }
    r
}

/// Default connector width by level.
pub fn connector_width(level: usize) -> f32 {
    match level {
        0 => 4.0,
        1 => 3.0,
        2 => 2.2,
        3 => 1.7,
        _ => 1.3,
    }
}

pub fn hgap(level: usize) -> f32 {
    if level == 0 { 64.0 } else if level == 1 { 44.0 } else { 30.0 }
}

pub fn vgap(level: usize) -> f32 {
    if level <= 1 { 22.0 } else if level == 2 { 12.0 } else { 8.0 }
}
