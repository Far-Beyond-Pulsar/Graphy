//! Renders the review scenes to PNG files, offscreen (no window).
//!
//!     cargo run --release -p graphy-render-wgpu --example render_scenes -- [out_dir] [--font file.ttf] [--bold file.ttf]
//!
//! Writes, for the clean and crowded sample graphs: normal, hover, selection,
//! connection-drag, low/overview/high zoom, fractional zoom, angular wires and
//! a 2x high-DPI frame. Compare them side by side with
//! `docs/mockups/animation-editor.html` at the same canvas size.
//!
//! It creates its own wgpu device because it is a standalone tool; the library
//! itself only ever uses the host's.

use std::path::{Path, PathBuf};

use graphy::GraphDescription;
use graphy_render::samples::{clean_scene, crowded_scene, AngularPresenter, SamplePresenter};
use graphy_render::*;
use graphy_render_wgpu::{FontSet, GraphRenderer};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

fn find_fonts() -> (Option<PathBuf>, Option<PathBuf>) {
    let candidates: [(&str, &str); 4] = [
        (
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\seguisb.ttf",
        ),
        (
            "/System/Library/Fonts/SFNS.ttf",
            "/System/Library/Fonts/SFNS.ttf",
        ),
        (
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        ),
        (
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
        ),
    ];
    for (r, b) in candidates {
        if Path::new(r).exists() {
            return (Some(r.into()), Path::new(b).exists().then(|| b.into()));
        }
    }
    (None, None)
}

