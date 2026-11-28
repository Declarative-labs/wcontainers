//! THE COMPOSITION: a piece is confined when it is described, word by word, by blocks that each hold alone.
//!
//! A description is a list of [`Desc`]: each names the words one block owns, where it is entered and what it needs
//! there, as one of the block kinds form I's pieces are made of (a register-only word, the cut, a branch either way,
//! an access through the cut, an access cut by its own addressing, the mark, the probes of a page-crossing access, the
//! exit, the jump, a floor call, a frame access or move). What a block needs where it is entered is the part's fact ([`part`]): the frame in x28, the window's
//! base in a named register `bb` (x24, or the stand-in's while x24 is the guest's), the stack pointer in the window
//! unless displaced, and for some x23 in the window.
//!
//! [`described`] is decidable over the words (each block's shape, and that every place a block goes on to is where a
//! block of a matching need is entered); [`all_meant`] is what the words do (the hypotheses on the free `step`, see
//! the crate's documentation). [`described_is_confined`] proves that the two make the piece confined, whatever the
//! piece: confinement of a piece follows from its blocks, not from its shape. The pins at the end are pieces the
//! translator emits (`tests/compose.rs`) with their descriptions, checked by `compute`.

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

/// The step from `m` inside block `b` is confined, and the run goes on inside the block or enters one.
pub open spec fn steps_within(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, blocks: Seq<Block>, b: int, m: Machine) -> bool {
    let k = blocks[b];
    let s = step(m, word_at(instructions, a, m.pc));
    &&& confined_step(a, m, s)
    &&& runs(a, s.after) ==> ((k.owns)(index(a, s.after.pc)) && (k.inv)(s.after)) || enters(blocks, a, s.after)
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

/// THE RULE OF BLOCKS: if the first block is entered at the piece's first word with what the entry rule gives, and
/// every block holds alone, the piece is confined.
pub proof fn lemma_blocks_confine(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, blocks: Seq<Block>)
    requires
        a.length == instructions.len() * 4,
        instructions.len() > 0,
        blocks.len() > 0 && blocks[0].entry == 0,
        forall|m: Machine| entered(a, m) ==> #[trigger] (blocks[0].needs)(m),
        forall|b: int| 0 <= b < blocks.len() ==> #[trigger] block_holds(step, instructions, a, blocks, b),
    ensures
        is_confined(step, instructions, a),
{
    let inv = |m: Machine| somewhere(blocks, a, m);
    assert forall|m: Machine| entered(a, m) implies #[trigger] inv(m) by {
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
    lemma_invariant_confines(step, instructions, a, inv);
}

/// The part's fact: the frame in x28, the window's base in `bb`, and with `cut` x23 in the window. (No block here
/// needs the entry rule's stack pointer: the stack's pieces are proven whole, stack.rs.)
pub open spec fn part(a: Allowed, bb: int, cut: bool, m: Machine) -> bool {
    &&& m.x.len() == 31
    &&& m.x[frame_register()] as int == a.frame
    &&& m.x[bb] as int == a.base
    &&& cut ==> in_window(a, m.x[23])
}

/// A register that may hold the base: not x23, which the cut writes, nor x28, the frame's.
pub open spec fn base_ok(bb: int) -> bool {
    0 <= bb < 31 && bb != 23 && bb != frame_register()
}

/// The piece where form I puts it, its window and frame as form I has them (copy.rs `in_reach` without a guest).
pub open spec fn placed(a: Allowed, instructions: Seq<u32>) -> bool {
    &&& a.length == instructions.len() * 4
    &&& a.base % 0x2_0000_0000 == 0 && 0x2_0000_0000 <= a.base && a.base < 2048 * 0x2_0000_0000
    &&& 0 <= a.below <= 0x1_0000
    &&& a.code >= 0 && a.code + a.length <= PROCESS_SLOTS
    &&& (a.code + a.length <= FLOOR_PAGE || a.code >= FLOOR_PAGE + 4096)
    &&& frame_in_reach(a)
}

/// The kinds of block, each with the word it is entered at (`j`).
pub enum Desc {
    /// A register-only word writing `rd` (31: none) and nothing else.
    Register { j: int, rd: int, bb: int, cut: bool },
    /// `add x23, xbb, wN, uxtw`: the cut.
    Cut { j: int, n: int, bb: int },
    /// A branch by `by` bytes, or on to the next word.
    Either { j: int, by: int, bb: int, cut: bool },
    /// A branch by `by` bytes (`b`).
    Branch { j: int, by: int, bb: int, cut: bool },
    /// An access of `bytes` at x23 plus `off`, loading `rt` (31: none), x23 written back (to anything) when `back`.
    Through { j: int, rt: int, back: bool, off: int, bytes: nat, bb: int },
    /// An access of `bytes` at `bb` plus wN: cut by its own addressing.
    Own { j: int, n: int, rt: int, bytes: nat, bb: int },
    /// The mark through `c`: `eor xc, xbb, #2^32`, `ldrb wzr, [xc, #4095]`, `udf`.
    Mark { j: int, bb: int, c: int },
    /// The probes through `c` of the first and last byte of `bytes` at x23 plus `off` (a page-crossing access's).
    Probes { j: int, c: int, off: int, bytes: nat, store: bool, bb: int },
    /// The exit to `guest`'s entry slot.
    Exit { j: int, guest: int },
    /// The jump through `xN` into the slot table, or to `out` when its target is outside the window.
    Jump { j: int, n: int, out: int },
    /// A call into the floor, `code` its entry.
    Call { j: int, code: int },
    /// A frame access or a move, after which the base is in `bb2`.
    Frame { j: int, f: FrameAccess, bb: int, bb2: int },
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
    }
}

/// A block whose need the part's fact with `bb` and `cut` meets.
pub open spec fn iface_ok(d: Desc, bb: int, cut: bool) -> bool {
    match d {
        Desc::Register { bb: b, cut: c, .. } => b == bb && (c ==> cut),
        Desc::Cut { bb: b, .. } => b == bb,
        Desc::Either { bb: b, cut: c, .. } => b == bb && (c ==> cut),
        Desc::Branch { bb: b, cut: c, .. } => b == bb && (c ==> cut),
        Desc::Through { bb: b, .. } => b == bb && cut,
        Desc::Own { bb: b, .. } => b == bb,
        Desc::Mark { bb: b, .. } => b == bb,
        Desc::Probes { bb: b, .. } => b == bb && cut,
        Desc::Exit { .. } => true,
        Desc::Jump { .. } => bb == 24,
        Desc::Call { .. } => true,
        Desc::Frame { bb: b, .. } => b == bb,
    }
}

/// Some block of the list is entered at `entry` with a need the part's fact meets.
pub open spec fn accepts(descs: Seq<Desc>, entry: int, bb: int, cut: bool) -> bool
    decreases descs.len(),
{
    if descs.len() == 0 {
        false
    } else {
        (entry_of(descs.last()) == entry && iface_ok(descs.last(), bb, cut)) || accepts(descs.drop_last(), entry, bb, cut)
    }
}

