use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use graphy::{
    GraphDescription, NodeInstance, Connection, Pin, PinInstance, PinType,
    DataType, NodeTypes, PropertyValue, ConnectionType, Position,
    DataResolver, ExecutionRouting, NodeMetadata, ParamInfo, NodeMetadataProvider,
};
use std::collections::HashMap;

// ============================================================================
// Mock Metadata Provider for Benchmarks
// ============================================================================

struct BenchmarkMetadataProvider {
    metadata: HashMap<String, NodeMetadata>,
}

impl BenchmarkMetadataProvider {
    fn new() -> Self {
        let mut metadata = HashMap::new();

        // Pure math nodes
        metadata.insert(
            "math.add".to_string(),
            NodeMetadata::new("add", NodeTypes::pure, "Math")
                .with_params(vec![
                    ParamInfo::new("a", "f64"),
                    ParamInfo::new("b", "f64"),
                ])
                .with_return_type("f64")
                .with_source("a + b"),
        );

        metadata.insert(
            "math.constant".to_string(),
            NodeMetadata::new("constant", NodeTypes::pure, "Math")
                .with_return_type("f64")
                .with_source("value"),
        );

        metadata.insert(
            "math.multiply".to_string(),
            NodeMetadata::new("multiply", NodeTypes::pure, "Math")
                .with_params(vec![
                    ParamInfo::new("a", "f64"),
                    ParamInfo::new("b", "f64"),
                ])
                .with_return_type("f64")
                .with_source("a * b"),
        );

        metadata.insert(
            "math.sqrt".to_string(),
            NodeMetadata::new("sqrt", NodeTypes::pure, "Math")
                .with_params(vec![ParamInfo::new("value", "f64")])
                .with_return_type("f64")
                .with_source("value.sqrt()"),
        );

        // Function nodes (with side effects)
        metadata.insert(
            "print".to_string(),
            NodeMetadata::new("print", NodeTypes::fn_, "IO")
                .with_params(vec![ParamInfo::new("value", "String")])
                .with_exec_outputs(vec!["then".to_string()])
                .with_source(r#"println!("{}", value)"#),
        );

        // Control flow
        metadata.insert(
            "branch".to_string(),
            NodeMetadata::new("branch", NodeTypes::control_flow, "Flow")
                .with_params(vec![ParamInfo::new("condition", "bool")])
                .with_exec_outputs(vec!["true".to_string(), "false".to_string()]),
        );

        metadata.insert(
            "for_loop".to_string(),
            NodeMetadata::new("for_loop", NodeTypes::control_flow, "Flow")
                .with_params(vec![
                    ParamInfo::new("start", "i32"),
                    ParamInfo::new("end", "i32"),
                ])
                .with_exec_outputs(vec!["body".to_string(), "completed".to_string()]),
        );

        // Event
        metadata.insert(
            "event.start".to_string(),
            NodeMetadata::new("start", NodeTypes::event, "Events")
                .with_exec_outputs(vec!["exec".to_string()]),
        );

        Self { metadata }
    }
}

impl NodeMetadataProvider for BenchmarkMetadataProvider {
    fn get_node_metadata(&self, node_type: &str) -> Option<&NodeMetadata> {
        self.metadata.get(node_type)
    }

    fn get_all_nodes(&self) -> Vec<&NodeMetadata> {
        self.metadata.values().collect()
    }

