//! Tests for GraphDescription, GraphMetadata, GraphComment, and graph manipulation.

use graphy::*;

// ===========================================================================
// GraphMetadata
// ===========================================================================

#[test]
fn graph_metadata_new() {
    let meta = core::GraphMetadata::new("my_graph");
    assert_eq!(meta.name, "my_graph");
    assert_eq!(meta.description, "");
    assert_eq!(meta.version, "1.0.0");
    assert!(meta.created_at.is_empty());
    assert!(meta.modified_at.is_empty());
}

#[test]
fn graph_metadata_custom_fields() {
    let mut meta = core::GraphMetadata::new("test");
    meta.description = "A test graph".to_string();
    meta.version = "2.0.0".to_string();
    meta.created_at = "2025-01-01".to_string();
    meta.modified_at = "2025-06-15".to_string();

    assert_eq!(meta.description, "A test graph");
    assert_eq!(meta.version, "2.0.0");
    assert_eq!(meta.created_at, "2025-01-01");
    assert_eq!(meta.modified_at, "2025-06-15");
}

// ===========================================================================
// GraphDescription - Creation
// ===========================================================================

#[test]
fn graph_new_is_empty() {
    let graph = GraphDescription::new("empty");
    assert_eq!(graph.metadata.name, "empty");
    assert!(graph.nodes.is_empty());
    assert!(graph.connections.is_empty());
    assert!(graph.comments.is_empty());
}

#[test]
fn graph_new_with_string() {
    let name = String::from("dynamic_name");
    let graph = GraphDescription::new(name);
    assert_eq!(graph.metadata.name, "dynamic_name");
}

// ===========================================================================
// GraphDescription - Node Management
// ===========================================================================

#[test]
fn graph_add_and_get_node() {
    let mut graph = GraphDescription::new("test");
    let node = NodeInstance::new("node_1", "add", Position::zero());
    graph.add_node(node);

    assert_eq!(graph.nodes.len(), 1);
    let retrieved = graph.get_node("node_1");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, "node_1");
    assert_eq!(retrieved.unwrap().node_type, "add");
}

#[test]
fn graph_get_nonexistent_node_returns_none() {
    let graph = GraphDescription::new("test");
    assert!(graph.get_node("nonexistent").is_none());
}

#[test]
fn graph_get_node_mut() {
    let mut graph = GraphDescription::new("test");
    graph.add_node(NodeInstance::new("node_1", "add", Position::zero()));

    let node_mut = graph.get_node_mut("node_1").unwrap();
    node_mut.set_property("x", PropertyValue::Number(99.0));

    let node = graph.get_node("node_1").unwrap();
    assert!(node.get_property("x").is_some());
}

#[test]
fn graph_add_multiple_nodes() {
    let mut graph = GraphDescription::new("test");
    for i in 0..10 {
        graph.add_node(NodeInstance::new(
            format!("node_{}", i),
            "add",
            Position::new(i as f64, 0.0),
        ));
    }
    assert_eq!(graph.nodes.len(), 10);
}

#[test]
fn graph_add_node_overwrites_same_id() {
    let mut graph = GraphDescription::new("test");

    let mut node_v1 = NodeInstance::new("same_id", "add", Position::zero());
    node_v1.set_property("version", PropertyValue::Number(1.0));
    graph.add_node(node_v1);

    let mut node_v2 = NodeInstance::new("same_id", "multiply", Position::zero());
    node_v2.set_property("version", PropertyValue::Number(2.0));
    graph.add_node(node_v2);

    assert_eq!(graph.nodes.len(), 1);
    let node = graph.get_node("same_id").unwrap();
    assert_eq!(node.node_type, "multiply");
}

// ===========================================================================
// GraphDescription - Connection Management
// ===========================================================================

#[test]
fn graph_add_connection() {
    let mut graph = GraphDescription::new("test");

    graph.add_node(NodeInstance::new("a", "add", Position::zero()));
    graph.add_node(NodeInstance::new("b", "add", Position::zero()));
    graph.add_connection(Connection::data("a", "result", "b", "a"));

    assert_eq!(graph.connections.len(), 1);
    assert_eq!(graph.connections[0].source_node, "a");
    assert_eq!(graph.connections[0].target_node, "b");
}

#[test]
fn graph_add_multiple_connections() {
    let mut graph = GraphDescription::new("test");

    graph.add_node(NodeInstance::new("a", "add", Position::zero()));
    graph.add_node(NodeInstance::new("b", "add", Position::zero()));
    graph.add_node(NodeInstance::new("c", "add", Position::zero()));

    graph.add_connection(Connection::data("a", "result", "b", "a"));
    graph.add_connection(Connection::data("a", "result", "c", "a"));
    graph.add_connection(Connection::execution("b", "exec_out", "c", "exec_in"));

    assert_eq!(graph.connections.len(), 3);
}

// ===========================================================================
// GraphDescription - Comments
// ===========================================================================

#[test]
fn graph_comments() {
    let mut graph = GraphDescription::new("test");

    graph.comments.push(core::GraphComment {
        text: "This is a math section".to_string(),
        position: Position::new(100.0, 50.0),
        size: (300.0, 100.0),
    });

    assert_eq!(graph.comments.len(), 1);
    assert_eq!(graph.comments[0].text, "This is a math section");
    assert_eq!(graph.comments[0].position.x, 100.0);
    assert_eq!(graph.comments[0].size.0, 300.0);
    assert_eq!(graph.comments[0].size.1, 100.0);
}

// ===========================================================================
// GraphDescription - Clone
// ===========================================================================

#[test]
fn graph_clone_is_independent() {
    let mut graph = GraphDescription::new("original");
    graph.add_node(NodeInstance::new("node_1", "add", Position::zero()));
    graph.add_connection(Connection::data("node_1", "out", "node_1", "in"));

    let mut cloned = graph.clone();
    cloned.metadata.name = "cloned".to_string();
    cloned.add_node(NodeInstance::new("node_2", "multiply", Position::zero()));

    assert_eq!(graph.metadata.name, "original");
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(cloned.nodes.len(), 2);
}
