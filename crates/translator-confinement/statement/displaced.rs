//! Form I's piece of an access through the stack pointer in displaced code (look-ahead none; carry.rs
//! `access_in_window` through `sp`, rule e): the stack pointer is the guest's own there, so the access is checked and
//! cut as through any other register. The stack pointer's alignment test (`mov x23, sp`, `and x23, x23, #15`, `cbnz
//! x23` to a floor call, the misaligned one), `mov x23, sp`, the window check of x23, the cut `add x23, x24, w23,
//! uxtw`, the access through x23 (a written back one between `sub/add x15, sp, #k`, the new stack pointer, and `mov sp, x15`), the exit, the
//! branch to the floor call, the mark's tail, the floor call, and the way back after it (never reached from entry).
//!
//! These are the accesses with no page probe (of at most 16 bytes, aligned to their size by the stack pointer's
//! alignment); what the offset reaches below the window, up to 1 KiB, is reserved. What each word does is a hypothesis
//! on the free `step` (see the crate's documentation). `tests/pieces.rs` checks the pieces are the translator's.

use super::*;
use super::meaning::*;
use super::copy::*;
use super::access::*;
use super::branch::*;
use super::stack::*;

verus! {

/// `cbnz x23, .+by`, `cbnz x15, .+by`, `b .+16`, `b .-36`, `mov sp, x15`.
pub open spec fn cbnz_x23(by: int) -> u32 {
    (0xb500_0017u32 | (((by / 4) as u32) << 5u32)) as u32
}

pub open spec fn cbnz_x15(by: int) -> u32 {
    (0xb500_000fu32 | (((by / 4) as u32) << 5u32)) as u32
}

pub const B_16: u32 = 0x1400_0004;
pub const B_BACK_36: u32 = 0x17ff_fff7;
pub const MOV_SP_X15: u32 = 0x9100_01ff;

/// The extra bytes a written back access takes: its two words.
pub open spec fn extra(back: bool) -> int {
    if back { 8 } else { 0 }
}

/// The piece: 23 words, 25 for a written back access (`new_sp` the `sub x15, sp, #k` before it).
pub open spec fn displaced_stack(word: u32, back: bool, new_sp: u32, slot: u64) -> Seq<u32> {
    let d = extra(back);
    seq![MOV_X23_SP, AND_X23_15, cbnz_x23(40 + d), MOV_X23_SP, check_word(23), cbnz_x15(32 + d), add_cut(23)]
        + (if back { seq![new_sp, word, MOV_SP_X15] } else { seq![word] })
        + exit_words(slot)
        + seq![B_16, EOR_X15_TOP, LDRB_MARK, UDF]
        + floor_call(floor_entry(6))
        + seq![LDR_X30_FRAME, B_BACK_36]
}

/// What the access through x23 does (Arm ARM): `bytes` bytes at x23 plus `off`, up to two registers written (`rt`,
/// `rt2`; 31 for none) and x23 (written back), nothing else.
pub open spec fn through_x23_means(step: spec_fn(Machine, u32) -> Stepped, word: u32, rt: int, rt2: int, off: int, bytes: nat) -> bool {
    forall|m: Machine| m.x.len() == 31 ==> {
        let s = #[trigger] step(m, word);
        &&& s.accesses == seq![((m.x[23] + off) % 0x1_0000_0000_0000_0000int, bytes)]
        &&& !s.system_call
        &&& s.after.unlisted == m.unlisted
        &&& s.after.x.len() == 31
        &&& forall|r: int| 0 <= r < 31 && r != rt && r != rt2 && r != 23 ==> s.after.x[r] == m.x[r]
        &&& s.after.ended || s.after.pc == (m.pc + 4) as u64
    }
}

/// What the piece's words do (Arm ARM).
pub open spec fn displaced_stack_means(
    step: spec_fn(Machine, u32) -> Stepped,
    word: u32,
    rt: int,
    rt2: int,
    off: int,
    bytes: nat,
    back: bool,
    new_sp: u32,
    new_value: spec_fn(Machine) -> u64,
) -> bool {
    let d = extra(back);
    &&& writes(step, MOV_X23_SP, 23, |m: Machine| m.sp)
    &&& writes(step, AND_X23_15, 23, |m: Machine| m.x[23] & 15u64)
    &&& branches_if(step, cbnz_x23(40 + d), 23, 40 + d, true)
    &&& writes(step, check_word(23), 15, |m: Machine| (m.x[23] >> 32u64) & 0xff_ffffu64)
    &&& branches_if(step, cbnz_x15(32 + d), 15, 32 + d, true)
    &&& writes(step, add_cut(23), 23, |m: Machine| ((m.x[24] + (m.x[23] & 0xffff_ffffu64)) % two_to_64()) as u64)
    &&& through_x23_means(step, word, rt, rt2, off, bytes)
    &&& back ==> writes(step, new_sp, 15, new_value)
    &&& back ==> forall|m: Machine| #[trigger] step(m, MOV_SP_X15) == Stepped {
        after: Machine { sp: m.x[15], pc: (m.pc + 4) as u64, ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
    &&& exit_means(step)
    &&& branches(step, B_16, 16)
    &&& mark_means(step)
    &&& floor_means(step)
}

/// Where the run stands: the frame and base kept; x23 the cut address from the cut to the access; the exit's moves;
/// the mark; the floor call's moves; the way back after the floor call never.
pub open spec fn displaced_stack_inv(a: Allowed, slot: u64, back: bool, m: Machine) -> bool {
    let o = m.pc - a.code;
    let d = extra(back);
    let at = 28 + d / 2;
    &&& kept(a, m) && m.x[base_register()] as int == a.base
    &&& o < 84 + d
    &&& 28 <= o <= at ==> in_window(a, m.x[23])
    &&& 32 + d <= o < 48 + d ==> at_exit(m, slot, (o - 32 - d) / 4)
    &&& o == 56 + d ==> m.x[15] as int == a.base + 0x1_0000_0000
    &&& 64 + d <= o < 84 + d ==> at_floor_call(m, floor_entry(6), (o - 64 - d) / 4)
}

/// THE DISPLACED STACK ACCESS PIECE IS CONFINED, with what its offset reaches below the window reserved.
pub proof fn displaced_stack_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    word: u32,
    rt: int,
    rt2: int,
    off: int,
    bytes: nat,
    back: bool,
    new_sp: u32,
    new_value: spec_fn(Machine) -> u64,
    a: Allowed,
    guest: int,
)
    requires
        rt != frame_register() && rt2 != frame_register() && rt != base_register() && rt2 != base_register(),
        -1024 <= off && off + bytes <= 0x1_0000 && 1 <= bytes <= 16,
        displaced_stack_means(step, word, rt, rt2, off, bytes, back, new_sp, new_value),
        in_reach(a, guest) && a.displaced,
        a.below >= 1024,
        frame_in_reach(a),
        a.length == 92 + extra(back),
    ensures
        is_confined(step, displaced_stack(word, back, new_sp, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let d = extra(back);
    let instructions = displaced_stack(word, back, new_sp, slot);
    let inv = |m: Machine| displaced_stack_inv(a, slot, back, m);
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    assert(instructions.len() == 23 + d / 4);
    assert(instructions.subrange(8 + d / 4, 12 + d / 4) =~= exit_words(slot));
    assert(instructions.subrange(16 + d / 4, 21 + d / 4) =~= floor_call(floor_entry(6)));
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
            assert(w == cbnz_x23(40 + d));
        } else if o == 12 {
            assert(w == MOV_X23_SP);
        } else if o == 16 {
            assert(w == check_word(23));
        } else if o == 20 {
            assert(w == cbnz_x15(32 + d));
        } else if o == 24 {
            assert(w == add_cut(23));
            lemma_low_word(m.x[23]);
        } else if back && o == 28 {
            assert(w == new_sp);
        } else if o == 28 + d / 2 {
            assert(w == word);
            lemma_offset_any(m.x[23] as int, off);
        } else if back && o == 36 {
            assert(w == MOV_SP_X15);
        } else if 32 + d <= o < 48 + d {
            lemma_exit_step(step, instructions, a, guest, 32 + d, m);
        } else if o == 48 + d {
            assert(w == B_16);
        } else if o == 52 + d {
            assert(w == EOR_X15_TOP);
            lemma_mark(a, m.x[24]);
        } else if o == 56 + d {
            assert(w == LDRB_MARK);
        } else if o == 60 + d {
            assert(w == UDF);
        } else {
            lemma_floor_step(step, instructions, a, 6, 64 + d, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// x plus an offset under 2^16 either way, taken modulo 2^64, is x plus the offset where that stays an address.
proof fn lemma_offset_any(x: int, off: int)
    requires
        0 <= x < 0x1_0000_0000_0000_0000int,
        -0x1_0000 <= off < 0x1_0000,
        0 <= x + off < 0x1_0000_0000_0000_0000int,
    ensures
        (x + off) % 0x1_0000_0000_0000_0000int == x + off,
{
}

/// The pins (see copy.rs): one line each, the piece's function and its arguments, the words.
pub open spec fn displaced_stack_at(word: u32, back: bool, new_sp: u32, base: int, guest: int) -> Seq<u32> {
    displaced_stack(word, back, new_sp, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced: true }, guest + 4) as u64)
}

proof fn pin_displaced_stack_load() { assert(displaced_stack_at(0xf940_06e0u32, false, 0u32, 0x6_0000_0000int, 0x40_1000int) =~= seq![0x9100_03f7u32, 0x9240_0ef7u32, 0xb500_0157u32, 0x9100_03f7u32, 0xd360_deefu32, 0xb500_010fu32, 0x8b37_4317u32, 0xf940_06e0u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32, 0x1400_0004u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0xf900_2f9eu32, 0xd280_0c1eu32, 0xf2a0_001eu32, 0xf2c4_003eu32, 0xd63f_03c0u32, 0xf940_2f9eu32, 0x17ff_fff7u32]) by (compute); }
proof fn pin_displaced_stack_push() { assert(displaced_stack_at(0xa9bf_7afdu32, true, 0xd100_43efu32, 0x6_0000_0000int, 0x40_1000int) =~= seq![0x9100_03f7u32, 0x9240_0ef7u32, 0xb500_0197u32, 0x9100_03f7u32, 0xd360_deefu32, 0xb500_014fu32, 0x8b37_4317u32, 0xd100_43efu32, 0xa9bf_7afdu32, 0x9100_01ffu32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32, 0x1400_0004u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0xf900_2f9eu32, 0xd280_0c1eu32, 0xf2a0_001eu32, 0xf2c4_003eu32, 0xd63f_03c0u32, 0xf940_2f9eu32, 0x17ff_fff7u32]) by (compute); }

} // verus!
