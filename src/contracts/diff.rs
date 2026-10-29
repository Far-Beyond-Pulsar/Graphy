//! Turns "this graph should become that graph" into edit commands, so a whole
//! document replacement (an AI edit, a revert) is one undoable transaction like
//! any other edit.

use std::collections::HashSet;

use crate::{Connection, GraphCommand, GraphDescription, NodeInstance};

fn edge_key(c: &Connection) -> (&str, &str, &str, &str) {
    (&c.source_node, &c.source_pin, &c.target_node, &c.target_pin)
}

fn pins_equal(a: &NodeInstance, b: &NodeInstance) -> bool {
    let same = |x: &[crate::PinInstance], y: &[crate::PinInstance]| {
        x.len() == y.len()
            && x.iter().zip(y).all(|(p, q)| {
                p.id == q.id && p.pin.name == q.pin.name && p.pin.data_type == q.pin.data_type
            })
    };
    same(&a.inputs, &b.inputs) && same(&a.outputs, &b.outputs)
}

fn json_equal(a: &crate::JsonValue, b: &crate::JsonValue) -> bool {
    a == b
}

/// Commands that turn `before` into `after`: nodes whose type differs are
/// replaced, others are moved, re-pinned and have their properties set;
/// connections are added or removed to match; comments are replaced when they
/// differ. Connection ids and comment ids come from `after`.
pub fn diff_commands(before: &GraphDescription, after: &GraphDescription) -> Vec<GraphCommand> {
    let mut cmds = Vec::new();

    // Nodes that must be rebuilt: gone, or a different type.
    let replaced: HashSet<&str> = before
        .nodes
        .iter()
        .filter(|(id, n)| {
            after
                .nodes
                .get(*id)
                .map_or(true, |m| m.node_type != n.node_type)
        })
        .map(|(id, _)| id.as_str())
        .collect();

    // Edges that disappear (or touch a node about to be removed, which takes
    // them along).
    let after_edges: HashSet<_> = after.connections.iter().map(edge_key).collect();
    for c in &before.connections {
        let touches_replaced =
            replaced.contains(c.source_node.as_str()) || replaced.contains(c.target_node.as_str());
        if !touches_replaced && !after_edges.contains(&edge_key(c)) {
            cmds.push(GraphCommand::Disconnect(c.clone()));
        }
    }

    let mut ids: Vec<&String> = before.nodes.keys().collect();
    ids.sort();
    for id in ids {
        if replaced.contains(id.as_str()) {
            cmds.push(GraphCommand::RemoveNode { id: id.clone() });
        }
    }

    let mut ids: Vec<&String> = after.nodes.keys().collect();
    ids.sort();
    for id in ids {
        let new = &after.nodes[id];
        match before.nodes.get(id) {
            Some(old) if !replaced.contains(id.as_str()) => {
                if (old.position.x, old.position.y) != (new.position.x, new.position.y) {
                    cmds.push(GraphCommand::MoveNode {
                        id: id.clone(),
                        x: new.position.x,
                        y: new.position.y,
                    });
                }
                if !pins_equal(old, new) {
                    cmds.push(GraphCommand::SetPins {
                        node: id.clone(),
                        inputs: new.inputs.clone(),
                        outputs: new.outputs.clone(),
                    });
                }
                for (k, v) in &new.properties {
                    if old.properties.get(k).map_or(true, |o| !json_equal(o, v)) {
                        cmds.push(GraphCommand::SetProperty {
                            node: id.clone(),
                            key: k.clone(),
                            value: Some(v.clone()),
                        });
                    }
                }
                for k in old.properties.keys() {
                    if !new.properties.contains_key(k) {
                        cmds.push(GraphCommand::SetProperty {
                            node: id.clone(),
                            key: k.clone(),
                            value: None,
                        });
                    }
                }
            }
            _ => cmds.push(GraphCommand::AddNode(new.clone())),
        }
    }

    // Connections to (re)create: everything in `after` that `before` lacked or
    // that lost its endpoint to a replaced node.
    let before_edges: HashSet<_> = before.connections.iter().map(edge_key).collect();
    for c in &after.connections {
        let touches_replaced =
            replaced.contains(c.source_node.as_str()) || replaced.contains(c.target_node.as_str());
        if touches_replaced || !before_edges.contains(&edge_key(c)) {
            cmds.push(GraphCommand::Connect(c.clone()));
        }
    }

    let comments_equal = before.comments.len() == after.comments.len()
        && before.comments.iter().zip(&after.comments).all(|(a, b)| {
            a.id == b.id
                && a.text == b.text
                && (a.position.x, a.position.y) == (b.position.x, b.position.y)
                && a.size == b.size
                && a.color == b.color
                && a.contained_node_ids == b.contained_node_ids
        });
    if !comments_equal {
        for index in (0..before.comments.len()).rev() {
            cmds.push(GraphCommand::RemoveComment { index });
        }
        for (index, comment) in after.comments.iter().enumerate() {
            cmds.push(GraphCommand::AddComment {
                index,
                comment: comment.clone(),
            });
        }
    }
    if before.zip_ties != after.zip_ties {
        for index in (0..before.zip_ties.len()).rev() {
            cmds.push(GraphCommand::RemoveZipTie { index });
        }
        for (index, tie) in after.zip_ties.iter().enumerate() {
            cmds.push(GraphCommand::AddZipTie {
                index,
                tie: tie.clone(),
            });
        }
    }
    cmds
}
