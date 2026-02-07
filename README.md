<div align="center">

# 🌐 Graphy

### *General-Purpose Graph Compilation Library*

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-0.1.0-green.svg)](https://github.com/Trident-For-U/Graphy)

*Transform visual node graphs into executable code with elegance and precision.*

[Features](#-features) • [Installation](#-installation) • [Quick Start](#-quick-start) • [Architecture](#-architecture) • [Documentation](#-documentation) • [Examples](#-examples)

</div>

---

## 🎯 Overview

**Graphy** is a flexible, extensible framework for compiling visual node graphs into executable code. Designed for node-based visual programming environments, Graphy provides a robust compilation pipeline that transforms interconnected nodes into optimized, type-safe code in multiple target languages.

Whether you're building a visual scripting system, shader graph editor, or computational pipeline designer, Graphy handles the complexity of graph analysis, dependency resolution, and code generation through a clean, trait-based architecture.

### ✨ Key Highlights

- 🔄 **Multi-Phase Compilation** - Graph expansion, data flow analysis, execution routing, and code generation
- 🎨 **Target-Agnostic** - Support Rust, WGSL, or implement your own code generator
- 🧩 **Extensible Architecture** - Trait-based design for custom nodes and languages
- 📊 **Smart Analysis** - Topological sorting, cycle detection, and dependency resolution
- ⚡ **Parallel Processing** - Multi-threaded analysis with Rayon for large graphs (1.5x speedup at 6400+ nodes)
- 🔒 **Type-Safe** - Full type information tracking and validation
- 🎯 **Optimized Output** - Pure function inlining and execution flow optimization

---

## 🚀 Features

### Core Capabilities

- **Graph Structure Representation**
  - Nodes with typed input/output pins
  - Data and execution flow connections
  - Sub-graph support with expansion utilities
  - Property values and metadata

- **Advanced Analysis**
  - 📈 **Data Flow Analysis** - Resolve dependencies, topological sorting, and evaluation order
  - 🔀 **Execution Flow Analysis** - Build routing tables for control flow and branching
  - 🔍 **Cycle Detection** - Identify and report circular dependencies
  - 🎯 **Type Resolution** - Track and validate data types throughout the graph

- **Code Generation Framework**
  - 🛠️ **Pluggable Generators** - Implement `CodeGenerator` trait for any target language
  - 📝 **AST Transformation** - Built-in utilities for Rust AST manipulation
  - 🔤 **Variable Generation** - Automatic unique variable naming
  - 🎨 **Indentation Management** - Context-aware code formatting

### Node Type Support

| Type | Description | Characteristics |
|------|-------------|----------------|
| **Pure** | Computational units | No side effects, can be inlined as expressions |
| **Function** | Operations with side effects | Linear execution flow, requires exec pins |
| **Control Flow** | Branching logic | Multiple execution outputs (if/else, loops) |
| **Event** | Graph entry points | Trigger execution chains |

---

## 📦 Installation

Add Graphy to your `Cargo.toml`:

```toml
[dependencies]
graphy = "0.1.0"
```

Or use cargo-add:

```bash
cargo add graphy
```

---

## 🏃 Quick Start

### Basic Example: Compiling a Simple Graph

```rust
use graphy::{
    GraphDescription, NodeInstance, Connection, Pin, PinInstance,
    DataType, NodeTypes, PropertyValue, ConnectionType,
    DataResolver, ExecutionRouting, CodeGeneratorContext,
};

// 1. Define your graph structure
let mut graph = GraphDescription::new("my_graph");

// 2. Add nodes
graph.add_node(NodeInstance {
    id: "add_1".to_string(),
    node_type: "math.add".to_string(),
    position: Default::default(),
    properties: vec![
        ("a".to_string(), PropertyValue::Number(5.0)),
        ("b".to_string(), PropertyValue::Number(3.0)),
    ].into_iter().collect(),
});

// 3. Add connections
graph.add_connection(Connection {
    source_node: "add_1".to_string(),
    source_pin: "result".to_string(),
    target_node: "print_1".to_string(),
    target_pin: "value".to_string(),
    connection_type: ConnectionType::Data,
});

// 4. Analyze the graph
let metadata_provider = MyMetadataProvider::new();

// For small graphs (< 2000 nodes) - use sequential
let data_resolver = DataResolver::build(&graph, &metadata_provider)?;

// For large graphs (2000+ nodes) - use parallel processing
// let data_resolver = DataResolver::build_parallel(&graph, &metadata_provider)?;

let exec_routing = ExecutionRouting::build(&graph, &metadata_provider)?;

// 5. Generate code
let context = CodeGeneratorContext::new(
    &graph,
    &metadata_provider,
    &data_resolver,
    &exec_routing,
);

let generated_code = my_generator.generate(&context)?;
println!("{}", generated_code);
```

### Implementing a Custom Node Provider

```rust
use graphy::{NodeMetadataProvider, NodeMetadata, NodeTypes, ParamInfo};

struct MyMetadataProvider {
    // Your node definitions
}

impl NodeMetadataProvider for MyMetadataProvider {
    fn get_metadata(&self, node_type: &str) -> Option<NodeMetadata> {
        match node_type {
            "math.add" => Some(
                NodeMetadata::new("add", NodeTypes::pure, "Math")
                    .with_params(vec![
                        ParamInfo::new("a", "f64"),
                        ParamInfo::new("b", "f64"),
                    ])
                    .with_return_type("f64")
                    .with_function_source("a + b")
            ),
            _ => None,
        }
    }

    fn is_pure(&self, node_type: &str) -> bool {
        matches!(
            self.get_metadata(node_type).map(|m| m.node_type),
            Some(NodeTypes::pure)
        )
    }
}
```

---

## 🏗️ Architecture

Graphy follows a multi-phase compilation pipeline:

```
┌─────────────────────┐
│   Graph Input       │  JSON/Serialized graph description
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  Graph Expansion    │  Inline sub-graphs (optional)
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  Data Flow Analysis │  • Build dependency graph
│                     │  • Topological sort
└──────────┬──────────┤  • Resolve data sources
           │          │  • Generate variable names
           ▼          │
┌─────────────────────┐
│  Execution Flow     │  • Build routing table
│     Analysis        │  • Map exec connections
└──────────┬──────────┤  • Validate control flow
           │          │
           ▼          │
┌─────────────────────┐
│  Code Generation    │  • Generate target code
│                     │  • Inline pure nodes
└──────────┬──────────┤  • Emit control structures
           │          │  • Apply transformations
           ▼          │
┌─────────────────────┐
│   Output Code       │  Rust, WGSL, or custom target
└─────────────────────┘
```

### Module Organization

```
graphy/
├── core/              # Core data structures
│   ├── graph.rs       # Graph description and metadata
│   ├── node.rs        # Node instances and pins
│   ├── connection.rs  # Connection definitions
│   ├── types.rs       # Type system and enums
│   └── metadata.rs    # Node metadata and traits
│
├── analysis/          # Graph analysis passes
│   ├── data_flow.rs   # Data dependency resolution
│   └── exec_flow.rs   # Execution routing
│
├── generation/        # Code generation framework
│   ├── context.rs     # Generator context
│   └── strategies.rs  # Generation strategies
│
└── utils/             # Utility functions
    ├── subgraph_expander.rs  # Sub-graph inlining
    ├── variable_gen.rs       # Variable naming
    └── ast_transform.rs      # AST utilities
```

---

## 📚 Documentation

### Core Concepts

#### Graph Structure

A **Graph** consists of:
- **Nodes**: Computational or control flow units
- **Connections**: Links between node pins
- **Pins**: Input/output ports with type information

```rust
pub struct GraphDescription {
    pub id: String,
    pub metadata: GraphMetadata,
    pub nodes: Vec<NodeInstance>,
    pub connections: Vec<Connection>,
}
```

#### Node Instance

Each node in the graph has:
- Unique ID
- Node type (references metadata)
- Position (for visual editor)
- Properties (constant values)

```rust
pub struct NodeInstance {
    pub id: String,
    pub node_type: String,
    pub position: Position,
    pub properties: HashMap<String, PropertyValue>,
}
```

#### Connections

Links between nodes can be:
- **Data**: Transfer values between pins
- **Execution**: Control flow sequencing

```rust
pub enum ConnectionType {
    Data,
    Execution,
}
```

### Analysis Phase

#### Data Flow Resolution

The `DataResolver` determines:
1. Where each input gets its data from
2. What order to evaluate pure nodes
3. Variable names for intermediate results

```rust
pub enum DataSource {
    Connection { source_node_id: String, source_pin: String },
    Constant(String),
    Default,
}
```

#### Execution Flow Routing

The `ExecutionRouting` maps:
1. Which nodes follow each execution output
2. Entry points for graph execution
3. Control flow branching paths

### Generation Phase

Implement the generator trait for your target language:

```rust
pub trait CodeGenerator {
    fn generate<P: NodeMetadataProvider>(
        &self,
        context: &mut CodeGeneratorContext<P>,
    ) -> Result<String, GraphyError>;
}
```

---

## 🎨 Examples

### Example 1: Simple Math Expression

**Graph:**
```
[Constant: 10] ──┐
                 ├──> [Add] ──> [Multiply] ──> [Print]
[Constant: 5]  ──┘              ▲
                                │
[Constant: 2] ──────────────────┘
```

**Generated Code:**
```rust
fn my_graph() {
    let v0 = 10.0 + 5.0;
    let v1 = v0 * 2.0;
    println!("{}", v1);
}
```

### Example 2: Control Flow

**Graph:**
```
[Event: OnStart] ──> [If] ──┬──[true]──> [Print: "Yes"]
                      ▲     │
                      │     └──[false]──> [Print: "No"]
                      │
            [Compare: x > 10]
```

**Generated Code:**
```rust
fn on_start() {
    if x > 10.0 {
        println!("Yes");
    } else {
        println!("No");
    }
}
```

### Example 3: Sub-Graph Expansion

**Main Graph:**
```
[Input] ──> [SubGraph: Smoothing] ──> [Output]
```

**After Expansion:**
```
[Input] ──> [Multiply: 0.5] ──> [Add] ──> [Output]
                                  ▲
                    [Previous] ───┘
```

---

## ⚡ Performance

Graphy is designed for high performance with both sequential and parallel processing modes.

### Sequential Mode (Default)
- **Best for:** Interactive editing, graphs < 2,000 nodes
- **Complexity:** O(V + E) for all analysis passes
- **Latency:** < 5ms for typical graphs (100-500 nodes)

### Parallel Mode (Opt-in)
- **Best for:** Large graphs (2,000+ nodes), batch processing
- **Speedup:** Up to 1.5x faster for 6,400+ nodes
- **Trade-off:** Thread overhead makes it slower for small graphs

```rust
// Automatic selection based on graph size
let resolver = if graph.nodes.len() > 2000 {
    DataResolver::build_parallel(&graph, &provider)?
} else {
    DataResolver::build(&graph, &provider)?
};
```

See [Performance Documentation](docs/PARALLEL_PERFORMANCE.md) for detailed benchmarks.

### Benchmarks

Run the comprehensive benchmark suite:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench monster_graph
cargo bench parallel_scaling

# Run stress test
cargo run --example stress_test --release
```

**Sample Results** (80×80 grid, 6,400 nodes):
- Sequential: 28.47 ms
- Parallel: 19.26 ms
- **Speedup: 1.48x** ✨

---

## 🔧 Advanced Usage

### Custom Analysis Pass

```rust
pub trait AnalysisPass {
    fn analyze(
        &self,
        graph: &GraphDescription,
        metadata_provider: &dyn NodeMetadataProvider,
    ) -> Result<(), GraphyError>;
}
```

### AST Transformation

Graphy includes utilities for Rust AST manipulation:

```rust
use graphy::utils::ast_transform::*;

// Parse function source
let func = parse_function_source("fn add(a: i32, b: i32) -> i32 { a + b }")?;

// Transform and inline
let inlined = inline_function_as_expression(&func, &["x", "y"])?;
```

### Variable Name Generation

```rust
use graphy::utils::variable_gen::VariableNameGenerator;

let mut gen = VariableNameGenerator::new();
let var1 = gen.generate("result");  // "result_0"
let var2 = gen.generate("result");  // "result_1"
```

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

```
MIT License

Copyright (c) 2026 Tristan Poland (Trident_For_U)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files...
```

---

## 🙏 Acknowledgments

- Built with ❤️ by the Pulsar Team
- Inspired by visual programming paradigms in Unreal Engine Blueprints, Unity Visual Scripting, and Blender Geometry Nodes
- Powered by the Rust ecosystem: `syn`, `quote`, `serde`, and `thiserror`

---

<div align="center">

**[⬆ Back to Top](#-graphy)**

Made with 🦀 Rust

</div>
