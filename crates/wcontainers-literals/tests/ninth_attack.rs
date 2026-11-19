//! The holes an independent check of milestone L2's second run found: a test's associated constant
//! named like an associated constant of code escaped both lists; a binary's unit test was told to make
//! its own file's constant public; a copy of a constant inside a function was told to import it; and a
//! C string was not compared as text. Also the forms that still get through, recorded.

mod common;

use std::path::Path;
use std::sync::OnceLock;

use common::{Row, in_item, list, workspace};
use wcontainers_literals::{List, MANIFEST, NOT_IMPORTABLE};

const ABI: &str = r#"pub const PAGE_SIZE: u64 = 4096;
pub const GROUP_FILE: &str = "etc/group";
pub trait Backend { const GRANULE: u64; }
pub struct Real;
impl Backend for Real { const GRANULE: u64 = PAGE_SIZE; }
pub struct Frame;
impl Frame { pub const BYTES: u64 = 4096; }
pub struct Block;
impl Block { pub const SECTOR: u64 = 3333; }
pub fn code_local() -> u64 { const LOCAL_COPY: u64 = 4096; LOCAL_COPY }
mod p2 { const P2_ONLY: u64 = 8282; pub fn g() -> u64 { P2_ONLY } }
pub fn p2() -> u64 { p2::g() }

#[cfg(test)]
mod tests {
    #[test]
    fn s01_unit_test_private_sibling_module() { assert_eq!(super::p2(), 8282); }
}
"#;

const MAIN: &str = r#"mod cli;
const MAIN_LIMIT: u64 = 9191;
fn main() { let _ = MAIN_LIMIT; cli::run(); }
fn b02_bin_code_own_constant() -> u64 { 9191 }

#[cfg(test)]
mod tests {
    #[test]
    fn b01_bin_unit_test_own_constant() { assert_eq!(super::MAIN_LIMIT, 9191); }
}
"#;

const CLI: &str = r#"const CLI_LIMIT: u64 = 7171;
pub fn run() { let _ = CLI_LIMIT; }
fn b03_bin_module_code_own_constant() -> u64 { 7171 }
"#;

const DOWN_TEST: &str = r#"struct A01Mine;
impl A01Mine { const BYTES: u64 = 4096; }
struct A02Mock;
impl abi::Backend for A02Mock { const GRANULE: u64 = 4096; }
struct A03Disk;
impl A03Disk { const SECTOR: u64 = 3333; }
#[test]
fn a04_same_name_other_value() { const PAGE_SIZE: u64 = 4096 * 2; assert_eq!(PAGE_SIZE, 2 * abi::PAGE_SIZE); }
#[test]
fn a05_same_name_as_digits() { const PAGE_SIZE: &str = "4096"; assert_eq!(PAGE_SIZE, abi::PAGE_SIZE.to_string()); }
#[test]
fn c01_c_string() { const G: &core::ffi::CStr = c"etc/group"; assert_eq!(G.to_bytes(), abi::GROUP_FILE.as_bytes()); }
#[test]
fn r01_copy_of_a_code_local() { const LOCAL_COPY: u64 = 4096; assert_eq!(LOCAL_COPY, abi::code_local()); }
#[test]
fn x01_path_joined_at_run_time() { assert_eq!(["etc", "group"].join("/"), abi::GROUP_FILE); }
#[test]
fn x02_path_from_chars() { let p: String = ['e', 't', 'c', '/', 'g', 'r', 'o', 'u', 'p'].iter().collect(); assert_eq!(p, abi::GROUP_FILE); }
#[test]
fn x03_number_from_bytes() { const P: u64 = u64::from_le_bytes([0, 16, 0, 0, 0, 0, 0, 0]); assert_eq!(P, abi::PAGE_SIZE); }
#[test]
fn x04_expression_in_a_test_constant() { const P: u64 = 4095 + 1; assert_eq!(P, abi::PAGE_SIZE); }
"#;

