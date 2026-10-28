//! Tests for the document contracts: IDs, invariants, type resolution,
//! diagnostics, dynamic pins and deterministic/undoable edits.

use graphy::contracts::invariants::codes;
use graphy::{
    check_connection_types, check_invariants, ChainResolver, Compatibility, Connection,
    ConnectionType, DataType, GraphCommand, GraphDescription, IdAllocator, NodeInstance, Pin,
    PinInstance, PinType, Position, Severity, StructuralResolver, TypeResolver,
};

fn pin(id: &str, ty: DataType, dir: PinType) -> PinInstance {
    PinInstance::new(id, Pin::new(id, id, ty, dir))
}

fn node(id: &str, inputs: Vec<PinInstance>, outputs: Vec<PinInstance>) -> NodeInstance {
    let mut n = NodeInstance::new(id, "test", Position { x: 0.0, y: 0.0 });
    n.inputs = inputs;
    n.outputs = outputs;
    n
}

fn sample() -> GraphDescription {
    let mut g = GraphDescription::new("t");
    g.add_node(node(
        "a",
        vec![],
        vec![
            pin("then", DataType::Exec, PinType::Output),
            pin("val", DataType::typed("i64"), PinType::Output),
        ],
    ));
    g.add_node(node(
        "b",
        vec![
            pin("exec", DataType::Exec, PinType::Input),
            pin("x", DataType::typed("i64"), PinType::Input),
        ],
        vec![],
    ));
    g.add_connection(Connection::new("a", "then", "b", "exec", ConnectionType::Execution));
    g.add_connection(Connection::new("a", "val", "b", "x", ConnectionType::Data));
    g
}

fn codes_of(g: &GraphDescription) -> Vec<String> {
    check_invariants(g)
        .into_iter()
        .map(|d| d.code.unwrap())
        .collect()
}

fn snapshot(g: &GraphDescription) -> String {
    let mut ids: Vec<_> = g.nodes.keys().cloned().collect();
    ids.sort();
    let nodes: Vec<_> = ids
        .iter()
        .map(|i| {
            let n = &g.nodes[i];
            format!(
                "{}@{},{} in={:?} out={:?}",
                n.id,
                n.position.x,
                n.position.y,
                n.inputs.iter().map(|p| &p.id).collect::<Vec<_>>(),
                n.outputs.iter().map(|p| &p.id).collect::<Vec<_>>()
            )
        })
        .collect();
    let conns: Vec<_> = g
        .connections
        .iter()
        .map(|c| format!("{}.{}>{}.{}", c.source_node, c.source_pin, c.target_node, c.target_pin))
        .collect();
    format!("{nodes:?}{conns:?}")
}

#[test]
fn valid_graph_has_no_violations() {
    assert!(check_invariants(&sample()).is_empty());
}

#[test]
fn endpoint_integrity_violations_are_reported_with_stable_codes() {
    let mut g = sample();
    g.add_connection(Connection::new("a", "val", "ghost", "x", ConnectionType::Data));
    g.add_connection(Connection::new("a", "nope", "b", "x", ConnectionType::Data));
    // wrong direction: connect from an input pin
    g.add_connection(Connection::new("b", "x", "b", "exec", ConnectionType::Data));
    let found = codes_of(&g);
    assert!(found.contains(&codes::MISSING_NODE.to_string()));
    assert!(found.contains(&codes::MISSING_PIN.to_string()));
    assert!(found.contains(&codes::WRONG_DIRECTION.to_string()));
    // deterministic: same graph, same report
    assert_eq!(codes_of(&g), found);
}

