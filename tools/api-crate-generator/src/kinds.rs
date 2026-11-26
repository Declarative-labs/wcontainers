//! Default deny: every parameter and return of every method the set carries, classified by its metadata type into a
//! closed list of kinds, each with one rule for how the calling glue treats it.
//!
//! A type that fits no kind fails the generator. A kind the system trusts and acts on (a module, a function pointer, a
//! raw pointer, an integer of pointer width, an object of no kind of the set), or memory lent until a later call that
//! no exception line holds, makes the calling glue refuse the method unless a row of src/api.rs (`pins`) gives the rule
//! for that exact method with its reason and hostile test. Every enumeration met is judged value by value (src/api.rs
//! `judged` and `specials`), and every interface's effects are judged (`effects`). The report prints, for every method,
//! the kind of each parameter and its treatment, and the count of methods per treatment.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use windows_metadata::Type;

use super::{GUID, Generator, Key, Typedef, UNKNOWN, integer_argument, key, named, pointee, primitive};
use crate::api::{Exception, HANDLES, HandleKind, Hold, Special, Values};
use crate::metadata::{Attribute, Literal, Method, Record, RecordKind, attribute_named};

/// The closed list of kinds a parameter, a return or a field is of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// An integer, a float, a BOOL, an HRESULT, a GUID or a LARGE_INTEGER by value.
    Number,
    /// A value of an enumeration (by its type, or a number whose `AssociatedEnum` names it).
    Flags(String),
    /// A COM object of a kind of the set (or `IUnknown`, an object of the set or one the system handed over).
    Object(String),
    /// Memory the caller lends for the call: what it holds, and whether the system writes it.
    Lends { what: Box<Kind>, out: bool },
    /// Memory lent until a later call, and whether an exception line of src/api.rs holds it until its end.
    LentUntil { what: String, line: bool },
    /// A handle the system's kernel checks.
    KernelHandle(String),
    /// A handle or address the system trusts and acts on as it is.
    Trusted(String),
    /// Text: a string pointer by value.
    Text,
    /// A structure (or union), by its label: its fields are of these kinds.
    Structure(String),
}

/// Each kind's name and its one rule, in the order of `Kind`.
pub(super) const RULES: [(&str, &str); 9] = [
    ("number", "passed as the value: the system reads a number"),
    ("flags", "its enumeration's judgment: every value plain, or only the declared ones; a special value cleared or refused"),
    ("object", "a counted reference of ours, live for the call; one handed out is adopted, one reference"),
    ("lends", "a buffer of the glue's own, of the length the glue computes, for the call only, copied in or out"),
    ("lent-until", "refused unless an exception line of src/api.rs holds the memory until its end and revokes it first"),
    ("kernel-handle", "passed: the kernel looks it up in this process's tables and checks it before acting"),
    ("trusted", "refused unless a row of src/api.rs gives the rule for that exact method, with its reason and hostile test"),
    ("text", "copied with its terminator into a buffer of the glue's own for the call"),
    ("structure", "copied as the bytes C lays it out in; each field treated by its own kind (a trusting field makes it trusting)"),
];

impl Kind {
    fn name(&self) -> &'static str {
        let index = match self {
            Kind::Number => 0,
            Kind::Flags(_) => 1,
            Kind::Object(_) => 2,
            Kind::Lends { .. } => 3,
            Kind::LentUntil { .. } => 4,
            Kind::KernelHandle(_) => 5,
            Kind::Trusted(_) => 6,
            Kind::Text => 7,
            Kind::Structure(_) => 8,
        };
        RULES[index].0
    }

    fn text(&self) -> String {
        match self {
            Kind::Number | Kind::Text => self.name().to_string(),
            Kind::Flags(name) | Kind::Object(name) | Kind::KernelHandle(name) | Kind::Structure(name) => format!("{} {name}", self.name()),
            Kind::Lends { what, out } => format!("lends{}({})", if *out { "-out" } else { "" }, what.text()),
            Kind::LentUntil { what, line } => format!("lent-until({what}{})", if *line { "; held by its exception line" } else { "" }),
            Kind::Trusted(what) => format!("trusted({what})"),
        }
    }
}

/// How the calling glue treats one method.
#[derive(Clone, Debug)]
pub(super) enum Treatment {
    /// Every part is of a kind passed by its rule.
    Passed,
    /// Passed after checks: values cleared or refused, a handle that passes only as null.
    Checked(Vec<String>),
    /// A row: answered by the glue, the system never called.
    Answered,
    /// A row: never called.
    NotCalled,
    /// A part of a kind the system trusts, or memory lent until later, with no row: refused, the system never called.
    Refused(Vec<String>),
}

impl Treatment {
    fn name(&self) -> &'static str {
        match self {
            Treatment::Passed => "passed",
            Treatment::Checked(_) => "checked",
            Treatment::Answered => "answered",
            Treatment::NotCalled => "never-called",
            Treatment::Refused(_) => "refused",
        }
    }
}

/// The checks of an enumeration's values on one parameter passed by value.
#[derive(Clone, Debug)]
pub(super) struct ValueRule {
    /// The parameter's metadata name.
    pub(super) param: String,
    pub(super) enumeration: String,
    /// The parameter's integer type.
    pub(super) integer: &'static str,
    pub(super) flags: bool,
    /// The declared values, when only they pass.
    pub(super) declared: Option<Vec<i128>>,
    pub(super) clear: Vec<i128>,
    pub(super) refuse: Vec<i128>,
}

