//! # Code Generation Strategies
//!
//! Traits and utilities for implementing code generation strategies.

use crate::core::{NodeInstance, NodeMetadata};
use crate::GraphyError;

/// Trait for code generation strategies
///
/// Implement this trait to create custom code generators for different
/// target languages or platforms.
pub trait CodeGenerator {
    /// Generate code for a pure node (inline expression)
    fn generate_pure_node(
        &self,
        node: &NodeInstance,
        metadata: &NodeMetadata,
    ) -> Result<String, GraphyError>;

    /// Generate code for a function node (statement with side effects)
    fn generate_function_node(
        &self,
        node: &NodeInstance,
        metadata: &NodeMetadata,
    ) -> Result<String, GraphyError>;

    /// Generate code for a control flow node (branching)
    fn generate_control_flow_node(
        &self,
        node: &NodeInstance,
        metadata: &NodeMetadata,
    ) -> Result<String, GraphyError>;

    /// Generate code for an event node (entry point)
    fn generate_event_node(
        &self,
        node: &NodeInstance,
        metadata: &NodeMetadata,
    ) -> Result<String, GraphyError>;

    /// Generate the complete program from a graph
    fn generate_program(&self) -> Result<String, GraphyError>;
}

/// Helper for collecting node arguments
pub fn collect_node_arguments(
    node: &NodeInstance,
    metadata: &NodeMetadata,
) -> Result<Vec<String>, GraphyError> {
    let mut args = Vec::new();

    for param in &metadata.params {
        // Look for property value or default
        if let Some(prop_value) = node.properties.get(&param.name) {
            // Convert property value to string
            args.push(format!("{:?}", prop_value));
        } else {
            // Use default value for the type
            args.push(crate::utils::get_default_value_for_type(&param.param_type));
        }
    }

    Ok(args)
}
