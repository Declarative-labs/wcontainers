//! The API's own description, read from Microsoft's Win32 metadata into owned values the generator walks.

use std::collections::BTreeMap;

use windows_metadata::reader::{HasAttributes, TypeCategory, TypeDef, TypeIndex};
use windows_metadata::{FieldAttributes, ParamAttributes, Type, TypeAttributes, Value};

/// A value written in the metadata: a constant, or an argument of an attribute.
#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Integer(i128),
    Float(f64),
    Text(String),
    Bool(bool),
}

pub fn literal_of(value: Value) -> Literal {
    match value {
        Value::Bool(value) => Literal::Bool(value),
        Value::U8(value) => Literal::Integer(value.into()),
        Value::I8(value) => Literal::Integer(value.into()),
        Value::U16(value) => Literal::Integer(value.into()),
        Value::I16(value) => Literal::Integer(value.into()),
        Value::U32(value) => Literal::Integer(value.into()),
        Value::I32(value) => Literal::Integer(value.into()),
        Value::U64(value) => Literal::Integer(value.into()),
        Value::I64(value) => Literal::Integer(value.into()),
        Value::F32(value) => Literal::Float(value.into()),
        Value::F64(value) => Literal::Float(value),
        Value::Utf8(value) | Value::Utf16(value) => Literal::Text(value),
        Value::AttributeEnum(_, value) => Literal::Integer(value.into()),
    }
}

/// One custom attribute: its name without the `Attribute` suffix, and its arguments in order.
#[derive(Clone, Debug)]
pub struct Attribute {
    pub name: String,
    pub arguments: Vec<(String, Literal)>,
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub is_in: bool,
    pub is_out: bool,
    pub is_optional: bool,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Debug)]
pub struct Method {
    pub name: String,
    pub return_type: Type,
    pub params: Vec<Param>,
    /// The library a function is exported from; none for an interface's method.
    pub library: Option<String>,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Debug)]
pub struct Interface {
    pub namespace: String,
    pub name: String,
    pub guid: Option<Guid>,
    pub base: Option<(String, String)>,
    pub methods: Vec<Method>,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub constant: Option<Literal>,
    pub attributes: Vec<Attribute>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordKind {
    Struct,
    Union,
    Enum,
    /// A C typedef of one value (`HRESULT`, `HWND`), which the metadata writes as a struct of one field.
    Typedef,
    Delegate,
}

#[derive(Clone, Debug)]
pub struct Record {
    pub namespace: String,
    pub name: String,
    pub kind: RecordKind,
    pub fields: Vec<Field>,
    pub nested: Vec<Record>,
    pub attributes: Vec<Attribute>,
    /// An enum's integer type (its `value__` field).
    pub underlying: Option<Type>,
}

/// Every interface, record, constant and function of the namespaces asked for.
#[derive(Debug, Default)]
pub struct Namespaces {
    pub interfaces: BTreeMap<(String, String), Interface>,
    pub records: BTreeMap<(String, String), Record>,
    pub constants: BTreeMap<(String, String), Field>,
    pub functions: BTreeMap<(String, String), Method>,
}

pub fn attribute_named<'a>(attributes: &'a [Attribute], name: &str) -> Option<&'a Attribute> {
    attributes.iter().find(|attribute| attribute.name == name)
}

fn attributes_of<'a>(row: &impl HasAttributes<'a>) -> Vec<Attribute> {
    row.attributes()
        .map(|attribute| {
            let full = attribute.ctor().parent().name().to_string();
            let name = full.strip_suffix("Attribute").unwrap_or(&full).to_string();
            Attribute { name, arguments: attribute.value().into_iter().map(|(name, value)| (name, literal_of(value))).collect() }
        })
        .collect()
}

fn guid_of(attributes: &[Attribute]) -> Option<Guid> {
    let guid = attribute_named(attributes, "Guid")?;
    let mut words = guid.arguments.iter().map(|(_, value)| value);
    let mut next = || match words.next() {
        Some(Literal::Integer(value)) => Some(*value),
        _ => None,
    };
    let data1 = u32::try_from(next()?).ok()?;
    let data2 = u16::try_from(next()?).ok()?;
    let data3 = u16::try_from(next()?).ok()?;
    let mut data4 = [0u8; 8];
    for byte in &mut data4 {
        *byte = u8::try_from(next()?).ok()?;
    }
    Some(Guid { data1, data2, data3, data4 })
}