/// A hostile test a row or a special value owes, for a method the set carries.
#[derive(Clone, Debug)]
pub(super) struct Owed {
    pub(super) what: String,
    pub(super) folder: &'static str,
    pub(super) method: String,
    pub(super) test: &'static str,
    /// A never-called row, which the glue written can show instead: its calling form holds no call out.
    pub(super) not_called: bool,
}

/// What the classification found: each method's treatment, the value checks, the report and the tests owed.
pub(super) struct Classified {
    pub(super) treatments: BTreeMap<String, Treatment>,
    pub(super) rules: BTreeMap<String, Vec<ValueRule>>,
    pub(super) report: String,
    pub(super) owed: Vec<Owed>,
}

/// What an exception line puts in place of one field of the record a parameter points at.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Override {
    None,
    /// Initial data: the field holds read-only bytes of the length the line's rule gives.
    InitialBytes(String),
    /// Map's record: the field holds memory lent until the line's end.
    Lent(String, String),
}

struct Classifier<'g, 'a> {
    g: &'g Generator<'a>,
    /// Each structure's fields, by label.
    structures: RefCell<BTreeMap<String, Vec<(String, Kind)>>>,
    /// Every enumeration met, with whether it was met in memory passed in (where only `Any` without specials holds).
    enumerations: RefCell<BTreeMap<String, Key>>,
    /// Numbers named as flags, masks or options the metadata links to no enumeration, by `place(parameter)`.
    unlinked: RefCell<BTreeSet<String>>,
    failures: RefCell<Vec<String>>,
}

fn associated_enum(attributes: &[Attribute]) -> Option<String> {
    attribute_named(attributes, "AssociatedEnum").and_then(|attribute| {
        attribute.arguments.iter().find_map(|(_, value)| match value {
            Literal::Text(text) => Some(text.clone()),
            _ => None,
        })
    })
}

impl Classifier<'_, '_> {
    fn fail(&self, what: String) {
        self.failures.borrow_mut().push(what);
    }

    /// The kind of an enumeration's value, by the enumeration's name.
    fn enumeration(&self, name: &str, place: &str) -> Kind {
        let found = self.g.read.records.iter().find(|(k, record)| k.1 == name && record.kind == RecordKind::Enum).map(|(k, _)| k.clone());
        match found {
            Some(record_key) => {
                self.enumerations.borrow_mut().insert(name.to_string(), record_key);
                Kind::Flags(name.to_string())
            }
            None => {
                self.fail(format!("NO KIND: {place}: the enumeration {name} is not in the metadata read"));
                Kind::Trusted(format!("an enumeration not read ({name})"))
            }
        }
    }

    /// The kind of a value of `ty` (a parameter by value, a field, a return, or what a pointer points at).
    fn of_value(&self, ty: &Type, attributes: &[Attribute], out: bool, outer: Option<(&Record, &str)>, place: &str) -> Kind {
        if primitive(ty).is_some()
            && let Some(enumeration) = associated_enum(attributes)
        {
            return self.enumeration(&enumeration, place);
        }
        match ty {
            Type::Bool | Type::Char | Type::I8 | Type::U8 | Type::I16 | Type::U16 | Type::I32 | Type::U32 | Type::I64 | Type::U64 | Type::F32 | Type::F64 => Kind::Number,
            // A size the system hands back is a number; one passed in may be an address the system acts on.
            Type::ISize | Type::USize if out => Kind::Number,
            Type::ISize | Type::USize => Kind::Trusted("an integer of pointer width, which the system may take as an address".to_string()),
            Type::ArrayFixed(inner, _) => self.of_value(inner, &[], out, outer, place),
            Type::PtrConst(..) | Type::PtrMut(..) => self.of_pointer(ty, attributes, out, outer, place, &Override::None, false),
            Type::Name(name) if name.namespace.is_empty() => {
                let Some((record, id)) = outer else {
                    self.fail(format!("NO KIND: {place}: a nested type {} outside its record", name.name));
                    return Kind::Trusted("a nested type outside its record".to_string());
                };
                match record.nested.iter().find(|nested| nested.name == name.name) {
                    Some(nested) => self.structure(nested, &format!("{id}.{}", nested.name), out, &Override::None, place),
                    None => {
                        self.fail(format!("NO KIND: {place}: no nested type {} in {}", name.name, record.name));
                        Kind::Trusted("a nested type not found".to_string())
                    }
                }
            }
            Type::Name(_) => {
                let named_key = named(ty).unwrap_or_default();
                if named_key == key(GUID.0, GUID.1) || Generator::is_large_integer(&named_key) {
                    return Kind::Number;
                }
                if named_key == key(UNKNOWN.0, UNKNOWN.1) {
                    return Kind::Object("IUnknown".to_string());
                }
                if self.g.read.interfaces.contains_key(&named_key) || self.g.all_methods.contains_key(&named_key) {
                    if self.g.interface_api.contains_key(&named_key) && !self.g.kinds_deriving(&named_key).is_empty() {
                        return Kind::Object(named_key.1);
                    }
                    return Kind::Trusted(format!("a COM object of no kind of the set ({}), whose code the system would call", named_key.1));
                }
                let Some(record) = self.g.read.records.get(&named_key) else {
                    self.fail(format!("NO KIND: {place}: the type {}.{} is not in the metadata read", named_key.0, named_key.1));
                    return Kind::Trusted(format!("a type not read ({})", named_key.1));
                };
                match record.kind {
                    RecordKind::Enum => self.enumeration(&named_key.1, place),
                    RecordKind::Typedef => match self.g.typedef_form(&named_key) {
                        Typedef::Bool | Typedef::Status => Kind::Number,
                        Typedef::String | Typedef::WideString => Kind::Text,
                        Typedef::Newtype => match HANDLES.iter().find(|(name, _, _)| *name == named_key.1) {
                            Some((name, HandleKind::Kernel, _)) => Kind::KernelHandle((*name).to_string()),
                            Some((name, HandleKind::Trusted, why)) => Kind::Trusted(format!("{name}: {why}")),
                            Some((_, HandleKind::Number, _)) => Kind::Number,
                            None => {
                                self.fail(format!("NO KIND: {place}: the typedef {} fits no kind (src/api.rs HANDLES names it not)", named_key.1));
                                Kind::Trusted(format!("a typedef of no kind ({})", named_key.1))
                            }
                        },
                    },
                    RecordKind::Struct | RecordKind::Union => {
                        self.structure(record, &named_key.1, out, &Override::None, place)
                    }
                    RecordKind::Delegate => Kind::Trusted(format!("the function pointer {}, whose code the system would run", named_key.1)),
                }
            }
            _ => {
                self.fail(format!("NO KIND: {place}: the type {ty:?} fits no kind"));
                Kind::Trusted("a type of no kind".to_string())
            }
        }
    }

