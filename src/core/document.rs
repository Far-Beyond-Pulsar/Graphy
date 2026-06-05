//! # Blueprint Document Model (v2)
//!
//! The `BlueprintDocument` is the top-level container for a v2 blueprint. It
//! supersedes the flat `GraphDescription` from v1 by introducing a unified
//! multi-graph namespace: every graph in a blueprint (the root event graph,
//! local macros, and collapsed regions) lives in the same `graphs` map.
//!
//! ## Relationship to `GraphDescription`
//!
//! `GraphDescription` is **not** removed — it remains the serialized form used
//! by `NodeInstance.node_type` resolution and by external code that hasn't
//! migrated yet. `BlueprintDocument` wraps it in a richer container.
//!
//! Migration from a v1 `GraphDescription` is provided by
//! [`BlueprintDocument::from_graph_description`].
//!
//! ## Graph ID Conventions
//!
//! | `GraphId`                   | Meaning                                      |
//! |-----------------------------|----------------------------------------------|
//! | `"main"`                    | Root event graph (always present)            |
//! | `"macro:<uuid>"`            | User-defined local macro                     |
//! | `"collapsed:<uuid>"`        | Collapsed visual region (expands inline)     |
//!
//! These conventions align with [`NodeKind`] derivation in `node.rs`.

use super::{
    Connection, GraphDescription, GraphMetadata, NodeInstance, Position, PropertyValue,
    ReflectedType,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Current document format version.
pub const DOCUMENT_FORMAT_VERSION: u32 = 2;

// ─────────────────────────────────────────────────────────────────────────────
// GraphId
// ─────────────────────────────────────────────────────────────────────────────

/// Identifier for a graph within a `BlueprintDocument`.
///
/// Conventions:
/// - `"main"` — root event graph
/// - `"macro:<uuid>"` — local macro
/// - `"collapsed:<uuid>"` — collapsed visual region
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphId(pub String);

impl GraphId {
    pub fn main() -> Self { Self("main".to_string()) }

    pub fn macro_id(uuid: impl Into<String>) -> Self {
        Self(format!("macro:{}", uuid.into()))
    }

    pub fn collapsed(uuid: impl Into<String>) -> Self {
        Self(format!("collapsed:{}", uuid.into()))
    }

    pub fn as_str(&self) -> &str { &self.0 }

    pub fn is_main(&self) -> bool { self.0 == "main" }

    pub fn is_macro(&self) -> bool { self.0.starts_with("macro:") }

    pub fn is_collapsed(&self) -> bool { self.0.starts_with("collapsed:") }
}

impl std::fmt::Display for GraphId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for GraphId {
    fn from(s: &str) -> Self { Self(s.to_string()) }
}

impl From<String> for GraphId {
    fn from(s: String) -> Self { Self(s) }
}

// ─────────────────────────────────────────────────────────────────────────────
// GraphKind
// ─────────────────────────────────────────────────────────────────────────────

/// The semantic role of a graph within a `BlueprintDocument`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphKind {
    /// Root execution graph containing event entry-points.
    EventGraph,

    /// A named, reusable sub-routine with an explicit input/output interface.
    /// Compiled as an inline expansion (pure) or emitted function (impure).
    Macro,

    /// A purely visual grouping of nodes folded into a single box.
    /// Always expanded inline before compilation; carries no interface of its own.
    CollapsedRegion,
}

// ─────────────────────────────────────────────────────────────────────────────
// GraphInterface & InterfacePin
// ─────────────────────────────────────────────────────────────────────────────

/// The public interface of a `Macro` graph — its declared inputs and outputs.
///
/// The interface is reflected as a matching pair of `SubgraphEntry` /
/// `SubgraphExit` nodes placed inside the macro graph. Call-site connections
/// target these interface pins by ID.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphInterface {
    pub inputs:  Vec<InterfacePin>,
    pub outputs: Vec<InterfacePin>,
}

impl GraphInterface {
    pub fn new() -> Self { Self::default() }

    pub fn add_input(&mut self, pin: InterfacePin) { self.inputs.push(pin); }
    pub fn add_output(&mut self, pin: InterfacePin) { self.outputs.push(pin); }
}

/// A single pin on a graph's public interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfacePin {
    /// Stable ID used to wire call-site connections.
    pub id: String,

    /// Human-readable display name.
    pub name: String,

    /// Type of data or execution flow carried by this pin.
    pub ty: ReflectedType,

    /// Default value shown in the editor when the pin is left unconnected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<PropertyValue>,

    /// If `true`, callers may supply a per-instance value in the node property
    /// panel even when the pin is not explicitly connected.
    #[serde(default)]
    pub is_editable_at_call_site: bool,

    /// Optional UI category grouping within the node's pin list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

