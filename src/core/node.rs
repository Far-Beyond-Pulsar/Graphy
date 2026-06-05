//! # Node Representation
//!
//! Data structures for representing nodes and their pins in the graph.
//!
//! ## NodeKind
//!
//! Every `NodeInstance` exposes a [`NodeKind`] via [`NodeInstance::kind()`].
//! The kind is derived at runtime from the `node_type` string using the
//! following conventions:
//!
//! | `node_type` prefix / value     | `NodeKind`                          |
//! |-------------------------------|--------------------------------------|
//! | `"reroute"`                   | `NodeKind::Reroute`                  |
//! | `"subgraph_entry"`            | `NodeKind::SubgraphEntry`            |
//! | `"subgraph_exit"`             | `NodeKind::SubgraphExit`             |
//! | `"macro:<id>"` or `"subgraph:<id>"` | `NodeKind::SubgraphCall(id)` |
//! | anything else                 | `NodeKind::Native(node_type)`        |
//!
//! These conventions are documented and stable. Application code may rely on
//! them for node creation and graph traversal.
//!
//! ## Typed Properties
//!
//! `NodeInstance` now carries an optional `typed_properties` map alongside
//! the legacy `properties: HashMap<String, JsonValue>`. New code should
//! populate `typed_properties`; the legacy map is retained for serialization
//! compatibility with v1 graphs.

use super::{DataType, JsonValue, Position, PropertyValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// NodeKind
// ─────────────────────────────────────────────────────────────────────────────

/// The semantic kind of a node, derived from its `node_type` string.
///
/// `NodeKind` is *not* serialized — it is computed on demand from
/// [`NodeInstance::node_type`]. This keeps the serialized format stable while
/// allowing typed dispatch in compiler passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    /// A standard node whose metadata is looked up from the metadata provider.
    /// The contained string is identical to `NodeInstance::node_type`.
    Native(String),

    /// A call-site that invokes another graph by ID.
    /// Graph IDs follow the convention `"macro:<uuid>"` or `"subgraph:<uuid>"`.
    SubgraphCall(String),

    /// Auto-generated entry-point node placed inside a macro/subgraph graph.
    SubgraphEntry,

    /// Auto-generated exit-point node placed inside a macro/subgraph graph.
    SubgraphExit,

    /// A visual pass-through reroute node (no code generation).
    Reroute,
}

impl NodeKind {
    /// Returns `true` if this node is a subgraph call site.
    #[inline]
    pub fn is_subgraph_call(&self) -> bool {
        matches!(self, NodeKind::SubgraphCall(_))
    }

    /// Returns the target graph ID if this is a `SubgraphCall`.
    #[inline]
    pub fn subgraph_id(&self) -> Option<&str> {
        match self {
            NodeKind::SubgraphCall(id) => Some(id),
            _ => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pin / PinType / PinInstance
// ─────────────────────────────────────────────────────────────────────────────

/// Direction of data flow for a pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinType {
    Input,
    Output,
}

/// A pin definition template.
///
/// Pins are the connection points on nodes. Every pin has a direction and a
/// [`DataType`] that constrains what values may flow through it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pin {
    /// Unique identifier for this pin on its node.
    pub id: String,

    /// Human-readable display name.
    pub name: String,

    /// Type of data this pin accepts or produces.
    pub data_type: DataType,

    /// Whether this is an input or output pin.
    pub pin_type: PinType,
}

impl Pin {
    /// Creates a new pin.
    #[inline]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        data_type: DataType,
        pin_type: PinType,
    ) -> Self {
        Self { id: id.into(), name: name.into(), data_type, pin_type }
    }

    /// Returns `true` if this is an execution-flow pin.
    #[inline]
    pub fn is_exec(&self) -> bool {
        self.data_type.is_execution()
    }
}

/// A pin instance on a specific node in the graph.
///
/// While [`Pin`] is a template definition, `PinInstance` represents an actual
/// pin occupying a slot on a [`NodeInstance`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinInstance {
    /// Instance-specific ID (may differ from the pin template ID when nodes
    /// are cloned during subgraph expansion).
    pub id: String,

    /// The underlying pin definition.
    pub pin: Pin,
}

impl PinInstance {
    #[inline]
    pub fn new(id: impl Into<String>, pin: Pin) -> Self {
        Self { id: id.into(), pin }
    }

    /// Convenience: returns the data type of the underlying pin.
    #[inline]
    pub fn data_type(&self) -> &DataType {
        &self.pin.data_type
    }

