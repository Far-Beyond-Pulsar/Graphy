//! Copy/paste of node selections as a JSON string, host-clipboard friendly.
//!
//! Only nodes and the connections *between* copied nodes travel; connections
//! to nodes outside the selection are dropped. Pasted nodes get fresh ids from
//! [`graphy::IdAllocator`] (deterministic for a given graph), and the paste is
//! returned as commands so it is one undoable edit.

use std::collections::{BTreeSet, HashMap};

use graphy::{Connection, GraphCommand, GraphDescription, IdAllocator, NodeInstance, Position};
use graphy_render::Vec2;
use serde::{Deserialize, Serialize};

const FORMAT: &str = "graphy/nodes@1";

#[derive(Serialize, Deserialize)]
struct Payload {
    format: String,
    nodes: Vec<NodeInstance>,
    connections: Vec<Connection>,
}

/// Serialises the selected nodes, or `None` when nothing is selected.
pub fn copy(graph: &GraphDescription, ids: &BTreeSet<String>) -> Option<String> {
    let mut nodes: Vec<NodeInstance> = ids
        .iter()
        .filter_map(|id| graph.nodes.get(id).cloned())
        .collect();
    if nodes.is_empty() {
        return None;
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let connections = graph
        .connections
        .iter()
        .filter(|c| ids.contains(&c.source_node) && ids.contains(&c.target_node))
        .cloned()
        .collect();
    serde_json::to_string(&Payload {
        format: FORMAT.into(),
        nodes,
        connections,
    })
    .ok()
}

pub struct Paste {
    pub commands: Vec<GraphCommand>,
    /// Ids of the new nodes, in paste order.
    pub new_ids: Vec<String>,
}

/// Builds the commands that paste `text` with its top-left corner at `at`.
pub fn paste(
    text: &str,
    graph: &GraphDescription,
    at: Vec2,
    id_prefix: &str,
) -> Result<Paste, String> {
    let payload: Payload =
        serde_json::from_str(text).map_err(|e| format!("not a graphy selection: {e}"))?;
    if payload.format != FORMAT {
        return Err(format!("unsupported clipboard format '{}'", payload.format));
    }
    if payload.nodes.is_empty() {
        return Err("clipboard holds no nodes".into());
    }
    let min = payload
        .nodes
        .iter()
        .fold((f64::MAX, f64::MAX), |(x, y), n| {
            (x.min(n.position.x), y.min(n.position.y))
        });
    let (dx, dy) = (at.x as f64 - min.0, at.y as f64 - min.1);
    let mut alloc = IdAllocator::for_graph(graph, id_prefix);
    let mut remap: HashMap<String, String> = HashMap::new();
    let mut commands = Vec::new();
    let mut new_ids = Vec::new();
    for n in &payload.nodes {
        let id = alloc.allocate().0;
        remap.insert(n.id.clone(), id.clone());
        let mut copy = n.clone();
        copy.id = id.clone();
        copy.position = Position {
            x: n.position.x + dx,
            y: n.position.y + dy,
        };
        commands.push(GraphCommand::AddNode(copy));
        new_ids.push(id);
    }
    for c in &payload.connections {
        if let (Some(a), Some(b)) = (remap.get(&c.source_node), remap.get(&c.target_node)) {
            commands.push(GraphCommand::Connect(Connection::new(
                a,
                &c.source_pin,
                b,
                &c.target_pin,
                c.connection_type,
            )));
        }
    }
    Ok(Paste { commands, new_ids })
}
