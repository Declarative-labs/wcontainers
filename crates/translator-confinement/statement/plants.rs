//! Wrong translators the statement refuses, one for each clause of [`confined_step`]: each output is one wrong word and
//! `udf`, under a semantics of those words taken from the Arm ARM. With the clause dropped or weakened (`plants.txt`),
//! its refusal no longer verifies: the clause carries weight.

use super::*;

verus! {

/// `svc #0`, `ldr x0, [x1]`, `ldur x0, [x28, #252]`, `mov x28, x0`, `msr dit, #1`, `b .+8`, `udf #0`.
pub const SVC: u32 = 0xd400_0001;
pub const LOAD_THROUGH_X1: u32 = 0xf940_0020;
pub const LOAD_ACROSS_FRAME_END: u32 = 0xf84f_c380;
pub const WRITE_X28: u32 = 0xaa00_03fc;
pub const SET_DIT: u32 = 0xd503_415f;
pub const BRANCH_PAST: u32 = 0x1400_0002;
pub const UDF: u32 = 0;

/// What each of those words does (Arm ARM): the system call, two loads, a move, a PSTATE write, a branch; any other word
/// raises a signal.
pub open spec fn semantics(m: Machine, word: u32) -> Stepped {
    let next = Machine { pc: (m.pc + 4) as u64, ..m };
    if word == SVC {
        Stepped { after: next, accesses: Seq::empty(), system_call: true }
    } else if word == LOAD_THROUGH_X1 {
        Stepped { after: next, accesses: seq![(m.x[1] as int, 8nat)], system_call: false }
    } else if word == LOAD_ACROSS_FRAME_END {
        Stepped { after: next, accesses: seq![(m.x[frame_register()] as int + 252, 8nat)], system_call: false }
    } else if word == WRITE_X28 {
        Stepped { after: Machine { x: m.x.update(frame_register(), m.x[0]), ..next }, accesses: Seq::empty(), system_call: false }
    } else if word == SET_DIT {
        Stepped {
            after: Machine { unlisted: (if m.unlisted.0 == 0 { 1u64 } else { 0u64 }, m.unlisted.1, m.unlisted.2), ..next },
            accesses: Seq::empty(),
            system_call: false,
        }
    } else if word == BRANCH_PAST {
        Stepped { after: Machine { pc: (m.pc + 8) as u64, ..m }, accesses: Seq::empty(), system_call: false }
    } else {
        Stepped { after: Machine { ended: true, ..m }, accesses: Seq::empty(), system_call: false }
    }
}

/// [`semantics`] as the step [`is_confined`] takes.
pub open spec fn toy() -> spec_fn(Machine, u32) -> Stepped {
    |m: Machine, w: u32| semantics(m, w)
}

/// Slot 3's window, a frame far from it, two instructions in the code area.
pub open spec fn place() -> Allowed {
    Allowed {
        base: 0x6_0000_0000int,
        below: 0int,
        frame: 0x7fff_0000_1000int,
        code: 0x2004_0000_0000int,
        length: 8,
        displaced: false,
    }
}

/// A machine the entry rule allows: x28 the frame, x24 the base, the rest zero.
pub open spec fn start() -> Machine {
    Machine {
        x: Seq::new(31, |i: int| if i == frame_register() { 0x7fff_0000_1000u64 } else if i == base_register() { 0x6_0000_0000u64 } else { 0u64 }),
        sp: 0x6_0000_0000,
        pc: 0x2004_0000_0000,
        unlisted: (0, Seq::empty(), 0),
        ended: false,
    }
}

/// The first step of `[word, udf]` from [`start`] is not confined, so neither is the output.
proof fn refused_at_first_step(word: u32)
    requires
        !confined_step(place(), start(), semantics(start(), word)),
    ensures
        !is_confined(toy(), seq![word, UDF], place()),
{
    let step = toy();
    let instructions = seq![word, UDF];
    assert(step(start(), word) == semantics(start(), word));
    assert(entered(place(), start()));
    assert(run(step, instructions, place(), start(), 0) == start());
    assert(word_at(instructions, place(), start().pc) == word);
    assert(runs(place(), run(step, instructions, place(), start(), 0)));
}

proof fn a_system_call_is_refused()
    ensures
        !is_confined(toy(), seq![SVC, UDF], place()),
{
    refused_at_first_step(SVC);
}

proof fn a_load_through_an_unreserved_register_is_refused()
    ensures
        !is_confined(toy(), seq![LOAD_THROUGH_X1, UDF], place()),
{
    assert(start().x[1] == 0);
    assert(semantics(start(), LOAD_THROUGH_X1).accesses.len() == 1);
    assert(semantics(start(), LOAD_THROUGH_X1).accesses[0] == (0int, 8nat));
    assert(!touches_allowed(place(), 0, 8));
    let s = semantics(start(), LOAD_THROUGH_X1);
    assert(!touches_allowed(place(), s.accesses[0].0, s.accesses[0].1));
    refused_at_first_step(LOAD_THROUGH_X1);
}

proof fn a_load_across_the_frame_s_end_is_refused()
    ensures
        !is_confined(toy(), seq![LOAD_ACROSS_FRAME_END, UDF], place()),
{
    assert(start().x[frame_register()] == 0x7fff_0000_1000);
    assert(semantics(start(), LOAD_ACROSS_FRAME_END).accesses.len() == 1);
    assert(semantics(start(), LOAD_ACROSS_FRAME_END).accesses[0] == (0x7fff_0000_1000int + 252, 8nat));
    assert(!touches_allowed(place(), 0x7fff_0000_1000int + 252, 8));
    let s = semantics(start(), LOAD_ACROSS_FRAME_END);
    assert(!touches_allowed(place(), s.accesses[0].0, s.accesses[0].1));
    refused_at_first_step(LOAD_ACROSS_FRAME_END);
}

proof fn a_write_of_the_frame_register_is_refused()
    ensures
        !is_confined(toy(), seq![WRITE_X28, UDF], place()),
{
    assert(semantics(start(), WRITE_X28).after.x[frame_register()] == 0);
    refused_at_first_step(WRITE_X28);
}

proof fn a_write_of_an_unlisted_register_is_refused()
    ensures
        !is_confined(toy(), seq![SET_DIT, UDF], place()),
{
    assert(semantics(start(), SET_DIT).after.unlisted.0 == 1);
    refused_at_first_step(SET_DIT);
}

proof fn a_branch_out_of_the_sequence_is_refused()
    ensures
        !is_confined(toy(), seq![BRANCH_PAST, UDF], place()),
{
    refused_at_first_step(BRANCH_PAST);
}

} // verus!
