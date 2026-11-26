//! Each API's `implementation/direct` crate: the C tables, the serving glue (raw C calls in, any implementation of
//! the traits out) and the calling glue (a raw foreign object wrapped as an implementation), method by method from
//! the metadata, by the same rows of MAPPING.md the traits were written by. A method with a parameter of a form the
//! glue does not carry gets an entry that refuses it (src/glue.rs `refusal`: nothing read, COM's null and zeros written
//! through its out-parameters, E_NOTIMPL or no result answered), never a null entry; its calling form answers the same
//! refusal, never a panic. The report lists each such method with what each glue wrote, from the same record.
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
use crate::api::{Exception, Shape};
use crate::layout::Layouts;
use crate::metadata::{Method, Param, Record, RecordKind, attribute_named};
use crate::names;

/// The most elements an array parameter may hold before the glue refuses it, so that no count C passes can make
/// it allocate without bound.
const MOST_ELEMENTS: &str = "1 << 16";

/// Where a rendering is: a direct crate, which names every API by its alias, never as `crate`.
const NOWHERE: usize = usize::MAX;

/// The offset of element `index` of an array at `at`, each element of the mirror type `element`.
fn element_at(at: &str, element: &str) -> String {
    format!("{at} + index * size_of::<{element}>()")
}

/// The end of a record whose mirror is `mirror`, at `at`.
fn end_of(at: &str, mirror: &str) -> String {
    format!("{at} + size_of::<{mirror}>()")
}

/// The name of a record's `#[repr(C)]` mirror, from its codec's identifier.
fn mirror_name(ident: &str) -> String {
    let mut name: String = ident.split('_').filter(|part| !part.is_empty()).map(|part| {
        let mut chars = part.chars();
        chars.next().map_or_else(String::new, |first| first.to_ascii_uppercase().to_string() + chars.as_str())
    }).collect();
    name.push_str("Layout");
    name
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
    /// The `#[repr(C)]` mirror of the record in data.rs, whose `size_of` and `offset_of!` the byte form is written with.
    mirror: String,
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
    /// An array of values and the index of its count; `as_option` when the trait takes it as an `Option` (bytes `_opt_`).
    ValuesIn { count: usize, element: Box<Part>, size: u64, optional: bool, as_option: bool },
    /// An array of objects, each may be null, and the index of its count.
    ObjectsIn { count: usize, kind: usize, optional: bool },
    /// A count another parameter names, dropped from the trait.
    Count,
    ObjectOut { kinds: Vec<usize>, made: bool },
    ValueOut { size: u64, element: Box<Part> },
    StructOut { coded: Coded },
    /// `void*` written by the callee, with the index of its byte count; the trait passes `&impl VolatileBytes`.
    BytesOut { count: usize, optional: bool },
    /// Initial data (an exception line): one record, or one per subresource, of the description at index `desc`;
    /// each record's bytes field is read-only, of the length the shape's rule gives.
    InitialData { desc: usize, shape: Shape, per_subresource: bool, size: u64, bytes_at: u64, pitch_at: u64, slice_at: u64, path: String, fields: [String; 3] },
    /// Memory lent until a later call (an exception line): Map's record out, with the indices of the resource and the
    /// subresource, and the names of the record's three fields.
    LentOut { coded: Coded, resource: usize, subresource: usize, fields: [String; 3] },
    /// An `IUnknown*` in: one of the API's kinds, or any other COM object.
    UnknownIn { optional: bool },
    /// The interface id a bound object is asked for (an exception line).
    Riid,
    /// A bound object out (an exception line), of the top trait's associated type `name`.
    BoundOut { name: String },
    /// `void**` out of the interface the GUID at index `riid` names: the API's `UnknownObject`, one reference.
    InterfaceOut { riid: usize },
    /// A fixed array in (`CountConst`): `length` elements of `size` bytes each.
    FixedIn { element: Box<Part>, length: u64, size: u64, optional: bool },
    /// A fixed array out (`CountConst`): `length` elements of `size` bytes each.
    FixedOut { element: Box<Part>, length: u64, size: u64 },
    /// An array of values the callee writes, and the index of its count: passed by value (kept in the trait), or by
    /// pointer (in, the room C gave; out, how many were written).
    ValuesOut { count: usize, element: Box<Part>, size: u64, by_pointer: bool },
    /// An array of objects the callee writes, each one reference handed over or null, and the index of its count,
    /// passed by value.
    ObjectsOut { count: usize, kind: usize },
    /// A count passed by pointer that an array out names: the room in, the number written out.
    CountByPointer,
    /// A text in (`PSTR` or `PWSTR` by value, `In`): units ending at the first zero unit, copied once by the serving glue;
    /// the trait's `&CStr` or `&[u16]`.
    TextIn { wide: bool, optional: bool },
}

/// What an entry that refuses its method writes through one of its parameters.
#[derive(Clone, PartialEq)]
pub(super) enum Writes {
    /// Nothing: an input, an array or bytes out (no count of them is trusted), or a parameter in and out.
    Nothing,
    /// A null pointer: COM's rule for an out-pointer on failure.
    Null,
    /// Zero bytes of this size: a value out, which holds nothing.
    Zeros(u64),
}

