//! Edge case tests: empty graphs, disconnected nodes, large graphs,
//! special characters, boundary conditions.

mod common;

use common::*;
use graphy::*;

// ===========================================================================
// Empty / minimal graphs
// ===========================================================================

#[test]
fn edge_empty_graph_data_resolver() {
    let graph = GraphDescription::new("empty");
    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();
    assert!(resolver.get_pure_evaluation_order().is_empty());
}

#[test]
fn edge_empty_graph_exec_routing() {
    let graph = GraphDescription::new("empty");
    let routing = ExecutionRouting::build_from_graph(&graph);
    assert!(!routing.has_execution_outputs("any"));
}

#[test]
fn edge_single_node_no_connections() {
    let mut graph = GraphDescription::new("single");
    let provider = TestMetadataProvider::with_math_nodes();

    let mut node = NodeInstance::new("lonely", "add", Position::zero());
    node.add_input_pin("a", DataType::Typed("i64".into()));
    node.add_input_pin("b", DataType::Typed("i64".into()));
    node.add_output_pin("result", DataType::Typed("i64".into()));
    node.set_property("a", PropertyValue::Number(1.0));
    node.set_property("b", PropertyValue::Number(2.0));
    graph.add_node(node);

    let resolver = DataResolver::build(&graph, &provider).unwrap();
    assert_eq!(resolver.get_pure_evaluation_order().len(), 1);
    assert!(resolver.get_result_variable("lonely").is_some());
}

// ===========================================================================
// Special characters in identifiers
// ===========================================================================

#[test]
fn edge_special_chars_in_node_id() {
    let mut graph = GraphDescription::new("test");

    let mut node = NodeInstance::new("node-with.special@chars!", "add", Position::zero());
    node.add_input_pin("a", DataType::Typed("i64".into()));
    node.set_property("a", PropertyValue::Number(1.0));
    graph.add_node(node);

    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    // Should still generate a valid variable name
    let var = resolver.get_result_variable("node-with.special@chars!");
    assert!(var.is_some());
    let var_name = var.unwrap();
    // Variable name should be sanitized
    assert!(!var_name.contains('-'));
    assert!(!var_name.contains('.'));
    assert!(!var_name.contains('@'));
}

#[test]
fn edge_unicode_node_id() {
    let mut graph = GraphDescription::new("unicode");

    let node = NodeInstance::new("узел_1", "add", Position::zero());
    graph.add_node(node);

    assert!(graph.get_node("узел_1").is_some());
}

#[test]
fn edge_empty_string_node_id() {
    let mut graph = GraphDescription::new("test");
    let node = NodeInstance::new("", "add", Position::zero());
    graph.add_node(node);

    // Should be retrievable
    assert!(graph.get_node("").is_some());
}

// ===========================================================================
// Disconnected subgraphs
// ===========================================================================

#[test]
fn edge_disconnected_subgraphs() {
    let mut graph = GraphDescription::new("disconnected");
    let provider = TestMetadataProvider::with_math_nodes();

    // Subgraph 1: a -> b
    let mut a = NodeInstance::new("a", "add", Position::zero());
    a.add_input_pin("a", DataType::Typed("i64".into()));
    a.add_input_pin("b", DataType::Typed("i64".into()));
    a.add_output_pin("result", DataType::Typed("i64".into()));
    a.set_property("a", PropertyValue::Number(1.0));
    a.set_property("b", PropertyValue::Number(2.0));
    graph.add_node(a);

    let mut b = NodeInstance::new("b", "add", Position::zero());
    b.add_input_pin("a", DataType::Typed("i64".into()));
    b.add_input_pin("b", DataType::Typed("i64".into()));
    b.add_output_pin("result", DataType::Typed("i64".into()));
    b.set_property("b", PropertyValue::Number(3.0));
    graph.add_node(b);

    graph.add_connection(Connection::data("a", "result", "b", "a"));

    // Subgraph 2: c -> d (completely disconnected)
    let mut c = NodeInstance::new("c", "multiply", Position::new(500.0, 0.0));
    c.add_input_pin("a", DataType::Typed("i64".into()));
    c.add_input_pin("b", DataType::Typed("i64".into()));
    c.add_output_pin("result", DataType::Typed("i64".into()));
    c.set_property("a", PropertyValue::Number(4.0));
    c.set_property("b", PropertyValue::Number(5.0));
    graph.add_node(c);

    let mut d = NodeInstance::new("d", "multiply", Position::new(700.0, 0.0));
    d.add_input_pin("a", DataType::Typed("i64".into()));
    d.add_input_pin("b", DataType::Typed("i64".into()));
    d.add_output_pin("result", DataType::Typed("i64".into()));
    d.set_property("b", PropertyValue::Number(6.0));
    graph.add_node(d);

    graph.add_connection(Connection::data("c", "result", "d", "a"));

    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let order = resolver.get_pure_evaluation_order();
    assert_eq!(order.len(), 4);

    // Both subgraph orderings should be valid
    let pos = |id: &str| order.iter().position(|x| x == id).unwrap();
    assert!(pos("a") < pos("b"));
    assert!(pos("c") < pos("d"));
}

