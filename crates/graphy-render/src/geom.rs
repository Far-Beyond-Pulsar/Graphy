//! Minimal 2D geometry used by every renderer-facing type. Graph space and
//! screen space share these types; which one a value is in is stated by the
//! API that produces it.

use std::ops::{Add, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn distance(self, other: Vec2) -> f32 {
        (self - other).length()
    }

    pub fn lerp(self, other: Vec2, t: f32) -> Vec2 {
        self + (other - self) * t
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

/// Axis-aligned rectangle; `min` is the top-left corner (y grows downwards).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub fn from_corners(a: Vec2, b: Vec2) -> Self {
        Self {
            min: Vec2::new(a.x.min(b.x), a.y.min(b.y)),
            max: Vec2::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    pub fn from_pos_size(pos: Vec2, size: Vec2) -> Self {
        Self::from_corners(pos, pos + size)
    }

    pub fn around(center: Vec2, radius: f32) -> Self {
        let r = Vec2::new(radius, radius);
        Self {
            min: center - r,
            max: center + r,
        }
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    pub fn center(&self) -> Vec2 {
        self.min.lerp(self.max, 0.5)
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    pub fn intersects(&self, o: &Rect) -> bool {
        self.min.x <= o.max.x
            && self.max.x >= o.min.x
            && self.min.y <= o.max.y
            && self.max.y >= o.min.y
    }

    pub fn expand(&self, by: f32) -> Rect {
        Rect {
            min: self.min - Vec2::new(by, by),
            max: self.max + Vec2::new(by, by),
        }
    }

    pub fn translate(&self, by: Vec2) -> Rect {
        Rect {
            min: self.min + by,
            max: self.max + by,
        }
    }

    pub fn union(&self, o: &Rect) -> Rect {
        Rect {
            min: Vec2::new(self.min.x.min(o.min.x), self.min.y.min(o.min.y)),
            max: Vec2::new(self.max.x.max(o.max.x), self.max.y.max(o.max.y)),
        }
    }

    /// Distance from `p` to the rectangle (0 when inside).
    pub fn distance_to(&self, p: Vec2) -> f32 {
        let dx = (self.min.x - p.x).max(0.0).max(p.x - self.max.x);
        let dy = (self.min.y - p.y).max(0.0).max(p.y - self.max.y);
        dx.hypot(dy)
    }
}

/// RGBA, 0..1, straight (non-premultiplied) alpha.
pub type Color = [f32; 4];

pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

pub fn with_alpha(c: Color, a: f32) -> Color {
    [c[0], c[1], c[2], a]
}

/// Parses `#RRGGBB` or `#RRGGBBAA` (the leading `#` is optional).
pub fn parse_hex(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    match hex.len() {
        6 => Some(rgba(byte(0)?, byte(2)?, byte(4)?, 1.0)),
        8 => Some(rgba(byte(0)?, byte(2)?, byte(4)?, f32::from(byte(6)?) / 255.0)),
        _ => None,
    }
}
