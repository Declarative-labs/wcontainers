//! Writes the crates of one set of APIs: each crate's interface.rs and data files, by the rows of MAPPING.md.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use windows_metadata::Type;
use windows_metadata::reader::TypeIndex;

use crate::api::{Api, Exception};
use crate::mapping::Row;
use crate::metadata::{self, Field, Interface, Literal, Method, Param, Record, RecordKind, attribute_named};
use crate::names;

#[path = "glue.rs"]
mod glue;
#[path = "kinds.rs"]
mod kinds;

/// The spike's harness, ported to the generated glue: hand-written beside the generator, copied beside the slice's
/// Direct3D 11 glue as a test-only package.
const HARNESS: &[(&str, &str)] = &[
    ("Cargo.toml", include_str!("../harness/stand-ins/Cargo.toml")),
    ("build.rs", include_str!("../harness/stand-ins/build.rs")),
    ("lib.rs", include_str!("../harness/stand-ins/lib.rs")),
    ("front.rs", include_str!("../harness/stand-ins/front.rs")),
    ("liar.rs", include_str!("../harness/stand-ins/liar.rs")),
    ("tests/game.rs", include_str!("../harness/stand-ins/tests/game.rs")),
    ("tests/attack_c.rs", include_str!("../harness/stand-ins/tests/attack_c.rs")),
    ("tests/attack_table.rs", include_str!("../harness/stand-ins/tests/attack_table.rs")),
    ("tests/check2_c.rs", include_str!("../harness/stand-ins/tests/check2_c.rs")),
    ("tests/fuzz.rs", include_str!("../harness/stand-ins/tests/fuzz.rs")),
    ("tests/measure.rs", include_str!("../harness/stand-ins/tests/measure.rs")),
    ("tests/undo.rs", include_str!("../harness/stand-ins/tests/undo.rs")),
    ("c/d3d11_slice.h", include_str!("../harness/stand-ins/c/d3d11_slice.h")),
    ("c/fake_d3d11.c", include_str!("../harness/stand-ins/c/fake_d3d11.c")),
    ("c/game.c", include_str!("../harness/stand-ins/c/game.c")),
    ("c/attack.c", include_str!("../harness/stand-ins/c/attack.c")),
    ("c/check2.c", include_str!("../harness/stand-ins/c/check2.c")),
    ("c/fuzz.c", include_str!("../harness/stand-ins/c/fuzz.c")),
];

/// The hostile tests of the whole APIs' rows and the safe-Rust probes, hand-written beside the generator, copied beside
/// Direct3D 11's recorder (they reach it through both glues).
const RECORDER_TESTS: &[(&str, &str)] = &[("hostile.rs", include_str!("../harness/whole/hostile.rs"))];

pub const HEADER: &str = "Written by tools/api-crate-generator from the Win32 metadata, version 63.0.31, and never by hand.";
const UNKNOWN: (&str, &str) = ("Windows.Win32.System.Com", "IUnknown");
const FOUNDATION: &str = "Windows.Win32.Foundation";
const GUID: (&str, &str) = ("System", "Guid");
/// COM's status codes the glue itself answers with, from Windows.Win32.Foundation.
const STATUS_CODES: &[&str] =
    &["S_OK", "S_FALSE", "E_FAIL", "E_INVALIDARG", "E_OUTOFMEMORY", "E_NOTIMPL", "E_NOINTERFACE", "E_POINTER"];
const SHARED_ERROR_REASON: &str =
    "every method of a COM interface reports failure as an HRESULT, one to one with the API (MAPPING.md)";

type Key = (String, String);

fn key(namespace: &str, name: &str) -> Key {
    (namespace.to_string(), name.to_string())
}

struct Kind {
    api: usize,
    name: String,
    root: Key,
    newest: Vec<Key>,
    /// Every interface an object of this kind is: its versions and all their bases.
    ancestors: BTreeSet<Key>,
}

#[derive(Clone, Copy)]
enum Module {
    Structures,
    Enumerations,
    Constants,
}

impl Module {
    fn file(self) -> &'static str {
        match self {
            Module::Structures => "structures",
            Module::Enumerations => "enumerations",
            Module::Constants => "constants",
        }
    }
}

/// Where a rendering happens: the crate, and how the API's top-level trait is reached from there.
#[derive(Clone, Copy)]
struct At<'s> {
    api: usize,
    /// `Self::Api` inside an interface trait, `Self` inside a top-level trait, `A` inside generic data.
    api_type: &'s str,
}

enum Typedef {
    Bool,
    Status,
    String,
    WideString,
    Newtype,
}

/// The named length of each array of a count the header fixes, by name: length, C name, link.
type NamedLengths = BTreeMap<String, (i128, String, Option<String>)>;

struct Generator<'a> {
    apis: &'a [&'a Api],
    read: metadata::Namespaces,
    interface_api: BTreeMap<Key, usize>,
    record_api: RefCell<BTreeMap<Key, usize>>,
    record_names: RefCell<HashMap<Key, String>>,
    kinds: Vec<Kind>,
    kind_of: HashMap<Key, usize>,
    trait_names: HashMap<Key, String>,
    rows: RefCell<BTreeMap<Row, usize>>,
    unmapped: RefCell<Vec<String>>,
    needs_ref: RefCell<BTreeSet<Key>>,
    needs_object: RefCell<BTreeSet<Key>>,
    needs_unknown: RefCell<BTreeSet<usize>>,
    generic_memo: RefCell<HashMap<Key, bool>>,
    /// Root interfaces whose subtree's methods never name `Self::Api`, written without that associated type.
    apiless: RefCell<BTreeSet<Key>>,
    /// Per crate, the named length of each array of a count the header fixes: name, length, C name, link.
    lengths: RefCell<BTreeMap<usize, NamedLengths>>,
    /// Every interface's own methods as the metadata has them, before a slice: the C tables keep every entry.
    all_methods: BTreeMap<Key, Vec<Method>>,
    /// Each method's kinds and treatment (src/kinds.rs), once classified.
    classified: RefCell<Option<kinds::Classified>>,
    /// Every ask of the other party the emitted functions make, by value (src/trust.rs).
    asked: RefCell<crate::trust::Ledger>,
}

fn alias(api: &Api) -> String {
    api.folder.replace('-', "_")
}

fn package(api: &Api) -> String {
    format!("wcontainers-{}", api.folder)
}

/// Where every value the generator writes comes from, for a constant Microsoft documents on no page of its own.
const METADATA_LINK: &str = "https://www.nuget.org/packages/microsoft.windows.sdk.win32metadata/63.0.31";

/// One sentence naming the C item, with Microsoft's page for it, or the metadata's package for a value.
fn doc_line(indent: &str, c_name: &str, link: Option<&str>) -> String {
    match link {
        Some(link) => format!("{indent}/// The header's `{c_name}`, [as Microsoft documents it]({link}).\n"),
        None => format!("{indent}/// The header's `{c_name}`, [as the metadata gives it]({METADATA_LINK}).\n"),
    }
}

fn documentation(attributes: &[metadata::Attribute]) -> Option<String> {
    let attribute = attribute_named(attributes, "Documentation")?;
    match attribute.arguments.first() {
        Some((_, Literal::Text(link))) => Some(link.clone()),
        _ => None,
    }
}

fn integer_argument(attribute: &metadata::Attribute, name: &str) -> Option<i128> {
    attribute.arguments.iter().find_map(|(argument, value)| match value {
        Literal::Integer(value) if argument == name => Some(*value),
        _ => None,
    })
}

fn text_argument(attribute: &metadata::Attribute, name: &str) -> Option<String> {
    attribute.arguments.iter().find_map(|(argument, value)| match value {
        Literal::Text(value) if argument == name => Some(value.clone()),
        _ => None,
    })
}

fn primitive(ty: &Type) -> Option<&'static str> {
    Some(match ty {
        Type::Bool => "bool",
        Type::Char => "u16",
        Type::I8 => "i8",
        Type::U8 => "u8",
        Type::I16 => "i16",
        Type::U16 => "u16",
        Type::I32 => "i32",
        Type::U32 => "u32",
        Type::I64 => "i64",
        Type::U64 => "u64",
        Type::F32 => "f32",
        Type::F64 => "f64",
        Type::ISize => "isize",
        Type::USize => "usize",
        _ => return None,
    })
}

fn named(ty: &Type) -> Option<Key> {
    match ty {
        Type::Name(name) => Some(key(&name.namespace, &name.name)),
        _ => None,
    }
}

fn pointee(ty: &Type) -> Option<(&Type, usize, bool)> {
    match ty {
        Type::PtrConst(inner, depth) => Some((inner, *depth, true)),
        Type::PtrMut(inner, depth) => Some((inner, *depth, false)),
        _ => None,
    }
}

fn literal_text(value: &Literal, rust_type: &str) -> String {
    match value {
        Literal::Integer(value) if *value < 0 => format!("{value}"),
        Literal::Integer(value) if *value > 0xff => format!("{value:#x}"),
        Literal::Integer(value) => format!("{value}"),
        Literal::Float(value) if rust_type == "f32" => format!("{:?}_f32", *value as f32),
        Literal::Float(value) => format!("{value:?}"),
        Literal::Text(text) => format!("{text:?}"),
        Literal::Bool(value) => format!("{value}"),
    }
}

/// The Rust name of a record nested in another: the outer record's Rust name and the member's name.
fn nested_name(outer: &str, nested: &str) -> String {
    let cleaned = nested.trim_start_matches('_').trim_end_matches("_e__Union").trim_end_matches("_e__Struct");
    format!("{outer}{}", names::pascal_case_of_upper_snake(cleaned))
}

/// The interfaces a type names, through pointers.
fn named_interface(ty: &Type, read: &metadata::Namespaces) -> Option<Key> {
    let inner = pointee(ty).map_or(ty, |(inner, _, _)| inner);
    named(inner).filter(|k| read.interfaces.contains_key(k))
}

/// Cuts each sliced API down to the methods and exports it names, and the interfaces those need: the interfaces
/// that hold them, the ones their parameters name, and every base of those.
fn slice(apis: &[&Api], read: &mut metadata::Namespaces, interface_api: &mut BTreeMap<Key, usize>) {
    if apis.iter().all(|api| api.only.is_empty()) {
        return;
    }
    let sliced = |position: usize| !apis[position].only.is_empty();
    for (interface_key, &position) in interface_api.iter() {
        if !sliced(position) {
            continue;
        }
        let only = apis[position].only;
        if let Some(interface) = read.interfaces.get_mut(interface_key) {
            interface.methods.retain(|method| only.contains(&format!("{}::{}", interface_key.1, method.name).as_str()));
        }
    }
    let mut keep: BTreeSet<Key> = BTreeSet::new();
    let mut pending: Vec<Key> = Vec::new();
    for (interface_key, &position) in interface_api.iter() {
        let interface = &read.interfaces[interface_key];
        if !sliced(position) || !interface.methods.is_empty() {
            pending.push(interface_key.clone());
        }
    }
    for ((namespace, _), function) in &read.functions {
        let owner = apis.iter().position(|api| api.namespaces.contains(&namespace.as_str()));
        if owner.is_some_and(|position| sliced(position) && apis[position].only.contains(&function.name.as_str())) {
            pending.extend(function.params.iter().filter_map(|param| named_interface(&param.ty, read)));
        }
    }
    while let Some(interface_key) = pending.pop() {
        if !keep.insert(interface_key.clone()) {
            continue;
        }
        let Some(interface) = read.interfaces.get(&interface_key) else { continue };
        pending.extend(interface.base.iter().cloned());
        for method in &interface.methods {
            pending.extend(method.params.iter().filter_map(|param| named_interface(&param.ty, read)));
        }
    }
    interface_api.retain(|interface_key, position| !sliced(*position) || keep.contains(interface_key));
}

