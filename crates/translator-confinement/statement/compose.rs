//! THE COMPOSITION AND THE CHAIN: a piece is confined when it is described, word by word, by blocks that each hold
//! alone; every exit of such a piece hands on what the next piece's entry needs; so every step of a whole run of such
//! pieces is confined.
//!
//! A description is a list of [`Desc`]: each names the words one block owns, where it is entered and what it needs
//! there, as one of the block kinds form I's pieces are made of. What a block needs where it is entered is the part's
//! fact ([`part`]): the frame in x28, the window's base in a named register `bb` (x24, or the stand-in's while x24 is
//! the guest's), the stack pointer within [`STACK_SLACK`] of the window unless displaced, and for some a window address
//! in a named register `cut` (x23 after `add x23, x24, wN, uxtw`, or another; 31 for none).
//!
//! [`described`] is decidable over the words (each block's shape, and that every place a block goes on to is where a
//! block of a matching need is entered); [`all_meant`] is what the words do. [`described_is_chained`] proves that the
//! two make the piece chained ([`is_chained`]: confined from every machine a run hands on, and handing on at every
//! exit into the slot table), and [`whole_run_is_confined`] that a whole run of chained pieces is confined.
//!
//! WHAT THIS STILL ASSUMES. (1) What each word does: [`all_meant`] is a hypothesis on the free `step`, and for the
//! words whose shape the description does not fix (a register-only word, a branch, an access, a frame access, the
//! stack pointer's step) the meaning the Rust decomposer gives them comes from `decode.rs`, not from Arm's
//! pseudocode: until `step` is bound to the instruction-proof model, every figure below is only as sound as decode.rs.
//! (2) The memory map: an access through the stack pointer that moves it, and the probe after a stack step, end the run
//! when they touch the slot's never mapped bytes (in [`stack_touches`] with `faults`). (3) The slot table: an exit into
//! it goes on at the first word of the piece of its guest address with every register kept ([`whole_run`]); a floor
//! call ends the run, and a run the floor resumes in the middle of a piece is not covered. (4) The pieces: only
//! described pieces are covered.
//!
//! WHAT IS DESCRIBED (measured by `tests/compose.rs` over 8 drawn words of each of the 4,429 forms, look-ahead none;
//! each figure rests on (1): the meaning of each word is a hypothesis taken from decode.rs, and `step` is still free):
//! in the window 35,424 of 35,432 pieces, every piece of 4,422 forms; displaced 35,426 of 35,432, every piece of 4,425.
//! The rest: jumps through a register the run loads from the frame, and accesses through x23 holding no window
//! address in these blocks.

use super::*;
use super::meaning::*;
use super::copy::*;
use super::access::*;
use super::branch::*;
use super::withheld::*;
use super::withheld_access::*;

verus! {

/// A block: the words it owns, the word where it is entered, what it needs there, and where its run stands inside it.
pub struct Block {
    pub owns: spec_fn(int) -> bool,
    pub entry: int,
    pub needs: spec_fn(Machine) -> bool,
    pub inv: spec_fn(Machine) -> bool,
}

/// The word of the piece the machine is at.
pub open spec fn index(a: Allowed, pc: u64) -> int {
    (pc as int - a.code) / 4
}

/// The run is where some block is entered, with what that block needs.
pub open spec fn enters(blocks: Seq<Block>, a: Allowed, m: Machine) -> bool {
    exists|c: int| 0 <= c < blocks.len() && blocks[c].entry == index(a, m.pc) && #[trigger] (blocks[c].needs)(m)
}

/// The step from `m` inside block `b` is confined, the run goes on inside the block or enters one, and a step into
/// the slot table hands on.
pub open spec fn steps_within(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, blocks: Seq<Block>, b: int, m: Machine) -> bool {
    let k = blocks[b];
    let s = step(m, word_at(instructions, a, m.pc));
    &&& confined_step(a, m, s)
    &&& runs(a, s.after) ==> ((k.owns)(index(a, s.after.pc)) && (k.inv)(s.after)) || enters(blocks, a, s.after)
    &&& !s.after.ended && in_table(a, s.after.pc) ==> hands_on(a, s.after)
}

/// Block `b` holds alone: it owns its entry, what it needs there gives its invariant, and each step from where its
/// invariant holds is confined and goes on inside it or into a block.
pub open spec fn block_holds(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, blocks: Seq<Block>, b: int) -> bool {
    let k = blocks[b];
    &&& (k.owns)(k.entry)
    &&& forall|m: Machine| runs(a, m) && index(a, m.pc) == k.entry && #[trigger] (k.needs)(m) ==> (k.inv)(m)
    &&& forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) ==> steps_within(step, instructions, a, blocks, b, m)
}

/// Some block owns the word the run is at, and its invariant holds.
pub open spec fn somewhere(blocks: Seq<Block>, a: Allowed, m: Machine) -> bool {
    exists|b: int| 0 <= b < blocks.len() && (blocks[b].owns)(index(a, m.pc)) && #[trigger] (blocks[b].inv)(m)
}

/// An invariant that every machine the chain's entry rule allows meets and every step keeps holds after any steps.
proof fn lemma_chain_holds(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    inv: spec_fn(Machine) -> bool,
    start: Machine,
    n: nat,
)
    requires
        forall|m: Machine| handed(a, m) ==> #[trigger] inv(m),
        kept_by_every_step(step, instructions, a, inv),
        handed(a, start),
    ensures
        runs(a, run(step, instructions, a, start, n)) ==> inv(run(step, instructions, a, start, n)),
    decreases n,
{
    if n > 0 {
        lemma_chain_holds(step, instructions, a, inv, start, (n - 1) as nat);
        let before = run(step, instructions, a, start, (n - 1) as nat);
        if runs(a, before) {
            assert(inv(before));
        }
    } else {
        assert(inv(start));
    }
}

/// THE RULE OF BLOCKS: if the first block is entered at the piece's first word with what a run hands on, and every
/// block holds alone, the piece is chained.
pub proof fn lemma_blocks_chain(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, blocks: Seq<Block>)
    requires
        a.length == instructions.len() * 4,
        instructions.len() > 0,
        blocks.len() > 0 && blocks[0].entry == 0,
        forall|m: Machine| handed(a, m) ==> #[trigger] (blocks[0].needs)(m),
        forall|b: int| 0 <= b < blocks.len() ==> #[trigger] block_holds(step, instructions, a, blocks, b),
    ensures
        is_chained(step, instructions, a),
{
    let inv = |m: Machine| somewhere(blocks, a, m);
    assert forall|m: Machine| handed(a, m) implies #[trigger] inv(m) by {
        assert(block_holds(step, instructions, a, blocks, 0));
        assert((blocks[0].needs)(m));
        assert(runs(a, m));
        assert(index(a, m.pc) == 0);
        assert((blocks[0].inv)(m));
    }
    assert forall|m: Machine| #[trigger] inv(m) && runs(a, m) implies {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    } by {
        let b = choose|b: int| 0 <= b < blocks.len() && (blocks[b].owns)(index(a, m.pc)) && #[trigger] (blocks[b].inv)(m);
        assert(block_holds(step, instructions, a, blocks, b));
        assert(steps_within(step, instructions, a, blocks, b, m));
        let s = step(m, word_at(instructions, a, m.pc));
        if runs(a, s.after) {
            if !((blocks[b].owns)(index(a, s.after.pc)) && (blocks[b].inv)(s.after)) {
                let c = choose|c: int| 0 <= c < blocks.len() && blocks[c].entry == index(a, s.after.pc) && #[trigger] (blocks[c].needs)(s.after);
                assert(block_holds(step, instructions, a, blocks, c));
                assert((blocks[c].inv)(s.after));
            }
        }
    }
    assert forall|start: Machine, n: nat|
        #![trigger run(step, instructions, a, start, n)]
        handed(a, start) && runs(a, run(step, instructions, a, start, n)) implies {
            let now = run(step, instructions, a, start, n);
            let s = step(now, word_at(instructions, a, now.pc));
            &&& confined_step(a, now, s)
            &&& !s.after.ended && in_table(a, s.after.pc) ==> hands_on(a, s.after)
        } by {
        lemma_chain_holds(step, instructions, a, inv, start, n);
        let now = run(step, instructions, a, start, n);
        assert(inv(now));
        let b = choose|b: int| 0 <= b < blocks.len() && (blocks[b].owns)(index(a, now.pc)) && #[trigger] (blocks[b].inv)(now);
        assert(block_holds(step, instructions, a, blocks, b));
        assert(steps_within(step, instructions, a, blocks, b, now));
    }
}

/// A chained piece is confined: every machine the entry rule allows is one a run hands on.
pub proof fn lemma_chained_confines(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed)
    requires
        is_chained(step, instructions, a),
    ensures
        is_confined(step, instructions, a),
{
    assert forall|start: Machine, n: nat|
        #![trigger run(step, instructions, a, start, n)]
        entered(a, start) && runs(a, run(step, instructions, a, start, n)) implies {
            let now = run(step, instructions, a, start, n);
            confined_step(a, now, step(now, word_at(instructions, a, now.pc)))
        } by {
        assert(handed(a, start));
    }
}

/// The run of a whole program so far is a run of its present piece from a machine some run handed on.
proof fn lemma_whole_segment(
    step: spec_fn(Machine, u32) -> Stepped,
    pieces: Seq<Piece>,
    link: spec_fn(u64) -> int,
    k: int,
    start: Machine,
    n: nat,
) -> (r: (Machine, nat))
    requires
        0 <= k < pieces.len(),
        one_window(pieces, pieces[k].a),
        forall|i: int| 0 <= i < pieces.len() ==> #[trigger] is_chained(step, pieces[i].instructions, pieces[i].a),
        forall|pc: u64| 0 <= #[trigger] link(pc) < pieces.len(),
        handed(pieces[k].a, start),
    ensures
        ({
            let w = whole_run(step, pieces, link, k, start, n);
            &&& 0 <= w.0 < pieces.len()
            &&& handed(pieces[w.0].a, r.0)
            &&& w.1 == run(step, pieces[w.0].instructions, pieces[w.0].a, r.0, r.1)
        }),
    decreases n,
{
    if n == 0 {
        (start, 0)
    } else {
        let (s0, n0) = lemma_whole_segment(step, pieces, link, k, start, (n - 1) as nat);
        let before = whole_run(step, pieces, link, k, start, (n - 1) as nat);
        let i = before.0;
        let p = pieces[i];
        assert(run(step, p.instructions, p.a, s0, n0 + 1) == if runs(p.a, before.1) {
            step(before.1, word_at(p.instructions, p.a, before.1.pc)).after
        } else {
            before.1
        });
        if runs(p.a, before.1) {
            let s = step(before.1, word_at(p.instructions, p.a, before.1.pc));
            let after = s.after;
            if !after.ended && in_table(p.a, after.pc) {
                assert(is_chained(step, p.instructions, p.a));
                assert(hands_on(p.a, after));
                let next = link(after.pc);
                let q = pieces[next].a;
                assert(q.base == pieces[k].a.base && q.frame == pieces[k].a.frame && q.displaced == pieces[k].a.displaced);
                assert(0 <= q.code <= PROCESS_SLOTS);
                let s1 = Machine { pc: q.code as u64, ..after };
                assert(handed(q, s1));
                assert(run(step, pieces[next].instructions, q, s1, 0) == s1);
                (s1, 0)
            } else {
                (s0, n0 + 1)
            }
        } else {
            (s0, n0 + 1)
        }
    }
}

/// THE CHAIN THEOREM: in a whole run of chained pieces of one window, from any machine a run hands on to any piece,
/// every step is confined in the piece it is in.
pub proof fn whole_run_is_confined(
    step: spec_fn(Machine, u32) -> Stepped,
    pieces: Seq<Piece>,
    link: spec_fn(u64) -> int,
    k: int,
    start: Machine,
    n: nat,
)
    requires
        0 <= k < pieces.len(),
        one_window(pieces, pieces[k].a),
        forall|i: int| 0 <= i < pieces.len() ==> #[trigger] is_chained(step, pieces[i].instructions, pieces[i].a),
        forall|pc: u64| 0 <= #[trigger] link(pc) < pieces.len(),
        handed(pieces[k].a, start),
    ensures
        ({
            let w = whole_run(step, pieces, link, k, start, n);
            let p = pieces[w.0];
            &&& 0 <= w.0 < pieces.len()
            &&& runs(p.a, w.1) ==> confined_step(p.a, w.1, step(w.1, word_at(p.instructions, p.a, w.1.pc)))
        }),
{
    let (s0, n0) = lemma_whole_segment(step, pieces, link, k, start, n);
    let w = whole_run(step, pieces, link, k, start, n);
    assert(is_chained(step, pieces[w.0].instructions, pieces[w.0].a));
    assert(w.1 == run(step, pieces[w.0].instructions, pieces[w.0].a, s0, n0));
}

/// The part's fact without the stack pointer: the frame in x28, the window's base in `bb`, a window address in `cut`
/// (31: none).
pub open spec fn part_x(a: Allowed, bb: int, cut: int, m: Machine) -> bool {
    &&& m.x.len() == 31
    &&& m.x[frame_register()] as int == a.frame
    &&& m.x[bb] as int == a.base
    &&& 0 <= cut < 31 ==> in_window(a, m.x[cut])
}

/// The part's fact.
pub open spec fn part(a: Allowed, bb: int, cut: int, m: Machine) -> bool {
    part_x(a, bb, cut, m) && stack_ok(a, m.sp)
}

/// A register that may hold the base: not x23, which the exits write, nor x28, the frame's.
pub open spec fn base_ok(bb: int) -> bool {
    0 <= bb < 31 && bb != 23 && bb != frame_register()
}

/// A register a word may write or load beside the base and the frame (31: none).
pub open spec fn reg_ok(r: int, bb: int) -> bool {
    0 <= r < 32 && r != bb && r != frame_register()
}

/// The cut after a word writing `r`: gone if `r` held it.
pub open spec fn after_cut(cut: int, r: int) -> int {
    if r == cut { 31 } else { cut }
}

/// A need for a window address in `need` (31: none) that a fact of one in `cut` meets.
pub open spec fn needs_cut(need: int, cut: int) -> bool {
    need == 31 || need == cut
}

/// The piece where form I puts it, its window, reservation and frame as form I has them (copy.rs `in_reach` without a
/// guest; form I's pool reserves 64 KiB below each window).
pub open spec fn placed(a: Allowed, instructions: Seq<u32>) -> bool {
    &&& a.length == instructions.len() * 4
    &&& a.base % 0x2_0000_0000 == 0 && 0x2_0000_0000 <= a.base && a.base < 2048 * 0x2_0000_0000
    &&& a.below == 0x1_0000
    &&& a.code >= 0 && a.code + a.length <= PROCESS_SLOTS
    &&& (a.code + a.length <= FLOOR_PAGE || a.code >= FLOOR_PAGE + 4096)
    &&& frame_in_reach(a)
}

