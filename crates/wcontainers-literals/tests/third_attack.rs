//! The holes an independent check of the second run found: a name given through an attribute, an exit
//! status, ids bound as a tuple, a descriptor returned by a function, Cargo's own variables, marks over
//! named or repeated numbers, scripts that compare or shift, a folder tagged as a build folder, code
//! placed in a hidden or ignored folder, and linux-raw-sys missing. The ones that still get through are
//! tests that they pass, listed in docs/literals.md, so an improvement shows here.

mod common;

use std::sync::OnceLock;

use common::{Row, commit_all, list, repository, workspace};
use wcontainers_literals::List;

const ABI: &str = r#"pub const PAGE_SIZE: u64 = 4096;
pub const CONFIG_KEY: &str = "st_uid";
pub const SIGNAL_KILL: i32 = 9;
"#;

const SNIPPETS: &str = r#"#![allow(dead_code, unused)]
use abi::PAGE_SIZE;
use std::process::ExitCode;

fn kill(_: i32, _: i32) {}

#[derive(serde::Deserialize)]
pub struct R01SerdeRename {
    #[serde(rename = "st_uid")]
    owner: u32,
}
pub fn r02_finish() -> ExitCode { ExitCode::from(2) }
pub fn r03_ids() -> u32 { let (uid, gid) = (0, 0); uid + gid }
pub fn r04_stdout_fd() -> i32 { 1 }
pub fn r05_pid(found: bool) -> i32 { if found { return -1; } 0 }
pub fn r06_secret() -> &'static str { env!("WC_SECRET") }
pub fn r07_kill_marked(pid: i32) {
    // standalone 9: the kill signal
    kill(pid, 9)
}
pub fn r08_first_copy(x: u64) -> u64 {
    // standalone 0x0101_0101_0101_0101: one in every byte
    x * 0x0101_0101_0101_0101
}
pub fn r09_second_copy(x: u64) -> u64 {
    // standalone 0x0101_0101_0101_0101: one in every byte, again
    x * 0x0101_0101_0101_0101
}
pub fn r10_decimal_in_a_message() { panic!("needs 4.5 GiB"); }

pub fn k01_cargo_version() -> &'static str { env!("CARGO_PKG_VERSION") }
pub fn k02_out_dir() -> Option<&'static str> { option_env!("OUT_DIR") }
pub fn k03_plain_attribute() {}
#[derive(serde::Deserialize)]
pub struct K04SerdeRenamePlain {
    #[serde(rename = "owner")]
    owner_id: u32,
}
"#;

const BUILD: &str = r#"fn main() { println!("cargo:rustc-cfg=has_seccomp"); }
"#;

const PYTHON: &str = "size = 1\nif 4096>size:\n    print(size)\nopen(\"/etc/hosts\")\n";
const SHELL: &str = "#!/bin/sh\necho $((4096>>2)) 2>/dev/null\ncat /etc/hosts\n";

const FILES: &[(&str, &str)] = &[
    ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
    ("crates/abi/Cargo.toml", "[package]\nname = \"abi\"\n"),
    ("crates/abi/src/lib.rs", ABI),
    (
        "crates/victim/Cargo.toml",
        "[package]\nname = \"victim\"\n\n[dependencies]\nabi = { path = \"../abi\" }\n",
    ),
    ("crates/victim/src/lib.rs", SNIPPETS),
    ("crates/victim/build.rs", BUILD),
    ("crates/victim/tools/sizes.py", PYTHON),
    ("crates/victim/tools/shift.sh", SHELL),
];

fn rows() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| list(&workspace("third-attack", FILES)))
}

fn caught(item: &str) {
    common::caught_in(FILES, rows(), item, None);
}

fn passes(item: &str) {
    common::passes_in(FILES, rows(), item);
}