    /// The kind of a pointer: memory lent for the call, an object, or a trusted address.
    #[allow(clippy::too_many_arguments, reason = "the pointer's own facts")]
    fn of_pointer(&self, ty: &Type, attributes: &[Attribute], out: bool, outer: Option<(&Record, &str)>, place: &str, over: &Override, in_field: bool) -> Kind {
        let Some((inner, depth, is_const)) = pointee(ty) else { return self.of_value(ty, attributes, out, outer, place) };
        let written = out || !is_const && !in_field;
        let sized = attribute_named(attributes, "MemorySize").is_some() || attribute_named(attributes, "NativeArrayInfo").is_some();
        if depth >= 2 {
            if *inner == Type::Void && attribute_named(attributes, "ComOutPtr").is_some() {
                return Kind::Object("the interface its riid names".to_string());
            }
            if out {
                return Kind::LentUntil { what: format!("an address the system writes (a {depth}-deep pointer), valid until a later call the metadata does not name"), line: false };
            }
            return Kind::Trusted(format!("a {depth}-deep pointer, whose inner pointer the system follows"));
        }
        if in_field && out {
            // A pointer the system writes into a structure it hands back: its memory, valid until some later call.
            return Kind::LentUntil { what: "an address the system writes into a structure, valid until a later call the metadata does not name".to_string(), line: false };
        }
        if *inner == Type::Void {
            if sized {
                return Kind::Lends { what: Box::new(Kind::Number), out: written };
            }
            return Kind::Trusted("a raw pointer with no size, which the system reads or writes as it likes".to_string());
        }
        let inner_key = named(inner);
        if let Some(inner_key) = &inner_key
            && self.g.read.records.get(inner_key).is_some_and(|r| r.kind == RecordKind::Typedef)
            && matches!(self.g.typedef_form(inner_key), Typedef::String | Typedef::WideString)
        {
            if out {
                return Kind::LentUntil { what: "a text pointer the system writes, valid until a later call the metadata does not name".to_string(), line: false };
            }
            return Kind::Trusted("a pointer to a text pointer, which the system follows".to_string());
        }
        let what = match (inner_key, over) {
            (Some(inner_key), Override::InitialBytes(_) | Override::Lent(..)) => match self.g.read.records.get(&inner_key) {
                Some(record) => self.structure(record, &inner_key.1, out, over, place),
                None => self.of_value(inner, attributes, out, outer, place),
            },
            _ => self.of_value(inner, &[], out, outer, place),
        };
        if let Kind::Object(_) = what
            && !sized
        {
            // An object handed out through a pointer: the object itself, adopted.
            return what;
        }
        Kind::Lends { what: Box::new(what), out: written }
    }

