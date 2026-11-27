//! Form I's pieces that leave by a branch, in the window's way (look-ahead none; carry.rs `exit`, `jump`,
//! `process_call`):
//!
//! - a direct branch (`b label`): the exit to the target's entry slot;
//! - a system call (`svc #0`): a floor call, `str x30, [x28, #88]`, `movz/movk x30, entry`, `blr x30`, into the
//!   floor's page;
//! - a jump through a register (`br xN`): the progress count, the window check of xN, then its entry slot built in x23
//!   (`add x23, x24, wN, uxtw; ubfiz x23, x23, #2, #44; bfxil x23, x23, #2, #2; eor x23, x23, #2^46; br x23`), or a
//!   floor call when xN is outside the window.
//!
//! Each piece's words after a floor call (`ldr x30, [x28, #88]`, the way back) run only when a run resumes there,
//! which the entry rule does not allow yet (`Entry::Resume`, FOR-THE-NATIVE-TRACK.md): from the first instruction they
//! are never reached, and the invariant says so. What each word does is a hypothesis on the free `step` (see the
//! crate's documentation). `tests/pieces.rs` checks the pieces are the translator's, by the pins at the end.

use super::*;
use super::meaning::*;
use super::copy::*;
use super::access::*;

verus! {

/// `movz x30, #half, lsl #16*k`, `movk x30, ...`, `blr x30`, and the frame's word that keeps the guest's x30 meanwhile.
pub open spec fn movz_x30(half: u64, k: u64) -> u32 {
    (0xd280_001eu32 | ((k as u32) << 21u32) | ((half as u32) << 5u32)) as u32
}

pub open spec fn movk_x30(half: u64, k: u64) -> u32 {
    (0xf280_001eu32 | ((k as u32) << 21u32) | ((half as u32) << 5u32)) as u32
}

pub const BLR_X30: u32 = 0xd63f_03c0;
pub const STR_X30_FRAME: u32 = 0xf900_2f9e;
pub const LDR_X30_FRAME: u32 = 0xf940_2f9e;

/// The floor's entry of call `code`.
pub open spec fn floor_entry(code: int) -> u64 {
    (FLOOR_PAGE as int + 16 * code) as u64
}

/// A floor call: the guest's x30 kept in the frame, the entry built in x30, `blr x30`.
pub open spec fn floor_call(entry: u64) -> Seq<u32> {
    seq![STR_X30_FRAME, movz_x30(entry & 0xffff, 0), movk_x30((entry >> 16u64) & 0xffff, 1), movk_x30((entry >> 32u64) & 0xffff, 2), BLR_X30]
}

/// What the floor call's words do (Arm ARM): the store of x30 to the frame, the moves, `blr x30` (x30 the next
/// word's address, then there).
pub open spec fn floor_means(step: spec_fn(Machine, u32) -> Stepped) -> bool {
    &&& accesses(step, STR_X30_FRAME, 31, |m: Machine| (m.x[28] + 88) % two_to_64(), 8)
    &&& forall|m: Machine, half: u64, k: u64| k < 3 && half < 0x1_0000 ==> #[trigger] step(m, movz_x30(half, k)) == Stepped {
        after: Machine { x: m.x.update(30, (half << ((16 * k) as u64)) as u64), pc: (m.pc + 4) as u64, ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
    &&& forall|m: Machine, half: u64, k: u64| k < 3 && half < 0x1_0000 ==> #[trigger] step(m, movk_x30(half, k)) == Stepped {
        after: Machine { x: m.x.update(30, kept_half(m.x[30], half, k)), pc: (m.pc + 4) as u64, ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
    &&& forall|m: Machine| #[trigger] step(m, BLR_X30) == Stepped {
        after: Machine { x: m.x.update(30, (m.pc + 4) as u64), pc: m.x[30], ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
}

/// Where the run stands at word `k` (0 to 4) of a floor call to `entry`.
pub open spec fn at_floor_call(m: Machine, entry: u64, k: int) -> bool {
    &&& k == 2 ==> m.x[30] == (entry & 0xffff) << 0u64
    &&& k == 3 ==> m.x[30] == kept_half((entry & 0xffff) << 0u64, (entry >> 16u64) & 0xffff, 1)
    &&& k == 4 ==> m.x[30] == entry
}

/// The frame where its words can be addressed.
pub open spec fn frame_in_reach(a: Allowed) -> bool {
    0 <= a.frame && a.frame + FRAME_BYTES < 0x1_0000_0000_0000_0000int
}

/// The step of floor-call word `k` at `call` bytes into the piece is confined and keeps [`at_floor_call`].
pub proof fn lemma_floor_step(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, code: int, call: int, m: Machine)
    requires
        floor_means(step),
        0 <= code < 256,
        frame_in_reach(a),
        a.length == instructions.len() * 4,
        0 <= call && call + 20 <= a.length && call % 4 == 0,
        a.code >= 0 && a.code + a.length <= PROCESS_SLOTS,
        a.code + a.length <= FLOOR_PAGE || a.code >= FLOOR_PAGE + 4096,
        instructions.subrange(call / 4, call / 4 + 5) == floor_call(floor_entry(code)),
        kept(a, m),
        runs(a, m),
        call <= m.pc - a.code < call + 20,
        at_floor_call(m, floor_entry(code), (m.pc - a.code - call) / 4),
    ensures
        ({
            let s = step(m, word_at(instructions, a, m.pc));
            let k = (m.pc - a.code - call) / 4 + 1;
            &&& confined_step(a, m, s)
            &&& kept(a, s.after) && s.after.x[base_register()] == m.x[base_register()]
            &&& k < 5 ==> s.after.ended || (s.after.pc == m.pc + 4 && at_floor_call(s.after, floor_entry(code), k))
            &&& k == 5 ==> !in_sequence(a, s.after.pc)
        }),
{
    let entry = floor_entry(code);
    lemma_moves_build(entry);
    let k = (m.pc - a.code - call) / 4;
    let w = word_at(instructions, a, m.pc);
    assert((m.pc - a.code) / 4 == call / 4 + k);
    assert(w == instructions.subrange(call / 4, call / 4 + 5)[k]);
    if k == 0 {
        assert(w == STR_X30_FRAME);
        assert((m.x[28] + 88) % two_to_64() == m.x[28] + 88);
    } else if k == 1 {
        assert(w == movz_x30(entry & 0xffff, 0));
    } else if k == 2 {
        assert(w == movk_x30((entry >> 16u64) & 0xffff, 1));
    } else if k == 3 {
        assert(w == movk_x30((entry >> 32u64) & 0xffff, 2));
    } else {
        assert(w == BLR_X30);
        assert(is_exit(a, entry));
    }
}

/// THE DIRECT BRANCH'S PIECE IS CONFINED: the exit to its target.
pub proof fn direct_branch_is_confined(step: spec_fn(Machine, u32) -> Stepped, a: Allowed, target: int)
    requires
        exit_means(step),
        in_reach(a, target - 4),
        a.length == 16,
    ensures
        is_confined(step, exit_words(exit_slot(a, target) as u64), a),
{
    let guest = target - 4;
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = exit_words(slot);
    let inv = |m: Machine| kept(a, m) && (in_sequence(a, m.pc) ==> at_exit(m, slot, (m.pc - a.code) / 4));
    assert(instructions.subrange(0, 4) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        lemma_exit_step(step, instructions, a, guest, 0, m);
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// THE BACKWARD BRANCH'S PIECE IS CONFINED: the progress count, then the exit to its target.
pub proof fn backward_branch_is_confined(step: spec_fn(Machine, u32) -> Stepped, a: Allowed, target: int)
    requires
        writes(step, COUNT_UP, 18, |m: Machine| ((m.x[18] + 1) % two_to_64()) as u64),
        exit_means(step),
        in_reach(a, target - 4),
        a.length == 20,
    ensures
        is_confined(step, seq![COUNT_UP] + exit_words(exit_slot(a, target) as u64), a),
{
    let guest = target - 4;
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = seq![COUNT_UP] + exit_words(slot);
    let inv = |m: Machine| kept(a, m) && (in_sequence(a, m.pc) && m.pc - a.code >= 4 ==> at_exit(m, slot, (m.pc - a.code - 4) / 4));
    assert(instructions.subrange(1, 5) =~= exit_words(slot));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        if m.pc - a.code == 0 {
            assert(word_at(instructions, a, m.pc) == COUNT_UP);
        } else {
            lemma_exit_step(step, instructions, a, guest, 4, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// `b .+20`, `b .+4` and the ways back after a floor call, `b .-40` and `b .-24`.
pub const B_20: u32 = 0x1400_0005;
pub const B_4: u32 = 0x1400_0001;
pub const B_BACK_40: u32 = 0x17ff_fff6;
pub const B_BACK_24: u32 = 0x17ff_fffa;

/// The system call's piece: to the floor call, the exit to the next instruction (after the call), the floor call, the
/// guest's x30 back, the way to the exit.
pub open spec fn system_call(slot: u64) -> Seq<u32> {
    seq![B_20] + exit_words(slot) + floor_call(floor_entry(2)) + seq![LDR_X30_FRAME, B_BACK_40]
}

/// THE SYSTEM CALL'S PIECE IS CONFINED: from its first word it reaches the floor, which ends the run.
pub proof fn system_call_is_confined(step: spec_fn(Machine, u32) -> Stepped, a: Allowed, guest: int)
    requires
        branches(step, B_20, 20),
        floor_means(step),
        exit_means(step),
        in_reach(a, guest),
        frame_in_reach(a),
        a.length == 48,
    ensures
        is_confined(step, system_call(exit_slot(a, guest + 4) as u64), a),
{
    let slot = exit_slot(a, guest + 4) as u64;
    let instructions = system_call(slot);
    let entry = floor_entry(2);
    let inv = |m: Machine| {
        let o = m.pc - a.code;
        &&& kept(a, m)
        &&& (o == 0 || 20 <= o < 40)
        &&& 20 <= o < 40 ==> at_floor_call(m, entry, (o - 20) / 4)
    };
    assert(instructions.subrange(5, 10) =~= floor_call(entry));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        if m.pc - a.code == 0 {
            assert(word_at(instructions, a, m.pc) == B_20);
        } else {
            lemma_floor_step(step, instructions, a, 2, 20, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// `add x18, x18, #1` (the progress count), `ubfx x23, xN, #32, #24`, `cbnz x23, .+24`, `ubfiz x23, x23, #2, #44`,
/// `bfxil x23, x23, #2, #2`, `eor x23, x23, #0x400000000000`.
pub const COUNT_UP: u32 = 0x9100_0652;
pub open spec fn check_x23(n: u32) -> u32 {
    (0xd360_dc17u32 | (n << 5u32)) as u32
}
pub const CBNZ_X23_24: u32 = 0xb500_00d7;
pub const UBFIZ_X23: u32 = 0xd37e_aef7;
pub const BFXIL_X23: u32 = 0xb342_0ef7;
pub const EOR_X23_TABLE: u32 = 0xd252_02f7;

/// The jump's piece: 16 words.
pub open spec fn jump(n: u32) -> Seq<u32> {
    seq![COUNT_UP, check_x23(n), CBNZ_X23_24, add_cut(n), UBFIZ_X23, BFXIL_X23, EOR_X23_TABLE, BR_X23, B_4]
        + floor_call(floor_entry(7)) + seq![LDR_X30_FRAME, B_BACK_24]
}

/// What the jump's words do (Arm ARM).
pub open spec fn jump_means(step: spec_fn(Machine, u32) -> Stepped, n: u32) -> bool {
    &&& writes(step, COUNT_UP, 18, |m: Machine| ((m.x[18] + 1) % two_to_64()) as u64)
    &&& writes(step, check_x23(n), 23, |m: Machine| (m.x[n as int] >> 32u64) & 0xff_ffffu64)
    &&& branches_if(step, CBNZ_X23_24, 23, 24, true)
    &&& writes(step, add_cut(n), 23, |m: Machine| ((m.x[24] + (m.x[n as int] & 0xffff_ffffu64)) % two_to_64()) as u64)
    &&& writes(step, UBFIZ_X23, 23, |m: Machine| (m.x[23] & 0xfff_ffff_ffffu64) << 2u64)
    &&& writes(step, BFXIL_X23, 23, |m: Machine| (m.x[23] & !3u64) | ((m.x[23] >> 2u64) & 3u64))
    &&& writes(step, EOR_X23_TABLE, 23, |m: Machine| m.x[23] ^ 0x4000_0000_0000u64)
    &&& jumps(step, BR_X23, 23)
    &&& branches(step, B_4, 4)
    &&& floor_means(step)
}

/// The entry slot's address made from a cut address below 2^44: four times it, plus at most 3, in the slot tables.
pub proof fn lemma_slot_address(v: u64)
    requires
        v < 0x1000_0000_0000u64,
    ensures
        (v & 0xfff_ffff_ffffu64) << 2u64 == v * 4,
        ({
            let u = (v * 4) as u64;
            let w = (u & !3u64) | ((u >> 2u64) & 3u64);
            &&& u <= w && w <= u + 3 && w < 0x4000_0000_0000u64
            &&& w ^ 0x4000_0000_0000u64 == w + 0x4000_0000_0000u64
        }),
{
    assert((v & 0xfff_ffff_ffffu64) << 2u64 == v * 4) by (bit_vector)
        requires
            v < 0x1000_0000_0000u64,
    ;
    let u = (v * 4) as u64;
    assert(u & 3u64 == 0 && u < 0x4000_0000_0000u64) by (bit_vector)
        requires
            v < 0x1000_0000_0000u64,
            u == v * 4,
    ;
    let w = (u & !3u64) | ((u >> 2u64) & 3u64);
    assert(u <= w && w <= u + 3 && w < 0x4000_0000_0000u64) by (bit_vector)
        requires
            u & 3u64 == 0,
            u < 0x4000_0000_0000u64,
            w == (u & !3u64) | ((u >> 2u64) & 3u64),
    ;
    assert(w ^ 0x4000_0000_0000u64 == w + 0x4000_0000_0000u64) by (bit_vector)
        requires
            w < 0x4000_0000_0000u64,
    ;
}

/// Where the jump's run stands: the count and the cut address as the words make them, the floor call's moves; the
/// words after the floor call never.
pub open spec fn jump_inv(a: Allowed, m: Machine) -> bool {
    let o = m.pc - a.code;
    &&& kept(a, m) && m.x[base_register()] as int == a.base
    &&& o < 56
    &&& o == 16 ==> in_window(a, m.x[23])
    &&& o == 20 ==> a.base * 4 <= m.x[23] && m.x[23] < a.base * 4 + 0x4_0000_0000 && m.x[23] % 4 == 0
    &&& o == 24 ==> a.base * 4 <= m.x[23] && m.x[23] < a.base * 4 + 0x4_0000_0003 && m.x[23] < 0x4000_0000_0000
    &&& o == 28 ==> slot_table(a) <= m.x[23] && m.x[23] < slot_table(a) + 0x4_0000_0003
    &&& 36 <= o < 56 ==> at_floor_call(m, floor_entry(7), (o - 36) / 4)
}

/// THE JUMP'S PIECE IS CONFINED: it lands in its window's slot table or in the floor.
pub proof fn jump_is_confined(step: spec_fn(Machine, u32) -> Stepped, n: u32, a: Allowed, guest: int)
    requires
        n < 31,
        jump_means(step, n),
        in_reach(a, guest) && !a.displaced,
        frame_in_reach(a),
        a.length == 64,
    ensures
        is_confined(step, jump(n), a),
{
    let instructions = jump(n);
    let inv = |m: Machine| jump_inv(a, m);
    assert(instructions.len() == 16);
    assert(instructions.subrange(9, 14) =~= floor_call(floor_entry(7)));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let w = word_at(instructions, a, m.pc);
        if o == 0 {
            assert(w == COUNT_UP);
        } else if o == 4 {
            assert(w == check_x23(n));
        } else if o == 8 {
            assert(w == CBNZ_X23_24);
        } else if o == 12 {
            assert(w == add_cut(n));
            lemma_low_word(m.x[n as int]);
        } else if o == 16 {
            assert(w == UBFIZ_X23);
            lemma_slot_address(m.x[23]);
        } else if o == 20 {
            assert(w == BFXIL_X23);
            let v = (m.x[23] / 4) as u64;
            assert(m.x[23] == v * 4);
            lemma_slot_address(v);
        } else if o == 24 {
            assert(w == EOR_X23_TABLE);
            let u = m.x[23];
            assert(u ^ 0x4000_0000_0000u64 == u + 0x4000_0000_0000u64) by (bit_vector)
                requires
                    u < 0x4000_0000_0000u64,
            ;
        } else if o == 28 {
            assert(w == BR_X23);
        } else if o == 32 {
            assert(w == B_4);
        } else {
            lemma_floor_step(step, instructions, a, 7, 36, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// `b .-4`: a branch to itself, linked inside its piece (the exit's first move overwritten by it).
pub const B_BACK_4: u32 = 0x17ff_ffff;

/// The piece of `b .` (a branch to itself): the progress count, the branch back to it, and what the link left of the
/// exit to the instruction's own slot (never reached).
pub open spec fn self_loop(slot: u64) -> Seq<u32> {
    seq![COUNT_UP, B_BACK_4] + exit_words(slot).subrange(1, 4)
}

/// THE BRANCH TO ITSELF IS CONFINED: the count and the branch back, for ever.
pub proof fn self_loop_is_confined(step: spec_fn(Machine, u32) -> Stepped, a: Allowed, guest: int)
    requires
        writes(step, COUNT_UP, 18, |m: Machine| ((m.x[18] + 1) % two_to_64()) as u64),
        branches(step, B_BACK_4, -4),
        in_reach(a, guest),
        a.length == 20,
    ensures
        is_confined(step, self_loop(exit_slot(a, guest) as u64), a),
{
    let slot = exit_slot(a, guest) as u64;
    let instructions = self_loop(slot);
    let inv = |m: Machine| kept(a, m) && m.pc - a.code < 8;
    assert(instructions.len() == 5);
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        if m.pc - a.code == 0 {
            assert(word_at(instructions, a, m.pc) == COUNT_UP);
        } else {
            assert(word_at(instructions, a, m.pc) == B_BACK_4);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// `ubfiz x23, xN, #2, #32`: the displaced jump's entry slot, four times the low word of xN.
pub open spec fn ubfiz_x23(n: u32) -> u32 {
    (0xd37e_7c17u32 | (n << 5u32)) as u32
}

/// The displaced jump's piece (slot 0's table): 15 words, xN's low word made the slot with no cut.
pub open spec fn displaced_jump(n: u32) -> Seq<u32> {
    seq![COUNT_UP, check_x23(n), CBNZ_X23_20, ubfiz_x23(n), BFXIL_X23, EOR_X23_TABLE, BR_X23, B_4]
        + floor_call(floor_entry(7)) + seq![LDR_X30_FRAME, B_BACK_24]
}

/// `cbnz x23, .+20`.
pub const CBNZ_X23_20: u32 = 0xb500_00b7;

/// What the displaced jump's words do (Arm ARM).
pub open spec fn displaced_jump_means(step: spec_fn(Machine, u32) -> Stepped, n: u32) -> bool {
    &&& writes(step, COUNT_UP, 18, |m: Machine| ((m.x[18] + 1) % two_to_64()) as u64)
    &&& writes(step, check_x23(n), 23, |m: Machine| (m.x[n as int] >> 32u64) & 0xff_ffffu64)
    &&& branches_if(step, CBNZ_X23_20, 23, 20, true)
    &&& writes(step, ubfiz_x23(n), 23, |m: Machine| (m.x[n as int] & 0xffff_ffffu64) << 2u64)
    &&& writes(step, BFXIL_X23, 23, |m: Machine| (m.x[23] & !3u64) | ((m.x[23] >> 2u64) & 3u64))
    &&& writes(step, EOR_X23_TABLE, 23, |m: Machine| m.x[23] ^ 0x4000_0000_0000u64)
    &&& jumps(step, BR_X23, 23)
    &&& branches(step, B_4, 4)
    &&& floor_means(step)
}

/// Four times a low word is below 2^34 and a multiple of 4.
proof fn lemma_low_word_times_four(v: u64)
    ensures
        ((v & 0xffff_ffffu64) << 2u64) < 0x4_0000_0000u64,
        ((v & 0xffff_ffffu64) << 2u64) & 3u64 == 0,
{
    assert(((v & 0xffff_ffffu64) << 2u64) < 0x4_0000_0000u64) by (bit_vector);
    assert(((v & 0xffff_ffffu64) << 2u64) & 3u64 == 0) by (bit_vector);
}

/// Where the displaced jump's run stands.
pub open spec fn displaced_jump_inv(a: Allowed, m: Machine) -> bool {
    let o = m.pc - a.code;
    &&& kept(a, m)
    &&& o < 52
    &&& o == 16 ==> m.x[23] < 0x4_0000_0000u64 && m.x[23] & 3u64 == 0
    &&& o == 20 ==> m.x[23] < 0x4_0000_0003
    &&& o == 24 ==> PROCESS_SLOTS <= m.x[23] && m.x[23] < PROCESS_SLOTS + 0x4_0000_0003
    &&& 32 <= o < 52 ==> at_floor_call(m, floor_entry(7), (o - 32) / 4)
}

/// THE DISPLACED JUMP'S PIECE IS CONFINED: it lands in slot 0's table or in the floor.
pub proof fn displaced_jump_is_confined(step: spec_fn(Machine, u32) -> Stepped, n: u32, a: Allowed, guest: int)
    requires
        n < 31,
        displaced_jump_means(step, n),
        in_reach(a, guest) && a.displaced,
        frame_in_reach(a),
        a.length == 60,
    ensures
        is_confined(step, displaced_jump(n), a),
{
    let instructions = displaced_jump(n);
    let inv = |m: Machine| displaced_jump_inv(a, m);
    assert(instructions.len() == 15);
    assert(instructions.subrange(8, 13) =~= floor_call(floor_entry(7)));
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let o = m.pc - a.code;
        let w = word_at(instructions, a, m.pc);
        if o == 0 {
            assert(w == COUNT_UP);
        } else if o == 4 {
            assert(w == check_x23(n));
        } else if o == 8 {
            assert(w == CBNZ_X23_20);
        } else if o == 12 {
            assert(w == ubfiz_x23(n));
            lemma_low_word_times_four(m.x[n as int]);
        } else if o == 16 {
            assert(w == BFXIL_X23);
            let u = m.x[23];
            assert(u <= ((u & !3u64) | ((u >> 2u64) & 3u64)) && ((u & !3u64) | ((u >> 2u64) & 3u64)) <= u + 3) by (bit_vector)
                requires
                    u & 3u64 == 0,
                    u < 0x4_0000_0000u64,
            ;
        } else if o == 20 {
            assert(w == EOR_X23_TABLE);
            let u = m.x[23];
            assert(u ^ 0x4000_0000_0000u64 == u + 0x4000_0000_0000u64) by (bit_vector)
                requires
                    u < 0x4_0000_0003u64,
            ;
        } else if o == 24 {
            assert(w == BR_X23);
        } else if o == 28 {
            assert(w == B_4);
        } else {
            lemma_floor_step(step, instructions, a, 7, 32, m);
        }
    }
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The pins (see copy.rs): one line each, the piece's function and its arguments, the words.
pub open spec fn self_loop_at(base: int, guest: int, displaced: bool) -> Seq<u32> {
    self_loop(exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest) as u64)
}

pub open spec fn exit_words_at(target: int, base: int, displaced: bool) -> Seq<u32> {
    exit_words(exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, target) as u64)
}

pub open spec fn system_call_at(base: int, guest: int, displaced: bool) -> Seq<u32> {
    system_call(exit_slot(Allowed { base, below: 0, frame: 0, code: 0, length: 0, displaced }, guest + 4) as u64)
}

proof fn pin_branch_forward() { assert(exit_words_at(0x40_1008int, 0x6_0000_0000int, false) =~= seq![0xd288_0417u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_branch_displaced() { assert(exit_words_at(0x40_1008int, 0x6_0000_0000int, true) =~= seq![0xd288_0417u32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_system_call() { assert(system_call_at(0x6_0000_0000int, 0x40_1000int, false) =~= seq![0x1400_0005u32, 0xd288_0217u32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32, 0xf900_2f9eu32, 0xd280_041eu32, 0xf2a0_001eu32, 0xf2c4_003eu32, 0xd63f_03c0u32, 0xf940_2f9eu32, 0x17ff_fff6u32]) by (compute); }
proof fn pin_self_loop() { assert(self_loop_at(0x6_0000_0000int, 0x40_1000int, false) =~= seq![0x9100_0652u32, 0x17ff_ffffu32, 0xf2a0_2017u32, 0xf2c8_0317u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_self_loop_displaced() { assert(self_loop_at(0x6_0000_0000int, 0x40_1000int, true) =~= seq![0x9100_0652u32, 0x17ff_ffffu32, 0xf2a0_2017u32, 0xf2c8_0017u32, 0xd61f_02e0u32]) by (compute); }
proof fn pin_displaced_jump_x0() { assert(displaced_jump(0u32) =~= seq![0x9100_0652u32, 0xd360_dc17u32, 0xb500_00b7u32, 0xd37e_7c17u32, 0xb342_0ef7u32, 0xd252_02f7u32, 0xd61f_02e0u32, 0x1400_0001u32, 0xf900_2f9eu32, 0xd280_0e1eu32, 0xf2a0_001eu32, 0xf2c4_003eu32, 0xd63f_03c0u32, 0xf940_2f9eu32, 0x17ff_fffau32]) by (compute); }
proof fn pin_jump_x1() { assert(jump(1u32) =~= seq![0x9100_0652u32, 0xd360_dc37u32, 0xb500_00d7u32, 0x8b21_4317u32, 0xd37e_aef7u32, 0xb342_0ef7u32, 0xd252_02f7u32, 0xd61f_02e0u32, 0x1400_0001u32, 0xf900_2f9eu32, 0xd280_0e1eu32, 0xf2a0_001eu32, 0xf2c4_003eu32, 0xd63f_03c0u32, 0xf940_2f9eu32, 0x17ff_fffau32]) by (compute); }

} // verus!
