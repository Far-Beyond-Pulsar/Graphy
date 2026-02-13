//! # Connection Representation
//!
//! Data structures for connections between nodes in the graph.
//!
//! Connections link pins on different nodes and can carry either:
//! - **Data**: Values flowing between computational nodes
//! - **Execution**: Control flow between operations
//!
//! # Example
//!
//! ```
//! use graphy::{Connection, ConnectionType};
//!
//! // Data connection
//! let data_conn = Connection::data("add_1", "result", "print_1", "value");
//!
//! // Execution connection
//! let exec_conn = Connection::execution("start", "exec", "print_1", "exec");
//! ```

use serde::{Deserialize, Serialize};

/// Type of connection between nodes.
///
/// Determines whether the connection carries data values or execution flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionType {
    /// Data connection that passes values between nodes
    ///
    /// Data flows from an output pin to an input pin, transferring computed values.
    Data,

    /// Execution connection that determines control flow
    ///
    /// Execution flows from one node to the next, determining the order of operations.
    Execution,
}

/// A connection between two pins on different nodes.
///
/// Connections are validated during the analysis phase to ensure:
/// - Both nodes exist
/// - Both pins exist  
/// - Pin types are compatible
/// - No circular dependencies (for data connections)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// ID of the source node
    pub source_node: String,

    /// ID of the output pin on the source node
    pub source_pin: String,

    /// ID of the target node
    pub target_node: String,

    /// ID of the input pin on the target node
    pub target_pin: String,

    /// Type of connection (data or execution)
    pub connection_type: ConnectionType,
}

impl Connection {
    /// Creates a new connection with the specified type.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::{Connection, ConnectionType};
    ///
    /// let conn = Connection::new(
    ///     "node_1", "out",
    ///     "node_2", "in",
    ///     ConnectionType::Data
    /// );
    /// ```
    #[inline]
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

    /// Creates a data connection.
    ///
    /// Convenience method equivalent to `new(..., ConnectionType::Data)`.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::Connection;
    ///
    /// let conn = Connection::data("add", "result", "print", "value");
    /// ```
    #[inline]
    pub fn data(
        source_node: impl Into<String>,
        source_pin: impl Into<String>,
        target_node: impl Into<String>,
        target_pin: impl Into<String>,
    ) -> Self {
        Self::new(source_node, source_pin, target_node, target_pin, ConnectionType::Data)
    }

    /// Creates an execution connection.
    ///
    /// Convenience method equivalent to `new(..., ConnectionType::Execution)`.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::Connection;
    ///
    /// let conn = Connection::execution("start", "exec", "print", "exec");
    /// ```
    #[inline]
    pub fn execution(
        source_node: impl Into<String>,
        source_pin: impl Into<String>,
        target_node: impl Into<String>,
        target_pin: impl Into<String>,
    ) -> Self {
        Self::new(source_node, source_pin, target_node, target_pin, ConnectionType::Execution)
    }
}