fn method_of(def: windows_metadata::reader::MethodDef<'_>) -> Method {
    let signature = def.signature(&[]);
    let mut params: Vec<Param> = Vec::new();
    for param in def.params() {
        let Some(position) = usize::from(param.sequence()).checked_sub(1) else { continue };
        let flags = param.flags();
        params.push(Param {
            name: param.name().to_string(),
            ty: signature.types[position].clone(),
            is_in: flags.contains(ParamAttributes::In),
            is_out: flags.contains(ParamAttributes::Out),
            is_optional: flags.contains(ParamAttributes::Optional),
            attributes: attributes_of(&param),
        });
    }
    Method {
        name: def.name().to_string(),
        return_type: signature.return_type,
        params,
        library: def.impl_map().map(|map| map.import_scope().name().to_string()),
        attributes: attributes_of(&def),
    }
}

fn record_of(index: &TypeIndex, def: TypeDef<'_>, kind: RecordKind) -> Record {
    let underlying = def.fields().find(|field| field.flags().contains(FieldAttributes::SpecialName)).map(|field| field.ty());
    let fields = def
        .fields()
        .filter(|field| !field.flags().contains(FieldAttributes::SpecialName))
        .map(|field| Field {
            name: field.name().to_string(),
            ty: field.ty(),
            constant: field.constant().map(|constant| literal_of(constant.value())),
            attributes: attributes_of(&field),
        })
        .collect();
    let nested = index.nested(def).map(|inner| record_of(index, inner, kind_of(inner).unwrap_or(RecordKind::Struct))).collect();
    Record {
        namespace: def.namespace().to_string(),
        name: def.name().to_string(),
        kind,
        fields,
        nested,
        attributes: attributes_of(&def),
        underlying,
    }
}

fn kind_of(def: TypeDef<'_>) -> Option<RecordKind> {
    match def.category() {
        TypeCategory::Enum => Some(RecordKind::Enum),
        TypeCategory::Delegate => Some(RecordKind::Delegate),
        TypeCategory::Struct if def.has_attribute("NativeTypedefAttribute") => Some(RecordKind::Typedef),
        TypeCategory::Struct if def.flags().contains(TypeAttributes::ExplicitLayout) => Some(RecordKind::Union),
        TypeCategory::Struct => Some(RecordKind::Struct),
        _ => None,
    }
}

pub fn read(index: &TypeIndex, namespaces: &[&str]) -> Namespaces {
    let mut read = Namespaces::default();
    for def in index.types() {
        let namespace = def.namespace();
        if !namespaces.contains(&namespace) {
            continue;
        }
        let key = (namespace.to_string(), def.name().to_string());
        if def.name() == "Apis" {
            for field in def.fields() {
                read.constants.insert(
                    (namespace.to_string(), field.name().to_string()),
                    Field {
                        name: field.name().to_string(),
                        ty: field.ty(),
                        constant: field.constant().map(|constant| literal_of(constant.value())),
                        attributes: attributes_of(&field),
                    },
                );
            }
            for method in def.methods() {
                read.functions.insert((namespace.to_string(), method.name().to_string()), method_of(method));
            }
            continue;
        }
        if def.category() == TypeCategory::Interface {
            let attributes = attributes_of(&def);
            let base = def.interface_impls().next().and_then(|base| match base.interface(&[]) {
                Type::Name(name) => Some((name.namespace, name.name)),
                _ => None,
            });
            read.interfaces.insert(
                key,
                Interface {
                    namespace: namespace.to_string(),
                    name: def.name().to_string(),
                    guid: guid_of(&attributes),
                    base,
                    methods: def.methods().map(method_of).collect(),
                    attributes,
                },
            );
            continue;
        }
        if let Some(kind) = kind_of(def) {
            read.records.insert(key, record_of(index, def, kind));
        }
    }
    read
}
