//! The holes an independent check of the third run found: data built with `format!` and other
//! message positions, a key split across a format's arguments, a shipped target whose root sits in
//! tests/, a folder the workspace manifest excludes, the C library's one-word id functions, and names
//! that hold a role word but count or collect. The ones that still get through are tests that they
//! pass, listed in docs/literals.md, so an improvement shows here.

mod common;

use std::sync::OnceLock;

use common::{Row, list, workspace};
use wcontainers_literals::List;

const ABI: &str = r#"pub const SIGNAL_KILL: i32 = 9;
pub const PATH_SEPARATOR: &str = "::";
"#;

const SNIPPETS: &str = r#"#![allow(dead_code, unused)]
use std::fmt;

fn setuid(_: u32) {}
fn getuid() -> u32 { 0 }

pub fn d01_home_entry(home: &str) -> String { format!("HOME={home}") }
pub fn d02_split_key(entry: &str) -> bool { entry.starts_with(&format!("{}=", "HOME")) }
pub struct D03Method;
impl fmt::Display for D03Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "GET") }
}
pub fn d04_errno_name() -> Result<(), &'static str> { Err("EAGAIN") }
pub fn d05_file_name(name: &str) -> String { format!("{name}.tar") }
pub fn d06_setuid_root() { setuid(0) }
pub fn d07_getuid_is_root() -> bool { getuid() == 0 }

pub fn e01_pid_counter(pids: &[u32]) -> u32 { let mut pid_count = 0; for _ in pids { pid_count += 1; } pid_count }
pub fn e02_count_status(status_messages: &[String]) -> usize { if status_messages.is_empty() { return 0; } status_messages.len() }
pub fn e03_signal_mask_clear() -> u64 { let signal_mask = 0; signal_mask }
pub fn e04_plain_word(v: Option<u8>) -> u8 { v.expect("checked") }
pub fn e05_sentence(n: usize) -> String { format!("{n} files") }

pub fn g01_header(n: usize) -> String { format!("Content-Length: {n}\r\n") }
pub fn g02_true_sum() -> u64 { true as u64 + true as u64 + true as u64 }
pub fn g03_page_from_bit_widths() -> u64 { 1u64 << (u8::BITS + u8::BITS / 2) }
pub fn g04_separator_in_a_sentence() { println!("join the parts with :: here"); }
"#;

const TOOL: &str = r#"mod f02_helper;
fn f01_tool_main() { let page = 4243u64; std::process::exit((page % f02_helper::BASE) as i32); }
"#;
const TOOL_HELPER: &str =
    "pub const BASE: u64 = 7;\npub fn f02_helper_scale(x: u64) -> u64 { x * 4245 }\n";
const EXCLUDED: &str = "pub fn f03_sneaky(x: u64) -> u64 { x * 4244 }\n";

const TEST: &str = r#"#[test]
fn g05_test_writes_named_small_signal() { let sig = abi::SIGNAL_KILL; assert_eq!(sig, 9); }
"#;

const FILES: &[(&str, &str)] = &[
    (
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"crates/victim/src/fs\"]\n",
    ),
    ("crates/abi/Cargo.toml", "[package]\nname = \"abi\"\n"),
    ("crates/abi/src/lib.rs", ABI),
    (
        "crates/victim/Cargo.toml",
        "[package]\nname = \"victim\"\n\n[dependencies]\nabi = { path = \"../abi\" }\n\n[[bin]]\nname = \"tool\"\npath = \"tests/tool.rs\"\ntest = false\n",
    ),
    ("crates/victim/src/lib.rs", SNIPPETS),
    ("crates/victim/src/fs/proc.rs", EXCLUDED),
    ("crates/victim/tests/tool.rs", TOOL),
    ("crates/victim/tests/f02_helper.rs", TOOL_HELPER),
    ("crates/victim/tests/signals.rs", TEST),
];

fn rows() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| list(&workspace("fourth-attack", FILES)))
}

fn caught(item: &str) {
    common::caught_in(FILES, rows(), item, None);
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
    d01_home_entry in Code, d02_split_key in Code, d04_errno_name in Code, d05_file_name in Code,
    d06_setuid_root in Role, d07_getuid_is_root in Role,
}

/// A `Display` that writes a protocol word writes data.
#[test]
fn d03_display_writes_a_protocol_word() {
    caught("fmt");
}

/// A shipped binary whose root, and the module it declares, sit in tests/ are code.
#[test]
fn f01_bin_rooted_in_tests() {
    caught_in("f01_tool_main", List::Code);
    caught_in("f02_helper_scale", List::Code);
}

/// A folder the workspace manifest excludes is scanned like any other: only a policy line, which is
/// growth, takes a path out of the check.
#[test]
fn f03_excluded_folder_is_scanned() {
    caught_in("f03_sneaky", List::Code);
}

passes! {
    e01_pid_counter, e02_count_status, e03_signal_mask_clear, e04_plain_word, e05_sentence,
}

/// Still gets through (docs/literals.md): a protocol line that reads as a sentence, booleans used as
/// numbers, a value built from the standard library's bit widths, a punctuation constant written in a
/// sentence, and a test that writes a small value the code names.
mod gets_through {
    use super::passes;

    passes! {
        g01_header, g02_true_sum, g03_page_from_bit_widths, g04_separator_in_a_sentence,
        g05_test_writes_named_small_signal,
    }
}
