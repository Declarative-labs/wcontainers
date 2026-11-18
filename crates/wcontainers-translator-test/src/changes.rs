//! Page permissions changed at random points where the thread pauses: at stop checks and after system calls.
//! A run at full look-ahead makes the changes and records where; a run at look-ahead none, stopping at every
//! instruction, makes the same changes at the same points, so the two must end the same: the same fault at
//! the same address, the same registers and the same memory. A fault is handled as a program's own handler
//! would (Java's safepoints do this): the page is made to allow reading and writing and the thread carries on
//! from the faulting instruction, so one run meets many faults, and each is compared.
//!
//! A system call point is found again by its count. A stop point is found again by how often the thread
//! had come back before it (a handled fault returns to the same registers and memory) and its registers and
//! memory there: between two comebacks a program that ends never returns to the same state, so the first
//! stop of none in that state is the point.

use crate::guest::{GuestTest, State};
use crate::programs::{Blob, Ran, STACK_TOP, placement, ran, ready, ready_for};
use crate::runner::{Machine, Pause, Paused, Stopped, fields};
use proptest::prelude::RngExt;
use proptest::test_runner::{RngAlgorithm, TestRng};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hasher};
use std::ops::Range;
use std::rc::Rc;
use wcontainers_abi::{Exit, PAGE_SIZE, Permissions};
use wcontainers_translator::contract::Reason;
use wcontainers_translator::{LookAhead, Omit, Optimise};
use zerocopy::IntoBytes;

/// How a run with changes is made.
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    /// Seeds every choice of where and what to change.
    pub seed: u64,
    /// One stop in this many makes a change; every system call does (they are far fewer).
    pub every: u32,
    /// The chance that the run at full raises its stop flag again at each pause, so it stops at the next
    /// stop check (the flag starts raised when this is above 0): 1 stops at every check, 0 pauses only
    /// after system calls, and between the two some system calls are followed by a stop and some are not.
    pub stops: f64,
}

#[derive(Clone, Debug)]
enum Point {
    /// A stop after the thread had come back `came_back` times, in this state (its block's bytes).
    Stop {
        came_back: usize,
        registers: Vec<u8>,
        memory: u64,
    },
    /// After the system call of this number, counting from 1.
    Syscall(u64),
}

#[derive(Clone, Debug)]
struct Change {
    point: Point,
    /// Pages and what each allows from then on.
    pages: Vec<(u64, Permissions)>,
}

/// Faults handled in one run; the next one ends it.
const HANDLED: usize = 64;

/// How one run went, for comparing: each time the thread came back, and the output.
struct Ended {
    stops: Vec<(Stopped, State)>,
    ran: Ran,
}

/// What the two runs did, for the caller to count.
pub struct Compared {
    pub changes: usize,
    /// Faults handled and carried on from.
    pub handled: usize,
    pub stopped: Stopped,
    /// At full: pieces translated at run time, and instructions run alone at none after a check failed.
    pub translated: u64,
    pub left: u64,
}

/// Pages a change may touch: the blob's pages and the page after it, the stack and the page below it.
fn universe(blob: &Blob) -> Vec<Range<u64>> {
    placement(blob)
        .regions
        .into_iter()
        .enumerate()
        .map(|(i, (range, _))| {
            let pages = range.start / PAGE_SIZE..range.end / PAGE_SIZE;
            if i == 0 {
                pages.start..pages.end + 1
            } else {
                pages.start - 1..pages.end
            }
        })
        .collect()
}

/// A hash of every byte of the pages a change may touch.
fn memory<G: GuestTest>(paused: &mut Paused<'_, '_, G>, universe: &[Range<u64>]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for pages in universe {
        let mut bytes = vec![0; ((pages.end - pages.start) * PAGE_SIZE) as usize];
        paused.read(pages.start * PAGE_SIZE, &mut bytes);
        hasher.write(&bytes);
    }
    hasher.finish()
}

/// The pages of running functions' frames: from the stack pointer's page to the top of the stack. The plan
/// lets a frame's check hold until the next function entry, so changes leave these alone.
fn frames<G: GuestTest>(state: &State) -> Range<u64> {
    sp::<G>(state) / PAGE_SIZE..STACK_TOP / PAGE_SIZE
}

fn sp<G: GuestTest>(state: &State) -> u64 {
    state.0[G::SP_FIELD] as u64
}

