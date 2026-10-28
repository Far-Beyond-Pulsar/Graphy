//! Generic type-resolution interface.
//!
//! Graphy does not own a type universe. Hosts supply one (Pulsar reflection,
//! WGSL, Blueprint library types) behind [`TypeResolver`], and may compose
//! several with [`ChainResolver`] so that, for example, WGSL types stay
//! distinct from reflected Rust types.

use crate::{
    ConnectionType, DataType, Diagnostic, GraphDescription, PassName, Severity,
};

use super::invariants::codes;

/// Result of asking whether an output type may feed an input type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compatibility {
    /// Identical types.
    Exact,
    /// Either side is a wildcard; the pin takes its type on connection.
    Wildcard,
    /// Needs an inserted conversion (`lossy` if information can be lost).
    Convertible { lossy: bool },
    /// Cannot connect.
    Incompatible(String),
    /// This resolver has no knowledge of one of the types.
    Unknown,
}

impl Compatibility {
    pub fn is_connectable(&self) -> bool {
        matches!(
            self,
            Compatibility::Exact | Compatibility::Wildcard | Compatibility::Convertible { .. }
        )
    }
}

/// Domain-supplied judgement of pin type compatibility.
pub trait TypeResolver {
    /// Whether a value of type `from` (output) may connect to `to` (input).
    fn compatibility(&self, from: &DataType, to: &DataType) -> Compatibility;
}

/// Exec/wildcard/identity rules with no domain knowledge. Anything else is
/// [`Compatibility::Unknown`] so a domain resolver can be chained after it.
#[derive(Debug, Default, Clone, Copy)]
pub struct StructuralResolver;

impl TypeResolver for StructuralResolver {
    fn compatibility(&self, from: &DataType, to: &DataType) -> Compatibility {
        match (from, to) {
            (DataType::Exec, DataType::Exec) => Compatibility::Exact,
            (DataType::Exec, _) | (_, DataType::Exec) => {
                Compatibility::Incompatible("execution pin cannot connect to a data pin".into())
            }
            (DataType::Data(a), DataType::Data(b)) => {
                if a.type_string == b.type_string {
                    Compatibility::Exact
                } else if is_wildcard(&a.type_string) || is_wildcard(&b.type_string) {
                    Compatibility::Wildcard
                } else {
                    Compatibility::Unknown
                }
            }
        }
    }
}

fn is_wildcard(type_string: &str) -> bool {
    matches!(type_string, "?" | "_" | "T" | "wildcard")
}

/// Asks each resolver in order; the first definite answer (anything but
/// [`Compatibility::Unknown`]) wins. If all are unsure the result is `Unknown`.
#[derive(Default)]
pub struct ChainResolver {
    resolvers: Vec<Box<dyn TypeResolver>>,
}

impl ChainResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, resolver: impl TypeResolver + 'static) -> Self {
        self.resolvers.push(Box::new(resolver));
        self
    }
}

impl TypeResolver for ChainResolver {
    fn compatibility(&self, from: &DataType, to: &DataType) -> Compatibility {
        self.resolvers
            .iter()
            .map(|r| r.compatibility(from, to))
            .find(|c| *c != Compatibility::Unknown)
            .unwrap_or(Compatibility::Unknown)
    }
}

/// Checks every data connection's pin types with `resolver`, in connection
/// order. Connections with missing endpoints are skipped (those are reported
/// by [`super::invariants::check_invariants`]). `Unknown` is a warning, an
/// `Incompatible` answer is an error.
pub fn check_connection_types(
    graph: &GraphDescription,
    resolver: &dyn TypeResolver,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for c in &graph.connections {
        if c.connection_type == ConnectionType::Execution {
            continue;
        }
        let from = graph
            .nodes
            .get(&c.source_node)
            .and_then(|n| n.outputs.iter().find(|p| p.id == c.source_pin));
        let to = graph
            .nodes
            .get(&c.target_node)
            .and_then(|n| n.inputs.iter().find(|p| p.id == c.target_pin));
        let (Some(from), Some(to)) = (from, to) else {
            continue;
        };
        let (severity, code, msg) = match resolver.compatibility(from.data_type(), to.data_type())
        {
            Compatibility::Incompatible(why) => (Severity::Error, codes::TYPE_MISMATCH, why),
            Compatibility::Unknown => (
                Severity::Warning,
                codes::TYPE_UNKNOWN,
                "no resolver knows how to connect these types".to_string(),
            ),
            _ => continue,
        };
        out.push(
            Diagnostic::new(severity, PassName::TypeChecker, msg)
                .at_pin(c.target_node.clone(), c.target_pin.clone())
                .with_code(code),
        );
    }
    out
}
