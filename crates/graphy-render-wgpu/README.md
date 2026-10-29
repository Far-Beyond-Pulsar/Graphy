# graphy-render-wgpu

Shared GPU drawing for Graphy graph canvases. Consumes
`graphy_render::DrawData`, draws into a render pass **the host owns**, on the
host's `wgpu::Device`/`Queue` (no second device, adapter or surface).

```rust
let mut r = GraphRenderer::with_fonts(&device, surface_format, fonts);
let measure = fonts.measure();            // build the Scene with this
r.prepare(&device, &queue, &draw_data, scale_factor, (w, h));
r.render(&mut pass);                      // or r.render_to_view(...)
```

Targets: both `*Unorm` and `*Srgb` formats work. Colours are authored in display
space and linearised on the CPU for `*Srgb` targets (GPUI surfaces are
`Bgra8UnormSrgb`; `GraphCanvas` turns the linearisation off with `set_linearize_colors(false)`, because the window shows the surface without sRGB encoding and linearised colours came out far too dark).

## Viewing the review scenes

No window needed; the example renders offscreen to PNG with its own device:

```
cargo run --release -p graphy-render-wgpu --example render_scenes -- [out_dir] [--font f.ttf] [--bold b.ttf]
```

Default output `target/graphy-scenes/`: clean scene (normal, hover, selection +
focus, connection drag with compatible-pin feedback, angular wires, summary /
overview / close / fractional zoom, 2x high DPI) and a crowded 14x8 scene
(detail, selection, summary, overview, angular). Compare with
`docs/mockups/animation-editor.html` at the same canvas size.

## Visual system (canvas only)

Tokens live in `graphy_render::Theme` (no colour is hard-coded in a shader
except white/black mixes).

| Element | Treatment |
|---|---|
| Canvas | near-black `#0d0d0d`, multi-level grid: levels at `20 * 5^j` units fade in/out by on-screen spacing so density stays constant across zoom; line centres snapped to pixel centres; 1 logical px wide at any DPI |
| Card | `#171717` body with a faint vertical gradient, `#202020` header, 1 physical-px hairline border, soft contact shadow (fades out with zoom), radius 6 |
| Category | only a thin gradient underline under the header (95% -> 12% across the width) plus the icon chip tint; never a flood fill |
| Pins | SDF shapes (circle, exec arrow, diamond, square, hexagon); connected = solid, open = hollow outline; container types get a second ring; hit target is far larger than the glyph |
| Wires | GPU-evaluated bezier or rounded-orthogonal routes as capsule segments, butt-joined (no beading); resting wires sit back (22% toward canvas), selection/hover emphasises touching wires with a soft max-blended halo and dims the rest |
| States | hover: lighter border + lift; selected: tight light ring + glow; focused: separated blue ring; active: accent pulse and travelling wire packet; disabled: desaturated; dragging: deeper shadow |
| LOD | `Overview` (<0.30 zoom) flat silhouettes, coarse wires, no text/pins; `Summary` (<0.62) enlarged titles + pin dots; `Detail` labels, inline widgets, previews |
| Text | glyph atlas rasterised at the exact physical pixel size (half-pixel steps), quads placed on whole pixels |

## Cost model

One instanced draw per layer (comments, halos, wires, cards, tails, pins, text,
overlays): the draw-call count does not depend on node count. Culling happens in
`graphy-render` (R-tree) before upload; wire curves are evaluated in the vertex
stage; no per-node UI tree.
