//! # Document contracts
//!
//! Engine-neutral contracts shared by every graph editor and compiler built on
//! Graphy. Nothing here knows about a domain (Blueprint, shader, render graph,
//! animation) or about rendering; domains plug in through [`TypeResolver`] and
//! the [`GraphCommand`] vocabulary.
//!
//! ## Ownership and serialization boundaries
//!
//! - The *authored document* is owned by the domain crate that defines its
//!   on-disk format (for Blueprint: `graphy-blueprint::BlueprintAsset`). It is
//!   the only persisted form; there is no second in-memory schema.
//! - [`crate::GraphDescription`] is the graph payload both the editors and the
//!   compiler operate on. IDs inside it are **stable**: no operation here
//!   renumbers or regenerates an existing ID, and (de)serialization never
//!   touches them. See [`ids`].
//! - Edits are expressed as [`GraphCommand`]s and produce a [`ChangeSet`] that
//!   can be undone exactly; the same command sequence on the same graph always
//!   produces the same graph (no hash-order or clock dependence).
//! - Validation reports [`crate::Diagnostic`]s with the stable codes in
//!   [`invariants::codes`]; it never panics and never mutates.
//! - Rendering inputs (layout, hit-testing, canvas) and editor profile
//!   extension points are intentionally *not* here; they are owned by the
//!   `graphy-render*` crates, which are created when they contain code.

pub mod commands;
pub mod ids;
pub mod invariants;
pub mod resolver;

pub use commands::{ChangeSet, EditError, GraphChange, GraphCommand};
pub use ids::{IdAllocator, NodeId, PinRef};
pub use invariants::check_invariants;
pub use resolver::{
    check_connection_types, ChainResolver, Compatibility, ScopedResolver, StructuralResolver,
    TypeResolver,
};
