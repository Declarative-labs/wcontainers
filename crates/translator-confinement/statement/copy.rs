//! The first proof: form I's piece of one register-only integer instruction (look-ahead none, route Copy, naming no
//! withheld register) is confined. Its words are the translator's (carry.rs: `carry_route` commits the copied word,
//! `exit` builds the next instruction's entry slot in x23 and jumps there): the word, `movz`/`movk`/`movk x23`, `br x23`.
//! What each word does is a hypothesis on the free `step` (see the crate's documentation): for the copied word, one
//! general register written and the flags, which is what the model's `integer_effect` gives for the operands its decode
//! table (outside `verus!`) finds. `tests/pieces.rs` checks that [`copied`] is what the translator emits, by the pins
//! at the end of this file.

use super::*;
use super::meaning::*;

verus! {

/// `movz x23, #half, lsl #16*k` and `movk x23, #half, lsl #16*k`, and `br x23`.
pub open spec fn movz_x23(half: u64, k: u64) -> u32 {
    (0xd280_0000u32 | ((k as u32) << 21u32) | ((half as u32) << 5u32) | 23u32) as u32
}

pub open spec fn movk_x23(half: u64, k: u64) -> u32 {
    (0xf280_0000u32 | ((k as u32) << 21u32) | ((half as u32) << 5u32) | 23u32) as u32
}

pub const BR_X23: u32 = 0xd61f_02e0;

/// x after `movk` of `half` at 16-bit part `k`.
pub open spec fn kept_half(x: u64, half: u64, k: u64) -> u64 {
    (x & !(0xffffu64 << ((16 * k) as u64))) | (half << ((16 * k) as u64))
}

/// The entry slot of guest address `guest` (way.rs `slot`, `table | guest << 2`): in its window's slot table, or in
/// slot 0's for displaced code.
pub open spec fn exit_slot(a: Allowed, guest: int) -> int {
    slot_table(a) + guest * 4
}

/// The exit to the slot: `movz`/`movk`/`movk x23` building it, `br x23`.
pub open spec fn exit_words(slot: u64) -> Seq<u32> {
    seq![
        movz_x23(slot & 0xffff, 0),
        movk_x23((slot >> 16u64) & 0xffff, 1),
        movk_x23((slot >> 32u64) & 0xffff, 2),
        BR_X23,
    ]
}

/// The piece of the copied `word`: the word, then the exit to the next instruction's slot.
pub open spec fn copied(word: u32, slot: u64) -> Seq<u32> {
    seq![word] + exit_words(slot)
}

/// What the exit's words do (Arm ARM): each move writes x23, `br x23` goes there.
pub open spec fn exit_means(step: spec_fn(Machine, u32) -> Stepped) -> bool {
    &&& forall|m: Machine, half: u64, k: u64| k < 3 && half < 0x1_0000 ==> #[trigger] step(m, movz_x23(half, k)) == Stepped {
        after: Machine { x: m.x.update(23, (half << ((16 * k) as u64)) as u64), pc: (m.pc + 4) as u64, ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
    &&& forall|m: Machine, half: u64, k: u64| k < 3 && half < 0x1_0000 ==> #[trigger] step(m, movk_x23(half, k)) == Stepped {
        after: Machine { x: m.x.update(23, kept_half(m.x[23], half, k)), pc: (m.pc + 4) as u64, ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
    &&& jumps(step, BR_X23, 23)
}

/// What a register-only integer instruction writing `rd` (not x28) does: one register and the flags, nothing else.
pub open spec fn copied_means(step: spec_fn(Machine, u32) -> Stepped, word: u32, rd: int) -> bool {
    &&& 0 <= rd < 31 && rd != frame_register()
    &&& forall|m: Machine| m.x.len() == 31 ==> {
        let s = #[trigger] step(m, word);
        &&& s.accesses.len() == 0 && !s.system_call && !s.after.ended
        &&& s.after.unlisted == m.unlisted && s.after.pc == (m.pc + 4) as u64
        &&& s.after.x.len() == 31
        &&& forall|r: int| 0 <= r < 31 && r != rd ==> s.after.x[r] == #[trigger] m.x[r]
    }
}

/// The three moves build any address below 2^48, each half fitting its move.
pub proof fn lemma_moves_build(slot: u64)
    requires
        slot < 0x1_0000_0000_0000,
    ensures
        kept_half(kept_half((slot & 0xffff) << 0u64, (slot >> 16u64) & 0xffff, 1), (slot >> 32u64) & 0xffff, 2) == slot,
        (slot & 0xffff) < 0x1_0000,
        ((slot >> 16u64) & 0xffff) < 0x1_0000,
        ((slot >> 32u64) & 0xffff) < 0x1_0000,
{
    assert(((((((slot & 0xffff) << 0u64) & !(0xffffu64 << 16u64)) | (((slot >> 16u64) & 0xffff) << 16u64))
        & !(0xffffu64 << 32u64)) | (((slot >> 32u64) & 0xffff) << 32u64)) == slot) by (bit_vector)
        requires
            slot < 0x1_0000_0000_0000,
    ;
    assert((slot & 0xffff) < 0x1_0000) by (bit_vector);
    assert(((slot >> 16u64) & 0xffff) < 0x1_0000) by (bit_vector);
    assert(((slot >> 32u64) & 0xffff) < 0x1_0000) by (bit_vector);
}

/// The window's base and the guest address as form I has them: slot 1 to 2047, at most 64 KiB reserved below the
/// window, an address under 2^32 whose next is too; the instructions below the slot tables and apart from the floor's
/// page.
pub open spec fn in_reach(a: Allowed, guest: int) -> bool {
    &&& a.base % 0x2_0000_0000 == 0 && 0x2_0000_0000 <= a.base && a.base < 2048 * 0x2_0000_0000
    &&& 0 <= a.below <= 0x1_0000
    &&& 0 <= guest && guest + 4 < 0x1_0000_0000
    &&& a.code >= 0 && a.code + a.length <= PROCESS_SLOTS
    &&& (a.code + a.length <= FLOOR_PAGE || a.code >= FLOOR_PAGE + 4096)
}

/// The exit slot is in the slot table, below 2^48.
pub proof fn lemma_exit_slot(a: Allowed, guest: int)
    requires
        in_reach(a, guest),
    ensures
        exit_slot(a, guest + 4) < 0x1_0000_0000_0000,
        slot_table(a) <= exit_slot(a, guest + 4) < slot_table(a) + SLOT_TABLE_BYTES as int,
        is_exit(a, exit_slot(a, guest + 4) as u64),
{
}

/// Where the run of a piece ending in an exit stands at the exit's word `k` (0 to 3) after the copied part.
pub open spec fn at_exit(m: Machine, slot: u64, k: int) -> bool {
    &&& k == 1 ==> m.x[23] == (slot & 0xffff) << 0u64
    &&& k == 2 ==> m.x[23] == kept_half((slot & 0xffff) << 0u64, (slot >> 16u64) & 0xffff, 1)
    &&& k == 3 ==> m.x[23] == slot
}

/// The fact every machine of a run holds: the frame register as the entry rule set it.
pub open spec fn kept(a: Allowed, m: Machine) -> bool {
    &&& m.x.len() == 31
    &&& m.x[frame_register()] as int == a.frame
}

/// The step of exit word `k`, at `exit` bytes into the piece, is confined and keeps [`at_exit`].
pub proof fn lemma_exit_step(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, guest: int, exit: int, m: Machine)
    requires
        exit_means(step),
        in_reach(a, guest),
        a.length == instructions.len() * 4,
        0 <= exit && exit + 16 <= a.length && exit % 4 == 0,
        instructions.subrange(exit / 4, exit / 4 + 4) == exit_words(exit_slot(a, guest + 4) as u64),
        kept(a, m),
        runs(a, m),
        exit <= m.pc - a.code < exit + 16,
        at_exit(m, exit_slot(a, guest + 4) as u64, (m.pc - a.code - exit) / 4),
    ensures
        ({
            let s = step(m, word_at(instructions, a, m.pc));
            let k = (m.pc - a.code - exit) / 4 + 1;
            &&& confined_step(a, m, s)
            &&& kept(a, s.after) && s.after.ended == m.ended && s.after.x[base_register()] == m.x[base_register()]
            &&& s.after.sp == m.sp
            &&& k < 4 ==> s.after.pc == m.pc + 4 && at_exit(s.after, exit_slot(a, guest + 4) as u64, k)
            &&& k == 4 ==> !in_sequence(a, s.after.pc)
        }),
{
    let slot = exit_slot(a, guest + 4) as u64;
    lemma_exit_slot(a, guest);
    lemma_moves_build(slot);
    let k = (m.pc - a.code - exit) / 4;
    let w = word_at(instructions, a, m.pc);
    assert((m.pc - a.code) / 4 == exit / 4 + k);
    assert(w == instructions.subrange(exit / 4, exit / 4 + 4)[k]);
    if k == 0 {
        assert(w == movz_x23(slot & 0xffff, 0));
    } else if k == 1 {
        assert(w == movk_x23((slot >> 16u64) & 0xffff, 1));
    } else if k == 2 {
        assert(w == movk_x23((slot >> 32u64) & 0xffff, 2));
    } else {
        assert(w == BR_X23);
        assert(slot as int >= PROCESS_SLOTS);
    }
}

/// THE FIRST PROOF: the piece of a register-only integer instruction is confined.
pub proof fn copied_integer_is_confined(step: spec_fn(Machine, u32) -> Stepped, word: u32, rd: int, a: Allowed, guest: int)
    requires
        copied_means(step, word, rd),
        exit_means(step),
        in_reach(a, guest),
        a.length == 20,
    ensures
        is_confined(step, copied(word, exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = copied(word, slot);
    let inv = |m: Machine| kept(a, m) && (in_sequence(a, m.pc) && m.pc - a.code >= 4 ==> at_exit(m, slot, (m.pc - a.code - 4) / 4));
    assert(instructions.subrange(1, 5) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        if m.pc - a.code == 0 {
            assert(word_at(instructions, a, m.pc) == word);
        } else {
            lemma_exit_step(step, instructions, a, guest, 4, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The pins: [`copied`] for given words, windows and guest addresses, word by word, which `tests/pieces.rs` checks the
/// translator emits. Each is one line: `pin_*`, the piece's function and its arguments, the words.
pub open spec fn copied_at(word: u32, base: int, guest: int, displaced: bool) -> Seq<u32> {
    copied(word, exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

proof fn pin_copied_add() { assert(copied_at(0x8b02_0020u32, 0x6_0000_0000int, 0x40_1000int, false) =~= seq![0x8b02_0020u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_copied_add_displaced() { assert(copied_at(0x8b02_0020u32, 0x6_0000_0000int, 0x40_1000int, true) =~= seq![0x8b02_0020u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32]) by (compute); }

} // verus!