const FILES: &[(&str, &str)] = &[
    (MANIFEST, "[workspace]\nmembers = [\"crates/*\"]\n"),
    ("crates/abi/Cargo.toml", "[package]\nname = \"abi\"\n"),
    ("crates/abi/src/lib.rs", ABI),
    ("crates/abi/src/main.rs", MAIN),
    ("crates/abi/src/cli.rs", CLI),
    (
        "crates/abi/src/bin/one.rs",
        "#[path = \"../shared.rs\"]\nmod shared;\nfn s02_first_bin() -> u64 { 6161 }\nfn main() {}\n",
    ),
    (
        "crates/abi/src/bin/two.rs",
        "#[path = \"../shared.rs\"]\nmod shared;\nfn s02_second_bin() -> u64 { 6161 }\nfn main() {}\n",
    ),
    (
        "crates/abi/src/shared.rs",
        "const SHARED_LIMIT: u64 = 6161;\npub fn get() -> u64 { SHARED_LIMIT }\n",
    ),
    (
        "crates/down/Cargo.toml",
        "[package]\nname = \"down\"\n\n[dependencies]\nabi = { path = \"../abi\" }\n",
    ),
    ("crates/down/src/lib.rs", "\n"),
    ("crates/down/tests/t.rs", DOWN_TEST),
];

fn root() -> &'static Path {
    static ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| workspace("ninth-attack", FILES))
}

fn rows() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| list(root()))
}

fn caught_in(item: &str, wanted: List) {
    common::caught_in(FILES, rows(), item, Some(wanted));
}

/// The printed lines of the item's findings, one after another.
fn notes(item: &str) -> String {
    assert!(
        common::written(FILES, item),
        "{item} is not in the snippets"
    );
    let found: Vec<String> = rows()
        .iter()
        .filter(|r| r.item.contains(item))
        .map(|r| r.text.clone())
        .collect();
    assert!(!found.is_empty(), "{item} has no finding: {:#?}", rows());
    found.join("\n")
}

macro_rules! caught {
    ($($name:ident in $list:ident),* $(,)?) => { $(#[test] #[allow(non_snake_case)] fn $name() { caught_in(stringify!($name), List::$list); })* };
}

caught! {
    A01Mine in Restated,
    A03Disk in Restated,
    a04_same_name_other_value in Restated,
    a05_same_name_as_digits in Restated,
    c01_c_string in Restated,
    r01_copy_of_a_code_local in Redefinition,
}

/// A mock that implements the real trait with the real value restates it.
#[test]
fn a02_mock_trait_impl_writes_the_real_value() {
    let line = notes("A02Mock");
    assert!(
        line.starts_with(&List::Restated.name()) && line.contains("PAGE_SIZE"),
        "{line}"
    );
}

/// A binary's own files can use their crate's private constants as they are; the copy of a constant
/// inside a function is told that it must first be made importable.
#[test]
fn notes_know_a_binary_s_own_constants() {
    let unit = notes("b01_bin_unit_test_own_constant");
    assert!(
        unit.contains("MAIN_LIMIT") && !unit.contains(NOT_IMPORTABLE),
        "{unit}"
    );
    assert!(
        notes("b02_bin_code_own_constant").contains("MAIN_LIMIT"),
        "{}",
        notes("b02_bin_code_own_constant")
    );
    assert!(
        notes("b03_bin_module_code_own_constant").contains("CLI_LIMIT"),
        "{}",
        notes("b03_bin_module_code_own_constant")
    );
    let local = notes("r01_copy_of_a_code_local");
    assert!(local.contains(NOT_IMPORTABLE), "{local}");
}

/// Still gets through (docs/literals.md): a path joined from pieces or built from characters at run
/// time, a named number built from its bytes by the standard library, a test constant written as an
/// expression whose literals no constant has; and in notes, a private constant of a private sibling
/// module named without saying it cannot be reached, and a module two binaries declare counted only
/// in the first.
mod gets_through {
    use super::{FILES, in_item, notes, rows};
    use wcontainers_literals::NOT_IMPORTABLE;

    fn passes(item: &str) {
        super::common::passes_in(FILES, rows(), item);
    }

    #[test]
    fn x01_path_joined_at_run_time() {
        passes("x01_path_joined_at_run_time");
    }

    #[test]
    fn x02_path_from_chars() {
        passes("x02_path_from_chars");
    }

    #[test]
    fn x03_number_from_bytes() {
        passes("x03_number_from_bytes");
    }

    #[test]
    fn x04_expression_in_a_test_constant() {
        passes("x04_expression_in_a_test_constant");
    }

    #[test]
    fn s01_unit_test_private_sibling_module() {
        let line = in_item(rows(), "s01_unit_test_private_sibling_module")
            .first()
            .map(|r| r.text.clone())
            .unwrap_or_default();
        assert!(
            line.contains("P2_ONLY") && !line.contains(NOT_IMPORTABLE),
            "{line}"
        );
    }

    #[test]
    fn s02_module_two_binaries_declare() {
        assert!(notes("s02_first_bin").contains("SHARED_LIMIT"));
        let second = notes("s02_second_bin");
        assert!(!second.contains("SHARED_LIMIT"), "{second}");
    }
}
