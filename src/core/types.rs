//! # Type System
//!
//! Data types and type information for node pins and values.
//!
//! ## Pin types — `DataType`
//!
//! Every pin carries exactly one of two variants:
//!
//! - `DataType::Exec` — execution-flow pin (control flow, no value).
//! - `DataType::Data(TypeInfo)` — data pin. `TypeInfo` carries the Rust type
//!   string plus optional display metadata (human name, byte size, alignment).
//!   The runtime does not hard-code knowledge of any specific type; callers
//!   supply all domain-specific types through `TypeInfo`.
//!
//! ## Structural types — `ReflectedType`
//!
//! `TypeInfo::parse()` converts a stored type string into a `ReflectedType`,
//! the full Rust type algebra (primitives, structs, enums, generics, wrappers,
//! tuples, arrays, wildcards). `ReflectedType::to_type_string()` round-trips
//! back to a string for code emission.
//!
//! ## Property values — `PropertyValue`
//!
//! Typed constant storage for node properties.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

pub type JsonValue = serde_json::Value;

// ─────────────────────────────────────────────────────────────────────────────
// TypeInfo
// ─────────────────────────────────────────────────────────────────────────────

/// Type descriptor for a data pin.
///
/// Carries a Rust type string plus optional runtime reflection metadata
/// (human-readable name, byte size, byte alignment). The Pulsar reflection
/// system can populate `size_bytes` and `align_bytes` from `TypeLayout`;
/// code that only needs type-checking can leave them `None`.
///
/// # Constructing
///
/// ```
/// use graphy::TypeInfo;
///
/// // Minimal — type string only
/// let ti = TypeInfo::new("Vec2");
///
/// // With display metadata
/// let ti = TypeInfo::new("Vec2")
///     .with_display_name("Vector 2D")
///     .with_layout(8, 4);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeInfo {
    /// Rust type string, e.g. `"i64"`, `"Vec2"`, `"Vec<Option<f32>>"`.
    pub type_string: String,

    /// Human-readable display name shown in the editor (e.g. `"Vector 2D"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Byte size of this type at runtime (`std::mem::size_of`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<usize>,

    /// Byte alignment of this type at runtime (`std::mem::align_of`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align_bytes: Option<usize>,
}

impl TypeInfo {
    /// Creates a `TypeInfo` from a Rust type string, no display metadata.
    #[inline]
    #[must_use]
    pub fn new(type_string: impl Into<String>) -> Self {
        Self {
            type_string: type_string.into(),
            display_name: None,
            size_bytes: None,
            align_bytes: None,
        }
    }

    /// Builder: set a human-readable display name.
    #[inline]
    #[must_use]
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    /// Builder: set byte size and alignment (from Pulsar's `TypeLayout` or `std::mem`).
    #[inline]
    #[must_use]
    pub fn with_layout(mut self, size_bytes: usize, align_bytes: usize) -> Self {
        self.size_bytes = Some(size_bytes);
        self.align_bytes = Some(align_bytes);
        self
    }

    /// Returns the display name if set, otherwise the type string.
    #[inline]
    pub fn label(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.type_string)
    }

    /// Parses the type string into a structured [`ReflectedType`].
    #[inline]
    pub fn parse(&self) -> ReflectedType {
        ReflectedType::parse_str(&self.type_string)
    }
}

impl fmt::Display for TypeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.type_string)
    }
}

impl From<&str> for TypeInfo {
    #[inline]
    fn from(s: &str) -> Self { Self::new(s) }
}

impl From<String> for TypeInfo {
    #[inline]
    fn from(s: String) -> Self { Self::new(s) }
}

