//! THE INTERRUPT BOUND (the owner's ruling of 2026-11-28: "an interrupt stops a run" is stated not in time but as
//! "after its interrupter is used, a run executes at most N more guest instructions", each executor stating its N).
//!
//! The look is the executor's ([`since_look`]): the translated code looks at nothing. So the bound is a property of
//! the pieces: a piece whose run never comes back to a word it ran ([`runs_each_word_once`]) is left within as many
//! steps as it has words ([`leaves_within`]), and then a whole run makes each step within that many steps of its last
//! look ([`whole_run_looks`]). A described piece has that property when its moves inside itself can be ranked: every
//! word a block can go on to inside the piece ([`goes_to`]) has a higher rank ([`ranked`], decidable over the
//! description; `tests/compose.rs` finds the rank).
//!
//! WHAT IT TAKES ON TRUST, beside what [`compose`](super::compose) does (the word meanings, the memory map, the re-entry):
//! (1) The look: the in-process executor reads its flag before every entry of a piece (execution.rs:478, then `enter`
//! at :501), and after a run left a piece only through a step into the slot table, the floor or the end of the run;
//! its other paths back into a piece (`resume` at :589 after the floor answered a call, `Resolved::Again`,
//! `FloatStatusNotKept`, `LastTarget`) go on in the middle of the piece and are not covered (as for confinement).
//! (2) That the flag's store is seen by the next look (Rust's atomics: `swap` with `AcqRel` against the
//! interrupter's store; the memory model promises visibility only "in a finite period of time"). (3) The stop signal
//! is not needed for the bound: it only ends a run sooner (execution.rs:530-547). (4) From host steps to guest
//! instructions: a piece of look-ahead none is the translation of one guest instruction (execution.rs:127-133 gives the
//! translator one word), and a piece that runs no word twice runs it at most once.

use super::*;
use super::meaning::*;
use super::copy::*;
use super::access::*;
use super::branch::*;
use super::withheld::*;
use super::withheld_access::*;
use super::compose::*;

