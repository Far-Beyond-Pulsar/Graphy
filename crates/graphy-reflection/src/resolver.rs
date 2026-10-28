//! Type resolution over the reflection registries.

use graphy::core::{ReflectedType, WrapperKind};
use graphy::{Compatibility, DataType, TypeResolver};

use crate::source::{AttachedRegistry, TypeSource};

/// Decides pin compatibility for reflected Rust types.
///
/// Type names are resolved through `S` on every call. A pair is
/// `Exact` when it names the same type, `Convertible` when a conversion edge
/// is registered between the two runtime types, `Incompatible` when both are
/// known and neither holds, and `Unknown` when either side is not a type this
/// registry knows (so a domain resolver chained after it can answer).
/// `&T` and `&mut T` are transparent: they do not change compatibility.
#[derive(Debug, Clone)]
pub struct ReflectionResolver<S = AttachedRegistry> {
    source: S,
}

impl Default for ReflectionResolver<AttachedRegistry> {
    fn default() -> Self {
        Self::new(AttachedRegistry)
    }
}

impl<S: TypeSource> ReflectionResolver<S> {
    pub fn new(source: S) -> Self {
        Self { source }
    }

    /// Whether every leaf of `ty` is a registered type.
    fn is_known(&self, ty: &ReflectedType) -> bool {
        match ty {
            ReflectedType::Wildcard => true,
            ReflectedType::Wrapper { kind, inner } => {
                matches!(kind, WrapperKind::Ref | WrapperKind::RefMut | WrapperKind::Box
                    | WrapperKind::Arc | WrapperKind::Rc | WrapperKind::Vec
                    | WrapperKind::Option | WrapperKind::HashSet)
                    && self.is_known(inner)
            }
            ReflectedType::Array { element, .. } => self.is_known(element),
            ReflectedType::Tuple(items) => items.iter().all(|t| self.is_known(t)),
            other => self.source.runtime_type(&other.to_type_string()).is_some(),
        }
    }

    fn compare(&self, a: &ReflectedType, b: &ReflectedType) -> Compatibility {
        let (a, b) = (peel_refs(a), peel_refs(b));
        if a.is_wildcard() || b.is_wildcard() {
            return Compatibility::Wildcard;
        }
        if !self.is_known(a) || !self.is_known(b) {
            return Compatibility::Unknown;
        }
        if a == b {
            return Compatibility::Exact;
        }
        match (a, b) {
            (
                ReflectedType::Wrapper { kind: ka, inner: ia },
                ReflectedType::Wrapper { kind: kb, inner: ib },
            ) if ka == kb => self.compare(ia, ib),
            (
                ReflectedType::Array { element: ea, .. },
                ReflectedType::Array { element: eb, .. },
            ) => self.compare(ea, eb),
            _ => self.leaf(a, b),
        }
    }

    fn leaf(&self, a: &ReflectedType, b: &ReflectedType) -> Compatibility {
        let (na, nb) = (a.to_type_string(), b.to_type_string());
        let (Some(ia), Some(ib)) = (self.source.runtime_type(&na), self.source.runtime_type(&nb))
        else {
            return Compatibility::Incompatible(format!("'{na}' does not connect to '{nb}'"));
        };
        if ia.type_id == ib.type_id {
            return Compatibility::Exact;
        }
        match self.source.conversion(ia.type_id, ib.type_id) {
            Some(c) => Compatibility::Convertible { lossy: c.fallible },
            None => Compatibility::Incompatible(format!(
                "no conversion from '{na}' to '{nb}' is registered"
            )),
        }
    }
}

fn peel_refs(mut ty: &ReflectedType) -> &ReflectedType {
    while let ReflectedType::Wrapper {
        kind: WrapperKind::Ref | WrapperKind::RefMut,
        inner,
    } = ty
    {
        ty = inner;
    }
    ty
}

impl<S: TypeSource> TypeResolver for ReflectionResolver<S> {
    fn compatibility(&self, from: &DataType, to: &DataType) -> Compatibility {
        match (from, to) {
            (DataType::Data(a), DataType::Data(b)) => {
                self.compare(&a.parse(), &b.parse())
            }
            // Execution pins carry no reflected type; the structural resolver owns them.
            _ => Compatibility::Unknown,
        }
    }
}