/// The kinds of block, each with the word it is entered at (`j`); `cut` is the window address a block needs and
/// carries on (31: none).
pub enum Desc {
    /// A register-only word writing `rd` (31: none) and, with `sp` (displaced only), maybe the stack pointer.
    Register { j: int, rd: int, sp: bool, bb: int, cut: int },
    /// `add xc, xbb, wN, uxtw`: the cut into `c`.
    Cut { j: int, n: int, c: int, bb: int },
    /// A branch by `by` bytes, or on to the next word.
    Either { j: int, by: int, bb: int, cut: int },
    /// A branch by `by` bytes (`b`).
    Branch { j: int, by: int, bb: int, cut: int },
    /// An access of `bytes` at x[cut] plus `off`, loading `rt` and `rt2` (31: none), x[cut] written back when `back`.
    Through { j: int, rt: int, rt2: int, back: bool, off: int, bytes: nat, bb: int, cut: int },
    /// An access of `bytes` at `bb` plus wN: cut by its own addressing.
    Own { j: int, n: int, rt: int, rt2: int, bytes: nat, bb: int },
    /// The mark through `c`: `eor xc, xbb, #2^32`, `ldrb wzr, [xc, #4095]`, `udf`.
    Mark { j: int, bb: int, c: int },
    /// The probes through `c` of the first and last byte of `bytes` at x[from] (31: the stack pointer) plus `off`.
    Probes { j: int, c: int, from: int, off: int, bytes: nat, store: bool, bb: int },
    /// The exit to `guest`'s entry slot.
    Exit { j: int, guest: int },
    /// The jump through `xN` into the slot table, or to `out` when its target is outside the window.
    Jump { j: int, n: int, out: int },
    /// A call into the floor, `code` its entry.
    Call { j: int, code: int },
    /// A frame access or a move, after which the base is in `bb2`.
    Frame { j: int, f: FrameAccess, bb: int, bb2: int, cut: int },
    /// A word that raises a signal (`brk`): the run ends.
    Stop { j: int },
    /// `add sp, xbb, wN, uxtw`: the stack pointer cut into the window.
    StackCut { j: int, n: int, bb: int, cut: int },
    /// `mov sp, x[cut]`.
    StackMove { j: int, bb: int, cut: int },
    /// A stack step by `k` bytes (`add/sub sp, sp, #k`) and its probe of 4 bytes at the stack pointer plus `off`.
    StackStep { j: int, k: int, off: int, bb: int, cut: int },
    /// An access of `bytes` at the stack pointer plus `off`, loading `rt` and `rt2`, moving it by `moves`.
    StackAccess { j: int, rt: int, rt2: int, off: int, moves: int, bytes: nat, bb: int },
    /// `mov x23, sp`, then an access of `bytes` at x23 loading `rt` and `rt2`, x23 written back.
    StackVia { j: int, rt: int, rt2: int, bytes: nat, bb: int, cut: int },
    /// `mov xc, sp`, then the probes through `c` of the first and last byte of `bytes` there.
    StackProbes { j: int, c: int, bytes: nat, store: bool, bb: int },
    /// `movz/movn x23, #off`, `add x23, sp, x23`, then an access of `bytes` at x23 loading `rt` and `rt2`.
    StackAt { j: int, off: int, rt: int, rt2: int, bytes: nat, bb: int, cut: int },
    /// `mov xc, xc` of the cut: it stays.
    Same { j: int, bb: int, cut: int },
    /// The displaced jump through `xN` into slot 0's table, or to `out` when its target is too high.
    DisplacedJump { j: int, n: int, out: int },
}

pub open spec fn entry_of(d: Desc) -> int {
    match d {
        Desc::Register { j, .. } => j,
        Desc::Cut { j, .. } => j,
        Desc::Either { j, .. } => j,
        Desc::Branch { j, .. } => j,
        Desc::Through { j, .. } => j,
        Desc::Own { j, .. } => j,
        Desc::Mark { j, .. } => j,
        Desc::Probes { j, .. } => j,
        Desc::Exit { j, .. } => j,
        Desc::Jump { j, .. } => j,
        Desc::Call { j, .. } => j,
        Desc::Frame { j, .. } => j,
        Desc::Stop { j } => j,
        Desc::StackCut { j, .. } => j,
        Desc::StackMove { j, .. } => j,
        Desc::StackStep { j, .. } => j,
        Desc::StackAccess { j, .. } => j,
        Desc::StackVia { j, .. } => j,
        Desc::StackProbes { j, .. } => j,
        Desc::Same { j, .. } => j,
        Desc::DisplacedJump { j, .. } => j,
        Desc::StackAt { j, .. } => j,
    }
}

/// A block whose need the part's fact with `bb` and `cut` meets.
pub open spec fn iface_ok(d: Desc, bb: int, cut: int) -> bool {
    match d {
        Desc::Register { bb: b, cut: c, .. } => b == bb && needs_cut(c, cut),
        Desc::Cut { bb: b, .. } => b == bb,
        Desc::Either { bb: b, cut: c, .. } => b == bb && needs_cut(c, cut),
        Desc::Branch { bb: b, cut: c, .. } => b == bb && needs_cut(c, cut),
        Desc::Through { bb: b, cut: c, .. } => b == bb && c == cut,
        Desc::Own { bb: b, .. } => b == bb,
        Desc::Mark { bb: b, .. } => b == bb,
        Desc::Probes { bb: b, from, .. } => b == bb && needs_cut(from, cut),
        Desc::Exit { .. } => bb == 24,
        Desc::Jump { .. } => bb == 24,
        Desc::Call { .. } => true,
        Desc::Frame { bb: b, cut: c, .. } => b == bb && needs_cut(c, cut),
        Desc::Stop { .. } => true,
        Desc::StackCut { bb: b, cut: c, .. } => b == bb && needs_cut(c, cut),
        Desc::StackMove { bb: b, cut: c, .. } => b == bb && c == cut,
        Desc::StackStep { bb: b, cut: c, .. } => b == bb && needs_cut(c, cut),
        Desc::StackAccess { bb: b, .. } => b == bb,
        Desc::StackVia { bb: b, cut: c, .. } => b == bb && needs_cut(c, cut),
        Desc::StackProbes { bb: b, .. } => b == bb,
        Desc::Same { bb: b, cut: c, .. } => b == bb && c == cut,
        Desc::DisplacedJump { .. } => bb == 24,
        Desc::StackAt { bb: b, cut: c, .. } => b == bb && needs_cut(c, cut),
    }
}

/// Some block of the list is entered at `entry` with a need the part's fact meets.
pub open spec fn accepts(descs: Seq<Desc>, entry: int, bb: int, cut: int) -> bool
    decreases descs.len(),
{
    if descs.len() == 0 {
        false
    } else {
        (entry_of(descs.last()) == entry && iface_ok(descs.last(), bb, cut)) || accepts(descs.drop_last(), entry, bb, cut)
    }
}

/// `add xc, xbb, wN, uxtw`, `add sp, xbb, wN, uxtw`, `mov sp, xc`, `mov x23, sp`, a stack step's probe.
pub open spec fn add_cut_into(n: u32, bb: int, c: int) -> u32 {
    (0x8b20_4000u32 | (n << 16u32) | ((bb as u32) << 5u32) | (c as u32)) as u32
}

pub open spec fn add_sp_cut(n: u32, bb: int) -> u32 {
    (0x8b20_401fu32 | (n << 16u32) | ((bb as u32) << 5u32)) as u32
}

pub open spec fn mov_sp_from(c: int) -> u32 {
    (0x9100_001fu32 | ((c as u32) << 5u32)) as u32
}

pub const STACK_TO_X23: u32 = 0x9100_03f7;

/// `mov xc, sp`; `mov xc, xc`.
pub open spec fn mov_c_sp(c: int) -> u32 {
    (0x9100_03e0u32 | (c as u32)) as u32
}

pub open spec fn same_word(c: int) -> u32 {
    (0x9100_0000u32 | ((c as u32) << 5u32) | (c as u32)) as u32
}

/// The cut after an access through it: gone if written back or loaded over.
pub open spec fn through_cut(cut: int, rt: int, rt2: int, back: bool) -> int {
    if back { 31 } else { after_cut(after_cut(cut, rt), rt2) }
}

/// The stack probes' words: `mov xc, sp`, then a load's `ldrb wzr, [xc]`, `ldrb wzr, [xc, #bytes-1]` or a store's
/// `staddb wzr, [xc]`, `add xc, xc, #bytes-1`, `staddb wzr, [xc]`.
pub open spec fn stack_probes_words(c: int, bytes: nat, store: bool) -> Seq<u32> {
    seq![mov_c_sp(c)]
        + if store {
            seq![probe_store_c(c), add_c_c(c, bytes - 1), probe_store_c(c)]
        } else {
            seq![probe_load_c(c, 0), probe_load_c(c, bytes - 1)]
        }
}

pub open spec fn stack_probes_length(store: bool) -> int {
    if store { 4 } else { 3 }
}

/// Where the stack probes' run stands: `c` the stack pointer, then a store's last byte.
pub open spec fn stack_probes_inv(a: Allowed, j: int, c: int, bytes: nat, store: bool, m: Machine) -> bool {
    let i = index(a, m.pc);
    &&& (i == j + 1 || (!store && i == j + 2)) ==> m.x[c] == m.sp
    &&& (store && i == j + 2) ==> m.x[c] == m.sp
    &&& (store && i == j + 3) ==> m.x[c] as int == m.sp + bytes - 1
}

/// The displaced jump's words (branch.rs `displaced_jump` without the count and its way to the floor): 6, the
/// `cbnz` going to `out`.
pub open spec fn displaced_jump_words(n: u32, j: int, out: int) -> Seq<u32> {
    seq![check_x23(n), cbnz_c(23, (out - j - 1) * 4), ubfiz_x23(n), BFXIL_X23, EOR_X23_TABLE, BR_X23]
}

/// What the displaced jump's words do (Arm ARM; branch.rs `displaced_jump_means` with its `cbnz` going `by` bytes).
pub open spec fn displaced_jump_main_means(step: spec_fn(Machine, u32) -> Stepped, n: u32, by: int) -> bool {
    &&& writes(step, check_x23(n), 23, |m: Machine| (m.x[n as int] >> 32u64) & 0xff_ffffu64)
    &&& branches_if(step, cbnz_c(23, by), 23, by, true)
    &&& writes(step, ubfiz_x23(n), 23, |m: Machine| (m.x[n as int] & 0xffff_ffffu64) << 2u64)
    &&& writes(step, BFXIL_X23, 23, |m: Machine| (m.x[23] & !3u64) | ((m.x[23] >> 2u64) & 3u64))
    &&& writes(step, EOR_X23_TABLE, 23, |m: Machine| m.x[23] ^ 0x4000_0000_0000u64)
    &&& jumps(step, BR_X23, 23)
}

/// Where the displaced jump's run stands (branch.rs `displaced_jump_inv`, from its second word).
pub open spec fn displaced_jump_at(a: Allowed, j: int, m: Machine) -> bool {
    let o = (index(a, m.pc) - j) * 4;
    &&& o == 12 ==> m.x[23] < 0x4_0000_0000u64 && m.x[23] & 3u64 == 0
    &&& o == 16 ==> m.x[23] < 0x4_0000_0003
    &&& o == 20 ==> PROCESS_SLOTS <= m.x[23] && m.x[23] < PROCESS_SLOTS + 0x4_0000_0003
}

/// `add xc, xfrom, xc` (x31: the stack pointer).
pub open spec fn add_from_c(c: int, from: int) -> u32 {
    (0x8b20_6000u32 | ((c as u32) << 16u32) | ((from as u32) << 5u32) | (c as u32)) as u32
}

/// `movz xc, #off`, or `movn xc, #-off-1` for a negative offset, and the value it writes.
pub open spec fn movx_c(c: int, off: int) -> u32 {
    if off >= 0 { movz_c(c, off) } else { (0x9280_0000u32 | (((-off - 1) as u32) << 5u32) | (c as u32)) as u32 }
}

pub open spec fn wide(off: int) -> int {
    if off >= 0 { off } else { off + two_to_64() }
}

/// The probes' words: `movz xc, #off` (or `movn`), `add xc, xfrom, xc`, and a load's `ldrb wzr, [xc]`, `ldrb wzr, [xc, #bytes-1]`
/// or a store's `staddb wzr, [xc]`, `add xc, xc, #bytes-1`, `staddb wzr, [xc]`.
pub open spec fn probes_words(c: int, from: int, off: int, bytes: nat, store: bool) -> Seq<u32> {
    seq![movx_c(c, off), add_from_c(c, from)]
        + if store {
            seq![probe_store_c(c), add_c_c(c, bytes - 1), probe_store_c(c)]
        } else {
            seq![probe_load_c(c, 0), probe_load_c(c, bytes - 1)]
        }
}

pub open spec fn probes_length(store: bool) -> int {
    if store { 5 } else { 4 }
}

/// The address register `from` holds (31: the stack pointer).
pub open spec fn from_value(m: Machine, from: int) -> int {
    if from == 31 { m.sp as int } else { m.x[from] as int }
}

/// The jump's words (branch.rs `jump` without the count and its way to the floor): 7, the `cbnz` going to `out`.
pub open spec fn jump_words(n: u32, j: int, out: int) -> Seq<u32> {
    seq![check_x23(n), cbnz_c(23, (out - j - 1) * 4), add_cut(n), UBFIZ_X23, BFXIL_X23, EOR_X23_TABLE, BR_X23]
}

/// What the jump's words do (Arm ARM; branch.rs `jump_means` with its `cbnz` going `by` bytes).
pub open spec fn jump_main_means(step: spec_fn(Machine, u32) -> Stepped, n: u32, by: int) -> bool {
    &&& writes(step, check_x23(n), 23, |m: Machine| (m.x[n as int] >> 32u64) & 0xff_ffffu64)
    &&& branches_if(step, cbnz_c(23, by), 23, by, true)
    &&& writes(step, add_cut(n), 23, |m: Machine| ((m.x[24] + (m.x[n as int] & 0xffff_ffffu64)) % two_to_64()) as u64)
    &&& writes(step, UBFIZ_X23, 23, |m: Machine| (m.x[23] & 0xfff_ffff_ffffu64) << 2u64)
    &&& writes(step, BFXIL_X23, 23, |m: Machine| (m.x[23] & !3u64) | ((m.x[23] >> 2u64) & 3u64))
    &&& writes(step, EOR_X23_TABLE, 23, |m: Machine| m.x[23] ^ 0x4000_0000_0000u64)
    &&& jumps(step, BR_X23, 23)
}

/// The words of `instructions` from `j` are `words`.
pub open spec fn holds_at(instructions: Seq<u32>, j: int, words: Seq<u32>) -> bool {
    0 <= j && j + words.len() <= instructions.len() && instructions.subrange(j, j + words.len()) == words
}

