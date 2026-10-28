//! Deterministic sample graphs and a presenter for them.
//!
//! Used by the headless tests and by the example binaries that render the
//! "clean" and "crowded" review scenes. They exercise every presentation
//! feature: header outputs, icons, texture-preview rows, reroutes, comments,
//! active and disabled nodes, exec and data wires of several types.

use graphy::core::GraphComment;
use graphy::{Connection, DataType, GraphDescription, NodeInstance, Position};

use crate::geom::rgba;
use crate::layout::{ExtraRow, NodeShape, RowPosition};
use crate::presenter::{category_accent, prettify, NodePresentation, Presenter};

/// Slot id of the sample texture-preview row.
pub const PREVIEW_SLOT: u32 = 1;

/// Presents sample nodes: `node_type` is `category::name`; properties
/// `preview`, `active`, `disabled` and `pure` toggle features.
pub struct SamplePresenter;

impl Presenter for SamplePresenter {
    fn node(&self, node: &NodeInstance) -> NodePresentation {
        let (category, name) = node.node_type.split_once("::").unwrap_or(("misc", node.node_type.as_str()));
        if node.is_reroute() {
            let mut p = NodePresentation::card("", rgba(0x8b, 0x8b, 0x8b, 1.0));
            p.shape = NodeShape::Reroute;
            return p;
        }
        let flag = |k: &str| node.properties.contains_key(k);
        let mut p = NodePresentation::card(prettify(name), category_accent(category));
        p.icon = category.chars().next().map(|c| c.to_ascii_uppercase());
        p.active = flag("active");
        p.disabled = flag("disabled");
        if flag("pure") && !node.outputs.is_empty() {
            p.header_output = Some(node.outputs.len() - 1);
        }
        if flag("preview") {
            p.extra_rows.push(ExtraRow { position: RowPosition::Bottom, slot: PREVIEW_SLOT, height: 64.0, min_width: 150.0 });
        }
        p
    }
}

fn node(id: &str, ty: &str, x: f64, y: f64, ins: &[(&str, DataType)], outs: &[(&str, DataType)]) -> NodeInstance {
    let mut n = NodeInstance::new(id, ty, Position::new(x, y));
    for (name, t) in ins {
        n.add_named_input_pin(format!("in_{name}"), *name, t.clone());
    }
    for (name, t) in outs {
        n.add_named_output_pin(format!("out_{name}"), *name, t.clone());
    }
    n
}

fn f() -> DataType {
    DataType::typed("f32")
}
fn b() -> DataType {
    DataType::typed("bool")
}
fn v() -> DataType {
    DataType::typed("Vec3")
}
fn s() -> DataType {
    DataType::typed("String")
}

fn data(g: &mut GraphDescription, a: &str, ap: &str, c: &str, cp: &str) {
    g.add_connection(Connection::data(a, format!("out_{ap}"), c, format!("in_{cp}")));
}
fn exec(g: &mut GraphDescription, a: &str, ap: &str, c: &str, cp: &str) {
    g.add_connection(Connection::execution(a, format!("out_{ap}"), c, format!("in_{cp}")));
}