/// The probes' words: `movz xc, #off`, `add xc, x23, xc`, and a load's `ldrb wzr, [xc]`, `ldrb wzr, [xc, #bytes-1]` or a
/// store's `staddb wzr, [xc]`, `add xc, xc, #bytes-1`, `staddb wzr, [xc]`.
pub open spec fn probes_words(c: int, off: int, bytes: nat, store: bool) -> Seq<u32> {
    seq![movz_c(c, off), add_x23_c(c)]
        + if store {
            seq![probe_store_c(c), add_c_c(c, bytes - 1), probe_store_c(c)]
        } else {
            seq![probe_load_c(c, 0), probe_load_c(c, bytes - 1)]
        }
}

pub open spec fn probes_length(store: bool) -> int {
    if store { 5 } else { 4 }
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
        Desc::Register { j, rd, bb, cut } =>
            0 <= j && j + 1 < len && base_ok(bb) && 0 <= rd < 32 && rd != bb && rd != frame_register() && (cut ==> rd != 23)
            && accepts(descs, j + 1, bb, cut),
        Desc::Cut { j, n, bb } =>
            0 <= j && j + 1 < len && base_ok(bb) && 0 <= n < 31 && instructions[j] == add_cut_by(n as u32, bb)
            && accepts(descs, j + 1, bb, true),
        Desc::Either { j, by, bb, cut } =>
            0 <= j && j + 1 < len && base_ok(bb) && by % 4 == 0 && 0 <= j + by / 4 < len
            && accepts(descs, j + 1, bb, cut) && accepts(descs, j + by / 4, bb, cut),
        Desc::Branch { j, by, bb, cut } =>
            0 <= j < len && base_ok(bb) && by % 4 == 0 && 0 <= j + by / 4 < len && accepts(descs, j + by / 4, bb, cut),
        Desc::Through { j, rt, back, off, bytes, bb } =>
            0 <= j && j + 1 < len && base_ok(bb) && 0 <= rt < 32 && rt != bb && rt != frame_register()
            && 0 <= off && 1 <= bytes && off + bytes <= 0x1_0000
            && accepts(descs, j + 1, bb, false),
        Desc::Own { j, n, rt, bytes, bb } =>
            0 <= j && j + 1 < len && base_ok(bb) && 0 <= n < 31 && 0 <= rt < 32 && rt != bb && rt != frame_register()
            && 1 <= bytes <= 16 && accepts(descs, j + 1, bb, false),
        Desc::Mark { j, bb, c } =>
            base_ok(bb) && 0 <= c < 31 && c != bb && c != frame_register()
            && holds_at(instructions, j, seq![eor_c(c, bb), mark_load_c(c), UDF]),
        Desc::Probes { j, c, off, bytes, store, bb } =>
            base_ok(bb) && 0 <= c < 31 && c != bb && c != 23 && c != frame_register()
            && 0 <= off && 1 <= bytes && off + bytes <= 0x1_0000
            && j + probes_length(store) < len && holds_at(instructions, j, probes_words(c, off, bytes, store))
            && accepts(descs, j + probes_length(store), bb, true),
        Desc::Exit { j, guest } =>
            0 <= guest && guest + 4 < 0x1_0000_0000 && holds_at(instructions, j, exit_words(exit_slot(a, guest + 4) as u64)),
        Desc::Jump { j, n, out } =>
            !a.displaced && 0 <= n < 31 && j + 1 < out < instructions.len() && holds_at(instructions, j, jump_words(n as u32, j, out))
            && accepts(descs, out, 24, false),
        Desc::Call { j, code } =>
            0 <= code < 256 && holds_at(instructions, j, floor_call(floor_entry(code))),
        Desc::Frame { j, f, bb, bb2 } =>
            0 <= j && j + 1 < len && base_ok(bb) && base_ok(bb2) && instructions[j] == f.word
            && (if f.moves && f.off == bb && f.rt == bb2 { true } else { bb2 == bb && f.rt != bb })
            && accepts(descs, j + 1, bb2, false),
    }
}

