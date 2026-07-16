//! # Node Metadata
//!
//! Metadata structures and traits describing node type interfaces.
//!
//! `NodeMetadata` describes everything needed to:
//! - Display a node in a visual editor (name, category, params, properties)
//! - Validate connections (param types, return type)
//! - Generate code (function source, imports, exec outputs)
//!
//! ## Backward Compatibility
//!
//! All new fields on `NodeMetadata` carry `#[serde(default)]` so that
//! metadata serialized by older code (pre-v2) deserializes correctly with
//! zero-value defaults.

use super::{NodeTypes, PropertySchema, ReflectedType, TypeInfo};
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// ParamInfo — legacy parameter descriptor
// ─────────────────────────────────────────────────────────────────────────────

/// Parameter definition for a node input — legacy string-based form.
///
/// Retained for backward compatibility with code that builds `NodeMetadata`
/// using plain type strings. For rich structural types use [`ParamMeta`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamInfo {
    /// Parameter name — used as variable name in generated code.
    pub name: String,

    /// Rust type string (e.g. `"f64"`, `"String"`, `"&str"`).
    pub param_type: String,
}

impl ParamInfo {
    #[inline]
    pub fn new(name: impl Into<String>, param_type: impl Into<String>) -> Self {
        Self { name: name.into(), param_type: param_type.into() }
    }

