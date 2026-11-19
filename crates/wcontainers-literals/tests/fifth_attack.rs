//! The holes an independent check of the third run's tip found: a shipped binary whose path holds
//! `..`, attributes inside `cfg_attr`, a negated 2 in parentheses, roles in plain sight (a closure's
//! parameter, a wrapped owner id, a `user` parameter, an exit status through a binding), keys written
//! with `stringify!`, tests that write a named number as a float or a string, a mark on a number
//! another crate names or with no words in its reason, labels and names in messages wrongly gated,
//! string constants and messages that write out another constant or the crate's own name, and notes
//! that named private constants. The ones that still get through, or are still wrongly caught, are
//! tests of that, listed in docs/literals.md, so an improvement shows here.

mod common;

use std::sync::OnceLock;

use common::{Row, in_item, list, workspace};
use wcontainers_literals::{List, MANIFEST, NEW_MARK};

const ABI: &str = r#"pub const PAGE_SIZE: u64 = 4096;
pub const HOME_KEY: &str = "HOME";
"#;

const OTHER: &str = r#"pub const DAMPING: u64 = 4251;
pub const SHARED_LIMIT: u64 = 4254;
const HIDDEN_SIZE: u64 = 4253;
pub fn other_uses(x: u64) -> u64 { x + HIDDEN_SIZE }
"#;

const SNIPPETS: &str = r#"#![allow(dead_code, unused)]
use std::path::Path;
fn chown(_: &Path, _: Option<u32>, _: Option<u32>) {}

#[cfg_attr(all(), repr(align(4096)))]
pub struct H07CfgAttrAlign([u8; 16]);
#[cfg_attr(all(), serde(rename = "HOME"))]
pub struct N03CfgAttrRename;
pub fn h12_negated_paren() -> i32 { let errno = -(2); errno }
pub fn h13_closure_uid() -> u32 { let run_as = |uid: u32| uid; run_as(0) }
pub fn h14_chown_root(p: &Path) { chown(p, Some(0), Some(0)) }
fn spawn_user(user: u32) {}
pub fn h15_user_parameter() { spawn_user(0) }
pub fn h16_exit_through_binding(ok: bool) { let code = if ok { 0 } else { 1 }; std::process::exit(code) }
pub fn h17_stringify_key() -> Option<String> { std::env::var(stringify!(HOME)).ok() }
pub fn h18_stringify_length() -> usize { stringify!(abcdefghijkl).len() }

pub fn h21_marked_named_elsewhere(x: u64) -> u64 {
    // standalone 4251: the damping
    x * 4251
}
pub fn h22_marked_without_words(x: u64) -> u64 {
    // standalone 4252: .
    x * 4252
}
pub fn h23_private_elsewhere(x: u64) -> u64 { x * 4253 }
pub fn h24_public_elsewhere(x: u64) -> u64 { x * 4254 }
pub fn h25_bare(x: u64) -> u64 { x * 4255 }

pub const STATE_FILE: &str = "state.baseline";
pub const H26_HINT: &str = "delete the line from state.baseline";
pub fn h27_own_name() { println!("victim: started"); }
pub const H28_USAGE: &str = "usage: victim run";

pub fn l01_percent(pct: u32) -> String { format!("{pct}%") }
pub fn l02_label(name: &str) { println!("{name}:"); }
pub fn l03_encoding() -> Result<(), &'static str> { Err("input is not valid UTF-8") }
pub fn l04_seconds(secs: u64) -> String { format!("{secs}s") }

pub fn w01_progress(done: usize, total: usize) -> String { format!("{done}/{total}") }

pub fn g06_line() -> u32 { line!() }
pub enum Implicit { A, B, C, D, E }
pub fn g07_implicit_discriminant() -> u32 { Implicit::E as u32 }
pub const G08_DAMPING: f64 = 0.577;
"#;

const SNIPPETS_TAIL: &str = "pub const G08_DAMPING: f64 = 0.577;\n";

const TOOL: &str = "fn h09_bin_path_with_dots() -> u64 { 4247 }\nfn main() {}\n";

const TEST: &str = r#"#[test]
fn h11_test_writes_page_as_float() { assert_eq!(abi::PAGE_SIZE as f64, 4096.0); }
#[test]
fn h20_test_writes_page_as_string() { assert_eq!(abi::PAGE_SIZE.to_string(), "4096"); }
"#;

