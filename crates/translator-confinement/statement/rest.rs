//! Form I's pieces of the shapes the earlier proofs left open (look-ahead none):
//!
//! - a post-indexed pair of quadwords through the stack pointer (`stp/ldp qT, qT2, [sp], #imm`): stack.rs's pair piece
//!   with the access at the stack pointer itself, whose probes start at `mov x23, sp` (one word shorter);
//! - a byte at `[sp]` in displaced code: displaced.rs's piece with the access cut by its own addressing
//!   (`ldrb/strb wT, [x24, w23, uxtw]`) in place of the cut and the access through x23;
//! - an unscaled register-offset access of 4 or 8 bytes (`ldr xT, [xN, xM]`): the address made in x23, checked, cut,
//!   the page test of its last byte inline, the access `[x23, xzr]`, the probes out of line;
//! - a literal load that crosses a page (`ldr xT, label` whose 8 bytes end on the next page): offset.rs's literal
//!   piece with the probes of its first and last byte inline before the load.
//!
//! What each word does is a hypothesis on the free `step` (see the crate's documentation). `tests/rest.rs` checks the
//! pieces are the translator's, by the pins at the end of this file.

use super::*;
use super::meaning::*;
use super::copy::*;
use super::access::*;
use super::branch::*;
use super::stack::*;
use super::displaced::*;