/// Block `d`'s shape, decidable over the words: its own words, its registers, and that each place it goes on to is
/// where a block of the list is entered with a need it meets.
pub open spec fn shape_ok(instructions: Seq<u32>, a: Allowed, descs: Seq<Desc>, d: Desc) -> bool {
    let len = instructions.len() as int;
    match d {
        Desc::Register { j, rd, sp, bb, cut } =>
            0 <= j && j + 1 < len && base_ok(bb) && 0 <= cut <= 31 && reg_ok(rd, bb) && (sp ==> a.displaced)
            && accepts(descs, j + 1, bb, after_cut(cut, rd)),
        Desc::Cut { j, n, c, bb } =>
            0 <= j && j + 1 < len && base_ok(bb) && 0 <= n < 31 && 0 <= c < 31 && c != bb && c != frame_register()
            && instructions[j] == add_cut_into(n as u32, bb, c) && accepts(descs, j + 1, bb, c),
        Desc::Either { j, by, bb, cut } =>
            0 <= j && j + 1 < len && base_ok(bb) && 0 <= cut <= 31 && by % 4 == 0 && 0 <= j + by / 4 < len
            && accepts(descs, j + 1, bb, cut) && accepts(descs, j + by / 4, bb, cut),
        Desc::Branch { j, by, bb, cut } =>
            0 <= j < len && base_ok(bb) && 0 <= cut <= 31 && by % 4 == 0 && 0 <= j + by / 4 < len
            && accepts(descs, j + by / 4, bb, cut),
        Desc::Through { j, rt, rt2, back, off, bytes, bb, cut } =>
            0 <= j && j + 1 < len && base_ok(bb) && 0 <= cut < 31 && cut != bb && cut != frame_register()
            && reg_ok(rt, bb) && reg_ok(rt2, bb) && -0x1_0000 <= off && 1 <= bytes && off + bytes <= 0x1_0000
            && accepts(descs, j + 1, bb, through_cut(cut, rt, rt2, back)),
        Desc::Own { j, n, rt, rt2, bytes, bb } =>
            0 <= j && j + 1 < len && base_ok(bb) && 0 <= n < 31 && reg_ok(rt, bb) && reg_ok(rt2, bb)
            && 1 <= bytes <= 16 && accepts(descs, j + 1, bb, 31),
        Desc::Mark { j, bb, c } =>
            base_ok(bb) && 0 <= c < 31 && c != bb && c != frame_register()
            && holds_at(instructions, j, seq![eor_c(c, bb), mark_load_c(c), UDF]),
        Desc::Probes { j, c, from, off, bytes, store, bb } =>
            base_ok(bb) && 0 <= c < 31 && c != bb && c != from && c != frame_register() && 0 <= from <= 31
            && (from == 31 ==> !a.displaced) && -0x400 <= off && 1 <= bytes && off + bytes <= 0x1_0000
            && j + probes_length(store) < len && holds_at(instructions, j, probes_words(c, from, off, bytes, store))
            && accepts(descs, j + probes_length(store), bb, from),
        Desc::Exit { j, guest } =>
            0 <= guest && guest + 4 < 0x1_0000_0000 && holds_at(instructions, j, exit_words(exit_slot(a, guest + 4) as u64)),
        Desc::Jump { j, n, out } =>
            !a.displaced && 0 <= n < 31 && j + 1 < out < instructions.len() && holds_at(instructions, j, jump_words(n as u32, j, out))
            && accepts(descs, out, 24, 31),
        Desc::Call { j, code } =>
            0 <= code < 256 && holds_at(instructions, j, floor_call(floor_entry(code))),
        Desc::Frame { j, f, bb, bb2, cut } =>
            0 <= j && j + 1 < len && base_ok(bb) && base_ok(bb2) && 0 <= cut <= 31 && instructions[j] == f.word
            && (if f.moves && f.off == bb && f.rt == bb2 { true } else { bb2 == bb && f.rt != bb })
            && accepts(descs, j + 1, bb2, after_cut(cut, f.rt)),
        Desc::Stop { j } => 0 <= j < len,
        Desc::StackCut { j, n, bb, cut } =>
            0 <= j && j + 1 < len && base_ok(bb) && 0 <= cut <= 31 && 0 <= n < 31
            && instructions[j] == add_sp_cut(n as u32, bb) && accepts(descs, j + 1, bb, cut),
        Desc::StackMove { j, bb, cut } =>
            0 <= j && j + 1 < len && base_ok(bb) && 0 <= cut < 31 && instructions[j] == mov_sp_from(cut)
            && accepts(descs, j + 1, bb, cut),
        Desc::StackStep { j, k, off, bb, cut } =>
            !a.displaced && 0 <= j && j + 2 < len && base_ok(bb) && 0 <= cut <= 31 && k != 0 && -0xf000 <= k <= 0xf000
            && -0x400 <= off <= 0x400 && accepts(descs, j + 2, bb, cut),
        Desc::StackAccess { j, rt, rt2, off, moves, bytes, bb } =>
            !a.displaced && 0 <= j && j + 1 < len && base_ok(bb) && reg_ok(rt, bb) && reg_ok(rt2, bb)
            && -0x400 <= off && 1 <= bytes && off + bytes <= 0x1_0000 && -0x400 <= moves <= 0x400
            && (moves != 0 ==> off == 0 || off == moves) && accepts(descs, j + 1, bb, 31),
        Desc::StackVia { j, rt, rt2, bytes, bb, cut } =>
            !a.displaced && 0 <= j && j + 2 < len && base_ok(bb) && 0 <= cut <= 31 && cut != 23
            && reg_ok(rt, bb) && reg_ok(rt2, bb) && (cut < 31 ==> rt != cut && rt2 != cut) && 1 <= bytes <= 0x1_0000
            && instructions[j] == STACK_TO_X23 && accepts(descs, j + 2, bb, cut),
        Desc::StackProbes { j, c, bytes, store, bb } =>
            !a.displaced && base_ok(bb) && 0 <= c < 31 && c != bb && c != frame_register() && 1 <= bytes <= 0x1_0000
            && j + stack_probes_length(store) < len && holds_at(instructions, j, stack_probes_words(c, bytes, store))
            && accepts(descs, j + stack_probes_length(store), bb, 31),
        Desc::Same { j, bb, cut } =>
            0 <= j && j + 1 < len && base_ok(bb) && 0 <= cut < 31 && instructions[j] == same_word(cut)
            && accepts(descs, j + 1, bb, cut),
        Desc::DisplacedJump { j, n, out } =>
            a.displaced && 0 <= n < 31 && j + 1 < out < instructions.len()
            && holds_at(instructions, j, displaced_jump_words(n as u32, j, out)) && accepts(descs, out, 24, 31),
        Desc::StackAt { j, off, rt, rt2, bytes, bb, cut } =>
            !a.displaced && 0 <= j && j + 3 < len && base_ok(bb) && 0 <= cut <= 31 && cut != 23
            && reg_ok(rt, bb) && reg_ok(rt2, bb) && (cut < 31 ==> rt != cut && rt2 != cut)
            && -0x400 <= off && 1 <= bytes && off + bytes <= 0x1_0000
            && holds_at(instructions, j, seq![movx_c(23, off), add_from_c(23, 31)]) && accepts(descs, j + 3, bb, cut),
    }
}

/// `word` writes `rd` (31: none of x0 to x30), with `sp` maybe the stack pointer, and nothing else: no access, on to
/// the next word (a register-only instruction; Arm ARM).
pub open spec fn keeps_but(step: spec_fn(Machine, u32) -> Stepped, word: u32, rd: int, sp: bool) -> bool {
    forall|m: Machine| m.x.len() == 31 ==> {
        let s = #[trigger] step(m, word);
        &&& s.accesses.len() == 0 && !s.system_call && !s.after.ended
        &&& s.after.unlisted == m.unlisted && s.after.pc == (m.pc + 4) as u64
        &&& s.after.x.len() == 31
        &&& forall|r: int| 0 <= r < 31 && r != rd ==> s.after.x[r] == m.x[r]
        &&& !sp ==> s.after.sp == m.sp
    }
}

/// `word` goes `by` bytes from itself or on to the next word, changing nothing else (a branch; the flags or a register
/// decide, which the proofs need not know).
pub open spec fn goes_either(step: spec_fn(Machine, u32) -> Stepped, word: u32, by: int) -> bool {
    forall|m: Machine| m.x.len() == 31 ==> {
        let s = #[trigger] step(m, word);
        &&& s.accesses.len() == 0 && !s.system_call
        &&& s.after == Machine { pc: s.after.pc, ..m }
        &&& s.after.pc == (m.pc + by) as u64 || s.after.pc == (m.pc + 4) as u64
    }
}

/// `word` touches `bytes` bytes from `address` and writes no general register but `rt`, `rt2` and `rb` (31: none),
/// not the stack pointer; a fault ends the run, else it goes on at the next word (meaning.rs `accesses` with a pair
/// and a written-back base).
pub open spec fn touches(
    step: spec_fn(Machine, u32) -> Stepped,
    word: u32,
    rt: int,
    rt2: int,
    rb: int,
    address: spec_fn(Machine) -> int,
    bytes: nat,
) -> bool {
    forall|m: Machine| m.x.len() == 31 ==> {
        let s = #[trigger] step(m, word);
        &&& s.accesses == seq![(address(m), bytes)]
        &&& !s.system_call
        &&& s.after.unlisted == m.unlisted
        &&& s.after.x.len() == 31 && s.after.sp == m.sp
        &&& forall|r: int| 0 <= r < 31 && r != rt && r != rt2 && r != rb ==> s.after.x[r] == m.x[r]
        &&& s.after.ended || s.after.pc == (m.pc + 4) as u64
    }
}

/// `word` writes `value` of the machine to the stack pointer and nothing else.
pub open spec fn sets_sp(step: spec_fn(Machine, u32) -> Stepped, word: u32, value: spec_fn(Machine) -> u64) -> bool {
    forall|m: Machine| m.x.len() == 31 ==> #[trigger] step(m, word) == Stepped {
        after: Machine { sp: value(m), pc: (m.pc + 4) as u64, ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
}

/// Every byte in the window.
pub open spec fn in_window_bytes(a: Allowed, first: int, bytes: nat) -> bool {
    a.base <= first && first + bytes <= a.base + WINDOW_BYTES as int
}

/// `word` touches `bytes` bytes at the stack pointer plus `off`, writes no general register but `rt` and `rt2`, and
/// goes on at the next word with the stack pointer moved by `moves`, or a fault ends the run. With `faults`, it ends
/// the run when a byte it touches is outside the window: the slot's bytes there are never mapped (the memory map, an
/// assumption of the chain).
pub open spec fn stack_touches(
    step: spec_fn(Machine, u32) -> Stepped,
    a: Allowed,
    word: u32,
    rt: int,
    rt2: int,
    off: int,
    moves: int,
    bytes: nat,
    faults: bool,
) -> bool {
    forall|m: Machine| m.x.len() == 31 ==> {
        let s = #[trigger] step(m, word);
        let first = (m.sp + off) % two_to_64();
        &&& s.accesses == seq![(first, bytes)]
        &&& !s.system_call
        &&& s.after.unlisted == m.unlisted
        &&& s.after.x.len() == 31
        &&& forall|r: int| 0 <= r < 31 && r != rt && r != rt2 ==> s.after.x[r] == m.x[r]
        &&& s.after.ended || (s.after.pc == (m.pc + 4) as u64 && s.after.sp as int == (m.sp + moves) % two_to_64())
        &&& faults && !in_window_bytes(a, first, bytes) ==> s.after.ended
    }
}

/// What block `d`'s words do (Arm ARM), as hypotheses on the free `step`.
pub open spec fn meant(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, d: Desc) -> bool {
    match d {
        Desc::Register { j, rd, sp, .. } => keeps_but(step, instructions[j], rd, sp),
        Desc::Cut { n, c, bb, .. } =>
            writes(step, add_cut_into(n as u32, bb, c), c, |m: Machine| ((m.x[bb] + (m.x[n] & 0xffff_ffffu64)) % two_to_64()) as u64),
        Desc::Either { j, by, .. } => goes_either(step, instructions[j], by),
        Desc::Branch { j, by, .. } => branches(step, instructions[j], by),
        Desc::Through { j, rt, rt2, back, off, bytes, cut, .. } =>
            touches(step, instructions[j], rt, rt2, if back { cut } else { 31 }, |m: Machine| (m.x[cut] + off) % two_to_64(), bytes),
        Desc::Own { j, n, rt, rt2, bytes, bb } =>
            touches(step, instructions[j], rt, rt2, 31, |m: Machine| (m.x[bb] + (m.x[n] & 0xffff_ffffu64)) % two_to_64(), bytes),
        Desc::Mark { bb, c, .. } => mark_c_means(step, bb, c),
        Desc::Probes { c, from, off, bytes, store, .. } => {
            &&& writes(step, movx_c(c, off), c, |m: Machine| wide(off) as u64)
            &&& writes(step, add_from_c(c, from), c, |m: Machine| ((from_value(m, from) + m.x[c]) % two_to_64()) as u64)
            &&& store ==> accesses(step, probe_store_c(c), 31, |m: Machine| m.x[c] as int, 1)
            &&& store ==> writes(step, add_c_c(c, bytes - 1), c, |m: Machine| ((m.x[c] + (bytes - 1)) % two_to_64()) as u64)
            &&& !store ==> accesses(step, probe_load_c(c, 0), 31, |m: Machine| (m.x[c] + 0) % two_to_64(), 1)
            &&& !store ==> accesses(step, probe_load_c(c, bytes - 1), 31, |m: Machine| (m.x[c] + (bytes - 1)) % two_to_64(), 1)
        },
        Desc::Exit { .. } => exit_means(step),
        Desc::Jump { j, n, out } => jump_main_means(step, n as u32, (out - j - 1) * 4),
        Desc::Call { .. } => floor_means(step),
        Desc::Frame { f, .. } => frame_access_means(step, f),
        Desc::Stop { j } => stops(step, instructions[j]),
        Desc::StackCut { n, bb, .. } =>
            sets_sp(step, add_sp_cut(n as u32, bb), |m: Machine| ((m.x[bb] + (m.x[n] & 0xffff_ffffu64)) % two_to_64()) as u64),
        Desc::StackMove { cut, .. } => sets_sp(step, mov_sp_from(cut), |m: Machine| m.x[cut]),
        Desc::StackStep { j, k, off, .. } => {
            &&& sets_sp(step, instructions[j], |m: Machine| ((m.sp + k) % two_to_64()) as u64)
            &&& stack_touches(step, a, instructions[j + 1], 31, 31, off, 0, 4, true)
        },
        Desc::StackAccess { j, rt, rt2, off, moves, bytes, .. } =>
            stack_touches(step, a, instructions[j], rt, rt2, off, moves, bytes, moves != 0),
        Desc::StackVia { j, rt, rt2, bytes, .. } => {
            &&& writes(step, STACK_TO_X23, 23, |m: Machine| m.sp)
            &&& touches(step, instructions[j + 1], rt, rt2, 23, |m: Machine| m.x[23] as int, bytes)
        },
        Desc::StackProbes { c, bytes, store, .. } => {
            &&& writes(step, mov_c_sp(c), c, |m: Machine| m.sp)
            &&& store ==> accesses(step, probe_store_c(c), 31, |m: Machine| m.x[c] as int, 1)
            &&& store ==> writes(step, add_c_c(c, bytes - 1), c, |m: Machine| ((m.x[c] + (bytes - 1)) % two_to_64()) as u64)
            &&& !store ==> accesses(step, probe_load_c(c, 0), 31, |m: Machine| (m.x[c] + 0) % two_to_64(), 1)
            &&& !store ==> accesses(step, probe_load_c(c, bytes - 1), 31, |m: Machine| (m.x[c] + (bytes - 1)) % two_to_64(), 1)
        },
        Desc::Same { cut, .. } => writes(step, same_word(cut), cut, |m: Machine| m.x[cut]),
        Desc::DisplacedJump { j, n, out } => displaced_jump_main_means(step, n as u32, (out - j - 1) * 4),
        Desc::StackAt { j, off, rt, rt2, bytes, .. } => {
            &&& writes(step, movx_c(23, off), 23, |m: Machine| wide(off) as u64)
            &&& writes(step, add_from_c(23, 31), 23, |m: Machine| ((m.sp + m.x[23]) % two_to_64()) as u64)
            &&& touches(step, instructions[j + 2], rt, rt2, 23, |m: Machine| m.x[23] as int, bytes)
        },
    }
}

/// The piece is described: the first block is entered at its first word with what a run hands on, and every block
/// has its shape.
pub open spec fn shapes_ok(instructions: Seq<u32>, a: Allowed, all: Seq<Desc>, k: int) -> bool
    decreases k,
{
    if k <= 0 { true } else { shape_ok(instructions, a, all, all[k - 1]) && shapes_ok(instructions, a, all, k - 1) }
}

pub open spec fn described(instructions: Seq<u32>, a: Allowed, descs: Seq<Desc>) -> bool {
    &&& descs.len() > 0 && entry_of(descs[0]) == 0 && iface_ok(descs[0], 24, 31)
    &&& shapes_ok(instructions, a, descs, descs.len() as int)
}

pub open spec fn all_meant(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, descs: Seq<Desc>) -> bool {
    forall|i: int| 0 <= i < descs.len() ==> meant(step, instructions, a, #[trigger] descs[i])
}

/// How far a stack step may have moved the stack pointer from where the part's fact has it.
pub open spec fn near(a: Allowed, sp: u64, k: int) -> bool {
    a.base - STACK_SLACK as int + k <= sp as int && sp as int <= a.base + WINDOW_BYTES as int + STACK_SLACK as int + k
}

/// The block of a description.
pub open spec fn block_of(a: Allowed, d: Desc) -> Block {
    match d {
        Desc::Register { j, bb, cut, .. } => one(a, j, bb, cut),
        Desc::Cut { j, bb, .. } => one(a, j, bb, 31),
        Desc::Either { j, bb, cut, .. } => one(a, j, bb, cut),
        Desc::Branch { j, bb, cut, .. } => one(a, j, bb, cut),
        Desc::Through { j, bb, cut, .. } => one(a, j, bb, cut),
        Desc::Own { j, bb, .. } => one(a, j, bb, 31),
        Desc::Frame { j, bb, cut, .. } => one(a, j, bb, cut),
        Desc::StackCut { j, bb, cut, .. } => one(a, j, bb, cut),
        Desc::StackMove { j, bb, cut } => one(a, j, bb, cut),
        Desc::StackAccess { j, bb, .. } => one(a, j, bb, 31),
        Desc::Mark { j, bb, c } => Block {
            owns: |i: int| j <= i < j + 3,
            entry: j,
            needs: |m: Machine| part(a, bb, 31, m),
            inv: |m: Machine| part(a, bb, 31, m) && (index(a, m.pc) == j + 1 ==> m.x[c] as int == a.base + 0x1_0000_0000),
        },
        Desc::Probes { j, c, from, off, bytes, store, bb } => Block {
            owns: |i: int| j <= i < j + probes_length(store),
            entry: j,
            needs: |m: Machine| part(a, bb, from, m),
            inv: |m: Machine| part(a, bb, from, m) && probes_inv(a, j, c, from, off, bytes, store, m),
        },
        Desc::Exit { j, guest } => Block {
            owns: |i: int| j <= i < j + 4,
            entry: j,
            needs: |m: Machine| part(a, 24, 31, m),
            inv: |m: Machine| part(a, 24, 31, m) && at_exit(m, exit_slot(a, guest + 4) as u64, index(a, m.pc) - j),
        },
        Desc::Jump { j, .. } => Block {
            owns: |i: int| j <= i < j + 7,
            entry: j,
            needs: |m: Machine| part(a, 24, 31, m),
            inv: |m: Machine| part(a, 24, 31, m) && jump_at(a, j, m),
        },
        Desc::Call { j, code } => Block {
            owns: |i: int| j <= i < j + 5,
            entry: j,
            needs: |m: Machine| kept(a, m),
            inv: |m: Machine| kept(a, m) && at_floor_call(m, floor_entry(code), index(a, m.pc) - j),
        },
        Desc::Stop { j } => Block { owns: |i: int| i == j, entry: j, needs: |m: Machine| kept(a, m), inv: |m: Machine| kept(a, m) },
        Desc::StackStep { j, k, bb, cut, .. } => Block {
            owns: |i: int| j <= i < j + 2,
            entry: j,
            needs: |m: Machine| part(a, bb, cut, m),
            inv: |m: Machine| (index(a, m.pc) == j ==> part(a, bb, cut, m))
                && (index(a, m.pc) == j + 1 ==> part_x(a, bb, cut, m) && near(a, m.sp, k)),
        },
        Desc::StackVia { j, bb, cut, .. } => Block {
            owns: |i: int| j <= i < j + 2,
            entry: j,
            needs: |m: Machine| part(a, bb, cut, m),
            inv: |m: Machine| part(a, bb, cut, m) && (index(a, m.pc) == j + 1 ==> m.x[23] == m.sp),
        },
        Desc::StackProbes { j, c, bytes, store, bb } => Block {
            owns: |i: int| j <= i < j + stack_probes_length(store),
            entry: j,
            needs: |m: Machine| part(a, bb, 31, m),
            inv: |m: Machine| part(a, bb, 31, m) && stack_probes_inv(a, j, c, bytes, store, m),
        },
        Desc::Same { j, bb, cut } => one(a, j, bb, cut),
        Desc::StackAt { j, off, bb, cut, .. } => Block {
            owns: |i: int| j <= i < j + 3,
            entry: j,
            needs: |m: Machine| part(a, bb, cut, m),
            inv: |m: Machine| part(a, bb, cut, m) && (index(a, m.pc) == j + 1 ==> m.x[23] as int == wide(off))
                && (index(a, m.pc) == j + 2 ==> m.x[23] as int == m.sp + off),
        },
        Desc::DisplacedJump { j, .. } => Block {
            owns: |i: int| j <= i < j + 6,
            entry: j,
            needs: |m: Machine| part(a, 24, 31, m),
            inv: |m: Machine| part(a, 24, 31, m) && displaced_jump_at(a, j, m),
        },
    }
}

/// A one-word block needing the part's fact.
pub open spec fn one(a: Allowed, j: int, bb: int, cut: int) -> Block {
    Block { owns: |i: int| i == j, entry: j, needs: |m: Machine| part(a, bb, cut, m), inv: |m: Machine| part(a, bb, cut, m) }
}

/// Where the probes' run stands: `c` the offset after its move, then the address it probes.
pub open spec fn probes_inv(a: Allowed, j: int, c: int, from: int, off: int, bytes: nat, store: bool, m: Machine) -> bool {
    let i = index(a, m.pc);
    &&& i == j + 1 ==> m.x[c] as int == wide(off)
    &&& (i == j + 2 || i == j + 3) ==> m.x[c] as int == from_value(m, from) + off
    &&& (store && i == j + 4) ==> m.x[c] as int == from_value(m, from) + off + bytes - 1
}

/// Where the jump's run stands (branch.rs `jump_inv`, from its second word).
pub open spec fn jump_at(a: Allowed, j: int, m: Machine) -> bool {
    let o = (index(a, m.pc) - j) * 4;
    &&& o == 12 ==> in_window(a, m.x[23])
    &&& o == 16 ==> a.base * 4 <= m.x[23] && m.x[23] < a.base * 4 + 0x4_0000_0000 && m.x[23] % 4 == 0
    &&& o == 20 ==> a.base * 4 <= m.x[23] && m.x[23] < a.base * 4 + 0x4_0000_0003 && m.x[23] < 0x4000_0000_0000
    &&& o == 24 ==> slot_table(a) <= m.x[23] && m.x[23] < slot_table(a) + 0x4_0000_0003
}

/// The blocks of a description.
pub open spec fn blocks_of(a: Allowed, descs: Seq<Desc>) -> Seq<Block> {
    Seq::new(descs.len(), |i: int| block_of(a, descs[i]))
}

/// Block `c` is entered at `entry` with a need the part's fact meets.
pub open spec fn takes(blocks: Seq<Block>, c: int, entry: int, a: Allowed, bb: int, cut: int) -> bool {
    &&& 0 <= c < blocks.len() && blocks[c].entry == entry
    &&& forall|m: Machine| part(a, bb, cut, m) ==> #[trigger] (blocks[c].needs)(m)
}

proof fn lemma_accepts(a: Allowed, descs: Seq<Desc>, entry: int, bb: int, cut: int) -> (c: int)
    requires
        accepts(descs, entry, bb, cut),
        base_ok(bb),
    ensures
        takes(blocks_of(a, descs), c, entry, a, bb, cut),
    decreases descs.len(),
{
    let blocks = blocks_of(a, descs);
    let last = descs.len() - 1;
    if entry_of(descs.last()) == entry && iface_ok(descs.last(), bb, cut) {
        assert(blocks[last] == block_of(a, descs[last]));
        assert forall|m: Machine| part(a, bb, cut, m) implies #[trigger] (blocks[last].needs)(m) by {}
        last
    } else {
        let c = lemma_accepts(a, descs.drop_last(), entry, bb, cut);
        let fewer = blocks_of(a, descs.drop_last());
        assert(fewer[c] == blocks[c]);
        assert forall|m: Machine| part(a, bb, cut, m) implies #[trigger] (blocks[c].needs)(m) by {
            assert((fewer[c].needs)(m));
        }
        c
    }
}

/// A step that goes on at word `t` of the piece with the part's fact is confined, enters the block `c` there, and
/// is not into the slot table.
proof fn lemma_goes_on(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    m: Machine,
    s: Stepped,
    t: int,
    c: int,
    bb: int,
    cut: int,
)
    requires
        placed(a, instructions),
        0 <= t < instructions.len(),
        s.after.pc as int == a.code + 4 * t,
        s.accesses.len() == 0 && !s.system_call && s.after.unlisted == m.unlisted,
        part(a, bb, cut, s.after),
        takes(blocks, c, t, a, bb, cut),
    ensures
        confined_step(a, m, s),
        enters(blocks, a, s.after),
        !in_table(a, s.after.pc),
{
    assert(in_sequence(a, s.after.pc));
    assert(index(a, s.after.pc) == t);
    assert((blocks[c].needs)(s.after));
}

/// A step that ends the run or goes on at word `t` of the piece is not into the slot table.
proof fn lemma_not_table(instructions: Seq<u32>, a: Allowed, s: Stepped, t: int)
    requires
        placed(a, instructions),
        0 <= t < instructions.len(),
        s.after.ended || s.after.pc as int == a.code + 4 * t,
    ensures
        !s.after.ended ==> !in_table(a, s.after.pc),
{
}

/// THE REGISTER-ONLY WORD'S BLOCK HOLDS.
proof fn lemma_register(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    rd: int,
    sp: bool,
    bb: int,
    cut: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, cut),
        0 <= j && j + 1 < instructions.len(),
        base_ok(bb) && 0 <= cut <= 31 && reg_ok(rd, bb) && (sp ==> a.displaced),
        keeps_but(step, instructions[j], rd, sp),
        takes(blocks, c, j + 1, a, bb, after_cut(cut, rd)),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, instructions[j]);
        assert(m.pc as int == a.code + 4 * j);
        assert(part(a, bb, after_cut(cut, rd), s.after));
        lemma_goes_on(step, instructions, a, blocks, m, s, j + 1, c, bb, after_cut(cut, rd));
    }
}

