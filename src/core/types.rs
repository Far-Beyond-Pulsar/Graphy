//! # Type System
//!
//! Data types and type information for node pins and values.
//!
//! The type system supports both legacy enum-based types (Number, String, etc.)
//! and modern typed pins using Rust type strings for maximum flexibility.
//!
//! # Example
//!
//! ```
//! use graphy::{DataType, TypeInfo, PropertyValue, Position};
//!
//! // Modern typed pin
//! let typed = DataType::Typed(TypeInfo::new("f64"));
//!
//! // Legacy type
//! let legacy = DataType::Number;
//!
//! // Property value
//! let value = PropertyValue::Number(42.0);
//! ```
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// Data type for a pin.
///
/// Supports both legacy enum variants for backward compatibility
/// and modern typed variants using Rust type strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    /// Execution flow pin (carries no data, only control flow)
    Execution,

    /// Modern typed data pin with explicit Rust type
    Typed(TypeInfo),

    // ===== Legacy types for backward compatibility =====
    /// Numeric value (f64)
    Number,
    
    /// String value
    String,
    
    /// Boolean value
    Boolean,
    
    /// 2D vector (x, y)
    Vector2,
    
    /// 3D vector (x, y, z)
    Vector3,
    
    /// RGBA color (r, g, b, a)
    Color,
    
    /// Wildcard type (accepts any data)
    Any,
}

/// Type information for typed pins.
///
/// Wraps a Rust type string (e.g., "i64", "String", "(f32, f32)")
/// for type-safe code generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeInfo {
    /// Rust type string (e.g., "i64", "String", "(f32, f32)")
    pub type_string: String,
}

impl TypeInfo {
    /// Creates a new type info from a type string.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::core::TypeInfo;
    ///
    /// let int_type = TypeInfo::new("i64");
    /// let tuple_type = TypeInfo::new("(f32, f32)");
    /// let custom_type = TypeInfo::new("MyStruct");
    /// ```
    #[inline]
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
    #[inline]
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for TypeInfo {
    #[inline]
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Node type classification.
///
/// Determines how a node is processed during code generation:
/// - **Pure**: Can be inlined as expressions
/// - **Function**: Requires statement-level generation
/// - **Control Flow**: Generates branching constructs
/// - **Event**: Entry points for execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum NodeTypes {
    /// Pure function node (no side effects, no exec pins)
    ///
    /// Pure nodes can be inlined as expressions and don't affect control flow.
    /// Example: math operations, getters, constants
    #[serde(rename = "pure")]
    pure,

    /// Function node with side effects (has exec pins)
    ///
    /// Function nodes execute in sequence and may modify state.
    /// Example: print, set variable, API calls
    #[serde(rename = "fn")]
    fn_,

    /// Control flow node (multiple exec outputs)
    ///
    /// Control flow nodes create branching paths in the execution graph.
    /// Example: if/else, switch, loops
    #[serde(rename = "control_flow")]
    control_flow,

    /// Event node (entry point for execution)
    ///
    /// Event nodes mark where execution begins.
    /// Example: OnStart, OnUpdate, OnButtonClick
    #[serde(rename = "event")]
    event,
}

/// Property value types for node configuration.
///
/// Properties are constant values stored directly on nodes,
/// typically used for defaults or configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PropertyValue {
    /// String value
    String(String),
    
    /// Numeric value (stored as f64 for flexibility)
    Number(f64),
    
    /// Boolean flag
    Boolean(bool),
    
    /// 2D vector (x, y)
    Vector2(f64, f64),
    
    /// 3D vector (x, y, z)
    Vector3(f64, f64, f64),
    
    /// RGBA color (r, g, b, a) with values in [0, 1]
    Color(f64, f64, f64, f64),
}

/// 2D position in visual editor space.
///
/// Used to track node placement in visual programming environments.
/// Coordinates are in arbitrary editor units.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    /// X coordinate
    pub x: f64,
    
    /// Y coordinate
    pub y: f64,
}

impl Position {
    /// Creates a new position with the given coordinates.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::Position;
    ///
    /// let pos = Position::new(100.0, 200.0);
    /// assert_eq!(pos.x, 100.0);
    /// assert_eq!(pos.y, 200.0);
    /// ```
    #[inline]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Returns the origin position (0, 0).
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::Position;
    ///
    /// let origin = Position::zero();
    /// assert_eq!(origin.x, 0.0);
    /// assert_eq!(origin.y, 0.0);
    /// ```
    #[inline]
    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl Default for Position {
    #[inline]
    fn default() -> Self {
        Self::zero()
    }
}
