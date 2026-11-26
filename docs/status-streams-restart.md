# Status of the streams track (restart of 2026-11-25)

Brief: orchestrator/restart-2026-11-25/brief-streams.md. Plan: docs/streams.md. The old status (docs/status-streams.md)
is history. Branch `streams-restart`, cut from the owner's main 95bceadf0; his main of 2026-11-25 (e6672b3eb) merged
in c1c9d16c4 (no stream interface changed: system, readiness and volatile-bytes interface files equal).

## Terminal: the owner's ruling, not a question

The first run asked him how a terminal's size change reaches the kernel. He had already ruled on 2026-11-22
(orchestrator/open-with-owner.md:980-982, interface-changes.md:231-232): a terminal's size is not on the interface;
the window size travels inside the stream; a program inside the container holds the pseudo-terminal; no terminal is
in the kernel by default. The question is deleted, and with it the `process.consoleSize` workaround; nothing on this
branch expects a resize through the interface (the suite has no terminal check; docs/streams.md says the ruling).
For other tracks, not done here: the kernel's legacy terminal path (`Standard::Terminal`, `terminal_resized` from abi
`Event::TerminalResized` at crates/abi/host.rs:51) and the unicorn harness's `Step::Resize` and tests/terminal.rs are
the kernel track's to remove with the legacy events (KP2); who builds the program inside the container is not
assigned (the orchestrator's to place).

## Where things stand

- ST0 (stock): done. At main the system suite did not compile (E0283 at conformance.rs:42, the placeholder);
  kernel-linux 346 errors (its own track); system, readiness and readiness-conformance check and clippy clean.
- ST1 (the suite on main's rules): met but for linter findings that wait (below). The suite came from origin/agents
  198bb11d6 by path.
- ST2 (the stream contract checked): met but for the same findings. Seven stream checks and the readiness suite's
  six, on a loop with no limit and one holding a page (26 tests), and 15 broken loops each caught by its message.
- ST3 (this file and the plan): done.
- ST4 (the suite run on each implementation): waits on RT1 (seeded-test-policy, `LocalSystem`), KS and X3 (unicorn
  `TestHost`), R2 (`LinkHost`). The three runs are written as patches in
  pyro-parked/wcontainers-scratch/streams-restart/patches/, one per track (below).

## ST4 patches, one per track (2026-11-25, second run)

Each patch adds a `stream_tests!` run and needs this branch's suite merged first. "Applies" is `git apply --check`
exit 0 on that tip (MEASURED). No tip builds the patched crate yet, so each run was MEASURED on origin/agents 198bb11d6
with this branch's crates/system/conformance laid over it and the patch applied (it applies there too), on ARM64 and
x86-64 with the same results (the link host, an ARM64 program, on ARM64 only).

| Patch | Track, tip it applies to | Builds on the tip today | Run on agents 198bb11d6 |
|---|---|---|---|
| runtime-seeded-test-policy-stream-suite.patch: conformance_run.rs (the loop of agents, unchanged), `[[test]]` and dev-dependencies in Cargo.toml | runtime; no `runtime-restart` branch exists (MEASURED `git ls-remote`), so his main e6672b3eb, which RT1 is cut from, and kernel-port 4b872a8e9 | no: the policy is still on `&[u8]` streams (implementation.rs:233) and kernel-linux fails on main; RT1's | 7 passed, 6 failed (the loop's three faults, below) |
| native-link-host-stream-suite.patch: `mod stream_suite` in host.rs, a monitor whose one stream loops back through `answer`, dev-dependency | native, native-restart e708118da | no: wcontainers-arm64 thread_state.rs:46 is `unsafe` under `forbid` (MEASURED, exit 101); R2's | 1 passed, 12 failed (below) |
| executor-unicorn-test-host-stream-suite.patch: a `looped` client in streams.rs (gives back on descriptor 0 what descriptor 1 sends, closes 0 when 1 is let go), `loop_back` in driver.rs ending ready waits at once for that client, `mod stream_suite`, dev-dependency | executor (the unicorn folder; its harness files are the kernel track's), executor-restart 0ad228c01; does not apply to kernel-port, whose driver.rs differs | no: wcontainers-precise-instruction-set-unicorn fails with 10 errors (MEASURED x86-64, exit 101); X3's | 12 passed, 1 failed (below) |

The faults each run shows, with the owner and what fixing it would change for a guest:
- seeded-test-policy's loop (runtime; a test double, not a product; conformance_run.rs at agents), three faults as
  six failing tests:
  1. a receive from the empty queue gives `Ok(0)`: `a_receive_with_nothing_sent_would_block` (suite.rs:386,
     `left: Ok([])`, `right: Err(WouldBlock)`);
  2. the close of the sending endpoint is ignored: `closing_the_sending_endpoint_ends_the_stream_after_what_was_sent`
     (suite.rs:434, the report after the close holds READABLE but no `HANG_UP`) and
     `closing_the_sending_endpoint_with_nothing_sent_ends_the_stream` (suite.rs:128, "a wait for READABLE was never
     woken");
  3. it never reports a hang-up and never wakes a wait: readiness `a_wait_finishes_when_the_handle_becomes_ready`
     (readiness suite.rs:136), `many_waits_each_wake_once` (:229), `a_failure_ends_a_wait_that_did_not_ask_for_it`
     (:259).
  For a guest under the policy (INFERRED from ends.rs's contract): fault 1 makes the kernel read an empty stream as
  its end, so descriptor 0 hits end of file before the bytes come; faults 2 and 3 leave a reader blocked forever once
  the writer is gone, or before bytes arrive.
- `LinkHost` (native): the link carries no input and no wait (host.rs:152 `receive_stream` is `NotSupported`; its
  readiness answers WRITABLE at once and waits forever for anything else). Ten checks fail on `NotSupported` from a
  receive or a READABLE wait never woken, two readiness checks because a wait finished before it was made ready
  (readiness suite.rs:86); only `a_wait_does_not_finish_before_the_handle_is_ready` passes. For a guest on the native
  back end (INFERRED): descriptor 0 is at its end at once, as the kernel reads any error but `WouldBlock` as the
  other side letting go; `docker run -i` input would need a link request for input and a wait.
- the unicorn `TestHost` with the looped client (executor and kernel tracks): `many_waits_each_wake_once` fails
  (readiness suite.rs:229, "every wait woken"): `Readied` keeps one waker per endpoint number (tcp.rs:49), so a second
  wait on one endpoint takes the first one's place. A guest sees no change while the kernel waits on an endpoint once
  at a time (INFERRED from ends.rs); the harness breaks the readiness contract either way.

## Measurements

Second run, at 744a35467 (his main e6672b3eb merged, the readiness module's allow given its reason), both bars with
`--jobs 12`: ARM64 (load 16 at start, linter from reviewable-rust da55b9d7) and x86-64 (load 15 at start): check,
clippy `-D warnings` and fmt exit 0; `cargo test` 41 passed, 0 failed, 0 ignored; 0 `unsafe` lines; on ARM64
`cargo reviewable check` exit 1 with 59 findings in crates/system/conformance, the same 59 by code as below (18
`conformance_not_run`, 13 `name_unanswered`, 26 `summary_unanswered`). Logs: SCRATCH/streams-restart/run2, st4.

First run, at 1c188eaae (same tree as the commit that adds this file):

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
  gives no endpoint, so no check of the required shape can send or receive (the brief's question 2). Smallest
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
- The ST4 patches add no `unsafe` line and no outside dependency (only wcontainers-system-conformance as a
  dev-dependency, and wcontainers-volatile-bytes for the policy's test); clippy `-D warnings` exit 0 on the patched
  link host and unicorn crates on the agents tree.
