//! # Code Generation Context
//!
//! Shared context and state for code generation.

use crate::analysis::{DataResolver, ExecutionRouting};
use crate::core::{GraphDescription, NodeMetadataProvider};
use std::collections::HashSet;

/// Context for code generation
///
/// Holds all the state and data structures needed during code generation.
pub struct CodeGeneratorContext<'a, P: NodeMetadataProvider> {
    /// The graph being compiled
    pub graph: &'a GraphDescription,

    /// Node metadata provider
    pub metadata_provider: &'a P,

    /// Data flow resolver
    pub data_resolver: &'a DataResolver,

    /// Execution routing table
    pub exec_routing: &'a ExecutionRouting,

    /// Visited nodes (for cycle detection)
    pub visited: HashSet<String>,

    /// Current indentation level
    pub indent_level: usize,
}

impl<'a, P: NodeMetadataProvider> CodeGeneratorContext<'a, P> {
    pub fn new(
        graph: &'a GraphDescription,
        metadata_provider: &'a P,
        data_resolver: &'a DataResolver,
        exec_routing: &'a ExecutionRouting,
    ) -> Self {
        Self {
            graph,
            metadata_provider,
            data_resolver,
            exec_routing,
            visited: HashSet::new(),
            indent_level: 0,
        }
    }

    /// Get current indentation string
    pub fn indent(&self) -> String {
        "    ".repeat(self.indent_level)
    }

    /// Increase indentation level
    pub fn push_indent(&mut self) {
        self.indent_level += 1;
    }

    /// Decrease indentation level
    pub fn pop_indent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }

    /// Mark a node as visited
    pub fn mark_visited(&mut self, node_id: &str) {
        self.visited.insert(node_id.to_string());
    }

    /// Check if a node has been visited
    pub fn is_visited(&self, node_id: &str) -> bool {
        self.visited.contains(node_id)
    }

    /// Reset visited nodes
    pub fn reset_visited(&mut self) {
        self.visited.clear();
    }
}
