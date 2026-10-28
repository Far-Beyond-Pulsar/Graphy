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
    div, px, wgpu_surface, AnyElement, App, ClipboardItem, Context, CursorStyle, ElementGeometry,
    Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton as GpuiButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Point, Render, ScrollDelta, ScrollWheelEvent, Styled, WeakEntity, WgpuSurfaceHandle, Window,
};
use graphy::{ChangeSet, EditError, GraphCommand, GraphDescription};
use graphy_render::{DrawParams, LayoutMetrics, LodTier, Scene, Vec2};
use graphy_render_wgpu::{FontMeasure, FontSet, Fonts, GraphRenderer};

use crate::controller::{apply_transaction, Action, Controller, GraphCtx, Outcome};
use crate::input::{CursorHint, InputEvent, Key, Modifiers, MouseButton};
use crate::profile::GraphProfile;

/// A [`GraphProfile`] that can also contribute GPUI content. Implement it
/// (usually with an empty body) for any profile used with [`GraphCanvas`].
pub trait GpuiProfile: GraphProfile {
    /// Content for a host row (texture preview, custom body, inline editor)
    /// reported by the layout as `slot` of `node`. The element is positioned
    /// over the row's rectangle by the canvas. Return `None` to leave the
    /// placeholder well the renderer already draws.
    fn render_slot(
        &self,
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
        changes: ChangeSet,
    },
    /// A command batch failed and was rolled back.
    Rejected(String),
    SelectionChanged,
    ViewChanged,
    /// Menus, graph navigation, inline-value editing, dropped connections,
    /// slot presses: anything the host must present or decide.
    Request(Action),
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
        })
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
                self.scene.apply_changes(
                    &self.graph,
                    &changes,
                    self.profile.presenter(),
                    &self.measure,
                );
                cx.emit(CanvasEvent::Edited { label, changes });
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                .bg(gpui::rgb(0x0d0d0d))
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
                if let Some(el) = self.profile.render_slot(&s.node, s.slot, _window, cx) {
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