    /// The kind of a structure, classifying its fields once per label.
    fn structure(&self, record: &Record, id: &str, out: bool, over: &Override, place: &str) -> Kind {
        let mut label = id.to_string();
        if out {
            label.push_str(" out");
        }
        match over {
            Override::None => {}
            Override::InitialBytes(field) => label.push_str(&format!(" (its {field}: read-only bytes by its exception line)")),
            Override::Lent(field, until) => label.push_str(&format!(" (its {field}: lent until {until} by its exception line)")),
        }
        if self.structures.borrow().contains_key(&label) {
            return Kind::Structure(label);
        }
        self.structures.borrow_mut().insert(label.clone(), Vec::new());
        let mut fields = Vec::new();
        for field in &record.fields {
            let field_place = format!("{place}/{}.{}", record.name, field.name);
            let kind = match over {
                Override::InitialBytes(bytes) if *bytes == field.name => Kind::Lends { what: Box::new(Kind::Number), out: false },
                Override::Lent(lent, until) if *lent == field.name => Kind::LentUntil { what: format!("memory the system lends until {until}"), line: true },
                _ => match &field.ty {
                    Type::PtrConst(..) | Type::PtrMut(..) => self.of_pointer(&field.ty, &field.attributes, out, Some((record, id)), &field_place, &Override::None, true),
                    other => self.of_value(other, &field.attributes, out, Some((record, id)), &field_place),
                },
            };
            fields.push((field.name.clone(), kind));
        }
        self.structures.borrow_mut().insert(label.clone(), fields);
        Kind::Structure(label)
    }

    /// The kind of one parameter of `method`, with what an exception line of src/api.rs says of it.
    fn parameter(&self, place: &str, method: &Method, position: usize, counts: &BTreeSet<usize>) -> Kind {
        let param = &method.params[position];
        let param_place = format!("{place}({})", param.name);
        let out = param.is_out && !param.is_in;
        if counts.contains(&position) {
            if pointee(&param.ty).is_some() {
                return Kind::Lends { what: Box::new(Kind::Number), out: param.is_out };
            }
            return Kind::Number;
        }
        if let Some(Exception::InitialData { bytes, .. }) = self.g.initial_data(place, &param.name) {
            return self.of_pointer(&param.ty, &param.attributes, false, None, &param_place, &Override::InitialBytes((*bytes).to_string()), false);
        }
        if let Some(Exception::Lent { record, field, until, .. }) = self.g.lent_of(place)
            && let Some((inner, 1, _)) = pointee(&param.ty)
            && named(inner).is_some_and(|k| k.1 == *record)
        {
            return self.of_pointer(&param.ty, &param.attributes, true, None, &param_place, &Override::Lent((*field).to_string(), (*until).to_string()), false);
        }
        let kind = match &param.ty {
            Type::PtrConst(..) | Type::PtrMut(..) => self.of_pointer(&param.ty, &param.attributes, out, None, &param_place, &Override::None, false),
            other => self.of_value(other, &param.attributes, false, None, &param_place),
        };
        // A number named as flags, a mask or options that the metadata links to no enumeration: judged as one of its own.
        if kind == Kind::Number && matches!(param.ty, Type::I32 | Type::U32 | Type::I64 | Type::U64) && ["Flag", "Mask", "Option"].iter().any(|part| param.name.contains(part)) {
            self.unlinked.borrow_mut().insert(param_place.clone());
            return Kind::Flags(param_place);
        }
        kind
    }

    /// What in `kind` the calling glue cannot pass by its rule: a trusted part, memory lent until later with no line,
    /// or an enumeration in memory passed in whose judgment the glue checks only on a parameter by value.
    fn refusals(&self, kind: &Kind, in_memory: bool, seen: &mut BTreeSet<String>, found: &mut Vec<String>) {
        match kind {
            Kind::Trusted(what) => found.push(what.clone()),
            Kind::LentUntil { what, line: false } => found.push(what.clone()),
            Kind::Lends { what, out } => self.refusals(what, !out, seen, found),
            Kind::Structure(label) => {
                if !seen.insert(label.clone()) {
                    return;
                }
                let fields = self.structures.borrow().get(label).cloned().unwrap_or_default();
                let passed_in = in_memory && !label.ends_with(" out") && !label.contains(" out ");
                for (field, kind) in fields {
                    let mut inner = Vec::new();
                    self.refusals(&kind, passed_in, seen, &mut inner);
                    found.extend(inner.into_iter().map(|why| format!("{label}.{field}: {why}")));
                }
            }
            Kind::Flags(name) if in_memory => {
                let (values, specials) = self.judgment(name);
                if values != Some(Values::Any) || specials > 0 {
                    found.push(format!("the enumeration {name} in memory passed in, judged by declared or special values, which the glue checks only on a parameter by value"));
                }
            }
            _ => {}
        }
    }

    /// An enumeration's judgment and how many special values it has.
    fn judgment(&self, name: &str) -> (Option<Values>, usize) {
        let values = self.g.apis.iter().flat_map(|api| api.judged.iter()).find(|judged| judged.enumerations.contains(&name)).map(|judged| judged.values);
        let specials = self.g.apis.iter().flat_map(|api| api.specials.iter()).filter(|special| special.enumeration == name).count();
        (values, specials)
    }

