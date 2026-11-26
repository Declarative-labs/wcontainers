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

/// Logs one call.
fn record(call: String) {
    LOG.lock().unwrap_or_else(PoisonError::into_inner).push(call);
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

    fn read(&self, into: &mut [u8]) -> usize {
        let held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let count = into.len().min(held.len());
        into[..count].copy_from_slice(&held[..count]);
        count
    }

    fn write(&self, bytes: &[u8]) -> usize {
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let count = bytes.len().min(held.len());
        held[..count].copy_from_slice(&bytes[..count]);
        count
    }

    fn read_to<Sink: wcontainers_byte_sink::ByteSink>(&self, sink: &mut Sink) -> Result<usize, Sink::Error> {
        let copy = self.0.lock().unwrap_or_else(PoisonError::into_inner).clone();
        sink.write(&copy)
    }

    fn write_from<Source: wcontainers_byte_source::ByteSource>(&self, source: &mut Source) -> Result<usize, Source::Error> {
        let mut copy = vec![0_u8; self.0.lock().unwrap_or_else(PoisonError::into_inner).len()];
        let count = source.read(&mut copy)?;
        Ok(self.write(&copy[..count.min(copy.len())]))
    }
}
"#;

/// Splits a list of types at its top-level commas.
fn split_types(list: &str) -> Vec<String> {
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
                _ if fallback == "Default::default()" => format!("unimplemented!(\"{place} is not glued\")"),
                _ => fallback.clone(),
            };
            let _ = writeln!(text, "        {none}\n    }}");
            return text;
        };
        let mut answers: Vec<String> = Vec::new();
        if glued.returned.is_some() {
            answers.push("Default::default()".to_string());
        }
        let one_of = self.g.exceptions().find_map(|exception| match exception {
            Exception::OneOf { method: m, value, list, default } if *m == place => Some((*value, *list, *default)),
            _ => None,
        });
        for ((name, part), param) in glued.parts.iter().zip(&method.params) {
            match part {
                Part::ObjectOut { kinds, made } => {
                    let new = format!("{}::new()", self.recorded_type(kinds[0], from));
                    answers.push(if *made { new } else { format!("Some({new})") });
                }
                Part::ValueOut { element, .. } => match (one_of, element.as_ref()) {
                    (Some((value, list, default)), Part::Value { rust, .. }) if param.name == value => {
                        let list = crate::names::value_name(list);
                        answers.push(format!("{list}.first().copied().unwrap_or({rust}({}))", default.first().copied().unwrap_or(0)));
                    }
                    _ => answers.push("Default::default()".to_string()),
                },
                Part::LentOut { coded, fields: [data, row, depth], .. } => {
                    let path = coded.path.trim_end_matches("<usize>");
                    if self.hostile.get() {
                        answers.push(format!("{path} {{ {data}: RecordedBytes::new(), {row}: lying_pitch(MAPPED_ROW_PITCH), {depth}: lying_pitch(MAPPED_DEPTH_PITCH) }}"));
                    } else {
                        answers.push(format!("{path} {{ {data}: RecordedBytes::new(), {row}: MAPPED_ROW_PITCH, {depth}: MAPPED_DEPTH_PITCH }}"));
                    }
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
                Part::StructOut { .. } if fallback == "Default::default()" && self.made_value(result).is_some() => {
                    // A structure the recorder can make: zero bytes decoded, or its default.
                    let value = self.made_value(result).unwrap_or_default();
                    let _ = writeln!(text, "        {value}\n    }}");
                    let _ = name;
                    return text;
                }
                Part::StructOut { .. } | Part::BoundOut { .. } => {
                    let none = if fallback == "Default::default()" { format!("unimplemented!(\"the recorder makes no structure for {place}\")") } else { fallback.clone() };
                    let _ = writeln!(text, "        // A structure or a bound object: the recorder makes none.\n        {none}\n    }}");
                    let _ = name;
                    return text;
                }
                _ => {}
            }
        }
        let value = match answers.len() {
            0 => "()".to_string(),
            1 => answers.remove(0),
            _ => format!("({})", answers.join(", ")),
        };
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
                let _ = writeln!(text, "impl<A> Clone for {name}<A> {{\n    fn clone(&self) -> {name}<A> {{\n        never_asked(\"Clone\")\n    }}\n}}\n");
                let _ = writeln!(text, "impl<A> PartialEq for {name}<A> {{\n    fn eq(&self, other: &{name}<A>) -> bool {{\n        never_asked(\"PartialEq\")\n    }}\n}}\n\nimpl<A> Eq for {name}<A> {{}}\n");
                let _ = writeln!(text, "impl<A> Hash for {name}<A> {{\n    fn hash<H: Hasher>(&self, state: &mut H) {{\n        never_asked(\"Hash\")\n    }}\n}}\n");
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
                .faces
                .iter()
                .filter(|(root, _)| *root == k.root.1)
                .flat_map(|(_, faces)| faces.iter())
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
