//! The profile / extension API: what a domain (Blueprint, Shader, Render
//! Graph, animation) supplies to the shared canvas. It succeeds the Blueprint
//! plugin's `EditorProfile`.
//!
//! A profile is passed **explicitly** to the controller and canvas (an
//! `Arc<dyn GraphProfile>` owned by the editor that opened the document). There
//! is no registry, no process-global state and no `TypeId` comparison, so a
//! profile defined in one plugin DLL is used by the host and by Graphy without
//! sharing statics across DLL boundaries. Hosts that want to look a profile up
//! by file extension keep their own table.
//!
//! Everything has a working default; a minimal profile only names itself and
//! supplies a [`Presenter`].

use graphy::{
    can_connect_replacing, Connection, ConnectionType, DataType, GraphDescription, NodeInstance,
    PinInstance, PinRef, Position, StructuralResolver, TypeResolver,
};
use graphy_render::{DrawData, Presenter, Scene, Vec2};

/// One addable node type, for the node picker.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CatalogEntry {
    /// Domain-defined id passed back to [`GraphProfile::create_node`].
    pub definition: String,
    pub title: String,
    pub category: String,
    pub keywords: Vec<String>,
    /// One line shown under the title.
    pub description: String,
    /// Glyph shown in the entry's badge (may be empty).
    pub icon: String,
    /// Accent of the entry and its category (display-space RGBA); the picker
    /// falls back to the theme's focus colour.
    pub color: Option<graphy_render::Color>,
    /// Types of the entry's input pins, for filtering by a dragged wire. `None`
    /// asks the profile to build a probe node with [`GraphProfile::create_node`].
    pub inputs: Option<Vec<DataType>>,
}

/// How a catalogue entry can take a dragged wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragMatch {
    /// An input that takes the type directly.
    Exact,
    /// Only a wildcard input accepts it; listed after the exact ones.
    Wildcard,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem {
    /// `graphy.*` ids are handled by the shared controller; any other id is
    /// passed to [`GraphProfile::run_menu_item`].
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub separator_before: bool,
    pub children: Vec<MenuItem>,
}

impl MenuItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            enabled: true,
            separator_before: false,
            children: Vec::new(),
        }
    }

    pub fn separated(mut self) -> Self {
        self.separator_before = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// What a context menu was requested on.
#[derive(Clone, Debug)]
pub enum MenuTarget {
    Canvas { graph_pos: Vec2 },
    Node(String),
    Pin(PinRef),
    Wire(Connection),
    Comment(usize),
    Selection,
}

/// New pin lists for a node whose pins depend on its properties.
#[derive(Clone, Debug)]
pub struct PinUpdate {
    pub inputs: Vec<PinInstance>,
    pub outputs: Vec<PinInstance>,
}

/// A node that opens another graph when activated (Blueprint macro or
/// collapsed subgraph, a Shader function).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacroTarget {
    pub graph_id: String,
}

pub trait GraphProfile {
    /// Stable id of this profile.
    fn id(&self) -> &str;

    /// Presentation: node/pin/wire/comment looks, row heights, inline pin
    /// widgets, custom body rows and sections.
    fn presenter(&self) -> &dyn Presenter;

    // ----------------------------------------------------------- connecting

    /// Pin type judgement; chain the domain resolver over the structural one
    /// (or the reflection adapter) here.
    fn type_resolver(&self) -> &dyn TypeResolver {
        &StructuralResolver
    }

    /// Whether `from` (an output) may connect to `to` (an input). The default
    /// asks the edit contract (pin existence, kinds, no self loops) and the
    /// type resolver. An occupied input is allowed: connecting replaces it.
    fn can_connect(
        &self,
        graph: &GraphDescription,
        from: &PinRef,
        to: &PinRef,
    ) -> Result<(), String> {
        default_can_connect(self.type_resolver(), graph, from, to)
    }

    // ------------------------------------------------------ nodes and pins

    fn catalog(&self) -> Vec<CatalogEntry> {
        Vec::new()
    }

    /// Whether (and how) a catalogue entry takes a wire dragged from an output of
    /// type `from`. The default asks the type resolver about the entry's input
    /// pins (`entry.inputs`, or those of a probe node from [`Self::create_node`]).
    fn drag_match(&self, entry: &CatalogEntry, from: &DataType) -> Option<DragMatch> {
        let probe;
        let inputs: Vec<&DataType> = match &entry.inputs {
            Some(inputs) => inputs.iter().collect(),
            None => {
                probe = self.create_node(&entry.definition, "probe", Position::new(0.0, 0.0))?;
                probe.inputs.iter().map(|p| p.data_type()).collect()
            }
        };
        let resolver = self.type_resolver();
        let mut best = None;
        for to in inputs {
            match resolver.compatibility(from, to) {
                graphy::Compatibility::Exact | graphy::Compatibility::Convertible { .. } => {
                    return Some(DragMatch::Exact)
                }
                graphy::Compatibility::Wildcard => best = Some(DragMatch::Wildcard),
                _ => {}
            }
        }
        best
    }

    /// Builds a node of a catalogue type at `pos` with the given id.
    fn create_node(&self, _definition: &str, _id: &str, _pos: Position) -> Option<NodeInstance> {
        None
    }

    /// The node dropped onto a wire (or made by a reroute gesture). Pin ids
    /// are `in` and `out`.
    fn create_reroute(&self, id: &str, pos: Position, data_type: &DataType) -> NodeInstance {
        let mut n = NodeInstance::new(id, "reroute", pos);
        n.add_input_pin("in", data_type.clone());
        n.add_output_pin("out", data_type.clone());
        n
    }

