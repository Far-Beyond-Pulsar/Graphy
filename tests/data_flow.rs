//! Tests for DataResolver: data connection mapping, variable name generation,
//! topological sorting, cycle detection, and parallel builds.

mod common;

use common::*;
use graphy::*;

// ===========================================================================
// DataResolver - Basic Building
// ===========================================================================

#[test]
fn data_resolver_empty_graph() {
    let graph = GraphDescription::new("empty");
    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    assert!(resolver.get_pure_evaluation_order().is_empty());
}

#[test]
fn data_resolver_single_node_with_constants() {
    let mut graph = GraphDescription::new("test");

    let mut node = NodeInstance::new("add_1", "add", Position::zero());
    node.add_input_pin("a", DataType::Typed("i64".into()));
    node.add_input_pin("b", DataType::Typed("i64".into()));
    node.add_output_pin("result", DataType::Typed("i64".into()));
    node.set_property("a", PropertyValue::Number(5.0));
    node.set_property("b", PropertyValue::Number(3.0));
    graph.add_node(node);

    let provider = TestMetadataProvider::with_math_nodes();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    // Both inputs should be constants
    let a = resolver.get_input_source("add_1", "a").unwrap();
    let b = resolver.get_input_source("add_1", "b").unwrap();
    assert!(matches!(a, DataSource::Constant(_)));
    assert!(matches!(b, DataSource::Constant(_)));

    // Should have a result variable
    let var = resolver.get_result_variable("add_1");
    assert!(var.is_some());
    assert!(var.unwrap().contains("add_1"));
}

#[test]
fn data_resolver_unconnected_input_default() {
    let mut graph = GraphDescription::new("test");

    let mut node = NodeInstance::new("add_1", "add", Position::zero());
    node.add_input_pin("a", DataType::Typed("i64".into()));
    // No property, no connection -> Default
    graph.add_node(node);

    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    let source = resolver.get_input_source("add_1", "a").unwrap();
    assert!(matches!(source, DataSource::Default));
}

#[test]
fn data_resolver_connected_input() {
    let mut graph = GraphDescription::new("test");

    let mut node_a = NodeInstance::new("node_a", "add", Position::zero());
    node_a.add_output_pin("result", DataType::Typed("i64".into()));
    graph.add_node(node_a);

    let mut node_b = NodeInstance::new("node_b", "add", Position::zero());
    node_b.add_input_pin("a", DataType::Typed("i64".into()));
    graph.add_node(node_b);

    graph.add_connection(Connection::data("node_a", "result", "node_b", "a"));

    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    let source = resolver.get_input_source("node_b", "a").unwrap();
    match source {
        DataSource::Connection {
            source_node_id,
            source_pin,
        } => {
            assert_eq!(source_node_id, "node_a");
            assert_eq!(source_pin, "result");
        }
        _ => panic!("expected Connection source"),
    }
}

#[test]
fn data_resolver_connection_overrides_property() {
    let mut graph = GraphDescription::new("test");

    let mut node_a = NodeInstance::new("node_a", "add", Position::zero());
    node_a.add_output_pin("result", DataType::Typed("i64".into()));
    graph.add_node(node_a);

    let mut node_b = NodeInstance::new("node_b", "add", Position::zero());
    node_b.add_input_pin("a", DataType::Typed("i64".into()));
    // Property set, but a connection exists -> Connection should win
    node_b.set_property("a", PropertyValue::Number(99.0));
    graph.add_node(node_b);

    graph.add_connection(Connection::data("node_a", "result", "node_b", "a"));

    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    let source = resolver.get_input_source("node_b", "a").unwrap();
    assert!(matches!(source, DataSource::Connection { .. }));
}

// ===========================================================================
// DataResolver - Variable Names
// ===========================================================================

#[test]
fn data_resolver_result_variables_for_all_nodes() {
    let mut graph = GraphDescription::new("test");
    for i in 0..5 {
        graph.add_node(NodeInstance::new(
            format!("node_{}", i),
            "add",
            Position::zero(),
        ));
    }

    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    for i in 0..5 {
        let var = resolver.get_result_variable(&format!("node_{}", i));
        assert!(var.is_some(), "missing variable for node_{}", i);
    }
}

