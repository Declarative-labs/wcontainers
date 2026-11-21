//! N2's forced interrupts and the count across look-aheads: guest functions run as the native output's pieces,
//! one piece after another, inside Hypervisor.framework, at every look-ahead setting. At look-ahead none every
//! instruction boundary is a stop, which gives the run's boundaries; each is checked against definition E's
//! count, and the end against the WebAssembly output under wasmtime at every look-ahead. Then, at every
//! look-ahead, the function runs whole and an interrupt is forced at every host instruction it is about to run
//! (found by software step and at every return from the inside): the guest state the position map resolves
//! there (registers, pc, count and the rest) must be one of the boundaries, in order, every boundary at an
//! instruction the run carries in host bytes must be reached by some interrupt, and the run going on from there
//! must end as the unbroken one. So the count at
//! every stop is the same at every look-ahead. Each case runs from seeded starting states (flags, thread
//! pointer, vectors, data area). It runs itself as a copy signed with the hypervisor entitlement.

#[cfg(host_hypervisor)]
mod common;

#[cfg(host_hypervisor)]
#[path = "../../hardware-virtualization-inside/bin/native-cases/protocol.rs"]
mod protocol;

#[cfg(host_hypervisor)]
mod runner;

#[cfg(host_hypervisor)]
fn main() {
    hosted::main();
}

#[cfg(not(host_hypervisor))]
fn main() {
    println!(
        "the interrupt test needs a hypervisor this crate drives; nothing to run on this host"
    );
}

#[cfg(host_hypervisor)]
mod hosted {
    use std::time::Instant;

    use wcontainers_arm64::{Arm64, decode};
    use wcontainers_guest_cpu::{Guest, Native, Route, counted};
    use wcontainers_translator::LookAhead;
    use wcontainers_translator_test::guest::arm64::words::Case;
    use wcontainers_translator_test::layout::CODE;

    use crate::common::{be_signed, load_average, setting};
    use crate::runner::function::{Run, same_end, webassembly};
    use crate::runner::functions::{
        DEFAULT_SEED, DEFAULT_STARTS, LOOK_AHEADS, RETURNS, SEED_VARIABLE, STARTS_VARIABLE,
        functions,
    };
    use crate::runner::{GuestState, Runner};

    /// What one function's checks found.
    #[derive(Default)]
    struct Tally {
        forced: u64,
        failed: u64,
    }

    /// The function's word at guest address `pc`, if it has one there.
    fn word_at(words: &[u32], pc: u64) -> Option<&u32> {
        let index = pc.checked_sub(CODE)? / Arm64::MAX_LENGTH;
        words.get(usize::try_from(index).ok()?)
    }

