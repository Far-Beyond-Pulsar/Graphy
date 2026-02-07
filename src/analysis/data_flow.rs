//! # Data Flow Analysis
//!
//! Resolves data dependencies and determines evaluation order.
//! 
//! Includes both single-threaded and parallel implementations for performance.

use crate::core::*;
use crate::GraphyError;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// Where an input value comes from
#[derive(Debug, Clone)]
pub enum DataSource {
    /// Connected to another node's output
    Connection {
        source_node_id: String,
        source_pin: String,
    },

    /// Constant value from node properties
    Constant(String),

    /// Use default value for this type
    Default,
}

/// Data flow resolver
pub struct DataResolver {
    /// Maps (node_id, input_pin) -> DataSource
    input_sources: HashMap<(String, String), DataSource>,

    /// Maps node_id -> unique variable name for its result
    result_variables: HashMap<String, String>,

    /// Topologically sorted list of pure nodes
    pure_evaluation_order: Vec<String>,
}

impl DataResolver {
    /// Build a data resolver from a graph (single-threaded)
    ///
    /// # Arguments
    ///
    /// * `graph` - The graph to analyze
    /// * `metadata_provider` - Provider for node metadata
    pub fn build<P: NodeMetadataProvider>(
        graph: &GraphDescription,
        metadata_provider: &P,
    ) -> Result<Self, GraphyError> {
        let mut resolver = DataResolver {
            input_sources: HashMap::new(),
            result_variables: HashMap::new(),
            pure_evaluation_order: Vec::new(),
        };

        // Phase 1: Map all data connections
        resolver.map_data_connections(graph)?;

        // Phase 2: Generate variable names for node results
        resolver.generate_variable_names(graph);

        // Phase 3: Determine evaluation order for pure nodes
        resolver.compute_pure_evaluation_order(graph, metadata_provider)?;

        Ok(resolver)
    }

    /// Build a data resolver from a graph (parallel version)
    ///
    /// Uses a dedicated thread pool for parallel processing of independent operations.
    /// Significantly faster for large graphs (2000+ nodes).
    ///
    /// Call `graphy::parallel::init_thread_pool()` at startup to eliminate cold-start overhead.
    ///
    /// # Arguments
    ///
    /// * `graph` - The graph to analyze
    /// * `metadata_provider` - Provider for node metadata
    pub fn build_parallel<P: NodeMetadataProvider + Sync>(
        graph: &GraphDescription,
        metadata_provider: &P,
    ) -> Result<Self, GraphyError> {
        let mut resolver = DataResolver {
            input_sources: HashMap::new(),
            result_variables: HashMap::new(),
            pure_evaluation_order: Vec::new(),
        };

        // Use the pre-warmed thread pool
        let pool = crate::parallel::get_thread_pool();
        
        pool.install(|| {
            // Phase 1: Map all data connections (parallel)
            resolver.map_data_connections_parallel(graph)?;

            // Phase 2: Generate variable names (parallel)
            resolver.generate_variable_names_parallel(graph);

            Ok::<(), GraphyError>(())
        })?;

        // Phase 3: Determine evaluation order for pure nodes (sequential)
        resolver.compute_pure_evaluation_order(graph, metadata_provider)?;

        Ok(resolver)
    }

    /// Map all data connections in the graph
    fn map_data_connections(&mut self, graph: &GraphDescription) -> Result<(), GraphyError> {
        for connection in &graph.connections {
            if matches!(connection.connection_type, ConnectionType::Data) {
                let key = (connection.target_node.clone(), connection.target_pin.clone());
                let source = DataSource::Connection {
                    source_node_id: connection.source_node.clone(),
                    source_pin: connection.source_pin.clone(),
                };

                self.input_sources.insert(key, source);
            }
        }

        // For inputs not connected, check properties or use defaults
        for (node_id, node) in &graph.nodes {
            for pin_instance in &node.inputs {
                let pin_name = &pin_instance.id;
                let key = (node_id.clone(), pin_name.clone());

                if !self.input_sources.contains_key(&key) {
                    // Check if there's a property value
                    if let Some(prop_value) = node.properties.get(pin_name) {
                        self.input_sources.insert(
                            key,
                            DataSource::Constant(property_value_to_string(prop_value)),
                        );
                    } else {
                        self.input_sources.insert(key, DataSource::Default);
                    }
                }
            }
        }

        Ok(())
    }

    /// Generate unique variable names for each node's result
    fn generate_variable_names(&mut self, graph: &GraphDescription) {
        for (node_id, _node) in &graph.nodes {
            let var_name = format!("node_{}_result", sanitize_var_name(node_id));
            self.result_variables.insert(node_id.clone(), var_name);
        }
    }