#[test]
fn data_resolver_nonexistent_node_returns_none() {
    let graph = GraphDescription::new("test");
    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    assert!(resolver.get_result_variable("nope").is_none());
    assert!(resolver.get_input_source("nope", "a").is_none());
}

// ===========================================================================
// DataResolver - Topological Sort
// ===========================================================================

#[test]
fn data_resolver_linear_chain_order() {
    let provider = TestMetadataProvider::with_math_nodes();
    let graph = build_linear_chain(5, &provider);
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    let order = resolver.get_pure_evaluation_order();
    assert_eq!(order.len(), 5);

    // node_0 must come before node_1, node_1 before node_2, etc.
    for i in 0..4 {
        let pos_i = order.iter().position(|x| x == &format!("node_{}", i)).unwrap();
        let pos_next = order.iter().position(|x| x == &format!("node_{}", i + 1)).unwrap();
        assert!(
            pos_i < pos_next,
            "node_{} (pos {}) should come before node_{} (pos {})",
            i,
            pos_i,
            i + 1,
            pos_next
        );
    }
}

#[test]
fn data_resolver_diamond_order() {
    let provider = TestMetadataProvider::with_math_nodes();
    let graph = build_diamond_graph();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    let order = resolver.get_pure_evaluation_order();
    assert_eq!(order.len(), 4);

    let pos = |id: &str| order.iter().position(|x| x == id).unwrap();

    // node_a must be first (no deps)
    // node_b and node_c must come after node_a
    // node_d must come after both node_b and node_c
    assert!(pos("node_a") < pos("node_b"));
    assert!(pos("node_a") < pos("node_c"));
    assert!(pos("node_b") < pos("node_d"));
    assert!(pos("node_c") < pos("node_d"));
}

#[test]
fn data_resolver_independent_nodes_all_included() {
    let mut graph = GraphDescription::new("test");
    let provider = TestMetadataProvider::with_math_nodes();

    // Three unconnected pure nodes
    for i in 0..3 {
        let mut node = NodeInstance::new(format!("ind_{}", i), "add", Position::zero());
        node.add_input_pin("a", DataType::Typed("i64".into()));
        node.add_input_pin("b", DataType::Typed("i64".into()));
        node.add_output_pin("result", DataType::Typed("i64".into()));
        node.set_property("a", PropertyValue::Number(1.0));
        node.set_property("b", PropertyValue::Number(2.0));
        graph.add_node(node);
    }

    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let order = resolver.get_pure_evaluation_order();
    assert_eq!(order.len(), 3);
}

// ===========================================================================
// DataResolver - Cycle Detection
// ===========================================================================

#[test]
fn data_resolver_detects_cycle() {
    let mut graph = GraphDescription::new("cyclic");
    let provider = TestMetadataProvider::with_math_nodes();

    // Create A -> B -> A cycle among pure nodes
    let mut node_a = NodeInstance::new("cycle_a", "add", Position::zero());
    node_a.add_input_pin("a", DataType::Typed("i64".into()));
    node_a.add_input_pin("b", DataType::Typed("i64".into()));
    node_a.add_output_pin("result", DataType::Typed("i64".into()));
    node_a.set_property("b", PropertyValue::Number(1.0));
    graph.add_node(node_a);

    let mut node_b = NodeInstance::new("cycle_b", "add", Position::zero());
    node_b.add_input_pin("a", DataType::Typed("i64".into()));
    node_b.add_input_pin("b", DataType::Typed("i64".into()));
    node_b.add_output_pin("result", DataType::Typed("i64".into()));
    node_b.set_property("b", PropertyValue::Number(1.0));
    graph.add_node(node_b);

    // A -> B and B -> A
    graph.add_connection(Connection::data("cycle_a", "result", "cycle_b", "a"));
    graph.add_connection(Connection::data("cycle_b", "result", "cycle_a", "a"));

    let result = DataResolver::build(&graph, &provider);
    assert!(result.is_err());
    if let Err(err) = result {
        assert!(
            format!("{}", err).contains("Cyclic"),
            "error should mention cyclic: {}",
            err
        );
    }
}

