//! Tests for CodeGeneratorContext, CodeGenerator trait, and collect_node_arguments.

mod common;

use common::*;
use graphy::*;
use graphy::generation::collect_node_arguments;

// ===========================================================================
// CodeGeneratorContext - Indentation
// ===========================================================================

#[test]
fn context_initial_indent_is_zero() {
    let graph = GraphDescription::new("test");
    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let routing = ExecutionRouting::build_from_graph(&graph);

    let ctx = CodeGeneratorContext::new(&graph, &provider, &resolver, &routing);
    assert_eq!(ctx.indent(), "");
    assert_eq!(ctx.indent_level, 0);
}

#[test]
fn context_push_indent() {
    let graph = GraphDescription::new("test");
    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let routing = ExecutionRouting::build_from_graph(&graph);

    let mut ctx = CodeGeneratorContext::new(&graph, &provider, &resolver, &routing);

    ctx.push_indent();
    assert_eq!(ctx.indent(), "    ");
    assert_eq!(ctx.indent_level, 1);

    ctx.push_indent();
    assert_eq!(ctx.indent(), "        ");
    assert_eq!(ctx.indent_level, 2);
}

#[test]
fn context_pop_indent() {
    let graph = GraphDescription::new("test");
    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let routing = ExecutionRouting::build_from_graph(&graph);

    let mut ctx = CodeGeneratorContext::new(&graph, &provider, &resolver, &routing);

    ctx.push_indent();
    ctx.push_indent();
    ctx.pop_indent();
    assert_eq!(ctx.indent_level, 1);
    assert_eq!(ctx.indent(), "    ");
}

#[test]
fn context_pop_indent_at_zero_stays_zero() {
    let graph = GraphDescription::new("test");
    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let routing = ExecutionRouting::build_from_graph(&graph);

    let mut ctx = CodeGeneratorContext::new(&graph, &provider, &resolver, &routing);
    ctx.pop_indent(); // Already at 0
    assert_eq!(ctx.indent_level, 0);
}

#[test]
fn context_deep_indentation() {
    let graph = GraphDescription::new("test");
    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let routing = ExecutionRouting::build_from_graph(&graph);

    let mut ctx = CodeGeneratorContext::new(&graph, &provider, &resolver, &routing);
    for _ in 0..5 {
        ctx.push_indent();
    }
    assert_eq!(ctx.indent_level, 5);
    assert_eq!(ctx.indent(), "                    "); // 20 spaces
}

// ===========================================================================
// CodeGeneratorContext - Visited tracking
// ===========================================================================

#[test]
fn context_visited_initially_empty() {
    let graph = GraphDescription::new("test");
    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let routing = ExecutionRouting::build_from_graph(&graph);

    let ctx = CodeGeneratorContext::new(&graph, &provider, &resolver, &routing);
    assert!(!ctx.is_visited("any_node"));
}

#[test]
fn context_mark_and_check_visited() {
    let graph = GraphDescription::new("test");
    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let routing = ExecutionRouting::build_from_graph(&graph);

    let mut ctx = CodeGeneratorContext::new(&graph, &provider, &resolver, &routing);
    ctx.mark_visited("node_1");
    ctx.mark_visited("node_2");

    assert!(ctx.is_visited("node_1"));
    assert!(ctx.is_visited("node_2"));
    assert!(!ctx.is_visited("node_3"));
}

#[test]
fn context_reset_visited() {
    let graph = GraphDescription::new("test");
    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let routing = ExecutionRouting::build_from_graph(&graph);

    let mut ctx = CodeGeneratorContext::new(&graph, &provider, &resolver, &routing);
    ctx.mark_visited("node_1");
    ctx.mark_visited("node_2");
    ctx.reset_visited();

    assert!(!ctx.is_visited("node_1"));
    assert!(!ctx.is_visited("node_2"));
}

#[test]
fn context_mark_visited_idempotent() {
    let graph = GraphDescription::new("test");
    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let routing = ExecutionRouting::build_from_graph(&graph);

    let mut ctx = CodeGeneratorContext::new(&graph, &provider, &resolver, &routing);
    ctx.mark_visited("node_1");
    ctx.mark_visited("node_1"); // Mark again
    assert!(ctx.is_visited("node_1"));
}

// ===========================================================================
// CodeGeneratorContext - Graph access
// ===========================================================================

#[test]
fn context_has_graph_access() {
    let mut graph = GraphDescription::new("my_graph");
    graph.add_node(NodeInstance::new("n1", "add", Position::zero()));

    let provider = TestMetadataProvider::empty();
    let resolver = DataResolver::build(&graph, &provider).unwrap();
    let routing = ExecutionRouting::build_from_graph(&graph);

    let ctx = CodeGeneratorContext::new(&graph, &provider, &resolver, &routing);
    assert_eq!(ctx.graph.metadata.name, "my_graph");
    assert!(ctx.graph.get_node("n1").is_some());
}

// ===========================================================================
// collect_node_arguments
// ===========================================================================

#[test]
fn collect_args_with_properties() {
    let meta = NodeMetadata::new("add", NodeTypes::pure, "math")
        .with_params(vec![
            ParamInfo::new("a", "i64"),
            ParamInfo::new("b", "i64"),
        ]);

    let mut node = NodeInstance::new("add_1", "add", Position::zero());
    node.set_property("a", PropertyValue::Number(5.0));
    node.set_property("b", PropertyValue::Number(3.0));

    let args = collect_node_arguments(&node, &meta).unwrap();
    assert_eq!(args.len(), 2);
}

#[test]
fn collect_args_uses_defaults_for_missing_properties() {
    let meta = NodeMetadata::new("add", NodeTypes::pure, "math")
        .with_params(vec![
            ParamInfo::new("a", "i64"),
            ParamInfo::new("b", "i64"),
        ]);

    let node = NodeInstance::new("add_1", "add", Position::zero());
    // No properties set

    let args = collect_node_arguments(&node, &meta).unwrap();
    assert_eq!(args.len(), 2);
    // Both should be default "0" for i64
    assert_eq!(args[0], "0");
    assert_eq!(args[1], "0");
}

#[test]
fn collect_args_mixed_properties_and_defaults() {
    let meta = NodeMetadata::new("add", NodeTypes::pure, "math")
        .with_params(vec![
            ParamInfo::new("a", "f64"),
            ParamInfo::new("b", "f64"),
        ]);

    let mut node = NodeInstance::new("add_1", "add", Position::zero());
    node.set_property("a", PropertyValue::Number(7.5));
    // "b" not set

    let args = collect_node_arguments(&node, &meta).unwrap();
    assert_eq!(args.len(), 2);
    // a has property, b falls back to default
    assert_eq!(args[1], "0.0");
}

#[test]
fn collect_args_no_params() {
    let meta = NodeMetadata::new("start", NodeTypes::event, "events");
    let node = NodeInstance::new("start_1", "start", Position::zero());

    let args = collect_node_arguments(&node, &meta).unwrap();
    assert!(args.is_empty());
}