    /// Convenience: returns the pin direction.
    #[inline]
    pub fn pin_type(&self) -> PinType {
        self.pin.pin_type
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NodeInstance
// ─────────────────────────────────────────────────────────────────────────────

/// A node instance in the graph.
///
/// Represents a placed instantiation of a node type with specific inputs,
/// outputs, and property values. Each node has a unique `id` within its
/// containing `Graph`.
///
/// ## Properties
///
/// Two property maps coexist for compatibility:
///
/// - `properties` — legacy `serde_json::Value` map, always present in
///   serialized form. Do not remove.
/// - `typed_properties` — `PropertyValue` map introduced in v2. Populated by
///   editors and compiler passes that understand the richer type. Serialized
///   only when non-empty.
///
/// When both maps contain an entry for the same key, `typed_properties` takes
/// precedence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInstance {
    /// Unique identifier within the graph.
    pub id: String,

    /// Type identifier used to look up metadata from the provider.
    ///
    /// Conventions:
    /// - `"reroute"` — visual reroute node
    /// - `"subgraph_entry"` / `"subgraph_exit"` — macro interface nodes
    /// - `"macro:<graph_id>"` or `"subgraph:<graph_id>"` — subgraph call
    /// - anything else — native node looked up in the metadata registry
    pub node_type: String,

    /// Position in the visual editor canvas.
    pub position: Position,

    /// Input pins.
    pub inputs: Vec<PinInstance>,

    /// Output pins.
    pub outputs: Vec<PinInstance>,

    /// Legacy constant properties (JSON). Retained for v1 compatibility.
    pub properties: HashMap<String, JsonValue>,

    /// Typed constant properties (v2). Populated by v2-aware code.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub typed_properties: HashMap<String, PropertyValue>,
}

impl NodeInstance {
    /// Creates a new node instance with no pins or properties.
    #[inline]
    pub fn new(
        id: impl Into<String>,
        node_type: impl Into<String>,
        position: Position,
    ) -> Self {
        Self {
            id: id.into(),
            node_type: node_type.into(),
            position,
            inputs: Vec::new(),
            outputs: Vec::new(),
            properties: HashMap::new(),
            typed_properties: HashMap::new(),
        }
    }

    /// Derives the [`NodeKind`] for this node from its `node_type` string.
    ///
    /// This is a pure function — it performs no allocations for `Reroute`,
    /// `SubgraphEntry`, and `SubgraphExit` variants.
    pub fn kind(&self) -> NodeKind {
        match self.node_type.as_str() {
            "reroute" => NodeKind::Reroute,
            "subgraph_entry" => NodeKind::SubgraphEntry,
            "subgraph_exit" => NodeKind::SubgraphExit,
            s if s.starts_with("macro:") => {
                NodeKind::SubgraphCall(s["macro:".len()..].to_string())
            }
            s if s.starts_with("subgraph:") => {
                NodeKind::SubgraphCall(s["subgraph:".len()..].to_string())
            }
            other => NodeKind::Native(other.to_string()),
        }
    }

    /// Returns `true` if this is a subgraph call node.
    #[inline]
    pub fn is_subgraph_call(&self) -> bool {
        self.kind().is_subgraph_call()
    }

    /// Returns `true` if this is a reroute node.
    #[inline]
    pub fn is_reroute(&self) -> bool {
        self.node_type == "reroute"
    }

    // ── Pin helpers ──────────────────────────────────────────────────────────

    /// Adds an input pin (id and name are identical).
    #[inline]
    pub fn add_input_pin(&mut self, id: impl Into<String>, data_type: DataType) {
        let id_str = id.into();
        let pin = Pin::new(id_str.clone(), id_str.clone(), data_type, PinType::Input);
        self.inputs.push(PinInstance::new(id_str, pin));
    }

    /// Adds an output pin (id and name are identical).
    #[inline]
    pub fn add_output_pin(&mut self, id: impl Into<String>, data_type: DataType) {
        let id_str = id.into();
        let pin = Pin::new(id_str.clone(), id_str.clone(), data_type, PinType::Output);
        self.outputs.push(PinInstance::new(id_str, pin));
    }

    /// Adds a named input pin (id and name may differ).
    #[inline]
    pub fn add_named_input_pin(
        &mut self,
        id: impl Into<String>,
        name: impl Into<String>,
        data_type: DataType,
    ) {
        let id_str = id.into();
        let pin = Pin::new(id_str.clone(), name, data_type, PinType::Input);
        self.inputs.push(PinInstance::new(id_str, pin));
    }