    /// Runs one function's checks from one case.
    fn check(runner: &mut Runner, name: &str, words: &[u32], case: &Case, tally: &mut Tally) {
        let mut fail = |what: String| {
            println!("FAIL {name} (starting state {:#x}): {what}", case.seed);
            tally.failed += 1;
        };
        // Look-ahead none stops at every boundary: they are the run's boundaries.
        let mut function = runner.load_function(words, LookAhead::None);
        let reference = runner.run_function(&mut function, case, RETURNS, None);
        runner.unload();
        // A stop at an instruction (a fault) is at the boundary before it.
        let mut boundaries = reference.trace.clone();
        boundaries.dedup();
        let boundaries = &boundaries;
        let mut count = 0;
        for (k, state) in boundaries.iter().enumerate() {
            if state.count != count {
                fail(format!(
                    "count {} at boundary {k}, E gives {count}",
                    state.count
                ));
            }
            if let Some(&word) = word_at(words, state.pc) {
                count += u64::from(counted(state.pc, &Arm64::flow(&decode(word))));
            }
        }
        let last = boundaries.last().expect("a boundary");
        for look_ahead in LOOK_AHEADS {
            let (stop, after, data) = webassembly(words, case, look_ahead);
            if !same_end(stop, &after, reference.stop, last) || data != reference.data {
                fail(format!(
                    "the WebAssembly output at {look_ahead:?} ends {stop:?} {after:x?}, natively {:?} {last:x?}",
                    reference.stop
                ));
            }
        }
        let mut lines = Vec::new();
        for look_ahead in LOOK_AHEADS {
            let mut function = runner.load_function(words, look_ahead);
            let whole = runner.run_function(&mut function, case, RETURNS, None);
            let stepped = runner.run_function(&mut function, case, RETURNS, Some(u64::MAX));
            if whole.stop != reference.stop || whole.data != reference.data {
                fail(format!(
                    "at {look_ahead:?} the run ends {:?}, at none {:?}",
                    whole.stop, reference.stop
                ));
            }
            if whole.trace != stepped.trace || whole.data != stepped.data {
                fail(format!("at {look_ahead:?} stepping changed the run"));
            }
            let mut from = 0;
            for state in &whole.trace {
                match boundaries[from..].iter().position(|b| b == state) {
                    Some(k) => from += k,
                    None => fail(format!(
                        "at {look_ahead:?} a piece's stop is no boundary: {state:x?}"
                    )),
                }
            }
            let mut reached = vec![false; boundaries.len()];
            let mut from = 0;
            for budget in 0..stepped.landings {
                let run: Run = runner.run_function(&mut function, case, RETURNS, Some(budget));
                let (state, _) = run.interrupted.as_ref().expect("an interrupt").clone();
                tally.forced += 1;
                if run.stop != whole.stop
                    || run.trace.last() != whole.trace.last()
                    || run.data != whole.data
                {
                    fail(format!(
                        "at {look_ahead:?}, going on after an interrupt at landing point {budget} ends {:?} {:x?}, unbroken {:?} {:x?}",
                        run.stop,
                        run.trace.last(),
                        whole.stop,
                        whole.trace.last()
                    ));
                }
                match boundaries[from..].iter().position(|b| *b == state) {
                    Some(k) => {
                        from += k;
                        reached[from] = true;
                    }
                    None => fail(format!(
                        "at {look_ahead:?}, interrupt at landing point {budget}: {state:x?}\n  boundary {from}: {:x?}",
                        boundaries[from]
                    )),
                }
            }
            // In a run of more than one instruction the Nothing route carries no host bytes: the host
            // instruction before it is the one before its successor, which the position map names, with
            // the same count and registers. Only a stop asked for at that pc (replay's check) lands there.
            let carried = |state: &GuestState| {
                look_ahead == LookAhead::None
                    || word_at(words, state.pc)
                        .is_none_or(|&word| Arm64::route(&decode(word)) != Route::Nothing)
            };
            if let Some(k) = (0..boundaries.len()).find(|&k| !reached[k] && carried(&boundaries[k]))
            {
                fail(format!(
                    "at {look_ahead:?} no interrupt reached boundary {k}: {:x?}",
                    boundaries[k]
                ));
            }
            runner.unload();
            lines.push(format!(
                "{look_ahead:?} {} pieces run, {} landing points",
                whole.trace.len() - 1,
                stepped.landings
            ));
        }
        println!(
            "{name}: {} boundaries, stop {:?}, count {} at the end; {}",
            boundaries.len(),
            reference.stop,
            last.count,
            lines.join(", ")
        );
    }

    pub fn main() {
        be_signed();
        let started = Instant::now();
        let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
        let starts = setting(STARTS_VARIABLE, DEFAULT_STARTS);
        let mut runner = Runner::start();
        let mut seeds = fastrand::Rng::with_seed(seed);
        let mut tally = Tally::default();
        let mut cases = 0;
        let mut names = 0;
        for draw in 0..starts {
            // The first starting state is the seed's own, so a failing one replays with it alone.
            let draw_seed = if draw == 0 { seed } else { seeds.u64(..) };
            let functions = functions(draw_seed);
            names = functions.len();
            for (name, words, function_cases) in &functions {
                for case in function_cases {
                    check(&mut runner, name, words, case, &mut tally);
                    cases += 1;
                }
            }
        }
        runner.finish();
        println!(
            "forced interrupts: {} over {names} functions ({cases} cases from {starts} starting states, seed {seed:#x}) at {} look-aheads, {} failed, in {:?}, load {}",
            tally.forced,
            LOOK_AHEADS.len(),
            tally.failed,
            started.elapsed(),
            load_average()
        );
        assert_eq!(tally.failed, 0, "interrupts resolved wrong");
    }
}
