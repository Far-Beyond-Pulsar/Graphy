//! Presentation hooks: how a domain wants its nodes, pins, wires and comments
//! to look. This is the headless half of the profile contract (the
//! interaction half lives in `graphy-gpui`); layout and draw-data generation
//! only need this trait.

use graphy::core::GraphComment;
use graphy::{Connection, ConnectionType, DataType, NodeInstance, PinInstance};

use crate::geom::{mix, rgba, Color};
use crate::layout::{ExtraRow, LayoutMetrics, NodeShape, PinSide, RowContent};
use crate::style::Theme;
use crate::wire::WireRouting;

/// Outline of a pin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinShape {
    Circle,
    /// Triangle pointing along the flow; used for execution pins.
    Arrow,
    Diamond,
    Square,
    Hexagon,
}

impl PinShape {
    /// Value the pin shader switches on.
    pub fn shader_kind(self) -> u32 {
        match self {
            PinShape::Circle => 0,
            PinShape::Arrow => 1,
            PinShape::Diamond => 2,
            PinShape::Square => 3,
            PinShape::Hexagon => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PinStyle {
    pub shape: PinShape,
    /// Second outline, for data that outlives the frame (arrays, containers).
    pub ringed: bool,
    pub color: Color,
}

/// Domain view of a node's look and structure.
#[derive(Clone, Debug, PartialEq)]
pub struct NodePresentation {
    pub title: String,
    pub subtitle: Option<String>,
    /// Category accent: a thin stripe and the icon tint, never a flood fill.
    pub accent: Color,
    /// A single glyph drawn in the header (domain icon).
    pub icon: Option<char>,
    pub shape: NodeShape,
    /// Index of the output drawn in the header (a pure node's return value).
    pub header_output: Option<usize>,
    /// Host rows (texture preview, inline widgets, custom body).
    pub extra_rows: Vec<ExtraRow>,
    pub min_width: f32,
    /// Greyed out and non-interactive in the graph's semantics.
    pub disabled: bool,
    /// Currently executing / receiving data; animates.
    pub active: bool,
}

impl NodePresentation {
    pub fn card(title: impl Into<String>, accent: Color) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            accent,
            icon: None,
            shape: NodeShape::Card,
            header_output: None,
            extra_rows: Vec::new(),
            min_width: 0.0,
            disabled: false,
            active: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WireStyle {
    pub color: Color,
    pub routing: WireRouting,
    /// Wire carries execution flow (drawn a little heavier).
    pub exec: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommentPresentation {
    pub accent: Color,
}

/// Domain hooks consulted when a [`crate::Scene`] is built or refreshed.
/// Every method has a working default.
pub trait Presenter {
    fn node(&self, node: &NodeInstance) -> NodePresentation {
        default_node(node)
    }

    fn pin(&self, _node: &NodeInstance, pin: &PinInstance, _side: PinSide) -> PinStyle {
        default_pin_style(&pin.pin.data_type)
    }

    fn pin_label(&self, _node: &NodeInstance, pin: &PinInstance, _side: PinSide) -> String {
        if pin.pin.name.is_empty() { pin.id.clone() } else { pin.pin.name.clone() }
    }

    /// Height of a pin row; override for variable-height rows.
    fn row_height(&self, _node: &NodeInstance, _row: &RowContent, metrics: &LayoutMetrics) -> f32 {
        metrics.row_h
    }

    fn wire(&self, conn: &Connection, from: &PinStyle) -> WireStyle {
        WireStyle {
            color: from.color,
            routing: WireRouting::Bezier,
            exec: conn.connection_type == ConnectionType::Execution,
        }
    }

    fn comment(&self, _index: usize, _comment: &GraphComment) -> CommentPresentation {
        CommentPresentation { accent: rgba(0xa3, 0xa3, 0xa3, 1.0) }
    }
}

/// The presenter used when a host supplies none.
#[derive(Default, Clone, Copy)]
pub struct DefaultPresenter;

impl Presenter for DefaultPresenter {}

fn hash_str(s: &str) -> u32 {
    // FNV-1a: stable across runs and platforms (unlike `DefaultHasher` seeds).
    let mut h = 0x811c9dc5u32;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

/// Curated category accents: distinct hues, equal perceived weight.
const ACCENTS: [(u8, u8, u8); 8] = [
    (0x60, 0xa5, 0xfa),
    (0x4a, 0xde, 0x80),
    (0xfb, 0xbf, 0x24),
    (0xc0, 0x84, 0xfc),
    (0xf4, 0x72, 0xb6),
    (0x2d, 0xd4, 0xbf),
    (0xfb, 0x92, 0x3c),
    (0xa3, 0xe6, 0x35),
];

pub fn category_accent(category: &str) -> Color {
    let (r, g, b) = ACCENTS[(hash_str(category) as usize) % ACCENTS.len()];
    rgba(r, g, b, 1.0)
}

/// `"math::add_float"` -> `"Add Float"`.
pub fn prettify(node_type: &str) -> String {
    let base = node_type.rsplit(|c| c == ':' || c == '.').next().unwrap_or(node_type);
    let mut out = String::with_capacity(base.len());
    let mut upper = true;
    for c in base.chars() {
        if c == '_' || c == '-' {
            out.push(' ');
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn default_node(node: &NodeInstance) -> NodePresentation {
    let category = node
        .node_type
        .split(|c| c == ':' || c == '.')
        .find(|s| !s.is_empty())
        .unwrap_or("node");
    if node.is_reroute() {
        let mut p = NodePresentation::card("", rgba(0x8b, 0x8b, 0x8b, 1.0));
        p.shape = NodeShape::Reroute;
        return p;
    }
    NodePresentation::card(prettify(&node.node_type), category_accent(category))
}

/// Type colours, keyed on the pin's type string; unknown types hash into the
/// accent palette so each distinct type is stable and distinguishable.
pub fn default_pin_style(data_type: &DataType) -> PinStyle {
    match data_type {
        DataType::Exec => PinStyle { shape: PinShape::Arrow, ringed: false, color: rgba(0xe4, 0xe4, 0xe7, 1.0) },
        DataType::Data(info) => {
            let t = info.type_string.as_str();
            let color = match t {
                "bool" => rgba(0x22, 0xc5, 0x5e, 1.0),
                "f32" | "f64" | "float" => rgba(0x3b, 0x82, 0xf6, 1.0),
                "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize" | "isize" => {
                    rgba(0x06, 0xb6, 0xd4, 1.0)
                }
                "String" | "&str" | "str" => rgba(0xe8, 0x79, 0xf9, 1.0),
                "?" => rgba(0xa3, 0xa3, 0xa3, 1.0),
                other => {
                    let (r, g, b) = ACCENTS[(hash_str(other) as usize) % ACCENTS.len()];
                    rgba(r, g, b, 1.0)
                }
            };
            let ringed = t.starts_with("Vec<") || t.starts_with('[');
            let shape = if t == "?" { PinShape::Diamond } else { PinShape::Circle };
            PinStyle { shape, ringed, color }
        }
    }
}

/// Fades a colour toward the theme's dim target.
pub fn dimmed(theme: &Theme, c: Color, amount: f32) -> Color {
    let mut out = mix(c, theme.dim_target, amount);
    out[3] = c[3];
    out
}
