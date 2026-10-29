//! Scene: the derived, renderable view of a [`GraphDescription`].
//!
//! The graph document stays the only mutable model. A scene holds per-node
//! layouts, resolved wires and the spatial index, and is kept current either
//! with [`Scene::rebuild`] or incrementally with [`Scene::apply_changes`]
//! from the [`ChangeSet`] an edit produced. During a drag the editor moves
//! nodes in the scene transiently ([`Scene::set_node_position`]) and commits
//! one command on release.

use std::collections::HashMap;

use graphy::{ChangeSet, GraphChange, GraphDescription};

use crate::geom::{Rect, Vec2};
use crate::layout::{LayoutMetrics, NodeLayout, NodeShape, NodeSpec, PinSide};
use crate::presenter::{CommentPresentation, NodePresentation, PinStyle, Presenter, WireStyle};
use crate::spatial::{Entry, SpatialIndex};
use crate::text::TextMeasure;
use crate::wire::{WireRouting, WireShape};

/// Label widths are capped so a long name truncates instead of growing the
/// card without bound.
pub const MAX_LABEL_WIDTH: f32 = 150.0;
pub const MAX_TITLE_WIDTH: f32 = 220.0;
/// Gap between a pin label and its inline widget chip.
pub const WIDGET_GAP: f32 = 6.0;
/// Chip height, graph units.
pub const WIDGET_HEIGHT: f32 = 15.0;

/// Distance between neighbouring wires through a zip tie slot, graph units.
pub const TIE_SPACING: f32 = 14.0;
/// How far the tie bar sticks out past the outermost slots.
pub const TIE_MARGIN: f32 = 20.0;
/// Seconds the add-wobble runs for.
const TIE_WOBBLE_TIME: f32 = 0.6;
/// Wobble amplitude start, graph units (quadratic falloff).
const TIE_WOBBLE_AMP: f32 = 11.0;
/// Wobble frequency, radians/second.
const TIE_WOBBLE_FREQ: f32 = 18.0;

#[derive(Clone, Debug)]
pub struct PinInfo {
    pub id: String,
    pub label: String,
    pub style: PinStyle,
    pub connected: bool,
    /// Inline value chip shown after the label while the input is unconnected.
    pub widget: Option<crate::presenter::PinWidget>,
    /// Measured label width (capped), graph units.
    pub label_width: f32,
}

#[derive(Clone, Debug)]
pub struct SceneNode {
    pub id: String,
    pub pos: Vec2,
    pub presentation: NodePresentation,
    pub layout: NodeLayout,
    pub inputs: Vec<PinInfo>,
    pub outputs: Vec<PinInfo>,
}

impl SceneNode {
    pub fn rect(&self) -> Rect {
        Rect::from_pos_size(self.pos, self.layout.size)
    }

    /// Graph-space pin centre.
    pub fn pin_pos(&self, side: PinSide, index: usize) -> Option<Vec2> {
        self.layout.pin(side, index).map(|p| self.pos + p.center)
    }