#[test]
fn data_resolver_three_node_cycle() {
    let mut graph = GraphDescription::new("cyclic3");
    let provider = TestMetadataProvider::with_math_nodes();

    for id in ["cyc_1", "cyc_2", "cyc_3"] {
        let mut node = NodeInstance::new(id, "add", Position::zero());
        node.add_input_pin("a", DataType::Typed("i64".into()));
        node.add_input_pin("b", DataType::Typed("i64".into()));
        node.add_output_pin("result", DataType::Typed("i64".into()));
        node.set_property("b", PropertyValue::Number(1.0));
        graph.add_node(node);
    }

    graph.add_connection(Connection::data("cyc_1", "result", "cyc_2", "a"));
    graph.add_connection(Connection::data("cyc_2", "result", "cyc_3", "a"));
    graph.add_connection(Connection::data("cyc_3", "result", "cyc_1", "a"));

    let result = DataResolver::build(&graph, &provider);
    assert!(result.is_err());
}

// ===========================================================================
// DataResolver - Non-pure nodes are excluded from topological sort
// ===========================================================================

#[test]
fn data_resolver_excludes_non_pure_from_order() {
    let mut graph = GraphDescription::new("mixed");
    let provider = TestMetadataProvider::comprehensive();

    // Pure node
    let mut pure_node = NodeInstance::new("pure_1", "add", Position::zero());
    pure_node.add_input_pin("a", DataType::Typed("i64".into()));
    pure_node.add_input_pin("b", DataType::Typed("i64".into()));
    pure_node.add_output_pin("result", DataType::Typed("i64".into()));
    pure_node.set_property("a", PropertyValue::Number(1.0));
    pure_node.set_property("b", PropertyValue::Number(2.0));
    graph.add_node(pure_node);

    // Function node (not pure)
    let mut fn_node = NodeInstance::new("fn_1", "print_string", Position::zero());
    fn_node.add_input_pin("exec_in", DataType::Execution);
    fn_node.add_input_pin("message", DataType::Typed("String".into()));
    fn_node.add_output_pin("exec_out", DataType::Execution);
    graph.add_node(fn_node);

    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let order = resolver.get_pure_evaluation_order();

    assert!(order.contains(&"pure_1".to_string()));
    assert!(!order.contains(&"fn_1".to_string()));
}

// ===========================================================================
// DataResolver - Parallel Build
// ===========================================================================

#[test]
fn data_resolver_parallel_empty_graph() {
    let graph = GraphDescription::new("empty");
    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build_parallel(&graph, &provider).unwrap();
    assert!(resolver.get_pure_evaluation_order().is_empty());
}

#[test]
fn data_resolver_parallel_matches_sequential() {
    let provider = TestMetadataProvider::with_math_nodes();
    let graph = build_linear_chain(10, &provider);

    let seq = DataResolver::build(&graph, &provider).unwrap();
    let par = DataResolver::build_parallel(&graph, &provider).unwrap();

    // Same evaluation order (both use same topological sort)
    assert_eq!(
        seq.get_pure_evaluation_order(),
        par.get_pure_evaluation_order()
    );

    // Same result variables for all nodes
    for i in 0..10 {
        let id = format!("node_{}", i);
        assert_eq!(
            seq.get_result_variable(&id),
            par.get_result_variable(&id),
            "mismatch for {}",
            id
        );
    }
}