impl InterfacePin {
    pub fn new(id: impl Into<String>, name: impl Into<String>, ty: ReflectedType) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            ty,
            default_value: None,
            is_editable_at_call_site: false,
            category: None,
        }
    }

    /// Builder: set a default value.
    #[must_use]
    pub fn with_default(mut self, v: PropertyValue) -> Self {
        self.default_value = Some(v);
        self
    }

    /// Builder: allow per-instance editing at the call site.
    #[must_use]
    pub fn editable_at_call_site(mut self) -> Self {
        self.is_editable_at_call_site = true;
        self
    }

    /// Convenience: execution-flow interface pin.
    pub fn exec(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(id, name, ReflectedType::Wildcard)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Graph — a single graph within a document
// ─────────────────────────────────────────────────────────────────────────────

/// A visual comment annotation within a graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphComment {
    pub id: String,
    pub text: String,
    pub position: Position,
    /// (width, height)
    pub size: (f64, f64),
    /// Contained node IDs (updated by the editor on drag).
    #[serde(default)]
    pub contained_node_ids: Vec<String>,
}

/// A single graph (event graph, macro, or collapsed region) within a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub id: GraphId,
    pub kind: GraphKind,
    pub metadata: GraphMetadata,

    /// All nodes, keyed by their ID.
    pub nodes: HashMap<String, NodeInstance>,

    /// All connections.
    pub connections: Vec<Connection>,

    /// Public interface — `Some` for `Macro`, `None` for `EventGraph` and
    /// `CollapsedRegion` (collapsed region interfaces are auto-derived).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<GraphInterface>,

    /// Visual comment annotations.
    #[serde(default)]
    pub comments: Vec<GraphComment>,
}

impl Graph {
    /// Creates a new empty event graph.
    pub fn new_event_graph() -> Self {
        Self {
            id: GraphId::main(),
            kind: GraphKind::EventGraph,
            metadata: GraphMetadata::new("EventGraph"),
            nodes: HashMap::new(),
            connections: Vec::new(),
            interface: None,
            comments: Vec::new(),
        }
    }

    /// Creates a new empty macro graph with the given ID and interface.
    pub fn new_macro(id: GraphId, name: impl Into<String>, interface: GraphInterface) -> Self {
        Self {
            id,
            kind: GraphKind::Macro,
            metadata: GraphMetadata::new(name),
            nodes: HashMap::new(),
            connections: Vec::new(),
            interface: Some(interface),
            comments: Vec::new(),
        }
    }

    /// Creates a new collapsed-region graph.
    pub fn new_collapsed(id: GraphId, name: impl Into<String>) -> Self {
        Self {
            id,
            kind: GraphKind::CollapsedRegion,
            metadata: GraphMetadata::new(name),
            nodes: HashMap::new(),
            connections: Vec::new(),
            interface: None,
            comments: Vec::new(),
        }
    }

    // ── Node helpers ─────────────────────────────────────────────────────────

