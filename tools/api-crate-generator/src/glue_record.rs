//! A recording implementation of every trait of one API, for tests only: `implementation/recorder`. Every call is logged
//! by its place (with the values passed by value); an object asked for is made, one of each kind's type; a structure,
//! a bound object or a list asked for is not (E_NOTIMPL). Served by the serving glue and called through the calling
//! glue, it stands for the system on any machine, so that a hostile test can see what reached the system.

use std::fmt::Write as _;
use std::path::Path;

use super::{Glued, Part, Writer, direct_alias};
use crate::api::Exception;
use crate::emit::{HEADER, Key, alias, package};
use crate::metadata::Method;

/// The crate alias of an API's recorder.
pub(super) fn recorder_alias(folder: &str) -> String {
    format!("{}_recorder", folder.replace('-', "_"))
}

/// The length of the bytes the recorder lends for each Map, and the pitches it answers: one 64 KiB page, rows of 4 KiB.
pub(super) const MAPPED: (u32, u32, u32) = (1 << 16, 1 << 12, 1 << 16);

/// The value the hostile implementation answers where an exception line says the answer is one of a list (src/api.rs
/// `Exception::OneOf`) and two or more are asked: no D3D_FEATURE_LEVEL is 1.
const NOT_ASKED: i32 = 1;
/// What a known answer's salt moves by from one part of an answer to the next.
const KNOWN_STEP: usize = 7;
/// Stands in the recorder's answer list for a known value of the answer's type, until the types are split (followed
/// by the element count of a list).
const KNOWN_MARK: &str = "@KNOWN@";

/// A method's salt: its place's bytes folded (FNV-1a), so that different methods answer different known values.
pub(super) fn salt_of(place: &str) -> usize {
    const OFFSET: u32 = 0x811c_9dc5;
    const PRIME: u32 = 0x0100_0193;
    let folded = place.bytes().fold(OFFSET, |hash, byte| (hash ^ u32::from(byte)).wrapping_mul(PRIME));
    usize::try_from(folded % 1_000_003).unwrap_or(0)
}

/// An expression of which objects an argument of objects holds (`name`, of Rust type `ty`): whether an optional object is
/// there, and for an array, whether each element is there or how many there are; none for an argument of no such form
/// (one object, which is always there, or bytes).
fn presence_of(name: &str, ty: &str) -> Option<String> {
    let ty = ty.trim();
    // Bytes are `impl VolatileBytes` and plain data names no `Self`: neither holds objects.
    if !ty.contains("Self") {
        return None;
    }
    fn slice_of(ty: &str) -> Option<&str> {
        ty.strip_prefix("&[").or_else(|| ty.strip_prefix("&'_ [")).and_then(|rest| rest.strip_suffix(']')).map(str::trim)
    }
    if let Some(inner) = ty.strip_prefix("Option<").and_then(|rest| rest.strip_suffix('>')).map(str::trim) {
        // An optional array of objects holds optional objects (`Option<&[Option<&T>]>`, as every such header array is).
        return Some(match slice_of(inner) {
            Some(_) => format!("{name}.map(|items| items.iter().map(|item| item.is_some()).collect::<Vec<_>>())"),
            None => format!("{name}.is_some()"),
        });
    }
    match slice_of(ty) {
        Some(element) if element.starts_with("Option<") => Some(format!("{name}.iter().map(|item| item.is_some()).collect::<Vec<_>>()")),
        Some(_) => Some(format!("{name}.len()")),
        None => None,
    }
}

/// The recorder's log, objects, bytes and helpers, the same in every recorder crate.
pub(super) const RECORDER_BASE: &str = r#"use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use wcontainers_component_object_model::constants::{E_NOINTERFACE, E_NOTIMPL};

/// Every call any object of this crate received, oldest first: the place, and the values passed by value.
static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// The next object's identity.
static NEXT: AtomicU64 = AtomicU64::new(1);
/// Objects alive now.
static LIVE: AtomicI64 = AtomicI64::new(0);

/// Every call's arguments as the recorder received them, oldest first: the place and each argument of plain data
/// (`Debug`, every field), an object named only as one.
static ARGUMENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Logs one call.
fn record(call: String) {
    LOG.lock().unwrap_or_else(PoisonError::into_inner).push(call);
}

/// Logs one call's arguments.
fn record_arguments(call: String) {
    ARGUMENTS.lock().unwrap_or_else(PoisonError::into_inner).push(call);
}

/// Returns how many calls' arguments were logged so far.
pub fn arguments_logged() -> usize {
    ARGUMENTS.lock().unwrap_or_else(PoisonError::into_inner).len()
}

/// Returns the arguments of every call logged after the first `from`, oldest first.
pub fn arguments_since(from: usize) -> Vec<String> {
    ARGUMENTS.lock().unwrap_or_else(PoisonError::into_inner).get(from..).map(<[String]>::to_vec).unwrap_or_default()
}

/// Returns every call logged so far whose text starts with `place` (a method's `Interface::Method`, or an export).
pub fn calls(place: &str) -> Vec<String> {
    LOG.lock().unwrap_or_else(PoisonError::into_inner).iter().filter(|call| call.starts_with(place) && call[place.len()..].starts_with('(')).cloned().collect()
}

/// Returns how many calls were logged so far.
pub fn logged() -> usize {
    LOG.lock().unwrap_or_else(PoisonError::into_inner).len()
}