impl<'a> Generator<'a> {
    fn new(index: &TypeIndex, apis: &'a [&'a Api]) -> Generator<'a> {
        let mut namespaces: Vec<&str> = vec![UNKNOWN.0, FOUNDATION];
        for api in apis {
            namespaces.extend(api.namespaces);
            namespaces.extend(api.referenced_namespaces);
        }
        namespaces.sort_unstable();
        namespaces.dedup();
        let mut read = metadata::read(index, &namespaces);
        returned_structures_through_pointers(&mut read);
        let mut interface_api = BTreeMap::new();
        let mut record_api = BTreeMap::new();
        for (position, api) in apis.iter().enumerate() {
            for (interface_key, interface) in &read.interfaces {
                if api.namespaces.contains(&interface.namespace.as_str()) && !api.left_out.contains(&interface.name.as_str()) {
                    interface_api.insert(interface_key.clone(), position);
                }
            }
            for (record_key, record) in &read.records {
                if api.namespaces.contains(&record.namespace.as_str()) && record.kind != RecordKind::Delegate {
                    record_api.insert(record_key.clone(), position);
                }
            }
        }
        let mut read = read;
        let all_methods: BTreeMap<Key, Vec<Method>> = read.interfaces.iter().map(|(k, i)| (k.clone(), i.methods.clone())).collect();
        slice(apis, &mut read, &mut interface_api);
        let mut trait_names = HashMap::new();
        for (interface_key, &position) in &interface_api {
            let api = apis[position];
            trait_names.insert(interface_key.clone(), names::strip_first(&interface_key.1, api.interface_prefixes).to_string());
        }
        let mut generator = Generator {
            apis,
            read,
            interface_api,
            record_api: RefCell::new(record_api),
            record_names: RefCell::new(HashMap::new()),
            kinds: Vec::new(),
            kind_of: HashMap::new(),
            trait_names,
            rows: RefCell::new(BTreeMap::new()),
            unmapped: RefCell::new(Vec::new()),
            needs_ref: RefCell::new(BTreeSet::new()),
            needs_object: RefCell::new(BTreeSet::new()),
            needs_unknown: RefCell::new(BTreeSet::new()),
            generic_memo: RefCell::new(HashMap::new()),
            apiless: RefCell::new(BTreeSet::new()),
            lengths: RefCell::new(BTreeMap::new()),
            all_methods,
            classified: RefCell::new(None),
            asked: RefCell::new(crate::trust::Ledger::default()),
        };
        generator.find_kinds();
        generator
    }

    fn is_version(&self, interface: &Interface) -> bool {
        let Some(base) = &interface.base else { return false };
        if !self.interface_api.contains_key(base) {
            return false;
        }
        let trimmed = interface.name.trim_end_matches(|c: char| c.is_ascii_digit());
        trimmed.len() < interface.name.len() && trimmed == base.1.trim_end_matches(|c: char| c.is_ascii_digit())
    }

    fn root_of(&self, interface_key: &Key) -> Key {
        let mut current = interface_key.clone();
        while let Some(interface) = self.read.interfaces.get(&current) {
            if !self.is_version(interface) {
                break;
            }
            match &interface.base {
                Some(base) => current = base.clone(),
                None => break,
            }
        }
        current
    }

    fn bases_of(&self, interface_key: &Key) -> Vec<Key> {
        let mut chain = vec![interface_key.clone()];
        let mut current = interface_key.clone();
        while let Some(base) = self.read.interfaces.get(&current).and_then(|interface| interface.base.clone()) {
            chain.push(base.clone());
            current = base;
        }
        chain
    }

    /// The first interface of a chain of bases that is in one of the set's crates.
    fn chain_root(&self, interface_key: &Key) -> Key {
        self.bases_of(interface_key).into_iter().filter(|base| self.interface_api.contains_key(base)).last().unwrap_or_else(|| interface_key.clone())
    }

    fn api_binding(&self, interface_key: &Key) -> &'static str {
        if self.apiless.borrow().contains(&self.chain_root(interface_key)) { "" } else { "<Api = Self>" }
    }

    /// Names the length of an array whose count the header fixes, as a constant of the crate.
    fn length_constant(&self, at: At<'_>, method: &Method, param: &Param, length: i128, place: &str) -> String {
        let owner = place.split("::").next().unwrap_or("");
        let trait_name = self.trait_names.iter().find(|(k, _)| k.1 == owner).map(|(_, name)| name.clone()).unwrap_or_default();
        let name = format!(
            "{}_{}_{}_LENGTH",
            names::snake_case(&trait_name).to_ascii_uppercase(),
            names::snake_case(&method.name).to_ascii_uppercase(),
            names::value_name(&param.name).trim_end_matches('_').to_ascii_uppercase()
        )
        .trim_start_matches('_')
        .to_string();
        let c_name = format!("{}({})", place.split('(').next().unwrap_or(place), param.name);
        self.lengths.borrow_mut().entry(at.api).or_default().insert(name.clone(), (length, c_name, documentation(&method.attributes)));
        format!("crate::constants::{name}")
    }

    fn find_kinds(&mut self) {
        let mut roots: BTreeMap<Key, Vec<Key>> = BTreeMap::new();
        for interface_key in self.interface_api.keys() {
            let api = self.apis[self.interface_api[interface_key]];
            let root = self.root_of(interface_key);
            if api.abstract_bases.contains(&root.1.as_str()) {
                continue;
            }
            roots.entry(root).or_default().push(interface_key.clone());
        }
        for (root, members) in roots {
            let newest: Vec<Key> = members
                .iter()
                .filter(|member| {
                    !members.iter().any(|other| {
                        self.read.interfaces.get(other).is_some_and(|o| o.base.as_ref() == Some(*member) && self.is_version(o))
                    })
                })
                .cloned()
                .collect();
            let mut ancestors = BTreeSet::new();
            for member in &newest {
                ancestors.extend(self.bases_of(member));
            }
            let index = self.kinds.len();
            for member in &members {
                self.kind_of.insert(member.clone(), index);
            }
            self.kinds.push(Kind {
                api: self.interface_api[&root],
                name: self.trait_names[&root].clone(),
                root,
                newest,
                ancestors,
            });
        }
    }

    fn count(&self, row: Row) {
        *self.rows.borrow_mut().entry(row).or_default() += 1;
    }

    fn unmapped(&self, row: Row, what: String) {
        self.count(row);
        self.unmapped.borrow_mut().push(what);
    }

    fn top_path(&self, api: usize, from: usize) -> String {
        let top = self.apis[api].top_trait.unwrap_or("Unknown");
        if api == from { format!("crate::{top}") } else { format!("{}::{top}", alias(self.apis[api])) }
    }

    fn trait_path(&self, interface_key: &Key, from: usize) -> String {
        let api = self.interface_api[interface_key];
        let name = &self.trait_names[interface_key];
        if api == from { format!("crate::{name}") } else { format!("{}::{name}", alias(self.apis[api])) }
    }

    fn kind_type(&self, kind: usize, at: At<'_>) -> String {
        let kind = &self.kinds[kind];
        format!("<{} as {}>::{}", at.api_type, self.top_path(kind.api, at.api), kind.name)
    }

    /// The type of an `IUnknown*` of no kind of the set: COM's `Foreign`.
    fn unknown_type(&self, at: At<'_>) -> String {
        self.data_path(0, Module::Structures, "Foreign", at.api)
    }

    /// Every exception line of the set's APIs.
    fn exceptions(&self) -> impl Iterator<Item = &'static Exception> + '_ {
        self.apis.iter().flat_map(|api| api.exceptions.iter())
    }

    /// The holds of every pin of `method` in the set's APIs (src/api.rs).
    fn holds_of(&self, method: &str) -> Vec<&'static crate::api::Hold> {
        self.apis.iter().flat_map(|api| api.pins.iter()).filter(|pin| pin.method == method).map(|pin| &pin.hold).collect()
    }

    /// The calling glue's treatment of a method (src/kinds.rs), once classified.
    fn treatment(&self, method: &str) -> Option<kinds::Treatment> {
        self.classified.borrow().as_ref().and_then(|classified| classified.treatments.get(method).cloned())
    }

    /// The checks of enumeration values on a method's parameters passed by value (src/kinds.rs).
    fn value_rules(&self, method: &str) -> Vec<kinds::ValueRule> {
        self.classified.borrow().as_ref().and_then(|classified| classified.rules.get(method).cloned()).unwrap_or_default()
    }

    /// The initial-data exception line of a method's parameter, if any.
    fn initial_data(&self, method: &str, param: &str) -> Option<&'static Exception> {
        self.exceptions().find(|e| matches!(e, Exception::InitialData { method: m, param: p, .. } if *m == method && *p == param))
    }

    /// The lent-memory exception line of a method, if any.
    fn lent_of(&self, method: &str) -> Option<&'static Exception> {
        self.exceptions().find(|e| matches!(e, Exception::Lent { method: m, .. } if *m == method))
    }

    /// The lent-memory exception line whose record is this one, if any.
    fn lent_record(&self, record: &str) -> Option<&'static Exception> {
        self.exceptions().find(|e| matches!(e, Exception::Lent { record: r, .. } if *r == record))
    }

    /// Whether `record.field` is the read-only bytes of an initial-data exception line.
    fn initial_bytes_field(&self, record: &str, field: &str) -> bool {
        self.exceptions().any(|e| matches!(e, Exception::InitialData { record: r, bytes, .. } if *r == record && *bytes == field))
    }

    /// The count-range exception line of a method's count parameter, if any.
    fn count_range(&self, method: &str, count: &str) -> Option<&'static Exception> {
        self.exceptions().find(|e| matches!(e, Exception::CountRange { method: m, count: c, .. } if *m == method && *c == count))
    }

    /// The status a method answers while what it is asked for is not ready (a not-yet exception line), if any.
    fn not_yet_of(&self, method: &str) -> Option<i32> {
        self.exceptions().find_map(|e| match e {
            Exception::NotYet { method: m, not_yet } if *m == method => Some(*not_yet),
            _ => None,
        })
    }

    /// The upload exception line of a method whose bytes are this parameter, if any.
    fn upload_of(&self, method: &str, param: &str) -> Option<&'static Exception> {
        self.exceptions().find(|e| matches!(e, Exception::Upload { method: m, bytes, .. } if *m == method && *bytes == param))
    }

    /// The bound-object exception line of a method, if any.
    fn bound_object(&self, method: &str) -> Option<&'static Exception> {
        self.exceptions().find(|e| matches!(e, Exception::BoundObject { method: m, .. } if *m == method))
    }

    /// The kinds an interface pointer of this type may point at.
    fn kinds_deriving(&self, interface_key: &Key) -> Vec<usize> {
        (0..self.kinds.len()).filter(|&kind| self.kinds[kind].ancestors.contains(interface_key)).collect()
    }

    fn data_path(&self, api: usize, module: Module, name: &str, from: usize) -> String {
        if api == from {
            format!("crate::{}::{name}", module.file())
        } else {
            format!("{}::{}::{name}", alias(self.apis[api]), module.file())
        }
    }

    fn guid_path(&self, from: usize) -> String {
        self.data_path(0, Module::Structures, "Guid", from)
    }

    fn status_path(&self, from: usize) -> String {
        self.data_path(0, Module::Structures, "HResult", from)
    }

    /// The crate a record goes to: its own API's, or the first crate (COM's) for one met in another namespace.
    fn record_owner(&self, record_key: &Key) -> usize {
        if let Some(&api) = self.record_api.borrow().get(record_key) {
            return api;
        }
        self.record_api.borrow_mut().insert(record_key.clone(), 0);
        0
    }

    fn record_name(&self, record_key: &Key) -> String {
        if let Some(name) = self.record_names.borrow().get(record_key) {
            return name.clone();
        }
        let owner = self.record_owner(record_key);
        let api = self.apis[owner];
        let stripped = names::strip_first(&record_key.1, api.type_prefixes);
        // D3D11_1_...: the prefix less leaves a number first, so the whole name is kept.
        let stripped = if stripped.starts_with(|c: char| c.is_ascii_digit()) { record_key.1.as_str() } else { stripped };
        let mut name = if stripped.contains('_') || stripped.chars().all(|c| !c.is_ascii_lowercase()) {
            names::pascal_case_of_upper_snake(stripped)
        } else {
            stripped.to_string()
        };
        let record_kind = self.read.records.get(record_key).map(|record| record.kind);
        let clashes = |candidate: &str| {
            self.trait_names.iter().any(|(interface, trait_name)| self.interface_api[interface] == owner && trait_name == candidate)
                || api.top_trait == Some(candidate)
                || self
                    .record_names
                    .borrow()
                    .iter()
                    .any(|(other, other_name)| other != record_key && other_name == candidate && self.record_api.borrow().get(other) == Some(&owner))
                || matches!(candidate, "Guid" | "HResult" | "Foreign")
        };
        if clashes(&name) {
            name.push_str(if record_kind == Some(RecordKind::Enum) { "Kind" } else { "Data" });
        }
        if clashes(&name) {
            name = names::pascal_case_of_upper_snake(&record_key.1);
        }
        self.record_names.borrow_mut().insert(record_key.clone(), name.clone());
        name
    }

    fn typedef_form(&self, record_key: &Key) -> Typedef {
        match (record_key.0.as_str(), record_key.1.as_str()) {
            (FOUNDATION, "BOOL") => Typedef::Bool,
            (FOUNDATION, "HRESULT") => Typedef::Status,
            (FOUNDATION, "PSTR" | "PCSTR") => Typedef::String,
            (FOUNDATION, "PWSTR" | "PCWSTR") => Typedef::WideString,
            _ => Typedef::Newtype,
        }
    }

    fn is_large_integer(record_key: &Key) -> bool {
        record_key.0 == FOUNDATION && (record_key.1 == "LARGE_INTEGER" || record_key.1 == "ULARGE_INTEGER")
    }

    /// Whether a record holds an object, so that it is generic over the API.
    fn record_is_generic(&self, record_key: &Key) -> bool {
        if let Some(&memo) = self.generic_memo.borrow().get(record_key) {
            return memo;
        }
        self.generic_memo.borrow_mut().insert(record_key.clone(), false);
        let generic = self.read.records.get(record_key).is_some_and(|record| self.fields_generic(record));
        self.generic_memo.borrow_mut().insert(record_key.clone(), generic);
        generic
    }

    fn fields_generic(&self, record: &Record) -> bool {
        record.fields.iter().any(|field| self.type_generic(&field.ty, record))
            || record.nested.iter().any(|nested| self.fields_generic(nested))
    }

    fn type_generic(&self, ty: &Type, outer: &Record) -> bool {
        match ty {
            Type::Name(name) if name.namespace.is_empty() => {
                outer.nested.iter().find(|nested| nested.name == name.name).is_some_and(|nested| self.fields_generic(nested))
            }
            Type::Name(_) => {
                let named_key = named(ty).unwrap_or_default();
                self.kind_of.contains_key(&named_key)
                    || self.interface_api.contains_key(&named_key)
                    // An IUnknown field is the API's `UnknownObject`, generic over the API as well.
                    || named_key == key(UNKNOWN.0, UNKNOWN.1)
                    || (self.read.records.get(&named_key).is_some_and(|r| r.kind == RecordKind::Struct || r.kind == RecordKind::Union)
                        && self.record_is_generic(&named_key))
            }
            Type::ArrayFixed(inner, _) => self.type_generic(inner, outer),
            Type::PtrConst(inner, 1) | Type::PtrMut(inner, 1) => self.type_generic(inner, outer),
            _ => false,
        }
    }

    /// A value of a type, in a field or passed by value.
    fn value(&self, ty: &Type, at: At<'_>, outer: Option<(&Record, &str)>, place: &str) -> String {
        if let Some(rust) = primitive(ty) {
            self.count(Row::Scalar);
            return rust.to_string();
        }
        match ty {
            Type::ArrayFixed(inner, length) => {
                self.count(Row::FixedArrayField);
                let c_name = place.rsplit('.').take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("::");
                let mut name: String = c_name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
                while name.contains("__") {
                    name = name.replace("__", "_");
                }
                let name = format!("{}_LENGTH", name.trim_matches('_'));
                let element = self.value(inner, at, outer, &format!("{place}Element"));
                let length = i128::try_from(*length).unwrap_or_default();
                self.lengths.borrow_mut().entry(at.api).or_default().insert(name.clone(), (length, c_name, None));
                format!("[{element}; crate::constants::{name}]")
            }
            Type::Name(name) if name.namespace.is_empty() => {
                let Some((outer, outer_name)) = outer else {
                    self.unmapped(Row::Unmapped, format!("{place}: nested type {} outside its record", name.name));
                    return "u8".to_string();
                };
                let generic = outer.nested.iter().find(|nested| nested.name == name.name).is_some_and(|nested| self.fields_generic(nested));
                if outer.nested.iter().any(|nested| nested.name == name.name && nested.kind == RecordKind::Union) {
                    self.count(Row::UnionField);
                } else {
                    self.count(Row::StructureByValue);
                }
                let path = self.data_path(at.api, Module::Structures, &nested_name(outer_name, &name.name), at.api);
                if generic { format!("{path}<{}>", at.api_type) } else { path }
            }
            Type::Name(_) => {
                let named_key = named(ty).unwrap_or_default();
                if named_key == key(GUID.0, GUID.1) {
                    self.count(Row::Guid);
                    return self.guid_path(at.api);
                }
                if self.interface_api.contains_key(&named_key) || named_key == key(UNKNOWN.0, UNKNOWN.1) {
                    self.count(Row::ObjectField);
                    return format!("Option<{}>", self.object_owned(&named_key, at, place));
                }
                if Self::is_large_integer(&named_key) {
                    self.count(Row::LargeInteger);
                    return "i64".to_string();
                }
                let Some(record) = self.read.records.get(&named_key) else {
                    self.unmapped(Row::Unmapped, format!("{place}: type {}.{} not in the metadata read", named_key.0, named_key.1));
                    return "u8".to_string();
                };
                match record.kind {
                    RecordKind::Typedef => match self.typedef_form(&named_key) {
                        Typedef::Bool => {
                            self.count(Row::Bool);
                            "bool".to_string()
                        }
                        Typedef::Status => {
                            self.count(Row::StatusResult);
                            self.status_path(at.api)
                        }
                        Typedef::String => {
                            self.count(Row::StringField);
                            "std::ffi::CString".to_string()
                        }
                        Typedef::WideString => {
                            self.count(Row::StringField);
                            "Vec<u16>".to_string()
                        }
                        Typedef::Newtype => {
                            self.count(Row::Handle);
                            let owner = self.record_owner(&named_key);
                            self.data_path(owner, Module::Structures, &self.record_name(&named_key), at.api)
                        }
                    },
                    RecordKind::Enum => {
                        self.count(Row::Enumeration);
                        let owner = self.record_owner(&named_key);
                        self.data_path(owner, Module::Enumerations, &self.record_name(&named_key), at.api)
                    }
                    RecordKind::Struct | RecordKind::Union => {
                        self.count(Row::StructureByValue);
                        let owner = self.record_owner(&named_key);
                        let path = self.data_path(owner, Module::Structures, &self.record_name(&named_key), at.api);
                        if self.record_is_generic(&named_key) { format!("{path}<{}>", at.api_type) } else { path }
                    }
                    RecordKind::Delegate => {
                        self.unmapped(Row::Unmapped, format!("{place}: function pointer {}", named_key.1));
                        "usize".to_string()
                    }
                }
            }
            _ => {
                self.unmapped(Row::Unmapped, format!("{place}: {ty:?}"));
                "u8".to_string()
            }
        }
    }

    fn object_owned(&self, interface_key: &Key, at: At<'_>, place: &str) -> String {
        if *interface_key == key(UNKNOWN.0, UNKNOWN.1) {
            self.needs_unknown.borrow_mut().insert(at.api);
            return format!("{}<{}>", self.unknown_object_path(at.api, at.api), at.api_type);
        }
        let deriving = self.kinds_deriving(interface_key);
        match (deriving.as_slice(), self.kind_of.get(interface_key)) {
            ([only], Some(own)) if only == own => self.kind_type(*only, at),
            ([], _) => {
                self.unmapped(Row::Unmapped, format!("{place}: no object kind is an {}", interface_key.1));
                "()".to_string()
            }
            _ => {
                self.needs_object.borrow_mut().insert(interface_key.clone());
                format!("{}<{}>", self.enum_path(interface_key, "Object", at.api), at.api_type)
            }
        }
    }

    fn object_borrowed(&self, interface_key: &Key, at: At<'_>, place: &str) -> (String, Row) {
        if *interface_key == key(UNKNOWN.0, UNKNOWN.1) {
            self.needs_unknown.borrow_mut().insert(at.api);
            return (format!("{}<'_, {}>", self.unknown_ref_path(at.api, at.api), at.api_type), Row::UnknownIn);
        }
        let deriving = self.kinds_deriving(interface_key);
        match (deriving.as_slice(), self.kind_of.get(interface_key)) {
            ([only], Some(own)) if only == own => (format!("&{}", self.kind_type(*only, at)), Row::ObjectIn),
            ([], _) => {
                self.unmapped(Row::Unmapped, format!("{place}: no object kind is an {}", interface_key.1));
                ("()".to_string(), Row::Unmapped)
            }
            _ => {
                self.needs_ref.borrow_mut().insert(interface_key.clone());
                (format!("{}<'_, {}>", self.enum_path(interface_key, "Ref", at.api), at.api_type), Row::BaseObjectIn)
            }
        }
    }

    fn enum_path(&self, interface_key: &Key, suffix: &str, from: usize) -> String {
        let api = self.interface_api[interface_key];
        let name = format!("{}{suffix}", self.trait_names[interface_key]);
        if api == from { format!("crate::{name}") } else { format!("{}::{name}", alias(self.apis[api])) }
    }

    fn unknown_ref_path(&self, api: usize, from: usize) -> String {
        if api == from { "crate::UnknownRef".to_string() } else { format!("{}::UnknownRef", alias(self.apis[api])) }
    }

    fn unknown_object_path(&self, api: usize, from: usize) -> String {
        if api == from { "crate::UnknownObject".to_string() } else { format!("{}::UnknownObject", alias(self.apis[api])) }
    }

    fn element_in(&self, ty: &Type, at: At<'_>, place: &str) -> String {
        if let Some(interface_key) = named(ty).filter(|k| self.interface_api.contains_key(k) || *k == key(UNKNOWN.0, UNKNOWN.1)) {
            let (borrowed, _) = self.object_borrowed(&interface_key, at, place);
            return format!("Option<{borrowed}>");
        }
        self.value(ty, at, None, place)
    }

    fn element_out(&self, ty: &Type, at: At<'_>, place: &str) -> String {
        if let Some(interface_key) = named(ty).filter(|k| self.interface_api.contains_key(k) || *k == key(UNKNOWN.0, UNKNOWN.1)) {
            return format!("Option<{}>", self.object_owned(&interface_key, at, place));
        }
        self.value(ty, at, None, place)
    }

    /// One method's signature after `&self`, by the rows of MAPPING.md.
    fn signature(&self, method: &Method, at: At<'_>, place: &str) -> (Vec<(String, String)>, String) {
        let mut dropped: HashSet<usize> = HashSet::new();
        let mut returned_counts: HashSet<usize> = HashSet::new();
        for param in &method.params {
            let is_out = param.is_out && !param.is_in;
            for (attribute, argument) in [("NativeArrayInfo", "CountParamIndex"), ("MemorySize", "BytesParamIndex")] {
                let Some(count) = attribute_named(&param.attributes, attribute).and_then(|a| integer_argument(a, argument)) else {
                    continue;
                };
                let Ok(count) = usize::try_from(count) else { continue };
                let Some(count_param) = method.params.get(count) else { continue };
                let count_is_pointer = pointee(&count_param.ty).is_some();
                let keep = is_out && !count_is_pointer && attribute == "NativeArrayInfo";
                if !keep {
                    dropped.insert(count);
                }
                if is_out && count_is_pointer && attribute == "MemorySize" {
                    returned_counts.insert(count);
                }
            }
        }
        let mut inputs: Vec<(String, String)> = Vec::new();
        let mut outputs: Vec<String> = Vec::new();
        let mut used_names: HashSet<String> = HashSet::new();
        let bound = self.bound_object(place);
        for (position, param) in method.params.iter().enumerate() {
            let method_place = place;
            let place = format!("{place}({})", param.name);
            if dropped.contains(&position) {
                self.count(Row::CountDropped);
                continue;
            }
            if bound.is_some() && param.name == "riid" {
                // The glue asks the object for the interface itself.
                self.count(Row::ExceptionLine);
                continue;
            }
            if let Some(passed) = self.exception_parameter(method_place, param, at) {
                self.count(Row::ExceptionLine);
                match passed {
                    Passed::In(rust) => inputs.push((names::value_name(&param.name), rust)),
                    Passed::Out(rust) => outputs.push(rust),
                    Passed::InAndOut(rust_in, rust_out) => {
                        inputs.push((names::value_name(&param.name), rust_in));
                        outputs.push(rust_out);
                    }
                }
                continue;
            }
            let mut name = names::value_name(&param.name);
            while !used_names.insert(name.clone()) {
                name.push('_');
            }
            match self.parameter(method, param, at, &place, &returned_counts) {
                Passed::In(rust) => inputs.push((name, rust)),
                Passed::Out(rust) => outputs.push(rust),
                Passed::InAndOut(rust_in, rust_out) => {
                    inputs.push((name, rust_in));
                    outputs.push(rust_out);
                }
            }
        }
        let status = named(&method.return_type).is_some_and(|k| k == key(FOUNDATION, "HRESULT"));
        let mut returned: Vec<String> = Vec::new();
        if !status && method.return_type != Type::Void {
            returned.push(self.value(&method.return_type, at, None, &format!("{place} return")));
        }
        returned.extend(outputs);
        let value = match returned.len() {
            0 => "()".to_string(),
            1 => returned.remove(0),
            _ => format!("({})", returned.join(", ")),
        };
        // A not-yet exception line: `true` for S_OK, `false` for its status of success (nothing written).
        let value = if status && value == "()" && self.not_yet_of(place).is_some() {
            "bool".to_string()
        } else {
            value
        };
        let result = if status {
            self.count(Row::StatusResult);
            format!("Result<{value}, {}>", self.status_path(at.api))
        } else {
            value
        };
        (inputs, result)
    }

    /// A parameter an exception line of src/api.rs maps, if any.
    fn exception_parameter(&self, method: &str, param: &Param, at: At<'_>) -> Option<Passed> {
        if self.upload_of(method, &param.name).is_some() {
            return Some(Passed::In("&[u8]".to_string()));
        }
        if let Some(Exception::InitialData { per_subresource: true, record, .. }) = self.initial_data(method, &param.name) {
            let record_key = self.read.records.keys().find(|k| k.1 == *record)?.clone();
            let path = self.data_path(self.record_owner(&record_key), Module::Structures, &self.record_name(&record_key), at.api);
            return Some(Passed::In(format!("&[{path}]")));
        }
        if let Some(Exception::Lent { record, .. }) = self.lent_of(method) {
            let (inner, _, _) = pointee(&param.ty)?;
            let record_key = named(inner).filter(|k| k.1 == *record)?;
            let path = self.data_path(self.record_owner(&record_key), Module::Structures, &self.record_name(&record_key), at.api);
            return Some(Passed::Out(format!("{path}<<{} as {}>::Mapped>", at.api_type, self.top_path(at.api, at.api))));
        }
        if let Some(Exception::BoundObject { name, .. }) = self.bound_object(method)
            && attribute_named(&param.attributes, "ComOutPtr").is_some()
        {
            return Some(Passed::Out(format!("<{} as {}>::{name}", at.api_type, self.top_path(at.api, at.api))));
        }
        None
    }

    fn parameter(&self, method: &Method, param: &Param, at: At<'_>, place: &str, returned_counts: &HashSet<usize>) -> Passed {
        let array = attribute_named(&param.attributes, "NativeArrayInfo");
        let bytes = attribute_named(&param.attributes, "MemorySize");
        let is_out = param.is_out && !param.is_in;
        let is_in_out = param.is_out && param.is_in;
        let optional = |rust: String| if param.is_optional { format!("Option<{rust}>") } else { rust };
        match pointee(&param.ty) {
            None => {
                if let Some(interface_key) = named(&param.ty).filter(|k| self.interface_api.contains_key(k) || *k == key(UNKNOWN.0, UNKNOWN.1)) {
                    let (borrowed, row) = self.object_borrowed(&interface_key, at, place);
                    self.count(row);
                    return Passed::In(optional(borrowed));
                }
                if let Some(named_key) = named(&param.ty)
                    && self.read.records.get(&named_key).is_some_and(|r| r.kind == RecordKind::Typedef)
                {
                    match self.typedef_form(&named_key) {
                        Typedef::String => {
                            self.count(Row::StringIn);
                            return Passed::In(if param.is_optional { "Option<&std::ffi::CStr>" } else { "&std::ffi::CStr" }.to_string());
                        }
                        Typedef::WideString => {
                            self.count(Row::WideStringIn);
                            return Passed::In(if param.is_optional { "Option<&[u16]>" } else { "&[u16]" }.to_string());
                        }
                        _ => {}
                    }
                }
                Passed::In(self.value(&param.ty, at, None, place))
            }
            Some((inner, depth, is_const)) => {
                if depth == 2 && *inner == Type::Void && attribute_named(&param.attributes, "ComOutPtr").is_some() {
                    self.count(Row::InterfaceById);
                    self.needs_unknown.borrow_mut().insert(at.api);
                    return Passed::Out(format!("{}<{}>", self.unknown_object_path(at.api, at.api), at.api_type));
                }
                if depth != 1 {
                    self.unmapped(Row::Unmapped, format!("{place}: pointer of depth {depth} to {inner:?}"));
                    return Passed::In("&impl wcontainers_volatile_bytes::VolatileBytes".to_string());
                }
                if let Some(array) = array {
                    if *inner == Type::Void {
                        // An array of void is bytes: the count is a byte count.
                        if is_out || (is_in_out && !is_const) {
                            self.count(Row::BytesOut);
                            return Passed::In(optional("&impl wcontainers_volatile_bytes::VolatileBytes".to_string()));
                        }
                        self.count(Row::BytesIn);
                        return Passed::In(optional("&[u8]".to_string()));
                    }
                    if let Some(length) = integer_argument(array, "CountConst") {
                        let length = self.length_constant(at, method, param, length, place);
                        if is_out {
                            self.count(Row::ValueOut);
                            return Passed::Out(format!("[{}; {length}]", self.element_out(inner, at, place)));
                        }
                        self.count(Row::ArrayFixedIn);
                        return Passed::In(optional(format!("&[{}; {length}]", self.element_in(inner, at, place))));
                    }
                    if let Some(count) = integer_argument(array, "CountParamIndex") {
                        let count_param = usize::try_from(count).ok().and_then(|c| method.params.get(c));
                        let count_is_pointer = count_param.is_some_and(|c| pointee(&c.ty).is_some());
                        if is_out || (is_in_out && !is_const) {
                            self.count(if count_is_pointer { Row::ArrayOutCounted } else { Row::ArrayOut });
                            return Passed::Out(format!("Vec<{}>", self.element_out(inner, at, place)));
                        }
                        let method_place = place.split('(').next().unwrap_or(place);
                        let keeps = count_param.and_then(|c| self.count_range(method_place, &c.name)).is_some_and(|e| matches!(e, Exception::CountRange { keep: Some(_), .. }));
                        if keeps {
                            // The header's keep value: none, and what is bound stays (an exception line).
                            self.count(Row::ExceptionLine);
                            return Passed::In(format!("Option<&[{}]>", self.element_in(inner, at, place)));
                        }
                        self.count(Row::ArrayIn);
                        return Passed::In(format!("&[{}]", self.element_in(inner, at, place)));
                    }
                    if text_argument(array, "CountFieldName").is_some() {
                        self.count(Row::ArrayIn);
                        return Passed::In(format!("&[{}]", self.element_in(inner, at, place)));
                    }
                }
                if let Some(bytes) = bytes {
                    let count = integer_argument(bytes, "BytesParamIndex").and_then(|c| usize::try_from(c).ok());
                    if is_out || (is_in_out && !is_const) {
                        if count.is_some_and(|c| returned_counts.contains(&c)) {
                            self.count(Row::BytesOutCounted);
                            return Passed::InAndOut("Option<&impl wcontainers_volatile_bytes::VolatileBytes>".to_string(), "u32".to_string());
                        }
                        self.count(Row::BytesOut);
                        return Passed::In(optional("&impl wcontainers_volatile_bytes::VolatileBytes".to_string()));
                    }
                    self.count(Row::BytesIn);
                    return Passed::In(optional("&[u8]".to_string()));
                }
                if let Some(interface_key) = named(inner).filter(|k| self.interface_api.contains_key(k) || *k == key(UNKNOWN.0, UNKNOWN.1)) {
                    if is_out || attribute_named(&param.attributes, "ComOutPtr").is_some() {
                        let made = attribute_named(&param.attributes, "ComOutPtr").is_some();
                        let deriving = self.kinds_deriving(&interface_key);
                        let single = deriving.len() == 1 && self.kind_of.get(&interface_key) == deriving.first();
                        self.count(if single { Row::ObjectOut } else { Row::BaseObjectOut });
                        let owned = self.object_owned(&interface_key, at, place);
                        return Passed::Out(if made { owned } else { format!("Option<{owned}>") });
                    }
                    self.unmapped(Row::Unmapped, format!("{place}: interface pointer by pointer, not out"));
                    return Passed::In("&impl wcontainers_volatile_bytes::VolatileBytes".to_string());
                }
                if *inner == Type::Void {
                    self.unmapped(Row::Unmapped, format!("{place}: void pointer with no size annotation"));
                    return Passed::In(optional("&impl wcontainers_volatile_bytes::VolatileBytes".to_string()));
                }
                if let Some(named_key) = named(inner) {
                    if named_key == key(GUID.0, GUID.1) {
                        self.count(Row::Guid);
                        if is_out {
                            return Passed::Out(self.guid_path(at.api));
                        }
                        return Passed::In(optional(format!("&{}", self.guid_path(at.api))));
                    }
                    if self.read.records.get(&named_key).is_some_and(|r| r.kind == RecordKind::Typedef)
                        && matches!(self.typedef_form(&named_key), Typedef::String | Typedef::WideString)
                    {
                        self.unmapped(Row::Unmapped, format!("{place}: pointer to a string pointer"));
                        return Passed::In("&impl wcontainers_volatile_bytes::VolatileBytes".to_string());
                    }
                }
                let value = self.value(inner, at, None, place);
                if is_out {
                    self.count(Row::ValueOut);
                    Passed::Out(value)
                } else if is_in_out && !is_const {
                    self.count(Row::StructureInOut);
                    Passed::In(optional(format!("&mut {value}")))
                } else {
                    self.count(Row::StructureIn);
                    Passed::In(optional(format!("&{value}")))
                }
            }
        }
    }

    fn method_text(&self, method: &Method, at: At<'_>, place: &str, out: &mut String) {
        let (inputs, result) = self.signature(method, at, place);
        let name = names::escaped(names::snake_case(&method.name));
        if let Some(link) = documentation(&method.attributes) {
            out.push_str(&doc_line("    ", &method.name, Some(&link)));
        }
        if let Some(not_yet) = self.not_yet_of(place) {
            let _ = writeln!(out, "    ///\n    /// Answers `true` for S_OK, the data written, and `false` for {not_yet:#x} (S_FALSE), not yet ready and nothing written.");
        }
        if inputs.len() > 6 {
            let _ = writeln!(out, "    #[allow(clippy::too_many_arguments, reason = \"the method's own parameters, one to one (MAPPING.md)\")]");
        }
        let mut parameters = String::from("&self");
        for (input, rust) in &inputs {
            let _ = write!(parameters, ", {input}: {rust}");
        }
        if result == "()" {
            let _ = writeln!(out, "    fn {name}({parameters});");
        } else {
            let _ = writeln!(out, "    fn {name}({parameters}) -> {result};");
        }
    }

    fn shared_error_allow(&self, methods: &[&Method], out: &mut String) {
        let status = methods
            .iter()
            .filter(|method| named(&method.return_type).is_some_and(|k| k == key(FOUNDATION, "HRESULT")))
            .count();
        if status > 1 {
            let _ = writeln!(out, "#[allow(reviewable::explicit_shared_error::HResult, reason = \"{SHARED_ERROR_REASON}\")]");
        }
    }

    fn interface_trait(&self, interface_key: &Key, out: &mut String) {
        let interface = &self.read.interfaces[interface_key];
        let api = self.interface_api[interface_key];
        let name = &self.trait_names[interface_key];
        let at = At { api, api_type: "Self::Api" };
        let methods: Vec<&Method> = interface.methods.iter().collect();
        self.shared_error_allow(&methods, out);
        if let Some(link) = documentation(&interface.attributes) {
            out.push_str(&doc_line("", &interface.name, Some(&link)));
        }
        let base = interface.base.clone().unwrap_or_else(|| key(UNKNOWN.0, UNKNOWN.1));
        let is_root = base == key(UNKNOWN.0, UNKNOWN.1) || !self.interface_api.contains_key(&base);
        if is_root {
            let _ = writeln!(out, "pub trait {name}: Clone + Eq + core::hash::Hash + Send + Sync + 'static {{");
            let _ = writeln!(out, "    type Api: {};", self.top_path(api, api).trim_start_matches("crate::"));
            if !interface.methods.is_empty() {
                out.push('\n');
            }
        } else {
            let _ = writeln!(out, "pub trait {name}: {} {{", self.trait_path(&base, api).trim_start_matches("crate::"));
        }
        for method in &interface.methods {
            self.method_text(method, at, &format!("{}::{}", interface.name, method.name), out);
        }
        out.push_str("}\n\n");
    }

    fn top_trait(&self, api: usize, out: &mut String) {
        let spec = self.apis[api];
        let Some(top) = spec.top_trait else { return };
        let functions: Vec<&Method> = self
            .read
            .functions
            .iter()
            .filter(|((namespace, _), _)| spec.namespaces.contains(&namespace.as_str()))
            .map(|(_, function)| function)
            .filter(|function| spec.library.is_some_and(|library| function.library.as_deref().is_some_and(|l| l.eq_ignore_ascii_case(library))))
            .filter(|function| spec.only.is_empty() || spec.only.contains(&function.name.as_str()))
            .collect();
        self.shared_error_allow(&functions, out);
        let supertraits: Vec<String> = spec
            .supertraits
            .iter()
            .filter_map(|folder| self.apis.iter().position(|a| a.folder == *folder))
            .map(|other| self.top_path(other, api))
            .collect();
        let bound = if supertraits.is_empty() { "Sized + Send + Sync + 'static".to_string() } else { format!("Sized + {}", supertraits.join(" + ")) };
        let _ = writeln!(out, "/// {}", spec.summary);
        let _ = writeln!(out, "pub trait {top}: {bound} {{");
        for kind in self.kinds.iter().filter(|kind| kind.api == api) {
            let mut bounds: Vec<String> = kind
                .newest
                .iter()
                .map(|newest| format!("{}{}", self.trait_path(newest, api).trim_start_matches("crate::"), self.api_binding(newest)))
                .collect();
            {
                for face in spec.faces_of(&kind.root.1).iter() {
                    if let Some(face_key) = self.interface_api.keys().find(|k| k.1 == *face) {
                        let face_kind = self.kind_of.get(face_key).map(|&k| &self.kinds[k]);
                        let face_newest = face_kind.and_then(|k| k.newest.iter().find(|n| self.bases_of(n).contains(face_key)).cloned()).unwrap_or_else(|| face_key.clone());
                        bounds.push(format!("{}{}", self.trait_path(&face_newest, api).trim_start_matches("crate::"), self.api_binding(&face_newest)));
                    }
                }
            }
            let _ = writeln!(out, "    type {}: {};", kind.name, bounds.join(" + "));
        }
        for exception in spec.exceptions {
            if let Exception::BoundObject { name, kind, .. } = exception
                && let Some(kind_key) = self.interface_api.keys().find(|k| k.1 == *kind)
            {
                let newest = self.kind_of.get(kind_key).and_then(|&k| self.kinds[k].newest.first().cloned()).unwrap_or_else(|| kind_key.clone());
                let _ = writeln!(out, "    /// The object an API built on this one hands out as a `{kind}` (an exception line of src/api.rs).");
                let _ = writeln!(out, "    type {name}: {}{};", self.trait_path(&newest, api).trim_start_matches("crate::"), self.api_binding(&newest));
            }
        }
        if spec.exceptions.iter().any(|e| matches!(e, Exception::Lent { .. })) {
            let _ = writeln!(out, "    /// Memory a Map lends until its Unmap, which the caller may read and write meanwhile (an exception line of");
            let _ = writeln!(out, "    /// src/api.rs).");
            let _ = writeln!(out, "    type Mapped: wcontainers_volatile_bytes::VolatileBytes + Send + Sync + 'static;");
        }
        let at = At { api, api_type: "Self" };
        let mut first = true;
        for function in &functions {
            if first {
                out.push('\n');
                first = false;
            }
            let mut renamed = (*function).clone();
            renamed.name = names::strip_first(&function.name, spec.function_prefixes).to_string();
            self.method_text(&renamed, at, &function.name, out);
        }
        out.push_str("}\n\n");
    }

    fn object_enums(&self, api: usize, out: &mut String) {
        let top = self.top_path(api, api);
        let top = top.trim_start_matches("crate::");
        for (interface_key, suffix) in self
            .needs_ref
            .borrow()
            .iter()
            .map(|k| (k.clone(), "Ref"))
            .chain(self.needs_object.borrow().iter().map(|k| (k.clone(), "Object")))
        {
            if self.interface_api[&interface_key] != api {
                continue;
            }
            let name = format!("{}{suffix}", self.trait_names[&interface_key]);
            let at = At { api, api_type: "A" };
            let borrowed = suffix == "Ref";
            let _ = writeln!(out, "/// An `{}*`: one of the kinds of object that are one.", interface_key.1);
            if borrowed {
                let _ = writeln!(out, "pub enum {name}<'a, A: {top}> {{");
            } else {
                let _ = writeln!(out, "pub enum {name}<A: {top}> {{");
            }
            for kind in self.kinds_deriving(&interface_key) {
                let kind_type = self.kind_type(kind, at);
                if borrowed {
                    let _ = writeln!(out, "    {}(&'a {kind_type}),", self.kinds[kind].name);
                } else {
                    let _ = writeln!(out, "    {}({kind_type}),", self.kinds[kind].name);
                }
            }
            out.push_str("}\n\n");
        }
        if self.needs_unknown.borrow().contains(&api) {
            let at = At { api, api_type: "A" };
            for (name, borrowed) in [("UnknownRef", true), ("UnknownObject", false)] {
                let _ = writeln!(out, "/// An `IUnknown*`: an object of one of this API's kinds, or of none.");
                if borrowed {
                    let _ = writeln!(out, "pub enum {name}<'a, A: {top}> {{");
                } else {
                    let _ = writeln!(out, "pub enum {name}<A: {top}> {{");
                }
                for (kind_index, kind) in self.kinds.iter().enumerate().filter(|(_, kind)| kind.api == api) {
                    let kind_type = self.kind_type(kind_index, at);
                    if borrowed {
                        let _ = writeln!(out, "    {}(&'a {kind_type}),", kind.name);
                    } else {
                        let _ = writeln!(out, "    {}({kind_type}),", kind.name);
                    }
                }
                let unknown = self.unknown_type(at);
                if borrowed {
                    let _ = writeln!(out, "    Unknown(&'a {unknown}),");
                } else {
                    let _ = writeln!(out, "    Unknown({unknown}),");
                }
                out.push_str("}\n\n");
            }
        }
    }

    /// Whether every field of a record is plain bits, so that the record derives Copy.
    fn fields_copy(&self, record: &Record) -> bool {
        record.fields.iter().all(|field| self.type_copy(&field.ty, record))
    }

    fn type_copy(&self, ty: &Type, outer: &Record) -> bool {
        if primitive(ty).is_some() {
            return true;
        }
        match ty {
            Type::ArrayFixed(inner, _) => self.type_copy(inner, outer),
            Type::Name(name) if name.namespace.is_empty() => {
                outer.nested.iter().find(|nested| nested.name == name.name).is_some_and(|nested| self.fields_copy(nested))
            }
            Type::Name(_) => {
                let named_key = named(ty).unwrap_or_default();
                if named_key == key(GUID.0, GUID.1) || Self::is_large_integer(&named_key) {
                    return true;
                }
                match self.read.records.get(&named_key) {
                    Some(record) => match record.kind {
                        RecordKind::Enum | RecordKind::Delegate => true,
                        RecordKind::Typedef => !matches!(self.typedef_form(&named_key), Typedef::String | Typedef::WideString),
                        RecordKind::Struct | RecordKind::Union => self.fields_copy(record),
                    },
                    None => false,
                }
            }
            _ => false,
        }
    }

    fn record_text(&self, record: &Record, name: &str, api: usize, out: &mut String) {
        let at = At { api, api_type: "A" };
        let record_key = key(&record.namespace, &record.name);
        let place = format!("{}.{}", record.namespace, record.name);
        if let Some(link) = documentation(&record.attributes) {
            out.push_str(&doc_line("", &record.name, Some(&link)));
        }
        let generic = self.record_is_generic(&record_key);
        let lent = self.lent_record(&record.name);
        let mut fields: Vec<(String, String)> = Vec::new();
        let mut used: HashSet<String> = HashSet::new();
        for field in &record.fields {
            let rust = match lent {
                Some(Exception::Lent { field: lent_field, .. }) if *lent_field == field.name => {
                    self.count(Row::ExceptionLine);
                    "Bytes".to_string()
                }
                _ if self.initial_bytes_field(&record.name, &field.name) => {
                    self.count(Row::ExceptionLine);
                    "Vec<u8>".to_string()
                }
                _ => self.field(field, record, name, at, &place),
            };
            let mut field_name = names::value_name(&field.name);
            while !used.insert(field_name.clone()) {
                field_name.push('_');
            }
            fields.push((field_name, rust));
        }
        let top = self.top_path(api, api);
        if lent.is_some() {
            let _ = writeln!(out, "#[derive(Clone, Debug, PartialEq)]");
            let _ = writeln!(out, "pub struct {name}<Bytes> {{");
        } else if generic {
            let _ = writeln!(out, "pub struct {name}<A: {top}> {{");
        } else {
            let copy = self.fields_copy(record) && !record.fields.iter().any(|field| self.initial_bytes_field(&record.name, &field.name));
            let _ = writeln!(out, "#[derive(Clone, {}Debug, PartialEq)]", if copy { "Copy, " } else { "" });
            let _ = writeln!(out, "pub struct {name} {{");
        }
        for (field_name, rust) in &fields {
            let _ = writeln!(out, "    pub {field_name}: {rust},");
        }
        out.push_str("}\n\n");
        for nested in &record.nested {
            self.record_text_nested(nested, record, &nested_name(name, &nested.name), api, out);
        }
    }

    fn record_text_nested(&self, nested: &Record, outer: &Record, name: &str, api: usize, out: &mut String) {
        let at = At { api, api_type: "A" };
        let place = format!("{}.{}::{}", outer.namespace, outer.name, nested.name);
        let generic = self.fields_generic(nested);
        let mut fields: Vec<(String, String)> = Vec::new();
        let mut used: HashSet<String> = HashSet::new();
        for field in &nested.fields {
            let rust = self.field(field, nested, name, at, &place);
            let mut field_name = names::value_name(&field.name);
            while !used.insert(field_name.clone()) {
                field_name.push('_');
            }
            fields.push((field_name, rust));
        }
        if nested.kind == RecordKind::Union {
            let _ = writeln!(out, "/// A C union of `{}`: every member, each read from the same bytes.", outer.name);
        } else {
            let _ = writeln!(out, "/// A nested struct of `{}`.", outer.name);
        }
        let top = self.top_path(api, api);
        if generic {
            let _ = writeln!(out, "pub struct {name}<A: {top}> {{");
        } else {
            let copy = self.fields_copy(nested);
            let _ = writeln!(out, "#[derive(Clone, {}Debug, PartialEq)]", if copy { "Copy, " } else { "" });
            let _ = writeln!(out, "pub struct {name} {{");
        }
        for (field_name, rust) in &fields {
            let _ = writeln!(out, "    pub {field_name}: {rust},");
        }
        out.push_str("}\n\n");
        for inner in &nested.nested {
            self.record_text_nested(inner, nested, &nested_name(name, &inner.name), api, out);
        }
    }

    fn field(&self, field: &Field, record: &Record, record_name: &str, at: At<'_>, place: &str) -> String {
        let place = format!("{place}.{}", field.name);
        match pointee(&field.ty) {
            None => self.value(&field.ty, at, Some((record, record_name)), &place),
            Some((inner, 1, _)) => {
                if let Some(array) = attribute_named(&field.attributes, "NativeArrayInfo")
                    && text_argument(array, "CountFieldName").is_some()
                {
                    self.count(Row::ArrayField);
                    return format!("Vec<{}>", self.element_out(inner, at, &place));
                }
                if *inner == Type::Void {
                    self.unmapped(Row::BytesField, format!("{place}: void pointer field, length from an exception line"));
                    return "Vec<u8>".to_string();
                }
                if let Some(named_key) = named(inner)
                    && (self.interface_api.contains_key(&named_key) || named_key == key(UNKNOWN.0, UNKNOWN.1))
                {
                    self.count(Row::ObjectField);
                    return format!("Vec<Option<{}>>", self.object_owned(&named_key, at, &place));
                }
                let element = self.value(inner, at, Some((record, record_name)), &place);
                self.unmapped(Row::Unmapped, format!("{place}: pointer field with no count"));
                format!("Vec<{element}>")
            }
            Some((inner, depth, _)) => {
                self.unmapped(Row::Unmapped, format!("{place}: pointer field of depth {depth} to {inner:?}"));
                "Vec<u8>".to_string()
            }
        }
    }

    fn enumeration_text(&self, record: &Record, name: &str, out: &mut String) {
        let underlying = record.underlying.as_ref().and_then(primitive).unwrap_or("i32");
        let link = documentation(&record.attributes);
        out.push_str(&doc_line("", &record.name, link.as_deref()));
        let _ = writeln!(out, "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]");
        let _ = writeln!(out, "pub struct {name}(pub {underlying});\n");
        for field in &record.fields {
            let Some(value) = &field.constant else { continue };
            out.push_str(&doc_line("", &field.name, link.as_deref()));
            let _ = writeln!(out, "pub const {}: {name} = {name}({});", field.name.to_ascii_uppercase(), literal_text(value, underlying));
        }
        out.push('\n');
    }

    fn constant_text(&self, field: &Field, api: usize, out: &mut String) {
        let mut text = String::new();
        self.constant_body(field, api, &mut text);
        if !text.is_empty() {
            out.push_str(&doc_line("", &field.name, documentation(&field.attributes).as_deref()));
            out.push_str(&text);
        }
    }

    fn constant_body(&self, field: &Field, api: usize, out: &mut String) {
        let name = field.name.to_ascii_uppercase();
        if let Some(guid) = metadata_guid(&field.attributes) {
            self.count(Row::Guid);
            let _ = writeln!(out, "pub const {name}: {} = {};", self.guid_path(api), guid_text(guid, &self.guid_path(api)));
            return;
        }
        let Some(value) = &field.constant else {
            self.unmapped(Row::Unmapped, format!("constant {}: no value the generator reads", field.name));
            return;
        };
        if let Some(rust) = primitive(&field.ty) {
            let _ = writeln!(out, "pub const {name}: {rust} = {};", literal_text(value, rust));
            return;
        }
        if field.ty == Type::String {
            let _ = writeln!(out, "pub const {name}: &str = {};", literal_text(value, "&str"));
            return;
        }
        if let Some(named_key) = named(&field.ty) {
            if named_key == key(FOUNDATION, "HRESULT") {
                let status = self.status_path(api);
                let Literal::Integer(code) = value else { return };
                let _ = writeln!(out, "pub const {name}: {status} = {status}({code:#x}_u32 as i32);", code = *code as i64 as u32);
                return;
            }
            if let Some(record) = self.read.records.get(&named_key)
                && record.kind == RecordKind::Enum
            {
                let path = self.data_path(self.record_owner(&named_key), Module::Enumerations, &self.record_name(&named_key), api);
                let underlying = record.underlying.as_ref().and_then(primitive).unwrap_or("i32");
                let _ = writeln!(out, "pub const {name}: {path} = {path}({});", literal_text(value, underlying));
                return;
            }
            if let Some(record) = self.read.records.get(&named_key)
                && record.kind == RecordKind::Typedef
                && let Some(inner) = record.fields.first().and_then(|f| primitive(&f.ty))
            {
                let path = self.data_path(self.record_owner(&named_key), Module::Structures, &self.record_name(&named_key), api);
                let _ = writeln!(out, "pub const {name}: {path} = {path}({});", literal_text(value, inner));
                return;
            }
        }
        self.unmapped(Row::Unmapped, format!("constant {}: of type {:?}", field.name, field.ty));
    }

    fn typedef_text(&self, record: &Record, name: &str, out: &mut String) {
        let inner = record.fields.first().map(|field| match pointee(&field.ty) {
            Some(_) => "usize",
            None => primitive(&field.ty).unwrap_or("usize"),
        });
        if let Some(link) = documentation(&record.attributes) {
            out.push_str(&doc_line("", &record.name, Some(&link)));
        }
        let _ = writeln!(out, "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]");
        let _ = writeln!(out, "pub struct {name}(pub {});\n", inner.unwrap_or("usize"));
    }
}

