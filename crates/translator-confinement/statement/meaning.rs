//! The hypotheses the proofs put on `step`, one shape per kind of word (the Arm ARM's meaning, written by hand: `step`
//! is free, see the crate's documentation), and the rule every proof here uses: an invariant that holds at entry and
//! that each step keeps, and under which each step is confined, makes the instructions confined.

use super::*;

verus! {

/// The step that writes `value` to `rd` and goes on at the next word, changing nothing else.
pub open spec fn wrote(m: Machine, rd: int, value: u64) -> Stepped {
    Stepped {
        after: Machine { x: m.x.update(rd, value), pc: (m.pc + 4) as u64, ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
}

/// `word` writes `value` of the machine to `rd` and nothing else (a data-processing word).
pub open spec fn writes(step: spec_fn(Machine, u32) -> Stepped, word: u32, rd: int, value: spec_fn(Machine) -> u64) -> bool {
    forall|m: Machine| m.x.len() == 31 ==> #[trigger] step(m, word) == wrote(m, rd, value(m))
}

/// `word` goes `by` bytes from itself when `x[rt]` is non-zero (`cbnz`, `nonzero`) or zero (`cbz`), else to the next
/// word; it changes nothing else.
pub open spec fn branches_if(step: spec_fn(Machine, u32) -> Stepped, word: u32, rt: int, by: int, nonzero: bool) -> bool {
    forall|m: Machine| m.x.len() == 31 ==> #[trigger] step(m, word) == Stepped {
        after: Machine {
            pc: if (m.x[rt] != 0) == nonzero { (m.pc + by) as u64 } else { (m.pc + 4) as u64 },
            ..m
        },
        accesses: Seq::empty(),
        system_call: false,
    }
}

/// `word` goes `by` bytes from itself (`b`).
pub open spec fn branches(step: spec_fn(Machine, u32) -> Stepped, word: u32, by: int) -> bool {
    forall|m: Machine| #[trigger] step(m, word) == Stepped {
        after: Machine { pc: (m.pc + by) as u64, ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
}

/// `word` goes to the address in `x[rn]` (`br`).
pub open spec fn jumps(step: spec_fn(Machine, u32) -> Stepped, word: u32, rn: int) -> bool {
    forall|m: Machine| #[trigger] step(m, word) == Stepped {
        after: Machine { pc: m.x[rn], ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
}

/// `word` raises a signal (`udf`): the run ends, nothing changed.
pub open spec fn stops(step: spec_fn(Machine, u32) -> Stepped, word: u32) -> bool {
    forall|m: Machine| #[trigger] step(m, word) == Stepped {
        after: Machine { ended: true, ..m },
        accesses: Seq::empty(),
        system_call: false,
    }
}

/// `word` touches `bytes` bytes from `address` of the machine (a load, a store, or both, as an atomic does). A fault
/// ends the run there; else it goes on at the next word. Either way it writes no register but `rt` (none for 31, the
/// zero register) and no unlisted one, and is no system call.
pub open spec fn accesses(step: spec_fn(Machine, u32) -> Stepped, word: u32, rt: int, address: spec_fn(Machine) -> int, bytes: nat) -> bool {
    forall|m: Machine| m.x.len() == 31 ==> {
        let s = #[trigger] step(m, word);
        &&& s.accesses == seq![(address(m), bytes)]
        &&& !s.system_call
        &&& s.after.unlisted == m.unlisted
        &&& s.after.x.len() == 31
        &&& forall|r: int| 0 <= r < 31 && r != rt ==> s.after.x[r] == m.x[r]
        &&& s.after.ended || s.after.pc == (m.pc + 4) as u64
    }
}

/// [`accesses`] by a word that also writes its base register `rb` back (pre- and post-index).
pub open spec fn accesses_writing_back(
    step: spec_fn(Machine, u32) -> Stepped,
    word: u32,
    rt: int,
    rb: int,
    address: spec_fn(Machine) -> int,
    bytes: nat,
) -> bool {
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

/// Each step from a machine where `inv` holds and the run goes on is confined, and leaves `inv` holding where the run
/// still goes on.
pub open spec fn kept_by_every_step(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, inv: spec_fn(Machine) -> bool) -> bool {
    forall|m: Machine| #[trigger] inv(m) && runs(a, m) ==> {
        let s = step(m, word_at(instructions, a, m.pc));
        &&& confined_step(a, m, s)
        &&& runs(a, s.after) ==> inv(s.after)
    }
}

proof fn lemma_invariant_holds(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    inv: spec_fn(Machine) -> bool,
    start: Machine,
    n: nat,
)
    requires
        forall|m: Machine| entered(a, m) ==> #[trigger] inv(m),
        kept_by_every_step(step, instructions, a, inv),
        entered(a, start),
    ensures
        runs(a, run(step, instructions, a, start, n)) ==> inv(run(step, instructions, a, start, n)),
    decreases n,
{
    if n > 0 {
        lemma_invariant_holds(step, instructions, a, inv, start, (n - 1) as nat);
        let before = run(step, instructions, a, start, (n - 1) as nat);
        if runs(a, before) {
            assert(inv(before));
        }
    } else {
        assert(inv(start));
    }
}

/// THE RULE: an invariant that every entered machine meets and every step keeps makes the instructions confined.
pub proof fn lemma_invariant_confines(
    step: spec_fn(Machine, u32) -> Stepped,
    instructions: Seq<u32>,
    a: Allowed,
    inv: spec_fn(Machine) -> bool,
)
    requires
        a.length == instructions.len() * 4,
        forall|m: Machine| entered(a, m) ==> #[trigger] inv(m),
        kept_by_every_step(step, instructions, a, inv),
    ensures
        is_confined(step, instructions, a),
{
    assert forall|start: Machine, n: nat|
        #![trigger run(step, instructions, a, start, n)]
        entered(a, start) && runs(a, run(step, instructions, a, start, n)) implies {
            let now = run(step, instructions, a, start, n);
            confined_step(a, now, step(now, word_at(instructions, a, now.pc)))
        } by {
        lemma_invariant_holds(step, instructions, a, inv, start, n);
        let now = run(step, instructions, a, start, n);
        assert(inv(now));
    }
}

} // verus!
