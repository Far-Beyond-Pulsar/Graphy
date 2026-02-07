//! # Node Metadata
//!
//! Metadata structures and traits for node definitions.

use super::{NodeTypes, TypeInfo};
use serde::{Deserialize, Serialize};

/// Parameter definition for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamInfo {
    pub name: String,
    pub param_type: String,
}

impl ParamInfo {
    pub fn new(name: impl Into<String>, param_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            param_type: param_type.into(),
        }
    }
}

/// Complete metadata for a node type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// Node name/identifier
    pub name: String,

    /// Node type classification
    pub node_type: NodeTypes,

    /// Category for organization
    pub category: String,

    /// Input parameters
    pub params: Vec<ParamInfo>,

    /// Return type (if any)
    pub return_type: Option<TypeInfo>,

    /// Execution output pin names (for control flow and events)
    pub exec_outputs: Vec<String>,

    /// Required imports for code generation
    pub imports: Vec<String>,

    /// Source code of the function (for inlining)
    pub function_source: String,
}

impl NodeMetadata {
    pub fn new(name: impl Into<String>, node_type: NodeTypes, category: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            node_type,
            category: category.into(),
            params: Vec::new(),
            return_type: None,
            exec_outputs: Vec::new(),
            imports: Vec::new(),
            function_source: String::new(),
        }
    }

    pub fn with_params(mut self, params: Vec<ParamInfo>) -> Self {
        self.params = params;
        self
    }

    pub fn with_return_type(mut self, return_type: impl Into<TypeInfo>) -> Self {
        self.return_type = Some(return_type.into());
        self
    }

    pub fn with_exec_outputs(mut self, exec_outputs: Vec<String>) -> Self {
        self.exec_outputs = exec_outputs;
        self
    }

    pub fn with_imports(mut self, imports: Vec<String>) -> Self {
        self.imports = imports;
        self
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.function_source = source.into();
        self
    }
}

/// Trait for providing node metadata
///
/// Implement this trait to integrate your custom node system with Graphy.
pub trait NodeMetadataProvider {
    /// Get metadata for a node type by name
    fn get_node_metadata(&self, node_type: &str) -> Option<&NodeMetadata>;

    /// Get all available node types
    fn get_all_nodes(&self) -> Vec<&NodeMetadata>;

    /// Get nodes by category
    fn get_nodes_by_category(&self, category: &str) -> Vec<&NodeMetadata>;
}
