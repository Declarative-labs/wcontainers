//! What each marked `unsafe` of the direct crates relies on.
//!
//! Every mark's reason is matched to the facts it relies on (`REASONS`); a reason no line matches fails the
//! generator. What could undo a fact is no longer searched for by name: src/kinds.rs classifies every parameter and
//! return of every method by its type (default deny), and judges every interface's effects and every enumeration's
//! values; the rows of src/api.rs name, for an exact method, the fact they hold.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

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
    (Match::Contains("own exports, declared with the parameters the metadata gives"), &[Reliance::LibraryExport, Reliance::Layout]),
    (Match::Contains("own exports, linked by name above"), &[Reliance::LibraryExport]),
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
    (Match::Contains("the hostile C caller's"), &[Reliance::StandIn, Reliance::Layout]),
    (Match::Contains("the calling glue over exact.c's stand-in"), &[Reliance::StandIn, Reliance::LibraryExport, Reliance::Layout]),
    (Match::Contains("exact.c"), &[Reliance::StandIn, Reliance::Layout]),
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

/// Checks every mark of the direct crates written under `out` against `REASONS`, and returns the report's section.
pub fn check(out: &Path) -> Result<String, String> {
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
    let mut report = String::new();
    let _ = writeln!(report, "RELIANCES (what each mark relies on):");
    let _ = writeln!(report, "  marks {} with {} distinct reasons", marks.len(), reasons.len());
    for reliance in Reliance::ALL {
        let count = by_reliance.get(&reliance).copied().unwrap_or(0);
        let _ = writeln!(report, "  {reliance:?}: {count} marks rely on it: {}", reliance.statement());
    }
    if failures.is_empty() {
        Ok(report)
    } else {
        Err(format!("{report}\nthe marks' reasons are not all matched:\n  {}", failures.join("\n  ")))
    }
}