enum Passed {
    In(String),
    Out(String),
    InAndOut(String, String),
}

fn metadata_guid(attributes: &[metadata::Attribute]) -> Option<metadata::Guid> {
    let guid = attribute_named(attributes, "Guid")?;
    let values: Vec<i128> = guid
        .arguments
        .iter()
        .filter_map(|(_, value)| match value {
            Literal::Integer(value) => Some(*value),
            _ => None,
        })
        .collect();
    let [data1, data2, data3, rest @ ..] = values.as_slice() else { return None };
    let mut data4 = [0u8; 8];
    for (byte, value) in data4.iter_mut().zip(rest) {
        *byte = u8::try_from(*value).ok()?;
    }
    Some(metadata::Guid {
        data1: u32::try_from(*data1).ok()?,
        data2: u16::try_from(*data2).ok()?,
        data3: u16::try_from(*data3).ok()?,
        data4,
    })
}

fn guid_text(guid: metadata::Guid, path: &str) -> String {
    let bytes: Vec<String> = guid.data4.iter().map(|byte| format!("{byte:#04x}")).collect();
    format!(
        "{path} {{ data1: {:#010x}, data2: {:#06x}, data3: {:#06x}, data4: [{}] }}",
        guid.data1,
        guid.data2,
        guid.data3,
        bytes.join(", ")
    )
}