/// Returns how many objects of this crate are alive.
pub fn live() -> i64 {
    LIVE.load(Ordering::SeqCst)
}

/// The known bytes the recorder writes into the caller's bytes (a recorder writing none uses none of it).
#[allow(dead_code, reason = "a recorder writing no caller's bytes writes none")]
mod known {
    /// Each known byte is one of `KNOWN_SPAN` values from 1: never zero.
    const KNOWN_SPAN: usize = 126;
    /// What a known byte moves by from one byte to the next, so that neighboring bytes differ.
    const KNOWN_STRIDE: usize = 29;
    /// The most bytes the recorder writes into the caller's bytes.
    const KNOWN_MOST: usize = 1 << 16;

    /// The known byte at `at` of an answer of salt `salt`.
    fn known_byte(salt: usize, at: usize) -> u8 {
        u8::try_from(1 + (salt + at * KNOWN_STRIDE) % KNOWN_SPAN).unwrap_or(1)
    }

    /// Writes known bytes over all of the caller's bytes (the first `KNOWN_MOST` of them), as the system writes what it
    /// is told of; the conformance suite compares what reaches the caller through both glues with the same call made
    /// directly.
    pub(super) fn known_fill<Bytes: wcontainers_volatile_bytes::VolatileBytes>(bytes: &Bytes, salt: usize) {
        let length = usize::try_from(bytes.len()).unwrap_or(KNOWN_MOST).min(KNOWN_MOST);
        let known: Vec<u8> = (0..length).map(|at| known_byte(salt, at)).collect();
        let _ = bytes.write(&known);
    }
}

use known::known_fill;

/// One object alive, counted until its last clone drops.
#[derive(Debug)]
struct Live(u64);

impl Live {
    fn new() -> Arc<Live> {
        LIVE.fetch_add(1, Ordering::SeqCst);
        Arc::new(Live(NEXT.fetch_add(1, Ordering::SeqCst)))
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        LIVE.fetch_sub(1, Ordering::SeqCst);
    }
}
"#;

/// The bytes the Direct3D 11 recorder lends for a Map.
pub(super) const RECORDED_BYTES: &str = r#"
/// Bytes the recorder lends for a Map: shared by every clone, zero at first.
#[derive(Clone, Debug)]
pub struct RecordedBytes(Arc<Mutex<Vec<u8>>>);

impl RecordedBytes {
    /// New zero bytes of the length every Map lends.
    pub fn new() -> RecordedBytes {
        RecordedBytes(Arc::new(Mutex::new(vec![0; MAPPED_LENGTH])))
    }
}

impl Default for RecordedBytes {
    fn default() -> RecordedBytes {
        RecordedBytes::new()
    }
}

impl wcontainers_volatile_bytes::VolatileBytes for RecordedBytes {
    fn len(&self) -> u64 {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).len() as u64
    }

    fn read_at(&self, offset: u64, into: &mut [u8]) -> usize {
        let held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let from = usize::try_from(offset).ok().and_then(|offset| held.get(offset..)).unwrap_or_default();
        let count = into.len().min(from.len());
        into[..count].copy_from_slice(&from[..count]);
        count
    }

    fn write_at(&self, offset: u64, bytes: &[u8]) -> usize {
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let to = usize::try_from(offset).ok().and_then(|offset| held.get_mut(offset..)).unwrap_or_default();
        let count = bytes.len().min(to.len());
        to[..count].copy_from_slice(&bytes[..count]);
        count
    }

    fn read_to<Sink: wcontainers_byte_sink::ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let copy = self.0.lock().unwrap_or_else(PoisonError::into_inner).clone();
        sink.write(&copy)
    }

    fn write_from<Source: wcontainers_byte_source::ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut copy = vec![u8::MIN; self.0.lock().unwrap_or_else(PoisonError::into_inner).len()];
        let count = source.read(&mut copy)?;
        Ok(self.write(&copy[..count.min(copy.len())]))
    }
}
"#;

/// Splits a list of types at its top-level commas.
pub(super) fn split_types(list: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0_i32, 0);
    for (at, ch) in list.char_indices() {
        match ch {
            '(' | '<' | '[' => depth += 1,
            ')' | '>' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(list[start..at].trim().to_string());
                start = at + 1;
            }
            _ => {}
        }
    }
    if !list[start..].trim().is_empty() {
        parts.push(list[start..].trim().to_string());
    }
    parts
}

