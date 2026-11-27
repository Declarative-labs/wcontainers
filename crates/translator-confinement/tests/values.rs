//! What the checker knows each register holds after hand-written instructions, and how it reads each family of words:
//! every value from the Arm ARM's definition of the instruction, the words encoded here from its encoding tables.

use wcontainers_translator_confinement::{
    Breach, FLOOR_ENTRY_BYTES, FLOOR_PAGE, Found, Made, Offset, PROCESS_SLOTS, Place, SLOT_BYTES,
    State, Step, Value, check, decode,
};

const BASE: u64 = 3 << 33;
const FRAME: u64 = 0x7fff_0000_1000;
const CODE: u64 = (1 << 45) + (1 << 34);
const PLACE: Place = Place {
    base: BASE,
    frame: FRAME,
    code: CODE,
    below: 0,
    displaced: false,
};
const UDF: u32 = 0;
const WINDOW_END: u64 = BASE + (1 << 32) - 1;

fn movz(rd: u32, imm: u32, hw: u32) -> u32 {
    0xd280_0000 | hw << 21 | imm << 5 | rd
}
fn movk(rd: u32, imm: u32, hw: u32) -> u32 {
    0xf280_0000 | hw << 21 | imm << 5 | rd
}
fn add_imm(rd: u32, rn: u32, imm: u32, shifted: bool) -> u32 {
    0x9100_0000 | u32::from(shifted) << 22 | imm << 10 | rn << 5 | rd
}
fn sub_imm(rd: u32, rn: u32, imm: u32) -> u32 {
    0xd100_0000 | imm << 10 | rn << 5 | rd
}
/// `add`/`sub xd, xn|sp, wm|xm, <option> #amount`.
fn add_ext(rd: u32, rn: u32, rm: u32, option: u32, amount: u32, subtract: bool) -> u32 {
    0x8b20_0000 | u32::from(subtract) << 30 | rm << 16 | option << 13 | amount << 10 | rn << 5 | rd
}
/// `add`/`sub xd, xn, xm, <shift> #amount`.
fn add_shift(rd: u32, rn: u32, rm: u32, shift: u32, amount: u32, subtract: bool) -> u32 {
    0x8b00_0000 | u32::from(subtract) << 30 | shift << 22 | rm << 16 | amount << 10 | rn << 5 | rd
}
fn orr_reg(rd: u32, rn: u32, rm: u32) -> u32 {
    0xaa00_0000 | rm << 16 | rn << 5 | rd
}
/// A 64-bit logical immediate: `opc` 0 and, 1 orr, 2 eor, 3 ands.
fn logical(opc: u32, rd: u32, rn: u32, n: u32, immr: u32, imms: u32) -> u32 {
    0x9200_0000 | opc << 29 | n << 22 | immr << 16 | imms << 10 | rn << 5 | rd
}
fn ubfm(rd: u32, rn: u32, immr: u32, imms: u32) -> u32 {
    0xd340_0000 | immr << 16 | imms << 10 | rn << 5 | rd
}
fn bfm(rd: u32, rn: u32, immr: u32, imms: u32) -> u32 {
    0xb340_0000 | immr << 16 | imms << 10 | rn << 5 | rd
}
fn adr(rd: u32, imm: i32, page: bool) -> u32 {
    let imm = imm as u32 & 0x1f_ffff;
    0x1000_0000 | u32::from(page) << 31 | (imm & 3) << 29 | (imm >> 2) << 5 | rd
}
fn br(rn: u32) -> u32 {
    0xd61f_0000 | rn << 5
}
fn blr(rn: u32) -> u32 {
    0xd63f_0000 | rn << 5
}

fn bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// The registers after the words, run from the entry rule's state with x0 to x3 as given, and their breaches.
fn run_from(start: &State, words: &[u32]) -> (Vec<Found>, Option<State>) {
    let mut all = words.to_vec();
    all.push(UDF);
    let (found, states) = check(&bytes(&all), &PLACE, &[(0, start.clone())]);
    (found, states.get(&(4 * words.len())).cloned())
}

fn with(values: &[(usize, Value)]) -> State {
    let mut state = State::entry(&PLACE);
    for &(r, v) in values {
        state.0[r] = v;
    }
    state
}

/// Register `r` after the words, which must breach nothing.
fn after(values: &[(usize, Value)], words: &[u32], r: usize) -> Value {
    let (found, state) = run_from(&with(values), words);
    assert_eq!(found, [], "{words:x?}");
    state.expect("the end is reached").0[r]
}

fn one(v: u64) -> Value {
    Some((v, v))
}

#[test]
fn wide_moves_build_constants() {
    assert_eq!(after(&[], &[movz(0, 0x1234, 1)], 0), one(0x1234_0000));
    assert_eq!(after(&[], &[0x9280_0000], 0), one(u64::MAX), "movn x0, #0");
    assert_eq!(
        after(&[], &[0x1280_0020], 0),
        one(0xffff_fffe),
        "movn w0, #1"
    );
    assert_eq!(
        after(&[], &[movz(0, 1, 0), movk(0, 2, 2)], 0),
        one(0x2_0000_0001)
    );
    assert_eq!(
        after(&[], &[movk(0, 2, 2)], 0),
        None,
        "movk keeps what is unknown"
    );
    assert_eq!(after(&[(0, Some((1, 2)))], &[movk(0, 2, 2)], 0), None);
    assert_eq!(
        after(&[], &[0xf2a0_0040], 0),
        None,
        "movk w0, #2, lsl #16 of an unknown"
    );
    assert_eq!(
        after(&[], &[0x72a0_0040], 0),
        Some((0, u32::MAX.into())),
        "movk w0"
    );
}

