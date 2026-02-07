//! # Execution Flow Analysis
//!
//! Tracks execution connections between nodes.

use crate::core::{GraphDescription, ConnectionType};
use std::collections::HashMap;

/// Execution routing table
///
/// Maps (source_node_id, output_pin_name) -> Vec<target_node_ids>
pub struct ExecutionRouting {
    routes: HashMap<(String, String), Vec<String>>,
}

impl ExecutionRouting {
    /// Build routing table from graph connections
    pub fn build_from_graph(graph: &GraphDescription) -> Self {
        let mut routes = HashMap::new();

        for connection in &graph.connections {
            if matches!(connection.connection_type, ConnectionType::Execution) {
                let key = (
                    connection.source_node.clone(),
                    connection.source_pin.clone(),
                );
                routes
                    .entry(key)
                    .or_insert_with(Vec::new)
                    .push(connection.target_node.clone());
            }
        }

        tracing::info!("[ROUTING] Built execution routing table with {} routes", routes.len());
        for ((node_id, pin_name), targets) in &routes {
            tracing::info!("[ROUTING]   ({}, {}) -> {:?}", node_id, pin_name, targets);
        }

        ExecutionRouting { routes }
    }

    /// Get all nodes connected to a specific execution output pin
    pub fn get_connected_nodes(&self, node_id: &str, output_pin: &str) -> &[String] {
        self.routes
            .get(&(node_id.to_string(), output_pin.to_string()))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Check if a node has any outgoing execution connections
    pub fn has_execution_outputs(&self, node_id: &str) -> bool {
        self.routes.keys().any(|(id, _)| id == node_id)
    }

    /// Get all execution output pins for a node
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