/// Up to three pages and what each will allow: often near where a register points (a loop's base, the page
/// it is on, the one after), otherwise anywhere a change may touch.
fn choose<G: GuestTest>(
    rng: &mut TestRng,
    state: &State,
    universe: &[Range<u64>],
) -> Vec<(u64, Permissions)> {
    const ALLOWS: [Permissions; 7] = [
        Permissions::empty(),
        Permissions::empty(),
        Permissions::READ,
        Permissions::WRITE,
        Permissions::READ.union(Permissions::WRITE),
        Permissions::READ.union(Permissions::WRITE),
        Permissions::all(),
    ];
    let frames = frames::<G>(state);
    let within = |page: &u64| universe.iter().any(|pages| pages.contains(page));
    // Registers that point where a change may touch, as a base does, and the page below sp's.
    let pointers: Vec<u64> = state.0[G::GENERAL_FIELDS]
        .iter()
        .map(|&value| value as u64 / PAGE_SIZE)
        .chain([(sp::<G>(state) / PAGE_SIZE).wrapping_sub(1)])
        .filter(|page| within(page) && !frames.contains(page))
        .collect();
    let mut pages = Vec::new();
    for _ in 0..rng.random_range(1..=3) {
        let page = if !pointers.is_empty() && rng.random_bool(0.6) {
            // Its own page half the time, otherwise the one before or one of the two after.
            const NEAR: [i64; 6] = [0, 0, 0, -1, 1, 2];
            let page = pointers[rng.random_range(0..pointers.len())];
            page.wrapping_add_signed(NEAR[rng.random_range(0..NEAR.len())])
        } else {
            let pages = &universe[rng.random_range(0..universe.len())];
            rng.random_range(pages.clone())
        };
        if within(&page) && !frames.contains(&page) {
            pages.push((page, ALLOWS[rng.random_range(0..ALLOWS.len())]));
        }
    }
    pages
}

/// The pages that may run code, the blob's. Changes leave that as placed: the stand-in drops no pieces when a
/// page stops allowing it, and translates only from pages that allow it.
fn runnable(blob: &Blob) -> Range<u64> {
    let (range, _) = &placement(blob).regions[0];
    range.start / PAGE_SIZE..range.end / PAGE_SIZE
}

/// What a page allows with these reading and writing permissions.
fn with_running(runnable: &Range<u64>, page: u64, permissions: Permissions) -> Permissions {
    let running = if runnable.contains(&page) {
        Permissions::EXECUTE
    } else {
        Permissions::empty()
    };
    permissions.difference(Permissions::EXECUTE) | running
}

fn apply<G: GuestTest>(
    paused: &mut Paused<'_, '_, G>,
    pages: &[(u64, Permissions)],
    runnable: &Range<u64>,
) {
    for &(page, permissions) in pages {
        let permissions = with_running(runnable, page, permissions);
        paused.set_permissions(page * PAGE_SIZE..(page + 1) * PAGE_SIZE, permissions);
    }
}

/// Run until the thread comes back for good, counting in `came_back` each time it comes back. A fault on a
/// page a change may touch is handled.
fn end<G: GuestTest>(
    blob: &Blob,
    mut machine: Machine<G>,
    universe: &[Range<u64>],
    came_back: &Cell<usize>,
) -> (Ended, Machine<G>) {
    let mut stops = Vec::new();
    loop {
        let (stopped, registers) = machine.run();
        came_back.set(came_back.get() + 1);
        stops.push((stopped, fields::<G>(&registers)));
        match stopped {
            Stopped::Exit(Exit::MemoryFault { address, .. })
                if stops.len() <= HANDLED
                    && universe
                        .iter()
                        .any(|pages| pages.contains(&(address / PAGE_SIZE))) =>
            {
                let page = address / PAGE_SIZE;
                let permissions = with_running(
                    &runnable(blob),
                    page,
                    Permissions::READ | Permissions::WRITE,
                );
                machine.set_permissions(page * PAGE_SIZE..(page + 1) * PAGE_SIZE, permissions);
            }
            _ => {
                let ran = ran(blob, &machine, stopped, &registers);
                return (Ended { stops, ran }, machine);
            }
        }
    }
}

/// Every byte of the pages a change may touch.
fn bytes<G: GuestTest>(machine: &mut Machine<G>, universe: &[Range<u64>]) -> Vec<u8> {
    let mut all = Vec::new();
    for pages in universe {
        let mut part = vec![0; ((pages.end - pages.start) * PAGE_SIZE) as usize];
        machine.read(pages.start * PAGE_SIZE, &mut part);
        all.extend(part);
    }
    all
}