impl From<&ReflectedType> for TypeInfo {
    fn from(rt: &ReflectedType) -> Self {
        TypeInfo::new(rt.to_type_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DataType
// ─────────────────────────────────────────────────────────────────────────────

/// Pin type — exactly two variants, no domain-specific knowledge baked in.
///
/// - `Exec` — execution-flow pin (control flow, no value).
/// - `Data(TypeInfo)` — data pin carrying any Rust type. The runtime has no
///   internal knowledge of specific types; every type is represented uniformly
///   through `TypeInfo { type_string, display_name, size_bytes, align_bytes }`.
///
/// This design means the graph library is infinitely extensible: adding new
/// types (e.g. `Vec2`, `Transform`, `AssetHandle<Mesh>`) requires no changes
/// to Graphy — callers construct a `TypeInfo` and wrap it in `DataType::Data`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    /// Execution-flow pin — carries control flow, holds no data value.
    Exec,
    /// Data pin — carries a value of the type described by `TypeInfo`.
    Data(TypeInfo),
}

impl DataType {
    // ── Convenience constructors ─────────────────────────────────────────────

    /// Creates a data pin from a Rust type string.
    #[inline]
    pub fn typed(type_string: impl Into<String>) -> Self {
        DataType::Data(TypeInfo::new(type_string))
    }

    /// Creates a wildcard data pin (accepts any type).
    #[inline]
    pub fn any() -> Self {
        DataType::Data(TypeInfo::new("?"))
    }

    // ── Queries ──────────────────────────────────────────────────────────────

    /// Returns `true` if this is an execution-flow pin.
    #[inline]
    pub fn is_execution(&self) -> bool {
        matches!(self, DataType::Exec)
    }

    /// Returns `true` if this is a data-carrying pin.
    #[inline]
    pub fn is_data(&self) -> bool {
        matches!(self, DataType::Data(_))
    }

    /// Returns the inner `TypeInfo`, or `None` for `Exec` pins.
    #[inline]
    pub fn type_info(&self) -> Option<&TypeInfo> {
        match self {
            DataType::Data(ti) => Some(ti),
            DataType::Exec => None,
        }
    }

    /// Parses this pin's type into a [`ReflectedType`], or `None` for `Exec`.
    #[inline]
    pub fn reflect(&self) -> Option<ReflectedType> {
        match self {
            DataType::Exec => None,
            DataType::Data(ti) => Some(ti.parse()),
        }
    }

    /// Returns the canonical type string, or `None` for `Exec` pins.
    #[inline]
    pub fn type_string(&self) -> Option<&str> {
        match self {
            DataType::Exec => None,
            DataType::Data(ti) => Some(&ti.type_string),
        }
    }

    /// Constructs a `DataType::Data` from a [`ReflectedType`].
    #[inline]
    pub fn from_reflected(rt: &ReflectedType) -> Self {
        DataType::Data(TypeInfo::new(rt.to_type_string()))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ReflectedType — structured Rust type representation
// ─────────────────────────────────────────────────────────────────────────────

/// A fully-structured representation of a Rust type.
///
/// `ReflectedType` models the Rust type algebra used in blueprint node pin
/// signatures. It is derived from `TypeInfo` via `TypeInfo::parse()` and can
/// be round-tripped back to a string via `to_type_string()`.
///
/// # Compatibility
///
/// `ReflectedType` is the in-memory form; `TypeInfo` (a string) is the
/// serialized form. Existing serialized graphs are unaffected.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum ReflectedType {
    /// A primitive Rust type (`i32`, `f32`, `bool`, `String`, etc.).
    Primitive(PrimitiveKind),

    /// A named struct, optionally generic (e.g. `MyStruct<T>`).
    Struct {
        /// Fully-qualified or short path (e.g. `"std::collections::HashMap"` or `"MyStruct"`).
        path: String,
        /// Resolved type arguments, if any.
        generics: Vec<ReflectedType>,
    },

    /// A named enum, optionally generic.
    Enum {
        /// Fully-qualified or short path.
        path: String,
        /// Variant definitions, including optional payload types.
        variants: Vec<EnumVariant>,
    },

    /// An anonymous tuple type, e.g. `(f32, f32)`.
    Tuple(Vec<ReflectedType>),

    /// A fixed-size or unsized array, e.g. `[f32; 4]` or `[f32]`.
    Array {
        element: Box<ReflectedType>,
        /// `Some(n)` for `[T; N]`, `None` for `[T]` (slice).
        len: Option<usize>,
    },

    /// A single-parameter generic wrapper (Vec, Option, Arc, Box, &, &mut, …).
    Wrapper {
        kind: WrapperKind,
        /// The "inner" type parameter (e.g. `T` in `Vec<T>`, `T` in `Result<T, E>`).
        inner: Box<ReflectedType>,
    },

    /// A trait object `dyn Trait`.
    TraitObject {
        trait_path: String,
    },

    /// Wildcard — the `?` or `_` placeholder, compatible with any type.
    Wildcard,
}

impl ReflectedType {
    /// Convenience constructor for primitive types.
    #[inline]
    pub fn prim(kind: PrimitiveKind) -> Self {
        ReflectedType::Primitive(kind)
    }

    /// Returns `true` if this type is a wildcard (`?`/`_`).
    #[inline]
    pub fn is_wildcard(&self) -> bool {
        matches!(self, ReflectedType::Wildcard)
    }

    /// Returns `true` if this is a primitive type.
    #[inline]
    pub fn is_primitive(&self) -> bool {
        matches!(self, ReflectedType::Primitive(_))
    }

    /// Returns `true` if this is a numeric primitive (`i*/u*/f*`).
    pub fn is_numeric(&self) -> bool {
        match self {
            ReflectedType::Primitive(p) => p.is_numeric(),
            _ => false,
        }
    }

    /// Returns `true` if this is a `Copy` type (all primitives, references, small structs
    /// that are *known* to be `Copy`).
    pub fn is_copy_hint(&self) -> bool {
        match self {
            ReflectedType::Primitive(p) => p.is_copy(),
            ReflectedType::Wrapper { kind: WrapperKind::Ref | WrapperKind::RefMut, .. } => true,
            _ => false,
        }
    }

    /// Emits the Rust type string for use in generated code.
    pub fn to_type_string(&self) -> String {
        match self {
            ReflectedType::Primitive(p) => p.as_str().to_string(),
            ReflectedType::Struct { path, generics } => {
                if generics.is_empty() {
                    path.clone()
                } else {
                    let args = generics.iter().map(|g| g.to_type_string()).collect::<Vec<_>>().join(", ");
                    format!("{}<{}>", path, args)
                }
            }
            ReflectedType::Enum { path, .. } => path.clone(),
            ReflectedType::Tuple(elems) => {
                let inner = elems.iter().map(|e| e.to_type_string()).collect::<Vec<_>>().join(", ");
                format!("({})", inner)
            }
            ReflectedType::Array { element, len } => {
                match len {
                    Some(n) => format!("[{}; {}]", element.to_type_string(), n),
                    None => format!("[{}]", element.to_type_string()),
                }
            }
            ReflectedType::Wrapper { kind, inner } => kind.format_with(inner),
            ReflectedType::TraitObject { trait_path } => format!("dyn {}", trait_path),
            ReflectedType::Wildcard => "?".to_string(),
        }
    }

    /// Converts to a `TypeInfo` for use where string-based types are required.
    pub fn to_type_info(&self) -> TypeInfo {
        TypeInfo::new(self.to_type_string())
    }

    /// Parses a Rust type string into a `ReflectedType`.
    ///
    /// This is a best-effort parser covering the most common type patterns.
    /// Types not recognized fall back to `ReflectedType::Struct { path, generics: [] }`.
    pub fn parse_str(s: &str) -> ReflectedType {
        TypeParser::new(s.trim()).parse()
    }
}

impl fmt::Display for ReflectedType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_type_string())
    }
}

