//! # Graph Representation
//!
//! Core data structures for representing visual node graphs.
//!
//! A graph consists of:
//! - **Nodes**: Computational or control flow units
//! - **Connections**: Data or execution flow links between nodes
//! - **Comments**: Visual annotations for documentation
//!
//! # Example
//!
//! ```
//! use graphy::{GraphDescription, NodeInstance, Connection, Position, ConnectionType};
//!
//! let mut graph = GraphDescription::new("my_graph");
//!
//! // Add a node
//! let node = NodeInstance::new("add_1", "math.add", Position::zero());
//! graph.add_node(node);
//!
//! // Add a connection
//! graph.add_connection(Connection {
//!     source_node: "add_1".to_string(),
//!     source_pin: "result".to_string(),
//!     target_node: "print_1".to_string(),
//!     target_pin: "value".to_string(),
//!     connection_type: ConnectionType::Data,
//! });
//! ```

use super::{Connection, NodeInstance};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metadata about a graph.
///
/// Contains descriptive information including name, version, and timestamps.
/// All fields are serialized for persistence in visual editors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMetadata {
    /// Human-readable name of the graph
    pub name: String,
    
    /// Optional description of the graph's purpose
    pub description: String,
    
    /// Semantic version string (e.g., "1.0.0")
    pub version: String,
    
    /// ISO 8601 timestamp of creation
    pub created_at: String,
    
    /// ISO 8601 timestamp of last modification
    pub modified_at: String,
}

impl GraphMetadata {
    /// Creates new graph metadata with default values.
    ///
    /// Sets version to "1.0.0" and leaves timestamps empty.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::core::GraphMetadata;
    ///
    /// let meta = GraphMetadata::new("shader_graph");
    /// assert_eq!(meta.name, "shader_graph");
    /// assert_eq!(meta.version, "1.0.0");
    /// ```
    #[inline]
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

/// Complete graph description containing all nodes, connections, and metadata.
///
/// This is the primary data structure that gets serialized/deserialized
/// and passed through the compilation pipeline.
///
/// # Performance
///
/// Nodes are stored in a `HashMap` for O(1) lookups by ID.
/// For graphs with 1000+ nodes, this provides significant performance benefits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDescription {
    /// Metadata about the graph
    pub metadata: GraphMetadata,

    /// All nodes in the graph, indexed by their unique ID
    pub nodes: HashMap<String, NodeInstance>,

    /// All connections between nodes
    pub connections: Vec<Connection>,

    /// Visual comments for documentation in editors
    pub comments: Vec<GraphComment>,
}

/// A visual comment in the graph for documentation purposes.
///
/// Comments appear as text boxes in visual editors and are preserved
/// during serialization but don't affect code generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphComment {
    /// The comment text content
    pub text: String,
    
    /// Position in the visual editor
    pub position: super::Position,
    
    /// Size of the comment box (width, height)
    pub size: (f64, f64),
}

impl GraphDescription {
    /// Creates a new empty graph with the given name.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::GraphDescription;
    ///
    /// let graph = GraphDescription::new("my_graph");
    /// assert_eq!(graph.metadata.name, "my_graph");
    /// assert!(graph.nodes.is_empty());
    /// ```
    #[inline]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            metadata: GraphMetadata::new(name),
            nodes: HashMap::new(),
            connections: Vec::new(),
            comments: Vec::new(),
        }
    }

    /// Adds a node to the graph.
    ///
    /// If a node with the same ID already exists, it will be replaced.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::{GraphDescription, NodeInstance, Position};
    ///
    /// let mut graph = GraphDescription::new("test");
    /// let node = NodeInstance::new("node_1", "math.add", Position::zero());
    /// graph.add_node(node);
    /// assert!(graph.get_node("node_1").is_some());
    /// ```
    #[inline]
    pub fn add_node(&mut self, node: NodeInstance) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Adds a connection to the graph.
    ///
    /// Connections are validated during analysis phases, not at insertion time.
    #[inline]
    pub fn add_connection(&mut self, connection: Connection) {
        self.connections.push(connection);
    }

    /// Gets an immutable reference to a node by ID.
    ///
    /// Returns `None` if the node doesn't exist.
    ///
    /// # Performance
    ///
    /// This is an O(1) operation thanks to `HashMap` storage.
    #[inline]
    pub fn get_node(&self, id: &str) -> Option<&NodeInstance> {
        self.nodes.get(id)
    }

    /// Gets a mutable reference to a node by ID.
    ///
    /// Returns `None` if the node doesn't exist.
    ///
    /// # Performance
    ///
    /// This is an O(1) operation thanks to `HashMap` storage.
    #[inline]
    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut NodeInstance> {
        self.nodes.get_mut(id)
    }
}
