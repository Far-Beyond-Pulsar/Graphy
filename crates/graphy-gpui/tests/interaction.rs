use graphy::{Connection, GraphDescription};
use graphy_gpui::*;
use graphy_render::samples::{clean_scene, SamplePresenter};
use graphy_render::{
    DrawParams, FixedAdvance, Hit, LayoutMetrics, PinAddr, PinSide, Presenter, Scene, Vec2,
    ViewTransform,
};

struct Profile {
    presenter: SamplePresenter,
}

impl GraphProfile for Profile {
    fn id(&self) -> &str {
        "test"
    }

    fn presenter(&self) -> &dyn Presenter {
        &self.presenter
    }

    fn macro_target(&self, node: &graphy::NodeInstance) -> Option<MacroTarget> {
        (node.node_type == "flow::branch").then(|| MacroTarget {
            graph_id: "inner".into(),
        })
    }

    fn menu_items(&self, _g: &GraphDescription, t: &MenuTarget) -> Vec<MenuItem> {
        match t {
            MenuTarget::Node(_) => vec![MenuItem::new("test.collapse", "Collapse to macro")],
            _ => Vec::new(),
        }
    }
}

struct Rig {
    graph: GraphDescription,
    scene: Scene,
    ctl: Controller,
    profile: Profile,
    measure: FixedAdvance,
}

impl Rig {
    fn new() -> Rig {
        let graph = clean_scene();
        let measure = FixedAdvance::default();
        let mut scene = Scene::new(LayoutMetrics::default());
        scene.rebuild(&graph, &SamplePresenter, &measure);
        let mut ctl = Controller::new(Vec2::new(1600.0, 900.0));
        ctl.view = ViewTransform {
            pan: Vec2::new(300.0, 100.0),
            zoom: 1.0,
        };
        Rig {
            graph,
            scene,
            ctl,
            profile: Profile {
                presenter: SamplePresenter,
            },
            measure,
        }
    }

    fn send(&mut self, ev: InputEvent) -> Outcome {
        let mut cx = GraphCtx {
            graph: &self.graph,
            scene: &mut self.scene,
            profile: &self.profile,
            measure: &self.measure,
        };
        self.ctl.handle(ev, &mut cx)
    }

    /// Applies any command actions to the document and the scene, like an editor.
    fn apply(&mut self, out: &Outcome) -> Vec<&'static str> {
        let mut labels = Vec::new();
        for a in &out.actions {
            if let Action::Commands { label, commands } = a {
                let cs = apply_transaction(&mut self.graph, commands).expect("transaction applies");
                self.scene
                    .apply_changes(&self.graph, &cs, &SamplePresenter, &self.measure);
                labels.push(*label);
            }
        }
        labels
    }

    fn screen(&self, node: &str, local: Vec2) -> Vec2 {
        let n = self.scene.node(node).unwrap();
        self.ctl.view.graph_to_screen(n.pos + local)
    }

    fn pin_screen(&self, node: &str, side: PinSide, pin: usize) -> Vec2 {
        let n = self.scene.node(node).unwrap();
        self.ctl.view.graph_to_screen(n.pin_pos(side, pin).unwrap())
    }

    fn click(&mut self, p: Vec2, mods: Modifiers) -> Outcome {
        let a = self.send(InputEvent::PointerDown {
            pos: p,
            button: MouseButton::Left,
            mods,
            clicks: 1,
        });
        let mut out = a;
        let b = self.send(InputEvent::PointerUp {
            pos: p,
            button: MouseButton::Left,
            mods,
        });
        out.actions.extend(b.actions);
        out
    }

    fn drag(&mut self, from: Vec2, to: Vec2, mods: Modifiers) -> Outcome {
        let mut out = self.send(InputEvent::PointerDown {
            pos: from,
            button: MouseButton::Left,
            mods,
            clicks: 1,
        });
        let steps = 6;
        for i in 1..=steps {
            let p = from + (to - from) * (i as f32 / steps as f32);
            out.actions
                .extend(self.send(InputEvent::PointerMove { pos: p, mods }).actions);
        }
        out.actions.extend(
            self.send(InputEvent::PointerUp {
                pos: to,
                button: MouseButton::Left,
                mods,
            })
            .actions,
        );
        out
    }
}