    pub fn pins(&self, side: PinSide) -> &[PinInfo] {
        match side {
            PinSide::Input => &self.inputs,
            PinSide::Output => &self.outputs,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SceneComment {
    pub text: String,
    pub rect: Rect,
    pub presentation: CommentPresentation,
    /// `false` for profile-supplied sections, which are not document comments.
    pub editable: bool,
}

/// A resolved connection.
#[derive(Clone, Debug)]
pub struct SceneWire {
    /// Index into `GraphDescription::connections` when the scene was built.
    pub connection: usize,
    pub from_node: usize,
    pub from_pin: usize,
    pub to_node: usize,
    pub to_pin: usize,
    pub style: WireStyle,
    pub shape: WireShape,
}

/// A resolved zip tie: the wires it clamps and where the bundle runs.
#[derive(Clone, Debug)]
pub struct SceneTie {
    /// Where the tie sits, graph units.
    pub position: Vec2,
    /// Unit direction the slots spread along (perpendicular to the flow).
    pub axis: Vec2,
    /// Wire indices, in slot order.
    pub members: Vec<usize>,
    /// Seconds of wobble left, and how long since it started.
    pub wobble_left: f32,
    pub wobble_t: f32,
}

impl SceneTie {
    /// The two ends of the visible tie bar (and its hitbox centre line).
    pub fn segment(&self) -> (Vec2, Vec2) {
        let n = self.members.len() as f32;
        let half = (n - 1.0).max(0.0) * 0.5 * TIE_SPACING + TIE_MARGIN;
        (
            self.position - self.axis * half,
            self.position + self.axis * half,
        )
    }

    /// Bounding box of the tie bar, for the spatial index.
    pub fn rect(&self) -> crate::geom::Rect {
        let (a, b) = self.segment();
        crate::geom::Rect::from_corners(a, b)
    }

    /// Where member `slot_i` passes the tie, including any wobble.
    pub fn pass_point(&self, slot_i: usize) -> Vec2 {
        let n = self.members.len();
        let slot = (slot_i as f32 - (n as f32 - 1.0) * 0.5) * TIE_SPACING;
        let wob = if self.wobble_left > 0.0 {
            let fade = (self.wobble_left / TIE_WOBBLE_TIME).clamp(0.0, 1.0);
            TIE_WOBBLE_AMP
                * fade
                * fade
                * (TIE_WOBBLE_FREQ * (TIE_WOBBLE_TIME - self.wobble_left) + slot_i as f32 * 0.9)
                    .sin()
        } else {
            0.0
        };
        self.position + self.axis * (slot + wob)
    }
}

pub struct Scene {
    pub metrics: LayoutMetrics,
    pub nodes: Vec<SceneNode>,
    pub comments: Vec<SceneComment>,
    pub wires: Vec<SceneWire>,
    pub ties: Vec<SceneTie>,
    node_ix: HashMap<String, usize>,
    /// node index -> wires touching it
    adjacency: Vec<Vec<usize>>,
    pub spatial: SpatialIndex,
}

impl Scene {
    pub fn new(metrics: LayoutMetrics) -> Scene {
        Scene {
            metrics,
            nodes: Vec::new(),
            comments: Vec::new(),
            wires: Vec::new(),
            ties: Vec::new(),
            node_ix: HashMap::new(),
            adjacency: Vec::new(),
            spatial: SpatialIndex::default(),
        }
    }

    pub fn node_index(&self, id: &str) -> Option<usize> {
        self.node_ix.get(id).copied()
    }

    pub fn node(&self, id: &str) -> Option<&SceneNode> {
        self.node_index(id).map(|i| &self.nodes[i])
    }

    pub fn wires_of(&self, node: usize) -> &[usize] {
        self.adjacency.get(node).map(Vec::as_slice).unwrap_or(&[])
    }

    // ------------------------------------------------------------------ build

    /// Rebuilds everything from the document. Node draw order is the ids' sort
    /// order, which keeps scenes deterministic across hash-map iteration.
    pub fn rebuild(
        &mut self,
        graph: &GraphDescription,
        presenter: &dyn Presenter,
        measure: &dyn TextMeasure,
    ) {
        let mut ids: Vec<&String> = graph.nodes.keys().collect();
        ids.sort();
        self.nodes = ids
            .iter()
            .map(|id| build_node(&graph.nodes[*id], &self.metrics, presenter, measure))
            .collect();
        self.node_ix = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.clone(), i))
            .collect();
        self.comments = graph
            .comments
            .iter()
            .enumerate()
            .map(|(i, c)| SceneComment {
                text: c.text.clone(),
                rect: Rect::from_pos_size(
                    Vec2::new(c.position.x as f32, c.position.y as f32),
                    Vec2::new(c.size.0 as f32, c.size.1 as f32),
                ),
                presentation: presenter.comment(i, c),
                editable: true,
            })
            .collect();
        let doc_comments = self.comments.len();
        for s in presenter.sections(graph) {
            self.comments.push(SceneComment {
                text: s.title,
                rect: s.rect,
                presentation: CommentPresentation { accent: s.accent },
                editable: false,
            });
        }
        debug_assert!(self.comments[..doc_comments].iter().all(|c| c.editable));
        self.rebuild_wires(graph, presenter);
        self.spatial
            .nodes
            .rebuild(self.nodes.iter().enumerate().map(|(i, n)| Entry {
                index: i,
                rect: n.rect(),
            }));
        self.spatial
            .comments
            .rebuild(self.comments.iter().enumerate().map(|(i, c)| Entry {
                index: i,
                rect: c.rect,
            }));
    }

    /// Resolves connections to pin indices, recomputes wire shapes and the
    /// wire spatial layer. Invalid connections (missing node/pin) are skipped;
    /// the document's invariants report them separately.
    pub fn rebuild_wires(&mut self, graph: &GraphDescription, presenter: &dyn Presenter) {
        self.wires.clear();
        for n in &mut self.nodes {
            for p in n.inputs.iter_mut().chain(n.outputs.iter_mut()) {
                p.connected = false;
            }
        }
        for (ci, c) in graph.connections.iter().enumerate() {
            let (Some(&from_node), Some(&to_node)) = (
                self.node_ix.get(&c.source_node),
                self.node_ix.get(&c.target_node),
            ) else {
                continue;
            };
            let Some(from_pin) = self.nodes[from_node]
                .outputs
                .iter()
                .position(|p| p.id == c.source_pin)
            else {
                continue;
            };
            let Some(to_pin) = self.nodes[to_node]
                .inputs
                .iter()
                .position(|p| p.id == c.target_pin)
            else {
                continue;
            };
            self.nodes[from_node].outputs[from_pin].connected = true;
            self.nodes[to_node].inputs[to_pin].connected = true;
            let style = presenter.wire(c, &self.nodes[from_node].outputs[from_pin].style);
            self.wires.push(SceneWire {
                connection: ci,
                from_node,
                from_pin,
                to_node,
                to_pin,
                style,
                shape: WireShape::bezier(Vec2::ZERO, Vec2::ZERO),
            });
        }
        self.adjacency = vec![Vec::new(); self.nodes.len()];
        for (wi, w) in self.wires.iter().enumerate() {
            self.adjacency[w.from_node].push(wi);
            if w.to_node != w.from_node {
                self.adjacency[w.to_node].push(wi);
            }
        }
        self.ties.clear();
        self.resolve_ties(graph);
        for wi in 0..self.wires.len() {
            self.reshape_wire(wi);
        }
        self.spatial
            .wires
            .rebuild(self.wires.iter().enumerate().map(|(i, w)| Entry {
                index: i,
                rect: w.shape.bounds().expand(4.0),
            }));
    }

    /// Re-routes one wire: through its zip tie slot when it is in one, else
    /// its plain shape. Returns the expanded bounds for the spatial index.
    fn reshape_wire(&mut self, wi: usize) -> Rect {
        let w = &self.wires[wi];
        let from = self.nodes[w.from_node]
            .pin_pos(PinSide::Output, w.from_pin)
            .unwrap_or_default();
        let to = self.nodes[w.to_node]
            .pin_pos(PinSide::Input, w.to_pin)
            .unwrap_or_default();
        let shape = match self.tied_through(wi) {
            Some(q) if w.style.routing == WireRouting::Angular => {
                WireShape::through_angular(from, q, to)
            }
            Some(q) => WireShape::through(from, q, to),
            None => WireShape::new(w.style.routing, from, to),
        };
        let bounds = shape.bounds().expand(4.0);
        self.wires[wi].shape = shape;
        bounds
    }

    /// Where wire `wi` passes a zip tie clamp, if it is in one (wobble read
    /// straight off the tie).
    fn tied_through(&self, wi: usize) -> Option<Vec2> {
        self.ties.iter().find_map(|t| {
            t.members
                .iter()
                .position(|&m| m == wi)
                .map(|i| t.pass_point(i))
        })
    }

    /// Resolves the document's zip ties to live wire indices (stale edges are
    /// dropped), routes members through their slots and rebuilds the tie
    /// index. Empty ties stay as placeholders so scene indices always match
    /// the document's.
    fn resolve_ties(&mut self, graph: &GraphDescription) {
        self.ties.clear();
        for doc_tie in &graph.zip_ties {
            let mut members: Vec<usize> = doc_tie
                .wires
                .iter()
                .filter_map(|e| {
                    self.wires.iter().position(|w| {
                        graph
                            .connections
                            .get(w.connection)
                            .is_some_and(|c| e.matches(c))
                    })
                })
                .collect();
            members.dedup();
            // The bar runs perpendicular to the members' average flow.
            let mut flow = Vec2::ZERO;
            for &wi in &members {
                let w = &self.wires[wi];
                if let (Some(a), Some(b)) = (
                    self.nodes[w.from_node].pin_pos(PinSide::Output, w.from_pin),
                    self.nodes[w.to_node].pin_pos(PinSide::Input, w.to_pin),
                ) {
                    let d = b - a;
                    if d.length() > 1e-6 {
                        flow = flow + d * (1.0 / d.length());
                    }
                }
            }
            let mut axis = {
                let a = Vec2::new(-flow.y, flow.x);
                let len = a.length();
                if len < 1e-6 {
                    Vec2::new(0.0, 1.0)
                } else {
                    a * (1.0 / len)
                }
            };
            let position = Vec2::new(doc_tie.position.x as f32, doc_tie.position.y as f32);
            // Point the axis so the doc's member order ascends along it.
            if members.len() >= 2 {
                let proj = |t: &SceneTie, wi: usize| {
                    let w = &self.wires[wi];
                    let a = self.nodes[w.from_node]
                        .pin_pos(PinSide::Output, w.from_pin)
                        .unwrap_or_default();
                    let b = self.nodes[w.to_node]
                        .pin_pos(PinSide::Input, w.to_pin)
                        .unwrap_or_default();
                    let mid = (a + b) * 0.5 - t.position;
                    mid.x * t.axis.x + mid.y * t.axis.y
                };
                let t = SceneTie {
                    position,
                    axis,
                    members: members.clone(),
                    wobble_left: 0.0,
                    wobble_t: 0.0,
                };
                if proj(&t, members[0]) > proj(&t, members[members.len() - 1]) {
                    axis = -axis;
                }
            }
            self.ties.push(SceneTie {
                position,
                axis,
                members,
                wobble_left: 0.0,
                wobble_t: 0.0,
            });
            // Route the members through their slots.
            for slot in 0..self.ties[self.ties.len() - 1].members.len() {
                let wi = self.ties[self.ties.len() - 1].members[slot];
                self.reshape_wire(wi);
            }
        }
        self.spatial
            .ties
            .rebuild(self.ties.iter().enumerate().map(|(i, t)| Entry {
                index: i,
                rect: t.rect().expand(6.0),
            }));
    }

    // ------------------------------------------------------------ incremental

    /// Moves a node in the scene only (drag preview, or applying a
    /// `NodeMoved` change) and updates its wires and index entries.
    pub fn set_node_position(&mut self, index: usize, pos: Vec2) {
        let old = Entry {
            index,
            rect: self.nodes[index].rect(),
        };
        self.nodes[index].pos = pos;
        self.spatial.nodes.update(old, self.nodes[index].rect());
        for wi in self.adjacency[index].clone() {
            let old_bounds = self.wires[wi].shape.bounds().expand(4.0);
            let new_bounds = self.reshape_wire(wi);
            self.spatial.wires.update(
                Entry {
                    index: wi,
                    rect: old_bounds,
                },
                new_bounds,
            );
        }
    }

    pub fn set_comment_rect(&mut self, index: usize, rect: Rect) {
        let old = Entry {
            index,
            rect: self.comments[index].rect,
        };
        self.comments[index].rect = rect;
        self.spatial.comments.update(old, rect);
    }

    /// Moves a tie in the scene only (tie-drag preview) and re-routes its
    /// members; callers commit a `MoveZipTie` change to make it stick.
    pub fn set_zip_tie_position(&mut self, index: usize, pos: Vec2) {
        let old = Entry {
            index,
            rect: self.ties[index].rect().expand(6.0),
        };
        self.ties[index].position = pos;
        for &wi in self.ties[index].members.clone().iter() {
            let old_bounds = self.wires[wi].shape.bounds().expand(4.0);
            let new_bounds = self.reshape_wire(wi);
            self.spatial.wires.update(
                Entry {
                    index: wi,
                    rect: old_bounds,
                },
                new_bounds,
            );
        }
        self.spatial
            .ties
            .update(old, self.ties[index].rect().expand(6.0));
    }

    /// Starts the add-wobble on a tie.
    pub fn start_tie_wobble(&mut self, index: usize) {
        if self.ties.get(index).map_or(true, |t| t.members.is_empty()) {
            return;
        }
        let tie = &mut self.ties[index];
        tie.wobble_left = TIE_WOBBLE_TIME;
        tie.wobble_t = 0.0;
    }

    /// Advances tie wobbles; returns true while anything is still moving.
    pub fn tick_ties(&mut self, dt: f32) -> bool {
        if dt <= 0.0 {
            return false;
        }
        let mut active = false;
        for i in 0..self.ties.len() {
            if self.ties[i].wobble_left > 0.0 {
                active = true;
                self.ties[i].wobble_left = (self.ties[i].wobble_left - dt).max(0.0);
                self.ties[i].wobble_t += dt;
                self.reshuffle_tie(i);
            }
        }
        active
    }

    /// Re-routes one tie's members (wobble animation / drag) and refreshes
    /// the wire index entries.
    fn reshuffle_tie(&mut self, ti: usize) {
        for &wi in self.ties[ti].members.clone().iter() {
            let old_bounds = self.wires[wi].shape.bounds().expand(4.0);
            let new_bounds = self.reshape_wire(wi);
            self.spatial.wires.update(
                Entry {
                    index: wi,
                    rect: old_bounds,
                },
                new_bounds,
            );
        }
    }

    /// Brings the scene in line with the edits in `changes`. Node moves are
    /// applied in place; structural changes relayout the touched nodes and
    /// re-resolve wires; adding or removing a node rebuilds (indices shift).
    pub fn apply_changes(
        &mut self,
        graph: &GraphDescription,
        changes: &ChangeSet,
        presenter: &dyn Presenter,
        measure: &dyn TextMeasure,
    ) {
        let structural = changes.changes.iter().any(|c| {
            matches!(
                c,
                GraphChange::NodeAdded(_) | GraphChange::NodeRemoved { .. }
            )
        });
        if structural || changes.changes.iter().any(|c| !is_incremental(c)) {
            self.rebuild(graph, presenter, measure);
            return;
        }
        let mut wires_dirty = false;
        let mut wobble_tie: Option<usize> = None;
        for change in &changes.changes {
            match change {
                GraphChange::NodeMoved { id, to, .. } => {
                    if let Some(i) = self.node_index(id) {
                        self.set_node_position(i, Vec2::new(to.0 as f32, to.1 as f32));
                    }
                }
                GraphChange::CommentMoved { index, to, .. } => {
                    if let Some(c) = self.comments.get(*index) {
                        let r =
                            Rect::from_pos_size(Vec2::new(to.0 as f32, to.1 as f32), c.rect.size());
                        self.set_comment_rect(*index, r);
                    }
                }
                GraphChange::CommentResized { index, to, .. } => {
                    if let Some(c) = self.comments.get(*index) {
                        let r =
                            Rect::from_pos_size(c.rect.min, Vec2::new(to.0 as f32, to.1 as f32));
                        self.set_comment_rect(*index, r);
                    }
                }
                GraphChange::ConnectionAdded(_) | GraphChange::ConnectionRemoved(_) => {
                    wires_dirty = true
                }
                GraphChange::ZipTieAdded { index, .. } => {
                    wires_dirty = true;
                    wobble_tie = Some(*index);
                }
                GraphChange::ZipTieMoved { .. } | GraphChange::ZipTieRemoved { .. } => {
                    wires_dirty = true
                }
                GraphChange::PropertySet { node, .. } => {
                    self.refresh_node(graph, node, presenter, measure)
                }
                GraphChange::PinsReplaced { node, .. } => {
                    self.refresh_node(graph, node, presenter, measure);
                    wires_dirty = true;
                }
                _ => {}
            }
        }
        if wires_dirty {
            self.rebuild_wires(graph, presenter);
        }
        if let Some(i) = wobble_tie {
            self.start_tie_wobble(i);
        }
    }

    /// Re-presents and relayouts one node (title/pins/properties changed).
    pub fn refresh_node(
        &mut self,
        graph: &GraphDescription,
        id: &str,
        presenter: &dyn Presenter,
        measure: &dyn TextMeasure,
    ) {
        let (Some(i), Some(node)) = (self.node_index(id), graph.nodes.get(id)) else {
            return;
        };
        let old = Entry {
            index: i,
            rect: self.nodes[i].rect(),
        };
        let was_connected: Vec<(PinSide, String)> = self.nodes[i]
            .inputs
            .iter()
            .filter(|p| p.connected)
            .map(|p| (PinSide::Input, p.id.clone()))
            .chain(
                self.nodes[i]
                    .outputs
                    .iter()
                    .filter(|p| p.connected)
                    .map(|p| (PinSide::Output, p.id.clone())),
            )
            .collect();
        self.nodes[i] = build_node(node, &self.metrics, presenter, measure);
        for (side, pid) in &was_connected {
            let n = &mut self.nodes[i];
            let list = if *side == PinSide::Input {
                &mut n.inputs
            } else {
                &mut n.outputs
            };
            if let Some(p) = list.iter_mut().find(|p| &p.id == pid) {
                p.connected = true;
            }
        }
        self.spatial.nodes.update(old, self.nodes[i].rect());
        for wi in self.adjacency.get(i).cloned().unwrap_or_default() {
            let old_bounds = self.wires[wi].shape.bounds().expand(4.0);
            let new_bounds = self.reshape_wire(wi);
            self.spatial.wires.update(
                Entry {
                    index: wi,
                    rect: old_bounds,
                },
                new_bounds,
            );
        }
    }

    // ---------------------------------------------------------------- queries

    /// Bounds of every node and comment, if any.
    pub fn bounds(&self) -> Option<Rect> {
        self.nodes
            .iter()
            .map(SceneNode::rect)
            .chain(self.comments.iter().map(|c| c.rect))
            .reduce(|a, b| a.union(&b))
    }

    pub fn nodes_in(&self, rect: Rect) -> Vec<usize> {
        self.spatial.nodes.query(rect, false)
    }

    /// Nodes touched by (`contain == false`) or fully inside (`true`) `rect`.
    pub fn marquee_nodes(&self, rect: Rect, contain: bool) -> Vec<usize> {
        self.spatial
            .nodes
            .query(rect, false)
            .into_iter()
            .filter(|&i| {
                let r = self.nodes[i].rect();
                !contain
                    || (r.min.x >= rect.min.x
                        && r.min.y >= rect.min.y
                        && r.max.x <= rect.max.x
                        && r.max.y <= rect.max.y)
            })
            .collect()
    }
}

fn is_incremental(c: &GraphChange) -> bool {
    matches!(
        c,
        GraphChange::NodeMoved { .. }
            | GraphChange::ConnectionAdded(_)
            | GraphChange::ConnectionRemoved(_)
            | GraphChange::PropertySet { .. }
            | GraphChange::PinsReplaced { .. }
            | GraphChange::ZipTieAdded { .. }
            | GraphChange::ZipTieMoved { .. }
            | GraphChange::ZipTieRemoved { .. },
    )}

fn build_node(
    node: &graphy::NodeInstance,
    m: &LayoutMetrics,
    presenter: &dyn Presenter,
    measure: &dyn TextMeasure,
) -> SceneNode {
    let presentation = presenter.node(node);
    let pin_info = |list: &[graphy::PinInstance], side: PinSide| -> Vec<PinInfo> {
        list.iter()
            .map(|p| PinInfo {
                id: p.id.clone(),
                label: presenter.pin_label(node, p, side),
                style: presenter.pin(node, p, side),
                connected: false,
                widget: if side == PinSide::Input {
                    presenter.pin_widget(node, p, side)
                } else {
                    None
                },
                label_width: measure
                    .width(&presenter.pin_label(node, p, side), m.label_font)
                    .min(MAX_LABEL_WIDTH),
            })
            .collect()
    };
    let inputs = pin_info(&node.inputs, PinSide::Input);
    let outputs = pin_info(&node.outputs, PinSide::Output);

    let label_w =
        |p: &PinInfo| p.label_width + p.widget.as_ref().map_or(0.0, |w| WIDGET_GAP + w.width);
    let header_output = presentation.header_output.filter(|h| *h < outputs.len());
    let (rows, extra_min) = NodeSpec::build_rows(
        inputs.len(),
        outputs.len(),
        header_output,
        &presentation.extra_rows,
        |c| presenter.row_height(node, c, m),
    );
    // Rows carrying the header output's label would double-count; the header
    // output has no label slot.
    let mut output_widths: Vec<f32> = outputs.iter().map(label_w).collect();
    if let Some(h) = header_output {
        output_widths[h] = 0.0;
    }
    let title_width = if presentation.shape == NodeShape::Reroute {
        0.0
    } else {
        measure
            .width(&presentation.title, m.title_font)
            .min(MAX_TITLE_WIDTH)
    };
    let spec = NodeSpec {
        shape: presentation.shape,
        title_width,
        has_icon: presentation.icon.is_some(),
        input_label_widths: inputs.iter().map(label_w).collect(),
        output_label_widths: output_widths,
        header_output,
        rows,
        min_width: presentation.min_width.max(extra_min),
    };
    let layout = NodeLayout::compute(&spec, m);
    SceneNode {
        id: node.id.clone(),
        pos: Vec2::new(node.position.x as f32, node.position.y as f32),
        presentation,
        layout,
        inputs,
        outputs,
    }
}
