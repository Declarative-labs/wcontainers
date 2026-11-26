//! Each API's `implementation/direct` crate: the C tables, the serving glue (raw C calls in, any implementation of
//! the traits out) and the calling glue (a raw foreign object wrapped as an implementation), method by method from
//! the metadata, by the same rows of MAPPING.md the traits were written by. A method with a parameter of a row the
//! glue does not handle yet keeps a null entry in our tables, and its calling form answers E_NOTIMPL (or nothing);
//! the report lists each such method with the reason.
//!
//! The glue trusts neither side (SPIKE.md's invariants): every structure is copied once at the size C's layout
//! gives; every implementation call runs under `guarded` and no lock; every out-parameter is written on every path;
//! an object handed out again comes from what the glue recorded when it crossed; each `unsafe` block is one
//! operation with its own mark, its reason naming the metadata fact it rests on.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use windows_metadata::Type;

use super::{FOUNDATION, GUID, Generator, Key, Module, Typedef, UNKNOWN, alias, key, named, nested_name, pointee, primitive};
use crate::layout::Layouts;
use crate::metadata::{Method, Param, Record, RecordKind, attribute_named};
use crate::names;

/// The most elements an array parameter may hold before the glue refuses it, so that no count C passes can make
/// it allocate without bound.
const MOST_ELEMENTS: &str = "1 << 16";

/// Where a rendering is: a direct crate, which names every API by its alias, never as `crate`.
const NOWHERE: usize = usize::MAX;

/// `at + amount` as text, added up when `at` is a number, so that no generated offset adds zero.
fn plus(at: &str, amount: u64) -> String {
    match at.parse::<u64>() {
        Ok(at) => (at + amount).to_string(),
        Err(_) if amount == 0 => at.to_string(),
        Err(_) => format!("{at} + {amount}"),
    }
}

/// The offset of element `index` of an array at `at`.
fn element_at(at: &str, size: u64) -> String {
    if at == "0" { format!("index * {size}") } else { format!("{at} + index * {size}") }
}

fn snake(name: &str) -> String {
    names::snake_case(name)
}

fn upper(name: &str) -> String {
    names::snake_case(name).to_ascii_uppercase()
}

fn direct_alias(api_folder: &str) -> String {
    format!("{}_direct", api_folder.replace('-', "_"))
}

/// A record's byte form: its size, the name of its functions, and whether only reading it is generated (a union
/// read back cannot know which member was written).
#[derive(Clone)]
struct Coded {
    size: u64,
    ident: String,
    path: String,
    decode_only: bool,
}

/// One parameter of a method, as the glue passes it.
#[derive(Clone)]
enum Part {
    /// By value: the C type, the Rust type, and how one becomes the other.
    Value { c: String, rust: String, form: ValueForm },
    StructIn { optional: bool, coded: Coded },
    ObjectIn { optional: bool, kinds: Vec<usize>, enum_path: Option<String> },
    /// An array of values and the index of its count.
    ValuesIn { count: usize, element: Box<Part>, size: u64 },
    /// An array of objects, each may be null, and the index of its count.
    ObjectsIn { count: usize, kind: usize },
    /// A count another parameter names, dropped from the trait.
    Count,
    ObjectOut { kinds: Vec<usize>, made: bool },
    ValueOut { size: u64, element: Box<Part> },
    StructOut { coded: Coded },
    /// `void*` written by the callee, with the index of its byte count; the trait passes `&impl VolatileBytes`.
    BytesOut { count: usize, optional: bool },
}

#[derive(Clone, Copy, PartialEq)]
enum ValueForm {
    Same,
    Bool,
    Newtype,
}

/// What the glue writes for one method.
struct Glued {
    parts: Vec<(String, Part)>,
    status: bool,
    /// A value the method returns, not a status.
    returned: Option<Part>,
}

pub(super) struct Writer<'g, 'a> {
    g: &'g Generator<'a>,
    layouts: Layouts<'g>,
    coded: RefCell<BTreeMap<String, Option<Coded>>>,
    codecs: RefCell<String>,
    pub(super) glued: RefCell<Vec<String>>,
    pub(super) not_glued: RefCell<Vec<String>>,
    /// The API whose direct crate is being written: its own items are named without a crate path.
    current: std::cell::Cell<usize>,
}

