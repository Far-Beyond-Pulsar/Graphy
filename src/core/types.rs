//! # Type System
//!
//! Data types and type information for node pins.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Data type for a pin
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    /// Execution flow (no data)
    Execution,

    /// Typed data pin with Rust type string
    Typed(TypeInfo),

    // Legacy types for backward compatibility
    Number,
    String,
    Boolean,
    Vector2,
    Vector3,
    Color,
    Any,
}

/// Type information for typed pins
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeInfo {
    /// Rust type string (e.g., "i64", "String", "(f32, f32)")
    pub type_string: String,
}

impl TypeInfo {
    pub fn new(type_string: impl Into<String>) -> Self {
        Self {
            type_string: type_string.into(),
        }
    }
}

impl fmt::Display for TypeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.type_string)
    }
}

impl From<&str> for TypeInfo {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for TypeInfo {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Node type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum NodeTypes {
    /// Pure function (no side effects, no exec pins)
    #[serde(rename = "pure")]
    pure,

    /// Function with side effects (has exec pins)
    #[serde(rename = "fn")]
    fn_,

    /// Control flow node (multiple exec outputs)
    #[serde(rename = "control_flow")]
    control_flow,

    /// Event node (entry point)
    #[serde(rename = "event")]
    event,
}

/// Property value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PropertyValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Vector2(f64, f64),
    Vector3(f64, f64, f64),
    Color(f64, f64, f64, f64),
}

/// Position in 2D space (for visual editor)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Position {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}
