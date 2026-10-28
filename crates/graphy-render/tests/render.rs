use graphy::{ChangeSet, GraphCommand};
use graphy_render::samples::{clean_scene, crowded_scene, SamplePresenter, PREVIEW_SLOT};
use graphy_render::*;

fn scene_for(g: &graphy::GraphDescription) -> Scene {
    let mut s = Scene::new(LayoutMetrics::default());
    s.rebuild(g, &SamplePresenter, &FixedAdvance::default());
    s
}

#[test]
fn view_round_trips_and_zoom_keeps_anchor() {
    let mut v = ViewTransform { pan: Vec2::new(-40.0, 25.0), zoom: 1.3 };
    let g = Vec2::new(120.0, -60.0);
    let back = v.screen_to_graph(v.graph_to_screen(g));
    assert!(back.distance(g) < 1e-3);

    let anchor = Vec2::new(300.0, 200.0);
    let under = v.screen_to_graph(anchor);
    assert!(v.zoom_at(anchor, 1.5));
    assert!(v.screen_to_graph(anchor).distance(under) < 1e-3);
    // clamps
    for _ in 0..40 {
        v.zoom_at(anchor, 2.0);
    }
    assert_eq!(v.zoom, MAX_ZOOM);
}

#[test]
fn framing_centres_bounds_in_viewport() {
    let bounds = Rect::from_pos_size(Vec2::new(100.0, 100.0), Vec2::new(400.0, 200.0));
    let vp = Vec2::new(800.0, 600.0);
    let v = ViewTransform::framing(bounds, vp, 40.0, 1.0);
    let c = v.graph_to_screen(bounds.center());
    assert!(c.distance(Vec2::new(400.0, 300.0)) < 1e-2);
    let r = v.graph_rect_to_screen(bounds);
    assert!(r.min.x >= 40.0 - 1e-2 && r.max.x <= 760.0 + 1e-2);
}

#[test]
fn lod_tiers_follow_zoom() {
    assert_eq!(LodTier::for_zoom(0.1), LodTier::Overview);
    assert_eq!(LodTier::for_zoom(0.45), LodTier::Summary);
    assert_eq!(LodTier::for_zoom(1.0), LodTier::Detail);
    assert!(LodTier::Detail.wire_segments() > LodTier::Overview.wire_segments());
}

#[test]
fn preview_row_makes_variable_height_node() {
    let g = clean_scene();
    let s = scene_for(&g);
    let tex = s.node("tex").unwrap();
    let m = &s.metrics;
    // 2 inputs/2 outputs -> 2 pin rows + one 64-high preview row
    let expected_body = 2.0 * m.row_h + 64.0 + 2.0 * m.row_gap;
    let expected = m.snap_up(m.header_h + m.body_pad_y * 2.0 + expected_body);
    assert_eq!(tex.layout.size.y, expected);
    let custom = tex.layout.rows.iter().find(|r| matches!(r.content, RowContent::Custom { slot } if slot == PREVIEW_SLOT)).unwrap();
    assert_eq!(custom.rect.height(), 64.0);
    // pin rows come first and are fixed height
    assert_eq!(tex.layout.rows[0].rect.height(), m.row_h);
    // widths snap to the grid and honour the preview's minimum width
    assert_eq!(tex.layout.size.x % m.grid_snap, 0.0);
    assert!(tex.layout.size.x >= 150.0);
}

#[test]
fn header_output_pin_sits_in_header() {
    let g = clean_scene();
    let s = scene_for(&g);
    let gt = s.node("gt").unwrap();
    let p = gt.layout.pin(PinSide::Output, 0).unwrap();
    assert!(p.row.is_none());
    assert!((p.center.y - s.metrics.header_h * 0.5).abs() < 1e-4);
    assert!((p.center.x - (gt.layout.size.x - s.metrics.pin_inset)).abs() < 1e-4);
    // and the body has only the two input rows
    assert_eq!(gt.layout.rows.len(), 2);
}

#[test]
fn long_labels_are_capped_not_unbounded() {
    let mut g = clean_scene();
    let n = g.nodes.get_mut("print").unwrap();
    n.add_named_input_pin("in_long", "An extraordinarily long pin label that keeps going and going and going", graphy::DataType::typed("String"));
    let s = scene_for(&g);
    let w = s.node("print").unwrap().layout.size.x;
    let m = &s.metrics;
    assert!(w <= m.snap_up(m.pin_inset * 2.0 + m.pin_size + m.label_gap * 2.0 + scene::MAX_LABEL_WIDTH * 2.0 + m.column_gap + 40.0));
}