verus! {

/// `b .-48`, the load's way back to the post-indexed pair's access.
pub const B_BACK_48: u32 = 0x17ff_fff4;

/// The post-indexed pair's piece: 17 words for a store, 16 for a load.
pub open spec fn stack_pair_post(word: u32, store: bool, slot: u64) -> Seq<u32> {
    seq![pair_page_test(0), AND_X23_PAIR, CBZ_X23_24, word] + exit_words(slot)
        + seq![MOV_X23_SP, AND_X23_15, CBZ_X23_8, BRK_MISALIGNED, MOV_X23_SP]
        + if store { seq![PROBE_STORE_X23, ADD_X23_31, PROBE_STORE_X23, B_BACK_52_PAIR] } else { seq![PROBE_LOAD_X23, PROBE_LOAD_X23_31, B_BACK_48] }
}

/// What its words do (Arm ARM); the access, 32 bytes at the stack pointer, may write the stack pointer back by any
/// amount and writes no general register (its registers are vectors).
pub open spec fn pair_post_means(step: spec_fn(Machine, u32) -> Stepped, word: u32, store: bool) -> bool {
    &&& writes(step, pair_page_test(0), 23, |m: Machine| ((m.sp + 31) % 0x1_0000_0000_0000_0000int) as u64)
    &&& writes(step, AND_X23_PAIR, 23, |m: Machine| m.x[23] & 0xfe0u64)
    &&& branches_if(step, CBZ_X23_24, 23, 24, false)
    &&& stack_means(step, word, 31, 31, 0, 32)
    &&& exit_means(step)
    &&& writes(step, MOV_X23_SP, 23, |m: Machine| m.sp)
    &&& writes(step, AND_X23_15, 23, |m: Machine| m.x[23] & 15u64)
    &&& branches_if(step, CBZ_X23_8, 23, 8, false)
    &&& stops(step, BRK_MISALIGNED)
    &&& store ==> accesses(step, PROBE_STORE_X23, 31, |m: Machine| m.x[23] as int, 1)
    &&& store ==> writes(step, ADD_X23_31, 23, |m: Machine| ((m.x[23] + 31) % 0x1_0000_0000_0000_0000int) as u64)
    &&& store ==> branches(step, B_BACK_52_PAIR, -52)
    &&& !store ==> accesses(step, PROBE_LOAD_X23, 31, |m: Machine| m.x[23] as int, 1)
    &&& !store ==> accesses(step, PROBE_LOAD_X23_31, 31, |m: Machine| (m.x[23] + 31) % 0x1_0000_0000_0000_0000int, 1)
    &&& !store ==> branches(step, B_BACK_48, -48)
}

/// Where its run stands: the stack pointer in the window everywhere but at the exit, x23 the stack pointer at the
/// probes (then its last byte's address before a store's second probe), the exit's moves.
pub open spec fn pair_post_inv(a: Allowed, slot: u64, store: bool, m: Machine) -> bool {
    let o = m.pc - a.code;
    &&& kept(a, m)
    &&& (o < 16 || o >= 32) ==> a.base <= m.sp && m.sp < a.base + 0x1_0000_0000
    &&& (o == 52 || o == 56) ==> m.x[23] == m.sp
    &&& (store && o == 60) ==> m.x[23] as int == m.sp + 31
    &&& 16 <= o < 32 ==> at_exit(m, slot, (o - 16) / 4)
}

/// THE POST-INDEXED STACK PAIR PIECE IS CONFINED: it reaches nothing below the stack pointer.
pub proof fn stack_pair_post_is_confined(step: spec_fn(Machine, u32) -> Stepped, word: u32, store: bool, a: Allowed, guest: int)
    requires
        !a.displaced,
        pair_post_means(step, word, store),
        in_reach(a, guest),
        a.length == if store { 68int } else { 64int },
    ensures
        is_confined(step, stack_pair_post(word, store, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = stack_pair_post(word, store, slot);
    let inv = |m: Machine| pair_post_inv(a, slot, store, m);
    assert(instructions.len() == if store { 17int } else { 16int });
    assert(instructions.subrange(4, 8) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let w = word_at(instructions, a, m.pc);
        if o == 0 {
            assert(w == pair_page_test(0));
        } else if o == 4 {
            assert(w == AND_X23_PAIR);
        } else if o == 8 {
            assert(w == CBZ_X23_24);
        } else if o == 12 {
            assert(w == word);
            assert((m.sp + 0) % 0x1_0000_0000_0000_0000int == m.sp as int);
        } else if 16 <= o < 32 {
            lemma_exit_step(step, instructions, a, guest, 16, m);
        } else if o == 32 {
            assert(w == MOV_X23_SP);
        } else if o == 36 {
            assert(w == AND_X23_15);
        } else if o == 40 {
            assert(w == CBZ_X23_8);
        } else if o == 44 {
            assert(w == BRK_MISALIGNED);
        } else if o == 48 {
            assert(w == MOV_X23_SP);
        } else if store {
            if o == 52 {
                assert(w == PROBE_STORE_X23);
            } else if o == 56 {
                assert(w == ADD_X23_31);
            } else if o == 60 {
                assert(w == PROBE_STORE_X23);
            } else {
                assert(o == 64);
                assert(w == B_BACK_52_PAIR);
            }
        } else {
            if o == 52 {
                assert(w == PROBE_LOAD_X23);
            } else if o == 56 {
                assert(w == PROBE_LOAD_X23_31);
            } else {
                assert(o == 60);
                assert(w == B_BACK_48);
            }
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The displaced byte at `[sp]`: 22 words.
pub open spec fn displaced_byte(t: u32, store: bool, slot: u64) -> Seq<u32> {
    seq![MOV_X23_SP, AND_X23_15, cbnz_x23(36), MOV_X23_SP, check_word(23), cbnz_x15(28), cut_byte(23, t, store)]
        + exit_words(slot)
        + seq![B_16, EOR_X15_TOP, LDRB_MARK, UDF]
        + floor_call(floor_entry(6))
        + seq![LDR_X30_FRAME, B_BACK_36]
}

/// What its words do (Arm ARM).
pub open spec fn displaced_byte_means(step: spec_fn(Machine, u32) -> Stepped, t: u32, store: bool) -> bool {
    &&& writes(step, MOV_X23_SP, 23, |m: Machine| m.sp)
    &&& writes(step, AND_X23_15, 23, |m: Machine| m.x[23] & 15u64)
    &&& branches_if(step, cbnz_x23(36), 23, 36, true)
    &&& writes(step, check_word(23), 15, |m: Machine| (m.x[23] >> 32u64) & 0xff_ffffu64)
    &&& branches_if(step, cbnz_x15(28), 15, 28, true)
    &&& accesses(step, cut_byte(23, t, store), loaded(t, store), |m: Machine| (m.x[24] + (m.x[23] & 0xffff_ffffu64)) % two_to_64(), 1)
    &&& exit_means(step)
    &&& branches(step, B_16, 16)
    &&& mark_means(step)
    &&& floor_means(step)
}

/// Where its run stands: the frame and base kept, the exit's moves, the mark, the floor call's moves, the way back
/// after the floor call never.
pub open spec fn displaced_byte_inv(a: Allowed, slot: u64, m: Machine) -> bool {
    let o = m.pc - a.code;
    &&& kept(a, m) && m.x[base_register()] as int == a.base
    &&& o < 80
    &&& 28 <= o < 44 ==> at_exit(m, slot, (o - 28) / 4)
    &&& o == 52 ==> m.x[15] as int == a.base + 0x1_0000_0000
    &&& 60 <= o < 80 ==> at_floor_call(m, floor_entry(6), (o - 60) / 4)
}

/// THE DISPLACED BYTE AT `[sp]` IS CONFINED, whatever the stack pointer holds.
pub proof fn displaced_byte_is_confined(step: spec_fn(Machine, u32) -> Stepped, t: u32, store: bool, a: Allowed, guest: int)
    requires
        registers(23, t, store),
        displaced_byte_means(step, t, store),
        in_reach(a, guest) && a.displaced,
        frame_in_reach(a),
        a.length == 88,
    ensures
        is_confined(step, displaced_byte(t, store, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = displaced_byte(t, store, slot);
    let inv = |m: Machine| displaced_byte_inv(a, slot, m);
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    assert(instructions.len() == 22);
    assert(instructions.subrange(7, 11) =~= exit_words(slot));
    assert(instructions.subrange(15, 20) =~= floor_call(floor_entry(6)));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let w = word_at(instructions, a, m.pc);
        if o == 0 {
            assert(w == MOV_X23_SP);
        } else if o == 4 {
            assert(w == AND_X23_15);
        } else if o == 8 {
            assert(w == cbnz_x23(36));
        } else if o == 12 {
            assert(w == MOV_X23_SP);
        } else if o == 16 {
            assert(w == check_word(23));
        } else if o == 20 {
            assert(w == cbnz_x15(28));
        } else if o == 24 {
            assert(w == cut_byte(23, t, store));
            lemma_low_word(m.x[23]);
        } else if 28 <= o < 44 {
            lemma_exit_step(step, instructions, a, guest, 28, m);
        } else if o == 44 {
            assert(w == B_16);
        } else if o == 48 {
            assert(w == EOR_X15_TOP);
            lemma_mark(a, m.x[24]);
        } else if o == 52 {
            assert(w == LDRB_MARK);
        } else if o == 56 {
            assert(w == UDF);
        } else {
            lemma_floor_step(step, instructions, a, 6, 60, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The unscaled register-offset piece's own words: `add x15, x23, #k`, `and x15, x15, #(page less the access)`,
/// `ldrb wzr, [x15, #k]`, `add x15, x15, #k`, and its branches.
pub open spec fn add_x15_x23(k: int) -> u32 {
    (0x9100_02efu32 | ((k as u32) << 10u32)) as u32
}

pub open spec fn and_x15_page(bytes: nat) -> u32 {
    if bytes == 8 { 0x927d_21efu32 } else { 0x927e_25efu32 }
}

pub open spec fn page_mask(bytes: nat) -> u64 {
    if bytes == 8 { 0xff8u64 } else { 0xffcu64 }
}

pub open spec fn probe_x15_at(k: int) -> u32 {
    (0x3940_01ffu32 | ((k as u32) << 10u32)) as u32
}

pub open spec fn add_x15_x15(k: int) -> u32 {
    (0x9100_01efu32 | ((k as u32) << 10u32)) as u32
}

pub const CBNZ_X15_40: u32 = 0xb500_014f;
pub const CBZ_X15_36: u32 = 0xb400_012f;
pub const B_BACK_52: u32 = 0x17ff_fff3;

/// The unscaled register-offset piece: 21 words for a store, 20 for a load.
pub open spec fn register_unscaled(address: u32, access: u32, bytes: nat, store: bool, slot: u64) -> Seq<u32> {
    seq![address, check_word(23), CBNZ_X15_40, add_cut(23), add_x15_x23(bytes - 1), and_x15_page(bytes), CBZ_X15_36, access]
        + exit_words(slot)
        + seq![EOR_X15_TOP, LDRB_MARK, UDF, MOVZ_X15_0, ADD_X15_X23_X15]
        + if store { seq![PROBE_STORE, add_x15_x15(bytes - 1), PROBE_STORE, B_BACK_52] } else { seq![PROBE_LOAD_FIRST, probe_x15_at(bytes - 1), B_BACK_48] }
}

/// What its words do (Arm ARM), the first word's value free.
pub open spec fn register_unscaled_means(
    step: spec_fn(Machine, u32) -> Stepped,
    address: u32,
    value: spec_fn(Machine) -> u64,
    access: u32,
    t: u32,
    bytes: nat,
    store: bool,
) -> bool {
    &&& writes(step, address, 23, value)
    &&& writes(step, check_word(23), 15, |m: Machine| (m.x[23] >> 32u64) & 0xff_ffffu64)
    &&& branches_if(step, CBNZ_X15_40, 15, 40, true)
    &&& writes(step, add_cut(23), 23, |m: Machine| ((m.x[24] + (m.x[23] & 0xffff_ffffu64)) % two_to_64()) as u64)
    &&& writes(step, add_x15_x23(bytes - 1), 15, |m: Machine| ((m.x[23] + bytes - 1) % two_to_64()) as u64)
    &&& writes(step, and_x15_page(bytes), 15, |m: Machine| m.x[15] & page_mask(bytes))
    &&& branches_if(step, CBZ_X15_36, 15, 36, false)
    &&& accesses(step, access, loaded(t, store), |m: Machine| m.x[23] as int, bytes)
    &&& mark_means(step)
    &&& exit_means(step)
    &&& writes(step, MOVZ_X15_0, 15, |m: Machine| 0u64)
    &&& writes(step, ADD_X15_X23_X15, 15, |m: Machine| ((m.x[23] + m.x[15]) % two_to_64()) as u64)
    &&& store ==> accesses(step, PROBE_STORE, 31, |m: Machine| m.x[15] as int, 1)
    &&& store ==> writes(step, add_x15_x15(bytes - 1), 15, |m: Machine| ((m.x[15] + bytes - 1) % two_to_64()) as u64)
    &&& store ==> branches(step, B_BACK_52, -52)
    &&& !store ==> accesses(step, PROBE_LOAD_FIRST, 31, |m: Machine| m.x[15] as int, 1)
    &&& !store ==> accesses(step, probe_x15_at(bytes - 1), 31, |m: Machine| (m.x[15] + bytes - 1) % two_to_64(), 1)
    &&& !store ==> branches(step, B_BACK_48, -48)
}

/// Where its run stands: the frame and base kept; x23 the cut address from the cut to the access and all through the
/// probes; x15 zero, then the address probed; the exit's moves; the mark.
pub open spec fn register_unscaled_inv(a: Allowed, slot: u64, store: bool, bytes: nat, m: Machine) -> bool {
    let o = m.pc - a.code;
    let back = if store { 80int } else { 76int };
    &&& kept(a, m) && m.x[base_register()] as int == a.base
    &&& (16 <= o <= 28 || (60 <= o && o <= back)) ==> in_window(a, m.x[23])
    &&& o == 64 ==> m.x[15] == 0
    &&& (o == 68 || o == 72) ==> in_window(a, m.x[15])
    &&& (store && o == 76) ==> a.base + bytes - 1 <= m.x[15] as int && (m.x[15] as int) < a.base + 0x1_0000_0000 + bytes - 1
    &&& 32 <= o < 48 ==> at_exit(m, slot, (o - 32) / 4)
    &&& o == 52 ==> m.x[15] as int == a.base + 0x1_0000_0000
}

/// THE UNSCALED REGISTER-OFFSET PIECE IS CONFINED, whatever address the first word makes.
pub proof fn register_unscaled_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    address: u32,
    value: spec_fn(Machine) -> u64,
    access: u32,
    t: u32,
    bytes: nat,
    store: bool,
    a: Allowed,
    guest: int,
)
    requires
        t < 32 && (store || (t != 23 && t != 24 && t != 28)),
        bytes == 4 || bytes == 8,
        register_unscaled_means(step, address, value, access, t, bytes, store),
        in_reach(a, guest),
        a.length == if store { 84int } else { 80int },
    ensures
        is_confined(step, register_unscaled(address, access, bytes, store, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = register_unscaled(address, access, bytes, store, slot);
    let inv = |m: Machine| register_unscaled_inv(a, slot, store, bytes, m);
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    assert(instructions.len() == if store { 21int } else { 20int });
    assert(instructions.subrange(8, 12) =~= exit_words(slot));
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
            assert(w == CBNZ_X15_40);
        } else if o == 12 {
            assert(w == add_cut(23));
            lemma_low_word(m.x[23]);
        } else if o == 16 {
            assert(w == add_x15_x23(bytes - 1));
        } else if o == 20 {
            assert(w == and_x15_page(bytes));
        } else if o == 24 {
            assert(w == CBZ_X15_36);
        } else if o == 28 {
            assert(w == access);
        } else if 32 <= o < 48 {
            lemma_exit_step(step, instructions, a, guest, 32, m);
        } else if o == 48 {
            assert(w == EOR_X15_TOP);
            lemma_mark(a, m.x[24]);
        } else if o == 52 {
            assert(w == LDRB_MARK);
        } else if o == 56 {
            assert(w == UDF);
        } else if o == 60 {
            assert(w == MOVZ_X15_0);
        } else if o == 64 {
            assert(w == ADD_X15_X23_X15);
        } else if store {
            if o == 68 {
                assert(w == PROBE_STORE);
            } else if o == 72 {
                assert(w == add_x15_x15(bytes - 1));
            } else if o == 76 {
                assert(w == PROBE_STORE);
            } else {
                assert(o == 80);
                assert(w == B_BACK_52);
            }
        } else {
            if o == 68 {
                assert(w == PROBE_LOAD_FIRST);
            } else if o == 72 {
                assert(w == probe_x15_at(bytes - 1));
            } else {
                assert(o == 76);
                assert(w == B_BACK_48);
            }
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// `ldrb wzr, [x23, #k]`.
pub open spec fn probe_x23_at(k: int) -> u32 {
    (0x3940_02ffu32 | ((k as u32) << 10u32)) as u32
}

/// The literal load that crosses a page: the label's guest address built in x23, the cut, the probes of its first and
/// last byte, the load `[x23]`, the exit. 11 words.
pub open spec fn literal_crossing(label: u64, load: u32, bytes: nat, slot: u64) -> Seq<u32> {
    seq![movz_x23(label & 0xffff, 0), movk_x23((label >> 16u64) & 0xffff, 1), movk_x23((label >> 32u64) & 0xffff, 2), add_cut(23),
        probe_x23_at(0), probe_x23_at(bytes - 1), load]
        + exit_words(slot)
}

/// THE LITERAL LOAD THAT CROSSES A PAGE IS CONFINED: the probes and the load read at most 16 bytes at the cut address.
pub proof fn literal_crossing_is_confined(
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
        accesses(step, probe_x23_at(0), 31, |m: Machine| m.x[23] as int, 1),
        accesses(step, probe_x23_at(bytes - 1), 31, |m: Machine| (m.x[23] + bytes - 1) % two_to_64(), 1),
        accesses(step, load, t, |m: Machine| m.x[23] as int, bytes),
        in_reach(a, guest),
        a.length == 44,
    ensures
        is_confined(step, literal_crossing(label, load, bytes, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = literal_crossing(label, load, bytes, slot);
    let inv = |m: Machine| {
        let o = m.pc - a.code;
        &&& kept(a, m) && m.x[base_register()] as int == a.base
        &&& 16 <= o <= 24 ==> in_window(a, m.x[23])
        &&& 28 <= o < 44 ==> at_exit(m, slot, (o - 28) / 4)
    };
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    lemma_moves_build(label);
    assert(instructions.len() == 11);
    assert(instructions.subrange(7, 11) =~= exit_words(slot));
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
            assert(w == probe_x23_at(0));
        } else if o == 20 {
            assert(w == probe_x23_at(bytes - 1));
        } else if o == 24 {
            assert(w == load);
        } else {
            lemma_exit_step(step, instructions, a, guest, 28, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The pins (see copy.rs): one line each, the piece's function and its arguments, the words. `tests/rest.rs` names the
/// guest instruction of each.
pub open spec fn at(base: int, guest: int, displaced: bool) -> u64 {
    exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64
}

proof fn pin_stack_pair_post_store() { assert(stack_pair_post(0xacbf_07e0u32, true, at(0x6_0000_0000int, 0x40_1000int, false)) =~= seq![0x9100_7ff7u32, 0x927b_1af7u32, 0xb400_00d7u32, 0xacbf_07e0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0x9100_03f7u32, 0x9240_0ef7u32, 0xb400_0057u32, 0xd420_00a0u32, 0x9100_03f7u32, 0x383f_02ffu32, 0x9100_7ef7u32, 0x383f_02ffu32, 0x17ff_fff3u32]) by (compute); }
proof fn pin_stack_pair_post_load() { assert(stack_pair_post(0xacc1_07e0u32, false, at(0x6_0000_0000int, 0x40_1000int, false)) =~= seq![0x9100_7ff7u32, 0x927b_1af7u32, 0xb400_00d7u32, 0xacc1_07e0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0x9100_03f7u32, 0x9240_0ef7u32, 0xb400_0057u32, 0xd420_00a0u32, 0x9100_03f7u32, 0x3940_02ffu32, 0x3940_7effu32, 0x17ff_fff4u32]) by (compute); }
proof fn pin_displaced_byte_load() { assert(displaced_byte(0u32, false, at(0x6_0000_0000int, 0x40_1000int, true)) =~= seq![0x9100_03f7u32, 0x9240_0ef7u32, 0xb500_0137u32, 0x9100_03f7u32, 0xd360_deefu32, 0xb500_00efu32, 0x3877_4b00u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32, 0x1400_0004u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0xf900_2f9eu32, 0xd280_0c1eu32, 0xf2a0_001eu32, 0xf2c4_003eu32, 0xd63f_03c0u32, 0xf940_2f9eu32, 0x17ff_fff7u32]) by (compute); }
proof fn pin_displaced_byte_store() { assert(displaced_byte(0u32, true, at(0x6_0000_0000int, 0x40_1000int, true)) =~= seq![0x9100_03f7u32, 0x9240_0ef7u32, 0xb500_0137u32, 0x9100_03f7u32, 0xd360_deefu32, 0xb500_00efu32, 0x3837_4b00u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32, 0x1400_0004u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0xf900_2f9eu32, 0xd280_0c1eu32, 0xf2a0_001eu32, 0xf2c4_003eu32, 0xd63f_03c0u32, 0xf940_2f9eu32, 0x17ff_fff7u32]) by (compute); }
proof fn pin_register_unscaled_load() { assert(register_unscaled(0x8b22_6037u32, 0xf87f_6ae0u32, 8nat, false, at(0x6_0000_0000int, 0x40_1000int, false)) =~= seq![0x8b22_6037u32, 0xd360_deefu32, 0xb500_014fu32, 0x8b37_4317u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_012fu32, 0xf87f_6ae0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_fff4u32]) by (compute); }
proof fn pin_register_unscaled_store() { assert(register_unscaled(0x8b22_6037u32, 0xf83f_6ae0u32, 8nat, true, at(0x6_0000_0000int, 0x40_1000int, false)) =~= seq![0x8b22_6037u32, 0xd360_deefu32, 0xb500_014fu32, 0x8b37_4317u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_012fu32, 0xf83f_6ae0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x383f_01ffu32, 0x9100_1defu32, 0x383f_01ffu32, 0x17ff_fff3u32]) by (compute); }
proof fn pin_register_unscaled_word_displaced() { assert(register_unscaled(0x8b22_6037u32, 0xb87f_6ae0u32, 4nat, false, at(0x6_0000_0000int, 0x40_1000int, true)) =~= seq![0x8b22_6037u32, 0xd360_deefu32, 0xb500_014fu32, 0x8b37_4317u32, 0x9100_0eefu32, 0x927e_25efu32, 0xb400_012fu32, 0xb87f_6ae0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_0dffu32, 0x17ff_fff4u32]) by (compute); }
proof fn pin_literal_crossing() { assert(literal_crossing(0x40_1ffcu64, 0xf940_02e0u32, 8nat, at(0x6_0000_0000int, 0x40_1000int, false)) =~= seq![0xd283_ff97u32, 0xf2a0_0817u32, 0xf2c0_0017u32, 0x8b37_4317u32, 0x3940_02ffu32, 0x3940_1effu32, 0xf940_02e0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }

} // verus!
