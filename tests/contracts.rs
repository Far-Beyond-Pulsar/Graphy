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
