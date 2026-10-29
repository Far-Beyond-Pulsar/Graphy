//! Loading and re-saving an authored asset must not change any identifier and
//! must reach a value-stable fixed point (node maps are HashMaps, so key order
//! in the text is not stable).

use graphy_blueprint::codec::deserialize_blueprint;
use std::collections::BTreeSet;

const FIXTURE: &str = include_str!("fixtures/graph_save.json");

fn ids(asset: &graphy_blueprint::BlueprintAsset) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (graph_id, graph) in std::iter::once(("main".to_string(), &asset.main_graph)).chain(
        asset
            .subgraphs
            .iter()
            .map(|s| (format!("sub:{}", s.id), &s.graph)),
    ) {
        for (key, node) in &graph.nodes {
            out.insert(format!("{graph_id}/node/{key}={}", node.id));
            for pin in node.inputs.iter().chain(node.outputs.iter()) {
                out.insert(format!("{graph_id}/pin/{key}.{}", pin.id));
            }
        }
        for c in &graph.connections {
            out.insert(format!(
                "{graph_id}/conn/{}.{}->{}.{}",
                c.source_node, c.source_pin, c.target_node, c.target_pin
            ));
        }
    }
    for v in &asset.variables {
        out.insert(format!("var/{}", v.id));
    }
    out
}

#[test]
fn resaving_keeps_every_id_and_reaches_a_value_fixed_point() {
    let first = deserialize_blueprint(FIXTURE).unwrap();
    let before = ids(&first);
    assert!(before.len() > 10, "fixture should exercise many ids");

    let saved_once = serde_json::to_value(&first).unwrap();
    let second = deserialize_blueprint(&saved_once.to_string()).unwrap();
    assert_eq!(ids(&second), before, "ids changed across save/load");

    let saved_twice = serde_json::to_value(&second).unwrap();
    assert_eq!(saved_once, saved_twice, "second save differs from first");
}

#[test]
fn rejects_unknown_versions_and_duplicate_variable_ids() {
    let mut value: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    value["format_version"] = 99.into();
    assert!(deserialize_blueprint(&value.to_string())
        .unwrap_err()
        .contains("Unsupported"));

    let mut value: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let first = value["variables"][0].clone();
    value["variables"].as_array_mut().unwrap().push(first);
    assert!(deserialize_blueprint(&value.to_string())
        .unwrap_err()
        .contains("duplicate id"));
}
