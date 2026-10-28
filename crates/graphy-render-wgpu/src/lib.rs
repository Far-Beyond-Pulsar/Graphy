//! # graphy-render-wgpu
//!
//! Shared GPU drawing for Graphy canvases. It consumes
//! [`graphy_render::DrawData`] and draws into a render pass the *host* owns,
//! with the host's `wgpu::Device`/`Queue`. It never creates an adapter, device
//! or surface, so it can live inside a GPUI/engine frame without a second
//! device.
//!
//! Draw order (one call per layer, all instanced):
//! grid -> comments -> wire halos -> wires -> cards -> pin tails -> pins ->
//! text -> overlays (marquee, guides, connection-drag wire).
//!
//! Typical host use:
//! ```ignore
//! let mut r = GraphRenderer::new(&device, format, FontSet { regular, bold: None })?;
//! let measure = r.text_measure();            // build the Scene with this
//! r.prepare(&device, &queue, &draw_data, scale, (w, h));
//! r.render(&mut pass);                        // inside the host's pass
//! ```

mod instances;
mod text;

use std::mem::size_of;

use graphy_render::DrawData;

pub use instances::{BoxInstance, PinInstance, TextVertex, Uniforms, WireInstance};
pub use text::{FontMeasure, FontSet, Fonts, TextEngine};

use instances::WIRE_VERTS;

const COMMON: &str = include_str!("shaders/common.wgsl");

fn shader_source(body: &str) -> String {
    format!("{COMMON}\n{body}")
}

/// All WGSL sources, each with the shared prelude, for validation tooling.
pub fn shader_sources() -> [(&'static str, String); 5] {
    [
        ("grid", shader_source(include_str!("shaders/grid.wgsl"))),
        ("box", shader_source(include_str!("shaders/box.wgsl"))),
        ("pin", shader_source(include_str!("shaders/pin.wgsl"))),
        ("wire", shader_source(include_str!("shaders/wire.wgsl"))),
        ("text", shader_source(include_str!("shaders/text.wgsl"))),
    ]
}

struct Grow {
    buf: wgpu::Buffer,
    cap: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl Grow {
    fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Self {
        let cap = 4096;
        Self {
            buf: Self::make(device, label, usage, cap),
            cap,
            usage,
            label,
        }
    }

    fn make(
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsages,
        cap: u64,
    ) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: cap,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let need = bytes.len() as u64;
        if need > self.cap {
            self.cap = need.next_power_of_two();
            self.buf = Self::make(device, self.label, self.usage, self.cap);
        }
        queue.write_buffer(&self.buf, 0, bytes);
    }
}

#[derive(Default, Clone, Copy)]
struct Ranges {
    comments: (u32, u32),
    nodes: (u32, u32),
    overlay_boxes: (u32, u32),
    pins: u32,
    wires: u32,
    halo_from: u32,
    tails: (u32, u32),
    overlay_wires: (u32, u32),
    text_verts: u32,
}

pub struct GraphRenderer {
    uniform_buf: wgpu::Buffer,
    uniform_bg: wgpu::BindGroup,
    grid: wgpu::RenderPipeline,
    boxes: wgpu::RenderPipeline,
    wires: wgpu::RenderPipeline,
    halo: wgpu::RenderPipeline,
    pins: wgpu::RenderPipeline,
    text_pipe: wgpu::RenderPipeline,
    box_buf: Grow,
    pin_buf: Grow,
    wire_buf: Grow,
    text_buf: Grow,
    text: TextEngine,
    srgb: bool,
    ranges: Ranges,
    // scratch reused across frames
    box_scratch: Vec<BoxInstance>,
    wire_scratch: Vec<WireInstance>,
    text_scratch: Vec<TextVertex>,
}

const BOX_ATTRS: [wgpu::VertexAttribute; 11] = wgpu::vertex_attr_array![
    0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4,
    5 => Float32x4, 6 => Float32x4, 7 => Float32, 8 => Float32, 9 => Uint32, 10 => Uint32
];
const PIN_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    0 => Float32x2, 1 => Float32, 2 => Uint32, 3 => Float32x4, 4 => Uint32, 5 => Uint32
];
const WIRE_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Uint32x4, 5 => Float32x4
];
const TEXT_ATTRS: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];

const MAX_BLEND: wgpu::BlendComponent = wgpu::BlendComponent {
    src_factor: wgpu::BlendFactor::One,
    dst_factor: wgpu::BlendFactor::One,
    operation: wgpu::BlendOperation::Max,
};

