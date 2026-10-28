// Canvas: a quiet infinite grid.
//
// Lines come from several geometric levels (spacing = step * 5^j). Each level
// fades in as its on-screen spacing grows past ~10 px and fades out past
// several hundred, and its intensity moves from "minor" to "major" with
// spacing. A graph line shared by two levels takes the stronger one, so
// zooming never pops and the apparent grid density stays constant. Line
// centres are snapped to pixel centres so panning does not shimmer.

struct VOut {
    @builtin(position) pos: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VOut {
    var v = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    );
    var o: VOut;
    o.pos = vec4<f32>(v[vi], 0.0, 1.0);
    return o;
}

fn level_alpha(spacing_px: f32) -> f32 {
    let fade_in = smoothstep(9.0 * u.scale, 26.0 * u.scale, spacing_px);
    let fade_out = 1.0 - smoothstep(260.0 * u.scale, 700.0 * u.scale, spacing_px);
    return fade_in * fade_out;
}

fn level_strength(spacing_px: f32) -> f32 {
    return mix(u.grid_minor.a, u.grid_major.a, smoothstep(36.0 * u.scale, 150.0 * u.scale, spacing_px));
}

fn line_cov(coord: f32, origin: f32, spacing: f32, lw: f32) -> f32 {
    let n = round((coord - origin) / spacing);
    var pos = origin + n * spacing;
    // snap the line centre so odd widths sit on pixel centres
    let odd = (i32(lw) % 2) == 1;
    pos = select(round(pos), floor(pos) + 0.5, odd);
    return clamp(0.5 * lw + 0.5 - abs(coord - pos), 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let px = frag.xy;
    let origin = u.pan * kk();
    let s0 = u.grid_params.x * kk();
    let lw = max(1.0, round(u.scale));

    // first level whose spacing reaches ~5 px; five levels cover the usable range
    let j0 = floor(log2(5.0 * u.scale / s0) / log2(u.grid_params.y));
    var alpha = 0.0;
    for (var i = 0; i < 5; i = i + 1) {
        let j = j0 + f32(i);
        let spacing = s0 * pow(u.grid_params.y, j);
        let w = level_alpha(spacing);
        if w > 0.001 {
            let c = max(line_cov(px.x, origin.x, spacing, lw), line_cov(px.y, origin.y, spacing, lw));
            alpha = max(alpha, c * w * level_strength(spacing));
        }
    }
    let line = vec3<f32>(1.0);
    let col = mix(u.canvas.rgb, line, alpha);
    return vec4<f32>(col, 1.0);
}