// ===========================================================================
// Large graph stress tests
// ===========================================================================

#[test]
fn edge_large_linear_chain() {
    let provider = TestMetadataProvider::with_math_nodes();
    let graph = build_linear_chain(200, &provider);

    let resolver = DataResolver::build(&graph, &provider).unwrap();
    assert_eq!(resolver.get_pure_evaluation_order().len(), 200);
}

#[test]
fn edge_large_wide_graph() {
    let mut graph = GraphDescription::new("wide");
    let provider = TestMetadataProvider::with_math_nodes();

    // One source, 100 targets (fan-out)
    let mut source = NodeInstance::new("source", "add", Position::zero());
    source.add_input_pin("a", DataType::Typed("i64".into()));
    source.add_input_pin("b", DataType::Typed("i64".into()));
    source.add_output_pin("result", DataType::Typed("i64".into()));
    source.set_property("a", PropertyValue::Number(1.0));
    source.set_property("b", PropertyValue::Number(1.0));
    graph.add_node(source);

    for i in 0..100 {
        let mut node = NodeInstance::new(format!("fan_{}", i), "add", Position::zero());
        node.add_input_pin("a", DataType::Typed("i64".into()));
        node.add_input_pin("b", DataType::Typed("i64".into()));
        node.add_output_pin("result", DataType::Typed("i64".into()));
        node.set_property("b", PropertyValue::Number(i as f64));
        graph.add_node(node);

        graph.add_connection(Connection::data("source", "result", format!("fan_{}", i), "a"));
    }

    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let order = resolver.get_pure_evaluation_order();
    assert_eq!(order.len(), 101); // source + 100 targets

    // Source should come before all fan-out nodes
    let source_pos = order.iter().position(|x| x == "source").unwrap();
    for i in 0..100 {
        let fan_pos = order.iter().position(|x| x == &format!("fan_{}", i)).unwrap();
        assert!(source_pos < fan_pos);
    }
}

#[test]
fn edge_large_exec_chain() {
    let graph = build_exec_chain(100);
    let routing = ExecutionRouting::build_from_graph(&graph);

    for i in 0..99 {
        let connected = routing.get_connected_nodes(&format!("fn_{}", i), "exec_out");
        assert_eq!(connected.len(), 1);
    }
}

// ===========================================================================
// Nodes with no metadata in provider
// ===========================================================================

#[test]
fn edge_node_type_not_in_provider() {
    let mut graph = GraphDescription::new("test");

    let mut node = NodeInstance::new("unknown_1", "unknown_type", Position::zero());
    node.add_input_pin("x", DataType::Typed("i64".into()));
    node.set_property("x", PropertyValue::Number(42.0));
    graph.add_node(node);

    // Provider doesn't know "unknown_type", but build should still succeed
    // (unknown nodes just won't appear in pure evaluation order)
    let provider = TestMetadataProvider::with_math_nodes();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    // Not in pure order since metadata lookup fails
    assert!(!resolver.get_pure_evaluation_order().contains(&"unknown_1".to_string()));

    // But it should still have input sources and variable names
    assert!(resolver.get_input_source("unknown_1", "x").is_some());
    assert!(resolver.get_result_variable("unknown_1").is_some());
}

