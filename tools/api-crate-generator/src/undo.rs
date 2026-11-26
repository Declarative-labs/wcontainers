//! What each marked `unsafe` of the direct crates relies on, and every method of the APIs that could undo it.
//!
//! Every mark's reason is matched to the facts it relies on (`REASONS`); a reason no line matches fails the
//! generator. Each fact has a search over the whole metadata of every API of the set (not only a slice) for the
//! methods that could switch it off, weaken it or replace it; each method found needs a pin in its API's `pins`
//! (src/api.rs): the calling glue answers it, clears or refuses the undoing value, never calls it, or the method is
//! shown harmless. A method found and not pinned fails the generator, and so does a pin of a method the calling glue
//! carries whose hostile test is missing from the harness.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use windows_metadata::Type;
use windows_metadata::reader::TypeIndex;

use crate::api::{Api, Hold, Pin};
use crate::metadata::{self, Literal, Method, Namespaces, RecordKind, attribute_named};

/// A fact a marked `unsafe` relies on.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Reliance {
    /// A table's or a record's layout is the one the metadata declares.
    Layout,
    /// An object the glue holds a counted reference to stays live, and `Release` gives back exactly that reference.
    LiveObject,
    /// The system's objects may be called and released from any thread (`ForeignObject`'s `Send` and `Sync`).
    FreeThreaded,
    /// A call into the system returns to its caller: it raises no exception through our frames.
    ReturnsNormally,
    /// Memory the system lends (Map) stays valid until the glue's own call of its end (Unmap).
    LentUntilEnd,
    /// The system runs no code of ours except through the COM objects we handed it.
    NoCallback,
    /// The C caller's pointers (the serving glue's promise, made by whoever calls our exports).
    CallerPromise,
    /// The functions the calling glue was built with are the library's exports, loaded while it lives.
    LibraryExport,
    /// The glue's key is held only by the glue.
    GlueKey,
    /// The harness's own C stand-ins and programs, which are no API.
    StandIn,
}

impl Reliance {
    pub const ALL: [Reliance; 10] = [
        Reliance::Layout,
        Reliance::LiveObject,
        Reliance::FreeThreaded,
        Reliance::ReturnsNormally,
        Reliance::LentUntilEnd,
        Reliance::NoCallback,
        Reliance::CallerPromise,
        Reliance::LibraryExport,
        Reliance::GlueKey,
        Reliance::StandIn,
    ];

