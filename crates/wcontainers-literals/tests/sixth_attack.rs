//! The holes an independent check of the fourth run's tip found: target roots without the .rs
//! extension, workspace members in a submodule, roles compared through a method, an array or an
//! exit status's code, a test that writes a named string with `concat!`, the same fact under another
//! name in a crate that does not depend on the owner, a test that writes a byte string's text, notes
//! that named public constants in private modules, and an error text returned from a closure wrongly
//! gated. The ones that still get through, or are still wrongly caught, are tests of that, listed in
//! docs/literals.md, so an improvement shows here.

mod common;

use std::path::Path;
use std::sync::OnceLock;

use common::{Row, commit_all, git, in_item, list, repository, workspace};
use wcontainers_literals::{List, MANIFEST};

const ABI: &str = r#"pub const PAGE_SIZE: u64 = 4096;
pub const HOME_KEY: &str = "HOME";
pub const HOSTS_PATH: &str = "/etc/hosts";
pub const GROUP_TABLE: &[u8] = b"etc/group";
"#;

const OTHER: &str = r#"mod inner;
pub const HOSTS_FILE: &str = "/etc/hosts";
pub static PAGE_BYTES: u64 = 4096;
pub enum Granule { Page = 4096 }
mod deep { pub const DEEP_LIMIT: u64 = 4257; }
"#;

const OTHER_INNER: &str = "pub const INNER_LIMIT: u64 = 4256;\n";

const SNIPPETS: &str = r#"#![allow(dead_code, unused)]
fn setuid(_: u32) {}

pub fn r01_uid_eq(uid: u32) -> bool { uid.eq(&0) }
pub fn r02_ids_array() { let ids = [0, 0]; setuid(ids[0]) }
pub fn r03_status_code(st: std::process::ExitStatus) -> bool { st.code() == Some(2) }
pub fn n01_private_module_elsewhere(x: u64) -> u64 { x * 4256 }
pub fn n02_inline_private_module_elsewhere(x: u64) -> u64 { x * 4257 }

pub fn ok01_map_err(n: u64) -> Result<u32, &'static str> { u32::try_from(n).map_err(|_| "too large") }

pub fn w02_range(a: u32, b: u32) -> String { format!("{a}..{b}") }

pub fn g09_role_from_free_arithmetic() -> i32 { let stderr_fd = 1 + 1; stderr_fd }
pub const G10_TWICE_IN_ONE: [u64; 2] = [4094, 4094];
"#;

const TEST: &str = r#"#[test]
fn t01_test_writes_key_by_concat() { assert_eq!(abi::HOME_KEY, concat!("HO", "ME")); }
#[test]
fn t02_test_writes_byte_string_text() { assert_eq!(abi::GROUP_TABLE, "etc/group".as_bytes()); }
#[test]
fn g11_test_writes_page_as_product() { assert_eq!(abi::PAGE_SIZE, 4 * 1024); }
"#;

const HIDE_MANIFEST: &str = r#"[package]
name = "hide"

[dependencies]
abi = { path = "../abi" }

[lib]
path = "src/lib.txt"

[[bin]]
name = "hide-bin"
path = "src/main.in"

[[test]]
name = "hide-test"
path = "tests/t.txt"
"#;

const HIDE_LIB: &str = r#"pub fn h01_lib_not_rs() -> u64 { 4096 }
pub const H02_HOME_KEY: &str = "HOME";
"#;
const HIDE_BIN: &str = "fn h03_bin_not_rs() { std::process::exit(3) }\nfn main() {}\n";
const HIDE_TEST: &str =
    "#[test]\nfn h04_test_not_rs() { assert_eq!(abi::PAGE_SIZE as u64, 4096); }\n";

const FILES: &[(&str, &str)] = &[
    ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
    ("crates/abi/Cargo.toml", "[package]\nname = \"abi\"\n"),
    ("crates/abi/src/lib.rs", ABI),
    ("crates/other/Cargo.toml", "[package]\nname = \"other\"\n"),
    ("crates/other/src/lib.rs", OTHER),
    ("crates/other/src/inner.rs", OTHER_INNER),
    (
        "crates/victim/Cargo.toml",
        "[package]\nname = \"victim\"\n\n[dependencies]\nabi = { path = \"../abi\" }\n",
    ),
    ("crates/victim/src/lib.rs", SNIPPETS),
    ("crates/victim/tests/t.rs", TEST),
    ("crates/hide/Cargo.toml", HIDE_MANIFEST),
    ("crates/hide/src/lib.txt", HIDE_LIB),
    ("crates/hide/src/main.in", HIDE_BIN),
    ("crates/hide/tests/t.txt", HIDE_TEST),
];

