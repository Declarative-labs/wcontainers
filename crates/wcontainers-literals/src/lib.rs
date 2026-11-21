//! wcontainers-literals: holds the workspace to the rule that every number and every string a program
//! interprets comes from one clear place (docs/literals.md), against a baseline that only shrinks.
//!
//! [`check`] prints only what needs doing, one line each, and fails when there is any:
//!   `+` a gated finding the baseline does not accept;
//!   `-` a baseline line that no longer occurs (delete it: that is how the baseline shrinks);
//!   `~` a baselined literal that is still there but now sits in a list that is not gated (hidden, not
//!       fixed);
//!   `>` a line the baseline gained after it started (see the history module);
//!   `!` a baseline that is not sorted.
//! It also prints every mark with its reason, and how many there are.
//!
//! The baseline's `policy` lines say which paths are skipped, generated or test code; they are the only
//! way a path leaves the check, and they may only shrink like every other line.

mod baseline;
mod deps;
mod eval;
mod findings;
mod history;
mod marks;
mod modules;
mod rules;
mod scan;
mod scripts;
mod text;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use rayon::prelude::*;

pub use baseline::BASELINE_FILE;
use deps::Crates;
use eval::{ConstDef, ConstScope, Value};
pub use findings::{FIELD_SEPARATOR, Kind, LOCATION_SEPARATOR, List, PATH_SEPARATOR};
use findings::{FILE_SCOPE, Finding, NOTE_MARK, PATH_COMPONENT_SEPARATOR, Position, clean_literal};
pub use history::WORKING_TREE;
pub use marks::MARK_WORD;
pub use rules::SOURCE as RULES_SOURCE;
use scan::{FileRole, FileScan};

/// The command line program, as Cargo names it.
const PROGRAM: &str = env!("CARGO_PKG_NAME");
/// Between the commands in the usage line.
const ALTERNATIVES: &str = " | ";

/// The commands, each named by its variant in lower case.
#[derive(Clone, Copy, Debug)]
enum Command {
    Check,
    Init,
    List,
}

impl Command {
    const ALL: [Command; 3] = [Command::Check, Command::Init, Command::List];

    fn name(self) -> String {
        format!("{self:?}").to_lowercase()
    }

    fn named(name: &str) -> Option<Command> {
        Command::ALL.into_iter().find(|c| c.name() == name)
    }

    fn what(self) -> &'static str {
        match self {
            Command::Check => CHECK_DOES,
            Command::Init => INIT_DOES,
            Command::List => LIST_DOES,
        }
    }
}

const CHECK_DOES: &str = "fail on anything outside the baseline, a baseline line no longer needed, or a baseline that grew";
const INIT_DOES: &str = "write the baseline from the tree, once, when it starts or has just arrived by a merge, and again after a new version of the rules";
const LIST_DOES: &str = "print every finding of every list, gated or not";

/// How to call the command line program.
pub fn usage() -> String {
    let names: Vec<String> = Command::ALL.iter().map(|c| c.name()).collect();
    let mut out = format!(
        "usage: {PROGRAM} ({}) [WORKSPACE_ROOT]",
        names.join(ALTERNATIVES)
    );
    for c in Command::ALL {
        out.push(NEW_LINE);
        out.push_str(&format!("{ADDITION_INDENT}{}: {}", c.name(), c.what()));
    }
    out
}
/// Cargo writes a crate name with underscores where its package name has hyphens.
const CRATE_NAME_JOINER: char = '_';
const PACKAGE_NAME_JOINER: char = '-';
/// The extension of Rust source files.
pub(crate) const RUST_EXTENSION: &str = "rs";
/// A crate's (and a workspace's) manifest.
pub const MANIFEST: &str = "Cargo.toml";
/// The files git tracks, and new files it does not ignore: build output and other ignored files are
/// left out by the repository's own ignore rules, and everything committed is scanned.
const LIST_FILES: &[&str] = &[
    "ls-files",
    "-z",
    "--cached",
    "--others",
    "--exclude-standard",
    "--deduplicate",
];
/// How `git ls-files -z` ends each path.
const LIST_END: char = '\0';
pub(crate) const WORKSPACE_TABLE: &str = "workspace";
/// A crate manifest's targets that Cargo ships, and the key that places each one's root file.
const LIB_TABLE: &str = "lib";
const BIN_TABLES: &str = "bin";
/// A crate manifest's targets that Cargo builds only for tests.
const TEST_TABLES: &[&str] = &["test", "bench", "example"];
const TARGET_PATH: &str = "path";
pub(crate) const PACKAGE_TABLE: &str = "package";
const BUILD_KEY: &str = "build";
pub(crate) const PACKAGE_NAME: &str = "name";
/// A crate's source folder.
const SOURCE_FOLDER: &str = "src";
/// A crate's folders of test code, by their position in the crate (not anywhere in a path).
const TEST_DIRECTORIES: &[&str] = &["tests", "benches", "examples"];
/// A file whose first lines carry one of these markers is proposed as a `generated` policy line by init.
const GENERATED_MARKERS: &[&str] = &["@generated", "do not edit", "automatically generated"];
const GENERATED_HEADER_LINES: usize = 5;
/// Words that many constants' names share without being the same fact.
const GENERIC_WORDS: &[&str] = &[
    "max", "min", "size", "len", "length", "count", "bytes", "bits", "default", "offset", "mask",
    "shift", "num", "total", "limit", "first", "last", "start", "end", "base", "flag", "flags",
    "value", "id",
];
/// How many matching constants a note names before it summarises the rest as a count.
const MAX_NOTE_MATCHES: usize = 3;
pub(crate) const NOTE_LIST_SEPARATOR: &str = ", ";
/// Rust's separator between the digits of a number (`0x0101_0101`).
const DIGIT_SEPARATOR: char = '_';
/// Prefixes of the check's output lines.
pub const NEW_MARK: &str = "+ ";
pub const STALE_MARK: &str = "- ";
pub const HIDDEN_MARK: &str = "~ ";
pub const GROWN_MARK: &str = "> ";
pub const ORDER_MARK: &str = "! ";
/// Introduces what to do about an output line.
pub const DO: &str = "do: ";
/// What to do about a baseline line no longer needed, before the baseline's name.
pub const DELETE_LINE: &str = "delete this line from ";
const UNHIDE: &str = "undo the move and name the value: it was hidden, not fixed";
const DO_NOT_GROW: &str = "and name the value instead: the baseline only shrinks";
const SORT_IT: &str = "(LC_ALL=C sort), which is how the check writes it";
/// The first field of a baseline policy line.
const POLICY_LIST: &str = "policy";
/// Characters around a word of a message that are punctuation, not part of the word.
const WORD_PUNCTUATION: &[char] = &[
    '"', '\'', '`', ',', '.', ':', ';', '(', ')', '[', ']', '!', '?',
];
/// Characters that make a word of a message look like a name rather than plain English.
const NAME_CHARACTERS: &[char] = &['_', '-', '.', ':', '/'];

/// What a `policy` line of the baseline does to the path it names.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum PolicyKind {
    /// Not scanned at all (vendored code).
    Skip,
    /// Scanned into the generated list.
    Generated,
    /// Scanned into the test list (a harness crate whose tests live in src/).
    Test,
}

impl PolicyKind {
    const ALL: [PolicyKind; 3] = [PolicyKind::Skip, PolicyKind::Generated, PolicyKind::Test];

    fn name(self) -> String {
        format!("{self:?}").to_lowercase()
    }

    fn named(name: &str) -> Option<PolicyKind> {
        PolicyKind::ALL.into_iter().find(|k| k.name() == name)
    }
}

#[derive(Default, Clone)]
struct Policy {
    lines: Vec<(PolicyKind, String)>,
}

/// The baseline line of a policy for a path.
pub fn policy_line(kind: PolicyKind, path: &str) -> String {
    Policy::key(kind, path)
}

impl Policy {
    fn key(kind: PolicyKind, path: &str) -> String {
        [POLICY_LIST.to_string(), kind.name(), path.to_string()].join(&FIELD_SEPARATOR.to_string())
    }

    fn parse_line(line: &str) -> Option<(PolicyKind, String)> {
        let mut fields = line.split(FIELD_SEPARATOR);
        if fields.next() != Some(POLICY_LIST) {
            return None;
        }
        let kind = PolicyKind::named(fields.next()?)?;
        Some((kind, fields.next()?.to_string()))
    }

    fn of_keys(keys: &baseline::Keys) -> Policy {
        Policy {
            lines: keys.keys().filter_map(|k| Policy::parse_line(k)).collect(),
        }
    }

