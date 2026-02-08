//! Tests for NodeInstance, Pin, PinInstance, and PinType.

use graphy::*;

// ===========================================================================
// PinType
// ===========================================================================

#[test]
fn pintype_input_output_ne() {
    assert_ne!(PinType::Input, PinType::Output);
}

#[test]
fn pintype_copy() {
    let a = PinType::Input;
    let b = a;
    assert_eq!(a, b);
}

// ===========================================================================
// Pin
// ===========================================================================

#[test]
fn pin_new() {
    let pin = Pin::new("my_pin", "My Pin", DataType::Number, PinType::Input);
    assert_eq!(pin.id, "my_pin");
    assert_eq!(pin.name, "My Pin");
    assert_eq!(pin.data_type, DataType::Number);
    assert_eq!(pin.pin_type, PinType::Input);
}

#[test]
fn pin_typed_data() {
    let pin = Pin::new("result", "Result", DataType::Typed("Vec<f64>".into()), PinType::Output);
    assert_eq!(pin.pin_type, PinType::Output);
    match &pin.data_type {
        DataType::Typed(ti) => assert_eq!(ti.type_string, "Vec<f64>"),
        _ => panic!("expected Typed"),
    }
}

#[test]
fn pin_execution_type() {
    let pin = Pin::new("exec", "Exec", DataType::Execution, PinType::Input);
    assert_eq!(pin.data_type, DataType::Execution);
}

#[test]
fn pin_clone() {
    let original = Pin::new("a", "A", DataType::Boolean, PinType::Input);
    let cloned = original.clone();
    assert_eq!(cloned.id, "a");
    assert_eq!(cloned.name, "A");
    assert_eq!(cloned.data_type, DataType::Boolean);
}

// ===========================================================================
// PinInstance
// ===========================================================================

#[test]
fn pin_instance_new() {
    let pin = Pin::new("x", "X", DataType::Number, PinType::Input);
    let instance = PinInstance::new("x", pin);
    assert_eq!(instance.id, "x");
    assert_eq!(instance.pin.name, "X");
}

#[test]
fn pin_instance_clone() {
    let pin = Pin::new("y", "Y", DataType::String, PinType::Output);
    let instance = PinInstance::new("y", pin);
    let cloned = instance.clone();
    assert_eq!(cloned.id, "y");
}

// ===========================================================================
// NodeInstance - Construction
// ===========================================================================

#[test]
fn node_new() {
    let node = NodeInstance::new("node_1", "add", Position::new(10.0, 20.0));
    assert_eq!(node.id, "node_1");
    assert_eq!(node.node_type, "add");
    assert_eq!(node.position.x, 10.0);
    assert_eq!(node.position.y, 20.0);
    assert!(node.inputs.is_empty());
    assert!(node.outputs.is_empty());
    assert!(node.properties.is_empty());
}

#[test]
fn node_new_with_string_args() {
    let id = String::from("dynamic_id");
    let ntype = String::from("multiply");
    let node = NodeInstance::new(id, ntype, Position::zero());
    assert_eq!(node.id, "dynamic_id");
    assert_eq!(node.node_type, "multiply");
}

// ===========================================================================
// NodeInstance - Pins
// ===========================================================================

#[test]
fn node_add_input_pin() {
    let mut node = NodeInstance::new("n", "add", Position::zero());
    node.add_input_pin("a", DataType::Typed("i64".into()));
    node.add_input_pin("b", DataType::Typed("i64".into()));

    assert_eq!(node.inputs.len(), 2);
    assert_eq!(node.inputs[0].id, "a");
    assert_eq!(node.inputs[1].id, "b");
    assert_eq!(node.inputs[0].pin.pin_type, PinType::Input);
}

#[test]
fn node_add_output_pin() {
    let mut node = NodeInstance::new("n", "add", Position::zero());
    node.add_output_pin("result", DataType::Typed("i64".into()));

    assert_eq!(node.outputs.len(), 1);
    assert_eq!(node.outputs[0].id, "result");
    assert_eq!(node.outputs[0].pin.pin_type, PinType::Output);
}

#[test]
fn node_add_execution_pins() {
    let mut node = NodeInstance::new("n", "print", Position::zero());
    node.add_input_pin("exec_in", DataType::Execution);
    node.add_output_pin("exec_out", DataType::Execution);

    assert_eq!(node.inputs.len(), 1);
    assert_eq!(node.outputs.len(), 1);
    assert_eq!(node.inputs[0].pin.data_type, DataType::Execution);
    assert_eq!(node.outputs[0].pin.data_type, DataType::Execution);
}

#[test]
fn node_mixed_pins() {
    let mut node = NodeInstance::new("branch", "branch", Position::zero());
    node.add_input_pin("exec_in", DataType::Execution);
    node.add_input_pin("condition", DataType::Typed("bool".into()));
    node.add_output_pin("True", DataType::Execution);
    node.add_output_pin("False", DataType::Execution);

    assert_eq!(node.inputs.len(), 2);
    assert_eq!(node.outputs.len(), 2);
}

// ===========================================================================
// NodeInstance - Properties
// ===========================================================================

#[test]
fn node_set_and_get_property() {
    let mut node = NodeInstance::new("n", "add", Position::zero());
    node.set_property("value", PropertyValue::Number(42.0));

    let prop = node.get_property("value");
    assert!(prop.is_some());
    match prop.unwrap() {
        PropertyValue::Number(n) => assert_eq!(*n, 42.0),
        _ => panic!("expected Number"),
    }
}

#[test]
fn node_get_nonexistent_property_returns_none() {
    let node = NodeInstance::new("n", "add", Position::zero());
    assert!(node.get_property("missing").is_none());
}

#[test]
fn node_set_property_overwrites() {
    let mut node = NodeInstance::new("n", "add", Position::zero());
    node.set_property("x", PropertyValue::Number(1.0));
    node.set_property("x", PropertyValue::Number(2.0));

    match node.get_property("x").unwrap() {
        PropertyValue::Number(n) => assert_eq!(*n, 2.0),
        _ => panic!("expected Number"),
    }
}

#[test]
fn node_multiple_property_types() {
    let mut node = NodeInstance::new("n", "complex", Position::zero());
    node.set_property("name", PropertyValue::String("foo".into()));
    node.set_property("count", PropertyValue::Number(10.0));
    node.set_property("enabled", PropertyValue::Boolean(true));
    node.set_property("pos", PropertyValue::Vector2(1.0, 2.0));
    node.set_property("dir", PropertyValue::Vector3(0.0, 1.0, 0.0));
    node.set_property("tint", PropertyValue::Color(1.0, 0.0, 0.0, 1.0));

    assert_eq!(node.properties.len(), 6);
}

// ===========================================================================
// NodeInstance - Clone
// ===========================================================================

#[test]
fn node_clone_preserves_all_data() {
    let mut node = NodeInstance::new("original", "add", Position::new(5.0, 10.0));
    node.add_input_pin("a", DataType::Typed("i64".into()));
    node.add_output_pin("result", DataType::Typed("i64".into()));
    node.set_property("a", PropertyValue::Number(7.0));

    let cloned = node.clone();
    assert_eq!(cloned.id, "original");
    assert_eq!(cloned.node_type, "add");
    assert_eq!(cloned.position.x, 5.0);
    assert_eq!(cloned.inputs.len(), 1);
    assert_eq!(cloned.outputs.len(), 1);
    assert!(cloned.get_property("a").is_some());
}