    fn get_nodes_by_category(&self, category: &str) -> Vec<&NodeMetadata> {
        self.metadata.values()
            .filter(|m| m.category == category)
            .collect()
    }
}

// ============================================================================
// Graph Generation Functions
// ============================================================================

/// Creates a linear chain of pure math nodes
/// Pattern: constant -> add -> add -> add -> ... -> add
fn create_linear_chain(length: usize) -> GraphDescription {
    let mut graph = GraphDescription::new(format!("linear_chain_{}", length));

    // Create constant node
    let mut constant = NodeInstance::new("const_0", "math.constant", Position::new(0.0, 0.0));
    constant.properties.insert("value".to_string(), PropertyValue::Number(1.0));
    constant.outputs.push(PinInstance::new(
        "out",
        Pin::new("value", "Value", DataType::Typed("f64".into()), PinType::Output),
    ));
    graph.add_node(constant);

    // Create chain of add nodes
    for i in 0..length {
        let node_id = format!("add_{}", i);
        let mut node = NodeInstance::new(&node_id, "math.add", Position::new(100.0 * (i + 1) as f64, 0.0));
        
        node.inputs.push(PinInstance::new(
            "a",
            Pin::new("a", "A", DataType::Typed("f64".into()), PinType::Input),
        ));
        node.inputs.push(PinInstance::new(
            "b",
            Pin::new("b", "B", DataType::Typed("f64".into()), PinType::Input),
        ));
        node.outputs.push(PinInstance::new(
            "result",
            Pin::new("result", "Result", DataType::Typed("f64".into()), PinType::Output),
        ));
        
        node.properties.insert("b".to_string(), PropertyValue::Number(1.0));
        graph.add_node(node);

        // Connect to previous node
        let source = if i == 0 { "const_0" } else { &format!("add_{}", i - 1) };
        let source_pin = if i == 0 { "value" } else { "result" };

        graph.add_connection(Connection {
            source_node: source.to_string(),
            source_pin: source_pin.to_string(),
            target_node: node_id.clone(),
            target_pin: "a".to_string(),
            connection_type: ConnectionType::Data,
        });
    }

    graph
}

/// Creates a wide graph with many parallel pure nodes
/// Pattern: Multiple constants feeding into multiple operations that converge
fn create_wide_graph(width: usize) -> GraphDescription {
    let mut graph = GraphDescription::new(format!("wide_graph_{}", width));

    // Create multiple constant nodes
    for i in 0..width {
        let node_id = format!("const_{}", i);
        let mut node = NodeInstance::new(&node_id, "math.constant", Position::new(i as f64 * 100.0, 0.0));
        node.properties.insert("value".to_string(), PropertyValue::Number(i as f64));
        node.outputs.push(PinInstance::new(
            "value",
            Pin::new("value", "Value", DataType::Typed("f64".into()), PinType::Output),
        ));
        graph.add_node(node);
    }

    // Create operations for each pair
    for i in 0..width - 1 {
        let node_id = format!("op_{}", i);
        let mut node = NodeInstance::new(&node_id, "math.multiply", Position::new(i as f64 * 100.0 + 50.0, 200.0));
        
        node.inputs.push(PinInstance::new("a", Pin::new("a", "A", DataType::Typed("f64".into()), PinType::Input)));
        node.inputs.push(PinInstance::new("b", Pin::new("b", "B", DataType::Typed("f64".into()), PinType::Input)));
        node.outputs.push(PinInstance::new("result", Pin::new("result", "Result", DataType::Typed("f64".into()), PinType::Output)));
        
        graph.add_node(node);

        // Connect constants
        graph.add_connection(Connection {
            source_node: format!("const_{}", i),
            source_pin: "value".to_string(),
            target_node: node_id.clone(),
            target_pin: "a".to_string(),
            connection_type: ConnectionType::Data,
        });

        graph.add_connection(Connection {
            source_node: format!("const_{}", i + 1),
            source_pin: "value".to_string(),
            target_node: node_id.clone(),
            target_pin: "b".to_string(),
            connection_type: ConnectionType::Data,
        });
    }

    // Final convergence node
    let mut final_node = NodeInstance::new("final_add", "math.add", Position::new(width as f64 * 50.0, 400.0));
    final_node.inputs.push(PinInstance::new("a", Pin::new("a", "A", DataType::Typed("f64".into()), PinType::Input)));
    final_node.inputs.push(PinInstance::new("b", Pin::new("b", "B", DataType::Typed("f64".into()), PinType::Input)));
    final_node.outputs.push(PinInstance::new("result", Pin::new("result", "Result", DataType::Typed("f64".into()), PinType::Output)));
    graph.add_node(final_node);

    // Connect first and last operation to final
    if width >= 2 {
        graph.add_connection(Connection {
            source_node: "op_0".to_string(),
            source_pin: "result".to_string(),
            target_node: "final_add".to_string(),
            target_pin: "a".to_string(),
            connection_type: ConnectionType::Data,
        });

        graph.add_connection(Connection {
            source_node: format!("op_{}", width - 2),
            source_pin: "result".to_string(),
            target_node: "final_add".to_string(),
            target_pin: "b".to_string(),
            connection_type: ConnectionType::Data,
        });
    }

    graph
}

/// Creates a deeply nested dependency tree
/// Pattern: Binary tree of operations where each node depends on two children
fn create_dependency_tree(depth: usize) -> GraphDescription {
    let mut graph = GraphDescription::new(format!("dependency_tree_{}", depth));
    let mut node_counter = 0;

    fn add_tree_level(
        graph: &mut GraphDescription,
        depth: usize,
        current_depth: usize,
        _parent_id: &str,
        _is_left: bool,
        counter: &mut usize,
        x_offset: f64,
        y_pos: f64,
    ) -> String {
        let node_id = format!("node_{}", counter);
        *counter += 1;

        if current_depth == 0 {
            // Leaf node - constant
            let mut node = NodeInstance::new(&node_id, "math.constant", Position::new(x_offset, y_pos));
            node.properties.insert("value".to_string(), PropertyValue::Number(*counter as f64));
            node.outputs.push(PinInstance::new("value", Pin::new("value", "Value", DataType::Typed("f64".into()), PinType::Output)));
            graph.add_node(node);
        } else {
            // Internal node - operation
            let mut node = NodeInstance::new(&node_id, "math.add", Position::new(x_offset, y_pos));
            node.inputs.push(PinInstance::new("a", Pin::new("a", "A", DataType::Typed("f64".into()), PinType::Input)));
            node.inputs.push(PinInstance::new("b", Pin::new("b", "B", DataType::Typed("f64".into()), PinType::Input)));
            node.outputs.push(PinInstance::new("result", Pin::new("result", "Result", DataType::Typed("f64".into()), PinType::Output)));
            graph.add_node(node.clone());

            // Create children
            let spacing = 100.0 * 2_f64.powi(current_depth as i32);
            let left_child = add_tree_level(graph, depth, current_depth - 1, &node_id, true, counter, x_offset - spacing, y_pos + 150.0);
            let right_child = add_tree_level(graph, depth, current_depth - 1, &node_id, false, counter, x_offset + spacing, y_pos + 150.0);

            // Connect children
            graph.add_connection(Connection {
                source_node: left_child,
                source_pin: if current_depth == 1 { "value" } else { "result" }.to_string(),
                target_node: node_id.clone(),
                target_pin: "a".to_string(),
                connection_type: ConnectionType::Data,
            });

            graph.add_connection(Connection {
                source_node: right_child,
                source_pin: if current_depth == 1 { "value" } else { "result" }.to_string(),
                target_node: node_id.clone(),
                target_pin: "b".to_string(),
                connection_type: ConnectionType::Data,
            });
        }

        node_id
    }

    add_tree_level(&mut graph, depth, depth, "root", false, &mut node_counter, 1000.0, 0.0);
    graph
}

/// Creates a graph with complex control flow and branching
fn create_control_flow_graph(num_branches: usize) -> GraphDescription {
    let mut graph = GraphDescription::new(format!("control_flow_{}", num_branches));

    // Event start
    let mut event = NodeInstance::new("start", "event.start", Position::new(0.0, 0.0));
    event.outputs.push(PinInstance::new("exec", Pin::new("exec", "Exec", DataType::Execution, PinType::Output)));
    graph.add_node(event);

    // Create chain of branches
    for i in 0..num_branches {
        let branch_id = format!("branch_{}", i);
        let mut branch = NodeInstance::new(&branch_id, "branch", Position::new(200.0 * (i + 1) as f64, 0.0));
        
        branch.inputs.push(PinInstance::new("exec", Pin::new("exec", "Exec", DataType::Execution, PinType::Input)));
        branch.inputs.push(PinInstance::new("condition", Pin::new("condition", "Condition", DataType::Typed("bool".into()), PinType::Input)));
        branch.outputs.push(PinInstance::new("true", Pin::new("true", "True", DataType::Execution, PinType::Output)));
        branch.outputs.push(PinInstance::new("false", Pin::new("false", "False", DataType::Execution, PinType::Output)));
        
        branch.properties.insert("condition".to_string(), PropertyValue::Boolean(i % 2 == 0));
        graph.add_node(branch);

        // Connect execution flow
        let source_node = if i == 0 { "start" } else { &format!("print_false_{}", i - 1) };
        let source_pin = if i == 0 { "exec" } else { "then" };

        graph.add_connection(Connection {
            source_node: source_node.to_string(),
            source_pin: source_pin.to_string(),
            target_node: branch_id.clone(),
            target_pin: "exec".to_string(),
            connection_type: ConnectionType::Execution,
        });

        // Create print nodes for true and false paths
        let print_true_id = format!("print_true_{}", i);
        let mut print_true = NodeInstance::new(&print_true_id, "print", Position::new(200.0 * (i + 1) as f64, -150.0));
        print_true.inputs.push(PinInstance::new("exec", Pin::new("exec", "Exec", DataType::Execution, PinType::Input)));
        print_true.inputs.push(PinInstance::new("value", Pin::new("value", "Value", DataType::Typed("String".into()), PinType::Input)));
        print_true.outputs.push(PinInstance::new("then", Pin::new("then", "Then", DataType::Execution, PinType::Output)));
        print_true.properties.insert("value".to_string(), PropertyValue::String(format!("True branch {}", i)));
        graph.add_node(print_true);

        let print_false_id = format!("print_false_{}", i);
        let mut print_false = NodeInstance::new(&print_false_id, "print", Position::new(200.0 * (i + 1) as f64, 150.0));
        print_false.inputs.push(PinInstance::new("exec", Pin::new("exec", "Exec", DataType::Execution, PinType::Input)));
        print_false.inputs.push(PinInstance::new("value", Pin::new("value", "Value", DataType::Typed("String".into()), PinType::Input)));
        print_false.outputs.push(PinInstance::new("then", Pin::new("then", "Then", DataType::Execution, PinType::Output)));
        print_false.properties.insert("value".to_string(), PropertyValue::String(format!("False branch {}", i)));
        graph.add_node(print_false);

        // Connect branches to prints
        graph.add_connection(Connection {
            source_node: branch_id.clone(),
            source_pin: "true".to_string(),
            target_node: print_true_id,
            target_pin: "exec".to_string(),
            connection_type: ConnectionType::Execution,
        });

        graph.add_connection(Connection {
            source_node: branch_id,
            source_pin: "false".to_string(),
            target_node: print_false_id,
            target_pin: "exec".to_string(),
            connection_type: ConnectionType::Execution,
        });
    }

    graph
}

/// Creates a massive graph combining all stress patterns
fn create_monster_graph(scale: usize) -> GraphDescription {
    let mut graph = GraphDescription::new(format!("monster_graph_{}", scale));

    // Create a grid of interconnected pure nodes
    for row in 0..scale {
        for col in 0..scale {
            let node_id = format!("grid_{}_{}", row, col);
            let mut node = NodeInstance::new(&node_id, "math.multiply", Position::new(col as f64 * 150.0, row as f64 * 150.0));
            
            node.inputs.push(PinInstance::new("a", Pin::new("a", "A", DataType::Typed("f64".into()), PinType::Input)));
            node.inputs.push(PinInstance::new("b", Pin::new("b", "B", DataType::Typed("f64".into()), PinType::Input)));
            node.outputs.push(PinInstance::new("result", Pin::new("result", "Result", DataType::Typed("f64".into()), PinType::Output)));
            
            if col == 0 {
                node.properties.insert("a".to_string(), PropertyValue::Number(row as f64));
                node.properties.insert("b".to_string(), PropertyValue::Number(1.0));
            }
            
            graph.add_node(node);

            // Connect to left neighbor
            if col > 0 {
                graph.add_connection(Connection {
                    source_node: format!("grid_{}_{}", row, col - 1),
                    source_pin: "result".to_string(),
                    target_node: node_id.clone(),
                    target_pin: "a".to_string(),
                    connection_type: ConnectionType::Data,
                });
            }

            // Connect to top neighbor
            if row > 0 {
                graph.add_connection(Connection {
                    source_node: format!("grid_{}_{}", row - 1, col),
                    source_pin: "result".to_string(),
                    target_node: node_id.clone(),
                    target_pin: "b".to_string(),
                    connection_type: ConnectionType::Data,
                });
            }
        }
    }

    graph
}

// ============================================================================
// Benchmark Definitions
// ============================================================================

fn bench_linear_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("linear_chain_analysis");
    let provider = BenchmarkMetadataProvider::new();

