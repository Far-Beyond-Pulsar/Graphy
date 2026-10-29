//! The shared interaction engine.
//!
//! [`Controller`] turns [`InputEvent`]s into [`Action`]s. It never mutates the
//! document: graph edits leave as [`Action::Commands`] (one undoable
//! transaction each) and the editor applies them through its own
//! document/undo/save path, then calls `Scene::apply_changes`. While a drag is
//! in flight the *scene* is moved transiently so the canvas stays live, and a
//! single command batch is emitted on release.
//!
//! All hit testing goes through [`graphy_render::Scene`], i.e. the same layout
//! the renderer draws from.

use std::collections::{BTreeSet, HashMap, HashSet};

use graphy::{Connection, ConnectionType, GraphCommand, GraphDescription, PinRef, Position};
use graphy_render::{
    ConnectPreview, Hit, HitTolerance, InteractionState, NodePart, PinAddr, PinSide, Rect, Scene,
    TextMeasure, Vec2, ViewTransform, WireRouting,
};

use crate::clipboard;
use crate::input::{CursorHint, InputEvent, Key, Modifiers, MouseButton};
use crate::profile::{dynamic_pin_commands, GraphProfile, MacroTarget, MenuItem, MenuTarget};
use crate::snap::{snap_move, SnapOptions};

#[derive(Clone, Debug)]
pub struct InteractionSettings {
    pub snap_to_grid: bool,
    pub snap_to_nodes: bool,
    /// Capture distance for alignment snapping, screen pixels.
    pub snap_threshold_px: f32,
    /// Plain wheel zooms (otherwise it pans and Ctrl zooms).
    pub wheel_zooms: bool,
    /// Zoom factor per wheel notch.
    pub zoom_step: f32,
    /// Movement before a press becomes a drag, screen pixels.
    pub drag_threshold_px: f32,
    /// Marquee selects only fully enclosed nodes.
    pub marquee_contain: bool,
    pub wire_routing: WireRouting,
    /// Id prefix for nodes created by paste/duplicate/reroute.
    pub id_prefix: String,
    pub paste_offset: f32,
}

impl Default for InteractionSettings {
    fn default() -> Self {
        Self {
            snap_to_grid: true,
            snap_to_nodes: true,
            snap_threshold_px: 6.0,
            wheel_zooms: true,
            zoom_step: 1.1,
            drag_threshold_px: 4.0,
            marquee_contain: false,
            wire_routing: WireRouting::Bezier,
            id_prefix: "node".into(),
            paste_offset: 30.0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub nodes: BTreeSet<String>,
    pub comments: BTreeSet<usize>,
    /// Connections, by index into the document's connection list.
    pub wires: BTreeSet<usize>,
    /// Zip ties, by index into the document's tie list.
    pub ties: BTreeSet<usize>,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
            && self.comments.is_empty()
            && self.wires.is_empty()
            && self.ties.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Selection::default();
    }
}

/// What the editor must do in response to input.
#[derive(Clone, Debug)]
pub enum Action {
    /// Apply in order as one undoable edit. Use [`apply_transaction`].
    Commands {
        label: &'static str,
        commands: Vec<GraphCommand>,
    },
    SelectionChanged,
    ViewChanged,
    ContextMenu {
        at: Vec2,
        target: MenuTarget,
        items: Vec<MenuItem>,
    },
    /// Double-click on a node whose profile says it opens another graph.
    OpenGraph(MacroTarget),
    /// Double-click on any other node.
    NodeActivated(String),
    /// Double-click on a comment's header (rename it).
    CommentActivated(usize),
    /// A host row (preview, inline widget) was pressed; `local` is relative to
    /// the row rectangle in graph units.
    SlotPressed {
        node: String,
        slot: u32,
        local: Vec2,
    },
    /// An inline pin value chip was clicked.
    EditPinWidget {
        node: String,
        pin: String,
    },
    /// A connection drag ended on empty canvas: offer a node list filtered by
    /// the dragged pin.
    ConnectionDropped {
        from: PinRef,
        side: PinSide,
        /// Pointer position, canvas-relative pixels (as in [`Action::ContextMenu`]).
        at: Vec2,
        /// The same point in graph space, where a new node would go.
        graph_at: Vec2,
    },
    /// A domain menu item the profile left to the host (its id and the target
    /// it was chosen on).
    Custom {
        id: String,
        target: MenuTarget,
    },
    /// The drag target refused the connection.
    ConnectionRefused(String),
    /// Put this text on the system clipboard.
    ClipboardWrite(String),
    /// Ctrl+V: read the system clipboard and send back `InputEvent::Paste`.
    ClipboardReadRequest,
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub actions: Vec<Action>,
    pub redraw: bool,
}

pub struct GraphCtx<'a> {
    pub graph: &'a GraphDescription,
    pub scene: &'a mut Scene,
    pub profile: &'a dyn GraphProfile,
    pub measure: &'a dyn TextMeasure,
}

enum Mode {
    Idle,
    Pan {
        last: Vec2,
        button: MouseButton,
        start: Vec2,
        moved: bool,
        target: MenuTarget,
    },
    Marquee {
        start: Vec2,
        base: Selection,
    },
    PendingNode {
        node: usize,
        down_screen: Vec2,
        toggle_off: bool,
    },
    MoveNodes {
        down: Vec2,
        primary: usize,
        origin: Vec<(usize, Vec2)>,
        comments: Vec<(usize, Rect)>,
    },
    MoveComment {
        index: usize,
        down: Vec2,
        origin: Rect,
        carried: Vec<(usize, Vec2)>,
    },
    ResizeComment {
        index: usize,
        down: Vec2,
        origin: Rect,
    },
    Connect {
        anchor: PinAddr,
        replacing: Option<(usize, Connection)>,
        snapped: Option<PinAddr>,
    },
    /// Middle-drag over wires: once the drag crosses wires, release creates a
    /// zip tie clamping them.
    ZipLasso {
        start: Vec2,
    },
    /// Middle-drag on a tie bar: the bundle moves with the cursor (scene
    /// transient), a `MoveZipTie` is committed if it moved.
    MoveZipTie {
        index: usize,
        origin: Vec2,
        down: Vec2,
    },
}

pub struct Controller {
    pub view: ViewTransform,
    /// Canvas size, logical pixels.
    pub viewport: Vec2,
    pub selection: Selection,
    pub settings: InteractionSettings,
    pub focused: Option<String>,
    mode: Mode,
    hover: Option<Hit>,
    cursor_graph: Vec2,
    marquee: Option<Rect>,
    guides: Vec<Rect>,
    compatible: HashSet<PinAddr>,
    clipboard: Option<String>,
    /// Live while zip-tying: (drag start, current graph point).
    zip_lasso: Option<(Vec2, Vec2)>,
}

impl Controller {
    pub fn new(viewport: Vec2) -> Self {
        Self {
            view: ViewTransform::default(),
            viewport,
            selection: Selection::default(),
            settings: InteractionSettings::default(),
            focused: None,
            mode: Mode::Idle,
            hover: None,
            cursor_graph: Vec2::ZERO,
            marquee: None,
            guides: Vec::new(),
            compatible: HashSet::new(),
            clipboard: None,
            zip_lasso: None,
        }
    }

    pub fn is_dragging(&self) -> bool {
        !matches!(self.mode, Mode::Idle)
    }

    pub fn tolerance(&self) -> HitTolerance {
        HitTolerance::default()
    }

