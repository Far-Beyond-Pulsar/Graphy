// Pins. One quad per pin, shape from a signed distance in physical pixels.
//   connected  solid type colour
//   open       hollow outline with a faint tinted core
//   ringed     second outline (containers / arrays)
//   hovered    slightly larger with a soft halo
//   compatible pulsing halo while a connection is being dragged
//   dim        pins that cannot accept the dragged connection

struct Inst {
    @location(0) center: vec2<f32>,
    @location(1) size: f32,
    @location(2) flags: u32,
    @location(3) color: vec4<f32>,
    @location(4) shape: u32,
    @location(5) input: u32,
}

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) radius: f32,
    @location(2) @interpolate(flat) color: vec4<f32>,
    @location(3) @interpolate(flat) flags: u32,
    @location(4) @interpolate(flat) shape: u32,
}

@vertex
fn vs_main(i: Inst, @builtin(vertex_index) vi: u32) -> VOut {
    var cx = array<f32, 6>(-1.0, 1.0, -1.0, -1.0, 1.0, 1.0);
    var cy = array<f32, 6>(-1.0, -1.0, 1.0, 1.0, -1.0, 1.0);
    let c = to_screen(i.center);
    let boost = select(1.0, 1.22, flag(i.flags, F_HOVERED) || flag(i.flags, F_COMPATIBLE));
    let r = max(i.size * kk() * 0.5 * boost, 2.0 * u.scale);
    let ext = r + 14.0 * u.scale;
    let local = vec2<f32>(cx[vi], cy[vi]) * ext;
    var o: VOut;
    o.pos = to_clip(c + local);
    o.local = local;
    o.radius = r;
    o.color = i.color;
    o.flags = i.flags;
    o.shape = i.shape;
    return o;
}

fn sd_triangle(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> f32 {
    let e0 = b - a; let e1 = c - b; let e2 = a - c;
    let v0 = p - a; let v1 = p - b; let v2 = p - c;
    let pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    let pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    let pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
    let s = sign(e0.x * e2.y - e0.y * e2.x);
    let d0 = vec2<f32>(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x));
    let d1 = vec2<f32>(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x));
    let d2 = vec2<f32>(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x));
    let d = min(min(d0, d1), d2);
    return -sqrt(d.x) * sign(d.y);
}

fn sd_hexagon(p: vec2<f32>, r: f32) -> f32 {
    let k = vec3<f32>(-0.866025404, 0.5, 0.577350269);
    var q = abs(p);
    q = q - 2.0 * min(dot(k.xy, q), 0.0) * k.xy;
    q = q - vec2<f32>(clamp(q.x, -k.z * r, k.z * r), r);
    return length(q) * sign(q.y);
}

fn pin_sd(shape: u32, p: vec2<f32>, r: f32) -> f32 {
    switch shape {
        case 1u: {
            // execution arrow: soft right-pointing pentagon
            return sd_triangle(p, vec2<f32>(-r * 0.78, -r * 0.98), vec2<f32>(r * 1.0, 0.0), vec2<f32>(-r * 0.78, r * 0.98));
        }
        case 2u: { return (abs(p.x) + abs(p.y)) * 0.70710678 - r * 0.98; }
        case 3u: {
            let q = abs(p) - vec2<f32>(r * 0.8);
            return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - 1.4 * u.scale;
        }
        case 4u: { return sd_hexagon(p, r * 0.95); }
        default: { return length(p) - r; }
    }
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let s = u.scale;
    let r = in.radius;
    let lw = max(1.4 * s, 1.0);
    let d = pin_sd(in.shape, in.local, r);
    let connected = flag(in.flags, F_CONNECTED);
    let hovered = flag(in.flags, F_HOVERED);
    let compat = flag(in.flags, F_COMPATIBLE);
    let pulse = 0.5 + 0.5 * sin(u.time * 5.0);
    var out = vec4<f32>(0.0);

    // halo (hover, compatible target)
    if hovered || compat {
        let amt = select(0.20, 0.22 + 0.22 * pulse, compat);
        let h = (1.0 - smoothstep(0.0, 9.0 * s, max(d, 0.0))) * amt;
        out = over(out, pm(vec4<f32>(mix(in.color.rgb, vec3<f32>(1.0), 0.35), h * (1.0 - cover(d)))));
    }

    // second outline for containers
    if flag(in.flags, F_RINGED) {
        let ring_d = pin_sd(in.shape, in.local, r + 2.8 * s);
        let band = cover(ring_d) - cover(ring_d + 1.0 * s);
        out = over(out, pm(vec4<f32>(in.color.rgb, 0.7 * band)));
    }

    let c = cover(d);
    var fill_a = 1.0;
    var fill = in.color.rgb;
    if !connected {
        // open pin: dark core, tinted slightly, outlined in the type colour
        fill = mix(u.card_body.rgb * 0.7, in.color.rgb, 0.16);
    }
    out = over(out, pm(vec4<f32>(fill, fill_a * c)));
    // outline on open pins; a brighter rim on connected ones keeps shape legible
    let rim = c - cover(d + lw);
    let rim_col = select(in.color.rgb, mix(in.color.rgb, vec3<f32>(1.0), 0.28), connected);
    out = over(out, pm(vec4<f32>(rim_col, rim)));

    if flag(in.flags, F_DIM) {
        out = out * 0.42;
    }
    if flag(in.flags, F_DISABLED) {
        out = out * 0.6;
    }
    return out;
}