    /// Adds a named output pin (id and name may differ).
    #[inline]
    pub fn add_named_output_pin(
        &mut self,
        id: impl Into<String>,
        name: impl Into<String>,
        data_type: DataType,
    ) {
        let id_str = id.into();
        let pin = Pin::new(id_str.clone(), name, data_type, PinType::Output);
        self.outputs.push(PinInstance::new(id_str, pin));
    }

    /// Finds an input pin by ID.
    #[inline]
    pub fn input_pin(&self, id: &str) -> Option<&PinInstance> {
        self.inputs.iter().find(|p| p.id == id)
    }

    /// Finds an output pin by ID.
    #[inline]
    pub fn output_pin(&self, id: &str) -> Option<&PinInstance> {
        self.outputs.iter().find(|p| p.id == id)
    }

    // ── Property helpers ─────────────────────────────────────────────────────

    /// Sets a legacy JSON property.
    #[inline]
    pub fn set_property(&mut self, key: impl Into<String>, value: impl Into<JsonValue>) {
        self.properties.insert(key.into(), value.into());
    }

    /// Gets a legacy JSON property.
    #[inline]
    pub fn get_property(&self, key: &str) -> Option<&JsonValue> {
        self.properties.get(key)
    }

    /// Sets a typed property (v2).
    #[inline]
    pub fn set_typed_property(&mut self, key: impl Into<String>, value: PropertyValue) {
        let k = key.into();
        self.properties.insert(k.clone(), value.to_json());
        self.typed_properties.insert(k, value);
    }

    /// Gets a typed property (v2). Falls back to parsing the legacy JSON entry
    /// if no typed entry is present.
    pub fn get_typed_property(&self, key: &str) -> Option<PropertyValue> {
        if let Some(v) = self.typed_properties.get(key) {
            return Some(v.clone());
        }
        self.properties.get(key).map(PropertyValue::from_json)
    }

    // ── Subgraph helpers ─────────────────────────────────────────────────────

    /// Prefixes all node IDs, pin IDs, and the node's own ID with `prefix`.
    ///
    /// Used by [`SubGraphExpander`] to avoid ID collisions when inlining a
    /// subgraph into a parent graph.
    pub fn with_id_prefix(&self, prefix: &str) -> NodeInstance {
        let mut cloned = self.clone();
        cloned.id = format!("{}{}", prefix, self.id);
        for pin in &mut cloned.inputs {
            pin.id = format!("{}{}", prefix, pin.id);
            pin.pin.id = format!("{}{}", prefix, pin.pin.id);
        }
        for pin in &mut cloned.outputs {
            pin.id = format!("{}{}", prefix, pin.id);
            pin.pin.id = format!("{}{}", prefix, pin.pin.id);
        }
        cloned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_dispatch() {
        assert!(matches!(NodeInstance::new("a", "reroute", Position::zero()).kind(), NodeKind::Reroute));
        assert!(matches!(NodeInstance::new("a", "subgraph_entry", Position::zero()).kind(), NodeKind::SubgraphEntry));
        assert!(matches!(NodeInstance::new("a", "subgraph_exit", Position::zero()).kind(), NodeKind::SubgraphExit));
        let call = NodeInstance::new("a", "macro:abc123", Position::zero());
        assert_eq!(call.kind().subgraph_id(), Some("abc123"));
        let native = NodeInstance::new("a", "math.add", Position::zero());
        assert!(matches!(native.kind(), NodeKind::Native(_)));
    }

    #[test]
    fn id_prefix() {
        let mut node = NodeInstance::new("node1", "add", Position::zero());
        node.add_input_pin("a", DataType::typed("f64"));
        node.add_output_pin("result", DataType::typed("f64"));
        let prefixed = node.with_id_prefix("scope_");
        assert_eq!(prefixed.id, "scope_node1");
        assert_eq!(prefixed.inputs[0].id, "scope_a");
        assert_eq!(prefixed.outputs[0].id, "scope_result");
    }

    #[test]
    fn typed_property_roundtrip() {
        let mut node = NodeInstance::new("n", "const", Position::zero());
        node.set_typed_property("value", PropertyValue::Float(2.5));
        let v = node.get_typed_property("value").unwrap();
        assert!(matches!(v, PropertyValue::Float(f) if (f - 2.5).abs() < 1e-10));
        // legacy map also populated
        assert!(node.properties.contains_key("value"));
    }
}
