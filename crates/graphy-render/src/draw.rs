//! Renderer-neutral draw data.
//!
//! [`Scene::build_draw_data`] turns a scene, the current view and the
//! transient interaction state into flat lists a backend can upload as
//! instances. This is where culling, level of detail and the visual state
//! hierarchy (hover / selected / active / disabled / focused, wire emphasis)
//! are decided, so every backend and every golden test sees the same scene.

use std::collections::HashSet;

use crate::geom::{with_alpha, Color, Rect, Vec2};
use crate::hit::{Hit, PinAddr};
use crate::layout::{NodeShape, PinSide, RowContent};
use crate::presenter::{dimmed, PinShape};
use crate::scene::{Scene, SceneNode};
use crate::style::{Theme, WireWeights};
use crate::text::{fit_text, TextMeasure};
use crate::view::{LodTier, ViewTransform};
use crate::wire::{WireRouting, WireShape};

pub mod flags {
    pub const HOVERED: u32 = 1;
    pub const SELECTED: u32 = 1 << 1;
    pub const ACTIVE: u32 = 1 << 2;
    pub const DISABLED: u32 = 1 << 3;
    pub const FOCUSED: u32 = 1 << 4;
    pub const DRAGGING: u32 = 1 << 5;
    /// Overview tier: draw a flat silhouette.
    pub const FLAT: u32 = 1 << 6;
    /// Wire: dimmed by selection context.
    pub const DIM: u32 = 1 << 7;
    /// Wire: emphasised (touches selection/hover) — also gets a halo.
    pub const EMPHASIS: u32 = 1 << 8;
    /// Wire: connection-drag preview (animated flow).
    pub const PREVIEW: u32 = 1 << 9;
    /// Pin: connected (filled).
    pub const CONNECTED: u32 = 1 << 10;
    /// Pin: valid drop target while connecting.
    pub const COMPATIBLE: u32 = 1 << 11;
    /// Pin: wire tail from the card edge to the pin.
    pub const RINGED: u32 = 1 << 12;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxKind {
    Card,
    Reroute,
    Pill,
    Comment,
    Marquee,
    /// Inset surface (texture/value preview placeholder).
    Well,
    /// Small rounded square behind a header icon.
    Icon,
    /// Thin snap guide.
    Guide,
}

impl BoxKind {
    pub fn shader_kind(self) -> u32 {
        match self {
            BoxKind::Card => 0,
            BoxKind::Reroute => 1,
            BoxKind::Pill => 2,
            BoxKind::Comment => 3,
            BoxKind::Marquee => 4,
            BoxKind::Well => 5,
            BoxKind::Icon => 6,
            BoxKind::Guide => 7,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxDraw {
    pub rect: Rect,
    pub kind: BoxKind,
    pub body: Color,
    pub header: Color,
    pub accent: Color,
    pub border: Color,
    /// Resolved state ring (focus / selection), alpha 0 when none.
    pub ring: Color,
    pub header_h: f32,
    pub radius: f32,
    pub flags: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PinDraw {
    pub center: Vec2,
    pub size: f32,
    pub color: Color,
    pub shape: PinShape,
    pub input: bool,
    pub flags: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WireDraw {
    pub shape: WireShape,
    pub color: Color,
    /// Full stroke width in graph units.
    pub width: f32,
    pub flags: u32,
    /// Deterministic per-wire phase for flow animation.
    pub phase: f32,
    pub segments: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Right,
    Center,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextDraw {
    pub text: String,
    /// Anchor in graph space: x per `align`, y is the vertical centre of the line.
    pub pos: Vec2,
    /// Font size in graph units.
    pub size: f32,
    pub color: Color,
    pub align: TextAlign,
    pub bold: bool,
}

/// A host-owned region the backend leaves for the profile to fill (a texture
/// preview, an inline widget). A placeholder `Well` is drawn underneath.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewSlot {
    pub node: String,
    pub slot: u32,
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasDraw {
    pub background: Color,
    pub grid_minor: Color,
    pub grid_major: Color,
    /// Graph units between minor lines at zoom-independent base spacing.
    pub minor_step: f32,
    /// Minor lines per major line.
    pub major_every: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawStats {
    pub nodes: usize,
    pub nodes_culled: usize,
    pub wires: usize,
    pub wires_culled: usize,
    pub pins: usize,
    pub texts: usize,
}

#[derive(Clone, Debug)]
pub struct DrawData {
    pub viewport: Vec2,
    pub view: ViewTransform,
    pub lod: LodTier,
    pub time: f32,
    pub canvas: CanvasDraw,
    /// Back to front: comments.
    pub comments: Vec<BoxDraw>,
    /// Back to front: dim, normal, emphasised.
    pub wires: Vec<WireDraw>,
    pub nodes: Vec<BoxDraw>,
    /// Wire stubs from a card edge to its pin, drawn over the card.
    pub tails: Vec<WireDraw>,
    pub pins: Vec<PinDraw>,
    pub texts: Vec<TextDraw>,
    pub previews: Vec<PreviewSlot>,
    /// Marquee, guides, connection-drag preview.
    pub overlay_boxes: Vec<BoxDraw>,
    pub overlay_wires: Vec<WireDraw>,
    pub stats: DrawStats,
}

/// Transient, editor-owned state that changes how things are drawn.
#[derive(Clone, Debug, Default)]
pub struct InteractionState {
    pub hovered: Option<Hit>,
    pub selected_nodes: HashSet<String>,
    pub selected_comments: HashSet<usize>,
    pub selected_wires: HashSet<usize>,
    pub focused_node: Option<String>,
    pub dragging_nodes: HashSet<String>,
    pub marquee: Option<Rect>,
    pub guides: Vec<Rect>,
    pub connecting: Option<ConnectPreview>,
}

/// An in-progress pin drag.
#[derive(Clone, Debug)]
pub struct ConnectPreview {
    pub anchor: PinAddr,
    /// Cursor, graph space.
    pub cursor: Vec2,
    /// Pin the cursor is snapped to, if any.
    pub snapped: Option<PinAddr>,
    pub routing: WireRouting,
    /// Pins that would accept the connection.
    pub compatible: HashSet<PinAddr>,
}

#[derive(Clone, Debug)]
pub struct DrawParams {
    pub viewport: Vec2,
    pub view: ViewTransform,
    pub time: f32,
    pub theme: Theme,
    pub weights: WireWeights,
    /// Texts smaller than this on screen are skipped (they would be mush).
    pub min_text_px: f32,
}

impl DrawParams {
    pub fn new(viewport: Vec2, view: ViewTransform) -> Self {
        Self { viewport, view, time: 0.0, theme: Theme::dark(), weights: WireWeights::default(), min_text_px: 5.5 }
    }
}

impl Scene {
    pub fn build_draw_data(&self, p: &DrawParams, st: &InteractionState, measure: &dyn TextMeasure) -> DrawData {
        let lod = LodTier::for_zoom(p.view.zoom);
        let theme = &p.theme;
        let visible = p.view.visible_rect(p.viewport, 48.0);
        let mut out = DrawData {
            viewport: p.viewport,
            view: p.view,
            lod,
            time: p.time,
            canvas: CanvasDraw {
                background: theme.canvas,
                grid_minor: theme.grid_minor,
                grid_major: theme.grid_major,
                minor_step: 20.0,
                major_every: 5.0,
            },
            comments: Vec::new(),
            wires: Vec::new(),
            nodes: Vec::new(),
            tails: Vec::new(),
            pins: Vec::new(),
            texts: Vec::new(),
            previews: Vec::new(),
            overlay_boxes: Vec::new(),
            overlay_wires: Vec::new(),
            stats: DrawStats::default(),
        };
        let text_ok = |size: f32| size * p.view.zoom >= p.min_text_px;

        // ---- comments ------------------------------------------------------
        for ci in self.spatial.comments.query(visible, false) {
            let c = &self.comments[ci];
            let selected = st.selected_comments.contains(&ci);
            let hovered = matches!(st.hovered, Some(Hit::CommentHeader(i)) | Some(Hit::CommentResize(i)) if i == ci);
            let mut f = 0;
            if selected {
                f |= flags::SELECTED;
            }
            if hovered {
                f |= flags::HOVERED;
            }
            out.comments.push(BoxDraw {
                rect: c.rect,
                kind: BoxKind::Comment,
                body: theme.comment_fill,
                header: with_alpha(c.presentation.accent, 0.10),
                accent: c.presentation.accent,
                border: theme.comment_border,
                ring: if selected { theme.select } else { [0.0; 4] },
                header_h: self.metrics.comment_header_h,
                radius: self.metrics.corner_radius + 2.0,
                flags: f,
            });
            if lod.shows_titles() && text_ok(self.metrics.title_font) {
                let max_w = c.rect.width() - 24.0;
                out.texts.push(TextDraw {
                    text: fit_text(measure, &c.text, self.metrics.title_font, max_w),
                    pos: Vec2::new(c.rect.min.x + 12.0, c.rect.min.y + self.metrics.comment_header_h * 0.5),
                    size: self.metrics.title_font,
                    color: theme.text_dim,
                    align: TextAlign::Left,
                    bold: true,
                });
            }
        }

        // ---- wires ---------------------------------------------------------
        let focus_nodes: HashSet<usize> = st
            .selected_nodes
            .iter()
            .filter_map(|id| self.node_index(id))
            .chain(match st.hovered {
                Some(Hit::Node { node, .. }) => Some(node),
                Some(Hit::Pin(a)) => Some(a.node),
                _ => None,
            })
            .collect();
        let hovered_wire = match st.hovered {
            Some(Hit::Wire(w)) => Some(w),
            _ => None,
        };
        let candidates = self.spatial.wires.query(visible, false);
        out.stats.wires_culled = self.wires.len().saturating_sub(candidates.len());
        let (mut dim, mut normal, mut emph) = (Vec::new(), Vec::new(), Vec::new());
        for wi in candidates {
            let w = &self.wires[wi];
            let touches_focus = focus_nodes.contains(&w.from_node) || focus_nodes.contains(&w.to_node);
            let picked = hovered_wire == Some(wi) || st.selected_wires.contains(&w.connection);
            let src = &self.nodes[w.from_node];
            let active = src.presentation.active;
            let mut f = 0;
            let (color, width) = if picked || touches_focus {
                f |= flags::EMPHASIS;
                (w.style.color, p.weights.emphasis)
            } else if !focus_nodes.is_empty() || st.connecting.is_some() {
                f |= flags::DIM;
                (dimmed(theme, w.style.color, 0.68), p.weights.dim)
            } else {
                // Resting wires sit slightly back so dense graphs stay calm.
                (dimmed(theme, w.style.color, 0.22), p.weights.base)
            };
            if active {
                f |= flags::ACTIVE;
            }
            let width = if w.style.exec { width * 1.12 } else { width };
            let draw = WireDraw {
                shape: w.shape,
                color,
                width: if lod == LodTier::Overview { width * 1.4 } else { width },
                flags: f,
                phase: ((wi as u32).wrapping_mul(2654435761) >> 8) as f32 / (1u32 << 24) as f32,
                segments: lod.wire_segments(),
            };
            if f & flags::EMPHASIS != 0 {
                emph.push(draw);
            } else if f & flags::DIM != 0 {
                dim.push(draw);
            } else {
                normal.push(draw);
            }
        }
        out.stats.wires = dim.len() + normal.len() + emph.len();
        out.wires.extend(dim);
        out.wires.extend(normal);
        out.wires.extend(emph);

        // ---- nodes, pins, text --------------------------------------------
        let node_ids = self.spatial.nodes.query(visible, false);
        out.stats.nodes = node_ids.len();
        out.stats.nodes_culled = self.nodes.len().saturating_sub(node_ids.len());
        for ni in node_ids {
            self.draw_node(ni, p, st, lod, measure, &mut out);
        }

        // ---- overlays ------------------------------------------------------
        if let Some(r) = st.marquee {
            out.overlay_boxes.push(BoxDraw {
                rect: r,
                kind: BoxKind::Marquee,
                body: theme.marquee_fill,
                header: [0.0; 4],
                accent: theme.marquee_border,
                border: theme.marquee_border,
                ring: [0.0; 4],
                header_h: 0.0,
                radius: 2.0,
                flags: 0,
            });
        }
        for g in &st.guides {
            out.overlay_boxes.push(BoxDraw {
                rect: *g,
                kind: BoxKind::Guide,
                body: with_alpha(theme.focus, 0.55),
                header: [0.0; 4],
                accent: theme.focus,
                border: [0.0; 4],
                ring: [0.0; 4],
                header_h: 0.0,
                radius: 0.0,
                flags: 0,
            });
        }
        if let Some(c) = &st.connecting {
            if let Some(anchor) = self.nodes.get(c.anchor.node) {
                if let Some(a) = anchor.pin_pos(c.anchor.side, c.anchor.pin) {
                    let end = c
                        .snapped
                        .and_then(|s| self.nodes.get(s.node).and_then(|n| n.pin_pos(s.side, s.pin)))
                        .unwrap_or(c.cursor);
                    let (from, to) = if c.anchor.side == PinSide::Output { (a, end) } else { (end, a) };
                    let color = anchor.pins(c.anchor.side)[c.anchor.pin].style.color;
                    out.overlay_wires.push(WireDraw {
                        shape: WireShape::new(c.routing, from, to),
                        color,
                        width: p.weights.emphasis,
                        flags: flags::PREVIEW | flags::EMPHASIS,
                        phase: 0.0,
                        segments: 32,
                    });
                }
            }
        }
        out.stats.pins = out.pins.len();
        out.stats.texts = out.texts.len();
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_node(
        &self,
        ni: usize,
        p: &DrawParams,
        st: &InteractionState,
        lod: LodTier,
        measure: &dyn TextMeasure,
        out: &mut DrawData,
    ) {
        let theme = &p.theme;
        let m = &self.metrics;
        let node: &SceneNode = &self.nodes[ni];
        let pres = &node.presentation;
        let rect = node.rect();
        let hovered = matches!(st.hovered, Some(Hit::Node { node: n, .. }) if n == ni)
            || matches!(st.hovered, Some(Hit::Pin(a)) if a.node == ni);
        let selected = st.selected_nodes.contains(&node.id);
        let focused = st.focused_node.as_deref() == Some(node.id.as_str());
        let mut f = 0;
        if hovered {
            f |= flags::HOVERED;
        }
        if selected {
            f |= flags::SELECTED;
        }
        if focused {
            f |= flags::FOCUSED;
        }
        if pres.active {
            f |= flags::ACTIVE;
        }
        if pres.disabled {
            f |= flags::DISABLED;
        }
        if st.dragging_nodes.contains(&node.id) {
            f |= flags::DRAGGING;
        }
        if lod == LodTier::Overview {
            f |= flags::FLAT;
        }
        let ring = if focused {
            theme.focus
        } else if selected {
            theme.select
        } else {
            [0.0; 4]
        };
        let kind = match node.layout.shape {
            NodeShape::Card => BoxKind::Card,
            NodeShape::Reroute => BoxKind::Reroute,
            NodeShape::Pill => BoxKind::Pill,
        };
        let border = if hovered { theme.card_border_hover } else { theme.card_border };
        out.nodes.push(BoxDraw {
            rect,
            kind,
            body: theme.card_body,
            header: theme.card_header,
            accent: pres.accent,
            border,
            ring,
            header_h: if node.layout.shape == NodeShape::Card { m.header_h } else { 0.0 },
            radius: if node.layout.shape == NodeShape::Pill { m.pill_height * 0.5 } else { m.corner_radius },
            flags: f,
        });

        if lod.shows_pins() {
            self.draw_pins(ni, p, st, lod, out);
        }

        if node.layout.shape != NodeShape::Card {
            if node.layout.shape == NodeShape::Pill && lod.shows_titles() && text_fits(m.label_font, p) {
                out.texts.push(TextDraw {
                    text: pres.title.clone(),
                    pos: rect.center(),
                    size: m.label_font,
                    color: theme.text,
                    align: TextAlign::Center,
                    bold: false,
                });
            }
            return;
        }

        // header content
        let tint = if pres.disabled { 0.45 } else { 1.0 };
        let text_color = |c: Color| with_alpha(c, c[3] * tint);
        if lod.shows_titles() && text_fits(m.title_font, p) {
            let mut x = rect.min.x + m.header_pad_x;
            if let Some(icon) = pres.icon {
                let ir = Rect::from_pos_size(
                    Vec2::new(x, rect.min.y + (m.header_h - m.icon_size) * 0.5),
                    Vec2::new(m.icon_size, m.icon_size),
                );
                out.nodes.push(BoxDraw {
                    rect: ir,
                    kind: BoxKind::Icon,
                    body: with_alpha(pres.accent, 0.16 * tint),
                    header: [0.0; 4],
                    accent: pres.accent,
                    border: [0.0; 4],
                    ring: [0.0; 4],
                    header_h: 0.0,
                    radius: 3.5,
                    flags: 0,
                });
                out.texts.push(TextDraw {
                    text: icon.to_string(),
                    pos: ir.center(),
                    size: m.icon_size * 0.72,
                    color: text_color(pres.accent),
                    align: TextAlign::Center,
                    bold: true,
                });
                x += m.icon_size + 6.0;
            }
            let right_pad = if pres.header_output.is_some() { m.pin_inset + m.pin_size * 0.5 + 8.0 } else { m.header_pad_x };
            let max_w = (rect.max.x - right_pad - x).max(0.0);
            out.texts.push(TextDraw {
                text: fit_text(measure, &pres.title, m.title_font, max_w),
                pos: Vec2::new(x, rect.min.y + m.header_h * 0.5),
                size: m.title_font,
                color: text_color(theme.text),
                align: TextAlign::Left,
                bold: true,
            });
        }

        // body rows
        for row in &node.layout.rows {
            let rr = row.rect.translate(node.pos);
            match row.content {
                RowContent::Pins { input, output } => {
                    if !lod.shows_pin_labels() || !text_fits(m.label_font, p) {
                        continue;
                    }
                    let edge = m.pin_inset + m.pin_size * 0.5 + m.label_gap;
                    let lw = |s: &str| measure.width(s, m.label_font).min(crate::scene::MAX_LABEL_WIDTH);
                    if let Some(i) = input {
                        let label = &node.inputs[i].label;
                        let max_w = rr.width() - edge - output.map_or(m.header_pad_x, |o| lw(&node.outputs[o].label) + edge + m.column_gap);
                        out.texts.push(TextDraw {
                            text: fit_text(measure, label, m.label_font, max_w.max(0.0)),
                            pos: Vec2::new(rr.min.x + edge, rr.center().y),
                            size: m.label_font,
                            color: text_color(theme.text_dim),
                            align: TextAlign::Left,
                            bold: false,
                        });
                    }
                    if let Some(o) = output {
                        let label = &node.outputs[o].label;
                        let max_w = rr.width() - edge - input.map_or(m.header_pad_x, |i| lw(&node.inputs[i].label) + edge + m.column_gap);
                        out.texts.push(TextDraw {
                            text: fit_text(measure, label, m.label_font, max_w.max(0.0)),
                            pos: Vec2::new(rr.max.x - edge, rr.center().y),
                            size: m.label_font,
                            color: text_color(theme.text_dim),
                            align: TextAlign::Right,
                            bold: false,
                        });
                    }
                }
                RowContent::Custom { slot } => {
                    if lod != LodTier::Detail {
                        continue;
                    }
                    let well = rr.expand(-m.pin_inset * 0.5);
                    out.nodes.push(BoxDraw {
                        rect: well,
                        kind: BoxKind::Well,
                        body: theme.preview_well,
                        header: [0.0; 4],
                        accent: pres.accent,
                        border: theme.card_border,
                        ring: [0.0; 4],
                        header_h: 0.0,
                        radius: 4.0,
                        flags: f & flags::DISABLED,
                    });
                    out.previews.push(PreviewSlot { node: node.id.clone(), slot, rect: well });
                }
            }
        }
    }

    fn draw_pins(&self, ni: usize, p: &DrawParams, st: &InteractionState, lod: LodTier, out: &mut DrawData) {
        let node = &self.nodes[ni];
        let m = &self.metrics;
        let theme = &p.theme;
        let dragging = st.connecting.as_ref();
        for side in [PinSide::Input, PinSide::Output] {
            for (pi, pin) in node.pins(side).iter().enumerate() {
                let Some(center) = node.pin_pos(side, pi) else { continue };
                let addr = PinAddr { node: ni, side, pin: pi };
                let mut f = 0;
                if pin.connected {
                    f |= flags::CONNECTED;
                }
                if pin.style.ringed {
                    f |= flags::RINGED;
                }
                if st.hovered == Some(Hit::Pin(addr)) {
                    f |= flags::HOVERED;
                }
                let mut color = pin.style.color;
                if let Some(c) = dragging {
                    if c.compatible.contains(&addr) {
                        f |= flags::COMPATIBLE;
                    } else if c.anchor != addr {
                        f |= flags::DIM;
                        color = dimmed(theme, color, 0.6);
                    }
                }
                if node.presentation.disabled {
                    f |= flags::DISABLED;
                    color = dimmed(theme, color, 0.55);
                }
                match node.layout.shape {
                    // reroute/pill draw their own pin through the body shape
                    NodeShape::Reroute => continue,
                    NodeShape::Pill if lod != LodTier::Detail => continue,
                    _ => {}
                }
                out.pins.push(PinDraw {
                    center,
                    size: m.pin_size,
                    color,
                    shape: pin.style.shape,
                    input: side == PinSide::Input,
                    flags: f,
                });
                // Tail: the wire passes under the card, so continue it from the
                // card edge to the pin centre over the card surface.
                if pin.connected && node.layout.shape == NodeShape::Card && lod != LodTier::Overview {
                    let edge_x = if side == PinSide::Input { node.pos.x } else { node.pos.x + node.layout.size.x };
                    let edge = Vec2::new(edge_x, center.y);
                    let (a, b) = if side == PinSide::Input { (edge, center) } else { (center, edge) };
                    let focus = st.selected_nodes.contains(&node.id);
                    let tail_color = if focus { color } else { dimmed(theme, color, 0.22) };
                    out.tails.push(WireDraw {
                        shape: WireShape::straight(a, b),
                        color: tail_color,
                        width: p.weights.base,
                        flags: 0,
                        phase: 0.0,
                        segments: 2,
                    });
                }
            }
        }
    }
}

fn text_fits(size: f32, p: &DrawParams) -> bool {
    size * p.view.zoom >= p.min_text_px
}
