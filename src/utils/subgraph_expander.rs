//! # Sub-Graph Expander
//!
//! Inline expansion of macro and collapsed-region nodes.
//!
//! The expander takes a [`BlueprintDocument`] (or a single [`GraphDescription`]
//! with a graph library) and rewrites every `SubgraphCall` node into its
//! constituent nodes, wiring connections through the `SubgraphEntry` /
//! `SubgraphExit` interface nodes. The result is a flat graph that can be
//! passed to data-flow and execution-flow analysis without any further
//! understanding of subgraph structure.
//!
//! ## Algorithm
//!
//! For each `SubgraphCall` node `C` in the parent graph:
//!
//! 1. Look up the target graph `G` by ID.
//! 2. Clone all nodes in `G`, prefixing their IDs with `"<call_id>__"` to
//!    avoid collision with the parent's node IDs.
//! 3. Gather the `SubgraphEntry` and `SubgraphExit` nodes in `G`.
//! 4. For each incoming data connection `(X.out → C.in_pin)` in the parent,
//!    rewire it to `(X.out → <entry_node>.<in_pin>)`.
//! 5. For each outgoing data connection `(C.out_pin → Y.in)` in the parent,
//!    rewire it to `(<exit_node>.<out_pin> → Y.in)`.
//! 6. For each exec connection through `C`, rewire through the entry/exit exec
//!    pins of the cloned nodes.
//! 7. Remove the `SubgraphCall` node from the parent.
//! 8. Recurse on the result until no `SubgraphCall` nodes remain (handles
//!    nested macros).
//!
//! ## Cycle Detection
//!
//! The expander tracks a `visited` stack of graph IDs. If a graph is found in
//! the stack when about to expand it, the expansion is aborted with a
//! [`GraphyError::GraphExpansion`] describing the cycle.
//!
//! ## Flat-graph mode
//!
//! [`SubGraphExpander::expand_all_flat`] accepts a single `GraphDescription`
//! plus a `HashMap<String, GraphDescription>` library, for callers that don't
//! use [`BlueprintDocument`].

use std::collections::HashMap;

use crate::{
    core::{Connection, GraphDescription, NodeKind},
    GraphyError, Result,
};

// ─────────────────────────────────────────────────────────────────────────────
// GraphLibrary trait
// ─────────────────────────────────────────────────────────────────────────────

/// Provides sub-graph definitions by ID to the expander.
pub trait GraphLibrary {
    /// Returns a clone of the graph with the given ID, or `None`.
    fn get_graph(&self, id: &str) -> Option<GraphDescription>;

    /// Returns `true` if the library contains a graph with this ID.
    fn contains(&self, id: &str) -> bool {
        self.get_graph(id).is_some()
    }
}