    pub fn cursor(&self) -> CursorHint {
        match &self.mode {
            Mode::Pan { .. } => CursorHint::Grabbing,
            Mode::MoveNodes { .. } | Mode::MoveComment { .. } | Mode::MoveZipTie { .. } => {
                CursorHint::Move
            }
            Mode::ResizeComment { .. } => CursorHint::ResizeNwse,
            Mode::Connect { .. } | Mode::ZipLasso { .. } => CursorHint::Crosshair,
            Mode::Marquee { .. } => CursorHint::Default,
            _ => match self.hover {
                Some(Hit::Pin(_)) => CursorHint::Crosshair,
                Some(Hit::PinWidget(_)) | Some(Hit::Wire(_)) | Some(Hit::ZipTie(_)) => {
                    CursorHint::Pointer
                }
                Some(Hit::CommentResize(_)) => CursorHint::ResizeNwse,
                Some(Hit::CommentHeader(_))
                | Some(Hit::Node {
                    part: NodePart::Header,
                    ..
                }) => CursorHint::Move,
                _ => CursorHint::Default,
            },
        }
    }

    /// The transient state the renderer needs this frame.
    pub fn interaction_state(&self, scene: &Scene) -> InteractionState {
        let mut st = InteractionState {
            hovered: self.hover,
            selected_nodes: self.selection.nodes.iter().cloned().collect(),
            selected_comments: self.selection.comments.iter().copied().collect(),
            selected_wires: self.selection.wires.iter().copied().collect(),
            selected_ties: self.selection.ties.iter().copied().collect(),
            focused_node: self.focused.clone(),
            marquee: self.marquee,
            guides: self.guides.clone(),
            zip_preview: self.zip_lasso,
            ..Default::default()
        };
        match &self.mode {
            Mode::MoveNodes { origin, .. } => {
                st.dragging_nodes = origin
                    .iter()
                    .filter_map(|(i, _)| scene.nodes.get(*i))
                    .map(|n| n.id.clone())
                    .collect();
            }
            Mode::Connect {
                anchor,
                replacing,
                snapped,
            } => {
                if let Some((ci, _)) = replacing {
                    st.hidden_connections.insert(*ci);
                }
                st.connecting = Some(ConnectPreview {
                    anchor: *anchor,
                    cursor: self.cursor_graph,
                    snapped: *snapped,
                    routing: self.settings.wire_routing,
                    compatible: self.compatible.clone(),
                });
            }
            _ => {}
        }
        st
    }

    // ------------------------------------------------------------ view ops

    pub fn frame_all(&mut self, scene: &Scene) -> bool {
        match scene.bounds() {
            Some(b) => {
                self.view = ViewTransform::framing(b, self.viewport, 60.0, 1.0);
                true
            }
            None => false,
        }
    }

    pub fn frame_selection(&mut self, scene: &Scene) -> bool {
        let rect = self
            .selection
            .nodes
            .iter()
            .filter_map(|id| scene.node(id))
            .map(|n| n.rect())
            .chain(
                self.selection
                    .comments
                    .iter()
                    .filter_map(|i| scene.comments.get(*i))
                    .map(|c| c.rect),
            )
            .reduce(|a, b| a.union(&b));
        match rect {
            Some(b) => {
                self.view = ViewTransform::framing(b, self.viewport, 80.0, 1.2);
                true
            }
            None => self.frame_all(scene),
        }
    }

    pub fn select_all(&mut self, scene: &Scene) {
        self.selection.nodes = scene.nodes.iter().map(|n| n.id.clone()).collect();
        self.selection.comments = scene
            .comments
            .iter()
            .enumerate()
            .filter(|(_, c)| c.editable)
            .map(|(i, _)| i)
            .collect();
    }

    // -------------------------------------------------------------- events

    pub fn handle(&mut self, ev: InputEvent, cx: &mut GraphCtx) -> Outcome {
        let mut out = Outcome::default();
        match ev {
            InputEvent::PointerDown {
                pos,
                button,
                mods,
                clicks,
            } => self.pointer_down(pos, button, mods, clicks, cx, &mut out),
            InputEvent::PointerMove { pos, mods } => self.pointer_move(pos, mods, cx, &mut out),
            InputEvent::PointerUp { pos, button, mods } => {
                self.pointer_up(pos, button, mods, cx, &mut out)
            }
            InputEvent::Wheel {
                pos,
                delta,
                pixels,
                mods,
            } => self.wheel(pos, delta, pixels, mods, &mut out),
            InputEvent::KeyDown { key, mods } => self.key(key, mods, cx, &mut out),
            InputEvent::Paste(text) => self.paste(&text, cx, &mut out),
            InputEvent::PointerLeave => {
                if self.hover.take().is_some() {
                    out.redraw = true;
                }
            }
        }
        out
    }

    fn graph_pos(&self, screen: Vec2) -> Vec2 {
        self.view.screen_to_graph(screen)
    }

    fn pin_ref(cx: &GraphCtx, a: PinAddr) -> Option<PinRef> {
        let n = cx.scene.nodes.get(a.node)?;
        let p = n.pins(a.side).get(a.pin)?;
        Some(PinRef::new(&n.id, &p.id))
    }

    /// The connection (document index) feeding an input pin.
    fn feeding(cx: &GraphCtx, a: PinAddr) -> Option<(usize, Connection)> {
        let r = Self::pin_ref(cx, a)?;
        cx.graph
            .connections
            .iter()
            .enumerate()
            .find(|(_, c)| c.target_node == r.node && c.target_pin == r.pin)
            .map(|(i, c)| (i, c.clone()))
    }

    fn select_node(&mut self, id: &str, additive: bool) {
        if !additive {
            self.selection.clear();
        }
        self.selection.nodes.insert(id.to_string());
        self.focused = Some(id.to_string());
    }

    fn pointer_down(
        &mut self,
        pos: Vec2,
        button: MouseButton,
        mods: Modifiers,
        clicks: u8,
        cx: &mut GraphCtx,
        out: &mut Outcome,
    ) {
        let g = self.graph_pos(pos);
        self.cursor_graph = g;
        let hit = cx.scene.hit_test(g, self.view.zoom, self.tolerance());
        out.redraw = true;
        match button {
            MouseButton::Middle => match hit {
                Hit::ZipTie(index) => {
                    self.mode = Mode::MoveZipTie {
                        index,
                        origin: cx.scene.ties[index].position,
                        down: g,
                    };
                }
                _ => {
                    // Tying gesture starts anywhere so the stroke can cross wires
                    // from empty canvas, nodes, or other graph content.
                    self.zip_lasso = Some((g, g));
                    self.mode = Mode::ZipLasso { start: g };
                }
            },
            MouseButton::Right => {
                let target = self.menu_target(&hit, cx, g);
                if let Hit::Node { node, .. } = hit {
                    let id = cx.scene.nodes[node].id.clone();
                    if !self.selection.nodes.contains(&id) {
                        self.select_node(&id, false);
                        out.actions.push(Action::SelectionChanged);
                    }
                }
                self.mode = Mode::Pan {
                    last: pos,
                    button,
                    start: pos,
                    moved: false,
                    target,
                };
            }
            MouseButton::Left => self.left_down(pos, g, hit, mods, clicks, cx, out),
        }
    }