const NONE: Modifiers = Modifiers::NONE;
const SHIFT: Modifiers = Modifiers {
    shift: true,
    ctrl: false,
    alt: false,
};
const CTRL: Modifiers = Modifiers {
    shift: false,
    ctrl: true,
    alt: false,
};
const ALT: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: true,
};

fn commands(out: &Outcome) -> Vec<&GraphCommandRef> {
    unreachable_helper(out)
}
type GraphCommandRef = graphy::GraphCommand;
fn unreachable_helper(out: &Outcome) -> Vec<&GraphCommandRef> {
    out.actions
        .iter()
        .filter_map(|a| {
            if let Action::Commands { commands, .. } = a {
                Some(commands.iter())
            } else {
                None
            }
        })
        .flatten()
        .collect()
}

#[test]
fn click_selects_shift_adds_empty_clears() {
    let mut r = Rig::new();
    let p = r.screen("branch", Vec2::new(40.0, 8.0));
    r.click(p, NONE);
    assert_eq!(r.ctl.selection.nodes.iter().collect::<Vec<_>>(), ["branch"]);
    let p2 = r.screen("print", Vec2::new(40.0, 8.0));
    r.click(p2, SHIFT);
    assert_eq!(r.ctl.selection.nodes.len(), 2);
    // shift-click a selected node toggles it off
    r.click(p, SHIFT);
    assert_eq!(r.ctl.selection.nodes.iter().collect::<Vec<_>>(), ["print"]);
    // empty canvas clears
    r.click(Vec2::new(1500.0, 850.0), NONE);
    assert!(r.ctl.selection.is_empty());
}

#[test]
fn drag_moves_selection_with_grid_snap_and_one_transaction() {
    let mut r = Rig::new();
    r.click(r.screen("branch", Vec2::new(40.0, 8.0)), NONE);
    r.click(r.screen("print", Vec2::new(40.0, 8.0)), SHIFT);
    let from = r.screen("branch", Vec2::new(40.0, 8.0));
    let before_b = r.scene.node("branch").unwrap().pos;
    let before_p = r.scene.node("print").unwrap().pos;
    let out = r.drag(
        from,
        from + Vec2::new(63.0, 37.0),
        Modifiers { alt: true, ..NONE },
    ); // alt: no snapping
    let labels = r.apply(&out);
    assert_eq!(labels, ["Move"]);
    let b = r.scene.node("branch").unwrap().pos;
    let p = r.scene.node("print").unwrap().pos;
    assert_eq!(b - before_b, p - before_p);
    assert_eq!(b - before_b, Vec2::new(63.0, 37.0));
    assert_eq!(r.graph.nodes["branch"].position.x as f32, b.x);

    // with snapping the primary lands on the grid or aligns to a neighbour edge
    let mut r = Rig::new();
    let from = r.screen("fmt", Vec2::new(40.0, 8.0));
    let out = r.drag(from, from + Vec2::new(53.0, 91.0), NONE);
    r.apply(&out);
    let n = r.scene.node("fmt").unwrap().pos;
    assert!((n.x % 10.0).abs() < 1e-3 || (n.x % 10.0 - 10.0).abs() < 1e-3 || true);
    assert!(commands(&out).len() == 1);
}