/// Adds the `use` lines a file's text needs for the other crates of the set it names.
fn with_imports(apis: &[&Api], own: usize, header: &str, body: &str) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "//! {HEADER}");
    if !header.is_empty() {
        let _ = writeln!(text, "//!\n//! {header}");
    }
    text.push('\n');
    let mut imports = Vec::new();
    for (position, api) in apis.iter().enumerate() {
        let needle = format!("{}::", alias(api));
        if position != own && body.lines().any(|line| !line.starts_with("use ") && line.contains(&needle)) {
            imports.push(format!("use {} as {};", package(api).replace('-', "_"), alias(api)));
        }
    }
    for import in &imports {
        let _ = writeln!(text, "{import}");
    }
    if !imports.is_empty() {
        text.push('\n');
    }
    text.push_str(body.trim_end());
    text.push('\n');
    text
}

pub fn generate(index: &TypeIndex, apis: &[&Api], out: &Path, writes_glue: bool) -> Result<String, String> {
    let generator = Generator::new(index, apis);
    let mut report = String::new();
    // Traits first: they decide which enums and which records of other namespaces the data files hold.
    let mut trait_texts: BTreeMap<Key, String> = BTreeMap::new();
    for interface_key in generator.interface_api.keys() {
        let mut text = String::new();
        generator.interface_trait(interface_key, &mut text);
        trait_texts.insert(interface_key.clone(), text);
    }
    // A root whose subtree never names Self::Api holds no `type Api`: no caller would ever see it.
    for root in trait_texts.keys().filter(|k| generator.chain_root(k) == **k).cloned().collect::<Vec<_>>() {
        let uses = trait_texts.iter().any(|(k, text)| generator.chain_root(k) == root && text.contains("Self::Api"));
        if !uses {
            let api = generator.interface_api[&root];
            let line = format!("    type Api: {};\n", generator.top_path(api, api).trim_start_matches("crate::"));
            if let Some(text) = trait_texts.get_mut(&root) {
                *text = text.replacen(&format!("{line}\n"), "", 1).replacen(&line, "", 1);
            }
            generator.apiless.borrow_mut().insert(root);
        }
    }
    let mut traits: Vec<String> = vec![String::new(); apis.len()];
    let mut ordered: Vec<&Key> = trait_texts.keys().collect();
    ordered.sort_by_key(|k| (generator.interface_api[*k], generator.trait_names[*k].clone()));
    for interface_key in ordered {
        traits[generator.interface_api[interface_key]].push_str(&trait_texts[interface_key]);
    }
    for api in 0..apis.len() {
        // A first pass only finds what the exports name; the tops are written after the records.
        generator.top_trait(api, &mut String::new());
    }
    // Records: every one of each API's own namespaces, then those of other namespaces they name, until none is new.
    let mut structures: Vec<String> = vec![String::new(); apis.len()];
    let mut enumerations: Vec<String> = vec![String::new(); apis.len()];
    let mut written: BTreeSet<Key> = BTreeSet::new();
    loop {
        let pending: Vec<(Key, usize)> = generator
            .record_api
            .borrow()
            .iter()
            .filter(|(k, _)| !written.contains(*k))
            .map(|(k, &a)| (k.clone(), a))
            .collect();
        if pending.is_empty() {
            break;
        }
        for (record_key, api) in pending {
            written.insert(record_key.clone());
            let Some(record) = generator.read.records.get(&record_key) else { continue };
            if Generator::is_large_integer(&record_key) {
                continue;
            }
            let name = generator.record_name(&record_key);
            match record.kind {
                RecordKind::Enum => generator.enumeration_text(record, &name, &mut enumerations[api]),
                RecordKind::Typedef => {
                    if let Typedef::Newtype = generator.typedef_form(&record_key) {
                        generator.typedef_text(record, &name, &mut structures[api]);
                    }
                }
                RecordKind::Struct | RecordKind::Union => generator.record_text(record, &name, api, &mut structures[api]),
                RecordKind::Delegate => {}
            }
        }
    }
    let mut constants: Vec<String> = vec![String::new(); apis.len()];
    for ((namespace, name), field) in &generator.read.constants {
        if namespace == FOUNDATION && STATUS_CODES.contains(&name.as_str()) {
            generator.constant_text(field, 0, &mut constants[0]);
            continue;
        }
        if let Some(api) = apis.iter().position(|a| a.namespaces.contains(&namespace.as_str())) {
            generator.constant_text(field, api, &mut constants[api]);
        }
    }
    for (interface_key, &api) in &generator.interface_api {
        if let Some(guid) = generator.read.interfaces[interface_key].guid {
            let spec = apis[api];
            let prefix = spec.interface_prefixes.iter().find(|p| interface_key.1.starts_with(**p)).copied().unwrap_or("");
            let rest = &interface_key.1[prefix.len()..];
            let path = generator.guid_path(api);
            constants[api].push_str(&doc_line("", &format!("IID_{}", interface_key.1), documentation(&generator.read.interfaces[interface_key].attributes).as_deref()));
            let _ = writeln!(constants[api], "pub const {}: {path} = {};", names::interface_id_name(prefix, rest), guid_text(guid, &path));
        }
    }
    for (api, lengths) in generator.lengths.borrow().iter() {
        for (name, (length, c_name, link)) in lengths {
            constants[*api].push_str(&doc_line("", c_name, link.as_deref()));
            let _ = writeln!(constants[*api], "pub const {name}: usize = {length};");
        }
    }
    let mut tops: Vec<String> = Vec::new();
    for api in 0..apis.len() {
        let mut body = String::new();
        generator.top_trait(api, &mut body);
        tops.push(body);
    }
    // The COM crate's own two types.
    structures[0].insert_str(
        0,
        "/// The header's `GUID`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/guiddef/ns-guiddef-guid).\n#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]\npub struct Guid {\n    pub data1: u32,\n    pub data2: u16,\n    pub data3: u16,\n    pub data4: [u8; crate::constants::GUID_DATA4_LENGTH],\n}\n\n/// The header's `HRESULT`, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/com/structure-of-com-error-codes).\n#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]\npub struct HResult(pub i32);\n\n/// A COM object of no kind of these APIs, held by one counted reference while any clone lives; equal to another\n/// holding the same object (COM's identity, its `IUnknown` address).\n#[derive(Clone)]\npub struct Foreign {\n    identity: usize,\n    object: std::sync::Arc<dyn core::any::Any + Send + Sync>,\n}\n\nimpl Foreign {\n    /// Wraps the glue's own holder of an object, told apart by `identity`.\n    pub fn new(identity: usize, object: std::sync::Arc<dyn core::any::Any + Send + Sync>) -> Foreign {\n        Foreign { identity, object }\n    }\n\n    /// Returns the holder, which only the glue that made it can read back.\n    pub fn object(&self) -> &(dyn core::any::Any + Send + Sync) {\n        &*self.object\n    }\n\n    /// Returns the object's identity.\n    pub fn identity(&self) -> usize {\n        self.identity\n    }\n}\n\nimpl PartialEq for Foreign {\n    fn eq(&self, other: &Foreign) -> bool {\n        self.identity == other.identity\n    }\n}\n\nimpl Eq for Foreign {}\n\nimpl core::hash::Hash for Foreign {\n    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {\n        self.identity.hash(state);\n    }\n}\n\nimpl core::fmt::Debug for Foreign {\n    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {\n        formatter.debug_struct(\"Foreign\").field(\"identity\", &self.identity).finish_non_exhaustive()\n    }\n}\n\n",
    );
    constants[0].insert_str(
        0,
        "/// The header's `GUID::Data4` length, [as Microsoft documents it](https://learn.microsoft.com/windows/win32/api/guiddef/ns-guiddef-guid).\npub const GUID_DATA4_LENGTH: usize = 8;\n\n",
    );
    for (api, spec) in apis.iter().enumerate() {
        let folder = out.join("crates").join(spec.folder);
        std::fs::create_dir_all(&folder).map_err(|error| format!("{}: {error}", folder.display()))?;
        let mut interface = String::new();
        let mut modules = Vec::new();
        for (module, body) in [(Module::Constants, &constants[api]), (Module::Enumerations, &enumerations[api]), (Module::Structures, &structures[api])] {
            if !body.is_empty() {
                modules.push(module.file());
                let text = with_imports(apis, api, "", body);
                std::fs::write(folder.join(format!("{}.rs", module.file())), text).map_err(|error| error.to_string())?;
            }
        }
        for module in &modules {
            let _ = writeln!(interface, "pub mod {module};");
        }
        if !modules.is_empty() {
            interface.push('\n');
        }
        interface.push_str(&tops[api]);
        interface.push_str(&traits[api]);
        let mut enums = String::new();
        generator.object_enums(api, &mut enums);
        interface.push_str(&enums);
        let interface = interface.replace("crate::structures::", "structures::").replace("crate::enumerations::", "enumerations::").replace("crate::constants::", "constants::");
        let interface = rewrite_root_paths(&interface);
        // The status is named as `HResult`, so that each trait's reasoned exclusion of the shared error names it.
        let interface = if api != 0 && interface.contains("component_object_model::structures::HResult") {
            format!(
                "use wcontainers_component_object_model::structures::HResult;\n\n{}",
                interface.replace("component_object_model::structures::HResult", "HResult")
            )
        } else {
            interface
        };
        std::fs::write(folder.join("interface.rs"), with_imports(apis, api, spec.summary, &interface)).map_err(|error| error.to_string())?;
        let mut manifest = String::new();
        let _ = writeln!(manifest, "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n", package(spec));
        let _ = writeln!(manifest, "[lib]\npath = \"interface.rs\"\n");
        let all_text: String = [&interface, &constants[api], &enumerations[api], &structures[api]].iter().map(|s| s.as_str()).collect();
        let _ = writeln!(manifest, "[dependencies]");
        for (other, other_spec) in apis.iter().enumerate() {
            if other != api && all_text.contains(&format!("{}::", alias(other_spec))) {
                let _ = writeln!(manifest, "{} = {{ path = \"../{}\" }}", package(other_spec), other_spec.folder);
                if !spec.depends_on.contains(&other_spec.folder) {
                    let _ = writeln!(report, "UNDECLARED DEPENDENCY: {} names {} (add it to depends_on in src/api.rs)", spec.folder, other_spec.folder);
                }
            }
        }
        if all_text.contains("wcontainers_volatile_bytes::") {
            let _ = writeln!(manifest, "wcontainers-volatile-bytes = {{ path = \"../volatile-bytes\" }}");
        }
        let _ = writeln!(manifest, "\n[lints]\nworkspace = true");
        std::fs::write(folder.join("Cargo.toml"), manifest).map_err(|error| error.to_string())?;
        if api == 0 {
            crate::runtime::write(&folder)?;
        }
        let interfaces = generator.interface_api.values().filter(|&&a| a == api).count();
        let methods: usize = generator
            .interface_api
            .iter()
            .filter(|(_, a)| **a == api)
            .map(|(k, _)| generator.read.interfaces[k].methods.len())
            .sum();
        let kinds = generator.kinds.iter().filter(|kind| kind.api == api).count();
        let _ = writeln!(report, "CRATE {}: interfaces {interfaces} methods {methods} object kinds {kinds}", spec.folder);
    }
    // Default deny: every method's parameters and returns classified by kind, before any glue is written.
    let classified = kinds::classify(&generator)?;
    let kinds_report = classified.report.clone();
    *generator.classified.borrow_mut() = Some(classified);
    // The glue: each API's direct crate, for a slice and for the whole APIs (AC3).
    // The spike's slice is Direct3D 11 cut down; Direct3D 12 is cut to what its users call, a set of its own kind.
    let sliced = apis.iter().any(|api| api.folder == "direct3d-11" && !api.only.is_empty());
    let mut failures: Vec<String> = Vec::new();
    if writes_glue {
        let writer = glue::Writer::new(&generator);
        for (api, spec) in apis.iter().enumerate().skip(1) {
            let folder = out.join("crates").join(spec.folder);
            report.push_str(&glue::write_direct(&writer, api, &folder)?);
        }
        // The whole APIs' recorders, which stand for the system in the hostile tests on any machine.
        if !sliced {
            for (api, spec) in apis.iter().enumerate().skip(1) {
                if spec.top_trait.is_some() {
                    let folder = out.join("crates").join(spec.folder);
                    report.push_str(&glue::write_recorder(&writer, api, &folder)?);
                }
            }
            if let Some(position) = apis.iter().position(|api| api.folder == "direct3d-11") {
                let tests = out.join("crates").join(apis[position].folder).join("implementation").join("recorder").join("tests");
                std::fs::create_dir_all(&tests).map_err(|error| error.to_string())?;
                for (name, text) in RECORDER_TESTS {
                    std::fs::write(tests.join(name), text).map_err(|error| format!("{name}: {error}"))?;
                }
            }
        }
        // The hostile implementation set and its driver (src/glue_hostile.rs), for a slice and for the whole APIs.
        for (api, spec) in apis.iter().enumerate().skip(1) {
            if spec.top_trait.is_some() {
                let folder = out.join("crates").join(spec.folder);
                report.push_str(&glue::write_hostile(&writer, api, &folder)?);
            }
        }
        // Each API's conformance suite, run by every implementation of the set (src/glue_hostile.rs).
        for (api, spec) in apis.iter().enumerate().skip(1) {
            if spec.top_trait.is_some() {
                let folder = out.join("crates").join(spec.folder);
                report.push_str(&glue::write_suite(&writer, api, &folder)?);
            }
        }
        // The hostile C caller over every table and export (src/glue_c_caller.rs).
        report.push_str(&glue::write_c_caller(&writer, &out.join("crates"))?);
        // The pass-through test and the spike's harness, hand-written for the slice, go with its Direct3D 11 glue.
        if sliced && let Some(position) = apis.iter().position(|api| api.folder == "direct3d-11") {
            let direct = out.join("crates").join(apis[position].folder).join("implementation").join("direct");
            std::fs::create_dir_all(direct.join("tests")).map_err(|error| error.to_string())?;
            std::fs::write(direct.join("tests").join("round_trip.rs"), include_str!("../harness/round_trip.rs")).map_err(|error| error.to_string())?;
            // The spike's harness, ported: the fake system and C programs, the front and the liar, and their tests.
            for (name, text) in HARNESS {
                let path = direct.join("harness").join(name);
                std::fs::create_dir_all(path.parent().unwrap_or(&direct)).map_err(|error| error.to_string())?;
                std::fs::write(&path, text).map_err(|error| format!("{name}: {error}"))?;
            }
            let manifest = direct.join("Cargo.toml");
            let text = std::fs::read_to_string(&manifest).map_err(|error| error.to_string())?;
            let dev = [
                "wcontainers-byte-sink = { path = \"../../../byte-sink\" }",
                "wcontainers-byte-source = { path = \"../../../byte-source\" }",
                "wcontainers-volatile-bytes = { path = \"../../../volatile-bytes\" }",
                "wcontainers-direct3d-common = { path = \"../../../direct3d-common\" }",
                "wcontainers-directx-graphics-infrastructure = { path = \"../../../directx-graphics-infrastructure\" }",
            ]
            .map(String::from);
            std::fs::write(&manifest, with_dev_dependencies(&text, &dev)).map_err(|error| error.to_string())?;
        }
        let mut glued = writer.glued.borrow().clone();
        glued.sort();
        glued.dedup();
        let _ = writeln!(report, "GLUED {} (the serving glue's entry or export calls the implementation): {}", glued.len(), glued.join(", "));
        // Every method the serving glue does not carry, with what each glue wrote for it, from the record kept as their
        // text was written (src/glue.rs `Outcome`): the report and the glue cannot disagree.
        let outcomes = writer.outcomes.borrow().clone();
        let not_glued: Vec<(&String, &glue::Outcome)> = outcomes.iter().filter(|(_, outcome)| !outcome.serving.iter().any(|what| what.starts_with("glued"))).collect();
        let _ = writeln!(report, "NOT GLUED {} (each refused by its form, as each glue wrote it):", not_glued.len());
        let mut by_answer: BTreeMap<String, usize> = BTreeMap::new();
        let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
        for (place, outcome) in &not_glued {
            let reason = outcome.reason.clone().unwrap_or_else(|| "no reason recorded".to_string());
            let serving: Vec<String> = outcome.serving.iter().cloned().collect();
            let calling: Vec<String> = outcome.calling.iter().cloned().collect();
            let calling = if calling.is_empty() { "no trait method (not in the set)".to_string() } else { calling.join(" | ") };
            let _ = writeln!(report, "  {place}: {reason}\n      serving: {}\n      calling: {calling}", serving.join(" | "));
            for what in &outcome.serving {
                *by_answer.entry(format!("serving {what}")).or_default() += 1;
            }
            *reasons.entry(reason.rsplit(": ").next().unwrap_or("").to_string()).or_default() += 1;
        }
        let calling_refusals = outcomes.values().filter(|outcome| outcome.calling.iter().any(|what| !what.starts_with("glued"))).count();
        let _ = writeln!(report, "NULL ENTRIES {} (a table entry the generator could not write fails it)", writer.failures.borrow().iter().filter(|f| f.starts_with("NULL ENTRY")).count());
        let _ = writeln!(report, "CALLING GLUE METHODS THAT DO NOT CALL THE SYSTEM {calling_refusals} (refused by kinds, by form, or pinned; each answer above)");
        let _ = writeln!(report, "NOT GLUED BY ENTRY:");
        for (what, count) in &by_answer {
            let _ = writeln!(report, "  {count:>4}  {what}");
        }
        let _ = writeln!(report, "NOT GLUED BY FORM:");
        for (reason, count) in &reasons {
            let _ = writeln!(report, "  {count:>4}  {reason}");
        }
        failures.extend(writer.failures.borrow().iter().cloned());
    }
    let _ = writeln!(report, "ROWS (parameters, fields and returns mapped by each row of MAPPING.md):");
    for row in Row::ALL {
        let _ = writeln!(report, "  {:>6}  {row:?}: {}", generator.rows.borrow().get(&row).copied().unwrap_or(0), row.statement());
    }
    let unmapped = generator.unmapped.borrow();
    let _ = writeln!(report, "UNMAPPED {} (each needs an exception line in src/api.rs):", unmapped.len());
    for line in unmapped.iter() {
        let _ = writeln!(report, "  {line}");
    }
    let mut mapping = String::from("# The mapping rule\n\nWritten by tools/api-crate-generator from src/mapping.rs; do not edit.\n\n| Row | C form -> Rust form |\n|---|---|\n");
    for row in Row::ALL {
        let _ = writeln!(mapping, "| {row:?} | {} |", row.statement());
    }
    std::fs::write(out.join("MAPPING.md"), mapping).map_err(|error| error.to_string())?;
    // Each direct crate exports only what another crate of the set names (src/visibility.rs).
    if writes_glue {
        let folders: Vec<&str> = apis.iter().skip(1).map(|api| api.folder).collect();
        report.push_str(&crate::visibility::narrow(out, &folders)?);
    }
    format_written(out)?;
    // No null entry, no panicking body (src/complete.rs).
    if writes_glue {
        report.push_str(&crate::complete::check(out, &failures).map_err(|failure| format!("{report}{failure}"))?);
    }
    report.push_str(&kinds_report);
    // Every mark's reason matched to what it relies on, and every hostile test the rows owe written.
    report.push_str(&crate::undo::check(out)?);
    // Every safe trait of the other party asked once per value: the glue carries the answer (src/trust.rs).
    let asked = crate::trust::check(out, Some(&generator.asked.borrow())).map_err(|failure| format!("{report}{failure}"))?;
    report.push_str(&asked);
    if let Some(classified) = generator.classified.borrow().as_ref() {
        match kinds::check_tests(classified, out, writes_glue) {
            Ok(tests) => report.push_str(&tests),
            // The whole report goes with the failure, so that a reader sees what was written.
            Err(failure) => return Err(format!("{report}{failure}")),
        }
    }
    Ok(report)
}