#[test]
fn adds_and_subtracts_of_immediates() {
    assert_eq!(
        after(&[(1, one(10))], &[add_imm(0, 1, 3, false)], 0),
        one(13)
    );
    assert_eq!(
        after(&[(1, one(10))], &[add_imm(0, 1, 1, true)], 0),
        one(4106)
    );
    assert_eq!(
        after(&[(1, Some((10, 20)))], &[sub_imm(0, 1, 3)], 0),
        Some((7, 17))
    );
    assert_eq!(
        after(&[(1, one(1))], &[sub_imm(0, 1, 3)], 0),
        one(u64::MAX - 1),
        "wraps"
    );
    assert_eq!(
        after(&[(1, Some((1, 5)))], &[sub_imm(0, 1, 3)], 0),
        None,
        "wraps part of the way"
    );
    assert_eq!(
        after(&[(1, one(10))], &[0x1100_0c20], 0),
        Some((0, u32::MAX.into())),
        "add w0, w1, #3"
    );
    // cmp x1, #3 writes no register; add sp, x1, #0 writes the stack pointer.
    assert_eq!(
        after(&[(1, one(10))], &[0xf100_0c3f], 31),
        Some((BASE, WINDOW_END))
    );
    assert_eq!(
        after(&[(1, one(10))], &[add_imm(31, 1, 0, false)], 31),
        one(10)
    );
}

#[test]
fn pc_relative_addresses() {
    assert_eq!(
        after(&[], &[UDF ^ 0xd503_201f, adr(0, 8, false)], 0),
        one(CODE + 4 + 8)
    );
    assert_eq!(after(&[], &[adr(0, -4, false)], 0), one(CODE - 4));
    assert_eq!(
        after(&[], &[adr(0, 2, true)], 0),
        one((CODE & !0xfff) + 0x2000)
    );
    assert_eq!(
        after(&[(0, one(9))], &[adr(31, 2, false)], 0),
        one(9),
        "adr xzr"
    );
}

#[test]
fn logical_immediates() {
    // and x0, x1, #0xff; and of an unknown; and of a range.
    assert_eq!(
        after(&[(1, one(0x1234))], &[logical(0, 0, 1, 1, 0, 7)], 0),
        one(0x34)
    );
    assert_eq!(after(&[], &[logical(0, 0, 1, 1, 0, 7)], 0), Some((0, 0xff)));
    assert_eq!(
        after(&[(1, Some((0x10, 0x20)))], &[logical(0, 0, 1, 1, 0, 7)], 0),
        Some((0, 0x20))
    );
    assert_eq!(
        after(&[(1, Some((0x10, 0x200)))], &[logical(0, 0, 1, 1, 0, 7)], 0),
        Some((0, 0xff))
    );
    // and x0, x1, #0xffff0000 (a rotated run): 16 ones at bit 16.
    assert_eq!(
        after(&[(1, one(0x1234_5678))], &[logical(0, 0, 1, 1, 48, 15)], 0),
        one(0x1234_0000)
    );
    // and x0, x1, #0x5555...: two-bit elements.
    assert_eq!(
        after(
            &[(1, one(u64::MAX))],
            &[logical(0, 0, 1, 0, 0, 0b11_1100)],
            0
        ),
        one(0x5555_5555_5555_5555)
    );
    // orr is not followed; eor of one bit is, where every value has it alike.
    assert_eq!(
        after(&[(0, one(1))], &[logical(1, 0, 31, 1, 0, 7)], 0),
        None
    );
    assert_eq!(
        after(&[], &[logical(2, 0, 24, 1, 32, 0)], 0),
        one(BASE + (1 << 32))
    );
    assert_eq!(
        after(&[(1, Some((4, 7)))], &[logical(2, 0, 1, 1, 62, 0)], 0),
        Some((0, 3)),
        "bit 2 cleared"
    );
    assert_eq!(
        after(&[(1, Some((0, 3)))], &[logical(2, 0, 1, 1, 62, 0)], 0),
        Some((4, 7)),
        "bit 2 set"
    );
    assert_eq!(
        after(&[(1, Some((2, 5)))], &[logical(2, 0, 1, 1, 62, 0)], 0),
        None,
        "bit 2 differs"
    );
    assert_eq!(
        after(&[(1, Some((0, 3)))], &[logical(2, 0, 1, 1, 0, 1)], 0),
        None,
        "two bits"
    );
    assert_eq!(
        after(&[(1, Some((0, 1 << 63)))], &[logical(2, 0, 1, 1, 1, 0)], 0),
        None,
        "bit 63"
    );
    assert_eq!(
        after(
            &[(1, Some((1 << 63, u64::MAX)))],
            &[logical(2, 0, 1, 1, 1, 0)],
            0
        ),
        None,
        "bit 63 set"
    );
    assert_eq!(
        after(&[], &[logical(2, 0, 1, 1, 32, 0)], 0),
        None,
        "eor of an unknown"
    );
    // ands writes the flags and xzr; and with sp as destination writes the stack pointer.
    assert_eq!(
        after(&[(1, one(3))], &[logical(3, 31, 1, 1, 0, 7)], 31),
        Some((BASE, WINDOW_END))
    );
    assert_eq!(
        after(&[(1, one(0x18))], &[logical(0, 31, 1, 1, 0, 7)], 31),
        one(0x18)
    );
    assert_eq!(
        after(&[(1, one(3))], &[0x1200_1c20], 0),
        Some((0, u32::MAX.into())),
        "and w0, w1, #0xff"
    );
}

#[test]
fn reserved_immediates_are_unknown() {
    // A logical immediate of all ones, N set in a 32-bit form, an element of no size.
    assert_eq!(decode(logical(0, 0, 1, 1, 0, 63)), Step::Unknown);
    assert_eq!(decode(0x1240_0020), Step::Unknown);
    assert_eq!(decode(logical(0, 0, 1, 0, 0, 0b11_1111)), Step::Unknown);
    // movz with hw 2 in a 32-bit form; the reserved wide move opc.
    assert_eq!(decode(0x52c0_0000), Step::Unknown);
    assert_eq!(decode(0xb280_0000), Step::Unknown, "opc 01");
    // A bitfield with N clear in a 64-bit form, and opc 11.
    assert_eq!(decode(0xd300_0000), Step::Unknown);
    assert_eq!(decode(0xf340_0000), Step::Unknown);
}

