// `#[derive(Reflectable)]` names its type-info static after the type.
#![allow(non_upper_case_globals)]

use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;

use graphy::core::{ReflectedType, WrapperKind};
use graphy::{
    check_connection_types, ChainResolver, Compatibility, Connection, ConnectionType, DataType,
    GraphDescription, NodeInstance, NodeTypes, Pin, PinInstance, PinType, Position,
    ScopedResolver, Severity, StructuralResolver, TypeResolver,
};
use graphy_reflection::{
    conversion_node, data_type_of, method_node, reflected_type_of, AttachedRegistry, Conversion,
    MethodNodeOptions, ReflectionResolver, TypeSource,
};
use pulsar_reflection::{
    methods_for, reflect_methods, Reflectable, RuntimeTypeInfo, CONVERSION_REGISTRY,
};

#[derive(Clone, Debug, Default, PartialEq, Reflectable)]
pub struct GrHealth {
    pub value: f32,
    pub max: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Reflectable)]
pub struct GrOther {
    pub id: f32,
}

#[reflect_methods]
impl GrHealth {
    /// Fraction of max health.
    #[reflect_method(pure, category = "Combat")]
    pub fn fraction(&self) -> f32 {
        self.value / self.max
    }

    #[reflect_method]
    pub fn damage(&mut self, amount: f32) {
        self.value -= amount;
    }
}

fn ty(s: &str) -> DataType {
    DataType::typed(s)
}

fn resolver() -> ReflectionResolver {
    ReflectionResolver::default()
}

#[test]
fn runtime_types_map_to_graphy_types() {
    let f = <f32 as Reflectable>::type_info();
    assert_eq!(
        reflected_type_of(f),
        ReflectedType::parse_str("f32"),
    );
    match data_type_of(f) {
        DataType::Data(info) => {
            assert_eq!(info.type_string, "f32");
            assert_eq!(info.size_bytes, Some(4));
        }
        DataType::Exec => panic!("f32 is a data type"),
    }

    let s = <GrHealth as Reflectable>::type_info();
    assert!(matches!(
        reflected_type_of(s),
        ReflectedType::Struct { ref path, .. } if path == "GrHealth"
    ));

    assert_eq!(data_type_of(RuntimeTypeInfo::wildcard()), DataType::any());
    assert_eq!(reflected_type_of(RuntimeTypeInfo::wildcard()), ReflectedType::Wildcard);
}

#[test]
fn primitives_structs_and_references_resolve() {
    let r = resolver();
    assert_eq!(r.compatibility(&ty("f32"), &ty("f32")), Compatibility::Exact);
    assert_eq!(r.compatibility(&ty("GrHealth"), &ty("GrHealth")), Compatibility::Exact);
    // references are transparent
    assert_eq!(r.compatibility(&ty("&f32"), &ty("f32")), Compatibility::Exact);
    assert_eq!(r.compatibility(&ty("&mut GrHealth"), &ty("GrHealth")), Compatibility::Exact);
    // wildcard
    assert_eq!(r.compatibility(&ty("f32"), &DataType::any()), Compatibility::Wildcard);
    // known but unrelated
    assert!(matches!(
        r.compatibility(&ty("GrHealth"), &ty("GrOther")),
        Compatibility::Incompatible(_)
    ));
    assert!(matches!(
        r.compatibility(&ty("GrHealth"), &ty("f32")),
        Compatibility::Incompatible(_)
    ));
    // execution pins are not this resolver's business
    assert_eq!(r.compatibility(&DataType::Exec, &DataType::Exec), Compatibility::Unknown);
}

#[test]
fn conversions_insert_with_lossiness() {
    let r = resolver();
    let widen = r.compatibility(&ty("f32"), &ty("f64"));
    assert!(widen.is_connectable(), "f32 -> f64 should convert: {widen:?}");
    // float -> int is a fallible conversion edge
    assert_eq!(
        r.compatibility(&ty("f32"), &ty("i32")),
        Compatibility::Convertible { lossy: true }
    );
}

#[test]
fn wrappers_compare_structurally() {
    let r = resolver();
    assert_eq!(r.compatibility(&ty("Vec<f32>"), &ty("Vec<f32>")), Compatibility::Exact);
    assert!(r.compatibility(&ty("Vec<f32>"), &ty("Vec<f64>")).is_connectable());
    assert!(matches!(
        r.compatibility(&ty("Vec<f32>"), &ty("Vec<GrHealth>")),
        Compatibility::Incompatible(_)
    ));
    assert!(matches!(
        r.compatibility(&ty("Vec<f32>"), &ty("Option<f32>")),
        Compatibility::Incompatible(_)
    ));
    // a wildcard element matches any element
    assert_eq!(
        r.compatibility(&ty("Vec<f32>"), &ty("Vec<?>")),
        Compatibility::Wildcard
    );
    let _ = WrapperKind::Vec;
}

#[test]
fn unknown_and_unregistered_types_are_unknown_not_errors() {
    let r = resolver();
    for unregistered in ["NeverRegistered", "Vec<NeverRegistered>", "vec3<f32>"] {
        assert_eq!(
            r.compatibility(&ty(unregistered), &ty("f32")),
            Compatibility::Unknown,
            "{unregistered}"
        );
        assert_eq!(
            r.compatibility(&ty("GrHealth"), &ty(unregistered)),
            Compatibility::Unknown,
            "{unregistered}"
        );
    }
}

/// A source whose contents change between calls, standing in for a plugin
/// that registers a type after startup.
#[derive(Default)]
struct LiveSource {
    names: RefCell<HashMap<String, &'static RuntimeTypeInfo>>,
}

