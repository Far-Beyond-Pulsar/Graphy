//! Shared test helpers used across all integration test modules.

use graphy::*;
use std::collections::HashMap;

/// A test metadata provider with configurable node definitions.
pub struct TestMetadataProvider {
    pub metadata: HashMap<String, NodeMetadata>,
}

impl TestMetadataProvider {
    pub fn new() -> Self {
        Self {
            metadata: HashMap::new(),
        }
    }

    pub fn empty() -> Self {
        Self::new()
    }

    /// Create a provider with common math nodes (add, multiply, negate).
    pub fn with_math_nodes() -> Self {
        let mut provider = Self::new();

        provider.add(
            NodeMetadata::new("add", NodeTypes::pure, "math")
                .with_params(vec![
                    ParamInfo::new("a", "i64"),
                    ParamInfo::new("b", "i64"),
                ])
                .with_return_type("i64"),
        );

        provider.add(
            NodeMetadata::new("multiply", NodeTypes::pure, "math")
                .with_params(vec![
                    ParamInfo::new("a", "i64"),
                    ParamInfo::new("b", "i64"),
                ])
                .with_return_type("i64"),
        );

        provider.add(
            NodeMetadata::new("negate", NodeTypes::pure, "math")
                .with_params(vec![ParamInfo::new("value", "i64")])
                .with_return_type("i64"),
        );

        provider
    }

    /// Create a provider with function nodes (print, set_variable).
    pub fn with_function_nodes() -> Self {
        let mut provider = Self::new();

        provider.add(
            NodeMetadata::new("print_string", NodeTypes::fn_, "io")
                .with_params(vec![ParamInfo::new("message", "String")])
                .with_exec_outputs(vec!["then".to_string()]),
        );

        provider.add(
            NodeMetadata::new("set_variable", NodeTypes::fn_, "variables")
                .with_params(vec![
                    ParamInfo::new("name", "String"),
                    ParamInfo::new("value", "i64"),
                ])
                .with_exec_outputs(vec!["then".to_string()]),
        );

        provider
    }

    /// Create a provider with control flow nodes (branch, for_loop).
    pub fn with_control_flow_nodes() -> Self {
        let mut provider = Self::new();

        provider.add(
            NodeMetadata::new("branch", NodeTypes::control_flow, "flow")
                .with_params(vec![ParamInfo::new("condition", "bool")])
                .with_exec_outputs(vec!["True".to_string(), "False".to_string()])
                .with_source(
                    r#"fn branch(condition: bool) {
                        if condition {
                            exec_output!("True");
                        } else {
                            exec_output!("False");
                        }
                    }"#,
                ),
        );

        provider.add(
            NodeMetadata::new("for_loop", NodeTypes::control_flow, "flow")
                .with_params(vec![
                    ParamInfo::new("start", "i64"),
                    ParamInfo::new("end", "i64"),
                ])
                .with_exec_outputs(vec!["body".to_string(), "completed".to_string()]),
        );

        provider
    }

    /// Create a provider with event nodes.
    pub fn with_event_nodes() -> Self {
        let mut provider = Self::new();

        provider.add(
            NodeMetadata::new("on_start", NodeTypes::event, "events")
                .with_exec_outputs(vec!["exec".to_string()]),
        );

        provider.add(
            NodeMetadata::new("on_tick", NodeTypes::event, "events")
                .with_params(vec![ParamInfo::new("delta_time", "f64")])
                .with_exec_outputs(vec!["exec".to_string()]),
        );

        provider
    }

    /// Create a comprehensive provider with all node types.
    pub fn comprehensive() -> Self {
        let mut provider = Self::new();
        for (_, meta) in Self::with_math_nodes().metadata {
            provider.metadata.insert(meta.name.clone(), meta);
        }
        for (_, meta) in Self::with_function_nodes().metadata {
            provider.metadata.insert(meta.name.clone(), meta);
        }
        for (_, meta) in Self::with_control_flow_nodes().metadata {
            provider.metadata.insert(meta.name.clone(), meta);
        }
        for (_, meta) in Self::with_event_nodes().metadata {
            provider.metadata.insert(meta.name.clone(), meta);
        }
        provider
    }

    pub fn add(&mut self, meta: NodeMetadata) {
        self.metadata.insert(meta.name.clone(), meta);
    }
}

impl NodeMetadataProvider for TestMetadataProvider {
    fn get_node_metadata(&self, node_type: &str) -> Option<&NodeMetadata> {
        self.metadata.get(node_type)
    }

    fn get_all_nodes(&self) -> Vec<&NodeMetadata> {
        self.metadata.values().collect()
    }

