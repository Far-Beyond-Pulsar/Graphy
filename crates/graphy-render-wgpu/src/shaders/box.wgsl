// Node cards, reroutes, pills, comments, icon chips, preview wells, marquee and
// snap guides: one instanced pipeline, one draw call per layer.
//
// Card language: a calm raised surface (hairline border, soft contact shadow),
// a compact header one step lighter than the body, and the category colour
// only as a thin gradient underline beneath the header. State is expressed with
// outlines and glow, never by recolouring large areas:
//   hover     lighter hairline + faint lift
//   selected  tight light ring, soft outer glow
//   focused   separated blue ring (keyboard/is_active target)
//   is_active    accent pulse (inner glow + animated outer glow)
//   disabled  desaturated and translucent
//   dragging  deeper shadow

struct Inst {
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) body: vec4<f32>,
    @location(3) header: vec4<f32>,
    @location(4) accent: vec4<f32>,
    @location(5) border: vec4<f32>,
    @location(6) ring: vec4<f32>,
    @location(7) header_h: f32,
    @location(8) radius: f32,
    @location(9) flags: u32,
    @location(10) kind: u32,
}

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) size_px: vec2<f32>,
    @location(2) @interpolate(flat) body: vec4<f32>,
    @location(3) @interpolate(flat) header: vec4<f32>,
    @location(4) @interpolate(flat) accent: vec4<f32>,
    @location(5) @interpolate(flat) border: vec4<f32>,
    @location(6) @interpolate(flat) ring: vec4<f32>,
    @location(7) @interpolate(flat) metrics: vec2<f32>,   // header_h px, radius px
    @location(8) @interpolate(flat) flags: u32,
    @location(9) @interpolate(flat) kind: u32,
}

const K_CARD: u32 = 0u;
const K_REROUTE: u32 = 1u;
const K_PILL: u32 = 2u;
const K_COMMENT: u32 = 3u;
const K_MARQUEE: u32 = 4u;
const K_WELL: u32 = 5u;
const K_ICON: u32 = 6u;
const K_GUIDE: u32 = 7u;

fn margin_px(kind: u32) -> f32 {
    let s = u.scale;
    switch kind {
        case 0u, 1u, 2u: { return 30.0 * s; }
        case 3u: { return 12.0 * s; }
        default: { return 3.0 * s; }
    }
}

@vertex
fn vs_main(i: Inst, @builtin(vertex_index) vi: u32) -> VOut {
    var cx = array<f32, 6>(0.0, 1.0, 0.0, 0.0, 1.0, 1.0);
    var cy = array<f32, 6>(0.0, 0.0, 1.0, 1.0, 0.0, 1.0);
    let a = to_screen(i.pos);
    let b = to_screen(i.pos + i.size);
    let m = margin_px(i.kind);
    let lo = a - vec2<f32>(m);
    let hi = b + vec2<f32>(m);
    let p = mix(lo, hi, vec2<f32>(cx[vi], cy[vi]));
    var o: VOut;
    o.pos = to_clip(p);
    o.local = p - a;
    o.size_px = b - a;
    o.body = i.body;
    o.header = i.header;
    o.accent = i.accent;
    o.border = i.border;
    o.ring = i.ring;
    o.metrics = vec2<f32>(i.header_h * kk(), i.radius * kk());
    o.flags = i.flags;
    o.kind = i.kind;
    return o;
}

fn desaturate(c: vec4<f32>, amount: f32) -> vec4<f32> {
    let l = dot(c.rgb, vec3<f32>(0.299, 0.587, 0.114));
    return vec4<f32>(mix(c.rgb, vec3<f32>(l), amount), c.a);
}