#[test]
fn bitfields() {
    // ubfiz x0, x1, #2, #44, of a range under 2^44 and of one past it.
    assert_eq!(
        after(&[(1, Some((8, 16)))], &[ubfm(0, 1, 62, 43)], 0),
        Some((32, 64))
    );
    assert_eq!(
        after(
            &[(1, Some((1 << 44, (1 << 44) + 1)))],
            &[ubfm(0, 1, 62, 43)],
            0
        ),
        Some((0, 4))
    );
    assert_eq!(
        after(&[(1, Some((0, 1 << 44)))], &[ubfm(0, 1, 62, 43)], 0),
        Some((0, ((1 << 44) - 1) << 2))
    );
    assert_eq!(
        after(&[], &[ubfm(0, 1, 62, 43)], 0),
        Some((0, ((1 << 44) - 1) << 2))
    );
    // lsl x0, x1, #4.
    assert_eq!(after(&[(1, one(3))], &[ubfm(0, 1, 60, 59)], 0), one(48));
    // ubfx is not followed.
    assert_eq!(after(&[(1, one(3))], &[ubfm(0, 1, 1, 2)], 0), None);
    // bfxil x0, x1, #2, #2: the low two bits replaced.
    assert_eq!(
        after(&[(0, Some((16, 33)))], &[bfm(0, 1, 2, 3)], 0),
        Some((16, 35))
    );
    assert_eq!(after(&[], &[bfm(0, 1, 2, 3)], 0), None);
    // bfi and sbfm are not followed; a 32-bit bitfield is a word.
    assert_eq!(after(&[(0, one(1))], &[bfm(0, 1, 60, 3)], 0), None);
    assert_eq!(
        after(&[(0, one(1))], &[0x9340_fc20], 0),
        None,
        "asr x0, x1, #0"
    );
    assert_eq!(
        after(&[(1, one(3))], &[0x5300_0820], 0),
        Some((0, u32::MAX.into())),
        "ubfx w0"
    );
    // extr is unknown, a word when 32-bit.
    assert_eq!(after(&[(0, one(1))], &[0x93c1_0020], 0), None);
    assert_eq!(
        after(&[(0, one(1))], &[0x1381_0020], 0),
        Some((0, u32::MAX.into()))
    );
}

#[test]
fn extended_and_shifted_adds() {
    // add x0, x24, w1, uxtw: the window.
    assert_eq!(
        after(&[], &[add_ext(0, 24, 1, 0b010, 0, false)], 0),
        Some((BASE, WINDOW_END))
    );
    assert_eq!(
        after(&[(1, one(5))], &[add_ext(0, 24, 1, 0b010, 2, false)], 0),
        one(BASE + 20)
    );
    assert_eq!(
        after(
            &[(1, one(0x1_0000_0005))],
            &[add_ext(0, 24, 1, 0b010, 0, false)],
            0
        ),
        one(BASE + 5)
    );
    assert_eq!(
        after(
            &[(1, Some((0xffff_ffff, 0x1_0000_0000)))],
            &[add_ext(0, 24, 1, 0b010, 0, false)],
            0
        ),
        Some((BASE, WINDOW_END))
    );
    // uxtb, uxtx, sxtw.
    assert_eq!(
        after(&[], &[add_ext(0, 24, 1, 0b000, 0, false)], 0),
        Some((BASE, BASE + 255))
    );
    assert_eq!(
        after(&[(1, one(7))], &[add_ext(0, 24, 1, 0b011, 1, false)], 0),
        one(BASE + 14)
    );
    assert_eq!(after(&[], &[add_ext(0, 24, 1, 0b011, 0, false)], 0), None);
    assert_eq!(
        after(
            &[(1, Some((1, 2)))],
            &[add_ext(0, 24, 1, 0b110, 0, false)],
            0
        ),
        Some((BASE + 1, BASE + 2))
    );
    assert_eq!(
        after(
            &[(1, one(0x8000_0000))],
            &[add_ext(0, 24, 1, 0b110, 0, false)],
            0
        ),
        None
    );
    assert_eq!(after(&[], &[add_ext(0, 24, 1, 0b110, 0, false)], 0), None);
    // sub x0, sp, x24: the guest's stack pointer.
    assert_eq!(
        after(&[], &[add_ext(0, 31, 24, 0b011, 0, true)], 0),
        Some((0, (1 << 32) - 1))
    );
    // An amount past 4 is reserved; adds writes xzr; a 32-bit add is a word; add sp, x24, w1, uxtw.
    assert_eq!(decode(add_ext(0, 24, 1, 0b010, 5, false)), Step::Unknown);
    assert_eq!(
        after(
            &[(0, one(1))],
            &[add_ext(31, 24, 1, 0b010, 0, false) | 1 << 29],
            0
        ),
        one(1)
    );
    assert_eq!(
        after(&[], &[0x0b21_4300], 0),
        Some((0, u32::MAX.into())),
        "add w0, w24, w1, uxtw"
    );
    assert_eq!(
        after(&[], &[add_ext(31, 24, 1, 0b010, 0, false)], 31),
        Some((BASE, WINDOW_END))
    );
    // add x0, x1, x2, lsl #2, and sub; a right shift is not followed; a 32-bit one is a word.
    let consts = [(1, one(100)), (2, one(3))];
    assert_eq!(
        after(&consts, &[add_shift(0, 1, 2, 0, 2, false)], 0),
        one(112)
    );
    assert_eq!(
        after(&consts, &[add_shift(0, 1, 2, 0, 0, true)], 0),
        one(97)
    );
    assert_eq!(after(&consts, &[add_shift(0, 1, 2, 1, 0, false)], 0), None);
    assert_eq!(
        after(&consts, &[0x0b02_0020], 0),
        Some((0, u32::MAX.into()))
    );
    // add xzr, x1, x2 may be a later form's stack pointer write.
    assert_eq!(
        after(&consts, &[add_shift(31, 1, 2, 0, 0, false)], 31),
        None
    );
    assert_eq!(
        after(&consts, &[add_shift(31, 1, 2, 0, 0, false) | 1 << 29], 31),
        Some((BASE, WINDOW_END))
    );
}

