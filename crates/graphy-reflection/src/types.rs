//! `RuntimeTypeInfo` to Graphy type conversion.

use graphy::core::{EnumVariant, ReflectedType, TypeInfo, WrapperKind};
use graphy::DataType;
use pulsar_reflection::{RuntimeTypeInfo, TypeStructure, WrapperType};

/// The pin type for a reflected type. A wildcard becomes a wildcard pin.
pub fn data_type_of(info: &RuntimeTypeInfo) -> DataType {
    if info.is_wildcard() {
        return DataType::any();
    }
    DataType::Data(TypeInfo {
        type_string: info.type_name.to_string(),
        display_name: None,
        size_bytes: Some(info.size),
        align_bytes: Some(info.align),
    })
}

/// The structural Graphy type for a reflected type.
pub fn reflected_type_of(info: &RuntimeTypeInfo) -> ReflectedType {
    match &info.structure {
        TypeStructure::Primitive | TypeStructure::String => {
            ReflectedType::parse_str(info.type_name)
        }
        TypeStructure::Struct { .. } => ReflectedType::Struct {
            path: info.type_name.to_string(),
            generics: Vec::new(),
        },
        TypeStructure::Enum { variants } => ReflectedType::Enum {
            path: info.type_name.to_string(),
            variants: variants.iter().map(|v| EnumVariant::unit(*v)).collect(),
        },
        TypeStructure::Wildcard => ReflectedType::Wildcard,
        TypeStructure::Wrapper {
            wrapper_kind,
            inner,
        } => {
            let inner = Box::new(reflected_type_of(inner));
            match wrapper_kind {
                WrapperType::Array => ReflectedType::Array {
                    element: inner,
                    len: None,
                },
                WrapperType::Vec => wrap(WrapperKind::Vec, inner),
                WrapperType::Box => wrap(WrapperKind::Box, inner),
                WrapperType::Arc => wrap(WrapperKind::Arc, inner),
                WrapperType::Rc => wrap(WrapperKind::Rc, inner),
                WrapperType::Option => wrap(WrapperKind::Option, inner),
                WrapperType::HashSet => wrap(WrapperKind::HashSet, inner),
                // Reflection records only the success/value type of these.
                WrapperType::Result => wrap(
                    WrapperKind::Result {
                        error: Box::new(ReflectedType::Wildcard),
                    },
                    inner,
                ),
                WrapperType::HashMap => wrap(
                    WrapperKind::HashMap {
                        key: Box::new(ReflectedType::Wildcard),
                    },
                    inner,
                ),
                WrapperType::Custom(path) => wrap(
                    WrapperKind::Other {
                        path: (*path).to_string(),
                    },
                    inner,
                ),
            }
        }
    }
}

fn wrap(kind: WrapperKind, inner: Box<ReflectedType>) -> ReflectedType {
    ReflectedType::Wrapper { kind, inner }
}