    /// The checks of an enumeration on a parameter passed by value.
    fn value_rule(&self, param: &str, integer: &'static str, name: &str) -> Option<ValueRule> {
        let record_key = self.enumerations.borrow().get(name).cloned()?;
        let record = self.g.read.records.get(&record_key)?;
        let flags = attribute_named(&record.attributes, "Flags").is_some();
        let value_of = |field: &str| {
            record.fields.iter().find(|f| f.name == field).and_then(|f| match f.constant {
                Some(Literal::Integer(value)) => Some(value),
                _ => None,
            })
        };
        let declared: Vec<i128> = record.fields.iter().filter_map(|f| match f.constant {
            Some(Literal::Integer(value)) => Some(value),
            _ => None,
        }).collect();
        let (values, _) = self.judgment(name);
        let mut rule = ValueRule {
            param: param.to_string(),
            enumeration: name.to_string(),
            integer,
            flags,
            declared: (values == Some(Values::Declared)).then_some(declared),
            clear: Vec::new(),
            refuse: Vec::new(),
        };
        for special in self.g.apis.iter().flat_map(|api| api.specials.iter()).filter(|special| special.enumeration == name) {
            match value_of(special.value) {
                Some(value) => match special.rule {
                    Special::Clear => rule.clear.push(value),
                    Special::Refuse => rule.refuse.push(value),
                },
                None => self.fail(format!("STALE SPECIAL: {name} has no value {}", special.value)),
            }
        }
        (rule.declared.is_some() || !rule.clear.is_empty() || !rule.refuse.is_empty()).then_some(rule)
    }
}

/// Every place the set's calling glue carries: interface methods (`Interface::Method`) and exports, with the folder of
/// the API that owns each.
fn carried<'m>(g: &'m Generator<'_>) -> Vec<(String, String, &'m Method, usize)> {
    let mut places = Vec::new();
    for (interface_key, &api) in &g.interface_api {
        for method in &g.read.interfaces[interface_key].methods {
            places.push((format!("{}::{}", interface_key.1, method.name), interface_key.1.clone(), method, api));
        }
    }
    for (position, spec) in g.apis.iter().enumerate() {
        if spec.top_trait.is_none() {
            continue;
        }
        for ((namespace, _), function) in &g.read.functions {
            if spec.namespaces.contains(&namespace.as_str())
                && spec.library.is_some_and(|library| function.library.as_deref().is_some_and(|l| l.eq_ignore_ascii_case(library)))
                && (spec.only.is_empty() || spec.only.contains(&function.name.as_str()))
            {
                places.push((function.name.clone(), function.name.clone(), function, position));
            }
        }
    }
    places
}

/// The index of every parameter another one names as its count.
fn counts_of(method: &Method) -> BTreeSet<usize> {
    let mut counts = BTreeSet::new();
    for param in &method.params {
        for (attribute, argument) in [("NativeArrayInfo", "CountParamIndex"), ("MemorySize", "BytesParamIndex")] {
            if let Some(count) = attribute_named(&param.attributes, attribute).and_then(|a| integer_argument(a, argument))
                && let Ok(count) = usize::try_from(count)
            {
                counts.insert(count);
            }
        }
    }
    counts
}