impl FromStr for ReflectedType {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ReflectedType::parse_str(s))
    }
}

impl From<PrimitiveKind> for ReflectedType {
    fn from(p: PrimitiveKind) -> Self { ReflectedType::Primitive(p) }
}

// ─────────────────────────────────────────────────────────────────────────────
// PrimitiveKind
// ─────────────────────────────────────────────────────────────────────────────

/// All Rust primitive types that blueprint nodes can produce or consume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrimitiveKind {
    I8, I16, I32, I64, I128, Isize,
    U8, U16, U32, U64, U128, Usize,
    F32, F64,
    Bool,
    Char,
    /// `&str` — borrowed string slice.
    StrSlice,
    /// `String` — owned string.
    StringOwned,
}

impl PrimitiveKind {
    /// Returns the canonical Rust spelling of this primitive.
    pub const fn as_str(self) -> &'static str {
        match self {
            PrimitiveKind::I8 => "i8",
            PrimitiveKind::I16 => "i16",
            PrimitiveKind::I32 => "i32",
            PrimitiveKind::I64 => "i64",
            PrimitiveKind::I128 => "i128",
            PrimitiveKind::Isize => "isize",
            PrimitiveKind::U8 => "u8",
            PrimitiveKind::U16 => "u16",
            PrimitiveKind::U32 => "u32",
            PrimitiveKind::U64 => "u64",
            PrimitiveKind::U128 => "u128",
            PrimitiveKind::Usize => "usize",
            PrimitiveKind::F32 => "f32",
            PrimitiveKind::F64 => "f64",
            PrimitiveKind::Bool => "bool",
            PrimitiveKind::Char => "char",
            PrimitiveKind::StrSlice => "&str",
            PrimitiveKind::StringOwned => "String",
        }
    }

    /// Returns `true` if this is a numeric type (`i*`, `u*`, or `f*`).
    pub const fn is_numeric(self) -> bool {
        !matches!(self, PrimitiveKind::Bool | PrimitiveKind::Char | PrimitiveKind::StrSlice | PrimitiveKind::StringOwned)
    }

    /// Returns `true` if this type is `Copy` in Rust.
    pub const fn is_copy(self) -> bool {
        !matches!(self, PrimitiveKind::StringOwned)
    }

    /// Returns `true` if this is a floating-point type.
    pub const fn is_float(self) -> bool {
        matches!(self, PrimitiveKind::F32 | PrimitiveKind::F64)
    }

    /// Returns `true` if this is a signed integer type.
    pub const fn is_signed_int(self) -> bool {
        matches!(self, PrimitiveKind::I8 | PrimitiveKind::I16 | PrimitiveKind::I32 | PrimitiveKind::I64 | PrimitiveKind::I128 | PrimitiveKind::Isize)
    }

    /// Returns `true` if this is an unsigned integer type.
    pub const fn is_unsigned_int(self) -> bool {
        matches!(self, PrimitiveKind::U8 | PrimitiveKind::U16 | PrimitiveKind::U32 | PrimitiveKind::U64 | PrimitiveKind::U128 | PrimitiveKind::Usize)
    }

    /// Attempts to parse a type name string into a `PrimitiveKind`.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "i8" => Some(Self::I8),
            "i16" => Some(Self::I16),
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "i128" => Some(Self::I128),
            "isize" => Some(Self::Isize),
            "u8" => Some(Self::U8),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "u128" => Some(Self::U128),
            "usize" => Some(Self::Usize),
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            "bool" => Some(Self::Bool),
            "char" => Some(Self::Char),
            "&str" => Some(Self::StrSlice),
            "str" => Some(Self::StrSlice),
            "String" => Some(Self::StringOwned),
            _ => None,
        }
    }
}

