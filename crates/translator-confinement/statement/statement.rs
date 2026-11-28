//! The second proof statement of the native in-process form: a run of the translator's instructions stays inside its
//! own memory and makes no system call.
//!
//! What one instruction does is `step`, a FREE parameter of [`is_confined`]: nothing in the statement comes from the
//! instruction-proof model. Each proof states, as a hypothesis on `step`, what the words of its piece do (the Arm ARM's
//! meaning, written out by hand), so a proof here holds for every `step` meeting those hypotheses, the model's one
//! included only once it is shown to meet them. Binding `step` to the model waits on its decode tables
//! (`to_branch_operation`, `to_integer_operation`, `to_load_store_operation`) moving inside `verus!`: all three are
//! outside it at instruction-proof `ad936546b` (its tip when fetched on 2026-11-27, fifth run), where only the branch
//! family's meaning (`branch_outcome`) is inside; that track's PROOF.md tests the tables against Arm's decode rather
//! than proving them.
//!
//! [`compose::described_is_confined`] is the general theorem: a piece described by blocks that each hold alone is
//! confined, whatever its shape; the other modules prove whole pieces one shape at a time.
//! [`compose::described_is_chained`] adds that every exit of such a piece hands on what the next piece's entry needs
//! ([`is_chained`]), and [`compose::whole_run_is_confined`] proves, by induction over the steps, every step of a
//! [`whole_run`] of such pieces confined; what it still takes on trust is listed there.

#![no_std]

use vstd::prelude::*;