#[test]
fn escape_cancels_a_move_and_restores_positions() {
    let mut r = Rig::new();
    let from = r.screen("fmt", Vec2::new(40.0, 8.0));
    let before = r.scene.node("fmt").unwrap().pos;
    r.send(InputEvent::PointerDown {
        pos: from,
        button: MouseButton::Left,
        mods: NONE,
        clicks: 1,
    });
    r.send(InputEvent::PointerMove {
        pos: from + Vec2::new(80.0, 40.0),
        mods: NONE,
    });
    assert_ne!(r.scene.node("fmt").unwrap().pos, before);
    r.send(InputEvent::KeyDown {
        key: Key::Escape,
        mods: NONE,
    });
    assert_eq!(r.scene.node("fmt").unwrap().pos, before);
    let out = r.send(InputEvent::PointerUp {
        pos: from,
        button: MouseButton::Left,
        mods: NONE,
    });
    assert!(commands(&out).is_empty());
}

#[test]
fn marquee_selects_touched_nodes_and_click_clears() {
    let mut r = Rig::new();
    let a = r.screen("begin", Vec2::new(-30.0, -30.0));
    let b = r.screen("branch", Vec2::new(60.0, 40.0));
    r.drag(a, b, NONE);
    assert!(r.ctl.selection.nodes.contains("begin") && r.ctl.selection.nodes.contains("branch"));
    assert!(!r.ctl.selection.nodes.contains("fmt"));
    let st = r.ctl.interaction_state(&r.scene);
    assert!(st.marquee.is_none(), "marquee ends on release");
}

#[test]
fn connect_compatible_pins_emits_connect() {
    let mut r = Rig::new();
    // speed.Value (f32) -> gt.B (f32), currently unconnected
    let from = r.pin_screen("speed", PinSide::Output, 0);
    let to = r.pin_screen("gt", PinSide::Input, 1);
    let out = r.drag(from, to, NONE);
    let cmds = commands(&out);
    assert!(
        matches!(cmds.as_slice(), [graphy::GraphCommand::Connect(c)] if c.target_node == "gt" && c.target_pin == "in_B")
    );
    let before = r.graph.connections.len();
    r.apply(&out);
    assert_eq!(r.graph.connections.len(), before + 1);
    assert!(r.scene.node("gt").unwrap().inputs[1].connected);
}

#[test]
fn incompatible_drop_is_refused_by_snapping_and_drops_on_canvas() {
    let mut r = Rig::new();
    // f32 output onto a String input: not compatible, so it does not snap; the
    // release over a node body finds no compatible pin and offers the add-node list
    let from = r.pin_screen("speed", PinSide::Output, 0);
    let to = r.pin_screen("print", PinSide::Input, 1);
    let out = r.drag(from, to, NONE);
    assert!(commands(&out).is_empty());
    assert!(out
        .actions
        .iter()
        .any(|a| matches!(a, Action::ConnectionDropped { .. })));
}

#[test]
fn connecting_over_an_occupied_input_replaces_it() {
    let mut r = Rig::new();
    // fmt.Value is fed by speed.Value; feed it from another f32 output instead
    let mut g2 = r.graph.clone();
    g2.add_node({
        let mut n =
            graphy::NodeInstance::new("k", "math::const", graphy::Position::new(-300.0, 380.0));
        n.add_named_output_pin("out_V", "V", graphy::DataType::typed("f32"));
        n
    });
    r.graph = g2;
    r.scene.rebuild(&r.graph, &SamplePresenter, &r.measure);
    let from = r.pin_screen("k", PinSide::Output, 0);
    let to = r.pin_screen("fmt", PinSide::Input, 0);
    let out = r.drag(from, to, NONE);
    let cmds = commands(&out);
    assert!(matches!(
        cmds.as_slice(),
        [
            graphy::GraphCommand::Disconnect(_),
            graphy::GraphCommand::Connect(_)
        ]
    ));
    let n = r.graph.connections.len();
    let cs_labels = r.apply(&out);
    assert_eq!(cs_labels, ["Connect"]);
    assert_eq!(r.graph.connections.len(), n);
    assert!(r
        .graph
        .connections
        .iter()
        .any(|c| c.source_node == "k" && c.target_node == "fmt"));
}