/// Classifies every method the set carries; a type of no kind, an enumeration not judged, an interface whose effects
/// are not judged, or a stale row fails.
pub(super) fn classify(g: &Generator<'_>) -> Result<Classified, String> {
    let classifier = Classifier { g, structures: RefCell::new(BTreeMap::new()), enumerations: RefCell::new(BTreeMap::new()), unlinked: RefCell::new(BTreeSet::new()), failures: RefCell::new(Vec::new()) };
    let places = carried(g);
    let mut treatments = BTreeMap::new();
    let mut rules: BTreeMap<String, Vec<ValueRule>> = BTreeMap::new();
    let mut lines = String::new();
    let mut by_kind: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut owed = Vec::new();
    for (place, owner, method, api) in &places {
        let counts = counts_of(method);
        let kinds: Vec<(String, Kind)> = (0..method.params.len()).map(|position| (method.params[position].name.clone(), classifier.parameter(place, method, position, &counts))).collect();
        let returned = match &method.return_type {
            Type::Void => None,
            Type::PtrConst(..) | Type::PtrMut(..) => Some(Kind::LentUntil { what: "an address the system returns, valid until a later call the metadata does not name".to_string(), line: false }),
            other => Some(classifier.of_value(other, &method.attributes, true, None, &format!("{place} return"))),
        };
        for (_, kind) in &kinds {
            *by_kind.entry(kind.name()).or_default() += 1;
        }
        if let Some(kind) = &returned {
            *by_kind.entry(kind.name()).or_default() += 1;
        }
        let pins: Vec<&crate::api::Pin> = g.apis.iter().flat_map(|spec| spec.pins.iter()).filter(|pin| pin.method == place).collect();
        let mut refused = Vec::new();
        for (name, kind) in &kinds {
            let mut found = Vec::new();
            classifier.refusals(kind, false, &mut BTreeSet::new(), &mut found);
            let held = pins.iter().any(|pin| matches!(pin.hold, Hold::NullOnly { param, .. } if param == name));
            if !held {
                refused.extend(found.into_iter().map(|why| format!("{name}: {why}")));
            }
        }
        if let Some(kind) = &returned {
            let mut found = Vec::new();
            classifier.refusals(kind, false, &mut BTreeSet::new(), &mut found);
            refused.extend(found.into_iter().map(|why| format!("return: {why}")));
        }
        let mut checks = Vec::new();
        for (name, kind) in &kinds {
            if let Kind::Flags(enumeration) = kind {
                let param = method.params.iter().find(|p| p.name == *name);
                let integer = param.and_then(|p| primitive(&p.ty)).or_else(|| {
                    let record_key = classifier.enumerations.borrow().get(enumeration).cloned()?;
                    g.read.records.get(&record_key)?.underlying.as_ref().and_then(primitive)
                });
                if let Some(rule) = classifier.value_rule(name, integer.unwrap_or("i32"), enumeration) {
                    let mut text = format!("{name}: {enumeration}");
                    if rule.declared.is_some() {
                        text.push_str(" declared values only");
                    }
                    if !rule.clear.is_empty() {
                        text.push_str(&format!(", clears {:?}", rule.clear));
                    }
                    if !rule.refuse.is_empty() {
                        text.push_str(&format!(", refuses {:?}", rule.refuse));
                    }
                    checks.push(text);
                    for special in g.apis.iter().flat_map(|spec| spec.specials.iter()).filter(|special| special.enumeration == enumeration) {
                        owed.push(Owed { what: format!("{enumeration}::{} on {place}", special.value), folder: g.apis[*api].folder, method: place.clone(), test: special.test, not_called: false });
                    }
                    rules.entry(place.clone()).or_default().push(rule);
                }
            }
        }
        let mut treatment = if refused.is_empty() { if checks.is_empty() { Treatment::Passed } else { Treatment::Checked(checks.clone()) } } else { Treatment::Refused(refused.clone()) };
        for pin in &pins {
            let relies: Vec<String> = pin.relies.iter().map(|reliance| format!("{reliance:?}")).collect();
            owed.push(Owed { what: format!("the row of {place} (holds: {})", if relies.is_empty() { "a parameter's kind".to_string() } else { relies.join(", ") }), folder: g.apis[*api].folder, method: place.clone(), test: pin.test, not_called: matches!(pin.hold, Hold::NotCalled { .. }) });
            match pin.hold {
                Hold::NotCalled { .. } => treatment = Treatment::NotCalled,
                Hold::Answers { .. } => {
                    if !matches!(treatment, Treatment::NotCalled) {
                        treatment = Treatment::Answered;
                    }
                }
                Hold::Clears { param, mask, .. } => {
                    if method.params.iter().all(|p| p.name != param) {
                        classifier.fail(format!("STALE ROW: {place} has no parameter {param}"));
                    }
                    if let Treatment::Passed | Treatment::Checked(_) = treatment {
                        let mut list = if let Treatment::Checked(list) = &treatment { list.clone() } else { Vec::new() };
                        list.push(format!("{param}: clears {mask:#x}"));
                        treatment = Treatment::Checked(list);
                    }
                }
                Hold::Refuses { param, when, .. } => {
                    if method.params.iter().all(|p| p.name != param) {
                        classifier.fail(format!("STALE ROW: {place} has no parameter {param}"));
                    }
                    if let Treatment::Passed | Treatment::Checked(_) = treatment {
                        let mut list = if let Treatment::Checked(list) = &treatment { list.clone() } else { Vec::new() };
                        list.push(format!("{param}: refused when {}", when.replace("{}", param)));
                        treatment = Treatment::Checked(list);
                    }
                }
                Hold::NullOnly { param, .. } => {
                    let kind = kinds.iter().find(|(name, _)| name == param).map(|(_, kind)| kind);
                    if !matches!(kind, Some(Kind::Trusted(_))) {
                        classifier.fail(format!("STALE ROW: {place}'s {param} is not of a trusted kind ({:?}), so its null-only row is not needed", kind.map(Kind::text)));
                    }
                    if let Treatment::Passed | Treatment::Checked(_) = treatment {
                        let mut list = if let Treatment::Checked(list) = &treatment { list.clone() } else { Vec::new() };
                        list.push(format!("{param}: null only"));
                        treatment = Treatment::Checked(list);
                    }
                }
            }
        }
        let why = match &treatment {
            Treatment::Refused(list) | Treatment::Checked(list) => format!(" [{}]", list.join("; ")),
            _ => String::new(),
        };
        let parameters: Vec<String> = kinds.iter().map(|(name, kind)| format!("{name}={}", kind.text())).collect();
        let returns = returned.as_ref().map_or("nothing".to_string(), Kind::text);
        let _ = writeln!(lines, "  METHOD {place} ({owner}): {}{why}; params: {}; returns {returns}", treatment.name(), if parameters.is_empty() { "none".to_string() } else { parameters.join(", ") });
        treatments.insert(place.clone(), treatment);
    }
    // Every enumeration met is judged, once.
    let mut enumeration_lines = String::new();
    for (name, record_key) in classifier.enumerations.borrow().iter() {
        let judgments: Vec<&crate::api::Judged> = g.apis.iter().flat_map(|api| api.judged.iter()).filter(|judged| judged.enumerations.contains(&name.as_str())).collect();
        let record = &g.read.records[record_key];
        let flags = attribute_named(&record.attributes, "Flags").is_some();
        let specials: Vec<String> = g.apis.iter().flat_map(|api| api.specials.iter()).filter(|special| special.enumeration == name).map(|special| format!("{} {:?} ({})", special.value, special.rule, special.why)).collect();
        match judgments.as_slice() {
            [judged] => {
                let _ = writeln!(enumeration_lines, "  {name} ({} values, {}): {:?}{} -- {}", record.fields.len(), if flags { "flag set" } else { "selector" }, judged.values, if specials.is_empty() { String::new() } else { format!(", {}", specials.join(", ")) }, judged.why);
            }
            [] => classifier.fail(format!("UNJUDGED ENUMERATION: {name} ({} values, {}): no line of src/api.rs judges its values", record.fields.len(), if flags { "flag set" } else { "selector" })),
            _ => classifier.fail(format!("ENUMERATION JUDGED TWICE: {name}")),
        }
    }
    for name in classifier.unlinked.borrow().iter() {
        let judgments: Vec<&crate::api::Judged> = g.apis.iter().flat_map(|api| api.judged.iter()).filter(|judged| judged.enumerations.contains(&name.as_str())).collect();
        match judgments.as_slice() {
            [judged] if judged.values == Values::Any => {
                let _ = writeln!(enumeration_lines, "  {name} (a number the metadata links to no enumeration): Any -- {}", judged.why);
            }
            [_] => classifier.fail(format!("UNLINKED FLAGS JUDGED BY DECLARED VALUES: {name}: with no enumeration only Any (and a row) can judge it")),
            [] => classifier.fail(format!("UNJUDGED FLAGS: {name}: a number named as flags, a mask or options, which the metadata links to no enumeration; no line of src/api.rs judges its values")),
            _ => classifier.fail(format!("FLAGS JUDGED TWICE: {name}")),
        }
    }
    for judged in g.apis.iter().flat_map(|api| api.judged.iter()) {
        for name in judged.enumerations {
            if let Some((method, _)) = name.split_once('(') {
                let known = g.all_methods.iter().any(|(k, methods)| methods.iter().any(|m| format!("{}::{}", k.1, m.name) == method)) || g.read.functions.values().any(|f| f.name == method);
                if !known {
                    classifier.fail(format!("STALE JUDGMENT: {name} names no method of the metadata read"));
                }
                continue;
            }
            if !g.read.records.iter().any(|(k, record)| k.1 == *name && record.kind == RecordKind::Enum) {
                classifier.fail(format!("STALE JUDGMENT: {name} is no enumeration of the metadata read"));
            }
        }
    }
    // Every interface and export carried has its effects judged, by its own API.
    let mut effect_lines = String::new();
    let mut judged_interfaces: BTreeMap<&str, usize> = BTreeMap::new();
    for (position, spec) in g.apis.iter().enumerate() {
        for effects in spec.effects {
            for interface in effects.interfaces {
                *judged_interfaces.entry(interface).or_default() += 1;
                let known = g.all_methods.keys().any(|k| k.1 == *interface && g.apis[position].namespaces.contains(&k.0.as_str()))
                    || g.read.functions.values().any(|function| function.name == *interface);
                if !known {
                    classifier.fail(format!("STALE EFFECTS: {} judges {interface}, which is no interface or export of its metadata", spec.folder));
                }
            }
            let _ = writeln!(effect_lines, "  {}: {} -- {}", spec.folder, effects.interfaces.join(", "), effects.why);
        }
    }
    let mut owners: BTreeSet<&str> = BTreeSet::new();
    for (_, owner, _, _) in &places {
        owners.insert(owner);
    }
    for owner in owners {
        match judged_interfaces.get(owner) {
            Some(1) => {}
            Some(_) => classifier.fail(format!("EFFECTS JUDGED TWICE: {owner}")),
            None => classifier.fail(format!("UNJUDGED EFFECTS: {owner}: no line of src/api.rs judges what its methods can do to a fact the marks rely on")),
        }
    }
    let mut report = String::new();
    let _ = writeln!(report, "KINDS (default deny: every parameter and return of every method the calling glue carries, by its metadata type):");
    for (name, rule) in RULES {
        let _ = writeln!(report, "  RULE {name}: {rule}");
    }
    report.push_str(&lines);
    let mut counted: BTreeMap<&'static str, usize> = BTreeMap::new();
    for treatment in treatments.values() {
        *counted.entry(treatment.name()).or_default() += 1;
    }
    let _ = writeln!(report, "  TREATMENTS of {} methods: {}", treatments.len(), counted.iter().map(|(name, count)| format!("{name} {count}")).collect::<Vec<_>>().join(", "));
    let _ = writeln!(report, "  PARTS by kind: {}", by_kind.iter().map(|(name, count)| format!("{name} {count}")).collect::<Vec<_>>().join(", "));
    let _ = writeln!(report, "ENUMERATIONS met, each judged:");
    report.push_str(&enumeration_lines);
    let _ = writeln!(report, "EFFECTS judged, by interface:");
    report.push_str(&effect_lines);
    let failures = classifier.failures.into_inner();
    if failures.is_empty() {
        Ok(Classified { treatments, rules, report, owed })
    } else {
        let mut unique = failures;
        unique.sort();
        unique.dedup();
        Err(format!("{report}\nthe kinds are not all classified and judged ({}):\n  {}", unique.len(), unique.join("\n  ")))
    }
}

