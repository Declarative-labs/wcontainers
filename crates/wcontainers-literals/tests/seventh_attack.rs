//! The holes an independent check of the fifth run's tip found: a test that writes a named value
//! inside a constant of its own, and notes that named a private constant to a test built as a crate of
//! its own. Every other gap docs/literals.md lists that had no test of its own is recorded here, with
//! the classes that check's snippets found, so that an improvement shows as a failing test to update.

mod common;

use std::path::Path;
use std::sync::OnceLock;

use common::{Row, in_item, list, workspace};
use wcontainers_literals::{BASELINE_FILE, List, MANIFEST, PolicyKind, policy_line};

const ABI: &str = r#"pub const PAGE_SIZE: u64 = 4096;
pub const HOME_KEY: &str = "HOME";
pub const MAGIC: u32 = 0x7f454c46;
mod hidden { pub const HIDDEN_LIMIT: u64 = 4321; }
pub use hidden::HIDDEN_LIMIT;
const SIGNAL_KILL: i32 = 9;
pub fn nine() -> i32 { SIGNAL_KILL }

#[cfg(test)]
mod tests {
    #[test]
    fn n02_note_in_unit_test() { assert_eq!(super::nine(), 9); }
}
"#;

const ABI_TEST: &str = "#[test]\nfn n01_note_in_separate_test() { assert_eq!(abi::nine(), 9); }\n";

const SNIPPETS: &str = r#"#![allow(dead_code, unused)]
pub mod generated;
mod signal { pub fn sender_info(_: i32, _: i32, _: i32, _: i32) {} }
fn fail(message: &str) -> String { message.to_string() }

pub fn x01_role_in_a_macro_body() { macro_rules! quit { () => { std::process::exit(1) } } }
pub fn x02_exit_status_by_match(ok: bool) { match ok { true => {} false => { let _ = 2; } } }
pub fn x03_module_name_not_read(sig: i32, code: i32) { signal::sender_info(sig, code, 0, 0) }
pub fn x04_number_as_words() { panic!("needs sixty-four bytes") }
#[deprecated(note = "needs 4096 bytes")]
pub fn x05_number_in_an_attribute() {}
pub fn x06_lower_case_key() -> Result<(), &'static str> { Err("eof") }
pub fn x07_protocol_word(f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "get") }
pub fn x08_header_with_a_space(n: usize) -> String { format!("Content-Length: {n}\r\n") }
pub fn x09_command_line(d: &str) -> String { format!("cd {d} && mkdir -p {d}") }
pub const X10_SHADER: &str = "@compute @workgroup_size(64) fn main() {}";
pub const X11_DATA: &str = include_str!("data.txt");
pub fn x12_macro_output() -> u64 { x12_macro::page_size!() }
pub fn x13_mark_in_a_block_comment(x: f64) -> f64 { /*
// standalone 0.913: read from inside a block comment */
x * 0.913 }
pub fn x14_reason_of_one_letter(x: f64) -> f64 {
// standalone 0.917: x
x * 0.917 }
pub const MQ_MSGSIZE_DEFAULT: u64 = 8192;
pub const MQ_MSGSIZE_MAX: u64 = 8192;
pub fn x16_reexport_not_in_note(x: u64) -> u64 { x * 4321 }
pub fn x18_bits_and_twos() -> u64 { 1u64 << (u8::BITS + 2 + 2) }
pub const X19_SHIFTED: u64 = 1 << 12;
pub fn x22_own_message_helper() -> String { fail("cannot open the file at its usual place") }

#[derive(Debug)]
enum Key { HOME }
pub fn s01_debug_name_as_key() -> Option<String> { std::env::var(format!("{:?}", Key::HOME)).ok() }
pub fn s04_superscript_digits() { panic!("needs 2¹² bytes") }
pub fn s05_full_width_digits() { panic!("needs ４０９６ bytes") }
pub fn s07_mark_on_a_standard_number(x: f64) -> f64 {
// standalone 3.14159: a turn's half, which the standard library names
x * 3.14159 }
pub fn s10_if_let_status(st: std::process::ExitStatus) -> bool { if let Some(2) = st.code() { true } else { false } }
pub fn s14_contains_root(uids: &[u32]) -> bool { uids.contains(&0) }
pub fn s16_relative_path() { panic!("could not read etc/hosts") }
pub fn kind_name() -> String { format!("{:?}", Kind::Code).to_lowercase() }
#[derive(Debug)]
enum Kind { Code }

#[cfg(test)]
mod tests {
    #[test]
    fn t08_unit_test_constant() { const LIMIT: u64 = 4096; assert_eq!(abi::PAGE_SIZE, LIMIT); }
}
"#;

const TEST: &str = r#"const T07_SIZES: [u64; 2] = [4096, 8191];
#[test]
fn t06_test_page_in_const() { const P: u64 = 4096; assert_eq!(abi::PAGE_SIZE, P); }
#[test]
fn t01_test_writes_magic_as_bytes() { assert_eq!(abi::MAGIC.to_be_bytes(), *b"\x7fELF"); }
#[test]
fn x24_test_writes_a_run_time_value() { assert_eq!(victim::kind_name(), "code"); }
"#;

