//! # Execution Flow Analysis
//!
//! Tracks execution connections between nodes and builds routing tables.
//!
//! This module analyzes control flow connections to determine which nodes
//! execute after others. Essential for generating proper control flow in
//! the compiled output.
//!
//! # Performance
//!
//! Uses `FxHashMap` for faster routing table lookups.

use crate::core::{GraphDescription, ConnectionType};
use rustc_hash::FxHashMap;

/// Execution routing table.
///
/// Maps (source_node_id, output_pin_name) -> target_node_ids
///
/// # Performance
///
/// Uses `FxHashMap` internally for ~2x faster lookups than standard HashMap.
pub struct ExecutionRouting {
    /// Maps (source_node, output_pin) -> Vec of target nodes
    routes: FxHashMap<(String, String), Vec<String>>,
}

impl ExecutionRouting {
    /// Builds routing table from graph execution connections.
    ///
    /// Analyzes all execution-type connections and creates a lookup table
    /// for fast querying during code generation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use graphy::{ExecutionRouting, GraphDescription};
    ///
    /// let routing = ExecutionRouting::build_from_graph(&graph);
    /// 
    /// // Query which nodes execute after "start" node's "exec" pin
    /// let next_nodes = routing.get_connected_nodes("start", "exec");
    /// ```
    pub fn build_from_graph(graph: &GraphDescription) -> Self {
        // Pre-allocate with estimated capacity
        let connection_count = graph.connections.len();
        let mut routes: FxHashMap<(String, String), Vec<String>> = 
            FxHashMap::with_capacity_and_hasher(connection_count / 2, Default::default());

        for connection in &graph.connections {
            if matches!(connection.connection_type, ConnectionType::Execution) {
                let key = (
                    connection.source_node.clone(),
                    connection.source_pin.clone(),
                );
                routes
                    .entry(key)
                    .or_default()
                    .push(connection.target_node.clone());
            }
        }

        tracing::info!("[ROUTING] Built execution routing table with {} routes", routes.len());
        for ((node_id, pin_name), targets) in &routes {
            tracing::info!("[ROUTING]   ({}, {}) -> {:?}", node_id, pin_name, targets);
        }

        ExecutionRouting { routes }
    }

    /// Retrieves all nodes connected to a specific execution output pin.
    ///
    /// Returns an empty slice if no connections exist.
    ///
    /// # Performance
    ///
    /// This is an O(1) lookup thanks to hash table storage.
    #[inline]
    pub fn get_connected_nodes(&self, node_id: &str, output_pin: &str) -> &[String] {
        self.routes
            .get(&(node_id.to_string(), output_pin.to_string()))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Checks if a node has any outgoing execution connections.
    #[inline]
    pub fn has_execution_outputs(&self, node_id: &str) -> bool {
        self.routes.keys().any(|(id, _)| id == node_id)
    }

    /// Returns all execution output pin names for a node.
    ///
    /// Useful for iterating over all branches in control flow nodes.
    pub fn get_output_pins(&self, node_id: &str) -> Vec<String> {
        self.routes
            .keys()
            .filter(|(id, _)| id == node_id)
            .map(|(_, pin)| pin.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::*;

    #[test]
    fn test_execution_routing() {
        let mut graph = GraphDescription::new("test");

        // Add nodes
        let mut node1 = NodeInstance::new("node1", "print", Position::zero());
        node1.add_output_pin("exec_out", DataType::Execution);
        graph.add_node(node1);

        let mut node2 = NodeInstance::new("node2", "print", Position::zero());
        node2.add_input_pin("exec_in", DataType::Execution);
        graph.add_node(node2);

        // Add execution connection
        graph.add_connection(Connection::execution("node1", "exec_out", "node2", "exec_in"));

        // Build routing
        let routing = ExecutionRouting::build_from_graph(&graph);

        // Test
        let connected = routing.get_connected_nodes("node1", "exec_out");
        assert_eq!(connected, &["node2"]);
    }
}
