//! The runner and look-ahead none on a few hand-made programs, each guest's own (`part::smoke`).

use wcontainers_abi::{Exit, Permissions};
use wcontainers_translator::LookAhead;
use wcontainers_translator::contract::Reason;

const LOAD: u64 = 0x10000;
/// The one executable page the programs run in, and the stack's page below it.
const PAGE: u64 = wcontainers_abi::PAGE_SIZE;

#[test]
fn exit_status_arm64() {
    arm64::exit_status();
}

#[test]
fn loop_and_call_arm64() {
    arm64::loop_and_call();
}

#[test]
fn a_raised_stop_flag_is_cleared_and_the_thread_carries_on_arm64() {
    arm64::a_raised_stop_flag_is_cleared_and_the_thread_carries_on();
}

#[test]
fn slow_paths_are_counted_and_warned_once_per_place_arm64() {
    arm64::slow_paths_are_counted_and_warned_once_per_place();
}

#[test]
fn stop_flag_raised_while_running_arm64() {
    arm64::stop_flag_raised_while_running();
}

#[test]
fn stop_raised_while_running_with_the_count_kept_arm64() {
    arm64::stop_raised_while_running_with_the_count_kept();
}

#[test]
fn instructions_not_offered_are_illegal_and_counted_arm64() {
    arm64::instructions_not_offered_are_illegal_and_counted();
}

#[test]
fn exit_status_x86_64() {
    x86_64::exit_status();
}

#[test]
fn loop_and_call_x86_64() {
    x86_64::loop_and_call();
}

#[test]
fn a_raised_stop_flag_is_cleared_and_the_thread_carries_on_x86_64() {
    x86_64::a_raised_stop_flag_is_cleared_and_the_thread_carries_on();
}

#[test]
fn slow_paths_are_counted_and_warned_once_per_place_x86_64() {
    x86_64::slow_paths_are_counted_and_warned_once_per_place();
}

#[test]
fn stop_flag_raised_while_running_x86_64() {
    x86_64::stop_flag_raised_while_running();
}

#[test]
fn stop_raised_while_running_with_the_count_kept_x86_64() {
    x86_64::stop_raised_while_running_with_the_count_kept();
}

#[test]
fn instructions_not_offered_are_illegal_and_counted_x86_64() {
    x86_64::instructions_not_offered_are_illegal_and_counted();
}

/// The tests with ARM64 as the guest, those written once for both included from `for_guest`.
mod arm64 {
    use super::*;
    use wcontainers_guest_cpu::Guest as _;
    use wcontainers_guest_cpu::Personality as _;
    pub(super) use wcontainers_translator_test::arm64::guest::UNKNOWN_SYSTEM_CALL;
    pub(super) use wcontainers_translator_test::arm64::runner::{
        Machine, Stopped, Syscalls, fields,
    };
    use wcontainers_translator_test::arm64::{Cpu, part};

    include!("for_guest/smoke.rs");
}

/// The tests with x86-64 as the guest, those written once for both included from `for_guest`.
mod x86_64 {
    use super::*;
    use wcontainers_guest_cpu::Guest as _;
    use wcontainers_guest_cpu::Personality as _;
    pub(super) use wcontainers_translator_test::x86_64::guest::UNKNOWN_SYSTEM_CALL;
    pub(super) use wcontainers_translator_test::x86_64::runner::{
        Machine, Stopped, Syscalls, fields,
    };
    use wcontainers_translator_test::x86_64::{Cpu, part};

    include!("for_guest/smoke.rs");
}
