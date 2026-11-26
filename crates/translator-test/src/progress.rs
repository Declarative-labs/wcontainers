//! The progress count: where a program is by it at its system calls and stops, and how a position is
//! reached again from any look-ahead.

use crate::guest::GuestTest;
use crate::programs::{Blob, Ran, ran, ready_with};
use crate::runner::{Machine, Pause, Paused, probe};
use std::cell::RefCell;
use std::rc::Rc;
use wcontainers_translator::{LookAhead, Optimise};

/// Where a thread is: its progress count and its pc.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub count: u64,
    pub pc: u64,
}

impl Position {
    fn of<G: GuestTest>(paused: &mut Paused<'_, '_, G>) -> Self {
        Position {
            count: paused.progress_count(),
            pc: paused.pc(),
        }
    }

    /// Whether this position comes before `other` in the run that reaches both.
    ///
    /// Between two events the pc only grows within the guest's own control flow, all the stand-ins give:
    /// they deliver no signals, whose delivery and return move it down with no event.
    pub fn before(self, other: Position) -> bool {
        self < other
    }
}

/// A machine with `blob` translated as `how` says and keeping the progress count; at look-ahead none, code
/// reached at run time is made at none too.
pub fn counting<G: GuestTest>(
    blob: &Blob,
    (look_ahead, optimise): (LookAhead, Optimise),
) -> Machine<G> {
    ready_with::<G>(blob, probe::<G>(), |machine| {
        machine.keep_progress_count();
        machine.optimise_for(optimise);
        machine.load(
            &blob.bytes,
            0..blob.code_bytes,
            (blob.load, blob.link_address),
            &[0],
            look_ahead,
        );
        if look_ahead == LookAhead::None {
            machine.translate_at_run_time(|| LookAhead::None);
        }
    })
}

/// What a run keeping the progress count did: the program's results, the position after each system call
/// but the last, and the count at its end.
#[derive(Debug, PartialEq)]
pub struct Counted {
    pub ran: Ran,
    pub syscalls: Vec<Position>,
    pub end: u64,
}

/// Run `machine` to its end, giving the program's results and the count at the end.
pub fn finish<G: GuestTest>(blob: &Blob, machine: &mut Machine<G>) -> (Ran, u64) {
    let (stopped, registers) = machine.run();
    (
        ran(blob, machine, stopped, &registers),
        machine.progress_count(),
    )
}

/// Run `blob` as `how` says, keeping the progress count.
pub fn run_counting<G: GuestTest>(blob: &Blob, how: (LookAhead, Optimise)) -> Counted {
    let mut machine = counting::<G>(blob, how);
    let log = Rc::new(RefCell::new(Vec::new()));
    let kept = log.clone();
    machine.on_pause(move |pause, paused| {
        if pause == Pause::Syscall {
            kept.borrow_mut().push(Position::of(paused));
        }
    });
    let (ran, end) = finish(blob, &mut machine);
    Counted {
        ran,
        syscalls: log.take(),
        end,
    }
}

/// Every position where the thread stops when it stops at every stop check, in order, and the program's
/// results.
pub fn every_check<G: GuestTest>(blob: &Blob, how: (LookAhead, Optimise)) -> (Vec<Position>, Ran) {
    let mut machine = counting::<G>(blob, how);
    machine.stop_at_every_check();
    let log = Rc::new(RefCell::new(Vec::new()));
    let kept = log.clone();
    machine.on_pause(move |pause, paused| {
        if pause == Pause::Stop {
            kept.borrow_mut().push(Position::of(paused));
        }
    });
    let (ran, _) = finish(blob, &mut machine);
    (log.take(), ran)
}

/// The first stop at a check where the count has reached `limit`, none when the program ends first, and the
/// program's results.
pub fn first_stop<G: GuestTest>(
    blob: &Blob,
    how: (LookAhead, Optimise),
    limit: u64,
) -> (Option<Position>, Ran) {
    let mut machine = counting::<G>(blob, how);
    machine.set_progress_limit(limit);
    let stop = Rc::new(RefCell::new(None));
    let kept = stop.clone();
    machine.on_pause(move |pause, paused| {
        if pause == Pause::Stop && kept.borrow().is_none() {
            *kept.borrow_mut() = Some(Position::of(paused));
            paused.set_progress_limit(u64::MAX);
        }
    });
    let (ran, _) = finish(blob, &mut machine);
    let stop = *stop.borrow();
    (stop, ran)
}

/// How a run looking for a position went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Approach {
    /// No stop yet.
    Waiting,
    /// Stopped before the position, and carrying on at look-ahead none.
    AtNone,
    Reached,
    /// The first stop came after the position.
    Late,
}

/// Reach `target` exactly, starting at look-ahead `how`: stop at that look-ahead at or before it (with a limit
/// lowered, from the target's count, until the first stop is not past it and comes before the end), then carry on at look-ahead none,
/// which checks at every instruction, until the count and pc are the target's. Gives the program's results
/// and how many runs it took. Panics if the run at none passes the target.
pub fn reach<G: GuestTest>(
    blob: &Blob,
    how: (LookAhead, Optimise),
    target: Position,
) -> (Ran, u32) {
    let mut back = 0;
    for runs in 1.. {
        let mut machine = counting::<G>(blob, how);
        machine.set_progress_limit(target.count.saturating_sub(back));
        let approach = Rc::new(RefCell::new(Approach::Waiting));
        let kept = approach.clone();
        machine.on_pause(move |pause, paused| {
            if pause != Pause::Stop {
                return;
            }
            let here = Position::of(paused);
            let mut approach = kept.borrow_mut();
            match *approach {
                Approach::Waiting if here == target => *approach = Approach::Reached,
                Approach::Waiting if here.before(target) => {
                    *approach = Approach::AtNone;
                    paused.set_progress_limit(target.count);
                    paused.carry_on_at_none();
                    return;
                }
                Approach::Waiting => *approach = Approach::Late,
                Approach::AtNone if here == target => *approach = Approach::Reached,
                Approach::AtNone if here.before(target) => return,
                Approach::AtNone => panic!("passed {target:?} at none: {here:?}"),
                Approach::Reached | Approach::Late => return,
            }
            paused.set_progress_limit(u64::MAX);
        });
        let (ran, _) = finish(blob, &mut machine);
        match *approach.borrow() {
            Approach::Reached => return (ran, runs),
            // No stop at or before the target, or none at all: stop earlier.
            Approach::Late | Approach::Waiting if back < target.count => back = (back * 2).max(1),
            outcome => panic!("{target:?} not reached from {how:?}: {outcome:?}"),
        }
    }
    unreachable!("runs never end")
}