/// THE CUT'S BLOCK HOLDS: xc is the window's base plus the low word of xN.
proof fn lemma_cut(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    n: int,
    cr: int,
    bb: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, 31),
        0 <= j && j + 1 < instructions.len(),
        base_ok(bb) && 0 <= n < 31 && 0 <= cr < 31 && cr != bb && cr != frame_register(),
        instructions[j] == add_cut_into(n as u32, bb, cr),
        writes(step, add_cut_into(n as u32, bb, cr), cr, |m: Machine| ((m.x[bb] + (m.x[n] & 0xffff_ffffu64)) % two_to_64()) as u64),
        takes(blocks, c, j + 1, a, bb, cr),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, instructions[j]);
        assert(m.pc as int == a.code + 4 * j);
        lemma_low_word(m.x[n]);
        assert(part(a, bb, cr, s.after));
        lemma_goes_on(step, instructions, a, blocks, m, s, j + 1, c, bb, cr);
    }
}

/// THE BRANCH'S BLOCK HOLDS: either way it enters a block.
proof fn lemma_either(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    by: int,
    bb: int,
    cut: int,
    c: int,
    t: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, cut),
        0 <= j && j + 1 < instructions.len() && by % 4 == 0 && 0 <= j + by / 4 < instructions.len(),
        base_ok(bb),
        goes_either(step, instructions[j], by),
        takes(blocks, c, j + 1, a, bb, cut),
        takes(blocks, t, j + by / 4, a, bb, cut),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, instructions[j]);
        assert(m.pc as int == a.code + 4 * j);
        assert(part(a, bb, cut, s.after));
        if s.after.pc == (m.pc + 4) as u64 {
            lemma_goes_on(step, instructions, a, blocks, m, s, j + 1, c, bb, cut);
        } else {
            assert(s.after.pc as int == a.code + 4 * (j + by / 4));
            lemma_goes_on(step, instructions, a, blocks, m, s, j + by / 4, t, bb, cut);
        }
    }
}

/// THE UNCONDITIONAL BRANCH'S BLOCK HOLDS: it enters the block where it goes.
proof fn lemma_branch(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    by: int,
    bb: int,
    cut: int,
    t: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, cut),
        0 <= j < instructions.len() && by % 4 == 0 && 0 <= j + by / 4 < instructions.len(),
        base_ok(bb),
        branches(step, instructions[j], by),
        takes(blocks, t, j + by / 4, a, bb, cut),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, instructions[j]);
        assert(m.pc as int == a.code + 4 * j);
        assert(s.after.pc as int == a.code + 4 * (j + by / 4));
        lemma_goes_on(step, instructions, a, blocks, m, s, j + by / 4, t, bb, cut);
    }
}

/// An access from `first` is in the slot's reservation when it starts at most 64 KiB below the window and ends at
/// most 2 GiB above it.
proof fn lemma_in_slot(a: Allowed, first: int, bytes: nat)
    requires
        a.below == 0x1_0000,
        a.base - 0x1_0000 <= first && first + bytes <= a.base + 0x1_8000_0000,
    ensures
        touches_allowed(a, first, bytes),
{
}

/// THE ACCESS THROUGH THE CUT'S BLOCK HOLDS: x[cut] is in the window, the offset within 64 KiB.
proof fn lemma_through(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    rt: int,
    rt2: int,
    back: bool,
    off: int,
    bytes: nat,
    bb: int,
    cut: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, cut),
        0 <= j && j + 1 < instructions.len(),
        base_ok(bb) && 0 <= cut < 31 && cut != bb && cut != frame_register() && reg_ok(rt, bb) && reg_ok(rt2, bb),
        -0x1_0000 <= off && 1 <= bytes && off + bytes <= 0x1_0000,
        touches(step, instructions[j], rt, rt2, if back { cut } else { 31 }, |m: Machine| (m.x[cut] + off) % two_to_64(), bytes),
        takes(blocks, c, j + 1, a, bb, through_cut(cut, rt, rt2, back)),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, instructions[j]);
        assert(m.pc as int == a.code + 4 * j);
        assert((m.x[cut] + off) % two_to_64() == m.x[cut] + off);
        lemma_in_slot(a, m.x[cut] + off, bytes);
        assert(touches_allowed(a, s.accesses[0].0, s.accesses[0].1));
        lemma_not_table(instructions, a, s, j + 1);
        if !s.after.ended {
            assert(part(a, bb, through_cut(cut, rt, rt2, back), s.after));
            assert(in_sequence(a, s.after.pc));
            assert(index(a, s.after.pc) == j + 1);
            assert((blocks[c].needs)(s.after));
        }
    }
}

/// THE ACCESS CUT BY ITS OWN ADDRESSING'S BLOCK HOLDS.
proof fn lemma_own(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    n: int,
    rt: int,
    rt2: int,
    bytes: nat,
    bb: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, 31),
        0 <= j && j + 1 < instructions.len(),
        base_ok(bb) && 0 <= n < 31 && reg_ok(rt, bb) && reg_ok(rt2, bb) && 1 <= bytes <= 16,
        touches(step, instructions[j], rt, rt2, 31, |m: Machine| (m.x[bb] + (m.x[n] & 0xffff_ffffu64)) % two_to_64(), bytes),
        takes(blocks, c, j + 1, a, bb, 31),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, instructions[j]);
        assert(m.pc as int == a.code + 4 * j);
        lemma_low_word(m.x[n]);
        assert((m.x[bb] + (m.x[n] & 0xffff_ffffu64)) % two_to_64() == m.x[bb] + (m.x[n] & 0xffff_ffffu64));
        lemma_in_slot(a, m.x[bb] + (m.x[n] & 0xffff_ffffu64), bytes);
        assert(touches_allowed(a, s.accesses[0].0, s.accesses[0].1));
        lemma_not_table(instructions, a, s, j + 1);
        if !s.after.ended {
            assert(part(a, bb, 31, s.after));
            assert(in_sequence(a, s.after.pc));
            assert(index(a, s.after.pc) == j + 1);
            assert((blocks[c].needs)(s.after));
        }
    }
}

/// THE MARK'S BLOCK HOLDS: its load is of the slot's never mapped half, and it ends the run.
proof fn lemma_mark_block(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, blocks: Seq<Block>, b: int, j: int, bb: int, c: int)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == block_of(a, Desc::Mark { j, bb, c }),
        base_ok(bb) && 0 <= c < 31 && c != bb && c != frame_register(),
        holds_at(instructions, j, seq![eor_c(c, bb), mark_load_c(c), UDF]),
        mark_c_means(step, bb, c),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert(instructions[j] == eor_c(c, bb) && instructions[j + 1] == mark_load_c(c) && instructions[j + 2] == UDF) by {
        assert(instructions.subrange(j, j + 3)[0] == instructions[j]);
        assert(instructions.subrange(j, j + 3)[1] == instructions[j + 1]);
        assert(instructions.subrange(j, j + 3)[2] == instructions[j + 2]);
    }
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        let i = index(a, m.pc);
        assert(word_at(instructions, a, m.pc) == instructions[i]);
        let s = step(m, instructions[i]);
        assert(m.pc as int == a.code + 4 * i);
        if i == j {
            lemma_mark(a, m.x[bb]);
            assert(in_sequence(a, s.after.pc));
            assert(index(a, s.after.pc) == j + 1);
            lemma_not_table(instructions, a, s, j + 1);
        } else if i == j + 1 {
            assert((m.x[c] + 4095) % two_to_64() == a.base + 0x1_0000_0000 + 4095);
            lemma_not_table(instructions, a, s, j + 2);
            if !s.after.ended {
                assert(in_sequence(a, s.after.pc));
                assert(index(a, s.after.pc) == j + 2);
            }
        } else {
        }
    }
}