const FILES: &[(&str, &str)] = &[
    (MANIFEST, "[workspace]\nmembers = [\"crates/*\"]\n"),
    ("crates/abi/Cargo.toml", "[package]\nname = \"abi\"\n"),
    ("crates/abi/src/lib.rs", ABI),
    ("crates/other/Cargo.toml", "[package]\nname = \"other\"\n"),
    ("crates/other/src/lib.rs", OTHER),
    (
        "crates/victim/Cargo.toml",
        "[package]\nname = \"victim\"\n\n[dependencies]\nabi = { path = \"../abi\" }\n\n[[bin]]\nname = \"tool\"\npath = \"tests/../tests/tool.rs\"\n",
    ),
    ("crates/victim/src/lib.rs", SNIPPETS),
    ("crates/victim/src/tail.rs", SNIPPETS_TAIL),
    ("crates/victim/tests/tool.rs", TOOL),
    ("crates/victim/tests/page.rs", TEST),
];

fn root() -> &'static std::path::Path {
    static ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| workspace("fifth-attack", FILES))
}

fn rows() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| list(root()))
}

fn caught_in(item: &str, wanted: List) {
    common::caught_in(FILES, rows(), item, Some(wanted));
}

fn passes(item: &str) {
    common::passes_in(FILES, rows(), item);
}

macro_rules! caught {
    ($($name:ident in $list:ident),* $(,)?) => { $(#[test] #[allow(non_snake_case)] fn $name() { caught_in(stringify!($name), List::$list); })* };
}
macro_rules! passes {
    ($($name:ident),* $(,)?) => { $(#[test] #[allow(non_snake_case)] fn $name() { passes(stringify!($name)); })* };
}

caught! {
    h09_bin_path_with_dots in Code, H07CfgAttrAlign in Code, N03CfgAttrRename in Restated,
    h12_negated_paren in Code, h13_closure_uid in Role, h14_chown_root in Role,
    h15_user_parameter in Role, h16_exit_through_binding in Role, h17_stringify_key in Code,
    h18_stringify_length in Code, h11_test_writes_page_as_float in Restated,
    h20_test_writes_page_as_string in Restated, H26_HINT in Restated, h27_own_name in Restated,
    H28_USAGE in Restated,
}

/// The rows of the marks in a function, which sit at the file's scope on the mark's line.
fn mark_rows(function: &str) -> Vec<&'static Row> {
    let at = in_item(rows(), function)
        .first()
        .map(|r| r.line)
        .expect("the function has a finding");
    rows()
        .iter()
        .filter(|r| r.is_in(List::Mark) && r.line + 1 == at)
        .collect()
}

/// A mark on a number another crate already names, even one this crate does not depend on, fails:
/// the number is one fact, so it takes that name or a name of its own.
#[test]
fn h21_mark_on_a_number_named_elsewhere_fails() {
    assert_eq!(mark_rows("h21_marked_named_elsewhere").len(), 1);
}

/// A reason with no word in it is no reason.
#[test]
fn h22_mark_without_words_fails() {
    assert_eq!(mark_rows("h22_marked_without_words").len(), 1);
}

/// A note names only the constants the literal's crate could import: public ones and its own.
#[test]
fn h23_notes_name_only_importable_constants() {
    let note = |item: &str| {
        in_item(rows(), item)
            .first()
            .map(|r| r.text.clone())
            .expect("a finding")
    };
    assert!(!note("h23_private_elsewhere").contains("HIDDEN_SIZE"));
    assert!(note("h24_public_elsewhere").contains("SHARED_LIMIT"));
}

/// The action on a new finding mentions a note only when there is one.
#[test]
fn h25_the_action_mentions_a_note_only_when_there_is_one() {
    let out = wcontainers_literals::check(root()).expect("check");
    let line = |item: &str| {
        out.lines
            .iter()
            .find(|l| l.starts_with(NEW_MARK) && l.contains(item))
            .cloned()
            .expect("a new finding")
    };
    assert!(!line("h25_bare").contains("note"));
    assert!(line("h24_public_elsewhere").contains("note"));
}

passes! { l01_percent, l02_label, l03_encoding, l04_seconds }

/// Still wrongly caught (docs/literals.md): a count over a total reads like a path joined by hand.
mod wrongly_caught {
    use super::caught_in;
    use wcontainers_literals::List;

    caught! { w01_progress in Code }
}

/// Still gets through (docs/literals.md): a number fixed by the file's layout, an enum's implicit
/// discriminant used as a number, and a floating-point value defined twice.
mod gets_through {
    use super::passes;

    passes! { g06_line, g07_implicit_discriminant }

    #[test]
    fn g08_float_constant_defined_twice() {
        super::passes("G08_DAMPING");
    }
}