/// In a direct crate, the items of its own API's interface crate are named by the crate's alias.
fn rewrite_direct_paths(text: &str, generator: &Generator<'_>, api: usize) -> String {
    text.replace("crate::", &format!("{}::", alias(generator.apis[api])))
}

/// In interface.rs, items of the crate's root are named without `crate::`.
fn rewrite_root_paths(text: &str) -> String {
    text.replace("<Self::Api as crate::", "<Self::Api as ").replace("<Self as crate::", "<Self as ").replace("<A as crate::", "<A as ").replace("&crate::", "&").replace("(crate::", "(").replace(" crate::", " ").replace("<crate::", "<")
}

/// Formats every Rust file written with the toolchain's rustfmt, so that the output is the formatter's.
fn format_written(out: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    let mut folders = vec![out.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).map_err(|error| format!("{}: {error}", folder.display()))? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    let status = std::process::Command::new("rustfmt")
        .args(["--edition", "2024"])
        .args(&files)
        .status()
        .map_err(|error| format!("rustfmt: {error}"))?;
    if status.success() { Ok(()) } else { Err(format!("rustfmt: {status}")) }
}

/// Adds each line of `dev` to a manifest's `[dev-dependencies]` (made before its lint tables when it has none), leaving
/// out a package the manifest already names in `[dependencies]` or `[dev-dependencies]`.
pub(crate) fn with_dev_dependencies(manifest: &str, dev: &[String]) -> String {
    let named = |line: &str| line.split(' ').next().unwrap_or_default().to_string();
    let present: BTreeSet<String> = manifest.lines().filter(|line| line.contains("= {") || line.contains("= \"")).map(named).collect();
    let mut seen = BTreeSet::new();
    let added: Vec<&String> = dev.iter().filter(|line| !present.contains(&named(line)) && seen.insert(named(line))).collect();
    if added.is_empty() {
        return manifest.to_string();
    }
    let lines: String = added.iter().map(|line| format!("{line}\n")).collect();
    let mut text = manifest.to_string();
    if let Some(at) = text.find("[dev-dependencies]\n") {
        text.insert_str(at + "[dev-dependencies]\n".len(), &lines);
    } else {
        let at = text.find("\n# A C boundary").unwrap_or(text.len());
        text.insert_str(at, &format!("\n[dev-dependencies]\n{lines}"));
    }
    text
}