/// THE PROBES' BLOCK HOLDS: they touch the first and last byte the access reaches, and the run goes on with x[from].
proof fn lemma_probes(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    c: int,
    from: int,
    off: int,
    bytes: nat,
    store: bool,
    bb: int,
    next: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == block_of(a, Desc::Probes { j, c, from, off, bytes, store, bb }),
        base_ok(bb) && 0 <= c < 31 && c != bb && c != from && c != frame_register() && 0 <= from <= 31,
        from == 31 ==> !a.displaced,
        -0x400 <= off && 1 <= bytes && off + bytes <= 0x1_0000,
        j + probes_length(store) < instructions.len(),
        holds_at(instructions, j, probes_words(c, from, off, bytes, store)),
        meant(step, instructions, a, Desc::Probes { j, c, from, off, bytes, store, bb }),
        takes(blocks, next, j + probes_length(store), a, bb, from),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    let w = probes_words(c, from, off, bytes, store);
    assert forall|i: int| 0 <= i < w.len() implies instructions[j + i] == #[trigger] w[i] by {
        assert(instructions.subrange(j, j + w.len())[i] == instructions[j + i]);
    }
    assert(w[0] == movx_c(c, off) && w[1] == add_from_c(c, from));
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        let i = index(a, m.pc);
        assert(word_at(instructions, a, m.pc) == instructions[i]);
        let s = step(m, instructions[i]);
        assert(m.pc as int == a.code + 4 * i);
        assert(part(a, bb, from, m));
        let v = from_value(m, from);
        assert(a.base - STACK_SLACK as int <= v && v <= a.base + WINDOW_BYTES as int + STACK_SLACK as int);
        let last = j + probes_length(store) - 1;
        if i == j {
            assert(instructions[i] == w[0]);
        } else if i == j + 1 {
            assert(instructions[i] == w[1]);
            assert((v + m.x[c]) % two_to_64() == v + off);
        } else if store {
            assert(w[2] == probe_store_c(c) && w[3] == add_c_c(c, bytes - 1) && w[4] == probe_store_c(c));
            if i == j + 2 {
                assert(instructions[i] == w[2]);
                lemma_in_slot(a, m.x[c] as int, 1);
            } else if i == j + 3 {
                assert(instructions[i] == w[3]);
                assert((m.x[c] + (bytes - 1)) % two_to_64() == v + off + bytes - 1);
            } else {
                assert(instructions[i] == w[4]);
                lemma_in_slot(a, m.x[c] as int, 1);
            }
        } else {
            assert(w[2] == probe_load_c(c, 0) && w[3] == probe_load_c(c, bytes - 1));
            if i == j + 2 {
                assert(instructions[i] == w[2]);
                lemma_in_slot(a, m.x[c] as int, 1);
            } else {
                assert(instructions[i] == w[3]);
                lemma_in_slot(a, m.x[c] + (bytes - 1), 1);
            }
        }
        lemma_not_table(instructions, a, s, i + 1);
        if runs(a, s.after) {
            assert(index(a, s.after.pc) == i + 1);
            assert(part(a, bb, from, s.after));
            if i == last {
                assert((blocks[next].needs)(s.after));
            }
        }
    }
}

/// THE EXIT'S BLOCK HOLDS (copy.rs `lemma_exit_step`), and its jump into the slot table hands on.
proof fn lemma_exit_block(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, blocks: Seq<Block>, b: int, j: int, guest: int)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == block_of(a, Desc::Exit { j, guest }),
        0 <= guest && guest + 4 < 0x1_0000_0000,
        holds_at(instructions, j, exit_words(exit_slot(a, guest + 4) as u64)),
        exit_means(step),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert(in_reach(a, guest));
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        let i = index(a, m.pc);
        assert(m.pc as int == a.code + 4 * i);
        assert((m.pc - a.code - 4 * j) / 4 == i - j);
        lemma_exit_step(step, instructions, a, guest, 4 * j, m);
        let s = step(m, word_at(instructions, a, m.pc));
        assert(part(a, 24, 31, s.after));
        if i - j + 1 < 4 {
            assert(index(a, s.after.pc) == i + 1);
            lemma_not_table(instructions, a, s, i + 1);
        }
    }
}

/// THE JUMP'S BLOCK HOLDS (branch.rs `jump_is_confined`, placed anywhere): it lands in the window's slot table with
/// what a run hands on, or enters the block at `out` when its target is outside the window.
proof fn lemma_jump_block(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, blocks: Seq<Block>, b: int, j: int, n: int, out: int, c: int)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == block_of(a, Desc::Jump { j, n, out }),
        !a.displaced && 0 <= n < 31 && j + 1 < out < instructions.len(),
        holds_at(instructions, j, jump_words(n as u32, j, out)),
        jump_main_means(step, n as u32, (out - j - 1) * 4),
        takes(blocks, c, out, a, 24, 31),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    let w = jump_words(n as u32, j, out);
    assert forall|i: int| 0 <= i < 7 implies instructions[j + i] == #[trigger] w[i] by {
        assert(instructions.subrange(j, j + 7)[i] == instructions[j + i]);
    }
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        let i = index(a, m.pc);
        let o = (i - j) * 4;
        assert(m.pc as int == a.code + 4 * i);
        assert(word_at(instructions, a, m.pc) == instructions[i]);
        let s = step(m, instructions[i]);
        if o == 0 {
            assert(instructions[i] == w[0]);
        } else if o == 4 {
            assert(instructions[i] == w[1]);
            if s.after.pc != (m.pc + 4) as u64 {
                assert(s.after.pc as int == a.code + 4 * out);
                lemma_goes_on(step, instructions, a, blocks, m, s, out, c, 24, 31);
            }
        } else if o == 8 {
            assert(instructions[i] == w[2]);
            lemma_low_word(m.x[n]);
        } else if o == 12 {
            assert(instructions[i] == w[3]);
            lemma_slot_address(m.x[23]);
        } else if o == 16 {
            assert(instructions[i] == w[4]);
            let v = (m.x[23] / 4) as u64;
            assert(m.x[23] == v * 4);
            lemma_slot_address(v);
        } else if o == 20 {
            assert(instructions[i] == w[5]);
            let u = m.x[23];
            assert(u ^ 0x4000_0000_0000u64 == u + 0x4000_0000_0000u64) by (bit_vector)
                requires
                    u < 0x4000_0000_0000u64,
            ;
        } else {
            assert(o == 24);
            assert(instructions[i] == w[6]);
            assert(hands_on(a, s.after));
        }
        if runs(a, s.after) && o < 24 && s.after.pc == (m.pc + 4) as u64 {
            assert(index(a, s.after.pc) == i + 1);
        }
        if o < 24 && s.after.pc == (m.pc + 4) as u64 {
            lemma_not_table(instructions, a, s, i + 1);
        }
    }
}

/// THE FLOOR CALL'S BLOCK HOLDS (branch.rs `lemma_floor_step`): it ends in the floor.
proof fn lemma_call_block(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, blocks: Seq<Block>, b: int, j: int, code: int)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == block_of(a, Desc::Call { j, code }),
        0 <= code < 256,
        holds_at(instructions, j, floor_call(floor_entry(code))),
        floor_means(step),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        let i = index(a, m.pc);
        assert(m.pc as int == a.code + 4 * i);
        assert((m.pc - a.code - 4 * j) / 4 == i - j);
        lemma_floor_step(step, instructions, a, code, 4 * j, m);
        let s = step(m, word_at(instructions, a, m.pc));
        if i - j + 1 < 5 {
            lemma_not_table(instructions, a, s, i + 1);
        } else {
            assert(s.after.pc == floor_entry(code));
        }
        if runs(a, s.after) {
            assert(index(a, s.after.pc) == i + 1);
        }
    }
}

/// THE FRAME ACCESS'S OR MOVE'S BLOCK HOLDS: the frame's own words; a move of the base takes it to `bb2`.
proof fn lemma_frame(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    f: FrameAccess,
    bb: int,
    bb2: int,
    cut: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, cut),
        0 <= j && j + 1 < instructions.len(),
        base_ok(bb) && base_ok(bb2) && 0 <= cut <= 31,
        instructions[j] == f.word,
        if f.moves && f.off == bb && f.rt == bb2 { true } else { bb2 == bb && f.rt != bb },
        frame_access_means(step, f),
        takes(blocks, c, j + 1, a, bb2, after_cut(cut, f.rt)),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, f.word);
        assert(m.pc as int == a.code + 4 * j);
        if f.moves {
            assert(part(a, bb2, after_cut(cut, f.rt), s.after));
            lemma_goes_on(step, instructions, a, blocks, m, s, j + 1, c, bb2, after_cut(cut, f.rt));
        } else {
            assert((m.x[28] + f.off) % two_to_64() == a.frame + f.off);
            assert(touches_allowed(a, s.accesses[0].0, s.accesses[0].1));
            assert(s.after.x[28] == m.x[28] && s.after.x[bb] == m.x[bb]);
            lemma_not_table(instructions, a, s, j + 1);
            if !s.after.ended {
                assert(part(a, bb2, after_cut(cut, f.rt), s.after));
                assert(in_sequence(a, s.after.pc));
                assert(index(a, s.after.pc) == j + 1);
                assert((blocks[c].needs)(s.after));
            }
        }
    }
}

/// THE STOP'S BLOCK HOLDS: it ends the run.
proof fn lemma_stop(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, blocks: Seq<Block>, b: int, j: int)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == block_of(a, Desc::Stop { j }),
        0 <= j < instructions.len(),
        stops(step, instructions[j]),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
    }
}

/// THE STACK POINTER'S CUT HOLDS: the window's base plus the low word of xN.
proof fn lemma_stack_cut(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    n: int,
    bb: int,
    cut: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, cut),
        0 <= j && j + 1 < instructions.len(),
        base_ok(bb) && 0 <= cut <= 31 && 0 <= n < 31,
        instructions[j] == add_sp_cut(n as u32, bb),
        sets_sp(step, add_sp_cut(n as u32, bb), |m: Machine| ((m.x[bb] + (m.x[n] & 0xffff_ffffu64)) % two_to_64()) as u64),
        takes(blocks, c, j + 1, a, bb, cut),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, instructions[j]);
        assert(m.pc as int == a.code + 4 * j);
        lemma_low_word(m.x[n]);
        assert(part(a, bb, cut, s.after));
        lemma_goes_on(step, instructions, a, blocks, m, s, j + 1, c, bb, cut);
    }
}

/// THE STACK POINTER'S MOVE HOLDS: from a window address.
proof fn lemma_stack_move(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    bb: int,
    cut: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, cut),
        0 <= j && j + 1 < instructions.len(),
        base_ok(bb) && 0 <= cut < 31,
        instructions[j] == mov_sp_from(cut),
        sets_sp(step, mov_sp_from(cut), |m: Machine| m.x[cut]),
        takes(blocks, c, j + 1, a, bb, cut),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, instructions[j]);
        assert(m.pc as int == a.code + 4 * j);
        assert(part(a, bb, cut, s.after));
        lemma_goes_on(step, instructions, a, blocks, m, s, j + 1, c, bb, cut);
    }
}

/// THE STACK STEP'S BLOCK HOLDS: the probe touches the slot's reservation, and the run goes on only when it found the
/// window there (which way the probe looks does not matter).
proof fn lemma_stack_step(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    k: int,
    off: int,
    bb: int,
    cut: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == block_of(a, Desc::StackStep { j, k, off, bb, cut }),
        !a.displaced && 0 <= j && j + 2 < instructions.len() && base_ok(bb) && 0 <= cut <= 31,
        k != 0 && -0xf000 <= k <= 0xf000 && -0x400 <= off <= 0x400,
        sets_sp(step, instructions[j], |m: Machine| ((m.sp + k) % two_to_64()) as u64),
        stack_touches(step, a, instructions[j + 1], 31, 31, off, 0, 4, true),
        takes(blocks, c, j + 2, a, bb, cut),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let bk = blocks[b];
    assert forall|m: Machine| runs(a, m) && (bk.owns)(index(a, m.pc)) && #[trigger] (bk.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        let i = index(a, m.pc);
        assert(m.pc as int == a.code + 4 * i);
        assert(word_at(instructions, a, m.pc) == instructions[i]);
        let s = step(m, instructions[i]);
        if i == j {
            assert((m.sp + k) % two_to_64() == m.sp + k);
            assert(near(a, s.after.sp, k));
            assert(index(a, s.after.pc) == j + 1);
            lemma_not_table(instructions, a, s, j + 1);
        } else {
            let first = (m.sp + off) % two_to_64();
            assert(first == m.sp + off);
            lemma_in_slot(a, first, 4);
            lemma_not_table(instructions, a, s, j + 2);
            if !s.after.ended {
                assert(in_window_bytes(a, first, 4));
                assert(s.after.sp == m.sp);
                assert(part(a, bb, cut, s.after));
                assert(index(a, s.after.pc) == j + 2);
                assert((blocks[c].needs)(s.after));
            }
        }
    }
}

/// THE STACK ACCESS'S BLOCK HOLDS: within the slack of the window and its offset; moved, only from the window.
proof fn lemma_stack_access(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    rt: int,
    rt2: int,
    off: int,
    moves: int,
    bytes: nat,
    bb: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, 31),
        !a.displaced && 0 <= j && j + 1 < instructions.len() && base_ok(bb) && reg_ok(rt, bb) && reg_ok(rt2, bb),
        -0x400 <= off && 1 <= bytes && off + bytes <= 0x1_0000 && -0x400 <= moves <= 0x400,
        moves != 0 ==> off == 0 || off == moves,
        stack_touches(step, a, instructions[j], rt, rt2, off, moves, bytes, moves != 0),
        takes(blocks, c, j + 1, a, bb, 31),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, instructions[j]);
        assert(m.pc as int == a.code + 4 * j);
        let first = (m.sp + off) % two_to_64();
        assert(first == m.sp + off);
        lemma_in_slot(a, first, bytes);
        lemma_not_table(instructions, a, s, j + 1);
        if !s.after.ended {
            assert(s.after.sp as int == m.sp + moves);
            assert(part(a, bb, 31, s.after));
            assert(in_sequence(a, s.after.pc));
            assert(index(a, s.after.pc) == j + 1);
            assert((blocks[c].needs)(s.after));
        }
    }
}

/// THE ACCESS THROUGH A COPY OF THE STACK POINTER HOLDS.
proof fn lemma_stack_via(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    rt: int,
    rt2: int,
    bytes: nat,
    bb: int,
    cut: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == block_of(a, Desc::StackVia { j, rt, rt2, bytes, bb, cut }),
        !a.displaced && 0 <= j && j + 2 < instructions.len() && base_ok(bb) && 0 <= cut <= 31 && cut != 23,
        reg_ok(rt, bb) && reg_ok(rt2, bb) && (cut < 31 ==> rt != cut && rt2 != cut) && 1 <= bytes <= 0x1_0000,
        instructions[j] == STACK_TO_X23,
        writes(step, STACK_TO_X23, 23, |m: Machine| m.sp),
        touches(step, instructions[j + 1], rt, rt2, 23, |m: Machine| m.x[23] as int, bytes),
        takes(blocks, c, j + 2, a, bb, cut),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        let i = index(a, m.pc);
        assert(m.pc as int == a.code + 4 * i);
        assert(word_at(instructions, a, m.pc) == instructions[i]);
        let s = step(m, instructions[i]);
        if i == j {
            assert(part(a, bb, cut, s.after));
            assert(index(a, s.after.pc) == j + 1);
            lemma_not_table(instructions, a, s, j + 1);
        } else {
            lemma_in_slot(a, m.x[23] as int, bytes);
            lemma_not_table(instructions, a, s, j + 2);
            if !s.after.ended {
                assert(part(a, bb, cut, s.after));
                assert(index(a, s.after.pc) == j + 2);
                assert((blocks[c].needs)(s.after));
            }
        }
    }
}

/// THE STACK PROBES' BLOCK HOLDS: they touch the first and last byte at the stack pointer.
proof fn lemma_stack_probes(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    c: int,
    bytes: nat,
    store: bool,
    bb: int,
    next: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == block_of(a, Desc::StackProbes { j, c, bytes, store, bb }),
        !a.displaced && base_ok(bb) && 0 <= c < 31 && c != bb && c != frame_register() && 1 <= bytes <= 0x1_0000,
        j + stack_probes_length(store) < instructions.len(),
        holds_at(instructions, j, stack_probes_words(c, bytes, store)),
        meant(step, instructions, a, Desc::StackProbes { j, c, bytes, store, bb }),
        takes(blocks, next, j + stack_probes_length(store), a, bb, 31),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    let w = stack_probes_words(c, bytes, store);
    assert forall|i: int| 0 <= i < w.len() implies instructions[j + i] == #[trigger] w[i] by {
        assert(instructions.subrange(j, j + w.len())[i] == instructions[j + i]);
    }
    assert(w[0] == mov_c_sp(c));
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        let i = index(a, m.pc);
        assert(word_at(instructions, a, m.pc) == instructions[i]);
        let s = step(m, instructions[i]);
        assert(m.pc as int == a.code + 4 * i);
        assert(part(a, bb, 31, m));
        let last = j + stack_probes_length(store) - 1;
        if i == j {
            assert(instructions[i] == w[0]);
        } else if store {
            assert(w[1] == probe_store_c(c) && w[2] == add_c_c(c, bytes - 1) && w[3] == probe_store_c(c));
            if i == j + 1 {
                assert(instructions[i] == w[1]);
                lemma_in_slot(a, m.x[c] as int, 1);
            } else if i == j + 2 {
                assert(instructions[i] == w[2]);
                assert((m.x[c] + (bytes - 1)) % two_to_64() == m.sp + bytes - 1);
            } else {
                assert(instructions[i] == w[3]);
                lemma_in_slot(a, m.x[c] as int, 1);
            }
        } else {
            assert(w[1] == probe_load_c(c, 0) && w[2] == probe_load_c(c, bytes - 1));
            if i == j + 1 {
                assert(instructions[i] == w[1]);
                lemma_in_slot(a, m.x[c] as int, 1);
            } else {
                assert(instructions[i] == w[2]);
                lemma_in_slot(a, m.x[c] + (bytes - 1), 1);
            }
        }
        lemma_not_table(instructions, a, s, i + 1);
        if runs(a, s.after) {
            assert(index(a, s.after.pc) == i + 1);
            assert(part(a, bb, 31, s.after));
            if i == last {
                assert((blocks[next].needs)(s.after));
            }
        }
    }
}