#[test]
fn duplicate_pins_keys_kinds_and_drivers_are_caught() {
    let mut g = sample();
    g.nodes
        .get_mut("b")
        .unwrap()
        .inputs
        .push(pin("x", DataType::typed("i64"), PinType::Input));
    g.nodes.get_mut("a").unwrap().id = "renamed".into();
    g.add_connection(Connection::new("a", "val", "b", "x", ConnectionType::Data));
    g.add_connection(Connection::new("a", "then", "b", "x", ConnectionType::Data));
    let found = codes_of(&g);
    for expected in [
        codes::DUPLICATE_PIN,
        codes::NODE_KEY_MISMATCH,
        codes::DUPLICATE_CONNECTION,
        codes::MULTIPLE_INPUT_DRIVERS,
        codes::KIND_MISMATCH,
    ] {
        assert!(found.contains(&expected.to_string()), "missing {expected}: {found:?}");
    }
}

#[test]
fn id_allocation_is_deterministic_and_never_reuses_live_ids() {
    let mut g = sample();
    g.add_node(node("n_7", vec![], vec![]));
    let mut a = IdAllocator::for_graph(&g, "n");
    let mut b = IdAllocator::for_graph(&g, "n");
    assert_eq!(a.allocate(), b.allocate());
    assert_eq!(a.allocate().as_str(), "n_9");
    assert!(!g.nodes.contains_key(a.allocate().as_str()));
}

#[test]
fn edits_are_deterministic_and_undo_restores_exact_state() {
    let mut g = sample();
    g.add_node(node(
        "c",
        vec![pin("x", DataType::typed("i64"), PinType::Input)],
        vec![],
    ));
    let before = snapshot(&g);

    // removing `a` drops both connections; undo restores them in order
    let cs = GraphCommand::RemoveNode { id: "a".into() }.apply(&mut g).unwrap();
    assert_eq!(g.connections.len(), 0);
    assert_eq!(cs.touched_nodes(), vec!["a", "b"]);
    cs.undo(&mut g);
    assert_eq!(snapshot(&g), before);
    assert!(check_invariants(&g).is_empty());

    // replaying the same commands on a clone yields the same graph
    let script = [
        GraphCommand::MoveNode { id: "c".into(), x: 5.0, y: 6.0 },
        GraphCommand::Connect(Connection::new("a", "val", "c", "x", ConnectionType::Data)),
        GraphCommand::SetProperty { node: "c".into(), key: "k".into(), value: Some(1.into()) },
    ];
    let mut g2 = g.clone();
    let mut sets = Vec::new();
    for cmd in &script {
        sets.push(cmd.apply(&mut g).unwrap());
        cmd.apply(&mut g2).unwrap();
    }
    assert_eq!(snapshot(&g), snapshot(&g2));
    for cs in sets.iter().rev() {
        cs.undo(&mut g);
    }
    assert_eq!(snapshot(&g), before);
    assert!(g.nodes["c"].properties.is_empty());
}

#[test]
fn failed_edits_leave_the_graph_untouched() {
    let mut g = sample();
    let before = snapshot(&g);
    for cmd in [
        GraphCommand::AddNode(node("a", vec![], vec![])),
        GraphCommand::RemoveNode { id: "zzz".into() },
        GraphCommand::Connect(Connection::new("a", "val", "b", "x", ConnectionType::Data)),
        GraphCommand::Connect(Connection::new("a", "then", "b", "x", ConnectionType::Execution)),
        GraphCommand::Disconnect(Connection::new("a", "val", "b", "exec", ConnectionType::Data)),
    ] {
        assert!(cmd.apply(&mut g).is_err());
    }
    assert_eq!(snapshot(&g), before);
}

#[test]
fn dynamic_pin_changes_drop_dangling_connections_and_undo_restores_them() {
    let mut g = sample();
    let before = snapshot(&g);
    let keep_exec = vec![pin("exec", DataType::Exec, PinType::Input)];
    let cs = GraphCommand::SetPins {
        node: "b".into(),
        inputs: keep_exec,
        outputs: vec![pin("extra", DataType::typed("f32"), PinType::Output)],
    }
    .apply(&mut g)
    .unwrap();
    assert_eq!(g.connections.len(), 1, "data connection to removed pin dropped");
    assert!(check_invariants(&g).is_empty());
    cs.undo(&mut g);
    assert_eq!(snapshot(&g), before);

    let dup = vec![
        pin("p", DataType::Exec, PinType::Input),
        pin("p", DataType::Exec, PinType::Input),
    ];
    assert!(GraphCommand::SetPins { node: "b".into(), inputs: dup, outputs: vec![] }
        .apply(&mut g)
        .is_err());
}