/// Every Rust test file written under `out`: the harness's and every crate's `tests`.
fn test_sources(out: &Path) -> String {
    let mut text = String::new();
    let mut folders = vec![out.join("crates")];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        let mut paths: Vec<_> = entries.filter_map(Result::ok).map(|entry| entry.path()).collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") && path.parent().is_some_and(|parent| parent.ends_with("tests")) {
                text.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    text
}

/// Whether every function of the API's calling glue named for `method` holds no `unsafe` (no call out), and there is
/// one; none when no calling glue was written.
fn never_calls_out(out: &Path, folder: &str, method: &str) -> Option<bool> {
    let path = out.join("crates").join(folder).join("implementation").join("direct").join("call.rs");
    let text = std::fs::read_to_string(path).ok()?;
    let name = crate::names::snake_case(method.rsplit("::").next()?);
    let mut found = false;
    for (at, _) in text.match_indices(&format!("fn {name}(")) {
        let body = &text[at..];
        let body = &body[..body.find("\n    }\n").unwrap_or(body.len())];
        if body.contains("unsafe") {
            return Some(false);
        }
        found = true;
    }
    Some(found)
}

/// Checks that every hostile test the rows and special values owe is in the tests written; returns the report's section.
pub(super) fn check_tests(classified: &Classified, out: &Path, glue_written: bool) -> Result<String, String> {
    let tests = test_sources(out);
    let mut report = String::new();
    let mut failures = Vec::new();
    let mut seen = BTreeSet::new();
    let _ = writeln!(report, "TESTS OWED by the rows and special values of the methods carried:");
    for owed in &classified.owed {
        if !seen.insert((owed.what.clone(), owed.test)) {
            continue;
        }
        let state = if !glue_written {
            "owed where the glue is written".to_string()
        } else if !owed.test.is_empty() && tests.contains(&format!("fn {}(", owed.test)) {
            format!("hostile test {}", owed.test)
        } else if owed.not_called && never_calls_out(out, owed.folder, &owed.method) == Some(true) {
            "checked in the glue written: its calling form holds no call out".to_string()
        } else {
            failures.push(format!("UNTESTED: {} owes the hostile test `{}`, which no test file written holds", owed.what, owed.test));
            "TEST MISSING".to_string()
        };
        let _ = writeln!(report, "  {}: {state}", owed.what);
    }
    if failures.is_empty() {
        Ok(report)
    } else {
        Err(format!("{report}\n{}", failures.join("\n")))
    }
}

