//! The GPUI graph canvas (feature `gpui`).
//!
//! [`GraphCanvas`] is a GPUI entity that **owns the editing session's
//! `GraphDescription`** (the one mutable node/edge document), a
//! [`graphy_render::Scene`] derived from it, the interaction
//! [`Controller`] and the GPU renderer. It translates GPUI events into
//! host-neutral [`InputEvent`]s, applies the resulting command batches to the
//! document and emits [`CanvasEvent`]s so the editor can save, mark dirty,
//! undo/redo, validate or broadcast the change sets. Everything else in the
//! editor (panels, toolbar, properties, file I/O) stays with the host.
//!
//! Rendering goes through the window's retained WGPU surface: this crate
//! draws into the surface's device/queue (never a second device) and the
//! surface is composited by GPUI.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    anchored, deferred, div, px, wgpu_surface, AnyElement, App, AppContext, ClipboardItem, Context, CursorStyle, ElementGeometry,
    Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton as GpuiButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Point, Render, ScrollDelta, ScrollWheelEvent, SharedString, Styled,
    Subscription, WeakEntity, WgpuSurfaceHandle, Window,
};
use ui::ActiveTheme;
use graphy::{ChangeSet, EditError, GraphCommand, GraphDescription, PinRef};
use graphy_render::{DrawParams, LayoutMetrics, LodTier, PinSide, Scene, Theme, Vec2};
use graphy_render_wgpu::{FontMeasure, FontSet, Fonts, GraphRenderer};

use crate::controller::{apply_transaction, Action, Controller, GraphCtx, Outcome};
use crate::input::{CursorHint, InputEvent, Key, Modifiers, MouseButton};
use crate::picker::{DragChip, NodePicker, PickerEvent, PICKER_SIZE};
use crate::profile::{CatalogEntry, GraphProfile, MenuItem, MenuTarget};

/// Width of a context menu.
const MENU_WIDTH: f32 = 220.0;