// Shadow, glow, fill, border and rings around a signed distance `d`.
// `fill` is straight RGBA already shaded for the interior.
fn shape(in: VOut, d: f32, fill: vec4<f32>, shadow_d: f32) -> vec4<f32> {
    let s = u.scale;
    let ks = clamp(u.zoom, 0.5, 1.5) * s;
    let hovered = flag(in.flags, F_HOVERED);
    let selected = flag(in.flags, F_SELECTED);
    let focused = flag(in.flags, F_FOCUSED);
    let is_active = flag(in.flags, F_ACTIVE);
    let flat_ = flag(in.flags, F_FLAT);
    let dragging = flag(in.flags, F_DRAGGING);
    let pulse = 0.5 + 0.5 * sin(u.time * 4.0);
    let bw = max(1.0, round(s));

    var out = vec4<f32>(0.0);

    // contact shadow (depth fades out with zoom; none in the flat overview)
    if !flat_ {
        let depth = smoothstep(0.3, 0.75, u.zoom);
        let lift = select(0.0, 0.22, dragging) + select(0.0, 0.05, hovered);
        let sh = 1.0 - smoothstep(-2.0 * ks, (14.0 + select(0.0, 8.0, dragging)) * ks, shadow_d);
        out = over(out, pm(vec4<f32>(0.0, 0.0, 0.0, sh * sh * (0.5 + lift) * depth)));
    }

    // outer glow: selection (soft white), activity (accent, animated)
    let outside = clamp(d, 0.0, 1000.0);
    if selected {
        let g = (1.0 - smoothstep(0.0, 9.0 * s, outside)) * 0.16;
        out = over(out, pm(vec4<f32>(in.ring.rgb, g * (1.0 - cover(d)))));
    }
    if is_active {
        let g = (1.0 - smoothstep(0.0, 14.0 * s, outside)) * (0.16 + 0.14 * pulse);
        out = over(out, pm(vec4<f32>(in.accent.rgb, g * (1.0 - cover(d)))));
    }

    // interior
    let c_body = cover(d);
    var body = fill;
    if hovered {
        body = vec4<f32>(body.rgb + vec3<f32>(0.022), body.a);
    }
    out = over(out, pm(vec4<f32>(body.rgb, body.a * c_body)));

    // inner activity glow
    if is_active {
        let inner = (1.0 - smoothstep(0.0, 6.0 * s, -d)) * c_body * (0.22 + 0.2 * pulse);
        out = over(out, pm(vec4<f32>(in.accent.rgb, inner)));
    }

    // hairline border
    var bcol = in.border;
    if selected {
        bcol = vec4<f32>(in.ring.rgb, 1.0);
    }
    if is_active {
        bcol = vec4<f32>(mix(bcol.rgb, in.accent.rgb, 0.55 + 0.3 * pulse), 1.0);
    }
    let band = c_body - cover(d + bw);
    out = over(out, pm(vec4<f32>(bcol.rgb, bcol.a * band)));

    // selection ring, hugging the edge
    if selected && !focused {
        let rw = 1.5 * s;
        let r = cover(d - rw) - cover(d);
        out = over(out, pm(vec4<f32>(in.ring.rgb, 0.95 * r)));
    }
    // focus ring, separated by a gap so it reads on top of selection
    if focused {
        let gap = 2.0 * s;
        let rw = 1.5 * s;
        let r = cover(d - gap - rw) - cover(d - gap);
        out = over(out, pm(vec4<f32>(in.ring.rgb, r)));
    }

    if flag(in.flags, F_DISABLED) {
        out = desaturate(out, 0.75);
        out = out * 0.55;
    }
    return out;
}

fn card(in: VOut) -> vec4<f32> {
    let s = u.scale;
    let half_size = in.size_px * 0.5;
    let p = in.local - half_size;
    let r = in.metrics.y;
    let d = sdf_rrect(p, half_size, r);
    let ks = clamp(u.zoom, 0.5, 1.5) * s;
    let sd = sdf_rrect(p - vec2<f32>(0.0, 5.0 * ks), half_size, r);

    let y = in.local.y;
    let hh = in.metrics.x;
    let flat_ = flag(in.flags, F_FLAT);
    let t = clamp(y / in.size_px.y, 0.0, 1.0);
    var fill = vec4<f32>(in.body.rgb * (1.035 - 0.07 * t), in.body.a);
    if y < hh {
        fill = vec4<f32>(mix(in.header.rgb, in.accent.rgb, select(0.05, 0.28, flat_)), in.header.a);
    }
    var out = shape(in, d, fill, sd);

    if !flat_ && hh > 0.0 {
        let c = cover(d);
        let bw = max(1.0, round(s));
        // hairline between header and body
        let sep = clamp(0.5 * bw + 0.5 - abs(y - hh), 0.0, 1.0);
        out = over(out, pm(vec4<f32>(0.0, 0.0, 0.0, 0.32 * sep * c)));
        // category accent: a gradient underline just above the separator
        let ul = max(1.5 * s, 2.0 * kk());
        let band = clamp(0.5 + (y - (hh - ul)), 0.0, 1.0) * clamp(0.5 - (y - hh), 0.0, 1.0);
        let xt = clamp(in.local.x / in.size_px.x, 0.0, 1.0);
        let a = mix(0.95, 0.12, smoothstep(0.0, 1.0, xt));
        let fade = select(1.0, 0.45, flag(in.flags, F_DISABLED));
        out = over(out, pm(vec4<f32>(in.accent.rgb, a * band * c * fade)));
    }
    return out;
}

fn reroute(in: VOut) -> vec4<f32> {
    let s = u.scale;
    let half_size = in.size_px * 0.5;
    let p = in.local - half_size;
    let rad = min(half_size.x, half_size.y);
    let d = length(p) - rad;
    let sd = length(p - vec2<f32>(0.0, 3.0 * s)) - rad;
    var out = shape(in, d, vec4<f32>(in.header.rgb, 1.0), sd);
    // centre dot: the node is a joint, not a card
    let dot_r = max(rad * 0.36, 1.2 * s);
    out = over(out, pm(vec4<f32>(in.accent.rgb, 0.9 * cover(length(p) - dot_r))));
    return out;
}

