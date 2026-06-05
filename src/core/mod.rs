//! # Core Data Structures
//!
//! Fundamental types for representing node graphs.

mod graph;
mod node;
mod connection;
mod types;
mod metadata;
pub mod document;

pub use graph::*;
pub use node::*;
pub use connection::*;
pub use types::*;
pub use metadata::*;
pub use document::{
    BlueprintDocument, BlueprintMetadata, ClassVariable, DocumentEditorState,
    Graph, GraphComment as DocumentGraphComment, GraphId, GraphInterface,
    GraphKind, GraphViewState, InterfacePin, DOCUMENT_FORMAT_VERSION,
};