/// A [`GraphProfile`] that can also contribute GPUI content. Implement it
/// (usually with an empty body) for any profile used with [`GraphCanvas`].
pub trait GpuiProfile: GraphProfile {
    /// Content for a host row (texture preview, custom body, inline editor)
    /// reported by the layout as `slot` of `node`. The element is positioned
    /// over the row's rectangle by the canvas. Return `None` to leave the
    /// placeholder well the renderer already draws.
    ///
    /// Called while the canvas entity is being rendered (inside its own
    /// `update`), so the implementation must not read the `GraphCanvas`
    /// entity: the current document is passed as `graph` instead.
    fn render_slot(
        &self,
        _graph: &GraphDescription,
        _node: &str,
        _slot: u32,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<AnyElement> {
        None
    }
}

/// Notifications for the editor.
#[derive(Debug)]
pub enum CanvasEvent {
    /// The document changed. Push `changes` on the undo stack, mark the
    /// document dirty, re-validate. `label` names the user-visible edit.
    Edited {
        label: &'static str,
        /// What was asked for, to replay on redo.
        commands: Vec<GraphCommand>,
        changes: ChangeSet,
    },
    /// A command batch failed and was rolled back.
    Rejected(String),
    SelectionChanged,
    ViewChanged,
    /// Menus, graph navigation, inline-value editing, dropped connections,
    /// slot presses: anything the host must present or decide.
    Request(Action),
    /// The user chose a node in the picker (opened by right-clicking empty
    /// canvas or dropping a wire there). Add it at `graph_pos`, wired from
    /// `source` when the picker was opened by a wire.
    NodePicked {
        definition: String,
        graph_pos: Vec2,
        source: Option<PinRef>,
    },
}

/// The popup over the canvas: a context menu or the node picker. Positions are
/// canvas-relative pixels, the space pointer events are delivered in.
enum Overlay {
    Menu {
        at: Vec2,
        target: MenuTarget,
        items: Vec<MenuItem>,
    },
    Picker {
        at: Vec2,
        graph_pos: Vec2,
        source: Option<PinRef>,
        /// Entries to build the picker from; taken once a window is at hand.
        init: Option<(Vec<CatalogEntry>, Option<DragChip>)>,
        view: Option<Entity<NodePicker>>,
        focus_pending: bool,
        subscription: Option<Subscription>,
    },
}

/// One context-menu row (children follow it, indented).
fn menu_entry(
    (item, depth): (MenuItem, usize),
    canvas: &Entity<GraphCanvas>,
    cx: &mut Context<GraphCanvas>,
) -> Vec<AnyElement> {
    let mut out: Vec<AnyElement> = Vec::new();
    if item.separator_before {
        out.push(
            div()
                .my(px(4.0))
                .mx(px(8.0))
                .h(px(1.0))
                .bg(cx.theme().border)
                .into_any_element(),
        );
    }
    let foreground = if item.enabled {
        cx.theme().popover_foreground
    } else {
        cx.theme().muted_foreground
    };
    let id = item.id.clone();
    let mut row = div()
        .pl(px(12.0 + depth as f32 * 12.0))
        .pr(px(12.0))
        .py(px(6.0))
        .text_sm()
        .text_color(foreground)
        .child(SharedString::from(item.label.clone()));
    if item.enabled {
        let canvas = canvas.clone();
        row = row
            .cursor_pointer()
            .hover(|s| s.bg(cx.theme().accent.opacity(0.12)))
            .on_mouse_down(GpuiButton::Left, move |_, _, cx| {
                canvas.update(cx, |c, cx| c.choose_menu_item(&id, cx));
            });
    }
    out.push(row.into_any_element());
    out
}

pub struct GraphCanvas {
    graph: GraphDescription,
    scene: Scene,
    controller: Controller,
    profile: Arc<dyn GpuiProfile>,
    focus: FocusHandle,
    fonts: Fonts,
    measure: FontMeasure,
    renderer: Option<GraphRenderer>,
    surface: Option<WgpuSurfaceHandle>,
    origin: Point<Pixels>,
    started: Instant,
    driver: Rc<Cell<u64>>,
    cursor: CursorHint,
    theme: Theme,
    overlay: Option<Overlay>,
}

impl EventEmitter<CanvasEvent> for GraphCanvas {}

impl Focusable for GraphCanvas {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn to_vec(p: Point<Pixels>, origin: Point<Pixels>) -> Vec2 {
    Vec2::new(
        f32::from(p.x) - f32::from(origin.x),
        f32::from(p.y) - f32::from(origin.y),
    )
}

fn mods(m: &gpui::Modifiers) -> Modifiers {
    Modifiers {
        shift: m.shift,
        ctrl: m.control || m.platform,
        alt: m.alt,
    }
}

fn button(b: GpuiButton) -> Option<MouseButton> {
    match b {
        GpuiButton::Left => Some(MouseButton::Left),
        GpuiButton::Middle => Some(MouseButton::Middle),
        GpuiButton::Right => Some(MouseButton::Right),
        _ => None,
    }
}

fn key(k: &str) -> Option<Key> {
    Some(match k {
        "delete" => Key::Delete,
        "backspace" => Key::Backspace,
        "escape" => Key::Escape,
        "enter" => Key::Enter,
        "home" => Key::Home,
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        s if s.chars().count() == 1 => Key::Char(s.chars().next()?.to_ascii_lowercase()),
        _ => return None,
    })
}

fn cursor_style(c: CursorHint) -> CursorStyle {
    match c {
        CursorHint::Default => CursorStyle::Arrow,
        CursorHint::Pointer => CursorStyle::PointingHand,
        CursorHint::Grab => CursorStyle::OpenHand,
        CursorHint::Grabbing => CursorStyle::ClosedHand,
        CursorHint::Move => CursorStyle::OpenHand,
        CursorHint::ResizeNwse => CursorStyle::ResizeUpLeftDownRight,
        CursorHint::Crosshair => CursorStyle::Crosshair,
    }
}

impl GraphCanvas {
    /// Opens `graph` with an explicit `profile` and fonts. The profile is
    /// owned by this canvas (an `Arc` the editor created); nothing is looked up
    /// through a global.
    pub fn new(
        graph: GraphDescription,
        profile: Arc<dyn GpuiProfile>,
        fonts: FontSet,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Result<Self, &'static str> {
        let fonts = Fonts::load(&fonts)?;
        let measure = fonts.measure();
        let mut scene = Scene::new(LayoutMetrics::default());
        scene.rebuild(&graph, profile.presenter(), &measure);
        let mut controller = Controller::new(Vec2::new(1280.0, 720.0));
        controller.frame_all(&scene);
        Ok(Self {
            graph,
            scene,
            controller,
            profile,
            focus: cx.focus_handle(),
            fonts,
            measure,
            renderer: None,
            surface: None,
            origin: Point::default(),
            started: Instant::now(),
            driver: Rc::new(Cell::new(0)),
            cursor: CursorHint::Default,
            theme,
            overlay: None,
        })
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Switches the canvas (and an open popup) to `theme`. Cheap when unchanged.
    pub fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        cx.notify();
    }