impl<'g, 'a> Writer<'g, 'a> {
    pub(super) fn new(g: &'g Generator<'a>) -> Writer<'g, 'a> {
        Writer {
            g,
            layouts: Layouts { read: &g.read },
            coded: RefCell::new(BTreeMap::new()),
            codecs: RefCell::new(String::new()),
            glued: RefCell::new(Vec::new()),
            not_glued: RefCell::new(Vec::new()),
            current: std::cell::Cell::new(NOWHERE),
        }
    }

    fn top(&self, api: usize) -> String {
        self.g.top_path(api, NOWHERE)
    }

    fn kind_type(&self, kind: usize, api_type: &str) -> String {
        let k = &self.g.kinds[kind];
        format!("<{api_type} as {}>::{}", self.top(k.api), k.name)
    }

    fn system_type(&self, kind: usize, from: usize) -> String {
        let k = &self.g.kinds[kind];
        if k.api == from { format!("System{}", k.name) } else { format!("{}::System{}", direct_alias(self.g.apis[k.api].folder), k.name) }
    }

    fn interfaces_fn(&self, kind: usize, from: usize) -> String {
        let k = &self.g.kinds[kind];
        let name = format!("{}_interfaces", snake(&k.name));
        if k.api == from { name } else { format!("{}::{name}", direct_alias(self.g.apis[k.api].folder)) }
    }


    fn iid(&self, interface_key: &Key) -> String {
        let api = self.g.interface_api[interface_key];
        let spec = self.g.apis[api];
        let prefix = spec.interface_prefixes.iter().find(|p| interface_key.1.starts_with(**p)).copied().unwrap_or("");
        format!("{}::constants::{}", alias(spec), names::interface_id_name(prefix, &interface_key.1[prefix.len()..]))
    }

    // ---- Records as bytes.

    fn record_path(&self, record_key: &Key) -> String {
        let owner = self.g.record_owner(record_key);
        self.g.data_path(owner, Module::Structures, &self.g.record_name(record_key), NOWHERE)
    }

    fn codec_of(&self, record_key: &Key) -> Option<Coded> {
        let record = self.g.read.records.get(record_key)?;
        if self.g.record_is_generic(record_key) {
            return None;
        }
        let path = self.record_path(record_key);
        let ident = format!("{}_{}", snake(&record_key.0.replace('.', "_")), snake(&record_key.1)).to_ascii_lowercase();
        let owner_name = self.g.record_name(record_key);
        self.codec(record, &path, &ident, &owner_name)
    }

    fn codec(&self, record: &Record, path: &str, ident: &str, rust_name: &str) -> Option<Coded> {
        if let Some(done) = self.coded.borrow().get(ident) {
            return done.clone();
        }
        self.coded.borrow_mut().insert(ident.to_string(), None);
        let places = self.layouts.field_places(record)?;
        let (size, _) = self.layouts.of_record(record)?;
        let mut decode = String::new();
        let mut encode = String::new();
        let mut decode_only = record.kind == RecordKind::Union;
        let mut used = BTreeSet::new();
        for (field, (offset, _)) in record.fields.iter().zip(&places) {
            let mut name = names::value_name(&field.name);
            while !used.insert(name.clone()) {
                name.push('_');
            }
            let at = offset.to_string();
            let (read, only) = self.read_value(&field.ty, record, rust_name, &at)?;
            decode_only |= only;
            let _ = writeln!(decode, "        {name}: {read},");
            if !decode_only {
                let _ = writeln!(encode, "    {}", self.write_value(&field.ty, record, rust_name, &at, &format!("value.{name}"))?);
            }
        }
        let mut text = String::new();
        let _ = writeln!(text, "/// Reads `{}` from the {size} bytes C lays it out in.", record.name);
        let _ = writeln!(text, "pub fn decode_{ident}(bytes: &[u8]) -> {path} {{\n    {path} {{\n{decode}    }}\n}}\n");
        if !decode_only {
            let _ = writeln!(text, "/// Writes `{}` as the {size} bytes C lays it out in.", record.name);
            let _ = writeln!(text, "pub fn encode_{ident}(value: &{path}, bytes: &mut [u8]) {{\n{encode}}}\n");
        }
        self.codecs.borrow_mut().push_str(&text);
        let coded = Coded { size, ident: ident.to_string(), path: path.to_string(), decode_only };
        self.coded.borrow_mut().insert(ident.to_string(), Some(coded.clone()));
        Some(coded)
    }

