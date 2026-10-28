//! Graph edit commands and change sets.
//!
//! Every editor mutation is a [`GraphCommand`]. [`GraphCommand::apply`] either
//! changes the graph and returns the exact [`ChangeSet`] it made, or fails
//! with an [`EditError`] and leaves the graph untouched. A `ChangeSet` can be
//! undone exactly with [`ChangeSet::undo`], and replaying the same commands on
//! the same graph is deterministic: connections keep document order, and
//! removals record the index they occupied so undo restores that order.

use serde::{Deserialize, Serialize};

use crate::{Connection, ConnectionType, GraphDescription, JsonValue, NodeInstance, PinInstance, Position};

/// A requested edit.
#[derive(Debug, Clone)]
pub enum GraphCommand {
    AddNode(NodeInstance),
    /// Removes the node and every connection touching it.
    RemoveNode { id: String },
    MoveNode { id: String, x: f64, y: f64 },
    Connect(Connection),
    Disconnect(Connection),
    /// Sets (`Some`) or clears (`None`) a legacy constant property.
    SetProperty {
        node: String,
        key: String,
        value: Option<JsonValue>,
    },
    /// Replaces a node's pin lists (dynamic pins). Connections to pins that no
    /// longer exist are removed and recorded.
    SetPins {
        node: String,
        inputs: Vec<PinInstance>,
        outputs: Vec<PinInstance>,
    },
}

/// A connection together with the index it occupied in the connection list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedConnection {
    pub index: usize,
    pub connection: Connection,
}

/// One applied, invertible change.
#[derive(Debug, Clone)]
pub enum GraphChange {
    NodeAdded(NodeInstance),
    NodeRemoved {
        node: NodeInstance,
        connections: Vec<IndexedConnection>,
    },
    NodeMoved {
        id: String,
        from: (f64, f64),
        to: (f64, f64),
    },
    ConnectionAdded(Connection),
    ConnectionRemoved(IndexedConnection),
    PropertySet {
        node: String,
        key: String,
        old: Option<JsonValue>,
        new: Option<JsonValue>,
    },
    PinsReplaced {
        node: String,
        old_inputs: Vec<PinInstance>,
        old_outputs: Vec<PinInstance>,
        new_inputs: Vec<PinInstance>,
        new_outputs: Vec<PinInstance>,
        removed_connections: Vec<IndexedConnection>,
    },
}

/// The changes one command made, in application order.
#[derive(Debug, Clone, Default)]
pub struct ChangeSet {
    pub changes: Vec<GraphChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error("node '{0}' does not exist")]
    NoSuchNode(String),
    #[error("node '{0}' already exists")]
    DuplicateNode(String),
    #[error("pin '{pin}' does not exist on node '{node}' as an {side}")]
    NoSuchPin {
        node: String,
        pin: String,
        side: &'static str,
    },
    #[error("connection already exists")]
    DuplicateConnection,
    #[error("connection does not exist")]
    NoSuchConnection,
    #[error("connection kind does not match its pins")]
    KindMismatch,
    #[error("input pin '{0}' is already driven")]
    InputOccupied(String),
    #[error("execution output '{0}' is already connected")]
    ExecOutputOccupied(String),
    #[error("duplicate pin id '{0}'")]
    DuplicatePin(String),
}

fn same_edge(a: &Connection, b: &Connection) -> bool {
    a.source_node == b.source_node
        && a.source_pin == b.source_pin
        && a.target_node == b.target_node
        && a.target_pin == b.target_pin
}

fn touching(graph: &GraphDescription, node: &str) -> Vec<IndexedConnection> {
    graph
        .connections
        .iter()
        .enumerate()
        .filter(|(_, c)| c.source_node == node || c.target_node == node)
        .map(|(index, c)| IndexedConnection {
            index,
            connection: c.clone(),
        })
        .collect()
}

fn remove_indexed(graph: &mut GraphDescription, removed: &[IndexedConnection]) {
    for r in removed.iter().rev() {
        graph.connections.remove(r.index);
    }
}

fn restore_indexed(graph: &mut GraphDescription, removed: &[IndexedConnection]) {
    for r in removed {
        graph.connections.insert(r.index, r.connection.clone());
    }
}

