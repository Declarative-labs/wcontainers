//! The audit's attack on the scanner, kept as tests: each snippet that must be caught is a test that it
//! is caught, each legitimate one a test that it passes, and each one that still gets through a test that
//! records it (listed in docs/literals.md), so that an improvement shows as a failing test to update.
//! The snippets are the audit's files as they were attacked; each `vNN_*` item hides one value, each
//! `lNN_*` item is legitimate.

mod common;

use std::sync::OnceLock;

use common::{Row, in_item, list, workspace};

const FILES: &[(&str, &str)] = &[
    (
        "Cargo.toml",
        r#"[workspace]
members = ["crates/*"]
resolver = "2"
"#,
    ),
    (
        "crates/abi/Cargo.toml",
        r#"[package]
name = "abi"
"#,
    ),
    (
        "crates/abi/src/lib.rs",
        r#"pub const PAGE_SIZE: u64 = 4096;
pub const PAGE_SHIFT: u32 = PAGE_SIZE.trailing_zeros();
pub const LOWEST_ADDRESS: u64 = 0x10000;
pub const PROC_ROOT: &str = "/proc";
pub const SEPARATOR: u8 = b'/';
pub const POLL_INTERVAL_MS: u64 = 250;
pub const MAX_ERRNO: u64 = 4095;
"#,
    ),
    (
        "crates/victim/Cargo.toml",
        r#"[package]
name = "victim"

[dependencies]
abi = { path = "../abi" }
"#,
    ),
    (
        "crates/victim/run.sh",
        r#"#!/bin/sh
# magic numbers in a script
sleep 37
dd if=/dev/zero bs=4096 count=1
timeout 250 true
"#,
    ),
    (
        "crates/victim/src/consts.in",
        r#"4096 * 2
"#,
    ),
    (
        "crates/victim/src/examples/mod.rs",
        r#"//! Example programs the runtime ships (production code, in a folder named examples).
pub fn v36_folder_named_examples(x: u64) -> u64 { x & 0xfff }
"#,
    ),
    (
        "crates/victim/src/generated_by_accident.rs",
        r#"//! The socket buffer sizes. Do not edit these without re-measuring on the reference kernel.

pub fn v34_generated_marker_in_a_doc(x: u64) -> u64 { x * 4096 + 0xfff }
"#,
    ),
    (
        "crates/victim/src/legit.rs",
        r#"//! Lines that are legitimate under the owner's rule.
use abi::{PAGE_SIZE, PAGE_SHIFT};
use core::mem::{offset_of, size_of};

#[repr(C)]
pub struct Header { magic: u32, len: u32 }

/// The one definition of this crate's own limit.
pub const MY_LIMIT: u32 = 37;
/// Derived from another constant.
pub const PAGE_MASK: u64 = PAGE_SIZE - 1;

pub fn l01_next(i: usize, n: usize) -> usize { let mut j = i + 1; j += 1; n - 1 + j }
pub fn l02_first(v: &[u8]) -> (u8, &[u8]) { (v[0], &v[1..]) }
pub fn l03_size_of() -> usize { size_of::<u64>() }
pub fn l04_offset_of() -> usize { offset_of!(Header, len) }
pub fn l05_trailing_zeros() -> u32 { PAGE_SIZE.trailing_zeros() }
pub fn l06_loop(n: usize) -> usize { let mut s = 0; for i in 0..n { s += i; } s }
pub fn l07_checked(x: u64) -> Option<u64> { x.checked_add(1) }
pub fn l08_repeat() -> Vec<u8> { vec![0u8; PAGE_SIZE as usize] }
pub fn l09_bool() -> bool { true }
pub fn l10_empty() -> String { "".to_string() }
pub fn l11_message(x: u64) { println!("done: {x}"); }
pub fn l12_err() -> Result<(), &'static str> { Err("bad input") }
pub fn l13_expect(v: Option<u8>) -> u8 { v.expect("a value") }
pub fn l14_div_ceil(x: u64) -> u64 { x.div_ceil(PAGE_SIZE) }
pub fn l15_shift_named(x: u64) -> u64 { x >> PAGE_SHIFT }
pub fn l16_custom_error_message() -> Result<(), MyError> { Err(MyError::Invalid("header too short")) }
pub enum MyError { Invalid(&'static str) }
pub fn l17_panic() { panic!("state that cannot happen"); }
pub fn l18_midpoint(lo: usize, hi: usize) -> usize { lo + (hi - lo) / 2 }
pub fn l19_is_empty(v: &[u8]) -> bool { v.len() == 0 }
pub fn l20_bits() -> u32 { u64::BITS - 1 }

#[cfg(all(test, not(miri)))]
mod tests_not_miri {
    fn sample_input() -> Vec<u8> { vec![7, 8, 9] }
    #[test]
    fn t() { assert_eq!(sample_input().len(), 3); }
}
"#,
    ),
    (
        "crates/victim/src/lib.rs",
        r#"//! Attack snippets: each `vNN_*` hides one magic value a hurried programmer might write. Each `lNN_*`
//! (in legit.rs) is legitimate under the owner's rule. The function name is the finding's item.
#![allow(dead_code, unused)]

use std::time::Duration;
use std::fmt::Write as _;
use abi::{PAGE_SIZE, PROC_ROOT};

mod legit;
mod generated_by_accident;
mod target;
mod examples;
mod protocol;
mod more;
#[cfg(test)]
mod tests;

pub struct Foo;
pub struct Buf<const N: usize>;
impl<const N: usize> Buf<N> { pub fn new() -> Self { Buf } }
pub trait Guest { const PAGE_SIZE: u64; const ALIGN: u64; }

pub fn v01_shift_by_12(address: u64) -> u64 { address >> 12 }
pub fn v02_mask_0xfff(address: u64) -> u64 { address & 0xfff }
pub fn v03_one_shl_12() -> u64 { 1 << 12 }
pub fn v04_inside_vec_macro() -> Vec<u8> { vec![0u8; 4096] }
pub fn v05_match_arm(n: u64) -> bool { match n { 4096 => true, _ => false } }
pub fn v06_array_length_in_let() { let page: [u8; 4096] = [0; PAGE_SIZE as usize]; }
pub fn v07_const_generic() -> Buf<4096> { Buf::<4096>::new() }
pub fn v08_closure(v: &[u64]) -> Vec<u64> { v.iter().map(|x| x / 4096).collect() }
pub fn v09_inline_const_block() -> u64 { const { 4096 * 2 } }
pub fn v10_local_const_other_name(x: u64) -> u64 { const LOCAL_PAGE: u64 = 4096; x / LOCAL_PAGE }
/// Retypes linux-raw-sys's errno::EINVAL under its own name.
pub const EINVAL: i32 = 22;
/// Retypes linux-raw-sys's O_CLOEXEC under another name.
pub const OPEN_CLOEXEC: u32 = 0o2000000;
pub fn v11_parsed_string() -> u64 { "4096".parse().unwrap() }
pub fn v12_char(path: &str) -> bool { path.ends_with('/') }
pub fn v13_byte(path: &[u8]) -> bool { path.first() == Some(&b'/') }
pub fn v14_float(x: f32) -> f32 { x + 0.5 }
pub fn v15_cast() -> u64 { 4096 as u64 }
pub fn v16_duration() -> Duration { Duration::from_secs(30) }
pub fn v17_path_string() -> &'static str { "/etc/passwd" }
pub fn v19_write_macro_hides_value(out: &mut String) { write!(out, "1048576\n").unwrap(); }
pub fn v20_format_hides_path(pid: u32) -> String { format!("/proc/{pid}/maps") }
pub fn v21_let_stdout_fd() -> i32 { let stdout_fd = 1; stdout_fd }
pub fn v22_root_uid_comparison(uid: u32) -> bool { uid == 0 }
pub fn v23_fd_argument(buf: &[u8]) { write_fd(1, buf); }
fn write_fd(_: i32, _: &[u8]) {}

/// A whole impl hidden inside an anonymous const block.
const _: () = {
    impl Foo {
        pub fn v24_impl_in_const_block(x: u64) -> u64 { x >> 12 }
    }
};
/// Run-time code inside a static's initialiser.
pub static V25_HANDLER: fn(u64) -> u64 = |x| x & 0xfff;

#[cfg(any(test, feature = "fast"))]
pub fn v26_production_under_any_test(x: u64) -> u64 { x * 4096 }

impl Foo {
    /// Same name and value as abi::PAGE_SIZE, but associated.
    pub const PAGE_SIZE: u64 = 4096;
}
pub struct X86;
impl Guest for X86 {
    const PAGE_SIZE: u64 = 0x1000;
    const ALIGN: u64 = 1;
}
#[repr(i32)]
pub enum Errno { EINVAL = 22, ENOENT = 2 }
#[repr(u8)]
pub enum Signal { Segv = 11, Bus = 7 }

macro_rules! v30_stdout { () => { 1 }; }
macro_rules! v30_page { () => { 4096 }; }

#[repr(align(4096))]
pub struct V31Aligned([u8; 16]);

pub fn v32_include() -> u64 { include!("consts.in") }

pub fn v37_poll_one_second() -> Duration { Duration::from_secs(1) }
pub fn v38_ok_or_hides_path(x: Option<u8>) -> Result<u8, &'static str> { x.ok_or("/dev/null") }
pub fn v39_assert_eq_hides_value(name: &str) { assert_eq!(name, "/proc"); }
pub fn v40_not_zero_mask() -> u64 { !0 }
pub fn v41_unknown_macro_tokens() { my_dsl!(page => 1, size 4096); }
pub fn v42_min_counting(x: u64) -> u64 { x.min(4096) }
pub fn v43_elf_magic(b: &[u8]) -> bool { b.starts_with(b"\x7fELF") }
pub fn v44_hex_string() -> u64 { u64::from_str_radix("1000", 16).unwrap() }
pub fn v45_size_of_disguise() -> usize { core::mem::size_of::<[u8; 4096]>() }
/// Same name as abi's, derived-looking, so it has no value and is never matched.
pub const PAGE_SIZE_DISGUISED: u64 = core::mem::size_of::<[u8; 4096]>() as u64;
pub const SEPARATOR: u8 = core::mem::size_of::<[u8; 47]>() as u8;
pub fn v46_struct_field() -> Config { Config { page: 4096, fd: 1 } }
pub struct Config { page: u64, fd: i32 }
pub fn v47_wrapping_shl(x: u64) -> u64 { x.wrapping_shl(12) }
pub fn v48_round_up(x: u64) -> u64 { (x + 4095) & !4095 }
pub fn v49_let_tuple() -> (u64, u64) { let (lo, hi) = (4096, 8192); (lo, hi) }
pub fn v50_negative_errno() -> i64 { -22 }
pub fn v51_exit_status() { std::process::exit(1) }
pub fn v52_return_zero_success() -> i32 { 0 }
pub fn v53_assign_root(uid: &mut u32) { *uid = 0; }
pub fn v54_concat() -> &'static str { concat!("/proc", "/self") }
pub fn v55_env_name() -> Option<String> { std::env::var("WC_REFERENCE_HOST").ok() }
pub fn v56_context_hides_path(r: Result<(), std::io::Error>) -> Result<(), String> { r.map_err(|_| "x".to_string()).map(|_| ()).or_else(|_| Err("/etc/hosts".to_string())) }
pub fn v57_err_hides_value() -> Result<(), &'static str> { Err("/etc/resolv.conf") }
pub fn v58_io_error_hides_path() -> std::io::Error { std::io::Error::other("/var/run/docker.sock") }
pub fn v59_index_with_meaning(regs: &[u64]) -> u64 { regs[8] }
pub fn v60_index_one_meaning(fds: &[i32]) -> i32 { fds[1] }
pub fn v61_float_one(x: f64) -> f64 { x.max(1.0) }
pub fn v62_long_string() -> &'static str {
    "0123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789TAIL-A"
}
pub fn v63_debug_assert_eq_message_value(s: &str) { debug_assert_eq!(s, "4096"); }
pub fn v64_tracing_like(n: u64) { log::info!(target: "page", "size {}", 4096); }
pub fn v65_byte_array_literal() -> [u8; 4] { [0x7f, b'E', b'L', b'F'] }
pub fn v66_let_else(v: Option<u64>) -> u64 { let Some(x) = v else { return 4096 }; x }
pub fn v67_async_block() -> impl std::future::Future<Output = u64> { async { 4096 } }
pub trait V68Trait { const SIZE: u64 = 4096; fn f(&self) -> u64 { 4096 } }
pub fn v69_const_param_default() {}
pub struct V69<const N: usize = 4096>;
#[cfg(not(test))]
pub fn v70_cfg_not_test(x: u64) -> u64 { x * 4096 }
pub fn v71_cfg_on_statement() -> u64 {
    #[cfg(test)]
    let x = 4096;
    #[cfg(not(test))]
    let x = 8192;
    x
}
"#,
    ),
    (
        "crates/victim/src/more.rs",
        r#"//! More attacks.
/// Same name as abi::PAGE_SIZE, value hidden behind size_of: never evaluated, never matched.
pub const PAGE_SIZE: u64 = core::mem::size_of::<[u8; 4096]>() as u64;
/// Same name and value as abi::SEPARATOR (b'/'), written as an integer.
pub mod sep { pub const SEPARATOR: u8 = 47; }
/// Same name and value as abi::POLL_INTERVAL_MS, written through Duration.
pub const POLL_INTERVAL_MS: u128 = core::time::Duration::from_millis(250).as_millis();

macro_rules! error { ($name:expr) => { crate::more::errno_named($name) }; }
pub fn errno_named(_: &str) -> i32 { 0 }
/// A user macro that happens to be called `error!`: its string is a value, skipped as a message.
pub fn v73_user_macro_named_like_log() -> i32 { error!("ENOENT") }
pub fn v74_expect_value(v: Option<u64>) -> u64 { v.expect("4096") }
pub fn v75_unwrap_or(v: Option<u64>) -> u64 { v.unwrap_or(4096) }
pub fn v76_assert_comparison(x: u64) { assert!(x <= 4096, "too big"); }
pub fn v77_sleep_millis() { std::thread::sleep(std::time::Duration::from_millis(1)); }
pub fn v78_string_from_number() -> String { 4096.to_string() }
pub fn v79_shift_assign(mut x: u64) -> u64 { x >>= 12; x }
pub fn v80_compound_mask(mut x: u64) -> u64 { x &= !0xfff; x }
"#,
    ),
    (
        "crates/victim/src/protocol/mod.rs",
        r#"//! Wire protocol.
pub mod tests;
"#,
    ),
    (
        "crates/victim/src/protocol/tests.rs",
        r#"//! Production module that happens to be named tests (a protocol's self-tests sent to the guest).
pub fn v72_module_file_named_tests(x: u64) -> u64 { x >> 12 }
"#,
    ),
    (
        "crates/victim/src/target/mod.rs",
        r#"//! The compilation target (a folder name that build tools also use).
pub fn v35_folder_named_target(x: u64) -> u64 { x >> 12 }
"#,
    ),
    (
        "crates/victim/src/tests.rs",
        r#"//! Unit tests in their own file, reached through `#[cfg(test)] mod tests;` in lib.rs.
use super::*;
fn l21_test_helper_data() -> Vec<u64> { vec![3, 5, 7] }
#[test]
fn l22_test_data_in_test_fn() { assert_eq!(super::v01_shift_by_12(8192), 2); }
"#,
    ),
    (
        "crates/victim/tests/page.rs",
        r#"//! Integration test that retypes the page size instead of importing abi::PAGE_SIZE.
#[test]
fn v18_test_retypes_page_size() {
    assert_eq!(victim::v01_shift_by_12(3 * 4096), 3);
    assert_eq!(victim::v03_one_shl_12(), 4096);
}
const PAGE_SIZE: u64 = 4096;
"#,
    ),
];

fn rows() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| list(&workspace("attack", FILES)))
}

