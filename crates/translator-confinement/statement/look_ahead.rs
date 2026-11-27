//! Form I's pieces with look-ahead (translator native.rs `translate_native_request`, `LookAhead::Bounded` and `Full`):
//! one piece holds the code of several instructions. Each straight-line part's instructions are carried one after
//! the other, each going on into the next's code where look-ahead none would exit; the part ends in its exit, and the
//! out-of-line tails of its instructions follow. Each proof here gives an invariant per part, which holds where the
//! part starts and which each of its words keeps: the frame in x28 and the window's base in x24.
//!
//! - Bounded, a line: register-only instructions, then the exit to the instruction after them ([`line`]).
//! - Bounded, an access and then register-only instructions in its part: withheld_access.rs's doubleword piece with
//!   those instructions as its words after (its proof allows them), pinned below at its look-ahead instance.
//! - Full, a counted loop: the count, register-only instructions, the conditional branch back to the count, the exit
//!   after the loop, and what linking left of the exit back to the loop's start (never reached) ([`counted_loop`]).
//! - Full, a loop with the test before it of its access's base (carry.rs `test_aligned_bases`): the test, the loop with
//!   the access's page test, and the loop's copy with the access cut by its own addressing ([`tested_loop`]). The
//!   test's answer does not matter for confinement: the copy's access is cut to the window whatever the base holds.
//!
//! What each word does is a hypothesis on the free `step` (see the crate's documentation); a conditional branch reads
//! the flags, which [`Machine`] does not hold, so its hypothesis lets it go either way. `tests/look_ahead.rs` checks
//! the pieces are the translator's, by the pins at the end of this file.

use super::*;
use super::meaning::*;
use super::copy::*;
use super::access::*;
use super::branch::*;
use super::withheld::*;
use super::withheld_access::*;