/// A tidy graph of about a dozen nodes.
pub fn clean_scene() -> GraphDescription {
    let mut g = GraphDescription::new("clean");
    let e = DataType::Exec;
    g.add_node(node("begin", "event::begin_play", 0.0, 40.0, &[], &[("Then", e.clone())]));
    g.add_node(node("branch", "flow::branch", 220.0, 40.0, &[("Exec", e.clone()), ("Condition", b())], &[("True", e.clone()), ("False", e.clone())]));
    g.add_node(node("print", "debug::print_string", 470.0, 0.0, &[("Exec", e.clone()), ("Message", s())], &[("Then", e.clone())]));
    g.add_node(node("gt", "math::greater", 0.0, 190.0, &[("A", f()), ("B", f())], &[("Result", b())]));
    g.nodes.get_mut("gt").unwrap().set_property("pure", true);
    g.add_node(node("speed", "math::get_speed", -230.0, 200.0, &[], &[("Value", f())]));
    g.nodes.get_mut("speed").unwrap().set_property("pure", true);
    g.add_node(node("fmt", "text::format", 220.0, 230.0, &[("Value", f())], &[("Text", s())]));
    g.add_node(node("tex", "shader::sample_texture", 470.0, 150.0, &[("UV", DataType::typed("Vec2")), ("Sampler", DataType::typed("Sampler"))], &[("Color", v()), ("Alpha", f())]));
    g.nodes.get_mut("tex").unwrap().set_property("preview", true);
    g.add_node(node("mix", "math::lerp_vec", 760.0, 150.0, &[("A", v()), ("B", v()), ("Alpha", f())], &[("Result", v())]));
    g.add_node(node("anim", "anim::play_montage", 760.0, -10.0, &[("Exec", e.clone()), ("Montage", DataType::typed("Montage"))], &[("Then", e.clone())]));
    g.nodes.get_mut("anim").unwrap().set_property("active", true);
    g.add_node(node("off", "debug::draw_line", 470.0, 330.0, &[("Exec", e.clone()), ("Color", v())], &[("Then", e.clone())]));
    g.nodes.get_mut("off").unwrap().set_property("disabled", true);
    g.add_node(node("rr", "reroute", 400.0, 285.0, &[("", s())], &[("", s())]));
    exec(&mut g, "begin", "Then", "branch", "Exec");
    exec(&mut g, "branch", "True", "print", "Exec");
    exec(&mut g, "print", "Then", "anim", "Exec");
    exec(&mut g, "branch", "False", "off", "Exec");
    data(&mut g, "speed", "Value", "gt", "A");
    data(&mut g, "speed", "Value", "fmt", "Value");
    data(&mut g, "gt", "Result", "branch", "Condition");
    data(&mut g, "fmt", "Text", "rr", "");
    data(&mut g, "rr", "", "print", "Message");
    data(&mut g, "tex", "Color", "mix", "A");
    data(&mut g, "tex", "Alpha", "mix", "Alpha");
    g.comments.push(GraphComment { text: "Branch on speed".into(), position: Position::new(-250.0, -50.0), size: (740.0, 440.0) });
    g
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

/// A dense grid of `cols * rows` mixed nodes with many cross links, reroutes,
/// preview nodes and comments. Deterministic for a given size.
pub fn crowded_scene(cols: usize, rows: usize) -> GraphDescription {
    let mut g = GraphDescription::new("crowded");
    let mut rng = Lcg(0x5eed);
    let cats = ["math", "flow", "shader", "anim", "text", "debug"];
    let types = [f(), v(), b(), s(), DataType::typed("Vec2"), DataType::typed("i32")];
    for r in 0..rows {
        for c in 0..cols {
            let id = format!("n{r}_{c}");
            let cat = cats[(rng.next() as usize) % cats.len()];
            let nin = 1 + (rng.next() % 5) as usize;
            let nout = 1 + (rng.next() % 4) as usize;
            let mut n = NodeInstance::new(&id, format!("{cat}::node_{}", rng.next() % 40), Position::new(c as f64 * 290.0, r as f64 * 230.0 + (rng.next() % 40) as f64));
            if c % 3 == 0 {
                n.add_named_input_pin("in_Exec", "Exec", DataType::Exec);
                n.add_named_output_pin("out_Then", "Then", DataType::Exec);
            }
            for k in 0..nin {
                n.add_named_input_pin(format!("in_p{k}"), format!("Input {k}"), types[(rng.next() as usize) % types.len()].clone());
            }
            for k in 0..nout {
                n.add_named_output_pin(format!("out_p{k}"), format!("Result Value {k}"), types[(rng.next() as usize) % types.len()].clone());
            }
            match rng.next() % 9 {
                0 => n.set_property("preview", true),
                1 => n.set_property("pure", true),
                2 => n.set_property("active", true),
                3 if r % 4 == 1 => n.set_property("disabled", true),
                _ => {}
            }
            g.add_node(n);
        }
    }
    // links: each node feeds one or two nodes in the next column(s), matching types
    for r in 0..rows {
        for c in 0..cols.saturating_sub(1) {
            let a = g.nodes[&format!("n{r}_{c}")].clone();
            for hop in 0..2 {
                let tr = ((r as u64 + rng.next() % 3) as usize).min(rows - 1);
                let tc = (c + 1 + hop).min(cols - 1);
                let Some(t) = g.nodes.get(&format!("n{tr}_{tc}")).cloned() else { continue };
                if t.id == a.id {
                    continue;
                }
                for out in &a.outputs {
                    let Some(inp) = t.inputs.iter().find(|i| i.data_type() == out.data_type()) else { continue };
                    let already = g.connections.iter().any(|x| x.target_node == t.id && x.target_pin == inp.id);
                    if already {
                        continue;
                    }
                    let conn = if out.pin.is_exec() {
                        Connection::execution(&a.id, &out.id, &t.id, &inp.id)
                    } else {
                        Connection::data(&a.id, &out.id, &t.id, &inp.id)
                    };
                    g.add_connection(conn);
                    break;
                }
            }
        }
    }
    for k in 0..(cols * rows / 12).max(1) {
        let (r, c) = ((rng.next() as usize) % rows, (rng.next() as usize) % cols);
        g.add_node(node(&format!("rr{k}"), "reroute", c as f64 * 290.0 + 240.0, r as f64 * 230.0 + 150.0, &[("", f())], &[("", f())]));
    }
    for k in 0..(cols / 3).max(1) {
        g.comments.push(GraphComment {
            text: format!("Section {}", k + 1),
            position: Position::new(k as f64 * 870.0 - 20.0, -50.0),
            size: (850.0, rows as f64 * 230.0 + 80.0),
        });
    }
    g
}