/// The item holds a finding in a gated list (or in `list` when given).
fn caught(item: &str, wanted: Option<&str>) {
    let found = in_item(rows(), item);
    let hit = found
        .iter()
        .any(|r| r.gated() && wanted.is_none_or(|w| r.list == w));
    assert!(
        hit,
        "{item} was not caught{}; its findings: {found:#?}",
        wanted
            .map(|w| format!(" in the {w} list"))
            .unwrap_or_default()
    );
}

/// The item holds no gated finding.
fn passes(item: &str) {
    let gated: Vec<&Row> = in_item(rows(), item)
        .into_iter()
        .filter(|r| r.gated())
        .collect();
    assert!(gated.is_empty(), "{item} was wrongly caught: {gated:#?}");
}

macro_rules! caught {
    ($($name:ident $(in $list:literal)?),* $(,)?) => { $(#[test] #[allow(non_snake_case)] fn $name() { caught(stringify!($name), None $(.or(Some($list)))?); })* };
}
macro_rules! passes {
    ($($name:ident),* $(,)?) => { $(#[test] #[allow(non_snake_case)] fn $name() { passes(stringify!($name)); })* };
}

caught! {
    v01_shift_by_12, v02_mask_0xfff, v03_one_shl_12, v04_inside_vec_macro, v05_match_arm, v06_array_length_in_let,
    v07_const_generic, v08_closure, v09_inline_const_block, v10_local_const_other_name in "restated",
    v11_parsed_string, v12_char, v13_byte, v14_float, v15_cast, v16_duration, v17_path_string,
    v18_test_retypes_page_size in "restated", v19_write_macro_hides_value, v20_format_hides_path,
    v21_let_stdout_fd in "role", v22_root_uid_comparison in "role", v23_fd_argument in "role",
    v24_impl_in_const_block, V25_HANDLER, v26_production_under_any_test, v30_page, V31Aligned, v32_include,
    v34_generated_marker_in_a_doc, v35_folder_named_target, v36_folder_named_examples, v38_ok_or_hides_path,
    v39_assert_eq_hides_value, v41_unknown_macro_tokens, v42_min_counting, v43_elf_magic, v44_hex_string,
    v45_size_of_disguise, PAGE_SIZE_DISGUISED in "restated", v46_struct_field, v47_wrapping_shl, v48_round_up,
    v49_let_tuple, v50_negative_errno, v51_exit_status in "role", v53_assign_root in "role", v54_concat,
    v55_env_name, v56_context_hides_path, v57_err_hides_value, v58_io_error_hides_path, v59_index_with_meaning,
    v60_index_one_meaning in "role", v62_long_string, v63_debug_assert_eq_message_value, v64_tracing_like,
    v65_byte_array_literal, v66_let_else, v67_async_block, V68Trait, V69, v70_cfg_not_test, v71_cfg_on_statement,
    v72_module_file_named_tests, v73_user_macro_named_like_log, v74_expect_value, v75_unwrap_or,
    v76_assert_comparison, v78_string_from_number, v79_shift_assign, v80_compound_mask,
    EINVAL in "redefinition", SEPARATOR in "redefinition", POLL_INTERVAL_MS in "redefinition",
}

/// The same name and value as another constant: `Foo::PAGE_SIZE`, `X86::PAGE_SIZE` and more.rs's.
#[test]
fn page_size_copies() {
    let copies: Vec<&Row> = rows()
        .iter()
        .filter(|r| r.list == "redefinition" && r.literal.contains("PAGE_SIZE = 4096"))
        .collect();
    let paths: Vec<String> = copies
        .iter()
        .map(|r| format!("{}:{}", r.path, r.line))
        .collect();
    for wanted in [
        "crates/victim/src/lib.rs:65",
        "crates/victim/src/lib.rs:69",
        "crates/victim/src/more.rs:3",
        "crates/victim/tests/page.rs:7",
    ] {
        assert!(
            paths.iter().any(|p| p == wanted),
            "{wanted} not reported as a copy: {paths:?}"
        );
    }
}

/// The owner of a fact is never reported as its copy.
#[test]
fn owners_pass() {
    let abi: Vec<&Row> = rows()
        .iter()
        .filter(|r| r.path.starts_with("crates/abi/") && r.gated())
        .collect();
    assert!(abi.is_empty(), "{abi:#?}");
}

passes! {
    l01_next, l02_first, l03_size_of, l04_offset_of, l05_trailing_zeros, l06_loop, l07_checked, l08_repeat,
    l09_bool, l10_empty, l11_message, l12_err, l13_expect, l14_div_ceil, l15_shift_named,
    l16_custom_error_message, l17_panic, l18_midpoint, l19_is_empty, l20_bits, l21_test_helper_data,
    l22_test_data_in_test_fn, MY_LIMIT, PAGE_MASK, tests_not_miri,
}

/// Still gets through (docs/literals.md, "What the check cannot see"): a 1 or 0 with no role in sight
/// (`Duration::from_secs(1)`, `return 0`, `!0`, a 1 in a `macro_rules!` body), a small value named
/// differently from its owner (`Signal::Segv = 11`), and a linux-raw-sys value retyped under another name
/// in a crate that does not depend on linux-raw-sys (`OPEN_CLOEXEC`).
mod gets_through {
    use super::passes;

    passes! { v30_stdout, v37_poll_one_second, v40_not_zero_mask, v52_return_zero_success, v61_float_one, v77_sleep_millis, OPEN_CLOEXEC, Signal }
}
