# Plan: host clock and random source

Two implementations of each of the two smallest interfaces, crates/clock (`Clock`) and
crates/random-source (`RandomSource`), and a real conformance suite for each. The interfaces are
reviewed and do not change.

## The crates

- crates/clock/implementation/host, package wcontainers-clock-host, type `HostClock`: the operating
  system's clocks through rustix (`rustix::time::clock_gettime`).
  - `monotonic_nanoseconds`: `CLOCK_MONOTONIC`, turned into nanoseconds through rustix's
    `Timespec` to `Duration` conversion.
  - `realtime_nanoseconds`: `CLOCK_REALTIME`, the same way.
  - Linux keeps both clocks at or after zero and below 2262 (`timespec64_valid_settod` refuses
    a negative time), so a reading always fits a `u64`; a reading that does not is a broken host and
    panics with a message, never a made-up value.
- crates/clock/implementation/seeded, package wcontainers-clock-seeded, type `SeededClock`: simulated
  time from a seed, the same readings for the same seed in the same order. One `ChaCha8Rng`
  (rand_chacha) behind a mutex draws the start of the monotonic clock, the wall clock's distance from
  it, and a step before each reading. Both clocks read the one simulated time, so they advance
  together as Linux's do. It needs no operating system.
- crates/random-source/implementation/host, package wcontainers-random-source-host, type
  `HostRandomSource`: the getrandom crate (`getrandom::fill`): getrandom(2) on Linux, the system's
  own source on macOS. The trait has no error to give; a failure of the host's source panics with
  getrandom's error, never a fill that is not random.
- crates/random-source/implementation/seeded, package wcontainers-random-source-seeded, type
  `SeededRandomSource`: a `ChaCha8Rng` from a seed behind a mutex; the same bytes for the same seed
  in the same order. Not fit for cryptography. It needs no operating system.

The host types have no fields, so they are built by name. The seeded types need a seed and have a
private field, so each has the smallest constructor, `new(seed: u64)`, until the interface has a way
to build one (for the owner: `StandaloneClock` and `StandaloneRandomSource`, shaped like
`StandaloneRuntime`).

## What cannot be honoured

- wasm32-unknown-unknown has no operating system: the two host crates do not build for it (rustix
  and getrandom refuse the target). The interfaces, the suites and the seeded crates build for it.
- The seeded clock is simulated time: it cannot follow a real sleep or the host's wall clock, so it
  fails the two suite checks that compare with them. The linter's fixed `tests!` shape has no way to
  mark a known failure, so those two tests fail on it, by name, recorded in the status file.

## The suites

Each suite function takes one value and checks one behaviour; `tests!` runs each on a value made
anew. Every reference is the standard library's view of the same host.

Clock:
- `read_monotonic_clock_forward`: many readings in a row never go backwards.
- `read_monotonic_clock_forward_across_threads`: one reader per processor, each reading under one
  lock and comparing with the last reading any thread took; a reading that happens after another
  is never smaller.
- `count_monotonic_nanoseconds_across_a_sleep`: readings around a sleep differ by at least the
  sleep in nanoseconds and by no more than twice what `std::time::Instant` saw around them (units a
  thousand apart fail either bound).
- `read_realtime_clock_as_the_wall_clock`: a reading lies between two readings of
  `std::time::SystemTime` since `UNIX_EPOCH` taken around it, in nanoseconds.

Random source:
- `fill_random_bytes_differently`: two fills of a `u128`'s size differ (equal by chance once in
  2^128).
- `fill_every_byte_given`: for lengths from none to 64 KiB, every byte of the buffer is written in
  at least one of sixteen fills (an unwritten byte would keep its first value; a written one keeps
  it by chance once in 2^128).
- `fill_random_bytes_with_every_byte_value`: one 64 KiB fill holds all 256 byte values (a constant
  or narrow fill fails).
- `fill_random_bytes_differently_across_threads`: fills made at once on several threads all
  differ (a generator copied per thread fails).

## Runs

On the Mac and on the ARM64 Linux machine: `cargo test` of the four implementation crates (each
runs its suite through `tests!`), clippy with warnings denied, and `cargo build --target
wasm32-unknown-unknown` of the interfaces, suites and seeded crates.
