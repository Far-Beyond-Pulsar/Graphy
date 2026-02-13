//! Tests for JSON serialization and deserialization of all core types.

use graphy::*;

// ===========================================================================
// DataType serialization
// ===========================================================================

#[test]
fn serde_datatype_execution() {
    let dt = DataType::Execution;
    let json = serde_json::to_string(&dt).unwrap();
    let deserialized: DataType = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, DataType::Execution);
}

#[test]
fn serde_datatype_typed() {
    let dt = DataType::Typed("Vec<f64>".into());
    let json = serde_json::to_string(&dt).unwrap();
    let deserialized: DataType = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, dt);
}

#[test]
fn serde_datatype_all_legacy_variants() {
    for dt in [
        DataType::Number,
        DataType::String,
        DataType::Boolean,
        DataType::Vector2,
        DataType::Vector3,
        DataType::Color,
        DataType::Any,
    ] {
        let json = serde_json::to_string(&dt).unwrap();
        let deserialized: DataType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, dt);
    }
}

// ===========================================================================
// NodeTypes serialization
// ===========================================================================

#[test]
fn serde_nodetypes_pure() {
    let nt = NodeTypes::pure;
    let json = serde_json::to_string(&nt).unwrap();
    assert!(json.contains("pure"));
    let deserialized: NodeTypes = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, NodeTypes::pure);
}

#[test]
fn serde_nodetypes_fn() {
    let nt = NodeTypes::fn_;
    let json = serde_json::to_string(&nt).unwrap();
    assert!(json.contains("fn"));
    let deserialized: NodeTypes = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, NodeTypes::fn_);
}

#[test]
fn serde_nodetypes_control_flow() {
    let nt = NodeTypes::control_flow;
    let json = serde_json::to_string(&nt).unwrap();
    assert!(json.contains("control_flow"));
    let deserialized: NodeTypes = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, NodeTypes::control_flow);
}

#[test]
fn serde_nodetypes_event() {
    let nt = NodeTypes::event;
    let json = serde_json::to_string(&nt).unwrap();
    assert!(json.contains("event"));
    let deserialized: NodeTypes = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, NodeTypes::event);
}

// ===========================================================================
// PropertyValue serialization
// ===========================================================================