    /// The expression reading a value of `ty` at byte offset `at` of `bytes`, and whether it holds a union.
    fn read_value(&self, ty: &Type, outer: &Record, outer_name: &str, at: &str) -> Option<(String, bool)> {
        if let Some(p) = primitive(ty) {
            if p == "bool" {
                return None;
            }
            return Some((format!("get_{p}(bytes, {at})"), false));
        }
        match ty {
            Type::ArrayFixed(inner, length) => {
                let (element_size, _) = self.layouts.of_type(inner, outer)?;
                let (element, only) = self.read_value(inner, outer, outer_name, &element_at(at, element_size))?;
                let _ = length;
                Some((format!("core::array::from_fn(|index| {element})"), only))
            }
            Type::Name(name) if name.namespace.is_empty() => {
                let nested = outer.nested.iter().find(|nested| nested.name == name.name)?;
                let owner = self.g.record_owner(&key(&outer.namespace, &outer.name));
                let rust = nested_name(outer_name, &nested.name);
                let path = self.g.data_path(owner, Module::Structures, &rust, NOWHERE);
                let ident = format!("{}_{}", snake(&outer.namespace.replace('.', "_")), snake(&rust)).to_ascii_lowercase();
                let coded = self.codec(nested, &path, &ident, &rust)?;
                Some((format!("decode_{}(&bytes[{at}..{}])", coded.ident, plus(at, coded.size)), coded.decode_only))
            }
            Type::Name(_) => {
                let named_key = named(ty)?;
                if named_key == key(GUID.0, GUID.1) {
                    return Some((format!("get_guid(bytes, {at})"), false));
                }
                if Generator::is_large_integer(&named_key) {
                    return Some((format!("get_i64(bytes, {at})"), false));
                }
                let record = self.g.read.records.get(&named_key)?;
                match record.kind {
                    RecordKind::Enum => {
                        let underlying = record.underlying.as_ref().and_then(primitive).unwrap_or("i32");
                        let path = self.g.data_path(self.g.record_owner(&named_key), Module::Enumerations, &self.g.record_name(&named_key), NOWHERE);
                        Some((format!("{path}(get_{underlying}(bytes, {at}))"), false))
                    }
                    RecordKind::Typedef => match self.g.typedef_form(&named_key) {
                        Typedef::Bool => Some((format!("get_i32(bytes, {at}) != 0"), false)),
                        Typedef::Status => Some((format!("{}(get_i32(bytes, {at}))", self.g.status_path(NOWHERE)), false)),
                        Typedef::Newtype => {
                            let inner = record.fields.first().map_or("usize", |f| if pointee(&f.ty).is_some() { "usize" } else { primitive(&f.ty).unwrap_or("usize") });
                            Some((format!("{}(get_{inner}(bytes, {at}))", self.record_path(&named_key)), false))
                        }
                        _ => None,
                    },
                    RecordKind::Struct | RecordKind::Union => {
                        let coded = self.codec_of(&named_key)?;
                        Some((format!("decode_{}(&bytes[{at}..{}])", coded.ident, plus(at, coded.size)), coded.decode_only))
                    }
                    RecordKind::Delegate => None,
                }
            }
            _ => None,
        }
    }

