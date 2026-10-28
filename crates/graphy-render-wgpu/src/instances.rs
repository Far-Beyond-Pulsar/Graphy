//! GPU instance layouts and their conversion from renderer-neutral draw data.
//! All `repr(C)` and `Pod` so they upload with a plain byte copy.

use bytemuck::{Pod, Zeroable};
use graphy_render::{BoxDraw, DrawData, PinDraw, WireDraw, WireRouting};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Uniforms {
    pub viewport: [f32; 2],
    pub pan: [f32; 2],
    pub zoom: f32,
    pub scale: f32,
    pub time: f32,
    pub _pad: f32,
    pub canvas: [f32; 4],
    pub grid_minor: [f32; 4],
    pub grid_major: [f32; 4],
    pub grid_params: [f32; 4],
    pub card_body: [f32; 4],
}

impl Uniforms {
    pub fn new(d: &DrawData, scale: f32, target: (u32, u32)) -> Self {
        Self {
            viewport: [target.0 as f32, target.1 as f32],
            pan: [d.view.pan.x, d.view.pan.y],
            zoom: d.view.zoom,
            scale,
            time: d.time,
            _pad: 0.0,
            canvas: d.canvas.background,
            grid_minor: d.canvas.grid_minor,
            grid_major: d.canvas.grid_major,
            grid_params: [d.canvas.minor_step, d.canvas.major_every, 0.0, 0.0],
            card_body: d.canvas.card_body,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct BoxInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub body: [f32; 4],
    pub header: [f32; 4],
    pub accent: [f32; 4],
    pub border: [f32; 4],
    pub ring: [f32; 4],
    pub header_h: f32,
    pub radius: f32,
    pub flags: u32,
    pub kind: u32,
}

impl From<&BoxDraw> for BoxInstance {
    fn from(b: &BoxDraw) -> Self {
        Self {
            pos: [b.rect.min.x, b.rect.min.y],
            size: [b.rect.width(), b.rect.height()],
            body: b.body,
            header: b.header,
            accent: b.accent,
            border: b.border,
            ring: b.ring,
            header_h: b.header_h,
            radius: b.radius,
            flags: b.flags,
            kind: b.kind.shader_kind(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct PinInstance {
    pub center: [f32; 2],
    pub size: f32,
    pub flags: u32,
    pub color: [f32; 4],
    pub shape: u32,
    pub input: u32,
}

impl From<&PinDraw> for PinInstance {
    fn from(p: &PinDraw) -> Self {
        Self {
            center: [p.center.x, p.center.y],
            size: p.size,
            flags: p.flags,
            color: p.color,
            shape: p.shape.shader_kind(),
            input: p.input as u32,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct WireInstance {
    pub p01: [f32; 4],
    pub p23: [f32; 4],
    pub p45: [f32; 4],
    pub color: [f32; 4],
    /// mode (0 bezier, 1 angular), waypoint count, segments, flags
    pub params: [u32; 4],
    /// width (graph units), phase
    pub fparams: [f32; 4],
}

impl From<&WireDraw> for WireInstance {
    fn from(w: &WireDraw) -> Self {
        let p = &w.shape.pts;
        let mode = match w.shape.routing {
            WireRouting::Bezier => 0,
            WireRouting::Angular => 1,
        };
        Self {
            p01: [p[0].x, p[0].y, p[1].x, p[1].y],
            p23: [p[2].x, p[2].y, p[3].x, p[3].y],
            p45: [p[4].x, p[4].y, p[5].x, p[5].y],
            color: w.color,
            params: [mode, w.shape.n as u32, w.segments, w.flags],
            fparams: [w.width, w.phase, 0.0, 0.0],
        }
    }
}

/// Segment slots per wire instance; must match `MAX_SEGS` in `wire.wgsl`.
pub const WIRE_VERTS: u32 = 32 * 6;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TextVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}