    for size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let graph = create_linear_chain(size);
            b.iter(|| {
                let data_resolver = DataResolver::build(black_box(&graph), black_box(&provider)).unwrap();
                black_box(data_resolver);
            });
        });
    }
    group.finish();
}

fn bench_wide_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("wide_graph_analysis");
    let provider = BenchmarkMetadataProvider::new();

    for width in [10, 25, 50, 100, 200].iter() {
        group.throughput(Throughput::Elements(*width as u64));
        
        group.bench_with_input(BenchmarkId::from_parameter(width), width, |b, &width| {
            let graph = create_wide_graph(width);
            b.iter(|| {
                let data_resolver = DataResolver::build(black_box(&graph), black_box(&provider)).unwrap();
                black_box(data_resolver);
            });
        });
    }
    group.finish();
}

fn bench_dependency_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("dependency_tree_analysis");
    let provider = BenchmarkMetadataProvider::new();

    for depth in [3, 5, 7, 9, 10].iter() {
        let num_nodes = 2_usize.pow(*depth as u32 + 1) - 1;
        group.throughput(Throughput::Elements(num_nodes as u64));
        
        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, &depth| {
            let graph = create_dependency_tree(depth);
            b.iter(|| {
                let data_resolver = DataResolver::build(black_box(&graph), black_box(&provider)).unwrap();
                black_box(data_resolver);
            });
        });
    }
    group.finish();
}