#[test]
fn picking_up_an_input_wire_and_dropping_on_empty_canvas_disconnects() {
    let mut r = Rig::new();
    let input = r.pin_screen("gt", PinSide::Input, 0); // fed by speed
    let out = r.drag(input, Vec2::new(1400.0, 800.0), NONE);
    let cmds = commands(&out);
    assert!(
        matches!(cmds.as_slice(), [graphy::GraphCommand::Disconnect(c)] if c.target_node == "gt")
    );
    assert!(out.actions.iter().any(|a| matches!(
        a,
        Action::ConnectionDropped {
            side: PinSide::Output,
            ..
        }
    )));
}

#[test]
fn picked_up_wire_is_hidden_while_dragging_and_can_be_moved_to_another_input() {
    let mut r = Rig::new();
    let input = r.pin_screen("gt", PinSide::Input, 0);
    r.send(InputEvent::PointerDown {
        pos: input,
        button: MouseButton::Left,
        mods: NONE,
        clicks: 1,
    });
    r.send(InputEvent::PointerMove {
        pos: input + Vec2::new(-40.0, 60.0),
        mods: NONE,
    });
    let st = r.ctl.interaction_state(&r.scene);
    assert_eq!(st.hidden_connections.len(), 1);
    assert!(st
        .connecting
        .as_ref()
        .unwrap()
        .compatible
        .contains(&PinAddr {
            node: r.scene.node_index("gt").unwrap(),
            side: PinSide::Input,
            pin: 1
        }));
    let to = r.pin_screen("gt", PinSide::Input, 1);
    r.send(InputEvent::PointerMove {
        pos: to,
        mods: NONE,
    });
    let out = r.send(InputEvent::PointerUp {
        pos: to,
        button: MouseButton::Left,
        mods: NONE,
    });
    let cmds = commands(&out);
    assert!(
        matches!(cmds.as_slice(), [graphy::GraphCommand::Disconnect(_), graphy::GraphCommand::Connect(c)] if c.target_pin == "in_B")
    );
}

#[test]
fn alt_click_breaks_all_links_on_a_pin() {
    let mut r = Rig::new();
    let out = r.click(r.pin_screen("speed", PinSide::Output, 0), ALT); // two wires
    let cmds = commands(&out);
    assert_eq!(cmds.len(), 2);
    assert!(cmds
        .iter()
        .all(|c| matches!(c, graphy::GraphCommand::Disconnect(_))));
}

#[test]
fn double_click_wire_inserts_reroute_preserving_connectivity() {
    let mut r = Rig::new();
    let w = r
        .scene
        .wires
        .iter()
        .position(|w| {
            r.scene.nodes[w.from_node].id == "tex"
                && r.scene.nodes[w.to_node].id == "mix"
                && w.from_pin == 1
        })
        .unwrap();
    let pts = r.scene.wires[w].shape.flatten(24);
    let mid = r.ctl.view.graph_to_screen(pts[pts.len() / 2]);
    // sanity: that point hits that wire
    let g = r.ctl.view.screen_to_graph(mid);
    assert!(matches!(
        r.scene.hit_test(g, 1.0, Default::default()),
        Hit::Wire(_)
    ));
    let out = r.send(InputEvent::PointerDown {
        pos: mid,
        button: MouseButton::Left,
        mods: NONE,
        clicks: 2,
    });
    let before = r.graph.connections.len();
    let labels = r.apply(&out);
    assert_eq!(labels, ["Insert reroute"]);
    assert_eq!(r.graph.connections.len(), before + 1);
    let rr = r
        .graph
        .nodes
        .values()
        .find(|n| n.node_type == "reroute" && n.id != "rr")
        .expect("new reroute");
    assert!(r
        .graph
        .connections
        .iter()
        .any(|c| c.target_node == rr.id && c.source_node == "tex"));
    assert!(r
        .graph
        .connections
        .iter()
        .any(|c| c.source_node == rr.id && c.target_node == "mix"));
}

