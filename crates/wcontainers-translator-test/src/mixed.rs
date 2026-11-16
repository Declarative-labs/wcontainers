//! Across the layers: pieces of different look-aheads mixed at random in one run, and rare events injected
//! at random stop checks: the WebAssembly stack lost (a fresh thread instance carries on from the
//! registers), an address made to have no piece yet, and the stop flag raised. The result must be none's.

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
    pub stops: u64,
    pub stacks_lost: u64,
    pub forgotten: u64,
}

/// Pauses at which the stop flag is raised again at random; after them only system calls raise it.
const PAUSES: u64 = 4000;

fn rng(seed: u64, what: &[u8; 8]) -> TestRng {
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(what);
    TestRng::from_seed(RngAlgorithm::ChaCha, &key)
}

/// Run a blob with its code at none, at bounded look-ahead of a random N from its first byte and random
/// other starts, and at full, each address's piece picked at random among them; code reached at run time
/// at a random bounded look-ahead each time; and events injected at random stops, all from `seed`.
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
    {
        let events = events.clone();
        let mut inject = rng(seed, b"inject  ");
        machine.on_pause(move |at, paused| {
            let mut now = events.get();
            if at == Pause::Stop {
                now.stops += 1;
                match inject.random_range(0..8) {
                    0 => {
                        paused.lose_stack();
                        now.stacks_lost += 1;
                    }
                    1 if !paused.placed().is_empty() => {
                        let placed = paused.placed();
                        let address = placed[inject.random_range(0..placed.len())];
                        paused.forget(address);
                        now.forgotten += 1;
                    }
                    _ => {}
                }
            }
            if now.stops < PAUSES && inject.random_bool(0.5) {
                paused.raise_stop_flag();
            }
            events.set(now);
        });
    }
    machine.raise_stop_flag();
    let (stopped, registers) = machine.run();
    let result = ran(blob, &machine, stopped, &registers);
    (result, machine, registers, events.get())
}
