//! Boundary between an authored graph file and the document a canvas edits.
//!
//! Editors keep one in-memory [`GraphDescription`] (the one the Graphy canvas
//! owns). The authored schema (`graphy_blueprint::GraphDescription`, shared by
//! `.blueprint` and `.animgraph`) is read once on load and rebuilt once on
//! save; nothing else converts between the two. Anything an authored graph
//! carries that the canvas document has no field for (a Blueprint's custom
//! event definitions, say) stays with the editor that owns it and is written
//! back unchanged.
//!
//! Identifiers survive the round trip: node ids and keys, pin ids, connection
//! ids, comment ids and zip ties. The pin back-links (`connected_to`) are
//! derived data, rebuilt here from the connection ids in connection order.

use graphy_blueprint as authored;

use crate::core::GraphComment;
use crate::{Connection, ConnectionType, DataType, GraphDescription, GraphZipTie, NodeInstance, Pin, PinInstance, PinType, Position, ZipTieEdge};

fn datatype_in(dt: &authored::DataType) -> DataType {
    match dt {
        authored::DataType::Execution => DataType::Exec,
        authored::DataType::Data(info) => DataType::typed(info.to_string()),
    }
}

fn datatype_out(dt: &DataType) -> authored::DataType {
    match dt {
        DataType::Exec => authored::DataType::Execution,
        DataType::Data(info) => authored::DataType::from_type_str(&info.type_string),
    }
}

/// One authored node as the editing document holds it (a palette creates nodes
/// in the authored form, so a canvas profile converts them with this).
pub fn node_from_authored(node: &authored::NodeInstance) -> NodeInstance {
    let mut out = NodeInstance::new(&node.id, &node.node_type, Position::new(f64::from(node.position.x), f64::from(node.position.y)));
    out.properties = node.properties.clone();
    let pin = |p: &authored::PinInstance, kind: PinType| PinInstance {
        id: p.id.clone(),
        pin: Pin { id: p.id.clone(), name: p.pin.name.clone(), data_type: datatype_in(&p.pin.data_type), pin_type: kind },
    };
    out.inputs = node.inputs.iter().map(|p| pin(p, PinType::Input)).collect();
    out.outputs = node.outputs.iter().map(|p| pin(p, PinType::Output)).collect();
    out
}

/// The editing document for an authored graph.
pub fn from_authored(graph: &authored::GraphDescription) -> GraphDescription {
    let mut doc = GraphDescription::new(&graph.metadata.name);
    doc.metadata.description = graph.metadata.description.clone();
    doc.metadata.version = graph.metadata.version.clone();
    doc.metadata.created_at = graph.metadata.created_at.clone();
    doc.metadata.modified_at = graph.metadata.modified_at.clone();

    for (key, node) in &graph.nodes {
        doc.nodes.insert(key.clone(), node_from_authored(node));
    }

    doc.connections = graph
        .connections
        .iter()
        .map(|c| {
            let mut out = Connection::new(
                &c.source_node,
                &c.source_pin,
                &c.target_node,
                &c.target_pin,
                match c.connection_type {
                    authored::ConnectionType::Execution => ConnectionType::Execution,
                    authored::ConnectionType::Data => ConnectionType::Data,
                },
            );
            out.id = Some(c.id.clone());
            out
        })
        .collect();

    doc.comments = graph
        .comments
        .iter()
        .map(|c| {
            let mut out = GraphComment::new(&c.text, Position::new(f64::from(c.position.0), f64::from(c.position.1)), (f64::from(c.size.0), f64::from(c.size.1)));
            out.id = c.id.clone();
            out.color = c.color;
            out.contained_node_ids = c.contained_node_ids.clone();
            out
        })
        .collect();

    doc.zip_ties = graph
        .zip_ties
        .iter()
        .map(|tie| GraphZipTie {
            position: Position::new(f64::from(tie.position.0), f64::from(tie.position.1)),
            straight_length: f64::from(tie.straight_length),
            wires: tie
                .wires
                .iter()
                .map(|edge| ZipTieEdge {
                    source_node: edge.source_node.clone(),
                    source_pin: edge.source_pin.clone(),
                    target_node: edge.target_node.clone(),
                    target_pin: edge.target_pin.clone(),
                })
                .collect(),
        })
        .collect();
    doc
}