    /// Parallel version: Map data connections using rayon
    fn map_data_connections_parallel(&mut self, graph: &GraphDescription) -> Result<(), GraphyError> {
        // Process data connections in parallel
        let data_sources: Vec<_> = graph.connections
            .par_iter()
            .filter(|c| matches!(c.connection_type, ConnectionType::Data))
            .map(|connection| {
                let key = (connection.target_node.clone(), connection.target_pin.clone());
                let source = DataSource::Connection {
                    source_node_id: connection.source_node.clone(),
                    source_pin: connection.source_pin.clone(),
                };
                (key, source)
            })
            .collect();

        self.input_sources.extend(data_sources);

        // Process unconnected inputs in parallel
        let default_sources: Vec<_> = graph.nodes
            .par_iter()
            .flat_map(|(node_id, node)| {
                node.inputs
                    .par_iter()
                    .filter_map(|pin_instance| {
                        let pin_name = &pin_instance.id;
                        let key = (node_id.clone(), pin_name.clone());
                        
                        if !self.input_sources.contains_key(&key) {
                            if let Some(prop_value) = node.properties.get(pin_name) {
                                Some((key, DataSource::Constant(property_value_to_string(prop_value))))
                            } else {
                                Some((key, DataSource::Default))
                            }
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        self.input_sources.extend(default_sources);
        Ok(())
    }

    /// Parallel version: Generate variable names using rayon
    fn generate_variable_names_parallel(&mut self, graph: &GraphDescription) {
        let var_names: Vec<_> = graph.nodes
            .par_iter()
            .map(|(node_id, _node)| {
                let var_name = format!("node_{}_result", sanitize_var_name(node_id));
                (node_id.clone(), var_name)
            })
            .collect();

        self.result_variables.extend(var_names);
    }

    /// Compute evaluation order for pure nodes using topological sort
    fn compute_pure_evaluation_order<P: NodeMetadataProvider>(
        &mut self,
        graph: &GraphDescription,
        metadata_provider: &P,
    ) -> Result<(), GraphyError> {
        // Build dependency graph for pure nodes
        let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
        let mut pure_nodes: HashSet<String> = HashSet::new();

        // Identify pure nodes
        for (node_id, node) in &graph.nodes {
            if let Some(node_meta) = metadata_provider.get_node_metadata(&node.node_type) {
                if node_meta.node_type == NodeTypes::pure && node_meta.return_type.is_some() {
                    pure_nodes.insert(node_id.clone());
                    dependencies.insert(node_id.clone(), Vec::new());
                }
            }
        }

        // Build dependency edges
        for connection in &graph.connections {
            if matches!(connection.connection_type, ConnectionType::Data) {
                if pure_nodes.contains(&connection.target_node)
                    && pure_nodes.contains(&connection.source_node)
                {
                    dependencies
                        .entry(connection.target_node.clone())
                        .or_insert_with(Vec::new)
                        .push(connection.source_node.clone());
                }
            }
        }

        // Build reverse dependency map
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
        for (target, sources) in &dependencies {
            for source in sources {
                dependents
                    .entry(source.clone())
                    .or_insert_with(Vec::new)
                    .push(target.clone());
            }
        }

        // Topological sort using Kahn's algorithm
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for node_id in &pure_nodes {
            let num_deps = dependencies.get(node_id).map(|v| v.len()).unwrap_or(0);
            in_degree.insert(node_id.clone(), num_deps);
        }

        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        while let Some(node_id) = queue.pop_front() {
            self.pure_evaluation_order.push(node_id.clone());

            if let Some(dependent_nodes) = dependents.get(&node_id) {
                for dependent in dependent_nodes {
                    if let Some(degree) = in_degree.get_mut(dependent) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }
        }

        // Check for cycles
        if self.pure_evaluation_order.len() != pure_nodes.len() {
            return Err(GraphyError::CyclicDependency);
        }

        Ok(())
    }

    /// Get the source of data for a specific node input
    pub fn get_input_source(&self, node_id: &str, pin_name: &str) -> Option<&DataSource> {
        self.input_sources
            .get(&(node_id.to_string(), pin_name.to_string()))
    }

    /// Get the variable name for a node's result
    pub fn get_result_variable(&self, node_id: &str) -> Option<&String> {
        self.result_variables.get(node_id)
    }

    /// Get the evaluation order for pure nodes
    pub fn get_pure_evaluation_order(&self) -> &[String] {
        &self.pure_evaluation_order
    }
}

/// Convert a property value to a string representation
fn property_value_to_string(value: &PropertyValue) -> String {
    match value {
        PropertyValue::String(s) => format!("\"{}\"", s.escape_default()),
        PropertyValue::Number(n) => {
            // Format number appropriately
            if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                n.to_string()
            }
        }
        PropertyValue::Boolean(b) => b.to_string(),
        PropertyValue::Vector2(x, y) => format!("({}, {})", x, y),
        PropertyValue::Vector3(x, y, z) => format!("({}, {}, {})", x, y, z),
        PropertyValue::Color(r, g, b, a) => format!("({}, {}, {}, {})", r, g, b, a),
    }
}

/// Sanitize a string to be a valid variable name
fn sanitize_var_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestMetadataProvider {
        metadata: HashMap<String, NodeMetadata>,
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

    #[test]
    fn test_data_flow_with_constants() {
        let mut graph = GraphDescription::new("test");

        let mut node = NodeInstance::new("add_1", "add", Position::zero());
        node.add_input_pin("a", DataType::Typed("i64".into()));
        node.add_input_pin("b", DataType::Typed("i64".into()));
        node.set_property("a", PropertyValue::Number(5.0));
        node.set_property("b", PropertyValue::Number(3.0));
        graph.add_node(node);

        let provider = TestMetadataProvider {
            metadata: HashMap::new(),
        };

        let resolver = DataResolver::build(&graph, &provider).unwrap();

        let a_source = resolver.get_input_source("add_1", "a").unwrap();
        let b_source = resolver.get_input_source("add_1", "b").unwrap();

        assert!(matches!(a_source, DataSource::Constant(_)));
        assert!(matches!(b_source, DataSource::Constant(_)));
    }
}
