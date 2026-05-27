//! Tests for core type system: DataType, TypeInfo, NodeTypes, JsonValue, Position.

use graphy::*;

// ===========================================================================
// TypeInfo
// ===========================================================================

#[test]
fn typeinfo_new_from_str() {
    let ti = core::TypeInfo::new("i64");
    assert_eq!(ti.type_string, "i64");
}

#[test]
fn typeinfo_new_from_string() {
    let s = String::from("Vec<u8>");
    let ti = core::TypeInfo::new(s.clone());
    assert_eq!(ti.type_string, s);
}

#[test]
fn typeinfo_display() {
    let ti = core::TypeInfo::new("(f32, f32)");
    assert_eq!(format!("{}", ti), "(f32, f32)");
}

#[test]
fn typeinfo_from_str_ref() {
    let ti: core::TypeInfo = "bool".into();
    assert_eq!(ti.type_string, "bool");
}

#[test]
fn typeinfo_from_string() {
    let ti: core::TypeInfo = String::from("String").into();
    assert_eq!(ti.type_string, "String");
}

#[test]
fn typeinfo_clone_and_eq() {
    let a = core::TypeInfo::new("u32");
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn typeinfo_ne() {
    let a = core::TypeInfo::new("u32");
    let b = core::TypeInfo::new("i32");
    assert_ne!(a, b);
}

// ===========================================================================
// DataType
// ===========================================================================

#[test]
fn datatype_execution_variant() {
    let dt = DataType::Execution;
    assert_eq!(dt, DataType::Execution);
}

#[test]
fn datatype_typed_variant() {
    let dt = DataType::Typed("f64".into());
    match &dt {
        DataType::Typed(ti) => assert_eq!(ti.type_string, "f64"),
        _ => panic!("expected Typed variant"),
    }
}

#[test]
fn datatype_legacy_variants() {
    let variants = vec![
        DataType::Number,
        DataType::String,
        DataType::Boolean,
        DataType::Vector2,
        DataType::Vector3,
        DataType::Color,
        DataType::Any,
    ];
    // Each variant should be equal to itself
    for v in &variants {
        assert_eq!(v, v);
    }
}

#[test]
fn datatype_ne_across_variants() {
    assert_ne!(DataType::Number, DataType::Boolean);
    assert_ne!(DataType::Execution, DataType::Any);
}

#[test]
fn datatype_clone() {
    let original = DataType::Typed("HashMap<String, i32>".into());
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

// ===========================================================================
// NodeTypes
// ===========================================================================

#[test]
fn nodetypes_pure() {
    assert_eq!(NodeTypes::pure, NodeTypes::pure);
}

#[test]
fn nodetypes_fn() {
    assert_eq!(NodeTypes::fn_, NodeTypes::fn_);
}

#[test]
fn nodetypes_control_flow() {
    assert_eq!(NodeTypes::control_flow, NodeTypes::control_flow);
}

#[test]
fn nodetypes_event() {
    assert_eq!(NodeTypes::event, NodeTypes::event);
}

#[test]
fn nodetypes_ne() {
    assert_ne!(NodeTypes::pure, NodeTypes::fn_);
    assert_ne!(NodeTypes::control_flow, NodeTypes::event);
}

#[test]
fn nodetypes_copy() {
    let a = NodeTypes::pure;
    let b = a; // Copy
    assert_eq!(a, b);
}

// ===========================================================================
// JsonValue
// ===========================================================================

#[test]
fn property_value_string() {
    let pv: JsonValue = serde_json::json!("hello");
    assert_eq!(pv, serde_json::json!("hello"));
}

#[test]
fn property_value_number() {
    let pv: JsonValue = serde_json::json!(42.0);
    assert_eq!(pv, serde_json::json!(42.0));
}

#[test]
fn property_value_boolean() {
    let pv: JsonValue = serde_json::json!(true);
    assert_eq!(pv, serde_json::json!(true));
}

#[test]
fn property_value_vector2() {
    let pv: JsonValue = serde_json::json!([1.0, 2.0]);
    assert_eq!(pv, serde_json::json!([1.0, 2.0]));
}

#[test]
fn property_value_vector3() {
    let pv: JsonValue = serde_json::json!([1.0, 2.0, 3.0]);
    assert_eq!(pv, serde_json::json!([1.0, 2.0, 3.0]));
}

#[test]
fn property_value_color() {
    let pv: JsonValue = serde_json::json!([1.0, 0.5, 0.0, 1.0]);
    assert_eq!(pv, serde_json::json!([1.0, 0.5, 0.0, 1.0]));
}

#[test]
fn property_value_clone() {
    let original: JsonValue = serde_json::json!([1.0, 2.0, 3.0]);
    let cloned = original.clone();
    assert_eq!(original, cloned);
}

// ===========================================================================
// Position
// ===========================================================================

#[test]
fn position_new() {
    let p = Position::new(10.0, 20.0);
    assert_eq!(p.x, 10.0);
    assert_eq!(p.y, 20.0);
}

#[test]
fn position_zero() {
    let p = Position::zero();
    assert_eq!(p.x, 0.0);
    assert_eq!(p.y, 0.0);
}

#[test]
fn position_negative_coords() {
    let p = Position::new(-5.5, -10.3);
    assert_eq!(p.x, -5.5);
    assert_eq!(p.y, -10.3);
}

#[test]
fn position_copy() {
    let a = Position::new(1.0, 2.0);
    let b = a; // Copy
    assert_eq!(a.x, b.x);
    assert_eq!(a.y, b.y);
}

#[test]
fn position_clone() {
    let a = Position::new(3.0, 4.0);
    let b = a.clone();
    assert_eq!(a.x, b.x);
    assert_eq!(a.y, b.y);
}
