// Shared by every graph pipeline (concatenated in front of each shader).
//
// Coordinate model: instances are in graph space; the vertex stage maps them to
// physical pixels with  px = (graph + pan) * zoom * scale  and every fragment
// computes its coverage from a signed distance in *physical pixels*, so edges
// stay one-pixel crisp at any fractional zoom or display scale. All colours
// are straight RGBA on input and composited as premultiplied.

struct Uniforms {
    viewport: vec2<f32>,   // render target size, physical px
    pan: vec2<f32>,        // graph units
    zoom: f32,             // logical zoom
    scale: f32,            // device pixel ratio
    time: f32,
    _pad: f32,
    canvas: vec4<f32>,
    grid_minor: vec4<f32>,
    grid_major: vec4<f32>,
    grid_params: vec4<f32>, // x: minor step (graph units), y: minor lines per major
    card_body: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

fn kk() -> f32 {
    return u.zoom * u.scale;
}

fn to_screen(p: vec2<f32>) -> vec2<f32> {
    return (p + u.pan) * kk();
}

fn to_clip(p: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(p.x / u.viewport.x * 2.0 - 1.0, 1.0 - p.y / u.viewport.y * 2.0, 0.0, 1.0);
}

// Pixel coverage for a signed distance (0 at the edge, negative inside).
fn cover(d: f32) -> f32 {
    return clamp(0.5 - d, 0.0, 1.0);
}

fn pm(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(c.rgb * c.a, c.a);
}

fn over(dst: vec4<f32>, src: vec4<f32>) -> vec4<f32> {
    return src + dst * (1.0 - src.a);
}

fn sdf_rrect(p: vec2<f32>, half_size: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

fn flag(flags: u32, bit: u32) -> bool {
    return (flags & bit) != 0u;
}

const F_HOVERED: u32 = 1u;
const F_SELECTED: u32 = 2u;
const F_ACTIVE: u32 = 4u;
const F_DISABLED: u32 = 8u;
const F_FOCUSED: u32 = 16u;
const F_DRAGGING: u32 = 32u;
const F_FLAT: u32 = 64u;
const F_DIM: u32 = 128u;
const F_EMPHASIS: u32 = 256u;
const F_PREVIEW: u32 = 512u;
const F_CONNECTED: u32 = 1024u;
const F_COMPATIBLE: u32 = 2048u;
const F_RINGED: u32 = 4096u;
const F_DASHED: u32 = 8192u;