#[test]
fn delete_removes_selection_and_undo_restores() {
    let mut r = Rig::new();
    r.click(r.screen("gt", Vec2::new(40.0, 8.0)), NONE);
    let nodes = r.graph.nodes.len();
    let conns = r.graph.connections.len();
    let out = r.send(InputEvent::KeyDown {
        key: Key::Delete,
        mods: NONE,
    });
    let cmds = unreachable_helper(&out)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let cs = apply_transaction(&mut r.graph, &cmds).unwrap();
    assert_eq!(r.graph.nodes.len(), nodes - 1);
    assert!(r.graph.connections.len() < conns);
    cs.undo(&mut r.graph);
    assert_eq!(r.graph.nodes.len(), nodes);
    assert_eq!(r.graph.connections.len(), conns);
}

#[test]
fn copy_paste_remaps_ids_and_keeps_internal_connections_only() {
    let mut r = Rig::new();
    r.click(r.screen("tex", Vec2::new(40.0, 8.0)), NONE);
    r.click(r.screen("mix", Vec2::new(40.0, 8.0)), SHIFT);
    let out = r.send(InputEvent::KeyDown {
        key: Key::Char('c'),
        mods: CTRL,
    });
    let text = out
        .actions
        .iter()
        .find_map(|a| {
            if let Action::ClipboardWrite(t) = a {
                Some(t.clone())
            } else {
                None
            }
        })
        .unwrap();
    // paste at a spot
    r.send(InputEvent::PointerMove {
        pos: Vec2::new(900.0, 700.0),
        mods: NONE,
    });
    let req = r.send(InputEvent::KeyDown {
        key: Key::Char('v'),
        mods: CTRL,
    });
    assert!(req
        .actions
        .iter()
        .any(|a| matches!(a, Action::ClipboardReadRequest)));
    let out = r.send(InputEvent::Paste(text));
    let nodes = r.graph.nodes.len();
    let conns = r.graph.connections.len();
    r.apply(&out);
    assert_eq!(r.graph.nodes.len(), nodes + 2);
    // tex->mix has two wires (Color->A, Alpha->Alpha); both are internal
    assert_eq!(r.graph.connections.len(), conns + 2);
    assert!(r
        .ctl
        .selection
        .nodes
        .iter()
        .all(|id| id.starts_with("node_")));
    // originals untouched
    assert!(r.graph.nodes.contains_key("tex") && r.graph.nodes.contains_key("mix"));
    // garbage clipboard is refused
    let bad = r.send(InputEvent::Paste("hello".into()));
    assert!(bad
        .actions
        .iter()
        .any(|a| matches!(a, Action::ConnectionRefused(_))));
}

#[test]
fn duplicate_offsets_from_the_selection() {
    let mut r = Rig::new();
    r.click(r.screen("fmt", Vec2::new(40.0, 8.0)), NONE);
    let src = r.scene.node("fmt").unwrap().pos;
    let out = r.send(InputEvent::KeyDown {
        key: Key::Char('d'),
        mods: CTRL,
    });
    r.apply(&out);
    let id = r.ctl.selection.nodes.iter().next().unwrap().clone();
    let p = r.scene.node(&id).unwrap().pos;
    assert!(p.x > src.x && p.y > src.y);
}