const GENERATED: &str = "pub fn x21_hand_written_in_generated(x: u64) -> u64 { x * 4099 }\n";

const DEPENDS_ON_ABI: &str =
    "[package]\nname = \"victim\"\n\n[dependencies]\nabi = { path = \"../abi\" }\n";

const FILES: &[(&str, &str)] = &[
    (MANIFEST, "[workspace]\nmembers = [\"crates/*\"]\n"),
    ("crates/abi/Cargo.toml", "[package]\nname = \"abi\"\n"),
    ("crates/abi/src/lib.rs", ABI),
    ("crates/abi/tests/n.rs", ABI_TEST),
    ("crates/other/Cargo.toml", "[package]\nname = \"other\"\n"),
    (
        "crates/other/src/lib.rs",
        "pub const X17_DIR_VARIABLE: &str = \"HOME\";\n",
    ),
    ("crates/engine/Cargo.toml", "[package]\nname = \"engine\"\n"),
    (
        "crates/engine/src/lib.rs",
        "pub fn x23_package_name_a_word() { panic!(\"the engine stopped\") }\n",
    ),
    ("crates/victim/Cargo.toml", DEPENDS_ON_ABI),
    ("crates/victim/src/lib.rs", SNIPPETS),
    ("crates/victim/src/generated.rs", GENERATED),
    ("crates/victim/src/data.txt", "size = 4096\n"),
    ("crates/victim/src/x20_helper.c", "int page = 4096;\n"),
    ("crates/victim/tests/t.rs", TEST),
];

/// The generated file's policy line, which only the baseline can give.
const GENERATED_PATH: &str = "crates/victim/src/generated.rs";

fn root() -> &'static Path {
    static ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = workspace("seventh-attack", FILES);
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

fn passes(item: &str) {
    common::passes_in(FILES, rows(), item);
}

/// The note of the item's first finding.
fn note(item: &str) -> String {
    in_item(rows(), item)
        .first()
        .map(|r| r.text.clone())
        .unwrap_or_else(|| panic!("{item} has no finding: {:#?}", rows()))
}

macro_rules! caught {
    ($($name:ident in $list:ident),* $(,)?) => { $(#[test] #[allow(non_snake_case)] fn $name() { caught_in(stringify!($name), List::$list); })* };
}
macro_rules! passes {
    ($($name:ident),* $(,)?) => { $(#[test] #[allow(non_snake_case)] fn $name() { passes(stringify!($name)); })* };
}

caught! {
    t06_test_page_in_const in Restated, T07_SIZES in Restated, t08_unit_test_constant in Restated,
}

/// A test built as a crate of its own is told only of constants it can import; a unit test also of the
/// private ones beside it.
#[test]
fn notes_name_only_what_the_test_can_import() {
    assert!(!note("n01_note_in_separate_test").contains("SIGNAL_KILL"));
    assert!(note("n02_note_in_unit_test").contains("SIGNAL_KILL"));
}

/// Still wrongly caught (docs/literals.md): a message given to the crate's own helper, and a crate
/// whose package name is a plain word.
mod wrongly_caught {
    use super::caught_in;
    use wcontainers_literals::List;

    caught! { x22_own_message_helper in Code, x23_package_name_a_word in Restated }
}

/// Still gets through (docs/literals.md).
mod gets_through {
    use super::{FILES, note, passes, rows};

    passes! {
        x01_role_in_a_macro_body, x02_exit_status_by_match, x03_module_name_not_read,
        x04_number_as_words, x05_number_in_an_attribute, x06_lower_case_key, x07_protocol_word,
        x08_header_with_a_space, x09_command_line, X10_SHADER, X11_DATA,
        x12_macro_output, x13_mark_in_a_block_comment, x14_reason_of_one_letter,
        MQ_MSGSIZE_DEFAULT, MQ_MSGSIZE_MAX, X17_DIR_VARIABLE, x18_bits_and_twos, X19_SHIFTED,
        x21_hand_written_in_generated, x24_test_writes_a_run_time_value,
        s01_debug_name_as_key, s04_superscript_digits, s05_full_width_digits,
        s07_mark_on_a_standard_number, s10_if_let_status, s14_contains_root, s16_relative_path,
        t01_test_writes_magic_as_bytes,
    }

    /// A constant re-exported from a private module is left out of notes.
    #[test]
    fn x16_reexport_not_in_note() {
        assert!(common::written(FILES, "x16_reexport_not_in_note"));
        assert!(!note("x16_reexport_not_in_note").contains("HIDDEN_LIMIT"));
    }

    /// Files in other languages are not read at all.
    #[test]
    fn x20_other_languages() {
        assert!(!rows().iter().any(|r| r.path.ends_with("x20_helper.c")));
    }

    use super::common;
}