    fn covers(&self, kind: PolicyKind, rel: &str) -> bool {
        self.lines.iter().any(|(k, p)| *k == kind && under(rel, p))
    }
}

/// `rel` is `prefix` or inside it, component by component.
fn under(rel: &str, prefix: &str) -> bool {
    prefix.is_empty()
        || rel == prefix
        || rel
            .strip_prefix(prefix)
            .is_some_and(|r| r.starts_with(PATH_COMPONENT_SEPARATOR))
}

struct SourceFile {
    path: PathBuf,
    rel: String,
    crate_dir: String,
    role: FileRole,
    /// Cargo builds the file as the root of a shipped target named in its crate's manifest.
    crate_root: bool,
    /// Other crates can import its public items: no private module declaration leads to it.
    exposed: bool,
    /// When Cargo builds it into a crate of its own beside the library (an integration test, a bench,
    /// an example, a binary beside a library), the root file of that crate: it can import only the
    /// library's public items, and its own crate's.
    unit: Option<String>,
}

/// A path relative to the root, with `.` and `..` resolved by their names (`tests/../src/a.rs`).
fn rel_string(path: &Path, root: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let mut parts = Vec::new();
    for c in rel.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                parts.pop();
            }
            _ => parts.push(c.as_os_str().to_string_lossy()),
        }
    }
    parts.join(PATH_COMPONENT_SEPARATOR)
}

#[derive(Default)]
struct Walk {
    rust: Vec<SourceFile>,
    scripts: Vec<(PathBuf, String)>,
    crate_dirs: BTreeSet<String>,
    /// Folders git lists as one entry, so that nothing in them could be read: a submodule that is not
    /// checked out, or a link to a folder.
    closed: Vec<(String, String)>,
}

/// The root files of a crate's targets, relative to the workspace root, whatever their extension.
#[derive(Default)]
struct Roots {
    /// Shipped targets (`[lib]`, `[[bin]]`, `package.build`): code wherever they sit.
    shipped: Vec<String>,
    /// Targets Cargo builds only for tests (`[[test]]`, `[[bench]]`, `[[example]]`).
    test: Vec<String>,
    /// Binaries of a crate that also has a library: crates of their own beside it, by root file or
    /// by folder (`src/bin`).
    bins: Vec<String>,
    bin_folders: Vec<String>,
}

impl Walk {
    /// Files a source file of `crate_dir` at `path` (relative `rel`), by its kind and place. A target's
    /// root is Rust whatever its extension, since Cargo builds it as one.
    fn add(&mut self, path: PathBuf, rel: String, crate_dir: &str, policy: &Policy, roots: &Roots) {
        let crate_root = roots.shipped.contains(&rel);
        let test_root = roots.test.contains(&rel);
        if crate_root || test_root || path.extension().is_some_and(|e| e == RUST_EXTENSION) {
            let in_crate = rel
                .strip_prefix(crate_dir)
                .map(|r| r.trim_start_matches(PATH_COMPONENT_SEPARATOR))
                .unwrap_or(&rel);
            let test = !crate_root
                && (test_root
                    || TEST_DIRECTORIES
                        .iter()
                        .any(|t| in_crate.split(PATH_COMPONENT_SEPARATOR).next() == Some(*t)));
            let role = if test || policy.covers(PolicyKind::Test, &rel) {
                FileRole::Test
            } else if policy.covers(PolicyKind::Generated, &rel) {
                FileRole::Generated
            } else {
                FileRole::Code
            };
            let separate = test
                || roots.bins.contains(&rel)
                || roots.bin_folders.iter().any(|f| under(&rel, f));
            self.rust.push(SourceFile {
                path,
                unit: separate.then(|| rel.clone()),
                rel,
                crate_dir: crate_dir.to_string(),
                role,
                crate_root,
                exposed: true,
            });
        } else if scripts::is_script(&path) {
            self.scripts.push((path, crate_dir.to_string()));
        }
    }
}

/// The folder git keeps a repository in, inside a nested repository's folder.
const GIT_FOLDER: &str = ".git";