    fn get_nodes_by_category(&self, category: &str) -> Vec<&NodeMetadata> {
        self.metadata
            .values()
            .filter(|m| m.category == category)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Graph builder helpers
// ---------------------------------------------------------------------------

/// Build a simple linear chain: node_0 -> node_1 -> ... -> node_{n-1}
/// All nodes are pure "add" nodes with data connections.
pub fn build_linear_chain(n: usize, provider: &TestMetadataProvider) -> GraphDescription {
    let _ = provider; // provider needed for DataResolver, not graph building
    let mut graph = GraphDescription::new("linear_chain");

    for i in 0..n {
        let mut node = NodeInstance::new(
            format!("node_{}", i),
            "add",
            Position::new(i as f64 * 200.0, 0.0),
        );
        node.add_input_pin("a", DataType::Typed("i64".into()));
        node.add_input_pin("b", DataType::Typed("i64".into()));
        node.add_output_pin("result", DataType::Typed("i64".into()));
        node.set_property("b", PropertyValue::Number(1.0));
        graph.add_node(node);
    }

    // Chain: node_i.result -> node_{i+1}.a
    for i in 0..n.saturating_sub(1) {
        graph.add_connection(Connection::data(
            format!("node_{}", i),
            "result",
            format!("node_{}", i + 1),
            "a",
        ));
    }

    graph
}

/// Build a diamond-shaped dependency graph:
///
///       node_a
///      /      \
///   node_b   node_c
///      \      /
///       node_d
pub fn build_diamond_graph() -> GraphDescription {
    let mut graph = GraphDescription::new("diamond");

    for (id, node_type) in [
        ("node_a", "add"),
        ("node_b", "multiply"),
        ("node_c", "multiply"),
        ("node_d", "add"),
    ] {
        let mut node = NodeInstance::new(id, node_type, Position::zero());
        node.add_input_pin("a", DataType::Typed("i64".into()));
        node.add_input_pin("b", DataType::Typed("i64".into()));
        node.add_output_pin("result", DataType::Typed("i64".into()));
        node.set_property("a", PropertyValue::Number(1.0));
        node.set_property("b", PropertyValue::Number(2.0));
        graph.add_node(node);
    }

    graph.add_connection(Connection::data("node_a", "result", "node_b", "a"));
    graph.add_connection(Connection::data("node_a", "result", "node_c", "a"));
    graph.add_connection(Connection::data("node_b", "result", "node_d", "a"));
    graph.add_connection(Connection::data("node_c", "result", "node_d", "b"));

    graph
}

/// Build a graph with an execution flow chain.
pub fn build_exec_chain(n: usize) -> GraphDescription {
    let mut graph = GraphDescription::new("exec_chain");

    for i in 0..n {
        let mut node = NodeInstance::new(
            format!("fn_{}", i),
            "print_string",
            Position::new(i as f64 * 200.0, 0.0),
        );
        node.add_input_pin("exec_in", DataType::Execution);
        node.add_input_pin("message", DataType::Typed("String".into()));
        node.add_output_pin("exec_out", DataType::Execution);
        node.set_property("message", PropertyValue::String(format!("step {}", i)));
        graph.add_node(node);
    }

    for i in 0..n.saturating_sub(1) {
        graph.add_connection(Connection::execution(
            format!("fn_{}", i),
            "exec_out",
            format!("fn_{}", i + 1),
            "exec_in",
        ));
    }

    graph
}

/// Build a graph with a branch (control flow).
pub fn build_branch_graph() -> GraphDescription {
    let mut graph = GraphDescription::new("branch_graph");

    // Event entry
    let mut event = NodeInstance::new("start", "on_start", Position::zero());
    event.add_output_pin("exec", DataType::Execution);
    graph.add_node(event);

    // Branch node
    let mut branch = NodeInstance::new("branch_1", "branch", Position::new(200.0, 0.0));
    branch.add_input_pin("exec_in", DataType::Execution);
    branch.add_input_pin("condition", DataType::Typed("bool".into()));
    branch.add_output_pin("True", DataType::Execution);
    branch.add_output_pin("False", DataType::Execution);
    branch.set_property("condition", PropertyValue::Boolean(true));
    graph.add_node(branch);

    // True-side print
    let mut print_true = NodeInstance::new("print_true", "print_string", Position::new(400.0, -100.0));
    print_true.add_input_pin("exec_in", DataType::Execution);
    print_true.add_input_pin("message", DataType::Typed("String".into()));
    print_true.add_output_pin("exec_out", DataType::Execution);
    print_true.set_property("message", PropertyValue::String("true branch".into()));
    graph.add_node(print_true);

    // False-side print
    let mut print_false = NodeInstance::new("print_false", "print_string", Position::new(400.0, 100.0));
    print_false.add_input_pin("exec_in", DataType::Execution);
    print_false.add_input_pin("message", DataType::Typed("String".into()));
    print_false.add_output_pin("exec_out", DataType::Execution);
    print_false.set_property("message", PropertyValue::String("false branch".into()));
    graph.add_node(print_false);

    // Connections
    graph.add_connection(Connection::execution("start", "exec", "branch_1", "exec_in"));
    graph.add_connection(Connection::execution("branch_1", "True", "print_true", "exec_in"));
    graph.add_connection(Connection::execution("branch_1", "False", "print_false", "exec_in"));

    graph
}
