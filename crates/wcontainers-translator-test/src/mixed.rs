//! Across the layers: pieces of different look-aheads mixed at random in one run, and rare events injected
//! at random stop checks spread over the whole run: the WebAssembly stack lost (a fresh thread instance
//! carries on from the registers), and an address made to have no piece yet. The result must be none's.

use crate::guest::GuestTest;
use crate::programs::{Blob, Ran, ran, ready_with};
use crate::runner::{Machine, Pause};
use crate::seeded::generator;
use proptest::prelude::RngExt;
use std::cell::Cell;
use std::rc::Rc;
use wcontainers_translator::{BOUND, LookAhead, Optimise};

/// What was injected into one run.
#[derive(Clone, Copy, Debug, Default)]
pub struct Events {
    /// Stop checks the thread stopped at.
    pub stops: u64,
    pub stacks_lost: u64,
    pub forgotten: u64,
}

/// The first event comes within this many stops; each later gap may be a quarter longer than the one
/// before, so a run of any length has events throughout, about 4.5 for each doubling of its stops.
const FIRST_GAP: u64 = 8;
/// A gap grows by at most its own length divided by this.
const GAP_GROWTH_DIVISOR: u64 = 4;
/// How likely an event is to lose the stack rather than make an address have no piece.
const STACK_LOST_ODDS: f64 = 0.5;

/// The largest random N of a bounded look-ahead.
const LARGEST_BOUND: u32 = 64;
/// One random start for each this many bytes of code, and at most [`MOST_STARTS`].
const CODE_BYTES_PER_START: u64 = 64;
const MOST_STARTS: u64 = 64;
/// The kinds of look-ahead code reached at run time is translated at: 1, a random N, and [`BOUND`].
const RUN_TIME_LOOK_AHEADS: u32 = 3;

/// The tags that give each random choice of a run its own sequence.
const MIXING: &[u8] = b"mixing  ";
const RUN_TIME: &[u8] = b"run time";
const INJECT: &[u8] = b"inject  ";

/// Runs whose seed is a multiple of this stop nowhere and have nothing injected: pieces of every look-ahead
/// meet on their fast paths.
const QUIET: u64 = 4;

/// Run a blob with its code at none, at bounded look-ahead of a random N from its first byte and random
/// other starts, and at full, each address's piece picked at random among them; code reached at run time
/// at a random bounded look-ahead each time; and, unless the seed is a multiple of [`QUIET`], a stop at
/// every stop check with events injected at random ones among them, all from `seed`.
pub fn mixed<G: GuestTest>(blob: &Blob, seed: u64) -> (Ran, Machine<G>, G::Block, Events) {
    let mut pick = generator(seed, MIXING, "");
    let bound = pick.random_range(2..=LARGEST_BOUND);
    let mut starts = vec![0];
    let boundaries = crate::programs::boundaries::<G>(&blob.bytes[..blob.code_bytes as usize]);
    for _ in 0..(blob.code_bytes / CODE_BYTES_PER_START).clamp(1, MOST_STARTS) {
        starts.push(boundaries[pick.random_range(0..boundaries.len())]);
    }
    let optimise = |size: bool| {
        if size {
            Optimise::Size
        } else {
            Optimise::Speed
        }
    };
    let translations = [
        (LookAhead::None, Optimise::Speed, vec![0]),
        (LookAhead::Bounded(bound), optimise(pick.random()), starts),
        (LookAhead::Full, Optimise::Speed, vec![0]),
        (LookAhead::Full, Optimise::Size, vec![0]),
    ];
    let at_run_time = optimise(pick.random());
    let mut machine = ready_with::<G>(blob, crate::runner::probe::<G>(), |machine| {
        machine.load_mixed(
            &blob.bytes,
            0..blob.code_bytes,
            (blob.load, blob.link_address),
            &translations,
            seed,
        );
    });
    machine.optimise_for(at_run_time);
    let mut at_run_time = generator(seed, RUN_TIME, "");
    machine.translate_at_run_time(move || {
        match at_run_time.random_range(0..RUN_TIME_LOOK_AHEADS) {
            0 => LookAhead::Bounded(1),
            1 => LookAhead::Bounded(at_run_time.random_range(2..=LARGEST_BOUND)),
            _ => LookAhead::Bounded(BOUND),
        }
    });
    let events = Rc::new(Cell::new(Events::default()));
    if !seed.is_multiple_of(QUIET) {
        let events = events.clone();
        let mut inject = generator(seed, INJECT, "");
        let mut gap = FIRST_GAP;
        let mut next = inject.random_range(1..=gap);
        machine.on_pause(move |at, paused| {
            if at != Pause::Stop {
                return;
            }
            let mut now = events.get();
            now.stops += 1;
            if now.stops == next {
                let placed = paused.placed();
                if inject.random_bool(STACK_LOST_ODDS) || placed.is_empty() {
                    paused.lose_stack();
                    now.stacks_lost += 1;
                } else {
                    let address = placed[inject.random_range(0..placed.len())];
                    paused.forget(address);
                    now.forgotten += 1;
                }
                gap += gap / GAP_GROWTH_DIVISOR;
                next += inject.random_range(1..=gap);
            }
            events.set(now);
        });
        machine.stop_at_every_check();
    }
    let (stopped, registers) = machine.run();
    let result = ran(blob, &machine, stopped, &registers);
    (result, machine, registers, events.get())
}
