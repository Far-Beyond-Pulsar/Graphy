//! Tests for NodeMetadata, ParamInfo, and NodeMetadataProvider.

mod common;

use common::TestMetadataProvider;
use graphy::*;

// ===========================================================================
// ParamInfo
// ===========================================================================

#[test]
fn param_info_new() {
    let p = ParamInfo::new("value", "f64");
    assert_eq!(p.name, "value");
    assert_eq!(p.param_type, "f64");
}

#[test]
fn param_info_with_string_args() {
    let name = String::from("input");
    let ptype = String::from("Vec<u8>");
    let p = ParamInfo::new(name, ptype);
    assert_eq!(p.name, "input");
    assert_eq!(p.param_type, "Vec<u8>");
}

#[test]
fn param_info_clone() {
    let original = ParamInfo::new("x", "i32");
    let cloned = original.clone();
    assert_eq!(cloned.name, "x");
    assert_eq!(cloned.param_type, "i32");
}

// ===========================================================================
// NodeMetadata - Construction
// ===========================================================================

#[test]
fn node_metadata_new_defaults() {
    let meta = NodeMetadata::new("add", NodeTypes::pure, "math");
    assert_eq!(meta.name, "add");
    assert_eq!(meta.node_type, NodeTypes::pure);
    assert_eq!(meta.category, "math");
    assert!(meta.params.is_empty());
    assert!(meta.return_type.is_none());
    assert!(meta.exec_outputs.is_empty());
    assert!(meta.imports.is_empty());
    assert!(meta.function_source.is_empty());
}

#[test]
fn node_metadata_builder_pattern() {
    let meta = NodeMetadata::new("branch", NodeTypes::control_flow, "flow")
        .with_params(vec![ParamInfo::new("condition", "bool")])
        .with_exec_outputs(vec!["True".to_string(), "False".to_string()])
        .with_source("fn branch(condition: bool) {}")
        .with_imports(vec!["use std::fmt;".to_string()]);

    assert_eq!(meta.params.len(), 1);
    assert_eq!(meta.params[0].name, "condition");
    assert_eq!(meta.exec_outputs, vec!["True", "False"]);
    assert!(!meta.function_source.is_empty());
    assert_eq!(meta.imports.len(), 1);
}

#[test]
fn node_metadata_with_return_type_str() {
    let meta = NodeMetadata::new("add", NodeTypes::pure, "math")
        .with_return_type("i64");
    assert!(meta.return_type.is_some());
    assert_eq!(meta.return_type.unwrap().type_string, "i64");
}

#[test]
fn node_metadata_with_return_type_typeinfo() {
    let ti = core::TypeInfo::new("(f32, f32)");
    let meta = NodeMetadata::new("pos", NodeTypes::pure, "math")
        .with_return_type(ti);
    assert_eq!(meta.return_type.unwrap().type_string, "(f32, f32)");
}

#[test]
fn node_metadata_all_node_types() {
    for nt in [NodeTypes::pure, NodeTypes::fn_, NodeTypes::control_flow, NodeTypes::event] {
        let meta = NodeMetadata::new("test", nt, "test");
        assert_eq!(meta.node_type, nt);
    }
}

#[test]
fn node_metadata_clone() {
    let original = NodeMetadata::new("add", NodeTypes::pure, "math")
        .with_params(vec![ParamInfo::new("a", "i64")])
        .with_return_type("i64");
    let cloned = original.clone();
    assert_eq!(cloned.name, "add");
    assert_eq!(cloned.params.len(), 1);
    assert!(cloned.return_type.is_some());
}

// ===========================================================================
// NodeMetadataProvider
// ===========================================================================

#[test]
fn provider_get_node_metadata() {
    let provider = TestMetadataProvider::with_math_nodes();
    let add = provider.get_node_metadata("add");
    assert!(add.is_some());
    assert_eq!(add.unwrap().name, "add");
}

#[test]
fn provider_get_nonexistent_returns_none() {
    let provider = TestMetadataProvider::with_math_nodes();
    assert!(provider.get_node_metadata("nonexistent").is_none());
}

#[test]
fn provider_get_all_nodes() {
    let provider = TestMetadataProvider::with_math_nodes();
    let all = provider.get_all_nodes();
    assert_eq!(all.len(), 3); // add, multiply, negate
}

#[test]
fn provider_get_nodes_by_category() {
    let provider = TestMetadataProvider::comprehensive();
    let math_nodes = provider.get_nodes_by_category("math");
    assert_eq!(math_nodes.len(), 3);

    let flow_nodes = provider.get_nodes_by_category("flow");
    assert_eq!(flow_nodes.len(), 2); // branch, for_loop

    let empty = provider.get_nodes_by_category("nonexistent");
    assert!(empty.is_empty());
}

#[test]
fn provider_empty() {
    let provider = TestMetadataProvider::empty();
    assert!(provider.get_all_nodes().is_empty());
    assert!(provider.get_node_metadata("add").is_none());
}

#[test]
fn provider_comprehensive_has_all_types() {
    let provider = TestMetadataProvider::comprehensive();
    let all = provider.get_all_nodes();

    let has_pure = all.iter().any(|m| m.node_type == NodeTypes::pure);
    let has_fn = all.iter().any(|m| m.node_type == NodeTypes::fn_);
    let has_cf = all.iter().any(|m| m.node_type == NodeTypes::control_flow);
    let has_ev = all.iter().any(|m| m.node_type == NodeTypes::event);

    assert!(has_pure);
    assert!(has_fn);
    assert!(has_cf);
    assert!(has_ev);
}