impl fmt::Display for PrimitiveKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WrapperKind
// ─────────────────────────────────────────────────────────────────────────────

/// Single-parameter (or two-parameter) generic wrapper types.
///
/// For two-parameter types (`HashMap`, `Result`), the secondary type parameter
/// is carried in the variant. The "inner" field on `ReflectedType::Wrapper`
/// always holds the primary type parameter (`T`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "wrapper")]
pub enum WrapperKind {
    Vec,
    HashSet,
    Arc,
    Rc,
    Box,
    /// `&T` — shared reference.
    Ref,
    /// `&mut T` — mutable reference.
    RefMut,
    Option,
    /// `Result<T, E>` where the `inner` field on `ReflectedType::Wrapper` holds `T`.
    Result {
        error: Box<ReflectedType>,
    },
    /// `HashMap<K, V>` where the `inner` field on `ReflectedType::Wrapper` holds `V`.
    HashMap {
        key: Box<ReflectedType>,
    },
    /// `BTreeMap<K, V>`.
    BTreeMap {
        key: Box<ReflectedType>,
    },
    /// Any other single-generic wrapper not listed above, identified by path.
    Other {
        path: String,
    },
}

impl WrapperKind {
    /// Formats this wrapper with a given inner type string.
    pub fn format_with(&self, inner: &ReflectedType) -> String {
        let i = inner.to_type_string();
        match self {
            WrapperKind::Vec => format!("Vec<{}>", i),
            WrapperKind::HashSet => format!("HashSet<{}>", i),
            WrapperKind::Arc => format!("Arc<{}>", i),
            WrapperKind::Rc => format!("Rc<{}>", i),
            WrapperKind::Box => format!("Box<{}>", i),
            WrapperKind::Ref => format!("&{}", i),
            WrapperKind::RefMut => format!("&mut {}", i),
            WrapperKind::Option => format!("Option<{}>", i),
            WrapperKind::Result { error } => format!("Result<{}, {}>", i, error.to_type_string()),
            WrapperKind::HashMap { key } => format!("HashMap<{}, {}>", key.to_type_string(), i),
            WrapperKind::BTreeMap { key } => format!("BTreeMap<{}, {}>", key.to_type_string(), i),
            WrapperKind::Other { path } => format!("{}<{}>", path, i),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EnumVariant
// ─────────────────────────────────────────────────────────────────────────────

/// A single variant in an enum type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EnumVariant {
    /// Variant name, e.g. `"Some"`, `"Ok"`, `"MyVariant"`.
    pub name: String,
    /// Optional tuple/struct payload type.
    pub payload: Option<ReflectedType>,
}

impl EnumVariant {
    pub fn unit(name: impl Into<String>) -> Self {
        Self { name: name.into(), payload: None }
    }
    pub fn tuple(name: impl Into<String>, payload: ReflectedType) -> Self {
        Self { name: name.into(), payload: Some(payload) }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PropertyValue — typed constant storage
// ─────────────────────────────────────────────────────────────────────────────

/// A typed constant value stored in a node property.
///
/// Replaces the opaque `serde_json::Value` bag for node properties. Old
/// JSON properties can be migrated via [`PropertyValue::from_json`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum PropertyValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    /// A named enum variant, e.g. `MyEnum::Variant`.
    Enum {
        type_path: String,
        variant: String,
    },
    /// A struct literal represented as named fields.
    Struct {
        type_path: String,
        fields: HashMap<String, PropertyValue>,
    },
    Array(Vec<PropertyValue>),
    /// A reference to an asset by type and path.
    AssetRef {
        asset_type: String,
        path: String,
    },
    /// Null / not-set.
    Null,
}

impl PropertyValue {
    /// Best-effort conversion from an untyped `serde_json::Value`.
    pub fn from_json(v: &JsonValue) -> Self {
        match v {
            JsonValue::Null => PropertyValue::Null,
            JsonValue::Bool(b) => PropertyValue::Bool(*b),
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    PropertyValue::Int(i)
                } else {
                    PropertyValue::Float(n.as_f64().unwrap_or(0.0))
                }
            }
            JsonValue::String(s) => PropertyValue::String(s.clone()),
            JsonValue::Array(arr) => {
                PropertyValue::Array(arr.iter().map(PropertyValue::from_json).collect())
            }
            JsonValue::Object(obj) => {
                let fields = obj.iter()
                    .map(|(k, v)| (k.clone(), PropertyValue::from_json(v)))
                    .collect();
                PropertyValue::Struct { type_path: String::new(), fields }
            }
        }
    }

    /// Converts this value back to an untyped `serde_json::Value`.
    pub fn to_json(&self) -> JsonValue {
        match self {
            PropertyValue::Null => JsonValue::Null,
            PropertyValue::Bool(b) => JsonValue::Bool(*b),
            PropertyValue::Int(i) => JsonValue::Number((*i).into()),
            PropertyValue::Float(f) => serde_json::Number::from_f64(*f)
                .map(JsonValue::Number)
                .unwrap_or(JsonValue::Null),
            PropertyValue::String(s) => JsonValue::String(s.clone()),
            PropertyValue::Enum { variant, .. } => JsonValue::String(variant.clone()),
            PropertyValue::Struct { fields, .. } => {
                let obj: serde_json::Map<_, _> = fields.iter()
                    .map(|(k, v)| (k.clone(), v.to_json()))
                    .collect();
                JsonValue::Object(obj)
            }
            PropertyValue::Array(arr) => {
                JsonValue::Array(arr.iter().map(|v| v.to_json()).collect())
            }
            PropertyValue::AssetRef { path, .. } => JsonValue::String(path.clone()),
        }
    }

    /// Returns `true` if this value is `Null` or an empty string.
    pub fn is_empty(&self) -> bool {
        matches!(self, PropertyValue::Null)
            || matches!(self, PropertyValue::String(s) if s.is_empty())
    }
}

impl Default for PropertyValue {
    fn default() -> Self { PropertyValue::Null }
}

// ─────────────────────────────────────────────────────────────────────────────
// PropertySchema — metadata for a node's configurable properties
// ─────────────────────────────────────────────────────────────────────────────

/// Describes one configurable property of a node type.
///
/// The schema is emitted by the `#[blueprint]` proc-macro for any function
/// arguments decorated with `#[property(...)]` and used by both the editor
/// (to render the property panel) and the compiler (to validate values).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertySchema {
    /// Machine-readable key — matches the key used in `NodeInstance::properties`.
    pub id: String,