verus! {

/// `word` goes `by` bytes from itself or to the next word, either way (`b.cond`: the flags decide), changing nothing
/// else.
pub open spec fn branches_either(step: spec_fn(Machine, u32) -> Stepped, word: u32, by: int) -> bool {
    forall|m: Machine| {
        let s = #[trigger] step(m, word);
        &&& s.accesses.len() == 0 && !s.system_call
        &&& s.after == Machine { pc: s.after.pc, ..m }
        &&& s.after.pc == (m.pc + by) as u64 || s.after.pc == (m.pc + 4) as u64
    }
}

/// Register-only instructions: each writes its register, none x24 or x28 (copy.rs `copied_means`).
pub open spec fn register_only(step: spec_fn(Machine, u32) -> Stepped, words: Seq<u32>, rds: Seq<int>) -> bool {
    &&& words.len() == rds.len()
    &&& forall|k: int| 0 <= k < words.len() ==> copied_means(step, #[trigger] words[k], rds[k]) && rds[k] != base_register()
}

/// The part's invariant: the frame in x28, the base in x24.
pub open spec fn part_holds(a: Allowed, m: Machine) -> bool {
    kept(a, m) && m.x[base_register()] as int == a.base
}

/// A line's piece: the instructions, then the exit to `slot`.
pub open spec fn line(words: Seq<u32>, slot: u64) -> Seq<u32> {
    words + exit_words(slot)
}

/// THE LINE IS CONFINED: bounded look-ahead's piece of register-only instructions, exiting after the last.
pub proof fn line_is_confined(step: spec_fn(Machine, u32) -> Stepped, words: Seq<u32>, rds: Seq<int>, a: Allowed, guest: int)
    requires
        register_only(step, words, rds),
        exit_means(step),
        in_reach(a, guest),
        a.length == (words.len() + 4) * 4,
    ensures
        is_confined(step, line(words, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = line(words, slot);
    let k = words.len() as int;
    let e = k * 4;
    let inv = |m: Machine| {
        let o = m.pc - a.code;
        &&& part_holds(a, m)
        &&& e <= o < e + 16 ==> at_exit(m, slot, (o - e) / 4)
    };
    assert(e / 4 == k);
    assert(instructions.subrange(e / 4, e / 4 + 4) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let i = (m.pc - a.code) / 4;
        let w = word_at(instructions, a, m.pc);
        assert(w == instructions[i]);
        if i < k {
            assert(w == words[i]);
            assert(copied_means(step, words[i], rds[i]) && rds[i] != base_register());
        } else {
            lemma_exit_step(step, instructions, a, guest, e, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// A counted loop's piece: the count, the instructions, the conditional branch `back` to the count, the exit to
/// `slot` after the loop, and the link left in the exit to the loop's start `head` (its first move made `b` to the
/// count; never reached).
pub open spec fn counted_loop(words: Seq<u32>, back: u32, slot: u64, head: u64) -> Seq<u32> {
    seq![COUNT_UP] + words + seq![back] + exit_words(slot) + seq![b_back((words.len() as int + 6) * 4)] + exit_words(head).subrange(1, 4)
}

/// THE COUNTED LOOP IS CONFINED: full look-ahead's piece of a loop of register-only instructions. The loop's invariant
/// is the part's: it holds at the count, every instruction keeps it, and the branch back keeps it.
pub proof fn counted_loop_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    words: Seq<u32>,
    rds: Seq<int>,
    back: u32,
    a: Allowed,
    guest: int,
    head: u64,
)
    requires
        register_only(step, words, rds),
        writes(step, COUNT_UP, 18, |m: Machine| ((m.x[18] + 1) % two_to_64()) as u64),
        branches_either(step, back, -(words.len() + 1) * 4),
        exit_means(step),
        in_reach(a, guest),
        a.length == (words.len() + 10) * 4,
    ensures
        is_confined(step, counted_loop(words, back, exit_slot(a, guest + 4) as u64, head), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = counted_loop(words, back, slot, head);
    let k = words.len() as int;
    let e = (k + 2) * 4;
    let inv = |m: Machine| {
        let o = m.pc - a.code;
        &&& part_holds(a, m)
        &&& o < e + 16
        &&& e <= o ==> at_exit(m, slot, (o - e) / 4)
    };
    assert(instructions.len() == k + 10);
    assert(e / 4 == k + 2);
    assert(instructions.subrange(e / 4, e / 4 + 4) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let i = (m.pc - a.code) / 4;
        let w = word_at(instructions, a, m.pc);
        assert(w == instructions[i]);
        if i == 0 {
            assert(w == COUNT_UP);
        } else if i <= k {
            assert(w == words[i - 1]);
            assert(copied_means(step, words[i - 1], rds[i - 1]) && rds[i - 1] != base_register());
        } else if i == k + 1 {
            assert(w == back);
        } else {
            lemma_exit_step(step, instructions, a, guest, e, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// `and x23, xN, #7` (the test of the base's alignment before the loop), `cbz x23, .+by`, and the copy's access
/// `ldr xT, [x24, wN, uxtw]`.
pub open spec fn test_base(n: u32) -> u32 {
    (0x9240_0817u32 | (n << 5u32)) as u32
}

pub open spec fn cbz_x23(by: int) -> u32 {
    (0xb400_0017u32 | (((by / 4) as u32) << 5u32)) as u32
}

pub open spec fn cut_wide_load(n: u32, t: u32) -> u32 {
    (0xf860_4b00u32 | (n << 16u32) | t) as u32
}

/// The tested loop's piece, for a doubleword load `ldr xT, [xN]` and one register-only instruction `word` before the
/// conditional branch back (`back` in the loop, `back_copy` in the copy): 48 words.
///
/// - 0 the test of xN's alignment, the branch to the copy when aligned;
/// - 2 the loop: the count, access.rs's doubleword core, `word`, `back` (to the test), the exit, the mark, the slow
///   tail, the link left in the exit to the loop's start;
/// - 31 the copy: the count, the check, the access cut by its own addressing, `word`, `back_copy` (to the copy's
///   count), the exit, the mark, the link left in the exit to the copy's start.
pub open spec fn tested_loop(n: u32, t: u32, word: u32, back: u32, back_copy: u32, slot: u64, head: u64) -> Seq<u32> {
    seq![test_base(n), cbz_x23(120), COUNT_UP]
        + seq![check_c(n, 15), cbnz_c(15, 44), add_cut(n), align_word(n), cbnz_c(15, 44), wide_word(t, false), word, back]
        + exit_words(slot)
        + seq![EOR_X15_TOP, LDRB_MARK, UDF, ADD_X15_X23_7, AND_X15_PAGE, CBZ_X15_8, b_back(52), MOVZ_X15_0, ADD_X15_X23_X15,
            PROBE_LOAD_FIRST, PROBE_LOAD_LAST, b_back(72)]
        + seq![b_back(108)] + exit_words(head).subrange(1, 4)
        + seq![COUNT_UP, check_c(n, 15), cbnz_c(15, 32), cut_wide_load(n, t), word, back_copy]
        + exit_words(slot)
        + seq![EOR_X15_TOP, LDRB_MARK, UDF]
        + seq![b_back(52)] + exit_words(head).subrange(1, 4)
}

/// What the tested loop's words do (Arm ARM).
pub open spec fn tested_loop_means(step: spec_fn(Machine, u32) -> Stepped, n: u32, t: u32, word: u32, rd: int, back: u32, back_copy: u32) -> bool {
    &&& writes(step, test_base(n), 23, |m: Machine| m.x[n as int] & 7u64)
    &&& branches_if(step, cbz_x23(120), 23, 120, false)
    &&& writes(step, COUNT_UP, 18, |m: Machine| ((m.x[18] + 1) % two_to_64()) as u64)
    &&& writes(step, check_c(n, 15), 15, |m: Machine| (m.x[n as int] >> 32u64) & 0xff_ffffu64)
    &&& branches_if(step, cbnz_c(15, 44), 15, 44, true)
    &&& branches_if(step, cbnz_c(15, 32), 15, 32, true)
    &&& writes(step, add_cut(n), 23, |m: Machine| ((m.x[24] + (m.x[n as int] & 0xffff_ffffu64)) % two_to_64()) as u64)
    &&& writes(step, align_word(n), 15, |m: Machine| m.x[n as int] & 7u64)
    &&& accesses(step, wide_word(t, false), t as int, |m: Machine| m.x[23] as int, 8)
    &&& copied_means(step, word, rd)
    &&& branches_either(step, back, -40)
    &&& branches_either(step, back_copy, -20)
    &&& exit_means(step)
    &&& mark_means(step)
    &&& writes(step, ADD_X15_X23_7, 15, |m: Machine| ((m.x[23] + 7) % two_to_64()) as u64)
    &&& writes(step, AND_X15_PAGE, 15, |m: Machine| m.x[15] & 0xff8u64)
    &&& branches_if(step, CBZ_X15_8, 15, 8, false)
    &&& branches(step, b_back(52), -52)
    &&& writes(step, MOVZ_X15_0, 15, |m: Machine| 0u64)
    &&& writes(step, ADD_X15_X23_X15, 15, |m: Machine| ((m.x[23] + m.x[15]) % two_to_64()) as u64)
    &&& accesses(step, PROBE_LOAD_FIRST, 31, |m: Machine| m.x[15] as int, 1)
    &&& accesses(step, PROBE_LOAD_LAST, 31, |m: Machine| (m.x[15] + 7) % two_to_64(), 1)
    &&& branches(step, b_back(72), -72)
    &&& accesses(step, cut_wide_load(n, t), t as int, |m: Machine| (m.x[24] + (m.x[n as int] & 0xffff_ffffu64)) % two_to_64(), 8)
}

/// Where the tested loop's run stands, per part: in all three the frame in x28 and the base in x24; in the loop, x23
/// the cut address from the cut to the access and through the slow tail, x15 zero then the probed address at the
/// probes, the mark's address; the exits' moves; the links never.
pub open spec fn tested_loop_inv(a: Allowed, slot: u64, m: Machine) -> bool {
    let i = (m.pc - a.code) / 4;
    &&& part_holds(a, m)
    &&& !(27 <= i < 31) && i < 44
    &&& (6 <= i <= 8 || 18 <= i <= 26) ==> in_window(a, m.x[23])
    &&& i == 23 ==> m.x[15] == 0
    &&& (i == 24 || i == 25) ==> in_window(a, m.x[15])
    &&& 44 <= m.pc - a.code < 60 ==> at_exit(m, slot, (m.pc - a.code - 44) / 4)
    &&& 148 <= m.pc - a.code < 164 ==> at_exit(m, slot, (m.pc - a.code - 148) / 4)
    &&& (i == 16 || i == 42) ==> m.x[15] as int == a.base + 0x1_0000_0000
}

/// THE TESTED LOOP IS CONFINED: full look-ahead's piece of a loop whose access's base is tested once before it.
pub proof fn tested_loop_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    n: u32,
    t: u32,
    word: u32,
    rd: int,
    back: u32,
    back_copy: u32,
    a: Allowed,
    guest: int,
    head: u64,
)
    requires
        n < 31 && n != 23 && n != 15,
        t < 31 && t != 23 && t != 24 && t != 28,
        rd != base_register(),
        tested_loop_means(step, n, t, word, rd, back, back_copy),
        in_reach(a, guest),
        a.length == 192,
    ensures
        is_confined(step, tested_loop(n, t, word, back, back_copy, exit_slot(a, guest + 4) as u64, head), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = tested_loop(n, t, word, back, back_copy, slot, head);
    let inv = |m: Machine| tested_loop_inv(a, slot, m);
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    assert(instructions.len() == 48);
    assert(instructions.subrange(11, 15) =~= exit_words(slot));
    assert(instructions.subrange(37, 41) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let i = (m.pc - a.code) / 4;
        let w = word_at(instructions, a, m.pc);
        assert(w == instructions[i]);
        if i == 0 {
            assert(w == test_base(n));
        } else if i == 1 {
            assert(w == cbz_x23(120));
        } else if i == 2 || i == 31 {
            assert(w == COUNT_UP);
        } else if i == 3 || i == 32 {
            assert(w == check_c(n, 15));
        } else if i == 4 || i == 7 {
            assert(w == cbnz_c(15, 44));
        } else if i == 5 {
            assert(w == add_cut(n));
            lemma_low_word(m.x[n as int]);
        } else if i == 6 {
            assert(w == align_word(n));
        } else if i == 8 {
            assert(w == wide_word(t, false));
        } else if i == 9 || i == 35 {
            assert(w == word);
        } else if i == 10 {
            assert(w == back);
        } else if 11 <= i < 15 {
            lemma_exit_step(step, instructions, a, guest, 44, m);
        } else if i == 15 || i == 41 {
            assert(w == EOR_X15_TOP);
            lemma_mark(a, m.x[24]);
        } else if i == 16 || i == 42 {
            assert(w == LDRB_MARK);
        } else if i == 17 || i == 43 {
            assert(w == UDF);
        } else if i == 18 {
            assert(w == ADD_X15_X23_7);
        } else if i == 19 {
            assert(w == AND_X15_PAGE);
        } else if i == 20 {
            assert(w == CBZ_X15_8);
        } else if i == 21 {
            assert(w == b_back(52));
        } else if i == 22 {
            assert(w == MOVZ_X15_0);
        } else if i == 23 {
            assert(w == ADD_X15_X23_X15);
        } else if i == 24 {
            assert(w == PROBE_LOAD_FIRST);
        } else if i == 25 {
            assert(w == PROBE_LOAD_LAST);
        } else if i == 26 {
            assert(w == b_back(72));
        } else if i == 33 {
            assert(w == cbnz_c(15, 32));
        } else if i == 34 {
            assert(w == cut_wide_load(n, t));
            lemma_low_word(m.x[n as int]);
        } else if i == 36 {
            assert(w == back_copy);
        } else {
            lemma_exit_step(step, instructions, a, guest, 148, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The pins (see copy.rs and rest.rs): one line each, the piece's function and its arguments, the words.
/// `tests/look_ahead.rs` names the guest instructions and look-ahead of each.
proof fn pin_line_two_adds() { assert(line(seq![0x9100_0400u32, 0x9100_0821u32], super::rest::at(0x6_0000_0000int, 0x40_1004int, false)) =~= seq![0x9100_0400u32, 0x9100_0821u32, 0xd288_0417u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_line_add_subs() { assert(line(seq![0x9100_0421u32, 0xf100_0400u32], super::rest::at(0x6_0000_0000int, 0x40_1004int, false)) =~= seq![0x9100_0421u32, 0xf100_0400u32, 0xd288_0417u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_load_then_add() { assert(wide_stand_in(Seq::empty(), 1u32, 2u32, false, 0int, 24int, 15int, seq![integer_word(0x9100_0442u32, 2int)], super::rest::at(0x6_0000_0000int, 0x40_1004int, false)) =~= seq![0xd360_dc2fu32, 0xb500_014fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_014fu32, 0xf940_02e2u32, 0x9100_0442u32, 0xd288_0417u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff4u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_ffefu32]) by (compute); }
proof fn pin_load_then_subs() { assert(wide_stand_in(Seq::empty(), 1u32, 2u32, false, 0int, 24int, 15int, seq![integer_word(0xf100_0400u32, 0int)], super::rest::at(0x6_0000_0000int, 0x40_1004int, false)) =~= seq![0xd360_dc2fu32, 0xb500_014fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_014fu32, 0xf940_02e2u32, 0xf100_0400u32, 0xd288_0417u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff4u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_ffefu32]) by (compute); }
proof fn pin_counted_loop() { assert(counted_loop(seq![0x9100_0421u32, 0xf100_0400u32], 0x54ff_ffa1u32, super::rest::at(0x6_0000_0000int, 0x40_1008int, false), super::rest::at(0x6_0000_0000int, 0x40_0ffcint, false)) =~= seq![0x9100_0652u32, 0x9100_0421u32, 0xf100_0400u32, 0x54ff_ffa1u32, 0xd288_0617u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0x17ff_fff8u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_counted_loop_displaced() { assert(counted_loop(seq![0x9100_0421u32, 0xf100_0400u32], 0x54ff_ffa1u32, super::rest::at(0x6_0000_0000int, 0x40_1008int, true), super::rest::at(0x6_0000_0000int, 0x40_0ffcint, true)) =~= seq![0x9100_0652u32, 0x9100_0421u32, 0xf100_0400u32, 0x54ff_ffa1u32, 0xd288_0617u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32, 0x17ff_fff8u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_tested_loop() { assert(tested_loop(1u32, 2u32, 0xf100_0400u32, 0x54ff_fec1u32, 0x54ff_ff61u32, super::rest::at(0x6_0000_0000int, 0x40_1008int, false), super::rest::at(0x6_0000_0000int, 0x40_0ffcint, false)) =~= seq![0x9240_0837u32, 0xb400_03d7u32, 0x9100_0652u32, 0xd360_dc2fu32, 0xb500_016fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_016fu32, 0xf940_02e2u32, 0xf100_0400u32, 0x54ff_fec1u32, 0xd288_0617u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff3u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_ffeeu32, 0x17ff_ffe5u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0x9100_0652u32, 0xd360_dc2fu32, 0xb500_010fu32, 0xf861_4b02u32, 0xf100_0400u32, 0x54ff_ff61u32, 0xd288_0617u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x17ff_fff3u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_tested_loop_displaced() { assert(tested_loop(1u32, 2u32, 0xf100_0400u32, 0x54ff_fec1u32, 0x54ff_ff61u32, super::rest::at(0x6_0000_0000int, 0x40_1008int, true), super::rest::at(0x6_0000_0000int, 0x40_0ffcint, true)) =~= seq![0x9240_0837u32, 0xb400_03d7u32, 0x9100_0652u32, 0xd360_dc2fu32, 0xb500_016fu32, 0x8b21_4317u32, 0x9240_082fu32, 0xb500_016fu32, 0xf940_02e2u32, 0xf100_0400u32, 0x54ff_fec1u32, 0xd288_0617u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x9100_1eefu32, 0x927d_21efu32, 0xb400_004fu32, 0x17ff_fff3u32, 0xd280_000fu32, 0x8b2f_62efu32, 0x3940_01ffu32, 0x3940_1dffu32, 0x17ff_ffeeu32, 0x17ff_ffe5u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32, 0x9100_0652u32, 0xd360_dc2fu32, 0xb500_010fu32, 0xf861_4b02u32, 0xf100_0400u32, 0x54ff_ff61u32, 0xd288_0617u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32, 0xd260_030fu32, 0x397f_fdffu32, 0x0000_0000u32, 0x17ff_fff3u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32]) by (compute); }

} // verus!
