// Wires. Each instance is one connection described by up to six waypoints; the
// vertex stage evaluates the curve (cubic bezier, or a rounded orthogonal
// route) into up to MAX_SEGS capsule segments, so there is no CPU
// tessellation. Every segment is a quad whose fragments compute an exact
// capsule distance, which makes joins round and gives uniform antialiasing at
// any width. Resting wires are drawn opaque (already blended toward the canvas
// on the CPU) so the overlapping capsule ends never show as beads.
//
// A second entry point draws a soft halo for emphasised wires with a MAX
// blend, so overlapping halo capsules do not accumulate either.

struct Inst {
    @location(0) p01: vec4<f32>,
    @location(1) p23: vec4<f32>,
    @location(2) p45: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) params: vec4<u32>,   // mode, waypoint count, segments, flags
    @location(5) fparams: vec4<f32>,  // width (graph units), phase, 0, 0
}

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) a: vec2<f32>,
    @location(1) @interpolate(flat) b: vec2<f32>,
    @location(2) @interpolate(flat) color: vec4<f32>,
    @location(3) @interpolate(flat) half_w: f32,
    @location(4) @interpolate(flat) flags: u32,
    @location(5) @interpolate(flat) t01: vec2<f32>,
    @location(6) @interpolate(flat) phase: f32,
    // x: length of the segment (px), y: 1 when the a end is a round cap, z: same for b
    @location(7) @interpolate(flat) caps: vec3<f32>,
}

const MAX_SEGS: u32 = 32u;
const CORNER_STEPS: u32 = 7u;
const ANGULAR_RADIUS: f32 = 12.0;

fn waypoint(i: Inst, k: u32) -> vec2<f32> {
    switch k {
        case 0u: { return i.p01.xy; }
        case 1u: { return i.p01.zw; }
        case 2u: { return i.p23.xy; }
        case 3u: { return i.p23.zw; }
        case 4u: { return i.p45.xy; }
        default: { return i.p45.zw; }
    }
}

fn unit_or_zero(v: vec2<f32>) -> vec2<f32> {
    let l = length(v);
    return select(vec2<f32>(0.0), v / l, l > 0.000001);
}

fn cubic(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>, t: f32) -> vec2<f32> {
    let u1 = 1.0 - t;
    return p0 * (u1 * u1 * u1) + p1 * (3.0 * u1 * u1 * t) + p2 * (3.0 * u1 * t * t) + p3 * (t * t * t);
}

// Mirrors graphy_render::wire::WireShape::route_point.
fn route_point(i: Inst, k: u32) -> vec2<f32> {
    let n = i.params.y;
    let corners = n - 2u;
    if k == 0u {
        return waypoint(i, 0u);
    }
    let c = (k - 1u) / CORNER_STEPS;
    if c >= corners {
        return waypoint(i, n - 1u);
    }
    let sub = (k - 1u) % CORNER_STEPS;
    let prev = waypoint(i, c);
    let corner = waypoint(i, c + 1u);
    let next = waypoint(i, c + 2u);
    let r = min(ANGULAR_RADIUS, min(distance(prev, corner), distance(next, corner)) * 0.5);
    let a = corner - unit_or_zero(corner - prev) * r;
    let b = corner + unit_or_zero(next - corner) * r;
    let t = f32(sub) / f32(CORNER_STEPS - 1u);
    return mix(mix(a, corner, t), mix(corner, b, t), t);
}

fn curve_point(i: Inst, k: u32) -> vec2<f32> {
    if i.params.x == 0u {
        let t = f32(k) / f32(max(i.params.z, 1u));
        return cubic(waypoint(i, 0u), waypoint(i, 1u), waypoint(i, 2u), waypoint(i, 3u), t);
    }
    return route_point(i, k);
}

fn segment_count(i: Inst) -> u32 {
    if i.params.x == 0u {
        return min(i.params.z, MAX_SEGS);
    }
    return 1u + (i.params.y - 2u) * CORNER_STEPS;
}