/// `word` writes `rd` (31: none of x0 to x30) and no other general register: no access, on to the next word (a
/// register-only instruction; Arm ARM).
pub open spec fn keeps_but(step: spec_fn(Machine, u32) -> Stepped, word: u32, rd: int) -> bool {
    forall|m: Machine| m.x.len() == 31 ==> {
        let s = #[trigger] step(m, word);
        &&& s.accesses.len() == 0 && !s.system_call && !s.after.ended
        &&& s.after.unlisted == m.unlisted && s.after.pc == (m.pc + 4) as u64
        &&& s.after.x.len() == 31
        &&& forall|r: int| 0 <= r < 31 && r != rd ==> s.after.x[r] == m.x[r]
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

/// `word` touches `bytes` bytes from `address` and writes no general register but `rt` and `rb` (31: none); a fault
/// ends the run, else it goes on at the next word (meaning.rs `accesses` with a written-back base).
pub open spec fn touches(step: spec_fn(Machine, u32) -> Stepped, word: u32, rt: int, rb: int, address: spec_fn(Machine) -> int, bytes: nat) -> bool {
    forall|m: Machine| m.x.len() == 31 ==> {
        let s = #[trigger] step(m, word);
        &&& s.accesses == seq![(address(m), bytes)]
        &&& !s.system_call
        &&& s.after.unlisted == m.unlisted
        &&& s.after.x.len() == 31
        &&& forall|r: int| 0 <= r < 31 && r != rt && r != rb ==> s.after.x[r] == m.x[r]
        &&& s.after.ended || s.after.pc == (m.pc + 4) as u64
    }
}

/// What block `d`'s words do (Arm ARM), as hypotheses on the free `step`.
pub open spec fn meant(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, d: Desc) -> bool {
    match d {
        Desc::Register { j, rd, .. } => keeps_but(step, instructions[j], rd),
        Desc::Cut { n, bb, .. } =>
            writes(step, add_cut_by(n as u32, bb), 23, |m: Machine| ((m.x[bb] + (m.x[n] & 0xffff_ffffu64)) % two_to_64()) as u64),
        Desc::Either { j, by, .. } => goes_either(step, instructions[j], by),
        Desc::Branch { j, by, .. } => branches(step, instructions[j], by),
        Desc::Through { j, rt, back, off, bytes, .. } =>
            touches(step, instructions[j], rt, if back { 23 } else { 31 }, |m: Machine| (m.x[23] + off) % two_to_64(), bytes),
        Desc::Own { j, n, rt, bytes, bb } =>
            touches(step, instructions[j], rt, 31, |m: Machine| (m.x[bb] + (m.x[n] & 0xffff_ffffu64)) % two_to_64(), bytes),
        Desc::Mark { bb, c, .. } => mark_c_means(step, bb, c),
        Desc::Probes { c, off, bytes, store, .. } => {
            &&& writes(step, movz_c(c, off), c, |m: Machine| off as u64)
            &&& writes(step, add_x23_c(c), c, |m: Machine| ((m.x[23] + m.x[c]) % two_to_64()) as u64)
            &&& store ==> accesses(step, probe_store_c(c), 31, |m: Machine| m.x[c] as int, 1)
            &&& store ==> writes(step, add_c_c(c, bytes - 1), c, |m: Machine| ((m.x[c] + (bytes - 1)) % two_to_64()) as u64)
            &&& !store ==> accesses(step, probe_load_c(c, 0), 31, |m: Machine| (m.x[c] + 0) % two_to_64(), 1)
            &&& !store ==> accesses(step, probe_load_c(c, bytes - 1), 31, |m: Machine| (m.x[c] + (bytes - 1)) % two_to_64(), 1)
        },
        Desc::Exit { .. } => exit_means(step),
        Desc::Jump { j, n, out } => jump_main_means(step, n as u32, (out - j - 1) * 4),
        Desc::Call { .. } => floor_means(step),
        Desc::Frame { f, .. } => frame_access_means(step, f),
    }
}

/// The piece is described: the first block is entered at its first word with the entry rule's fact, and every block
/// has its shape.
pub open spec fn shapes_ok(instructions: Seq<u32>, a: Allowed, all: Seq<Desc>, k: int) -> bool
    decreases k,
{
    if k <= 0 { true } else { shape_ok(instructions, a, all, all[k - 1]) && shapes_ok(instructions, a, all, k - 1) }
}

pub open spec fn described(instructions: Seq<u32>, a: Allowed, descs: Seq<Desc>) -> bool {
    &&& descs.len() > 0 && entry_of(descs[0]) == 0 && iface_ok(descs[0], 24, false)
    &&& shapes_ok(instructions, a, descs, descs.len() as int)
}

pub open spec fn all_meant(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, descs: Seq<Desc>) -> bool {
    forall|i: int| 0 <= i < descs.len() ==> meant(step, instructions, #[trigger] descs[i])
}

/// The block of a description.
pub open spec fn block_of(a: Allowed, d: Desc) -> Block {
    match d {
        Desc::Register { j, bb, cut, .. } => one(a, j, bb, cut),
        Desc::Cut { j, bb, .. } => one(a, j, bb, false),
        Desc::Either { j, bb, cut, .. } => one(a, j, bb, cut),
        Desc::Branch { j, bb, cut, .. } => one(a, j, bb, cut),
        Desc::Through { j, bb, .. } => one(a, j, bb, true),
        Desc::Own { j, bb, .. } => one(a, j, bb, false),
        Desc::Frame { j, bb, .. } => one(a, j, bb, false),
        Desc::Mark { j, bb, c } => Block {
            owns: |i: int| j <= i < j + 3,
            entry: j,
            needs: |m: Machine| part(a, bb, false, m),
            inv: |m: Machine| part(a, bb, false, m) && (index(a, m.pc) == j + 1 ==> m.x[c] as int == a.base + 0x1_0000_0000),
        },
        Desc::Probes { j, c, off, bytes, store, bb } => Block {
            owns: |i: int| j <= i < j + probes_length(store),
            entry: j,
            needs: |m: Machine| part(a, bb, true, m),
            inv: |m: Machine| part(a, bb, true, m) && probes_inv(a, j, c, off, bytes, store, m),
        },
        Desc::Exit { j, guest } => Block {
            owns: |i: int| j <= i < j + 4,
            entry: j,
            needs: |m: Machine| kept(a, m),
            inv: |m: Machine| kept(a, m) && at_exit(m, exit_slot(a, guest + 4) as u64, index(a, m.pc) - j),
        },
        Desc::Jump { j, .. } => Block {
            owns: |i: int| j <= i < j + 7,
            entry: j,
            needs: |m: Machine| part(a, 24, false, m),
            inv: |m: Machine| part(a, 24, false, m) && jump_at(a, j, m),
        },
        Desc::Call { j, code } => Block {
            owns: |i: int| j <= i < j + 5,
            entry: j,
            needs: |m: Machine| kept(a, m),
            inv: |m: Machine| kept(a, m) && at_floor_call(m, floor_entry(code), index(a, m.pc) - j),
        },
    }
}

/// A one-word block needing the part's fact.
pub open spec fn one(a: Allowed, j: int, bb: int, cut: bool) -> Block {
    Block { owns: |i: int| i == j, entry: j, needs: |m: Machine| part(a, bb, cut, m), inv: |m: Machine| part(a, bb, cut, m) }
}

/// Where the probes' run stands: `c` the offset after its move, then the address it probes.
pub open spec fn probes_inv(a: Allowed, j: int, c: int, off: int, bytes: nat, store: bool, m: Machine) -> bool {
    let i = index(a, m.pc);
    &&& i == j + 1 ==> m.x[c] as int == off
    &&& (i == j + 2 || i == j + 3) ==> m.x[c] as int == m.x[23] as int + off
    &&& (store && i == j + 4) ==> m.x[c] as int == m.x[23] as int + off + bytes - 1
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
pub open spec fn takes(blocks: Seq<Block>, c: int, entry: int, a: Allowed, bb: int, cut: bool) -> bool {
    &&& 0 <= c < blocks.len() && blocks[c].entry == entry
    &&& forall|m: Machine| part(a, bb, cut, m) ==> #[trigger] (blocks[c].needs)(m)
}

proof fn lemma_accepts(a: Allowed, descs: Seq<Desc>, entry: int, bb: int, cut: bool) -> (c: int)
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

/// A step that goes on at word `t` of the piece with the part's fact is confined, and enters the block `c` there.
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
    cut: bool,
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
{
    assert(in_sequence(a, s.after.pc));
    assert(index(a, s.after.pc) == t);
    assert((blocks[c].needs)(s.after));
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
    bb: int,
    cut: bool,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, cut),
        0 <= j && j + 1 < instructions.len(),
        base_ok(bb) && 0 <= rd < 32 && rd != bb && rd != frame_register() && (cut ==> rd != 23),
        keeps_but(step, instructions[j], rd),
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

/// THE CUT'S BLOCK HOLDS: x23 is the window's base plus the low word of xN.
proof fn lemma_cut(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    n: int,
    bb: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, false),
        0 <= j && j + 1 < instructions.len(),
        base_ok(bb) && 0 <= n < 31,
        instructions[j] == add_cut_by(n as u32, bb),
        writes(step, add_cut_by(n as u32, bb), 23, |m: Machine| ((m.x[bb] + (m.x[n] & 0xffff_ffffu64)) % two_to_64()) as u64),
        takes(blocks, c, j + 1, a, bb, true),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, instructions[j]);
        assert(m.pc as int == a.code + 4 * j);
        lemma_low_word(m.x[n]);
        assert(part(a, bb, true, s.after));
        lemma_goes_on(step, instructions, a, blocks, m, s, j + 1, c, bb, true);
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
    cut: bool,
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
    cut: bool,
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

/// An access of `bytes` from `first` is in the slot's reservation when `first` is at most 64 KiB above the window.
proof fn lemma_in_slot(a: Allowed, first: int, bytes: nat)
    requires
        0 <= a.below <= 0x1_0000,
        a.base <= first && first + bytes <= a.base + 0x1_0000_0000 + 0x1_0000,
    ensures
        touches_allowed(a, first, bytes),
{
}

/// THE ACCESS THROUGH X23'S BLOCK HOLDS: x23 is in the window, the offset under 64 KiB.
proof fn lemma_through(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    rt: int,
    back: bool,
    off: int,
    bytes: nat,
    bb: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, true),
        0 <= j && j + 1 < instructions.len(),
        base_ok(bb) && 0 <= rt < 32 && rt != bb && rt != frame_register(),
        0 <= off && 1 <= bytes && off + bytes <= 0x1_0000,
        touches(step, instructions[j], rt, if back { 23 } else { 31 }, |m: Machine| (m.x[23] + off) % two_to_64(), bytes),
        takes(blocks, c, j + 1, a, bb, false),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, instructions[j]);
        assert(m.pc as int == a.code + 4 * j);
        assert((m.x[23] + off) % two_to_64() == m.x[23] + off);
        lemma_in_slot(a, m.x[23] + off, bytes);
        assert(touches_allowed(a, s.accesses[0].0, s.accesses[0].1));
        if !s.after.ended {
            assert(part(a, bb, false, s.after));
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
    bytes: nat,
    bb: int,
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, false),
        0 <= j && j + 1 < instructions.len(),
        base_ok(bb) && 0 <= n < 31 && 0 <= rt < 32 && rt != bb && rt != frame_register() && 1 <= bytes <= 16,
        touches(step, instructions[j], rt, 31, |m: Machine| (m.x[bb] + (m.x[n] & 0xffff_ffffu64)) % two_to_64(), bytes),
        takes(blocks, c, j + 1, a, bb, false),
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
        if !s.after.ended {
            assert(part(a, bb, false, s.after));
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
        } else if i == j + 1 {
            assert((m.x[c] + 4095) % two_to_64() == a.base + 0x1_0000_0000 + 4095);
            if !s.after.ended {
                assert(in_sequence(a, s.after.pc));
                assert(index(a, s.after.pc) == j + 2);
            }
        } else {
        }
    }
}