macro_rules! caught {
    ($($name:ident),* $(,)?) => { $(#[test] #[allow(non_snake_case)] fn $name() { caught(stringify!($name)); })* };
}
macro_rules! passes {
    ($($name:ident),* $(,)?) => { $(#[test] #[allow(non_snake_case)] fn $name() { passes(stringify!($name)); })* };
}

caught! { R01SerdeRename, r02_finish, r03_ids, r04_stdout_fd, r05_pid, r06_secret }

passes! { k01_cargo_version, k02_out_dir, k03_plain_attribute, K04SerdeRenamePlain }

fn mark_rows(literal: &str) -> Vec<&'static Row> {
    rows()
        .iter()
        .filter(|r| r.is_in(List::Mark) && r.literal == literal)
        .collect()
}

/// A mark on a small number that a reachable constant names is refused, and the number stays gated.
#[test]
fn r07_mark_on_a_small_named_number() {
    let marks = mark_rows("9");
    assert!(
        marks.iter().any(|r| r.text.contains("SIGNAL_KILL")),
        "{marks:#?}"
    );
    caught("r07_kill_marked");
}

/// The same number marked in two places is one fact written twice: both marks fail.
#[test]
fn r08_the_same_number_marked_twice() {
    assert_eq!(mark_rows("0x0101_0101_0101_0101").len(), 2);
    caught("r08_first_copy");
    caught("r09_second_copy");
}

fn script_rows(path: &str, literal: &str) -> Vec<&'static Row> {
    rows()
        .iter()
        .filter(|r| r.is_in(List::Script) && r.path == path && r.literal == literal)
        .collect()
}

/// A number compared or shifted in a script is a number, not a redirection.
#[test]
fn scripts_that_compare_and_shift() {
    assert_eq!(script_rows("crates/victim/tools/sizes.py", "4096").len(), 1);
    assert_eq!(script_rows("crates/victim/tools/shift.sh", "4096").len(), 1);
    assert!(script_rows("crates/victim/tools/shift.sh", "2").is_empty());
}

/// A build script's instructions to Cargo (`cargo:rerun-if-changed=...`) are data, not messages.
#[test]
fn a_build_script_s_instructions_are_code() {
    let found: Vec<_> = rows()
        .iter()
        .filter(|r| r.path == "crates/victim/build.rs")
        .collect();
    assert!(found.iter().any(|r| r.is_in(List::Code)), "{found:#?}");
}

/// Still gets through (docs/literals.md): a decimal in a message, read as a version; paths in scripts.
mod gets_through {
    use super::{List, passes, rows};

    passes! { r10_decimal_in_a_message }

    #[test]
    fn paths_in_scripts() {
        assert!(
            !rows()
                .iter()
                .any(|r| r.is_in(List::Script) && r.literal.contains("/etc"))
        );
    }
}

const HIDING_LIB: &str = "pub mod tagged;\n#[path = \"../.impl/m.rs\"]\npub mod hidden;\n#[path = \"../ignored/n.rs\"]\npub mod ignored;\n";

const HIDING_FILES: &[(&str, &str)] = &[
    ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
    ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n"),
    ("crates/a/src/lib.rs", HIDING_LIB),
    ("crates/a/src/tagged/CACHEDIR.TAG", ""),
    (
        "crates/a/src/tagged/mod.rs",
        "pub fn tagged(x: u64) -> u64 { x * 7771 }\n",
    ),
    (
        "crates/a/.impl/m.rs",
        "pub fn hidden(x: u64) -> u64 { x * 7772 }\n",
    ),
    ("crates/a/.gitignore", "ignored/\n"),
    (
        "crates/a/ignored/n.rs",
        "pub fn ignored(x: u64) -> u64 { x * 7773 }\n",
    ),
];

/// Code is scanned wherever it is: in a folder holding a build-folder tag, in a hidden folder placed
/// with `#[path]`, and in a folder git ignores placed with `#[path]`.
#[test]
fn code_in_tagged_hidden_and_ignored_folders_is_scanned() {
    let root = repository("hiding-places", HIDING_FILES);
    commit_all(&root, "hid three literals");
    let found = list(&root);
    for literal in ["7771", "7772", "7773"] {
        assert!(
            found
                .iter()
                .any(|r| r.is_in(List::Code) && r.literal == literal),
            "{literal} was not scanned: {found:#?}"
        );
    }
}

const UNFETCHED_FILES: &[(&str, &str)] = &[
    ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
    (
        "Cargo.lock",
        "version = 4\n\n[[package]]\nname = \"linux-raw-sys\"\nversion = \"0.0.0-absent\"\n",
    ),
    ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n"),
    ("crates/a/src/lib.rs", "pub fn a() {}\n"),
];

/// A lock that names linux-raw-sys whose sources Cargo has not unpacked is an error, not a check that
/// quietly compares with nothing.
#[test]
fn a_missing_reference_crate_is_an_error() {
    let root = workspace("missing-reference", UNFETCHED_FILES);
    let error = match wcontainers_literals::check(&root) {
        Ok(out) => panic!("the check ran: {}", out.report()),
        Err(e) => e,
    };
    assert!(error.contains("cargo fetch"), "{error}");
}
