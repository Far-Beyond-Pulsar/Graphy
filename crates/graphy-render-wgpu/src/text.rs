//! Glyph atlas text.
//!
//! Glyphs are rasterised by `fontdue` at the exact physical pixel size the
//! current zoom and display scale call for (quantised to half pixels), and each
//! quad is placed on a whole pixel. That keeps stems crisp at fractional zoom
//! and high DPI instead of resampling a fixed-size bitmap.
//!
//! Layout widths (used to size nodes) come from [`FontMeasure`], which scales
//! unhinted advances linearly, so a node's size never depends on the zoom.

use std::collections::HashMap;
use std::sync::Arc;

use graphy_render::{TextAlign, TextDraw, TextMeasure, ViewTransform};

use crate::instances::TextVertex;

pub const ATLAS_SIZE: u32 = 2048;
const REF_PX: f32 = 100.0;

/// Font bytes supplied by the host. `bold` falls back to `regular`.
pub struct FontSet {
    pub regular: Vec<u8>,
    pub bold: Option<Vec<u8>>,
}

#[derive(Clone)]
pub struct Fonts {
    regular: Arc<fontdue::Font>,
    bold: Arc<fontdue::Font>,
}

impl Fonts {
    pub fn load(set: &FontSet) -> Result<Fonts, &'static str> {
        let parse = |b: &[u8]| fontdue::Font::from_bytes(b, fontdue::FontSettings::default());
        let regular =
            Arc::new(parse(&set.regular).map_err(|_| "regular font could not be parsed")?);
        let bold = match &set.bold {
            Some(b) => Arc::new(parse(b).map_err(|_| "bold font could not be parsed")?),
            None => regular.clone(),
        };
        Ok(Fonts { regular, bold })
    }

    fn pick(&self, bold: bool) -> &fontdue::Font {
        if bold {
            &self.bold
        } else {
            &self.regular
        }
    }
}

/// Zoom-independent text measurement for layout (`graphy_render::TextMeasure`).
/// Bold advances are used for widths so a title never overflows its header.
#[derive(Clone)]
pub struct FontMeasure {
    fonts: Fonts,
}

impl TextMeasure for FontMeasure {
    fn width(&self, text: &str, size: f32) -> f32 {
        let f = self.fonts.pick(true);
        text.chars()
            .map(|c| f.metrics(c, REF_PX).advance_width)
            .sum::<f32>()
            * (size / REF_PX)
    }
}

#[derive(Clone, Copy)]
struct Slot {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    w: f32,
    h: f32,
    xmin: f32,
    ymin: f32,
    advance: f32,
}

type Key = (char, bool, u32);

pub struct TextEngine {
    fonts: Fonts,
    slots: HashMap<Key, Slot>,
    data: Vec<u8>,
    cur_x: u32,
    cur_y: u32,
    row_h: u32,
    dirty_rows: Option<(u32, u32)>,
    full: bool,
    texture: Option<wgpu::Texture>,
    pub(crate) bind_group: Option<wgpu::BindGroup>,
    pub(crate) bgl: wgpu::BindGroupLayout,
    sampler: Option<wgpu::Sampler>,
}

impl TextEngine {
    pub fn new(device: &wgpu::Device, fonts: Fonts) -> Self {
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("graphy_atlas_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        Self {
            fonts,
            slots: HashMap::new(),
            data: vec![0; (ATLAS_SIZE * ATLAS_SIZE) as usize],
            cur_x: 1,
            cur_y: 1,
            row_h: 0,
            dirty_rows: None,
            full: false,
            texture: None,
            bind_group: None,
            bgl,
            sampler: None,
        }
    }

    pub fn measure(&self) -> FontMeasure {
        FontMeasure {
            fonts: self.fonts.clone(),
        }
    }

    fn clear(&mut self) {
        self.slots.clear();
        self.data.fill(0);
        self.cur_x = 1;
        self.cur_y = 1;
        self.row_h = 0;
        self.dirty_rows = Some((0, ATLAS_SIZE));
        self.full = false;
    }