#[test]
fn moves_and_other_register_writes() {
    assert_eq!(after(&[(1, one(7))], &[orr_reg(0, 31, 1)], 0), one(7));
    assert_eq!(
        after(&[(1, one(7)), (2, one(1))], &[orr_reg(0, 2, 1)], 0),
        None,
        "orr is not a move"
    );
    assert_eq!(
        after(&[(1, one(7))], &[orr_reg(0, 31, 1) | 1 << 10], 0),
        None,
        "shifted"
    );
    assert_eq!(
        after(&[(1, one(7))], &[orr_reg(0, 31, 1) | 1 << 21], 0),
        None,
        "orn"
    );
    assert_eq!(
        after(&[(1, one(7))], &[orr_reg(0, 31, 1) | 1 << 22], 0),
        None,
        "lsr #0"
    );
    assert_eq!(
        after(&[(1, one(7))], &[0x2a01_03e0], 0),
        Some((0, u32::MAX.into())),
        "mov w0, w1"
    );
    assert_eq!(
        after(&[(1, one(7))], &[0x8a01_0020], 0),
        None,
        "and x0, x1, x1"
    );
    assert_eq!(after(&[(0, one(1))], &[0x9b02_0420], 0), None, "madd");
    assert_eq!(
        after(&[(0, one(1))], &[0x1b02_0420], 0),
        Some((0, u32::MAX.into())),
        "madd w0"
    );
    assert_eq!(after(&[(0, one(1))], &[0x9a82_0020], 0), None, "csel");
    assert_eq!(
        after(&[(8, one(1))], &[0xfa41_0828], 8),
        one(1),
        "ccmp writes only the flags"
    );
    assert_eq!(
        after(&[(0, one(1))], &[0xd53b_4200], 0),
        None,
        "mrs x0, nzcv"
    );
    assert_eq!(after(&[(0, one(1))], &[0xd53b_421f], 0), one(1), "mrs xzr");
    assert_eq!(
        after(&[(0, one(1))], &[0x9e66_0000], 0),
        None,
        "fmov x0, d0"
    );
    assert_eq!(
        after(&[(0, one(1))], &[0x9e78_0000], 0),
        None,
        "fcvtzs x0, d0"
    );
    assert_eq!(
        after(&[(0, one(1))], &[0x9e62_0000], 0),
        one(1),
        "scvtf d0, x0"
    );
    assert_eq!(
        after(&[(0, one(1))], &[0x9e67_0000], 0),
        one(1),
        "fmov d0, x0"
    );
    assert_eq!(
        after(&[(0, one(1))], &[0x9e58_0000], 0),
        None,
        "fcvtzs x0, d0, #64"
    );
    assert_eq!(
        after(&[(0, one(1))], &[0x9e42_0000], 0),
        one(1),
        "scvtf d0, x0, #64"
    );
    assert_eq!(
        after(&[(0, one(1))], &[0x0e04_3c00], 0),
        None,
        "umov w0, v0.s[0]"
    );
    assert_eq!(
        after(&[(0, one(1))], &[0x4e08_2c00], 0),
        None,
        "smov x0, v0.b[4]"
    );
    assert_eq!(
        after(&[(0, one(1))], &[0x4e08_1c00], 0),
        one(1),
        "ins v0.d[0], x0"
    );
    assert_eq!(
        after(&[(0, one(1))], &[0x4e20_8400], 0),
        one(1),
        "add v0.16b"
    );
    // A move into x28 breaches, also when it keeps the frame.
    let (found, _) = run_from(&with(&[(0, one(FRAME))]), &[orr_reg(28, 31, 0)]);
    assert_eq!(found, []);
    let (found, _) = run_from(&with(&[(0, one(1))]), &[0x9100_041c]);
    assert_eq!(
        found.iter().map(|f| f.breach).collect::<Vec<_>>(),
        [Breach::FrameChanged]
    );
}