fn build(i: Inst, vi: u32, halo: bool) -> VOut {
    var o: VOut;
    let seg = vi / 6u;
    let corner = vi % 6u;
    let count = segment_count(i);
    o.a = vec2<f32>(0.0);
    o.b = vec2<f32>(0.0);
    o.color = i.color;
    o.half_w = 0.0;
    o.flags = i.params.w;
    o.t01 = vec2<f32>(0.0);
    o.phase = i.fparams.y;
    o.caps = vec3<f32>(0.0);
    if seg >= count {
        o.pos = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        return o;
    }
    let a = to_screen(curve_point(i, seg));
    let b = to_screen(curve_point(i, seg + 1u));
    if distance(a, b) < 0.01 {
        o.pos = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        return o;
    }
    let half_w = max(i.fparams.x * kk() * 0.5, 0.6 * u.scale);
    let aa = 1.5 * u.scale;
    let grow = select(0.0, 8.0 * u.scale, halo);
    let ext = half_w + grow + aa;
    // Interior joins are exact butt ends, so adjacent
    // segments never double-cover the antialiased edge (no beading). Only the
    // ends of the whole wire get round caps. The halo pass is MAX-blended and
    // can overlap freely.
    let cap_a = seg == 0u;
    let cap_b = seg + 1u == count;
    let butt = 0.0;
    let ext_a = select(butt, ext, cap_a || halo);
    let ext_b = select(butt, ext, cap_b || halo);
    var d = b - a;
    var dir = vec2<f32>(1.0, 0.0);
    if length(d) > 0.0001 {
        dir = normalize(d);
    }
    let nrm = vec2<f32>(-dir.y, dir.x);
    var cx = array<f32, 6>(0.0, 1.0, 0.0, 0.0, 1.0, 1.0);
    var cy = array<f32, 6>(-1.0, -1.0, 1.0, 1.0, -1.0, 1.0);
    let along = select(-ext_a, ext_b, cx[corner] > 0.5);
    let base = select(a, b, cx[corner] > 0.5);
    let p = base + dir * along + nrm * cy[corner] * ext;
    o.pos = to_clip(p);
    o.a = a;
    o.b = b;
    o.half_w = half_w;
    o.t01 = vec2<f32>(f32(seg) / f32(count), f32(seg + 1u) / f32(count));
    o.caps = vec3<f32>(length(b - a), select(0.0, 1.0, cap_a), select(0.0, 1.0, cap_b));
    return o;
}

@vertex
fn vs_main(i: Inst, @builtin(vertex_index) vi: u32) -> VOut {
    return build(i, vi, false);
}

@vertex
fn vs_halo(i: Inst, @builtin(vertex_index) vi: u32) -> VOut {
    return build(i, vi, true);
}

fn seg_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let ab = b - a;
    let l2 = max(dot(ab, ab), 0.0001);
    let h = clamp(dot(p - a, ab) / l2, 0.0, 1.0);
    return vec2<f32>(length(p - (a + ab * h)), h);
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let dh = seg_dist(in.pos.xy, in.a, in.b);
    let ab = in.b - in.a;
    let dir = ab / max(in.caps.x, 0.0001);
    let rel = in.pos.xy - in.a;
    let along = dot(rel, dir);
    var d = abs(dot(rel, vec2<f32>(-dir.y, dir.x))) - in.half_w;
    if along < 0.0 && in.caps.y > 0.5 {
        d = length(rel) - in.half_w;
    } else if along > in.caps.x && in.caps.z > 0.5 {
        d = length(in.pos.xy - in.b) - in.half_w;
    }
    let cov = cover(d);
    if cov <= 0.0 {
        discard;
    }
    let t = mix(in.t01.x, in.t01.y, dh.y);
    var col = in.color.rgb;
    var alpha = in.color.a;
    if flag(in.flags, F_ACTIVE) {
        // a bright packet travels along the wire
        let head = fract(u.time * 0.45 + in.phase);
        let dd = abs(t - head);
        let w = min(dd, 1.0 - dd);
        col = mix(col, vec3<f32>(1.0), (1.0 - smoothstep(0.0, 0.10, w)) * 0.65);
    }
    if flag(in.flags, F_PREVIEW) {
        let wave = 0.72 + 0.28 * sin(t * 40.0 - u.time * 7.0);
        alpha = alpha * wave;
    }
    if flag(in.flags, F_EMPHASIS) {
        col = mix(col, vec3<f32>(1.0), 0.06);
    }
    return pm(vec4<f32>(col, alpha * cov));
}

@fragment
fn fs_halo(in: VOut) -> @location(0) vec4<f32> {
    let dh = seg_dist(in.pos.xy, in.a, in.b);
    let d = dh.x - in.half_w;
    let g = (1.0 - smoothstep(0.0, 8.0 * u.scale, max(d, 0.0))) * 0.20;
    if g <= 0.0 {
        discard;
    }
    return vec4<f32>(in.color.rgb * g, g);
}
