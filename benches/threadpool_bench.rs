use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use graphy::{
    GraphDescription, NodeInstance, Connection, Pin, PinInstance, PinType,
    DataType, NodeTypes, PropertyValue, ConnectionType, Position,
    DataResolver, NodeMetadata, ParamInfo, NodeMetadataProvider,
};
use std::collections::HashMap;

// Minimal metadata provider
struct BenchProvider {
    metadata: HashMap<String, NodeMetadata>,
}

impl BenchProvider {
    fn new() -> Self {
        let mut metadata = HashMap::new();
        metadata.insert(
            "math.add".to_string(),
            NodeMetadata::new("add", NodeTypes::pure, "Math")
                .with_params(vec![
                    ParamInfo::new("a", "f64"),
                    ParamInfo::new("b", "f64"),
                ])
                .with_return_type("f64"),
        );
        metadata.insert(
            "math.multiply".to_string(),
            NodeMetadata::new("multiply", NodeTypes::pure, "Math")
                .with_params(vec![
                    ParamInfo::new("a", "f64"),
                    ParamInfo::new("b", "f64"),
                ])
                .with_return_type("f64"),
        );
        Self { metadata }
    }
}

impl NodeMetadataProvider for BenchProvider {
    fn get_node_metadata(&self, node_type: &str) -> Option<&NodeMetadata> {
        self.metadata.get(node_type)
    }
    fn get_all_nodes(&self) -> Vec<&NodeMetadata> {
        self.metadata.values().collect()
    }
    fn get_nodes_by_category(&self, category: &str) -> Vec<&NodeMetadata> {
        self.metadata.values().filter(|m| m.category == category).collect()
    }
}

fn create_grid(size: usize) -> GraphDescription {
    let mut graph = GraphDescription::new(format!("grid_{}", size));
    
    for row in 0..size {
        for col in 0..size {
            let id = format!("n_{}_{}", row, col);
            let mut node = NodeInstance::new(
                &id,
                if (row + col) % 2 == 0 { "math.add" } else { "math.multiply" },
                Position::new(col as f64 * 100.0, row as f64 * 100.0)
            );
            
            node.inputs.push(PinInstance::new("a", Pin::new("a", "A", DataType::Typed("f64".into()), PinType::Input)));
            node.inputs.push(PinInstance::new("b", Pin::new("b", "B", DataType::Typed("f64".into()), PinType::Input)));
            node.outputs.push(PinInstance::new("result", Pin::new("result", "Result", DataType::Typed("f64".into()), PinType::Output)));
            
            if col == 0 || row == 0 {
                node.properties.insert("a".to_string(), PropertyValue::Number(1.0));
                node.properties.insert("b".to_string(), PropertyValue::Number(1.0));
            }
            
            graph.add_node(node);
        }
    }
    
    // Connections
    for row in 0..size {
        for col in 1..size {
            graph.add_connection(Connection {
                source_node: format!("n_{}_{}", row, col - 1),
                source_pin: "result".to_string(),
                target_node: format!("n_{}_{}", row, col),
                target_pin: "a".to_string(),
                connection_type: ConnectionType::Data,
            });
        }
    }
    
    for row in 1..size {
        for col in 0..size {
            graph.add_connection(Connection {
                source_node: format!("n_{}_{}", row - 1, col),
                source_pin: "result".to_string(),
                target_node: format!("n_{}_{}", row, col),
                target_pin: "b".to_string(),
                connection_type: ConnectionType::Data,
            });
        }
    }
    
    graph
}

fn bench_cold_vs_warm(c: &mut Criterion) {
    let mut group = c.benchmark_group("threadpool_warmup");
    let provider = BenchProvider::new();
    let graph = create_grid(50);
    
    // Benchmark WITHOUT pre-warming (cold start)
    group.bench_function("cold_start", |b| {
        b.iter(|| {
            // Each iteration has to pay thread spawn cost
            let resolver = DataResolver::build_parallel(black_box(&graph), black_box(&provider)).unwrap();
            black_box(resolver);
        });
    });
    
    // Initialize thread pool
    use graphy::parallel::{init_thread_pool, ThreadPoolConfig};
    let config = ThreadPoolConfig::new();
    let _ = init_thread_pool(config);
    
    // Benchmark WITH pre-warming (hot start)
    group.bench_function("pre_warmed", |b| {
        b.iter(|| {
            // Zero thread spawn cost!
            let resolver = DataResolver::build_parallel(black_box(&graph), black_box(&provider)).unwrap();
            black_box(resolver);
        });
    });
    
    group.finish();
}

fn bench_scaling_with_threadpool(c: &mut Criterion) {
    use graphy::parallel::{init_thread_pool, ThreadPoolConfig};
    
    let mut group = c.benchmark_group("optimized_parallel_scaling");
    group.sample_size(20);
    
    // Pre-warm the thread pool
    let config = ThreadPoolConfig::new();
    let _ = init_thread_pool(config);
    
    let provider = BenchProvider::new();
    
    for size in [30, 50, 70, 100].iter() {
        let graph = create_grid(*size);
        
        group.bench_with_input(BenchmarkId::new("sequential", size), size, |b, &_size| {
            b.iter(|| {
                let resolver = DataResolver::build(black_box(&graph), black_box(&provider)).unwrap();
                black_box(resolver);
            });
        });
        
        group.bench_with_input(BenchmarkId::new("parallel_optimized", size), size, |b, &_size| {
            b.iter(|| {
                let resolver = DataResolver::build_parallel(black_box(&graph), black_box(&provider)).unwrap();
                black_box(resolver);
            });
        });
    }
    
    group.finish();
}

criterion_group!(benches, bench_cold_vs_warm, bench_scaling_with_threadpool);
criterion_main!(benches);