/// The files git lists under `dir` (relative to `root`, empty for the root itself), with the files of
/// every nested repository in it (a submodule, or a repository inside the tree), of which git lists
/// only the folder. A listed folder that is not a checked-out repository goes to `closed`.
fn listed(
    root: &Path,
    dir: &str,
    files: &mut Vec<String>,
    closed: &mut Vec<String>,
) -> Result<(), String> {
    let at = root.join(dir);
    let out = history::run(&at, LIST_FILES)?;
    if !out.status.success() {
        return Err(format!(
            "cannot list the files of {}: {}",
            at.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    for entry in String::from_utf8_lossy(&out.stdout)
        .split(LIST_END)
        .filter(|r| !r.is_empty())
    {
        let entry = entry.trim_end_matches(PATH_COMPONENT_SEPARATOR);
        let rel = match dir.is_empty() {
            true => entry.to_string(),
            false => [dir, entry].join(PATH_COMPONENT_SEPARATOR),
        };
        let path = root.join(&rel);
        if !path.is_dir() {
            files.push(rel);
        } else if path.join(GIT_FOLDER).exists() {
            listed(root, &rel, files, closed)?;
        } else {
            closed.push(rel);
        }
    }
    Ok(())
}

/// Every Rust file and script git tracks or would track under `root`, in nested repositories too,
/// and every target root a crate's manifest names, except `skip` policy.
fn walk(root: &Path, policy: &Policy) -> Result<Walk, String> {
    let (mut all, mut closed) = (Vec::new(), Vec::new());
    listed(root, "", &mut all, &mut closed)?;
    let skipped = |r: &String| policy.covers(PolicyKind::Skip, r);
    all.retain(|r| !skipped(r));
    closed.retain(|r| !skipped(r));
    let mut tree = Walk::default();
    for rel in &all {
        if let Some(dir) = rel.strip_suffix(MANIFEST)
            && (dir.is_empty() || dir.ends_with(PATH_COMPONENT_SEPARATOR))
        {
            tree.crate_dirs
                .insert(dir.trim_end_matches(PATH_COMPONENT_SEPARATOR).to_string());
        }
    }
    let mut roots = Roots::default();
    for d in &tree.crate_dirs {
        target_roots(root, d, &mut roots);
    }
    // A root git does not list (an ignored file) is still built, so it is scanned like a module.
    let unlisted: Vec<String> = roots
        .shipped
        .iter()
        .chain(&roots.test)
        .filter(|r| !all.contains(r) && !skipped(r) && root.join(r).is_file())
        .cloned()
        .collect();
    all.extend(unlisted);
    all.sort();
    all.dedup();
    let crate_dirs = tree.crate_dirs.clone();
    let crate_of = |rel: &str| {
        crate_dirs
            .iter()
            .filter(|d| under(rel, d))
            .max_by_key(|d| d.len())
            .cloned()
            .unwrap_or_default()
    };
    tree.closed = closed.iter().map(|r| (r.clone(), crate_of(r))).collect();
    for rel in all {
        let crate_dir = crate_of(&rel);
        tree.add(root.join(&rel), rel, &crate_dir, policy, &roots);
    }
    Ok(tree)
}

/// Every Rust file under a folder that is not a repository (a reference crate's sources).
fn walk_folder(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.filter_map(Result::ok).map(|e| e.path()) {
        if path.is_dir() {
            walk_folder(&path, out);
        } else if path.extension().is_some_and(|e| e == RUST_EXTENSION) {
            out.push(path);
        }
    }
}

/// The root files a crate's manifest places (`path` of `[lib]`, `[[bin]]`, `[[test]]`, `[[bench]]`,
/// `[[example]]`, and `package.build`), relative to the root, whatever their extension: a shipped
/// root under tests/ is still code, and a root named `lib.txt` is still Rust.
fn target_roots(root: &Path, crate_dir: &str, roots: &mut Roots) {
    let Ok(manifest) = std::fs::read_to_string(root.join(crate_dir).join(MANIFEST))
        .map_err(|_| ())
        .and_then(|t| t.parse::<toml::Table>().map_err(|_| ()))
    else {
        return;
    };
    let path_of = |t: &toml::Value| {
        t.get(TARGET_PATH)
            .and_then(|p| p.as_str())
            .map(String::from)
    };
    let tables = |name: &str| {
        manifest
            .get(name)
            .and_then(|b| b.as_array())
            .into_iter()
            .flatten()
            .filter_map(path_of)
            .collect::<Vec<_>>()
    };
    let lib = manifest.get(LIB_TABLE).and_then(path_of);
    let build = manifest
        .get(PACKAGE_TABLE)
        .and_then(|p| p.get(BUILD_KEY))
        .and_then(|b| b.as_str())
        .map(String::from);
    let place = |p: String| rel_string(&root.join(crate_dir).join(p), root);
    let source = |file: &str| [SOURCE_FOLDER, file].join(PATH_COMPONENT_SEPARATOR);
    let bins = tables(BIN_TABLES);
    if lib.is_some()
        || root
            .join(crate_dir)
            .join(source(modules::LIB_FILE))
            .is_file()
    {
        roots.bins.extend(
            bins.iter()
                .cloned()
                .chain([source(modules::MAIN_FILE)])
                .map(place),
        );
        roots.bin_folders.push(place(source(modules::BIN_FOLDER)));
    }
    roots
        .shipped
        .extend(lib.into_iter().chain(bins).chain(build).map(place));
    roots
        .test
        .extend(TEST_TABLES.iter().flat_map(|t| tables(t)).map(place));
}

/// The checker's own source folder, relative to the root, when the checker's crate is in the tree.
fn own_source(root: &Path, crate_dirs: &BTreeSet<String>) -> Option<String> {
    crate_dirs
        .iter()
        .find(|d| {
            std::fs::read_to_string(root.join(d).join(MANIFEST))
                .ok()
                .and_then(|t| t.parse::<toml::Table>().ok())
                .is_some_and(|m| {
                    m.get(PACKAGE_TABLE)
                        .and_then(|p| p.get(PACKAGE_NAME))
                        .and_then(|n| n.as_str())
                        == Some(PROGRAM)
                })
        })
        .map(|d| [d.as_str(), SOURCE_FOLDER].join(PATH_COMPONENT_SEPARATOR))
}

fn has_generated_marker(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|text| {
        text.lines().take(GENERATED_HEADER_LINES).any(|l| {
            let l = l.to_lowercase();
            GENERATED_MARKERS.iter().any(|m| l.contains(m))
        })
    })
}

/// Runs `f` on every item on rayon's threads (as many as `RAYON_NUM_THREADS` says, or one per core).
fn parallel<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync + Send) -> Vec<R> {
    items.par_iter().map(f).collect()
}

/// Files declared as modules under a test-only attribute become test files, with their own modules;
/// a module production code declares is code even under tests/, and every declared module is scanned
/// wherever it is (an ignored or skipped folder), whether git lists it or not.
fn mark_test_modules(root: &Path, files: &mut Vec<SourceFile>) {
    let is_test = |attrs: &[syn::Attribute]| attrs.iter().any(scan::is_test_attribute);
    let declare = |file: &SourceFile| {
        if file.role == FileRole::Test {
            return (file.crate_dir.clone(), modules::Declared::default());
        }
        let Ok(ast) = std::fs::read_to_string(&file.path)
            .map_err(|_| ())
            .and_then(|t| syn::parse_file(&t).map_err(|_| ()))
        else {
            return (file.crate_dir.clone(), modules::Declared::default());
        };
        (
            file.crate_dir.clone(),
            modules::declared(&file.path, file.crate_root, &ast, &is_test),
        )
    };
    let canonical = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let mut known: HashSet<PathBuf> = files.iter().map(|f| canonical(&f.path)).collect();
    let mut declared = parallel(files, declare);
    let mut from = usize::default();
    loop {
        let mut new = Vec::new();
        for (crate_dir, d) in &declared[from..] {
            let wanted = d
                .code
                .iter()
                .map(|p| (p, FileRole::Code))
                .chain(d.test.iter().map(|m| (&m.file, FileRole::Test)));
            for (p, role) in wanted {
                let Ok(path) = p.canonicalize() else {
                    continue;
                };
                if known.insert(path.clone()) {
                    new.push(SourceFile {
                        rel: rel_string(&path, root),
                        path,
                        crate_dir: crate_dir.clone(),
                        role,
                        crate_root: false,
                        exposed: true,
                        unit: None,
                    });
                }
            }
        }
        if new.is_empty() {
            break;
        }
        from = declared.len();
        declared.extend(parallel(&new, declare));
        files.extend(new);
    }
    // A file declared privately is hidden, and so is every module it declares.
    let mut hidden: HashSet<PathBuf> = declared
        .iter()
        .flat_map(|(_, d)| d.private.iter().map(|p| canonical(p)))
        .collect();
    loop {
        let more: Vec<PathBuf> = files
            .iter()
            .zip(&declared)
            .filter(|(f, _)| hidden.contains(&canonical(&f.path)))
            .flat_map(|(_, (_, d))| d.code.iter().map(|p| canonical(p)))
            .filter(|p| !hidden.contains(p))
            .collect();
        if more.is_empty() {
            break;
        }
        hidden.extend(more);
    }
    for file in files.iter_mut() {
        file.exposed = !hidden.contains(&canonical(&file.path));
    }
    let placed: Vec<PathBuf> = declared
        .iter()
        .flat_map(|(_, d)| d.code.iter().map(|p| canonical(p)))
        .collect();
    for file in files.iter_mut() {
        if file.role != FileRole::Test
            && declared
                .iter()
                .flat_map(|(_, d)| &d.test)
                .any(|m| file.path == m.file || file.path.starts_with(&m.folder))
        {
            file.role = FileRole::Test;
        } else if file.role == FileRole::Test && placed.contains(&canonical(&file.path)) {
            file.role = FileRole::Code;
            file.unit = None;
        }
    }
    // A module of a file built as a crate of its own belongs to that crate (to the first that
    // declares it); a file of such a folder that no file declares is a crate of its own.
    let modules: HashSet<PathBuf> = declared
        .iter()
        .flat_map(|(_, d)| d.code.iter().chain(d.test.iter().map(|m| &m.file)))
        .map(|p| canonical(p))
        .collect();
    let mut unit_of: HashMap<PathBuf, String> = files
        .iter()
        .filter(|f| !modules.contains(&canonical(&f.path)))
        .filter_map(|f| f.unit.clone().map(|u| (canonical(&f.path), u)))
        .collect();
    loop {
        let more: Vec<(PathBuf, String)> = files
            .iter()
            .zip(&declared)
            .filter_map(|(f, (_, d))| unit_of.get(&canonical(&f.path)).map(|u| (u, d)))
            .flat_map(|(u, d)| {
                d.code
                    .iter()
                    .chain(d.test.iter().map(|m| &m.file))
                    .map(move |p| (canonical(p), u.clone()))
            })
            .filter(|(p, _)| !unit_of.contains_key(p))
            .collect();
        if more.is_empty() {
            break;
        }
        for (p, u) in more {
            unit_of.entry(p).or_insert(u);
        }
    }
    for file in files.iter_mut() {
        let own = file.unit.take();
        file.unit = unit_of.get(&canonical(&file.path)).cloned().or(own);
    }
}

/// A mark found in a file, with where it is.
struct FileMark {
    path: String,
    crate_dir: String,
    mark: marks::Mark,
}

/// What scanning one file gives.
#[derive(Default)]
struct FileResult {
    findings: Vec<Finding>,
    consts: Vec<ConstDef>,
    marks: Vec<FileMark>,
}

fn scan_file(file: &SourceFile, reference: Option<&str>) -> FileResult {
    match std::fs::read_to_string(&file.path) {
        Ok(text) => scan_text(file, &text, reference),
        Err(e) => FileResult {
            findings: vec![unparsed(
                file,
                &e.to_string(),
                proc_macro2::LineColumn {
                    line: usize::default(),
                    column: usize::default(),
                },
            )],
            ..FileResult::default()
        },
    }
}

fn scan_text(file: &SourceFile, text: &str, reference: Option<&str>) -> FileResult {
    let module = reference.map(|label| {
        let stem = file
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        format!("{label}{PATH_SEPARATOR}{stem}")
    });
    match syn::parse_file(text) {
        Ok(ast) => {
            let mut scan = FileScan::new(
                file.rel.clone(),
                file.crate_dir.clone(),
                file.role,
                module,
                scan::folder_of(&file.path),
                file.exposed,
            );
            scan.scan_file(&ast);
            let marks = match reference {
                Some(_) => Vec::new(),
                None => marks::parse(text)
                    .into_iter()
                    .map(|mark| FileMark {
                        path: file.rel.clone(),
                        crate_dir: file.crate_dir.clone(),
                        mark,
                    })
                    .collect(),
            };
            FileResult {
                findings: scan.findings,
                consts: scan.consts,
                marks,
            }
        }
        Err(e) if reference.is_none() => FileResult {
            findings: vec![unparsed(file, &e.to_string(), e.span().start())],
            ..FileResult::default()
        },
        Err(_) => FileResult::default(),
    }
}

/// A constant's place, which a redefinition line is matched on.
fn item_key(path: &str, item: &str) -> String {
    [path, item].join(&FIELD_SEPARATOR.to_string())
}

/// The keys of the literals in a file's text, without their lists: what the history compares a line
/// added at a new version of the rules with. A constant also gives its place, and a mark its number.
fn literal_keys(root: &Path, rel: &str, text: &str) -> HashSet<String> {
    let path = root.join(rel);
    // A file that is neither Rust by its name nor a script is a target root of another extension.
    let script = match path.extension().and_then(|e| e.to_str()) {
        Some(RUST_EXTENSION) => false,
        Some(ext) => scripts::SCRIPT_EXTENSIONS.contains(&ext),
        None => text.starts_with(scripts::SHEBANG) && syn::parse_file(text).is_err(),
    };
    if script {
        return scripts::scan(rel, "", text, &|_, _| None)
            .iter()
            .map(Finding::rest_key)
            .collect();
    }
    let file = SourceFile {
        path,
        rel: rel.to_string(),
        crate_dir: String::new(),
        role: FileRole::Code,
        crate_root: false,
        exposed: true,
        unit: None,
    };
    let scanned = scan_text(&file, text, None);
    let marks = scanned.marks.iter().map(|m| {
        [
            rel.to_string(),
            Kind::Mark.name(),
            FILE_SCOPE.to_string(),
            clean_literal(&m.mark.literal),
        ]
        .join(&FIELD_SEPARATOR.to_string())
    });
    scanned
        .findings
        .iter()
        .map(Finding::rest_key)
        .chain(scanned.consts.iter().map(|d| item_key(&d.path, &d.item)))
        .chain(marks)
        .collect()
}

/// The path of the literal a baseline line stands for, and its key among [`literal_keys`]; none for a
/// policy line.
fn line_literal(line: &str) -> Option<(String, String)> {
    let (line, _) = baseline::split_reason(line);
    let (list, rest) = line.split_once(FIELD_SEPARATOR)?;
    let list = List::named(list)?;
    let mut fields = rest.split(FIELD_SEPARATOR);
    let path = fields.next()?.to_string();
    let key = match list {
        List::Redefinition => item_key(&path, fields.nth(1)?),
        _ => rest.to_string(),
    };
    Some((path, key))
}

/// Why a folder git lists as one entry is a finding.
const CLOSED_FOLDER: &str =
    "a folder git lists as one entry (a submodule not checked out, or a link to a folder)";

fn unparsed(file: &SourceFile, message: &str, at: proc_macro2::LineColumn) -> Finding {
    Finding {
        list: List::Unparsed,
        path: file.rel.clone(),
        line: at.line,
        column: at.column + 1,
        kind: Kind::Parse,
        item: FILE_SCOPE.to_string(),
        literal: clean_literal(message),
        value: None,
        note: String::new(),
        crate_dir: file.crate_dir.clone(),
        in_const: None,
    }
}

fn scan_all(files: &[SourceFile], reference: Option<&str>) -> FileResult {
    let mut all = FileResult::default();
    for r in parallel(files, |file| scan_file(file, reference)) {
        all.findings.extend(r.findings);
        all.consts.extend(r.consts);
        all.marks.extend(r.marks);
    }
    all.consts
        .sort_by(|a, b| (&a.path, a.line, a.column).cmp(&(&b.path, b.line, b.column)));
    all.marks
        .sort_by(|a, b| (&a.path, a.mark.line).cmp(&(&b.path, b.mark.line)));
    all
}

/// The package a reference label stands for (`linux_raw_sys::aarch64` is linux-raw-sys).
fn reference_package(module: &str) -> String {
    module
        .split(PATH_SEPARATOR)
        .next()
        .unwrap_or_default()
        .replace(CRATE_NAME_JOINER, &PACKAGE_NAME_JOINER.to_string())
}

/// How close a constant is to the crate a literal is in, which orders the names a note gives: the
/// owner of a fact is the crate the others depend on, so upstream crates come first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Closeness {
    /// Hand-written in a crate this one depends on.
    Upstream,
    /// Hand-written in this crate.
    SameCrate,
    /// Generated, or in a reference crate, that this crate can reach.
    Reachable,
    /// Anywhere else.
    Elsewhere,
}