    /// The statement writing `value` of `ty` at byte offset `at` of `bytes`.
    fn write_value(&self, ty: &Type, outer: &Record, outer_name: &str, at: &str, value: &str) -> Option<String> {
        if let Some(p) = primitive(ty) {
            return Some(format!("put_{p}(bytes, {at}, {value});"));
        }
        match ty {
            Type::ArrayFixed(inner, _) => {
                let (element_size, _) = self.layouts.of_type(inner, outer)?;
                let element = self.write_value(inner, outer, outer_name, &element_at(at, element_size), "*item")?;
                Some(format!("for (index, item) in {value}.iter().enumerate() {{ {element} }}"))
            }
            Type::Name(name) if name.namespace.is_empty() => {
                let nested = outer.nested.iter().find(|nested| nested.name == name.name)?;
                let owner = self.g.record_owner(&key(&outer.namespace, &outer.name));
                let rust = nested_name(outer_name, &nested.name);
                let path = self.g.data_path(owner, Module::Structures, &rust, NOWHERE);
                let ident = format!("{}_{}", snake(&outer.namespace.replace('.', "_")), snake(&rust)).to_ascii_lowercase();
                let coded = self.codec(nested, &path, &ident, &rust)?;
                (!coded.decode_only).then(|| format!("encode_{}(&{value}, &mut bytes[{at}..{}]);", coded.ident, plus(at, coded.size)))
            }
            Type::Name(_) => {
                let named_key = named(ty)?;
                if named_key == key(GUID.0, GUID.1) {
                    return Some(format!("put_guid(bytes, {at}, &{value});"));
                }
                if Generator::is_large_integer(&named_key) {
                    return Some(format!("put_i64(bytes, {at}, {value});"));
                }
                let record = self.g.read.records.get(&named_key)?;
                match record.kind {
                    RecordKind::Enum => {
                        let underlying = record.underlying.as_ref().and_then(primitive).unwrap_or("i32");
                        Some(format!("put_{underlying}(bytes, {at}, {value}.0);"))
                    }
                    RecordKind::Typedef => match self.g.typedef_form(&named_key) {
                        Typedef::Bool => Some(format!("put_i32(bytes, {at}, i32::from({value}));")),
                        Typedef::Status => Some(format!("put_i32(bytes, {at}, {value}.0);")),
                        Typedef::Newtype => {
                            let inner = record.fields.first().map_or("usize", |f| if pointee(&f.ty).is_some() { "usize" } else { primitive(&f.ty).unwrap_or("usize") });
                            Some(format!("put_{inner}(bytes, {at}, {value}.0);"))
                        }
                        _ => None,
                    },
                    RecordKind::Struct | RecordKind::Union => {
                        let coded = self.codec_of(&named_key)?;
                        (!coded.decode_only).then(|| format!("encode_{}(&{value}, &mut bytes[{at}..{}]);", coded.ident, plus(at, coded.size)))
                    }
                    RecordKind::Delegate => None,
                }
            }
            _ => None,
        }
    }

    // ---- Parameters.

    fn is_object(&self, k: &Key) -> bool {
        self.g.interface_api.contains_key(k) || *k == key(UNKNOWN.0, UNKNOWN.1)
    }

    /// A value passed by value: C's type, Rust's type and how one becomes the other.
    fn value_part(&self, ty: &Type) -> Result<Part, String> {
        if let Some(p) = primitive(ty) {
            if p == "bool" {
                return Err("a C99 bool".to_string());
            }
            return Ok(Part::Value { c: p.to_string(), rust: p.to_string(), form: ValueForm::Same });
        }
        let named_key = named(ty).ok_or_else(|| format!("{ty:?}"))?;
        if Generator::is_large_integer(&named_key) {
            return Ok(Part::Value { c: "i64".to_string(), rust: "i64".to_string(), form: ValueForm::Same });
        }
        let record = self.g.read.records.get(&named_key).ok_or_else(|| format!("{} not read", named_key.1))?;
        match record.kind {
            RecordKind::Enum => {
                let underlying = record.underlying.as_ref().and_then(primitive).unwrap_or("i32");
                let path = self.g.data_path(self.g.record_owner(&named_key), Module::Enumerations, &self.g.record_name(&named_key), NOWHERE);
                Ok(Part::Value { c: underlying.to_string(), rust: path, form: ValueForm::Newtype })
            }
            RecordKind::Typedef => match self.g.typedef_form(&named_key) {
                Typedef::Bool => Ok(Part::Value { c: "i32".to_string(), rust: "bool".to_string(), form: ValueForm::Bool }),
                Typedef::Newtype => {
                    let inner = record.fields.first().map_or("usize", |f| if pointee(&f.ty).is_some() { "usize" } else { primitive(&f.ty).unwrap_or("usize") });
                    Ok(Part::Value { c: inner.to_string(), rust: self.record_path(&named_key), form: ValueForm::Newtype })
                }
                _ => Err(format!("a {} by value", named_key.1)),
            },
            _ => Err(format!("a {} by value", named_key.1)),
        }
    }

    fn object_kinds(&self, interface_key: &Key) -> (Vec<usize>, Option<String>) {
        let kinds = self.g.kinds_deriving(interface_key);
        let single = kinds.len() == 1 && self.g.kind_of.get(interface_key) == kinds.first();
        let enum_path = (!single).then(|| self.g.enum_path(interface_key, "Ref", NOWHERE));
        (kinds, enum_path)
    }