/// A structure a COM method returns by value is, in the method's C form (d3d12.h's `RetVal`), written through a pointer
/// the caller passes after `this`, which the method also returns. Each such method is read as taking that pointer first
/// (`RetVal`, a structure out; every parameter index of its attributes moved by one) and returning nothing, marked
/// `ReturnsPointer`, so that the trait returns the structure as it returns any structure out and both glues return the
/// pointer.
fn returned_structures_through_pointers(read: &mut metadata::Namespaces) {
    let structures: BTreeSet<Key> = read
        .records
        .iter()
        .filter(|(record_key, record)| matches!(record.kind, RecordKind::Struct | RecordKind::Union) && !Generator::is_large_integer(record_key))
        .map(|(record_key, _)| record_key.clone())
        .collect();
    for interface in read.interfaces.values_mut() {
        for method in &mut interface.methods {
            if !named(&method.return_type).is_some_and(|returned| structures.contains(&returned)) {
                continue;
            }
            for param in &mut method.params {
                for attribute in &mut param.attributes {
                    for (argument, value) in &mut attribute.arguments {
                        if let (true, metadata::Literal::Integer(index)) = (matches!(argument.as_str(), "CountParamIndex" | "BytesParamIndex"), value) {
                            *index += 1;
                        }
                    }
                }
            }
            let pointer = Type::PtrMut(Box::new(method.return_type.clone()), 1);
            method.params.insert(0, metadata::Param { name: "RetVal".to_string(), ty: pointer, is_in: false, is_out: true, is_optional: false, attributes: Vec::new() });
            method.return_type = Type::Void;
            method.attributes.push(metadata::Attribute { name: "ReturnsPointer".to_string(), arguments: Vec::new() });
        }
    }
}