/// The literal of `value` in the integer type `integer`, as the glue writes it.
pub(super) fn literal(value: i128, integer: &str) -> String {
    match integer {
        "i32" => format!("{}_i32", value as i32),
        "u32" => format!("{:#x}_u32", value as u32),
        "i16" => format!("{}_i16", value as i16),
        "u16" => format!("{:#x}_u16", value as u16),
        "i8" => format!("{}_i8", value as i8),
        "u8" => format!("{:#x}_u8", value as u8),
        "i64" => format!("{}_i64", value as i64),
        "u64" => format!("{:#x}_u64", value as u64),
        other => format!("{value}_{other}"),
    }
}

/// The calling glue's checks of one value rule on the parameter `name` (`.0` of a newtype when `newtype`).
pub(super) fn rule_checks(rule: &ValueRule, name: &str, newtype: bool, fail: &str) -> String {
    let value = if newtype { format!("{name}.0") } else { name.to_string() };
    let integer = rule.integer;
    let mut text = String::new();
    let _ = writeln!(text, "        // {}'s judgment (src/api.rs, checked by src/kinds.rs).", rule.enumeration);
    if !rule.clear.is_empty() {
        let mask = rule.clear.iter().fold(0_i128, |mask, value| mask | value);
        if newtype {
            let _ = writeln!(text, "        let mut {name} = {name};\n        {name}.0 &= !{};", literal(mask, integer));
        } else {
            let _ = writeln!(text, "        let {name} = {name} & !{};", literal(mask, integer));
        }
    }
    for refused in &rule.refuse {
        if rule.flags {
            let _ = writeln!(text, "        if {value} & {} != 0 {{ {fail} }}", literal(*refused, integer));
        } else {
            let _ = writeln!(text, "        if {value} == {} {{ {fail} }}", literal(*refused, integer));
        }
    }
    if let Some(declared) = &rule.declared {
        if rule.flags {
            let mask = declared.iter().fold(0_i128, |mask, value| mask | value);
            let _ = writeln!(text, "        if {value} & !{} != 0 {{ {fail} }}", literal(mask, integer));
        } else {
            // Runs of consecutive values as ranges, as clippy asks.
            let mut sorted: Vec<i128> = declared.clone();
            sorted.sort_unstable();
            sorted.dedup();
            let mut patterns: Vec<String> = Vec::new();
            let mut at = 0;
            while at < sorted.len() {
                let mut end = at;
                while end + 1 < sorted.len() && sorted[end + 1] == sorted[end] + 1 {
                    end += 1;
                }
                if end > at {
                    patterns.push(format!("{}..={}", literal(sorted[at], integer), literal(sorted[end], integer)));
                } else {
                    patterns.push(literal(sorted[at], integer));
                }
                at = end + 1;
            }
            let _ = writeln!(text, "        if !matches!({value}, {}) {{ {fail} }}", patterns.join(" | "));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::literal;

    #[test]
    fn literals_keep_the_bits() {
        assert_eq!(literal(0x8000_0000, "i32"), "-2147483648_i32");
        assert_eq!(literal(0x1, "u32"), "0x1_u32");
    }
}
