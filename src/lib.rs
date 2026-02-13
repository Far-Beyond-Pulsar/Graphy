//! # Graphy - General-Purpose Graph Compilation Library
//!
//! Graphy provides a flexible, extensible framework for compiling visual node graphs
//! into executable code. It's designed to be target-agnostic, supporting multiple
//! output languages (Rust, WGSL, etc.) through a trait-based architecture.
//!
//! ## Architecture
//!
//! Graphy follows a multi-phase compilation pipeline:
//!
//! ```text
//! ┌─────────────────┐
//! │  Graph Input    │
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │  Graph Expand   │  (Optional: Inline sub-graphs)
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │  Data Flow      │  (Build data dependency graph)
//! │    Analysis     │  (Topological sort)
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │  Execution      │  (Build execution routing table)
//! │  Flow Analysis  │
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │  Code           │  (Generate target code)
//! │  Generation     │
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────┐
//! │  Output Code    │
//! └─────────────────┘
//! ```
//!
//! ## Core Concepts
//!
//! ### Graph Structure
//! - **Nodes**: Computational units with inputs and outputs
//! - **Connections**: Links between node pins (data or execution flow)
//! - **Pins**: Input/output ports on nodes (typed)
//!
//! ### Node Types
//! - **Pure**: No side effects, can be inlined as expressions
//! - **Function**: Side effects, linear execution flow
//! - **ControlFlow**: Branching execution (if/else, loops, etc.)
//! - **Event**: Entry points for execution
//!
//! ## Extensibility
//!
//! Graphy is designed to be extended for different use cases:
//!
//! - Implement `NodeMetadataProvider` for your node system
//! - Implement `CodeGenerator` for your target language
//! - Add custom analysis passes with `AnalysisPass`

pub mod core;
pub mod analysis;
pub mod generation;
pub mod utils;
pub mod parallel;

// Re-export commonly used types
pub use core::{
    GraphDescription, NodeInstance, Connection, Pin, PinInstance,
    DataType, TypeInfo, NodeTypes, Position, ConnectionType, PropertyValue,
    GraphMetadata, NodeMetadata, ParamInfo, NodeMetadataProvider, PinType,
};

pub use analysis::{
    DataResolver, ExecutionRouting, DataSource,
};

pub use generation::{
    CodeGeneratorContext,
};

pub use utils::{
    SubGraphExpander,
};

/// Result type used throughout Graphy
pub type Result<T> = std::result::Result<T, GraphyError>;

/// Error types for Graphy
#[derive(Debug, thiserror::Error)]
pub enum GraphyError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Pin not found: {node}.{pin}")]
    PinNotFound { node: String, pin: String },

    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("Cyclic dependency detected, check your graph for looping code")]
    CyclicDependency,

    #[error("Invalid connection: {0}")]
    InvalidConnection(String),

    #[error("Code generation error: {0}")]
    CodeGeneration(String),

    #[error("AST parsing error: {0}")]
    AstParsing(String),

    #[error("Graph expansion error: {0}")]
    GraphExpansion(String),

    #[error("{0}")]
    Custom(String),
}