    fn glyph(&mut self, ch: char, bold: bool, size_q: u32) -> Slot {
        let key = (ch, bold, size_q);
        if let Some(s) = self.slots.get(&key) {
            return *s;
        }
        let px = size_q as f32 * 0.5;
        let (m, bitmap) = self.fonts.pick(bold).rasterize(ch, px);
        let mut slot = Slot {
            uv_min: [0.0; 2],
            uv_max: [0.0; 2],
            w: 0.0,
            h: 0.0,
            xmin: m.xmin as f32,
            ymin: m.ymin as f32,
            advance: m.advance_width,
        };
        if m.width > 0 && m.height > 0 {
            let (gw, gh) = (m.width as u32, m.height as u32);
            if self.cur_x + gw + 1 > ATLAS_SIZE {
                self.cur_y += self.row_h + 1;
                self.cur_x = 1;
                self.row_h = 0;
            }
            if self.cur_y + gh + 1 > ATLAS_SIZE {
                self.full = true;
                return slot;
            }
            for row in 0..gh {
                let src = (row * gw) as usize;
                let dst = ((self.cur_y + row) * ATLAS_SIZE + self.cur_x) as usize;
                self.data[dst..dst + gw as usize].copy_from_slice(&bitmap[src..src + gw as usize]);
            }
            slot.uv_min = [
                self.cur_x as f32 / ATLAS_SIZE as f32,
                self.cur_y as f32 / ATLAS_SIZE as f32,
            ];
            slot.uv_max = [
                (self.cur_x + gw) as f32 / ATLAS_SIZE as f32,
                (self.cur_y + gh) as f32 / ATLAS_SIZE as f32,
            ];
            slot.w = gw as f32;
            slot.h = gh as f32;
            let (y0, y1) = self.dirty_rows.unwrap_or((self.cur_y, self.cur_y));
            self.dirty_rows = Some((y0.min(self.cur_y), y1.max(self.cur_y + gh)));
            self.cur_x += gw + 1;
            self.row_h = self.row_h.max(gh);
        }
        self.slots.insert(key, slot);
        slot
    }

    /// Appends glyph quads (physical pixels) for every text. If the atlas fills
    /// mid-frame it is cleared and the frame is laid out again once.
    pub fn layout(
        &mut self,
        texts: &[TextDraw],
        view: &ViewTransform,
        scale: f32,
        out: &mut Vec<TextVertex>,
    ) {
        for attempt in 0..2 {
            out.clear();
            for t in texts {
                self.layout_one(t, view, scale, out);
            }
            if !self.full || attempt == 1 {
                break;
            }
            self.clear();
        }
    }

    fn layout_one(
        &mut self,
        t: &TextDraw,
        view: &ViewTransform,
        scale: f32,
        out: &mut Vec<TextVertex>,
    ) {
        let px = t.size * view.zoom * scale;
        let size_q = ((px * 2.0).round() as u32).max(2);
        let q = size_q as f32 * 0.5;
        let glyphs: Vec<Slot> = t
            .text
            .chars()
            .map(|c| self.glyph(c, t.bold, size_q))
            .collect();
        let width: f32 = glyphs.iter().map(|g| g.advance).sum();
        let anchor = view.graph_to_screen(t.pos) * scale;
        let x0 = match t.align {
            TextAlign::Left => anchor.x,
            TextAlign::Right => anchor.x - width,
            TextAlign::Center => anchor.x - width * 0.5,
        };
        // centre cap-height-ish on the anchor, on a whole-pixel baseline
        let baseline = (anchor.y + 0.36 * q).round();
        let mut pen = x0;
        for g in glyphs {
            if g.w > 0.0 {
                let left = (pen + g.xmin).round();
                let top = baseline - g.ymin - g.h;
                let (l, r, tp, b) = (left, left + g.w, top, top + g.h);
                let c = t.color;
                let v = |x: f32, y: f32, u: f32, w: f32| TextVertex {
                    pos: [x, y],
                    uv: [u, w],
                    color: c,
                };
                let (u0, v0, u1, v1) = (g.uv_min[0], g.uv_min[1], g.uv_max[0], g.uv_max[1]);
                out.extend_from_slice(&[
                    v(l, tp, u0, v0),
                    v(r, tp, u1, v0),
                    v(l, b, u0, v1),
                    v(l, b, u0, v1),
                    v(r, tp, u1, v0),
                    v(r, b, u1, v1),
                ]);
            }
            pen += g.advance;
        }
    }

    /// Creates the texture on first use and uploads rasterised rows.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.texture.is_none() {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("graphy_glyph_atlas"),
                size: wgpu::Extent3d {
                    width: ATLAS_SIZE,
                    height: ATLAS_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("graphy_glyph_sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("graphy_atlas_bg"),
                layout: &self.bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            }));
            self.sampler = Some(sampler);
            self.texture = Some(tex);
            self.dirty_rows = Some((0, ATLAS_SIZE));
        }
        if let (Some((y0, y1)), Some(tex)) = (self.dirty_rows.take(), &self.texture) {
            let y1 = y1.min(ATLAS_SIZE);
            if y1 > y0 {
                let start = (y0 * ATLAS_SIZE) as usize;
                let end = (y1 * ATLAS_SIZE) as usize;
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x: 0, y: y0, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &self.data[start..end],
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(ATLAS_SIZE),
                        rows_per_image: Some(y1 - y0),
                    },
                    wgpu::Extent3d {
                        width: ATLAS_SIZE,
                        height: y1 - y0,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
    }
}
