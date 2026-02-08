//! Integration tests: full compilation pipeline, end-to-end scenarios.

mod common;

use common::*;
use graphy::*;

// ===========================================================================
// Full Pipeline - Data flow + Execution routing
// ===========================================================================

#[test]
fn pipeline_linear_chain_data_flow_and_routing() {
    let provider = TestMetadataProvider::with_math_nodes();
    let graph = build_linear_chain(10, &provider);

    // Data flow analysis
    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let order = resolver.get_pure_evaluation_order();
    assert_eq!(order.len(), 10);

    // All nodes should have result variables
    for i in 0..10 {
        assert!(resolver.get_result_variable(&format!("node_{}", i)).is_some());
    }

    // Execution routing should be empty (pure nodes only)
    let routing = ExecutionRouting::build_from_graph(&graph);
    assert!(!routing.has_execution_outputs("node_0"));
}

#[test]
fn pipeline_diamond_data_flow() {
    let provider = TestMetadataProvider::with_math_nodes();
    let graph = build_diamond_graph();

    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let order = resolver.get_pure_evaluation_order();

    // All 4 nodes should be in the order
    assert_eq!(order.len(), 4);

    // Verify connections are properly mapped
    // node_b.a should come from node_a.result
    let b_input = resolver.get_input_source("node_b", "a").unwrap();
    match b_input {
        DataSource::Connection { source_node_id, source_pin } => {
            assert_eq!(source_node_id, "node_a");
            assert_eq!(source_pin, "result");
        }
        _ => panic!("expected Connection"),
    }

    // node_d.a should come from node_b.result
    let d_a = resolver.get_input_source("node_d", "a").unwrap();
    match d_a {
        DataSource::Connection { source_node_id, .. } => {
            assert_eq!(source_node_id, "node_b");
        }
        _ => panic!("expected Connection"),
    }

    // node_d.b should come from node_c.result
    let d_b = resolver.get_input_source("node_d", "b").unwrap();
    match d_b {
        DataSource::Connection { source_node_id, .. } => {
            assert_eq!(source_node_id, "node_c");
        }
        _ => panic!("expected Connection"),
    }
}

#[test]
fn pipeline_exec_chain_routing() {
    let graph = build_exec_chain(5);
    let routing = ExecutionRouting::build_from_graph(&graph);

    // Full chain: fn_0 -> fn_1 -> fn_2 -> fn_3 -> fn_4
    for i in 0..4 {
        let connected = routing.get_connected_nodes(&format!("fn_{}", i), "exec_out");
        assert_eq!(connected.len(), 1);
        assert_eq!(connected[0], format!("fn_{}", i + 1));
    }
}

#[test]
fn pipeline_branch_graph_full() {
    let provider = TestMetadataProvider::comprehensive();
    let graph = build_branch_graph();

    // Data flow
    let resolver = DataResolver::build(&graph, &provider).unwrap();

    // Branch condition from property
    let cond = resolver.get_input_source("branch_1", "condition").unwrap();
    assert!(matches!(cond, DataSource::Constant(_)));

    // Execution routing
    let routing = ExecutionRouting::build_from_graph(&graph);
    assert!(routing.has_execution_outputs("start"));
    assert!(routing.has_execution_outputs("branch_1"));

    let true_targets = routing.get_connected_nodes("branch_1", "True");
    assert_eq!(true_targets, &["print_true"]);

    let false_targets = routing.get_connected_nodes("branch_1", "False");
    assert_eq!(false_targets, &["print_false"]);
}

// ===========================================================================
// Full Pipeline - CodeGeneratorContext
// ===========================================================================

#[test]
fn pipeline_context_creation_from_analysis() {
    let provider = TestMetadataProvider::comprehensive();
    let graph = build_branch_graph();

    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let routing = ExecutionRouting::build_from_graph(&graph);

    let mut ctx = CodeGeneratorContext::new(&graph, &provider, &resolver, &routing);

    // Verify context has access to everything
    assert!(ctx.graph.get_node("branch_1").is_some());
    assert!(ctx.metadata_provider.get_node_metadata("branch").is_some());
    assert!(ctx.data_resolver.get_result_variable("branch_1").is_some());
    assert!(ctx.exec_routing.has_execution_outputs("branch_1"));

    // Test visit tracking during generation
    ctx.mark_visited("start");
    ctx.mark_visited("branch_1");
    assert!(ctx.is_visited("start"));
    assert!(ctx.is_visited("branch_1"));
    assert!(!ctx.is_visited("print_true"));

    // Indent for nested code gen
    ctx.push_indent();
    assert_eq!(ctx.indent(), "    ");
    ctx.push_indent();
    assert_eq!(ctx.indent(), "        ");
    ctx.pop_indent();
    assert_eq!(ctx.indent(), "    ");
}

// ===========================================================================
// Mixed data + execution graph
// ===========================================================================

