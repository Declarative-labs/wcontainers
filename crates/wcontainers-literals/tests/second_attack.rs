//! A second attack, written for the final rule: twenty snippets that try to hide a number or a
//! meaningful string (`hNN_*`) and ten that are legitimate (`kNN_*`). The ones that still get through
//! are tests that they pass, listed in docs/literals.md, so an improvement shows here.

mod common;

use std::sync::OnceLock;

use common::{Row, in_item, list, workspace};

const ABI: &str = r#"pub const PAGE_SIZE: u64 = 4096;
pub const PAGE_SHIFT: u32 = PAGE_SIZE.trailing_zeros();
pub const PAGE_MASK: u64 = PAGE_SIZE - 1;
"#;

const SNIPPETS: &str = r#"#![allow(dead_code, unused)]
use abi::{PAGE_MASK, PAGE_SIZE};

pub struct Wrapper(u64);
pub struct Header { a: u32, b: u32 }
const COUNT: usize = 8;
const HOST_VARIABLE: &str = "WC_REFERENCE_HOST";
fn write(_: i32, _: &[u8]) {}
fn kill(_: i32, _: i32) {}

pub fn h01_byte_string_index() -> u8 { b"\x00\x10"[1] }
pub fn h03_tuple_struct() -> Wrapper { Wrapper(4096) }
pub fn h04_sum_of_ones(x: u64) -> u64 { x * (1 + 1 + 1 + 1) }
pub fn h05_shifted_twos(x: u64) -> u64 { x >> (2 << 2 << 2) }
pub fn h07_joined_path() -> String { ["/", "proc"].concat() }
pub fn h10_format_width(x: u64) -> String { format!("{x:>12}") }
pub fn h11_minus_two(x: i64) -> i64 { x * -2 }
pub fn h12_scientific(x: f64) -> f64 { x * 1e3 }
pub fn h13_match_guard(x: u64) -> bool { match x { n if n > 4096 => true, _ => false } }
pub fn h15_byte_arithmetic() -> char { (b'a' + 25) as char }
pub fn h17_include_bytes() -> &'static [u8] { include_bytes!("lib.rs") }
pub fn h18_named_like_a_test() -> u64 { 8192 }
pub fn h19_message_names_a_constant() { panic!("set WC_REFERENCE_HOST first"); }
pub fn h20_message_holds_a_path() { eprintln!("cannot open /etc/hosts"); }
pub fn h21_assert_message_number(ok: bool) { assert!(ok, "needs 64 bytes"); }
pub const H22_PAGE: u64 = 2 * 2 * 2 * 2 * 2 * 2 * 2 * 2 * 2 * 2 * 2 * 2;
pub fn h24_stderr_as_two(m: &[u8]) { write(2, m); }
pub fn h25_exit_minus_one() { std::process::exit(-1) }
pub fn h26_every_process(signal: i32) { kill(-1, signal) }
pub fn h27_mark_on_a_named_number(x: u64) -> u64 {
    // standalone 4096: a page
    x * 4096
}

pub fn k01_half(len: usize) -> usize { len / 2 }
pub fn k02_double(cap: usize) -> usize { cap * 2 }
pub fn k03_previous(v: &[u8]) -> Option<u8> { v.get(v.len().checked_sub(1)?).copied() }
pub fn k04_derived_size() -> usize { size_of::<Header>() * COUNT }
pub fn k05_standard_library() -> (u32, f64) { (u32::MAX, std::f64::consts::PI) }
pub fn k06_plain_message(path: &str, e: std::io::Error) -> String { format!("cannot read {path}: {e}") }
pub fn k07_placeholder_number(count: usize) { println!("{count} entries"); }
pub fn k08_marked(x: f64) -> f64 {
    // standalone 0.577: the damping the reference implementation uses, which has no name upstream
    x * 0.577
}
pub fn k09_named(x: u64) -> u64 { x & PAGE_MASK }
pub fn k11_message_names_by_constant() { panic!("set {HOST_VARIABLE} first"); }
pub fn k12_mark_inside_a_string() -> &'static str {
    "first line
// standalone 7: inside a string, so not a mark
"
}
"#;

const FILES: &[(&str, &str)] = &[
    ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
    ("crates/abi/Cargo.toml", "[package]\nname = \"abi\"\n"),
    ("crates/abi/src/lib.rs", ABI),
    (
        "crates/victim/Cargo.toml",
        "[package]\nname = \"victim\"\n\n[dependencies]\nabi = { path = \"../abi\" }\n",
    ),
    ("crates/victim/src/lib.rs", SNIPPETS),
];

fn rows() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| list(&workspace("second-attack", FILES)))
}

fn caught(item: &str) {
    let found = in_item(rows(), item);
    assert!(
        found.iter().any(|r| r.gated()),
        "{item} was not caught: {found:#?}"
    );
}

fn passes(item: &str) {
    let gated: Vec<&Row> = in_item(rows(), item)
        .into_iter()
        .filter(|r| r.gated())
        .collect();
    assert!(gated.is_empty(), "{item} was wrongly caught: {gated:#?}");
}

macro_rules! caught {
    ($($name:ident),* $(,)?) => { $(#[test] #[allow(non_snake_case)] fn $name() { caught(stringify!($name)); })* };
}
macro_rules! passes {
    ($($name:ident),* $(,)?) => { $(#[test] #[allow(non_snake_case)] fn $name() { passes(stringify!($name)); })* };
}

caught! {
    h01_byte_string_index, h03_tuple_struct, h07_joined_path, h11_minus_two, h12_scientific, h13_match_guard,
    h15_byte_arithmetic, h17_include_bytes, h18_named_like_a_test, h19_message_names_a_constant,
    h20_message_holds_a_path, h21_assert_message_number, h25_exit_minus_one,
}

/// A mark on a number that has a name is refused, and the number stays gated.
#[test]
fn h27_mark_on_a_named_number() {
    let marks: Vec<&Row> = rows().iter().filter(|r| r.list == "mark").collect();
    assert!(
        marks
            .iter()
            .any(|r| r.text.contains("has a name, PAGE_SIZE")),
        "{marks:#?}"
    );
    caught("h27_mark_on_a_named_number");
}

/// A valid mark moves its number to the marked list.
#[test]
fn k08_marked_number_is_listed() {
    let marked: Vec<&Row> = in_item(rows(), "k08_marked")
        .into_iter()
        .filter(|r| r.list == "marked")
        .collect();
    assert_eq!(marked.len(), 1, "{:#?}", in_item(rows(), "k08_marked"));
}

/// Text inside a string that looks like a mark is not a mark.
#[test]
fn k12_mark_inside_a_string_is_text() {
    assert!(!rows().iter().any(|r| r.list == "mark" && r.literal == "7"));
}

passes! {
    k01_half, k02_double, k03_previous, k04_derived_size, k05_standard_library, k06_plain_message,
    k07_placeholder_number, k08_marked, k09_named, k11_message_names_by_constant,
}

/// Still gets through (docs/literals.md): a value built only from free numbers (`1 + 1 + 1 + 1`,
/// `2 << 2 << 2`, a constant of twelve 2s that equals the page size), a format width, and -1 or 2 given
/// to a function whose name does not say it takes a descriptor or a process id (`write(2, ..)`,
/// `kill(-1, ..)`).
mod gets_through {
    use super::passes;

    passes! { h04_sum_of_ones, h05_shifted_twos, h10_format_width, H22_PAGE, h24_stderr_as_two, h26_every_process }
}