/// How close a constant is to a crate.
fn closeness(crates: &Crates, d: &ConstDef, crate_dir: &str) -> Closeness {
    let upstream = crates.upstream(crate_dir, &d.crate_dir);
    match &d.scope {
        ConstScope::Code if upstream => Closeness::Upstream,
        ConstScope::Code if d.crate_dir == crate_dir => Closeness::SameCrate,
        ConstScope::Generated if upstream || d.crate_dir == crate_dir => Closeness::Reachable,
        ConstScope::Reference(module)
            if crates.uses_package(crate_dir, &reference_package(module)) =>
        {
            Closeness::Reachable
        }
        _ => Closeness::Elsewhere,
    }
}

/// Constants by value, for naming what a literal restates.
struct ValueIndex<'a> {
    by_value: HashMap<&'a Value, Vec<&'a ConstDef>>,
    crates: &'a Crates,
    /// Each file Cargo builds into a crate of its own beside the library, with that crate's root.
    units: HashMap<&'a str, &'a str>,
}

impl<'a> ValueIndex<'a> {
    fn new(
        defs: &'a [ConstDef],
        crates: &'a Crates,
        units: HashMap<&'a str, &'a str>,
    ) -> ValueIndex<'a> {
        let mut by_value: HashMap<&Value, Vec<&ConstDef>> = HashMap::new();
        for d in defs.iter().filter(|d| d.scope != ConstScope::Test) {
            if let Some(v) = d.value.as_ref().filter(|v| !v.is_free()) {
                by_value.entry(v).or_default().push(d);
            }
        }
        ValueIndex {
            by_value,
            crates,
            units,
        }
    }

    /// The constants with this value, closest first, leaving out the item the literal is in.
    fn matches(
        &self,
        value: &Value,
        crate_dir: &str,
        own: Option<(&str, &str)>,
    ) -> Vec<(Closeness, &'a ConstDef)> {
        let mut defs: Vec<(Closeness, &ConstDef)> = self
            .by_value
            .get(value)
            .into_iter()
            .flatten()
            .filter(|d| own.is_none_or(|(path, item)| !(d.path == path && d.item == item)))
            .map(|d| (closeness(self.crates, d, crate_dir), *d))
            .collect();
        defs.sort_by_key(|(c, d)| (*c, self.crates.depth(&d.crate_dir), d.path.as_str(), d.line));
        defs
    }

    /// The constants with this value that `file` of `crate_dir` could import or already has.
    fn importable(
        &self,
        value: &Value,
        crate_dir: &str,
        file: &str,
        own: Option<(&str, &str)>,
    ) -> Vec<(Closeness, &'a ConstDef)> {
        self.matches(value, crate_dir, own)
            .into_iter()
            .filter(|(_, d)| self.can_import(d, crate_dir, file))
            .collect()
    }

    /// `file` of `crate_dir` can import or already has this constant: a public one, its crate's own
    /// outside a function (only those of its own crate when Cargo builds it into a crate of its own
    /// beside the library), or a reference crate's.
    fn can_import(&self, d: &ConstDef, crate_dir: &str, file: &str) -> bool {
        let unit = |path: &str| self.units.get(path).copied();
        d.public
            || (!d.local && d.crate_dir == crate_dir && unit(&d.path) == unit(file))
            || matches!(d.scope, ConstScope::Reference(_))
    }

    /// The constants with this value that the crate can name: its own and its upstream crates'.
    fn nameable(
        &self,
        value: &Value,
        crate_dir: &str,
        own: Option<(&str, &str)>,
    ) -> Vec<(Closeness, &'a ConstDef)> {
        self.matches(value, crate_dir, own)
            .into_iter()
            .filter(|(c, _)| *c != Closeness::Elsewhere)
            .collect()
    }

    /// The first few names, closest first, and how many more there are.
    fn names(defs: &[(Closeness, &ConstDef)]) -> Option<String> {
        if defs.is_empty() {
            return None;
        }
        let mut names: Vec<String> = defs
            .iter()
            .take(MAX_NOTE_MATCHES)
            .map(|(_, d)| d.display())
            .collect();
        if let Some(rest) = defs
            .len()
            .checked_sub(MAX_NOTE_MATCHES)
            .filter(|r| *r > usize::default())
        {
            names.push(format!("+{rest} more"));
        }
        Some(names.join(NOTE_LIST_SEPARATOR))
    }

    fn note(defs: &[(Closeness, &ConstDef)]) -> Option<String> {
        ValueIndex::names(defs).map(|n| format!("= {n}"))
    }
}

