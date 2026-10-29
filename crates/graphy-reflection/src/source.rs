//! Where reflected types come from.

use std::any::TypeId;

use pulsar_reflection::{RuntimeTypeInfo, CONVERSION_REGISTRY, RUNTIME_TYPE_REGISTRY};

/// A registered conversion between two runtime types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Conversion {
    /// Stable identifier of the conversion edge (usable in saved graphs).
    pub id: &'static str,
    /// `true` for a fallible `TryFrom` edge; `false` for infallible `From`.
    pub fallible: bool,
}

/// Lookup of reflected types and conversions by name or id.
///
/// Implementations must resolve against live state on every call. Do not
/// cache the returned references beyond the call that needs them.
pub trait TypeSource {
    fn runtime_type(&self, name: &str) -> Option<&'static RuntimeTypeInfo>;
    fn runtime_type_by_id(&self, id: TypeId) -> Option<&'static RuntimeTypeInfo>;
    fn conversion(&self, from: TypeId, to: TypeId) -> Option<Conversion>;
}

/// The runtime's registries as seen by this copy of `pulsar_reflection`.
///
/// In a plugin that has attached to the editor's runtime these deref to the
/// editor's registries, so this type works unchanged in the editor and in
/// plugins.
#[derive(Debug, Default, Clone, Copy)]
pub struct AttachedRegistry;

impl TypeSource for AttachedRegistry {
    fn runtime_type(&self, name: &str) -> Option<&'static RuntimeTypeInfo> {
        RUNTIME_TYPE_REGISTRY.get_by_name(name)
    }

    fn runtime_type_by_id(&self, id: TypeId) -> Option<&'static RuntimeTypeInfo> {
        RUNTIME_TYPE_REGISTRY.get_by_id(id)
    }

    fn conversion(&self, from: TypeId, to: TypeId) -> Option<Conversion> {
        CONVERSION_REGISTRY.between(from, to).map(|c| Conversion {
            id: c.id,
            fallible: c.label != "INTO",
        })
    }
}