impl Writes {
    /// The bytes written, as the generated text names them; none for `Nothing`.
    pub(super) fn bytes(&self) -> Option<String> {
        match self {
            Writes::Nothing => None,
            Writes::Null => Some("[u8::MIN; size_of::<usize>()]".to_string()),
            Writes::Zeros(size) => Some(format!("[u8::MIN; {size}]")),
        }
    }

    /// The bytes written, for C.
    pub(super) fn c_size(&self) -> Option<String> {
        match self {
            Writes::Nothing => None,
            Writes::Null => Some("sizeof(void *)".to_string()),
            Writes::Zeros(size) => Some(size.to_string()),
        }
    }
}

/// The entry of a method no form of the glue carries: its C types by calling convention only, what it writes through
/// each parameter (out-pointers nulled, values out zeroed, nothing read), and the refusal it answers.
#[derive(Clone)]
pub(super) struct Refusal {
    /// Each parameter after `this`: its name in the metadata, its C type, and what the entry writes through it.
    pub(super) params: Vec<(String, String, Writes)>,
    /// C's return type.
    pub(super) returned: String,
    /// The Rust expression the entry answers (empty for no result).
    pub(super) answer: String,
    /// The answer as the report states it.
    pub(super) stated: String,
}

/// What each glue does with one method, recorded when its text is written: the report is printed from this, so that
/// the report and the glue cannot disagree.
#[derive(Clone, Default)]
pub(super) struct Outcome {
    /// Why no form of the glue carries it, if none does.
    pub(super) reason: Option<String>,
    /// What the serving glue's table entry does.
    pub(super) serving: BTreeSet<String>,
    /// What the calling glue's method does.
    pub(super) calling: BTreeSet<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum ValueForm {
    Same,
    Bool,
    Newtype,
    /// A GUID by value: C passes guiddef.h's structure (`GuidAbi`).
    Guid,
}

/// The type the glue passes a GUID by value as.
const GUID_ABI: &str = "component_object_model_direct::GuidAbi";

/// What the glue writes for one method.
struct Glued {
    parts: Vec<(String, Part)>,
    /// Counts with a range (an exception line): the count's index, the most, the start's index, the keep value, and
    /// the indices the keep value leaves unread.
    ranges: Vec<(usize, u32, Option<usize>, Option<u32>, Vec<usize>)>,
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
    /// What each glue wrote for each method, by its place (`Interface::Method`, or an export's name).
    pub(super) outcomes: RefCell<BTreeMap<String, Outcome>>,
    /// What the generator could not write without a panic or a null entry: each fails the generator.
    pub(super) failures: RefCell<Vec<String>>,
    /// The API whose direct crate is being written: its own items are named without a crate path.
    current: std::cell::Cell<usize>,
    /// Whether the recorder being written is the hostile one (src/glue_hostile.rs).
    hostile: std::cell::Cell<bool>,
    /// For the recorder and the hostile implementation: a value of each structure they can make, by its path (a
    /// decoder of zero bytes, or `Default`).
    struct_values: RefCell<BTreeMap<String, String>>,
    /// Whether the codecs also write, per crate, a test of each mirror's offsets and size against the generator's own
    /// (the mirror-check build, never the crates the repository keeps).
    pub(super) mirror_check: std::cell::Cell<bool>,
    /// The checks written so far for the crate being written.
    pub(super) mirror_checks: RefCell<String>,
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
            outcomes: RefCell::new(BTreeMap::new()),
            failures: RefCell::new(Vec::new()),
            current: std::cell::Cell::new(NOWHERE),
            hostile: std::cell::Cell::new(false),
            struct_values: RefCell::new(BTreeMap::new()),
            mirror_check: std::cell::Cell::new(std::env::var_os("MIRROR_CHECK").is_some()),
            mirror_checks: RefCell::new(String::new()),
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
        let mirror = mirror_name(ident);
        let mut mirror_fields = String::new();
        let mut mirror_names: Vec<String> = Vec::new();
        let mut decode = String::new();
        let mut encode = String::new();
        let union = record.kind == RecordKind::Union;
        let mut decode_only = false;
        let mut used = BTreeSet::new();
        // A lent record crosses with its memory's address: `Record<usize>`.
        let lent_field = match self.g.lent_record(&record.name) {
            Some(Exception::Lent { field, .. }) => Some(*field),
            _ => None,
        };
        for field in &record.fields {
            let mut name = names::value_name(&field.name);
            while !used.insert(name.clone()) {
                name.push('_');
            }
            mirror_names.push(name.clone());
            let at = format!("offset_of!({mirror}, {name})");
            if lent_field == Some(field.name.as_str()) {
                let _ = writeln!(mirror_fields, "    {name}: usize,");
                let _ = writeln!(decode, "        {name}: get_usize(bytes, {at}),");
                let _ = writeln!(encode, "    put_usize(bytes, {at}, value.{name});");
                continue;
            }
            let field_mirror = self.mirror_type(&field.ty, record, rust_name)?;
            let _ = writeln!(mirror_fields, "    {name}: {field_mirror},");
            let (read, only) = self.read_value(&field.ty, record, rust_name, &at)?;
            decode_only |= only;
            let _ = writeln!(decode, "        {name}: {read},");
            if !decode_only {
                let statement = self.write_value(&field.ty, record, rust_name, &at, &format!("value.{name}"))?;
                if union {
                    // A union is written as the largest of its members that is not all zeros: exact for a value read
                    // from C (every member holds the same bytes) and for one built with the other members zeroed.
                    let _ = writeln!(encode, "    {{\n        let mut scratch = [u8::MIN; size_of::<{mirror}>()];\n        {{\n            let bytes = &mut scratch[..];\n            {statement}\n        }}");
                    let _ = writeln!(encode, "        if scratch != [u8::MIN; size_of::<{mirror}>()] && size_of::<{field_mirror}>() > chosen.0 {{\n            chosen = (size_of::<{field_mirror}>(), scratch);\n        }}\n    }}");
                } else {
                    let _ = writeln!(encode, "    {statement}");
                }
            }
        }
        if union && !decode_only {
            encode = format!("    let mut chosen = (usize::MIN, [u8::MIN; size_of::<{mirror}>()]);\n{encode}    bytes[..size_of::<{mirror}>()].copy_from_slice(&chosen.1);\n");
        }
        let mut text = String::new();
        // The mirror: the record's fields in C's types and order, laid out by Rust's `repr(C)` with the record's packing.
        // The generator's own offsets (which the layout check compares with MinGW's headers) are checked against it by
        // the mirror-check build (MIRROR_CHECK set).
        let repr = record.packing.map_or_else(|| "C".to_string(), |pack| format!("C, packed({pack})"));
        let keyword = if union { "union" } else { "struct" };
        let _ = writeln!(text, "/// `{}` as C lays it out: its offsets and size are this type's.\n#[derive(Clone, Copy)]\n#[repr({repr})]\npub(crate) {keyword} {mirror} {{\n{mirror_fields}}}\n", record.name);
        if self.mirror_check.get() {
            let mut checks = String::new();
            for ((offset, _), name) in places.iter().zip(&mirror_names) {
                let _ = writeln!(checks, "    assert_eq!(offset_of!({mirror}, {name}), {offset}, \"{} {name}\");", record.name);
            }
            let _ = writeln!(checks, "    assert_eq!(size_of::<{mirror}>(), {size}, \"{} size\");", record.name);
            self.mirror_checks.borrow_mut().push_str(&checks);
        }
        let path = if lent_field.is_some() { format!("{path}<usize>") } else { path.to_string() };
        let path = path.as_str();
        let expression = path.replace('<', "::<");
        let _ = writeln!(text, "/// Reads `{}` from the {size} bytes C lays it out in.", record.name);
        let _ = writeln!(text, "pub fn decode_{ident}(bytes: &[u8]) -> {path} {{\n    {expression} {{\n{decode}    }}\n}}\n");
        if !decode_only {
            let _ = writeln!(text, "/// Writes `{}` as the {size} bytes C lays it out in.", record.name);
            let _ = writeln!(text, "pub fn encode_{ident}(value: &{path}, bytes: &mut [u8]) {{\n{encode}}}\n");
        }
        self.codecs.borrow_mut().push_str(&text);
        let coded = Coded { size, mirror, ident: ident.to_string(), path: path.to_string(), decode_only };
        self.coded.borrow_mut().insert(ident.to_string(), Some(coded.clone()));
        Some(coded)
    }