impl GraphCommand {
    /// Applies the command. On error the graph is unchanged.
    pub fn apply(&self, graph: &mut GraphDescription) -> Result<ChangeSet, EditError> {
        let change = match self {
            GraphCommand::AddNode(node) => {
                if graph.nodes.contains_key(&node.id) {
                    return Err(EditError::DuplicateNode(node.id.clone()));
                }
                graph.nodes.insert(node.id.clone(), node.clone());
                GraphChange::NodeAdded(node.clone())
            }
            GraphCommand::RemoveNode { id } => {
                if !graph.nodes.contains_key(id) {
                    return Err(EditError::NoSuchNode(id.clone()));
                }
                let connections = touching(graph, id);
                remove_indexed(graph, &connections);
                let node = graph.nodes.remove(id).expect("checked above");
                GraphChange::NodeRemoved { node, connections }
            }
            GraphCommand::MoveNode { id, x, y } => {
                let node = graph
                    .nodes
                    .get_mut(id)
                    .ok_or_else(|| EditError::NoSuchNode(id.clone()))?;
                let from = (node.position.x, node.position.y);
                node.position = Position { x: *x, y: *y };
                GraphChange::NodeMoved {
                    id: id.clone(),
                    from,
                    to: (*x, *y),
                }
            }
            GraphCommand::Connect(c) => {
                check_connect(graph, c)?;
                graph.connections.push(c.clone());
                GraphChange::ConnectionAdded(c.clone())
            }
            GraphCommand::Disconnect(c) => {
                let index = graph
                    .connections
                    .iter()
                    .position(|e| same_edge(e, c))
                    .ok_or(EditError::NoSuchConnection)?;
                let connection = graph.connections.remove(index);
                GraphChange::ConnectionRemoved(IndexedConnection { index, connection })
            }
            GraphCommand::SetProperty { node, key, value } => {
                let n = graph
                    .nodes
                    .get_mut(node)
                    .ok_or_else(|| EditError::NoSuchNode(node.clone()))?;
                let old = match value {
                    Some(v) => n.properties.insert(key.clone(), v.clone()),
                    None => n.properties.remove(key),
                };
                GraphChange::PropertySet {
                    node: node.clone(),
                    key: key.clone(),
                    old,
                    new: value.clone(),
                }
            }
            GraphCommand::SetPins {
                node,
                inputs,
                outputs,
            } => {
                if !graph.nodes.contains_key(node) {
                    return Err(EditError::NoSuchNode(node.clone()));
                }
                for pins in [inputs, outputs] {
                    let mut seen = std::collections::HashSet::new();
                    if let Some(dup) = pins.iter().find(|p| !seen.insert(p.id.as_str())) {
                        return Err(EditError::DuplicatePin(dup.id.clone()));
                    }
                }
                let removed_connections: Vec<IndexedConnection> = touching(graph, node)
                    .into_iter()
                    .filter(|r| {
                        let c = &r.connection;
                        let src_gone = c.source_node == *node
                            && !outputs.iter().any(|p| p.id == c.source_pin);
                        let dst_gone = c.target_node == *node
                            && !inputs.iter().any(|p| p.id == c.target_pin);
                        src_gone || dst_gone
                    })
                    .collect();
                remove_indexed(graph, &removed_connections);
                let n = graph.nodes.get_mut(node).expect("checked above");
                let old_inputs = std::mem::replace(&mut n.inputs, inputs.clone());
                let old_outputs = std::mem::replace(&mut n.outputs, outputs.clone());
                GraphChange::PinsReplaced {
                    node: node.clone(),
                    old_inputs,
                    old_outputs,
                    new_inputs: inputs.clone(),
                    new_outputs: outputs.clone(),
                    removed_connections,
                }
            }
        };
        Ok(ChangeSet {
            changes: vec![change],
        })
    }
}

