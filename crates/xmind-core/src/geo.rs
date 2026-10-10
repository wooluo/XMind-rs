#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub fn new(x: f32, y: f32) -> Self {
        V2 { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn from_center(cx: f32, cy: f32, w: f32, h: f32) -> Self {
        Rect { x: cx - w / 2.0, y: cy - h / 2.0, w, h }
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }
    pub fn contains(&self, p: V2) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }
    pub fn inflate(&self, d: f32) -> Rect {
        Rect { x: self.x - d, y: self.y - d, w: self.w + 2.0 * d, h: self.h + 2.0 * d }
    }
    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        let r = self.right().max(o.right());
        let b = self.bottom().max(o.bottom());
        Rect { x, y, w: r - x, h: b - y }
    }
    pub fn intersect(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }
}

/// Cubic bezier point at t.
pub fn cubic(p0: V2, c1: V2, c2: V2, p1: V2, t: f32) -> V2 {
    let u = 1.0 - t;
    let a = u * u * u;
    let b = 3.0 * u * u * t;
    let c = 3.0 * u * t * t;
    let d = t * t * t;
    V2::new(
        a * p0.x + b * c1.x + c * c2.x + d * p1.x,
        a * p0.y + b * c1.y + c * c2.y + d * p1.y,
    )
}

/// Cubic bezier tangent at t (not normalized).
pub fn cubic_tangent(p0: V2, c1: V2, c2: V2, p1: V2, t: f32) -> V2 {
    let u = 1.0 - t;
    let a = 3.0 * u * u;
    let b = 6.0 * u * t;
    let c = 3.0 * t * t;
    V2::new(
        a * (c1.x - p0.x) + b * (c2.x - c1.x) + c * (p1.x - c2.x),
        a * (c1.y - p0.y) + b * (c2.y - c1.y) + c * (p1.y - c2.y),
    )
}

/// Flatten a cubic bezier into a polyline.
pub fn flatten_cubic(p0: V2, c1: V2, c2: V2, p1: V2, n: usize) -> Vec<V2> {
    (0..=n).map(|i| cubic(p0, c1, c2, p1, i as f32 / n as f32)).collect()
}

/// Split a polyline into dashed segments (on/off lengths).
pub fn dash_polyline(pts: &[V2], on: f32, off: f32) -> Vec<(V2, V2)> {
    let mut segs = Vec::new();
    let mut on_seg = true;
    let mut remaining = if on_seg { on } else { off };
    for w in pts.windows(2) {
        let a = w[0];
        let b = w[1];
        let len = (b.x - a.x).hypot(b.y - a.y);
        if len <= f32::EPSILON {
            continue;
        }
        let mut start = a;
        let mut left = len;
        loop {
            if left <= remaining {
                if on_seg {
                    segs.push((start, b));
                }
                remaining -= left;
                break;
            } else {
                let t = remaining / left;
                let mid = V2::new(start.x + (b.x - start.x) * t, start.y + (b.y - start.y) * t);
                if on_seg {
                    segs.push((start, mid));
                }
                start = mid;
                left -= remaining;
                on_seg = !on_seg;
                remaining = if on_seg { on } else { off };
            }
        }
    }
    segs
}

pub fn dist_to_segment(p: V2, a: V2, b: V2) -> f32 {
    let vx = b.x - a.x;
    let vy = b.y - a.y;
    let wx = p.x - a.x;
    let wy = p.y - a.y;
    let c2 = vx * vx + vy * vy;
    if c2 <= f32::EPSILON {
        return (p.x - a.x).hypot(p.y - a.y);
    }
    let t = ((wx * vx + wy * vy) / c2).clamp(0.0, 1.0);
    let cx = a.x + t * vx;
    let cy = a.y + t * vy;
    (p.x - cx).hypot(p.y - cy)
}

pub fn dist_to_polyline(p: V2, pts: &[V2]) -> f32 {
    pts.windows(2).map(|w| dist_to_segment(p, w[0], w[1])).fold(f32::INFINITY, f32::min)
}

/// Regular-star polygon (outer radius r, n points, rotate so tip is up).
pub fn star_pts(c: V2, r: f32, n: usize, inner_ratio: f32) -> Vec<V2> {
    let mut pts = Vec::with_capacity(n * 2);
    for i in 0..n {
        let a1 = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / n as f32;
        let a2 = a1 + std::f32::consts::TAU / (n as f32 * 2.0);
        pts.push(V2::new(c.x + a1.cos() * r, c.y + a1.sin() * r));
        pts.push(V2::new(c.x + a2.cos() * r * inner_ratio, c.y + a2.sin() * r * inner_ratio));
    }
    pts
}