verus! {

/// Where word `i` of block `d` can go on to inside its piece: the next word, or a branch's target; nowhere from a word
/// that leaves the piece (an exit's or a table jump's `br x23`, a floor call's `blr x30`) or ends the run (`udf`).
pub open spec fn goes_to(d: Desc, i: int, t: int) -> bool {
    match d {
        Desc::Either { j, by, .. } => t == j + 1 || t == j + by / 4,
        Desc::Branch { j, by, .. } => t == j + by / 4,
        Desc::Stop { .. } => false,
        Desc::Mark { j, .. } => i < j + 2 && t == i + 1,
        Desc::Exit { j, .. } => i < j + 3 && t == i + 1,
        Desc::Call { j, .. } => i < j + 4 && t == i + 1,
        Desc::TableJump { j, link } => i < j + 3 + link_len(link) && t == i + 1,
        Desc::DisplacedTableJump { j, link, .. } => i < j + 3 + link_len(link) && t == i + 1,
        _ => t == i + 1,
    }
}

/// A move from word `i` to word `t` of the piece goes up in rank (none to check if `t` is outside the piece).
pub open spec fn rank_step(rank: Seq<int>, i: int, t: int) -> bool {
    0 <= t < rank.len() ==> 0 <= i < rank.len() && rank[i] < rank[t]
}

/// Each of the `n` words from `j` goes up in rank to the next.
pub open spec fn rank_line(rank: Seq<int>, j: int, n: int) -> bool
    decreases n,
{
    if n <= 0 { true } else { rank_step(rank, j + n - 1, j + n) && rank_line(rank, j, n - 1) }
}

/// Every move block `d`'s words can make inside the piece goes up in rank.
pub open spec fn desc_ranked(rank: Seq<int>, d: Desc) -> bool {
    match d {
        Desc::Either { j, by, .. } => rank_step(rank, j, j + 1) && rank_step(rank, j, j + by / 4),
        Desc::Branch { j, by, .. } => rank_step(rank, j, j + by / 4),
        Desc::Stop { .. } => true,
        Desc::Mark { j, .. } => rank_line(rank, j, 2),
        Desc::Probes { j, store, .. } => rank_line(rank, j, probes_length(store)),
        Desc::Exit { j, .. } => rank_line(rank, j, 3),
        Desc::Call { j, .. } => rank_line(rank, j, 4),
        Desc::StackStep { j, .. } => rank_line(rank, j, 2),
        Desc::StackVia { j, .. } => rank_line(rank, j, 2),
        Desc::StackProbes { j, store, .. } => rank_line(rank, j, stack_probes_length(store)),
        Desc::StackAt { j, .. } => rank_line(rank, j, 3),
        Desc::TableJump { j, link } => rank_line(rank, j, 3 + link_len(link)),
        Desc::DisplacedTableJump { j, link, .. } => rank_line(rank, j, 3 + link_len(link)),
        Desc::ShiftThrough { j, .. } => rank_line(rank, j, 2),
        _ => rank_line(rank, entry_of(d), 1),
    }
}

pub open spec fn ranks_below(rank: Seq<int>, k: int) -> bool
    decreases k,
{
    if k <= 0 { true } else { 0 <= rank[k - 1] < rank.len() && ranks_below(rank, k - 1) }
}

pub open spec fn descs_ranked(rank: Seq<int>, descs: Seq<Desc>, k: int) -> bool
    decreases k,
{
    if k <= 0 { true } else { desc_ranked(rank, descs[k - 1]) && descs_ranked(rank, descs, k - 1) }
}

/// THE PIECE HOLDS NO LOOP: a rank below the piece's length for each word, which every move inside it raises.
pub open spec fn ranked(descs: Seq<Desc>, rank: Seq<int>, len: int) -> bool {
    &&& rank.len() == len
    &&& ranks_below(rank, len)
    &&& descs_ranked(rank, descs, descs.len() as int)
}

proof fn lemma_ranks_below(rank: Seq<int>, k: int, i: int)
    requires
        ranks_below(rank, k),
        0 <= i < k <= rank.len(),
    ensures
        0 <= rank[i] < rank.len(),
    decreases k,
{
    if i < k - 1 {
        lemma_ranks_below(rank, k - 1, i);
    }
}

proof fn lemma_descs_ranked(rank: Seq<int>, descs: Seq<Desc>, k: int, b: int)
    requires
        descs_ranked(rank, descs, k),
        0 <= b < k <= descs.len(),
    ensures
        desc_ranked(rank, descs[b]),
    decreases k,
{
    if b < k - 1 {
        lemma_descs_ranked(rank, descs, k - 1, b);
    }
}

proof fn lemma_rank_line(rank: Seq<int>, j: int, n: int, i: int)
    requires
        rank_line(rank, j, n),
        j <= i < j + n,
    ensures
        rank_step(rank, i, i + 1),
    decreases n,
{
    if i < j + n - 1 {
        lemma_rank_line(rank, j, n - 1, i);
    }
}

/// A move block `d`'s word `i` can make inside the piece goes up in rank.
proof fn lemma_desc_ranked(a: Allowed, rank: Seq<int>, d: Desc, i: int, t: int)
    requires
        desc_ranked(rank, d),
        (block_of(a, d).owns)(i),
        goes_to(d, i, t),
        0 <= t < rank.len(),
    ensures
        0 <= i < rank.len() && rank[i] < rank[t],
{
    match d {
        Desc::Either { .. } => {},
        Desc::Branch { .. } => {},
        Desc::Stop { .. } => {},
        Desc::Mark { j, .. } => { lemma_rank_line(rank, j, 2, i); },
        Desc::Probes { j, store, .. } => { lemma_rank_line(rank, j, probes_length(store), i); },
        Desc::Exit { j, .. } => { lemma_rank_line(rank, j, 3, i); },
        Desc::Call { j, .. } => { lemma_rank_line(rank, j, 4, i); },
        Desc::StackStep { j, .. } => { lemma_rank_line(rank, j, 2, i); },
        Desc::StackVia { j, .. } => { lemma_rank_line(rank, j, 2, i); },
        Desc::StackProbes { j, store, .. } => { lemma_rank_line(rank, j, stack_probes_length(store), i); },
        Desc::StackAt { j, .. } => { lemma_rank_line(rank, j, 3, i); },
        Desc::TableJump { j, link } => { lemma_rank_line(rank, j, 3 + link_len(link), i); },
        Desc::DisplacedTableJump { j, link, .. } => { lemma_rank_line(rank, j, 3 + link_len(link), i); },
        Desc::ShiftThrough { j, .. } => { lemma_rank_line(rank, j, 2, i); },
        _ => { lemma_rank_line(rank, entry_of(d), 1, i); },
    }
}

/// A step to the next word stands at the next word.
proof fn lemma_next_word(instructions: Seq<u32>, a: Allowed, m: Machine, after: Machine)
    requires
        placed(a, instructions),
        runs(a, m),
        after.pc == (m.pc + 4) as u64,
    ensures
        runs(a, after) ==> index(a, after.pc) == index(a, m.pc) + 1,
{
}

/// A step `by` bytes to a word of the piece stands there.
proof fn lemma_by_word(instructions: Seq<u32>, a: Allowed, m: Machine, after: Machine, j: int, by: int)
    requires
        placed(a, instructions),
        runs(a, m),
        index(a, m.pc) == j,
        by % 4 == 0 && 0 <= j + by / 4 < instructions.len(),
        after.pc == (m.pc + by) as u64,
    ensures
        index(a, after.pc) == j + by / 4,
{
    assert(m.pc as int == a.code + 4 * j);
    assert(m.pc + by == a.code + 4 * (j + by / 4));
}

/// A step past the slot tables' start or into the floor's page is out of the piece.
proof fn lemma_far(instructions: Seq<u32>, a: Allowed, after: Machine)
    requires
        placed(a, instructions),
        after.pc as int >= PROCESS_SLOTS || (FLOOR_PAGE <= after.pc && (after.pc as int) < FLOOR_PAGE as int + 4096),
    ensures
        !in_sequence(a, after.pc),
{
}

/// THE MOVES OF A BLOCK: a step from a word some block owns, where its invariant holds, that stays in the piece goes
/// where [`goes_to`] says.
pub proof fn lemma_moves(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, descs: Seq<Desc>, b: int, m: Machine)
    requires
        placed(a, instructions),
        described(instructions, a, descs),
        all_meant(step, instructions, a, descs),
        0 <= b < descs.len(),
        runs(a, m),
        (block_of(a, descs[b]).owns)(index(a, m.pc)),
        (block_of(a, descs[b]).inv)(m),
    ensures
        ({
            let s = step(m, word_at(instructions, a, m.pc));
            runs(a, s.after) ==> goes_to(descs[b], index(a, m.pc), index(a, s.after.pc))
        }),
{
    lemma_shapes(instructions, a, descs, descs.len() as int, b);
    assert(meant(step, instructions, a, descs[b]));
    let i = index(a, m.pc);
    assert(word_at(instructions, a, m.pc) == instructions[i]);
    let s = step(m, instructions[i]);
    assert(m.pc as int == a.code + 4 * i);
    assert(m.x.len() == 31);
    match descs[b] {
        Desc::Register { .. } => {
            lemma_next_word(instructions, a, m, s.after);
        },
        Desc::Cut { .. } => {
            lemma_next_word(instructions, a, m, s.after);
        },
        Desc::Either { j, by, .. } => {
            if s.after.pc == (m.pc + 4) as u64 {
                lemma_next_word(instructions, a, m, s.after);
            } else {
                lemma_by_word(instructions, a, m, s.after, j, by);
            }
        },
        Desc::Branch { j, by, .. } => {
            lemma_by_word(instructions, a, m, s.after, j, by);
        },
        Desc::Through { .. } => {
            if !s.after.ended {
                lemma_next_word(instructions, a, m, s.after);
            }
        },
        Desc::Own { .. } => {
            if !s.after.ended {
                lemma_next_word(instructions, a, m, s.after);
            }
        },
        Desc::Mark { j, bb, c } => {
            assert(instructions[j] == eor_c(c, bb) && instructions[j + 1] == mark_load_c(c) && instructions[j + 2] == UDF) by {
                assert(instructions.subrange(j, j + 3)[0] == instructions[j]);
                assert(instructions.subrange(j, j + 3)[1] == instructions[j + 1]);
                assert(instructions.subrange(j, j + 3)[2] == instructions[j + 2]);
            }
            if i < j + 2 && !s.after.ended {
                lemma_next_word(instructions, a, m, s.after);
            }
        },
        Desc::Probes { j, c, from, off, bytes, store, .. } => {
            let w = probes_words(c, from, off, bytes, store);
            assert(instructions[i] == w[i - j]) by {
                assert(instructions.subrange(j, j + w.len())[i - j] == instructions[i]);
            }
            if store {
                assert(w[0] == movx_c(c, off) && w[1] == add_from_c(c, from) && w[2] == probe_store_c(c)
                    && w[3] == add_c_c(c, bytes - 1) && w[4] == probe_store_c(c));
            } else {
                assert(w[0] == movx_c(c, off) && w[1] == add_from_c(c, from) && w[2] == probe_load_c(c, 0)
                    && w[3] == probe_load_c(c, bytes - 1));
            }
            if !s.after.ended {
                lemma_next_word(instructions, a, m, s.after);
            }
        },
        Desc::Exit { j, guest } => {
            assert(in_reach(a, guest));
            assert((m.pc - a.code - 4 * j) / 4 == i - j);
            lemma_exit_step(step, instructions, a, guest, 4 * j, m);
            if i - j + 1 < 4 {
                lemma_next_word(instructions, a, m, s.after);
            }
        },
        Desc::Call { j, code } => {
            assert((m.pc - a.code - 4 * j) / 4 == i - j);
            lemma_floor_step(step, instructions, a, code, 4 * j, m);
            if i - j + 1 < 5 && !s.after.ended {
                lemma_next_word(instructions, a, m, s.after);
            }
        },
        Desc::Frame { .. } => {
            if !s.after.ended {
                lemma_next_word(instructions, a, m, s.after);
            }
        },
        Desc::Stop { .. } => {},
        Desc::StackCut { .. } => {
            lemma_next_word(instructions, a, m, s.after);
        },
        Desc::StackMove { .. } => {
            lemma_next_word(instructions, a, m, s.after);
        },
        Desc::StackStep { .. } => {
            if !s.after.ended {
                lemma_next_word(instructions, a, m, s.after);
            }
        },
        Desc::StackAccess { .. } => {
            if !s.after.ended {
                lemma_next_word(instructions, a, m, s.after);
            }
        },
        Desc::StackVia { .. } => {
            if !s.after.ended {
                lemma_next_word(instructions, a, m, s.after);
            }
        },
        Desc::StackProbes { j, c, bytes, store, .. } => {
            let w = stack_probes_words(c, bytes, store);
            assert(instructions[i] == w[i - j]) by {
                assert(instructions.subrange(j, j + w.len())[i - j] == instructions[i]);
            }
            if store {
                assert(w[0] == mov_c_sp(c) && w[1] == probe_store_c(c) && w[2] == add_c_c(c, bytes - 1) && w[3] == probe_store_c(c));
            } else {
                assert(w[0] == mov_c_sp(c) && w[1] == probe_load_c(c, 0) && w[2] == probe_load_c(c, bytes - 1));
            }
            if !s.after.ended {
                lemma_next_word(instructions, a, m, s.after);
            }
        },
        Desc::StackAt { j, off, .. } => {
            if i < j + 2 {
                assert(instructions[i] == seq![movx_c(23, off), add_from_c(23, 31)][i - j]) by {
                    assert(instructions.subrange(j, j + 2)[i - j] == instructions[i]);
                }
            }
            if !s.after.ended {
                lemma_next_word(instructions, a, m, s.after);
            }
        },
        Desc::TableJump { j, link } => {
            let o = i - j;
            if o < 3 {
                assert(instructions[i] == seq![UBFIZ_X23, BFXIL_X23, EOR_X23_TABLE][o]) by {
                    assert(instructions.subrange(j, j + 3)[o] == instructions[i]);
                }
                lemma_next_word(instructions, a, m, s.after);
            } else if o < 3 + link_len(link) {
                lemma_table_link(step, instructions, j, link, i, m);
                lemma_next_word(instructions, a, m, s.after);
            } else {
                assert(s.after.pc == m.x[23]);
                lemma_far(instructions, a, s.after);
            }
        },
        Desc::DisplacedTableJump { j, n, link } => {
            let o = i - j;
            if o < 3 {
                assert(instructions[i] == seq![ubfiz_x23(n as u32), BFXIL_X23, EOR_X23_TABLE][o]) by {
                    assert(instructions.subrange(j, j + 3)[o] == instructions[i]);
                }
                lemma_next_word(instructions, a, m, s.after);
            } else if o < 3 + link_len(link) {
                lemma_table_link(step, instructions, j, link, i, m);
                lemma_next_word(instructions, a, m, s.after);
            } else {
                assert(s.after.pc == m.x[23]);
                lemma_far(instructions, a, s.after);
            }
        },
        Desc::ShiftThrough { .. } => {
            if !s.after.ended {
                lemma_next_word(instructions, a, m, s.after);
            }
        },
    }
}

/// The run stands still once it stopped.
proof fn lemma_stopped_stays(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, start: Machine, n1: nat, n2: nat)
    requires
        n1 <= n2,
        !runs(a, run(step, instructions, a, start, n1)),
    ensures
        run(step, instructions, a, start, n2) == run(step, instructions, a, start, n1),
    decreases n2,
{
    if n2 > n1 {
        lemma_stopped_stays(step, instructions, a, start, n1, (n2 - 1) as nat);
    }
}

/// Along a run of a ranked described piece from a machine a run hands on, the rank goes up with every step.
proof fn lemma_rank_rises(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    descs: Seq<Desc>,
    rank: Seq<int>,
    start: Machine,
    n1: nat,
    n2: nat,
)
    requires
        placed(a, instructions),
        described(instructions, a, descs),
        all_meant(step, instructions, a, descs),
        ranked(descs, rank, instructions.len() as int),
        handed(a, start),
        n1 <= n2,
        runs(a, run(step, instructions, a, start, n2)),
    ensures
        runs(a, run(step, instructions, a, start, n1)),
        rank[index(a, run(step, instructions, a, start, n2).pc)] >= rank[index(a, run(step, instructions, a, start, n1).pc)] + (n2 - n1),
    decreases n2,
{
    if n2 > n1 {
        let before = run(step, instructions, a, start, (n2 - 1) as nat);
        if !runs(a, before) {
            lemma_stopped_stays(step, instructions, a, start, (n2 - 1) as nat, n2);
        }
        lemma_rank_rises(step, instructions, a, descs, rank, start, n1, (n2 - 1) as nat);
        let blocks = blocks_of(a, descs);
        lemma_described_blocks(step, instructions, a, descs);
        lemma_blocks_keep(step, instructions, a, blocks);
        lemma_chain_holds(step, instructions, a, in_blocks(blocks, a), start, (n2 - 1) as nat);
        assert(somewhere(blocks, a, before));
        let b = choose|b: int| 0 <= b < blocks.len() && (blocks[b].owns)(index(a, before.pc)) && #[trigger] (blocks[b].inv)(before);
        assert(blocks[b] == block_of(a, descs[b]));
        lemma_moves(step, instructions, a, descs, b, before);
        let after = run(step, instructions, a, start, n2);
        let i = index(a, before.pc);
        let t = index(a, after.pc);
        lemma_descs_ranked(rank, descs, descs.len() as int, b);
        lemma_desc_ranked(a, rank, descs[b], i, t);
    }
}

/// Some block owns the word the run is at, with its invariant.
pub open spec fn in_blocks(blocks: Seq<Block>, a: Allowed) -> spec_fn(Machine) -> bool {
    |m: Machine| somewhere(blocks, a, m)
}

/// The blocks' invariant: every machine a run hands on meets it, and every step keeps it.
proof fn lemma_blocks_keep(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, blocks: Seq<Block>)
    requires
        a.length == instructions.len() * 4,
        instructions.len() > 0,
        blocks.len() > 0 && blocks[0].entry == 0,
        forall|m: Machine| handed(a, m) ==> #[trigger] (blocks[0].needs)(m),
        forall|b: int| 0 <= b < blocks.len() ==> #[trigger] block_holds(step, instructions, a, blocks, b),
    ensures
        forall|m: Machine| handed(a, m) ==> #[trigger] (in_blocks(blocks, a))(m),
        kept_by_every_step(step, instructions, a, in_blocks(blocks, a)),
{
    let inv = in_blocks(blocks, a);
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
}

/// THE PIECE'S BOUND: a described piece that holds no loop runs no word twice, so it is left within as many steps as
/// it has words.
pub proof fn ranked_leaves(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, descs: Seq<Desc>, rank: Seq<int>)
    requires
        placed(a, instructions),
        described(instructions, a, descs),
        all_meant(step, instructions, a, descs),
        ranked(descs, rank, instructions.len() as int),
    ensures
        runs_each_word_once(step, instructions, a),
        leaves_within(step, instructions, a, instructions.len() as nat),
{
    assert forall|start: Machine, n: nat|
        #![trigger run(step, instructions, a, start, n)]
        handed(a, start) && runs(a, run(step, instructions, a, start, n)) implies n < instructions.len() by {
        lemma_rank_rises(step, instructions, a, descs, rank, start, 0, n);
        let i = index(a, run(step, instructions, a, start, n).pc);
        lemma_ranks_below(rank, instructions.len() as int, i);
        assert(index(a, start.pc) == 0);
        lemma_ranks_below(rank, instructions.len() as int, 0);
    }
    assert forall|start: Machine, n1: nat, n2: nat|
        #![trigger run(step, instructions, a, start, n1), run(step, instructions, a, start, n2)]
        handed(a, start) && n1 < n2 && runs(a, run(step, instructions, a, start, n2)) implies
            run(step, instructions, a, start, n1).pc != run(step, instructions, a, start, n2).pc by {
        lemma_rank_rises(step, instructions, a, descs, rank, start, n1, n2);
    }
}

/// THE INTERRUPT BOUND OF A WHOLE RUN: in a whole run of chained pieces, each left within `bound` steps, re-entered as
/// the executor does, every step is made within `bound` steps of the run's last look.
pub proof fn whole_run_looks(
    step: spec_fn(Machine, u32) -> Stepped,
    pieces: Seq<Piece>,
    reenter: spec_fn(u64, Machine) -> (int, Machine),
    k: int,
    start: Machine,
    bound: nat,
)
    requires
        0 <= k < pieces.len(),
        one_window(pieces, pieces[k].a),
        forall|i: int| 0 <= i < pieces.len() ==> #[trigger] is_chained(step, pieces[i].instructions, pieces[i].a),
        forall|i: int| 0 <= i < pieces.len() ==> #[trigger] leaves_within(step, pieces[i].instructions, pieces[i].a, bound),
        reenters(pieces, pieces[k].a, reenter),
        handed(pieces[k].a, start),
    ensures
        looks_within(step, pieces, reenter, k, start, bound),
{
    assert forall|n: nat| #![trigger whole_run(step, pieces, reenter, k, start, n)] {
        let w = whole_run(step, pieces, reenter, k, start, n);
        runs(pieces[w.0].a, w.1) ==> since_look(step, pieces, reenter, k, start, n) < bound
    } by {
        let (s0, n0) = lemma_whole_segment(step, pieces, reenter, k, start, n);
        let w = whole_run(step, pieces, reenter, k, start, n);
        assert(leaves_within(step, pieces[w.0].instructions, pieces[w.0].a, bound));
        assert(w.1 == run(step, pieces[w.0].instructions, pieces[w.0].a, s0, n0));
    }
}