// ===========================================================================
// Graph with only execution connections (no data)
// ===========================================================================

#[test]
fn edge_exec_only_graph() {
    let mut graph = GraphDescription::new("exec_only");
    let provider = TestMetadataProvider::comprehensive();

    let mut event = NodeInstance::new("start", "on_start", Position::zero());
    event.add_output_pin("exec", DataType::Execution);
    graph.add_node(event);

    let mut print1 = NodeInstance::new("p1", "print_string", Position::zero());
    print1.add_input_pin("exec_in", DataType::Execution);
    print1.add_output_pin("exec_out", DataType::Execution);
    graph.add_node(print1);

    let mut print2 = NodeInstance::new("p2", "print_string", Position::zero());
    print2.add_input_pin("exec_in", DataType::Execution);
    print2.add_output_pin("exec_out", DataType::Execution);
    graph.add_node(print2);

    graph.add_connection(Connection::execution("start", "exec", "p1", "exec_in"));
    graph.add_connection(Connection::execution("p1", "exec_out", "p2", "exec_in"));

    let resolver = DataResolver::build(&graph, &provider).unwrap();
    // No pure nodes in this graph
    assert!(resolver.get_pure_evaluation_order().is_empty());

    let routing = ExecutionRouting::build_from_graph(&graph);
    assert!(routing.has_execution_outputs("start"));
    assert!(routing.has_execution_outputs("p1"));
    assert!(!routing.has_execution_outputs("p2"));
}

// ===========================================================================
// Graph with only data connections (no execution)
// ===========================================================================

#[test]
fn edge_data_only_graph() {
    let provider = TestMetadataProvider::with_math_nodes();
    let graph = build_linear_chain(5, &provider);

    let routing = ExecutionRouting::build_from_graph(&graph);
    // No execution connections
    for i in 0..5 {
        assert!(!routing.has_execution_outputs(&format!("node_{}", i)));
    }

    let resolver = DataResolver::build(&graph, &provider).unwrap();
    assert_eq!(resolver.get_pure_evaluation_order().len(), 5);
}

// ===========================================================================
// Property boundary values
// ===========================================================================

#[test]
fn edge_property_extreme_numbers() {
    let mut node = NodeInstance::new("n", "add", Position::zero());
    node.set_property("max", PropertyValue::Number(f64::MAX));
    node.set_property("min", PropertyValue::Number(f64::MIN));
    node.set_property("zero", PropertyValue::Number(0.0));
    node.set_property("neg_zero", PropertyValue::Number(-0.0));
    node.set_property("tiny", PropertyValue::Number(f64::MIN_POSITIVE));

    assert!(node.get_property("max").is_some());
    assert!(node.get_property("min").is_some());
}

#[test]
fn edge_property_nan_and_infinity() {
    let mut node = NodeInstance::new("n", "add", Position::zero());
    node.set_property("nan", PropertyValue::Number(f64::NAN));
    node.set_property("inf", PropertyValue::Number(f64::INFINITY));
    node.set_property("neg_inf", PropertyValue::Number(f64::NEG_INFINITY));

    assert!(node.get_property("nan").is_some());
    assert!(node.get_property("inf").is_some());
}