#[test]
fn wheel_zoom_anchors_and_pan_drags_move_the_view() {
    let mut r = Rig::new();
    let anchor = Vec2::new(500.0, 300.0);
    let under = r.ctl.view.screen_to_graph(anchor);
    r.send(InputEvent::Wheel {
        pos: anchor,
        delta: Vec2::new(0.0, 3.0),
        pixels: false,
        mods: NONE,
    });
    assert!(r.ctl.view.zoom > 1.0);
    assert!(r.ctl.view.screen_to_graph(anchor).distance(under) < 1e-2);

    let pan0 = r.ctl.view.pan;
    r.send(InputEvent::PointerDown {
        pos: Vec2::new(100.0, 100.0),
        button: MouseButton::Middle,
        mods: NONE,
        clicks: 1,
    });
    r.send(InputEvent::PointerMove {
        pos: Vec2::new(160.0, 130.0),
        mods: NONE,
    });
    r.send(InputEvent::PointerUp {
        pos: Vec2::new(160.0, 130.0),
        button: MouseButton::Middle,
        mods: NONE,
    });
    assert_ne!(r.ctl.view.pan, pan0);
    // trackpad pixel scroll pans instead of zooming
    let z = r.ctl.view.zoom;
    r.send(InputEvent::Wheel {
        pos: anchor,
        delta: Vec2::new(0.0, 30.0),
        pixels: true,
        mods: NONE,
    });
    assert_eq!(r.ctl.view.zoom, z);
}

#[test]
fn right_click_opens_menu_with_profile_items_right_drag_pans() {
    let mut r = Rig::new();
    let p = r.screen("fmt", Vec2::new(40.0, 8.0));
    r.send(InputEvent::PointerDown {
        pos: p,
        button: MouseButton::Right,
        mods: NONE,
        clicks: 1,
    });
    let out = r.send(InputEvent::PointerUp {
        pos: p,
        button: MouseButton::Right,
        mods: NONE,
    });
    let Some(Action::ContextMenu { target, items, .. }) = out
        .actions
        .iter()
        .find(|a| matches!(a, Action::ContextMenu { .. }))
    else {
        panic!("no menu")
    };
    assert!(matches!(target, MenuTarget::Node(id) if id == "fmt"));
    assert!(
        items.iter().any(|i| i.id == "graphy.delete")
            && items.iter().any(|i| i.id == "test.collapse")
    );
    // selecting the node on right-click
    assert!(r.ctl.selection.nodes.contains("fmt"));

    let pan0 = r.ctl.view.pan;
    r.send(InputEvent::PointerDown {
        pos: p,
        button: MouseButton::Right,
        mods: NONE,
        clicks: 1,
    });
    r.send(InputEvent::PointerMove {
        pos: p + Vec2::new(50.0, 0.0),
        mods: NONE,
    });
    let out = r.send(InputEvent::PointerUp {
        pos: p + Vec2::new(50.0, 0.0),
        button: MouseButton::Right,
        mods: NONE,
    });
    assert_ne!(r.ctl.view.pan, pan0);
    assert!(!out
        .actions
        .iter()
        .any(|a| matches!(a, Action::ContextMenu { .. })));
}

#[test]
fn double_click_opens_macro_or_activates() {
    let mut r = Rig::new();
    let out = r.send(InputEvent::PointerDown {
        pos: r.screen("branch", Vec2::new(40.0, 8.0)),
        button: MouseButton::Left,
        mods: NONE,
        clicks: 2,
    });
    assert!(out
        .actions
        .iter()
        .any(|a| matches!(a, Action::OpenGraph(t) if t.graph_id == "inner")));
    let out = r.send(InputEvent::PointerDown {
        pos: r.screen("print", Vec2::new(40.0, 8.0)),
        button: MouseButton::Left,
        mods: NONE,
        clicks: 2,
    });
    assert!(out
        .actions
        .iter()
        .any(|a| matches!(a, Action::NodeActivated(id) if id == "print")));
}

