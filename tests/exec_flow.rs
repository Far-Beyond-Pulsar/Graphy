//! Tests for ExecutionRouting: route building, queries, edge cases.

mod common;

use common::*;
use graphy::*;

// ===========================================================================
// ExecutionRouting - Basic
// ===========================================================================

#[test]
fn exec_routing_empty_graph() {
    let graph = GraphDescription::new("empty");
    let routing = ExecutionRouting::build_from_graph(&graph);

    assert!(routing.get_connected_nodes("any", "any").is_empty());
    assert!(!routing.has_execution_outputs("any"));
    assert!(routing.get_output_pins("any").is_empty());
}

#[test]
fn exec_routing_single_connection() {
    let mut graph = GraphDescription::new("test");

    let mut n1 = NodeInstance::new("node1", "print", Position::zero());
    n1.add_output_pin("exec_out", DataType::Execution);
    graph.add_node(n1);

    let mut n2 = NodeInstance::new("node2", "print", Position::zero());
    n2.add_input_pin("exec_in", DataType::Execution);
    graph.add_node(n2);

    graph.add_connection(Connection::execution("node1", "exec_out", "node2", "exec_in"));

    let routing = ExecutionRouting::build_from_graph(&graph);
    let connected = routing.get_connected_nodes("node1", "exec_out");
    assert_eq!(connected, &["node2"]);
}

#[test]
fn exec_routing_chain() {
    let graph = build_exec_chain(5);
    let routing = ExecutionRouting::build_from_graph(&graph);

    for i in 0..4 {
        let connected = routing.get_connected_nodes(&format!("fn_{}", i), "exec_out");
        assert_eq!(connected, &[format!("fn_{}", i + 1)]);
    }

    // Last node has no outgoing exec
    let last = routing.get_connected_nodes("fn_4", "exec_out");
    assert!(last.is_empty());
}

// ===========================================================================
// ExecutionRouting - Branching
// ===========================================================================

#[test]
fn exec_routing_branch() {
    let graph = build_branch_graph();
    let routing = ExecutionRouting::build_from_graph(&graph);

    // start -> branch_1
    let from_start = routing.get_connected_nodes("start", "exec");
    assert_eq!(from_start, &["branch_1"]);

    // branch_1 True -> print_true
    let true_branch = routing.get_connected_nodes("branch_1", "True");
    assert_eq!(true_branch, &["print_true"]);

    // branch_1 False -> print_false
    let false_branch = routing.get_connected_nodes("branch_1", "False");
    assert_eq!(false_branch, &["print_false"]);
}

#[test]
fn exec_routing_multiple_targets_from_one_pin() {
    let mut graph = GraphDescription::new("fan_out");

    let mut source = NodeInstance::new("source", "event", Position::zero());
    source.add_output_pin("exec", DataType::Execution);
    graph.add_node(source);

    for i in 0..3 {
        let mut target = NodeInstance::new(format!("target_{}", i), "print", Position::zero());
        target.add_input_pin("exec_in", DataType::Execution);
        graph.add_node(target);

        graph.add_connection(Connection::execution(
            "source",
            "exec",
            format!("target_{}", i),
            "exec_in",
        ));
    }

    let routing = ExecutionRouting::build_from_graph(&graph);
    let connected = routing.get_connected_nodes("source", "exec");
    assert_eq!(connected.len(), 3);
}

// ===========================================================================
// ExecutionRouting - has_execution_outputs
// ===========================================================================

#[test]
fn exec_routing_has_execution_outputs_true() {
    let graph = build_exec_chain(2);
    let routing = ExecutionRouting::build_from_graph(&graph);
    assert!(routing.has_execution_outputs("fn_0"));
}

#[test]
fn exec_routing_has_execution_outputs_false_for_last() {
    let graph = build_exec_chain(2);
    let routing = ExecutionRouting::build_from_graph(&graph);
    assert!(!routing.has_execution_outputs("fn_1"));
}

#[test]
fn exec_routing_has_execution_outputs_nonexistent() {
    let graph = build_exec_chain(2);
    let routing = ExecutionRouting::build_from_graph(&graph);
    assert!(!routing.has_execution_outputs("nonexistent"));
}

// ===========================================================================
// ExecutionRouting - get_output_pins
// ===========================================================================

#[test]
fn exec_routing_get_output_pins_single() {
    let graph = build_exec_chain(2);
    let routing = ExecutionRouting::build_from_graph(&graph);
    let pins = routing.get_output_pins("fn_0");
    assert_eq!(pins.len(), 1);
    assert_eq!(pins[0], "exec_out");
}

#[test]
fn exec_routing_get_output_pins_branch() {
    let graph = build_branch_graph();
    let routing = ExecutionRouting::build_from_graph(&graph);
    let mut pins = routing.get_output_pins("branch_1");
    pins.sort();
    assert_eq!(pins, vec!["False", "True"]);
}

#[test]
fn exec_routing_get_output_pins_none() {
    let graph = build_exec_chain(2);
    let routing = ExecutionRouting::build_from_graph(&graph);
    let pins = routing.get_output_pins("fn_1");
    assert!(pins.is_empty());
}

// ===========================================================================
// ExecutionRouting - Ignores data connections
// ===========================================================================

#[test]
fn exec_routing_ignores_data_connections() {
    let mut graph = GraphDescription::new("data_only");

    let mut a = NodeInstance::new("a", "add", Position::zero());
    a.add_output_pin("result", DataType::Typed("i64".into()));
    graph.add_node(a);

    let mut b = NodeInstance::new("b", "add", Position::zero());
    b.add_input_pin("a", DataType::Typed("i64".into()));
    graph.add_node(b);

    graph.add_connection(Connection::data("a", "result", "b", "a"));

    let routing = ExecutionRouting::build_from_graph(&graph);
    assert!(!routing.has_execution_outputs("a"));
    assert!(routing.get_connected_nodes("a", "result").is_empty());
}

// ===========================================================================
// ExecutionRouting - Complex topology
// ===========================================================================

#[test]
fn exec_routing_diamond_execution() {
    // A -> B, A -> C, B -> D, C -> D
    let mut graph = GraphDescription::new("exec_diamond");

    for id in ["a", "b", "c", "d"] {
        let mut node = NodeInstance::new(id, "fn", Position::zero());
        node.add_input_pin("exec_in", DataType::Execution);
        node.add_output_pin("exec_out", DataType::Execution);
        graph.add_node(node);
    }

    // A fans out
    graph.add_connection(Connection::execution("a", "exec_out", "b", "exec_in"));
    graph.add_connection(Connection::execution("a", "exec_out", "c", "exec_in"));
    // B and C converge on D
    graph.add_connection(Connection::execution("b", "exec_out", "d", "exec_in"));
    graph.add_connection(Connection::execution("c", "exec_out", "d", "exec_in"));

    let routing = ExecutionRouting::build_from_graph(&graph);

    let from_a = routing.get_connected_nodes("a", "exec_out");
    assert_eq!(from_a.len(), 2);
    assert!(from_a.contains(&"b".to_string()));
    assert!(from_a.contains(&"c".to_string()));

    let from_b = routing.get_connected_nodes("b", "exec_out");
    assert_eq!(from_b, &["d"]);

    let from_c = routing.get_connected_nodes("c", "exec_out");
    assert_eq!(from_c, &["d"]);
}