/// A value specific enough that writing it where a constant of the same value can be named restates
/// that constant: a number that does not fit in a byte, or text of more than one character.
fn distinctive(value: &Value) -> bool {
    match value {
        Value::Int(v) => v.unsigned_abs() > u128::from(u8::MAX),
        Value::Str(s) => s.chars().nth(1).is_some(),
        Value::Byte(_) | Value::Char(_) => false,
    }
}

/// The number a test literal writes in another type: a float with no fraction (`4096.0`) or a string
/// of digits (`"4096"`).
fn written_number(f: &Finding) -> Option<Value> {
    match (&f.value, f.kind) {
        (Some(Value::Str(s)), _) => s.parse::<i128>().ok().map(Value::Int),
        (_, Kind::Float) => syn::parse_str::<syn::LitFloat>(&f.literal)
            .ok()?
            .base10_parse::<f64>()
            .ok()
            .filter(|v| v.fract() == f64::default())
            .map(|v| Value::Int(v as i128)),
        _ => None,
    }
}

/// The specific words of a constant's name: its words other than [`GENERIC_WORDS`].
fn specific_words(name: &str) -> Vec<String> {
    scan::words(name)
        .into_iter()
        .filter(|w| !GENERIC_WORDS.contains(&w.as_str()))
        .collect()
}

/// Tests and constant initialisers that restate, by value, a constant their crate can already name.
/// A literal in a test, in a constant or not, is gated on its value alone against what the test's crate
/// can name; a literal in a named constant of code only when the two names also share a specific word
/// (`LOCAL_PAGE` and `PAGE_SIZE`), so that a different fact of the same value can be given its own
/// name. A test's note names what the file can import, or says that the constant must first be made
/// importable. A literal in a test constant that `test_copies` holds is that redefinition only.
fn restated(all: &mut [Finding], index: &ValueIndex, test_copies: &HashSet<(String, Position)>) {
    for f in all
        .iter_mut()
        .filter(|f| matches!(f.list, List::Test | List::Const))
    {
        let written = match f.list {
            List::Test => written_number(f),
            _ => None,
        };
        let Some(value) = written
            .as_ref()
            .or(f.value.as_ref())
            .filter(|v| distinctive(v))
        else {
            continue;
        };
        // A test restates what its crate can name. A constant restates another crate's: the upstream
        // one, or between unrelated crates the one more crates depend on, else the one whose package
        // name sorts first (its folder when it has none), so that moving a crate's folder does not
        // change which copy is reported. Two names in one crate are left apart: a default equal to a
        // maximum is two facts.
        let crates = index.crates;
        let wanted = |c: Closeness, d: &ConstDef| match (f.list, c) {
            (List::Test, _) => matches!(c, Closeness::Upstream | Closeness::SameCrate),
            (_, Closeness::Upstream) => true,
            (_, Closeness::Elsewhere) => {
                crates.owns(&d.crate_dir, &f.crate_dir)
                    || (!crates.owns(&f.crate_dir, &d.crate_dir)
                        && crates.sort_name(&d.crate_dir) < crates.sort_name(&f.crate_dir))
            }
            (_, Closeness::SameCrate | Closeness::Reachable) => false,
        };
        let test = f.list == List::Test;
        // A test's constant that `redefinitions` reports as a copy is that redefinition only; any
        // other test literal is gated on its value whatever its constant is called.
        if test
            && f.in_const
                .is_some_and(|at| test_copies.contains(&(f.path.clone(), at)))
        {
            continue;
        }
        // Between constants of code, the same name and value is a redefinition, reported as one.
        let own_name = f.item.rsplit(PATH_SEPARATOR).next().unwrap_or_default();
        let own_words = specific_words(own_name);
        let named_alike = |d: &ConstDef| {
            f.in_const.is_none()
                || test
                || (d.bare != own_name
                    && specific_words(&d.bare)
                        .iter()
                        .any(|w| own_words.contains(w)))
        };
        let defs: Vec<(Closeness, &ConstDef)> = index
            .matches(
                value,
                &f.crate_dir,
                Some((f.path.as_str(), f.item.as_str())),
            )
            .into_iter()
            .filter(|(c, d)| wanted(*c, d) && d.scope == ConstScope::Code && named_alike(d))
            .collect();
        let (importable, hidden): (Vec<_>, Vec<_>) = defs
            .iter()
            .partition(|(_, d)| !test || index.can_import(d, &f.crate_dir, &f.path));
        let note = match (ValueIndex::names(&importable), ValueIndex::names(&hidden)) {
            (Some(names), _) => names,
            (None, Some(names)) => format!("{names}, {NOT_IMPORTABLE}"),
            (None, None) => continue,
        };
        let was = f.list.name();
        f.list = List::Restated;
        f.note = format!("{was} literal restates {note}");
    }
}

/// What a restated literal's note adds when the file cannot import the constant it restates.
pub const NOT_IMPORTABLE: &str = "which this file cannot import: make it public, outside any function and in public modules, then import it";

/// A word of a message that looks like a name (a key, a variable, a crate) rather than plain English.
fn name_like(word: &str) -> bool {
    word.chars().nth(1).is_some()
        && word
            .chars()
            .any(|c| c.is_uppercase() || c.is_ascii_digit() || NAME_CHARACTERS.contains(&c))
}

/// What a crate's own package name is to be written with.
const OWN_NAME: &str = "the crate's own name: write it with env!(\"CARGO_PKG_NAME\")";

/// Messages and string constants that write out, word for word, the value of another string constant
/// the crate can reach, or the crate's own package name: the name belongs there through its constant.
/// A constant whose whole value is the word is a name of its own, which redefinitions cover.
fn named_in_text(all: &mut [Finding], index: &ValueIndex, crates: &Crates) {
    for f in all
        .iter_mut()
        .filter(|f| matches!(f.list, List::Message | List::Const))
    {
        let Some(Value::Str(written)) = &f.value else {
            continue;
        };
        let constant = f.list == List::Const;
        let own = constant.then_some((f.path.as_str(), f.item.as_str()));
        let package = crates.package(&f.crate_dir);
        let text = text::without_placeholders(written);
        let named = text
            .split_whitespace()
            .map(|w| w.trim_matches(WORD_PUNCTUATION))
            .filter(|w| package == Some(*w) || name_like(w))
            .find_map(|w| {
                if package == Some(w) {
                    return Some((w.to_string(), OWN_NAME.to_string()));
                }
                if constant && w == text.trim() {
                    return None;
                }
                let defs = index.nameable(&Value::Str(w.to_string()), &f.crate_dir, own);
                ValueIndex::names(&defs).map(|names| (w.to_string(), names))
            });
        if let Some((word, names)) = named {
            let was = f.list.name();
            f.list = List::Restated;
            f.note = format!("{was} writes out {word}, which is {names}");
        }
    }
}

/// Redefinitions, and the test constants among them reported as copies of a constant of code,
/// generated code or a reference crate, by file and the position of the constant's name.
struct Redefinitions {
    findings: Vec<Finding>,
    test_copies: HashSet<(String, Position)>,
}

