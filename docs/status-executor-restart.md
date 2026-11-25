# Executor restart: status

## X0 (2026-11-25, before this branch)

The inventory is in `wcontainers-scratch/executor-restart/x0-inventory.md`.

## X1: suite and simulation on the owner's interfaces (2026-11-25)

Branch `executor-restart`, cut from `instruction-port` at d97891aa7 (PORT HOLDS); `git merge d97891aa7` is already up to date.

- 6f5c8a84d: `executor/conformance` and `executor/implementation/simulation` come back by path from `origin/agents`
  198bb11d6, with `rename-crates/uncommitted/page-table-split.patch` applied (it applied cleanly).
  The port's hunks in those folders are re-applied: the suite takes the workspace's lints, needs no
  `processor-architecture-conformance` and compares `ExecutionState`s with `==` (`Simulator<Error = Infallible>` bound).
  `wasmtime` and `unicorn` were left as they are on the base: they wait on X2 and X3.
- f6c0154ad: the simulation is a `BehaviorSource` on `Arm64<User>` with the `MemorySink` hook and the port's
  `Exception` enum; the suite runs on `PageTable`, runs the behavior source's suite, and adds
  `page_table_changes_its_own_space_only`. The `GuestArchitecture` trait is gone: the guest tests are bounded by
  `Executor<Architecture = Arm64<User>>` and their guests are private functions. `aarch64-cpu` and `wcontainers-arm64`
  left both crates; the guests encode through the arm64 crate's `encoding` fields.
- 5777dfe19: lint fixes that need no ruling.

MEASURED on the ARM64 machine at 5777dfe19 (`cargo test -p wcontainers-executor-simulation`, `-j 12`, load 1.5):

| Run | Result |
|---|---|
| unit tests | 8 of 8 |
| executor suite | 17 of 22 (the 21 before, and the new table test) |
| behavior source suite | 3 of 3 |
| clippy `-D warnings`, rustfmt | clean |
| lines with `unsafe` | 0 in both crates, under the workspace's `forbid` |

The 5 failures are the instructions the arm64 list lacks: `svc` (both system call tests) and `sub` immediate with
`cbnz` (three counting tests); each guest stops with `UndefinedInstruction` at its first word.

Mutants, each run on the simulation and reverted:

| Mutant | Suite tests failing beyond the 5 |
|---|---|
| `copy_from` zeroes the bytes it copies | `copy_from_carries_bytes_permissions_and_mappings` |
| the table made with a space changes another space | 7, `page_table_changes_its_own_space_only` among them |
| `map_from` and `copy_from` read the table's own space, not the other table's | `copy_from_...`, `map_from_shares_the_bytes_both_ways` |

`cargo reviewable check` in the two crates: 156 findings. 137 are answers the model must give (55 names, 82
summaries), which a run without the key cannot record. The other 19 each need a ruling:

- `Simulation::new` (2): the executor interface has no constructor (open question 5).
- `SimulatedBytes` (3): it fills `MemorySpace::Bytes` and implements `VolatileBytes`, so the rules ask for pub fields
  and for a crate under `volatile-bytes`.
- The suite depends on `processor-architecture-arm64` (3): no interface can write a guest.
- `conformance_not_run` (11): the suite takes a set and a generator besides the executor, and so does the behavior
  source's suite; the run file names the implementations that give them; `PerThreadWatch` has no group arms, since
  its tests wait on the thread-test review.

## Level with the owner's main `95bceadf0` (2026-11-25, evening)

`e60bcaec2` merges his main (the expectation type, `simulate` and `simulate_state`, the `MemorySnapshot` view, no
closure `MemorySink`, `ProcessorArchitecture: Clone`, workspace lints everywhere). Conflicts were his `[lints]` lines
against the port's manifests (kept the port's, which have them) and his `interface.rs` and vector sink (taken whole:
every interface file on the branch is byte-identical to his main). `581bf41f5` and `b2a60a444`: the simulation passes
the snapshot view and sets the processor to what `simulate` returns; the suite checks with `meets_expectations`.

Blocked by other tracks' crates (MEASURED, `cargo check`, ARM64 machine): on this branch alone
`processor-architecture-arm64` (6 errors: no `ProcessorStateExpectation`, `meets_expectations`, `simulate`) and
`behavior-source-conformance` (3) do not compile, nor does `instruction-input-domain-arm64` (named iterator types).
They are the instruction port's. The simulation assumes ARM64's expectation is its whole `Processor`.

The bar, measured in a throwaway copy with the smallest follow of those three crates
(`executor-restart/follow-main/foreign-standin-follow.patch`: expectation = `Processor`, `meets_expectations` = `==`):
unit 8 of 8; executor suite 17 of 22 (the same 5); behavior source suite 4 of 4; the 5 broken copies each fail 1 to 3
more tests; clippy `-D warnings` and rustfmt clean; 0 `unsafe` in both crates; `cargo reviewable check` 158 findings
in the two crates (83 summaries and 55 names need the model; the 20 others are the rulings listed above, plus
`DroppedChanges`' summary and the run file naming the arm64 crate, both from the level-port commit).

X2's dependencies on this branch (MEASURED, `cargo check --lib`): `translator`, `guest-cpu`, `docker` compile;
`runtime-single-kernel` fails in `kernel-linux` (346 errors: unsafe in net.rs, general.rs and others, `VolatileBytes`
for `[u8]` in socket/outside.rs); `precise-instruction-set-translator` fails in `wcontainers-arm64` (thread_state.rs,
an unsafe block).