#[test]
fn loads_give_the_range_of_what_they_load() {
    let cut = add_ext(23, 24, 1, 0b010, 0, false);
    assert_eq!(after(&[], &[cut, 0x3940_02e0], 0), Some((0, 255)), "ldrb");
    assert_eq!(
        after(&[], &[cut, 0x7940_02e0], 0),
        Some((0, 0xffff)),
        "ldrh"
    );
    assert_eq!(
        after(&[], &[cut, 0xb940_02e0], 0),
        Some((0, u32::MAX.into())),
        "ldr w"
    );
    assert_eq!(after(&[], &[cut, 0xf940_02e0], 0), None, "ldr x");
    assert_eq!(after(&[], &[cut, 0xb980_02e0], 0), None, "ldrsw");
    assert_eq!(after(&[], &[cut, 0x3980_02e0], 0), None, "ldrsb x");
    assert_eq!(
        after(&[(0, one(1))], &[cut, 0xf980_02e0], 0),
        one(1),
        "prfm"
    );
    assert_eq!(after(&[(0, one(1))], &[cut, 0xf900_02e0], 0), one(1), "str");
    assert_eq!(
        after(&[], &[cut, 0x2940_06e0], 1),
        Some((0, u32::MAX.into())),
        "ldp w0, w1"
    );
    assert_eq!(
        after(&[(1, one(1))], &[cut, 0xa940_06e0], 1),
        None,
        "ldp x0, x1"
    );
    assert_eq!(after(&[(1, one(1))], &[cut, 0x6940_06e0], 1), None, "ldpsw");
    assert_eq!(after(&[(1, one(1))], &[cut, 0x2900_06e0], 1), one(1), "stp");
    assert_eq!(
        after(&[(0, one(1))], &[cut, 0x3d40_02e0], 0),
        one(1),
        "ldr b0"
    );
    assert_eq!(
        after(&[], &[cut, 0x3860_4ae0], 0),
        Some((0, 255)),
        "ldrb w0, [x23, w0, uxtw]"
    );
    assert_eq!(
        after(&[], &[cut, 0x38e0_02e0], 0),
        Some((0, 255)),
        "ldaddalb w0, w0, [x23]"
    );
    assert_eq!(
        after(&[], &[cut, 0x38bf_c2e0], 0),
        Some((0, 255)),
        "ldaprb w0, [x23]"
    );
    assert_eq!(
        after(&[], &[cut, 0x3840_42e0], 0),
        Some((0, 255)),
        "ldurb w0, [x23, #4]"
    );
    assert_eq!(
        after(&[], &[cut, 0x1940_02e0], 0),
        Some((0, 255)),
        "ldapurb"
    );
    assert_eq!(
        after(&[(0, one(1))], &[cut, 0x1900_02e0], 0),
        one(1),
        "stlurb"
    );
    assert_eq!(after(&[], &[cut, 0xd940_02e0], 0), None, "ldapur x");
    assert_eq!(
        after(&[], &[cut, 0x9940_02e0], 0),
        Some((0, u32::MAX.into())),
        "ldapur w"
    );
    assert_eq!(after(&[], &[cut, 0xc85f_7ee0], 0), None, "ldxr");
    assert_eq!(
        after(&[(0, one(1))], &[cut, 0xc800_7ee1], 0),
        None,
        "stxr w0"
    );
    assert_eq!(
        after(&[(0, one(1))], &[cut, 0xc89f_fee0], 0),
        one(1),
        "stlr"
    );
    assert_eq!(after(&[], &[cut, 0xc8df_fee0], 0), None, "ldar");
    assert_eq!(
        after(&[(0, one(1))], &[cut, 0xc8a0_7ee1], 0),
        None,
        "cas x0"
    );
    assert_eq!(
        after(&[(0, one(1)), (1, one(1))], &[cut, 0x4820_7ee2], 1),
        None,
        "casp x0, x1"
    );
    assert_eq!(
        after(&[(1, one(1))], &[cut, 0xc87f_06e0], 1),
        None,
        "ldxp x0, x1"
    );
    assert_eq!(
        after(&[(0, one(1))], &[cut, 0xc820_06e2], 0),
        None,
        "stxp w0"
    );
    assert_eq!(
        after(&[], &[0x1800_0020], 0),
        Some((0, u32::MAX.into())),
        "ldr w0, .+4"
    );
    // The other literal forms, read as accesses of the right size with the right loads.
    let literal = |word: u32| match decode(word) {
        Step::Access(a) => (
            a.bytes,
            a.loads.len(),
            a.loads.first().map(|l| l.1),
            a.offset,
        ),
        other => panic!("{word:#x}: {other:?}"),
    };
    assert_eq!(
        literal(0x5800_0040),
        (8, 1, Some(Made::Unknown), Offset::Constant(8, 8)),
        "ldr x0, .+8"
    );
    assert_eq!(
        literal(0x9800_0040),
        (4, 1, Some(Made::Unknown), Offset::Constant(8, 8)),
        "ldrsw"
    );
    assert_eq!(
        literal(0xd800_0040),
        (1, 0, None, Offset::Constant(8, 8)),
        "prfm"
    );
    assert_eq!(
        literal(0x1c00_0040),
        (4, 0, None, Offset::Constant(8, 8)),
        "ldr s0"
    );
    assert_eq!(
        literal(0x5cff_ffe0),
        (8, 0, None, Offset::Constant(-4, -4)),
        "ldr d0, .-4"
    );
    assert_eq!(
        literal(0x9c00_0040),
        (16, 0, None, Offset::Constant(8, 8)),
        "ldr q0"
    );
}

#[test]
fn access_sizes_and_offsets() {
    let cut = add_ext(23, 24, 1, 0b010, 0, false);
    // Each reaches past the top of the slot by exactly one byte when the cut is the window's last byte.
    let last = (1, one(u32::MAX.into()));
    let fits = |word: u32| run_from(&with(&[last]), &[cut, word]).0.is_empty();
    // ldr x0, [x23, #32760]: 32760 + 8 bytes past the window's last byte stay in the slot.
    assert!(fits(0xf97f_fee0));
    // ldr q0, [x23, #65520]; ldp q0, q1, [x23, #1008]; ld4 {v0.16b-v3.16b}, [x23].
    assert!(fits(0x3dff_fee0));
    assert!(fits(0xad5f_86e0));
    assert!(fits(0x4c40_02e0));
    // From the window's base, a negative offset reaches below it.
    let first = (1, one(0));
    let below = |word: u32| run_from(&with(&[first]), &[cut, word]).0.len();
    assert_eq!(below(0xf85f_02e0), 1, "ldur x0, [x23, #-16]");
    assert_eq!(below(0xa97f_06e0), 1, "ldp x0, x1, [x23, #-16]");
    assert_eq!(below(0xad7f_06e0), 1, "ldp q0, q1, [x23, #-32]");
    assert_eq!(
        below(0xf840_06e0),
        0,
        "ldr x0, [x23], #0 (post-index reads at x23)"
    );
    assert_eq!(below(0xf85f_0ee0), 1, "ldr x0, [x23, #-16]!");
    assert_eq!(below(0xf8bf_02e0), 0, "ldadd x31? ldaddl x31, x0, [x23]");
    assert_eq!(below(0x0d40_02e0), 0, "ld1 {{v0.b}}[0], [x23]");
}

