//! Headless shader validation: every WGSL source must parse and validate in
//! naga, and the Rust-side instance layouts must match the shader inputs.

use graphy_render_wgpu::{
    shader_sources, BoxInstance, PinInstance, TextVertex, Uniforms, WireInstance,
};

#[test]
fn all_shaders_validate() {
    for (name, src) in shader_sources() {
        let module = naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&src)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&src)));
    }
}

#[test]
fn instance_layouts_are_tightly_packed() {
    use std::mem::size_of;
    assert_eq!(size_of::<BoxInstance>(), 2 * 4 + 2 * 4 + 5 * 16 + 4 * 4);
    assert_eq!(size_of::<PinInstance>(), 4 * 4 + 16 + 2 * 4);
    assert_eq!(size_of::<WireInstance>(), 6 * 16);
    assert_eq!(size_of::<TextVertex>(), 32);
    // uniform block: 16-byte multiple, mirrors `Uniforms` in common.wgsl
    assert_eq!(size_of::<Uniforms>() % 16, 0);
    assert_eq!(size_of::<Uniforms>(), 16 + 16 + 5 * 16 - 16 + 16);
}
