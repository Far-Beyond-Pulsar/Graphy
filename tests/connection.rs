//! Tests for Connection and ConnectionType.

use graphy::*;

// ===========================================================================
// ConnectionType
// ===========================================================================

#[test]
fn connection_type_data() {
    assert_eq!(ConnectionType::Data, ConnectionType::Data);
}

#[test]
fn connection_type_execution() {
    assert_eq!(ConnectionType::Execution, ConnectionType::Execution);
}

#[test]
fn connection_type_ne() {
    assert_ne!(ConnectionType::Data, ConnectionType::Execution);
}

#[test]
fn connection_type_copy() {
    let a = ConnectionType::Data;
    let b = a; // Copy
    assert_eq!(a, b);
}

// ===========================================================================
// Connection - Constructors
// ===========================================================================

#[test]
fn connection_new_data() {
    let c = Connection::new("src", "out", "tgt", "in", ConnectionType::Data);
    assert_eq!(c.source_node, "src");
    assert_eq!(c.source_pin, "out");
    assert_eq!(c.target_node, "tgt");
    assert_eq!(c.target_pin, "in");
    assert_eq!(c.connection_type, ConnectionType::Data);
}

#[test]
fn connection_new_execution() {
    let c = Connection::new("a", "exec_out", "b", "exec_in", ConnectionType::Execution);
    assert_eq!(c.connection_type, ConnectionType::Execution);
}

#[test]
fn connection_data_shorthand() {
    let c = Connection::data("node_a", "result", "node_b", "input_a");
    assert_eq!(c.source_node, "node_a");
    assert_eq!(c.source_pin, "result");
    assert_eq!(c.target_node, "node_b");
    assert_eq!(c.target_pin, "input_a");
    assert_eq!(c.connection_type, ConnectionType::Data);
}

#[test]
fn connection_execution_shorthand() {
    let c = Connection::execution("node_a", "exec_out", "node_b", "exec_in");
    assert_eq!(c.source_node, "node_a");
    assert_eq!(c.source_pin, "exec_out");
    assert_eq!(c.target_node, "node_b");
    assert_eq!(c.target_pin, "exec_in");
    assert_eq!(c.connection_type, ConnectionType::Execution);
}

#[test]
fn connection_with_string_args() {
    let src = String::from("src_node");
    let tgt = String::from("tgt_node");
    let c = Connection::data(src, "out", tgt, "in");
    assert_eq!(c.source_node, "src_node");
    assert_eq!(c.target_node, "tgt_node");
}

// ===========================================================================
// Connection - Clone
// ===========================================================================

#[test]
fn connection_clone() {
    let original = Connection::data("a", "out", "b", "in");
    let cloned = original.clone();
    assert_eq!(cloned.source_node, "a");
    assert_eq!(cloned.target_node, "b");
    assert_eq!(cloned.connection_type, ConnectionType::Data);
}

// ===========================================================================
// Connection - Self-loop
// ===========================================================================

#[test]
fn connection_self_loop() {
    // The data structure allows self-loops; validation is a higher-level concern.
    let c = Connection::data("node_a", "out", "node_a", "in");
    assert_eq!(c.source_node, c.target_node);
}

// ===========================================================================
// Connection - Same pin names on different nodes
// ===========================================================================

#[test]
fn connection_same_pin_names_different_nodes() {
    let c1 = Connection::data("node_1", "result", "node_2", "result");
    assert_eq!(c1.source_pin, "result");
    assert_eq!(c1.target_pin, "result");
    assert_ne!(c1.source_node, c1.target_node);
}