#[test]
fn written_back_bases_move() {
    let cut = add_ext(23, 24, 1, 0b010, 0, false);
    let base_at = |words: &[u32]| after(&[(1, one(0x100))], words, 23);
    assert_eq!(
        base_at(&[cut, 0xf841_06e0]),
        one(BASE + 0x110),
        "ldr x0, [x23], #16"
    );
    assert_eq!(
        base_at(&[cut, 0xf85f_0ee0]),
        one(BASE + 0xf0),
        "ldr x0, [x23, #-16]!"
    );
    assert_eq!(
        base_at(&[cut, 0xa9bf_06e0]),
        one(BASE + 0xf0),
        "stp x0, x1, [x23, #-16]!"
    );
    assert_eq!(
        base_at(&[cut, 0xa8c1_06e0]),
        one(BASE + 0x110),
        "ldp x0, x1, [x23], #16"
    );
    assert_eq!(
        base_at(&[cut, 0x4cdf_72e0]),
        one(BASE + 0x110),
        "ld1 {{v0.16b}}, [x23], #16"
    );
    assert_eq!(
        base_at(&[cut, 0x0cdf_72e0]),
        one(BASE + 0x108),
        "ld1 {{v0.8b}}, [x23], #8"
    );
    assert_eq!(
        base_at(&[cut, 0x4cdf_22e0]),
        one(BASE + 0x140),
        "ld1 {{v0.16b-v3.16b}}, [x23], #64"
    );
    assert_eq!(
        base_at(&[cut, 0x4cdf_62e0]),
        one(BASE + 0x130),
        "ld1 x3, #48"
    );
    assert_eq!(
        base_at(&[cut, 0x4cdf_a2e0]),
        one(BASE + 0x120),
        "ld1 x2, #32"
    );
    assert_eq!(base_at(&[cut, 0x4cdf_82e0]), one(BASE + 0x120), "ld2, #32");
    assert_eq!(base_at(&[cut, 0x4cdf_42e0]), one(BASE + 0x130), "ld3, #48");
    assert_eq!(base_at(&[cut, 0x4cdf_02e0]), one(BASE + 0x140), "ld4, #64");
    assert_eq!(
        after(&[(1, one(0x100)), (2, one(4))], &[cut, 0x4cc2_72e0], 23),
        one(BASE + 0x104),
        "ld1 post x2"
    );
    assert_eq!(
        base_at(&[cut, 0x0ddf_02e0]),
        one(BASE + 0x101),
        "ld1 {{v0.b}}[0], [x23], #1"
    );
    assert_eq!(
        base_at(&[cut, 0x0ddf_42e0]),
        one(BASE + 0x102),
        "ld1 {{v0.h}}[0], [x23], #2"
    );
    assert_eq!(
        base_at(&[cut, 0x0ddf_82e0]),
        one(BASE + 0x104),
        "ld1 {{v0.s}}[0], [x23], #4"
    );
    assert_eq!(
        base_at(&[cut, 0x0ddf_86e0]),
        one(BASE + 0x108),
        "ld1 {{v0.d}}[0], [x23], #8"
    );
    assert_eq!(
        base_at(&[cut, 0x0dff_e2e0]),
        one(BASE + 0x104),
        "ld4r {{v0.8b-v3.8b}}, [x23], #4"
    );
    assert_eq!(
        base_at(&[cut, 0x0dff_e6e0]),
        one(BASE + 0x108),
        "ld4r {{v0.4h-v3.4h}}, [x23], #8"
    );
    assert_eq!(
        base_at(&[cut, 0x0dff_22e0]),
        one(BASE + 0x104),
        "ld4 {{v0.b-v3.b}}[0], [x23], #4"
    );
    assert_eq!(
        base_at(&[cut, 0x0ddf_22e0]),
        one(BASE + 0x103),
        "ld3 {{v0.b-v2.b}}[0], [x23], #3"
    );
    assert_eq!(
        base_at(&[cut, 0x0dff_02e0]),
        one(BASE + 0x102),
        "ld2 {{v0.b-v1.b}}[0], [x23], #2"
    );
    assert_eq!(
        decode(0x4cdf_12e0),
        Step::Unknown,
        "a reserved structure opcode"
    );
}

#[test]
fn a_probe_that_did_not_fault_puts_its_base_in_the_window() {
    let reserved = Place {
        below: 1 << 16,
        ..PLACE
    };
    let words = [sub_imm(31, 31, 0x10), 0xb940_03ff, UDF];
    let (found, states) = check(&bytes(&words), &reserved, &[(0, State::entry(&reserved))]);
    assert_eq!(found, []);
    assert_eq!(states[&8].0[31], Some((BASE, WINDOW_END - 0x10)));
    // A probe at an offset bounds the base by it.
    let words = [sub_imm(31, 31, 0x100), 0xb940_43ff, UDF];
    let (_, states) = check(&bytes(&words), &reserved, &[(0, State::entry(&reserved))]);
    assert_eq!(states[&8].0[31], Some((BASE - 0x40, WINDOW_END - 0x100)));
    // No bound where the access was outside the slot's reservation.
    let words = [0xf940_0380, UDF];
    let (_, states) = check(&bytes(&words), &PLACE, &[(0, State::entry(&PLACE))]);
    assert_eq!(states[&4].0[28], one(FRAME));
}