impl GraphRenderer {
    /// Builds pipelines on the host's device for render targets of `format`.
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        fonts: FontSet,
    ) -> Result<Self, &'static str> {
        let fonts = Fonts::load(&fonts)?;
        let text = TextEngine::new(device, fonts);

        let uni_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("graphy_uniform_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("graphy_uniforms"),
            size: size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("graphy_uniform_bg"),
            layout: &uni_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        let layout = |label, groups: &[Option<&wgpu::BindGroupLayout>]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: groups,
                immediate_size: 0,
            })
        };
        let base_layout = layout("graphy_layout", &[Some(&uni_bgl)]);
        let text_layout = layout("graphy_text_layout", &[Some(&uni_bgl), Some(&text.bgl)]);

        let premult = Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let max_blend = Some(wgpu::BlendState {
            color: MAX_BLEND,
            alpha: MAX_BLEND,
        });

        let make = |label: &'static str,
                    src: &str,
                    vs: &'static str,
                    fs: &'static str,
                    layout: &wgpu::PipelineLayout,
                    buffer: Option<wgpu::VertexBufferLayout<'_>>,
                    blend: Option<wgpu::BlendState>| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(shader_source(src).into()),
            });
            let buffers: Vec<Option<wgpu::VertexBufferLayout<'_>>> =
                buffer.into_iter().map(Some).collect();
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vs),
                    buffers: &buffers,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let inst =
            |stride: usize, attrs: &'static [wgpu::VertexAttribute]| wgpu::VertexBufferLayout {
                array_stride: stride as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: attrs,
            };

        let grid = make(
            "graphy_grid",
            include_str!("shaders/grid.wgsl"),
            "vs_main",
            "fs_main",
            &base_layout,
            None,
            None,
        );
        let boxes = make(
            "graphy_box",
            include_str!("shaders/box.wgsl"),
            "vs_main",
            "fs_main",
            &base_layout,
            Some(inst(size_of::<BoxInstance>(), &BOX_ATTRS)),
            premult,
        );
        let pins = make(
            "graphy_pin",
            include_str!("shaders/pin.wgsl"),
            "vs_main",
            "fs_main",
            &base_layout,
            Some(inst(size_of::<PinInstance>(), &PIN_ATTRS)),
            premult,
        );
        let wires = make(
            "graphy_wire",
            include_str!("shaders/wire.wgsl"),
            "vs_main",
            "fs_main",
            &base_layout,
            Some(inst(size_of::<WireInstance>(), &WIRE_ATTRS)),
            premult,
        );
        let halo = make(
            "graphy_wire_halo",
            include_str!("shaders/wire.wgsl"),
            "vs_halo",
            "fs_halo",
            &base_layout,
            Some(inst(size_of::<WireInstance>(), &WIRE_ATTRS)),
            max_blend,
        );
        let text_pipe = make(
            "graphy_text",
            include_str!("shaders/text.wgsl"),
            "vs_main",
            "fs_main",
            &text_layout,
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<TextVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &TEXT_ATTRS,
            }),
            premult,
        );

        let vb = wgpu::BufferUsages::VERTEX;
        Ok(Self {
            uniform_buf,
            uniform_bg,
            grid,
            boxes,
            wires,
            halo,
            pins,
            text_pipe,
            box_buf: Grow::new(device, "graphy_boxes", vb),
            pin_buf: Grow::new(device, "graphy_pins", vb),
            wire_buf: Grow::new(device, "graphy_wires", vb),
            text_buf: Grow::new(device, "graphy_text_verts", vb),
            text,
            srgb: format.is_srgb(),
            ranges: Ranges::default(),
            box_scratch: Vec::new(),
            wire_scratch: Vec::new(),
            text_scratch: Vec::new(),
        })
    }

    /// Text measurer backed by the same fonts the renderer draws with. Build
    /// the `Scene` with this so layout and glyphs agree.
    pub fn text_measure(&self) -> FontMeasure {
        self.text.measure()
    }

    /// Uploads one frame. `scale` is the display scale factor and `target` the
    /// physical size of the render target.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        d: &DrawData,
        scale: f32,
        target: (u32, u32),
    ) {
        queue.write_buffer(
            &self.uniform_buf,
            0,
            bytemuck::bytes_of(&Uniforms::new(d, scale, target, self.srgb)),
        );

        let srgb = self.srgb;
        // boxes: comments | nodes | overlays
        self.box_scratch.clear();
        let mark = |src: &[graphy_render::BoxDraw], s: &mut Vec<BoxInstance>| {
            let a = s.len() as u32;
            s.extend(src.iter().map(|b| BoxInstance::new(b, srgb)));
            (a, s.len() as u32)
        };
        self.ranges.comments = mark(&d.comments, &mut self.box_scratch);
        self.ranges.nodes = mark(&d.nodes, &mut self.box_scratch);
        self.ranges.overlay_boxes = mark(&d.overlay_boxes, &mut self.box_scratch);
        self.box_buf
            .write(device, queue, bytemuck::cast_slice(&self.box_scratch));

        let pins: Vec<PinInstance> = d.pins.iter().map(|p| PinInstance::new(p, srgb)).collect();
        self.ranges.pins = pins.len() as u32;
        self.pin_buf
            .write(device, queue, bytemuck::cast_slice(&pins));

        // wires: scene | tails | overlay
        self.wire_scratch.clear();
        self.wire_scratch
            .extend(d.wires.iter().map(|w| WireInstance::new(w, srgb)));
        self.ranges.wires = self.wire_scratch.len() as u32;
        self.ranges.halo_from = d
            .wires
            .iter()
            .position(|w| w.flags & graphy_render::flags::EMPHASIS != 0)
            .unwrap_or(d.wires.len()) as u32;
        let t0 = self.wire_scratch.len() as u32;
        self.wire_scratch
            .extend(d.tails.iter().map(|w| WireInstance::new(w, srgb)));
        self.ranges.tails = (t0, self.wire_scratch.len() as u32);
        let o0 = self.wire_scratch.len() as u32;
        self.wire_scratch
            .extend(d.overlay_wires.iter().map(|w| WireInstance::new(w, srgb)));
        self.ranges.overlay_wires = (o0, self.wire_scratch.len() as u32);
        self.wire_buf
            .write(device, queue, bytemuck::cast_slice(&self.wire_scratch));

        self.text
            .layout(&d.texts, &d.view, scale, self.srgb, &mut self.text_scratch);
        self.ranges.text_verts = self.text_scratch.len() as u32;
        self.text.upload(device, queue);
        self.text_buf
            .write(device, queue, bytemuck::cast_slice(&self.text_scratch));
    }

    /// Draws the prepared frame into the host's pass. The grid pass fills the
    /// whole target, so no clear is required.
    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        let r = &self.ranges;
        pass.set_bind_group(0, &self.uniform_bg, &[]);

        pass.set_pipeline(&self.grid);
        pass.draw(0..6, 0..1);

        pass.set_pipeline(&self.boxes);
        pass.set_vertex_buffer(0, self.box_buf.buf.slice(..));
        if r.comments.1 > r.comments.0 {
            pass.draw(0..6, r.comments.0..r.comments.1);
        }

        pass.set_vertex_buffer(0, self.wire_buf.buf.slice(..));
        if r.wires > r.halo_from {
            pass.set_pipeline(&self.halo);
            pass.draw(0..WIRE_VERTS, r.halo_from..r.wires);
        }
        if r.wires > 0 {
            pass.set_pipeline(&self.wires);
            pass.draw(0..WIRE_VERTS, 0..r.wires);
        }

        pass.set_pipeline(&self.boxes);
        pass.set_vertex_buffer(0, self.box_buf.buf.slice(..));
        if r.nodes.1 > r.nodes.0 {
            pass.draw(0..6, r.nodes.0..r.nodes.1);
        }

        if r.tails.1 > r.tails.0 {
            pass.set_pipeline(&self.wires);
            pass.set_vertex_buffer(0, self.wire_buf.buf.slice(..));
            pass.draw(0..WIRE_VERTS, r.tails.0..r.tails.1);
        }

        if r.pins > 0 {
            pass.set_pipeline(&self.pins);
            pass.set_vertex_buffer(0, self.pin_buf.buf.slice(..));
            pass.draw(0..6, 0..r.pins);
        }

        if r.text_verts > 0 {
            if let Some(bg) = &self.text.bind_group {
                pass.set_pipeline(&self.text_pipe);
                pass.set_bind_group(1, bg, &[]);
                pass.set_vertex_buffer(0, self.text_buf.buf.slice(..));
                pass.draw(0..r.text_verts, 0..1);
            }
        }

        if r.overlay_boxes.1 > r.overlay_boxes.0 {
            pass.set_pipeline(&self.boxes);
            pass.set_vertex_buffer(0, self.box_buf.buf.slice(..));
            pass.draw(0..6, r.overlay_boxes.0..r.overlay_boxes.1);
        }
        if r.overlay_wires.1 > r.overlay_wires.0 {
            pass.set_vertex_buffer(0, self.wire_buf.buf.slice(..));
            pass.set_pipeline(&self.halo);
            pass.draw(0..WIRE_VERTS, r.overlay_wires.0..r.overlay_wires.1);
            pass.set_pipeline(&self.wires);
            pass.draw(0..WIRE_VERTS, r.overlay_wires.0..r.overlay_wires.1);
        }
    }

    /// Convenience for hosts that own only a target view: prepares, records
    /// one pass and submits it.
    pub fn render_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        target: (u32, u32),
        d: &DrawData,
        scale: f32,
    ) {
        self.prepare(device, queue, d, scale, target);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("graphy_encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("graphy_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.render(&mut pass);
        }
        queue.submit(Some(encoder.finish()));
    }
}