    /// What is relied on.
    pub fn statement(self) -> &'static str {
        match self {
            Reliance::Layout => "a table's or record's layout is the one the metadata declares",
            Reliance::LiveObject => "an object we hold a counted reference to stays live; Release gives back exactly ours",
            Reliance::FreeThreaded => "the system's objects may be called and released from any thread (Send and Sync)",
            Reliance::ReturnsNormally => "a call into the system returns: no exception is raised through our frames",
            Reliance::LentUntilEnd => "memory the system lends stays valid until the glue's own call of its end",
            Reliance::NoCallback => "the system runs no code of ours except through the COM objects we handed it",
            Reliance::CallerPromise => "the C caller's pointers, as the header declares them",
            Reliance::LibraryExport => "the calling glue's functions are the library's exports, loaded while it lives",
            Reliance::GlueKey => "the glue's key is held only by the glue",
            Reliance::StandIn => "the harness's own C stand-ins, which are no API",
        }
    }

    /// How the methods that could undo it are found.
    pub fn search(self) -> &'static str {
        match self {
            Reliance::Layout => "none can: a layout is fixed when the header is compiled",
            Reliance::LiveObject => {
                "names with Release, Destroy, Free, Trim, Discard, Offer, Evict or PrivateData; parameters of a flags type with a DEBUG value (a debug layer)"
            }
            Reliance::FreeThreaded => {
                "every method of an interface named Multithread; names with Deferred or DeviceContextState; parameters of a flags type with a SINGLETHREADED value"
            }
            Reliance::ReturnsNormally => "names with ExceptionMode or BreakOn",
            Reliance::LentUntilEnd => {
                "names with Map (any case), GetDC, ReleaseDC, Discard, Offer, Reclaim, UseRef, Resize, Trim, Evict or DecoderBuffer"
            }
            Reliance::NoCallback => "parameters naming a function pointer (a delegate) and names with Callback",
            Reliance::CallerPromise => "none: the serving glue's caller is the C side, which no method of ours can change",
            Reliance::LibraryExport => "none: no method of these APIs unloads its library",
            Reliance::GlueKey => "none: the key is a private static of the glue",
            Reliance::StandIn => "none: the stand-ins are the harness's own C",
        }
    }

    /// Whether `method` of `interface` (empty for an export) is found by the search.
    fn found(self, interface: &str, method: &Method, read: &Namespaces) -> bool {
        let name = method.name.as_str();
        let has = |part: &str| name.contains(part);
        match self {
            Reliance::Layout | Reliance::CallerPromise | Reliance::LibraryExport | Reliance::GlueKey | Reliance::StandIn => false,
            Reliance::LiveObject => {
                ["Release", "Destroy", "Free", "Trim", "Discard", "Offer", "Evict", "PrivateData"].iter().any(|part| has(part))
                    || flag_value(method, read, "DEBUG")
            }
            Reliance::FreeThreaded => {
                interface.contains("Multithread") || has("Deferred") || has("DeviceContextState") || flag_value(method, read, "SINGLETHREAD")
            }
            Reliance::ReturnsNormally => has("ExceptionMode") || has("BreakOn"),
            Reliance::LentUntilEnd => {
                name.to_ascii_lowercase().contains("map")
                    || ["GetDC", "ReleaseDC", "Discard", "Offer", "Reclaim", "UseRef", "Resize", "Trim", "Evict", "DecoderBuffer"]
                        .iter()
                        .any(|part| has(part))
            }
            Reliance::NoCallback => has("Callback") || method.params.iter().any(|param| names_delegate(&param.ty, read)),
        }
    }
}

/// Whether a parameter of `method` is of a flags type (by its type or its `AssociatedEnum` attribute) one of whose
/// values' names holds `part`.
fn flag_value(method: &Method, read: &Namespaces, part: &str) -> bool {
    method.params.iter().any(|param| {
        let by_type = match inner(&param.ty) {
            Type::Name(name) => Some(name.name.clone()),
            _ => None,
        };
        let by_attribute = attribute_named(&param.attributes, "AssociatedEnum").and_then(|attribute| {
            attribute.arguments.iter().find_map(|(_, value)| match value {
                Literal::Text(text) => Some(text.clone()),
                _ => None,
            })
        });
        [by_type, by_attribute].into_iter().flatten().any(|enum_name| {
            read.records
                .values()
                .any(|record| record.kind == RecordKind::Enum && record.name == enum_name && record.fields.iter().any(|field| field.name.contains(part)))
        })
    })
}

fn inner(ty: &Type) -> &Type {
    match ty {
        Type::PtrConst(inner, _) | Type::PtrMut(inner, _) => inner,
        other => other,
    }
}

fn names_delegate(ty: &Type, read: &Namespaces) -> bool {
    match inner(ty) {
        Type::Name(name) => read
            .records
            .get(&(name.namespace.clone(), name.name.clone()))
            .is_some_and(|record| record.kind == RecordKind::Delegate),
        _ => false,
    }
}

