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

/// The pins (see copy.rs): one line each, the piece's function and its arguments, the words.
pub open spec fn stepped_at(k: int, base: int, guest: int, displaced: bool) -> Seq<u32> {
    stepped(k, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

proof fn pin_stack_load() { assert(copied_at(0xf940_07e0u32, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xf940_07e0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_stack_push() { assert(copied_at(0xa9bf_7bfdu32, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xa9bf_7bfdu32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_stack_step_16() { assert(stepped_at(16int, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd100_43ffu32, 0xb940_03ffu32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_stack_step_60k() { assert(stepped_at(61440int, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xd140_3fffu32, 0xb940_03ffu32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }

} // verus!