/// THE PROBES' BLOCK HOLDS: they touch the first and last byte the access reaches, and the run goes on with the cut.
proof fn lemma_probes(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    blocks: Seq<Block>,
    b: int,
    j: int,
    c: int,
    off: int,
    bytes: nat,
    store: bool,
    bb: int,
    next: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == block_of(a, Desc::Probes { j, c, off, bytes, store, bb }),
        base_ok(bb) && 0 <= c < 31 && c != bb && c != 23 && c != frame_register(),
        0 <= off && 1 <= bytes && off + bytes <= 0x1_0000,
        j + probes_length(store) < instructions.len(),
        holds_at(instructions, j, probes_words(c, off, bytes, store)),
        meant(step, instructions, Desc::Probes { j, c, off, bytes, store, bb }),
        takes(blocks, next, j + probes_length(store), a, bb, true),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    let w = probes_words(c, off, bytes, store);
    assert forall|i: int| 0 <= i < w.len() implies instructions[j + i] == #[trigger] w[i] by {
        assert(instructions.subrange(j, j + w.len())[i] == instructions[j + i]);
    }
    assert(w[0] == movz_c(c, off) && w[1] == add_x23_c(c));
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        let i = index(a, m.pc);
        assert(word_at(instructions, a, m.pc) == instructions[i]);
        let s = step(m, instructions[i]);
        assert(m.pc as int == a.code + 4 * i);
        assert(part(a, bb, true, m));
        let last = j + probes_length(store) - 1;
        if i == j {
            assert(instructions[i] == w[0]);
        } else if i == j + 1 {
            assert(instructions[i] == w[1]);
            assert((m.x[23] + m.x[c]) % two_to_64() == m.x[23] + off);
        } else if store {
            assert(w[2] == probe_store_c(c) && w[3] == add_c_c(c, bytes - 1) && w[4] == probe_store_c(c));
            if i == j + 2 {
                assert(instructions[i] == w[2]);
                lemma_in_slot(a, m.x[c] as int, 1);
            } else if i == j + 3 {
                assert(instructions[i] == w[3]);
                assert((m.x[c] + (bytes - 1)) % two_to_64() == m.x[23] + off + bytes - 1);
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
        if runs(a, s.after) {
            assert(index(a, s.after.pc) == i + 1);
            assert(part(a, bb, true, s.after));
            if i == last {
                assert((blocks[next].needs)(s.after));
            }
        }
    }
}

/// THE EXIT'S BLOCK HOLDS (copy.rs `lemma_exit_step`).
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
        if i - j + 1 < 4 {
            assert(index(a, s.after.pc) == i + 1);
        }
    }
}