/// How a mark's reason is matched.
#[derive(Clone, Copy)]
enum Match {
    Ends(&'static str),
    Is(&'static str),
    Contains(&'static str),
}

/// Every mark reason the generator and its templates write, each with what it relies on; the first line that matches
/// decides.
const REASONS: &[(Match, &[Reliance])] = &[
    (Match::Ends(", the library's export, called out"), &[Reliance::LibraryExport, Reliance::Layout, Reliance::ReturnsNormally, Reliance::NoCallback, Reliance::FreeThreaded]),
    (Match::Ends(", called out"), &[Reliance::Layout, Reliance::LiveObject, Reliance::FreeThreaded, Reliance::ReturnsNormally, Reliance::NoCallback]),
    (Match::Contains("the system's Unmap of memory no view holds"), &[Reliance::Layout, Reliance::LiveObject, Reliance::LentUntilEnd, Reliance::ReturnsNormally]),
    (Match::Contains("memory the system lends"), &[Reliance::LentUntilEnd, Reliance::LiveObject]),
    (Match::Contains(", which the C side calls"), &[Reliance::CallerPromise, Reliance::Layout]),
    (Match::Contains("as the header declares it for the C side"), &[Reliance::CallerPromise, Reliance::Layout]),
    (Match::Contains("as the header declares them for the C side"), &[Reliance::CallerPromise, Reliance::Layout]),
    (Match::Contains("Out and ComOutPtr"), &[Reliance::Layout, Reliance::LiveObject]),
    (Match::Contains("an object the metadata's Out hands over"), &[Reliance::Layout, Reliance::LiveObject]),
    (Match::Contains("QueryInterface for"), &[Reliance::Layout, Reliance::LiveObject, Reliance::ReturnsNormally]),
    (Match::Contains("a live object of"), &[Reliance::Layout, Reliance::LiveObject]),
    (Match::Contains("a live COM object"), &[Reliance::Layout, Reliance::LiveObject]),
    (Match::Contains("the promise that a pointer is a live"), &[Reliance::Layout, Reliance::LiveObject]),
    (Match::Contains("the promise a table's layout and its objects' threads make"), &[Reliance::Layout, Reliance::FreeThreaded]),
    (Match::Contains("free threaded by its interface's promise"), &[Reliance::FreeThreaded, Reliance::LiveObject]),
    (Match::Contains("the layout and threads of the table of"), &[Reliance::Layout, Reliance::FreeThreaded]),
    (Match::Is("IUnknown's table"), &[Reliance::Layout, Reliance::FreeThreaded]),
    (Match::Contains("the answer of QueryInterface"), &[Reliance::Layout, Reliance::LiveObject]),
    (Match::Contains("an interface id and the table it names"), &[Reliance::Layout]),
    (Match::Contains("the first word of a live COM object"), &[Reliance::Layout, Reliance::LiveObject]),
    (Match::Contains("a table of entry points the C side calls"), &[Reliance::CallerPromise, Reliance::Layout]),
    (Match::Contains("a table the C side calls through"), &[Reliance::CallerPromise, Reliance::Layout]),
    (Match::Contains("the C side passes in"), &[Reliance::CallerPromise]),
    (Match::Contains("from the C side"), &[Reliance::CallerPromise]),
    (Match::Contains("C's read-only bytes"), &[Reliance::CallerPromise]),
    (Match::Contains("the C library's memcpy"), &[Reliance::LibraryExport]),
    (Match::Contains("IUnknown::QueryInterface's"), &[Reliance::CallerPromise, Reliance::Layout]),
    (Match::Contains("the promise that each entry is the library's export"), &[Reliance::LibraryExport]),
    (Match::Contains("exports that take what the metadata gives"), &[Reliance::LibraryExport, Reliance::Layout]),
    (Match::Contains("the glue's key"), &[Reliance::GlueKey]),
    (Match::Contains("the key to the glue's bookkeeping"), &[Reliance::GlueKey]),
    (Match::Contains("a block of ours whose Box was given up"), &[Reliance::LentUntilEnd]),
    (Match::Contains("the test's own block"), &[Reliance::StandIn]),
    (Match::Contains("standing for"), &[Reliance::StandIn]),
    (Match::Contains("the fake's"), &[Reliance::StandIn]),
    (Match::Contains("the stand-ins'"), &[Reliance::StandIn]),
    (Match::Contains("the game's C"), &[Reliance::StandIn]),
    (Match::Contains("the fuzzing drivers'"), &[Reliance::StandIn]),
    (Match::Contains("the C attack"), &[Reliance::StandIn]),
    (Match::Contains("check-2's C games"), &[Reliance::StandIn]),
    (Match::Contains("our own serving glue's exports"), &[Reliance::LibraryExport, Reliance::Layout]),
    (Match::Contains("an older interface of the kind"), &[Reliance::Layout, Reliance::LiveObject]),
];

fn reliances_of(reason: &str) -> Option<&'static [Reliance]> {
    REASONS.iter().find_map(|(matching, reliances)| {
        let matched = match matching {
            Match::Ends(end) => reason.ends_with(end),
            Match::Is(whole) => reason == *whole,
            Match::Contains(part) => reason.contains(part),
        };
        matched.then_some(*reliances)
    })
}

/// Every mark reason in the Rust files under `folder`, with the file it is in.
fn marks_under(folder: &Path, marks: &mut Vec<(String, String)>) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Ok(()) };
    let mut paths: Vec<_> = entries.filter_map(Result::ok).map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            marks_under(&path, marks)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let text = std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            let mut rest = text.as_str();
            while let Some(at) = rest.find("unsafe_code,") {
                rest = &rest[at + "unsafe_code,".len()..];
                let trimmed = rest.trim_start();
                let Some(after) = trimmed.strip_prefix("reason = \"") else { continue };
                let Some(end) = after.find('"') else { continue };
                marks.push((after[..end].to_string(), path.display().to_string()));
                rest = &after[end..];
            }
        }
    }
    Ok(())
}