/// A BRANCH TO ITSELF IS A LOOP INSIDE ITS PIECE: a word that goes on to the next, whose next branches back to it,
/// has no rank. That is the piece of `b .` with no look-ahead (the count's add, then `b .-4`), and `cbnz x0, .` and the
/// like are the same with an `Either` (tests/compose.rs): between two looks such a piece runs its guest instruction
/// without end, and only the stop signal ends the run.
pub proof fn a_branch_back_has_no_rank(rank: Seq<int>, j: int, rd: int, sp: bool, bb: int, cut: int, either: bool)
    requires
        0 <= j && j + 1 < rank.len(),
        desc_ranked(rank, Desc::Register { j, rd, sp, bb, cut }),
        either ==> desc_ranked(rank, Desc::Either { j: j + 1, by: -4, bb, cut }),
        !either ==> desc_ranked(rank, Desc::Branch { j: j + 1, by: -4, bb, cut }),
    ensures
        false,
{
    lemma_rank_line(rank, j, 1, j);
}

/// THE IN-PROCESS EXECUTOR'S BOUND: a whole run of described pieces that hold no loop, none longer than `bound` words,
/// makes every step within `bound` steps of its last look, and runs no word of a piece twice between two looks.
pub proof fn described_whole_run_looks(
    step: spec_fn(Machine, u32) -> Stepped,
    pieces: Seq<Piece>,
    descs: Seq<Seq<Desc>>,
    ranks: Seq<Seq<int>>,
    reenter: spec_fn(u64, Machine) -> (int, Machine),
    k: int,
    start: Machine,
    bound: nat,
)
    requires
        0 <= k < pieces.len(),
        descs.len() == pieces.len() && ranks.len() == pieces.len(),
        one_window(pieces, pieces[k].a),
        forall|i: int| 0 <= i < pieces.len() ==> {
            let p = #[trigger] pieces[i];
            &&& placed(p.a, p.instructions)
            &&& described(p.instructions, p.a, descs[i])
            &&& all_meant(step, p.instructions, p.a, descs[i])
            &&& ranked(descs[i], ranks[i], p.instructions.len() as int)
            &&& p.instructions.len() <= bound
        },
        reenters(pieces, pieces[k].a, reenter),
        handed(pieces[k].a, start),
    ensures
        looks_within(step, pieces, reenter, k, start, bound),
        forall|i: int| 0 <= i < pieces.len() ==> #[trigger] runs_each_word_once(step, pieces[i].instructions, pieces[i].a),
{
    assert forall|i: int| 0 <= i < pieces.len() implies {
        &&& #[trigger] is_chained(step, pieces[i].instructions, pieces[i].a)
        &&& leaves_within(step, pieces[i].instructions, pieces[i].a, bound)
        &&& runs_each_word_once(step, pieces[i].instructions, pieces[i].a)
    } by {
        let p = pieces[i];
        described_is_chained(step, p.instructions, p.a, descs[i]);
        ranked_leaves(step, p.instructions, p.a, descs[i], ranks[i]);
        assert forall|s: Machine, n: nat|
            #![trigger run(step, p.instructions, p.a, s, n)]
            handed(p.a, s) && runs(p.a, run(step, p.instructions, p.a, s, n)) implies n < bound by {
            assert(n < p.instructions.len());
        }
    }
    assert forall|i: int| 0 <= i < pieces.len() implies #[trigger] leaves_within(step, pieces[i].instructions, pieces[i].a, bound) by {
        assert(is_chained(step, pieces[i].instructions, pieces[i].a));
    }
    assert forall|i: int| 0 <= i < pieces.len() implies #[trigger] runs_each_word_once(step, pieces[i].instructions, pieces[i].a) by {
        assert(is_chained(step, pieces[i].instructions, pieces[i].a));
    }
    whole_run_looks(step, pieces, reenter, k, start, bound);
}