/// Constants that restate, by the same name and value, a constant defined elsewhere; reported on the
/// copy: a constant is left out when its crate owns the fact over every other definition's crate. A
/// test's constant never owns a fact that code, generated code or a reference crate also defines.
fn redefinitions(defs: &[ConstDef], index: &ValueIndex) -> Redefinitions {
    let crates = index.crates;
    let mut by_bare: HashMap<&str, Vec<&ConstDef>> = HashMap::new();
    for d in defs.iter().filter(|d| d.value.is_some()) {
        by_bare.entry(&d.bare).or_default().push(d);
    }
    // A reference crate is upstream of everything; a generated copy is the owner within its crate.
    let upstream_of = |d: &ConstDef, o: &ConstDef| match (&d.scope, &o.scope) {
        (ConstScope::Test, ConstScope::Test) => crates.owns(&d.crate_dir, &o.crate_dir),
        (ConstScope::Test, _) | (_, ConstScope::Reference(_)) => false,
        _ => crates.owns(&d.crate_dir, &o.crate_dir),
    };
    let mut out = Redefinitions {
        findings: Vec::new(),
        test_copies: HashSet::new(),
    };
    for d in defs
        .iter()
        .filter(|d| matches!(d.scope, ConstScope::Code | ConstScope::Test))
    {
        let Some(value) = &d.value else { continue };
        let alias = d.alias();
        if d.trait_impl && alias == Some(d.bare.as_str()) {
            continue;
        }
        let others: Vec<&&ConstDef> = by_bare
            .get(d.bare.as_str())
            .into_iter()
            .flatten()
            .filter(|o| !(o.path == d.path && o.line == d.line))
            .filter(|o| o.value.as_ref().is_some_and(|v| v.same_as(value)))
            .filter(|o| d.scope == ConstScope::Test || o.scope != ConstScope::Test)
            .filter(|o| {
                matches!(o.scope, ConstScope::Reference(_))
                    || !(d.associated() && o.associated())
                    || o.qualified == d.qualified
            })
            .collect();
        if others.is_empty() || others.iter().all(|o| upstream_of(d, o)) {
            continue;
        }
        let kind = match value {
            Value::Int(_) => Kind::Int,
            Value::Byte(_) => Kind::Byte,
            Value::Str(_) => Kind::Str,
            Value::Char(_) => Kind::Char,
        };
        let also: Vec<String> = others.iter().map(|o| o.display()).collect();
        let second_name = alias
            .map(|a| format!("a second name for {a}: import it with `use` instead; "))
            .unwrap_or_default();
        // Said when no constant of code, generated code or a reference crate it copies can be
        // imported as it is.
        let owners: Vec<&&&ConstDef> = others
            .iter()
            .filter(|o| o.scope != ConstScope::Test)
            .collect();
        if d.scope == ConstScope::Test && !owners.is_empty() {
            out.test_copies.insert((d.path.clone(), (d.line, d.column)));
        }
        let hidden = if owners.is_empty()
            || owners
                .iter()
                .any(|o| index.can_import(o, &d.crate_dir, &d.path))
        {
            String::new()
        } else {
            format!(", {NOT_IMPORTABLE}")
        };
        out.findings.push(Finding {
            list: List::Redefinition,
            path: d.path.clone(),
            line: d.line,
            column: d.column,
            kind,
            item: d.item.clone(),
            literal: clean_literal(&format!("{} = {value}", d.qualified)),
            value: Some(value.clone()),
            note: format!(
                "{second_name}also {}{hidden}",
                also.join(NOTE_LIST_SEPARATOR)
            ),
            crate_dir: d.crate_dir.clone(),
            in_const: Some((d.line, d.column)),
        });
    }
    out
}

/// A mark that may stand, the findings it covers, and its reason.
type Standing<'a> = (&'a FileMark, Vec<usize>, String);

/// Applies every mark: a mark with a reason over a gated number that has no name, and that no other
/// mark also covers, moves that number to the marked list and is reported; any other mark is a finding
/// of the mark list.
fn apply_marks(all: &mut Vec<Finding>, found: &[FileMark], index: &ValueIndex) -> Vec<String> {
    let mut problems = Vec::new();
    // Marks that may stand, with the findings they cover, by the number's value.
    let mut standing: BTreeMap<String, Vec<Standing>> = BTreeMap::new();
    for fm in found {
        let FileMark {
            path,
            crate_dir,
            mark,
        } = fm;
        let covered: Vec<usize> = all
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                f.list == List::Code
                    && f.path == *path
                    && f.line == mark.target
                    && matches!(f.kind, Kind::Int | Kind::Float)
                    && f.literal == mark.literal
            })
            .map(|(i, _)| i)
            .collect();
        let named = covered
            .iter()
            .filter_map(|&i| all[i].value.as_ref())
            .find_map(|v| ValueIndex::names(&index.matches(v, crate_dir, None)));
        let problem = match (&mark.reason, covered.is_empty(), named) {
            (Err(p), ..) => Some(p.note()),
            (Ok(_), true, _) => Some(format!(
                "no unnamed number {} in code on line {} below the mark",
                mark.literal, mark.target
            )),
            (Ok(_), false, Some(names)) => Some(format!(
                "{} has a name, {names}: use it; a mark is only for a number no name fits",
                mark.literal
            )),
            (Ok(reason), false, None) => {
                let value = covered
                    .first()
                    .and_then(|&i| all[i].value.as_ref())
                    .map_or_else(
                        || mark.literal.replace(DIGIT_SEPARATOR, ""),
                        |v| v.to_string(),
                    );
                standing
                    .entry(value)
                    .or_default()
                    .push((fm, covered, reason.clone()));
                None
            }
        };
        if let Some(note) = problem {
            problems.push((fm, note));
        }
    }
    let mut report = Vec::new();
    for marks in standing.values() {
        if let [(FileMark { path, mark, .. }, covered, reason)] = marks.as_slice() {
            covered.iter().for_each(|&i| all[i].list = List::Marked);
            report.push(format!(
                "{path}{LOCATION_SEPARATOR}{}: {} {}: {reason}",
                mark.line,
                marks::MARK_WORD,
                mark.literal
            ));
            continue;
        }
        let places: Vec<String> = marks
            .iter()
            .map(|(fm, ..)| format!("{}{LOCATION_SEPARATOR}{}", fm.path, fm.mark.line))
            .collect();
        for (fm, ..) in marks {
            problems.push((
                fm,
                format!(
                    "{} is marked in {} places ({}): a number written twice is one fact, so name it once",
                    fm.mark.literal,
                    marks.len(),
                    places.join(NOTE_LIST_SEPARATOR)
                ),
            ));
        }
    }
    all.extend(problems.into_iter().map(|(fm, note)| Finding {
        list: List::Mark,
        path: fm.path.clone(),
        line: fm.mark.line,
        column: 1,
        kind: Kind::Mark,
        item: FILE_SCOPE.to_string(),
        literal: clean_literal(&fm.mark.literal),
        value: None,
        note,
        crate_dir: fm.crate_dir.clone(),
        in_const: None,
    }));
    report
}

/// How the history reads literals in this tree.
fn literals<'a>(root: &'a Path, scanned: &Scanned) -> history::Literals<'a> {
    history::Literals {
        of_file: Box::new(move |rel, text| literal_keys(root, rel, text)),
        of_line: line_literal,
        rules: scanned.rules.clone(),
    }
}

/// Everything one scan of the tree found.
struct Scanned {
    findings: Vec<Finding>,
    /// One line per valid mark: where, the number and the reason.
    marks: Vec<String>,
    /// `generated` policy lines for files that say they are generated (proposed by init).
    proposed: Vec<String>,
    summary: String,
    /// The file that counts the versions of the rules, when the checker is in the tree.
    rules: Option<String>,
}