verus! {

/// A window's bytes, then as many never mapped: what a slot reserves from its base.
pub const WINDOW_BYTES: u64 = 0x1_0000_0000;
pub const SLOT_BYTES: u64 = 0x2_0000_0000;
/// The slot tables start here; a slot's table is its base's host addresses shifted by two.
pub const PROCESS_SLOTS: u64 = 0x4000_0000_0000;
pub const SLOT_TABLE_BYTES: u64 = 0x8_0000_0000;
/// The floor's page of exits, each 16 bytes.
pub const FLOOR_PAGE: u64 = 0x2001_0000_0000;
pub const FLOOR_ENTRY_BYTES: u64 = 16;
/// The frame's words the instructions own, and the registers that hold the frame and the window's base.
pub const FRAME_BYTES: u64 = 256;
pub open spec fn frame_register() -> int {
    28
}

pub open spec fn base_register() -> int {
    24
}

/// What a step needs of the processor (the model's `Processor` has these, but nothing here is tied to it): x0 to
/// x30, the stack pointer, the program counter, the registers user space can write that `run_code` does not list
/// (PSTATE's SSBS, DIT and TCO; SVE's predicates and FFR), and whether an exception ended the run.
pub struct Machine {
    pub x: Seq<u64>,
    pub sp: u64,
    pub pc: u64,
    pub unlisted: (u64, Seq<u16>, u16),
    pub ended: bool,
}

/// One step: the machine after it, the bytes it touched (first byte, length), and whether it was a system call.
pub struct Stepped {
    pub after: Machine,
    pub accesses: Seq<(int, nat)>,
    pub system_call: bool,
}

/// Where a run is: its window's base, how far below the base its slot's reservation starts (the slot reserves
/// `SLOT_BYTES` from there), its frame, its instructions' first byte and length, and whether its stack pointer is the
/// guest's own (displaced).
pub struct Allowed {
    pub base: int,
    pub below: int,
    pub frame: int,
    pub code: int,
    pub length: nat,
    pub displaced: bool,
}

/// Every byte of the access is in its slot's reservation, its frame's words or its own instructions.
pub open spec fn touches_allowed(a: Allowed, first: int, bytes: nat) -> bool {
    ||| a.base - a.below <= first && first + bytes <= a.base - a.below + SLOT_BYTES as int
    ||| a.frame <= first && first + bytes <= a.frame + FRAME_BYTES as int
    ||| a.code <= first && first + bytes <= a.code + a.length
}

/// An instruction of the sequence.
pub open spec fn in_sequence(a: Allowed, pc: u64) -> bool {
    a.code <= pc as int && (pc as int) < a.code + a.length && (pc as int - a.code) % 4 == 0
}

/// Where its slot table starts: its window's, or slot 0's for displaced code.
pub open spec fn slot_table(a: Allowed) -> int {
    if a.displaced { PROCESS_SLOTS as int } else { PROCESS_SLOTS as int + a.base * 4 }
}

/// An exit: an entry of the floor, which ends the run, or its slot table, held inaccessible.
pub open spec fn is_exit(a: Allowed, pc: u64) -> bool {
    ||| FLOOR_PAGE <= pc && (pc as int) < FLOOR_PAGE as int + 4096 && (pc as int - FLOOR_PAGE as int) % (FLOOR_ENTRY_BYTES as int) == 0
    ||| slot_table(a) <= pc as int && (pc as int) < slot_table(a) + SLOT_TABLE_BYTES as int
}

/// THE ENTRY RULE (measured smallest, docs C2): the run itself sets x28 to the frame and x24 to the window's base, the
/// stack pointer is in the window unless the code is displaced, and the run enters at the first instruction.
pub open spec fn entered(a: Allowed, m: Machine) -> bool {
    &&& m.x.len() == 31
    &&& m.x[frame_register()] as int == a.frame
    &&& m.x[base_register()] as int == a.base
    &&& (a.displaced || (a.base <= m.sp as int && (m.sp as int) < a.base + WINDOW_BYTES as int))
    &&& m.pc as int == a.code
    &&& !m.ended
}

pub open spec fn word_at(instructions: Seq<u32>, a: Allowed, pc: u64) -> u32 {
    instructions[(pc as int - a.code) / 4]
}

/// The run goes on from here: nothing ended it and it is at an instruction of the sequence.
pub open spec fn runs(a: Allowed, m: Machine) -> bool {
    !m.ended && in_sequence(a, m.pc)
}

/// The machine after `n` steps, standing where the run ended.
pub open spec fn run(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed, m: Machine, n: nat) -> Machine
    decreases n,
{
    if n == 0 {
        m
    } else {
        let before = run(step, instructions, a, m, (n - 1) as nat);
        if runs(a, before) {
            step(before, word_at(instructions, a, before.pc)).after
        } else {
            before
        }
    }
}

/// One step stays confined: its bytes are allowed, it is no system call, it changes no unlisted register, x28 keeps the
/// frame, and the run ends or goes on at an instruction of the sequence or at an exit.
pub open spec fn confined_step(a: Allowed, before: Machine, s: Stepped) -> bool {
    &&& forall|k: int| 0 <= k < s.accesses.len() ==> touches_allowed(a, #[trigger] s.accesses[k].0, s.accesses[k].1)
    &&& !s.system_call
    &&& s.after.unlisted == before.unlisted
    &&& s.after.x.len() == 31 && s.after.x[frame_register()] as int == a.frame
    &&& (s.after.ended || in_sequence(a, s.after.pc) || is_exit(a, s.after.pc))
}

/// The statement: from every machine the entry rule allows and after every number of steps, the next step stays
/// confined.
pub open spec fn is_confined(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed) -> bool {
    &&& a.length == instructions.len() * 4
    &&& forall|start: Machine, n: nat|
        #![trigger run(step, instructions, a, start, n)]
        entered(a, start) && runs(a, run(step, instructions, a, start, n)) ==> {
        let now = run(step, instructions, a, start, n);
        confined_step(a, now, step(now, word_at(instructions, a, now.pc)))
    }
}

/// How far outside the window a run may leave the stack pointer for the next piece: the most an access through it
/// moves it after touching it (`ldp qT, qT2, [sp], #1008`; the slot's never mapped bytes stop anything further).
pub const STACK_SLACK: u64 = 1024;

pub open spec fn stack_ok(a: Allowed, sp: u64) -> bool {
    a.displaced || (a.base - STACK_SLACK as int <= sp as int && sp as int <= a.base + WINDOW_BYTES as int + STACK_SLACK as int)
}

/// What a run hands to the piece it goes on to: the frame in x28, the window's base in x24, the stack pointer within
/// the slack of the window unless displaced.
pub open spec fn hands_on(a: Allowed, m: Machine) -> bool {
    &&& m.x.len() == 31
    &&& m.x[frame_register()] as int == a.frame
    &&& m.x[base_register()] as int == a.base
    &&& stack_ok(a, m.sp)
}

/// THE CHAIN'S ENTRY RULE: what a run hands on, at the first instruction ([`entered`] is one of these machines).
pub open spec fn handed(a: Allowed, m: Machine) -> bool {
    hands_on(a, m) && m.pc as int == a.code && !m.ended
}

/// The slot table: where an exit goes to the piece of a guest address.
pub open spec fn in_table(a: Allowed, pc: u64) -> bool {
    slot_table(a) <= pc as int && (pc as int) < slot_table(a) + SLOT_TABLE_BYTES as int
}

/// THE CHAIN: from every machine a run hands on and after every number of steps, the next step stays confined, and
/// a step into the slot table hands on.
pub open spec fn is_chained(step: spec_fn(Machine, u32) -> Stepped, instructions: Seq<u32>, a: Allowed) -> bool {
    &&& a.length == instructions.len() * 4
    &&& forall|start: Machine, n: nat|
        #![trigger run(step, instructions, a, start, n)]
        handed(a, start) && runs(a, run(step, instructions, a, start, n)) ==> {
        let now = run(step, instructions, a, start, n);
        let s = step(now, word_at(instructions, a, now.pc));
        &&& confined_step(a, now, s)
        &&& !s.after.ended && in_table(a, s.after.pc) ==> hands_on(a, s.after)
    }
}

/// A piece of a program: its instructions and where they run.
pub struct Piece {
    pub instructions: Seq<u32>,
    pub a: Allowed,
}

/// The pieces share one window, its reservation, frame and way, and lie below the slot tables.
pub open spec fn one_window(pieces: Seq<Piece>, a: Allowed) -> bool {
    forall|i: int| 0 <= i < pieces.len() ==> {
        let b = #[trigger] pieces[i].a;
        &&& b.base == a.base && b.below == a.below && b.frame == a.frame && b.displaced == a.displaced
        &&& 0 <= b.code <= PROCESS_SLOTS
    }
}

/// A WHOLE RUN of a program, piece `k` the one it is in: each step is its piece's, and a step into the slot table goes
/// on at the first instruction of piece `link(pc)` with every register as the step left it. That is what the slot
/// table's words do once exit linking wrote them: ASSUMED here, not proven (they are not the translator's pieces).
pub open spec fn whole_run(
    step: spec_fn(Machine, u32) -> Stepped,
    pieces: Seq<Piece>,
    link: spec_fn(u64) -> int,
    k: int,
    m: Machine,
    n: nat,
) -> (int, Machine)
    decreases n,
{
    if n == 0 {
        (k, m)
    } else {
        let before = whole_run(step, pieces, link, k, m, (n - 1) as nat);
        let p = pieces[before.0];
        if runs(p.a, before.1) {
            let after = step(before.1, word_at(p.instructions, p.a, before.1.pc)).after;
            if !after.ended && in_table(p.a, after.pc) {
                (link(after.pc), Machine { pc: pieces[link(after.pc)].a.code as u64, ..after })
            } else {
                (before.0, after)
            }
        } else {
            before
        }
    }
}

} // verus!

#[cfg(verus_keep_ghost)]
mod meaning;
#[cfg(verus_keep_ghost)]
mod copy;
#[cfg(verus_keep_ghost)]
mod access;
#[cfg(verus_keep_ghost)]
mod indexed;
#[cfg(verus_keep_ghost)]
mod stack;
#[cfg(verus_keep_ghost)]
mod branch;
#[cfg(verus_keep_ghost)]
mod displaced;
#[cfg(verus_keep_ghost)]
mod offset;
#[cfg(verus_keep_ghost)]
mod withheld;
#[cfg(verus_keep_ghost)]
mod rest;
#[cfg(verus_keep_ghost)]
mod withheld_access;
#[cfg(verus_keep_ghost)]
mod look_ahead;
#[cfg(verus_keep_ghost)]
mod compose;
#[cfg(verus_keep_ghost)]
mod plants;