impl Writer<'_, '_> {
    /// A value of type `ty` the recorder can answer with: empty, none, false, zero bytes decoded or a default; none
    /// for an object or a structure it cannot make.
    fn made_value(&self, ty: &str) -> Option<String> {
        let ty = ty.trim();
        if ty.starts_with("Vec<") {
            return Some("Vec::new()".to_string());
        }
        if ty.starts_with("Option<") {
            return Some("None".to_string());
        }
        if let Some(inner) = ty.strip_prefix('(').and_then(|rest| rest.strip_suffix(')')) {
            let parts: Option<Vec<String>> = split_types(inner).iter().map(|part| self.made_value(part)).collect();
            return parts.map(|parts| format!("({})", parts.join(", ")));
        }
        if let Some(value) = self.struct_values.borrow().get(ty) {
            return Some(value.clone());
        }
        // A fixed array: each element made.
        if let Some((element, _)) = ty.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')).and_then(|inner| inner.rsplit_once(';')) {
            return self.made_value(element).map(|value| format!("core::array::from_fn(|_| {value})"));
        }
        if ty.contains("::structures::") || ty.contains(" as ") || ty.starts_with('<') || ty.starts_with('[') {
            return None;
        }
        Some("Default::default()".to_string())
    }

    /// The recorder's known answer of type `ty`: never the type's default where the type has another value, so that a
    /// glue dropping, zeroing or mixing up what the implementation gave is seen (the conformance suite compares every
    /// answer through both glues with the recorder's own). Made of literals only (src/glue_hostile.rs `Shapes`), never
    /// of the glue's own decoders, so that the answer the glues are compared with shares no code with them.
    pub(super) fn known_value(&self, ty: &str, salt: usize) -> Option<String> {
        // A tuple's parts are asked one by one (`recorded_method`), each with a salt of its own.
        let ty = ty.trim();
        if let Some(value) = self.shapes.borrow().known(ty, salt, false) {
            return Some(value);
        }
        self.made_value(ty)
    }

    /// A value of an array's element the recorder answers with: zero bytes decoded, or zero.
    fn element_value(&self, element: &Part) -> String {
        match element {
            Part::StructIn { coded, .. } => self.made_value(&coded.path).unwrap_or_else(|| {
                self.failures.borrow_mut().push(format!("PANIC BODY recorder: no value of {}", coded.path));
                "Default::default()".to_string()
            }),
            _ => "Default::default()".to_string(),
        }
    }

    /// The recorder's type of a kind, named from the recorder crate of API `from`.
    fn recorded_type(&self, kind: usize, from: usize) -> String {
        let k = &self.g.kinds[kind];
        if k.api == from { format!("Recorded{}", k.name) } else { format!("{}::Recorded{}", recorder_alias(self.g.apis[k.api].folder), k.name) }
    }

    /// One recorded method: logs the call, then answers.
    #[allow(clippy::too_many_arguments, reason = "the method's own parts")]
    fn recorded_method(&self, method: &Method, glued: Option<&Glued>, rust_name: &str, inputs: &[(String, String)], result: &str, place: &str, from: usize) -> String {
        let mut text = String::new();
        let parameters: String = inputs.iter().map(|(name, ty)| format!(", {name}: {ty}")).collect();
        let returns = if result == "()" { String::new() } else { format!(" -> {result}") };
        let _ = writeln!(text, "    fn {rust_name}(&self{parameters}){returns} {{");
        for (name, _) in inputs {
            let _ = writeln!(text, "        let _ = &{name};");
        }
        let values: Vec<String> = glued
            .map(|glued| glued.parts.iter().filter(|(name, part)| matches!(part, Part::Value { .. }) && inputs.iter().any(|(input, _)| input == name)).map(|(name, _)| format!("{name}={{:?}}")).collect())
            .unwrap_or_default();
        let value_names: Vec<String> = glued
            .map(|glued| glued.parts.iter().filter(|(name, part)| matches!(part, Part::Value { .. }) && inputs.iter().any(|(input, _)| input == name)).map(|(name, _)| name.clone()).collect())
            .unwrap_or_default();
        if value_names.is_empty() {
            let _ = writeln!(text, "        record(\"{place}()\".to_string());");
        } else {
            let _ = writeln!(text, "        record(format!(\"{place}({})\", {}));", values.join(", "), value_names.join(", "));
        }
        // The recorder logs every argument it received, so that the suite can compare what reaches it through both glues
        // with what the same call made directly passes.
        if !self.hostile.get() {
            let mut shown: Vec<String> = Vec::new();
            let mut shown_values: Vec<String> = Vec::new();
            for (name, ty) in inputs {
                if let Some(presence) = presence_of(name, ty) {
                    // Objects are not compared by value: which are there is, so that a glue dropping one is seen.
                    shown.push(format!("{name}={{:?}}"));
                    shown_values.push(presence);
                } else if ty.contains("Self") || ty.contains("VolatileBytes") {
                    shown.push(format!("{name}=an object or the caller's bytes"));
                } else {
                    shown.push(format!("{name}={{:?}}"));
                    shown_values.push(name.clone());
                }
            }
            let values = shown_values.iter().map(|value| format!(", {value}")).collect::<String>();
            if values.is_empty() {
                let _ = writeln!(text, "        record_arguments(\"{place}({})\".to_string());", shown.join(", "));
            } else {
                let _ = writeln!(text, "        record_arguments(format!(\"{place}({})\"{values}));", shown.join(", "));
            }
        }
        // The hostile implementation uses every byte lent to it, as the system writes and reads all it is told of.
        if self.hostile.get() {
            for (name, ty) in inputs.iter().filter(|(_, ty)| ty.contains("VolatileBytes")) {
                if ty.trim_start().starts_with("Option<") {
                    let _ = writeln!(text, "        if let Some(bytes) = {name} {{ touch(bytes); }}");
                } else {
                    let _ = writeln!(text, "        touch({name});");
                }
            }
        }
        let fallback = if result.starts_with("Result<") {
            "Err(E_NOTIMPL)".to_string()
        } else if result == "()" {
            String::new()
        } else if result.starts_with("Option<") {
            "None".to_string()
        } else {
            "Default::default()".to_string()
        };
        let Some(glued) = glued else {
            // A slot the glue does not carry is never reached through C.
            let none = match self.made_value(result) {
                Some(value) if fallback == "Default::default()" => value,
                _ if fallback == "Default::default()" => {
                    self.failures.borrow_mut().push(format!("PANIC BODY recorder {place}: no value of {result}"));
                    "Default::default()".to_string()
                }
                _ => fallback.clone(),
            };
            let _ = writeln!(text, "        {none}\n    }}");
            return text;
        };
        // The recorder answers known values (`known_value`), the hostile implementation zeros and its lies.
        let known = !self.hostile.get();
        let salt = salt_of(place);
        // The caller's bytes are written only where the recorder answers success (on failure COM promises nothing of
        // them, and the calling glue gives nothing back).
        let mut fills = String::new();
        if known {
            for (name, ty) in inputs.iter().filter(|(_, ty)| ty.contains("VolatileBytes")) {
                if ty.trim_start().starts_with("Option<") {
                    let _ = writeln!(fills, "        if let Some(bytes) = {name} {{ known_fill(bytes, {}); }}", salt + KNOWN_STEP);
                } else {
                    let _ = writeln!(fills, "        known_fill({name}, {});", salt + KNOWN_STEP);
                }
            }
        }
        // The type of the answer at a position of the result's tuple (or the whole result's).
        let inner = result.strip_prefix("Result<").and_then(|rest| rest.strip_suffix('>')).map_or(result.to_string(), |rest| split_types(rest).first().cloned().unwrap_or_default());
        let tuple_types = inner.trim().strip_prefix('(').and_then(|rest| rest.strip_suffix(')')).map(split_types).unwrap_or_default();
        // (A tuple has two parts or more: the trait writes no tuple of one.)
        let type_at = |at: usize| if tuple_types.is_empty() { inner.clone() } else { tuple_types.get(at).cloned().unwrap_or_default() };
        let mut answers: Vec<String> = Vec::new();
        if glued.returned.is_some() {
            answers.push(if known { KNOWN_MARK.to_string() } else { "Default::default()".to_string() });
        }
        let one_of = self.g.exceptions().find_map(|exception| match exception {
            Exception::OneOf { method: m, value, list, default } if *m == place => Some((*value, *list, *default)),
            _ => None,
        });
        for ((name, part), param) in glued.parts.iter().zip(&method.params) {
            match part {
                Part::ObjectOut { kinds, .. } if kinds.len() > 1 => {
                    // An object of one of several kinds: the recorder makes the least derived, the hostile implementation the
                    // most derived, whatever the caller asked for; the calling glue asks the object which it is.
                    let made = if self.hostile.get() { kinds[0] } else { kinds[kinds.len() - 1] };
                    let interface_key = self.out_interface(param).unwrap_or_default();
                    let owner = self.g.interface_api.get(&interface_key).copied().unwrap_or(from);
                    answers.push(format!("{}::{}Object::{}({}::new())", alias(self.g.apis[owner]), self.g.trait_names[&interface_key], self.g.kinds[made].name, self.recorded_type(made, from)));
                }
                Part::ObjectOut { kinds, made } => {
                    let new = format!("{}::new()", self.recorded_type(kinds[0], from));
                    answers.push(if *made { new } else { format!("Some({new})") });
                }
                Part::ValueOut { element, .. } => match (one_of, element.as_ref()) {
                    (Some((value, list, default)), Part::Value { rust, .. }) if param.name == value => {
                        let list = crate::names::value_name(list);
                        let first = format!("{list}.first().copied().unwrap_or({rust}({}))", default.first().copied().unwrap_or(0));
                        // The hostile implementation answers a value none of a list of two or more names (1 is no
                        // feature level), so that the serving glue's check of it is reached; one value asked, it answers
                        // that one, as the recorder does.
                        answers.push(if self.hostile.get() { format!("if {list}.len() > 1 {{ {rust}({NOT_ASKED}) }} else {{ {first} }}") } else { first });
                    }
                    _ => answers.push(if known { KNOWN_MARK.to_string() } else { "Default::default()".to_string() }),
                },
                Part::LentOut { coded, fields: [data, row, depth], .. } => {
                    let path = coded.path.trim_end_matches("<usize>");
                    if self.hostile.get() {
                        answers.push(format!("{path} {{ {data}: RecordedBytes::new(), {row}: lying_pitch(MAPPED_ROW_PITCH), {depth}: lying_pitch(MAPPED_DEPTH_PITCH) }}"));
                    } else {
                        answers.push(format!("{path} {{ {data}: RecordedBytes::new(), {row}: MAPPED_ROW_PITCH, {depth}: MAPPED_DEPTH_PITCH }}"));
                    }
                }
                Part::FixedOut { element, .. } => {
                    let value = self.element_value(element);
                    answers.push(if known { KNOWN_MARK.to_string() } else { format!("core::array::from_fn(|_| {value})") });
                }
                Part::ValuesOut { element, count, by_pointer, .. } => {
                    let value = self.element_value(element);
                    // A few elements: the room C gave (or asked how many of) is the glue's to respect, not the recorder's.
                    // A count the trait drops (it is an array in's length) is not the recorder's to read.
                    let number = if *by_pointer || !matches!(glued.parts[*count].1, Part::Value { .. }) { "2_u32".to_string() } else { if self.hostile.get() { format!("{}.min(4)", glued.parts[*count].0) } else { glued.parts[*count].0.clone() } };
                    answers.push(if known { format!("{KNOWN_MARK}{number}") } else { format!("(0..{number}).map(|_| {value}).collect()") });
                }
                Part::ObjectsOut { kind, count } => {
                    let new = format!("{}::new()", self.recorded_type(*kind, from));
                    // As many as asked for (the recorder answers what a system answers; a known count is at most 126).
                    answers.push(format!("(0..{}).map(|_| Some({new})).collect()", glued.parts[*count].0));
                }
                Part::InterfaceOut { riid } => {
                    // An object of the kind the interface id names, of the API whose interface holds the method.
                    let owner = place.split_once("::").and_then(|(interface, _)| self.g.interface_api.iter().find(|(k, _)| k.1 == interface).map(|(_, &a)| a)).unwrap_or(self.current.get());
                    let riid_name = &glued.parts[*riid].0;
                    let mut arms = String::new();
                    for kind in (0..self.g.kinds.len()).filter(|&k| self.g.kinds[k].api == owner) {
                        let k = &self.g.kinds[kind];
                        let iids: Vec<String> = k.ancestors.iter().filter(|a| self.g.interface_api.contains_key(*a)).map(|a| self.iid(a)).collect();
                        let _ = write!(arms, "if [{}].contains({riid_name}) {{ {}::UnknownObject::{}({}::new()) }} else ", iids.join(", "), alias(self.g.apis[owner]), k.name, self.recorded_type(kind, from));
                    }
                    let _ = write!(arms, "{{ return Err(E_NOINTERFACE); }}");
                    answers.push(arms);
                }
                Part::StructOut { .. } if known && self.known_value(&type_at(answers.len()), salt).is_some() => {
                    // A structure out among the answers: a known one.
                    answers.push(KNOWN_MARK.to_string());
                    let _ = name;
                }
                // (A result the recorder can make is never one whose fallback is an error or none: those are answered
                // the same by the next arm.)
                Part::StructOut { .. } if self.made_value(result).is_some() => {
                    // A structure the recorder can make: known bytes decoded (the hostile implementation: zero bytes), or its
                    // default.
                    let value = if known { self.known_value(result, salt) } else { self.made_value(result) }.unwrap_or_default();
                    let _ = writeln!(text, "        {value}\n    }}");
                    let _ = name;
                    return text;
                }
                Part::StructOut { .. } | Part::BoundOut { .. } => {
                    let none = if fallback == "Default::default()" {
                        self.failures.borrow_mut().push(format!("PANIC BODY recorder {place}: no structure or bound object of {result}"));
                        "Default::default()".to_string()
                    } else {
                        fallback.clone()
                    };
                    let _ = writeln!(text, "        // A structure or a bound object: the recorder makes none.\n        {none}\n    }}");
                    let _ = name;
                    return text;
                }
                _ => {}
            }
        }
        // Each known answer's type: the result's, or its tuple's part.
        if answers.iter().any(|answer| answer.starts_with(KNOWN_MARK)) {
            let types = if answers.len() == 1 { vec![inner.clone()] } else { tuple_types.clone() };
            for (at, answer) in answers.iter_mut().enumerate() {
                let Some(number) = answer.strip_prefix(KNOWN_MARK) else { continue };
                let ty = types.get(at).cloned().unwrap_or_default();
                let part_salt = salt + KNOWN_STEP * (at + 1);
                let made = if number.is_empty() {
                    self.known_value(&ty, part_salt)
                } else {
                    let element = ty.trim().strip_prefix("Vec<").and_then(|rest| rest.strip_suffix('>')).unwrap_or_default().to_string();
                    self.known_value(&element, part_salt).map(|value| format!("(0..{number}).map(|_| {value}).collect()"))
                };
                *answer = made.unwrap_or_else(|| {
                    self.failures.borrow_mut().push(format!("PANIC BODY recorder {place}: no known value of {ty}"));
                    "Default::default()".to_string()
                });
            }
        }
        let value = match answers.len() {
            // A not-yet exception line: the recorder's data is always ready, the hostile implementation's never (it has
            // touched the bytes all the same), so that both answers cross both glues.
            0 if glued.not_yet.is_some() => (!self.hostile.get()).to_string(),
            0 => "()".to_string(),
            1 => answers.remove(0),
            _ => format!("({})", answers.join(", ")),
        };
        text.push_str(&fills);
        if glued.status {
            let _ = writeln!(text, "        Ok({value})\n    }}");
        } else if value == "()" {
            text.push_str("    }\n");
        } else {
            let _ = writeln!(text, "        {value}\n    }}");
        }
        text
    }

    /// The text of API `api`'s recorder crate.
    pub(super) fn recorder(&self, api: usize) -> String {
        let g = self.g;
        let spec = g.apis[api];
        let top = g.top_path(api, usize::MAX);
        let mut text = String::new();
        let _ = writeln!(text, "//! {HEADER}\n//!\n//! A recording implementation of {}'s traits, for tests only: every call is logged by its place, with the values", spec.folder);
        let _ = writeln!(text, "//! passed by value; an object asked for is made; a structure, a bound object or a list asked for is not (E_NOTIMPL).\n");
        text.push_str("#![allow(unused_variables, unused_imports, clippy::let_unit_value, clippy::unit_arg, clippy::new_without_default, reason = \"generated: every method has the same shape\")]\n\n");
        let hostile = self.hostile.get();
        text.push_str(if hostile { super::hostile::HOSTILE_BASE } else { RECORDER_BASE });
        let lends = spec.exceptions.iter().any(|e| matches!(e, Exception::Lent { .. }));
        if lends {
            let _ = writeln!(text, "\n/// The bytes every Map lends, and the pitches it answers.\nconst MAPPED_LENGTH: usize = {};\nconst MAPPED_ROW_PITCH: u32 = {};\nconst MAPPED_DEPTH_PITCH: u32 = {};", MAPPED.0, MAPPED.1, MAPPED.2);
            text.push_str(if hostile { super::hostile::HOSTILE_BYTES } else { RECORDED_BYTES });
        }
        let own_kinds: Vec<usize> = (0..g.kinds.len()).filter(|&k| g.kinds[k].api == api).collect();
        for &kind in &own_kinds {
            let name = format!("Recorded{}", g.kinds[kind].name);
            let _ = writeln!(text, "\n/// A recorded object of the kind `{}`.\npub struct {name}<A> {{\n    live: Arc<Live>,\n    api: PhantomData<fn() -> A>,\n}}\n", g.kinds[kind].name);
            let _ = writeln!(text, "impl<A> {name}<A> {{\n    /// A new object, of an identity of its own.\n    pub fn new() -> {name}<A> {{\n        {name} {{ live: Live::new(), api: PhantomData }}\n    }}\n\n    /// The object's identity.\n    pub fn identity(&self) -> u64 {{\n        self.live.0\n    }}\n}}\n");
            if self.hostile.get() {
                // The glue never asks these (src/trust.rs): each panics, so that a call of one fails the tests.
                let _ = writeln!(text, "impl<A> Clone for {name}<A> {{\n    fn clone(&self) -> {name}<A> {{\n        never_asked(CLONE_NAME)\n    }}\n}}\n");
                let _ = writeln!(text, "impl<A> PartialEq for {name}<A> {{\n    fn eq(&self, other: &{name}<A>) -> bool {{\n        never_asked(PARTIAL_EQ_NAME)\n    }}\n}}\n\nimpl<A> Eq for {name}<A> {{}}\n");
                let _ = writeln!(text, "impl<A> Hash for {name}<A> {{\n    fn hash<H: Hasher>(&self, state: &mut H) {{\n        never_asked(HASH_NAME)\n    }}\n}}\n");
            } else {
                let _ = writeln!(text, "impl<A> Clone for {name}<A> {{\n    fn clone(&self) -> {name}<A> {{\n        {name} {{ live: Arc::clone(&self.live), api: PhantomData }}\n    }}\n}}\n");
                let _ = writeln!(text, "impl<A> PartialEq for {name}<A> {{\n    fn eq(&self, other: &{name}<A>) -> bool {{\n        self.live.0 == other.live.0\n    }}\n}}\n\nimpl<A> Eq for {name}<A> {{}}\n");
                let _ = writeln!(text, "impl<A> Hash for {name}<A> {{\n    fn hash<H: Hasher>(&self, state: &mut H) {{\n        self.live.0.hash(state);\n    }}\n}}\n");
            }
            let _ = writeln!(text, "impl<A> core::fmt::Debug for {name}<A> {{\n    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {{\n        formatter.debug_tuple(\"{name}\").field(&self.live.0).finish()\n    }}\n}}");
        }
        // The trait every API value of the recorder meets: its kinds are this crate's types.
        let mut bindings: Vec<String> = own_kinds.iter().map(|&k| format!("{} = Recorded{}<Self>", g.kinds[k].name, g.kinds[k].name)).collect();
        for exception in spec.exceptions {
            match exception {
                Exception::BoundObject { name, kind, .. } => {
                    let _ = kind;
                    let _ = name;
                }
                Exception::Lent { .. } if !bindings.iter().any(|b| b.starts_with("Mapped")) => bindings.push("Mapped = RecordedBytes".to_string()),
                _ => {}
            }
        }
        let supers: Vec<String> = spec.supertraits.iter().map(|folder| format!("{}::Recorded", recorder_alias(folder))).collect();
        let mut bound = format!("{top}<{}>", bindings.join(", "));
        for s in &supers {
            bound.push_str(&format!(" + {s}"));
        }
        let _ = writeln!(text, "\n/// An API value whose objects are this recorder's.\npub trait Recorded: {bound} {{}}\n");
        // Each kind's traits, as the calling glue implements them.
        for &kind in &own_kinds {
            let k = &g.kinds[kind];
            let name = format!("Recorded{}", k.name);
            let mut traits: Vec<Key> = Vec::new();
            for newest in &k.newest {
                for base in g.bases_of(newest) {
                    if g.interface_api.contains_key(&base) && !traits.contains(&base) {
                        traits.push(base);
                    }
                }
            }
            let mut implemented: Vec<Key> = traits.clone();
            for interface_key in &traits {
                let root_has_api = !g.apiless.borrow().contains(&g.chain_root(interface_key));
                let is_root = g.chain_root(interface_key) == *interface_key;
                let _ = writeln!(text, "impl<A: Recorded> {} for {name}<A> {{", self.trait_path(interface_key));
                if is_root && root_has_api {
                    let _ = writeln!(text, "    type Api = A;\n");
                }
                for slot in self.slots(interface_key).iter().filter(|slot| slot.interface == *interface_key && g.read.interfaces[interface_key].methods.iter().any(|m| m.name == slot.method.name)) {
                    let place = format!("{}::{}", interface_key.1, slot.method.name);
                    let (inputs, result) = self.trait_signature(slot.method, api, "Self::Api", &place);
                    text.push_str(&self.recorded_method(slot.method, slot.glued.as_ref(), &Self::method_name(slot.method), &inputs, &result, &place, api));
                }
                let _ = writeln!(text, "}}\n");
            }
            let faces: Vec<Key> = spec
                .faces_of(&k.root.1)
                .into_iter()
                .filter_map(|face| g.interface_api.keys().find(|key| key.1 == *face).cloned())
                .collect();
            for face in &faces {
                let face_api = g.interface_api[face];
                for interface_key in self.chain(face) {
                    if implemented.contains(&interface_key) {
                        continue;
                    }
                    implemented.push(interface_key.clone());
                    let root_has_api = !g.apiless.borrow().contains(&g.chain_root(&interface_key));
                    let is_root = g.chain_root(&interface_key) == interface_key;
                    let _ = writeln!(text, "impl<A: Recorded> {} for {name}<A> {{", self.trait_path(&interface_key));
                    if is_root && root_has_api {
                        let _ = writeln!(text, "    type Api = A;\n");
                    }
                    let saved = self.current.replace(face_api);
                    let slots = self.slots(face);
                    for slot in slots.iter().filter(|slot| slot.interface == interface_key && g.read.interfaces[&interface_key].methods.iter().any(|m| m.name == slot.method.name)) {
                        let place = format!("{}::{}", interface_key.1, slot.method.name);
                        let (inputs, result) = self.trait_signature(slot.method, face_api, "Self::Api", &place);
                        text.push_str(&self.recorded_method(slot.method, slot.glued.as_ref(), &Self::method_name(slot.method), &inputs, &result, &place, api));
                    }
                    self.current.set(saved);
                    let _ = writeln!(text, "}}\n");
                }
            }
        }
        // The API value: the library's exports, and those of the APIs it extends.
        if let Some(top_name) = spec.top_trait {
            let _ = writeln!(text, "/// The recorder's library: `{top_name}` and every API it extends.\n#[derive(Clone, Copy, Debug, Default)]\npub struct Recorder;\n");
            let mut top_bindings: Vec<String> = own_kinds.iter().map(|&k| format!("    type {} = Recorded{}<Self>;", g.kinds[k].name, g.kinds[k].name)).collect();
            for exception in spec.exceptions {
                match exception {
                    Exception::BoundObject { name, kind, .. } => {
                        if let Some(&kind_index) = g.interface_api.keys().find(|k| k.1 == *kind).and_then(|k| g.kind_of.get(k)) {
                            top_bindings.push(format!("    type {name} = Recorded{}<Self>;", g.kinds[kind_index].name));
                        }
                    }
                    Exception::Lent { .. } if !top_bindings.iter().any(|b| b.contains("type Mapped")) => top_bindings.push("    type Mapped = RecordedBytes;".to_string()),
                    _ => {}
                }
            }
            let _ = writeln!(text, "impl {top} for Recorder {{\n{}\n", top_bindings.join("\n"));
            for function in self.exported(api) {
                let rust_name = crate::names::escaped(super::snake(crate::names::strip_first(&function.name, spec.function_prefixes)));
                let mut renamed = function.clone();
                renamed.name = crate::names::strip_first(&function.name, spec.function_prefixes).to_string();
                let (inputs, result) = self.trait_signature(&renamed, api, "Self", &function.name);
                let glued = self.glue_of(function, &function.name).ok();
                text.push_str(&self.recorded_method(function, glued.as_ref(), &rust_name, &inputs, &result, &function.name, api));
            }
            let _ = writeln!(text, "}}\n\nimpl Recorded for Recorder {{}}\n");
            for folder in spec.supertraits {
                let Some(other) = g.apis.iter().position(|a| a.folder == *folder) else { continue };
                let other_top = g.top_path(other, usize::MAX);
                let other_recorder = recorder_alias(folder);
                let mut lines: Vec<String> = (0..g.kinds.len()).filter(|&k| g.kinds[k].api == other).map(|k| format!("    type {} = {other_recorder}::Recorded{}<Self>;", g.kinds[k].name, g.kinds[k].name)).collect();
                for exception in spec.exceptions {
                    if let Exception::Binds { api: bound_api, name, kind } = exception
                        && bound_api == folder
                        && let Some(&kind_index) = g.interface_api.keys().find(|k| k.1 == *kind).and_then(|k| g.kind_of.get(k))
                    {
                        lines.push(format!("    type {name} = Recorded{}<Self>;", g.kinds[kind_index].name));
                    }
                }
                let mut functions = String::new();
                let other_spec = g.apis[other];
                let saved = self.current.replace(other);
                for function in self.exported(other) {
                    let rust_name = crate::names::escaped(super::snake(crate::names::strip_first(&function.name, other_spec.function_prefixes)));
                    let mut renamed = function.clone();
                    renamed.name = crate::names::strip_first(&function.name, other_spec.function_prefixes).to_string();
                    let (inputs, result) = self.trait_signature(&renamed, other, "Self", &function.name);
                    let glued = self.glue_of(function, &function.name).ok();
                    // The other API's objects are of its own recorder crate.
                    functions.push_str(&self.recorded_method(function, glued.as_ref(), &rust_name, &inputs, &result, &function.name, api));
                }
                self.current.set(saved);
                let _ = writeln!(text, "impl {other_top} for Recorder {{\n{}\n{functions}}}\n", lines.join("\n"));
                let _ = writeln!(text, "impl {other_recorder}::Recorded for Recorder {{}}\n");
            }
        }
        text
    }

    /// The exports of API `api` its top-level trait carries, by name.
    pub(super) fn exported(&self, api: usize) -> Vec<&Method> {
        let spec = self.g.apis[api];
        let mut listed: Vec<&Method> = self
            .g
            .read
            .functions
            .iter()
            .filter(|((namespace, _), _)| spec.namespaces.contains(&namespace.as_str()))
            .map(|(_, function)| function)
            .filter(|function| spec.library.is_some_and(|library| function.library.as_deref().is_some_and(|l| l.eq_ignore_ascii_case(library))))
            .filter(|function| spec.only.is_empty() || spec.only.contains(&function.name.as_str()))
            .collect();
        listed.sort_by(|a, b| a.name.cmp(&b.name));
        listed
    }
}

