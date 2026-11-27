//! Form I's pieces of a register-offset access and of a literal load (look-ahead none; carry.rs `access_in_window`
//! with an `Indexed` address, and the literal access rebased to its guest address):
//!
//! - a byte by register offset, `ldrb/strb wT, [xN, wM, uxtw]` and the like: the address made in x23 (`add x23, xN,
//!   wM, uxtw`), then the byte piece of access.rs through x23;
//! - a doubleword by a scaled register offset, `ldr/str xT, [xN, xM, lsl #3]` and the like: the address made in x23,
//!   then the doubleword piece through x23, its alignment test on the base xN and its access `[x23, xzr, lsl #3]`;
//! - a literal load, `ldr xT/wT, label`: the label's guest address built in x23 by the exit's moves, cut, loaded.
//!
//! The address's value does not matter (it is checked and cut): the proofs hold for whatever value the first word
//! writes to x23. What each word does is a hypothesis on the free `step` (see the crate's documentation).
//! `tests/pieces.rs` checks the pieces are the translator's, by the pins at the end of this file.

use super::*;
use super::meaning::*;
use super::copy::*;
use super::access::*;

verus! {

/// The byte piece by register offset: 11 words.
pub open spec fn register_byte(address: u32, t: u32, store: bool, slot: u64) -> Seq<u32> {
    seq![address] + byte_access(23, t, store, slot)
}

/// Where its run stands.
pub open spec fn register_byte_inv(a: Allowed, slot: u64, m: Machine) -> bool {
    let o = m.pc - a.code;
    &&& kept(a, m) && m.x[base_register()] as int == a.base
    &&& 16 <= o < 32 ==> at_exit(m, slot, (o - 16) / 4)
    &&& o == 36 ==> m.x[15] as int == a.base + 0x1_0000_0000
}

/// THE BYTE PIECE BY REGISTER OFFSET IS CONFINED, whatever address the first word makes.
pub proof fn register_byte_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    address: u32,
    value: spec_fn(Machine) -> u64,
    t: u32,
    store: bool,
    a: Allowed,
    guest: int,
)
    requires
        registers(23, t, store),
        writes(step, address, 23, value),
        byte_means(step, 23, t, store),
        in_reach(a, guest),
        a.length == 44,
    ensures
        is_confined(step, register_byte(address, t, store, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = register_byte(address, t, store, slot);
    let inv = |m: Machine| register_byte_inv(a, slot, m);
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    assert(instructions.len() == 11);
    assert(instructions.subrange(4, 8) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let w = word_at(instructions, a, m.pc);
        if o == 0 {
            assert(w == address);
        } else if o == 4 {
            assert(w == check_word(23));
        } else if o == 8 {
            assert(w == CBNZ_X15_24);
        } else if o == 12 {
            assert(w == cut_byte(23, t, store));
            lemma_low_word(m.x[23]);
        } else if 16 <= o < 32 {
            lemma_exit_step(step, instructions, a, guest, 16, m);
        } else if o == 32 {
            assert(w == EOR_X15_TOP);
            lemma_mark(a, m.x[24]);
        } else if o == 36 {
            assert(w == LDRB_MARK);
        } else {
            assert(o == 40);
            assert(w == UDF);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The doubleword piece by scaled register offset: 23 words for a load, 24 for a store.
pub open spec fn register_wide(address: u32, n: u32, access: u32, store: bool, slot: u64) -> Seq<u32> {
    seq![address, check_word(23), CBNZ_X15_36, add_cut(23), align_word(n), CBNZ_X15_36, access]
        + exit_words(slot)
        + seq![EOR_X15_TOP, LDRB_MARK, UDF, ADD_X15_X23_7, AND_X15_PAGE, CBZ_X15_8, B_BACK_44, MOVZ_X15_0, ADD_X15_X23_X15]
        + if store { seq![PROBE_STORE, ADD_X15_X15_7, PROBE_STORE, B_BACK_68] } else { seq![PROBE_LOAD_FIRST, PROBE_LOAD_LAST, B_BACK_64] }
}

/// Its words' meaning (Arm ARM), the first word's value free.
pub open spec fn register_wide_means(
    step: spec_fn(Machine, u32) -> Stepped,
    address: u32,
    value: spec_fn(Machine) -> u64,
    n: u32,
    access: u32,
    t: u32,
    store: bool,
) -> bool {
    &&& writes(step, address, 23, value)
    &&& check_means(step, 23, 36)
    &&& writes(step, add_cut(23), 23, |m: Machine| ((m.x[24] + (m.x[23] & 0xffff_ffffu64)) % two_to_64()) as u64)
    &&& writes(step, align_word(n), 15, |m: Machine| m.x[n as int] & 7u64)
    &&& accesses(step, access, loaded(t, store), |m: Machine| m.x[23] as int, 8)
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

/// Where its run stands: access.rs's doubleword invariant four bytes on.
pub open spec fn register_wide_inv(a: Allowed, slot: u64, store: bool, m: Machine) -> bool {
    let o = m.pc - a.code;
    let back = if store { 92int } else { 88int };
    &&& kept(a, m) && m.x[base_register()] as int == a.base
    &&& (o == 16 || o == 20 || o == 24 || (56 <= o && o <= back)) ==> in_window(a, m.x[23])
    &&& o == 76 ==> m.x[15] == 0
    &&& (o == 80 || o == 84) ==> in_window(a, m.x[15])
    &&& (store && o == 88) ==> a.base + 7 <= m.x[15] as int && (m.x[15] as int) < a.base + 0x1_0000_0007
    &&& 28 <= o < 44 ==> at_exit(m, slot, (o - 28) / 4)
    &&& o == 48 ==> m.x[15] as int == a.base + 0x1_0000_0000
}

/// THE DOUBLEWORD PIECE BY SCALED REGISTER OFFSET IS CONFINED, whatever address the first word makes.
pub proof fn register_wide_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    address: u32,
    value: spec_fn(Machine) -> u64,
    n: u32,
    access: u32,
    t: u32,
    store: bool,
    a: Allowed,
    guest: int,
)
    requires
        registers(n, t, store) && t != 23,
        register_wide_means(step, address, value, n, access, t, store),
        in_reach(a, guest),
        a.length == if store { 96int } else { 92int },
    ensures
        is_confined(step, register_wide(address, n, access, store, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = register_wide(address, n, access, store, slot);
    let inv = |m: Machine| register_wide_inv(a, slot, store, m);
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    assert(instructions.len() == if store { 24int } else { 23int });
    assert(instructions.subrange(7, 11) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let w = word_at(instructions, a, m.pc);
        if o == 0 {
            assert(w == address);
        } else if o == 4 {
            assert(w == check_word(23));
        } else if o == 8 {
            assert(w == CBNZ_X15_36);
        } else if o == 12 {
            assert(w == add_cut(23));
            lemma_low_word(m.x[23]);
        } else if o == 16 {
            assert(w == align_word(n));
        } else if o == 20 {
            assert(w == CBNZ_X15_36);
        } else if o == 24 {
            assert(w == access);
        } else if 28 <= o < 44 {
            lemma_exit_step(step, instructions, a, guest, 28, m);
        } else if o == 44 {
            assert(w == EOR_X15_TOP);
            lemma_mark(a, m.x[24]);
        } else if o == 48 {
            assert(w == LDRB_MARK);
        } else if o == 52 {
            assert(w == UDF);
        } else if o == 56 {
            assert(w == ADD_X15_X23_7);
        } else if o == 60 {
            assert(w == AND_X15_PAGE);
        } else if o == 64 {
            assert(w == CBZ_X15_8);
        } else if o == 68 {
            assert(w == B_BACK_44);
        } else if o == 72 {
            assert(w == MOVZ_X15_0);
        } else if o == 76 {
            assert(w == ADD_X15_X23_X15);
        } else if store {
            if o == 80 {
                assert(w == PROBE_STORE);
            } else if o == 84 {
                assert(w == ADD_X15_X15_7);
            } else if o == 88 {
                assert(w == PROBE_STORE);
            } else {
                assert(o == 92);
                assert(w == B_BACK_68);
            }
        } else {
            if o == 80 {
                assert(w == PROBE_LOAD_FIRST);
            } else if o == 84 {
                assert(w == PROBE_LOAD_LAST);
            } else {
                assert(o == 88);
                assert(w == B_BACK_64);
            }
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The literal load's piece: the label's guest address built in x23, the cut, the load `[x23]`, the exit. 9 words.
pub open spec fn literal(label: u64, load: u32, slot: u64) -> Seq<u32> {
    seq![movz_x23(label & 0xffff, 0), movk_x23((label >> 16u64) & 0xffff, 1), movk_x23((label >> 32u64) & 0xffff, 2), add_cut(23), load]
        + exit_words(slot)
}

/// THE LITERAL LOAD'S PIECE IS CONFINED: the load reads at most 16 bytes at the cut address.
pub proof fn literal_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    label: u64,
    load: u32,
    t: int,
    bytes: nat,
    a: Allowed,
    guest: int,
)
    requires
        0 <= t < 32 && t != frame_register() && t != base_register(),
        1 <= bytes <= 16,
        label < 0x1_0000_0000_0000,
        exit_means(step),
        writes(step, add_cut(23), 23, |m: Machine| ((m.x[24] + (m.x[23] & 0xffff_ffffu64)) % two_to_64()) as u64),
        accesses(step, load, t, |m: Machine| m.x[23] as int, bytes),
        in_reach(a, guest),
        a.length == 36,
    ensures
        is_confined(step, literal(label, load, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = literal(label, load, slot);
    let inv = |m: Machine| {
        let o = m.pc - a.code;
        &&& kept(a, m) && m.x[base_register()] as int == a.base
        &&& o == 16 ==> in_window(a, m.x[23])
        &&& 20 <= o < 36 ==> at_exit(m, slot, (o - 20) / 4)
    };
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    lemma_moves_build(label);
    assert(instructions.len() == 9);
    assert(instructions.subrange(5, 9) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let w = word_at(instructions, a, m.pc);
        if o == 0 {
            assert(w == movz_x23(label & 0xffff, 0));
        } else if o == 4 {
            assert(w == movk_x23((label >> 16u64) & 0xffff, 1));
        } else if o == 8 {
            assert(w == movk_x23((label >> 32u64) & 0xffff, 2));
        } else if o == 12 {
            assert(w == add_cut(23));
            lemma_low_word(m.x[23]);
        } else if o == 16 {
            assert(w == load);
        } else {
            lemma_exit_step(step, instructions, a, guest, 20, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The pins (see copy.rs): one line each, the piece's function and its arguments, the words.
pub open spec fn register_byte_at(address: u32, t: u32, store: bool, base: int, guest: int, displaced: bool) -> Seq<u32> {
    register_byte(address, t, store, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

pub open spec fn register_wide_at(address: u32, n: u32, access: u32, store: bool, base: int, guest: int, displaced: bool) -> Seq<u32> {
    register_wide(address, n, access, store, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

pub open spec fn literal_at(label: u64, load: u32, base: int, guest: int, displaced: bool) -> Seq<u32> {
    literal(label, load, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

proof fn pin_register_byte_load() { assert(register_byte_at(0x8b22_4037u32, 0u32, false, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0x8b22_4037u32, 0xd360_deefu32, 0xb500_00cfu32, 0x3877_4b00u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32]) by (compute); }
proof fn pin_register_wide_load() { assert(register_wide_at(0x8b22_6c37u32, 1u32, 0xf87f_7ae0u32, false, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0x8b22_6c37u32, 0xd360_deefu32, 0xb500_012fu32, 0x8b37_4317u32, 0x9240_082fu32, 0xb500_012fu32, 0xf87f_7ae0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff5u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_fff0u32]) by (compute); }
proof fn pin_register_wide_store() { assert(register_wide_at(0x8b22_cc37u32, 1u32, 0xf83f_dae0u32, true, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0x8b22_cc37u32, 0xd360_deefu32, 0xb500_012fu32, 0x8b37_4317u32, 0x9240_082fu32, 0xb500_012fu32, 0xf83f_dae0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff5u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x383f_01ffu32, 0x9100_1defu32, 0x383f_01ffu32, 0x17ff_ffefu32]) by (compute); }
proof fn pin_literal_doubleword() { assert(literal_at(0x40_1008u64, 0xf940_02e0u32, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd282_0117u32, 0xf2a0_0817u32, 0xf2c0_0017u32, 0x8b37_4317u32, 0xf940_02e0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_literal_word_displaced() { assert(literal_at(0x40_0ffcu64, 0xb940_02e0u32, 0x6_0000_0000int, 0x40_1000int, true) =~= seq![0xd281_ff97u32, 0xf2a0_0817u32, 0xf2c0_0017u32, 0x8b37_4317u32, 0xb940_02e0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32]) by (compute); }

} // verus!