/// THE MOVE OF THE CUT ONTO ITSELF HOLDS: it stays.
proof fn lemma_same(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    bb: int,
    cut: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, cut),
        0 <= j && j + 1 < instructions.len(),
        base_ok(bb) && 0 <= cut < 31,
        instructions[j] == same_word(cut),
        writes(step, same_word(cut), cut, |m: Machine| m.x[cut]),
        takes(blocks, c, j + 1, a, bb, cut),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, instructions[j]);
        assert(m.pc as int == a.code + 4 * j);
        assert(s.after.x =~= m.x);
        assert(part(a, bb, cut, s.after));
        lemma_goes_on(step, instructions, a, blocks, m, s, j + 1, c, bb, cut);
    }
}

/// THE DISPLACED JUMP'S BLOCK HOLDS (branch.rs `displaced_jump_is_confined`, placed anywhere): it lands in slot 0's
/// table with what a run hands on, or enters the block at `out`.
proof fn lemma_displaced_jump_block(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, blocks: Seq<Block>, b: int, j: int, n: int, out: int, c: int)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == block_of(a, Desc::DisplacedJump { j, n, out }),
        a.displaced && 0 <= n < 31 && j + 1 < out < instructions.len(),
        holds_at(instructions, j, displaced_jump_words(n as u32, j, out)),
        displaced_jump_main_means(step, n as u32, (out - j - 1) * 4),
        takes(blocks, c, out, a, 24, 31),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    let w = displaced_jump_words(n as u32, j, out);
    assert forall|i: int| 0 <= i < 6 implies instructions[j + i] == #[trigger] w[i] by {
        assert(instructions.subrange(j, j + 6)[i] == instructions[j + i]);
    }
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        let i = index(a, m.pc);
        let o = (i - j) * 4;
        assert(m.pc as int == a.code + 4 * i);
        assert(word_at(instructions, a, m.pc) == instructions[i]);
        let s = step(m, instructions[i]);
        if o == 0 {
            assert(instructions[i] == w[0]);
        } else if o == 4 {
            assert(instructions[i] == w[1]);
            if s.after.pc != (m.pc + 4) as u64 {
                assert(s.after.pc as int == a.code + 4 * out);
                lemma_goes_on(step, instructions, a, blocks, m, s, out, c, 24, 31);
            }
        } else if o == 8 {
            assert(instructions[i] == w[2]);
            lemma_low_word_times_four(m.x[n]);
        } else if o == 12 {
            assert(instructions[i] == w[3]);
            let u = m.x[23];
            assert(u <= ((u & !3u64) | ((u >> 2u64) & 3u64)) && ((u & !3u64) | ((u >> 2u64) & 3u64)) <= u + 3) by (bit_vector)
                requires
                    u & 3u64 == 0,
                    u < 0x4_0000_0000u64,
            ;
        } else if o == 16 {
            assert(instructions[i] == w[4]);
            let u = m.x[23];
            assert(u ^ 0x4000_0000_0000u64 == u + 0x4000_0000_0000u64) by (bit_vector)
                requires
                    u < 0x4_0000_0003u64,
            ;
        } else {
            assert(o == 20);
            assert(instructions[i] == w[5]);
            assert(hands_on(a, s.after));
        }
        if runs(a, s.after) && o < 20 && s.after.pc == (m.pc + 4) as u64 {
            assert(index(a, s.after.pc) == i + 1);
        }
        if o < 20 && s.after.pc == (m.pc + 4) as u64 {
            lemma_not_table(instructions, a, s, i + 1);
        }
    }
}

/// THE ACCESS AT THE STACK POINTER PLUS AN OFFSET HOLDS.
proof fn lemma_stack_at(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    off: int,
    rt: int,
    rt2: int,
    bytes: nat,
    bb: int,
    cut: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == block_of(a, Desc::StackAt { j, off, rt, rt2, bytes, bb, cut }),
        !a.displaced && 0 <= j && j + 3 < instructions.len() && base_ok(bb) && 0 <= cut <= 31 && cut != 23,
        reg_ok(rt, bb) && reg_ok(rt2, bb) && (cut < 31 ==> rt != cut && rt2 != cut),
        -0x400 <= off && 1 <= bytes && off + bytes <= 0x1_0000,
        holds_at(instructions, j, seq![movx_c(23, off), add_from_c(23, 31)]),
        meant(step, instructions, a, Desc::StackAt { j, off, rt, rt2, bytes, bb, cut }),
        takes(blocks, c, j + 3, a, bb, cut),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert(instructions[j] == movx_c(23, off) && instructions[j + 1] == add_from_c(23, 31)) by {
        assert(instructions.subrange(j, j + 2)[0] == instructions[j]);
        assert(instructions.subrange(j, j + 2)[1] == instructions[j + 1]);
    }
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        let i = index(a, m.pc);
        assert(m.pc as int == a.code + 4 * i);
        assert(word_at(instructions, a, m.pc) == instructions[i]);
        let s = step(m, instructions[i]);
        if i == j {
            assert(part(a, bb, cut, s.after));
        } else if i == j + 1 {
            assert((m.sp + m.x[23]) % two_to_64() == m.sp + off);
            assert(part(a, bb, cut, s.after));
        } else {
            lemma_in_slot(a, m.x[23] as int, bytes);
        }
        lemma_not_table(instructions, a, s, i + 1);
        if runs(a, s.after) {
            assert(index(a, s.after.pc) == i + 1);
            assert(part(a, bb, cut, s.after));
            if i == j + 2 {
                assert((blocks[c].needs)(s.after));
            }
        }
    }
}

/// Every block of a described piece holds alone.
proof fn lemma_block(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, descs: Seq<Desc>, b: int)
    requires
        placed(a, instructions),
        0 <= b < descs.len(),
        shape_ok(instructions, a, descs, descs[b]),
        meant(step, instructions, a, descs[b]),
    ensures
        block_holds(step, instructions, a, blocks_of(a, descs), b),
{
    let blocks = blocks_of(a, descs);
    assert(blocks[b] == block_of(a, descs[b]));
    match descs[b] {
        Desc::Register { j, rd, sp, bb, cut } => {
            let c = lemma_accepts(a, descs, j + 1, bb, after_cut(cut, rd));
            lemma_register(step, instructions, a, blocks, b, j, rd, sp, bb, cut, c);
        },
        Desc::Cut { j, n, c: cr, bb } => {
            let c = lemma_accepts(a, descs, j + 1, bb, cr);
            lemma_cut(step, instructions, a, blocks, b, j, n, cr, bb, c);
        },
        Desc::Either { j, by, bb, cut } => {
            let c = lemma_accepts(a, descs, j + 1, bb, cut);
            let t = lemma_accepts(a, descs, j + by / 4, bb, cut);
            lemma_either(step, instructions, a, blocks, b, j, by, bb, cut, c, t);
        },
        Desc::Branch { j, by, bb, cut } => {
            let t = lemma_accepts(a, descs, j + by / 4, bb, cut);
            lemma_branch(step, instructions, a, blocks, b, j, by, bb, cut, t);
        },
        Desc::Through { j, rt, rt2, back, off, bytes, bb, cut } => {
            let c = lemma_accepts(a, descs, j + 1, bb, through_cut(cut, rt, rt2, back));
            lemma_through(step, instructions, a, blocks, b, j, rt, rt2, back, off, bytes, bb, cut, c);
        },
        Desc::Own { j, n, rt, rt2, bytes, bb } => {
            let c = lemma_accepts(a, descs, j + 1, bb, 31);
            lemma_own(step, instructions, a, blocks, b, j, n, rt, rt2, bytes, bb, c);
        },
        Desc::Mark { j, bb, c } => {
            lemma_mark_block(step, instructions, a, blocks, b, j, bb, c);
        },
        Desc::Probes { j, c, from, off, bytes, store, bb } => {
            let next = lemma_accepts(a, descs, j + probes_length(store), bb, from);
            lemma_probes(step, instructions, a, blocks, b, j, c, from, off, bytes, store, bb, next);
        },
        Desc::Exit { j, guest } => {
            lemma_exit_block(step, instructions, a, blocks, b, j, guest);
        },
        Desc::Jump { j, n, out } => {
            let c = lemma_accepts(a, descs, out, 24, 31);
            lemma_jump_block(step, instructions, a, blocks, b, j, n, out, c);
        },
        Desc::Call { j, code } => {
            lemma_call_block(step, instructions, a, blocks, b, j, code);
        },
        Desc::Frame { j, f, bb, bb2, cut } => {
            let c = lemma_accepts(a, descs, j + 1, bb2, after_cut(cut, f.rt));
            lemma_frame(step, instructions, a, blocks, b, j, f, bb, bb2, cut, c);
        },
        Desc::Stop { j } => {
            lemma_stop(step, instructions, a, blocks, b, j);
        },
        Desc::StackCut { j, n, bb, cut } => {
            let c = lemma_accepts(a, descs, j + 1, bb, cut);
            lemma_stack_cut(step, instructions, a, blocks, b, j, n, bb, cut, c);
        },
        Desc::StackMove { j, bb, cut } => {
            let c = lemma_accepts(a, descs, j + 1, bb, cut);
            lemma_stack_move(step, instructions, a, blocks, b, j, bb, cut, c);
        },
        Desc::StackStep { j, k, off, bb, cut } => {
            let c = lemma_accepts(a, descs, j + 2, bb, cut);
            lemma_stack_step(step, instructions, a, blocks, b, j, k, off, bb, cut, c);
        },
        Desc::StackAccess { j, rt, rt2, off, moves, bytes, bb } => {
            let c = lemma_accepts(a, descs, j + 1, bb, 31);
            lemma_stack_access(step, instructions, a, blocks, b, j, rt, rt2, off, moves, bytes, bb, c);
        },
        Desc::StackVia { j, rt, rt2, bytes, bb, cut } => {
            let c = lemma_accepts(a, descs, j + 2, bb, cut);
            lemma_stack_via(step, instructions, a, blocks, b, j, rt, rt2, bytes, bb, cut, c);
        },
        Desc::StackProbes { j, c, bytes, store, bb } => {
            let next = lemma_accepts(a, descs, j + stack_probes_length(store), bb, 31);
            lemma_stack_probes(step, instructions, a, blocks, b, j, c, bytes, store, bb, next);
        },
        Desc::Same { j, bb, cut } => {
            let c = lemma_accepts(a, descs, j + 1, bb, cut);
            lemma_same(step, instructions, a, blocks, b, j, bb, cut, c);
        },
        Desc::StackAt { j, off, rt, rt2, bytes, bb, cut } => {
            let c = lemma_accepts(a, descs, j + 3, bb, cut);
            lemma_stack_at(step, instructions, a, blocks, b, j, off, rt, rt2, bytes, bb, cut, c);
        },
        Desc::DisplacedJump { j, n, out } => {
            let c = lemma_accepts(a, descs, out, 24, 31);
            lemma_displaced_jump_block(step, instructions, a, blocks, b, j, n, out, c);
        },
    }
}

proof fn lemma_shapes(instructions: Seq<u32>, a: Allowed, all: Seq<Desc>, k: int, i: int)
    requires
        shapes_ok(instructions, a, all, k),
        0 <= i < k <= all.len(),
    ensures
        shape_ok(instructions, a, all, all[i]),
    decreases k,
{
    if i < k - 1 {
        lemma_shapes(instructions, a, all, k - 1, i);
    }
}

/// THE COMPOSITION AND THE CHAIN: every described piece whose words mean what its description says is chained.
pub proof fn described_is_chained(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, descs: Seq<Desc>)
    requires
        placed(a, instructions),
        described(instructions, a, descs),
        all_meant(step, instructions, a, descs),
    ensures
        is_chained(step, instructions, a),
{
    let blocks = blocks_of(a, descs);
    assert forall|b: int| 0 <= b < blocks.len() implies #[trigger] block_holds(step, instructions, a, blocks, b) by {
        lemma_shapes(instructions, a, descs, descs.len() as int, b);
        assert(meant(step, instructions, a, descs[b]));
        lemma_block(step, instructions, a, descs, b);
    }
    lemma_shapes(instructions, a, descs, descs.len() as int, 0);
    assert(blocks[0] == block_of(a, descs[0]));
    assert forall|m: Machine| handed(a, m) implies #[trigger] (blocks[0].needs)(m) by {
        assert(part(a, 24, 31, m));
    }
    assert(instructions.len() > 0) by {
        match descs[0] {
            Desc::Mark { j, .. } => { assert(instructions.subrange(j, j + 3).len() == 3); },
            Desc::Probes { j, c, from, off, bytes, store, .. } => {
                assert(instructions.subrange(j, j + probes_words(c, from, off, bytes, store).len()).len() > 0);
            },
            Desc::Exit { j, guest } => { assert(instructions.subrange(j, j + 4).len() == 4); },
            Desc::Jump { j, .. } => { assert(instructions.subrange(j, j + 7).len() == 7); },
            Desc::Call { j, .. } => { assert(instructions.subrange(j, j + 5).len() == 5); },
            Desc::StackProbes { j, c, bytes, store, .. } => {
                assert(instructions.subrange(j, j + stack_probes_words(c, bytes, store).len()).len() > 0);
            },
            Desc::DisplacedJump { j, .. } => { assert(instructions.subrange(j, j + 6).len() == 6); },
            _ => {},
        }
    }
    lemma_blocks_chain(step, instructions, a, blocks);
}

/// THE COMPOSITION: every described piece whose words mean what its description says is confined.
pub proof fn described_is_confined(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, descs: Seq<Desc>)
    requires
        placed(a, instructions),
        described(instructions, a, descs),
        all_meant(step, instructions, a, descs),
    ensures
        is_confined(step, instructions, a),
{
    described_is_chained(step, instructions, a, descs);
    lemma_chained_confines(step, instructions, a);
}

/// Pieces are described alike wherever they are placed: a description names its window only by its base and way.
pub proof fn lemma_described_anywhere(instructions: Seq<u32>, a: Allowed, b: Allowed, descs: Seq<Desc>, k: int)
    requires
        a.base == b.base && a.displaced == b.displaced,
        shapes_ok(instructions, a, descs, k),
    ensures
        shapes_ok(instructions, b, descs, k),
    decreases k,
{
    if k > 0 {
        assert(slot_table(a) == slot_table(b));
        lemma_described_anywhere(instructions, a, b, descs, k - 1);
    }
}

/// The pins: pieces the translator emits for the guest code `tests/compose.rs` names, each with its description, which
/// `compute` checks is one `described` accepts (for slot 3; [`lemma_described_anywhere`] carries it to every placing).
pub open spec fn pin_at(displaced: bool) -> Allowed {
    Allowed { base: 0x6_0000_0000, below: 0x1_0000, frame: 0, code: 0, length: 0, displaced }
}