fn check_connect(graph: &GraphDescription, c: &Connection) -> Result<(), EditError> {
    let src = graph
        .nodes
        .get(&c.source_node)
        .ok_or_else(|| EditError::NoSuchNode(c.source_node.clone()))?;
    let dst = graph
        .nodes
        .get(&c.target_node)
        .ok_or_else(|| EditError::NoSuchNode(c.target_node.clone()))?;
    let from = src
        .outputs
        .iter()
        .find(|p| p.id == c.source_pin)
        .ok_or_else(|| EditError::NoSuchPin {
            node: c.source_node.clone(),
            pin: c.source_pin.clone(),
            side: "output",
        })?;
    let to = dst
        .inputs
        .iter()
        .find(|p| p.id == c.target_pin)
        .ok_or_else(|| EditError::NoSuchPin {
            node: c.target_node.clone(),
            pin: c.target_pin.clone(),
            side: "input",
        })?;
    let exec = c.connection_type == ConnectionType::Execution;
    if from.data_type().is_execution() != exec || to.data_type().is_execution() != exec {
        return Err(EditError::KindMismatch);
    }
    if graph.connections.iter().any(|e| same_edge(e, c)) {
        return Err(EditError::DuplicateConnection);
    }
    if !exec
        && graph.connections.iter().any(|e| {
            e.connection_type == ConnectionType::Data
                && e.target_node == c.target_node
                && e.target_pin == c.target_pin
        })
    {
        return Err(EditError::InputOccupied(c.target_pin.clone()));
    }
    if exec
        && graph.connections.iter().any(|e| {
            e.connection_type == ConnectionType::Execution
                && e.source_node == c.source_node
                && e.source_pin == c.source_pin
        })
    {
        return Err(EditError::ExecOutputOccupied(c.source_pin.clone()));
    }
    Ok(())
}

impl ChangeSet {
    /// Reverses the changes (last first). The graph must be in the state the
    /// change set left it in.
    pub fn undo(&self, graph: &mut GraphDescription) {
        for change in self.changes.iter().rev() {
            match change {
                GraphChange::NodeAdded(node) => {
                    graph.nodes.remove(&node.id);
                }
                GraphChange::NodeRemoved { node, connections } => {
                    graph.nodes.insert(node.id.clone(), node.clone());
                    restore_indexed(graph, connections);
                }
                GraphChange::NodeMoved { id, from, .. } => {
                    if let Some(n) = graph.nodes.get_mut(id) {
                        n.position = Position {
                            x: from.0,
                            y: from.1,
                        };
                    }
                }
                GraphChange::ConnectionAdded(c) => {
                    if let Some(i) = graph.connections.iter().position(|e| same_edge(e, c)) {
                        graph.connections.remove(i);
                    }
                }
                GraphChange::ConnectionRemoved(r) => {
                    graph.connections.insert(r.index, r.connection.clone());
                }
                GraphChange::PropertySet { node, key, old, .. } => {
                    if let Some(n) = graph.nodes.get_mut(node) {
                        match old {
                            Some(v) => {
                                n.properties.insert(key.clone(), v.clone());
                            }
                            None => {
                                n.properties.remove(key);
                            }
                        }
                    }
                }
                GraphChange::PinsReplaced {
                    node,
                    old_inputs,
                    old_outputs,
                    removed_connections,
                    ..
                } => {
                    if let Some(n) = graph.nodes.get_mut(node) {
                        n.inputs = old_inputs.clone();
                        n.outputs = old_outputs.clone();
                    }
                    restore_indexed(graph, removed_connections);
                }
            }
        }
    }

    /// IDs of nodes whose content or connectivity this change set touched, in
    /// first-touch order without repeats. Editors use it to invalidate caches.
    pub fn touched_nodes(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut add = |id: &str| {
            if !out.iter().any(|e| e == id) {
                out.push(id.to_string());
            }
        };
        for change in &self.changes {
            match change {
                GraphChange::NodeAdded(n) => add(&n.id),
                GraphChange::NodeRemoved { node, connections } => {
                    add(&node.id);
                    for r in connections {
                        add(&r.connection.source_node);
                        add(&r.connection.target_node);
                    }
                }
                GraphChange::NodeMoved { id, .. } => add(id),
                GraphChange::ConnectionAdded(c) => {
                    add(&c.source_node);
                    add(&c.target_node);
                }
                GraphChange::ConnectionRemoved(r) => {
                    add(&r.connection.source_node);
                    add(&r.connection.target_node);
                }
                GraphChange::PropertySet { node, .. } => add(node),
                GraphChange::PinsReplaced {
                    node,
                    removed_connections,
                    ..
                } => {
                    add(node);
                    for r in removed_connections {
                        add(&r.connection.source_node);
                        add(&r.connection.target_node);
                    }
                }
            }
        }
        out
    }
}
