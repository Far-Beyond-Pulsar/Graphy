//! # Node Representation
//!
//! Data structures for representing nodes and their pins.

use super::{DataType, Position, PropertyValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A pin definition (template)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pin {
    pub id: String,
    pub name: String,
    pub data_type: DataType,
    pub pin_type: PinType,
}

impl Pin {
    pub fn new(id: impl Into<String>, name: impl Into<String>, data_type: DataType, pin_type: PinType) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            data_type,
            pin_type,
        }
    }
}

/// Pin type (input or output)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinType {
    Input,
    Output,
}

/// A pin instance on a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinInstance {
    pub id: String,
    pub pin: Pin,
}

impl PinInstance {
    pub fn new(id: impl Into<String>, pin: Pin) -> Self {
        Self {
            id: id.into(),
            pin,
        }
    }
}

/// A node instance in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInstance {
    /// Unique ID for this node instance
    pub id: String,

    /// Node type (e.g., "add", "branch", "print_string")
    pub node_type: String,

    /// Visual position in the editor
    pub position: Position,

    /// Input pins
    pub inputs: Vec<PinInstance>,

    /// Output pins
    pub outputs: Vec<PinInstance>,

    /// Property values (constants)
    pub properties: HashMap<String, PropertyValue>,
}

impl NodeInstance {
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

    pub fn add_input_pin(&mut self, id: impl Into<String>, data_type: DataType) {
        let id_str = id.into();
        let pin = Pin::new(id_str.clone(), id_str.clone(), data_type, PinType::Input);
        self.inputs.push(PinInstance::new(id_str, pin));
    }

    pub fn add_output_pin(&mut self, id: impl Into<String>, data_type: DataType) {
        let id_str = id.into();
        let pin = Pin::new(id_str.clone(), id_str.clone(), data_type, PinType::Output);
        self.outputs.push(PinInstance::new(id_str, pin));
    }

    pub fn set_property(&mut self, key: impl Into<String>, value: PropertyValue) {
        self.properties.insert(key.into(), value);
    }

    pub fn get_property(&self, key: &str) -> Option<&PropertyValue> {
        self.properties.get(key)
    }
}
