//! The first proof: form I's piece of one register-only integer instruction (look-ahead none, route Copy, naming no
//! withheld register) is confined. Its words are the translator's (carry.rs: `carry_route` commits the copied word,
//! `exit` builds the next instruction's entry slot in x23 and jumps there): the word, `movz`/`movk`/`movk x23`, `br x23`.
//! What one word does is a hypothesis on `step`, one per word, from the Arm ARM; for the copied word it is what the
//! model's `integer_effect` gives (one general register written, the flags), which the model states only for operands
//! its decode tables give, outside `verus!`.

use super::*;

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

/// The entry slot of guest address `guest` in the window at `base` (way.rs `slot`, `table | guest << 2`).
pub open spec fn slot_of(base: int, guest: int) -> int {
    PROCESS_SLOTS as int + (base + guest) * 4
}

/// The piece of the copied `word` at guest `guest`: the word, then the exit to `guest + 4`.
pub open spec fn copied(word: u32, slot: u64) -> Seq<u32> {
    seq![
        word,
        movz_x23(slot & 0xffff, 0),
        movk_x23((slot >> 16u64) & 0xffff, 1),
        movk_x23((slot >> 32u64) & 0xffff, 2),
        BR_X23,
    ]
}

/// The Arm ARM's meaning of the words of the exit, and of a register-only integer instruction writing `rd` (not x28):
/// one register and the flags, nothing else.
pub open spec fn means(step: spec_fn(Machine, u32) -> Stepped, word: u32, rd: int) -> bool {
    &&& 0 <= rd < 31 && rd != frame_register()
    &&& forall|m: Machine| m.x.len() == 31 ==> {
        let s = #[trigger] step(m, word);
        &&& s.accesses.len() == 0 && !s.system_call && !s.after.ended
        &&& s.after.unlisted == m.unlisted && s.after.pc == (m.pc + 4) as u64
        &&& s.after.x.len() == 31
        &&& forall|r: int| 0 <= r < 31 && r != rd ==> s.after.x[r] == #[trigger] m.x[r]
    }
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
    &&& forall|m: Machine| #[trigger] step(m, BR_X23) == Stepped {
        after: Machine { pc: m.x[23], ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
}

/// The three moves build any address below 2^48.
proof fn lemma_moves_build(slot: u64)
    requires
        slot < 0x1_0000_0000_0000,
    ensures
        kept_half(kept_half((slot & 0xffff) << 0u64, (slot >> 16u64) & 0xffff, 1), (slot >> 32u64) & 0xffff, 2) == slot,
{
    assert(((((((slot & 0xffff) << 0u64) & !(0xffffu64 << 16u64)) | (((slot >> 16u64) & 0xffff) << 16u64))
        & !(0xffffu64 << 32u64)) | (((slot >> 32u64) & 0xffff) << 32u64)) == slot) by (bit_vector)
        requires
            slot < 0x1_0000_0000_0000,
    ;
}

/// The window's base and the guest address as form I has them: slot 1 to 2047, an address under 2^32 whose next is too.
pub open spec fn in_reach(a: Allowed, guest: int) -> bool {
    &&& !a.displaced
    &&& a.base % 0x2_0000_0000 == 0 && 0x2_0000_0000 <= a.base && a.base < 2048 * 0x2_0000_0000
    &&& 0 <= guest && guest + 4 < 0x1_0000_0000
    &&& a.code >= 0 && a.code + 20 <= PROCESS_SLOTS
    &&& a.length == 20
}

/// What the run holds after `n` steps of the piece, from any machine the entry rule allows.
proof fn lemma_run(step: spec_fn(Machine, u32) -> Stepped, word: u32, rd: int, a: Allowed, guest: int, start: Machine, n: nat)
    requires
        means(step, word, rd),
        in_reach(a, guest),
        entered(a, start),
    ensures
        ({
            let slot = slot_of(a.base, guest + 4) as u64;
            let now = run(step, copied(word, slot), a, start, n);
            &&& now.x.len() == 31 && now.x[frame_register()] as int == a.frame
            &&& now.unlisted == start.unlisted && !now.ended
            &&& n <= 4 ==> now.pc as int == a.code + 4 * n
            &&& n == 2 ==> now.x[23] == (slot & 0xffff) << 0u64
            &&& n == 3 ==> now.x[23] == kept_half((slot & 0xffff) << 0u64, (slot >> 16u64) & 0xffff, 1)
            &&& n == 4 ==> now.x[23] == slot
            &&& n >= 5 ==> now.pc == slot
        }),
    decreases n,
{
    let slot = slot_of(a.base, guest + 4) as u64;
    let instructions = copied(word, slot);
    assert(slot_of(a.base, guest + 4) < 0x1_0000_0000_0000);
    if n > 0 {
        lemma_run(step, word, rd, a, guest, start, (n - 1) as nat);
        let before = run(step, instructions, a, start, (n - 1) as nat);
        if n == 1 {
            assert(word_at(instructions, a, before.pc) == word);
        } else if n == 2 {
            assert(word_at(instructions, a, before.pc) == movz_x23(slot & 0xffff, 0));
            assert((slot & 0xffff) < 0x1_0000) by (bit_vector);
        } else if n == 3 {
            assert(word_at(instructions, a, before.pc) == movk_x23((slot >> 16u64) & 0xffff, 1));
            assert(((slot >> 16u64) & 0xffff) < 0x1_0000) by (bit_vector);
        } else if n == 4 {
            assert(word_at(instructions, a, before.pc) == movk_x23((slot >> 32u64) & 0xffff, 2));
            assert(((slot >> 32u64) & 0xffff) < 0x1_0000) by (bit_vector);
            lemma_moves_build(slot);
        } else if n == 5 {
            assert(word_at(instructions, a, before.pc) == BR_X23);
        } else {
            assert(!in_sequence(a, before.pc));
        }
    }
}

/// THE FIRST PROOF: the piece of a register-only integer instruction is confined.
proof fn copied_integer_is_confined(step: spec_fn(Machine, u32) -> Stepped, word: u32, rd: int, a: Allowed, guest: int)
    requires
        means(step, word, rd),
        in_reach(a, guest),
    ensures
        is_confined(step, copied(word, slot_of(a.base, guest + 4) as u64), a),
{
    let slot = slot_of(a.base, guest + 4) as u64;
    let instructions = copied(word, slot);
    assert forall|start: Machine, n: nat|
        #![trigger run(step, instructions, a, start, n)]
        entered(a, start) && runs(a, run(step, instructions, a, start, n)) implies {
            let now = run(step, instructions, a, start, n);
            confined_step(a, now, step(now, word_at(instructions, a, now.pc)))
        } by {
        lemma_run(step, word, rd, a, guest, start, n);
        lemma_run(step, word, rd, a, guest, start, n + 1);
        let now = run(step, instructions, a, start, n);
        let next = run(step, instructions, a, start, n + 1);
        assert(slot_of(a.base, guest + 4) < 0x1_0000_0000_0000);
        assert(n <= 4) by {
            if n >= 5 {
                assert(now.pc == slot);
                assert(!in_sequence(a, now.pc));
            }
        }
        let w = word_at(instructions, a, now.pc);
        if n == 0 {
            assert(w == word);
        } else if n == 1 {
            assert(w == movz_x23(slot & 0xffff, 0));
            assert((slot & 0xffff) < 0x1_0000) by (bit_vector);
        } else if n == 2 {
            assert(w == movk_x23((slot >> 16u64) & 0xffff, 1));
            assert(((slot >> 16u64) & 0xffff) < 0x1_0000) by (bit_vector);
        } else if n == 3 {
            assert(w == movk_x23((slot >> 32u64) & 0xffff, 2));
            assert(((slot >> 32u64) & 0xffff) < 0x1_0000) by (bit_vector);
        } else {
            assert(w == BR_X23);
        }
        let s = step(now, w);
        assert(next == s.after);
        assert(s.accesses.len() == 0 && !s.system_call && s.after.unlisted == now.unlisted);
        if n < 4 {
            assert(in_sequence(a, s.after.pc));
        } else {
            assert(slot_table(a) <= slot as int && (slot as int) < slot_table(a) + SLOT_TABLE_BYTES as int);
            assert(is_exit(a, s.after.pc));
        }
    }
}

} // verus!