#[test]
fn hit_and_draw_share_pin_geometry() {
    let g = clean_scene();
    let s = scene_for(&g);
    let state = InteractionState::default();
    let data = s.build_draw_data(&DrawParams::new(Vec2::new(1600.0, 900.0), ViewTransform { pan: Vec2::new(300.0, 100.0), zoom: 1.0 }), &state, &FixedAdvance::default());
    assert!(!data.pins.is_empty());
    for pin in &data.pins {
        let hit = s.pin_at(pin.center, 1.0, HitTolerance::default()).expect("every drawn pin is hittable at its centre");
        let addr_pos = s.nodes[hit.node].pin_pos(hit.side, hit.pin).unwrap();
        assert!(addr_pos.distance(pin.center) < 1e-3);
    }
}

#[test]
fn hit_priority_pin_node_comment_wire() {
    let g = clean_scene();
    let s = scene_for(&g);
    let tol = HitTolerance::default();
    let branch = s.node_index("branch").unwrap();
    let n = &s.nodes[branch];

    // pin has priority over the node it sits on, and the hit disc is generous
    let pin = n.pin_pos(PinSide::Input, 1).unwrap();
    let near = pin + Vec2::new(5.0, 4.0);
    assert_eq!(s.hit_test(near, 1.0, tol), Hit::Pin(PinAddr { node: branch, side: PinSide::Input, pin: 1 }));

    // header vs body
    let header = n.pos + Vec2::new(40.0, 6.0);
    assert!(matches!(s.hit_test(header, 1.0, tol), Hit::Node { part: NodePart::Header, .. }));
    let body = n.pos + Vec2::new(n.layout.size.x * 0.5, s.metrics.header_h + 10.0);
    assert!(matches!(s.hit_test(body, 1.0, tol), Hit::Node { part: NodePart::Body, .. }));

    // preview row reports its slot
    let tex = s.node("tex").unwrap();
    let row = tex.layout.rows.iter().find(|r| matches!(r.content, RowContent::Custom { .. })).unwrap();
    let p = tex.pos + row.rect.center();
    assert!(matches!(s.hit_test(p, 1.0, tol), Hit::Node { part: NodePart::Custom(PREVIEW_SLOT), .. }));

    // comment header and resize handle
    let mut g2 = g.clone();
    g2.comments[0].size = (1500.0, 1500.0);
    let s = scene_for(&g2);
    let c = s.comments[0].rect;
    let ch = Vec2::new(c.min.x + 4.0, c.min.y + 4.0);
    assert_eq!(s.hit_test(ch, 1.0, tol), Hit::CommentHeader(0));
    let cr = Vec2::new(c.max.x - 3.0, c.max.y - 3.0);
    assert_eq!(s.hit_test(cr, 1.0, tol), Hit::CommentResize(0));

    // empty canvas
    assert_eq!(s.hit_test(Vec2::new(9000.0, 9000.0), 1.0, tol), Hit::None);
}

#[test]
fn wires_are_pickable_on_their_curve() {
    let g = clean_scene();
    let s = scene_for(&g);
    let w = &s.wires[0];
    let pts = w.shape.flatten(24);
    let mid = pts[pts.len() / 2];
    // pick on the curve, away from any node
    let picked = s.wire_at(mid, 1.0, HitTolerance::default());
    assert!(picked.is_some());
    assert!(s.wire_at(mid + Vec2::new(0.0, 80.0), 1.0, HitTolerance::default()).map_or(true, |i| i != picked.unwrap()));
}

#[test]
fn angular_route_hits_endpoints_and_stays_orthogonal() {
    for (from, to) in [
        (Vec2::new(0.0, 0.0), Vec2::new(300.0, 120.0)),
        (Vec2::new(300.0, 0.0), Vec2::new(0.0, 200.0)),
        (Vec2::new(300.0, 100.0), Vec2::new(280.0, 110.0)),
    ] {
        let shape = WireShape::new(WireRouting::Angular, from, to);
        let pts = shape.flatten(0);
        assert!(pts.first().unwrap().distance(from) < 1e-3);
        assert!(pts.last().unwrap().distance(to) < 1e-3);
        assert!(shape.distance_to(from) < 1e-3 && shape.distance_to(to) < 1e-3);
    }
    let bez = WireShape::new(WireRouting::Bezier, Vec2::new(0.0, 0.0), Vec2::new(100.0, 50.0));
    let pts = bez.flatten(16);
    assert_eq!(pts.len(), 17);
    assert!(pts[0].distance(Vec2::ZERO) < 1e-4 && pts[16].distance(Vec2::new(100.0, 50.0)) < 1e-4);
}