struct Frame<'a> {
    name: &'a str,
    graph: &'a GraphDescription,
    presenter: &'a dyn Presenter,
    size: (u32, u32),
    scale: f32,
    view: ViewTransform,
    state: InteractionState,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut out_dir = PathBuf::from("target/graphy-scenes");
    let (mut font, mut bold) = find_fonts();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--font" => font = args.next().map(PathBuf::from),
            "--bold" => bold = args.next().map(PathBuf::from),
            other => out_dir = PathBuf::from(other),
        }
    }
    let font = font.expect("no system font found; pass --font <file.ttf>");
    std::fs::create_dir_all(&out_dir).unwrap();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("no GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("device");
    println!("adapter: {}", adapter.get_info().name);

    let mut renderer = GraphRenderer::new(
        &device,
        FORMAT,
        FontSet {
            regular: std::fs::read(&font).unwrap(),
            bold: bold.map(|b| std::fs::read(b).unwrap()),
        },
    )
    .expect("fonts");
    let measure = renderer.text_measure();

    let clean = clean_scene();
    let crowded = crowded_scene(14, 8);
    let vp = Vec2::new(1280.0, 720.0);
    let clean_bounds = {
        let mut s = Scene::new(LayoutMetrics::default());
        s.rebuild(&clean, &SamplePresenter, &measure);
        s.bounds().unwrap()
    };
    let fit = ViewTransform::framing(clean_bounds, vp, 56.0, 1.0);

    let mut frames: Vec<Frame> = Vec::new();
    let base = |name, graph, presenter, view: ViewTransform, state: InteractionState| Frame {
        name,
        graph,
        presenter,
        size: (1280, 720),
        scale: 1.0,
        view,
        state,
    };

    frames.push(base(
        "clean-01-normal",
        &clean,
        &SamplePresenter,
        fit,
        InteractionState::default(),
    ));

    let mut hover = InteractionState::default();
    hover.hovered = Some(Hit::Node {
        node: 0,
        part: NodePart::Header,
    });
    // resolved below once the scene exists
    frames.push(base("clean-02-hover", &clean, &SamplePresenter, fit, hover));

    let mut sel = InteractionState::default();
    sel.selected_nodes.insert("branch".into());
    sel.selected_nodes.insert("print".into());
    sel.focused_node = Some("print".into());
    frames.push(base(
        "clean-03-selection",
        &clean,
        &SamplePresenter,
        fit,
        sel,
    ));

    frames.push(base(
        "clean-04-connect-drag",
        &clean,
        &SamplePresenter,
        fit,
        InteractionState::default(),
    ));
    frames.push(base(
        "clean-05-angular",
        &clean,
        &AngularPresenter,
        fit,
        InteractionState::default(),
    ));

    frames.push(base(
        "clean-06-zoom-summary",
        &clean,
        &SamplePresenter,
        ViewTransform {
            pan: Vec2::new(947.0, 605.0),
            zoom: 0.45,
        },
        InteractionState::default(),
    ));
    frames.push(base(
        "clean-07-zoom-overview",
        &clean,
        &SamplePresenter,
        ViewTransform {
            pan: Vec2::new(2725.0, 1605.0),
            zoom: 0.2,
        },
        InteractionState::default(),
    ));
    frames.push(base(
        "clean-08-zoom-close",
        &clean,
        &SamplePresenter,
        ViewTransform {
            pan: Vec2::new(-350.0, -60.0),
            zoom: 2.2,
        },
        InteractionState::default(),
    ));
    frames.push(base(
        "clean-09-fractional-zoom",
        &clean,
        &SamplePresenter,
        ViewTransform {
            pan: Vec2::new(280.0, 80.0),
            zoom: 1.37,
        },
        InteractionState::default(),
    ));
    let mut hi = base(
        "clean-10-highdpi-2x",
        &clean,
        &SamplePresenter,
        fit,
        InteractionState::default(),
    );
    hi.scale = 2.0;
    hi.size = (2560, 1440);
    frames.push(hi);

    let cb = {
        let mut s = Scene::new(LayoutMetrics::default());
        s.rebuild(&crowded, &SamplePresenter, &measure);
        s.bounds().unwrap()
    };
    frames.push(base(
        "crowded-01-detail",
        &crowded,
        &SamplePresenter,
        ViewTransform {
            pan: Vec2::new(40.0, 70.0),
            zoom: 0.9,
        },
        InteractionState::default(),
    ));
    let mut csel = InteractionState::default();
    csel.selected_nodes.insert("n2_3".into());
    csel.selected_nodes.insert("n3_4".into());
    frames.push(base(
        "crowded-02-selection",
        &crowded,
        &SamplePresenter,
        ViewTransform {
            pan: Vec2::new(-560.0, -90.0),
            zoom: 0.9,
        },
        csel,
    ));
    frames.push(base(
        "crowded-03-summary",
        &crowded,
        &SamplePresenter,
        ViewTransform {
            pan: Vec2::new(60.0, 60.0),
            zoom: 0.5,
        },
        InteractionState::default(),
    ));
    frames.push(base(
        "crowded-04-overview",
        &crowded,
        &SamplePresenter,
        ViewTransform::framing(cb, vp, 24.0, 1.0),
        InteractionState::default(),
    ));
    frames.push(base(
        "crowded-05-angular",
        &crowded,
        &AngularPresenter,
        ViewTransform {
            pan: Vec2::new(40.0, 70.0),
            zoom: 0.9,
        },
        InteractionState::default(),
    ));

    for mut f in frames {
        let mut scene = Scene::new(LayoutMetrics::default());
        scene.rebuild(f.graph, f.presenter, &measure);
        if f.name == "clean-02-hover" {
            let i = scene.node_index("branch").unwrap();
            f.state.hovered = Some(Hit::Node {
                node: i,
                part: NodePart::Body,
            });
        }
        if f.name == "clean-04-connect-drag" {
            let anchor_node = scene.node_index("speed").unwrap();
            let anchor = PinAddr {
                node: anchor_node,
                side: PinSide::Output,
                pin: 0,
            };
            let ty = f.graph.nodes["speed"].outputs[0].data_type().clone();
            let mut compatible = std::collections::HashSet::new();
            for (ni, n) in scene.nodes.iter().enumerate() {
                if ni == anchor_node {
                    continue;
                }
                for (pi, p) in f.graph.nodes[&n.id].inputs.iter().enumerate() {
                    if p.data_type() == &ty {
                        compatible.insert(PinAddr {
                            node: ni,
                            side: PinSide::Input,
                            pin: pi,
                        });
                    }
                }
            }
            let target = scene.node("fmt").unwrap();
            let cursor = target.pin_pos(PinSide::Input, 0).unwrap() + Vec2::new(-26.0, 6.0);
            f.state.connecting = Some(ConnectPreview {
                anchor,
                cursor,
                snapped: None,
                routing: WireRouting::Bezier,
                compatible,
            });
        }
        let logical = Vec2::new(f.size.0 as f32 / f.scale, f.size.1 as f32 / f.scale);
        let mut params = DrawParams::new(logical, f.view);
        params.time = 1.0;
        let data = scene.build_draw_data(&params, &f.state, &measure);
        let path = out_dir.join(format!("{}.png", f.name));
        render_png(
            &device,
            &queue,
            &mut renderer,
            &data,
            f.size,
            f.scale,
            &path,
        );
        println!(
            "{:<28} lod={:?} nodes={} wires={} pins={} texts={}",
            f.name, data.lod, data.stats.nodes, data.stats.wires, data.stats.pins, data.stats.texts
        );
    }
    println!("wrote {}", out_dir.display());
}

fn render_png(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut GraphRenderer,
    data: &DrawData,
    size: (u32, u32),
    scale: f32,
    path: &Path,
) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene"),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    renderer.render_to_view(device, queue, &view, size, data, scale);

    let padded = (size.0 * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (padded * size.1) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(size.1),
            },
        },
        wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(enc.finish()));
    let slice = buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let mapped = slice.get_mapped_range().unwrap();
    let mut pixels = Vec::with_capacity((size.0 * size.1 * 4) as usize);
    for row in 0..size.1 {
        let s = (row * padded) as usize;
        pixels.extend_from_slice(&mapped[s..s + (size.0 * 4) as usize]);
    }
    drop(mapped);
    buf.unmap();

    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), size.0, size.1);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .unwrap()
        .write_image_data(&pixels)
        .unwrap();
}