struct WgslOnly;
impl TypeResolver for WgslOnly {
    fn compatibility(&self, from: &DataType, to: &DataType) -> Compatibility {
        match (from, to) {
            (DataType::Data(a), DataType::Data(b))
                if a.type_string.starts_with("vec") && b.type_string.starts_with("vec") =>
            {
                if a.type_string == b.type_string {
                    Compatibility::Exact
                } else {
                    Compatibility::Incompatible(format!("{} != {}", a.type_string, b.type_string))
                }
            }
            _ => Compatibility::Unknown,
        }
    }
}

#[test]
fn type_resolution_composes_domain_resolvers_and_reports_diagnostics() {
    let s = StructuralResolver;
    assert_eq!(
        s.compatibility(&DataType::typed("i64"), &DataType::any()),
        Compatibility::Wildcard
    );
    assert!(matches!(
        s.compatibility(&DataType::Exec, &DataType::typed("i64")),
        Compatibility::Incompatible(_)
    ));
    assert_eq!(
        s.compatibility(&DataType::typed("i64"), &DataType::typed("f32")),
        Compatibility::Unknown
    );

    let mut g = GraphDescription::new("t");
    g.add_node(node("a", vec![], vec![pin("o", DataType::typed("vec3f"), PinType::Output)]));
    g.add_node(node(
        "b",
        vec![
            pin("i", DataType::typed("vec4f"), PinType::Input),
            pin("j", DataType::typed("Mystery"), PinType::Input),
        ],
        vec![],
    ));
    g.add_connection(Connection::new("a", "o", "b", "i", ConnectionType::Data));

    let chain = ChainResolver::new().with(WgslOnly).with(StructuralResolver);
    let diags = check_connection_types(&g, &chain);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
    assert_eq!(diags[0].code.as_deref(), Some(codes::TYPE_MISMATCH));

    // an unknown type yields a warning, not an error
    g.connections.clear();
    g.add_connection(Connection::new("a", "o", "b", "j", ConnectionType::Data));
    let diags = check_connection_types(&g, &chain);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Warning);
    assert_eq!(diags[0].code.as_deref(), Some(codes::TYPE_UNKNOWN));
}

#[test]
fn graph_description_serialization_preserves_ids_and_connection_order() {
    let g = sample();
    let json = serde_json::to_string(&g).unwrap();
    let back: GraphDescription = serde_json::from_str(&json).unwrap();
    assert_eq!(snapshot(&g), snapshot(&back));
}

#[test]
fn scoped_resolver_confines_a_domain_and_blocks_cross_domain_connections() {
    use graphy::ScopedResolver;
    struct Same;
    impl TypeResolver for Same {
        fn compatibility(&self, a: &DataType, b: &DataType) -> Compatibility {
            if a == b {
                Compatibility::Exact
            } else {
                Compatibility::Incompatible("different".into())
            }
        }
    }
    let r = ScopedResolver::new("wgsl", Same);
    let t = |s: &str| DataType::typed(s);
    assert_eq!(r.compatibility(&t("wgsl::f32"), &t("wgsl::f32")), Compatibility::Exact);
    assert!(matches!(
        r.compatibility(&t("wgsl::f32"), &t("wgsl::vec3f")),
        Compatibility::Incompatible(_)
    ));
    assert!(matches!(
        r.compatibility(&t("wgsl::f32"), &t("f32")),
        Compatibility::Incompatible(_)
    ));
    assert_eq!(r.compatibility(&t("f32"), &t("f64")), Compatibility::Unknown);
    assert_eq!(r.compatibility(&DataType::Exec, &DataType::Exec), Compatibility::Unknown);
}