/// The pins: the ranks `tests/compose.rs` finds for the pieces of compose.rs's pins that hold no loop, which `compute`
/// checks are ranks [`ranked`] accepts.
proof fn pin_ranked_post_load_line() { assert(ranked(seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 48, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 1, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 4, by: 48, bb: 24, cut: 23 }, Desc::Register { j: 5, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Through { j: 6, rt: 2, rt2: 31, back: true, off: 0, bytes: 8, bb: 24, cut: 23 }, Desc::Register { j: 7, rd: 1, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 8, rd: 0, sp: false, bb: 24, cut: 31 }, Desc::Exit { j: 9, guest: 0x401004 }, Desc::Mark { j: 13, bb: 24, c: 15 }, Desc::Register { j: 16, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 17, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 18, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 19, by: -56, bb: 24, cut: 23 }, Desc::Probes { j: 20, c: 15, from: 23, off: 0, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 24, by: -76, bb: 24, cut: 23 }], seq![0, 1, 2, 3, 4, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 5, 6, 7, 8, 9, 10, 11, 12, 13], 25)) by (compute); }
proof fn pin_ranked_line_to_jump() { assert(ranked(seq![Desc::Register { j: 0, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 0, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 2, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 3, by: 24, bb: 24, cut: 31 }, Desc::Cut { j: 4, n: 30, c: 23, bb: 24 }, Desc::TableJump { j: 5, link: false }, Desc::Branch { j: 9, by: 4, bb: 24, cut: 31 }, Desc::Call { j: 10, code: 7 }], seq![2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 1, 0], 17)) by (compute); }
proof fn pin_ranked_stand_in_add_line() { assert(ranked(seq![Desc::Frame { j: 0, f: FrameAccess { word: 0xf9004f9du32, rt: 31, off: 152, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::Frame { j: 1, f: FrameAccess { word: 0xaa1803fdu32, rt: 29, off: 24, moves: true }, bb: 24, bb2: 29, cut: 31 }, Desc::Frame { j: 2, f: FrameAccess { word: 0xf9404798u32, rt: 24, off: 136, moves: false }, bb: 29, bb2: 29, cut: 31 }, Desc::Register { j: 3, rd: 24, sp: false, bb: 29, cut: 31 }, Desc::Register { j: 4, rd: 0, sp: false, bb: 29, cut: 31 }, Desc::Frame { j: 5, f: FrameAccess { word: 0xf9004798u32, rt: 31, off: 136, moves: false }, bb: 29, bb2: 29, cut: 31 }, Desc::Frame { j: 6, f: FrameAccess { word: 0xaa1d03f8u32, rt: 24, off: 29, moves: true }, bb: 29, bb2: 24, cut: 31 }, Desc::Frame { j: 7, f: FrameAccess { word: 0xf9404f9du32, rt: 29, off: 152, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::Exit { j: 8, guest: 0x401004 }], seq![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], 12)) by (compute); }
proof fn pin_ranked_stand_in_load_line() { assert(ranked(seq![Desc::Frame { j: 0, f: FrameAccess { word: 0xf9004f9du32, rt: 31, off: 152, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::Frame { j: 1, f: FrameAccess { word: 0xaa1803fdu32, rt: 29, off: 24, moves: true }, bb: 24, bb2: 29, cut: 31 }, Desc::Frame { j: 2, f: FrameAccess { word: 0xf9404798u32, rt: 24, off: 136, moves: false }, bb: 29, bb2: 29, cut: 31 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 29, cut: 31 }, Desc::Either { j: 4, by: 52, bb: 29, cut: 31 }, Desc::Cut { j: 5, n: 24, c: 23, bb: 29 }, Desc::Register { j: 6, rd: 15, sp: false, bb: 29, cut: 23 }, Desc::Either { j: 7, by: 52, bb: 29, cut: 23 }, Desc::Through { j: 8, rt: 2, rt2: 31, back: false, off: 0, bytes: 8, bb: 29, cut: 23 }, Desc::Register { j: 9, rd: 2, sp: false, bb: 29, cut: 23 }, Desc::Frame { j: 10, f: FrameAccess { word: 0xf9004798u32, rt: 31, off: 136, moves: false }, bb: 29, bb2: 29, cut: 23 }, Desc::Frame { j: 11, f: FrameAccess { word: 0xaa1d03f8u32, rt: 24, off: 29, moves: true }, bb: 29, bb2: 24, cut: 23 }, Desc::Frame { j: 12, f: FrameAccess { word: 0xf9404f9du32, rt: 29, off: 152, moves: false }, bb: 24, bb2: 24, cut: 23 }, Desc::Exit { j: 13, guest: 0x401004 }, Desc::Mark { j: 17, bb: 29, c: 15 }, Desc::Register { j: 20, rd: 15, sp: false, bb: 29, cut: 23 }, Desc::Register { j: 21, rd: 15, sp: false, bb: 29, cut: 23 }, Desc::Either { j: 22, by: 8, bb: 29, cut: 23 }, Desc::Branch { j: 23, by: -60, bb: 29, cut: 23 }, Desc::Probes { j: 24, c: 15, from: 23, off: 0, bytes: 8, store: false, bb: 29 }, Desc::Branch { j: 28, by: -80, bb: 29, cut: 23 }], seq![0, 1, 2, 3, 4, 5, 6, 7, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 8, 9, 10, 11, 12, 13, 14, 15, 16], 29)) by (compute); }
proof fn pin_ranked_load_4096() { assert(ranked(seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 36, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 1, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 4, by: 36, bb: 24, cut: 23 }, Desc::Through { j: 5, rt: 2, rt2: 31, back: false, off: 4096, bytes: 8, bb: 24, cut: 23 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Mark { j: 10, bb: 24, c: 15 }, Desc::Register { j: 13, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 14, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 15, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 16, by: -44, bb: 24, cut: 23 }, Desc::Probes { j: 17, c: 15, from: 23, off: 4096, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 21, by: -64, bb: 24, cut: 23 }], seq![0, 1, 2, 3, 4, 14, 15, 16, 17, 18, 19, 20, 21, 5, 6, 7, 8, 9, 10, 11, 12, 13], 22)) by (compute); }
proof fn pin_ranked_store_4096() { assert(ranked(seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 36, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 1, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 4, by: 36, bb: 24, cut: 23 }, Desc::Through { j: 5, rt: 31, rt2: 31, back: false, off: 4096, bytes: 8, bb: 24, cut: 23 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Mark { j: 10, bb: 24, c: 15 }, Desc::Register { j: 13, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 14, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 15, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 16, by: -44, bb: 24, cut: 23 }, Desc::Probes { j: 17, c: 15, from: 23, off: 4096, bytes: 8, store: true, bb: 24 }, Desc::Branch { j: 22, by: -68, bb: 24, cut: 23 }], seq![0, 1, 2, 3, 4, 15, 16, 17, 18, 19, 20, 21, 22, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14], 23)) by (compute); }
proof fn pin_ranked_load_32760() { assert(ranked(seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 36, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 1, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 4, by: 36, bb: 24, cut: 23 }, Desc::Through { j: 5, rt: 2, rt2: 31, back: false, off: 32760, bytes: 8, bb: 24, cut: 23 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Mark { j: 10, bb: 24, c: 15 }, Desc::Register { j: 13, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 14, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 15, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 16, by: -44, bb: 24, cut: 23 }, Desc::Probes { j: 17, c: 15, from: 23, off: 32760, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 21, by: -64, bb: 24, cut: 23 }], seq![0, 1, 2, 3, 4, 14, 15, 16, 17, 18, 19, 20, 21, 5, 6, 7, 8, 9, 10, 11, 12, 13], 22)) by (compute); }
proof fn pin_ranked_load_word_4096() { assert(ranked(seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 36, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 1, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 4, by: 36, bb: 24, cut: 23 }, Desc::Through { j: 5, rt: 2, rt2: 31, back: false, off: 4096, bytes: 4, bb: 24, cut: 23 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Mark { j: 10, bb: 24, c: 15 }, Desc::Register { j: 13, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 14, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 15, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 16, by: -44, bb: 24, cut: 23 }, Desc::Probes { j: 17, c: 15, from: 23, off: 4096, bytes: 4, store: false, bb: 24 }, Desc::Branch { j: 21, by: -64, bb: 24, cut: 23 }], seq![0, 1, 2, 3, 4, 14, 15, 16, 17, 18, 19, 20, 21, 5, 6, 7, 8, 9, 10, 11, 12, 13], 22)) by (compute); }
proof fn pin_ranked_load_byte_4095() { assert(ranked(seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 28, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 1, c: 23, bb: 24 }, Desc::Through { j: 3, rt: 2, rt2: 31, back: false, off: 4095, bytes: 1, bb: 24, cut: 23 }, Desc::Exit { j: 4, guest: 0x401000 }, Desc::Mark { j: 8, bb: 24, c: 15 }], seq![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 11)) by (compute); }
proof fn pin_ranked_load_4096_to_jump() { assert(ranked(seq![Desc::Register { j: 0, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 48, bb: 24, cut: 31 }, Desc::Cut { j: 3, n: 1, c: 23, bb: 24 }, Desc::Register { j: 4, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 5, by: 48, bb: 24, cut: 23 }, Desc::Through { j: 6, rt: 2, rt2: 31, back: false, off: 4096, bytes: 8, bb: 24, cut: 23 }, Desc::Register { j: 7, rd: 23, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 8, by: 72, bb: 24, cut: 31 }, Desc::Cut { j: 9, n: 30, c: 23, bb: 24 }, Desc::TableJump { j: 10, link: false }, Desc::Mark { j: 14, bb: 24, c: 15 }, Desc::Register { j: 17, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 18, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 19, by: 8, bb: 24, cut: 23 }, Desc::Branch { j: 20, by: -56, bb: 24, cut: 23 }, Desc::Probes { j: 21, c: 15, from: 23, off: 4096, bytes: 8, store: false, bb: 24 }, Desc::Branch { j: 25, by: -76, bb: 24, cut: 23 }, Desc::Branch { j: 26, by: 4, bb: 24, cut: 31 }, Desc::Call { j: 27, code: 7 }], seq![2, 3, 4, 5, 6, 7, 17, 18, 19, 20, 21, 22, 23, 24, 31, 32, 33, 8, 9, 10, 11, 12, 13, 14, 15, 16, 25, 26, 27, 28, 29, 30, 1, 0], 34)) by (compute); }
proof fn pin_ranked_stack_step() { assert(ranked(seq![Desc::StackStep { j: 0, k: -16, off: 0, bb: 24, cut: 31 }, Desc::Exit { j: 2, guest: 0x401000 }], seq![0, 1, 2, 3, 4, 5], 6)) by (compute); }
proof fn pin_ranked_stack_step_up() { assert(ranked(seq![Desc::StackStep { j: 0, k: 16, off: -4, bb: 24, cut: 31 }, Desc::Exit { j: 2, guest: 0x401000 }], seq![0, 1, 2, 3, 4, 5], 6)) by (compute); }
proof fn pin_ranked_stack_pair_post() { assert(ranked(seq![Desc::StackAccess { j: 0, rt: 29, rt2: 30, off: 0, moves: 16, bytes: 16, bb: 24 }, Desc::Exit { j: 1, guest: 0x401000 }], seq![0, 1, 2, 3, 4], 5)) by (compute); }
proof fn pin_ranked_stack_pair_pre() { assert(ranked(seq![Desc::StackAccess { j: 0, rt: 31, rt2: 31, off: -16, moves: -16, bytes: 16, bb: 24 }, Desc::Exit { j: 1, guest: 0x401000 }], seq![0, 1, 2, 3, 4], 5)) by (compute); }
proof fn pin_ranked_stack_cut() { assert(ranked(seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 24, bb: 24, cut: 31 }, Desc::StackCut { j: 3, n: 15, bb: 24, cut: 31 }, Desc::Exit { j: 4, guest: 0x401000 }, Desc::Mark { j: 8, bb: 24, c: 15 }], seq![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 11)) by (compute); }
proof fn pin_ranked_stack_structure() { assert(ranked(seq![Desc::Register { j: 0, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 24, bb: 24, cut: 31 }, Desc::StackAccess { j: 3, rt: 31, rt2: 31, off: 0, moves: 0, bytes: 64, bb: 24 }, Desc::Exit { j: 4, guest: 0x401000 }, Desc::Register { j: 8, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 9, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 10, by: 8, bb: 24, cut: 31 }, Desc::Stop { j: 11 }, Desc::Probes { j: 12, c: 23, from: 31, off: 0, bytes: 64, store: false, bb: 24 }, Desc::Branch { j: 16, by: -52, bb: 24, cut: 31 }], seq![0, 1, 2, 12, 13, 14, 15, 16, 3, 4, 5, 6, 7, 8, 9, 10, 11], 17)) by (compute); }
proof fn pin_ranked_stack_post_register() { assert(ranked(seq![Desc::Register { j: 0, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 64, bb: 24, cut: 31 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 4, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 5, by: 56, bb: 24, cut: 31 }, Desc::Register { j: 6, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 7, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 8, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 9, by: 80, bb: 24, cut: 31 }, Desc::Cut { j: 10, n: 15, c: 15, bb: 24 }, Desc::StackVia { j: 11, rt: 31, rt2: 31, bytes: 64, bb: 24, cut: 15 }, Desc::StackMove { j: 13, bb: 24, cut: 15 }, Desc::Exit { j: 14, guest: 0x401000 }, Desc::Branch { j: 18, by: 56, bb: 24, cut: 31 }, Desc::Register { j: 19, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 20, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 21, by: 8, bb: 24, cut: 31 }, Desc::Stop { j: 22 }, Desc::Probes { j: 23, c: 15, from: 31, off: 0, bytes: 64, store: true, bb: 24 }, Desc::Branch { j: 28, by: -88, bb: 24, cut: 31 }, Desc::Mark { j: 29, bb: 24, c: 15 }, Desc::Call { j: 32, code: 6 }], seq![2, 3, 4, 5, 6, 7, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 33, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 30, 31, 32, 34, 35, 36, 37, 38, 1, 0], 39)) by (compute); }
proof fn pin_ranked_stack_at() { assert(ranked(seq![Desc::Register { j: 0, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 8, bb: 24, cut: 31 }, Desc::Stop { j: 3 }, Desc::StackAt { j: 4, off: -208, rt: 31, rt2: 31, bytes: 4, bb: 24, cut: 31 }, Desc::Exit { j: 7, guest: 0x401000 }], seq![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 11)) by (compute); }
proof fn pin_ranked_stack_short_probes() { assert(ranked(seq![Desc::Register { j: 0, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 2, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 3, by: 24, bb: 24, cut: 31 }, Desc::StackAccess { j: 4, rt: 31, rt2: 31, off: 0, moves: 24, bytes: 24, bb: 24 }, Desc::Exit { j: 5, guest: 0x401000 }, Desc::Register { j: 9, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 10, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 11, by: 8, bb: 24, cut: 31 }, Desc::Stop { j: 12 }, Desc::StackProbes { j: 13, c: 23, bytes: 24, store: true, bb: 24 }, Desc::Branch { j: 17, by: -52, bb: 24, cut: 31 }], seq![0, 1, 2, 3, 13, 14, 15, 16, 17, 4, 5, 6, 7, 8, 9, 10, 11, 12], 18)) by (compute); }
proof fn pin_ranked_pair_through_cut() { assert(ranked(seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 36, bb: 24, cut: 31 }, Desc::Register { j: 2, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 3, by: 32, bb: 24, cut: 31 }, Desc::Cut { j: 4, n: 25, c: 23, bb: 24 }, Desc::Through { j: 5, rt: 8, rt2: 9, back: false, off: 0, bytes: 16, bb: 24, cut: 23 }, Desc::Exit { j: 6, guest: 0x401000 }, Desc::Branch { j: 10, by: 16, bb: 24, cut: 31 }, Desc::Mark { j: 11, bb: 24, c: 15 }, Desc::Call { j: 14, code: 6 }], seq![2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 15, 12, 13, 14, 16, 17, 18, 19, 20, 1, 0], 21)) by (compute); }
proof fn pin_ranked_cut_into_x29() { assert(ranked(seq![Desc::Frame { j: 0, f: FrameAccess { word: 0xf9004b9du32, rt: 31, off: 144, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::Frame { j: 1, f: FrameAccess { word: 0xf940438fu32, rt: 15, off: 128, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::Register { j: 2, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 3, by: 132, bb: 24, cut: 31 }, Desc::Register { j: 4, rd: 29, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 5, by: 128, bb: 24, cut: 31 }, Desc::Cut { j: 6, n: 5, c: 29, bb: 24 }, Desc::Frame { j: 7, f: FrameAccess { word: 0xf9403397u32, rt: 23, off: 96, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Register { j: 8, rd: 23, sp: false, bb: 24, cut: 29 }, Desc::Either { j: 9, by: 72, bb: 24, cut: 29 }, Desc::Frame { j: 10, f: FrameAccess { word: 0xf9403797u32, rt: 23, off: 104, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Register { j: 11, rd: 23, sp: false, bb: 24, cut: 29 }, Desc::Register { j: 12, rd: 23, sp: false, bb: 24, cut: 29 }, Desc::Either { j: 13, by: 56, bb: 24, cut: 29 }, Desc::Frame { j: 14, f: FrameAccess { word: 0xf9001f80u32, rt: 31, off: 56, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Register { j: 15, rd: 0, sp: false, bb: 24, cut: 29 }, Desc::Register { j: 16, rd: 0, sp: false, bb: 24, cut: 29 }, Desc::Frame { j: 17, f: FrameAccess { word: 0xf9403b97u32, rt: 23, off: 112, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Through { j: 18, rt: 23, rt2: 31, back: false, off: 0, bytes: 8, bb: 24, cut: 29 }, Desc::Frame { j: 19, f: FrameAccess { word: 0xf900339fu32, rt: 31, off: 96, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Frame { j: 20, f: FrameAccess { word: 0xf9403b8fu32, rt: 15, off: 112, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Register { j: 21, rd: 15, sp: false, bb: 24, cut: 29 }, Desc::Either { j: 22, by: 8, bb: 24, cut: 29 }, Desc::Branch { j: 23, by: 8, bb: 24, cut: 29 }, Desc::Register { j: 24, rd: 15, sp: false, bb: 24, cut: 29 }, Desc::Frame { j: 25, f: FrameAccess { word: 0xf9401f80u32, rt: 0, off: 56, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Branch { j: 26, by: 16, bb: 24, cut: 29 }, Desc::Through { j: 27, rt: 31, rt2: 31, back: false, off: 0, bytes: 8, bb: 24, cut: 29 }, Desc::Frame { j: 28, f: FrameAccess { word: 0xf900339fu32, rt: 31, off: 96, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Register { j: 29, rd: 15, sp: false, bb: 24, cut: 29 }, Desc::Frame { j: 30, f: FrameAccess { word: 0xf900438fu32, rt: 31, off: 128, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Frame { j: 31, f: FrameAccess { word: 0xf9404b9du32, rt: 29, off: 144, moves: false }, bb: 24, bb2: 24, cut: 29 }, Desc::Exit { j: 32, guest: 0x401000 }, Desc::Branch { j: 36, by: 16, bb: 24, cut: 31 }, Desc::Mark { j: 37, bb: 24, c: 29 }, Desc::Call { j: 40, code: 6 }], seq![2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 41, 38, 39, 40, 42, 43, 44, 45, 46, 1, 0], 47)) by (compute); }
proof fn pin_ranked_negative_probes() { assert(ranked(seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 40, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 7, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 4, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 5, by: 36, bb: 24, cut: 23 }, Desc::Through { j: 6, rt: 31, rt2: 31, back: false, off: -100, bytes: 8, bb: 24, cut: 23 }, Desc::Exit { j: 7, guest: 0x401000 }, Desc::Mark { j: 11, bb: 24, c: 15 }, Desc::Probes { j: 14, c: 15, from: 23, off: -100, bytes: 8, store: true, bb: 24 }, Desc::Branch { j: 19, by: -52, bb: 24, cut: 23 }], seq![0, 1, 2, 3, 4, 5, 12, 13, 14, 15, 16, 17, 18, 19, 6, 7, 8, 9, 10, 11], 20)) by (compute); }
proof fn pin_ranked_same_displaced() { assert(ranked(seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 52, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 20, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 4, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 5, by: 48, bb: 24, cut: 23 }, Desc::Register { j: 6, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::ShiftThrough { j: 7, k: 0, rt: 31, rt2: 31, bytes: 8, bb: 24, cut: 23 }, Desc::Register { j: 9, rd: 20, sp: false, bb: 24, cut: 31 }, Desc::Exit { j: 10, guest: 0x401000 }, Desc::Mark { j: 14, bb: 24, c: 15 }, Desc::Probes { j: 17, c: 15, from: 23, off: 0, bytes: 16, store: false, bb: 24 }, Desc::Branch { j: 21, by: -60, bb: 24, cut: 23 }], seq![0, 1, 2, 3, 4, 5, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 6, 7, 8, 9, 10], 22)) by (compute); }
proof fn pin_ranked_jump_displaced() { assert(ranked(seq![Desc::Register { j: 0, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 20, bb: 24, cut: 31 }, Desc::DisplacedTableJump { j: 3, n: 7, link: false }, Desc::Branch { j: 7, by: 4, bb: 24, cut: 31 }, Desc::Call { j: 8, code: 7 }], seq![2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 1, 0], 15)) by (compute); }
proof fn pin_ranked_stack_cut_displaced() { assert(ranked(seq![Desc::Register { j: 0, rd: 31, sp: true, bb: 24, cut: 31 }, Desc::Exit { j: 1, guest: 0x401000 }], seq![0, 1, 2, 3, 4], 5)) by (compute); }
proof fn pin_ranked_jump_frame() { assert(ranked(seq![Desc::Register { j: 0, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Frame { j: 1, f: FrameAccess { word: 0xf9400b97u32, rt: 23, off: 16, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::Register { j: 2, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 3, by: 28, bb: 24, cut: 31 }, Desc::Frame { j: 4, f: FrameAccess { word: 0xf9400b97u32, rt: 23, off: 16, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::Cut { j: 5, n: 23, c: 23, bb: 24 }, Desc::TableJump { j: 6, link: false }, Desc::Branch { j: 10, by: 4, bb: 24, cut: 31 }, Desc::Call { j: 11, code: 7 }], seq![2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 1, 0], 18)) by (compute); }
proof fn pin_ranked_call_link() { assert(ranked(seq![Desc::Register { j: 0, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 36, bb: 24, cut: 31 }, Desc::Cut { j: 3, n: 30, c: 23, bb: 24 }, Desc::TableJump { j: 4, link: true }, Desc::Branch { j: 11, by: 4, bb: 24, cut: 31 }, Desc::Call { j: 12, code: 7 }], seq![2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 1, 0], 19)) by (compute); }
proof fn pin_ranked_return_zero() { assert(ranked(seq![Desc::Register { j: 0, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 2, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 3, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 4, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 5, by: 8, bb: 24, cut: 31 }, Desc::Branch { j: 6, by: 8, bb: 24, cut: 31 }, Desc::Branch { j: 7, by: 32, bb: 24, cut: 31 }, Desc::Register { j: 8, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 9, by: 52, bb: 24, cut: 31 }, Desc::Cut { j: 10, n: 31, c: 23, bb: 24 }, Desc::TableJump { j: 11, link: false }, Desc::Call { j: 15, code: 8 }, Desc::Branch { j: 22, by: 4, bb: 24, cut: 31 }, Desc::Call { j: 23, code: 7 }], seq![4, 5, 6, 7, 8, 9, 10, 24, 11, 12, 13, 14, 15, 16, 17, 25, 26, 27, 28, 29, 3, 2, 18, 19, 20, 21, 22, 23, 1, 0], 30)) by (compute); }
proof fn pin_ranked_jump_zero_displaced() { assert(ranked(seq![Desc::Register { j: 0, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Register { j: 1, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 2, by: 20, bb: 24, cut: 31 }, Desc::DisplacedTableJump { j: 3, n: 31, link: false }, Desc::Branch { j: 7, by: 4, bb: 24, cut: 31 }, Desc::Call { j: 8, code: 7 }], seq![2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 1, 0], 15)) by (compute); }
proof fn pin_ranked_call_frame_displaced() { assert(ranked(seq![Desc::Register { j: 0, rd: 18, sp: false, bb: 24, cut: 31 }, Desc::Frame { j: 1, f: FrameAccess { word: 0xf9400b97u32, rt: 23, off: 16, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::Register { j: 2, rd: 23, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 3, by: 36, bb: 24, cut: 31 }, Desc::Frame { j: 4, f: FrameAccess { word: 0xf9400b97u32, rt: 23, off: 16, moves: false }, bb: 24, bb2: 24, cut: 31 }, Desc::DisplacedTableJump { j: 5, n: 23, link: true }, Desc::Branch { j: 12, by: 4, bb: 24, cut: 31 }, Desc::Call { j: 13, code: 7 }], seq![2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 1, 0], 20)) by (compute); }
proof fn pin_ranked_shift_through() { assert(ranked(seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 44, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 13, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 4, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 5, by: 40, bb: 24, cut: 23 }, Desc::ShiftThrough { j: 6, k: -312, rt: 31, rt2: 31, bytes: 8, bb: 24, cut: 23 }, Desc::Exit { j: 8, guest: 0x401000 }, Desc::Mark { j: 12, bb: 24, c: 15 }, Desc::Probes { j: 15, c: 15, from: 23, off: -312, bytes: 16, store: false, bb: 24 }, Desc::Branch { j: 19, by: -52, bb: 24, cut: 23 }], seq![0, 1, 2, 3, 4, 5, 11, 12, 13, 14, 15, 16, 17, 18, 19, 6, 7, 8, 9, 10], 20)) by (compute); }
proof fn pin_ranked_shift_through_displaced() { assert(ranked(seq![Desc::Register { j: 0, rd: 15, sp: false, bb: 24, cut: 31 }, Desc::Either { j: 1, by: 44, bb: 24, cut: 31 }, Desc::Cut { j: 2, n: 7, c: 23, bb: 24 }, Desc::Register { j: 3, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Register { j: 4, rd: 15, sp: false, bb: 24, cut: 23 }, Desc::Either { j: 5, by: 40, bb: 24, cut: 23 }, Desc::ShiftThrough { j: 6, k: -328, rt: 31, rt2: 31, bytes: 8, bb: 24, cut: 23 }, Desc::Exit { j: 8, guest: 0x401000 }, Desc::Mark { j: 12, bb: 24, c: 15 }, Desc::Probes { j: 15, c: 15, from: 23, off: -328, bytes: 16, store: false, bb: 24 }, Desc::Branch { j: 19, by: -52, bb: 24, cut: 23 }], seq![0, 1, 2, 3, 4, 5, 11, 12, 13, 14, 15, 16, 17, 18, 19, 6, 7, 8, 9, 10], 20)) by (compute); }

} // verus!