#[test]
fn branches_and_jumps_go_where_the_arm_arm_says() {
    let reached = |words: &[u32]| -> Vec<usize> {
        let (found, states) = check(&bytes(words), &PLACE, &[(0, State::entry(&PLACE))]);
        assert_eq!(found, [], "{words:x?}");
        states.keys().copied().collect()
    };
    // b.eq .+8, cbz x0, .+8, tbz x0, #0, .+8: both ways.
    assert_eq!(reached(&[0x5400_0040, UDF, UDF]), [0, 4, 8]);
    assert_eq!(reached(&[0xb400_0040, UDF, UDF]), [0, 4, 8]);
    assert_eq!(reached(&[0x3600_0040, UDF, UDF]), [0, 4, 8]);
    assert_eq!(reached(&[0x1400_0002, UDF, UDF]), [0, 8]);
    // bl .+8 gives x30 the next address.
    let (_, states) = check(
        &bytes(&[0x9400_0002, UDF, UDF]),
        &PLACE,
        &[(0, State::entry(&PLACE))],
    );
    assert_eq!(states[&8].0[30], one(CODE + 4));
    // A jump to an address of the sequence; to an unaligned one, which faults; a table of branches.
    assert_eq!(reached(&[adr(23, 8, false), br(23), UDF]), [0, 4, 8]);
    assert_eq!(reached(&[adr(23, 9, false), br(23), UDF, UDF]), [0, 4]);
    let table = [
        add_ext(22, 24, 1, 0b010, 0, false),
        0x3940_02c0,
        adr(23, 12, false),
        add_ext(23, 23, 0, 0b011, 2, false),
        br(23),
    ];
    let mut words = table.to_vec();
    words.extend([UDF; 256]);
    assert_eq!(reached(&words).len(), 5 + 256);
    // blr x30 to the floor's last entry ends the run; past the floor's page it breaches.
    let last = FLOOR_PAGE + 4096 - FLOOR_ENTRY_BYTES;
    let call = |entry: u64| {
        let words = [
            movz(30, (entry & 0xffff) as u32, 0),
            movk(30, (entry >> 16 & 0xffff) as u32, 1),
            movk(30, (entry >> 32 & 0xffff) as u32, 2),
            blr(30),
            UDF,
        ];
        check(&bytes(&words), &PLACE, &[(0, State::entry(&PLACE))]).0
    };
    assert_eq!(call(last), []);
    assert_eq!(call(FLOOR_PAGE), []);
    assert_eq!(call(FLOOR_PAGE + 4096).len(), 1);
    assert_eq!(call(FLOOR_PAGE - FLOOR_ENTRY_BYTES).len(), 1);
    // The slot table's first and last byte; past it.
    let table = PROCESS_SLOTS + (BASE << 2);
    for (target, breaches) in [
        (table, 0),
        (table + (SLOT_BYTES << 2) - 1, 0),
        (table + (SLOT_BYTES << 2), 1),
        (table - 1, 1),
        (CODE + 20, 1),
        (CODE - 4, 1),
    ] {
        let words = [
            movz(23, (target & 0xffff) as u32, 0),
            movk(23, (target >> 16 & 0xffff) as u32, 1),
            movk(23, (target >> 32 & 0xffff) as u32, 2),
            br(23),
            UDF,
        ];
        let (found, _) = check(&bytes(&words), &PLACE, &[(0, State::entry(&PLACE))]);
        assert_eq!(found.len(), breaches, "{target:#x}");
    }
    // Displaced code's table is slot 0's.
    let displaced = Place {
        displaced: true,
        ..PLACE
    };
    let words = [movz(23, 0, 0), movk(23, 0x4000, 2), br(23), UDF];
    assert_eq!(
        check(&bytes(&words), &displaced, &[(0, State::entry(&displaced))]).0,
        []
    );
    assert_eq!(
        check(&bytes(&words), &PLACE, &[(0, State::entry(&PLACE))])
            .0
            .len(),
        1
    );
    // ret x23; blr through x0 unknown; a jump with a link to the sequence gives x30.
    assert_eq!(
        decode(0xd65f_02e0),
        Step::Jump {
            rn: 23,
            link: false
        }
    );
    assert_eq!(decode(blr(5)), Step::Jump { rn: 5, link: true });
    let (_, states) = check(
        &bytes(&[adr(0, 8, false), blr(0), UDF]),
        &PLACE,
        &[(0, State::entry(&PLACE))],
    );
    assert_eq!(states[&8].0[30], one(CODE + 8));
    // A conditional branch at the end goes on past it.
    let (found, _) = check(
        &bytes(&[UDF, 0x54ff_ffe0]),
        &PLACE,
        &[(4, State::entry(&PLACE))],
    );
    assert_eq!(
        found.iter().map(|f| f.breach).collect::<Vec<_>>(),
        [Breach::FallsOffEnd]
    );
}

#[test]
fn a_loop_ends_with_what_grew_unknown() {
    // add x0, x0, #1; b .-4: x0 grows each turn, x1 stays.
    let words = [
        movz(0, 0, 0),
        movz(1, 5, 0),
        add_imm(0, 0, 1, false),
        0x17ff_ffff,
    ];
    let (found, states) = check(&bytes(&words), &PLACE, &[(0, State::entry(&PLACE))]);
    assert_eq!(found, []);
    assert_eq!(states[&8].0[0], None);
    assert_eq!(states[&8].0[1], one(5));
    // Two ways joined: the range holding both.
    let words = [0xb400_0060, movz(1, 1, 0), 0x1400_0002, movz(1, 5, 0), UDF];
    let (_, states) = check(&bytes(&words), &PLACE, &[(0, State::entry(&PLACE))]);
    assert_eq!(states[&16].0[1], Some((1, 5)));
    let words = [0xb400_0060, movz(1, 1, 0), 0x1400_0002, 0xd53b_4201, UDF];
    let (_, states) = check(&bytes(&words), &PLACE, &[(0, State::entry(&PLACE))]);
    assert_eq!(states[&16].0[1], None);
}

#[test]
fn system_words() {
    for word in [
        0xd503_201f,
        0xd503_203f,
        0xd503_207f,
        0xd503_20df,
        0xd503_229f,
        0xd503_241f,
        0xd503_24df,
    ] {
        assert_eq!(decode(word), Step::Plain(Vec::new()), "hint {word:#x}");
    }
    for word in [0xd503_221f, 0xd503_211f, 0xd503_223f] {
        assert_eq!(decode(word), Step::Unknown, "esb, paciasp, psb {word:#x}");
    }
    for word in [
        0xd503_3bbf,
        0xd503_3fdf,
        0xd503_305f,
        0xd503_3f9f,
        0xd503_30ff,
    ] {
        assert_eq!(decode(word), Step::Plain(Vec::new()), "barrier {word:#x}");
    }
    assert_eq!(decode(0xd503_301f), Step::Unknown, "a reserved barrier");
    for word in [0xd500_401f, 0xd500_403f, 0xd500_405f] {
        assert_eq!(
            decode(word),
            Step::Plain(Vec::new()),
            "cfinv, xaflag, axflag {word:#x}"
        );
    }
    assert_eq!(decode(0xd500_407f), Step::ForbiddenRegister);
    assert_eq!(
        decode(0xd500_41ff),
        Step::ForbiddenRegister,
        "cfinv with CRm set is msr uao? forbidden"
    );
    for word in [0xd51b_4200, 0xd51b_4400, 0xd51b_4420, 0xd51b_d040] {
        assert_eq!(decode(word), Step::Plain(Vec::new()), "msr {word:#x}");
    }
    for word in [
        0xd51b_d060,
        0xd51b_4220,
        0xd51b_4500,
        0xd51a_4200,
        0xd513_4200,
    ] {
        assert_eq!(decode(word), Step::ForbiddenRegister, "msr {word:#x}");
    }
    assert_eq!(decode(0xd508_7620), Step::Unknown, "dc ivac");
    assert_eq!(decode(0xd508_871f), Step::Unknown, "tlbi");
    assert_eq!(decode(0xd52b_7420), Step::Unknown, "sysl");
    assert_eq!(
        decode(0xd503_201e),
        Step::Unknown,
        "a hint naming a register"
    );
    assert_eq!(decode(0xd400_0002), Step::SystemCall, "hvc");
    assert_eq!(decode(0xd400_0003), Step::SystemCall, "smc");
    assert_eq!(decode(0xd440_0000), Step::Leaves, "hlt");
    assert_eq!(decode(0xd460_0000), Step::Unknown, "tcancel");
    assert_eq!(decode(0xd420_0001), Step::Unknown, "brk with LL");
    assert_eq!(decode(0xd400_0005), Step::Unknown, "svc with op2");
    assert_eq!(decode(0x0000_ffff), Step::Leaves, "udf #0xffff");
    assert_eq!(decode(0x0001_0000), Step::Unknown);
    assert_eq!(decode(0x0420_0000), Step::Unknown, "sve");
    assert_eq!(decode(0xd71f_0800), Step::Unknown, "braa");
    assert_eq!(decode(0xd61f_0001), Step::Unknown, "br with op4");
    assert_eq!(decode(0xd69f_03e0), Step::Unknown, "eret");
}