    pub fn add_node(&mut self, node: NodeInstance) {
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn get_node(&self, id: &str) -> Option<&NodeInstance> {
        self.nodes.get(id)
    }

    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut NodeInstance> {
        self.nodes.get_mut(id)
    }

    pub fn remove_node(&mut self, id: &str) -> Option<NodeInstance> {
        let node = self.nodes.remove(id);
        if node.is_some() {
            // Remove all connections that touch this node
            self.connections.retain(|c| c.source_node != id && c.target_node != id);
        }
        node
    }

    pub fn add_connection(&mut self, conn: Connection) {
        self.connections.push(conn);
    }

    /// Returns an iterator over all subgraph-call nodes in this graph.
    pub fn subgraph_calls(&self) -> impl Iterator<Item = &NodeInstance> {
        self.nodes.values().filter(|n| n.is_subgraph_call())
    }

    /// Converts this `Graph` to a legacy `GraphDescription` for use with
    /// passes that haven't migrated to the v2 document model.
    pub fn to_graph_description(&self) -> GraphDescription {
        use super::GraphComment as LegacyComment;

        let mut gd = GraphDescription::new(self.metadata.name.clone());
        gd.nodes = self.nodes.clone();
        gd.connections = self.connections.clone();
        gd.comments = self.comments.iter().map(|c| LegacyComment {
            text: c.text.clone(),
            position: c.position,
            size: c.size,
        }).collect();
        gd
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ClassVariable
// ─────────────────────────────────────────────────────────────────────────────

/// A class-level member variable declared on the blueprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassVariable {
    /// Unique identifier.
    pub id: String,

    /// Variable name — used as the Rust field/slot name in generated code.
    pub name: String,

    /// Rust type string. Use `ty()` for the structured form.
    pub type_string: String,

    /// Default initialisation value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<PropertyValue>,

    /// Optional description shown in the editor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl ClassVariable {
    pub fn new(id: impl Into<String>, name: impl Into<String>, type_string: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            type_string: type_string.into(),
            default_value: None,
            description: None,
        }
    }

    /// Returns the structured [`ReflectedType`] by parsing the type string.
    pub fn ty(&self) -> ReflectedType {
        ReflectedType::parse_str(&self.type_string)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BlueprintMetadata
// ─────────────────────────────────────────────────────────────────────────────

/// High-level metadata about a blueprint document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlueprintMetadata {
    /// The kind of blueprint (e.g. `"Actor"`, `"Component"`, `"FunctionLibrary"`).
    #[serde(default)]
    pub blueprint_type: String,

    /// Parent class this blueprint extends.
    #[serde(default)]
    pub parent_class: String,

    /// Human-readable description.
    #[serde(default)]
    pub description: String,

    /// UI category for the asset browser.
    #[serde(default)]
    pub category: String,

    /// Searchable tags.
    #[serde(default)]
    pub tags: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// EditorState — per-document editor viewport persistence
// ─────────────────────────────────────────────────────────────────────────────

/// Saved editor state for a single graph tab (viewport position and zoom).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphViewState {
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
}

impl Default for GraphViewState {
    fn default() -> Self {
        Self { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 }
    }
}

/// Saved editor state for the whole document (active tab, per-tab viewports).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentEditorState {
    /// ID of the currently active tab.
    #[serde(default)]
    pub active_graph: Option<String>,

    /// Per-graph viewport state, keyed by `GraphId.as_str()`.
    #[serde(default)]
    pub view_states: HashMap<String, GraphViewState>,
}

// ─────────────────────────────────────────────────────────────────────────────
// BlueprintDocument
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level container for a v2 blueprint.
///
/// A document holds all graphs (event graph + macros + collapsed regions),
/// class-level variables, and optional editor viewport state.
///
/// # Creating a document
///
/// ```
/// use graphy::core::{BlueprintDocument, GraphId};
///
/// let mut doc = BlueprintDocument::new("MyActor");
/// // doc.entry_graph is "main"; doc.graphs["main"] is an empty EventGraph.
/// ```
///
/// # Migrating from v1
///
/// ```ignore
/// let doc = BlueprintDocument::from_graph_description(graph_desc);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintDocument {
    /// Format version. Always [`DOCUMENT_FORMAT_VERSION`] for newly created
    /// documents; may be lower for migrated v1 content.
    pub format_version: u32,

    /// High-level blueprint metadata.
    #[serde(default)]
    pub metadata: BlueprintMetadata,

    /// The ID of the root execution graph. Typically `"main"`.
    pub entry_graph: GraphId,

    /// All graphs in this document, keyed by their `GraphId`.
    pub graphs: HashMap<String, Graph>,

    /// Class-level member variables.
    #[serde(default)]
    pub variables: Vec<ClassVariable>,

    /// Optional editor viewport state (not required for compilation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_state: Option<DocumentEditorState>,
}

impl BlueprintDocument {
    /// Creates a new document with an empty event graph.
    pub fn new(_name: impl Into<String>) -> Self {
        let mut graphs = HashMap::new();
        let root = Graph::new_event_graph();
        graphs.insert(root.id.as_str().to_string(), root);
        Self {
            format_version: DOCUMENT_FORMAT_VERSION,
            metadata: BlueprintMetadata::default(),
            entry_graph: GraphId::main(),
            graphs,
            variables: Vec::new(),
            editor_state: None,
        }
    }

    /// Returns a reference to the root event graph.
    ///
    /// # Panics
    ///
    /// Panics if the entry graph is missing (documents from [`Self::new`]
    /// always have a valid entry graph).
    pub fn entry_graph(&self) -> &Graph {
        self.graphs.get(self.entry_graph.as_str())
            .expect("BlueprintDocument: entry_graph ID points to a missing graph")
    }

