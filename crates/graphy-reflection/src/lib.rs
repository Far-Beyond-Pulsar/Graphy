//! # graphy-reflection
//!
//! Adapter between Pulsar's runtime reflection (`pulsar_reflection`) and
//! Graphy's engine-neutral contracts. `graphy` itself never depends on
//! Pulsar; hosts that want reflected types add this crate.
//!
//! - [`TypeSource`] is the lookup seam. [`AttachedRegistry`] reads the
//!   process-wide registries **on every call** and caches nothing, so a
//!   plugin copy of `pulsar_reflection` that was attached to the editor's
//!   runtime resolves exactly what the editor resolves, and a type
//!   registered after startup is seen immediately. There is no registry
//!   duplicated per plugin.
//! - [`ReflectionResolver`] implements [`graphy::TypeResolver`] over a
//!   `TypeSource`. Types it does not know answer `Unknown`, so it composes
//!   in a [`graphy::ChainResolver`] next to domain resolvers (for example a
//!   [`graphy::ScopedResolver`] for WGSL types).
//! - [`types`] converts `RuntimeTypeInfo` to Graphy types;
//!   [`nodes`] turns reflected methods, properties and conversions into
//!   [`graphy::NodeMetadata`].

pub mod nodes;
pub mod resolver;
pub mod source;
pub mod types;

pub use nodes::{
    conversion_node, method_node, property_getter_node, property_setter_node, MethodNodeOptions,
};
pub use resolver::ReflectionResolver;
pub use source::{AttachedRegistry, Conversion, TypeSource};
pub use types::{data_type_of, reflected_type_of};