/// Run a blob at full making changes as `plan` says, then at none making the same changes at the same
/// points, and compare how they went. Panics on any difference. The run at full is translated for speed or
/// for size as the plan's seed is even or odd.
pub fn compare<G: GuestTest>(blob: &Blob, plan: Plan) -> Compared {
    let universe = universe(blob);
    let runnable = runnable(blob);
    let made = Rc::new(RefCell::new(Vec::new()));

    let full_came_back = Rc::new(Cell::new(0));
    let optimise = match plan.seed % 2 {
        0 => Optimise::Speed,
        _ => Optimise::Size,
    };
    let mut full = ready_for::<G>(blob, LookAhead::Full, optimise, Omit::default());
    {
        let made = made.clone();
        let runnable = runnable.clone();
        let came_back = full_came_back.clone();
        let universe = universe.clone();
        let mut seed = [0u8; 32];
        seed[..8].copy_from_slice(&plan.seed.to_le_bytes());
        let mut rng = TestRng::from_seed(RngAlgorithm::ChaCha, &seed);
        let mut syscalls = 0;
        full.on_pause(move |at, paused| {
            if at == Pause::Syscall {
                syscalls += 1;
            }
            if rng.random_bool(plan.stops) {
                paused.raise_stop_flag();
            }
            if at == Pause::Stop && rng.random_range(0..plan.every) != 0 {
                return;
            }
            let registers = paused.registers();
            let pages = choose::<G>(&mut rng, &fields::<G>(&registers), &universe);
            if pages.is_empty() {
                return;
            }
            let point = match at {
                Pause::Stop => Point::Stop {
                    came_back: came_back.get(),
                    registers: registers.as_bytes().to_vec(),
                    memory: memory(paused, &universe),
                },
                Pause::Syscall => Point::Syscall(syscalls),
            };
            apply(paused, &pages, &runnable);
            made.borrow_mut().push(Change { point, pages });
        });
    }
    if plan.stops > 0.0 {
        full.raise_stop_flag();
    }
    let (full_ended, mut full) = end(blob, full, &universe, &full_came_back);

    let changes = made.borrow().len();
    let pending: Rc<RefCell<VecDeque<Change>>> =
        Rc::new(RefCell::new(made.borrow().iter().cloned().collect()));
    let none_came_back = Rc::new(Cell::new(0));
    let mut none = ready::<G>(blob, LookAhead::None, Omit::default());
    {
        let pending = pending.clone();
        let runnable = runnable.clone();
        let came_back = none_came_back.clone();
        let universe = universe.clone();
        let mut syscalls = 0;
        none.on_pause(move |at, paused| {
            let mut pending = pending.borrow_mut();
            if at == Pause::Syscall {
                syscalls += 1;
            }
            let registers_now = paused.registers();
            let mut memory_now = None;
            while let Some(change) = pending.front() {
                let reached = match (&change.point, at) {
                    (&Point::Syscall(n), Pause::Syscall) => n == syscalls,
                    (
                        Point::Stop {
                            came_back: then,
                            registers,
                            memory: hash,
                        },
                        Pause::Stop,
                    ) => {
                        *then == came_back.get()
                            && *registers == registers_now.as_bytes()
                            && *hash == *memory_now.get_or_insert_with(|| memory(paused, &universe))
                    }
                    _ => false,
                };
                if !reached {
                    break;
                }
                apply(paused, &change.pages, &runnable);
                pending.pop_front();
            }
        });
    }
    none.stop_at_every_check();
    let (none_ended, mut none) = end(blob, none, &universe, &none_came_back);

    for (i, (full_stop, none_stop)) in full_ended.stops.iter().zip(&none_ended.stops).enumerate() {
        assert_eq!(
            full_stop.0, none_stop.0,
            "why the thread came back, time {i}"
        );
        assert_eq!(
            full_stop.1, none_stop.1,
            "registers when it came back, time {i}"
        );
    }
    assert_eq!(
        full_ended.stops.len(),
        none_ended.stops.len(),
        "how often the thread came back"
    );
    assert_eq!(full_ended.ran, none_ended.ran, "output and status");
    assert!(
        bytes(&mut full, &universe) == bytes(&mut none, &universe),
        "memory at the end"
    );
    let missed = pending.borrow().len();
    let first = pending.borrow().front().map(|change| match &change.point {
        Point::Stop { registers, .. } => {
            let block = <G::Block as zerocopy::FromBytes>::read_from_bytes(registers).unwrap();
            format!("a stop at {:#x}", fields::<G>(&block).0[G::PC_FIELD])
        }
        Point::Syscall(n) => format!("system call {n}"),
    });
    assert_eq!(
        missed, 0,
        "{missed} of {changes} changes made at full were never reached at none, first at {first:?}"
    );
    Compared {
        changes,
        handled: full_ended.stops.len() - 1,
        stopped: full_ended.stops.last().unwrap().0,
        translated: full.count(Reason::TranslatedAtRunTime),
        left: full.count(Reason::LeftToAddress),
    }
}
