//! What an independent check of milestone L2 found: a test constant that shares its item path with a
//! test constant reported as a copy (two constants under different cfgs, a shadowing constant in an
//! inner block) escaped as a copy too; and a test's constant named like a crate's public constant took
//! over the value of every alias of it in code, hiding real redefinitions. Also the forms that still
//! get through, recorded.

mod common;

use std::path::Path;
use std::sync::OnceLock;

use common::{Row, in_item, list, workspace};
use wcontainers_literals::{List, MANIFEST};

const ABI: &str = r#"pub const PAGE_SIZE: u64 = 4096;
pub const LARGE_PAGE: u64 = 16384;
pub const GROUP_FILE: &str = "etc/group";
pub const HOST_NAME: &str = "wc-kernel";
pub struct Block;
impl Block { pub const SECTOR: u64 = 3333; }
"#;

const ABI_TEST: &str = r#"#[test]
fn e01_test_namesake() { const PAGE_SIZE: u8 = 7; assert_eq!(PAGE_SIZE, 7); }
"#;

const DOWN_TEST: &str = r#"mod d01_cfg_twins {
    #[cfg(target_os = "linux")]
    pub const PAGE_SIZE: u64 = 4096;
    #[cfg(not(target_os = "linux"))]
    pub const PAGE_SIZE: u64 = 16384;
}
#[test]
fn d02_block_shadow() { const PAGE_SIZE: u64 = 4096; { const PAGE_SIZE: u64 = 3333; assert!(PAGE_SIZE > 1); } assert!(PAGE_SIZE > 1); }
#[test]
fn g01_bytes_with_nul() { assert_eq!(b"wc-kernel\0".len(), abi::HOST_NAME.len() + 1); }
#[test]
fn g02_byte_chars() { assert_eq!(&[b'e', b't', b'c', b'/', b'g', b'r', b'o', b'u', b'p'], abi::GROUP_FILE.as_bytes()); }
"#;

const FILES: &[(&str, &str)] = &[
    (MANIFEST, "[workspace]\nmembers = [\"crates/*\"]\n"),
    ("crates/abi/Cargo.toml", "[package]\nname = \"abi\"\n"),
    ("crates/abi/src/lib.rs", ABI),
    ("crates/abi/tests/t.rs", ABI_TEST),
    (
        "crates/down/Cargo.toml",
        "[package]\nname = \"down\"\n\n[dependencies]\nabi = { path = \"../abi\" }\n",
    ),
    ("crates/down/src/lib.rs", "\n"),
    ("crates/down/tests/t.rs", DOWN_TEST),
    (
        "crates/x/Cargo.toml",
        "[package]\nname = \"x\"\n\n[dependencies]\nabi = { path = \"../abi\" }\n",
    ),
    (
        "crates/x/src/lib.rs",
        "const E01_PAGE: u64 = abi::PAGE_SIZE;\npub fn x() -> u64 { E01_PAGE }\n",
    ),
    ("crates/y/Cargo.toml", "[package]\nname = \"y\"\n"),
    (
        "crates/y/src/lib.rs",
        "const E01_PAGE: u64 = 4096;\npub fn y() -> u64 { E01_PAGE }\n",
    ),
];

fn root() -> &'static Path {
    static ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| workspace("tenth-attack", FILES))
}

fn rows() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| list(root()))
}

/// The item's findings of `literal`, each as its list.
fn lists_of(item: &str, literal: &str) -> Vec<String> {
    assert!(
        common::written(FILES, item),
        "{item} is not in the snippets"
    );
    in_item(rows(), item)
        .into_iter()
        .filter(|r| r.literal == literal)
        .map(|r| r.list.clone())
        .collect()
}

/// Two test constants of one item path under different cfgs: the one that copies `PAGE_SIZE` is a
/// redefinition, and the other still restates `LARGE_PAGE`.
#[test]
fn d01_cfg_twins() {
    let copy = in_item(rows(), "d01_cfg_twins");
    assert!(
        copy.iter()
            .any(|r| r.is_in(List::Redefinition) && r.literal.contains("4096")),
        "{copy:#?}"
    );
    assert_eq!(
        lists_of("d01_cfg_twins", "16384"),
        [List::Restated.name()],
        "{copy:#?}"
    );
}

/// A shadowing test constant in an inner block of a function whose outer constant is a copy.
#[test]
fn d02_block_shadow() {
    assert_eq!(
        lists_of("d02_block_shadow", "3333"),
        [List::Restated.name()],
        "{:#?}",
        in_item(rows(), "d02_block_shadow")
    );
}

/// A test's constant named like its crate's public constant leaves the aliases of that constant in
/// code alone: the redefinitions of code stay reported.
#[test]
fn e01_test_namesake_hides_no_redefinition() {
    for path in ["crates/x/src/lib.rs", "crates/y/src/lib.rs"] {
        assert!(
            rows()
                .iter()
                .any(|r| r.path == path && r.item == "E01_PAGE" && r.is_in(List::Redefinition)),
            "{path}: {:#?}",
            rows()
        );
    }
    assert!(common::written(FILES, "e01_test_namesake"));
}

/// Still gets through (docs/literals.md): a byte string that writes a text constant with a trailing
/// nul, and a text written as an array of byte characters.
mod gets_through {
    use super::{FILES, rows};

    fn passes(item: &str) {
        super::common::passes_in(FILES, rows(), item);
    }

    #[test]
    fn g01_bytes_with_nul() {
        passes("g01_bytes_with_nul");
    }

    #[test]
    fn g02_byte_chars() {
        passes("g02_byte_chars");
    }
}