    /// Converts to the richer [`ParamMeta`] by parsing the type string.
    pub fn to_param_meta(&self) -> ParamMeta {
        ParamMeta {
            name: self.name.clone(),
            ty: ReflectedType::parse_str(&self.param_type),
            default_value: None,
            is_exec: false,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ParamMeta — v2 parameter descriptor
// ─────────────────────────────────────────────────────────────────────────────

/// Rich parameter definition for a node input (v2).
///
/// Carries a [`ReflectedType`] instead of a raw type string, enabling
/// structural type checking and compatibility analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamMeta {
    /// Parameter name — used as variable name in generated code.
    pub name: String,

    /// Resolved Rust type.
    pub ty: ReflectedType,

    /// Default value if the caller does not supply this argument.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<super::PropertyValue>,

    /// `true` if this is an execution (control-flow) input pin rather than
    /// a data pin. Most nodes have at most one exec input.
    #[serde(default)]
    pub is_exec: bool,
}

impl ParamMeta {
    pub fn new(name: impl Into<String>, ty: ReflectedType) -> Self {
        Self { name: name.into(), ty, default_value: None, is_exec: false }
    }

    pub fn exec(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ty: ReflectedType::Wildcard,
            default_value: None,
            is_exec: true,
        }
    }

    /// Converts to legacy `ParamInfo` for code paths that require a type string.
    pub fn to_param_info(&self) -> ParamInfo {
        ParamInfo::new(self.name.clone(), self.ty.to_type_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NodeMetadata
// ─────────────────────────────────────────────────────────────────────────────

/// Complete metadata for a single node type.
///
/// Used by visual editors to build the node palette and by compilers to
/// resolve types, validate connections, and emit code.
///
/// ### Building metadata
///
/// Use [`NodeMetadata::new`] followed by the builder-pattern `with_*` methods.
///
/// ```
/// use graphy::{NodeMetadata, NodeTypes, ParamInfo};
///
/// let meta = NodeMetadata::new("add", NodeTypes::pure, "Math")
///     .with_params(vec![
///         ParamInfo::new("a", "f64"),
///         ParamInfo::new("b", "f64"),
///     ])
///     .with_return_type("f64")
///     .with_source("a + b");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    // ── Core identity (v1) ───────────────────────────────────────────────────

    /// Node identifier (e.g. `"add"`, `"print"`, `"branch"`).
    pub name: String,

    /// Node classification — determines compilation strategy.
    pub node_type: NodeTypes,

    /// Category for UI organisation (e.g. `"Math"`, `"String"`, `"Events"`).
    pub category: String,

    /// Input parameters with type strings.
    pub params: Vec<ParamInfo>,

    /// Return type (for pure nodes and functions).
    pub return_type: Option<TypeInfo>,

    /// Execution output pin names (for control flow and events).
    pub exec_outputs: Vec<String>,

    /// Required imports for code generation.
    pub imports: Vec<String>,

    /// Source code / expression body for this node.
    pub function_source: String,

    // ── Extended metadata (v2) ───────────────────────────────────────────────

    /// Rich parameter definitions. If present, takes precedence over `params`
    /// for type checking. Populated by the v2 `#[blueprint]` macro.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_metas: Vec<ParamMeta>,

    /// Schema for configurable node properties displayed in the editor.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub property_schema: Vec<PropertySchema>,

    /// `true` if the last parameter is variadic (can repeat).
    #[serde(default)]
    pub is_variadic: bool,

    /// Declared generic type parameters (e.g. `["T", "K"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generic_params: Vec<String>,

    /// Markdown documentation string shown in the node palette tooltip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,

    /// If `Some`, this node is deprecated and the string is the message shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,

    /// Monotonically increasing schema version. Bumped whenever the node
    /// signature changes to allow editors to flag stale node instances.
    #[serde(default)]
    pub metadata_version: u32,

    /// Named module-scope helper function definitions required by
    /// `function_source`, as `(name, wgsl_source)` pairs. Emitted once per
    /// generated module (deduplicated by name across nodes) by shader
    /// codegen; ignored by blueprint codegen. List helpers dependency-first
    /// within a node.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub helper_functions: Vec<(String, String)>,
}

impl NodeMetadata {
    /// Creates a new `NodeMetadata` with required fields. Use builder methods
    /// to add parameters, return types, source, etc.
    #[inline]
    pub fn new(
        name: impl Into<String>,
        node_type: NodeTypes,
        category: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            node_type,
            category: category.into(),
            params: Vec::new(),
            return_type: None,
            exec_outputs: Vec::new(),
            imports: Vec::new(),
            function_source: String::new(),
            param_metas: Vec::new(),
            property_schema: Vec::new(),
            is_variadic: false,
            generic_params: Vec::new(),
            doc: None,
            deprecated: None,
            metadata_version: 0,
            helper_functions: Vec::new(),
        }
    }

    // ── Builder methods ───────────────────────────────────────────────────────

    /// Sets the input parameters (legacy string-based).
    #[inline]
    #[must_use]
    pub fn with_params(mut self, params: Vec<ParamInfo>) -> Self {
        self.params = params;
        self
    }

    /// Sets the rich parameter metadata (v2).
    #[inline]
    #[must_use]
    pub fn with_param_metas(mut self, metas: Vec<ParamMeta>) -> Self {
        self.param_metas = metas;
        self
    }

    /// Sets the return type.
    #[inline]
    #[must_use]
    pub fn with_return_type(mut self, return_type: impl Into<TypeInfo>) -> Self {
        self.return_type = Some(return_type.into());
        self
    }

    /// Sets the execution output pin names.
    #[inline]
    #[must_use]
    pub fn with_exec_outputs(mut self, exec_outputs: Vec<String>) -> Self {
        self.exec_outputs = exec_outputs;
        self
    }

    /// Sets the required imports for code generation.
    #[inline]
    #[must_use]
    pub fn with_imports(mut self, imports: Vec<String>) -> Self {
        self.imports = imports;
        self
    }

    /// Sets the source code / expression for this node.
    #[inline]
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.function_source = source.into();
        self
    }

    /// Sets the property schema for configurable properties.
    #[inline]
    #[must_use]
    pub fn with_property_schema(mut self, schema: Vec<PropertySchema>) -> Self {
        self.property_schema = schema;
        self
    }

    /// Marks the last parameter as variadic.
    #[inline]
    #[must_use]
    pub fn variadic(mut self) -> Self {
        self.is_variadic = true;
        self
    }

    /// Declares generic type parameters.
    #[inline]
    #[must_use]
    pub fn with_generics(mut self, params: Vec<String>) -> Self {
        self.generic_params = params;
        self
    }

    /// Attaches Markdown documentation.
    #[inline]
    #[must_use]
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    /// Marks this node as deprecated with a message.
    #[inline]
    #[must_use]
    pub fn deprecated(mut self, message: impl Into<String>) -> Self {
        self.deprecated = Some(message.into());
        self
    }

    /// Sets the metadata schema version.
    #[inline]
    #[must_use]
    pub fn with_version(mut self, version: u32) -> Self {
        self.metadata_version = version;
        self
    }

    /// Sets named module-scope helper functions required by `function_source`.
    #[inline]
    #[must_use]
    pub fn with_helpers(mut self, helpers: &[(&str, &str)]) -> Self {
        self.helper_functions = helpers
            .iter()
            .map(|(name, source)| ((*name).to_string(), (*source).to_string()))
            .collect();
        self
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    /// Returns the rich [`ParamMeta`] list if available, otherwise synthesizes
    /// it from the legacy `params` list.
    pub fn effective_params(&self) -> Vec<ParamMeta> {
        if !self.param_metas.is_empty() {
            self.param_metas.clone()
        } else {
            self.params.iter().map(|p| p.to_param_meta()).collect()
        }
    }

    /// Returns the return [`ReflectedType`] if this node has a return value.
    pub fn reflected_return_type(&self) -> Option<ReflectedType> {
        self.return_type.as_ref().map(|ti| ti.parse())
    }

    /// Returns `true` if this node is an event entry-point.
    #[inline]
    pub fn is_event(&self) -> bool {
        self.node_type == NodeTypes::event
    }

    /// Returns `true` if this node is pure (side-effect-free).
    #[inline]
    pub fn is_pure(&self) -> bool {
        self.node_type == NodeTypes::pure
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NodeMetadataProvider — registry trait
// ─────────────────────────────────────────────────────────────────────────────

/// Trait for providing node type metadata to the compiler.
///
/// Implement this trait to connect your node registry to Graphy's compilation
/// pipeline. The provider is passed through every compiler phase.
///
/// # Example
///
/// ```
/// use graphy::{NodeMetadata, NodeMetadataProvider, NodeTypes, ParamInfo};
/// use std::collections::HashMap;
///
/// struct MyProvider { nodes: HashMap<String, NodeMetadata> }
///
/// impl NodeMetadataProvider for MyProvider {
///     fn get_node_metadata(&self, node_type: &str) -> Option<&NodeMetadata> {
///         self.nodes.get(node_type)
///     }
///     fn get_all_nodes(&self) -> Vec<&NodeMetadata> {
///         self.nodes.values().collect()
///     }
///     fn get_nodes_by_category(&self, category: &str) -> Vec<&NodeMetadata> {
///         self.nodes.values().filter(|m| m.category == category).collect()
///     }
/// }
/// ```
pub trait NodeMetadataProvider {
    /// Retrieves metadata for a specific node type identifier.
    fn get_node_metadata(&self, node_type: &str) -> Option<&NodeMetadata>;

    /// Returns all registered node types.
    fn get_all_nodes(&self) -> Vec<&NodeMetadata>;

    /// Returns all node types in the given category.
    fn get_nodes_by_category(&self, category: &str) -> Vec<&NodeMetadata>;
}

#[cfg(test)]
mod helper_function_tests {
    use super::*;

    #[test]
    fn with_helpers_stores_named_sources() {
        let meta = NodeMetadata::new("perlin_2d", NodeTypes::pure, "Noise")
            .with_helpers(&[("pn_hash21", "fn pn_hash21() {}"), ("pn_perlin", "fn pn_perlin() {}")]);
        assert_eq!(meta.helper_functions.len(), 2);
        assert_eq!(meta.helper_functions[0].0, "pn_hash21");
        assert_eq!(meta.helper_functions[1].1, "fn pn_perlin() {}");
    }

    #[test]
    fn helper_functions_default_empty_from_legacy_json() {
        // Serialized metadata written before this field existed must load.
        let meta = NodeMetadata::new("add", NodeTypes::pure, "Math");
        let mut json: serde_json::Value = serde_json::to_value(&meta).unwrap();
        json.as_object_mut().unwrap().remove("helper_functions");
        let restored: NodeMetadata = serde_json::from_value(json).unwrap();
        assert!(restored.helper_functions.is_empty());
    }

    #[test]
    fn helper_functions_round_trip_serde() {
        let meta = NodeMetadata::new("perlin_2d", NodeTypes::pure, "Noise")
            .with_helpers(&[("pn_hash21", "fn pn_hash21() {}")]);
        let json = serde_json::to_string(&meta).unwrap();
        let restored: NodeMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.helper_functions, meta.helper_functions);
    }
}
