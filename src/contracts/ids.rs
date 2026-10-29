//! Stable identifiers.
//!
//! Node and pin IDs are opaque strings chosen once and never rewritten. They
//! are the join key between the authored document, undo history, diagnostics
//! and (later) UI selection, so every operation in this crate preserves them.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::GraphDescription;

/// Identifier of a node within one graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A pin addressed by owning node and pin ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PinRef {
    pub node: String,
    pub pin: String,
}

impl PinRef {
    pub fn new(node: impl Into<String>, pin: impl Into<String>) -> Self {
        Self {
            node: node.into(),
            pin: pin.into(),
        }
    }
}

impl std::fmt::Display for PinRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.node, self.pin)
    }
}

/// Deterministic ID minting for new nodes.
///
/// IDs have the form `<prefix>_<n>`; `n` starts above every existing
/// `<prefix>_<n>` in the graph, so allocation is a pure function of the graph
/// and never reuses an ID that is still present.
#[derive(Debug, Clone)]
pub struct IdAllocator {
    prefix: String,
    next: u64,
    taken: HashSet<String>,
}

impl IdAllocator {
    pub fn for_graph(graph: &GraphDescription, prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        let lead = format!("{prefix}_");
        let next = graph
            .nodes
            .keys()
            .filter_map(|id| id.strip_prefix(&lead)?.parse::<u64>().ok())
            .max()
            .map_or(0, |max| max + 1);
        Self {
            prefix,
            next,
            taken: graph.nodes.keys().cloned().collect(),
        }
    }

    /// Returns a fresh ID that is not present in the graph or already issued.
    pub fn allocate(&mut self) -> NodeId {
        loop {
            let id = format!("{}_{}", self.prefix, self.next);
            self.next += 1;
            if self.taken.insert(id.clone()) {
                return NodeId(id);
            }
        }
    }
}