    /// Dynamic pins: the pins `node` should have now, or `None` to keep them.
    /// Called after the node's properties change; the controller turns a
    /// difference into a `SetPins` command so wires on removed pins go with it.
    fn refresh_pins(&self, _graph: &GraphDescription, _node: &str) -> Option<PinUpdate> {
        None
    }

    /// Commands that bring `node`'s pins in line with its properties. The
    /// default turns a [`refresh_pins`](Self::refresh_pins) difference into one
    /// `SetPins`; override it when the pins' *ids* change with the properties
    /// and the wires on them must follow rather than be dropped.
    fn pin_commands(&self, graph: &GraphDescription, node: &str) -> Vec<graphy::GraphCommand> {
        set_pins_commands(self.refresh_pins(graph, node), graph, node)
    }

    /// Whether `node` may be deleted (structural nodes such as a macro's
    /// entry and exit may not).
    fn is_deletable(&self, _node: &NodeInstance) -> bool {
        true
    }

    /// Whether activating `node` opens another graph (macro navigation).
    fn macro_target(&self, _node: &NodeInstance) -> Option<MacroTarget> {
        None
    }

    // ---------------------------------------------------------------- menus

    /// Domain items appended to the node/pin/wire/canvas menus.
    fn menu_items(&self, _graph: &GraphDescription, _target: &MenuTarget) -> Vec<MenuItem> {
        Vec::new()
    }

    /// Runs a domain menu item. Returns the actions to perform.
    fn run_menu_item(
        &self,
        _graph: &GraphDescription,
        _target: &MenuTarget,
        _item: &str,
    ) -> Vec<crate::Action> {
        Vec::new()
    }

    // ------------------------------------------------------------- overlays

    /// Adds profile overlays (badges, validation markers, execution trace,
    /// section chrome) to the frame after the shared layers are built.
    fn decorate(&self, _graph: &GraphDescription, _scene: &Scene, _data: &mut DrawData) {}
}

pub fn default_can_connect(
    resolver: &dyn TypeResolver,
    graph: &GraphDescription,
    from: &PinRef,
    to: &PinRef,
) -> Result<(), String> {
    if from.node == to.node {
        return Err("a node cannot connect to itself".into());
    }
    let src = graph.nodes.get(&from.node).ok_or("source node missing")?;
    let dst = graph.nodes.get(&to.node).ok_or("target node missing")?;
    let out = src
        .outputs
        .iter()
        .find(|p| p.id == from.pin)
        .ok_or("source pin missing")?;
    let inp = dst
        .inputs
        .iter()
        .find(|p| p.id == to.pin)
        .ok_or("target pin missing")?;
    let kind = if out.data_type().is_execution() {
        ConnectionType::Execution
    } else {
        ConnectionType::Data
    };
    let conn = Connection::new(&from.node, &from.pin, &to.node, &to.pin, kind);
    can_connect_replacing(graph, &conn).map_err(|e| e.to_string())?;
    if kind == ConnectionType::Data {
        let c = resolver.compatibility(out.data_type(), inp.data_type());
        match c {
            graphy::Compatibility::Incompatible(why) => return Err(why),
            graphy::Compatibility::Unknown => {
                return Err("these pin types cannot be connected".into())
            }
            _ => {}
        }
    }
    Ok(())
}

/// Commands that bring `node`'s pins in line with the profile (empty when the
/// profile has nothing to change).
pub fn dynamic_pin_commands(
    profile: &dyn GraphProfile,
    graph: &GraphDescription,
    node: &str,
) -> Vec<graphy::GraphCommand> {
    profile.pin_commands(graph, node)
}

fn set_pins_commands(
    update: Option<PinUpdate>,
    graph: &GraphDescription,
    node: &str,
) -> Vec<graphy::GraphCommand> {
    let Some(update) = update else {
        return Vec::new();
    };
    let Some(current) = graph.nodes.get(node) else {
        return Vec::new();
    };
    let same = |a: &[PinInstance], b: &[PinInstance]| {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(x, y)| x.id == y.id && x.data_type() == y.data_type())
    };
    if same(&current.inputs, &update.inputs) && same(&current.outputs, &update.outputs) {
        return Vec::new();
    }
    vec![graphy::GraphCommand::SetPins {
        node: node.to_string(),
        inputs: update.inputs,
        outputs: update.outputs,
    }]
}

/// Commands wiring `source` (an output pin) to the first compatible input of the
/// freshly added `node`. An execution output (or a reroute's) drives one wire, so
/// its existing wire is replaced.
pub fn connect_new_node_commands(
    profile: &dyn GraphProfile,
    graph: &GraphDescription,
    source: &PinRef,
    node: &NodeInstance,
) -> Vec<graphy::GraphCommand> {
    use graphy::GraphCommand;
    let Some(src_node) = graph.nodes.get(&source.node) else {
        return Vec::new();
    };
    let Some(src_pin) = src_node.outputs.iter().find(|p| p.id == source.pin) else {
        return Vec::new();
    };
    let resolver = profile.type_resolver();
    let Some(target) = node.inputs.iter().find(|p| {
        resolver
            .compatibility(src_pin.data_type(), p.data_type())
            .is_connectable()
    }) else {
        return Vec::new();
    };
    let exec = src_pin.data_type().is_execution();
    let mut commands = Vec::new();
    if exec || src_node.is_reroute() {
        commands.extend(
            graph
                .connections
                .iter()
                .filter(|c| c.source_node == source.node && c.source_pin == source.pin)
                .cloned()
                .map(GraphCommand::Disconnect),
        );
    }
    commands.push(GraphCommand::Connect(Connection::new(
        &source.node,
        &source.pin,
        &node.id,
        &target.id,
        if exec {
            ConnectionType::Execution
        } else {
            ConnectionType::Data
        },
    )));
    commands
}
