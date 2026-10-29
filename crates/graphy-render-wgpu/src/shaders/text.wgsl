// Glyph quads: coverage from an R8 atlas rasterised at the exact physical pixel
// size and positioned on whole pixels by the CPU, so text stays sharp at any
// zoom. Positions arrive in physical pixels.

@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct Vert {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(v: Vert) -> VOut {
    var o: VOut;
    o.pos = to_clip(v.pos);
    o.uv = v.uv;
    o.color = v.color;
    return o;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let cov = textureSample(atlas, atlas_sampler, in.uv).r;
    // light-on-dark text reads thin; a mild curve restores stroke weight
    let a = pow(cov, 0.78) * in.color.a;
    return vec4<f32>(in.color.rgb * a, a);
}