/// Scans the workspace under `root` with the baseline's policy.
fn scan_tree(root: &Path, policy: Policy) -> Result<Scanned, String> {
    let mut tree = walk(root, &policy)?;
    let proposed = tree
        .rust
        .iter()
        .filter(|f| f.role == FileRole::Code && has_generated_marker(&f.path))
        .map(|f| Policy::key(PolicyKind::Generated, &f.rel))
        .collect();
    mark_test_modules(root, &mut tree.rust);

    let FileResult {
        findings: mut all,
        consts: mut defs,
        marks: found,
    } = scan_all(&tree.rust, None);
    all.extend(tree.closed.iter().map(|(rel, crate_dir)| {
        let folder = SourceFile {
            path: root.join(rel),
            rel: rel.clone(),
            crate_dir: crate_dir.clone(),
            role: FileRole::Code,
            crate_root: false,
            exposed: true,
            unit: None,
        };
        unparsed(
            &folder,
            CLOSED_FOLDER,
            proc_macro2::LineColumn {
                line: usize::default(),
                column: usize::default(),
            },
        )
    }));
    for (label, dir) in lock_references(root)? {
        let mut paths = Vec::new();
        walk_folder(&dir, &mut paths);
        paths.sort();
        let files: Vec<SourceFile> = paths
            .into_iter()
            .map(|path| SourceFile {
                rel: rel_string(&path, &dir),
                path,
                crate_dir: String::new(),
                role: FileRole::Code,
                crate_root: false,
                exposed: true,
                unit: None,
            })
            .collect();
        defs.extend(scan_all(&files, Some(&label)).consts);
    }
    eval::evaluate(&mut defs);
    let crates = Crates::read(root, &tree.crate_dirs);

    let units = tree
        .rust
        .iter()
        .filter_map(|f| f.unit.as_deref().map(|u| (f.rel.as_str(), u)))
        .collect();
    let index = ValueIndex::new(&defs, &crates, units);
    for f in all.iter_mut().filter(|f| f.list != List::Message) {
        if let Some(v) = &f.value {
            let note = ValueIndex::note(&index.importable(
                v,
                &f.crate_dir,
                &f.path,
                Some((f.path.as_str(), f.item.as_str())),
            ))
            .unwrap_or_default();
            f.note = [f.note.as_str(), note.as_str()]
                .iter()
                .filter(|n| !n.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(NOTE_LIST_SEPARATOR);
        }
    }
    let redefined = redefinitions(&defs, &index);
    restated(&mut all, &index, &redefined.test_copies);
    named_in_text(&mut all, &index, &crates);
    all.extend(redefined.findings);
    let marks = apply_marks(&mut all, &found, &index);
    for (path, crate_dir) in &tree.scripts {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        all.extend(scripts::scan(
            &rel_string(path, root),
            crate_dir,
            &text,
            &|v, c| ValueIndex::note(&index.importable(v, c, "", None)),
        ));
    }
    all.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));

    let mut counts: BTreeMap<List, (usize, usize)> = BTreeMap::new();
    for f in &all {
        let entry = counts.entry(f.list).or_default();
        if f.kind.is_text() {
            entry.1 += 1
        } else {
            entry.0 += 1
        }
    }
    let lists: Vec<String> = counts
        .iter()
        .map(|(l, (n, s))| format!("{} {n} numbers {s} strings", l.name()))
        .collect();
    let summary = format!(
        "{PROGRAM} (version {} of the rules): {} Rust files, {} scripts; {}",
        rules::RULES.len(),
        tree.rust.len(),
        tree.scripts.len(),
        lists.join(NOTE_LIST_SEPARATOR)
    );
    Ok(Scanned {
        findings: all,
        marks,
        proposed,
        summary,
        rules: own_source(root, &tree.crate_dirs).and_then(|src| {
            Path::new(RULES_SOURCE)
                .file_name()
                .map(|name| [src.as_str(), &name.to_string_lossy()].join(PATH_COMPONENT_SEPARATOR))
        }),
    })
}

/// What a run printed, and whether a check passed.
#[derive(Default)]
pub struct Outcome {
    pub clean: bool,
    /// The findings (when listing) or the check's `+ - ~ > !` lines, each with what to do.
    pub lines: Vec<String>,
    /// Every mark with its reason, after a line that counts them.
    pub marks: Vec<String>,
    /// File counts, the numbers and strings in each list, and where the baseline started.
    pub summary: Vec<String>,
}

impl Outcome {
    /// Everything, as the command prints it.
    pub fn report(&self) -> String {
        [
            self.lines.as_slice(),
            self.marks.as_slice(),
            self.summary.as_slice(),
        ]
        .concat()
        .join(&NEW_LINE.to_string())
    }
}

pub(crate) const NEW_LINE: char = '\n';
/// Heads the list of changes allowed to add lines to the baseline.
pub const ADDITIONS_HEADING: &str = "changes that could add lines to the baseline: ";
const ADDITION_INDENT: &str = "  ";

/// Heads the list of marks, with their count.
pub const MARKS_HEADING: &str = "marks: ";

fn marks_report(marks: Vec<String>) -> Vec<String> {
    std::iter::once(format!("{MARKS_HEADING}{}", marks.len()))
        .chain(marks)
        .collect()
}

/// Every change that was allowed to add lines to the baseline, after a line that counts them: a
/// reviewer compares them with the starts and inits the branch should have.
fn additions_report(additions: Vec<String>) -> Vec<String> {
    std::iter::once(format!("{ADDITIONS_HEADING}{}", additions.len()))
        .chain(
            additions
                .into_iter()
                .map(|a| format!("{ADDITION_INDENT}{a}")),
        )
        .collect()
}

/// Heads the list of lines added after the baseline started whose reason names the adoption that
/// removes them, with their count.
pub const ACCEPTED_HEADING: &str = "accepted growth, until the adoption each names: ";
pub use baseline::ACCEPTED_MARK;

/// Every accepted line the baseline gained, after a line that counts them: a reviewer judges each
/// reason, as with marks, so that accepted lines cannot pile up unseen.
fn accepted_report(accepted: Vec<String>) -> Vec<String> {
    std::iter::once(format!("{ACCEPTED_HEADING}{}", accepted.len()))
        .chain(
            accepted
                .into_iter()
                .map(|a| format!("{ADDITION_INDENT}{a}")),
        )
        .collect()
}

fn canonical(root: &Path) -> Result<PathBuf, String> {
    root.canonicalize()
        .map_err(|e| format!("{}: {e}", root.display()))
}

/// Every finding of every list under `root`, gated or not.
pub fn list(root: &Path) -> Result<Outcome, String> {
    let root = canonical(root)?;
    let accepted = baseline::parse(&baseline::read(&root.join(BASELINE_FILE))?);
    let scanned = scan_tree(&root, Policy::of_keys(&accepted))?;
    let clean = !scanned.findings.iter().any(|f| f.list.gated());
    let lines = scanned.findings.iter().map(|f| f.to_string()).collect();
    Ok(Outcome {
        clean,
        lines,
        marks: marks_report(scanned.marks),
        summary: vec![scanned.summary],
    })
}

/// Writes the baseline from the tree: the gated findings, the policy lines the baseline already has,
/// and a `generated` line for each file that says it is generated. Refuses once the baseline has
/// started on this branch.
pub fn init(root: &Path) -> Result<Outcome, String> {
    let root = canonical(root)?;
    let path = root.join(BASELINE_FILE);
    let accepted = baseline::parse(&baseline::read(&path)?);
    let mut policy = Policy::of_keys(&accepted);
    let first = scan_tree(&root, policy.clone())?;
    let history = history::read(&root, BASELINE_FILE, &literals(&root, &first), &accepted)?;
    if !history.may_init {
        return Err(format!(
            "{}: init writes the baseline once, and again only after a new version of the rules; from now on it only shrinks",
            history.start
        ));
    }
    policy
        .lines
        .extend(first.proposed.iter().filter_map(|k| Policy::parse_line(k)));
    let scanned = if first.proposed.is_empty() {
        first
    } else {
        scan_tree(&root, policy.clone())?
    };
    // An accepted line keeps its reason for as long as its literal is there.
    let mut reasons: HashMap<String, Vec<String>> = HashMap::new();
    for (line, n) in &accepted {
        if let (key, Some(_)) = baseline::split_reason(line) {
            reasons
                .entry(key.to_string())
                .or_default()
                .extend(std::iter::repeat_n(line.clone(), *n));
        }
    }
    let mut keys: Vec<String> = scanned
        .findings
        .iter()
        .filter(|f| f.list.gated())
        .map(|f| {
            let key = f.key();
            reasons.get_mut(&key).and_then(Vec::pop).unwrap_or(key)
        })
        .collect();
    keys.extend(
        policy
            .lines
            .iter()
            .map(|(k, p)| Policy::key(*k, p))
            .collect::<BTreeSet<_>>(),
    );
    let written = keys.len();
    std::fs::write(&path, baseline::render(keys))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let lines = scanned
        .proposed
        .iter()
        .map(|p| format!("proposed: {p}"))
        .collect();
    let summary = vec![
        scanned.summary,
        format!("wrote {written} lines to {}", path.display()),
    ];
    Ok(Outcome {
        clean: true,
        lines,
        marks: marks_report(scanned.marks),
        summary,
    })
}

