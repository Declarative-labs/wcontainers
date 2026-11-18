//! Random call sequences, seeded and replayable: each is recorded by the recording back end without
//! breaking a rule, and its call stream replayed on a new one gives the same stream, the same answers
//! and the same contents. `WCONTAINERS_GPU_SEQUENCES` sets how many (default 1000) and
//! `WCONTAINERS_GPU_SEED` the seed (default 0x5eed); the test prints both.

use proptest::test_runner::{Config, RngAlgorithm, RngSeed, TestRunner};
use std::cell::Cell;
use std::collections::HashMap;
use wcontainers_gpu::WriteMode;
use wcontainers_gpu_test::{Call, Recorder, replay, sequence};

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().map_or(default, |v| {
        let v = v.trim();
        match v.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16),
            None => v.parse(),
        }
        .unwrap_or_else(|_| panic!("{name} is not a number: {v}"))
    })
}

#[test]
fn random_sequences_record_and_replay_identically() {
    let cases = knob("WCONTAINERS_GPU_SEQUENCES", 1000);
    let seed = knob("WCONTAINERS_GPU_SEED", 0x5eed);
    let mut runner = TestRunner::new(Config {
        cases: cases as u32,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(seed),
        failure_persistence: None,
        ..Config::default()
    });
    let ran = Cell::new(0u64);
    let calls = Cell::new(0usize);
    let dropped = Cell::new(0u64);
    // Multisampled passes, depth-stencil passes, indexed draws (and those dropped whole for fetching
    // past their stream, counted by the recorder), queries, scissored draws, tinted
    // draws, no-overwrite buffer writes made without waiting first, and texture writes that
    // discard, and that do not overwrite without waiting first; dispatches, vertex captures and
    // timestamps; copies within one subresource, and those of depth or stencil formats.
    let seen = Cell::new([0usize; 14]);
    let result = runner.run(&sequence::steps(40), |steps| {
        ran.set(ran.get() + 1);
        let mut r = Recorder::new();
        let read = sequence::run(&mut r, &steps).expect("a step was refused");
        assert!(r.violations.is_empty(), "violations: {:?}", r.violations);
        calls.set(calls.get() + r.calls.len());
        dropped.set(dropped.get() + r.dropped_draws);
        let mut n = seen.get();
        // The sample count of each view's texture, for the passes (surface textures, not made by
        // create_texture, have one).
        let mut samples = HashMap::new();
        let mut formats = HashMap::new();
        let mut view_samples = HashMap::new();
        for (i, c) in r.calls.iter().enumerate() {
            let k = match c {
                Call::CreateTexture(d, Ok(t)) => {
                    samples.insert(*t, d.samples);
                    formats.insert(*t, d.format);
                    continue;
                }
                Call::CreateView(t, _, Ok(v)) => {
                    view_samples.insert(*v, samples.get(t).copied().unwrap_or(1));
                    continue;
                }
                Call::BeginPass(p) => {
                    if p.colors
                        .iter()
                        .flatten()
                        .any(|c| view_samples.get(&c.0).is_some_and(|&s| s > 1))
                    {
                        n[0] += 1;
                    }
                    if p.depth.is_some() {
                        n[1] += 1;
                    }
                    continue;
                }
                Call::Draw(d) if d.index.is_some() => 2,
                Call::BeginQuery(_) => 3,
                Call::Draw(d) if !d.scissors.is_empty() => 4,
                Call::Draw(d) if d.bindings[0].is_some() => 5,
                Call::WriteBuffer(.., WriteMode::NoOverwrite)
                    if !matches!(r.calls.get(i.wrapping_sub(1)), Some(Call::WaitFence(_))) =>
                {
                    6
                }
                Call::WriteTexture(.., WriteMode::Discard) => 7,
                Call::Dispatch(..) => 9,
                Call::CaptureVertices(..) => 10,
                Call::WriteTimestamp(..) => 11,
                Call::CopyTexture(s, d, _) if (s.0, s.1, s.2) == (d.0, d.1, d.2) => {
                    if formats.get(&s.0).is_some_and(|f| f.is_depth_stencil()) {
                        n[13] += 1;
                    }
                    12
                }
                Call::WriteTexture(.., WriteMode::NoOverwrite)
                    if !matches!(r.calls.get(i.wrapping_sub(1)), Some(Call::WaitFence(_))) =>
                {
                    8
                }
                _ => continue,
            };
            n[k] += 1;
        }
        seen.set(n);
        let again = replay(&r.calls);
        assert!(
            again.violations.is_empty(),
            "violations: {:?}",
            again.violations
        );
        assert_eq!(again.calls, r.calls);
        assert_eq!(again.dropped_draws, r.dropped_draws);
        assert_eq!(again.contents(), r.contents());
        // The bytes the steps read back are in the stream's finish_read calls.
        let finished: Vec<_> = r
            .calls
            .iter()
            .filter_map(|c| match c {
                wcontainers_gpu_test::Call::FinishRead(_, bytes) => Some(bytes.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(finished, read);
        Ok(())
    });
    let n = seen.get();
    eprintln!(
        "random sequences: seed {seed:#x}, {} asked, {} run, {} calls, {} multisampled passes, \
         {} depth-stencil passes, {} indexed draws, {} queries, {} scissored draws, {} tinted \
         draws, {} no-overwrite writes without a wait, {} discarding texture writes, {} \
         no-overwrite texture writes without a wait, {} dispatches, {} vertex captures, {} \
         timestamps, {} copies within one subresource ({} of depth or stencil), {} draws dropped \
         for fetching past a stream",
        cases,
        ran.get(),
        calls.get(),
        n[0],
        n[1],
        n[2],
        n[3],
        n[4],
        n[5],
        n[6],
        n[7],
        n[8],
        n[9],
        n[10],
        n[11],
        n[12],
        n[13],
        dropped.get()
    );
    result.unwrap();
}
