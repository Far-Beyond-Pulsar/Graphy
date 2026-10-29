//! Wire geometry: control points for the bezier and rounded-orthogonal
//! ("angular") routings, CPU flattening and distance queries.
//!
//! The GPU evaluates the same curves from the same six waypoints
//! ([`WireShape::pts`]); `route_point` here and `route_point` in the wire
//! shader are deliberately the same algorithm so picking matches drawing.

use crate::geom::{Rect, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireRouting {
    Bezier,
    Angular,
}

    /// Points evaluated per rounded corner (inclusive of both ends).
    pub const CORNER_STEPS: usize = 7;
    /// Corner radius of angular wires, graph units.
    pub const ANGULAR_RADIUS: f32 = 12.0;
    const STUB: f32 = 26.0;

    /// Up to six waypoints. Bezier uses `pts[0..4]` as the cubic's control
    /// polygon; angular uses `pts[0..n]` as a polyline whose interior vertices are
    /// rounded. A five-waypoint bezier is a wire that passes through the
    /// point `pts[2]` (a zip tie slot), evaluated as two cubic halves
    /// `(pts[0], pts[1], pts[2], pts[2])` and `(pts[2], pts[2], pts[3], pts[4])`
    /// that meet there; the wire shader runs the same algorithm.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct WireShape {
    pub routing: WireRouting,
    pub n: usize,
    pub pts: [Vec2; 6],
}

impl WireShape {
    /// A cubic Bezier from four explicit points.
    pub fn cubic(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2) -> WireShape {
        let mut pts = [Vec2::ZERO; 6];
        pts[..4].copy_from_slice(&[p0, p1, p2, p3]);
        WireShape {
            routing: WireRouting::Bezier,
            n: 4,
            pts,
        }
    }

    pub fn new(routing: WireRouting, from: Vec2, to: Vec2) -> WireShape {
        match routing {
            WireRouting::Bezier => Self::bezier(from, to),
            WireRouting::Angular => Self::angular(from, to),
        }
    }

    pub fn bezier(from: Vec2, to: Vec2) -> WireShape {
        let dx = to.x - from.x;
        // Forward wires leave and arrive horizontally with a handle that grows
        // with distance; backward wires get a longer handle so the S curve
        // clears both nodes.
        let handle = if dx >= 0.0 {
            (dx * 0.5).max(36.0)
        } else {
            36.0 + (-dx) * 0.55
        };
        let c1 = Vec2::new(from.x + handle, from.y);
        let c2 = Vec2::new(to.x - handle, to.y);
        let mut pts = [Vec2::ZERO; 6];
        pts[..4].copy_from_slice(&[from, c1, c2, to]);
        WireShape {
            routing: WireRouting::Bezier,
            n: 4,
            pts,
        }
    }

    /// A straight segment expressed as a degenerate cubic (pin tails).
    pub fn straight(from: Vec2, to: Vec2) -> WireShape {
        let mut pts = [Vec2::ZERO; 6];
        pts[..4].copy_from_slice(&[from, from.lerp(to, 1.0 / 3.0), from.lerp(to, 2.0 / 3.0), to]);
        WireShape {
            routing: WireRouting::Bezier,
            n: 4,
            pts,
        }
    }

    /// Bezier wire from `from` to `to` forced through `q`: ties and clamps.
    /// Two degenerate-halves cubics share `q`, so the curve is exact there and
    /// both endpoint tangents stay horizontal.
    pub fn through(from: Vec2, q: Vec2, to: Vec2) -> WireShape {
        let dx = to.x - from.x;
        let handle = if dx >= 0.0 {
            (dx * 0.35).max(30.0)
        } else {
            30.0 + (-dx) * 0.4
        };
        let mut pts = [Vec2::ZERO; 6];
        pts[..5].copy_from_slice(&[
            from,
            Vec2::new(from.x + handle, from.y),
            q,
            Vec2::new(to.x - handle, to.y),
            to,
        ]);
        WireShape {
            routing: WireRouting::Bezier,
            n: 5,
            pts,
        }
    }

    /// Angular wire from `from` to `to` passing `q` as an interior vertex.
    pub fn through_angular(from: Vec2, q: Vec2, to: Vec2) -> WireShape {
        let mut pts = [Vec2::ZERO; 6];
        pts[..3].copy_from_slice(&[from, q, to]);
        WireShape {
            routing: WireRouting::Angular,
            n: 3,
            pts,
        }
    }

