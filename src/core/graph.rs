//! # Graph Representation
//!
//! The main graph data structure.

use super::{Connection, NodeInstance};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Graph metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMetadata {
    pub name: String,
    pub description: String,
    pub version: String,
    pub created_at: String,
    pub modified_at: String,
}

impl GraphMetadata {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            version: "1.0.0".to_string(),
            created_at: String::new(),
            modified_at: String::new(),
        }
    }
}

/// Complete graph description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDescription {
    /// Metadata about the graph
    pub metadata: GraphMetadata,

    /// All nodes in the graph (indexed by ID)
    pub nodes: HashMap<String, NodeInstance>,

    /// All connections in the graph
    pub connections: Vec<Connection>,

    /// Comments (for documentation in the visual editor)
    pub comments: Vec<GraphComment>,
}

/// A comment in the graph (for visual documentation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphComment {
    pub text: String,
    pub position: super::Position,
    pub size: (f64, f64),
}

impl GraphDescription {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            metadata: GraphMetadata::new(name),
            nodes: HashMap::new(),
            connections: Vec::new(),
            comments: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: NodeInstance) {
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn add_connection(&mut self, connection: Connection) {
        self.connections.push(connection);
    }

    pub fn get_node(&self, id: &str) -> Option<&NodeInstance> {
        self.nodes.get(id)
    }

    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut NodeInstance> {
        self.nodes.get_mut(id)
    }
}