#[test]
fn edge_property_empty_string() {
    let mut node = NodeInstance::new("n", "add", Position::zero());
    node.set_property("empty", PropertyValue::String(String::new()));

    match node.get_property("empty").unwrap() {
        PropertyValue::String(s) => assert!(s.is_empty()),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn edge_property_very_long_string() {
    let long = "x".repeat(10_000);
    let mut node = NodeInstance::new("n", "add", Position::zero());
    node.set_property("long", PropertyValue::String(long.clone()));

    match node.get_property("long").unwrap() {
        PropertyValue::String(s) => assert_eq!(s.len(), 10_000),
        _ => panic!("wrong variant"),
    }
}

// ===========================================================================
// Serialization edge cases
// ===========================================================================

#[test]
fn edge_serde_special_chars_in_strings() {
    let mut graph = GraphDescription::new("special\"chars\\in/name");
    let mut node = NodeInstance::new("node\"1", "add", Position::zero());
    node.set_property("msg", PropertyValue::String("hello\nworld\ttab".into()));
    graph.add_node(node);

    let json = serde_json::to_string(&graph).unwrap();
    let deserialized: GraphDescription = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.metadata.name, "special\"chars\\in/name");
}

#[test]
fn edge_serde_nan_serializes_as_null() {
    // JSON has no NaN representation. serde_json silently emits null,
    // which means a round-trip will NOT preserve the original NaN value.
    let json = serde_json::to_string(&PropertyValue::Number(f64::NAN)).unwrap();
    assert!(
        json.contains("null"),
        "NaN should serialize as null in JSON, got: {}",
        json
    );

    // Deserialization of the null-bearing JSON must fail or produce a
    // different value, proving the round-trip is lossy.
    let round_trip = serde_json::from_str::<PropertyValue>(&json);
    assert!(
        round_trip.is_err(),
        "Deserializing NaN-as-null back into PropertyValue::Number should fail"
    );
}

#[test]
fn edge_serde_infinity_serializes_as_null() {
    // JSON has no Infinity representation. serde_json silently emits null.
    let json = serde_json::to_string(&PropertyValue::Number(f64::INFINITY)).unwrap();
    assert!(
        json.contains("null"),
        "Infinity should serialize as null in JSON, got: {}",
        json
    );

    let round_trip = serde_json::from_str::<PropertyValue>(&json);
    assert!(
        round_trip.is_err(),
        "Deserializing Infinity-as-null back into PropertyValue::Number should fail"
    );
}

#[test]
fn edge_serde_neg_infinity_serializes_as_null() {
    let json = serde_json::to_string(&PropertyValue::Number(f64::NEG_INFINITY)).unwrap();
    assert!(json.contains("null"));

    let round_trip = serde_json::from_str::<PropertyValue>(&json);
    assert!(round_trip.is_err());
}

// ===========================================================================
// Parallel build edge cases
// ===========================================================================

#[test]
fn edge_parallel_single_node() {
    let mut graph = GraphDescription::new("single");
    let provider = TestMetadataProvider::with_math_nodes();

    let mut node = NodeInstance::new("only", "add", Position::zero());
    node.add_input_pin("a", DataType::Typed("i64".into()));
    node.add_input_pin("b", DataType::Typed("i64".into()));
    node.add_output_pin("result", DataType::Typed("i64".into()));
    node.set_property("a", PropertyValue::Number(1.0));
    node.set_property("b", PropertyValue::Number(2.0));
    graph.add_node(node);

    let resolver = DataResolver::build_parallel(&graph, &provider).unwrap();
    assert_eq!(resolver.get_pure_evaluation_order().len(), 1);
}

#[test]
fn edge_parallel_wide_graph() {
    let mut graph = GraphDescription::new("wide");
    let provider = TestMetadataProvider::with_math_nodes();

    for i in 0..50 {
        let mut node = NodeInstance::new(format!("ind_{}", i), "add", Position::zero());
        node.add_input_pin("a", DataType::Typed("i64".into()));
        node.add_input_pin("b", DataType::Typed("i64".into()));
        node.add_output_pin("result", DataType::Typed("i64".into()));
        node.set_property("a", PropertyValue::Number(1.0));
        node.set_property("b", PropertyValue::Number(2.0));
        graph.add_node(node);
    }

    let resolver = DataResolver::build_parallel(&graph, &provider).unwrap();
    assert_eq!(resolver.get_pure_evaluation_order().len(), 50);
}
