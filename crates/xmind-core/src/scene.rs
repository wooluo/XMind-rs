use crate::col::Col;
use crate::geo::{Rect, V2};
use std::collections::HashMap;

/// Renderer-independent drawing primitive. Both the egui canvas and the
/// headless exporters (svg / png / pdf) consume this list.
#[derive(Clone, Debug)]
pub enum Prim {
    Rect {
        r: Rect,
        rounding: f32,
        fill: Option<Col>,
        stroke: Option<(f32, Col)>,
    },
    /// Open or closed path made of line segments; fill only applies to closed.
    Poly {
        pts: Vec<V2>,
        closed: bool,
        fill: Option<Col>,
        stroke: Option<(f32, Col)>,
    },
    Circle {
        c: V2,
        r: f32,
        fill: Option<Col>,
        stroke: Option<(f32, Col)>,
    },
    Text {
        p: V2,
        text: String,
        size: f32,
        color: Col,
        bold: bool,
        italic: bool,
        /// true => `p` is the horizontal center, otherwise left edge.
        center: bool,
    },
    /// Filled triangles (star fill etc.)
    Tris {
        tris: Vec<[V2; 3]>,
        color: Col,
    },
    Image {
        r: Rect,
        asset: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum HitTarget {
    Topic(String),
    /// expand/collapse badge of a topic
    Badge(String),
    Boundary { parent: String, id: String },
    Summary { parent: String, id: String },
}

#[derive(Clone, Debug)]
pub struct HitRect {
    pub rect: Rect,
    pub target: HitTarget,
}

#[derive(Clone, Debug)]
pub struct RelCurve {
    pub id: String,
    pub p0: V2,
    pub c1: V2,
    pub c2: V2,
    pub p1: V2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NodeSide {
    Left,
    Right,
    Down,
    Up,
}

/// Complete renderable representation of one sheet.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub prims: Vec<Prim>,
    pub hits: Vec<HitRect>,
    /// bezier curves of relationships, for hit testing (distance based)
    pub rels: Vec<RelCurve>,
    pub rects: HashMap<String, Rect>,
    /// which prims belong to which topic (index range), for dim/redraw tricks
    pub node_prims: HashMap<String, (usize, usize)>,
    /// badge world-rects by topic id
    pub badges: HashMap<String, Rect>,
    pub bounds: Rect,
    pub bg: Col,
}

impl Scene {
    pub fn rect_of(&self, id: &str) -> Option<Rect> {
        self.rects.get(id).copied()
    }
    pub fn mark_node_start(&mut self, id: &str) {
        let s = self.prims.len();
        self.node_prims.insert(id.to_string(), (s, s));
    }
    pub fn mark_node_end(&mut self, id: &str) {
        let e = self.prims.len();
        if let Some(r) = self.node_prims.get_mut(id) {
            r.1 = e;
        }
    }
}
