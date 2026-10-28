//! Reflected methods, properties and conversions as Graphy node metadata.
//!
//! These builders only read reflection data; they never invoke anything. Type
//! names are taken from the runtime type registry when the type is registered
//! and from the Rust type name otherwise.

use graphy::core::ParamMeta;
use graphy::{NodeMetadata, NodeTypes, ParamInfo};
use pulsar_reflection::methods::{MethodInfo, TypeRef};
use pulsar_reflection::{ConversionInfo, PropertyMetadata};

use crate::source::TypeSource;
use crate::types::reflected_type_of;

/// Options for [`method_node`].
#[derive(Debug, Clone, Default)]
pub struct MethodNodeOptions {
    /// Include a leading `self` parameter of the owning type.
    pub with_receiver: bool,
    /// Category shown in the palette; falls back to the method's `category`
    /// attribute, then the owning type's name.
    pub category: Option<String>,
}

fn type_name(source: &dyn TypeSource, ty: &TypeRef) -> String {
    source
        .runtime_type_by_id(ty.type_id())
        .map(|i| i.type_name.to_string())
        .unwrap_or_else(|| ty.type_name().to_string())
}

/// Node metadata for a reflected method of `owner`.
///
/// A method flagged side-effect free becomes a `pure` node; any other is a
/// `fn_` node with execution pins. The node name is `Owner::method`.
pub fn method_node(
    source: &dyn TypeSource,
    owner: &str,
    method: &MethodInfo,
    options: &MethodNodeOptions,
) -> NodeMetadata {
    let node_type = if method.flags.side_effect_free {
        NodeTypes::pure
    } else {
        NodeTypes::fn_
    };
    let category = options
        .category
        .clone()
        .or_else(|| method.attr("category").map(str::to_string))
        .unwrap_or_else(|| owner.to_string());

    let mut params = Vec::new();
    if options.with_receiver {
        params.push(ParamInfo::new("self", owner));
    }
    for p in method.params {
        params.push(ParamInfo::new(p.name, type_name(source, &p.ty)));
    }

    let mut node = NodeMetadata::new(format!("{owner}::{}", method.name), node_type, category)
        .with_params(params);
    if let Some(ret) = &method.ret {
        node = node.with_return_type(type_name(source, ret));
    }
    if !method.doc.is_empty() {
        node = node.with_doc(method.doc);
    }
    node
}

/// A pure node reading a reflected property: `get Owner::prop(self) -> T`.
pub fn property_getter_node(owner: &str, property: &PropertyMetadata) -> NodeMetadata {
    NodeMetadata::new(
        format!("{owner}::get_{}", property.name),
        NodeTypes::pure,
        property.category.unwrap_or(owner),
    )
    .with_params(vec![ParamInfo::new("self", owner)])
    .with_return_type(property.type_info.type_name)
}

/// A node writing a reflected property: `set Owner::prop(self, value)`.
pub fn property_setter_node(owner: &str, property: &PropertyMetadata) -> NodeMetadata {
    NodeMetadata::new(
        format!("{owner}::set_{}", property.name),
        NodeTypes::fn_,
        property.category.unwrap_or(owner),
    )
    .with_param_metas(vec![
        ParamMeta::new("self", graphy::core::ReflectedType::parse_str(owner)),
        ParamMeta::new("value", reflected_type_of(property.type_info)),
    ])
    .with_params(vec![
        ParamInfo::new("self", owner),
        ParamInfo::new("value", property.type_info.type_name),
    ])
}

/// A pure conversion node for a registered reflection conversion edge. The
/// node declares the conversion so the compiler and editors can insert it.
pub fn conversion_node(conversion: &ConversionInfo) -> NodeMetadata {
    NodeMetadata::new(conversion.id, NodeTypes::pure, "Conversion")
        .with_params(vec![ParamInfo::new("value", conversion.source_type_name)])
        .with_return_type(conversion.target_type_name)
        .with_conversion(
            conversion.source_type_name,
            conversion.target_type_name,
            conversion.label == "INTO",
        )
}
