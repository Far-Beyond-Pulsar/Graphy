//! # Connection Representation
//!
//! Data structures for connections between nodes.

use serde::{Deserialize, Serialize};

/// Connection type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionType {
    /// Data connection (passes values)
    Data,

    /// Execution connection (control flow)
    Execution,
}

/// A connection between two node pins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// Source node ID
    pub source_node: String,

    /// Source pin ID
    pub source_pin: String,

    /// Target node ID
    pub target_node: String,

    /// Target pin ID
    pub target_pin: String,

    /// Connection type
    pub connection_type: ConnectionType,
}

impl Connection {
    pub fn new(
        source_node: impl Into<String>,
        source_pin: impl Into<String>,
        target_node: impl Into<String>,
        target_pin: impl Into<String>,
        connection_type: ConnectionType,
    ) -> Self {
        Self {
            source_node: source_node.into(),
            source_pin: source_pin.into(),
            target_node: target_node.into(),
            target_pin: target_pin.into(),
            connection_type,
        }
    }

    pub fn data(
        source_node: impl Into<String>,
        source_pin: impl Into<String>,
        target_node: impl Into<String>,
        target_pin: impl Into<String>,
    ) -> Self {
        Self::new(source_node, source_pin, target_node, target_pin, ConnectionType::Data)
    }

    pub fn execution(
        source_node: impl Into<String>,
        source_pin: impl Into<String>,
        target_node: impl Into<String>,
        target_pin: impl Into<String>,
    ) -> Self {
        Self::new(source_node, source_pin, target_node, target_pin, ConnectionType::Execution)
    }
}
