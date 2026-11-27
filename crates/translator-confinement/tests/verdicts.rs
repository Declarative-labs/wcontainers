//! Hand-written sequences with the verdict the confinement statement gives them, the words encoded here from Arm's
//! encoding tables (Arm ARM, "A64 instruction set encoding"), not by the translator's encoder.

use wcontainers_translator_confinement::{
    Breach, FLOOR_ENTRY_BYTES, FLOOR_PAGE, Found, PROCESS_SLOTS, Place, State, check,
};

/// Slot 3's window, a frame far from it, and the instructions in the code area.
const PLACE: Place = Place {
    base: 3 << 33,
    frame: 0x7fff_0000_1000,
    code: (1 << 45) + (1 << 34),
    below: 0,
    displaced: false,
};

const X23: u32 = 23;
const X24: u32 = 24;
const X28: u32 = 28;
const SP: u32 = 31;
const ZR: u32 = 31;

fn ldr(rt: u32, rn: u32, offset: u32) -> u32 {
    0xf940_0000 | (offset / 8) << 10 | rn << 5 | rt
}
fn ldr_w(rt: u32, rn: u32, offset: u32) -> u32 {
    0xb940_0000 | (offset / 4) << 10 | rn << 5 | rt
}
fn ldrb(rt: u32, rn: u32, offset: u32) -> u32 {
    0x3940_0000 | offset << 10 | rn << 5 | rt
}
fn str_x(rt: u32, rn: u32, offset: u32) -> u32 {
    0xf900_0000 | (offset / 8) << 10 | rn << 5 | rt
}
/// `ldr xt, [xn, wm, uxtw #shift]`, shift 0 or 3.
fn ldr_uxtw(rt: u32, rn: u32, rm: u32, scaled: bool) -> u32 {
    0xf860_4800 | rm << 16 | u32::from(scaled) << 12 | rn << 5 | rt
}
/// `stp xt, xt2, [xn, #offset]!`.
fn stp_pre(rt: u32, rt2: u32, rn: u32, offset: i32) -> u32 {
    0xa980_0000 | (((offset / 8) as u32) & 0x7f) << 15 | rt2 << 10 | rn << 5 | rt
}
/// `add xd, xn|sp, wm, uxtw`.
fn add_uxtw(rd: u32, rn: u32, rm: u32) -> u32 {
    0x8b20_4000 | rm << 16 | rn << 5 | rd
}
/// `mov xd, xm`.
fn mov(rd: u32, rm: u32) -> u32 {
    0xaa00_03e0 | rm << 16 | rd
}
fn movz(rd: u32, imm: u32, hw: u32) -> u32 {
    0xd280_0000 | hw << 21 | imm << 5 | rd
}
fn movk(rd: u32, imm: u32, hw: u32) -> u32 {
    0xf280_0000 | hw << 21 | imm << 5 | rd
}
/// `eor xd, xn, #(1 << bit)`: N 1, imms 0, immr rotating the one bit up to `bit`.
fn eor_bit(rd: u32, rn: u32, bit: u32) -> u32 {
    0xd240_0000 | ((64 - bit) % 64) << 16 | rn << 5 | rd
}
/// `ubfiz xd, xn, #lsb, #width`.
fn ubfiz(rd: u32, rn: u32, lsb: u32, width: u32) -> u32 {
    0xd340_0000 | ((64 - lsb) % 64) << 16 | (width - 1) << 10 | rn << 5 | rd
}
/// `bfxil xd, xn, #lsb, #width`.
fn bfxil(rd: u32, rn: u32, lsb: u32, width: u32) -> u32 {
    0xb340_0000 | lsb << 16 | (lsb + width - 1) << 10 | rn << 5 | rd
}
fn br(rn: u32) -> u32 {
    0xd61f_0000 | rn << 5
}
fn blr(rn: u32) -> u32 {
    0xd63f_0000 | rn << 5
}
fn b(by: i32) -> u32 {
    0x1400_0000 | ((by / 4) as u32 & 0x03ff_ffff)
}
const SVC: u32 = 0xd400_0001;
const UDF: u32 = 0;
const BRK: u32 = 0xd420_0000;
/// `msr tpidr2_el0, x0`, `msr tpidr_el0, x0`, `msr nzcv, x0`, `msr dit, #1`.
const MSR_TPIDR2: u32 = 0xd51b_d0a0;
const MSR_TPIDR: u32 = 0xd51b_d040;
const MSR_NZCV: u32 = 0xd51b_4200;
const MSR_DIT: u32 = 0xd503_415f;
/// `sub sp, sp, #0xf000`, `ldr wzr, [sp]`.
const SUB_SP_STEP: u32 = 0xd140_3fff;

