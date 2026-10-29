//! Hit testing, derived from the same [`crate::NodeLayout`] the renderer
//! draws from. Priority: pins, then nodes (top-most first), then comment
//! chrome, then wires.

use crate::geom::{Rect, Vec2};
use crate::layout::{NodeShape, PinSide, RowContent};
use crate::scene::Scene;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PinAddr {
    pub node: usize,
    pub side: PinSide,
    pub pin: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodePart {
    Header,
    Body,
    /// A host row (inline widget / preview) with this slot id.
    Custom(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    None,
    Pin(PinAddr),
    Node {
        node: usize,
        part: NodePart,
    },
    /// The inline value chip of an unconnected input pin.
    PinWidget(PinAddr),
    Wire(usize),
    CommentHeader(usize),
    CommentResize(usize),
}

/// Screen-pixel tolerances; converted to graph units by the current zoom.
#[derive(Clone, Copy, Debug)]
pub struct HitTolerance {
    pub pin_px: f32,
    pub wire_px: f32,
}

impl Default for HitTolerance {
    fn default() -> Self {
        Self {
            pin_px: 9.0,
            wire_px: 6.0,
        }
    }
}

impl Scene {
    /// Pin whose generous hit disc contains `p` (graph space).
    pub fn pin_at(&self, p: Vec2, zoom: f32, tol: HitTolerance) -> Option<PinAddr> {
        let radius = self.metrics.pin_hit_radius.max(tol.pin_px / zoom);
        // the disc may poke out of a node, so query with the radius as padding
        let mut best: Option<(f32, PinAddr)> = None;
        for ni in self.spatial.nodes.query(Rect::around(p, radius), true) {
            let node = &self.nodes[ni];
            for side in [PinSide::Input, PinSide::Output] {
                for (pi, _) in node.pins(side).iter().enumerate() {
                    let Some(c) = node.pin_pos(side, pi) else {
                        continue;
                    };
                    let d = c.distance(p);
                    if d <= radius && best.map_or(true, |(bd, _)| d < bd) {
                        best = Some((
                            d,
                            PinAddr {
                                node: ni,
                                side,
                                pin: pi,
                            },
                        ));
                    }
                }
            }
        }
        best.map(|(_, a)| a)
    }

    /// Graph-space rectangle of an input pin's inline chip, if it has one.
    pub fn widget_rect(&self, node: usize, pin: usize) -> Option<Rect> {
        let n = self.nodes.get(node)?;
        let info = n.inputs.get(pin)?;
        let w = info.widget.as_ref()?;
        let c = n.pin_pos(PinSide::Input, pin)?;
        let x = c.x
            + self.metrics.pin_size * 0.5
            + self.metrics.label_gap
            + info.label_width
            + crate::scene::WIDGET_GAP;
        Some(Rect::from_pos_size(
            Vec2::new(x, c.y - crate::scene::WIDGET_HEIGHT * 0.5),
            Vec2::new(w.width, crate::scene::WIDGET_HEIGHT),
        ))
    }

    pub fn widget_at(&self, p: Vec2) -> Option<PinAddr> {
        for ni in self.spatial.nodes.query(Rect::around(p, 0.5), true) {
            for (pi, info) in self.nodes[ni].inputs.iter().enumerate() {
                if info.widget.is_some() && !info.connected {
                    if self.widget_rect(ni, pi).is_some_and(|r| r.contains(p)) {
                        return Some(PinAddr {
                            node: ni,
                            side: PinSide::Input,
                            pin: pi,
                        });
                    }
                }
            }
        }
        None
    }

    pub fn node_at(&self, p: Vec2) -> Option<(usize, NodePart)> {
        for ni in self.spatial.nodes.query(Rect::around(p, 0.5), true) {
            let node = &self.nodes[ni];
            let local = p - node.pos;
            if !node.layout.local_rect().contains(local) {
                continue;
            }
            let part = match node.layout.shape {
                NodeShape::Card if node.layout.header.contains(local) => NodePart::Header,
                NodeShape::Card => node
                    .layout
                    .rows
                    .iter()
                    .find(|r| r.rect.contains(local))
                    .and_then(|r| match r.content {
                        RowContent::Custom { slot } => Some(NodePart::Custom(slot)),
                        _ => None,
                    })
                    .unwrap_or(NodePart::Body),
                _ => NodePart::Header,
            };
            return Some((ni, part));
        }
        None
    }

    pub fn comment_at(&self, p: Vec2, zoom: f32) -> Option<Hit> {
        let handle = (self.metrics.comment_resize_handle).max(10.0 / zoom);
        for ci in self.spatial.comments.query(Rect::around(p, 0.5), true) {
            let r = self.comments[ci].rect;
            if !r.contains(p) {
                continue;
            }
            if p.x >= r.max.x - handle && p.y >= r.max.y - handle {
                return Some(Hit::CommentResize(ci));
            }
            if p.y <= r.min.y + self.metrics.comment_header_h {
                return Some(Hit::CommentHeader(ci));
            }
        }
        None
    }

    pub fn wire_at(&self, p: Vec2, zoom: f32, tol: HitTolerance) -> Option<usize> {
        let radius = tol.wire_px / zoom;
        self.spatial
            .wires
            .query(Rect::around(p, radius), true)
            .into_iter()
            .map(|wi| (wi, self.wires[wi].shape.distance_to(p)))
            .filter(|(_, d)| *d <= radius)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(wi, _)| wi)
    }

    pub fn hit_test(&self, p: Vec2, zoom: f32, tol: HitTolerance) -> Hit {
        if let Some(a) = self.pin_at(p, zoom, tol) {
            return Hit::Pin(a);
        }
        if let Some(a) = self.widget_at(p) {
            return Hit::PinWidget(a);
        }
        if let Some((node, part)) = self.node_at(p) {
            return Hit::Node { node, part };
        }
        if let Some(h) = self.comment_at(p, zoom) {
            return h;
        }
        if let Some(w) = self.wire_at(p, zoom, tol) {
            return Hit::Wire(w);
        }
        Hit::None
    }
}
