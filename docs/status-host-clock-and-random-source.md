# Status: host clock and random source

Plan: docs/plan-host-clock-and-random-source.md. Milestone H1 (host implementations, seeded test
doubles and real suites for `Clock` and `RandomSource`) is in progress.

## Done

- Suites: crates/clock/conformance/conformance.rs (4 checks) and
  crates/random-source/conformance/conformance.rs (4 checks), as the plan lists them.
- crates/clock/implementation/host (`HostClock`, rustix `clock_gettime`).
- crates/clock/implementation/seeded (`SeededClock`, simulated time from a `ChaCha8Rng`).
- crates/random-source/implementation/host (`HostRandomSource`, getrandom).
- crates/random-source/implementation/seeded (`SeededRandomSource`, a `ChaCha8Rng`).
- Each implementation runs its suite through `tests!` in its own conformance.rs ([[test]] target
  `conformance`), the linter's shape.

## Known failures

- wcontainers-clock-seeded, `count_monotonic_nanoseconds_across_a_sleep` and
  `read_realtime_clock_as_the_wall_clock` fail by design: the seeded clock is simulated time and
  follows neither a real sleep nor the host's wall clock. The linter's fixed `tests!` shape forbids
  `#[ignore]`, so they stay failing until the owner rules (below).

## How to run

- Mac: `CARGO_TARGET_DIR=$PWD/target/host-clock-and-random-source nix develop -c cargo test -j 4
  --no-fail-fast -p wcontainers-clock-host -p wcontainers-clock-seeded -p
  wcontainers-random-source-host -p wcontainers-random-source-seeded -- --test-threads 3`
- Linux: the same through `arm-run <checkout> --name host-clock-and-random-source -- '<command>'`.

## For the owner

- Clock: does it promise to follow real time? Example: `SeededClock` (simulated time, the reason
  `Clock` is a concern of its own) counts 42335 ns across a 1 ms `std::thread::sleep`, so the
  suite's `count_monotonic_nanoseconds_across_a_sleep` fails on it.
- Neither interface can build a value. The seeded doubles need a seed, so each has `new(seed: u64)`,
  an impl block of its own the linter will refuse. The trait I would need, coupled to the main one as
  `StandaloneRuntime` is:
  `pub trait StandaloneClock: Clock { type Config; fn new(config: Self::Config) -> Self; }`
  and the same `StandaloneRandomSource: RandomSource`.
- `RandomSource::fill_random_bytes` has no error. Example: getrandom(2) fails with ENOSYS under a
  seccomp filter that forbids it; `HostRandomSource` can only panic with getrandom's error.

## Dependencies and licences

- rustix 1.1.6: Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT (the host clock).
- getrandom 0.4.3: MIT OR Apache-2.0 (the host random source).
- rand 0.9.5 and rand_chacha 0.9.0: MIT OR Apache-2.0 (the seeded doubles).
All were already in Cargo.lock.
