//! Form I's piece of a register-only integer instruction that names a withheld register (x15, x18, x23, x24, x28;
//! look-ahead none; carry.rs `carry` with its stand-ins and `substitute`): the guest's value of each withheld register
//! it names lives in the frame's words, so a free register stands in for it. Some frame accesses (a stand-in's own
//! value kept, the guest's value loaded), the instruction with the stand-ins, some frame accesses (the value written
//! back, the stand-in's own restored), the exit. For example `add x0, x15, x1`: `str x29, [x28, #144]`, `ldr x15, [x28,
//! #128]`, `add x0, x15, x1`, `str x15, [x28, #128]`, `ldr x29, [x28, #144]`, the exit.
//!
//! The proof holds for any number of frame accesses and moves on either side (x24 is moved aside and back when the
//! instruction names it): doublewords at x28 plus an offset inside the frame's words, none loading x28, and moves to
//! any register but x28. What each word does is a hypothesis on the free `step` (see the crate's
//! documentation). `tests/pieces.rs` checks the pieces are the translator's, by the pins at the end of this file.

use super::*;
use super::meaning::*;
use super::copy::*;
use super::access::*;
use super::branch::*;

verus! {

/// A word around the instruction: a frame access (the register it loads, 31 for a store; the offset from x28) or a
/// move between registers (`mov xT, xN`: T; N as `off`).
pub struct FrameAccess {
    pub word: u32,
    pub rt: int,
    pub off: int,
    pub moves: bool,
}

pub open spec fn words_of(accesses: Seq<FrameAccess>) -> Seq<u32> {
    Seq::new(accesses.len(), |k: int| accesses[k].word)
}

/// The piece: the frame accesses before, the instruction, the frame accesses after, the exit.
pub open spec fn stand_in(before: Seq<FrameAccess>, word: u32, after: Seq<FrameAccess>, slot: u64) -> Seq<u32> {
    words_of(before) + seq![word] + words_of(after) + exit_words(slot)
}

/// What a frame access does (Arm ARM `ldr/str xT, [x28, #off]`): 8 bytes at x28 plus `off`, `rt` written, nothing
/// else; what a move does (`mov xT, xN`): xN written to xT.
pub open spec fn frame_access_means(step: spec_fn(Machine, u32) -> Stepped, f: FrameAccess) -> bool {
    &&& 0 <= f.rt < 32 && f.rt != frame_register()
    &&& f.moves ==> f.rt < 31 && 0 <= f.off < 31 && writes(step, f.word, f.rt, |m: Machine| m.x[f.off])
    &&& !f.moves ==> 0 <= f.off && f.off + 8 <= FRAME_BYTES
        && accesses(step, f.word, f.rt, |m: Machine| (m.x[28] + f.off) % two_to_64(), 8)
}

/// THE STAND-IN PIECE IS CONFINED: the frame accesses touch the frame's own words, the instruction one register.
pub proof fn stand_in_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    before: Seq<FrameAccess>,
    word: u32,
    rd: int,
    after: Seq<FrameAccess>,
    a: Allowed,
    guest: int,
)
    requires
        forall|k: int| 0 <= k < before.len() ==> frame_access_means(step, #[trigger] before[k]),
        forall|k: int| 0 <= k < after.len() ==> frame_access_means(step, #[trigger] after[k]),
        copied_means(step, word, rd),
        exit_means(step),
        in_reach(a, guest),
        frame_in_reach(a),
        a.length == (before.len() + 1 + after.len() + 4) * 4,
    ensures
        is_confined(step, stand_in(before, word, after, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = stand_in(before, word, after, slot);
    let b = before.len() as int;
    let e = (b + 1 + after.len()) * 4;
    let inv = |m: Machine| {
        let o = m.pc - a.code;
        &&& kept(a, m)
        &&& e <= o < e + 16 ==> at_exit(m, slot, (o - e) / 4)
    };
    assert(instructions.subrange(e / 4, e / 4 + 4) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let i = o / 4;
        let w = word_at(instructions, a, m.pc);
        if i < b {
            assert(w == before[i].word);
            assert(frame_access_means(step, before[i]));
            if !before[i].moves {
                assert((m.x[28] + before[i].off) % two_to_64() == m.x[28] + before[i].off);
            }
        } else if i == b {
            assert(w == word);
        } else if i < b + 1 + after.len() {
            assert(w == after[i - b - 1].word);
            assert(frame_access_means(step, after[i - b - 1]));
            if !after[i - b - 1].moves {
                assert((m.x[28] + after[i - b - 1].off) % two_to_64() == m.x[28] + after[i - b - 1].off);
            }
        } else {
            lemma_exit_step(step, instructions, a, guest, e, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// `ldr/str xT, [x28, #off]`.
pub open spec fn frame_word(store: bool, t: u32, off: int) -> FrameAccess {
    FrameAccess {
        word: ((if store { 0xf900_0380u32 } else { 0xf940_0380u32 }) | (((off / 8) as u32) << 10u32) | t) as u32,
        rt: if store { 31 } else { t as int },
        off,
        moves: false,
    }
}

/// `mov xT, xN`.
pub open spec fn move_word(t: u32, n: u32) -> FrameAccess {
    FrameAccess { word: (0xaa00_03e0u32 | (n << 16u32) | t) as u32, rt: t as int, off: n as int, moves: true }
}

/// The pins (see copy.rs): one line each, the piece's function and its arguments, the words.
pub open spec fn stand_in_at(before: Seq<FrameAccess>, word: u32, after: Seq<FrameAccess>, base: int, guest: int, displaced: bool) -> Seq<u32> {
    stand_in(before, word, after, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

proof fn pin_stand_in_add_x15() { assert(stand_in_at(seq![frame_word(true, 29u32, 144int), frame_word(false, 15u32, 128int)], 0x8b01_01e0u32, seq![frame_word(true, 15u32, 128int), frame_word(false, 29u32, 144int)], 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xf900_4b9du32, 0xf940_438fu32, 0x8b01_01e0u32, 0xf900_438fu32, 0xf940_4b9du32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_stand_in_add_x15_displaced() { assert(stand_in_at(seq![frame_word(true, 2u32, 24int), frame_word(false, 2u32, 128int)], 0x8b01_0040u32, seq![frame_word(false, 2u32, 24int)], 0x6_0000_0000int, 0x40_1000int, true) =~= seq![0xf900_0f82u32, 0xf940_4382u32, 0x8b01_0040u32, 0xf940_0f82u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_stand_in_add_to_x24() { assert(stand_in_at(seq![frame_word(true, 29u32, 152int), move_word(29u32, 24u32), frame_word(false, 24u32, 136int)], 0x8b01_0018u32, seq![frame_word(true, 24u32, 136int), move_word(24u32, 29u32), frame_word(false, 29u32, 152int)], 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xf900_4f9du32, 0xaa18_03fdu32, 0xf940_4798u32, 0x8b01_0018u32, 0xf900_4798u32, 0xaa1d_03f8u32, 0xf940_4f9du32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_stand_in_mov_x23() { assert(stand_in_at(seq![frame_word(true, 1u32, 24int), frame_word(false, 1u32, 8int)], 0xaa00_03e1u32, seq![frame_word(true, 1u32, 8int), frame_word(false, 1u32, 24int)], 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0xf900_0f81u32, 0xf940_0781u32, 0xaa00_03e1u32, 0xf900_0781u32, 0xf940_0f81u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }

} // verus!