#[test]
fn data_resolver_parallel_detects_cycle() {
    let mut graph = GraphDescription::new("cyclic");
    let provider = TestMetadataProvider::with_math_nodes();

    let mut a = NodeInstance::new("a", "add", Position::zero());
    a.add_input_pin("a", DataType::Typed("i64".into()));
    a.add_input_pin("b", DataType::Typed("i64".into()));
    a.add_output_pin("result", DataType::Typed("i64".into()));
    a.set_property("b", PropertyValue::Number(1.0));
    graph.add_node(a);

    let mut b = NodeInstance::new("b", "add", Position::zero());
    b.add_input_pin("a", DataType::Typed("i64".into()));
    b.add_input_pin("b", DataType::Typed("i64".into()));
    b.add_output_pin("result", DataType::Typed("i64".into()));
    b.set_property("b", PropertyValue::Number(1.0));
    graph.add_node(b);

    graph.add_connection(Connection::data("a", "result", "b", "a"));
    graph.add_connection(Connection::data("b", "result", "a", "a"));

    let result = DataResolver::build_parallel(&graph, &provider);
    assert!(result.is_err());
}

#[test]
fn data_resolver_parallel_diamond() {
    let provider = TestMetadataProvider::with_math_nodes();
    let graph = build_diamond_graph();
    let resolver = DataResolver::build_parallel(&graph, &provider).unwrap();

    let order = resolver.get_pure_evaluation_order();
    assert_eq!(order.len(), 4);

    let pos = |id: &str| order.iter().position(|x| x == id).unwrap();
    assert!(pos("node_a") < pos("node_d"));
}

// ===========================================================================
// DataResolver - Property value string conversion
// ===========================================================================

#[test]
fn data_resolver_constant_string_value() {
    let mut graph = GraphDescription::new("test");

    let mut node = NodeInstance::new("n", "print_string", Position::zero());
    node.add_input_pin("message", DataType::Typed("String".into()));
    node.set_property("message", PropertyValue::String("hello world".into()));
    graph.add_node(node);

    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    let source = resolver.get_input_source("n", "message").unwrap();
    match source {
        DataSource::Constant(s) => assert!(s.contains("hello world")),
        _ => panic!("expected Constant"),
    }
}

#[test]
fn data_resolver_constant_boolean_value() {
    let mut graph = GraphDescription::new("test");

    let mut node = NodeInstance::new("n", "branch", Position::zero());
    node.add_input_pin("condition", DataType::Typed("bool".into()));
    node.set_property("condition", PropertyValue::Boolean(true));
    graph.add_node(node);

    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    let source = resolver.get_input_source("n", "condition").unwrap();
    match source {
        DataSource::Constant(s) => assert_eq!(s, "true"),
        _ => panic!("expected Constant"),
    }
}

#[test]
fn data_resolver_constant_integer_number() {
    let mut graph = GraphDescription::new("test");

    let mut node = NodeInstance::new("n", "add", Position::zero());
    node.add_input_pin("a", DataType::Typed("i64".into()));
    node.set_property("a", PropertyValue::Number(42.0));
    graph.add_node(node);

    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    let source = resolver.get_input_source("n", "a").unwrap();
    match source {
        DataSource::Constant(s) => assert_eq!(s, "42"),
        _ => panic!("expected Constant"),
    }
}

#[test]
fn data_resolver_constant_float_number() {
    let mut graph = GraphDescription::new("test");

    let mut node = NodeInstance::new("n", "add", Position::zero());
    node.add_input_pin("a", DataType::Typed("f64".into()));
    node.set_property("a", PropertyValue::Number(3.14));
    graph.add_node(node);

    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    let source = resolver.get_input_source("n", "a").unwrap();
    match source {
        DataSource::Constant(s) => assert!(s.contains("3.14")),
        _ => panic!("expected Constant"),
    }
}

#[test]
fn data_resolver_constant_vector2_value() {
    let mut graph = GraphDescription::new("test");

    let mut node = NodeInstance::new("n", "pos", Position::zero());
    node.add_input_pin("v", DataType::Vector2);
    node.set_property("v", PropertyValue::Vector2(1.0, 2.0));
    graph.add_node(node);

    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    let source = resolver.get_input_source("n", "v").unwrap();
    match source {
        DataSource::Constant(s) => {
            assert!(s.contains("1"));
            assert!(s.contains("2"));
        }
        _ => panic!("expected Constant"),
    }
}
