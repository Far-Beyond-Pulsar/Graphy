//! Stress Test Example - Push Graphy to its limits
//!
//! This example creates increasingly complex graphs to test performance characteristics.

use graphy::{
    GraphDescription, NodeInstance, Connection, Pin, PinInstance, PinType,
    DataType, NodeTypes, PropertyValue, ConnectionType, Position,
    DataResolver, ExecutionRouting, NodeMetadata, ParamInfo, NodeMetadataProvider,
};
use std::collections::HashMap;
use std::time::Instant;

// Simple metadata provider for stress testing
struct StressTestProvider {
    metadata: HashMap<String, NodeMetadata>,
}

impl StressTestProvider {
    fn new() -> Self {
        let mut metadata = HashMap::new();

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
            "math.constant".to_string(),
            NodeMetadata::new("constant", NodeTypes::pure, "Math")
                .with_return_type("f64")
                .with_source("value"),
        );

        Self { metadata }
    }
}

impl NodeMetadataProvider for StressTestProvider {
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

/// Create a massive interconnected grid of nodes
fn create_stress_grid(width: usize, height: usize) -> GraphDescription {
    let mut graph = GraphDescription::new(format!("stress_grid_{}x{}", width, height));

    println!("Creating {}x{} grid ({} nodes)...", width, height, width * height);

    // Create grid nodes
    for row in 0..height {
        for col in 0..width {
            let node_id = format!("n_{}_{}", row, col);
            let mut node = NodeInstance::new(
                &node_id,
                if (row + col) % 2 == 0 { "math.add" } else { "math.multiply" },
                Position::new(col as f64 * 100.0, row as f64 * 100.0)
            );

            node.inputs.push(PinInstance::new("a", Pin::new("a", "A", DataType::Typed("f64".into()), PinType::Input)));
            node.inputs.push(PinInstance::new("b", Pin::new("b", "B", DataType::Typed("f64".into()), PinType::Input)));
            node.outputs.push(PinInstance::new("result", Pin::new("result", "Result", DataType::Typed("f64".into()), PinType::Output)));

            // Edge nodes have constants
            if col == 0 || row == 0 {
                node.properties.insert("a".to_string(), PropertyValue::Number((row + col) as f64));
                node.properties.insert("b".to_string(), PropertyValue::Number(1.0));
            }

            graph.add_node(node);
        }
    }

    println!("Creating connections...");
    let mut connection_count = 0;

    // Connect horizontally
    for row in 0..height {
        for col in 1..width {
            graph.add_connection(Connection {
                source_node: format!("n_{}_{}", row, col - 1),
                source_pin: "result".to_string(),
                target_node: format!("n_{}_{}", row, col),
                target_pin: "a".to_string(),
                connection_type: ConnectionType::Data,
            });
            connection_count += 1;
        }
    }

    // Connect vertically
    for row in 1..height {
        for col in 0..width {
            graph.add_connection(Connection {
                source_node: format!("n_{}_{}", row - 1, col),
                source_pin: "result".to_string(),
                target_node: format!("n_{}_{}", row, col),
                target_pin: "b".to_string(),
                connection_type: ConnectionType::Data,
            });
            connection_count += 1;
        }
    }

    // Connect diagonally (STRESS!)
    for row in 1..height {
        for col in 1..width {
            if (row + col) % 3 == 0 {
                graph.add_connection(Connection {
                    source_node: format!("n_{}_{}", row - 1, col - 1),
                    source_pin: "result".to_string(),
                    target_node: format!("n_{}_{}", row, col),
                    target_pin: if row % 2 == 0 { "a" } else { "b" }.to_string(),
                    connection_type: ConnectionType::Data,
                });
                connection_count += 1;
            }
        }
    }

    println!("Created {} connections", connection_count);
    graph
}

fn run_stress_test(name: &str, graph: &GraphDescription, provider: &StressTestProvider) {
    println!("\n========== {} ==========", name);
    println!("  Nodes: {}", graph.nodes.len());
    println!("  Connections: {}", graph.connections.len());

    // Test serialization
    let start = Instant::now();
    let json = serde_json::to_string(&graph).unwrap();
    let serialize_time = start.elapsed();
    println!("  📄 Serialization: {:?} ({} bytes)", serialize_time, json.len());

    let start = Instant::now();
    let _: GraphDescription = serde_json::from_str(&json).unwrap();
    let deserialize_time = start.elapsed();
    println!("  📄 Deserialization: {:?}", deserialize_time);

    // Test data flow analysis - Sequential
    let start = Instant::now();
    match DataResolver::build(&graph, provider) {
        Ok(_resolver) => {
            let analysis_time = start.elapsed();
            println!("  ✅ Data Flow Analysis (Sequential): {:?}", analysis_time);
        }
        Err(e) => {
            let analysis_time = start.elapsed();
            println!("  ❌ Data Flow Analysis Failed: {:?} after {:?}", e, analysis_time);
        }
    }

    // Test data flow analysis - Parallel
    let start = Instant::now();
    match DataResolver::build_parallel(&graph, provider) {
        Ok(_resolver) => {
            let analysis_time = start.elapsed();
            println!("  ⚡ Data Flow Analysis (Parallel): {:?}", analysis_time);
        }
        Err(e) => {
            let analysis_time = start.elapsed();
            println!("  ❌ Parallel Analysis Failed: {:?} after {:?}", e, analysis_time);
        }
    }

    // Test execution routing
    let start = Instant::now();
    let _routing = ExecutionRouting::build_from_graph(&graph);
    let routing_time = start.elapsed();
    println!("  ✅ Execution Routing: {:?}", routing_time);

    // Memory estimation
    let estimated_memory = (graph.nodes.len() * 200 + graph.connections.len() * 100) / 1024;
    println!("  💾 Est. Memory: ~{} KB", estimated_memory);
}

fn main() {
    println!("\n🔥🔥🔥 GRAPHY STRESS TEST 🔥🔥🔥\n");
    println!("Let's push this library to its limits!\n");

    // Pre-initialize thread pool for zero-overhead parallel processing
    println!("⚡ Initializing thread pool...");
    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    
    use graphy::parallel::{init_thread_pool, ThreadPoolConfig};
    let config = ThreadPoolConfig::new()
        .with_num_threads(num_cpus)
        .with_stack_size(2 * 1024 * 1024);
    
    init_thread_pool(config).expect("Failed to initialize thread pool");
    println!("✅ Thread pool ready with {} threads\n", num_cpus);

    let provider = StressTestProvider::new();

    // Test 1: Small warm-up
    let graph_10x10 = create_stress_grid(10, 10);
    run_stress_test("Warm-up: 10x10 Grid", &graph_10x10, &provider);

    // Test 2: Medium load
    let graph_50x50 = create_stress_grid(50, 50);
    run_stress_test("Medium: 50x50 Grid", &graph_50x50, &provider);

    // Test 3: Large graph
    let graph_100x100 = create_stress_grid(100, 100);
    run_stress_test("Large: 100x100 Grid", &graph_100x100, &provider);

    // Test 4: Extra large (if you dare!)
    println!("\n⚠️  Warning: The next test creates a MASSIVE graph!");
    println!("This may take significant time and memory...\n");
    
    let graph_200x200 = create_stress_grid(200, 200);
    run_stress_test("🚨 EXTREME: 200x200 Grid", &graph_200x200, &provider);

    // Test 5: THE MONSTER (40,000 nodes!)
    println!("\n💀💀💀 UNLEASHING THE MONSTER 💀💀💀");
    println!("Creating 200x200 grid = 40,000 nodes...");
    println!("This is where mere mortals cry...\n");

    let start = Instant::now();
    let monster = create_stress_grid(200, 200);
    let creation_time = start.elapsed();
    
    println!("\n  Graph creation took: {:?}", creation_time);
    run_stress_test("💀 THE MONSTER", &monster, &provider);

    println!("\n============================================\n");
    println!("🎉 Stress test complete! Graphy survived! 🎉\n");
}
