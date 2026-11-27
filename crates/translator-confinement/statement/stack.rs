//! Form I's pieces of the stack's immediate forms, in the window's way (look-ahead none; carry.rs `access_in_process`
//! rules a and b, `stack_pointer_in_process` rule c):
//!
//! - an access through the stack pointer with an immediate offset, pre- or post-index included (`ldr x0, [sp, #8]`,
//!   `stp x29, x30, [sp, #-16]!`, ...): the guest's word itself, then the exit ([`copied`]);
//! - a stack step, `sub sp, sp, #k` with k at most 60 KiB: the word, the probe `ldr wzr, [sp]`, the exit.
//!
//! Both rest on the entry rule's stack pointer in the window, and reach below it: the access by its negative offset (up
//! to 1 KiB, a pair of quadwords), the step's probe by the step. So each proof needs that much reserved below the
//! window (`a.below`), which form I's pool now reserves (64 KiB). What each word does is a hypothesis on the free
//! `step` (see the crate's documentation). `tests/pieces.rs` checks the pieces are the translator's, by the pins at the
//! end of this file and of copy.rs.

use super::*;
use super::meaning::*;
use super::copy::*;

verus! {

/// What an access through the stack pointer does (Arm ARM): it touches `bytes` bytes from the stack pointer plus
/// `off` (a fault ends the run), writes up to two registers (`rt`, `rt2`; 31 for none) and maybe the stack pointer
/// (written back), and nothing else.
pub open spec fn stack_means(step: spec_fn(Machine, u32) -> Stepped, word: u32, rt: int, rt2: int, off: int, bytes: nat) -> bool {
    forall|m: Machine| m.x.len() == 31 ==> {
        let s = #[trigger] step(m, word);
        &&& s.accesses == seq![((m.sp + off) % 0x1_0000_0000_0000_0000int, bytes)]
        &&& !s.system_call
        &&& s.after.unlisted == m.unlisted
        &&& s.after.x.len() == 31
        &&& forall|r: int| 0 <= r < 31 && r != rt && r != rt2 ==> s.after.x[r] == m.x[r]
        &&& s.after.ended || s.after.pc == (m.pc + 4) as u64
    }
}

/// THE STACK ACCESS PIECE IS CONFINED: the copied access through the stack pointer, with what its offset reaches below
/// the window reserved.
pub proof fn stack_access_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    word: u32,
    rt: int,
    rt2: int,
    off: int,
    bytes: nat,
    a: Allowed,
    guest: int,
)
    requires
        rt != frame_register() && rt2 != frame_register(),
        -1024 <= off && off + bytes <= 0x1_0000 && bytes <= 32,
        a.below >= 1024 && !a.displaced,
        stack_means(step, word, rt, rt2, off, bytes),
        exit_means(step),
        in_reach(a, guest),
        a.length == 20,
    ensures
        is_confined(step, copied(word, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = copied(word, slot);
    let inv = |m: Machine| kept(a, m)
        && (m.pc - a.code == 0 ==> a.base <= m.sp && m.sp < a.base + 0x1_0000_0000)
        && (in_sequence(a, m.pc) && m.pc - a.code >= 4 ==> at_exit(m, slot, (m.pc - a.code - 4) / 4));
    assert(instructions.subrange(1, 5) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        if m.pc - a.code == 0 {
            assert(word_at(instructions, a, m.pc) == word);
            assert((m.sp + off) % 0x1_0000_0000_0000_0000int == m.sp + off);
        } else {
            lemma_exit_step(step, instructions, a, guest, 4, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// `sub sp, sp, #k` (k under 4096, or `#k >> 12, lsl #12`) and the probe `ldr wzr, [sp]`.
pub open spec fn stack_step(k: int) -> u32 {
    if k < 4096 {
        (0xd100_03ffu32 | ((k as u32) << 10u32)) as u32
    } else {
        (0xd140_03ffu32 | (((k / 4096) as u32) << 10u32)) as u32
    }
}

pub const PROBE_SP: u32 = 0xb940_03ff;

/// The step's piece: the step, the probe, the exit.
pub open spec fn stepped(k: int, slot: u64) -> Seq<u32> {
    seq![stack_step(k), PROBE_SP] + exit_words(slot)
}

/// What the step and the probe do (Arm ARM): the step moves the stack pointer down by `k`, the probe reads 4 bytes there.
pub open spec fn step_means(step: spec_fn(Machine, u32) -> Stepped, k: int) -> bool {
    &&& forall|m: Machine| #[trigger] step(m, stack_step(k)) == Stepped {
        after: Machine { sp: ((m.sp - k) % 0x1_0000_0000_0000_0000int) as u64, pc: (m.pc + 4) as u64, ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
    &&& stack_means(step, PROBE_SP, 31, 31, 0, 4)
    &&& exit_means(step)
}

/// THE STACK STEP PIECE IS CONFINED, with the step reserved below the window.
pub proof fn stack_step_is_confined(step: spec_fn(Machine, u32) -> Stepped, k: int, a: Allowed, guest: int)
    requires
        0 < k <= 0xf000 && (k < 4096 || k % 4096 == 0),
        a.below >= k && !a.displaced,
        step_means(step, k),
        in_reach(a, guest),
        a.length == 24,
    ensures
        is_confined(step, stepped(k, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = stepped(k, slot);
    let inv = |m: Machine| kept(a, m)
        && (m.pc - a.code == 0 ==> a.base <= m.sp && m.sp < a.base + 0x1_0000_0000)
        && (m.pc - a.code == 4 ==> a.base - k <= m.sp && m.sp < a.base + 0x1_0000_0000)
        && (in_sequence(a, m.pc) && m.pc - a.code >= 8 ==> at_exit(m, slot, (m.pc - a.code - 8) / 4));
    assert(instructions.subrange(2, 6) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        if o == 0 {
            assert(word_at(instructions, a, m.pc) == stack_step(k));
            assert((m.sp - k) % 0x1_0000_0000_0000_0000int == m.sp - k);
        } else if o == 4 {
            assert(word_at(instructions, a, m.pc) == PROBE_SP);
            assert((m.sp + 0) % 0x1_0000_0000_0000_0000int == m.sp as int);
        } else {
            lemma_exit_step(step, instructions, a, guest, 8, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// `add sp, sp, #k` and its probe `ldur wzr, [sp, #-4]` (the freed frame's last word).
pub open spec fn stack_step_up(k: int) -> u32 {
    (stack_step(k) ^ 0x4000_0000u32) as u32
}

pub const PROBE_SP_BELOW: u32 = 0xb85f_c3ff;

/// The upward step's piece: the step, the probe, the exit.
pub open spec fn stepped_up(k: int, slot: u64) -> Seq<u32> {
    seq![stack_step_up(k), PROBE_SP_BELOW] + exit_words(slot)
}

/// What the upward step and its probe do (Arm ARM).
pub open spec fn step_up_means(step: spec_fn(Machine, u32) -> Stepped, k: int) -> bool {
    &&& forall|m: Machine| #[trigger] step(m, stack_step_up(k)) == Stepped {
        after: Machine { sp: ((m.sp + k) % 0x1_0000_0000_0000_0000int) as u64, pc: (m.pc + 4) as u64, ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
    &&& stack_means(step, PROBE_SP_BELOW, 31, 31, -4, 4)
    &&& exit_means(step)
}

/// THE UPWARD STACK STEP PIECE IS CONFINED: its probe reads below the new stack pointer, inside the slot's never mapped
/// half above the window.
pub proof fn stack_step_up_is_confined(step: spec_fn(Machine, u32) -> Stepped, k: int, a: Allowed, guest: int)
    requires
        0 < k <= 0xf000 && (k < 4096 || k % 4096 == 0),
        a.below >= 4 && !a.displaced,
        step_up_means(step, k),
        in_reach(a, guest),
        a.length == 24,
    ensures
        is_confined(step, stepped_up(k, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = stepped_up(k, slot);
    let inv = |m: Machine| kept(a, m)
        && (m.pc - a.code == 0 ==> a.base <= m.sp && m.sp < a.base + 0x1_0000_0000)
        && (m.pc - a.code == 4 ==> a.base <= m.sp && m.sp < a.base + 0x1_0000_0000 + k)
        && (in_sequence(a, m.pc) && m.pc - a.code >= 8 ==> at_exit(m, slot, (m.pc - a.code - 8) / 4));
    assert(instructions.subrange(2, 6) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        if o == 0 {
            assert(word_at(instructions, a, m.pc) == stack_step_up(k));
            assert((m.sp + k) % 0x1_0000_0000_0000_0000int == m.sp + k);
        } else if o == 4 {
            assert(word_at(instructions, a, m.pc) == PROBE_SP_BELOW);
            assert((m.sp + -4) % 0x1_0000_0000_0000_0000int == m.sp - 4);
        } else {
            lemma_exit_step(step, instructions, a, guest, 8, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// A pair of quadwords through the stack pointer (32 bytes, `stp/ldp qT, qT2, [sp, #off]`, pre-index too; post-index,
/// whose probes start at `mov x23, sp`, is not proven yet):
/// the page test of the access's last byte (`add x23, sp, #((off + 31) mod 4096)`, `and x23, x23, #0xfe0`, `cbz x23`
/// to the probes), the copied access, the exit, and out of line the stack pointer's alignment test (`mov x23, sp`,
/// `and x23, x23, #15`, `cbz x23, .+8`, `brk #5` when misaligned) and the probes of its first and last bytes at the
/// stack pointer plus `off`, back to the access (carry.rs `page_crossing` through `sp`).
pub open spec fn pair_page_test(off: int) -> u32 {
    (0x9100_03f7u32 | ((((off + 31) % 4096) as u32) << 10u32)) as u32
}

pub const AND_X23_PAIR: u32 = 0x927b_1af7;
pub const CBZ_X23_24: u32 = 0xb400_00d7;
pub const MOV_X23_SP: u32 = 0x9100_03f7;
pub const AND_X23_15: u32 = 0x9240_0ef7;
pub const CBZ_X23_8: u32 = 0xb400_0057;
pub const BRK_MISALIGNED: u32 = 0xd420_00a0;
pub const ADD_X23_SP_X23: u32 = 0x8b37_63f7;
pub const PROBE_STORE_X23: u32 = 0x383f_02ff;
pub const ADD_X23_31: u32 = 0x9100_7ef7;
pub const PROBE_LOAD_X23: u32 = 0x3940_02ff;
pub const PROBE_LOAD_X23_31: u32 = 0x3940_7eff;
pub const B_BACK_52_PAIR: u32 = 0x17ff_fff3;
pub const B_BACK_56_PAIR: u32 = 0x17ff_fff2;

/// `movz x23, #off`, or `movn x23, #(-off - 1)` for a negative one.
pub open spec fn move_offset_x23(off: int) -> u32 {
    if off >= 0 {
        (0xd280_0017u32 | ((off as u32) << 5u32)) as u32
    } else {
        (0x9280_0017u32 | ((((-off - 1) as int) as u32) << 5u32)) as u32
    }
}

/// The pair's piece: 17 words for a load, 18 for a store.
pub open spec fn stack_pair(word: u32, off: int, store: bool, slot: u64) -> Seq<u32> {
    seq![pair_page_test(off), AND_X23_PAIR, CBZ_X23_24, word] + exit_words(slot)
        + seq![MOV_X23_SP, AND_X23_15, CBZ_X23_8, BRK_MISALIGNED, move_offset_x23(off), ADD_X23_SP_X23]
        + if store { seq![PROBE_STORE_X23, ADD_X23_31, PROBE_STORE_X23, B_BACK_56_PAIR] } else { seq![PROBE_LOAD_X23, PROBE_LOAD_X23_31, B_BACK_52_PAIR] }
}

/// What the pair's piece's words do (Arm ARM); the access writes no general register (its registers are vectors).
pub open spec fn pair_means(step: spec_fn(Machine, u32) -> Stepped, word: u32, off: int, store: bool) -> bool {
    &&& writes(step, pair_page_test(off), 23, |m: Machine| ((m.sp + (off + 31) % 4096) % 0x1_0000_0000_0000_0000int) as u64)
    &&& writes(step, AND_X23_PAIR, 23, |m: Machine| m.x[23] & 0xfe0u64)
    &&& branches_if(step, CBZ_X23_24, 23, 24, false)
    &&& stack_means(step, word, 31, 31, off, 32)
    &&& exit_means(step)
    &&& writes(step, MOV_X23_SP, 23, |m: Machine| m.sp)
    &&& writes(step, AND_X23_15, 23, |m: Machine| m.x[23] & 15u64)
    &&& branches_if(step, CBZ_X23_8, 23, 8, false)
    &&& stops(step, BRK_MISALIGNED)
    &&& writes(step, move_offset_x23(off), 23, |m: Machine| (off % 0x1_0000_0000_0000_0000int) as u64)
    &&& writes(step, ADD_X23_SP_X23, 23, |m: Machine| ((m.sp + m.x[23]) % 0x1_0000_0000_0000_0000int) as u64)
    &&& store ==> accesses(step, PROBE_STORE_X23, 31, |m: Machine| m.x[23] as int, 1)
    &&& store ==> writes(step, ADD_X23_31, 23, |m: Machine| ((m.x[23] + 31) % 0x1_0000_0000_0000_0000int) as u64)
    &&& store ==> branches(step, B_BACK_56_PAIR, -56)
    &&& !store ==> accesses(step, PROBE_LOAD_X23, 31, |m: Machine| m.x[23] as int, 1)
    &&& !store ==> accesses(step, PROBE_LOAD_X23_31, 31, |m: Machine| (m.x[23] + 31) % 0x1_0000_0000_0000_0000int, 1)
    &&& !store ==> branches(step, B_BACK_52_PAIR, -52)
}

/// x plus an offset, taken modulo 2^64, is x plus the offset where that stays an address.
proof fn lemma_plus(x: int, off: int)
    requires
        0 <= x < 0x1_0000_0000_0000_0000int,
        -0x1_0000 <= off < 0x1_0000,
        0 <= x + off < 0x1_0000_0000_0000_0000int,
    ensures
        (x + off % 0x1_0000_0000_0000_0000int) % 0x1_0000_0000_0000_0000int == x + off,
        (x + off) % 0x1_0000_0000_0000_0000int == x + off,
{
    if off < 0 {
        assert(off % 0x1_0000_0000_0000_0000int == off + 0x1_0000_0000_0000_0000int);
        assert((x + off + 0x1_0000_0000_0000_0000int) % 0x1_0000_0000_0000_0000int == x + off);
    }
}

/// Where the pair's run stands: the stack pointer in the window everywhere but at the exit (the access may write it
/// back), x23 the offset, then the address probed, at the probes; the exit's moves.
pub open spec fn pair_inv(a: Allowed, slot: u64, store: bool, off: int, m: Machine) -> bool {
    let o = m.pc - a.code;
    &&& kept(a, m)
    &&& (o < 16 || o >= 32) ==> a.base <= m.sp && m.sp < a.base + 0x1_0000_0000
    &&& o == 52 ==> m.x[23] as int == off % 0x1_0000_0000_0000_0000int
    &&& (o == 56 || o == 60) ==> m.x[23] as int == m.sp + off
    &&& (store && o == 64) ==> m.x[23] as int == m.sp + off + 31
    &&& 16 <= o < 32 ==> at_exit(m, slot, (o - 16) / 4)
}

/// THE STACK PAIR PIECE IS CONFINED, with what its offset reaches below the window reserved.
pub proof fn stack_pair_is_confined(step: spec_fn(Machine, u32) -> Stepped, word: u32, off: int, store: bool, a: Allowed, guest: int)
    requires
        -1024 <= off && off + 32 <= 0x1_0000,
        a.below >= 1024 && !a.displaced,
        pair_means(step, word, off, store),
        in_reach(a, guest),
        a.length == if store { 72int } else { 68int },
    ensures
        is_confined(step, stack_pair(word, off, store, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = stack_pair(word, off, store, slot);
    let inv = |m: Machine| pair_inv(a, slot, store, off, m);
    assert(instructions.len() == if store { 18int } else { 17int });
    assert(instructions.subrange(4, 8) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let w = word_at(instructions, a, m.pc);
        if o == 0 {
            assert(w == pair_page_test(off));
        } else if o == 4 {
            assert(w == AND_X23_PAIR);
        } else if o == 8 {
            assert(w == CBZ_X23_24);
        } else if o == 12 {
            assert(w == word);
            lemma_plus(m.sp as int, off);
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
            assert(w == move_offset_x23(off));
        } else if o == 52 {
            assert(w == ADD_X23_SP_X23);
            lemma_plus(m.sp as int, off);
        } else if store {
            if o == 56 {
                assert(w == PROBE_STORE_X23);
            } else if o == 60 {
                assert(w == ADD_X23_31);
            } else if o == 64 {
                assert(w == PROBE_STORE_X23);
            } else {
                assert(o == 68);
                assert(w == B_BACK_56_PAIR);
            }
        } else {
            if o == 56 {
                assert(w == PROBE_LOAD_X23);
            } else if o == 60 {
                assert(w == PROBE_LOAD_X23_31);
            } else {
                assert(o == 64);
                assert(w == B_BACK_52_PAIR);
            }
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The pins (see copy.rs): one line each, the piece's function and its arguments, the words.
pub open spec fn stack_pair_at(word: u32, off: int, store: bool, base: int, guest: int, displaced: bool) -> Seq<u32> {
    stack_pair(word, off, store, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

pub open spec fn stepped_up_at(k: int, base: int, guest: int, displaced: bool) -> Seq<u32> {
    stepped_up(k, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

pub open spec fn stepped_at(k: int, base: int, guest: int, displaced: bool) -> Seq<u32> {
    stepped(k, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

proof fn pin_stack_load() { assert(copied_at(0xf940_07e0u32, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xf940_07e0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_stack_push() { assert(copied_at(0xa9bf_7bfdu32, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xa9bf_7bfdu32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_stack_step_16() { assert(stepped_at(16int, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd100_43ffu32, 0xb940_03ffu32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_stack_step_60k() { assert(stepped_at(61440int, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd140_3fffu32, 0xb940_03ffu32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_stack_step_up_16() { assert(stepped_up_at(16int, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0x9100_43ffu32, 0xb85f_c3ffu32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_stack_step_up_60k() { assert(stepped_up_at(61440int, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0x9140_3fffu32, 0xb85f_c3ffu32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_stack_pair_push() { assert(stack_pair_at(0xadbf_07e0u32, -32int, true, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0x913f_fff7u32, 0x927b_1af7u32, 0xb400_00d7u32, 0xadbf_07e0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0x9100_03f7u32, 0x9240_0ef7u32, 0xb400_0057u32, 0xd420_00a0u32, 0x9280_03f7u32, 0x8b37_63f7u32, 0x383f_02ffu32, 0x9100_7ef7u32, 0x383f_02ffu32, 0x17ff_fff2u32]) by (compute); }
proof fn pin_stack_pair_load() { assert(stack_pair_at(0xad40_07e0u32, 0int, false, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0x9100_7ff7u32, 0x927b_1af7u32, 0xb400_00d7u32, 0xad40_07e0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0x9100_03f7u32, 0x9240_0ef7u32, 0xb400_0057u32, 0xd420_00a0u32, 0xd280_0017u32, 0x8b37_63f7u32, 0x3940_02ffu32, 0x3940_7effu32, 0x17ff_fff3u32]) by (compute); }
proof fn pin_stack_pair_load_minus_1024() { assert(stack_pair_at(0xad60_07e0u32, -1024int, false, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0x9130_7ff7u32, 0x927b_1af7u32, 0xb400_00d7u32, 0xad60_07e0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0x9100_03f7u32, 0x9240_0ef7u32, 0xb400_0057u32, 0xd420_00a0u32, 0x9280_7ff7u32, 0x8b37_63f7u32, 0x3940_02ffu32, 0x3940_7effu32, 0x17ff_fff3u32]) by (compute); }

} // verus!