#[test]
fn serde_property_string() {
    let pv = PropertyValue::String("hello world".into());
    let json = serde_json::to_string(&pv).unwrap();
    let deserialized: PropertyValue = serde_json::from_str(&json).unwrap();
    match deserialized {
        PropertyValue::String(s) => assert_eq!(s, "hello world"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn serde_property_number() {
    let pv = PropertyValue::Number(3.1415926);
    let json = serde_json::to_string(&pv).unwrap();
    let deserialized: PropertyValue = serde_json::from_str(&json).unwrap();
    match deserialized {
        PropertyValue::Number(n) => assert!((n - 3.1415926).abs() < f64::EPSILON),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn serde_property_boolean() {
    let pv = PropertyValue::Boolean(true);
    let json = serde_json::to_string(&pv).unwrap();
    let deserialized: PropertyValue = serde_json::from_str(&json).unwrap();
    match deserialized {
        PropertyValue::Boolean(b) => assert!(b),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn serde_property_vector2() {
    let pv = PropertyValue::Vector2(1.5, -2.5);
    let json = serde_json::to_string(&pv).unwrap();
    let deserialized: PropertyValue = serde_json::from_str(&json).unwrap();
    match deserialized {
        PropertyValue::Vector2(x, y) => {
            assert_eq!(x, 1.5);
            assert_eq!(y, -2.5);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn serde_property_vector3() {
    let pv = PropertyValue::Vector3(1.0, 2.0, 3.0);
    let json = serde_json::to_string(&pv).unwrap();
    let deserialized: PropertyValue = serde_json::from_str(&json).unwrap();
    match deserialized {
        PropertyValue::Vector3(x, y, z) => {
            assert_eq!(x, 1.0);
            assert_eq!(y, 2.0);
            assert_eq!(z, 3.0);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn serde_property_color() {
    let pv = PropertyValue::Color(1.0, 0.5, 0.0, 0.8);
    let json = serde_json::to_string(&pv).unwrap();
    let deserialized: PropertyValue = serde_json::from_str(&json).unwrap();
    match deserialized {
        PropertyValue::Color(r, g, b, a) => {
            assert_eq!(r, 1.0);
            assert_eq!(g, 0.5);
            assert_eq!(b, 0.0);
            assert_eq!(a, 0.8);
        }
        _ => panic!("wrong variant"),
    }
}

// ===========================================================================
// Position serialization
// ===========================================================================

#[test]
fn serde_position() {
    let pos = Position::new(100.5, -200.3);
    let json = serde_json::to_string(&pos).unwrap();
    let deserialized: Position = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.x, 100.5);
    assert_eq!(deserialized.y, -200.3);
}

// ===========================================================================
// Connection serialization
// ===========================================================================

#[test]
fn serde_connection_data() {
    let conn = Connection::data("src", "out", "tgt", "in");
    let json = serde_json::to_string(&conn).unwrap();
    let deserialized: Connection = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.source_node, "src");
    assert_eq!(deserialized.target_node, "tgt");
    assert_eq!(deserialized.connection_type, ConnectionType::Data);
}

#[test]
fn serde_connection_execution() {
    let conn = Connection::execution("a", "exec_out", "b", "exec_in");
    let json = serde_json::to_string(&conn).unwrap();
    let deserialized: Connection = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.connection_type, ConnectionType::Execution);
}

// ===========================================================================
// NodeInstance serialization
// ===========================================================================

#[test]
fn serde_node_instance_basic() {
    let mut node = NodeInstance::new("node_1", "add", Position::new(10.0, 20.0));
    node.add_input_pin("a", DataType::Typed("i64".into()));
    node.add_output_pin("result", DataType::Typed("i64".into()));
    node.set_property("a", PropertyValue::Number(5.0));

    let json = serde_json::to_string(&node).unwrap();
    let deserialized: NodeInstance = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, "node_1");
    assert_eq!(deserialized.node_type, "add");
    assert_eq!(deserialized.position.x, 10.0);
    assert_eq!(deserialized.inputs.len(), 1);
    assert_eq!(deserialized.outputs.len(), 1);
    assert!(deserialized.get_property("a").is_some());
}

// ===========================================================================
// Full GraphDescription serialization round-trip
// ===========================================================================

#[test]
fn serde_full_graph_round_trip() {
    let mut graph = GraphDescription::new("test_graph");
    graph.metadata.description = "A serialization test".to_string();

    // Add nodes
    let mut n1 = NodeInstance::new("add_1", "add", Position::new(0.0, 0.0));
    n1.add_input_pin("a", DataType::Typed("i64".into()));
    n1.add_input_pin("b", DataType::Typed("i64".into()));
    n1.add_output_pin("result", DataType::Typed("i64".into()));
    n1.set_property("a", PropertyValue::Number(10.0));
    n1.set_property("b", PropertyValue::Number(20.0));
    graph.add_node(n1);

    let mut n2 = NodeInstance::new("print_1", "print", Position::new(200.0, 0.0));
    n2.add_input_pin("exec_in", DataType::Execution);
    n2.add_input_pin("message", DataType::Typed("String".into()));
    n2.add_output_pin("exec_out", DataType::Execution);
    n2.set_property("message", PropertyValue::String("hello".into()));
    graph.add_node(n2);

    // Add connections
    graph.add_connection(Connection::data("add_1", "result", "print_1", "message"));
    graph.add_connection(Connection::execution("event", "exec", "print_1", "exec_in"));

    // Add comments
    graph.comments.push(core::GraphComment {
        text: "Math section".into(),
        position: Position::zero(),
        size: (200.0, 100.0),
    });

    // Serialize
    let json = serde_json::to_string_pretty(&graph).unwrap();
    assert!(!json.is_empty());

    // Deserialize
    let deserialized: GraphDescription = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.metadata.name, "test_graph");
    assert_eq!(deserialized.metadata.description, "A serialization test");
    assert_eq!(deserialized.nodes.len(), 2);
    assert_eq!(deserialized.connections.len(), 2);
    assert_eq!(deserialized.comments.len(), 1);

    // Verify node content
    let add = deserialized.get_node("add_1").unwrap();
    assert_eq!(add.node_type, "add");
    assert_eq!(add.inputs.len(), 2);
    assert_eq!(add.outputs.len(), 1);
}

#[test]
fn serde_empty_graph_round_trip() {
    let graph = GraphDescription::new("empty");
    let json = serde_json::to_string(&graph).unwrap();
    let deserialized: GraphDescription = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.metadata.name, "empty");
    assert!(deserialized.nodes.is_empty());
    assert!(deserialized.connections.is_empty());
    assert!(deserialized.comments.is_empty());
}

// ===========================================================================
// NodeMetadata serialization
// ===========================================================================

#[test]
fn serde_node_metadata() {
    let meta = NodeMetadata::new("branch", NodeTypes::control_flow, "flow")
        .with_params(vec![ParamInfo::new("condition", "bool")])
        .with_return_type("bool")
        .with_exec_outputs(vec!["True".to_string(), "False".to_string()])
        .with_imports(vec!["use std::fmt;".to_string()])
        .with_source("fn branch(condition: bool) {}");

    let json = serde_json::to_string(&meta).unwrap();
    let deserialized: NodeMetadata = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.name, "branch");
    assert_eq!(deserialized.node_type, NodeTypes::control_flow);
    assert_eq!(deserialized.category, "flow");
    assert_eq!(deserialized.params.len(), 1);
    assert!(deserialized.return_type.is_some());
    assert_eq!(deserialized.exec_outputs, vec!["True", "False"]);
    assert_eq!(deserialized.imports.len(), 1);
    assert!(!deserialized.function_source.is_empty());
}

#[test]
fn serde_param_info() {
    let param = ParamInfo::new("value", "f64");
    let json = serde_json::to_string(&param).unwrap();
    let deserialized: ParamInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.name, "value");
    assert_eq!(deserialized.param_type, "f64");
}

// ===========================================================================
// Large graph serialization
// ===========================================================================

#[test]
fn serde_large_graph() {
    let mut graph = GraphDescription::new("large");

    for i in 0..100 {
        let mut node = NodeInstance::new(format!("n_{}", i), "add", Position::new(i as f64, 0.0));
        node.add_input_pin("a", DataType::Typed("i64".into()));
        node.add_output_pin("result", DataType::Typed("i64".into()));
        node.set_property("a", PropertyValue::Number(i as f64));
        graph.add_node(node);
    }

    for i in 0..99 {
        graph.add_connection(Connection::data(
            format!("n_{}", i),
            "result",
            format!("n_{}", i + 1),
            "a",
        ));
    }

    let json = serde_json::to_string(&graph).unwrap();
    let deserialized: GraphDescription = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.nodes.len(), 100);
    assert_eq!(deserialized.connections.len(), 99);
}