    fn glue_of(&self, method: &Method) -> Result<Glued, String> {
        let mut counts: BTreeSet<usize> = BTreeSet::new();
        for param in &method.params {
            for (attribute, argument) in [("NativeArrayInfo", "CountParamIndex"), ("MemorySize", "BytesParamIndex")] {
                if let Some(count) = attribute_named(&param.attributes, attribute).and_then(|a| super::integer_argument(a, argument)) {
                    counts.insert(usize::try_from(count).map_err(|_| "a negative count index".to_string())?);
                }
            }
        }
        let mut parts = Vec::new();
        let mut used = BTreeSet::new();
        for (position, param) in method.params.iter().enumerate() {
            let mut name = names::value_name(&param.name);
            while !used.insert(name.clone()) {
                name.push('_');
            }
            let part = if counts.contains(&position) {
                if pointee(&param.ty).is_some() {
                    return Err(format!("{}: a count passed by pointer", param.name));
                }
                Part::Count
            } else {
                self.part(param).map_err(|why| format!("{}: {why}", param.name))?
            };
            parts.push((name, part));
        }
        let status = named(&method.return_type).is_some_and(|k| k == key(FOUNDATION, "HRESULT"));
        let returned = if status || method.return_type == Type::Void {
            None
        } else {
            Some(self.value_part(&method.return_type).map_err(|why| format!("return: {why}"))?)
        };
        Ok(Glued { parts, status, returned })
    }

    fn part(&self, param: &Param) -> Result<Part, String> {
        let is_out = param.is_out && !param.is_in;
        let array = attribute_named(&param.attributes, "NativeArrayInfo");
        let bytes = attribute_named(&param.attributes, "MemorySize");
        let Some((inner, depth, is_const)) = pointee(&param.ty) else {
            if let Some(interface_key) = named(&param.ty).filter(|k| self.is_object(k)) {
                if interface_key == key(UNKNOWN.0, UNKNOWN.1) {
                    return Err("an IUnknown in".to_string());
                }
                let (kinds, enum_path) = self.object_kinds(&interface_key);
                if kinds.is_empty() {
                    return Err(format!("no kind is an {}", interface_key.1));
                }
                return Ok(Part::ObjectIn { optional: param.is_optional, kinds, enum_path });
            }
            return self.value_part(&param.ty);
        };
        if depth != 1 {
            return Err(format!("a pointer of depth {depth}"));
        }
        if let Some(array) = array {
            let count = super::integer_argument(array, "CountParamIndex").ok_or("an array whose count is not a parameter")?;
            let count = usize::try_from(count).map_err(|_| "a negative count")?;
            if is_out || !is_const && param.is_out {
                return Err("an array out".to_string());
            }
            if let Some(interface_key) = named(inner).filter(|k| self.is_object(k)) {
                let (kinds, enum_path) = self.object_kinds(&interface_key);
                if kinds.len() != 1 || enum_path.is_some() {
                    return Err("an array of objects of several kinds".to_string());
                }
                return Ok(Part::ObjectsIn { count, kind: kinds[0] });
            }
            let element = self.value_part(inner)?;
            let (size, _) = primitive_or_record_size(&self.layouts, inner).ok_or("an element of unknown size")?;
            return Ok(Part::ValuesIn { count, element: Box::new(element), size });
        }
        if let Some(bytes) = bytes {
            if !(is_out || param.is_out && !is_const) {
                return Err("bytes in".to_string());
            }
            let count = super::integer_argument(bytes, "BytesParamIndex").ok_or("bytes whose count is not a parameter")?;
            return Ok(Part::BytesOut { count: usize::try_from(count).map_err(|_| "a negative count")?, optional: param.is_optional });
        }
        if let Some(interface_key) = named(inner).filter(|k| self.is_object(k)) {
            if !(is_out || attribute_named(&param.attributes, "ComOutPtr").is_some()) {
                return Err("an interface pointer by pointer, not out".to_string());
            }
            if interface_key == key(UNKNOWN.0, UNKNOWN.1) {
                return Err("an IUnknown out".to_string());
            }
            let made = attribute_named(&param.attributes, "ComOutPtr").is_some();
            let kinds = self.g.kinds_deriving(&interface_key);
            let single = kinds.len() == 1 && self.g.kind_of.get(&interface_key) == kinds.first();
            if !single {
                return Err("an object out of several kinds".to_string());
            }
            return Ok(Part::ObjectOut { kinds, made });
        }
        if *inner == Type::Void {
            return Err("a void pointer".to_string());
        }
        let named_key = named(inner);
        if let Some(named_key) = &named_key
            && let Some(record) = self.g.read.records.get(named_key)
            && matches!(record.kind, RecordKind::Struct | RecordKind::Union)
            && !Generator::is_large_integer(named_key)
        {
            let coded = self.codec_of(named_key).ok_or("a structure the glue cannot copy as bytes")?;
            if is_out {
                return Ok(Part::StructOut { coded });
            }
            if param.is_out {
                return Err("a structure in and out".to_string());
            }
            return Ok(Part::StructIn { optional: param.is_optional, coded });
        }
        if named_key.as_ref().is_some_and(|k| *k == key(GUID.0, GUID.1)) {
            if is_out {
                return Err("a GUID out".to_string());
            }
            return Ok(Part::StructIn {
                optional: param.is_optional,
                coded: Coded { size: 16, ident: "guid".to_string(), path: self.g.guid_path(NOWHERE), decode_only: false },
            });
        }
        if is_out {
            let element = self.value_part(inner)?;
            let (size, _) = primitive_or_record_size(&self.layouts, inner).ok_or("a value of unknown size")?;
            return Ok(Part::ValueOut { size, element: Box::new(element) });
        }
        Err("a pointer to a value in".to_string())
    }
}