impl TypeSource for &LiveSource {
    fn runtime_type(&self, name: &str) -> Option<&'static RuntimeTypeInfo> {
        self.names.borrow().get(name).copied()
    }
    fn runtime_type_by_id(&self, id: TypeId) -> Option<&'static RuntimeTypeInfo> {
        self.names.borrow().values().find(|i| i.type_id == id).copied()
    }
    fn conversion(&self, _: TypeId, _: TypeId) -> Option<Conversion> {
        None
    }
}

#[test]
fn names_resolve_on_demand_and_nothing_is_cached() {
    let live = LiveSource::default();
    let r = ReflectionResolver::new(&live);
    assert_eq!(r.compatibility(&ty("GrHealth"), &ty("GrHealth")), Compatibility::Unknown);

    live.names
        .borrow_mut()
        .insert("GrHealth".into(), <GrHealth as Reflectable>::type_info());
    assert_eq!(r.compatibility(&ty("GrHealth"), &ty("GrHealth")), Compatibility::Exact);

    live.names.borrow_mut().clear();
    assert_eq!(r.compatibility(&ty("GrHealth"), &ty("GrHealth")), Compatibility::Unknown);
}

/// Stand-in for a WGSL domain resolver.
struct Wgsl;
impl TypeResolver for Wgsl {
    fn compatibility(&self, from: &DataType, to: &DataType) -> Compatibility {
        match (from, to) {
            (DataType::Data(a), DataType::Data(b)) if a.type_string == b.type_string => {
                Compatibility::Exact
            }
            (DataType::Data(a), DataType::Data(b)) => {
                Compatibility::Incompatible(format!("{} != {}", a.type_string, b.type_string))
            }
            _ => Compatibility::Unknown,
        }
    }
}

fn composed() -> ChainResolver {
    ChainResolver::new()
        .with(StructuralResolver)
        .with(ScopedResolver::new("wgsl", Wgsl))
        .with(ReflectionResolver::default())
}

#[test]
fn wgsl_and_reflected_types_stay_distinct_in_a_composed_resolver() {
    let c = composed();
    // within a domain
    assert_eq!(c.compatibility(&ty("wgsl::f32"), &ty("wgsl::f32")), Compatibility::Exact);
    assert!(matches!(
        c.compatibility(&ty("wgsl::vec3f"), &ty("wgsl::vec4f")),
        Compatibility::Incompatible(_)
    ));
    assert_eq!(c.compatibility(&ty("f32"), &ty("f64")), resolver().compatibility(&ty("f32"), &ty("f64")));
    // never across domains, even when the base names agree
    assert!(matches!(
        c.compatibility(&ty("wgsl::f32"), &ty("f32")),
        Compatibility::Incompatible(_)
    ));
    assert!(matches!(
        c.compatibility(&ty("GrHealth"), &ty("wgsl::f32")),
        Compatibility::Incompatible(_)
    ));
}

#[test]
fn composed_resolver_drives_graph_diagnostics() {
    let pin = |id: &str, t: &str, d: PinType| PinInstance::new(id, Pin::new(id, id, ty(t), d));
    let mut g = GraphDescription::new("t");
    let mut a = NodeInstance::new("a", "x", Position { x: 0.0, y: 0.0 });
    a.outputs = vec![pin("o", "GrHealth", PinType::Output), pin("w", "wgsl::f32", PinType::Output)];
    let mut b = NodeInstance::new("b", "x", Position { x: 0.0, y: 0.0 });
    b.inputs = vec![pin("i", "GrOther", PinType::Input), pin("j", "f32", PinType::Input)];
    g.add_node(a);
    g.add_node(b);
    g.add_connection(Connection::new("a", "o", "b", "i", ConnectionType::Data));
    g.add_connection(Connection::new("a", "w", "b", "j", ConnectionType::Data));
    let diags = check_connection_types(&g, &composed());
    assert_eq!(diags.len(), 2);
    assert!(diags.iter().all(|d| d.severity == Severity::Error));
}

#[test]
fn methods_become_node_metadata() {
    let methods = methods_for::<GrHealth>();
    let fraction = methods.iter().find(|m| m.info.name == "fraction").unwrap();
    let damage = methods.iter().find(|m| m.info.name == "damage").unwrap();
    let opts = MethodNodeOptions {
        with_receiver: true,
        category: None,
    };

    let n = method_node(&AttachedRegistry, "GrHealth", &fraction.info, &opts);
    assert_eq!(n.name, "GrHealth::fraction");
    assert_eq!(n.node_type, NodeTypes::pure);
    assert_eq!(n.category, "Combat");
    assert_eq!(n.params.len(), 1);
    assert_eq!(n.params[0].name, "self");
    assert_eq!(n.return_type.as_ref().unwrap().type_string, "f32");
    assert!(n.doc.as_deref().unwrap().contains("Fraction"));

    let n = method_node(&AttachedRegistry, "GrHealth", &damage.info, &MethodNodeOptions::default());
    assert_eq!(n.node_type, NodeTypes::fn_);
    assert_eq!(n.category, "GrHealth");
    assert_eq!(n.params.len(), 1);
    assert_eq!(n.params[0].param_type, "f32");
    assert!(n.return_type.is_none());
}

#[test]
fn conversion_edges_become_conversion_nodes() {
    let info = CONVERSION_REGISTRY
        .between_types::<f32, f64>()
        .expect("f32 -> f64 is registered");
    let n = conversion_node(info);
    assert_eq!(n.name, info.id);
    let c = n.conversion.as_ref().unwrap();
    assert_eq!(c.from_type.type_string, "f32");
    assert_eq!(c.to_type.type_string, "f64");
    assert_eq!(c.lossless, info.label == "INTO");
}
