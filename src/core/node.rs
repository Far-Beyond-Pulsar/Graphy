//! # Node Representation
//!
//! Data structures for representing nodes and their pins in the graph.
//!
//! Nodes are the computational units of a graph. Each node has:
//! - Input and output **pins** for connecting to other nodes
//! - **Properties** that store constant values
//! - A **node type** that determines its behavior
//!
//! # Example
//!
//! ```
//! use graphy::{NodeInstance, Position, DataType, PropertyValue};
//!
//! let mut node = NodeInstance::new("add_1", "math.add", Position::zero());
//! node.add_input_pin("a", DataType::Number);
//! node.add_input_pin("b", DataType::Number);
//! node.add_output_pin("result", DataType::Number);
//! node.set_property("default_a", PropertyValue::Number(0.0));
//! ```

use super::{DataType, Position, PropertyValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A pin definition template.
///
/// Pins are the connection points on nodes. They have a type (input/output)
/// and a data type that determines what kind of values can flow through them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pin {
    /// Unique identifier for this pin
    pub id: String,
    
    /// Human-readable display name
    pub name: String,
    
    /// Type of data this pin accepts/produces
    pub data_type: DataType,
    
    /// Whether this is an input or output pin
    pub pin_type: PinType,
}

impl Pin {
    /// Creates a new pin with the given parameters.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::{Pin, DataType, PinType};
    ///
    /// let pin = Pin::new("value", "Value", DataType::Number, PinType::Input);
    /// ```
    #[inline]
    pub fn new(id: impl Into<String>, name: impl Into<String>, data_type: DataType, pin_type: PinType) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            data_type,
            pin_type,
        }
    }
}

/// Direction of data flow for a pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinType {
    /// Input pin (receives data)
    Input,
    
    /// Output pin (produces data)
    Output,
}

/// A pin instance on a specific node.
///
/// While [`Pin`] is a template, `PinInstance` represents an actual pin
/// on a node instance in the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinInstance {
    /// Instance-specific ID (may differ from pin template ID)
    pub id: String,
    
    /// The pin template
    pub pin: Pin,
}

impl PinInstance {
    /// Creates a new pin instance.
    #[inline]
    pub fn new(id: impl Into<String>, pin: Pin) -> Self {
        Self {
            id: id.into(),
            pin,
        }
    }
}

/// A node instance in the graph.
///
/// Represents an instantiation of a node type with specific inputs, outputs,
/// and property values. Each node has a unique ID within the graph.
///
/// # Performance
///
/// Properties are stored in a `HashMap` for O(1) access. For nodes with
/// many properties (10+), this is significantly faster than linear search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInstance {
    /// Unique identifier for this node instance within the graph
    pub id: String,

    /// Type identifier (e.g., "math.add", "control.branch")
    /// Used to look up node metadata from the provider
    pub node_type: String,

    /// Position in the visual editor (2D coordinates)
    pub position: Position,

    /// Input pins for receiving values
    pub inputs: Vec<PinInstance>,

    /// Output pins for producing values
    pub outputs: Vec<PinInstance>,

    /// Constant property values (defaults, configuration, etc.)
    pub properties: HashMap<String, PropertyValue>,
}

impl NodeInstance {
    /// Creates a new node instance with no pins or properties.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::{NodeInstance, Position};
    ///
    /// let node = NodeInstance::new("node_1", "math.add", Position::new(100.0, 200.0));
    /// assert_eq!(node.id, "node_1");
    /// assert_eq!(node.node_type, "math.add");
    /// ```
    #[inline]
    pub fn new(id: impl Into<String>, node_type: impl Into<String>, position: Position) -> Self {
        Self {
            id: id.into(),
            node_type: node_type.into(),
            position,
            inputs: Vec::new(),
            outputs: Vec::new(),
            properties: HashMap::new(),
        }
    }

    /// Adds an input pin to this node.
    ///
    /// The pin ID and name will be the same. For custom names, create a [`Pin`] directly.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::{NodeInstance, Position, DataType};
    ///
    /// let mut node = NodeInstance::new("add_1", "math.add", Position::zero());
    /// node.add_input_pin("a", DataType::Number);
    /// node.add_input_pin("b", DataType::Number);
    /// ```
    #[inline]
    pub fn add_input_pin(&mut self, id: impl Into<String>, data_type: DataType) {
        let id_str = id.into();
        let pin = Pin::new(id_str.clone(), id_str.clone(), data_type, PinType::Input);
        self.inputs.push(PinInstance::new(id_str, pin));
    }

    /// Adds an output pin to this node.
    ///
    /// The pin ID and name will be the same. For custom names, create a [`Pin`] directly.
    #[inline]
    pub fn add_output_pin(&mut self, id: impl Into<String>, data_type: DataType) {
        let id_str = id.into();
        let pin = Pin::new(id_str.clone(), id_str.clone(), data_type, PinType::Output);
        self.outputs.push(PinInstance::new(id_str, pin));
    }

    /// Sets a property value on this node.
    ///
    /// Properties are constant values that configure the node's behavior.
    ///
    /// # Example
    ///
    /// ```
    /// use graphy::{NodeInstance, Position, PropertyValue};
    ///
    /// let mut node = NodeInstance::new("const_1", "constant", Position::zero());
    /// node.set_property("value", PropertyValue::Number(42.0));
    /// ```
    #[inline]
    pub fn set_property(&mut self, key: impl Into<String>, value: PropertyValue) {
        self.properties.insert(key.into(), value);
    }

    /// Gets a property value by key.
    ///
    /// Returns `None` if the property doesn't exist.
    ///
    /// # Performance
    ///
    /// This is an O(1) operation thanks to `HashMap` storage.
    #[inline]
    pub fn get_property(&self, key: &str) -> Option<&PropertyValue> {
        self.properties.get(key)
    }
}