fn primitive_or_record_size(layouts: &Layouts<'_>, ty: &Type) -> Option<(u64, u64)> {
    let empty = Record {
        namespace: String::new(),
        name: String::new(),
        kind: RecordKind::Struct,
        fields: Vec::new(),
        nested: Vec::new(),
        attributes: Vec::new(),
        underlying: None,
        packing: None,
    };
    layouts.of_type(ty, &empty)
}

/// The helpers every direct crate's byte forms use: one getter and one putter per primitive.
const BYTES_HELPERS: &str = r#"
macro_rules! primitive_bytes {
    ($($get:ident $put:ident $type:ty;)*) => {$(
        /// Reads the value at this offset, as C lays it out (little-endian).
        pub fn $get(bytes: &[u8], at: usize) -> $type {
            let mut value = [0_u8; size_of::<$type>()];
            value.copy_from_slice(&bytes[at..at + size_of::<$type>()]);
            <$type>::from_le_bytes(value)
        }

        /// Writes the value at this offset, as C lays it out (little-endian).
        pub fn $put(bytes: &mut [u8], at: usize, value: $type) {
            bytes[at..at + size_of::<$type>()].copy_from_slice(&value.to_le_bytes());
        }
    )*};
}

primitive_bytes! {
    get_u8 put_u8 u8;
    get_i8 put_i8 i8;
    get_u16 put_u16 u16;
    get_i16 put_i16 i16;
    get_u32 put_u32 u32;
    get_i32 put_i32 i32;
    get_u64 put_u64 u64;
    get_i64 put_i64 i64;
    get_f32 put_f32 f32;
    get_f64 put_f64 f64;
    get_usize put_usize usize;
    get_isize put_isize isize;
}

/// Reads a GUID at this offset.
pub fn get_guid(bytes: &[u8], at: usize) -> wcontainers_component_object_model::structures::Guid {
    let mut value = [0_u8; wcontainers_component_object_model_direct::GUID_SIZE];
    value.copy_from_slice(&bytes[at..at + wcontainers_component_object_model_direct::GUID_SIZE]);
    wcontainers_component_object_model_direct::guid_from(&value)
}

/// Writes a GUID at this offset.
pub fn put_guid(bytes: &mut [u8], at: usize, value: &wcontainers_component_object_model::structures::Guid) {
    let value = wcontainers_component_object_model_direct::guid_bytes(value);
    bytes[at..at + value.len()].copy_from_slice(&value);
}

/// Reads a GUID from its 16 bytes.
pub fn decode_guid(bytes: &[u8]) -> wcontainers_component_object_model::structures::Guid {
    get_guid(bytes, 0)
}

/// Writes a GUID as its 16 bytes.
pub fn encode_guid(value: &wcontainers_component_object_model::structures::Guid, bytes: &mut [u8]) {
    put_guid(bytes, 0, value);
}
"#;

#[path = "glue_call.rs"]
mod call;
#[path = "glue_write.rs"]
mod write;

pub(super) use write::write_direct;
