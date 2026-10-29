//! Graph invariants.
//!
//! [`check_invariants`] verifies the structural rules every editor edit and
//! every loaded document must satisfy before it reaches a compiler. It is
//! read-only and reports every violation rather than stopping at the first.
//!
//! Rules:
//! - the map key of each node equals `NodeInstance::id`;
//! - pin IDs are unique within a node's inputs and within its outputs;
//! - every connection endpoint resolves to an existing node and pin;
//! - a connection leaves an output pin and enters an input pin;
//! - the connection kind agrees with the pins' execution/data nature;
//! - no duplicate connections;
//! - an input pin has at most one incoming connection, and an execution output
//!   pin has at most one outgoing connection.
//!
//! Dynamic pins (pin lists that change at edit time) are plain edits of
//! `inputs`/`outputs`; the invariants above are exactly what must hold after
//! any such change, which is why a connection to a removed pin is an error.

use std::collections::HashSet;

use crate::{ConnectionType, Diagnostic, GraphDescription, PassName, Severity};

/// Stable diagnostic codes emitted by this module and the resolver helpers.
pub mod codes {
    pub const NODE_KEY_MISMATCH: &str = "G0001";
    pub const DUPLICATE_PIN: &str = "G0002";
    pub const MISSING_NODE: &str = "G0003";
    pub const MISSING_PIN: &str = "G0004";
    pub const WRONG_DIRECTION: &str = "G0005";
    pub const KIND_MISMATCH: &str = "G0006";
    pub const DUPLICATE_CONNECTION: &str = "G0007";
    pub const MULTIPLE_INPUT_DRIVERS: &str = "G0008";
    pub const MULTIPLE_EXEC_OUTPUTS: &str = "G0009";
    pub const TYPE_MISMATCH: &str = "G0100";
    pub const TYPE_UNKNOWN: &str = "G0101";
}

fn pass() -> PassName {
    PassName::Custom("invariants".into())
}

fn err(msg: String, code: &str) -> Diagnostic {
    Diagnostic::new(Severity::Error, pass(), msg).with_code(code)
}

/// Returns all invariant violations in deterministic order (nodes sorted by
/// ID, then connections in document order).
pub fn check_invariants(graph: &GraphDescription) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    let mut ids: Vec<&String> = graph.nodes.keys().collect();
    ids.sort();
    for id in ids {
        let node = &graph.nodes[id];
        if node.id != *id {
            out.push(
                err(
                    format!("node stored under '{id}' declares id '{}'", node.id),
                    codes::NODE_KEY_MISMATCH,
                )
                .at_node(id.clone()),
            );
        }
        for (side, pins) in [("input", &node.inputs), ("output", &node.outputs)] {
            let mut seen = HashSet::new();
            for p in pins {
                if !seen.insert(p.id.as_str()) {
                    out.push(
                        err(
                            format!("duplicate {side} pin '{}'", p.id),
                            codes::DUPLICATE_PIN,
                        )
                        .at_pin(id.clone(), p.id.clone()),
                    );
                }
            }
        }
    }

    let mut seen_conns = HashSet::new();
    let mut input_drivers = HashSet::new();
    let mut exec_sources = HashSet::new();
    for c in &graph.connections {
        let key = (
            c.source_node.as_str(),
            c.source_pin.as_str(),
            c.target_node.as_str(),
            c.target_pin.as_str(),
        );
        if !seen_conns.insert(key) {
            out.push(
                err(
                    format!(
                        "duplicate connection {}.{} -> {}.{}",
                        c.source_node, c.source_pin, c.target_node, c.target_pin
                    ),
                    codes::DUPLICATE_CONNECTION,
                )
                .at_pin(c.target_node.clone(), c.target_pin.clone()),
            );
        }

        let (Some(src), Some(dst)) = (
            graph.nodes.get(&c.source_node),
            graph.nodes.get(&c.target_node),
        ) else {
            for (id, present) in [
                (&c.source_node, graph.nodes.contains_key(&c.source_node)),
                (&c.target_node, graph.nodes.contains_key(&c.target_node)),
            ] {
                if !present {
                    out.push(
                        err(
                            format!("connection references missing node '{id}'"),
                            codes::MISSING_NODE,
                        )
                        .at_node(id.clone()),
                    );
                }
            }
            continue;
        };

        let from = src.outputs.iter().find(|p| p.id == c.source_pin);
        let to = dst.inputs.iter().find(|p| p.id == c.target_pin);
        if from.is_none() {
            let wrong_way = src.inputs.iter().any(|p| p.id == c.source_pin);
            let (code, what) = if wrong_way {
                (codes::WRONG_DIRECTION, "is an input, not an output")
            } else {
                (codes::MISSING_PIN, "does not exist")
            };
            out.push(
                err(format!("source pin '{}' {what}", c.source_pin), code)
                    .at_pin(c.source_node.clone(), c.source_pin.clone()),
            );
        }
        if to.is_none() {
            let wrong_way = dst.outputs.iter().any(|p| p.id == c.target_pin);
            let (code, what) = if wrong_way {
                (codes::WRONG_DIRECTION, "is an output, not an input")
            } else {
                (codes::MISSING_PIN, "does not exist")
            };
            out.push(
                err(format!("target pin '{}' {what}", c.target_pin), code)
                    .at_pin(c.target_node.clone(), c.target_pin.clone()),
            );
        }
        let (Some(from), Some(to)) = (from, to) else {
            continue;
        };

        let exec_conn = c.connection_type == ConnectionType::Execution;
        if from.data_type().is_execution() != exec_conn
            || to.data_type().is_execution() != exec_conn
        {
            out.push(
                err(
                    format!(
                        "{:?} connection joins incompatible pin kinds ({}.{} -> {}.{})",
                        c.connection_type, c.source_node, c.source_pin, c.target_node, c.target_pin
                    ),
                    codes::KIND_MISMATCH,
                )
                .at_pin(c.target_node.clone(), c.target_pin.clone()),
            );
        }

        if !exec_conn && !input_drivers.insert((c.target_node.as_str(), c.target_pin.as_str())) {
            out.push(
                err(
                    format!(
                        "input pin '{}' has more than one incoming data connection",
                        c.target_pin
                    ),
                    codes::MULTIPLE_INPUT_DRIVERS,
                )
                .at_pin(c.target_node.clone(), c.target_pin.clone()),
            );
        }
        if exec_conn && !exec_sources.insert((c.source_node.as_str(), c.source_pin.as_str())) {
            out.push(
                err(
                    format!(
                        "execution output '{}' has more than one outgoing connection",
                        c.source_pin
                    ),
                    codes::MULTIPLE_EXEC_OUTPUTS,
                )
                .at_pin(c.source_node.clone(), c.source_pin.clone()),
            );
        }
    }

    out
}