fn bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// The breaches of the words entered at their first under the entry rule, ended by `udf`.
fn breaches_at(place: &Place, words: &[u32]) -> Vec<(usize, Breach)> {
    let mut words = words.to_vec();
    words.push(UDF);
    let (found, _) = check(&bytes(&words), place, &[(0, State::entry(place))]);
    found.iter().map(|f: &Found| (f.at, f.breach)).collect()
}

fn breaches(words: &[u32]) -> Vec<(usize, Breach)> {
    breaches_at(&PLACE, words)
}

#[test]
fn a_system_call_breaches() {
    assert_eq!(breaches(&[SVC]), vec![(0, Breach::SystemCall)]);
}

#[test]
fn a_load_through_an_unreserved_register_breaches() {
    assert_eq!(breaches(&[ldr(0, 1, 0)]), vec![(0, Breach::AccessOutside)]);
}

#[test]
fn a_write_of_the_frame_register_breaches() {
    assert_eq!(breaches(&[mov(X28, 0)]), vec![(0, Breach::FrameChanged)]);
}

#[test]
fn an_access_through_an_overwritten_base_breaches() {
    assert_eq!(
        breaches(&[mov(X24, 0), add_uxtw(X23, X24, 1), ldr(0, X23, 0)]),
        vec![(8, Breach::AccessOutside)]
    );
}

#[test]
fn a_register_outside_the_list_breaches() {
    assert_eq!(
        breaches(&[MSR_TPIDR2]),
        vec![(0, Breach::ForbiddenRegister)]
    );
    assert_eq!(breaches(&[MSR_DIT]), vec![(0, Breach::ForbiddenRegister)]);
    assert_eq!(breaches(&[MSR_TPIDR, MSR_NZCV]), vec![]);
}

#[test]
fn a_branch_out_of_the_sequence_breaches() {
    assert_eq!(breaches(&[b(8)]), vec![(0, Breach::BranchOutside)]);
    assert_eq!(breaches(&[b(-4)]), vec![(0, Breach::BranchOutside)]);
    assert_eq!(breaches(&[br(0)]), vec![(0, Breach::BranchOutside)]);
}

#[test]
fn a_branch_inside_the_sequence_is_followed() {
    // Over the system call to the end.
    assert_eq!(breaches(&[b(8), SVC]), vec![]);
}

#[test]
fn going_on_past_the_end_breaches() {
    let (found, _) = check(&bytes(&[mov(0, 1)]), &PLACE, &[(0, State::entry(&PLACE))]);
    assert_eq!(
        found.iter().map(|f| f.breach).collect::<Vec<_>>(),
        [Breach::FallsOffEnd]
    );
}

#[test]
fn a_call_into_the_floor_is_an_exit() {
    let entry = FLOOR_PAGE + FLOOR_ENTRY_BYTES;
    let words = [
        movz(30, (entry & 0xffff) as u32, 0),
        movk(30, (entry >> 32 & 0xffff) as u32, 2),
        blr(30),
        SVC,
    ];
    assert_eq!(entry >> 16 & 0xffff, 0);
    assert_eq!(breaches(&words), vec![]);
    // A call between two entries is not one.
    let words = [movz(30, 8, 0), movk(30, (entry >> 32) as u32, 2), blr(30)];
    assert_eq!(breaches(&words), vec![(8, Breach::BranchOutside)]);
}