#[test]
fn move_updates_wires_and_index_incrementally() {
    let mut g = clean_scene();
    let mut s = scene_for(&g);
    let wires_before = s.wires.len();
    let cs: ChangeSet = GraphCommand::MoveNode { id: "gt".into(), x: 2000.0, y: 1500.0 }.apply(&mut g).unwrap();
    s.apply_changes(&g, &cs, &SamplePresenter, &FixedAdvance::default());
    assert_eq!(s.wires.len(), wires_before);
    let gt = s.node_index("gt").unwrap();
    assert_eq!(s.nodes[gt].pos, Vec2::new(2000.0, 1500.0));
    // spatial index follows
    assert!(s.nodes_in(Rect::around(Vec2::new(2050.0, 1520.0), 20.0)).contains(&gt));
    assert!(!s.nodes_in(Rect::around(Vec2::new(30.0, 220.0), 20.0)).contains(&gt));
    // wires attached to the moved node follow its pins
    for &wi in s.wires_of(gt) {
        let w = &s.wires[wi];
        let end = if w.from_node == gt { w.shape.pts[0] } else { w.shape.pts[w.shape.n - 1] };
        let pin = if w.from_node == gt { s.nodes[gt].pin_pos(PinSide::Output, w.from_pin) } else { s.nodes[gt].pin_pos(PinSide::Input, w.to_pin) };
        assert!(end.distance(pin.unwrap()) < 1e-3);
    }
    // undo restores
    cs.undo(&mut g);
    assert_eq!(g.nodes["gt"].position.x, 0.0);
}

#[test]
fn connect_and_comment_commands_update_scene() {
    let mut g = clean_scene();
    let mut s = scene_for(&g);
    let before = s.wires.len();
    let cs = GraphCommand::Connect(graphy::Connection::data("speed", "out_Value", "gt", "in_B")).apply(&mut g).unwrap();
    s.apply_changes(&g, &cs, &SamplePresenter, &FixedAdvance::default());
    assert_eq!(s.wires.len(), before + 1);
    let gt = s.node("gt").unwrap();
    assert!(gt.inputs[1].connected);

    let cs = GraphCommand::MoveComment { index: 0, x: 10.0, y: 20.0 }.apply(&mut g).unwrap();
    s.apply_changes(&g, &cs, &SamplePresenter, &FixedAdvance::default());
    assert_eq!(s.comments[0].rect.min, Vec2::new(10.0, 20.0));
    let cs2 = GraphCommand::ResizeComment { index: 0, width: 300.0, height: 200.0 }.apply(&mut g).unwrap();
    s.apply_changes(&g, &cs2, &SamplePresenter, &FixedAdvance::default());
    assert_eq!(s.comments[0].rect.size(), Vec2::new(300.0, 200.0));
    cs2.undo(&mut g);
    cs.undo(&mut g);
    assert_eq!(g.comments[0].position.x, -250.0);
    assert!(GraphCommand::MoveComment { index: 9, x: 0.0, y: 0.0 }.apply(&mut g).is_err());
}

#[test]
fn culling_drops_offscreen_work() {
    let g = crowded_scene(30, 20);
    let s = scene_for(&g);
    assert!(s.nodes.len() > 600);
    let vp = Vec2::new(1280.0, 800.0);
    let near = DrawParams::new(vp, ViewTransform { pan: Vec2::new(0.0, 0.0), zoom: 1.0 });
    let d = s.build_draw_data(&near, &InteractionState::default(), &FixedAdvance::default());
    assert!(d.stats.nodes < s.nodes.len() / 4, "visible {} of {}", d.stats.nodes, s.nodes.len());
    assert!(d.stats.nodes_culled + d.stats.nodes == s.nodes.len());
    assert!(d.stats.wires_culled > 0);

    // frame everything: far more nodes, but overview LOD emits no text or pins
    let all = ViewTransform::framing(s.bounds().unwrap(), vp, 20.0, 1.0);
    let far = s.build_draw_data(&DrawParams::new(vp, all), &InteractionState::default(), &FixedAdvance::default());
    assert_eq!(far.lod, LodTier::Overview);
    assert!(far.texts.is_empty() && far.pins.is_empty() && far.tails.is_empty());
    assert!(far.nodes.iter().all(|b| b.flags & flags::FLAT != 0));
}