/// THE JUMP'S BLOCK HOLDS (branch.rs `jump_is_confined`, placed anywhere): it lands in the window's slot table, or
/// enters the block at `out` when its target is outside the window.
proof fn lemma_jump_block(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, blocks: Seq<Block>, b: int, j: int, n: int, out: int, c: int)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == block_of(a, Desc::Jump { j, n, out }),
        !a.displaced && 0 <= n < 31 && j + 1 < out < instructions.len(),
        holds_at(instructions, j, jump_words(n as u32, j, out)),
        jump_main_means(step, n as u32, (out - j - 1) * 4),
        takes(blocks, c, out, a, 24, false),
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
                lemma_goes_on(step, instructions, a, blocks, m, s, out, c, 24, false);
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
        }
        if runs(a, s.after) && o < 24 && s.after.pc == (m.pc + 4) as u64 {
            assert(index(a, s.after.pc) == i + 1);
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
    c: int,
)
    requires
        placed(a, instructions),
        0 <= b < blocks.len() && blocks[b] == one(a, j, bb, false),
        0 <= j && j + 1 < instructions.len(),
        base_ok(bb) && base_ok(bb2),
        instructions[j] == f.word,
        if f.moves && f.off == bb && f.rt == bb2 { true } else { bb2 == bb && f.rt != bb },
        frame_access_means(step, f),
        takes(blocks, c, j + 1, a, bb2, false),
    ensures
        block_holds(step, instructions, a, blocks, b),
{
    let k = blocks[b];
    assert forall|m: Machine| runs(a, m) && (k.owns)(index(a, m.pc)) && #[trigger] (k.inv)(m) implies steps_within(step, instructions, a, blocks, b, m) by {
        assert(word_at(instructions, a, m.pc) == instructions[j]);
        let s = step(m, f.word);
        assert(m.pc as int == a.code + 4 * j);
        if f.moves {
            assert(part(a, bb2, false, s.after));
            lemma_goes_on(step, instructions, a, blocks, m, s, j + 1, c, bb2, false);
        } else {
            assert((m.x[28] + f.off) % two_to_64() == a.frame + f.off);
            assert(touches_allowed(a, s.accesses[0].0, s.accesses[0].1));
            assert(s.after.x[28] == m.x[28] && s.after.x[bb] == m.x[bb]);
            if !s.after.ended {
                assert(part(a, bb2, false, s.after));
                assert(in_sequence(a, s.after.pc));
                assert(index(a, s.after.pc) == j + 1);
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
        meant(step, instructions, descs[b]),
    ensures
        block_holds(step, instructions, a, blocks_of(a, descs), b),
{
    let blocks = blocks_of(a, descs);
    assert(blocks[b] == block_of(a, descs[b]));
    match descs[b] {
        Desc::Register { j, rd, bb, cut } => {
            let c = lemma_accepts(a, descs, j + 1, bb, cut);
            lemma_register(step, instructions, a, blocks, b, j, rd, bb, cut, c);
        },
        Desc::Cut { j, n, bb } => {
            let c = lemma_accepts(a, descs, j + 1, bb, true);
            lemma_cut(step, instructions, a, blocks, b, j, n, bb, c);
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
        Desc::Through { j, rt, back, off, bytes, bb } => {
            let c = lemma_accepts(a, descs, j + 1, bb, false);
            lemma_through(step, instructions, a, blocks, b, j, rt, back, off, bytes, bb, c);
        },
        Desc::Own { j, n, rt, bytes, bb } => {
            let c = lemma_accepts(a, descs, j + 1, bb, false);
            lemma_own(step, instructions, a, blocks, b, j, n, rt, bytes, bb, c);
        },
        Desc::Mark { j, bb, c } => {
            lemma_mark_block(step, instructions, a, blocks, b, j, bb, c);
        },
        Desc::Probes { j, c, off, bytes, store, bb } => {
            let next = lemma_accepts(a, descs, j + probes_length(store), bb, true);
            lemma_probes(step, instructions, a, blocks, b, j, c, off, bytes, store, bb, next);
        },
        Desc::Exit { j, guest } => {
            lemma_exit_block(step, instructions, a, blocks, b, j, guest);
        },
        Desc::Jump { j, n, out } => {
            let c = lemma_accepts(a, descs, out, 24, false);
            lemma_jump_block(step, instructions, a, blocks, b, j, n, out, c);
        },
        Desc::Call { j, code } => {
            lemma_call_block(step, instructions, a, blocks, b, j, code);
        },
        Desc::Frame { j, f, bb, bb2 } => {
            let c = lemma_accepts(a, descs, j + 1, bb2, false);
            lemma_frame(step, instructions, a, blocks, b, j, f, bb, bb2, c);
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

/// THE COMPOSITION: every described piece whose words mean what its description says is confined.
pub proof fn described_is_confined(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, descs: Seq<Desc>)
    requires
        placed(a, instructions),
        described(instructions, a, descs),
        all_meant(step, instructions, descs),
    ensures
        is_confined(step, instructions, a),
{
    let blocks = blocks_of(a, descs);
    assert forall|b: int| 0 <= b < blocks.len() implies #[trigger] block_holds(step, instructions, a, blocks, b) by {
        lemma_shapes(instructions, a, descs, descs.len() as int, b);
        assert(meant(step, instructions, descs[b]));
        lemma_block(step, instructions, a, descs, b);
    }
    lemma_shapes(instructions, a, descs, descs.len() as int, 0);
    assert(blocks[0] == block_of(a, descs[0]));
    assert forall|m: Machine| entered(a, m) implies #[trigger] (blocks[0].needs)(m) by {
        assert(part(a, 24, false, m));
    }
    assert(instructions.len() > 0) by {
        match descs[0] {
            Desc::Mark { j, .. } => { assert(instructions.subrange(j, j + 3).len() == 3); },
            Desc::Probes { j, c, off, bytes, store, .. } => {
                assert(instructions.subrange(j, j + probes_words(c, off, bytes, store).len()).len() > 0);
            },
            Desc::Exit { j, guest } => { assert(instructions.subrange(j, j + 4).len() == 4); },
            Desc::Jump { j, .. } => { assert(instructions.subrange(j, j + 7).len() == 7); },
            Desc::Call { j, .. } => { assert(instructions.subrange(j, j + 5).len() == 5); },
            _ => {},
        }
    }
    lemma_blocks_confine(step, instructions, a, blocks);
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
    Allowed { base: 0x6_0000_0000, below: 0, frame: 0, code: 0, length: 0, displaced }
}


proof fn pin_store_tested_loop() { assert(described(seq![0x92400837u32, 0xb40003f7u32, 0x91000652u32, 0xd360dc2fu32, 0xb500016fu32, 0x8b214317u32, 0x9240082fu32, 0xb500016fu32, 0xf90002e2u32, 0xf1000400u32, 0x54fffec1u32, 0xd2880617u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff3u32, 0xd280000fu32, 0x8b2f62efu32, 0x383f01ffu32, 0x91001defu32, 0x383f01ffu32, 0x17ffffedu32, 0x17ffffe4u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0x91000652u32, 0xd360dc2fu32, 0xb500010fu32, 0xf8214b02u32, 0xf1000400u32, 0x54ffff61u32, 0xd2880617u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x17fffff3u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32], pin_at(false), seq![Desc::Register { j: 0, rd: 23, bb: 24, cut: false }, Desc::Either { j: 1, by: 124, bb: 24, cut: false }, Desc::Register { j: 2, rd: 18, bb: 24, cut: false }, Desc::Register { j: 3, rd: 15, bb: 24, cut: false }, Desc::Either { j: 4, by: 44, bb: 24, cut: false }, Desc::Cut { j: 5, n: 1, bb: 24 }, Desc::Register { j: 6, rd: 15, bb: 24, cut: true }, Desc::Either { j: 7, by: 44, bb: 24, cut: true }, Desc::Through { j: 8, rt: 31, back: false, off: 0, bytes: 8, bb: 24 }, Desc::Register { j: 9, rd: 0, bb: 24, cut: false }, Desc::Either { j: 10, by: -40, bb: 24, cut: false }, Desc::Exit { j: 11, guest: 0x401008 }, Desc::Mark { j: 15, bb: 24, c: 15 }, Desc::Register { j: 18, rd: 15, bb: 24, cut: true }, Desc::Register { j: 19, rd: 15, bb: 24, cut: true }, Desc::Either { j: 20, by: 8, bb: 24, cut: true }, Desc::Branch { j: 21, by: -52, bb: 24, cut: true }, Desc::Probes { j: 22, c: 15, off: 0, bytes: 8, store: true, bb: 24 }, Desc::Branch { j: 27, by: -76, bb: 24, cut: true }, Desc::Register { j: 32, rd: 18, bb: 24, cut: false }, Desc::Register { j: 33, rd: 15, bb: 24, cut: false }, Desc::Either { j: 34, by: 32, bb: 24, cut: false }, Desc::Own { j: 35, n: 1, rt: 31, bytes: 8, bb: 24 }, Desc::Register { j: 36, rd: 0, bb: 24, cut: false }, Desc::Either { j: 37, by: -20, bb: 24, cut: false }, Desc::Exit { j: 38, guest: 0x401008 }, Desc::Mark { j: 42, bb: 24, c: 15 }])) by (compute); }
proof fn pin_post_load_loop() { assert(described(seq![0x91000652u32, 0xd360dc2fu32, 0xb50001afu32, 0x8b214317u32, 0x9240082fu32, 0xb50001afu32, 0x9100202fu32, 0xf84086e2u32, 0xaa0f03e1u32, 0xf1000400u32, 0x54fffec1u32, 0xd2880617u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff1u32, 0xd280000fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39401dffu32, 0x17ffffecu32, 0x17ffffe5u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32], pin_at(false), seq![Desc::Register { j: 0, rd: 18, bb: 24, cut: false }, Desc::Register { j: 1, rd: 15, bb: 24, cut: false }, Desc::Either { j: 2, by: 52, bb: 24, cut: false }, Desc::Cut { j: 3, n: 1, bb: 24 }, Desc::Register { j: 4, rd: 15, bb: 24, cut: true }, Desc::Either { j: 5, by: 52, bb: 24, cut: true }, Desc::Register { j: 6, rd: 15, bb: 24, cut: true }, Desc::Through { j: 7, rt: 2, back: true, off: 0, bytes: 8, bb: 24 }, Desc::Register { j: 8, rd: 1, bb: 24, cut: false }, Desc::Register { j: 9, rd: 0, bb: 24, cut: false }, Desc::Either { j: 10, by: -40, bb: 24, cut: false }, Desc::Exit { j: 11, guest: 0x401008 }, Desc::Mark { j: 15, bb: 24, c: 15 }, Desc::Register { j: 18, rd: 15, bb: 24, cut: true }, Desc::Register { j: 19, rd: 15, bb: 24, cut: true }, Desc::Either { j: 20, by: 8, bb: 24, cut: true }, Desc::Branch { j: 21, by: -60, bb: 24, cut: true }, Desc::Probes { j: 22, c: 15, off: 0, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 26, by: -80, bb: 24, cut: true }])) by (compute); }
proof fn pin_post_store_loop() { assert(described(seq![0x91000652u32, 0xd360dc2fu32, 0xb50001afu32, 0x8b214317u32, 0x9240082fu32, 0xb50001afu32, 0x9100202fu32, 0xf80086e2u32, 0xaa0f03e1u32, 0xf1000400u32, 0x54fffec1u32, 0xd2880617u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff1u32, 0xd280000fu32, 0x8b2f62efu32, 0x383f01ffu32, 0x91001defu32, 0x383f01ffu32, 0x17ffffebu32, 0x17ffffe4u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32], pin_at(false), seq![Desc::Register { j: 0, rd: 18, bb: 24, cut: false }, Desc::Register { j: 1, rd: 15, bb: 24, cut: false }, Desc::Either { j: 2, by: 52, bb: 24, cut: false }, Desc::Cut { j: 3, n: 1, bb: 24 }, Desc::Register { j: 4, rd: 15, bb: 24, cut: true }, Desc::Either { j: 5, by: 52, bb: 24, cut: true }, Desc::Register { j: 6, rd: 15, bb: 24, cut: true }, Desc::Through { j: 7, rt: 31, back: true, off: 0, bytes: 8, bb: 24 }, Desc::Register { j: 8, rd: 1, bb: 24, cut: false }, Desc::Register { j: 9, rd: 0, bb: 24, cut: false }, Desc::Either { j: 10, by: -40, bb: 24, cut: false }, Desc::Exit { j: 11, guest: 0x401008 }, Desc::Mark { j: 15, bb: 24, c: 15 }, Desc::Register { j: 18, rd: 15, bb: 24, cut: true }, Desc::Register { j: 19, rd: 15, bb: 24, cut: true }, Desc::Either { j: 20, by: 8, bb: 24, cut: true }, Desc::Branch { j: 21, by: -60, bb: 24, cut: true }, Desc::Probes { j: 22, c: 15, off: 0, bytes: 8, store: true, bb: 24 }, Desc::Branch { j: 27, by: -84, bb: 24, cut: true }])) by (compute); }
proof fn pin_post_load_line() { assert(described(seq![0xd360dc2fu32, 0xb500018fu32, 0x8b214317u32, 0x9240082fu32, 0xb500018fu32, 0x9100202fu32, 0xf84086e2u32, 0xaa0f03e1u32, 0xf1000400u32, 0xd2880417u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff2u32, 0xd280000fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39401dffu32, 0x17ffffedu32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, bb: 24, cut: false }, Desc::Either { j: 1, by: 48, bb: 24, cut: false }, Desc::Cut { j: 2, n: 1, bb: 24 }, Desc::Register { j: 3, rd: 15, bb: 24, cut: true }, Desc::Either { j: 4, by: 48, bb: 24, cut: true }, Desc::Register { j: 5, rd: 15, bb: 24, cut: true }, Desc::Through { j: 6, rt: 2, back: true, off: 0, bytes: 8, bb: 24 }, Desc::Register { j: 7, rd: 1, bb: 24, cut: false }, Desc::Register { j: 8, rd: 0, bb: 24, cut: false }, Desc::Exit { j: 9, guest: 0x401004 }, Desc::Mark { j: 13, bb: 24, c: 15 }, Desc::Register { j: 16, rd: 15, bb: 24, cut: true }, Desc::Register { j: 17, rd: 15, bb: 24, cut: true }, Desc::Either { j: 18, by: 8, bb: 24, cut: true }, Desc::Branch { j: 19, by: -56, bb: 24, cut: true }, Desc::Probes { j: 20, c: 15, off: 0, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 24, by: -76, bb: 24, cut: true }])) by (compute); }
proof fn pin_line_to_jump() { assert(described(seq![0x91000652u32, 0x91000400u32, 0xd360dfd7u32, 0xb50000d7u32, 0x8b3e4317u32, 0xd37eaef7u32, 0xb3420ef7u32, 0xd25202f7u32, 0xd61f02e0u32, 0x14000001u32, 0xf9002f9eu32, 0xd2800e1eu32, 0xf2a0001eu32, 0xf2c4003eu32, 0xd63f03c0u32, 0xf9402f9eu32, 0x17fffffau32], pin_at(false), seq![Desc::Register { j: 0, rd: 18, bb: 24, cut: false }, Desc::Register { j: 1, rd: 0, bb: 24, cut: false }, Desc::Jump { j: 2, n: 30, out: 9 }, Desc::Branch { j: 9, by: 4, bb: 24, cut: false }, Desc::Call { j: 10, code: 7 }])) by (compute); }
proof fn pin_stand_in_add_line() { assert(described(seq![0xf9004f9du32, 0xaa1803fdu32, 0xf9404798u32, 0x91000718u32, 0x91000400u32, 0xf9004798u32, 0xaa1d03f8u32, 0xf9404f9du32, 0xd2880417u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32], pin_at(false), seq![Desc::Frame { j: 0, f: FrameAccess { word: 0xf9004f9du32, rt: 31, off: 152, moves: false }, bb: 24, bb2: 24 }, Desc::Frame { j: 1, f: FrameAccess { word: 0xaa1803fdu32, rt: 29, off: 24, moves: true }, bb: 24, bb2: 29 }, Desc::Frame { j: 2, f: FrameAccess { word: 0xf9404798u32, rt: 24, off: 136, moves: false }, bb: 29, bb2: 29 }, Desc::Register { j: 3, rd: 24, bb: 29, cut: false }, Desc::Register { j: 4, rd: 0, bb: 29, cut: false }, Desc::Frame { j: 5, f: FrameAccess { word: 0xf9004798u32, rt: 31, off: 136, moves: false }, bb: 29, bb2: 29 }, Desc::Frame { j: 6, f: FrameAccess { word: 0xaa1d03f8u32, rt: 24, off: 29, moves: true }, bb: 29, bb2: 24 }, Desc::Frame { j: 7, f: FrameAccess { word: 0xf9404f9du32, rt: 29, off: 152, moves: false }, bb: 24, bb2: 24 }, Desc::Exit { j: 8, guest: 0x401004 }])) by (compute); }
proof fn pin_stand_in_load_line() { assert(described(seq![0xf9004f9du32, 0xaa1803fdu32, 0xf9404798u32, 0xd360df0fu32, 0xb50001afu32, 0x8b3843b7u32, 0x92400b0fu32, 0xb50001afu32, 0xf94002e2u32, 0x91000442u32, 0xf9004798u32, 0xaa1d03f8u32, 0xf9404f9du32, 0xd2880417u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd26003afu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff1u32, 0xd280000fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39401dffu32, 0x17ffffecu32], pin_at(false), seq![Desc::Frame { j: 0, f: FrameAccess { word: 0xf9004f9du32, rt: 31, off: 152, moves: false }, bb: 24, bb2: 24 }, Desc::Frame { j: 1, f: FrameAccess { word: 0xaa1803fdu32, rt: 29, off: 24, moves: true }, bb: 24, bb2: 29 }, Desc::Frame { j: 2, f: FrameAccess { word: 0xf9404798u32, rt: 24, off: 136, moves: false }, bb: 29, bb2: 29 }, Desc::Register { j: 3, rd: 15, bb: 29, cut: false }, Desc::Either { j: 4, by: 52, bb: 29, cut: false }, Desc::Cut { j: 5, n: 24, bb: 29 }, Desc::Register { j: 6, rd: 15, bb: 29, cut: true }, Desc::Either { j: 7, by: 52, bb: 29, cut: true }, Desc::Through { j: 8, rt: 2, back: false, off: 0, bytes: 8, bb: 29 }, Desc::Register { j: 9, rd: 2, bb: 29, cut: false }, Desc::Frame { j: 10, f: FrameAccess { word: 0xf9004798u32, rt: 31, off: 136, moves: false }, bb: 29, bb2: 29 }, Desc::Frame { j: 11, f: FrameAccess { word: 0xaa1d03f8u32, rt: 24, off: 29, moves: true }, bb: 29, bb2: 24 }, Desc::Frame { j: 12, f: FrameAccess { word: 0xf9404f9du32, rt: 29, off: 152, moves: false }, bb: 24, bb2: 24 }, Desc::Exit { j: 13, guest: 0x401004 }, Desc::Mark { j: 17, bb: 29, c: 15 }, Desc::Register { j: 20, rd: 15, bb: 29, cut: true }, Desc::Register { j: 21, rd: 15, bb: 29, cut: true }, Desc::Either { j: 22, by: 8, bb: 29, cut: true }, Desc::Branch { j: 23, by: -60, bb: 29, cut: true }, Desc::Probes { j: 24, c: 15, off: 0, bytes: 8, store: false, bb: 29 }, Desc::Branch { j: 28, by: -80, bb: 29, cut: true }])) by (compute); }
proof fn pin_load_4096() { assert(described(seq![0xd360dc2fu32, 0xb500012fu32, 0x8b214317u32, 0x9240082fu32, 0xb500012fu32, 0xf94802e2u32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff5u32, 0xd282000fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39401dffu32, 0x17fffff0u32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, bb: 24, cut: false }, Desc::Either { j: 1, by: 36, bb: 24, cut: false }, Desc::Cut { j: 2, n: 1, bb: 24 }, Desc::Register { j: 3, rd: 15, bb: 24, cut: true }, Desc::Either { j: 4, by: 36, bb: 24, cut: true }, Desc::Through { j: 5, rt: 2, back: false, off: 4096, bytes: 8, bb: 24 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Mark { j: 10, bb: 24, c: 15 }, Desc::Register { j: 13, rd: 15, bb: 24, cut: true }, Desc::Register { j: 14, rd: 15, bb: 24, cut: true }, Desc::Either { j: 15, by: 8, bb: 24, cut: true }, Desc::Branch { j: 16, by: -44, bb: 24, cut: true }, Desc::Probes { j: 17, c: 15, off: 4096, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 21, by: -64, bb: 24, cut: true }])) by (compute); }
proof fn pin_store_4096() { assert(described(seq![0xd360dc2fu32, 0xb500012fu32, 0x8b214317u32, 0x9240082fu32, 0xb500012fu32, 0xf90802e2u32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff5u32, 0xd282000fu32, 0x8b2f62efu32, 0x383f01ffu32, 0x91001defu32, 0x383f01ffu32, 0x17ffffefu32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, bb: 24, cut: false }, Desc::Either { j: 1, by: 36, bb: 24, cut: false }, Desc::Cut { j: 2, n: 1, bb: 24 }, Desc::Register { j: 3, rd: 15, bb: 24, cut: true }, Desc::Either { j: 4, by: 36, bb: 24, cut: true }, Desc::Through { j: 5, rt: 31, back: false, off: 4096, bytes: 8, bb: 24 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Mark { j: 10, bb: 24, c: 15 }, Desc::Register { j: 13, rd: 15, bb: 24, cut: true }, Desc::Register { j: 14, rd: 15, bb: 24, cut: true }, Desc::Either { j: 15, by: 8, bb: 24, cut: true }, Desc::Branch { j: 16, by: -44, bb: 24, cut: true }, Desc::Probes { j: 17, c: 15, off: 4096, bytes: 8, store: true, bb: 24 }, Desc::Branch { j: 22, by: -68, bb: 24, cut: true }])) by (compute); }
proof fn pin_load_32760() { assert(described(seq![0xd360dc2fu32, 0xb500012fu32, 0x8b214317u32, 0x9240082fu32, 0xb500012fu32, 0xf97ffee2u32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x913ffeefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff5u32, 0xd28fff0fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39401dffu32, 0x17fffff0u32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, bb: 24, cut: false }, Desc::Either { j: 1, by: 36, bb: 24, cut: false }, Desc::Cut { j: 2, n: 1, bb: 24 }, Desc::Register { j: 3, rd: 15, bb: 24, cut: true }, Desc::Either { j: 4, by: 36, bb: 24, cut: true }, Desc::Through { j: 5, rt: 2, back: false, off: 32760, bytes: 8, bb: 24 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Mark { j: 10, bb: 24, c: 15 }, Desc::Register { j: 13, rd: 15, bb: 24, cut: true }, Desc::Register { j: 14, rd: 15, bb: 24, cut: true }, Desc::Either { j: 15, by: 8, bb: 24, cut: true }, Desc::Branch { j: 16, by: -44, bb: 24, cut: true }, Desc::Probes { j: 17, c: 15, off: 32760, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 21, by: -64, bb: 24, cut: true }])) by (compute); }
proof fn pin_load_word_4096() { assert(described(seq![0xd360dc2fu32, 0xb500012fu32, 0x8b214317u32, 0x9240042fu32, 0xb500012fu32, 0xb95002e2u32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91000eefu32, 0x927e25efu32, 0xb400004fu32, 0x17fffff5u32, 0xd282000fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39400dffu32, 0x17fffff0u32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, bb: 24, cut: false }, Desc::Either { j: 1, by: 36, bb: 24, cut: false }, Desc::Cut { j: 2, n: 1, bb: 24 }, Desc::Register { j: 3, rd: 15, bb: 24, cut: true }, Desc::Either { j: 4, by: 36, bb: 24, cut: true }, Desc::Through { j: 5, rt: 2, back: false, off: 4096, bytes: 4, bb: 24 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Mark { j: 10, bb: 24, c: 15 }, Desc::Register { j: 13, rd: 15, bb: 24, cut: true }, Desc::Register { j: 14, rd: 15, bb: 24, cut: true }, Desc::Either { j: 15, by: 8, bb: 24, cut: true }, Desc::Branch { j: 16, by: -44, bb: 24, cut: true }, Desc::Probes { j: 17, c: 15, off: 4096, bytes: 4, store: false, bb: 24 }, Desc::Branch { j: 21, by: -64, bb: 24, cut: true }])) by (compute); }
proof fn pin_load_byte_4095() { assert(described(seq![0xd360dc2fu32, 0xb50000efu32, 0x8b214317u32, 0x397ffee2u32, 0xd2880217u32, 0xf2a02017u32, 0xf2c80317u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32], pin_at(false), seq![Desc::Register { j: 0, rd: 15, bb: 24, cut: false }, Desc::Either { j: 1, by: 28, bb: 24, cut: false }, Desc::Cut { j: 2, n: 1, bb: 24 }, Desc::Through { j: 3, rt: 2, back: false, off: 4095, bytes: 1, bb: 24 }, Desc::Exit { j: 4, guest: 0x401000 }, Desc::Mark { j: 8, bb: 24, c: 15 }])) by (compute); }
proof fn pin_load_4096_to_jump() { assert(described(seq![0x91000652u32, 0xd360dc2fu32, 0xb500018fu32, 0x8b214317u32, 0x9240082fu32, 0xb500018fu32, 0xf94802e2u32, 0xd360dfd7u32, 0xb5000257u32, 0x8b3e4317u32, 0xd37eaef7u32, 0xb3420ef7u32, 0xd25202f7u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff2u32, 0xd282000fu32, 0x8b2f62efu32, 0x394001ffu32, 0x39401dffu32, 0x17ffffedu32, 0x14000001u32, 0xf9002f9eu32, 0xd2800e1eu32, 0xf2a0001eu32, 0xf2c4003eu32, 0xd63f03c0u32, 0xf9402f9eu32, 0x17fffffau32], pin_at(false), seq![Desc::Register { j: 0, rd: 18, bb: 24, cut: false }, Desc::Register { j: 1, rd: 15, bb: 24, cut: false }, Desc::Either { j: 2, by: 48, bb: 24, cut: false }, Desc::Cut { j: 3, n: 1, bb: 24 }, Desc::Register { j: 4, rd: 15, bb: 24, cut: true }, Desc::Either { j: 5, by: 48, bb: 24, cut: true }, Desc::Through { j: 6, rt: 2, back: false, off: 4096, bytes: 8, bb: 24 }, Desc::Jump { j: 7, n: 30, out: 26 }, Desc::Mark { j: 14, bb: 24, c: 15 }, Desc::Register { j: 17, rd: 15, bb: 24, cut: true }, Desc::Register { j: 18, rd: 15, bb: 24, cut: true }, Desc::Either { j: 19, by: 8, bb: 24, cut: true }, Desc::Branch { j: 20, by: -56, bb: 24, cut: true }, Desc::Probes { j: 21, c: 15, off: 4096, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 25, by: -76, bb: 24, cut: true }, Desc::Branch { j: 26, by: 4, bb: 24, cut: false }, Desc::Call { j: 27, code: 7 }])) by (compute); }
proof fn pin_store_tested_loop_displaced() { assert(described(seq![0x92400837u32, 0xb40003f7u32, 0x91000652u32, 0xd360dc2fu32, 0xb500016fu32, 0x8b214317u32, 0x9240082fu32, 0xb500016fu32, 0xf90002e2u32, 0xf1000400u32, 0x54fffec1u32, 0xd2880617u32, 0xf2a02017u32, 0xf2c80017u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x91001eefu32, 0x927d21efu32, 0xb400004fu32, 0x17fffff3u32, 0xd280000fu32, 0x8b2f62efu32, 0x383f01ffu32, 0x91001defu32, 0x383f01ffu32, 0x17ffffedu32, 0x17ffffe4u32, 0xf2a02017u32, 0xf2c80017u32, 0xd61f02e0u32, 0x91000652u32, 0xd360dc2fu32, 0xb500010fu32, 0xf8214b02u32, 0xf1000400u32, 0x54ffff61u32, 0xd2880617u32, 0xf2a02017u32, 0xf2c80017u32, 0xd61f02e0u32, 0xd260030fu32, 0x397ffdffu32, 0x00000000u32, 0x17fffff3u32, 0xf2a02017u32, 0xf2c80017u32, 0xd61f02e0u32], pin_at(true), seq![Desc::Register { j: 0, rd: 23, bb: 24, cut: false }, Desc::Either { j: 1, by: 124, bb: 24, cut: false }, Desc::Register { j: 2, rd: 18, bb: 24, cut: false }, Desc::Register { j: 3, rd: 15, bb: 24, cut: false }, Desc::Either { j: 4, by: 44, bb: 24, cut: false }, Desc::Cut { j: 5, n: 1, bb: 24 }, Desc::Register { j: 6, rd: 15, bb: 24, cut: true }, Desc::Either { j: 7, by: 44, bb: 24, cut: true }, Desc::Through { j: 8, rt: 31, back: false, off: 0, bytes: 8, bb: 24 }, Desc::Register { j: 9, rd: 0, bb: 24, cut: false }, Desc::Either { j: 10, by: -40, bb: 24, cut: false }, Desc::Exit { j: 11, guest: 0x401008 }, Desc::Mark { j: 15, bb: 24, c: 15 }, Desc::Register { j: 18, rd: 15, bb: 24, cut: true }, Desc::Register { j: 19, rd: 15, bb: 24, cut: true }, Desc::Either { j: 20, by: 8, bb: 24, cut: true }, Desc::Branch { j: 21, by: -52, bb: 24, cut: true }, Desc::Probes { j: 22, c: 15, off: 0, bytes: 8, store: true, bb: 24 }, Desc::Branch { j: 27, by: -76, bb: 24, cut: true }, Desc::Register { j: 32, rd: 18, bb: 24, cut: false }, Desc::Register { j: 33, rd: 15, bb: 24, cut: false }, Desc::Either { j: 34, by: 32, bb: 24, cut: false }, Desc::Own { j: 35, n: 1, rt: 31, bytes: 8, bb: 24 }, Desc::Register { j: 36, rd: 0, bb: 24, cut: false }, Desc::Either { j: 37, by: -20, bb: 24, cut: false }, Desc::Exit { j: 38, guest: 0x401008 }, Desc::Mark { j: 42, bb: 24, c: 15 }])) by (compute); }

} // verus!
