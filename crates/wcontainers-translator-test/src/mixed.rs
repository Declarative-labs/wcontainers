//! Across the layers: pieces of different look-aheads mixed at random in one run, and rare events injected
//! at random stop checks spread over the whole run: the WebAssembly stack lost (a fresh thread instance
//! carries on from the registers), and an address made to have no piece yet. The result must be none's.

use crate::programs::{Blob, Ran, ran, ready_with};
use crate::runner::{Machine, Pause};
use proptest::prelude::RngExt;
use proptest::test_runner::{RngAlgorithm, TestRng};
use std::cell::Cell;
use std::rc::Rc;
use wcontainers_abi::RegisterBlock;
use wcontainers_translator::{BOUND, LookAhead};

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

/// Runs whose seed is a multiple of this stop nowhere and have nothing injected: pieces of every look-ahead
/// meet on their fast paths.
const QUIET: u64 = 4;

fn rng(seed: u64, what: &[u8; 8]) -> TestRng {
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(what);
    TestRng::from_seed(RngAlgorithm::ChaCha, &key)
}

/// Run a blob with its code at none, at bounded look-ahead of a random N from its first byte and random
/// other starts, and at full, each address's piece picked at random among them; code reached at run time
/// at a random bounded look-ahead each time; and, unless the seed is a multiple of [`QUIET`], a stop at
/// every stop check with events injected at random ones among them, all from `seed`.
pub fn mixed(blob: &Blob, seed: u64) -> (Ran, Machine, RegisterBlock, Events) {
    let mut pick = rng(seed, b"mixing  ");
    let bound = pick.random_range(2..=64);
    let mut starts = vec![0];
    for _ in 0..(blob.code_bytes / 64).clamp(1, 64) {
        starts.push(4 * pick.random_range(0..blob.code_bytes / 4));
    }
    let translations = [
        (LookAhead::None, vec![0]),
        (LookAhead::Bounded(bound), starts),
        (LookAhead::Full, vec![0]),
    ];
    let mut machine = ready_with(blob, crate::runner::probe(), |machine| {
        machine.load_mixed(
            &blob.bytes,
            0..blob.code_bytes,
            blob.load,
            &translations,
            seed,
        );
    });
    let mut at_run_time = rng(seed, b"run time");
    machine.translate_at_run_time(move || match at_run_time.random_range(0..3) {
        0 => LookAhead::Bounded(1),
        1 => LookAhead::Bounded(at_run_time.random_range(2..=64)),
        _ => LookAhead::Bounded(BOUND),
    });
    let events = Rc::new(Cell::new(Events::default()));
    if !seed.is_multiple_of(QUIET) {
        let events = events.clone();
        let mut inject = rng(seed, b"inject  ");
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
                if inject.random_bool(0.5) || placed.is_empty() {
                    paused.lose_stack();
                    now.stacks_lost += 1;
                } else {
                    let address = placed[inject.random_range(0..placed.len())];
                    paused.forget(address);
                    now.forgotten += 1;
                }
                gap += gap / 4;
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