#[test]
fn pipeline_mixed_data_and_execution() {
    let mut graph = GraphDescription::new("mixed");
    let provider = TestMetadataProvider::comprehensive();

    // Pure computation
    let mut add_node = NodeInstance::new("add_1", "add", Position::zero());
    add_node.add_input_pin("a", DataType::Typed("i64".into()));
    add_node.add_input_pin("b", DataType::Typed("i64".into()));
    add_node.add_output_pin("result", DataType::Typed("i64".into()));
    add_node.set_property("a", PropertyValue::Number(5.0));
    add_node.set_property("b", PropertyValue::Number(3.0));
    graph.add_node(add_node);

    // Event entry
    let mut event = NodeInstance::new("start", "on_start", Position::new(0.0, 200.0));
    event.add_output_pin("exec", DataType::Execution);
    graph.add_node(event);

    // Print (function node that consumes the pure result)
    let mut print = NodeInstance::new("print_1", "print_string", Position::new(200.0, 200.0));
    print.add_input_pin("exec_in", DataType::Execution);
    print.add_input_pin("message", DataType::Typed("String".into()));
    print.add_output_pin("exec_out", DataType::Execution);
    graph.add_node(print);

    // Data connection: add_1.result -> print_1.message
    graph.add_connection(Connection::data("add_1", "result", "print_1", "message"));
    // Exec connection: start -> print_1
    graph.add_connection(Connection::execution("start", "exec", "print_1", "exec_in"));

    // Build analysis
    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let routing = ExecutionRouting::build_from_graph(&graph);

    // Pure nodes in evaluation order
    let order = resolver.get_pure_evaluation_order();
    assert!(order.contains(&"add_1".to_string()));
    assert!(!order.contains(&"print_1".to_string()));
    assert!(!order.contains(&"start".to_string()));

    // Data flow: print_1.message comes from add_1.result
    let msg_source = resolver.get_input_source("print_1", "message").unwrap();
    assert!(matches!(msg_source, DataSource::Connection { .. }));

    // Execution flow: start -> print_1
    let targets = routing.get_connected_nodes("start", "exec");
    assert_eq!(targets, &["print_1"]);
}

// ===========================================================================
// Parallel pipeline matches sequential
// ===========================================================================

#[test]
fn pipeline_parallel_matches_sequential_full() {
    let provider = TestMetadataProvider::with_math_nodes();
    let graph = build_linear_chain(20, &provider);

    let seq_resolver = DataResolver::build(&graph, &provider).unwrap();
    let par_resolver = DataResolver::build_parallel(&graph, &provider).unwrap();

    // Same evaluation order
    assert_eq!(
        seq_resolver.get_pure_evaluation_order(),
        par_resolver.get_pure_evaluation_order()
    );

    // Same data sources for all inputs
    for i in 0..20 {
        let id = format!("node_{}", i);
        for pin in ["a", "b"] {
            let seq = seq_resolver.get_input_source(&id, pin);
            let par = par_resolver.get_input_source(&id, pin);
            assert_eq!(seq.is_some(), par.is_some(), "mismatch for {}.{}", id, pin);
        }
    }
}

// ===========================================================================
// SubGraphExpander (placeholder)
// ===========================================================================

#[test]
fn subgraph_expander_noop() {
    let expander = SubGraphExpander::new();
    let mut graph = GraphDescription::new("test");
    graph.add_node(NodeInstance::new("n1", "add", Position::zero()));

    // Should succeed without modifying the graph
    expander.expand_all(&mut graph).unwrap();
    assert_eq!(graph.nodes.len(), 1);
}

#[test]
fn subgraph_expander_default_trait() {
    let expander = SubGraphExpander::default();
    let mut graph = GraphDescription::new("test");
    expander.expand_all(&mut graph).unwrap();
}

// ===========================================================================
// Error types
// ===========================================================================

#[test]
fn error_display_node_not_found() {
    let err = GraphyError::NodeNotFound("missing_node".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("missing_node"));
}

#[test]
fn error_display_pin_not_found() {
    let err = GraphyError::PinNotFound {
        node: "node_1".to_string(),
        pin: "result".to_string(),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("node_1"));
    assert!(msg.contains("result"));
}

#[test]
fn error_display_type_mismatch() {
    let err = GraphyError::TypeMismatch {
        expected: "i64".to_string(),
        actual: "String".to_string(),
    };
    let msg = format!("{}", err);
    assert!(msg.contains("i64"));
    assert!(msg.contains("String"));
}

#[test]
fn error_display_cyclic() {
    let err = GraphyError::CyclicDependency;
    let msg = format!("{}", err);
    assert!(msg.contains("Cyclic") || msg.contains("cyclic"));
}

#[test]
fn error_display_invalid_connection() {
    let err = GraphyError::InvalidConnection("bad wire".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("bad wire"));
}

#[test]
fn error_display_code_generation() {
    let err = GraphyError::CodeGeneration("failed to emit".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("failed to emit"));
}

#[test]
fn error_display_ast_parsing() {
    let err = GraphyError::AstParsing("unexpected token".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("unexpected token"));
}

#[test]
fn error_display_graph_expansion() {
    let err = GraphyError::GraphExpansion("recursive".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("recursive"));
}

#[test]
fn error_display_custom() {
    let err = GraphyError::Custom("something weird".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("something weird"));
}

#[test]
fn error_is_std_error() {
    let err = GraphyError::Custom("test".to_string());
    let _: &dyn std::error::Error = &err;
}