    fn menu_target(&self, hit: &Hit, cx: &GraphCtx, g: Vec2) -> MenuTarget {
        match *hit {
            Hit::Pin(a) | Hit::PinWidget(a) => {
                Self::pin_ref(cx, a).map_or(MenuTarget::Canvas { graph_pos: g }, MenuTarget::Pin)
            }
            Hit::Node { node, .. } => {
                let id = cx.scene.nodes[node].id.clone();
                if self.selection.nodes.len() > 1 && self.selection.nodes.contains(&id) {
                    MenuTarget::Selection
                } else {
                    MenuTarget::Node(id)
                }
            }
            Hit::Wire(w) => cx
                .graph
                .connections
                .get(cx.scene.wires[w].connection)
                .cloned()
                .map_or(MenuTarget::Canvas { graph_pos: g }, MenuTarget::Wire),
            Hit::CommentHeader(i) | Hit::CommentResize(i) => MenuTarget::Comment(i),
            Hit::ZipTie(_) => MenuTarget::Canvas { graph_pos: g },
            Hit::None => MenuTarget::Canvas { graph_pos: g },
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn left_down(
        &mut self,
        pos: Vec2,
        g: Vec2,
        hit: Hit,
        mods: Modifiers,
        clicks: u8,
        cx: &mut GraphCtx,
        out: &mut Outcome,
    ) {
        match hit {
            Hit::Pin(a) => {
                let Some(pin) = Self::pin_ref(cx, a) else {
                    return;
                };
                if mods.alt {
                    // Alt-click breaks every link on the pin.
                    let cmds: Vec<GraphCommand> = cx
                        .graph
                        .connections
                        .iter()
                        .filter(|c| match a.side {
                            PinSide::Input => c.target_node == pin.node && c.target_pin == pin.pin,
                            PinSide::Output => c.source_node == pin.node && c.source_pin == pin.pin,
                        })
                        .cloned()
                        .map(GraphCommand::Disconnect)
                        .collect();
                    if !cmds.is_empty() {
                        out.actions.push(Action::Commands {
                            label: "Break pin links",
                            commands: cmds,
                        });
                    }
                    return;
                }
                // Dragging a connected input picks its wire up and re-routes it
                // from the original source.
                let (anchor, replacing) = match (a.side, Self::feeding(cx, a)) {
                    (PinSide::Input, Some((ci, conn))) => {
                        match cx.scene.node_index(&conn.source_node).and_then(|n| {
                            cx.scene.nodes[n]
                                .outputs
                                .iter()
                                .position(|p| p.id == conn.source_pin)
                                .map(|p| PinAddr {
                                    node: n,
                                    side: PinSide::Output,
                                    pin: p,
                                })
                        }) {
                            Some(src) => (src, Some((ci, conn))),
                            None => (a, None),
                        }
                    }
                    _ => (a, None),
                };
                self.compatible = self.compatible_pins(anchor, cx);
                self.mode = Mode::Connect {
                    anchor,
                    replacing,
                    snapped: None,
                };
            }
            Hit::PinWidget(a) => {
                if let Some(r) = Self::pin_ref(cx, a) {
                    out.actions.push(Action::EditPinWidget {
                        node: r.node,
                        pin: r.pin,
                    });
                }
            }
            Hit::Node { node, part } => {
                let id = cx.scene.nodes[node].id.clone();
                if clicks >= 2 {
                    match cx
                        .graph
                        .nodes
                        .get(&id)
                        .and_then(|n| cx.profile.macro_target(n))
                    {
                        Some(t) => out.actions.push(Action::OpenGraph(t)),
                        None => out.actions.push(Action::NodeActivated(id)),
                    }
                    return;
                }
                if let NodePart::Custom(slot) = part {
                    let n = &cx.scene.nodes[node];
                    let local = n
                        .layout
                        .rows
                        .iter()
                        .find(|r| matches!(r.content, graphy_render::RowContent::Custom { slot: s } if s == slot))
                        .map_or(Vec2::ZERO, |r| g - (n.pos + r.rect.min));
                    self.select_node(&id, mods.additive());
                    out.actions.push(Action::SelectionChanged);
                    out.actions.push(Action::SlotPressed {
                        node: id,
                        slot,
                        local,
                    });
                    return;
                }
                let was_selected = self.selection.nodes.contains(&id);
                if mods.additive() {
                    if was_selected {
                        // toggled off on release if this turns out not to be a drag
                    } else {
                        self.select_node(&id, true);
                        out.actions.push(Action::SelectionChanged);
                    }
                } else if !was_selected {
                    self.select_node(&id, false);
                    out.actions.push(Action::SelectionChanged);
                } else {
                    self.focused = Some(id.clone());
                }
                self.mode = Mode::PendingNode {
                    node,
                    down_screen: pos,
                    toggle_off: mods.additive() && was_selected,
                };
            }
            Hit::CommentHeader(i) => {
                let c = &cx.scene.comments[i];
                if !c.editable {
                    return;
                }
                if !mods.additive() {
                    self.selection.clear();
                }
                self.selection.comments.insert(i);
                out.actions.push(Action::SelectionChanged);
                if clicks >= 2 {
                    out.actions.push(Action::CommentActivated(i));
                    return;
                }
                let carried = cx
                    .scene
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| {
                        let r = n.rect();
                        r.min.x >= c.rect.min.x
                            && r.min.y >= c.rect.min.y
                            && r.max.x <= c.rect.max.x
                            && r.max.y <= c.rect.max.y
                    })
                    .map(|(ni, n)| (ni, n.pos))
                    .collect();
                self.mode = Mode::MoveComment {
                    index: i,
                    down: g,
                    origin: c.rect,
                    carried,
                };
            }
            Hit::CommentResize(i) => {
                let c = &cx.scene.comments[i];
                if c.editable {
                    self.selection.clear();
                    self.selection.comments.insert(i);
                    out.actions.push(Action::SelectionChanged);
                    self.mode = Mode::ResizeComment {
                        index: i,
                        down: g,
                        origin: c.rect,
                    };
                }
            }
            Hit::Wire(w) => {
                let ci = cx.scene.wires[w].connection;
                if clicks >= 2 {
                    if let Some(cmds) = self.reroute_commands(ci, g, cx) {
                        out.actions.push(Action::Commands {
                            label: "Insert reroute",
                            commands: cmds,
                        });
                    }
                    return;
                }
                if !mods.additive() {
                    self.selection.clear();
                }
                self.selection.wires.insert(ci);
                out.actions.push(Action::SelectionChanged);
            }
            Hit::ZipTie(i) => {
                if !mods.additive() {
                    self.selection.clear();
                }
                self.selection.ties.insert(i);
                out.actions.push(Action::SelectionChanged);
            }
            Hit::None => {
                let base = if mods.additive() {
                    self.selection.clone()
                } else {
                    Selection::default()
                };
                if !mods.additive() && !self.selection.is_empty() {
                    self.selection.clear();
                    self.focused = None;
                    out.actions.push(Action::SelectionChanged);
                }
                self.mode = Mode::Marquee { start: g, base };
            }
        }
    }

    fn compatible_pins(&self, anchor: PinAddr, cx: &GraphCtx) -> HashSet<PinAddr> {
        let mut set = HashSet::new();
        let Some(a) = Self::pin_ref(cx, anchor) else {
            return set;
        };
        let want = if anchor.side == PinSide::Output {
            PinSide::Input
        } else {
            PinSide::Output
        };
        let visible = self.view.visible_rect(self.viewport, 200.0);
        for ni in cx.scene.nodes_in(visible) {
            if ni == anchor.node {
                continue;
            }
            for (pi, p) in cx.scene.nodes[ni].pins(want).iter().enumerate() {
                let other = PinRef::new(&cx.scene.nodes[ni].id, &p.id);
                let ok = if anchor.side == PinSide::Output {
                    cx.profile.can_connect(cx.graph, &a, &other)
                } else {
                    cx.profile.can_connect(cx.graph, &other, &a)
                };
                if ok.is_ok() {
                    set.insert(PinAddr {
                        node: ni,
                        side: want,
                        pin: pi,
                    });
                }
            }
        }
        set
    }

    fn reroute_commands(&self, ci: usize, at: Vec2, cx: &GraphCtx) -> Option<Vec<GraphCommand>> {
        let c = cx.graph.connections.get(ci)?.clone();
        let src = cx.graph.nodes.get(&c.source_node)?;
        let ty = src
            .outputs
            .iter()
            .find(|p| p.id == c.source_pin)?
            .data_type()
            .clone();
        let mut alloc = graphy::IdAllocator::for_graph(cx.graph, &self.settings.id_prefix);
        let id = alloc.allocate().0;
        let size = cx.scene.metrics.reroute_size as f64;
        let node = cx.profile.create_reroute(
            &id,
            Position {
                x: at.x as f64 - size * 0.5,
                y: at.y as f64 - size * 0.5,
            },
            &ty,
        );
        let (inp, outp) = (
            node.inputs.first()?.id.clone(),
            node.outputs.first()?.id.clone(),
        );
        Some(vec![
            GraphCommand::Disconnect(c.clone()),
            GraphCommand::AddNode(node),
            GraphCommand::Connect(Connection::new(
                &c.source_node,
                &c.source_pin,
                &id,
                &inp,
                c.connection_type,
            )),
            GraphCommand::Connect(Connection::new(
                &id,
                &outp,
                &c.target_node,
                &c.target_pin,
                c.connection_type,
            )),
        ])
    }

    // ---------------------------------------------------------------- move

    fn snap_opts(&self, cx: &GraphCtx) -> SnapOptions {
        SnapOptions {
            grid: self
                .settings
                .snap_to_grid
                .then_some(cx.scene.metrics.grid_snap),
            align_to_nodes: self.settings.snap_to_nodes,
            threshold: self.settings.snap_threshold_px / self.view.zoom,
            guide_thickness: 1.0 / self.view.zoom,
        }
    }

    fn pointer_move(&mut self, pos: Vec2, mods: Modifiers, cx: &mut GraphCtx, out: &mut Outcome) {
        let g = self.graph_pos(pos);
        self.cursor_graph = g;
        match std::mem::replace(&mut self.mode, Mode::Idle) {
            Mode::Idle => {
                let hit = cx.scene.hit_test(g, self.view.zoom, self.tolerance());
                if self.hover != Some(hit) {
                    self.hover = Some(hit);
                    out.redraw = true;
                }
            }
            Mode::Pan {
                last,
                button,
                start,
                moved,
                target,
            } => {
                let moved = moved || pos.distance(start) > self.settings.drag_threshold_px;
                if moved {
                    self.view.pan_by_screen(pos - last);
                    out.actions.push(Action::ViewChanged);
                    out.redraw = true;
                }
                self.mode = Mode::Pan {
                    last: pos,
                    button,
                    start,
                    moved,
                    target,
                };
            }
            Mode::Marquee { start, base } => {
                let rect = Rect::from_corners(start, g);
                self.marquee = Some(rect);
                let hits = cx.scene.marquee_nodes(rect, self.settings.marquee_contain);
                let mut sel = base.clone();
                sel.nodes
                    .extend(hits.into_iter().map(|i| cx.scene.nodes[i].id.clone()));
                if sel != self.selection {
                    self.selection = sel;
                    out.actions.push(Action::SelectionChanged);
                }
                out.redraw = true;
                self.mode = Mode::Marquee { start, base };
            }
            Mode::PendingNode {
                node,
                down_screen,
                toggle_off,
            } => {
                if pos.distance(down_screen) > self.settings.drag_threshold_px {
                    let origin = self
                        .selection
                        .nodes
                        .iter()
                        .filter_map(|id| cx.scene.node_index(id))
                        .map(|i| (i, cx.scene.nodes[i].pos))
                        .collect();
                    let comments = self
                        .selection
                        .comments
                        .iter()
                        .filter_map(|i| {
                            cx.scene
                                .comments
                                .get(*i)
                                .filter(|c| c.editable)
                                .map(|c| (*i, c.rect))
                        })
                        .collect();
                    let down = self.graph_pos(down_screen);
                    self.mode = Mode::MoveNodes {
                        down,
                        primary: node,
                        origin,
                        comments,
                    };
                    self.pointer_move(pos, mods, cx, out);
                } else {
                    self.mode = Mode::PendingNode {
                        node,
                        down_screen,
                        toggle_off,
                    };
                }
            }
            Mode::MoveNodes {
                down,
                primary,
                origin,
                comments,
            } => {
                let mut delta = g - down;
                let primary_origin = origin
                    .iter()
                    .find(|(i, _)| *i == primary)
                    .map_or(Vec2::ZERO, |(_, p)| *p);
                let moving = Rect::from_pos_size(
                    primary_origin + delta,
                    cx.scene.nodes[primary].layout.size,
                );
                let neighbours: Vec<Rect> = if self.settings.snap_to_nodes {
                    let moving_ids: HashSet<usize> = origin.iter().map(|(i, _)| *i).collect();
                    cx.scene
                        .nodes_in(moving.expand(self.snap_opts(cx).threshold * 4.0 + 200.0))
                        .into_iter()
                        .filter(|i| !moving_ids.contains(i))
                        .map(|i| cx.scene.nodes[i].rect())
                        .collect()
                } else {
                    Vec::new()
                };
                let snap = if mods.alt {
                    Default::default()
                } else {
                    snap_move(moving, &neighbours, self.snap_opts(cx))
                };
                delta = delta + snap.delta;
                self.guides = snap.guides;
                for (i, p) in &origin {
                    cx.scene.set_node_position(*i, *p + delta);
                }
                for (i, r) in &comments {
                    cx.scene.set_comment_rect(*i, r.translate(delta));
                }
                out.redraw = true;
                self.mode = Mode::MoveNodes {
                    down,
                    primary,
                    origin,
                    comments,
                };
            }
            Mode::MoveComment {
                index,
                down,
                origin,
                carried,
            } => {
                let mut delta = g - down;
                if self.settings.snap_to_grid {
                    let gs = cx.scene.metrics.grid_snap;
                    delta = Vec2::new(
                        ((origin.min.x + delta.x) / gs).round() * gs - origin.min.x,
                        ((origin.min.y + delta.y) / gs).round() * gs - origin.min.y,
                    );
                }
                cx.scene.set_comment_rect(index, origin.translate(delta));
                for (i, p) in &carried {
                    cx.scene.set_node_position(*i, *p + delta);
                }
                out.redraw = true;
                self.mode = Mode::MoveComment {
                    index,
                    down,
                    origin,
                    carried,
                };
            }
            Mode::ResizeComment {
                index,
                down,
                origin,
            } => {
                let mut size = origin.size() + (g - down);
                let gs = cx.scene.metrics.grid_snap;
                size = Vec2::new(
                    size.x.max(80.0),
                    size.y.max(cx.scene.metrics.comment_header_h * 2.0),
                );
                if self.settings.snap_to_grid {
                    size = Vec2::new((size.x / gs).round() * gs, (size.y / gs).round() * gs);
                }
                cx.scene
                    .set_comment_rect(index, Rect::from_pos_size(origin.min, size));
                out.redraw = true;
                self.mode = Mode::ResizeComment {
                    index,
                    down,
                    origin,
                };
            }
            Mode::Connect {
                anchor, replacing, ..
            } => {
                let want = if anchor.side == PinSide::Output {
                    PinSide::Input
                } else {
                    PinSide::Output
                };
                let snapped = cx
                    .scene
                    .pin_at(g, self.view.zoom, self.tolerance())
                    .filter(|a| a.side == want && self.compatible.contains(a));
                out.redraw = true;
                self.mode = Mode::Connect {
                    anchor,
                    replacing,
                    snapped,
                };
            }
            Mode::ZipLasso { start } => {
                self.zip_lasso = Some((start, g));
                out.redraw = true;
                self.mode = Mode::ZipLasso { start };
            }
            Mode::MoveZipTie {
                index,
                origin,
                down,
            } => {
                cx.scene.set_zip_tie_position(index, origin + (g - down));
                out.redraw = true;
                self.mode = Mode::MoveZipTie {
                    index,
                    origin,
                    down,
                };
            }
        }
    }

    fn pointer_up(
        &mut self,
        pos: Vec2,
        button: MouseButton,
        mods: Modifiers,
        cx: &mut GraphCtx,
        out: &mut Outcome,
    ) {
        let g = self.graph_pos(pos);
        out.redraw = true;
        let mode = std::mem::replace(&mut self.mode, Mode::Idle);
        self.marquee = None;
        self.guides.clear();
        match mode {
            Mode::Idle => {}
            Mode::Pan {
                button: b,
                moved,
                target,
                ..
            } => {
                if b == MouseButton::Right && b == button && !moved {
                    let items = self.menu_items(&target, cx);
                    out.actions.push(Action::ContextMenu {
                        at: pos,
                        target,
                        items,
                    });
                }
            }
            Mode::Marquee { .. } => {}
            Mode::PendingNode {
                node, toggle_off, ..
            } => {
                let id = cx.scene.nodes[node].id.clone();
                if toggle_off {
                    self.selection.nodes.remove(&id);
                    out.actions.push(Action::SelectionChanged);
                } else if !mods.additive() && self.selection.nodes.len() > 1 {
                    // a plain click inside a multi-selection narrows to the node
                    self.selection.clear();
                    self.selection.nodes.insert(id.clone());
                    self.focused = Some(id);
                    out.actions.push(Action::SelectionChanged);
                }
            }
            Mode::MoveNodes {
                origin, comments, ..
            } => {
                let mut cmds = Vec::new();
                for (i, p) in &origin {
                    let now = cx.scene.nodes[*i].pos;
                    if now != *p {
                        cmds.push(GraphCommand::MoveNode {
                            id: cx.scene.nodes[*i].id.clone(),
                            x: now.x as f64,
                            y: now.y as f64,
                        });
                    }
                }
                for (i, r) in &comments {
                    let now = cx.scene.comments[*i].rect;
                    if now != *r {
                        cmds.push(GraphCommand::MoveComment {
                            index: *i,
                            x: now.min.x as f64,
                            y: now.min.y as f64,
                        });
                    }
                }
                if !cmds.is_empty() {
                    out.actions.push(Action::Commands {
                        label: "Move",
                        commands: cmds,
                    });
                }
            }
            Mode::MoveComment {
                index,
                origin,
                carried,
                ..
            } => {
                let now = cx.scene.comments[index].rect;
                let mut cmds = Vec::new();
                if now != origin {
                    cmds.push(GraphCommand::MoveComment {
                        index,
                        x: now.min.x as f64,
                        y: now.min.y as f64,
                    });
                }
                for (i, p) in &carried {
                    let np = cx.scene.nodes[*i].pos;
                    if np != *p {
                        cmds.push(GraphCommand::MoveNode {
                            id: cx.scene.nodes[*i].id.clone(),
                            x: np.x as f64,
                            y: np.y as f64,
                        });
                    }
                }
                if !cmds.is_empty() {
                    out.actions.push(Action::Commands {
                        label: "Move comment",
                        commands: cmds,
                    });
                }
            }
            Mode::ResizeComment { index, origin, .. } => {
                let now = cx.scene.comments[index].rect;
                if now.size() != origin.size() {
                    out.actions.push(Action::Commands {
                        label: "Resize comment",
                        commands: vec![GraphCommand::ResizeComment {
                            index,
                            width: now.width() as f64,
                            height: now.height() as f64,
                        }],
                    });
                }
            }
            Mode::Connect {
                anchor,
                replacing,
                snapped,
            } => {
                self.finish_connect(anchor, replacing, snapped, pos, g, cx, out);
                self.compatible.clear();
            }
            Mode::ZipLasso { start } => {
                self.zip_lasso = None;
                if button == MouseButton::Middle && start.distance(g) > 1.0 {
                    let tie_center = (start + g) * 0.5;
                    let mut wires: Vec<(f32, graphy::ZipTieEdge)> = cx
                        .scene
                        .wires
                        .iter()
                        .filter_map(|wire| {
                            let crossed = wire.shapes().any(|shape| {
                                shape.flatten(48).windows(2).any(|pair| {
                                    segments_distance(start, g, pair[0], pair[1])
                                        <= self.tolerance().tie_px / self.view.zoom
                                })
                            });
                            if !crossed {
                                return None;
                            }
                            let connection = cx.graph.connections.get(wire.connection)?;
                            let edge = graphy::ZipTieEdge::of(connection);
                            let a = cx.scene.nodes[wire.from_node]
                                .pin_pos(PinSide::Output, wire.from_pin)?;
                            let b = cx.scene.nodes[wire.to_node]
                                .pin_pos(PinSide::Input, wire.to_pin)?;
                            let midpoint = (a + b) * 0.5 - start;
                            let drag = g - start;
                            let along = midpoint.x * drag.x + midpoint.y * drag.y;
                            Some((along, edge))
                        })
                        .collect();
                    wires.sort_by(|a, b| a.0.total_cmp(&b.0));
                    wires.dedup_by(|a, b| a.1 == b.1);
                    if wires.len() >= 2 {
                        out.actions.push(Action::Commands {
                            label: "Zip tie wires",
                            commands: vec![GraphCommand::AddZipTie {
                                index: cx.graph.zip_ties.len(),
                                tie: graphy::GraphZipTie {
                                    position: Position {
                                            x: tie_center.x as f64,
                                            y: tie_center.y as f64,
                                    },
                                    straight_length: graphy::DEFAULT_ZIP_TIE_STRAIGHT_LENGTH,
                                    wires: wires.into_iter().map(|(_, edge)| edge).collect(),
                                },
                            }],
                        });
                    }
                }
            }
            Mode::MoveZipTie { index, origin, .. } => {
                if button == MouseButton::Middle {
                    let now = cx.scene.ties[index].position;
                    if now != origin {
                        out.actions.push(Action::Commands {
                            label: "Move zip tie",
                            commands: vec![GraphCommand::MoveZipTie {
                                index,
                                x: now.x as f64,
                                y: now.y as f64,
                            }],
                        });
                    }
                } else {
                    cx.scene.set_zip_tie_position(index, origin);
                }
            }
        }
    }

    fn finish_connect(
        &mut self,
        anchor: PinAddr,
        replacing: Option<(usize, Connection)>,
        snapped: Option<PinAddr>,
        pos: Vec2,
        g: Vec2,
        cx: &mut GraphCtx,
        out: &mut Outcome,
    ) {
        let want = if anchor.side == PinSide::Output {
            PinSide::Input
        } else {
            PinSide::Output
        };
        // Dropping on a node body connects to its first compatible pin.
        let target =
            snapped.or_else(
                || match cx.scene.hit_test(g, self.view.zoom, self.tolerance()) {
                    Hit::Node { node, .. } if node != anchor.node => {
                        (0..cx.scene.nodes[node].pins(want).len())
                            .map(|pin| PinAddr {
                                node,
                                side: want,
                                pin,
                            })
                            .find(|a| self.compatible.contains(a))
                    }
                    _ => None,
                },
            );
        let Some(anchor_ref) = Self::pin_ref(cx, anchor) else {
            return;
        };
        match target {
            Some(t) => {
                let Some(other) = Self::pin_ref(cx, t) else {
                    return;
                };
                let (from, to) = if anchor.side == PinSide::Output {
                    (anchor_ref, other)
                } else {
                    (other, anchor_ref)
                };
                if let Err(why) = cx.profile.can_connect(cx.graph, &from, &to) {
                    out.actions.push(Action::ConnectionRefused(why));
                    return;
                }
                let exec = cx
                    .graph
                    .nodes
                    .get(&from.node)
                    .and_then(|n| n.outputs.iter().find(|p| p.id == from.pin))
                    .is_some_and(|p| p.data_type().is_execution());
                let kind = if exec {
                    ConnectionType::Execution
                } else {
                    ConnectionType::Data
                };
                let new = Connection::new(&from.node, &from.pin, &to.node, &to.pin, kind);
                let mut cmds: Vec<GraphCommand> = Vec::new();
                if let Some((_, old)) = &replacing {
                    cmds.push(GraphCommand::Disconnect(old.clone()));
                }
                for c in &cx.graph.connections {
                    let occupied_input = c.target_node == to.node && c.target_pin == to.pin;
                    let occupied_exec =
                        exec && c.source_node == from.node && c.source_pin == from.pin;
                    let already = replacing.as_ref().is_some_and(|(_, r)| same_edge(r, c));
                    if (occupied_input || occupied_exec) && !already {
                        cmds.push(GraphCommand::Disconnect(c.clone()));
                    }
                }
                // re-dropping onto the pin it came from changes nothing
                if replacing.as_ref().is_some_and(|(_, r)| same_edge(r, &new)) {
                    return;
                }
                cmds.push(GraphCommand::Connect(new));
                out.actions.push(Action::Commands {
                    label: "Connect",
                    commands: cmds,
                });
            }
            None => {
                if let Some((_, old)) = replacing {
                    out.actions.push(Action::Commands {
                        label: "Disconnect",
                        commands: vec![GraphCommand::Disconnect(old)],
                    });
                }
                out.actions.push(Action::ConnectionDropped {
                    from: anchor_ref,
                    side: anchor.side,
                    at: pos,
                    graph_at: g,
                });
            }
        }
    }

    fn wheel(&mut self, pos: Vec2, delta: Vec2, pixels: bool, mods: Modifiers, out: &mut Outcome) {
        let zoom = if self.settings.wheel_zooms {
            !pixels || mods.ctrl
        } else {
            mods.ctrl
        };
        if zoom {
            let notches = if pixels { delta.y / 60.0 } else { delta.y };
            let factor = self.settings.zoom_step.powf(notches);
            if self.view.zoom_at(pos, factor) {
                out.actions.push(Action::ViewChanged);
                out.redraw = true;
            }
        } else {
            let d = if pixels { delta } else { delta * 40.0 };
            let d = if mods.shift && !pixels {
                Vec2::new(d.y, d.x)
            } else {
                d
            };
            self.view.pan_by_screen(d);
            out.actions.push(Action::ViewChanged);
            out.redraw = true;
        }
    }

    // ---------------------------------------------------------------- keys

    fn key(&mut self, key: Key, mods: Modifiers, cx: &mut GraphCtx, out: &mut Outcome) {
        out.redraw = true;
        match (key, mods.ctrl) {
            (Key::Escape, _) => self.cancel(cx, out),
            (Key::Delete | Key::Backspace, _) => {
                let cmds = self.delete_commands(cx);
                if !cmds.is_empty() {
                    self.selection.clear();
                    out.actions.push(Action::Commands {
                        label: "Delete",
                        commands: cmds,
                    });
                    out.actions.push(Action::SelectionChanged);
                }
            }
            (Key::Char('a'), true) => {
                self.select_all(cx.scene);
                out.actions.push(Action::SelectionChanged);
            }
            (Key::Char('c'), true) => self.copy(cx, out),
            (Key::Char('x'), true) => {
                self.copy(cx, out);
                let cmds = self.delete_commands(cx);
                if !cmds.is_empty() {
                    self.selection.clear();
                    out.actions.push(Action::Commands {
                        label: "Cut",
                        commands: cmds,
                    });
                    out.actions.push(Action::SelectionChanged);
                }
            }
            (Key::Char('v'), true) => out.actions.push(Action::ClipboardReadRequest),
            (Key::Char('d'), true) => {
                if let Some(text) = clipboard::copy(cx.graph, &self.selection.nodes) {
                    let at = self.selection_origin(cx)
                        + Vec2::new(self.settings.paste_offset, self.settings.paste_offset);
                    self.paste_at(&text, at, "Duplicate", cx, out);
                }
            }
            (Key::Char('f'), false) => {
                if self.frame_selection(cx.scene) {
                    out.actions.push(Action::ViewChanged);
                }
            }
            (Key::Char('q'), false) => {
                let cmds = self.straighten_commands(cx);
                if !cmds.is_empty() {
                    out.actions.push(Action::Commands {
                        label: "Straighten",
                        commands: cmds,
                    });
                }
            }
            (Key::Home, _) => {
                if self.frame_all(cx.scene) {
                    out.actions.push(Action::ViewChanged);
                }
            }
            (Key::Left | Key::Right | Key::Up | Key::Down, false) => {
                let step = cx.scene.metrics.grid_snap * if mods.shift { 5.0 } else { 1.0 };
                let d = match key {
                    Key::Left => Vec2::new(-step, 0.0),
                    Key::Right => Vec2::new(step, 0.0),
                    Key::Up => Vec2::new(0.0, -step),
                    _ => Vec2::new(0.0, step),
                };
                let cmds: Vec<GraphCommand> = self
                    .selection
                    .nodes
                    .iter()
                    .filter_map(|id| cx.scene.node(id))
                    .map(|n| GraphCommand::MoveNode {
                        id: n.id.clone(),
                        x: (n.pos.x + d.x) as f64,
                        y: (n.pos.y + d.y) as f64,
                    })
                    .collect();
                if !cmds.is_empty() {
                    out.actions.push(Action::Commands {
                        label: "Nudge",
                        commands: cmds,
                    });
                }
            }
            _ => out.redraw = false,
        }
    }

    fn cancel(&mut self, cx: &mut GraphCtx, out: &mut Outcome) {
        match std::mem::replace(&mut self.mode, Mode::Idle) {
            Mode::MoveNodes {
                origin, comments, ..
            } => {
                for (i, p) in origin {
                    cx.scene.set_node_position(i, p);
                }
                for (i, r) in comments {
                    cx.scene.set_comment_rect(i, r);
                }
            }
            Mode::MoveComment {
                index,
                origin,
                carried,
                ..
            } => {
                cx.scene.set_comment_rect(index, origin);
                for (i, p) in carried {
                    cx.scene.set_node_position(i, p);
                }
            }
            Mode::ResizeComment { index, origin, .. } => cx.scene.set_comment_rect(index, origin),
            Mode::MoveZipTie { index, origin, .. } => cx.scene.set_zip_tie_position(index, origin),
            Mode::ZipLasso { .. } => self.zip_lasso = None,
            Mode::Idle => {
                if !self.selection.is_empty() {
                    self.selection.clear();
                    self.focused = None;
                    out.actions.push(Action::SelectionChanged);
                }
            }
            _ => {}
        }
        self.marquee = None;
        self.guides.clear();
        self.compatible.clear();
    }

    fn delete_commands(&self, cx: &GraphCtx) -> Vec<GraphCommand> {
        let mut cmds: Vec<GraphCommand> = Vec::new();
        // wires first (by edge, so indices shifting does not matter), skipping
        // those a node removal would take anyway
        for &ci in &self.selection.wires {
            if let Some(c) = cx.graph.connections.get(ci) {
                if !self.selection.nodes.contains(&c.source_node)
                    && !self.selection.nodes.contains(&c.target_node)
                {
                    cmds.push(GraphCommand::Disconnect(c.clone()));
                }
            }
        }
        cmds.extend(
            self.selection
                .nodes
                .iter()
                .filter(|id| {
                    cx.graph
                        .nodes
                        .get(*id)
                        .map_or(true, |n| cx.profile.is_deletable(n))
                })
                .map(|id| GraphCommand::RemoveNode { id: id.clone() }),
        );
        // highest index first so earlier removals do not shift later ones
        cmds.extend(
            self.selection
                .comments
                .iter()
                .rev()
                .filter(|i| cx.scene.comments.get(**i).is_some_and(|c| c.editable))
                .map(|&index| GraphCommand::RemoveComment { index }),
        );
        // Remove high indices first so each remaining selected index stays valid.
        cmds.extend(
            self.selection
                .ties
                .iter()
                .rev()
                .map(|&index| GraphCommand::RemoveZipTie { index }),
        );
        cmds
    }

    /// Q: make wires touching selected nodes as level as possible with the
    /// smallest vertical movement. One selected node per connected group stays
    /// fixed, so a two-node straighten moves only one node and cannot translate
    /// the pair to an arbitrary height.
    fn straighten_commands(&self, cx: &GraphCtx) -> Vec<GraphCommand> {
        let selected: Vec<usize> = self
            .selection
            .nodes
            .iter()
            .filter_map(|id| cx.scene.node_index(id))
            .collect();
        if selected.is_empty() {
            return Vec::new();
        }
        let selected_set: HashSet<usize> = selected.iter().copied().collect();
        let mut selected_edges = vec![Vec::new(); cx.scene.nodes.len()];
        for wire in &cx.scene.wires {
            if selected_set.contains(&wire.from_node) && selected_set.contains(&wire.to_node) {
                selected_edges[wire.from_node].push(wire.to_node);
                selected_edges[wire.to_node].push(wire.from_node);
            }
        }
        let mut anchors = HashSet::new();
        let mut visited = HashSet::new();
        for &start in &selected {
            if !visited.insert(start) {
                continue;
            }
            let mut component = vec![start];
            let mut pending = vec![start];
            while let Some(node) = pending.pop() {
                for &other in &selected_edges[node] {
                    if visited.insert(other) {
                        component.push(other);
                        pending.push(other);
                    }
                }
            }
            if component.len() > 1 {
                let anchor = self
                    .focused
                    .as_ref()
                    .and_then(|id| cx.scene.node_index(id))
                    .filter(|node| component.contains(node))
                    .unwrap_or(component[0]);
                anchors.insert(anchor);
            }
        }
        let movers: Vec<usize> = selected
            .into_iter()
            .filter(|node| !anchors.contains(node))
            .collect();
        let variables: HashMap<usize, usize> = movers
            .iter()
            .enumerate()
            .map(|(i, &node)| (node, i))
            .collect();
        let n = movers.len();
        if n == 0 {
            return Vec::new();
        }
        let mut normal = vec![vec![0.0f64; n + 1]; n];
        for w in &cx.scene.wires {
            let (Some(from), Some(to)) = (
                cx.scene.nodes[w.from_node].pin_pos(PinSide::Output, w.from_pin),
                cx.scene.nodes[w.to_node].pin_pos(PinSide::Input, w.to_pin),
            ) else {
                continue;
            };
            let mut terms = Vec::with_capacity(2);
            if let Some(&i) = variables.get(&w.from_node) {
                terms.push((i, -1.0f64));
            }
            if let Some(&i) = variables.get(&w.to_node) {
                terms.push((i, 1.0f64));
            }
            if terms.is_empty() {
                continue;
            }
            let delta = (to.y - from.y) as f64;
            for &(i, ai) in &terms {
                normal[i][n] -= ai * delta;
                for &(j, aj) in &terms {
                    normal[i][j] += ai * aj;
                }
            }
        }
        for i in 0..n {
            normal[i][i] += 1e-6;
        }
        // Solve the regularized normal equations with pivoted Gauss-Jordan.
        for col in 0..n {
            let pivot = (col..n)
                .max_by(|&a, &b| normal[a][col].abs().total_cmp(&normal[b][col].abs()))
                .unwrap_or(col);
            if normal[pivot][col].abs() < f64::EPSILON {
                continue;
            }
            normal.swap(col, pivot);
            let scale = normal[col][col];
            for j in col..=n {
                normal[col][j] /= scale;
            }
            for row in 0..n {
                if row == col {
                    continue;
                }
                let factor = normal[row][col];
                for j in col..=n {
                    normal[row][j] -= factor * normal[col][j];
                }
            }
        }
        movers
            .into_iter()
            .enumerate()
            .filter_map(|(i, node_i)| {
                let dy = normal[i][n];
                if dy.abs() < 0.5 {
                    return None;
                }
                let node = &cx.scene.nodes[node_i];
                Some(GraphCommand::MoveNode {
                    id: node.id.clone(),
                    x: node.pos.x as f64,
                    y: node.pos.y as f64 + dy,
                })
            })
            .collect()
    }

    fn selection_origin(&self, cx: &GraphCtx) -> Vec2 {
        self.selection
            .nodes
            .iter()
            .filter_map(|id| cx.scene.node(id))
            .map(|n| n.pos)
            .fold(None, |acc: Option<Vec2>, p| {
                Some(acc.map_or(p, |a| Vec2::new(a.x.min(p.x), a.y.min(p.y))))
            })
            .unwrap_or(self.cursor_graph)
    }

    fn copy(&mut self, cx: &GraphCtx, out: &mut Outcome) {
        if let Some(text) = clipboard::copy(cx.graph, &self.selection.nodes) {
            self.clipboard = Some(text.clone());
            out.actions.push(Action::ClipboardWrite(text));
        }
    }

    fn paste(&mut self, text: &str, cx: &mut GraphCtx, out: &mut Outcome) {
        let at = self.cursor_graph;
        self.paste_at(text, at, "Paste", cx, out);
    }

    fn paste_at(
        &mut self,
        text: &str,
        at: Vec2,
        label: &'static str,
        cx: &mut GraphCtx,
        out: &mut Outcome,
    ) {
        let at = if self.settings.snap_to_grid {
            let gs = cx.scene.metrics.grid_snap;
            Vec2::new((at.x / gs).round() * gs, (at.y / gs).round() * gs)
        } else {
            at
        };
        match clipboard::paste(text, cx.graph, at, &self.settings.id_prefix) {
            Ok(p) => {
                self.selection.clear();
                self.selection.nodes.extend(p.new_ids.iter().cloned());
                self.focused = p.new_ids.last().cloned();
                out.actions.push(Action::Commands {
                    label,
                    commands: p.commands,
                });
                out.actions.push(Action::SelectionChanged);
            }
            Err(e) => out.actions.push(Action::ConnectionRefused(e)),
        }
    }

    // --------------------------------------------------------------- menus

    fn menu_items(&self, target: &MenuTarget, cx: &GraphCtx) -> Vec<MenuItem> {
        let mut items = Vec::new();
        match target {
            MenuTarget::Canvas { .. } => {
                items.push(MenuItem::new("graphy.paste", "Paste"));
                items.push(MenuItem::new("graphy.select_all", "Select all"));
                items.push(MenuItem::new("graphy.frame_all", "Frame all"));
            }
            MenuTarget::Node(_) | MenuTarget::Selection => {
                items.push(MenuItem::new("graphy.copy", "Copy"));
                items.push(MenuItem::new("graphy.cut", "Cut"));
                items.push(MenuItem::new("graphy.duplicate", "Duplicate"));
                items.push(MenuItem::new("graphy.delete", "Delete").separated());
                items.push(MenuItem::new("graphy.frame_selection", "Frame selection"));
            }
            MenuTarget::Pin(_) => items.push(MenuItem::new("graphy.break_links", "Break link(s)")),
            MenuTarget::Wire(_) => {
                items.push(MenuItem::new("graphy.delete_wire", "Delete connection"));
                items.push(MenuItem::new("graphy.add_reroute", "Add reroute"));
            }
            MenuTarget::Comment(_) => {}
        }
        items.extend(cx.profile.menu_items(cx.graph, target));
        items
    }

    /// Runs a menu item chosen from an [`Action::ContextMenu`]. `graphy.*` ids
    /// are handled here; the rest go to the profile.
    pub fn run_menu_item(
        &mut self,
        target: &MenuTarget,
        id: &str,
        at: Vec2,
        cx: &mut GraphCtx,
    ) -> Outcome {
        let mut out = Outcome {
            redraw: true,
            ..Default::default()
        };
        match id {
            "graphy.copy" => self.copy(cx, &mut out),
            "graphy.cut" => {
                self.copy(cx, &mut out);
                let cmds = self.delete_commands(cx);
                self.selection.clear();
                out.actions.push(Action::Commands {
                    label: "Cut",
                    commands: cmds,
                });
            }
            "graphy.duplicate" => self.key(
                Key::Char('d'),
                Modifiers {
                    ctrl: true,
                    ..Default::default()
                },
                cx,
                &mut out,
            ),
            "graphy.delete" => {
                if let MenuTarget::Node(n) = target {
                    self.selection.nodes.insert(n.clone());
                }
                let cmds = self.delete_commands(cx);
                self.selection.clear();
                out.actions.push(Action::Commands {
                    label: "Delete",
                    commands: cmds,
                });
            }
            "graphy.paste" => {
                self.cursor_graph = self.graph_pos(at);
                out.actions.push(Action::ClipboardReadRequest);
            }
            "graphy.select_all" => {
                self.select_all(cx.scene);
                out.actions.push(Action::SelectionChanged);
            }
            "graphy.frame_all" => {
                self.frame_all(cx.scene);
                out.actions.push(Action::ViewChanged);
            }
            "graphy.frame_selection" => {
                self.frame_selection(cx.scene);
                out.actions.push(Action::ViewChanged);
            }
            "graphy.break_links" => {
                if let MenuTarget::Pin(p) = target {
                    let cmds: Vec<GraphCommand> = cx
                        .graph
                        .connections
                        .iter()
                        .filter(|c| {
                            (c.target_node == p.node && c.target_pin == p.pin)
                                || (c.source_node == p.node && c.source_pin == p.pin)
                        })
                        .cloned()
                        .map(GraphCommand::Disconnect)
                        .collect();
                    out.actions.push(Action::Commands {
                        label: "Break pin links",
                        commands: cmds,
                    });
                }
            }
            "graphy.delete_wire" => {
                if let MenuTarget::Wire(c) = target {
                    out.actions.push(Action::Commands {
                        label: "Delete connection",
                        commands: vec![GraphCommand::Disconnect(c.clone())],
                    });
                }
            }
            "graphy.add_reroute" => {
                if let MenuTarget::Wire(c) = target {
                    let ci = cx.graph.connections.iter().position(|e| same_edge(e, c));
                    if let Some(cmds) =
                        ci.and_then(|ci| self.reroute_commands(ci, self.graph_pos(at), cx))
                    {
                        out.actions.push(Action::Commands {
                            label: "Insert reroute",
                            commands: cmds,
                        });
                    }
                }
            }
            other => out
                .actions
                .extend(cx.profile.run_menu_item(cx.graph, target, other)),
        }
        out
    }

    /// Commands that refresh dynamic pins of `nodes` after their properties
    /// changed (profile hook), as one transaction.
    pub fn refresh_pins(&self, nodes: &[String], cx: &GraphCtx) -> Option<Action> {
        let commands: Vec<GraphCommand> = nodes
            .iter()
            .flat_map(|n| dynamic_pin_commands(cx.profile, cx.graph, n))
            .collect();
        (!commands.is_empty()).then_some(Action::Commands {
            label: "Update pins",
            commands,
        })
    }
}

fn same_edge(a: &Connection, b: &Connection) -> bool {
    a.source_node == b.source_node
        && a.source_pin == b.source_pin
        && a.target_node == b.target_node
        && a.target_pin == b.target_pin
}

/// Applies `commands` to `graph` as one transaction: if any fails, the ones
/// already applied are undone and the error is returned, leaving the graph
/// unchanged. On success the combined change set (for undo and
/// `Scene::apply_changes`) is returned.
fn segments_distance(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> f32 {
    let cross = |u: Vec2, v: Vec2| u.x * v.y - u.y * v.x;
    let ab = b - a;
    let cd = d - c;
    let denom = cross(ab, cd);
    if denom.abs() > 1e-6 {
        let ac = c - a;
        let t = cross(ac, cd) / denom;
        let u = cross(ac, ab) / denom;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            return 0.0;
        }
    }
    graphy_render::wire::segment_distance(a, c, d)
        .min(graphy_render::wire::segment_distance(b, c, d))
        .min(graphy_render::wire::segment_distance(c, a, b))
        .min(graphy_render::wire::segment_distance(d, a, b))
}

pub fn apply_transaction(
    graph: &mut GraphDescription,
    commands: &[GraphCommand],
) -> Result<graphy::ChangeSet, graphy::EditError> {
    let mut all = graphy::ChangeSet::default();
    for c in commands {
        match c.apply(graph) {
            Ok(cs) => all.changes.extend(cs.changes),
            Err(e) => {
                all.undo(graph);
                return Err(e);
            }
        }
    }
    Ok(all)
}