fn bench_control_flow(c: &mut Criterion) {
    let mut group = c.benchmark_group("control_flow_analysis");

    for branches in [5, 10, 20, 50, 100].iter() {
        group.throughput(Throughput::Elements(*branches as u64));
        
        group.bench_with_input(BenchmarkId::from_parameter(branches), branches, |b, &branches| {
            let graph = create_control_flow_graph(branches);
            b.iter(|| {
                let exec_routing = ExecutionRouting::build_from_graph(black_box(&graph));
                black_box(exec_routing);
            });
        });
    }
    group.finish();
}

fn bench_monster_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("monster_graph_analysis");
    group.sample_size(10); // Reduce sample size for large graphs
    let provider = BenchmarkMetadataProvider::new();

    for scale in [10, 20, 30, 40, 50].iter() {
        let num_nodes = scale * scale;
        group.throughput(Throughput::Elements(num_nodes as u64));
        
        group.bench_with_input(BenchmarkId::from_parameter(scale), scale, |b, &scale| {
            let graph = create_monster_graph(scale);
            b.iter(|| {
                let data_resolver = DataResolver::build(black_box(&graph), black_box(&provider)).unwrap();
                black_box(data_resolver);
            });
        });
    }
    group.finish();
}

fn bench_graph_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_serialization");
    
    for size in [100, 500, 1000, 2000].iter() {
        let graph = create_linear_chain(*size);
        
        group.bench_with_input(BenchmarkId::new("serialize", size), &graph, |b, graph| {
            b.iter(|| {
                let json = serde_json::to_string(black_box(graph)).unwrap();
                black_box(json);
            });
        });
        
        let json = serde_json::to_string(&graph).unwrap();
        group.bench_with_input(BenchmarkId::new("deserialize", size), &json, |b, json| {
            b.iter(|| {
                let graph: GraphDescription = serde_json::from_str(black_box(json)).unwrap();
                black_box(graph);
            });
        });
    }
    group.finish();
}

fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_pipeline");
    group.sample_size(10);
    let provider = BenchmarkMetadataProvider::new();

    for size in [50, 100, 250, 500].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let graph = create_linear_chain(size);
            b.iter(|| {
                let data_resolver = DataResolver::build(black_box(&graph), black_box(&provider)).unwrap();
                let exec_routing = ExecutionRouting::build_from_graph(black_box(&graph));
                black_box((data_resolver, exec_routing));
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_linear_chain,
    bench_wide_graph,
    bench_dependency_tree,
    bench_control_flow,
    bench_monster_graph,
    bench_graph_serialization,
    bench_full_pipeline,
);

criterion_main!(benches);