/// The check: nothing outside the baseline, no baseline line that is no longer needed, nothing hidden,
/// and a baseline that has only shrunk since it started on this branch.
pub fn check(root: &Path) -> Result<Outcome, String> {
    let root = canonical(root)?;
    let path = root.join(BASELINE_FILE);
    let text = baseline::read(&path)?;
    let current = baseline::parse(&text);
    // Findings are matched without the reason an accepted line carries; each key keeps its line as written.
    let mut accepted = baseline::Keys::new();
    let mut written: HashMap<String, String> = HashMap::new();
    for (line, n) in &current {
        let (key, _) = baseline::split_reason(line);
        *accepted.entry(key.to_string()).or_default() += n;
        written
            .entry(key.to_string())
            .or_insert_with(|| line.clone());
    }
    let scanned = scan_tree(&root, Policy::of_keys(&accepted))?;
    let history = history::read(&root, BASELINE_FILE, &literals(&root, &scanned), &current)?;
    let mut lines = Vec::new();
    if let Some(line) = baseline::first_unsorted(&text) {
        lines.push(format!(
            "{ORDER_MARK}{BASELINE_FILE}{LOCATION_SEPARATOR}{line}{FIELD_SEPARATOR}{DO}sort {BASELINE_FILE} {SORT_IT}"
        ));
    }

    // Where each literal not gated now sits, by everything but its list; a marked number is fixed.
    let mut ungated: HashMap<String, &Finding> = HashMap::new();
    for f in scanned
        .findings
        .iter()
        .filter(|f| !f.list.gated() && f.list != List::Marked)
    {
        ungated.entry(f.rest_key()).or_insert(f);
    }
    // A restated literal leaves its list when the constant it restated goes, not by being moved.
    let hidden = |key: &str| -> Option<String> {
        let (key, _) = baseline::split_reason(key);
        let (list, rest) = key.split_once(FIELD_SEPARATOR)?;
        if list == List::Restated.name() {
            return None;
        }
        let f = ungated.get(rest)?;
        Some(format!(
            "{key}{FIELD_SEPARATOR}{NOTE_MARK}was {list}, now in the {} list at {}{LOCATION_SEPARATOR}{}{FIELD_SEPARATOR}{DO}{UNHIDE}",
            f.list.name(),
            f.path,
            f.line
        ))
    };
    for f in scanned.findings.iter().filter(|f| f.list.gated()) {
        match accepted.get_mut(&f.key()) {
            Some(n) if *n > usize::default() => *n -= 1,
            _ => lines.push(format!(
                "{NEW_MARK}{f}{FIELD_SEPARATOR}{DO}{}",
                f.instruction()
            )),
        }
    }
    let mut hidden_keys = HashSet::new();
    for (key, left) in &accepted {
        if let Some((_, prefix)) = Policy::parse_line(key) {
            if !root.join(prefix).exists() {
                lines.push(format!(
                    "{STALE_MARK}{key}{FIELD_SEPARATOR}{DO}{DELETE_LINE}{BASELINE_FILE}"
                ));
            }
            continue;
        }
        for _ in usize::default()..*left {
            lines.push(match hidden(key) {
                Some(h) => {
                    hidden_keys.insert(key.clone());
                    format!("{HIDDEN_MARK}{h}")
                }
                None => {
                    let line = written.get(key).unwrap_or(key);
                    format!("{STALE_MARK}{line}{FIELD_SEPARATOR}{DO}{DELETE_LINE}{BASELINE_FILE}")
                }
            });
        }
    }
    for key in history
        .removed
        .iter()
        .filter(|k| !current.contains_key(*k) && !hidden_keys.contains(baseline::split_reason(k).0))
    {
        if let Some(h) = hidden(key) {
            lines.push(format!("{HIDDEN_MARK}{h} (removed from the baseline)"));
        }
    }
    let mut accepted_growth = Vec::new();
    for (key, commit) in history
        .grown
        .iter()
        .filter(|(k, _)| current.contains_key(k))
    {
        if line_literal(key).is_some()
            && baseline::split_reason(key)
                .1
                .and_then(baseline::adopter)
                .is_some()
        {
            accepted_growth.push(format!("{key} (added by {commit})"));
            continue;
        }
        lines.push(format!(
            "{GROWN_MARK}{key}{FIELD_SEPARATOR}{NOTE_MARK}added by {commit}{FIELD_SEPARATOR}{DO}{DELETE_LINE}{BASELINE_FILE} {DO_NOT_GROW}"
        ));
    }
    for (what, action) in &history.problems {
        lines.push(format!("{GROWN_MARK}{what}{FIELD_SEPARATOR}{DO}{action}"));
    }
    let summary = [scanned.summary, history.start]
        .into_iter()
        .chain(additions_report(history.additions))
        .chain(accepted_report(accepted_growth))
        .collect();
    Ok(Outcome {
        clean: lines.is_empty(),
        lines,
        marks: marks_report(scanned.marks),
        summary,
    })
}

/// The reference crate whose constants a workspace constant must not restate, and the architectures
/// of it that are read (the guests the project runs).
const REFERENCE_PACKAGE: &str = "linux-raw-sys";
const REFERENCE_ARCHITECTURES: &[&str] = &["aarch64", "x86_64"];
/// The lock file at the workspace root, which names the reference crate's versions.
pub const LOCK_FILE: &str = "Cargo.lock";
const LOCK_PACKAGES: &str = "package";
const LOCK_NAME: &str = "name";
const LOCK_VERSION: &str = "version";
/// Where Cargo unpacks downloaded crates, under its home.
const CARGO_HOME_VARIABLE: &str = "CARGO_HOME";
const HOME_VARIABLE: &str = "HOME";
const CARGO_HOME_FOLDER: &str = ".cargo";
const REGISTRY_SOURCES: &[&str] = &["registry", "src"];

/// The workspace root at or above a folder: the nearest manifest with a `[workspace]` table.
pub fn workspace_root(dir: &Path) -> Result<PathBuf, String> {
    let start = canonical(dir)?;
    start
        .ancestors()
        .find(|d| {
            std::fs::read_to_string(d.join(MANIFEST))
                .ok()
                .and_then(|t| t.parse::<toml::Table>().ok())
                .is_some_and(|m| m.contains_key(WORKSPACE_TABLE))
        })
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("no workspace manifest at or above {}", start.display()))
}

/// The labels and source folders of every version of linux-raw-sys the lock file names, from Cargo's
/// sources; a version that is not unpacked is an error, since its constants could not be compared.
fn lock_references(root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let Ok(text) = std::fs::read_to_string(root.join(LOCK_FILE)) else {
        return Ok(Vec::new());
    };
    let lock: toml::Table = text.parse().map_err(|e| format!("{LOCK_FILE}: {e}"))?;
    let versions: Vec<String> = lock
        .get(LOCK_PACKAGES)
        .and_then(|p| p.as_array())
        .into_iter()
        .flatten()
        .filter(|p| p.get(LOCK_NAME).and_then(|n| n.as_str()) == Some(REFERENCE_PACKAGE))
        .filter_map(|p| {
            p.get(LOCK_VERSION)
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect();
    if versions.is_empty() {
        return Ok(Vec::new());
    }
    let home = std::env::var_os(CARGO_HOME_VARIABLE)
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os(HOME_VARIABLE).map(|h| PathBuf::from(h).join(CARGO_HOME_FOLDER))
        })
        .unwrap_or_default();
    let registries = REGISTRY_SOURCES.iter().fold(home, |p, s| p.join(s));
    let mut sources: Vec<PathBuf> = std::fs::read_dir(&registries)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    sources.sort();
    let label = REFERENCE_PACKAGE.replace(PACKAGE_NAME_JOINER, &CRATE_NAME_JOINER.to_string());
    let mut out = Vec::new();
    for version in &versions {
        for arch in REFERENCE_ARCHITECTURES {
            let found = sources
                .iter()
                .map(|r| {
                    r.join(format!("{REFERENCE_PACKAGE}{PACKAGE_NAME_JOINER}{version}"))
                        .join(SOURCE_FOLDER)
                        .join(arch)
                })
                .find(|d| d.is_dir())
                .and_then(|d| d.canonicalize().ok());
            let Some(dir) = found else {
                return Err(format!(
                    "{REFERENCE_PACKAGE} {version}, which {LOCK_FILE} names, has no {arch} sources under {}, so the workspace's constants cannot be compared with it: run `cargo fetch` in the workspace, then the check again",
                    registries.display()
                ));
            };
            out.push((format!("{label}{PATH_SEPARATOR}{arch}"), dir));
        }
    }
    Ok(out)
}

/// The command line: `check`, `init` or `list`, on the workspace at or above the given folder or the
/// current one.
pub fn main_with(args: &[String]) -> Result<Outcome, String> {
    let (command, dir) = match args {
        [command] => (command, std::env::current_dir().map_err(|e| e.to_string())?),
        [command, dir] => (command, PathBuf::from(dir)),
        _ => return Err(usage()),
    };
    let root = workspace_root(&dir)?;
    match Command::named(command) {
        Some(Command::Check) => check(&root),
        Some(Command::Init) => init(&root),
        Some(Command::List) => list(&root),
        None => Err(usage()),
    }
}

/// Runs git in a folder, as the check does.
pub fn git(dir: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    history::run(dir, args)
}