    /// Human-readable label for display in editors.
    pub label: String,

    /// The expected type of this property value.
    pub ty: ReflectedType,

    /// Default value if the user has not configured the property.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<PropertyValue>,

    /// Optional tooltip shown in editors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,

    /// Minimum numeric value (for `Int` and `Float` properties).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,

    /// Maximum numeric value (for `Int` and `Float` properties).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

impl PropertySchema {
    /// Creates a minimal property schema with id, label, and type.
    pub fn new(id: impl Into<String>, label: impl Into<String>, ty: ReflectedType) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            ty,
            default_value: None,
            tooltip: None,
            min: None,
            max: None,
        }
    }

    /// Builder: set default value.
    #[must_use]
    pub fn with_default(mut self, v: PropertyValue) -> Self {
        self.default_value = Some(v);
        self
    }

    /// Builder: set tooltip.
    #[must_use]
    pub fn with_tooltip(mut self, tip: impl Into<String>) -> Self {
        self.tooltip = Some(tip.into());
        self
    }

    /// Builder: set min/max bounds for numeric properties.
    #[must_use]
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NodeTypes (unchanged, re-documented)
// ─────────────────────────────────────────────────────────────────────────────

/// Node type classification — determines compilation strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum NodeTypes {
    /// Pure, side-effect-free node — can be inlined as an expression.
    #[serde(rename = "pure")]
    pure,
    /// Function node with side effects — requires statement-level emission.
    #[serde(rename = "fn")]
    fn_,
    /// Control-flow node — generates branching constructs.
    #[serde(rename = "control_flow")]
    control_flow,
    /// Event node — marks an execution entry point.
    #[serde(rename = "event")]
    event,
}