fn pill(in: VOut) -> vec4<f32> {
    let half_size = in.size_px * 0.5;
    let p = in.local - half_size;
    let d = sdf_rrect(p, half_size, half_size.y);
    let sd = sdf_rrect(p - vec2<f32>(0.0, 4.0 * clamp(u.zoom, 0.5, 1.5) * u.scale), half_size, half_size.y);
    var out = shape(in, d, vec4<f32>(mix(in.header.rgb, in.accent.rgb, 0.08), 1.0), sd);
    return out;
}

fn comment(in: VOut) -> vec4<f32> {
    let s = u.scale;
    let half_size = in.size_px * 0.5;
    let p = in.local - half_size;
    let r = in.metrics.y;
    let d = sdf_rrect(p, half_size, r);
    let c = cover(d);
    let bw = max(1.0, round(s));
    var out = vec4<f32>(0.0);
    let selected = flag(in.flags, F_SELECTED);
    let hovered = flag(in.flags, F_HOVERED);

    out = over(out, pm(vec4<f32>(in.body.rgb, in.body.a * c)));
    if in.local.y < in.metrics.x {
        out = over(out, pm(vec4<f32>(in.header.rgb, in.header.a * c)));
        // header rule
        let sep = clamp(0.5 * bw + 0.5 - abs(in.local.y - in.metrics.x), 0.0, 1.0);
        out = over(out, pm(vec4<f32>(in.accent.rgb, 0.16 * sep * c)));
    }
    let band = c - cover(d + bw);
    var bcol = in.border;
    if hovered {
        bcol = vec4<f32>(bcol.rgb, min(bcol.a * 1.8, 1.0));
    }
    if selected {
        bcol = vec4<f32>(in.ring.rgb, 0.9);
    }
    out = over(out, pm(vec4<f32>(bcol.rgb, bcol.a * band)));

    // resize grip: two short diagonal strokes in the bottom-right corner
    let q = in.size_px - in.local;
    if q.x < 18.0 * s && q.y < 18.0 * s {
        let g1 = clamp(0.5 * bw + 0.5 - abs((q.x + q.y) - 8.0 * s) * 0.7071, 0.0, 1.0);
        let g2 = clamp(0.5 * bw + 0.5 - abs((q.x + q.y) - 13.0 * s) * 0.7071, 0.0, 1.0);
        let gi = max(g1, g2) * select(0.22, 0.55, hovered);
        out = over(out, pm(vec4<f32>(in.accent.rgb, gi * c)));
    }
    return out;
}

fn marquee(in: VOut) -> vec4<f32> {
    let s = u.scale;
    let half_size = in.size_px * 0.5;
    let p = in.local - half_size;
    let d = sdf_rrect(p, half_size, in.metrics.y);
    let c = cover(d);
    let bw = max(1.0, round(s));
    let band = c - cover(d + bw);
    // dashed outline
    let dash = step(fract((in.local.x + in.local.y) / (9.0 * s)), 0.62);
    var out = pm(vec4<f32>(in.body.rgb, in.body.a * c));
    out = over(out, pm(vec4<f32>(in.border.rgb, in.border.a * band * mix(0.45, 1.0, dash))));
    return out;
}

fn well(in: VOut) -> vec4<f32> {
    let s = u.scale;
    let half_size = in.size_px * 0.5;
    let p = in.local - half_size;
    let d = sdf_rrect(p, half_size, in.metrics.y);
    let c = cover(d);
    // quiet checker so alpha previews read as "transparent", not as black
    let tile = 6.0 * s;
    let chk = (i32(floor(in.local.x / tile)) + i32(floor(in.local.y / tile))) & 1;
    let base = in.body.rgb + vec3<f32>(select(0.0, 0.018, chk == 1));
    var out = pm(vec4<f32>(base, c));
    // inset top shadow
    let top = (1.0 - smoothstep(0.0, 5.0 * s, in.local.y)) * 0.35;
    out = over(out, pm(vec4<f32>(0.0, 0.0, 0.0, top * c)));
    let bw = max(1.0, round(s));
    out = over(out, pm(vec4<f32>(in.border.rgb, in.border.a * (c - cover(d + bw)))));
    if flag(in.flags, F_DISABLED) {
        out = out * 0.5;
    }
    return out;
}

fn icon(in: VOut) -> vec4<f32> {
    let s = u.scale;
    let half_size = in.size_px * 0.5;
    let p = in.local - half_size;
    let d = sdf_rrect(p, half_size, in.metrics.y);
    let c = cover(d);
    let bw = max(1.0, round(s));
    var out = pm(vec4<f32>(in.body.rgb, in.body.a * c));
    out = over(out, pm(vec4<f32>(in.accent.rgb, 0.30 * (c - cover(d + bw)))));
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    switch in.kind {
        case 0u: { return card(in); }
        case 1u: { return reroute(in); }
        case 2u: { return pill(in); }
        case 3u: { return comment(in); }
        case 4u: { return marquee(in); }
        case 5u: { return well(in); }
        case 6u: { return icon(in); }
        default: { return pm(in.body); }
    }
}