    pub fn angular(from: Vec2, to: Vec2) -> WireShape {
        let mut pts = [Vec2::ZERO; 6];
        let dx = to.x - from.x;
        if dx >= STUB * 1.6 {
            let mx = from.x + dx * 0.5;
            pts[..4].copy_from_slice(&[from, Vec2::new(mx, from.y), Vec2::new(mx, to.y), to]);
            WireShape {
                routing: WireRouting::Angular,
                n: 4,
                pts,
            }
        } else {
            let dy = to.y - from.y;
            let my = if dy.abs() >= 40.0 {
                from.y + dy * 0.5
            } else {
                from.y.max(to.y) + 48.0
            };
            pts = [
                from,
                Vec2::new(from.x + STUB, from.y),
                Vec2::new(from.x + STUB, my),
                Vec2::new(to.x - STUB, my),
                Vec2::new(to.x - STUB, to.y),
                to,
            ];
            WireShape {
                routing: WireRouting::Angular,
                n: 6,
                pts,
            }
        }
    }

    pub fn waypoints(&self) -> &[Vec2] {
        &self.pts[..self.n]
    }

    pub fn bounds(&self) -> Rect {
        let mut r = Rect {
            min: self.pts[0],
            max: self.pts[0],
        };
        for p in self.waypoints() {
            r.min = Vec2::new(r.min.x.min(p.x), r.min.y.min(p.y));
            r.max = Vec2::new(r.max.x.max(p.x), r.max.y.max(p.y));
        }
        r
    }

    /// Flattens the curve into `segments` straight pieces (bezier) or the
    /// rounded polyline (angular; `segments` is ignored there).
    pub fn flatten(&self, segments: u32) -> Vec<Vec2> {
        match self.routing {
            WireRouting::Bezier if self.n == 5 => {
                // two halves meeting at pts[2], `t <= 0.5` on the first
                let h = ((segments / 2) as usize).max(1);
                let r = ((segments - h as u32) as usize).max(1);
                let first =
                    |k: usize| cubic(self.pts[0], self.pts[1], self.pts[2], self.pts[2], k as f32 / h as f32);
                let second =
                    |k: usize| cubic(self.pts[2], self.pts[2], self.pts[3], self.pts[4], k as f32 / r as f32);
                let mut out = Vec::with_capacity(segments as usize + 1);
                (0..=h).for_each(|k| out.push(first(k)));
                (1..=r).for_each(|k| out.push(second(k)));
                out
            }
            WireRouting::Bezier => {
                let [p0, p1, p2, p3] = [self.pts[0], self.pts[1], self.pts[2], self.pts[3]];
                (0..=segments)
                    .map(|i| cubic(p0, p1, p2, p3, i as f32 / segments.max(1) as f32))
                    .collect()
            }
            WireRouting::Angular => {
                let total = 2 + (self.n - 2) * CORNER_STEPS;
                (0..total).map(|k| self.route_point(k)).collect()
            }
        }
    }

    /// Point `k` of the rounded polyline: the start, then `CORNER_STEPS`
    /// points per interior vertex, then the end.
    pub fn route_point(&self, k: usize) -> Vec2 {
        let corners = self.n - 2;
        if k == 0 {
            return self.pts[0];
        }
        let c = (k - 1) / CORNER_STEPS;
        if c >= corners {
            return self.pts[self.n - 1];
        }
        let sub = (k - 1) % CORNER_STEPS;
        let prev = self.pts[c];
        let corner = self.pts[c + 1];
        let next = self.pts[c + 2];
        let r = ANGULAR_RADIUS
            .min(prev.distance(corner) * 0.5)
            .min(next.distance(corner) * 0.5);
        let dir_in = unit(corner - prev);
        let dir_out = unit(next - corner);
        let a = corner - dir_in * r;
        let b = corner + dir_out * r;
        let t = sub as f32 / (CORNER_STEPS - 1) as f32;
        // quadratic bezier a -> corner -> b
        a.lerp(corner, t).lerp(corner.lerp(b, t), t)
    }

    /// Shortest distance from `p` to the flattened wire.
    pub fn distance_to(&self, p: Vec2) -> f32 {
        let pts = self.flatten(24);
        pts.windows(2)
            .map(|w| segment_distance(p, w[0], w[1]))
            .fold(f32::INFINITY, f32::min)
    }
}

fn unit(v: Vec2) -> Vec2 {
    let l = v.length();
    if l < 1e-6 {
        Vec2::ZERO
    } else {
        v * (1.0 / l)
    }
}

pub fn cubic(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t)
}

pub fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.x * ab.x + ab.y * ab.y;
    if len2 < 1e-9 {
        return p.distance(a);
    }
    let t = (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}