/// `HashMap<String, GraphDescription>` works as a library directly.
impl GraphLibrary for HashMap<String, GraphDescription> {
    fn get_graph(&self, id: &str) -> Option<GraphDescription> {
        self.get(id).cloned()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SubGraphExpander
// ─────────────────────────────────────────────────────────────────────────────

/// Expands all subgraph call nodes in a graph by inlining the sub-graphs.
///
/// The expander is stateless — create a new one each time or reuse the same
/// instance (it carries no mutable state).
#[derive(Default)]
pub struct SubGraphExpander;

impl SubGraphExpander {
    pub fn new() -> Self { Self }

    // ── Public API ────────────────────────────────────────────────────────────

    /// Backward-compatible no-library expansion.
    ///
    /// If the graph has no subgraph-call nodes this is a no-op. If it does,
    /// callers should use [`expand_all_flat`] and supply a [`GraphLibrary`].
    pub fn expand_all(&self, _graph: &mut GraphDescription) -> Result<()> {
        Ok(())
    }

    /// Expands all subgraph-call nodes in `root` using graphs from `library`.
    ///
    /// Modifies `root` in place. Recurses into nested macros.
    ///
    /// # Errors
    ///
    /// - `GraphyError::GraphExpansion` if a cycle is detected or a referenced
    ///    sub-graph is not found.
    pub fn expand_all_flat(
        &self,
        root: &mut GraphDescription,
        library: &dyn GraphLibrary,
    ) -> Result<()> {
        let mut visited: Vec<String> = Vec::new();
        self.expand_recursive(root, library, &mut visited)
    }

    /// Convenience wrapper: creates an expander and runs expansion.
    pub fn expand(
        root: &mut GraphDescription,
        library: &dyn GraphLibrary,
    ) -> Result<()> {
        Self::new().expand_all_flat(root, library)
    }

    // ── Recursive core ────────────────────────────────────────────────────────

    fn expand_recursive(
        &self,
        graph: &mut GraphDescription,
        library: &dyn GraphLibrary,
        visited: &mut Vec<String>,
    ) -> Result<()> {
        // Collect all subgraph-call node IDs (avoid borrow issues)
        let call_ids: Vec<String> = graph
            .nodes
            .iter()
            .filter(|(_, n)| n.is_subgraph_call())
            .map(|(id, _)| id.clone())
            .collect();

        for call_id in call_ids {
            let node = match graph.nodes.get(&call_id) {
                Some(n) => n.clone(),
                None => continue, // already removed by a previous step
            };

            let subgraph_id = match node.kind() {
                NodeKind::SubgraphCall(id) => id,
                _ => continue,
            };

            // Cycle detection
            if visited.contains(&subgraph_id) {
                return Err(GraphyError::GraphExpansion(format!(
                    "Circular subgraph reference detected: '{}' is already being expanded (cycle: {})",
                    subgraph_id,
                    visited.join(" → ")
                )));
            }

            // Fetch sub-graph
            let mut sub = library.get_graph(&subgraph_id).ok_or_else(|| {
                GraphyError::GraphExpansion(format!(
                    "SubgraphCall '{}' references unknown graph '{}'",
                    call_id, subgraph_id
                ))
            })?;

            // Recursively expand the sub-graph first
            visited.push(subgraph_id.clone());
            self.expand_recursive(&mut sub, library, visited)?;
            visited.pop();

            // Inline the expanded sub into the parent
            self.inline_subgraph(graph, &call_id, &sub)?;
        }

        Ok(())
    }

    // ── Inlining ─────────────────────────────────────────────────────────────

    fn inline_subgraph(
        &self,
        parent: &mut GraphDescription,
        call_id: &str,
        sub: &GraphDescription,
    ) -> Result<()> {
        let prefix = format!("{}__", call_id);

        // Clone all non-entry/exit nodes from sub into parent, with prefix
        let mut entry_node_id: Option<String> = None;
        let mut exit_node_id:  Option<String> = None;

        for (_, node) in &sub.nodes {
            match node.kind() {
                NodeKind::SubgraphEntry => {
                    entry_node_id = Some(format!("{}{}", prefix, node.id));
                }
                NodeKind::SubgraphExit => {
                    exit_node_id = Some(format!("{}{}", prefix, node.id));
                }
                _ => {}
            }
            let prefixed = node.with_id_prefix(&prefix);
            parent.nodes.insert(prefixed.id.clone(), prefixed);
        }

        // Clone all connections from sub, with prefixed node/pin IDs
        for conn in &sub.connections {
            parent.connections.push(Connection {
                source_node: format!("{}{}", prefix, conn.source_node),
                source_pin: conn.source_pin.clone(),
                target_node: format!("{}{}", prefix, conn.target_node),
                target_pin: conn.target_pin.clone(),
                connection_type: conn.connection_type,
            });
        }

        // Rewire parent connections that touched the call node
        let mut to_add: Vec<Connection> = Vec::new();
        let mut to_remove: Vec<usize> = Vec::new();

        for (i, conn) in parent.connections.iter().enumerate() {
            if conn.target_node == call_id {
                // Incoming to call site → rewire to entry node
                if let Some(entry_id) = &entry_node_id {
                    to_add.push(Connection {
                        source_node: conn.source_node.clone(),
                        source_pin: conn.source_pin.clone(),
                        target_node: entry_id.clone(),
                        target_pin: conn.target_pin.clone(),
                        connection_type: conn.connection_type,
                    });
                    to_remove.push(i);
                }
            } else if conn.source_node == call_id {
                // Outgoing from call site → rewire from exit node
                if let Some(exit_id) = &exit_node_id {
                    to_add.push(Connection {
                        source_node: exit_id.clone(),
                        source_pin: conn.source_pin.clone(),
                        target_node: conn.target_node.clone(),
                        target_pin: conn.target_pin.clone(),
                        connection_type: conn.connection_type,
                    });
                    to_remove.push(i);
                }
            }
        }

        // Apply removals in reverse order to preserve indices
        for &i in to_remove.iter().rev() {
            parent.connections.swap_remove(i);
        }
        parent.connections.extend(to_add);

        // Remove the call node itself
        parent.nodes.remove(call_id);

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{DataType, NodeInstance, Position};

    fn make_add_node(id: &str) -> NodeInstance {
        let mut n = NodeInstance::new(id, "math.add", Position::zero());
        n.add_input_pin("a", DataType::typed("f64"));
        n.add_input_pin("b", DataType::typed("f64"));
        n.add_output_pin("result", DataType::typed("f64"));
        n
    }

    fn make_entry_node(id: &str) -> NodeInstance {
        let mut n = NodeInstance::new(id, "subgraph_entry", Position::zero());
        n.add_output_pin("value", DataType::typed("f64"));
        n
    }

    fn make_exit_node(id: &str) -> NodeInstance {
        let mut n = NodeInstance::new(id, "subgraph_exit", Position::zero());
        n.add_input_pin("result", DataType::typed("f64"));
        n
    }

    fn make_call_node(id: &str, subgraph_id: &str) -> NodeInstance {
        let mut n = NodeInstance::new(id, format!("macro:{}", subgraph_id), Position::zero());
        n.add_input_pin("value", DataType::typed("f64"));
        n.add_output_pin("result", DataType::typed("f64"));
        n
    }

    #[test]
    fn simple_inline() {
        // Sub-graph: entry → add → exit
        let mut sub = GraphDescription::new("double");
        sub.add_node(make_entry_node("entry"));
        sub.add_node(make_exit_node("exit"));
        sub.add_node(make_add_node("inner_add"));
        sub.add_connection(Connection::data("entry", "value", "inner_add", "a"));
        sub.add_connection(Connection::data("entry", "value", "inner_add", "b"));
        sub.add_connection(Connection::data("inner_add", "result", "exit", "result"));

        // Parent: source → call → sink
        let mut parent = GraphDescription::new("parent");
        let mut src = NodeInstance::new("src", "const", Position::zero());
        src.add_output_pin("out", DataType::typed("f64"));
        parent.add_node(src);
        parent.add_node(make_call_node("call1", "double"));
        let mut sink = NodeInstance::new("sink", "print", Position::zero());
        sink.add_input_pin("in", DataType::typed("f64"));
        parent.add_node(sink);

        parent.add_connection(Connection::data("src", "out", "call1", "value"));
        parent.add_connection(Connection::data("call1", "result", "sink", "in"));

        // Library
        let mut lib = HashMap::new();
        lib.insert("double".to_string(), sub);

        SubGraphExpander::expand(&mut parent, &lib).unwrap();

        // call1 node must be gone
        assert!(!parent.nodes.contains_key("call1"), "call node should be removed");

        // entry and exit must be present under prefixed names
        assert!(parent.nodes.contains_key("call1__entry"), "entry node missing");
        assert!(parent.nodes.contains_key("call1__exit"), "exit node missing");
        assert!(parent.nodes.contains_key("call1__inner_add"), "inner_add missing");

        // Connections should not reference call1 any more
        for conn in &parent.connections {
            assert_ne!(conn.source_node, "call1");
            assert_ne!(conn.target_node, "call1");
        }
    }

    #[test]
    fn cycle_detection() {
        // Graph A calls B, B calls A
        let mut a = GraphDescription::new("a");
        a.add_node(make_call_node("call_b", "b"));
        let mut b = GraphDescription::new("b");
        b.add_node(make_call_node("call_a", "a"));

        let mut lib = HashMap::new();
        lib.insert("a".to_string(), a.clone());
        lib.insert("b".to_string(), b);

        let result = SubGraphExpander::expand(&mut a, &lib);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Circular") || msg.contains("circular") || msg.contains("cycle"),
            "unexpected error message: {}", msg);
    }

    #[test]
    fn missing_library_graph() {
        let mut parent = GraphDescription::new("parent");
        parent.add_node(make_call_node("call_ghost", "nonexistent"));

        let lib: HashMap<String, GraphDescription> = HashMap::new();
        let result = SubGraphExpander::expand(&mut parent, &lib);
        assert!(result.is_err());
    }
}