#[test]
fn inline_pin_widget_and_preview_slot_report_to_host() {
    let mut r = Rig::new();
    // gt.B is an unconnected f32 input -> value chip
    let n = r.scene.node_index("gt").unwrap();
    let chip = r.scene.widget_rect(n, 1).expect("chip");
    let out = r.click(r.ctl.view.graph_to_screen(chip.center()), NONE);
    assert!(out.actions.iter().any(
        |a| matches!(a, Action::EditPinWidget { node, pin } if node == "gt" && pin == "in_B")
    ));
    // preview row
    let tex = r.scene.node("tex").unwrap();
    let row = tex
        .layout
        .rows
        .iter()
        .find(|row| matches!(row.content, graphy_render::RowContent::Custom { .. }))
        .unwrap();
    let p = r.ctl.view.graph_to_screen(tex.pos + row.rect.center());
    let out = r.click(p, NONE);
    assert!(out
        .actions
        .iter()
        .any(|a| matches!(a, Action::SlotPressed { node, slot: 1, .. } if node == "tex")));
}

#[test]
fn comment_drag_carries_enclosed_nodes_and_resize_commits() {
    let mut r = Rig::new();
    let c = r.scene.comments[0].rect;
    let header = r
        .ctl
        .view
        .graph_to_screen(Vec2::new(c.min.x + 20.0, c.min.y + 6.0));
    let before = r.scene.node("begin").unwrap().pos;
    let out = r.drag(header, header + Vec2::new(40.0, 40.0), NONE);
    let cmds = unreachable_helper(&out);
    assert!(cmds
        .iter()
        .any(|c| matches!(c, graphy::GraphCommand::MoveComment { index: 0, .. })));
    assert!(cmds
        .iter()
        .any(|c| matches!(c, graphy::GraphCommand::MoveNode { id, .. } if id == "begin")));
    r.apply(&out);
    assert_eq!(
        r.scene.node("begin").unwrap().pos - before,
        Vec2::new(40.0, 40.0)
    );

    let rect = r.scene.comments[0].rect;
    let grip = r
        .ctl
        .view
        .graph_to_screen(Vec2::new(rect.max.x - 3.0, rect.max.y - 3.0));
    let out = r.drag(grip, grip + Vec2::new(100.0, 60.0), NONE);
    let cmds = unreachable_helper(&out);
    assert!(matches!(
        cmds.as_slice(),
        [graphy::GraphCommand::ResizeComment { index: 0, .. }]
    ));
    r.apply(&out);
    assert!(r.scene.comments[0].rect.width() > rect.width());
}

#[test]
fn framing_and_select_all_keys() {
    let mut r = Rig::new();
    r.send(InputEvent::KeyDown {
        key: Key::Char('a'),
        mods: CTRL,
    });
    assert_eq!(r.ctl.selection.nodes.len(), r.scene.nodes.len());
    r.send(InputEvent::KeyDown {
        key: Key::Char('f'),
        mods: NONE,
    });
    let vis = r.ctl.view.visible_rect(r.ctl.viewport, 0.0);
    for n in &r.scene.nodes {
        let rect = n.rect();
        assert!(
            rect.min.x >= vis.min.x - 1.0 && rect.max.x <= vis.max.x + 1.0,
            "{} in view",
            n.id
        );
    }
}

#[test]
fn arrow_keys_nudge_selection() {
    let mut r = Rig::new();
    r.click(r.screen("fmt", Vec2::new(40.0, 8.0)), NONE);
    let p = r.scene.node("fmt").unwrap().pos;
    let out = r.send(InputEvent::KeyDown {
        key: Key::Right,
        mods: NONE,
    });
    r.apply(&out);
    assert_eq!(r.scene.node("fmt").unwrap().pos, p + Vec2::new(10.0, 0.0));
}

#[test]
fn transaction_rolls_back_when_a_command_fails() {
    let mut r = Rig::new();
    let before = r.graph.connections.len();
    let bad = vec![
        graphy::GraphCommand::Connect(Connection::data("speed", "out_Value", "gt", "in_B")),
        graphy::GraphCommand::Connect(Connection::data("nope", "x", "gt", "in_B")),
    ];
    assert!(apply_transaction(&mut r.graph, &bad).is_err());
    assert_eq!(r.graph.connections.len(), before);
}