#[test]
fn cache_operations_touch_their_block() {
    // dc zva, x23 and dc civac, x23 at the window's last byte; ic ivau through xzr.
    let cut = add_ext(23, 24, 1, 0b010, 0, false);
    let start = with(&[(1, one(u32::MAX.into()))]);
    assert_eq!(run_from(&start, &[cut, 0xd50b_7437]).0, []);
    assert_eq!(run_from(&start, &[cut, 0xd50b_7e37]).0, []);
    let start = with(&[(1, one(0))]);
    assert_eq!(
        run_from(&start, &[cut, 0xd50b_7437]).0.len(),
        1,
        "a block's start may be below the base"
    );
    assert_eq!(run_from(&start, &[0xd50b_753f]).0.len(), 1, "ic ivau, xzr");
}

#[test]
fn families_the_checker_does_not_follow_are_unknown() {
    for (word, what) in [
        (0xf820_0400u32, "ldraa"),
        (0xd920_0800, "stg"),
        (0x1900_0400, "cpyfp"),
        (0x9180_0000, "addg"),
        (0x91c0_0000, "smax imm"),
        (0x9ac0_1000, "irg"),
        (0x9ac0_1400, "gmi"),
        (0x9ac0_0000, "subp"),
        (0xf83f_d000, "ld64b"),
        (0x6900_0000, "stgp"),
        (0xe900_0000, "ldp size 11"),
        (0xed40_0000, "ldp q size 11"),
        (0x3cc0_0000, "ldur q0: known"),
        (0x38a0_0800 | 0x2000, "register offset with option 001"),
        (
            0x3c80_0800 | 0x4000 | 0x4000_0000,
            "ldr q register offset size 01",
        ),
        (0x78c0_0000, "ldursh? opc 11 size 01 is 32-bit: known"),
        (0xb8c0_0000, "ldursw? opc 11 size 10"),
        (0x3c80_0800, "sttr v"),
        (0xdc00_0000, "ldr literal v size 11"),
        (0x38ff_d000, "atomic opc 1101"),
        (0x3ca0_0000, "atomic v"),
        (0x38a0_c000, "ldapr with rs"),
        (0x9900_0400, "ldapur bits 01"),
        (0x1d00_0000, "ldapur v"),
    ] {
        let known = matches!(what, w if w.ends_with("known"));
        assert_eq!(
            decode(word) == Step::Unknown,
            !known,
            "{what} {word:#010x}: {:?}",
            decode(word)
        );
    }
}

#[test]
fn the_slot_s_and_the_frame_s_last_bytes_are_inside() {
    let cut = add_ext(23, 24, 1, 0b010, 0, false);
    // ldrb at base + 2^33 - 1 and at base + 2^33.
    // ldrb w0, [x23, x2] from the window's last byte: the slot's last byte, and the byte past it.
    let at = |offset: u64| {
        let start = with(&[(1, one(u32::MAX.into()))]);
        let words = [
            cut,
            movz(2, (offset & 0xffff) as u32, 0),
            movk(2, (offset >> 16 & 0xffff) as u32, 1),
            movk(2, (offset >> 32 & 0xffff) as u32, 2),
            0x3862_6ae0,
        ];
        run_from(&start, &words).0.len()
    };
    assert_eq!(at(1 << 32), 0);
    assert_eq!(at((1 << 32) + 1), 1);
    // ldrb w0, [x28, #255] and #256.
    assert_eq!(run_from(&with(&[]), &[0x3943_ff80]).0, []);
    assert_eq!(run_from(&with(&[]), &[0x3944_0380]).0.len(), 1);
    // A literal load of the sequence's own last word, and of the word past it.
    assert_eq!(
        run_from(&with(&[]), &[0x1800_0020]).0,
        [],
        "ldr w0, .+4: the udf"
    );
    assert_eq!(run_from(&with(&[]), &[0x1800_0040]).0.len(), 1);
    // The 64 KiB below a reserved window, and a byte more.
    let reserved = Place {
        below: 1 << 16,
        ..PLACE
    };
    let words = [
        sub_imm(23, 24, 16),
        add_imm(23, 23, 0, false),
        0x385f_02e0,
        UDF,
    ];
    assert_eq!(
        check(&bytes(&words), &reserved, &[(0, State::entry(&reserved))]).0,
        []
    );
    let words = [0xd140_4317, 0xd100_06f7, 0x3940_02e0, UDF];
    assert_eq!(
        check(&bytes(&words), &reserved, &[(0, State::entry(&reserved))])
            .0
            .len(),
        1
    );
}
