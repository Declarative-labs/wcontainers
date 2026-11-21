# Status: host clock and random source

Plan: docs/plan-host-clock-and-random-source.md. Milestone H1 (host implementations, seeded test
doubles and real suites for `Clock` and `RandomSource`) is done, with two known failures of the
seeded clock that need the owner's ruling (below).

## Done

- Suites: crates/clock/conformance/conformance.rs (4 checks) and
  crates/random-source/conformance/conformance.rs (4 checks), as the plan lists them.
- crates/clock/implementation/host: `HostClock`, rustix `clock_gettime` (`CLOCK_MONOTONIC`,
  `CLOCK_REALTIME`).
- crates/clock/implementation/seeded: `SeededClock::new(seed)`, simulated time from a `ChaCha8Rng`.
- crates/random-source/implementation/host: `HostRandomSource`, `getrandom::fill`.
- crates/random-source/implementation/seeded: `SeededRandomSource::new(seed)`, a `ChaCha8Rng`.
- Each implementation runs its suite through `tests!` in its own conformance.rs ([[test]] target
  `conformance`), the shape the owner's linter requires.

## Results (2026-11-21)

- Mac (M5 Max, load 10 to 16 from other tracks), from a clean build folder, 14 s for all of it:
  rustfmt check and clippy with `-D warnings` clean; HostClock 4 of 4, HostRandomSource 4 of 4,
  SeededRandomSource 4 of 4, SeededClock 2 of 4 (the known failures).
- ARM64 Linux machine (Graviton4, 96 cores, Linux 7.0.0-1018-aws, load 2.2 to 2.7), cold build folder:
  the same results, clippy clean.
- wasm32-unknown-unknown, Mac and Linux: wcontainers-clock, wcontainers-random-source, both
  conformance crates, wcontainers-clock-seeded and wcontainers-random-source-seeded build. The two host
  crates do not, as expected with no operating system: rustix's errno dependency refuses an unknown OS,
  and getrandom 0.4.3 refuses wasm32-unknown-unknown without its `wasm_js` backend. A browser
  implementation (performance.now, crypto.getRandomValues) would be its own crate.
- The owner's linter (`cargo reviewable check`, rule sets layout, literals, naming, comments), run on a
  scratch workspace holding only these two interfaces and their crates: 4 findings, all the seeded
  constructors (`impl` block and `pub fn new` on `SeededClock` and `SeededRandomSource`), which wait for
  the Standalone traits below. No literal finding.
- cargo tree: no interface crate and no conformance crate of the workspace reaches any of its 14
  implementation packages. No interface.rs or interface manifest changed since the reorganization
  (git diff from 4308b9f7).

## Which check fails when a method breaks

Tried in a scratch copy (pyro-parked/wcontainers-scratch/host-clock-and-random-source/mutate.sh, Mac),
each breakage alone; every one failed exactly the check named:
- `monotonic_nanoseconds` in microseconds: `count_monotonic_nanoseconds_across_a_sleep`.
- `monotonic_nanoseconds` stepping back a millisecond now and then: `read_monotonic_clock_forward` and
  `read_monotonic_clock_forward_across_threads`. (A first try stepping back one microsecond was not
  caught, rightly: macOS's monotonic clock ticks in microseconds, so it never went backwards.)
- `monotonic_nanoseconds` with each new thread a millisecond behind: `read_monotonic_clock_forward_across_threads`.
- `realtime_nanoseconds` reading the monotonic clock: `read_realtime_clock_as_the_wall_clock`.
- `fill_random_bytes` leaving the second half: `fill_every_byte_given`.
- `fill_random_bytes` from a generator per thread seeded alike: `fill_random_bytes_differently_across_threads`.
- `fill_random_bytes` with the top bit always set: `fill_random_bytes_with_every_byte_value`.

## Known failures

- wcontainers-clock-seeded, `count_monotonic_nanoseconds_across_a_sleep` (it counts 42335 ns across a
  1 ms sleep) and `read_realtime_clock_as_the_wall_clock` (it reads a simulated date in 2066): the seeded
  clock is simulated time and follows neither a real sleep nor the host's wall clock. The linter's fixed
  `tests!` shape forbids `#[ignore]`, so they stay failing until the owner rules.

## How to run

- Everything (rustfmt check, clippy, the four suites, the wasm32 builds), in the checkout:
  `CARGO_TARGET_DIR=$PWD/target/host-clock-and-random-source nix develop -c bash
  ~/Documents/GitHub/pyro-parked/wcontainers-scratch/host-clock-and-random-source/check.sh`
  (exits 1 while the two known failures stand).
- Linux: `arm-run <checkout> --name host-clock-and-random-source -- '<the cargo commands of check.sh>'`.
- No check is random, so there is no case count; the seeded doubles' conformance tests use seed
  `u64::MIN`.

## For the owner

- Clock: does it promise to follow real time? Example: `SeededClock` (simulated time, the reason
  `Clock` is a concern of its own) counts 42335 ns across a 1 ms `std::thread::sleep`, so the
  suite's `count_monotonic_nanoseconds_across_a_sleep` fails on it.
- Neither interface can build a value. The seeded doubles need a seed, so each has `new(seed: u64)`,
  an impl block of its own the linter refuses. The trait I would need, coupled to the main one as
  `StandaloneRuntime` is:
  `pub trait StandaloneClock: Clock { type Config; fn new(config: Self::Config) -> Self; }`
  and the same `StandaloneRandomSource: RandomSource`.
- `RandomSource::fill_random_bytes` has no error. Example: getrandom(2) fails with ENOSYS under a
  seccomp filter that forbids it; `HostRandomSource` can only panic with getrandom's error.

## Departures from the plan

- None in behaviour. Every exported type derives `Debug`, which the linter expects of an exported
  implementation type.

## Dependencies and licences

- rustix 1.1.6: Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT (the host clock; brings errno,
  bitflags, cfg-if, libc on macOS: MIT OR Apache-2.0).
- getrandom 0.4.3: MIT OR Apache-2.0 (the host random source).
- rand 0.9.5, rand_core 0.9.5, rand_chacha 0.9.0, ppv-lite86 0.2.21: MIT OR Apache-2.0; zerocopy 0.8:
  BSD-2-Clause OR Apache-2.0 OR MIT (the seeded doubles).
All were already in Cargo.lock.
