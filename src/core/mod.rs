//! # Core Data Structures
//!
//! Fundamental types for representing node graphs.

mod connection;
pub mod document;
mod graph;
mod metadata;
mod node;
mod types;

pub use connection::*;
pub use document::{
    BlueprintDocument, BlueprintMetadata, ClassVariable, DocumentEditorState, Graph,
    GraphComment as DocumentGraphComment, GraphId, GraphInterface, GraphKind, GraphViewState,
    InterfacePin, DOCUMENT_FORMAT_VERSION,
};
pub use graph::*;
pub use metadata::*;
pub use node::*;
pub use types::*;