fn root() -> &'static Path {
    static ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| workspace("sixth-attack", FILES))
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
    r01_uid_eq in Role, r02_ids_array in Role, r03_status_code in Role,
    t01_test_writes_key_by_concat in Restated, t02_test_writes_byte_string_text in Restated,
    HOSTS_FILE in Restated, PAGE_BYTES in Restated, Granule in Restated,
    h01_lib_not_rs in Code, H02_HOME_KEY in Restated, h03_bin_not_rs in Code,
    h04_test_not_rs in Restated,
}

passes! { ok01_map_err }

/// A note names no public constant that sits in another crate's private module.
#[test]
fn n01_notes_leave_out_constants_in_private_modules() {
    let note = |item: &str| {
        in_item(rows(), item)
            .first()
            .map(|r| r.text.clone())
            .expect("a finding")
    };
    assert!(!note("n01_private_module_elsewhere").contains("INNER_LIMIT"));
    assert!(!note("n02_inline_private_module_elsewhere").contains("DEEP_LIMIT"));
}

/// Still wrongly caught (docs/literals.md): a range shown as `a..b` reads like data.
mod wrongly_caught {
    use super::caught_in;
    use wcontainers_literals::List;

    caught! { w02_range in Code }
}

/// Still gets through (docs/literals.md): a role given a value built from free numbers, one fact
/// written twice inside one constant, and a test that writes a named number as a product.
mod gets_through {
    use super::passes;

    passes! { g09_role_from_free_arithmetic, g11_test_writes_page_as_product }

    #[test]
    fn g10_one_fact_twice_inside_one_constant() {
        super::passes("G10_TWICE_IN_ONE");
    }
}

const SUB_FILES: &[(&str, &str)] = &[
    (MANIFEST, "[package]\nname = \"sub\"\n"),
    ("src/lib.rs", "pub fn in_submodule() -> u64 { 4096 }\n"),
];

const OUTER_FILES: &[(&str, &str)] = &[
    (MANIFEST, "[workspace]\nmembers = [\"crates/*\"]\n"),
    ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n"),
    ("crates/a/src/lib.rs", "pub fn plain() {}\n"),
];

/// Git lists a submodule as one entry; the files of a checked-out one are scanned like any others.
#[test]
fn a_submodule_is_scanned() {
    let source = repository("sixth-sub-source", SUB_FILES);
    let root = repository("sixth-sub", OUTER_FILES);
    let source = source.to_string_lossy().into_owned();
    git(
        &root,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            &source,
            "crates/sub",
        ],
    );
    commit_all(&root, "a crate in a submodule");
    let rows = list(&root);
    assert!(
        in_item(&rows, "in_submodule")
            .iter()
            .any(|r| r.is_in(List::Code) && r.path == "crates/sub/src/lib.rs"),
        "{rows:#?}"
    );
    assert!(!wcontainers_literals::check(&root).expect("check").clean);
}

/// A submodule that is not checked out cannot be read, so it is a gated finding, not a silent pass.
#[test]
fn a_submodule_not_checked_out_fails() {
    let source = repository("sixth-closed-source", SUB_FILES);
    let root = repository("sixth-closed", OUTER_FILES);
    let head = git(&source, &["for-each-ref", "--format=%(objectname)"]);
    let link = format!("160000,{},crates/sub", head.trim());
    git(&root, &["update-index", "--add", "--cacheinfo", &link]);
    std::fs::create_dir_all(root.join("crates/sub")).expect("the empty folder of a submodule");
    commit_all(&root, "a submodule not checked out");
    let rows = list(&root);
    assert!(
        rows.iter()
            .any(|r| r.is_in(List::Unparsed) && r.path == "crates/sub"),
        "{rows:#?}"
    );
}
