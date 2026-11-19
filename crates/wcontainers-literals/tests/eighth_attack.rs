//! The holes an independent check of milestone L2's first run found: a test constant named after a
//! constant elsewhere (generated, in a dependent crate, or in an unrelated one) escaped as a
//! redefinition that was reported on the other constant, or not at all, never on the test's copy; a gated test literal in a crate of its own
//! was told to use a constant it cannot import; and a binary beside a library was told of the
//! library's private constants.

mod common;

use std::path::Path;
use std::sync::OnceLock;

use common::{Row, in_item, list, workspace};
use wcontainers_literals::{
    BASELINE_FILE, List, MANIFEST, NOT_IMPORTABLE, PolicyKind, policy_line,
};

const ABI: &str = r#"pub const PAGE_SIZE: u64 = 4096;
const PRIVATE_LIMIT: u64 = 5555;
mod inner { pub const INNER_LIMIT: u64 = 6666; }
pub fn limits() -> u64 { const LOCAL_LIMIT: u64 = 7777; PRIVATE_LIMIT + inner::INNER_LIMIT + LOCAL_LIMIT }

#[cfg(test)]
mod tests {
    #[test]
    fn l05_unit_test_note() { assert_eq!(super::limits(), 5555); }
}
"#;

const ABI_TEST: &str = r#"#[test]
fn h01_test_named_after_downstream() { const GRANULE: u64 = 4096; assert_eq!(abi::PAGE_SIZE, GRANULE); }
#[test]
fn h02_test_named_after_generated() { const LINUX_PAGE: u64 = 4096; assert_eq!(abi::PAGE_SIZE, LINUX_PAGE); }
#[test]
fn h13_test_copy_under_the_owner_s_name() { const PAGE_SIZE: u64 = 4096; assert_eq!(abi::PAGE_SIZE, PAGE_SIZE); }
#[test]
fn l01_separate_note_private() { assert_eq!(abi::limits(), 5555); }
#[test]
fn l02_separate_note_inner() { assert_eq!(abi::limits(), 6666); }
#[test]
fn l03_separate_note_local() { assert_eq!(abi::limits(), 7777); }
#[test]
fn l06_separate_note_public() { assert_eq!(abi::limits(), 4096); }
"#;

const VICTIM_TEST: &str = r#"#[test]
fn h18_test_named_after_unrelated() { const BLOCK: u64 = 4096; assert_eq!(abi::PAGE_SIZE, BLOCK); }
#[test]
fn w01_alias_one() { const P: u64 = abi::PAGE_SIZE; assert_eq!(P, abi::PAGE_SIZE); }
#[test]
fn w02_alias_two() { const P: u64 = abi::PAGE_SIZE; assert_eq!(P, abi::PAGE_SIZE); }
"#;

const VICTIM: &str = r#"pub const GRANULE: u64 = 4096;
#[path = "x25_place/mod.rs"]
mod x25_path_in_an_attribute;
"#;

/// The generated file, which only the baseline's policy line makes generated.
const GENERATED_PATH: &str = "crates/gen/src/linux.rs";

const FILES: &[(&str, &str)] = &[
    (MANIFEST, "[workspace]\nmembers = [\"crates/*\"]\n"),
    ("crates/abi/Cargo.toml", "[package]\nname = \"abi\"\n"),
    ("crates/abi/src/lib.rs", ABI),
    ("crates/abi/tests/up.rs", ABI_TEST),
    (
        "crates/abi/benches/b.rs",
        "fn l04_bench_note() { assert_eq!(abi::limits(), 5555); }\n",
    ),
    (
        "crates/abi/src/bin/tool.rs",
        "fn n05_bin_note() -> u64 { 5555 }\nfn main() {}\n",
    ),
    (
        "crates/abi/src/main.rs",
        "mod cli;\nfn n06_main_note() -> u64 { 5555 }\nfn main() {}\n",
    ),
    (
        "crates/abi/src/cli.rs",
        "pub fn n07_bin_module_note() -> u64 { 5555 }\n",
    ),
    (
        "crates/gen/Cargo.toml",
        "[package]\nname = \"gen\"\n\n[dependencies]\nabi = { path = \"../abi\" }\n",
    ),
    ("crates/gen/src/lib.rs", "pub mod linux;\n"),
    (GENERATED_PATH, "pub const LINUX_PAGE: u64 = 4096;\n"),
    (
        "crates/victim/Cargo.toml",
        "[package]\nname = \"victim\"\n\n[dependencies]\nabi = { path = \"../abi\" }\n",
    ),
    ("crates/victim/src/lib.rs", VICTIM),
    ("crates/victim/src/x25_place/mod.rs", "\n"),
    ("crates/victim/tests/t.rs", VICTIM_TEST),
    (
        "crates/consumer/Cargo.toml",
        "[package]\nname = \"consumer\"\n\n[dependencies]\nvictim = { path = \"../victim\" }\n",
    ),
    ("crates/consumer/src/lib.rs", "\n"),
    ("crates/other/Cargo.toml", "[package]\nname = \"other\"\n"),
    ("crates/other/src/lib.rs", "pub const BLOCK: u64 = 4096;\n"),
];

