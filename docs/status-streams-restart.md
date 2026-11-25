# Status of the streams track (restart of 2026-11-25)

Brief: orchestrator/restart-2026-11-25/brief-streams.md. Plan: docs/streams.md. The old status (docs/status-streams.md)
is history. Branch `streams-restart`, cut from the owner's main 95bceadf0.

## Where things stand

- ST0 (stock): done. At main the system suite did not compile (E0283 at conformance.rs:42, the placeholder);
  kernel-linux 346 errors (its own track); system, readiness and readiness-conformance check and clippy clean.
- ST1 (the suite on main's rules): met but for linter findings that wait (below). The suite came from origin/agents
  198bb11d6 by path.
- ST2 (the stream contract checked): met but for the same findings. Seven stream checks and the readiness suite's
  six, on a loop with no limit and one holding a page (26 tests), and 15 broken loops each caught by its message.
- ST3 (this file and the plan): done.
- ST4 (the suite run on each implementation): waits on RT1 (seeded-test-policy, `LocalSystem`), KS and X3 (unicorn
  `TestHost`), R2 (`LinkHost`). READ (origin/agents seeded-test-policy conformance_run.rs:45-90): its loop gives
  `Ok(0)` from an empty queue, ignores the close of the sending endpoint, never reports `HANG_UP` and never wakes a
  wait, so `a_receive_with_nothing_sent_would_block`, both closing checks and the readiness suite's waking and
  failure checks will fail on it (INFERRED, not run) until the runtime track fixes that loop, which is a test loop,
  not a product.

## Measurements

At 1c188eaae (same tree as the commit that adds this file):

| Bar | ARM64 (Graviton4, load 18 at start) | x86-64 (load 22 at start) |
|---|---|---|
| `cargo check -p wcontainers-system-conformance --all-targets` | exit 0 | exit 0 |
| `cargo clippy ... --all-targets --no-deps -- -D warnings` | exit 0 | exit 0 |
| `cargo fmt -p wcontainers-system-conformance --check` | exit 0 | exit 0 |
| `cargo test -p wcontainers-system-conformance` | 41 passed, 0 failed, 0 ignored | 41 passed, 0 failed, 0 ignored |
| lines with `unsafe` in crates/system/conformance | 0 | 0 |
| `cargo reviewable check`, findings in crates/system/conformance | 60, one tree earlier (one summary over two lines, fixed before x86-64 ran) | 59 |

The 59 findings, by cause:
- 26 `summary_unanswered` and 13 `name_unanswered`: the model's answers need its key, which no rented machine may
  hold; they clear when the check runs where the key is and reviewable-answers.toml is committed.
- 9 `conformance_not_run` "must be `pub fn f<T: Trait>(value: T)`" (seven checks and the two `Waiting` makers),
  1 "a second macro" (`stream_tests!`) and 1 "`StreamTransport` requires `Readiness`": a `StreamTransport` alone
  gives no endpoint, so no check of the required shape can send or receive (the brief's question 3). Smallest
  example:

  ```rust
  pub trait StreamTransport: Readiness<Self::Endpoint> { type Endpoint: Ord; fn close_stream(&self, peer: &Self::Endpoint); }
  // The required shape names no endpoint to close:
  pub fn closing_ends_the_stream<T: StreamTransport>(value: T) { value.close_stream(/* none */) }
  ```
- 7 `conformance_not_run` "`System` requires X": the clock and random source suites on main take no group argument
  (`tests!($value)`), where the line the finding gives passes `StandaloneClock: no`; the ICMP, raw, route, TCP and UDP
  suites do not compile on main under `forbid` (the host-sockets track's HS1). The lines are added when those land.

Logs: SCRATCH/streams-restart/st0 (stock), st1-arm, st2-arm, st2-x86 (bars). Fault of this run: the x86-64 bar ran
with 96 jobs (that machine's default), not the 12 allowed; later x86 jobs pass `--jobs 12`.

## SECURITY

- Lines with `unsafe`: crates/system/conformance 0 before, 0 after. No marked exception.
- New dependencies: none from outside (wcontainers-abi and wcontainers-readiness-conformance are the workspace's own).
- Hostile-guest suite: none exists; none run.
