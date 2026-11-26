//! Checks that cover several accesses at once (a stack frame, a group through one base) failing, also on the
//! first instruction of a function: the same result as Linux, and at full the same registers and memory as at
//! none. The programs are each guest's own (`part::checks`).

use wcontainers_translator::{LookAhead, Omit, Optimise};

#[test]
fn checks_match_linux_arm64() {
    arm64::checks_match_linux();
}

#[test]
fn failing_at_an_entry_in_chrome_arm64() {
    arm64::failing_at_an_entry_in_chrome();
}

#[test]
fn checks_match_linux_x86_64() {
    x86_64::checks_match_linux();
}

#[test]
fn failing_at_an_entry_in_chrome_x86_64() {
    x86_64::failing_at_an_entry_in_chrome();
}

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
mod arm64 {
    use super::*;
    pub(super) use wcontainers_translator_test::arm64::browser;
    pub(super) use wcontainers_translator_test::arm64::guest::{Check, Counted};
    use wcontainers_translator_test::arm64::part;
    pub(super) use wcontainers_translator_test::arm64::programs::{
        Blob, native, reachable_memory, translated, translated_omitting,
    };
    pub(super) use wcontainers_translator_test::arm64::runner::Machine;

    include!("for_guest/checks.rs");
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    pub(super) use wcontainers_translator_test::x86_64::browser;
    pub(super) use wcontainers_translator_test::x86_64::guest::{Check, Counted};
    use wcontainers_translator_test::x86_64::part;
    pub(super) use wcontainers_translator_test::x86_64::programs::{
        Blob, native, reachable_memory, translated, translated_omitting,
    };
    pub(super) use wcontainers_translator_test::x86_64::runner::Machine;

    include!("for_guest/checks.rs");
}