    pub fn graph(&self) -> &GraphDescription {
        &self.graph
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn controller(&self) -> &Controller {
        &self.controller
    }

    pub fn controller_mut(&mut self) -> &mut Controller {
        &mut self.controller
    }

    /// Window position -> graph position (valid after the first frame).
    pub fn window_to_graph(&self, p: Point<Pixels>) -> Vec2 {
        self.controller.view.screen_to_graph(to_vec(p, self.origin))
    }

    /// Graph position -> window position.
    pub fn graph_to_window(&self, g: Vec2) -> Point<Pixels> {
        let s = self.controller.view.graph_to_screen(g);
        Point::new(px(s.x + f32::from(self.origin.x)), px(s.y + f32::from(self.origin.y)))
    }

    /// Canvas-relative pixels (as reported by menu requests) -> window position.
    pub fn local_to_window(&self, p: Vec2) -> Point<Pixels> {
        Point::new(px(p.x + f32::from(self.origin.x)), px(p.y + f32::from(self.origin.y)))
    }

    /// Size of the canvas in logical pixels, as of the last frame.
    pub fn viewport(&self) -> Vec2 {
        self.controller.viewport
    }

    /// Pan (graph units) and zoom of the view; `screen = (graph + pan) * zoom`.
    pub fn view(&self) -> (Vec2, f32) {
        (self.controller.view.pan, self.controller.view.zoom)
    }

    /// Restores a saved view.
    pub fn set_view(&mut self, pan: Vec2, zoom: f32, cx: &mut Context<Self>) {
        self.controller.view.pan = pan;
        self.controller.view.zoom = zoom.clamp(graphy_render::MIN_ZOOM, graphy_render::MAX_ZOOM);
        cx.emit(CanvasEvent::ViewChanged);
        cx.notify();
    }

    /// Frames the whole document, for a document that carries no saved view.
    pub fn frame_all(&mut self, cx: &mut Context<Self>) {
        self.controller.frame_all(&self.scene);
        cx.emit(CanvasEvent::ViewChanged);
        cx.notify();
    }

    /// Centres the view on a node. Returns whether the node exists.
    pub fn center_on(&mut self, node: &str, cx: &mut Context<Self>) -> bool {
        let Some(n) = self.scene.node(node) else {
            return false;
        };
        let center = n.rect().center();
        let zoom = self.controller.view.zoom;
        self.controller.view.pan = Vec2::new(
            self.controller.viewport.x * 0.5 / zoom - center.x,
            self.controller.viewport.y * 0.5 / zoom - center.y,
        );
        cx.emit(CanvasEvent::ViewChanged);
        cx.notify();
        true
    }

    /// Replaces the selection with these nodes.
    pub fn select_nodes<I: IntoIterator<Item = String>>(&mut self, ids: I, cx: &mut Context<Self>) {
        self.controller.selection.clear();
        self.controller.selection.nodes.extend(ids);
        cx.emit(CanvasEvent::SelectionChanged);
        cx.notify();
    }

    /// Re-presents every node. Call after the profile's own data changed
    /// (subgraph names, the running-node set).
    pub fn refresh_scene(&mut self, cx: &mut Context<Self>) {
        self.scene
            .rebuild(&self.graph, self.profile.presenter(), &self.measure);
        cx.notify();
    }

    /// Replaces the document (load, revert). Selection is cleared.
    pub fn set_graph(&mut self, graph: GraphDescription, cx: &mut Context<Self>) {
        self.graph = graph;
        self.controller.selection.clear();
        self.scene
            .rebuild(&self.graph, self.profile.presenter(), &self.measure);
        cx.notify();
    }

    /// Applies commands as one transaction (menus, palettes, scripts). The
    /// editor's undo/save path hears about it through [`CanvasEvent::Edited`].
    pub fn apply(
        &mut self,
        label: &'static str,
        commands: &[GraphCommand],
        cx: &mut Context<Self>,
    ) -> Result<(), EditError> {
        match apply_transaction(&mut self.graph, commands) {
            Ok(changes) => {
                self.overlay = None;
                self.scene.apply_changes(
                    &self.graph,
                    &changes,
                    self.profile.presenter(),
                    &self.measure,
                );
                cx.emit(CanvasEvent::Edited {
                    label,
                    commands: commands.to_vec(),
                    changes,
                });
                cx.notify();
                Ok(())
            }
            Err(e) => {
                cx.emit(CanvasEvent::Rejected(e.to_string()));
                Err(e)
            }
        }
    }

    /// Reverts a change set the editor popped from its undo stack. Does not
    /// emit `Edited`; the editor already knows.
    pub fn undo_changes(&mut self, changes: &ChangeSet, cx: &mut Context<Self>) {
        changes.undo(&mut self.graph);
        self.scene
            .rebuild(&self.graph, self.profile.presenter(), &self.measure);
        cx.notify();
    }

    /// Re-applies commands that were undone (the editor keeps the commands).
    pub fn redo_commands(
        &mut self,
        commands: &[GraphCommand],
        cx: &mut Context<Self>,
    ) -> Result<(), EditError> {
        let changes = apply_transaction(&mut self.graph, commands)?;
        self.scene.apply_changes(
            &self.graph,
            &changes,
            self.profile.presenter(),
            &self.measure,
        );
        cx.notify();
        Ok(())
    }

    fn dispatch(&mut self, ev: InputEvent, cx: &mut Context<Self>) {
        let out = {
            let mut gx = GraphCtx {
                graph: &self.graph,
                scene: &mut self.scene,
                profile: &*self.profile,
                measure: &self.measure,
            };
            self.controller.handle(ev, &mut gx)
        };
        self.process(out, cx);
    }

    fn process(&mut self, out: Outcome, cx: &mut Context<Self>) {
        let mut followups = Vec::new();
        for action in out.actions {
            match action {
                Action::Commands { label, commands } => {
                    let _ = self.apply(label, &commands, cx);
                }
                Action::ClipboardWrite(text) => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text))
                }
                Action::ClipboardReadRequest => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
                        followups.push(InputEvent::Paste(text));
                    }
                }
                Action::SelectionChanged => cx.emit(CanvasEvent::SelectionChanged),
                Action::ViewChanged => cx.emit(CanvasEvent::ViewChanged),
                Action::ContextMenu { at, target, items } => match target {
                    MenuTarget::Canvas { graph_pos } => self.open_picker(at, graph_pos, None, cx),
                    target => self.overlay = Some(Overlay::Menu { at, target, items }),
                },
                Action::ConnectionDropped {
                    from,
                    side: PinSide::Output,
                    at,
                    graph_at,
                } => self.open_picker(at, graph_at, Some(from), cx),
                Action::ConnectionDropped { .. } => {}
                other => cx.emit(CanvasEvent::Request(other)),
            }
        }
        for ev in followups {
            self.dispatch(ev, cx);
        }
        let cursor = self.controller.cursor();
        if out.redraw || cursor != self.cursor {
            self.cursor = cursor;
            cx.notify();
        }
    }

    /// Runs a context-menu item the host's menu reported (see
    /// [`CanvasEvent::Request`] with [`Action::ContextMenu`]).
    pub fn run_menu_item(
        &mut self,
        target: &crate::MenuTarget,
        id: &str,
        at: Vec2,
        cx: &mut Context<Self>,
    ) {
        let out = {
            let mut gx = GraphCtx {
                graph: &self.graph,
                scene: &mut self.scene,
                profile: &*self.profile,
                measure: &self.measure,
            };
            self.controller.run_menu_item(target, id, at, &mut gx)
        };
        self.process(out, cx);
    }

    // -------------------------------------------------------------- popups

    /// Closes the open context menu or node picker, if any.
    pub fn close_popup(&mut self, cx: &mut Context<Self>) {
        if self.overlay.take().is_some() {
            cx.notify();
        }
    }

    /// Opens the node picker at `at` (canvas-relative pixels). A node chosen in
    /// it is reported as [`CanvasEvent::NodePicked`] for `graph_pos`. With
    /// `source`, only the entries that can take a wire from that output are
    /// listed.
    pub fn open_picker(
        &mut self,
        at: Vec2,
        graph_pos: Vec2,
        source: Option<PinRef>,
        cx: &mut Context<Self>,
    ) {
        let mut entries = self.profile.catalog();
        let mut chip = None;
        if let Some(from) = &source {
            let from_type = self
                .graph
                .nodes
                .get(&from.node)
                .and_then(|n| n.outputs.iter().find(|p| p.id == from.pin))
                .map(|p| p.data_type().clone());
            if let Some(from_type) = from_type {
                entries = crate::catalog::filter_for_drag(&*self.profile, entries, &from_type);
                chip = Some(DragChip {
                    label: match from_type.type_info() {
                        Some(info) => format!("Takes {}", info.type_string),
                        None => "Takes exec".to_string(),
                    },
                    color: None,
                });
            }
        }
        self.overlay = Some(Overlay::Picker {
            at,
            graph_pos,
            source,
            init: Some((entries, chip)),
            view: None,
            focus_pending: true,
            subscription: None,
        });
        cx.notify();
    }

    /// Runs a menu item of the open context menu, then closes it.
    fn choose_menu_item(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(Overlay::Menu { at, target, .. }) = self.overlay.take() else {
            return;
        };
        self.run_menu_item(&target, id, at, cx);
        cx.notify();
    }

    /// Builds the picker entity once a window is at hand (opening it from input
    /// handling has none).
    fn ensure_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::Picker { init, .. }) = &mut self.overlay else {
            return;
        };
        let Some((entries, chip)) = init.take() else {
            return;
        };
        let picker = cx.new(|cx| NodePicker::new(entries, chip, window, cx));
        let subscription = cx.subscribe(&picker, |this, _, event: &PickerEvent, cx| {
            let Some(Overlay::Picker {
                graph_pos, source, ..
            }) = this.overlay.take()
            else {
                return;
            };
            if let PickerEvent::Chosen(definition) = event {
                cx.emit(CanvasEvent::NodePicked {
                    definition: definition.clone(),
                    graph_pos,
                    source,
                });
            }
            cx.notify();
        });
        if let Some(Overlay::Picker {
            view: slot,
            subscription: sub,
            ..
        }) = &mut self.overlay
        {
            *slot = Some(picker);
            *sub = Some(subscription);
        }
    }

    /// The popup element, anchored at the pointer and kept inside the window.
    ///
    /// `at` is canvas-relative (the space pointer events arrive in), so it is
    /// mapped to window coordinates through the canvas's own origin: right-click
    /// and wire-drop both land under the pointer whatever the pan, zoom, DPI or
    /// the canvas's offset in the window.
    fn popup(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let border = cx.theme().border;
        let shell = |at: Vec2, priority: usize, body: gpui::Div| {
            let position = self.local_to_window(at);
            deferred(
                anchored()
                    .position(position)
                    .snap_to_window_with_margin(px(8.0))
                    .anchor(gpui::Corner::TopLeft)
                    .child(
                        body.occlude()
                            .shadow_lg()
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(border)
                            // Pointer input over a popup belongs to the popup, not the
                            // canvas underneath.
                            .on_mouse_down(GpuiButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_mouse_down(GpuiButton::Right, |_, _, cx| cx.stop_propagation())
                            .on_mouse_down(GpuiButton::Middle, |_, _, cx| cx.stop_propagation())
                            .on_mouse_up(GpuiButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_mouse_up(GpuiButton::Right, |_, _, cx| cx.stop_propagation())
                            .on_scroll_wheel(|_, _, cx| cx.stop_propagation()),
                    ),
            )
            .with_priority(priority)
            .into_any_element()
        };
        match self.overlay.as_ref()? {
            Overlay::Menu { at, items, .. } => {
                let entity = cx.entity();
                let mut list = div().w(px(MENU_WIDTH)).bg(cx.theme().popover).py(px(4.0));
                for item in flatten_menu(items) {
                    list = list.children(menu_entry(item, &entity, cx));
                }
                Some(shell(*at, 2, list))
            }
            Overlay::Picker { at, view, .. } => {
                let view = view.clone()?;
                let entity = cx.entity();
                let body = div()
                    .w(px(PICKER_SIZE.0))
                    .h(px(PICKER_SIZE.1))
                    .overflow_hidden()
                    .child(view)
                    .on_children_prepainted(move |_, window, cx| {
                        entity.update(cx, |c, cx| {
                            if let Some(Overlay::Picker {
                                view: Some(view),
                                focus_pending,
                                ..
                            }) = &mut c.overlay
                            {
                                if std::mem::take(focus_pending) {
                                    let handle = view.read(cx).search_focus_handle(cx);
                                    window.focus(&handle, cx);
                                }
                            }
                        });
                    });
                Some(shell(*at, 1, body))
            }
        }
    }

    // ------------------------------------------------------------- drawing

    /// Draws one frame into the retained surface. Returns whether another
    /// frame is needed without a state change (animation).
    fn draw_frame(
        &mut self,
        geometry: ElementGeometry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let bounds = geometry.bounds;
        self.origin = bounds.origin;
        let (lw, lh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let (pw, ph) = (lw.max(64.0) as u32, lh.max(64.0) as u32);
        self.controller.viewport = Vec2::new(lw, lh);

        // A lost device takes the surface and everything built on it with it.
        if self
            .surface
            .as_ref()
            .is_some_and(WgpuSurfaceHandle::is_device_lost)
        {
            self.surface = None;
            self.renderer = None;
        }
        if self.surface.is_none() {
            if let Some(s) = window
                .create_wgpu_surface(pw, ph, wgpu::TextureFormat::Bgra8UnormSrgb)
                .filter(|s| !s.is_device_lost())
            {
                self.surface = Some(s);
                cx.notify();
            }
        }
        let Some(surface) = self.surface.clone() else {
            return false;
        };
        if surface.is_resize_pending() {
            return true;
        }
        let Some((view, (w, h))) = surface.back_view_with_size() else {
            return false;
        };
        if self.renderer.is_none() {
            self.renderer = Some(GraphRenderer::with_fonts(
                surface.device(),
                surface.format(),
                self.fonts.clone(),
            ));
        }

        let mut params = DrawParams::new(Vec2::new(lw, lh), self.controller.view);
        params.theme = self.theme.clone();
        params.time = self.started.elapsed().as_secs_f32();
        let state = self.controller.interaction_state(&self.scene);
        let mut data = self.scene.build_draw_data(&params, &state, &self.measure);
        self.profile.decorate(&self.graph, &self.scene, &mut data);
        let animating = data.stats.nodes > 0
            && self.scene.nodes.iter().any(|n| n.presentation.active)
            || self.controller.is_dragging() && state.connecting.is_some();
        if let Some(r) = self.renderer.as_mut() {
            r.render_to_view(
                surface.device(),
                surface.queue(),
                &view,
                (w, h),
                &data,
                geometry.scale_factor,
            );
        }
        drop(view);
        surface.swap_buffers();
        animating
    }
}

type FrameFn =
    Box<dyn Fn(&mut GraphCanvas, ElementGeometry, &mut Window, &mut Context<GraphCanvas>) -> bool>;

/// Invisible element that renders the surface when GPUI hands it geometry and
/// re-ticks while an animation needs frames, without rebuilding views.
struct SurfaceDriver {
    generation: u64,
    current: Rc<Cell<u64>>,
    entity: WeakEntity<GraphCanvas>,
    interval: Duration,
    render: FrameFn,
    geometry: Cell<Option<ElementGeometry>>,
    pending: Cell<bool>,
}

impl SurfaceDriver {
    fn tick(self: &Rc<Self>, window: &mut Window, cx: &mut App) {
        if self.current.get() != self.generation || self.pending.get() {
            return;
        }
        let Some(entity) = self.entity.upgrade() else {
            return;
        };
        let Some(geometry) = self.geometry.get() else {
            return;
        };
        if geometry
            .bounds
            .intersect(&geometry.content_mask.bounds)
            .is_empty()
        {
            return;
        }
        let animate = entity.update(cx, |view, cx| (self.render)(view, geometry, window, cx));
        if !animate {
            return;
        }
        self.pending.set(true);
        let weak = Rc::downgrade(self);
        let interval = self.interval;
        window
            .spawn(cx, async move |cx| {
                cx.background_executor().timer(interval).await;
                let Some(driver) = weak.upgrade() else { return };
                driver.pending.set(false);
                let _ = cx.update(|window, cx| {
                    if driver.current.get() == driver.generation
                        && window.was_view_rendered(driver.entity.entity_id())
                    {
                        driver.tick(window, cx);
                        window.refresh_buffers();
                    }
                });
            })
            .detach();
    }
}

/// The menu's items in display order, children indented below their parent.
fn theme_color(c: graphy_render::Color) -> gpui::Hsla {
    gpui::Rgba { r: c[0], g: c[1], b: c[2], a: c[3] }.into()
}

fn flatten_menu(items: &[MenuItem]) -> Vec<(MenuItem, usize)> {
    fn walk(items: &[MenuItem], depth: usize, out: &mut Vec<(MenuItem, usize)>) {
        for item in items {
            let mut flat = item.clone();
            flat.children.clear();
            out.push((flat, depth));
            walk(&item.children, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(items, 0, &mut out);
    out
}

fn surface_driver(entity: &Entity<GraphCanvas>, current: &Rc<Cell<u64>>) -> gpui::Div {
    let generation = current.get().wrapping_add(1);
    current.set(generation);
    let driver = Rc::new(SurfaceDriver {
        generation,
        current: current.clone(),
        entity: entity.downgrade(),
        interval: Duration::from_millis(33),
        render: Box::new(|c, g, w, cx| c.draw_frame(g, w, cx)),
        geometry: Cell::new(None),
        pending: Cell::new(false),
    });
    div().on_frame(move |geometry, window, cx| {
        driver.geometry.set(Some(geometry));
        driver.tick(window, cx);
    })
}

impl Render for GraphCanvas {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_picker(window, cx);
        let entity = cx.entity();
        let display: AnyElement = match &self.surface {
            Some(s) => wgpu_surface(s.clone())
                .defer_resize_until_mouse_up(true)
                .absolute()
                .inset_0()
                .into_any_element(),
            None => div()
                .absolute()
                .inset_0()
                .bg(theme_color(self.theme.canvas))
                .into_any_element(),
        };
        let driver = surface_driver(&entity, &self.driver)
            .absolute()
            .inset_0()
            .size_full();

        // Host rows (previews/widgets) are real GPUI elements over the canvas,
        // only at the closest zoom tier where they are legible.
        let mut slots: Vec<AnyElement> = Vec::new();
        if LodTier::for_zoom(self.controller.view.zoom) == LodTier::Detail {
            let visible = self
                .controller
                .view
                .visible_rect(self.controller.viewport, 0.0);
            for s in self.scene.slot_rects(visible) {
                let r = self.controller.view.graph_rect_to_screen(s.rect);
                if let Some(el) = self.profile.render_slot(&self.graph, &s.node, s.slot, window, cx) {
                    slots.push(
                        div()
                            .absolute()
                            .left(px(r.min.x))
                            .top(px(r.min.y))
                            .w(px(r.width()))
                            .h(px(r.height()))
                            .overflow_hidden()
                            .child(el)
                            .into_any_element(),
                    );
                }
            }
        }

        let ent = |cx: &Context<Self>| cx.entity();
        let down = {
            let e = ent(cx);
            move |ev: &MouseDownEvent, window: &mut Window, cx: &mut App| {
                let Some(b) = button(ev.button) else { return };
                e.update(cx, |c, cx| {
                    c.overlay = None;
                    c.focus.focus(window, cx);
                    let pos = to_vec(ev.position, c.origin);
                    c.dispatch(
                        InputEvent::PointerDown {
                            pos,
                            button: b,
                            mods: mods(&ev.modifiers),
                            clicks: ev.click_count.min(255) as u8,
                        },
                        cx,
                    );
                });
            }
        };
        let up = {
            let e = ent(cx);
            move |ev: &MouseUpEvent, _: &mut Window, cx: &mut App| {
                let Some(b) = button(ev.button) else { return };
                e.update(cx, |c, cx| {
                    let pos = to_vec(ev.position, c.origin);
                    c.dispatch(
                        InputEvent::PointerUp {
                            pos,
                            button: b,
                            mods: mods(&ev.modifiers),
                        },
                        cx,
                    );
                });
            }
        };
        let mv = {
            let e = ent(cx);
            move |ev: &MouseMoveEvent, _: &mut Window, cx: &mut App| {
                e.update(cx, |c, cx| {
                    let pos = to_vec(ev.position, c.origin);
                    c.dispatch(
                        InputEvent::PointerMove {
                            pos,
                            mods: mods(&ev.modifiers),
                        },
                        cx,
                    );
                });
            }
        };
        let wheel = {
            let e = ent(cx);
            move |ev: &ScrollWheelEvent, _: &mut Window, cx: &mut App| {
                e.update(cx, |c, cx| {
                    let (delta, pixels) = match ev.delta {
                        ScrollDelta::Pixels(p) => (Vec2::new(f32::from(p.x), f32::from(p.y)), true),
                        ScrollDelta::Lines(l) => (Vec2::new(l.x, l.y), false),
                    };
                    if !delta.x.is_finite() || !delta.y.is_finite() {
                        return;
                    }
                    let pos = to_vec(ev.position, c.origin);
                    c.dispatch(
                        InputEvent::Wheel {
                            pos,
                            delta,
                            pixels,
                            mods: mods(&ev.modifiers),
                        },
                        cx,
                    );
                });
            }
        };
        let keys = {
            let e = ent(cx);
            move |ev: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                e.update(cx, |c, cx| {
                    // Key events bubble from overlays too; only act when the canvas has focus.
                    if !c.focus.is_focused(window) {
                        return;
                    }
                    if ev.keystroke.key == "escape" && c.overlay.take().is_some() {
                        cx.notify();
                        return;
                    }
                    if let Some(k) = key(&ev.keystroke.key) {
                        c.dispatch(
                            InputEvent::KeyDown {
                                key: k,
                                mods: mods(&ev.keystroke.modifiers),
                            },
                            cx,
                        );
                    }
                });
            }
        };

        div()
            .size_full()
            .relative()
            .overflow_hidden()
            .track_focus(&self.focus)
            .key_context("GraphyCanvas")
            .cursor(cursor_style(self.cursor))
            .child(display)
            .child(driver)
            .children(slots)
            .children(self.popup(cx))
            .on_mouse_down(GpuiButton::Left, down.clone())
            .on_mouse_down(GpuiButton::Right, down.clone())
            .on_mouse_down(GpuiButton::Middle, down)
            .on_mouse_up(GpuiButton::Left, up.clone())
            .on_mouse_up_out(GpuiButton::Left, up.clone())
            .on_mouse_up(GpuiButton::Right, up.clone())
            .on_mouse_up_out(GpuiButton::Right, up.clone())
            .on_mouse_up(GpuiButton::Middle, up.clone())
            .on_mouse_up_out(GpuiButton::Middle, up)
            .on_mouse_move(mv)
            .on_scroll_wheel(wheel)
            .on_key_down(keys)
    }
}

/// Fonts for the canvas text: a readable system font, with a compiled-in
/// fallback so the canvas works on unusual setups. Every editor on the shared
/// canvas uses these.
pub fn system_fonts() -> FontSet {
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &[
            "/System/Library/Fonts/Helvetica.ttc",
            "/Library/Fonts/Arial.ttf",
            "/System/Library/Fonts/SFNSText.ttf",
        ]
    } else if cfg!(target_os = "windows") {
        &[
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\arial.ttf",
            r"C:\Windows\Fonts\calibri.ttf",
        ]
    } else {
        &[
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        ]
    };
    let regular = candidates
        .iter()
        .find_map(|p| std::fs::read(p).ok())
        .unwrap_or_else(|| include_bytes!("../assets/fallback_font.ttf").to_vec());
    FontSet {
        regular,
        bold: None,
    }
}