/// The id a connection is saved under: its authored id, or a deterministic one
/// derived from its endpoints for edges made in this session (so saving twice
/// does not rename them).
pub fn connection_id(c: &Connection) -> String {
    c.id.clone().unwrap_or_else(|| format!("{}.{}->{}.{}", c.source_node, c.source_pin, c.target_node, c.target_pin))
}

/// The authored graph for saving or compiling. Fields the canvas document does
/// not model are left at their defaults for the caller to fill in.
pub fn to_authored(doc: &GraphDescription) -> authored::GraphDescription {
    let mut out = authored::GraphDescription::new(&doc.metadata.name);
    out.metadata.description = doc.metadata.description.clone();
    out.metadata.version = doc.metadata.version.clone();
    out.metadata.created_at = doc.metadata.created_at.clone();
    out.metadata.modified_at = doc.metadata.modified_at.clone();

    for (key, node) in &doc.nodes {
        let pin = |p: &PinInstance, kind: authored::PinType| authored::PinInstance {
            id: p.id.clone(),
            pin: authored::Pin { name: p.pin.name.clone(), pin_type: kind, data_type: datatype_out(&p.pin.data_type), connected_to: Vec::new() },
        };
        out.nodes.insert(
            key.clone(),
            authored::NodeInstance {
                id: node.id.clone(),
                node_type: node.node_type.clone(),
                position: authored::Position { x: node.position.x as f32, y: node.position.y as f32 },
                properties: node.properties.clone(),
                inputs: node.inputs.iter().map(|p| pin(p, authored::PinType::Input)).collect(),
                outputs: node.outputs.iter().map(|p| pin(p, authored::PinType::Output)).collect(),
            },
        );
    }

    for c in &doc.connections {
        let id = connection_id(c);
        if let Some(n) = out.nodes.get_mut(&c.source_node) {
            if let Some(p) = n.outputs.iter_mut().find(|p| p.id == c.source_pin) {
                p.pin.connected_to.push(id.clone());
            }
        }
        if let Some(n) = out.nodes.get_mut(&c.target_node) {
            if let Some(p) = n.inputs.iter_mut().find(|p| p.id == c.target_pin) {
                p.pin.connected_to.push(id.clone());
            }
        }
        out.connections.push(authored::Connection {
            id,
            source_node: c.source_node.clone(),
            source_pin: c.source_pin.clone(),
            target_node: c.target_node.clone(),
            target_pin: c.target_pin.clone(),
            connection_type: match c.connection_type {
                ConnectionType::Execution => authored::ConnectionType::Execution,
                ConnectionType::Data => authored::ConnectionType::Data,
            },
        });
    }

    out.comments = doc
        .comments
        .iter()
        .enumerate()
        .map(|(i, c)| authored::BlueprintComment {
            id: if c.id.is_empty() { format!("comment_{i}") } else { c.id.clone() },
            text: c.text.clone(),
            position: (c.position.x as f32, c.position.y as f32),
            size: (c.size.0 as f32, c.size.1 as f32),
            color: c.color,
            contained_node_ids: c.contained_node_ids.clone(),
        })
        .collect();

    out.zip_ties = doc
        .zip_ties
        .iter()
        .map(|tie| authored::BlueprintZipTie {
            position: (tie.position.x as f32, tie.position.y as f32),
            straight_length: tie.straight_length as f32,
            wires: tie
                .wires
                .iter()
                .map(|edge| authored::BlueprintZipTieEdge {
                    source_node: edge.source_node.clone(),
                    source_pin: edge.source_pin.clone(),
                    target_node: edge.target_node.clone(),
                    target_pin: edge.target_pin.clone(),
                })
                .collect(),
        })
        .collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(id: &str, name: &str, kind: authored::PinType, data: authored::DataType, wired: &[&str]) -> authored::PinInstance {
        authored::PinInstance { id: id.into(), pin: authored::Pin { name: name.into(), pin_type: kind, data_type: data, connected_to: wired.iter().map(|s| s.to_string()).collect() } }
    }

    fn sample() -> authored::GraphDescription {
        let mut graph = authored::GraphDescription::new("Sample");
        graph.metadata.description = "round trip".into();
        let node = |id: &str, ty: &str, inputs: Vec<authored::PinInstance>, outputs: Vec<authored::PinInstance>| authored::NodeInstance {
            id: id.into(),
            node_type: ty.into(),
            position: authored::Position { x: 12.5, y: -40.0 },
            properties: [("note".to_string(), serde_json::json!("keep"))].into_iter().collect(),
            inputs,
            outputs,
        };
        graph.nodes.insert(
            "a".into(),
            node("a", "t.source", vec![], vec![pin("out", "out", authored::PinType::Output, authored::DataType::from_type_str("f32"), &["c1", "c2"])]),
        );
        graph.nodes.insert(
            "b".into(),
            node("b", "t.sink", vec![pin("in", "in", authored::PinType::Input, authored::DataType::from_type_str("f32"), &["c1"])], vec![]),
        );
        graph.nodes.insert(
            "c".into(),
            node("c", "t.sink", vec![pin("in", "in", authored::PinType::Input, authored::DataType::from_type_str("f32"), &["c2"])], vec![]),
        );
        for (id, to) in [("c1", "b"), ("c2", "c")] {
            graph.connections.push(authored::Connection {
                id: id.into(),
                source_node: "a".into(),
                source_pin: "out".into(),
                target_node: to.into(),
                target_pin: "in".into(),
                connection_type: authored::ConnectionType::Data,
            });
        }
        graph.comments.push(authored::BlueprintComment { id: "k1".into(), text: "Group".into(), position: (1.0, 2.0), size: (300.0, 120.0), color: [0.1, 0.2, 0.3, 1.0], contained_node_ids: vec!["a".into(), "b".into()] });
        graph.zip_ties.push(authored::BlueprintZipTie {
            position: (50.0, 60.0),
            straight_length: 33.0,
            wires: vec![authored::BlueprintZipTieEdge { source_node: "a".into(), source_pin: "out".into(), target_node: "b".into(), target_pin: "in".into() }],
        });
        graph
    }

    #[test]
    fn ids_properties_comments_zip_ties_and_back_links_survive_a_round_trip() {
        let original = sample();
        let doc = from_authored(&original);
        assert_eq!(doc.nodes.len(), 3);
        assert_eq!(doc.connections[0].id.as_deref(), Some("c1"));
        assert_eq!(doc.zip_ties[0].straight_length, 33.0);
        let back = to_authored(&doc);
        let sorted = |g: &authored::GraphDescription| { let mut k: Vec<_> = g.nodes.keys().cloned().collect(); k.sort(); k };
        assert_eq!(sorted(&back), sorted(&original));
        for (key, node) in &original.nodes {
            let got = &back.nodes[key];
            assert_eq!((&got.id, &got.node_type, got.position.x, got.position.y), (&node.id, &node.node_type, node.position.x, node.position.y));
            assert_eq!(got.properties, node.properties);
            let ids = |pins: &[authored::PinInstance]| pins.iter().map(|p| (p.id.clone(), p.pin.name.clone(), p.pin.connected_to.clone())).collect::<Vec<_>>();
            assert_eq!(ids(&got.inputs), ids(&node.inputs), "{key}: input pins and back-links");
            assert_eq!(ids(&got.outputs), ids(&node.outputs), "{key}: output pins and back-links");
        }
        assert_eq!(back.connections.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["c1", "c2"]);
        assert_eq!(back.comments[0].id, "k1");
        assert_eq!(back.comments[0].contained_node_ids, vec!["a", "b"]);
        assert_eq!(back.zip_ties[0].straight_length, 33.0);
        assert_eq!(back.zip_ties[0].wires.len(), 1);
        assert_eq!(back.metadata.description, "round trip");
        // saving twice changes nothing
        let again = to_authored(&from_authored(&back));
        assert_eq!(serde_json::to_value(&again.connections).unwrap(), serde_json::to_value(&back.connections).unwrap());
    }

    #[test]
    fn a_connection_made_in_the_editor_gets_a_stable_derived_id() {
        let mut doc = from_authored(&sample());
        doc.connections.push(Connection::new("a", "out", "b", "in", ConnectionType::Data));
        let first = to_authored(&doc);
        let second = to_authored(&doc);
        let derived = &first.connections[2].id;
        assert_eq!(derived, "a.out->b.in");
        assert_eq!(derived, &second.connections[2].id);
        assert!(first.nodes["b"].inputs[0].pin.connected_to.contains(derived));
    }

    #[test]
    fn a_comment_without_an_id_is_given_a_positional_one() {
        let mut doc = from_authored(&sample());
        doc.comments[0].id.clear();
        assert_eq!(to_authored(&doc).comments[0].id, "comment_0");
    }
}