/// The methods of the APIs' whole metadata, by place (`Interface::Method`, or an export's name), with their interface.
fn every_method<'r>(apis: &[&Api], read: &'r Namespaces) -> Vec<(usize, String, String, &'r Method)> {
    let mut methods = Vec::new();
    for (position, api) in apis.iter().enumerate() {
        for interface in read.interfaces.values() {
            if api.namespaces.contains(&interface.namespace.as_str()) && !api.left_out.contains(&interface.name.as_str()) {
                for method in &interface.methods {
                    methods.push((position, format!("{}::{}", interface.name, method.name), interface.name.clone(), method));
                }
            }
        }
        // A slice searches its whole API, exports included.
        if let Some(library) = crate::api::whole(api.folder).and_then(|whole| whole.library) {
            for ((namespace, name), function) in &read.functions {
                if api.namespaces.contains(&namespace.as_str()) && function.library.as_deref().is_some_and(|l| l.eq_ignore_ascii_case(library)) {
                    methods.push((position, name.clone(), String::new(), function));
                }
            }
        }
    }
    methods
}

/// Whether the calling glue of this set carries `place`: the whole API, or a slice naming it.
fn carried(api: &Api, place: &str) -> bool {
    api.only.is_empty() || api.only.contains(&place)
}

fn hold_text(hold: &Hold) -> String {
    match hold {
        Hold::Answers { value, why } => format!("answered `{value}`, the system never called: {why}"),
        Hold::Clears { param, mask, why } => format!("`{param}` & !{mask:#x} before the call: {why}"),
        Hold::Refuses { param, when, why } => format!("refused (E_INVALIDARG) when `{}`: {why}", when.replace("{}", param)),
        Hold::NotCalled { why } => format!("never called by the calling glue (E_NOTIMPL or nothing): {why}"),
        Hold::Harmless { why } => format!("cannot undo it: {why}"),
    }
}

