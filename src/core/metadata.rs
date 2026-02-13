//! # Node Metadata
//!
//! Metadata structures and traits for node type definitions.
//!
//! The metadata system describes the interface and behavior of each node type,
//! including:
//! - Input/output parameters and their types
//! - Node classification (pure, function, control flow, event)
//! - Code generation information (imports, source code)
//!
//! # Example
//!
//! ```
//! use graphy::{NodeMetadata, NodeTypes, ParamInfo};
//!
//! let meta = NodeMetadata::new("add", NodeTypes::pure, "Math")
//!     .with_params(vec![
//!         ParamInfo::new("a", "f64"),
//!         ParamInfo::new("b", "f64"),
//!     ])
//!     .with_return_type("f64")
//!     .with_source("a + b");
//! ```

use super::{NodeTypes, TypeInfo};
use serde::{Deserialize, Serialize};

/// Parameter definition for a node input.
///
/// Describes an input parameter including its name and type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamInfo {
    /// Parameter name (used as variable name in generated code)
    pub name: String,
    
    /// Rust type string (e.g., "f64", "String", "&str")
    pub param_type: String,
}

impl ParamInfo {
    /// Creates a new parameter definition.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::ParamInfo;
    ///
    /// let param = ParamInfo::new("value", "f64");
    /// ```
    #[inline]
    pub fn new(name: impl Into<String>, param_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            param_type: param_type.into(),
        }
    }
}

/// Complete metadata for a node type.
///
/// Contains all information needed to:
/// - Display the node in a visual editor
/// - Validate connections
/// - Generate code
///
/// Use the builder pattern with `with_*` methods to configure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// Node name/identifier (e.g., "add", "print", "branch")
    pub name: String,

    /// Node classification (pure, function, control flow, event)
    pub node_type: NodeTypes,

    /// Category for organization in UI (e.g., "Math", "String", "Flow Control")
    pub category: String,

    /// Input parameters with types
    pub params: Vec<ParamInfo>,

    /// Return type (for pure nodes and functions)
    pub return_type: Option<TypeInfo>,

    /// Execution output pin names (for control flow and events)
    ///
    /// Examples: `vec!["then"]` for simple flow, `vec!["true", "false"]` for branches
    pub exec_outputs: Vec<String>,

    /// Required imports for code generation
    ///
    /// Example: `vec!["use std::io::Write;"]`
    pub imports: Vec<String>,

    /// Source code of the function for inlining
    ///
    /// For pure nodes, this can be an expression like "a + b".
    /// For functions, include the full function body.
    pub function_source: String,
}

impl NodeMetadata {
    /// Creates a new node metadata with the given name, type, and category.
    ///
    /// Use the builder methods to add parameters, return types, etc.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::{NodeMetadata, NodeTypes};
    ///
    /// let meta = NodeMetadata::new("multiply", NodeTypes::pure, "Math");
    /// ```
    #[inline]
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

    /// Sets the input parameters for this node type.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::{NodeMetadata, NodeTypes, ParamInfo};
    ///
    /// let meta = NodeMetadata::new("add", NodeTypes::pure, "Math")
    ///     .with_params(vec![
    ///         ParamInfo::new("a", "f64"),
    ///         ParamInfo::new("b", "f64"),
    ///     ]);
    /// ```
    #[inline]
    #[must_use]
    pub fn with_params(mut self, params: Vec<ParamInfo>) -> Self {
        self.params = params;
        self
    }

    /// Sets the return type for this node.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::{NodeMetadata, NodeTypes};
    ///
    /// let meta = NodeMetadata::new("random", NodeTypes::pure, "Math")
    ///     .with_return_type("f64");
    /// ```
    #[inline]
    #[must_use]
    pub fn with_return_type(mut self, return_type: impl Into<TypeInfo>) -> Self {
        self.return_type = Some(return_type.into());
        self
    }

    /// Sets the execution output pin names.
    ///
    /// Used for control flow nodes to define branching paths.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::{NodeMetadata, NodeTypes};
    ///
    /// // If/else node with two exec outputs
    /// let meta = NodeMetadata::new("branch", NodeTypes::control_flow, "Flow")
    ///     .with_exec_outputs(vec!["true".to_string(), "false".to_string()]);
    /// ```
    #[inline]
    #[must_use]
    pub fn with_exec_outputs(mut self, exec_outputs: Vec<String>) -> Self {
        self.exec_outputs = exec_outputs;
        self
    }

    /// Sets the required imports for code generation.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::{NodeMetadata, NodeTypes};
    ///
    /// let meta = NodeMetadata::new("write_file", NodeTypes::fn_, "IO")
    ///     .with_imports(vec!["use std::fs::File;".to_string()]);
    /// ```
    #[inline]
    #[must_use]
    pub fn with_imports(mut self, imports: Vec<String>) -> Self {
        self.imports = imports;
        self
    }

    /// Sets the source code for this node.
    ///
    /// For pure nodes, this is typically an expression.
    /// For functions, include the full function body.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::{NodeMetadata, NodeTypes};
    ///
    /// let meta = NodeMetadata::new("max", NodeTypes::pure, "Math")
    ///     .with_source("a.max(b)");
    /// ```
    #[inline]
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.function_source = source.into();
        self
    }
}

/// Trait for providing node type metadata.
///
/// Implement this trait to integrate your custom node system with Graphy.
/// The provider acts as a registry of all available node types.
///
/// # Example
///
/// ```
/// use graphy::{NodeMetadata, NodeMetadataProvider, NodeTypes, ParamInfo};
/// use std::collections::HashMap;
///
/// struct MyProvider {
///     nodes: HashMap<String, NodeMetadata>,
/// }
///
/// impl NodeMetadataProvider for MyProvider {
///     fn get_node_metadata(&self, node_type: &str) -> Option<&NodeMetadata> {
///         self.nodes.get(node_type)
///     }
///
///     fn get_all_nodes(&self) -> Vec<&NodeMetadata> {
///         self.nodes.values().collect()
///     }
///
///     fn get_nodes_by_category(&self, category: &str) -> Vec<&NodeMetadata> {
///         self.nodes.values()
///             .filter(|m| m.category == category)
///             .collect()
///     }
/// }
/// ```
pub trait NodeMetadataProvider {
    /// Retrieves metadata for a specific node type.
    ///
    /// Returns `None` if the node type doesn't exist.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let meta = provider.get_node_metadata("math.add");
    /// ```
    fn get_node_metadata(&self, node_type: &str) -> Option<&NodeMetadata>;

    /// Returns all available node types.
    ///
    /// Used for populating node palettes in visual editors.
    fn get_all_nodes(&self) -> Vec<&NodeMetadata>;

    /// Returns all node types in a specific category.
    ///
    /// Used for organizing nodes by functionality.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let math_nodes = provider.get_nodes_by_category("Math");
    /// ```
    fn get_nodes_by_category(&self, category: &str) -> Vec<&NodeMetadata>;
}