fn root() -> &'static Path {
    static ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = workspace("eighth-attack", FILES);
        let line = policy_line(PolicyKind::Generated, GENERATED_PATH);
        std::fs::write(root.join(BASELINE_FILE), format!("{line}\n")).expect("the policy line");
        root
    })
}

fn rows() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| list(root()))
}

fn caught_in(item: &str, wanted: List) {
    common::caught_in(FILES, rows(), item, Some(wanted));
}

/// The printed line of the item's first finding.
fn note(item: &str) -> String {
    assert!(
        common::written(FILES, item),
        "{item} is not in the snippets"
    );
    in_item(rows(), item)
        .first()
        .map(|r| r.text.clone())
        .unwrap_or_else(|| panic!("{item} has no finding: {:#?}", rows()))
}

macro_rules! caught {
    ($($name:ident in $list:ident),* $(,)?) => { $(#[test] #[allow(non_snake_case)] fn $name() { caught_in(stringify!($name), List::$list); })* };
}

caught! {
    h01_test_named_after_downstream in Redefinition,
    h02_test_named_after_generated in Redefinition,
    h18_test_named_after_unrelated in Redefinition,
    h13_test_copy_under_the_owner_s_name in Redefinition,
    l01_separate_note_private in Restated,
    l02_separate_note_inner in Restated,
    l03_separate_note_local in Restated,
    l04_bench_note in Restated,
}

/// A code constant is never told to import a test's constant of the same name.
#[test]
fn no_code_constant_is_a_copy_of_a_test_s() {
    let wrong: Vec<&Row> = rows()
        .iter()
        .filter(|r| r.is_in(List::Redefinition) && !r.path.contains("/tests/"))
        .filter(|r| r.text.contains("/tests/"))
        .collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
    for (path, item) in [
        ("crates/victim/src/lib.rs", "GRANULE"),
        ("crates/other/src/lib.rs", "BLOCK"),
    ] {
        assert!(
            !rows()
                .iter()
                .any(|r| r.path == path && r.item == item && r.gated()),
            "{path} {item}: {:#?}",
            rows()
        );
    }
}

/// A gated test literal in a crate of its own says when the constant it restates must first be made
/// importable; a unit test beside a private constant can use it as it is.
#[test]
fn gated_notes_say_what_cannot_be_imported() {
    for item in [
        "l01_separate_note_private",
        "l02_separate_note_inner",
        "l03_separate_note_local",
        "l04_bench_note",
    ] {
        assert!(note(item).contains(NOT_IMPORTABLE), "{}", note(item));
    }
    let public = note("l06_separate_note_public");
    assert!(
        public.contains("PAGE_SIZE") && !public.contains(NOT_IMPORTABLE),
        "{public}"
    );
    let unit = note("l05_unit_test_note");
    assert!(
        unit.contains("PRIVATE_LIMIT") && !unit.contains(NOT_IMPORTABLE),
        "{unit}"
    );
}

/// A binary beside a library, and the modules it declares, are told only of public constants.
#[test]
fn binaries_are_told_only_of_public_constants() {
    for item in ["n05_bin_note", "n06_main_note", "n07_bin_module_note"] {
        assert!(!note(item).contains("PRIVATE_LIMIT"), "{}", note(item));
    }
}

/// Still wrongly caught (docs/literals.md): two tests of one crate with a local constant of the same
/// name and value are redefinitions of each other, also when each is an alias of the owner's.
mod wrongly_caught {
    use super::caught_in;
    use wcontainers_literals::List;

    caught! { w01_alias_one in Redefinition, w02_alias_two in Redefinition }
}

/// Still gets through (docs/literals.md): a path in an attribute string.
mod gets_through {
    use super::{FILES, rows};

    #[test]
    fn x25_path_in_an_attribute() {
        assert!(super::common::written(FILES, "x25_path_in_an_attribute"));
        let place: Vec<_> = rows()
            .iter()
            .filter(|r| r.literal.contains("x25_place"))
            .collect();
        assert!(
            !place.is_empty() && place.iter().all(|r| !r.gated()),
            "{place:#?}"
        );
    }
}