    /// The type a field of `ty` has in a record's `#[repr(C)]` mirror: C's primitive, the mirror of a record, an array of
    /// those; none where the byte forms have no reading either.
    fn mirror_type(&self, ty: &Type, outer: &Record, outer_name: &str) -> Option<String> {
        if let Some(p) = primitive(ty) {
            return (p != "bool").then(|| p.to_string());
        }
        match ty {
            Type::ArrayFixed(inner, length) => Some(format!("[{}; {length}]", self.mirror_type(inner, outer, outer_name)?)),
            Type::Name(name) if name.namespace.is_empty() => {
                let nested = outer.nested.iter().find(|nested| nested.name == name.name)?;
                let owner = self.g.record_owner(&key(&outer.namespace, &outer.name));
                let rust = nested_name(outer_name, &nested.name);
                let path = self.g.data_path(owner, Module::Structures, &rust, NOWHERE);
                let ident = format!("{}_{}", snake(&outer.namespace.replace('.', "_")), snake(&rust)).to_ascii_lowercase();
                Some(self.codec(nested, &path, &ident, &rust)?.mirror)
            }
            Type::Name(_) => {
                let named_key = named(ty)?;
                if named_key == key(GUID.0, GUID.1) {
                    return Some("GuidLayout".to_string());
                }
                if Generator::is_large_integer(&named_key) {
                    return Some("i64".to_string());
                }
                let record = self.g.read.records.get(&named_key)?;
                match record.kind {
                    RecordKind::Enum => Some(record.underlying.as_ref().and_then(primitive).unwrap_or("i32").to_string()),
                    RecordKind::Typedef => match self.g.typedef_form(&named_key) {
                        Typedef::Bool | Typedef::Status => Some("i32".to_string()),
                        Typedef::Newtype => Some(record.fields.first().map_or("usize", |f| if pointee(&f.ty).is_some() { "usize" } else { primitive(&f.ty).unwrap_or("usize") }).to_string()),
                        _ => None,
                    },
                    RecordKind::Struct | RecordKind::Union => Some(self.codec_of(&named_key)?.mirror),
                    RecordKind::Delegate => None,
                }
            }
            _ => None,
        }
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
            Type::ArrayFixed(inner, _) => {
                let element_type = self.mirror_type(inner, outer, outer_name)?;
                let (element, only) = self.read_value(inner, outer, outer_name, &element_at(at, &element_type))?;
                Some((format!("core::array::from_fn(|index| {element})"), only))
            }
            Type::Name(name) if name.namespace.is_empty() => {
                let nested = outer.nested.iter().find(|nested| nested.name == name.name)?;
                let owner = self.g.record_owner(&key(&outer.namespace, &outer.name));
                let rust = nested_name(outer_name, &nested.name);
                let path = self.g.data_path(owner, Module::Structures, &rust, NOWHERE);
                let ident = format!("{}_{}", snake(&outer.namespace.replace('.', "_")), snake(&rust)).to_ascii_lowercase();
                let coded = self.codec(nested, &path, &ident, &rust)?;
                Some((format!("decode_{}(&bytes[{at}..{}])", coded.ident, end_of(at, &coded.mirror)), coded.decode_only))
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
                        Some((format!("decode_{}(&bytes[{at}..{}])", coded.ident, end_of(at, &coded.mirror)), coded.decode_only))
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
                let element_type = self.mirror_type(inner, outer, outer_name)?;
                let element = self.write_value(inner, outer, outer_name, &element_at(at, &element_type), "*item")?;
                Some(format!("for (index, item) in {value}.iter().enumerate() {{ {element} }}"))
            }
            Type::Name(name) if name.namespace.is_empty() => {
                let nested = outer.nested.iter().find(|nested| nested.name == name.name)?;
                let owner = self.g.record_owner(&key(&outer.namespace, &outer.name));
                let rust = nested_name(outer_name, &nested.name);
                let path = self.g.data_path(owner, Module::Structures, &rust, NOWHERE);
                let ident = format!("{}_{}", snake(&outer.namespace.replace('.', "_")), snake(&rust)).to_ascii_lowercase();
                let coded = self.codec(nested, &path, &ident, &rust)?;
                (!coded.decode_only).then(|| {
                    // An array's element is already a reference: `*item` is passed as `item`, not reborrowed.
                    let reference = value.strip_prefix('*').map_or_else(|| format!("&{value}"), str::to_string);
                    format!("encode_{}({reference}, &mut bytes[{at}..{}]);", coded.ident, end_of(at, &coded.mirror))
                })
            }
            Type::Name(_) => {
                let named_key = named(ty)?;
                if named_key == key(GUID.0, GUID.1) {
                    return Some(format!("put_guid(bytes, {at}, {});", value.strip_prefix('*').map_or_else(|| format!("&{value}"), str::to_string)));
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
                        (!coded.decode_only).then(|| {
                    // An array's element is already a reference: `*item` is passed as `item`, not reborrowed.
                    let reference = value.strip_prefix('*').map_or_else(|| format!("&{value}"), str::to_string);
                    format!("encode_{}({reference}, &mut bytes[{at}..{}]);", coded.ident, end_of(at, &coded.mirror))
                })
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

    /// The kinds an interface pointer may be, and the path of its `Ref` enum when there are several; an error for an
    /// interface no kind of the set is (one left out to another API), which the trait does not carry either.
    fn object_kinds(&self, interface_key: &Key) -> Result<(Vec<usize>, Option<String>), String> {
        let kinds = self.g.kinds_deriving(interface_key);
        if kinds.is_empty() || !self.g.interface_api.contains_key(interface_key) {
            return Err(format!("no object kind of the set is an {}", interface_key.1));
        }
        let single = kinds.len() == 1 && self.g.kind_of.get(interface_key) == kinds.first();
        let enum_path = (!single).then(|| self.g.enum_path(interface_key, "Ref", NOWHERE));
        Ok((kinds, enum_path))
    }

    /// Whether a record holds only integers (no float, at any depth): passed by value as one integer of its size by both
    /// x64 calling conventions when it is 1, 2, 4 or 8 bytes.
    fn integer_record(&self, record: &Record) -> bool {
        record.fields.iter().all(|field| match primitive(&field.ty) {
            Some(p) => !p.starts_with('f'),
            None => match &field.ty {
                Type::ArrayFixed(inner, _) => primitive(inner).is_some_and(|p| !p.starts_with('f')),
                Type::Name(name) if name.namespace.is_empty() => record.nested.iter().find(|nested| nested.name == name.name).is_some_and(|nested| self.integer_record(nested)),
                _ => named(&field.ty).and_then(|k| self.g.read.records.get(&k)).is_some_and(|inner| match inner.kind {
                    RecordKind::Enum => true,
                    RecordKind::Typedef => !matches!(self.g.typedef_form(&key(&inner.namespace, &inner.name)), Typedef::String | Typedef::WideString),
                    RecordKind::Struct | RecordKind::Union => self.integer_record(inner),
                    RecordKind::Delegate => false,
                }),
            },
        })
    }

    /// The C type a value passes by in an entry that reads nothing: its calling convention only.
    fn by_value_c(&self, ty: &Type) -> Result<String, String> {
        if let Ok(Part::Value { c, .. }) = self.value_part(ty) {
            return Ok(c);
        }
        let named_key = named(ty).ok_or_else(|| format!("{ty:?} by value"))?;
        let record = self.g.read.records.get(&named_key).ok_or_else(|| format!("{} not read", named_key.1))?;
        match record.kind {
            RecordKind::Typedef if matches!(self.g.typedef_form(&named_key), Typedef::String | Typedef::WideString) => Ok("*const u8".to_string()),
            RecordKind::Delegate => Ok("usize".to_string()),
            RecordKind::Struct | RecordKind::Union => {
                let (size, _) = self.layouts.of_record(record).ok_or_else(|| format!("{}: no layout", named_key.1))?;
                if !self.integer_record(record) {
                    return Err(format!("a {} by value holding a float", named_key.1));
                }
                match size {
                    1 => Ok("u8".to_string()),
                    2 => Ok("u16".to_string()),
                    4 => Ok("u32".to_string()),
                    8 => Ok("u64".to_string()),
                    _ => Err(format!("a {} of {size} bytes by value", named_key.1)),
                }
            }
            _ => Err(format!("a {} by value", named_key.1)),
        }
    }

    /// The entry of a method a form of the glue does not carry: every parameter by its calling convention only, and
    /// what it writes: COM's null through each out-pointer to one object or pointer, zero bytes through each value
    /// out of a fixed size, nothing through arrays and bytes (no count of them is trusted) or anything else. An error
    /// names a parameter or return whose calling convention the glue does not know; the generator fails on it, for a
    /// table never holds a null entry.
    pub(super) fn refusal(&self, method: &Method) -> Result<Refusal, String> {
        let mut params = Vec::new();
        for param in &method.params {
            let entry = if let Some((inner, depth, _)) = pointee(&param.ty) {
                let only_out = param.is_out && !param.is_in;
                let counted = attribute_named(&param.attributes, "NativeArrayInfo").is_some() || attribute_named(&param.attributes, "MemorySize").is_some();
                let writes = if !only_out || counted {
                    Writes::Nothing
                } else if depth >= 2 || named(inner).is_some_and(|k| self.is_object(&k) || self.g.read.interfaces.contains_key(&k)) {
                    Writes::Null
                } else if *inner == Type::Void {
                    Writes::Nothing
                } else {
                    primitive_or_record_size(&self.layouts, inner).map_or(Writes::Nothing, |(size, _)| Writes::Zeros(size))
                };
                ("*mut u8".to_string(), writes)
            } else if named(&param.ty).is_some_and(|k| self.is_object(&k) || self.g.read.interfaces.contains_key(&k)) {
                // An interface pointer, of the set or of an interface a slice leaves out.
                ("Raw".to_string(), Writes::Nothing)
            } else if named(&param.ty).is_some_and(|k| k == key(GUID.0, GUID.1)) {
                (GUID_ABI.to_string(), Writes::Nothing)
            } else {
                (self.by_value_c(&param.ty).map_err(|why| format!("{}: {why}", param.name))?, Writes::Nothing)
            };
            params.push((param.name.clone(), entry.0, entry.1));
        }
        let (returned, answer, stated) = if named(&method.return_type).is_some_and(|k| k == key(FOUNDATION, "HRESULT")) {
            ("i32".to_string(), "wcontainers_component_object_model::constants::E_NOTIMPL.0".to_string(), "E_NOTIMPL".to_string())
        } else if method.return_type == Type::Void {
            ("()".to_string(), String::new(), "no result (nothing done)".to_string())
        } else if pointee(&method.return_type).is_some() {
            ("*mut u8".to_string(), "core::ptr::null_mut()".to_string(), "null".to_string())
        } else {
            let c = self.by_value_c(&method.return_type).map_err(|why| format!("return: {why}"))?;
            if c == "*const u8" {
                ("*mut u8".to_string(), "core::ptr::null_mut()".to_string(), "null".to_string())
            } else {
                (c, "Default::default()".to_string(), "zero".to_string())
            }
        };
        Ok(Refusal { params, returned, answer, stated })
    }

    fn glue_of(&self, method: &Method, place: &str) -> Result<Glued, String> {
        // A count the trait drops (it follows from a slice's length) is a `Count`; a count by value of arrays out only is
        // kept in the trait as a value (the room the caller asks for); a count by pointer of an array out is the room in
        // and the number written out. The same rule as the trait's (src/emit.rs `signature`).
        let mut counts: BTreeSet<usize> = BTreeSet::new();
        let mut by_pointer: BTreeSet<usize> = BTreeSet::new();
        for param in &method.params {
            let is_out = param.is_out && !param.is_in;
            for (attribute, argument) in [("NativeArrayInfo", "CountParamIndex"), ("MemorySize", "BytesParamIndex")] {
                if let Some(count) = attribute_named(&param.attributes, attribute).and_then(|a| super::integer_argument(a, argument)) {
                    let count = usize::try_from(count).map_err(|_| "a negative count index".to_string())?;
                    let count_is_pointer = method.params.get(count).is_some_and(|c| pointee(&c.ty).is_some());
                    if count_is_pointer && is_out && attribute == "NativeArrayInfo" {
                        by_pointer.insert(count);
                    } else if !(is_out && !count_is_pointer && attribute == "NativeArrayInfo") {
                        counts.insert(count);
                    }
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
            let part = if by_pointer.contains(&position) {
                if counts.contains(&position) {
                    return Err(format!("{}: a count passed by pointer of bytes or an array in", param.name));
                }
                match pointee(&param.ty) {
                    Some((inner, 1, _)) if primitive(inner) == Some("u32") => Part::CountByPointer,
                    _ => return Err(format!("{}: a count by pointer that is not a UINT", param.name)),
                }
            } else if counts.contains(&position) {
                if pointee(&param.ty).is_some() {
                    return Err(format!("{}: a count passed by pointer of bytes or an array in", param.name));
                }
                Part::Count
            } else if let Some(part) = self.exception_part(place, method, param).map_err(|why| format!("{}: {why}", param.name))? {
                part
            } else if matches!(pointee(&param.ty), Some((Type::Void, 2, _))) && attribute_named(&param.attributes, "ComOutPtr").is_some() {
                // `REFIID riid, void** ppv`: the object is of the interface the nearest GUID before it names.
                let is_guid = |p: &Param| pointee(&p.ty).is_some_and(|(inner, depth, _)| depth == 1 && named(inner).is_some_and(|k| k == key(GUID.0, GUID.1)));
                let riid = method.params[..position].iter().rposition(is_guid).ok_or_else(|| format!("{}: an interface out with no interface id before it", param.name))?;
                Part::InterfaceOut { riid }
            } else {
                self.part(method, param).map_err(|why| format!("{}: {why}", param.name))?
            };
            parts.push((name, part));
        }
        // An array out counted by pointer is asked twice of the system (its count, then its elements): only when it is
        // the method's one out-parameter, and its count names it alone.
        if parts.iter().any(|(_, part)| matches!(part, Part::CountByPointer)) {
            let outs = parts.iter().filter(|(_, part)| !matches!(part, Part::Value { .. } | Part::Count | Part::CountByPointer | Part::StructIn { .. } | Part::ValuesIn { .. } | Part::ObjectIn { .. })).count();
            let counted = parts.iter().filter(|(_, part)| matches!(part, Part::ValuesOut { by_pointer: true, .. })).count();
            if outs != 1 || counted != 1 || parts.iter().filter(|(_, part)| matches!(part, Part::CountByPointer)).count() != 1 {
                return Err("an array out counted by pointer beside other outs".to_string());
            }
        }
        let status = named(&method.return_type).is_some_and(|k| k == key(FOUNDATION, "HRESULT"));
        let returned = if status || method.return_type == Type::Void {
            None
        } else {
            Some(self.value_part(&method.return_type).map_err(|why| format!("return: {why}"))?)
        };
        let mut ranges = Vec::new();
        for exception in self.g.exceptions() {
            if let Exception::CountRange { method: m, count, most, start, keep, keeps } = exception
                && *m == place
            {
                let index = |name: &str| method.params.iter().position(|p| p.name == name);
                let Some(count) = index(count) else { return Err(format!("no count {count}")) };
                ranges.push((count, *most, start.and_then(index), *keep, keeps.iter().filter_map(|k| index(k)).collect()));
            }
        }
        Ok(Glued { parts, ranges, status, returned })
    }

    /// A parameter an exception line of src/api.rs maps, if any.
    fn exception_part(&self, place: &str, method: &Method, param: &Param) -> Result<Option<Part>, String> {
        let position = |name: &str| method.params.iter().position(|p| p.name == name).ok_or_else(|| format!("no parameter {name}"));
        if let Some(Exception::InitialData { desc, shape, per_subresource, record, bytes, pitch, slice_pitch, .. }) = self.g.initial_data(place, &param.name) {
            let record_key = self.g.read.records.keys().find(|k| k.1 == *record).cloned().ok_or("the record of an exception line")?;
            let read = &self.g.read.records[&record_key];
            let places = self.layouts.field_places(read).ok_or("the record's layout")?;
            let (size, _) = self.layouts.of_record(read).ok_or("the record's size")?;
            if size % 8 != 0 {
                return Err("a record of initial data whose size is not a whole number of words".to_string());
            }
            let at = |field: &str| read.fields.iter().position(|f| f.name == field).map(|index| places[index].0).ok_or_else(|| format!("no field {field}"));
            return Ok(Some(Part::InitialData {
                desc: position(desc)?,
                shape: *shape,
                per_subresource: *per_subresource,
                size,
                bytes_at: at(bytes)?,
                pitch_at: at(pitch)?,
                slice_at: at(slice_pitch)?,
                path: self.record_path(&record_key),
                fields: [names::value_name(bytes), names::value_name(pitch), names::value_name(slice_pitch)],
            }));
        }
        if let Some(Exception::Lent { record, resource, subresource, field, row_pitch, depth_pitch, .. }) = self.g.lent_of(place)
            && let Some((inner, 1, _)) = pointee(&param.ty)
            && named(inner).is_some_and(|k| k.1 == *record)
        {
            let record_key = named(inner).unwrap_or_default();
            let coded = self.codec_of(&record_key).ok_or("a lent record the glue cannot copy as bytes")?;
            let fields = [names::value_name(field), names::value_name(row_pitch), names::value_name(depth_pitch)];
            return Ok(Some(Part::LentOut { coded, resource: position(resource)?, subresource: position(subresource)?, fields }));
        }
        if let Some(Exception::BoundObject { name, .. }) = self.g.bound_object(place) {
            if param.name == "riid" {
                return Ok(Some(Part::Riid));
            }
            if attribute_named(&param.attributes, "ComOutPtr").is_some() {
                return Ok(Some(Part::BoundOut { name: (*name).to_string() }));
            }
        }
        Ok(None)
    }

    /// An element of an array: a value, or a structure the glue copies as bytes both ways.
    fn element_part(&self, inner: &Type) -> Result<(Part, u64), String> {
        let (size, _) = primitive_or_record_size(&self.layouts, inner).ok_or("an element of unknown size")?;
        if let Ok(value) = self.value_part(inner) {
            return Ok((value, size));
        }
        let named_key = named(inner).ok_or("an element of no name")?;
        if let Some(record) = self.g.read.records.get(&named_key)
            && matches!(record.kind, RecordKind::Struct | RecordKind::Union)
            && !Generator::is_large_integer(&named_key)
        {
            let coded = self.codec_of(&named_key).ok_or_else(|| format!("an array of {} the glue cannot copy as bytes", named_key.1))?;
            if coded.decode_only {
                return Err(format!("an array of {} holding a union, which the glue only reads", named_key.1));
            }
            return Ok((Part::StructIn { optional: false, coded }, size));
        }
        Err(format!("an array of {}", named_key.1))
    }

    fn part(&self, method: &Method, param: &Param) -> Result<Part, String> {
        let is_out = param.is_out && !param.is_in;
        let array = attribute_named(&param.attributes, "NativeArrayInfo");
        let bytes = attribute_named(&param.attributes, "MemorySize");
        let Some((inner, depth, is_const)) = pointee(&param.ty) else {
            if let Some(interface_key) = named(&param.ty).filter(|k| self.is_object(k)) {
                if interface_key == key(UNKNOWN.0, UNKNOWN.1) {
                    return Ok(Part::UnknownIn { optional: param.is_optional });
                }
                let (kinds, enum_path) = self.object_kinds(&interface_key)?;
                if kinds.is_empty() {
                    return Err(format!("no kind is an {}", interface_key.1));
                }
                return Ok(Part::ObjectIn { optional: param.is_optional, kinds, enum_path });
            }
            if named(&param.ty).is_some_and(|k| k == key(GUID.0, GUID.1)) {
                return Ok(Part::Value { c: GUID_ABI.to_string(), rust: self.g.guid_path(NOWHERE), form: ValueForm::Guid });
            }
            if let Some(named_key) = named(&param.ty)
                && self.g.read.records.get(&named_key).is_some_and(|record| record.kind == RecordKind::Typedef)
                && let form @ (Typedef::String | Typedef::WideString) = self.g.typedef_form(&named_key)
            {
                if param.is_out {
                    return Err(format!("a {} out, whose room the metadata links to no parameter", named_key.1));
                }
                return Ok(Part::TextIn { wide: matches!(form, Typedef::WideString), optional: param.is_optional });
            }
            return self.value_part(&param.ty);
        };
        if depth != 1 {
            return Err(format!("a pointer of depth {depth}"));
        }
        if let Some(array) = array {
            if let Some(length) = super::integer_argument(array, "CountConst") {
                let length = u64::try_from(length).map_err(|_| "a negative length")?;
                if named(inner).is_some_and(|k| self.is_object(&k)) {
                    return Err("a fixed array of objects".to_string());
                }
                let (element, size) = self.element_part(inner)?;
                if is_out || !is_const && param.is_out {
                    return Ok(Part::FixedOut { element: Box::new(element), length, size });
                }
                return Ok(Part::FixedIn { element: Box::new(element), length, size, optional: param.is_optional });
            }
            let count = super::integer_argument(array, "CountParamIndex").ok_or("an array whose count is neither a parameter nor a constant")?;
            let count = usize::try_from(count).map_err(|_| "a negative count")?;
            if *inner == Type::Void && !(is_out || !is_const && param.is_out) {
                // An array of void in is bytes (the trait's `&[u8]`), its count a byte count.
                return Ok(Part::ValuesIn { count, element: Box::new(bytes_element()), size: 1, optional: param.is_optional, as_option: param.is_optional });
            }
            if is_out || !is_const && param.is_out {
                let by_pointer = method.params.get(count).is_some_and(|c| pointee(&c.ty).is_some());
                if let Some(interface_key) = named(inner).filter(|k| self.is_object(k)) {
                    if by_pointer {
                        return Err("an array of objects out counted by pointer".to_string());
                    }
                    if interface_key == key(UNKNOWN.0, UNKNOWN.1) {
                        return Err("an array of IUnknown out".to_string());
                    }
                    let kinds = self.g.kinds_deriving(&interface_key);
                    let single = kinds.len() == 1 && self.g.kind_of.get(&interface_key) == kinds.first();
                    if !single {
                        return Err("an array out of objects of several kinds".to_string());
                    }
                    return Ok(Part::ObjectsOut { count, kind: kinds[0] });
                }
                if *inner == Type::Void {
                    return Err("an array of void out".to_string());
                }
                let (element, size) = self.element_part(inner)?;
                return Ok(Part::ValuesOut { count, element: Box::new(element), size, by_pointer });
            }
            if let Some(interface_key) = named(inner).filter(|k| self.is_object(k)) {
                let (kinds, enum_path) = self.object_kinds(&interface_key)?;
                if kinds.len() != 1 || enum_path.is_some() {
                    return Err("an array of objects of several kinds".to_string());
                }
                return Ok(Part::ObjectsIn { count, kind: kinds[0], optional: param.is_optional });
            }
            let (element, size) = self.element_part(inner)?;
            return Ok(Part::ValuesIn { count, element: Box::new(element), size, optional: param.is_optional, as_option: false });
        }
        if let Some(bytes) = bytes {
            if !(is_out || param.is_out && !is_const) {
                // Bytes in (the trait's `&[u8]`): copied once from C, as an array of bytes of that count.
                let count = super::integer_argument(bytes, "BytesParamIndex").ok_or("bytes whose count is not a parameter")?;
                let count = usize::try_from(count).map_err(|_| "a negative count")?;
                return Ok(Part::ValuesIn { count, element: Box::new(bytes_element()), size: 1, optional: param.is_optional, as_option: param.is_optional });
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
            let coded = Coded { size: 16, mirror: "GuidLayout".to_string(), ident: "guid".to_string(), path: self.g.guid_path(NOWHERE), decode_only: false };
            if is_out {
                return Ok(Part::StructOut { coded });
            }
            return Ok(Part::StructIn { optional: param.is_optional, coded });
        }
        if is_out {
            let element = self.value_part(inner)?;
            let (size, _) = primitive_or_record_size(&self.layouts, inner).ok_or("a value of unknown size")?;
            return Ok(Part::ValueOut { size, element: Box::new(element) });
        }
        Err("a pointer to a value in".to_string())
    }
}

/// `count * size` as text, with no multiplication by one.
fn times(count: &str, size: &str) -> String {
    if size == "1" { count.to_string() } else { format!("{count} * {size}") }
}

/// One byte of an array of bytes.
fn bytes_element() -> Part {
    Part::Value { c: "u8".to_string(), rust: "u8".to_string(), form: ValueForm::Same }
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
use core::mem::offset_of;

macro_rules! primitive_bytes {
    ($($get:ident $put:ident $type:ty;)*) => {$(
        /// Reads the value at this offset, as C lays it out (little-endian).
        pub fn $get(bytes: &[u8], at: usize) -> $type {
            let mut value = [u8::MIN; size_of::<$type>()];
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
    let mut value = [u8::MIN; wcontainers_component_object_model_direct::GUID_SIZE];
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
    get_guid(bytes, usize::MIN)
}

/// Writes a GUID as its 16 bytes.
pub fn encode_guid(value: &wcontainers_component_object_model::structures::Guid, bytes: &mut [u8]) {
    put_guid(bytes, usize::MIN, value);
}

/// A GUID as C lays it out: its 16 bytes, aligned as its first field (a 32-bit word).
#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct GuidLayout([u32; wcontainers_component_object_model_direct::GUID_SIZE / size_of::<u32>()]);
"#;

#[path = "glue_call.rs"]
mod call;
#[path = "glue_write.rs"]
mod write;
#[path = "glue_record.rs"]
mod record;
#[path = "glue_hostile.rs"]
mod hostile;
#[path = "glue_c_caller.rs"]
mod c_caller;

pub(super) use c_caller::write_c_caller;
pub(super) use hostile::{write_hostile, write_suite};
pub(super) use record::write_recorder;
pub(super) use write::write_direct;
