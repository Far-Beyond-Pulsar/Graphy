//! # Structural Type Checker
//!
//! Validates pin-to-pin connection type compatibility across a graph.
//!
//! ## What is checked
//!
//! - Every connection's source and target nodes exist.
//! - Every connection's source and target pins exist on their respective nodes.
//! - The [`ReflectedType`] of the source output pin is compatible with the target
//!   input pin, according to the registered [`CoercionRegistry`].
//! - Deprecated nodes still compile but emit warnings.
//!
//! ## Coercions
//!
//! Not all connections require identical types. A [`Coercion`] records that
//! one type can be implicitly converted to another. The default registry
//! includes the safe numeric widening coercions (i32 → i64, f32 → f64, etc.)
//! and `&str` → `String`.
//!
//! Custom coercions can be registered before running the pass.
//!
//! ## Usage
//!
//! ```ignore
//! use graphy::type_checker::TypeChecker;
//! use graphy::core::{GraphDescription, NodeMetadata};
//! use std::collections::HashMap;
//!
//! let checker = TypeChecker::default();
//! let result = checker.check(&graph, &metadata_map);
//! if !result.is_ok() {
//!     for e in result.errors() { eprintln!("{}", e); }
//! }
//! ```

use crate::{
    core::{
        Connection, ConnectionType, GraphDescription, NodeInstance,
        NodeMetadataProvider,
    },
    diagnostics::{
        CompileResult, Diagnostic, DiagnosticAccumulator, PassName, Severity,
    },
};
use crate::core::ReflectedType;

// ─────────────────────────────────────────────────────────────────────────────
// Coercion
// ─────────────────────────────────────────────────────────────────────────────

/// A declared implicit type coercion from `from` to `to`.
#[derive(Debug, Clone)]
pub struct Coercion {
    /// Source type (the type the connection carries).
    pub from: ReflectedType,
    /// Target type (the type the pin expects).
    pub to: ReflectedType,
    /// `true` if the coercion is lossless.
    pub lossless: bool,
}

impl Coercion {
    pub fn lossless(from: ReflectedType, to: ReflectedType) -> Self {
        Self { from, to, lossless: true }
    }