#[test]
fn a_jump_through_its_slot_is_an_exit() {
    let words = [
        add_uxtw(X23, X24, 0),
        ubfiz(X23, X23, 2, 44),
        bfxil(X23, X23, 2, 2),
        eor_bit(X23, X23, PROCESS_SLOTS.trailing_zeros()),
        br(X23),
    ];
    assert_eq!(breaches(&words), vec![]);
}

#[test]
fn a_window_access_cut_by_an_add_is_inside() {
    assert_eq!(breaches(&[add_uxtw(X23, X24, 0), ldr(1, X23, 8)]), vec![]);
    assert_eq!(
        breaches(&[add_uxtw(X23, X24, 0), str_x(1, X23, 32760)]),
        vec![]
    );
}

#[test]
fn a_window_access_cut_by_its_own_addressing_is_inside_unless_scaled() {
    assert_eq!(breaches(&[ldr_uxtw(1, X24, 0, false)]), vec![]);
    assert_eq!(
        breaches(&[ldr_uxtw(1, X24, 0, true)]),
        vec![(0, Breach::AccessOutside)]
    );
}

#[test]
fn the_frame_s_words_are_inside_and_past_them_is_not() {
    assert_eq!(breaches(&[ldr(X23, X28, 248)]), vec![]);
    assert_eq!(
        breaches(&[ldr(X23, X28, 256)]),
        vec![(0, Breach::AccessOutside)]
    );
}

#[test]
fn the_mark_is_inside_the_slot() {
    assert_eq!(
        breaches(&[eor_bit(15, X24, 32), ldrb(ZR, 15, 4095)]),
        vec![]
    );
}

#[test]
fn a_stack_access_reaches_below_the_window_unless_that_is_reserved() {
    assert_eq!(breaches(&[ldr(0, SP, 16)]), vec![]);
    let push = [stp_pre(0, 1, SP, -16)];
    assert_eq!(breaches(&push), vec![(0, Breach::AccessOutside)]);
    let reserved = Place {
        below: 1 << 32,
        ..PLACE
    };
    assert_eq!(breaches_at(&reserved, &push), vec![]);
}

#[test]
fn a_stack_step_s_probe_reaches_below_the_window_unless_that_is_reserved() {
    let step = [SUB_SP_STEP, ldr_w(ZR, SP, 0), ldr(0, SP, 8)];
    assert_eq!(breaches(&step), vec![(4, Breach::AccessOutside)]);
    // Reserved, the probe that did not fault puts the stack pointer back in the window.
    let reserved = Place {
        below: 1 << 32,
        ..PLACE
    };
    assert_eq!(breaches_at(&reserved, &step), vec![]);
}

#[test]
fn a_new_stack_pointer_cut_to_the_window_is_inside() {
    assert_eq!(breaches(&[add_uxtw(SP, X24, 15), ldr(0, SP, 0)]), vec![]);
    // A stack pointer from anything is not.
    assert_eq!(
        breaches(&[0x9100_001f, ldr(0, SP, 0)]),
        vec![(4, Breach::AccessOutside)]
    );
}

#[test]
fn a_raised_signal_ends_the_run() {
    assert_eq!(breaches(&[BRK, SVC]), vec![]);
}

#[test]
fn without_the_entry_rule_the_page_and_the_window_are_unknown() {
    let words = bytes(&[ldr(X23, X28, 8), UDF]);
    let (found, _) = check(&words, &PLACE, &[(0, State::anything())]);
    assert_eq!(
        found.iter().map(|f| f.breach).collect::<Vec<_>>(),
        [Breach::AccessOutside]
    );
}

#[test]
fn entered_past_the_cut_the_access_is_unknown() {
    let words = bytes(&[add_uxtw(X23, X24, 0), ldr(1, X23, 0), UDF]);
    let (found, _) = check(&words, &PLACE, &[(4, State::entry(&PLACE))]);
    assert_eq!(
        found.iter().map(|f| (f.at, f.breach)).collect::<Vec<_>>(),
        [(4, Breach::AccessOutside)]
    );
}