proof fn pin_store_tested_loop() { assert(described(seq![0x92400837u32, 0xb40003f7u32, 0x91000652u32, 0xd360dc2fu32, 0xb500016fu32, 0x8b214317u32, 0x9240082fu32, 0xb500016fu32, 0xf90002e2u32, 0xf1000400u32, 0x54fffec1u32, 0xd2880617u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff3u32, 0xd280000fu32, 0x8b2f62efu32, 0x383f01ffu32, 0x91001defu32, 0x383f01ffu32, 0x17ffffedu32, 0x17ffffe4u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0x91000652u32, 0xd360dc2fu32, 0xb500010fu32, 0xf8214b02u32, 0xf1000400u32, 0x54ffff61u32, 0xd2880617u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x17fffff3u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32], pin_at(false), seq![Desc::Register { j: 0, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 124, bb: 24, cut: 31 }, Desc::Register { j: 2, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 4, by: 44, bb: 24, cut: 31 }, Desc::Cut { j: 5, n: 1, c: 23, bb: 24 }, Desc::Register { j: 6, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 7, by: 44, bb: 24, cut: 23 }, Desc::Through { j: 8, rt: 31, rt2: 31, back: false, off: 0, bytes: 8, bb: 24, cut: 23 }, Desc::Register { j: 9, rd: 0, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 10, by: -40, bb: 24, cut: 23 }, Desc::Exit { j: 11, guest: 0x401008 }, Desc::Mark { j: 15, bb: 24, c: 15 }, Desc::Register { j: 18, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 19, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 20, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 21, by: -52, bb: 24, cut: 23 }, Desc::Probes { j: 22, c: 15, from: 23, off: 0, bytes: 8, store: true, bb: 24 }, Desc::Branch { j: 27, by: -76, bb: 24, cut: 23 }, Desc::Register { j: 32, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 33, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 34, by: 32, bb: 24, cut: 31 }, Desc::Own { j: 35, n: 1, rt: 31, rt2: 31, bytes: 8, bb: 24 }, Desc::Register { j: 36, rd: 0, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 37, by: -20, bb: 24, cut: 31 }, Desc::Exit { j: 38, guest: 0x401008 }, Desc::Mark { j: 42, bb: 24, c: 15 }])) by (compute); }
proof fn pin_post_load_loop() { assert(described(seq![0x91000652u32, 0xd360dc2fu32, 0xb50001afu32, 0x8b214317u32, 0x9240082fu32, 0xb50001afu32, 0x9100202fu32, 0xf84086e2u32, 0xaa0f03e1u32, 0xf1000400u32, 0x54fffec1u32, 0xd2880617u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff1u32, 0xd280000fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39401dffu32, 0x17ffffecu32, 0x17ffffe5u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32], pin_at(false), seq![Desc::Register { j: 0, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 52, bb: 24, cut: 31 }, Desc::Cut { j: 3, n: 1, c: 23, bb: 24 }, Desc::Register { j: 4, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 5, by: 52, bb: 24, cut: 23 }, Desc::Register { j: 6, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Through { j: 7, rt: 2, rt2: 31, back: true, off: 0, bytes: 8, bb: 24, cut: 23 }, Desc::Register { j: 8, rd: 1, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 9, rd: 0, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 10, by: -40, bb: 24, cut: 31 }, Desc::Exit { j: 11, guest: 0x401008 }, Desc::Mark { j: 15, bb: 24, c: 15 }, Desc::Register { j: 18, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 19, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 20, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 21, by: -60, bb: 24, cut: 23 }, Desc::Probes { j: 22, c: 15, from: 23, off: 0, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 26, by: -80, bb: 24, cut: 23 }])) by (compute); }
proof fn pin_post_store_loop() { assert(described(seq![0x91000652u32, 0xd360dc2fu32, 0xb50001afu32, 0x8b214317u32, 0x9240082fu32, 0xb50001afu32, 0x9100202fu32, 0xf80086e2u32, 0xaa0f03e1u32, 0xf1000400u32, 0x54fffec1u32, 0xd2880617u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff1u32, 0xd280000fu32, 0x8b2f62efu32, 0x383f01ffu32, 0x91001defu32, 0x383f01ffu32, 0x17ffffebu32, 0x17ffffe4u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32], pin_at(false), seq![Desc::Register { j: 0, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 52, bb: 24, cut: 31 }, Desc::Cut { j: 3, n: 1, c: 23, bb: 24 }, Desc::Register { j: 4, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 5, by: 52, bb: 24, cut: 23 }, Desc::Register { j: 6, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Through { j: 7, rt: 31, rt2: 31, back: true, off: 0, bytes: 8, bb: 24, cut: 23 }, Desc::Register { j: 8, rd: 1, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 9, rd: 0, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 10, by: -40, bb: 24, cut: 31 }, Desc::Exit { j: 11, guest: 0x401008 }, Desc::Mark { j: 15, bb: 24, c: 15 }, Desc::Register { j: 18, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 19, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 20, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 21, by: -60, bb: 24, cut: 23 }, Desc::Probes { j: 22, c: 15, from: 23, off: 0, bytes: 8, store: true, bb: 24 }, Desc::Branch { j: 27, by: -84, bb: 24, cut: 23 }])) by (compute); }
proof fn pin_post_load_line() { assert(described(seq![0xd360dc2fu32, 0xb500018fu32, 0x8b214317u32, 0x9240082fu32, 0xb500018fu32, 0x9100202fu32, 0xf84086e2u32, 0xaa0f03e1u32, 0xf1000400u32, 0xd2880417u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff2u32, 0xd280000fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39401dffu32, 0x17ffffedu32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 48, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 1, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 4, by: 48, bb: 24, cut: 23 }, Desc::Register { j: 5, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Through { j: 6, rt: 2, rt2: 31, back: true, off: 0, bytes: 8, bb: 24, cut: 23 }, Desc::Register { j: 7, rd: 1, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 8, rd: 0, sp: false, bb: 24, cut: 31 }, Desc::Exit { j: 9, guest: 0x401004 }, Desc::Mark { j: 13, bb: 24, c: 15 }, Desc::Register { j: 16, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 17, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 18, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 19, by: -56, bb: 24, cut: 23 }, Desc::Probes { j: 20, c: 15, from: 23, off: 0, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 24, by: -76, bb: 24, cut: 23 }])) by (compute); }
proof fn pin_line_to_jump() { assert(described(seq![0x91000652u32, 0x91000400u32, 0xd360dfd7u32, 0xb50000d7u32, 0x8b3e4317u32, 0xd37eaef7u32, 0xb3420ef7u32, 0xd25202f7u32, 0xd61f02e0u32, 0x14000001u32, 0xf9002f9eu32, 0xd2800e1eu32, 0xf2a0001eu32, 0xf2c4003eu32, 0xd63f03c0u32, 0xf9402f9eu32, 0x17fffffau32], pin_at(false), seq![Desc::Register { j: 0, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 0, sp: false, bb: 24, cut: 31 }, Desc::Jump { j: 2, n: 30, out: 9 }, Desc::Branch { j: 9, by: 4, bb: 24, cut: 31 }, Desc::Call { j: 10, code: 7 }])) by (compute); }
proof fn pin_stand_in_add_line() { assert(described(seq![0xf9004f9du32, 0xaa1803fdu32, 0xf9404798u32, 0x91000718u32, 0x91000400u32, 0xf9004798u32, 0xaa1d03f8u32, 0xf9404f9du32, 0xd2880417u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32], pin_at(false), seq![Desc::Frame { j: 0, f: FrameAccess { word: 0xf9004f9du32, rt: 31, off: 152, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::Frame { j: 1, f: FrameAccess { word: 0xaa1803fdu32, rt: 29, off: 24, moves: true }, bb: 24, bb2: 29, cut: 31 }, Desc::Frame { j: 2, f: FrameAccess { word: 0xf9404798u32, rt: 24, off: 136, moves: false }, bb: 29, bb2: 29, cut: 31 }, Desc::Register { j: 3, rd: 24, sp: false, bb: 29, cut: 31 }, Desc::Register { j: 4, rd: 0, sp: false, bb: 29, cut: 31 }, Desc::Frame { j: 5, f: FrameAccess { word: 0xf9004798u32, rt: 31, off: 136, moves: false }, bb: 29, bb2: 29, cut: 31 }, Desc::Frame { j: 6, f: FrameAccess { word: 0xaa1d03f8u32, rt: 24, off: 29, moves: true }, bb: 29, bb2: 24, cut: 31 }, Desc::Frame { j: 7, f: FrameAccess { word: 0xf9404f9du32, rt: 29, off: 152, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::Exit { j: 8, guest: 0x401004 }])) by (compute); }
proof fn pin_stand_in_load_line() { assert(described(seq![0xf9004f9du32, 0xaa1803fdu32, 0xf9404798u32, 0xd360df0fu32, 0xb50001afu32, 0x8b3843b7u32, 0x92400b0fu32, 0xb50001afu32, 0xf94002e2u32, 0x91000442u32, 0xf9004798u32, 0xaa1d03f8u32, 0xf9404f9du32, 0xd2880417u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd26003afu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff1u32, 0xd280000fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39401dffu32, 0x17ffffecu32], pin_at(false), seq![Desc::Frame { j: 0, f: FrameAccess { word: 0xf9004f9du32, rt: 31, off: 152, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::Frame { j: 1, f: FrameAccess { word: 0xaa1803fdu32, rt: 29, off: 24, moves: true }, bb: 24, bb2: 29, cut: 31 }, Desc::Frame { j: 2, f: FrameAccess { word: 0xf9404798u32, rt: 24, off: 136, moves: false }, bb: 29, bb2: 29, cut: 31 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 29, cut: 31 }, Desc::Either { j: 4, by: 52, bb: 29, cut: 31 }, Desc::Cut { j: 5, n: 24, c: 23, bb: 29 }, Desc::Register { j: 6, rd: 15, sp: false, bb: 29, cut: 23 }, Desc::Either { j: 7, by: 52, bb: 29, cut: 23 }, Desc::Through { j: 8, rt: 2, rt2: 31, back: false, off: 0, bytes: 8, bb: 29, cut: 23 }, Desc::Register { j: 9, rd: 2, sp: false, bb: 29, cut: 23 }, Desc::Frame { j: 10, f: FrameAccess { word: 0xf9004798u32, rt: 31, off: 136, moves: false }, bb: 29, bb2: 29, cut: 23 }, Desc::Frame { j: 11, f: FrameAccess { word: 0xaa1d03f8u32, rt: 24, off: 29, moves: true }, bb: 29, bb2: 24, cut: 23 }, Desc::Frame { j: 12, f: FrameAccess { word: 0xf9404f9du32, rt: 29, off: 152, moves: false }, bb: 24, bb2: 24, cut: 23 }, Desc::Exit { j: 13, guest: 0x401004 }, Desc::Mark { j: 17, bb: 29, c: 15 }, Desc::Register { j: 20, rd: 15, sp: false, bb: 29, cut: 23 }, Desc::Register { j: 21, rd: 15, sp: false, bb: 29, cut: 23 }, Desc::Either { j: 22, by: 8, bb: 29, cut: 23 }, Desc::Branch { j: 23, by: -60, bb: 29, cut: 23 }, Desc::Probes { j: 24, c: 15, from: 23, off: 0, bytes: 8, store: false, bb: 29 }, Desc::Branch { j: 28, by: -80, bb: 29, cut: 23 }])) by (compute); }
proof fn pin_load_4096() { assert(described(seq![0xd360dc2fu32, 0xb500012fu32, 0x8b214317u32, 0x9240082fu32, 0xb500012fu32, 0xf94802e2u32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff5u32, 0xd282000fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39401dffu32, 0x17fffff0u32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 36, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 1, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 4, by: 36, bb: 24, cut: 23 }, Desc::Through { j: 5, rt: 2, rt2: 31, back: false, off: 4096, bytes: 8, bb: 24, cut: 23 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Mark { j: 10, bb: 24, c: 15 }, Desc::Register { j: 13, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 14, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 15, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 16, by: -44, bb: 24, cut: 23 }, Desc::Probes { j: 17, c: 15, from: 23, off: 4096, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 21, by: -64, bb: 24, cut: 23 }])) by (compute); }
proof fn pin_store_4096() { assert(described(seq![0xd360dc2fu32, 0xb500012fu32, 0x8b214317u32, 0x9240082fu32, 0xb500012fu32, 0xf90802e2u32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff5u32, 0xd282000fu32, 0x8b2f62efu32, 0x383f01ffu32, 0x91001defu32, 0x383f01ffu32, 0x17ffffefu32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 36, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 1, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 4, by: 36, bb: 24, cut: 23 }, Desc::Through { j: 5, rt: 31, rt2: 31, back: false, off: 4096, bytes: 8, bb: 24, cut: 23 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Mark { j: 10, bb: 24, c: 15 }, Desc::Register { j: 13, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 14, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 15, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 16, by: -44, bb: 24, cut: 23 }, Desc::Probes { j: 17, c: 15, from: 23, off: 4096, bytes: 8, store: true, bb: 24 }, Desc::Branch { j: 22, by: -68, bb: 24, cut: 23 }])) by (compute); }
proof fn pin_load_32760() { assert(described(seq![0xd360dc2fu32, 0xb500012fu32, 0x8b214317u32, 0x9240082fu32, 0xb500012fu32, 0xf97ffee2u32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x913ffeefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff5u32, 0xd28fff0fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39401dffu32, 0x17fffff0u32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 36, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 1, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 4, by: 36, bb: 24, cut: 23 }, Desc::Through { j: 5, rt: 2, rt2: 31, back: false, off: 32760, bytes: 8, bb: 24, cut: 23 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Mark { j: 10, bb: 24, c: 15 }, Desc::Register { j: 13, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 14, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 15, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 16, by: -44, bb: 24, cut: 23 }, Desc::Probes { j: 17, c: 15, from: 23, off: 32760, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 21, by: -64, bb: 24, cut: 23 }])) by (compute); }
proof fn pin_load_word_4096() { assert(described(seq![0xd360dc2fu32, 0xb500012fu32, 0x8b214317u32, 0x9240042fu32, 0xb500012fu32, 0xb95002e2u32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91000eefu32, 0x927e25efu32, 0xb400004fu32, 0x17fffff5u32, 0xd282000fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39400dffu32, 0x17fffff0u32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 36, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 1, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 4, by: 36, bb: 24, cut: 23 }, Desc::Through { j: 5, rt: 2, rt2: 31, back: false, off: 4096, bytes: 4, bb: 24, cut: 23 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Mark { j: 10, bb: 24, c: 15 }, Desc::Register { j: 13, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 14, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 15, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 16, by: -44, bb: 24, cut: 23 }, Desc::Probes { j: 17, c: 15, from: 23, off: 4096, bytes: 4, store: false, bb: 24 }, Desc::Branch { j: 21, by: -64, bb: 24, cut: 23 }])) by (compute); }
proof fn pin_load_byte_4095() { assert(described(seq![0xd360dc2fu32, 0xb50000efu32, 0x8b214317u32, 0x397ffee2u32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 28, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 1, c: 23, bb: 24 }, Desc::Through { j: 3, rt: 2, rt2: 31, back: false, off: 4095, bytes: 1, bb: 24, cut: 23 }, Desc::Exit { j: 4, guest: 0x401000 }, Desc::Mark { j: 8, bb: 24, c: 15 }])) by (compute); }
proof fn pin_load_4096_to_jump() { assert(described(seq![0x91000652u32, 0xd360dc2fu32, 0xb500018fu32, 0x8b214317u32, 0x9240082fu32, 0xb500018fu32, 0xf94802e2u32, 0xd360dfd7u32, 0xb5000257u32, 0x8b3e4317u32, 0xd37eaef7u32, 0xb3420ef7u32, 0xd25202f7u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff2u32, 0xd282000fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39401dffu32, 0x17ffffedu32, 0x14000001u32, 0xf9002f9eu32, 0xd2800e1eu32, 0xf2a0001eu32, 0xf2c4003eu32, 0xd63f03c0u32, 0xf9402f9eu32, 0x17fffffau32], pin_at(false), seq![Desc::Register { j: 0, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 48, bb: 24, cut: 31 }, Desc::Cut { j: 3, n: 1, c: 23, bb: 24 }, Desc::Register { j: 4, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 5, by: 48, bb: 24, cut: 23 }, Desc::Through { j: 6, rt: 2, rt2: 31, back: false, off: 4096, bytes: 8, bb: 24, cut: 23 }, Desc::Jump { j: 7, n: 30, out: 26 }, Desc::Mark { j: 14, bb: 24, c: 15 }, Desc::Register { j: 17, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 18, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 19, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 20, by: -56, bb: 24, cut: 23 }, Desc::Probes { j: 21, c: 15, from: 23, off: 4096, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 25, by: -76, bb: 24, cut: 23 }, Desc::Branch { j: 26, by: 4, bb: 24, cut: 31 }, Desc::Call { j: 27, code: 7 }])) by (compute); }
proof fn pin_store_tested_loop_displaced() { assert(described(seq![0x92400837u32, 0xb40003f7u32, 0x91000652u32, 0xd360dc2fu32, 0xb500016fu32, 0x8b214317u32, 0x9240082fu32, 0xb500016fu32, 0xf90002e2u32, 0xf1000400u32, 0x54fffec1u32, 0xd2880617u32, 0xf2a02017u32, 0xf2c80017u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff3u32, 0xd280000fu32, 0x8b2f62efu32, 0x383f01ffu32, 0x91001defu32, 0x383f01ffu32, 0x17ffffedu32, 0x17ffffe4u32, 0xf2a02017u32, 0xf2c80017u32, 0xd61f02e0u32, 0x91000652u32, 0xd360dc2fu32, 0xb500010fu32, 0xf8214b02u32, 0xf1000400u32, 0x54ffff61u32, 0xd2880617u32, 0xf2a02017u32, 0xf2c80017u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x17fffff3u32, 0xf2a02017u32, 0xf2c80017u32, 0xd61f02e0u32], pin_at(true), seq![Desc::Register { j: 0, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 124, bb: 24, cut: 31 }, Desc::Register { j: 2, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 4, by: 44, bb: 24, cut: 31 }, Desc::Cut { j: 5, n: 1, c: 23, bb: 24 }, Desc::Register { j: 6, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 7, by: 44, bb: 24, cut: 23 }, Desc::Through { j: 8, rt: 31, rt2: 31, back: false, off: 0, bytes: 8, bb: 24, cut: 23 }, Desc::Register { j: 9, rd: 0, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 10, by: -40, bb: 24, cut: 23 }, Desc::Exit { j: 11, guest: 0x401008 }, Desc::Mark { j: 15, bb: 24, c: 15 }, Desc::Register { j: 18, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 19, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 20, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 21, by: -52, bb: 24, cut: 23 }, Desc::Probes { j: 22, c: 15, from: 23, off: 0, bytes: 8, store: true, bb: 24 }, Desc::Branch { j: 27, by: -76, bb: 24, cut: 23 }, Desc::Register { j: 32, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 33, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 34, by: 32, bb: 24, cut: 31 }, Desc::Own { j: 35, n: 1, rt: 31, rt2: 31, bytes: 8, bb: 24 }, Desc::Register { j: 36, rd: 0, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 37, by: -20, bb: 24, cut: 31 }, Desc::Exit { j: 38, guest: 0x401008 }, Desc::Mark { j: 42, bb: 24, c: 15 }])) by (compute); }
proof fn pin_stack_step() { assert(described(seq![0xd10043ffu32, 0xb94003ffu32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32], pin_at(false), seq![Desc::StackStep { j: 0, k: -16, off: 0, bb: 24, cut: 31 }, Desc::Exit { j: 2, guest: 0x401000 }])) by (compute); }
proof fn pin_stack_step_up() { assert(described(seq![0x910043ffu32, 0xb85fc3ffu32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32], pin_at(false), seq![Desc::StackStep { j: 0, k: 16, off: -4, bb: 24, cut: 31 }, Desc::Exit { j: 2, guest: 0x401000 }])) by (compute); }
proof fn pin_stack_pair_post() { assert(described(seq![0xa8c17bfdu32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32], pin_at(false), seq![Desc::StackAccess { j: 0, rt: 29, rt2: 30, off: 0, moves: 16, bytes: 16, bb: 24 }, Desc::Exit { j: 1, guest: 0x401000 }])) by (compute); }
proof fn pin_stack_pair_pre() { assert(described(seq![0xa9bf7bfdu32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32], pin_at(false), seq![Desc::StackAccess { j: 0, rt: 31, rt2: 31, off: -16, moves: -16, bytes: 16, bb: 24 }, Desc::Exit { j: 1, guest: 0x401000 }])) by (compute); }
proof fn pin_stack_cut() { assert(described(seq![0x9100000fu32, 0xd360fdf7u32, 0xb50000d7u32, 0x8b2f431fu32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 24, bb: 24, cut: 31 }, Desc::StackCut { j: 3, n: 15, bb: 24, cut: 31 }, Desc::Exit { j: 4, guest: 0x401000 }, Desc::Mark { j: 8, bb: 24, c: 15 }])) by (compute); }
proof fn pin_stack_structure() { assert(described(seq![0x9100fff7u32, 0x927a16f7u32, 0xb40000d7u32, 0x4c4003ecu32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0x910003f7u32, 0x92400ef7u32, 0xb4000057u32, 0xd42000a0u32, 0xd2800017u32, 0x8b3763f7u32, 0x394002ffu32, 0x3940feffu32, 0x17fffff3u32], pin_at(false), seq![Desc::Register { j: 0, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 24, bb: 24, cut: 31 }, Desc::StackAccess { j: 3, rt: 31, rt2: 31, off: 0, moves: 0, bytes: 64, bb: 24 }, Desc::Exit { j: 4, guest: 0x401000 }, Desc::Register { j: 8, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 9, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 10, by: 8, bb: 24, cut: 31 }, Desc::Stop { j: 11 }, Desc::Probes { j: 12, c: 23, from: 31, off: 0, bytes: 64, store: false, bb: 24 }, Desc::Branch { j: 16, by: -52, bb: 24, cut: 31 }])) by (compute); }
proof fn pin_stack_post_register() { assert(described(seq![0x910003f7u32, 0x92400ef7u32, 0xb5000217u32, 0x9100ffefu32, 0x927a15efu32, 0xb40001cfu32, 0xcb3863efu32, 0x8b2361efu32, 0xd360fdf7u32, 0xb5000297u32, 0x8b2f430fu32, 0x910003f7u32, 0x4c830af3u32, 0x910001ffu32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0x1400000eu32, 0x910003efu32, 0x92400defu32, 0xb400004fu32, 0xd42000a0u32, 0xd280000fu32, 0x8b2f63efu32, 0x383f01ffu32, 0x9100fdefu32, 0x383f01ffu32, 0x17ffffeau32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0xf9002f9eu32, 0xd2800c1eu32, 0xf2a0001eu32, 0xf2c4003eu32, 0xd63f03c0u32, 0xf9402f9eu32, 0x17ffffedu32], pin_at(false), seq![Desc::Register { j: 0, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 64, bb: 24, cut: 31 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 4, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 5, by: 56, bb: 24, cut: 31 }, Desc::Register { j: 6, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 7, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 8, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 9, by: 80, bb: 24, cut: 31 }, Desc::Cut { j: 10, n: 15, c: 15, bb: 24 }, Desc::StackVia { j: 11, rt: 31, rt2: 31, bytes: 64, bb: 24, cut: 15 }, Desc::StackMove { j: 13, bb: 24, cut: 15 }, Desc::Exit { j: 14, guest: 0x401000 }, Desc::Branch { j: 18, by: 56, bb: 24, cut: 31 }, Desc::Register { j: 19, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 20, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 21, by: 8, bb: 24, cut: 31 }, Desc::Stop { j: 22 }, Desc::Probes { j: 23, c: 15, from: 31, off: 0, bytes: 64, store: true, bb: 24 }, Desc::Branch { j: 28, by: -88, bb: 24, cut: 31 }, Desc::Mark { j: 29, bb: 24, c: 15 }, Desc::Call { j: 32, code: 6 }])) by (compute); }
proof fn pin_stack_at() { assert(described(seq![0x910003f7u32, 0x92400ef7u32, 0xb4000057u32, 0xd42000a0u32, 0x928019f7u32, 0x8b3763f7u32, 0xbd4002ecu32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32], pin_at(false), seq![Desc::Register { j: 0, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 8, bb: 24, cut: 31 }, Desc::Stop { j: 3 }, Desc::StackAt { j: 4, off: -208, rt: 31, rt2: 31, bytes: 4, bb: 24, cut: 31 }, Desc::Exit { j: 7, guest: 0x401000 }])) by (compute); }
proof fn pin_stack_short_probes() { assert(described(seq![0x910003f7u32, 0x92402ef7u32, 0x91005ef7u32, 0x376000d7u32, 0x0c9f67ffu32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0x910003f7u32, 0x92400ef7u32, 0xb4000057u32, 0xd42000a0u32, 0x910003f7u32, 0x383f02ffu32, 0x91005ef7u32, 0x383f02ffu32, 0x17fffff3u32], pin_at(false), seq![Desc::Register { j: 0, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 2, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 3, by: 24, bb: 24, cut: 31 }, Desc::StackAccess { j: 4, rt: 31, rt2: 31, off: 0, moves: 24, bytes: 24, bb: 24 }, Desc::Exit { j: 5, guest: 0x401000 }, Desc::Register { j: 9, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 10, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 11, by: 8, bb: 24, cut: 31 }, Desc::Stop { j: 12 }, Desc::StackProbes { j: 13, c: 23, bytes: 24, store: true, bb: 24 }, Desc::Branch { j: 17, by: -52, bb: 24, cut: 31 }])) by (compute); }
proof fn pin_pair_through_cut() { assert(described(seq![0x92400f2fu32, 0xb500012fu32, 0xd360df2fu32, 0xb500010fu32, 0x8b394317u32, 0x48287ee0u32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0x14000004u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0xf9002f9eu32, 0xd2800c1eu32, 0xf2a0001eu32, 0xf2c4003eu32, 0xd63f03c0u32, 0xf9402f9eu32, 0x17fffff7u32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 36, bb: 24, cut: 31 }, Desc::Register { j: 2, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 3, by: 32, bb: 24, cut: 31 }, Desc::Cut { j: 4, n: 25, c: 23, bb: 24 }, Desc::Through { j: 5, rt: 8, rt2: 9, back: false, off: 0, bytes: 16, bb: 24, cut: 23 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Branch { j: 10, by: 16, bb: 24, cut: 31 }, Desc::Mark { j: 11, bb: 24, c: 15 }, Desc::Call { j: 14, code: 6 }])) by (compute); }
proof fn pin_cut_into_x29() { assert(described(seq![0xf9004b9du32, 0xf940438fu32, 0x924008b7u32, 0xb5000437u32, 0xd360dcbdu32, 0xb500041du32, 0x8b25431du32, 0xf9403397u32, 0xd27d02f7u32, 0xb5000257u32, 0xf9403797u32, 0xcb3760b7u32, 0x9240def7u32, 0xb50001d7u32, 0xf9001f80u32, 0xd3407d20u32, 0xb3607da0u32, 0xf9403b97u32, 0xc8b77fa0u32, 0xf900339fu32, 0xf9403b8fu32, 0xca1701efu32, 0xb500004fu32, 0x14000002u32, 0xd280002fu32, 0xf9401f80u32, 0x14000004u32, 0xf83f03bfu32, 0xf900339fu32, 0xd280002fu32, 0xf900438fu32, 0xf9404b9du32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0x14000004u32, 0xd260031du32, 0x397fffbfu32, 0x00000000u32, 0xf9002f9eu32, 0xd2800c1eu32, 0xf2a0001eu32, 0xf2c4003eu32, 0xd63f03c0u32, 0xf9402f9eu32, 0x17fffff7u32], pin_at(false), seq![Desc::Frame { j: 0, f: FrameAccess { word: 0xf9004b9du32, rt: 31, off: 144, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::Frame { j: 1, f: FrameAccess { word: 0xf940438fu32, rt: 15, off: 128, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::Register { j: 2, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 3, by: 132, bb: 24, cut: 31 }, Desc::Register { j: 4, rd: 29, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 5, by: 128, bb: 24, cut: 31 }, Desc::Cut { j: 6, n: 5, c: 29, bb: 24 }, Desc::Frame { j: 7, f: FrameAccess { word: 0xf9403397u32, rt: 23, off: 96, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Register { j: 8, rd: 23, sp: false, bb: 24, cut: 29 }, Desc::Either { j: 9, by: 72, bb: 24, cut: 29 }, Desc::Frame { j: 10, f: FrameAccess { word: 0xf9403797u32, rt: 23, off: 104, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Register { j: 11, rd: 23, sp: false, bb: 24, cut: 29 }, Desc::Register { j: 12, rd: 23, sp: false, bb: 24, cut: 29 }, Desc::Either { j: 13, by: 56, bb: 24, cut: 29 }, Desc::Frame { j: 14, f: FrameAccess { word: 0xf9001f80u32, rt: 31, off: 56, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Register { j: 15, rd: 0, sp: false, bb: 24, cut: 29 }, Desc::Register { j: 16, rd: 0, sp: false, bb: 24, cut: 29 }, Desc::Frame { j: 17, f: FrameAccess { word: 0xf9403b97u32, rt: 23, off: 112, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Through { j: 18, rt: 23, rt2: 31, back: false, off: 0, bytes: 8, bb: 24, cut: 29 }, Desc::Frame { j: 19, f: FrameAccess { word: 0xf900339fu32, rt: 31, off: 96, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Frame { j: 20, f: FrameAccess { word: 0xf9403b8fu32, rt: 15, off: 112, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Register { j: 21, rd: 15, sp: false, bb: 24, cut: 29 }, Desc::Either { j: 22, by: 8, bb: 24, cut: 29 }, Desc::Branch { j: 23, by: 8, bb: 24, cut: 29 }, Desc::Register { j: 24, rd: 15, sp: false, bb: 24, cut: 29 }, Desc::Frame { j: 25, f: FrameAccess { word: 0xf9401f80u32, rt: 0, off: 56, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Branch { j: 26, by: 16, bb: 24, cut: 29 }, Desc::Through { j: 27, rt: 31, rt2: 31, back: false, off: 0, bytes: 8, bb: 24, cut: 29 }, Desc::Frame { j: 28, f: FrameAccess { word: 0xf900339fu32, rt: 31, off: 96, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Register { j: 29, rd: 15, sp: false, bb: 24, cut: 29 }, Desc::Frame { j: 30, f: FrameAccess { word: 0xf900438fu32, rt: 31, off: 128, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Frame { j: 31, f: FrameAccess { word: 0xf9404b9du32, rt: 29, off: 144, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Exit { j: 32, guest: 0x401000 }, Desc::Branch { j: 36, by: 16, bb: 24, cut: 31 }, Desc::Mark { j: 37, bb: 24, c: 29 }, Desc::Call { j: 40, code: 6 }])) by (compute); }
proof fn pin_negative_probes() { assert(described(seq![0xd360dcefu32, 0xb500014fu32, 0x8b274317u32, 0x913e8eefu32, 0x927d21efu32, 0xb400012fu32, 0x28338ae6u32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x92800c6fu32, 0x8b2f62efu32, 0x383f01ffu32, 0x91001defu32, 0x383f01ffu32, 0x17fffff3u32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 40, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 7, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 4, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 5, by: 36, bb: 24, cut: 23 }, Desc::Through { j: 6, rt: 31, rt2: 31, back: false, off: -100, bytes: 8, bb: 24, cut: 23 }, Desc::Exit { j: 7, guest: 0x401000 }, Desc::Mark { j: 11, bb: 24, c: 15 }, Desc::Probes { j: 14, c: 15, from: 23, off: -100, bytes: 8, store: true, bb: 24 }, Desc::Branch { j: 19, by: -52, bb: 24, cut: 23 }])) by (compute); }
proof fn pin_same_displaced() { assert(described(seq![0xd360de8fu32, 0xb50001afu32, 0x8b344317u32, 0x91003eefu32, 0x927c1defu32, 0xb400018fu32, 0xd104628fu32, 0x910002f7u32, 0xfd4002e1u32, 0xaa0f03f4u32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80017u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0xd280000fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39403dffu32, 0x17fffff1u32], pin_at(true), seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 52, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 20, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 4, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 5, by: 48, bb: 24, cut: 23 }, Desc::Register { j: 6, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Same { j: 7, bb: 24, cut: 23 }, Desc::Through { j: 8, rt: 31, rt2: 31, back: false, off: 0, bytes: 8, bb: 24, cut: 23 }, Desc::Register { j: 9, rd: 20, sp: false, bb: 24, cut: 23 }, Desc::Exit { j: 10, guest: 0x401000 }, Desc::Mark { j: 14, bb: 24, c: 15 }, Desc::Probes { j: 17, c: 15, from: 23, off: 0, bytes: 16, store: false, bb: 24 }, Desc::Branch { j: 21, by: -60, bb: 24, cut: 23 }])) by (compute); }
proof fn pin_jump_displaced() { assert(described(seq![0x91000652u32, 0xd360dcf7u32, 0xb50000b7u32, 0xd37e7cf7u32, 0xb3420ef7u32, 0xd25202f7u32, 0xd61f02e0u32, 0x14000001u32, 0xf9002f9eu32, 0xd2800e1eu32, 0xf2a0001eu32, 0xf2c4003eu32, 0xd63f03c0u32, 0xf9402f9eu32, 0x17fffffau32], pin_at(true), seq![Desc::Register { j: 0, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::DisplacedJump { j: 1, n: 7, out: 7 }, Desc::Branch { j: 7, by: 4, bb: 24, cut: 31 }, Desc::Call { j: 8, code: 7 }])) by (compute); }
proof fn pin_stack_cut_displaced() { assert(described(seq![0x9100001fu32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80017u32, 0xd61f02e0u32], pin_at(true), seq![Desc::Register { j: 0, rd: 31, sp: true, bb: 24, cut: 31 }, Desc::Exit { j: 1, guest: 0x401000 }])) by (compute); }

} // verus!