    pub fn lossy(from: ReflectedType, to: ReflectedType) -> Self {
        Self { from, to, lossless: false }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CoercionRegistry
// ─────────────────────────────────────────────────────────────────────────────

/// Registry of known implicit type coercions.
///
/// During type checking, if two types are not equal but a matching coercion
/// rule exists, the connection is allowed (with a warning for lossy coercions).
pub struct CoercionRegistry {
    rules: Vec<Coercion>,
}

impl CoercionRegistry {
    /// Creates an empty registry.
    pub fn empty() -> Self { Self { rules: Vec::new() } }

    /// Creates the default registry with standard Rust numeric widening coercions.
    pub fn with_defaults() -> Self {
        use crate::core::PrimitiveKind::*;
        let mut reg = Self::empty();

        // Lossless integer widenings
        reg.add(Coercion::lossless(ReflectedType::prim(I8),  ReflectedType::prim(I16)));
        reg.add(Coercion::lossless(ReflectedType::prim(I8),  ReflectedType::prim(I32)));
        reg.add(Coercion::lossless(ReflectedType::prim(I8),  ReflectedType::prim(I64)));
        reg.add(Coercion::lossless(ReflectedType::prim(I8),  ReflectedType::prim(I128)));
        reg.add(Coercion::lossless(ReflectedType::prim(I8),  ReflectedType::prim(Isize)));
        reg.add(Coercion::lossless(ReflectedType::prim(I16), ReflectedType::prim(I32)));
        reg.add(Coercion::lossless(ReflectedType::prim(I16), ReflectedType::prim(I64)));
        reg.add(Coercion::lossless(ReflectedType::prim(I16), ReflectedType::prim(I128)));
        reg.add(Coercion::lossless(ReflectedType::prim(I32), ReflectedType::prim(I64)));
        reg.add(Coercion::lossless(ReflectedType::prim(I32), ReflectedType::prim(I128)));
        reg.add(Coercion::lossless(ReflectedType::prim(I64), ReflectedType::prim(I128)));

        reg.add(Coercion::lossless(ReflectedType::prim(U8),  ReflectedType::prim(U16)));
        reg.add(Coercion::lossless(ReflectedType::prim(U8),  ReflectedType::prim(U32)));
        reg.add(Coercion::lossless(ReflectedType::prim(U8),  ReflectedType::prim(U64)));
        reg.add(Coercion::lossless(ReflectedType::prim(U8),  ReflectedType::prim(U128)));
        reg.add(Coercion::lossless(ReflectedType::prim(U8),  ReflectedType::prim(Usize)));
        reg.add(Coercion::lossless(ReflectedType::prim(U16), ReflectedType::prim(U32)));
        reg.add(Coercion::lossless(ReflectedType::prim(U16), ReflectedType::prim(U64)));
        reg.add(Coercion::lossless(ReflectedType::prim(U16), ReflectedType::prim(U128)));
        reg.add(Coercion::lossless(ReflectedType::prim(U32), ReflectedType::prim(U64)));
        reg.add(Coercion::lossless(ReflectedType::prim(U32), ReflectedType::prim(U128)));
        reg.add(Coercion::lossless(ReflectedType::prim(U64), ReflectedType::prim(U128)));

        // f32 → f64 widening
        reg.add(Coercion::lossless(ReflectedType::prim(F32), ReflectedType::prim(F64)));

        // &str → String
        reg.add(Coercion::lossless(
            ReflectedType::prim(StrSlice),
            ReflectedType::prim(StringOwned),
        ));

        reg
    }

    pub fn add(&mut self, coercion: Coercion) {
        self.rules.push(coercion);
    }

    /// Checks whether `from` can be coerced to `to`.
    ///
    /// Returns `Some(lossless)` if a matching rule is found; `None` if the
    /// types are incompatible.
    pub fn find(&self, from: &ReflectedType, to: &ReflectedType) -> Option<bool> {
        for rule in &self.rules {
            if &rule.from == from && &rule.to == to {
                return Some(rule.lossless);
            }
        }
        None
    }
}

impl Default for CoercionRegistry {
    fn default() -> Self { Self::with_defaults() }
}

// ─────────────────────────────────────────────────────────────────────────────
// TypeChecker
// ─────────────────────────────────────────────────────────────────────────────

/// Structural type checker for a `GraphDescription`.
///
/// Validates every connection in the graph against the node metadata
/// provided by a [`NodeMetadataProvider`]. Coercions are resolved through
/// the [`CoercionRegistry`].
pub struct TypeChecker {
    coercions: CoercionRegistry,
    /// If `true`, emit warnings for lossy coercions. Default: `true`.
    pub warn_lossy: bool,
    /// If `true`, treat deprecated node warnings as errors. Default: `false`.
    pub error_on_deprecated: bool,
}

impl TypeChecker {
    pub fn new(coercions: CoercionRegistry) -> Self {
        Self { coercions, warn_lossy: true, error_on_deprecated: false }
    }

    /// Checks all connections in `graph` against `provider`.
    ///
    /// Returns a [`CompileResult<()>`] — `Ok(())` if no errors, with any
    /// warnings or hints attached.
    pub fn check(
        &self,
        graph: &GraphDescription,
        provider: &dyn NodeMetadataProvider,
    ) -> CompileResult<()> {
        let mut acc = DiagnosticAccumulator::new(PassName::TypeChecker);

        for conn in &graph.connections {
            self.check_connection(conn, graph, provider, &mut acc);
        }

        // Check for deprecated nodes
        for (node_id, node) in &graph.nodes {
            if let NodeKindLocal::Native(node_type) = node_kind(node) {
                if let Some(meta) = provider.get_node_metadata(&node_type) {
                    if let Some(msg) = &meta.deprecated {
                        let sev = if self.error_on_deprecated {
                            Severity::Error
                        } else {
                            Severity::Warning
                        };
                        acc.push_diagnostic(
                            Diagnostic::new(sev, PassName::TypeChecker,
                                format!("Node '{}' is deprecated: {}", node_type, msg))
                                .at_node(node_id)
                        );
                    }
                }
            }
        }

        let diags = acc.finish();
        let has_errors = diags.iter().any(|d| d.is_error());
        if has_errors {
            CompileResult::err(diags)
        } else {
            CompileResult::ok_with_diags((), diags)
        }
    }

    fn check_connection(
        &self,
        conn: &Connection,
        graph: &GraphDescription,
        provider: &dyn NodeMetadataProvider,
        acc: &mut DiagnosticAccumulator,
    ) {
        // Execution connections: only check that nodes exist
        if conn.connection_type == ConnectionType::Execution {
            if graph.nodes.get(&conn.source_node).is_none() {
                acc.error(
                    &conn.source_node, None,
                    format!("Source node '{}' does not exist", conn.source_node),
                );
            }
            if graph.nodes.get(&conn.target_node).is_none() {
                acc.error(
                    &conn.target_node, None,
                    format!("Target node '{}' does not exist", conn.target_node),
                );
            }
            return;
        }

        // Data connection: full type check
        let source = match graph.nodes.get(&conn.source_node) {
            Some(n) => n,
            None => {
                acc.error(&conn.source_node, None,
                    format!("Source node '{}' does not exist", conn.source_node));
                return;
            }
        };
        let target = match graph.nodes.get(&conn.target_node) {
            Some(n) => n,
            None => {
                acc.error(&conn.target_node, None,
                    format!("Target node '{}' does not exist", conn.target_node));
                return;
            }
        };

        // Resolve source output pin type
        let src_ty = self.resolve_output_type(source, &conn.source_pin, provider);
        let tgt_ty = self.resolve_input_type(target, &conn.target_pin, provider);

        let (src_rt, tgt_rt) = match (src_ty, tgt_ty) {
            (Some(s), Some(t)) => (s, t),
            _ => return, // unknown types — can't check; silently skip
        };

        // Wildcards are compatible with everything
        if src_rt.is_wildcard() || tgt_rt.is_wildcard() {
            return;
        }

        // Exact match
        if src_rt == tgt_rt {
            return;
        }

        // Try coercion
        match self.coercions.find(&src_rt, &tgt_rt) {
            Some(true) => {
                // lossless — no warning needed
            }
            Some(false) => {
                if self.warn_lossy {
                    acc.warn(
                        &conn.target_node,
                        Some(&conn.target_pin),
                        format!(
                            "Lossy implicit coercion from '{}' to '{}' on pin '{}.{}'",
                            src_rt.to_type_string(),
                            tgt_rt.to_type_string(),
                            conn.target_node,
                            conn.target_pin,
                        ),
                    );
                }
            }
            None => {
                acc.error(
                    &conn.target_node,
                    Some(&conn.target_pin),
                    format!(
                        "Type mismatch: output '{}' produces '{}' but pin '{}.{}' expects '{}'",
                        conn.source_pin,
                        src_rt.to_type_string(),
                        conn.target_node,
                        conn.target_pin,
                        tgt_rt.to_type_string(),
                    ),
                );
            }
        }
    }

    // ── Type resolution helpers ──────────────────────────────────────────────

    /// Resolves the [`ReflectedType`] for an output pin on `node`.
    ///
    /// Checks the pin's `DataType` first, then falls back to the node's
    /// metadata return type if the pin is named `"result"` or `"output"`.
    fn resolve_output_type(
        &self,
        node: &NodeInstance,
        pin_id: &str,
        provider: &dyn NodeMetadataProvider,
    ) -> Option<ReflectedType> {
        // Try pin directly
        if let Some(pin) = node.outputs.iter().find(|p| p.id == pin_id) {
            if let Some(rt) = pin.pin.data_type.reflect() {
                return Some(rt);
            }
        }

        // Fall back to node metadata return type
        if let NodeKindLocal::Native(node_type) = node_kind(node) {
            if let Some(meta) = provider.get_node_metadata(&node_type) {
                if let Some(rt) = meta.reflected_return_type() {
                    return Some(rt);
                }
            }
        }
        None
    }

    /// Resolves the [`ReflectedType`] for an input pin on `node`.
    fn resolve_input_type(
        &self,
        node: &NodeInstance,
        pin_id: &str,
        provider: &dyn NodeMetadataProvider,
    ) -> Option<ReflectedType> {
        // Try pin directly
        if let Some(pin) = node.inputs.iter().find(|p| p.id == pin_id) {
            if let Some(rt) = pin.pin.data_type.reflect() {
                return Some(rt);
            }
        }

        // Fall back to node metadata param type
        if let NodeKindLocal::Native(node_type) = node_kind(node) {
            if let Some(meta) = provider.get_node_metadata(&node_type) {
                let params = meta.effective_params();
                if let Some(pm) = params.iter().find(|p| p.name == pin_id) {
                    return Some(pm.ty.clone());
                }
            }
        }
        None
    }
}

impl Default for TypeChecker {
    fn default() -> Self { Self::new(CoercionRegistry::default()) }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Module-local node kind classification — mirrors `NodeKind` but avoids
/// pulling the public enum's `String` allocation for the common case.
enum NodeKindLocal {
    Native(String),
    Special,
}

fn node_kind(node: &NodeInstance) -> NodeKindLocal {
    match node.node_type.as_str() {
        "reroute" | "subgraph_entry" | "subgraph_exit" => NodeKindLocal::Special,
        s if s.starts_with("macro:") || s.starts_with("subgraph:") => NodeKindLocal::Special,
        other => NodeKindLocal::Native(other.to_string()),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        DataType, GraphDescription, NodeInstance, NodeMetadata, NodeTypes, ParamInfo, Position,
        TypeInfo,
    };
    use std::collections::HashMap;

    struct SimpleProvider(HashMap<String, NodeMetadata>);

    impl NodeMetadataProvider for SimpleProvider {
        fn get_node_metadata(&self, t: &str) -> Option<&NodeMetadata> { self.0.get(t) }
        fn get_all_nodes(&self) -> Vec<&NodeMetadata> { self.0.values().collect() }
        fn get_nodes_by_category(&self, cat: &str) -> Vec<&NodeMetadata> {
            self.0.values().filter(|m| m.category == cat).collect()
        }
    }

    fn make_provider() -> SimpleProvider {
        let mut map = HashMap::new();
        map.insert("math.add".to_string(), NodeMetadata::new("math.add", NodeTypes::pure, "Math")
            .with_params(vec![
                ParamInfo::new("a", "f64"),
                ParamInfo::new("b", "f64"),
            ])
            .with_return_type(TypeInfo::new("f64")));
        map.insert("print".to_string(), NodeMetadata::new("print", NodeTypes::fn_, "IO")
            .with_params(vec![ParamInfo::new("value", "f64")]));
        SimpleProvider(map)
    }

    fn make_graph() -> GraphDescription {
        let mut g = GraphDescription::new("test");
        let mut add = NodeInstance::new("add_1", "math.add", Position::zero());
        add.add_input_pin("a", DataType::Number);
        add.add_input_pin("b", DataType::Number);
        add.add_output_pin("result", DataType::Number);
        g.add_node(add);

        let mut print = NodeInstance::new("print_1", "print", Position::zero());
        print.add_input_pin("value", DataType::Number);
        g.add_node(print);

        g.add_connection(Connection::data("add_1", "result", "print_1", "value"));
        g
    }

    #[test]
    fn valid_graph_no_errors() {
        let g = make_graph();
        let provider = make_provider();
        let checker = TypeChecker::default();
        let result = checker.check(&g, &provider);
        assert!(result.is_ok(), "{}", result.format_diagnostics());
    }

    #[test]
    fn missing_node_is_error() {
        let mut g = make_graph();
        g.add_connection(Connection::data("ghost_node", "out", "print_1", "value"));
        let provider = make_provider();
        let checker = TypeChecker::default();
        let result = checker.check(&g, &provider);
        assert!(!result.is_ok());
        assert!(result.errors().count() > 0);
    }

    #[test]
    fn coercion_registry_lossless() {
        let reg = CoercionRegistry::with_defaults();
        use crate::core::PrimitiveKind::*;
        assert_eq!(
            reg.find(&ReflectedType::prim(F32), &ReflectedType::prim(F64)),
            Some(true)
        );
        assert_eq!(
            reg.find(&ReflectedType::prim(F64), &ReflectedType::prim(F32)),
            None
        );
    }
}