/// Checks every mark of the direct crates written under `out` against the pins, and returns the report's section.
pub fn check(index: &TypeIndex, apis: &[&Api], out: &Path) -> Result<String, String> {
    let mut marks = Vec::new();
    marks_under(&out.join("crates"), &mut marks)?;
    let mut failures: Vec<String> = Vec::new();
    let mut by_reliance: BTreeMap<Reliance, usize> = BTreeMap::new();
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    for (reason, file) in &marks {
        *reasons.entry(reason.clone()).or_default() += 1;
        match reliances_of(reason) {
            Some(reliances) => reliances.iter().for_each(|reliance| *by_reliance.entry(*reliance).or_default() += 1),
            None => failures.push(format!("UNMATCHED MARK: \"{reason}\" ({file}): no line of src/undo.rs's REASONS says what it relies on")),
        }
    }
    let mut namespaces: Vec<&str> = apis.iter().flat_map(|api| api.namespaces.iter().copied()).collect();
    namespaces.sort_unstable();
    namespaces.dedup();
    let read = metadata::read(index, &namespaces);
    let methods = every_method(apis, &read);
    let harness = harness_sources(out)?;
    let mut report = String::new();
    let _ = writeln!(report, "RELIANCES (what each mark relies on; every method of the whole APIs that could undo it, and its pin):");
    let _ = writeln!(report, "  marks {} with {} distinct reasons", marks.len(), reasons.len());
    let mut found: BTreeSet<(usize, String, Reliance)> = BTreeSet::new();
    for reliance in Reliance::ALL {
        let count = by_reliance.get(&reliance).copied().unwrap_or(0);
        let _ = writeln!(report, "  {reliance:?}: {count} marks rely on it: {}; searched: {}", reliance.statement(), reliance.search());
        for (position, place, interface, method) in &methods {
            if !reliance.found(interface, method, &read) {
                continue;
            }
            found.insert((*position, place.clone(), reliance));
            let api = apis[*position];
            let pin = api.pins.iter().find(|pin| pin.method == place && pin.relies.contains(&reliance));
            match pin {
                None => {
                    let _ = writeln!(report, "    UNPINNED {place}");
                    failures.push(format!("UNPINNED: {place} could undo {reliance:?} ({}), and no pin of {} holds it", reliance.statement(), api.folder));
                }
                Some(pin) => {
                    let test = test_state(out, api, pin, harness.as_deref(), &mut failures);
                    let _ = writeln!(report, "    {place}: {}{test}", hold_text(&pin.hold));
                }
            }
        }
    }
    // A pin names a method of the metadata, and each of its facts is one the search finds for it, so no pin is stale.
    for (position, api) in apis.iter().enumerate() {
        for pin in api.pins {
            if !methods.iter().any(|(p, place, _, _)| *p == position && place == pin.method) {
                failures.push(format!("STALE PIN: {} pins {}, which is no method of its metadata", api.folder, pin.method));
            }
            for reliance in pin.relies {
                if !found.contains(&(position, pin.method.to_string(), *reliance)) {
                    failures.push(format!("STALE PIN: {} pins {} for {reliance:?}, which the search does not find it undoing", api.folder, pin.method));
                }
            }
        }
    }
    let _ = writeln!(report, "  pins checked: {}; failures: {}", apis.iter().map(|api| api.pins.len()).sum::<usize>(), failures.len());
    if failures.is_empty() {
        Ok(report)
    } else {
        Err(format!("{report}\nthe marks' reliances are not all pinned:\n  {}", failures.join("\n  ")))
    }
}

/// The text of the harness's Rust tests, when the set writes a harness.
fn harness_sources(out: &Path) -> Result<Option<String>, String> {
    let folder = out.join("crates").join("direct3d-11").join("implementation").join("direct").join("harness").join("tests");
    let Ok(entries) = std::fs::read_dir(&folder) else { return Ok(None) };
    let mut text = String::new();
    let mut paths: Vec<_> = entries.filter_map(Result::ok).map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        text.push_str(&std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?);
    }
    Ok(Some(text))
}

/// Whether every function of the API's calling glue named for `method` holds no `unsafe` (no call out), and there is
/// one; none when no calling glue was written.
fn never_calls_out(out: &Path, api: &Api, method: &str) -> Option<bool> {
    let path = out.join("crates").join(api.folder).join("implementation").join("direct").join("call.rs");
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

/// The hostile test of a pin the calling glue carries, which must be in the harness when the set writes one; a method
/// the glue never calls, which no safe caller of the slice can reach, is checked in the glue written instead.
fn test_state(out: &Path, api: &Api, pin: &Pin, harness: Option<&str>, failures: &mut Vec<String>) -> String {
    if matches!(pin.hold, Hold::Harmless { .. }) {
        return String::new();
    }
    if !carried(api, pin.method) {
        return " (not in this set's traits)".to_string();
    }
    let Some(harness) = harness else { return format!(" (hostile test owed where the glue is written: {})", pin.test) };
    let in_harness = !pin.test.is_empty() && harness.contains(&format!("fn {}(", pin.test));
    if !in_harness && matches!(pin.hold, Hold::NotCalled { .. }) && never_calls_out(out, api, pin.method) == Some(true) {
        return " (checked in the glue written: its calling form holds no call out)".to_string();
    }
    if !in_harness {
        failures.push(format!("UNTESTED PIN: {} is carried by this set's calling glue and its hostile test `{}` is not in the harness", pin.method, pin.test));
        return " (TEST MISSING)".to_string();
    }
    format!(" (hostile test {})", pin.test)
}