/// Writes API `api`'s recorder crate under its folder, and returns the report's line.
pub(in crate::emit) fn write_recorder(writer: &Writer<'_, '_>, api: usize, crate_folder: &Path) -> Result<String, String> {
    let g = writer.g;
    writer.current.set(api);
    let spec = g.apis[api];
    let crates = crate_folder.parent().ok_or("no crates folder")?;
    let modules = super::hostile::struct_values(writer, crates, api);
    let text = writer.recorder(api);
    writer.struct_values.borrow_mut().clear();
    let mut text = with_recorder_imports(g, &text);
    // The direct crates' data modules, for the structures the recorder makes from zero bytes.
    let included: String = modules.iter().filter(|(module, _)| text.contains(&format!("{module}::"))).map(|(module, from)| format!("#[path = \"{from}\"]\nmod {module};\n")).collect();
    if !included.is_empty() {
        let lines: Vec<&str> = text.lines().collect();
        let at = lines.iter().position(|line| !(line.starts_with("//!") || line.starts_with("#![") || line.is_empty())).unwrap_or(lines.len());
        text = format!("{}\n{included}\n{}\n", lines[..at].join("\n"), lines[at..].join("\n"));
    }
    let folder = crate_folder.join("implementation").join("recorder");
    std::fs::create_dir_all(&folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    std::fs::write(folder.join("recorder.rs"), &text).map_err(|error| error.to_string())?;
    let mut manifest = format!(
        "[package]\nname = \"{}-recorder\"\ndescription = \"A recording implementation of {}'s traits, for tests only.\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[lib]\npath = \"recorder.rs\"\n\n[dependencies]\n",
        package(spec),
        spec.folder
    );
    let _ = writeln!(manifest, "{} = {{ path = \"../..\" }}", package(spec));
    for (other, other_spec) in g.apis.iter().enumerate() {
        if other == api {
            continue;
        }
        if text.contains(&format!("{}::", alias(other_spec))) || other == 0 {
            let _ = writeln!(manifest, "{} = {{ path = \"../../../{}\" }}", package(other_spec), other_spec.folder);
        }
        if text.contains(&format!("{}::", recorder_alias(other_spec.folder))) {
            let _ = writeln!(manifest, "{}-recorder = {{ path = \"../../../{}/implementation/recorder\" }}", package(other_spec), other_spec.folder);
        }
    }
    let _ = writeln!(manifest, "wcontainers-byte-sink = {{ path = \"../../../byte-sink\" }}\nwcontainers-byte-source = {{ path = \"../../../byte-source\" }}\nwcontainers-volatile-bytes = {{ path = \"../../../volatile-bytes\" }}");
    // The direct crates of the set: their data modules make structures, and the hostile tests reach the recorder
    // through both glues.
    for other_spec in g.apis.iter() {
        if !manifest.contains(&format!("\n{} =", package(other_spec))) {
            let _ = writeln!(manifest, "{} = {{ path = \"../../../{}\" }}", package(other_spec), other_spec.folder);
        }
        let _ = writeln!(manifest, "{}-direct = {{ path = \"../../../{}/implementation/direct\" }}", package(other_spec), other_spec.folder);
    }
    let _ = writeln!(manifest, "\n[dev-dependencies]");
    manifest.push_str("\n[lints]\nworkspace = true\n");
    std::fs::write(folder.join("Cargo.toml"), manifest).map_err(|error| error.to_string())?;
    Ok(format!("RECORDER {}: written\n", spec.folder))
}

/// Adds `use` lines for every crate of the set a recorder names by alias.
fn with_recorder_imports(g: &crate::emit::Generator<'_>, text: &str) -> String {
    let mut imports = Vec::new();
    let named = |needle: &str| text.lines().any(|line| !line.starts_with("use ") && !line.starts_with("//") && line.contains(needle));
    for api in g.apis.iter() {
        if named(&format!("{}::", alias(api))) {
            imports.push(format!("use {} as {};", package(api).replace('-', "_"), alias(api)));
        }
        if named(&format!("{}::", recorder_alias(api.folder))) {
            imports.push(format!("use {}_recorder as {};", package(api).replace('-', "_"), recorder_alias(api.folder)));
        }
        let _ = direct_alias(api.folder);
    }
    let lines: Vec<&str> = text.lines().collect();
    let at = lines.iter().position(|line| !(line.starts_with("//!") || line.starts_with("#![") || line.is_empty())).unwrap_or(lines.len());
    let mut out = lines[..at].join("\n");
    out.push('\n');
    out.push_str(&imports.join("\n"));
    out.push_str("\n\n");
    out.push_str(&lines[at..].join("\n"));
    out.push('\n');
    out
}