#[test]
fn selection_emphasises_touching_wires_and_dims_the_rest() {
    let g = clean_scene();
    let s = scene_for(&g);
    let vp = Vec2::new(1600.0, 900.0);
    let params = DrawParams::new(vp, ViewTransform { pan: Vec2::new(300.0, 100.0), zoom: 1.0 });
    let rest = s.build_draw_data(&params, &InteractionState::default(), &FixedAdvance::default());
    assert!(rest.wires.iter().all(|w| w.flags & (flags::DIM | flags::EMPHASIS) == 0));

    let mut st = InteractionState::default();
    st.selected_nodes.insert("branch".into());
    let d = s.build_draw_data(&params, &st, &FixedAdvance::default());
    let emph = d.wires.iter().filter(|w| w.flags & flags::EMPHASIS != 0).count();
    let dim = d.wires.iter().filter(|w| w.flags & flags::DIM != 0).count();
    assert_eq!(emph, 4); // begin->branch, branch->print, branch->off, gt->branch
    assert!(dim > 0);
    // draw order: dim first, emphasised last
    let last_dim = d.wires.iter().rposition(|w| w.flags & flags::DIM != 0).unwrap();
    let first_emph = d.wires.iter().position(|w| w.flags & flags::EMPHASIS != 0).unwrap();
    assert!(last_dim < first_emph);
    // selected node carries a ring
    let b = d.nodes.iter().find(|b| b.flags & flags::SELECTED != 0).unwrap();
    assert!(b.ring[3] > 0.0);
}

#[test]
fn state_flags_and_preview_slots_are_emitted() {
    let g = clean_scene();
    let s = scene_for(&g);
    let params = DrawParams::new(Vec2::new(1800.0, 900.0), ViewTransform { pan: Vec2::new(300.0, 100.0), zoom: 1.0 });
    let mut st = InteractionState::default();
    st.focused_node = Some("print".into());
    let d = s.build_draw_data(&params, &st, &FixedAdvance::default());
    assert!(d.nodes.iter().any(|b| b.flags & flags::ACTIVE != 0));
    assert!(d.nodes.iter().any(|b| b.flags & flags::DISABLED != 0));
    assert!(d.nodes.iter().any(|b| b.flags & flags::FOCUSED != 0));
    assert_eq!(d.previews.len(), 1);
    assert_eq!(d.previews[0].node, "tex");
    assert!(d.nodes.iter().any(|b| b.kind == BoxKind::Well));
    // pin labels are truncated to fit, never overlapping the opposite column
    assert!(d.texts.iter().any(|t| t.text == "Condition"));
    // low zoom hides previews and labels but keeps titles
    let mid = DrawParams::new(Vec2::new(1800.0, 900.0), ViewTransform { pan: Vec2::new(300.0, 100.0), zoom: 0.5 });
    let m = s.build_draw_data(&mid, &InteractionState::default(), &FixedAdvance::default());
    assert_eq!(m.lod, LodTier::Summary);
    assert!(m.previews.is_empty());
    assert!(!m.texts.iter().any(|t| t.text == "Condition"));
}

#[test]
fn marquee_modes() {
    let g = clean_scene();
    let s = scene_for(&g);
    let b = s.node("branch").unwrap().rect();
    let partial = Rect::from_pos_size(b.min + Vec2::new(10.0, 10.0), Vec2::new(30.0, 30.0));
    assert!(s.marquee_nodes(partial, false).contains(&s.node_index("branch").unwrap()));
    assert!(!s.marquee_nodes(partial, true).contains(&s.node_index("branch").unwrap()));
    assert!(s.marquee_nodes(b.expand(2.0), true).contains(&s.node_index("branch").unwrap()));
}

#[test]
fn fit_text_truncates_with_ellipsis() {
    let m = FixedAdvance::default();
    assert_eq!(fit_text(&m, "short", 10.0, 100.0), "short");
    let t = fit_text(&m, "a very long label indeed", 10.0, 60.0);
    assert!(t.ends_with('…'));
    assert!(m.width(&t, 10.0) <= 60.0 + 1e-3);
    assert_eq!(fit_text(&m, "abc", 10.0, 3.0), "");
}