#[test]
fn comment_add_edit_remove_undo_restores_exact_document() {
    use graphy::core::GraphComment;
    use graphy::{GraphCommand, Position};

    let mut graph = GraphDescription::new("t");
    let mut first = GraphComment::new("one", Position::new(1.0, 2.0), (10.0, 20.0));
    first.id = "c1".into();
    graph.comments.push(first);
    let before = graph.comments.clone();

    let mut second = GraphComment::new("two", Position::new(5.0, 6.0), (30.0, 40.0));
    second.id = "c2".into();
    let mut applied = Vec::new();
    for cmd in [
        GraphCommand::AddComment { index: 0, comment: second },
        GraphCommand::EditComment {
            index: 1,
            text: Some("renamed".into()),
            color: Some([1.0, 0.0, 0.0, 1.0]),
            contained_node_ids: Some(vec!["n".into()]),
        },
        GraphCommand::RemoveComment { index: 0 },
    ] {
        applied.push(cmd.apply(&mut graph).expect("edit applies"));
    }
    assert_eq!(graph.comments.len(), 1);
    assert_eq!(graph.comments[0].text, "renamed");
    assert_eq!(graph.comments[0].id, "c1");

    for changes in applied.iter().rev() {
        changes.undo(&mut graph);
    }
    assert_eq!(graph.comments.len(), before.len());
    assert_eq!(graph.comments[0].id, "c1");
    assert_eq!(graph.comments[0].text, "one");
    assert_eq!(graph.comments[0].color, before[0].color);
    assert!(graph.comments[0].contained_node_ids.is_empty());
}

#[test]
fn diff_commands_turn_one_graph_into_the_other_and_undo_back() {
    use graphy::core::GraphComment;
    use graphy::{diff_commands, Connection, DataType, GraphCommand, NodeInstance, Position};

    let node = |id: &str, ty: &str, x: f64| {
        let mut n = NodeInstance::new(id, ty, Position::new(x, 0.0));
        n.add_input_pin("i", DataType::Exec);
        n.add_output_pin("o", DataType::Exec);
        n
    };
    let mut before = GraphDescription::new("g");
    for n in [node("a", "t", 0.0), node("b", "t", 10.0), node("c", "t", 20.0)] {
        before.add_node(n);
    }
    before.add_connection(Connection::execution("a", "o", "b", "i"));
    before.add_connection(Connection::execution("b", "o", "c", "i"));

    let mut after = before.clone();
    after.nodes.get_mut("a").unwrap().position = Position::new(5.0, 5.0);
    after.nodes.remove("c");
    after.connections.retain(|c| c.target_node != "c");
    after.add_node(node("d", "u", 30.0));
    after.add_connection(Connection::execution("b", "o", "d", "i"));
    after.comments.push(GraphComment::new("hi", Position::zero(), (5.0, 5.0)));
    // "b" changes type, so it is rebuilt and its edges must be restored.
    after.nodes.get_mut("b").unwrap().node_type = "other".into();

    let cmds = diff_commands(&before, &after);
    let mut graph = before.clone();
    let mut applied = Vec::new();
    for c in &cmds {
        applied.push(c.apply(&mut graph).expect("diff command applies"));
    }
    let edges = |g: &GraphDescription| {
        let mut e: Vec<_> = g
            .connections
            .iter()
            .map(|c| format!("{}.{}->{}.{}", c.source_node, c.source_pin, c.target_node, c.target_pin))
            .collect();
        e.sort();
        e
    };
    let mut ids: Vec<_> = graph.nodes.keys().cloned().collect();
    ids.sort();
    assert_eq!(ids, ["a", "b", "d"]);
    assert_eq!(graph.nodes["b"].node_type, "other");
    assert_eq!(graph.nodes["a"].position.x, 5.0);
    assert_eq!(edges(&graph), edges(&after));
    assert_eq!(graph.comments.len(), 1);

    for cs in applied.iter().rev() {
        cs.undo(&mut graph);
    }
    assert_eq!(edges(&graph), edges(&before));
    assert_eq!(graph.nodes.len(), 3);
    assert!(graph.comments.is_empty());
    let _ = GraphCommand::RemoveNode { id: String::new() };
}
