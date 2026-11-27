//! Form I's pieces of a load and a store through a guest register with no offset (look-ahead none), as the translator
//! emits them (carry.rs `access_in_window`, `window_check`, `outside_words`, `page_crossing`):
//!
//! - a byte, `ldrb/strb wT, [xN]`: the window check (`ubfx x15, xN, #32, #24; cbnz x15, mark`), the access cut by its
//!   own addressing `[x24, wN, uxtw]`, the exit, and the mark's tail (`eor x15, x24, #2^32; ldrb wzr, [x15, #4095];
//!   udf`);
//! - a doubleword, `ldr/str xT, [xN]`: the window check, the cut `add x23, x24, wN, uxtw`, the alignment test
//!   (`and x15, xN, #7; cbnz x15, slow`), the access `[x23]`, the exit, the mark's tail, and the slow tail: whether
//!   the access crosses a page, and if it does the probes of its first and last byte (`ldrb wzr` for a load, `staddb
//!   wzr` for a store) before going back to the access.
//!
//! What each word does is a hypothesis on the free `step` (see the crate's documentation). `tests/pieces.rs` checks the
//! pieces are the translator's, by the pins at the end of this file.

use super::*;
use super::meaning::*;
use super::copy::*;

verus! {

pub open spec fn two_to_64() -> int {
    0x1_0000_0000_0000_0000
}

/// `ubfx x15, xN, #32, #24`.
pub open spec fn check_word(n: u32) -> u32 {
    (0xd360_dc0fu32 | (n << 5u32)) as u32
}

/// `cbnz x15, .+24` and `cbnz x15, .+36`, `cbz x15, .+8`.
pub const CBNZ_X15_24: u32 = 0xb500_00cf;
pub const CBNZ_X15_36: u32 = 0xb500_012f;
pub const CBZ_X15_8: u32 = 0xb400_004f;

/// `ldrb wT, [x24, wN, uxtw]` and `strb wT, [x24, wN, uxtw]`.
pub open spec fn cut_byte(n: u32, t: u32, store: bool) -> u32 {
    ((if store { 0x3820_4b00u32 } else { 0x3860_4b00u32 }) | (n << 16u32) | t) as u32
}

/// `add x23, x24, wN, uxtw`, `and x15, xN, #7`, and `ldr xT, [x23]` / `str xT, [x23]`.
pub open spec fn add_cut(n: u32) -> u32 {
    (0x8b20_4317u32 | (n << 16u32)) as u32
}

pub open spec fn align_word(n: u32) -> u32 {
    (0x9240_080fu32 | (n << 5u32)) as u32
}

pub open spec fn wide_word(t: u32, store: bool) -> u32 {
    ((if store { 0xf900_02e0u32 } else { 0xf940_02e0u32 }) | t) as u32
}

/// The mark: `eor x15, x24, #0x100000000`, `ldrb wzr, [x15, #4095]`, `udf #0`.
pub const EOR_X15_TOP: u32 = 0xd260_030f;
pub const LDRB_MARK: u32 = 0x397f_fdff;
pub const UDF: u32 = 0;

/// The slow tail: `add x15, x23, #7`, `and x15, x15, #0xff8`, `b .-44`, `movz x15, #0`, `add x15, x23, x15`; a load's
/// `ldrb wzr, [x15]`, `ldrb wzr, [x15, #7]`, `b .-64`; a store's `staddb wzr, [x15]`, `add x15, x15, #7`, `b .-68`.
pub const ADD_X15_X23_7: u32 = 0x9100_1eef;
pub const AND_X15_PAGE: u32 = 0x927d_21ef;
pub const B_BACK_44: u32 = 0x17ff_fff5;
pub const MOVZ_X15_0: u32 = 0xd280_000f;
pub const ADD_X15_X23_X15: u32 = 0x8b2f_62ef;
pub const PROBE_LOAD_FIRST: u32 = 0x3940_01ff;
pub const PROBE_LOAD_LAST: u32 = 0x3940_1dff;
pub const B_BACK_64: u32 = 0x17ff_fff0;
pub const PROBE_STORE: u32 = 0x383f_01ff;
pub const ADD_X15_X15_7: u32 = 0x9100_1def;
pub const B_BACK_68: u32 = 0x17ff_ffef;

/// The byte piece, 10 words.
pub open spec fn byte_access(n: u32, t: u32, store: bool, slot: u64) -> Seq<u32> {
    seq![check_word(n), CBNZ_X15_24, cut_byte(n, t, store)] + exit_words(slot) + seq![EOR_X15_TOP, LDRB_MARK, UDF]
}

/// The doubleword piece: 22 words for a load, 23 for a store.
pub open spec fn wide_access(n: u32, t: u32, store: bool, slot: u64) -> Seq<u32> {
    seq![check_word(n), CBNZ_X15_36, add_cut(n), align_word(n), CBNZ_X15_36, wide_word(t, store)]
        + exit_words(slot)
        + seq![EOR_X15_TOP, LDRB_MARK, UDF, ADD_X15_X23_7, AND_X15_PAGE, CBZ_X15_8, B_BACK_44, MOVZ_X15_0, ADD_X15_X23_X15]
        + if store { seq![PROBE_STORE, ADD_X15_X15_7, PROBE_STORE, B_BACK_68] } else { seq![PROBE_LOAD_FIRST, PROBE_LOAD_LAST, B_BACK_64] }
}

/// The mark's meaning (Arm ARM): the `eor`, a one-byte load of x15 + 4095 (which faults), `udf`.
pub open spec fn mark_means(step: spec_fn(Machine, u32) -> Stepped) -> bool {
    &&& writes(step, EOR_X15_TOP, 15, |m: Machine| m.x[24] ^ 0x1_0000_0000u64)
    &&& accesses(step, LDRB_MARK, 31, |m: Machine| (m.x[15] + 4095) % two_to_64(), 1)
    &&& stops(step, UDF)
}

/// The window check's meaning: `ubfx` writes x15, `cbnz` goes to the mark when it is not zero.
pub open spec fn check_means(step: spec_fn(Machine, u32) -> Stepped, n: u32, to_mark: int) -> bool {
    &&& writes(step, check_word(n), 15, |m: Machine| (m.x[n as int] >> 32u64) & 0xff_ffffu64)
    &&& branches_if(step, if to_mark == 24 { CBNZ_X15_24 } else { CBNZ_X15_36 }, 15, to_mark, true)
}

/// The register a load writes: `t` (31, none, for a store).
pub open spec fn loaded(t: u32, store: bool) -> int {
    if store { 31 } else { t as int }
}

/// The byte piece's words' meaning (Arm ARM).
pub open spec fn byte_means(step: spec_fn(Machine, u32) -> Stepped, n: u32, t: u32, store: bool) -> bool {
    &&& check_means(step, n, 24)
    &&& accesses(step, cut_byte(n, t, store), loaded(t, store), |m: Machine| (m.x[24] + (m.x[n as int] & 0xffff_ffffu64)) % two_to_64(), 1)
    &&& mark_means(step)
    &&& exit_means(step)
}

/// The doubleword piece's words' meaning (Arm ARM).
pub open spec fn wide_means(step: spec_fn(Machine, u32) -> Stepped, n: u32, t: u32, store: bool) -> bool {
    &&& check_means(step, n, 36)
    &&& writes(step, add_cut(n), 23, |m: Machine| ((m.x[24] + (m.x[n as int] & 0xffff_ffffu64)) % two_to_64()) as u64)
    &&& writes(step, align_word(n), 15, |m: Machine| m.x[n as int] & 7u64)
    &&& accesses(step, wide_word(t, store), loaded(t, store), |m: Machine| m.x[23] as int, 8)
    &&& mark_means(step)
    &&& exit_means(step)
    &&& writes(step, ADD_X15_X23_7, 15, |m: Machine| ((m.x[23] + 7) % two_to_64()) as u64)
    &&& writes(step, AND_X15_PAGE, 15, |m: Machine| m.x[15] & 0xff8u64)
    &&& branches_if(step, CBZ_X15_8, 15, 8, false)
    &&& branches(step, B_BACK_44, -44)
    &&& writes(step, MOVZ_X15_0, 15, |m: Machine| 0u64)
    &&& writes(step, ADD_X15_X23_X15, 15, |m: Machine| ((m.x[23] + m.x[15]) % two_to_64()) as u64)
    &&& store ==> accesses(step, PROBE_STORE, 31, |m: Machine| m.x[15] as int, 1)
    &&& store ==> writes(step, ADD_X15_X15_7, 15, |m: Machine| ((m.x[15] + 7) % two_to_64()) as u64)
    &&& store ==> branches(step, B_BACK_68, -68)
    &&& !store ==> accesses(step, PROBE_LOAD_FIRST, 31, |m: Machine| m.x[15] as int, 1)
    &&& !store ==> accesses(step, PROBE_LOAD_LAST, 31, |m: Machine| (m.x[15] + 7) % two_to_64(), 1)
    &&& !store ==> branches(step, B_BACK_64, -64)
}

/// A host address in the window.
pub open spec fn in_window(a: Allowed, v: u64) -> bool {
    a.base <= v as int && (v as int) < a.base + 0x1_0000_0000
}

/// The guest registers the pieces name: x0 to x30 as base, any but x24 and x28 loaded (x31 is the zero register).
pub open spec fn registers(n: u32, t: u32, store: bool) -> bool {
    &&& n < 31 && t < 32
    &&& store || (t != 24 && t != 28)
}

/// The mark's address, the window's top plus a page less one, is in the slot's never mapped half.
pub proof fn lemma_mark(a: Allowed, base: u64)
    requires
        a.base % 0x2_0000_0000 == 0 && 0x2_0000_0000 <= a.base && a.base < 2048 * 0x2_0000_0000,
        base as int == a.base,
    ensures
        (base ^ 0x1_0000_0000u64) as int == a.base + 0x1_0000_0000,
{
    assert(base & 0x1_ffff_ffffu64 == 0) by (bit_vector)
        requires
            base % 0x2_0000_0000u64 == 0,
    ;
    assert((base ^ 0x1_0000_0000u64) == base + 0x1_0000_0000u64) by (bit_vector)
        requires
            base & 0x1_ffff_ffffu64 == 0,
            base < 0x1000_0000_0000u64,
    ;
}

pub proof fn lemma_low_word(v: u64)
    ensures
        (v & 0xffff_ffffu64) < 0x1_0000_0000u64,
{
    assert((v & 0xffff_ffffu64) < 0x1_0000_0000u64) by (bit_vector);
}

/// Where the byte piece's run stands: the frame and the base kept, the exit's moves at the exit, the mark's address
/// at the mark's load.
pub open spec fn byte_inv(a: Allowed, slot: u64, m: Machine) -> bool {
    let o = m.pc - a.code;
    &&& kept(a, m) && m.x[base_register()] as int == a.base
    &&& 12 <= o < 28 ==> at_exit(m, slot, (o - 12) / 4)
    &&& o == 32 ==> m.x[15] as int == a.base + 0x1_0000_0000
}

/// THE BYTE PIECE IS CONFINED: `ldrb wT, [xN]` and `strb wT, [xN]`.
pub proof fn byte_access_is_confined(step: spec_fn(Machine, u32) -> Stepped, n: u32, t: u32, store: bool, a: Allowed, guest: int)
    requires
        registers(n, t, store),
        byte_means(step, n, t, store),
        in_reach(a, guest),
        a.length == 40,
    ensures
        is_confined(step, byte_access(n, t, store, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = byte_access(n, t, store, slot);
    let inv = |m: Machine| byte_inv(a, slot, m);
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    assert(instructions.len() == 10);
    assert(instructions.subrange(3, 7) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let w = word_at(instructions, a, m.pc);
        if o == 0 {
            assert(w == check_word(n));
        } else if o == 4 {
            assert(w == CBNZ_X15_24);
        } else if o == 8 {
            assert(w == cut_byte(n, t, store));
            lemma_low_word(m.x[n as int]);
        } else if 12 <= o < 28 {
            lemma_exit_step(step, instructions, a, guest, 12, m);
        } else if o == 28 {
            assert(w == EOR_X15_TOP);
            lemma_mark(a, m.x[24]);
        } else if o == 32 {
            assert(w == LDRB_MARK);
        } else {
            assert(o == 36);
            assert(w == UDF);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// Where the doubleword piece's run stands: the frame and base kept; x23 the cut address from the cut to the access
/// and all through the slow tail; x15 zero, then the address probed, at the probes; the exit's moves; the mark.
pub open spec fn wide_inv(a: Allowed, slot: u64, store: bool, m: Machine) -> bool {
    let o = m.pc - a.code;
    let back = if store { 88int } else { 84int };
    &&& kept(a, m) && m.x[base_register()] as int == a.base
    &&& (o == 12 || o == 16 || o == 20 || (52 <= o && o <= back)) ==> in_window(a, m.x[23])
    &&& o == 72 ==> m.x[15] == 0
    &&& (o == 76 || o == 80) ==> in_window(a, m.x[15])
    &&& (store && o == 84) ==> a.base + 7 <= m.x[15] as int && (m.x[15] as int) < a.base + 0x1_0000_0007
    &&& 24 <= o < 40 ==> at_exit(m, slot, (o - 24) / 4)
    &&& o == 44 ==> m.x[15] as int == a.base + 0x1_0000_0000
}

/// THE DOUBLEWORD PIECE IS CONFINED: `ldr xT, [xN]` and `str xT, [xN]`, page-crossing probes included.
pub proof fn wide_access_is_confined(step: spec_fn(Machine, u32) -> Stepped, n: u32, t: u32, store: bool, a: Allowed, guest: int)
    requires
        registers(n, t, store),
        wide_means(step, n, t, store),
        in_reach(a, guest),
        a.length == if store { 92int } else { 88int },
    ensures
        is_confined(step, wide_access(n, t, store, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = wide_access(n, t, store, slot);
    let inv = |m: Machine| wide_inv(a, slot, store, m);
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    assert(instructions.len() == if store { 23int } else { 22int });
    assert(instructions.subrange(6, 10) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let w = word_at(instructions, a, m.pc);
        if o == 0 {
            assert(w == check_word(n));
        } else if o == 4 {
            assert(w == CBNZ_X15_36);
        } else if o == 8 {
            assert(w == add_cut(n));
            lemma_low_word(m.x[n as int]);
        } else if o == 12 {
            assert(w == align_word(n));
        } else if o == 16 {
            assert(w == CBNZ_X15_36);
        } else if o == 20 {
            assert(w == wide_word(t, store));
        } else if 24 <= o < 40 {
            lemma_exit_step(step, instructions, a, guest, 24, m);
        } else if o == 40 {
            assert(w == EOR_X15_TOP);
            lemma_mark(a, m.x[24]);
        } else if o == 44 {
            assert(w == LDRB_MARK);
        } else if o == 48 {
            assert(w == UDF);
        } else if o == 52 {
            assert(w == ADD_X15_X23_7);
        } else if o == 56 {
            assert(w == AND_X15_PAGE);
        } else if o == 60 {
            assert(w == CBZ_X15_8);
        } else if o == 64 {
            assert(w == B_BACK_44);
        } else if o == 68 {
            assert(w == MOVZ_X15_0);
        } else if o == 72 {
            assert(w == ADD_X15_X23_X15);
        } else if store {
            if o == 76 {
                assert(w == PROBE_STORE);
            } else if o == 80 {
                assert(w == ADD_X15_X15_7);
            } else if o == 84 {
                assert(w == PROBE_STORE);
            } else {
                assert(o == 88);
                assert(w == B_BACK_68);
            }
        } else {
            if o == 76 {
                assert(w == PROBE_LOAD_FIRST);
            } else if o == 80 {
                assert(w == PROBE_LOAD_LAST);
            } else {
                assert(o == 84);
                assert(w == B_BACK_64);
            }
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The pins (see copy.rs): one line each, the piece's function and its arguments, the words.
pub open spec fn byte_access_at(n: u32, t: u32, store: bool, base: int, guest: int, displaced: bool) -> Seq<u32> {
    byte_access(n, t, store, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

pub open spec fn wide_access_at(n: u32, t: u32, store: bool, base: int, guest: int, displaced: bool) -> Seq<u32> {
    wide_access(n, t, store, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

proof fn pin_byte_load() { assert(byte_access_at(1u32, 0u32, false, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd360_dc2fu32, 0xb500_00cfu32, 0x3861_4b00u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32]) by (compute); }
proof fn pin_byte_store() { assert(byte_access_at(1u32, 0u32, true, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd360_dc2fu32, 0xb500_00cfu32, 0x3821_4b00u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32]) by (compute); }
proof fn pin_wide_load() { assert(wide_access_at(1u32, 0u32, false, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd360_dc2fu32, 0xb500_012fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_012fu32, 0xf940_02e0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff5u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_fff0u32]) by (compute); }
proof fn pin_wide_store() { assert(wide_access_at(1u32, 0u32, true, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd360_dc2fu32, 0xb500_012fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_012fu32, 0xf900_02e0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff5u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x383f_01ffu32, 0x9100_1defu32, 0x383f_01ffu32, 0x17ff_ffefu32]) by (compute); }

} // verus!