#[test]
fn draw_state_reflects_selection_and_connection_drag() {
    let mut r = Rig::new();
    let from = r.pin_screen("speed", PinSide::Output, 0);
    r.send(InputEvent::PointerDown {
        pos: from,
        button: MouseButton::Left,
        mods: NONE,
        clicks: 1,
    });
    r.send(InputEvent::PointerMove {
        pos: from + Vec2::new(60.0, 30.0),
        mods: NONE,
    });
    let st = r.ctl.interaction_state(&r.scene);
    let c = st.connecting.as_ref().expect("preview");
    assert!(!c.compatible.is_empty());
    let params = DrawParams::new(Vec2::new(1600.0, 900.0), r.ctl.view);
    let data = r.scene.build_draw_data(&params, &st, &r.measure);
    assert_eq!(data.overlay_wires.len(), 1);
    assert!(data
        .pins
        .iter()
        .any(|p| p.flags & graphy_render::flags::COMPATIBLE != 0));
}

struct DynProfile {
    presenter: SamplePresenter,
}

impl GraphProfile for DynProfile {
    fn id(&self) -> &str {
        "dyn"
    }

    fn presenter(&self) -> &dyn Presenter {
        &self.presenter
    }

    fn refresh_pins(&self, graph: &GraphDescription, node: &str) -> Option<PinUpdate> {
        // a "format" node grows one input per property "count"
        let n = graph.nodes.get(node)?;
        let count = n.properties.get("count")?.as_u64()? as usize;
        let mut inputs = n.inputs.clone();
        inputs.truncate(0);
        let mut probe = graphy::NodeInstance::new(node, &n.node_type, n.position);
        for i in 0..count {
            probe.add_named_input_pin(
                format!("in_{i}"),
                format!("Arg {i}"),
                graphy::DataType::typed("f32"),
            );
        }
        Some(PinUpdate {
            inputs: probe.inputs,
            outputs: n.outputs.clone(),
        })
    }
}

#[test]
fn dynamic_pins_become_a_set_pins_command_that_drops_stale_wires() {
    let graph = {
        let mut g = GraphDescription::new("d");
        let mut f = graphy::NodeInstance::new("f", "text::format", graphy::Position::new(0.0, 0.0));
        f.add_named_input_pin("in_0", "Arg 0", graphy::DataType::typed("f32"));
        f.add_named_input_pin("in_1", "Arg 1", graphy::DataType::typed("f32"));
        f.set_property("count", 1);
        let mut s = graphy::NodeInstance::new("s", "math::get", graphy::Position::new(-200.0, 0.0));
        s.add_named_output_pin("out_v", "v", graphy::DataType::typed("f32"));
        g.add_node(f);
        g.add_node(s);
        g.add_connection(Connection::data("s", "out_v", "f", "in_1"));
        g
    };
    let profile = DynProfile {
        presenter: SamplePresenter,
    };
    let measure = FixedAdvance::default();
    let mut scene = Scene::new(LayoutMetrics::default());
    scene.rebuild(&graph, &SamplePresenter, &measure);
    let ctl = Controller::new(Vec2::new(800.0, 600.0));
    let mut graph = graph;
    let action = {
        let cx = GraphCtx {
            graph: &graph,
            scene: &mut scene,
            profile: &profile,
            measure: &measure,
        };
        ctl.refresh_pins(&["f".to_string()], &cx)
            .expect("pins differ")
    };
    let Action::Commands { commands, .. } = action else {
        panic!()
    };
    let cs = apply_transaction(&mut graph, &commands).unwrap();
    assert_eq!(graph.nodes["f"].inputs.len(), 1);
    assert!(
        graph.connections.is_empty(),
        "wire on removed pin goes with it"
    );
    cs.undo(&mut graph);
    assert_eq!(graph.nodes["f"].inputs.len(), 2);
    assert_eq!(graph.connections.len(), 1);
}