// ─────────────────────────────────────────────────────────────────────────────
// Position (unchanged)
// ─────────────────────────────────────────────────────────────────────────────

/// 2D position in visual-editor canvas space.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Position {
    #[inline(always)]
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self { Self { x, y } }

    #[inline(always)]
    #[must_use]
    pub const fn zero() -> Self { Self { x: 0.0, y: 0.0 } }
}

impl Default for Position {
    #[inline(always)]
    fn default() -> Self { Self::zero() }
}

// ─────────────────────────────────────────────────────────────────────────────
// TypeParser — internal recursive-descent parser for Rust type strings
// ─────────────────────────────────────────────────────────────────────────────

struct TypeParser<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> TypeParser<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    fn remaining(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn skip_ws(&mut self) {
        while self.pos < self.src.len() && self.src.as_bytes()[self.pos] == b' ' {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.as_bytes().get(self.pos).copied()
    }

    fn consume(&mut self) -> Option<u8> {
        let b = self.src.as_bytes().get(self.pos).copied();
        if b.is_some() { self.pos += 1; }
        b
    }

    fn consume_if(&mut self, b: u8) -> bool {
        if self.src.as_bytes().get(self.pos) == Some(&b) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn consume_keyword(&mut self, kw: &str) -> bool {
        let r = self.remaining();
        if r.starts_with(kw) {
            // ensure it's not a longer identifier
            let after = r.len().min(kw.len());
            let next = r.as_bytes().get(after);
            if next.map_or(true, |&b| !b.is_ascii_alphanumeric() && b != b'_') {
                self.pos += kw.len();
                return true;
            }
        }
        false
    }

    /// Top-level parse entry point.
    fn parse(&mut self) -> ReflectedType {
        self.skip_ws();
        let rt = self.parse_type();
        rt
    }

    fn parse_type(&mut self) -> ReflectedType {
        self.skip_ws();
        match self.peek() {
            Some(b'(') => self.parse_tuple(),
            Some(b'[') => self.parse_array(),
            Some(b'&') => self.parse_ref(),
            Some(b'?') | Some(b'_') => { self.consume(); ReflectedType::Wildcard }
            _ => self.parse_named(),
        }
    }

    fn parse_tuple(&mut self) -> ReflectedType {
        self.consume_if(b'(');
        let mut elems = Vec::new();
        loop {
            self.skip_ws();
            if self.consume_if(b')') { break; }
            elems.push(self.parse_type());
            self.skip_ws();
            self.consume_if(b',');
        }
        if elems.len() == 1 {
            // single-element tuple stays as tuple for correctness
        }
        ReflectedType::Tuple(elems)
    }

    fn parse_array(&mut self) -> ReflectedType {
        self.consume_if(b'[');
        let element = self.parse_type();
        self.skip_ws();
        let len = if self.consume_if(b';') {
            self.skip_ws();
            let n_str = self.consume_while(|b| b.is_ascii_digit());
            n_str.parse::<usize>().ok()
        } else {
            None
        };
        self.skip_ws();
        self.consume_if(b']');
        ReflectedType::Array { element: Box::new(element), len }
    }

    fn parse_ref(&mut self) -> ReflectedType {
        self.consume_if(b'&');
        self.skip_ws();
        let mutable = self.consume_keyword("mut");
        self.skip_ws();
        let inner = self.parse_type();
        let kind = if mutable { WrapperKind::RefMut } else { WrapperKind::Ref };
        ReflectedType::Wrapper { kind, inner: Box::new(inner) }
    }

    fn parse_named(&mut self) -> ReflectedType {
        // handle `dyn Trait`
        if self.consume_keyword("dyn") {
            self.skip_ws();
            let path = self.consume_path();
            return ReflectedType::TraitObject { trait_path: path };
        }

        let path = self.consume_path();
        if path.is_empty() {
            return ReflectedType::Wildcard;
        }

        // Check for primitive types
        if let Some(prim) = PrimitiveKind::from_str(&path) {
            return ReflectedType::Primitive(prim);
        }

        // Check for wildcard aliases
        if path == "?" || path == "_" {
            return ReflectedType::Wildcard;
        }

        self.skip_ws();
        if self.consume_if(b'<') {
            // Generic type — parse type arguments
            let args = self.parse_generic_args();
            self.skip_ws();
            self.consume_if(b'>');
            self.build_generic(path, args)
        } else {
            // Non-generic named type
            ReflectedType::Struct { path, generics: vec![] }
        }
    }

    fn consume_path(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.src.len() {
            let b = self.src.as_bytes()[self.pos];
            if b.is_ascii_alphanumeric() || b == b'_' || b == b':' {
                self.pos += 1;
            } else {
                break;
            }
        }
        // Trim trailing :: (e.g. std::collections::)
        let s = &self.src[start..self.pos];
        s.trim_end_matches(':').to_string()
    }

    fn consume_while(&mut self, pred: impl Fn(u8) -> bool) -> &'a str {
        let start = self.pos;
        while self.pos < self.src.len() && pred(self.src.as_bytes()[self.pos]) {
            self.pos += 1;
        }
        &self.src[start..self.pos]
    }

    fn parse_generic_args(&mut self) -> Vec<ReflectedType> {
        let mut args = Vec::new();
        loop {
            self.skip_ws();
            if matches!(self.peek(), Some(b'>') | None) { break; }
            args.push(self.parse_type());
            self.skip_ws();
            if !self.consume_if(b',') { break; }
        }
        args
    }

    fn build_generic(&self, path: String, mut args: Vec<ReflectedType>) -> ReflectedType {
        // Normalise path — strip module prefixes for well-known types
        let short = path.rsplit("::").next().unwrap_or(&path);
        match short {
            "Vec" if args.len() == 1 => ReflectedType::Wrapper {
                kind: WrapperKind::Vec,
                inner: Box::new(args.remove(0)),
            },
            "HashSet" if args.len() == 1 => ReflectedType::Wrapper {
                kind: WrapperKind::HashSet,
                inner: Box::new(args.remove(0)),
            },
            "Arc" if args.len() == 1 => ReflectedType::Wrapper {
                kind: WrapperKind::Arc,
                inner: Box::new(args.remove(0)),
            },
            "Rc" if args.len() == 1 => ReflectedType::Wrapper {
                kind: WrapperKind::Rc,
                inner: Box::new(args.remove(0)),
            },
            "Box" if args.len() == 1 => ReflectedType::Wrapper {
                kind: WrapperKind::Box,
                inner: Box::new(args.remove(0)),
            },
            "Option" if args.len() == 1 => ReflectedType::Wrapper {
                kind: WrapperKind::Option,
                inner: Box::new(args.remove(0)),
            },
            "Result" if args.len() == 2 => {
                let error = args.remove(1);
                let ok = args.remove(0);
                ReflectedType::Wrapper {
                    kind: WrapperKind::Result { error: Box::new(error) },
                    inner: Box::new(ok),
                }
            }
            "HashMap" if args.len() == 2 => {
                let val = args.remove(1);
                let key = args.remove(0);
                ReflectedType::Wrapper {
                    kind: WrapperKind::HashMap { key: Box::new(key) },
                    inner: Box::new(val),
                }
            }
            "BTreeMap" if args.len() == 2 => {
                let val = args.remove(1);
                let key = args.remove(0);
                ReflectedType::Wrapper {
                    kind: WrapperKind::BTreeMap { key: Box::new(key) },
                    inner: Box::new(val),
                }
            }
            _ => ReflectedType::Struct { path, generics: args },
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_round_trip() {
        for s in ["i32", "f64", "bool", "String", "u8", "char"] {
            let rt = ReflectedType::parse_str(s);
            assert_eq!(rt.to_type_string(), s, "round-trip failed for {s}");
        }
    }

    #[test]
    fn vec_option_round_trip() {
        let cases = [
            "Vec<i32>",
            "Option<String>",
            "Arc<Vec<f32>>",
            "Box<dyn MyTrait>",
        ];
        for s in cases {
            let rt = ReflectedType::parse_str(s);
            assert_eq!(rt.to_type_string(), s, "round-trip failed for {s}");
        }
    }

    #[test]
    fn result_hashmap_round_trip() {
        let rt = ReflectedType::parse_str("Result<String, i32>");
        assert_eq!(rt.to_type_string(), "Result<String, i32>");

        let rt2 = ReflectedType::parse_str("HashMap<String, Vec<i32>>");
        assert_eq!(rt2.to_type_string(), "HashMap<String, Vec<i32>>");
    }

    #[test]
    fn tuple_array_round_trip() {
        assert_eq!(ReflectedType::parse_str("(f32, f32)").to_type_string(), "(f32, f32)");
        assert_eq!(ReflectedType::parse_str("[f32; 4]").to_type_string(), "[f32; 4]");
    }

    #[test]
    fn wildcard_round_trip() {
        assert!(matches!(ReflectedType::parse_str("?"), ReflectedType::Wildcard));
        assert!(matches!(ReflectedType::parse_str("_"), ReflectedType::Wildcard));
    }

    #[test]
    fn datatype_reflect() {
        assert!(DataType::typed("f64").reflect().unwrap().is_numeric());
        assert!(matches!(DataType::any().reflect().unwrap(), ReflectedType::Wildcard));
        assert!(DataType::Exec.reflect().is_none());
    }

    #[test]
    fn property_value_json_roundtrip() {
        let v = PropertyValue::Float(3.14);
        let j = v.to_json();
        let v2 = PropertyValue::from_json(&j);
        assert!(matches!(v2, PropertyValue::Float(f) if (f - 3.14).abs() < 1e-10));
    }
}