    /// Returns a mutable reference to the root event graph.
    pub fn entry_graph_mut(&mut self) -> &mut Graph {
        let id = self.entry_graph.as_str().to_string();
        self.graphs.get_mut(&id)
            .expect("BlueprintDocument: entry_graph ID points to a missing graph")
    }

    /// Returns a reference to a graph by ID, or `None` if not present.
    pub fn get_graph(&self, id: &GraphId) -> Option<&Graph> {
        self.graphs.get(id.as_str())
    }

    /// Returns a mutable reference to a graph by ID.
    pub fn get_graph_mut(&mut self, id: &GraphId) -> Option<&mut Graph> {
        self.graphs.get_mut(id.as_str())
    }

    /// Inserts or replaces a graph.
    pub fn insert_graph(&mut self, graph: Graph) {
        self.graphs.insert(graph.id.as_str().to_string(), graph);
    }

    /// Removes a graph by ID.
    ///
    /// Returns `Err` if the graph is the entry graph (cannot remove root).
    pub fn remove_graph(&mut self, id: &GraphId) -> Result<Option<Graph>, String> {
        if id == &self.entry_graph {
            return Err(format!("Cannot remove entry graph '{}'", id));
        }
        Ok(self.graphs.remove(id.as_str()))
    }

    /// Returns an iterator over all macro graphs.
    pub fn macros(&self) -> impl Iterator<Item = &Graph> {
        self.graphs.values().filter(|g| g.kind == GraphKind::Macro)
    }

    /// Returns an iterator over all graph IDs and graphs.
    pub fn all_graphs(&self) -> impl Iterator<Item = (&str, &Graph)> {
        self.graphs.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Migrates a v1 flat `GraphDescription` into a v2 `BlueprintDocument`.
    ///
    /// The `GraphDescription` becomes the root event graph. There are no macros
    /// because v1 had no cross-graph namespace.
    pub fn from_graph_description(gd: GraphDescription) -> Self {
        let mut doc = Self::new(gd.metadata.name.clone());
        let root = doc.entry_graph_mut();
        root.nodes = gd.nodes;
        root.connections = gd.connections;
        root.comments = gd.comments.into_iter().enumerate().map(|(i, c)| GraphComment {
            id: format!("comment_{}", i),
            text: c.text,
            position: c.position,
            size: c.size,
            contained_node_ids: Vec::new(),
        }).collect();
        root.metadata = gd.metadata;
        doc
    }

    /// Extracts the root event graph as a legacy `GraphDescription`.
    ///
    /// Useful for passing to v1 compiler passes that haven't migrated.
    pub fn to_graph_description(&self) -> GraphDescription {
        self.entry_graph().to_graph_description()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Connection, NodeInstance, Position};

    #[test]
    fn new_document_has_main_graph() {
        let doc = BlueprintDocument::new("Test");
        assert_eq!(doc.entry_graph, GraphId::main());
        assert!(doc.graphs.contains_key("main"));
        assert_eq!(doc.entry_graph().kind, GraphKind::EventGraph);
    }

    #[test]
    fn graph_id_conventions() {
        assert!(GraphId::main().is_main());
        assert!(GraphId::macro_id("abc").is_macro());
        assert!(GraphId::collapsed("xyz").is_collapsed());
        assert_eq!(GraphId::macro_id("abc").as_str(), "macro:abc");
    }

    #[test]
    fn insert_and_retrieve_macro() {
        let mut doc = BlueprintDocument::new("Test");
        let macro_id = GraphId::macro_id("test_macro");
        let g = Graph::new_macro(macro_id.clone(), "TestMacro", GraphInterface::new());
        doc.insert_graph(g);
        assert!(doc.get_graph(&macro_id).is_some());
        assert_eq!(doc.macros().count(), 1);
    }

    #[test]
    fn cannot_remove_entry_graph() {
        let mut doc = BlueprintDocument::new("Test");
        let result = doc.remove_graph(&GraphId::main());
        assert!(result.is_err());
    }

    #[test]
    fn roundtrip_from_graph_description() {
        let mut gd = GraphDescription::new("MyGraph");
        gd.add_node(NodeInstance::new("n1", "math.add", Position::zero()));
        gd.add_connection(Connection::data("n1", "result", "n1", "a")); // self-loop for test

        let doc = BlueprintDocument::from_graph_description(gd.clone());
        let recovered = doc.to_graph_description();

        assert_eq!(recovered.nodes.len(), gd.nodes.len());
        assert_eq!(recovered.connections.len(), gd.connections.len());
    }
}
