//! The second proof statement of the native in-process form: a run of the translator's instructions stays inside its
//! own memory and makes no system call. Stated over the model's processor (`Processor`, seen through
//! [`machine_of`]); one step is given as `step`, the model's semantics of one word, which the model does not yet give
//! as one specification for every word (its decode tables are outside `verus!`).

#![no_std]

use vstd::prelude::*;
use wcontainers_processor_architecture_arm64::Processor;

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

/// What a step needs of the processor: x0 to x30, the stack pointer, the program counter, the registers user space can
/// write that `run_code` does not list (PSTATE's SSBS, DIT and TCO; SVE's predicates and FFR), and whether an exception
/// ended the run.
pub struct Machine {
    pub x: Seq<u64>,
    pub sp: u64,
    pub pc: u64,
    pub unlisted: (u64, Seq<u16>, u16),
    pub ended: bool,
}

/// The model's processor as a [`Machine`].
pub open spec fn machine_of<L>(p: Processor<L>) -> Machine {
    Machine {
        x: p.general_purpose_registers@,
        sp: p.stack_pointer,
        pc: p.program_counter,
        unlisted: (p.process_state_controls, p.predicate_registers@, p.first_fault_register),
        ended: p.exception is Some,
    }
}

/// One step: the machine after it, the bytes it touched (first byte, length), and whether it was a system call.
pub struct Stepped {
    pub after: Machine,
    pub accesses: Seq<(int, nat)>,
    pub system_call: bool,
}

/// Where a run is: its window's base, the never mapped bytes below the window that are reserved too, its frame, its
/// instructions' first byte and length, and whether its stack pointer is the guest's own (displaced).
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
    ||| a.base - a.below <= first && first + bytes <= a.base + SLOT_BYTES as int
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

} // verus!

#[cfg(verus_keep_ghost)]
mod plants;
