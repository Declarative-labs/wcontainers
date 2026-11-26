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

## X1 on the branch itself, then X3 and X2 prepared (2026-11-25, night)

`34a70693e` merges `origin/arm64-instructions` d708c4a55 (one conflict, the suite's manifest: kept ours, which has the
lines theirs lacks); `93db97e5f` merges his main 1b6b5d6dc (the processor core interface, no conflict). All 98 files
in his 29 interface folders are byte-identical to his main (MEASURED, `git diff --stat 1b6b5d6dc HEAD`; the 30th
`interface.rs`, `crates/kvm`, is the port's and not on his main).

ARM64's expectation (READ: arm64 implementation.rs:153-194, simulation.rs:30-55): the processor a behavior leaves,
with each general-purpose register and NZCV as `Deterministic(value)` or `NonDeterministic`; only RNDR-like reads
leave one undecided. `c6ebee1af`:

- The simulation is `Simulation<Set, Random: RandomSource>`; each `NonDeterministic` register takes a word of the
  random source, NZCV the flags of a word's top half. Its error is `SimulationError { NoKnownBehavior,
  RandomSourceFailed(HostError) }`; `execute` panics on either (the trap handler has no answer for them).
- A supervisor call (new in the arm64 crate) goes to `handle_system_call` (x8, x0 to x5); `Return` puts the value in
  x0 with the program counter already after the call. The suite's "terminate stops at the call" now expects the
  architecture's stop: `SupervisorCall { immediate }`, the program counter after the call. UNICORN AND WASMTIME
  MUST FOLLOW THIS when they come onto the interfaces.
- A word several encodings match (CFINV and MSR immediate) runs as the one fixing most bits, first in list order on
  a tie; the encodings are sorted once in `new`.
- The executor suite skips the 4 instructions the architecture knows no behavior of (Hint, MsrImmediate, Sys, Sysl).
- The tests' set is the first known behavior of each instruction (`testing.rs`), with the arm64 track's
  `CriticalValueCut`: no recording measures the full list (4,430 instructions, 723 with several behaviors, 10
  recorded; MEASURED, probe on the machine), and measuring needs a source for every instruction.

MEASURED, Graviton4 (logs: `executor-restart/x1-merge/run4.log`, load 16 to 49):

| Run | Result |
|---|---|
| unit tests | 9 of 9 (new: undecided values come from the random source, two seeds differ) |
| executor suite | 22 of 22 (svc, sub immediate and cbnz now listed) |
| behavior source suite | 3 of 4: `measured_behavior_agrees_with_the_source` fails at suite.rs:90 for any source, since `measure_instructions` stops at the first of the 4 instructions with no known behavior |
| suite time | 246 s (1,135 s before the encodings were cached) |
| mutants of memory.rs, fast tests | 7 of 10 caught; `copy_from` zeroing (m01) and the other table's space (m04) both fail |
| clippy `-D warnings`, rustfmt | clean |
| `unsafe` | 0 in both crates |
| `cargo reviewable check` (reviewable-rust main da55b9d7, no key) | 169 in the two crates: 151 need the model; 18 are the rulings listed earlier plus `testing.rs`'s test random source (implementation_folder) |

`ff19a3a2c`: `unicorn` and `wasmtime` come back by path from `origin/agents` 198bb11d6 (his main had not changed
them since), each on `[lints] workspace = true`; `unicorn/conformance.rs` and `wasmtime/conformance.rs` go (agents
names them `conformance_run.rs`). Neither builds (MEASURED, `cargo check --lib`): `wcontainers-arm64`
thread_state.rs has an `unsafe` block, and `wcontainers-kernel-linux` has 346 errors.

X3 left (stand-in, my files): port `StandIn` to today's `Executor: BehaviorSource`, `MemorySnapshot` and
`MemorySink`, `create_guest_memory` giving `(UnicornSpace, page table)` with `set_permissions` moved to a page table;
the system call stop above; the 4 marked lines in standin.rs: `unsafe impl Send for Memory` (225, unicorn's handle
is `Rc`: each engine on one host thread, which changes how the kernel's harness holds the stand-in), `from_raw_parts_mut`
(423) and `from_raw_parts` (1456) to `mem_read`/`mem_write` and blocks in owned bytes, `mem_map_ptr` (516) to
`mem_map` with shared blocks and space.rs's shared host pages copied in before and out after each run. Then suite
and the LTP verdicts. Waits on KP1 and TR1 (`wcontainers-arm64`) to build, RT1 for `Execution`.

X2 left: 33 lines (wasmtime, raw): the deserialize cache (311), the custom memory (memory.rs, 20 lines), the store
data's erased lifetimes (1818, 1829, 2383), and fork in the tests (3097, tests/fork.rs: rustix has no fork, a stop
item). Waits on TR1 and RT1 to build.

## His main `ac696ea19`, the kernel and translator ports, X1 confirmed, X3's executor part (2026-11-26)

Merges (normal, no conflict, MEASURED `git merge`): `77391e4f1` his main `ac696ea19` (`prepare_execution`,
`Execution::run`, named iterators, `MemoryReader`, observation collector), `818181556` `origin/kernel-port`
`e177511f5`, `e829931b8` `origin/translator-port` `15968cb89`; his main again: already up to date.
`origin/arm64-instructions` is still at `d708c4a55`, merged before. After the merges, 91 files beside the
`interface.rs` of his 29 interface folders are byte-identical to his main (MEASURED, `git diff --quiet` on each).

- `ca24485a4`: the random-source implementations take the interface's `&impl VolatileBytes` (they still took
  `&mut [u8]`): `host` and `seeded` fill through `write_from` with a `ByteSource` of their generator, and fail if
  fewer bytes were taken; the suite fills `RefCell<&mut [u8]>`. The simulation's tests take `seeded` (ChaCha8 from
  a seed) instead of their own SplitMix64.
- `481a558ef`: the simulation and the suite on `prepare_execution` and `Execution::run`: `run` is a loop of
  `run_until_trap` (instructions until an exception or an interrupt), then the handler's answer. The simulation
  never waits (`Wait = Ready<()>`, never made). The suite's helper prepares again after a `Waiting` from the state
  it stopped at and hands each new interrupter to the interrupting thread. `interrupt_before_the_first_poll_...`
  is now `interrupt_before_the_run_is_answered_at_the_start`.
- `ccd4db9da` and the commit after it: X3's executor part, below.

### X1 (MEASURED, Graviton4, `-j 12`)

| Run | Result |
|---|---|
| random-source `host`, `seeded` suites | 4 of 4 each |
| simulation unit tests | 9 of 9 |
| executor suite on the simulation | 22 of 22 |
| behavior source suite on the simulation | 3 of 4 |
| clippy `-D warnings` (all targets), rustfmt `--check`, five crates | clean |
| `unsafe` | 0 in all five |

The failing test is `measured_behavior_agrees_with_the_source` (suite.rs:90): `measure_instructions` stops at
the instructions the architecture knows no behavior of. A probe measuring every instruction one by one
(simulation, `CriticalValueCut`, 48 s): 4,430 instructions, 4,426 measure, 4 have no known behavior (`Hint`,
`MsrImmediate`, `Sys`, `Sysl`), 0 fail otherwise. That is what remains on `origin/arm64-instructions`.

Planted wrong implementations, each applied to the simulation alone, every suite test but the two slow ones:

| Wrong implementation | Fails |
|---|---|
| `copy_from` zeroes the bytes it copies | `copy_from_carries_bytes_permissions_and_mappings` |
| `copy_from` drops permissions | `copy_from_carries_bytes_permissions_and_mappings` |
| `copy_from` copies a mapped page instead of sharing it | `copy_from_carries_bytes_permissions_and_mappings` |
| `map_from` reads its own space | 2 unit tests; `copy_from_...`, `map_from_shares_the_bytes_both_ways` |
| `copy_from` reads its own space | `copy_from_carries_bytes_permissions_and_mappings` |
| a table made with a space changes another space | 2 unit tests; `copy_from_...`, `map_from_...`, `permissions_bind_guest_code_and_not_borrowed_bytes` |
| a retry keeps the faulted state | 1 unit test (the suite passes: the simulation runs the faulting instruction again anyway) |
| a system call's result is dropped | `system_call_answered_return_goes_on_with_the_value` |
| an interrupt is never answered | 2 unit tests (the suite run hit its 900 s timeout) |
| an undecided register is a constant | `undecided_values_come_from_the_random_source` |

X1 holds by this measurement, except that the behavior source suite fails until the four instructions have a behavior.

### X3, the stand-in's executor (MEASURED unless marked)

- The stand-in implements today's `BehaviorSource` (`Arm64<User>`, `Error = Unsupported`) and `Executor`
  (`create_guest_memory` gives `UnicornSpace` and `UnicornPageTable`; `prepare_execution` gives
  `StandInExecution`, whose `run` is the same "run until the next trap, then ask the handler" loop). The impl of
  the runtime's `Execution` at standin.rs:932 is gone.
- 0 lines with `unsafe` in its files (8 raw `unsafe` words before in standin.rs; the 4 marked lines go):
  `unsafe impl Send for Memory` goes (a stand-in and its spaces now stay on the thread that made them);
  `mem_map_ptr`, `from_raw_parts_mut` and `from_raw_parts` go: unicorn owns the bytes of the pages it maps
  (`mem_map`, `mem_read`, `mem_write`), host memory holds the rest, and shared pages (`Rc<SharedPages>`) are copied
  into unicorn before a turn when they changed and given back after it, page by page where they differ. The shared
  block pointers, and with them the use after free, are gone: a dropped block a memory shows lives on through its
  `Rc` (still recorded as `Refused`). New file `memory.rs`.
- The kernel test harness is behind a default feature `harness`, so the stand-in builds and is measured
  without it: `cargo test -p wcontainers-executor-unicorn --no-default-features --lib --test conformance`.
- Results: unit tests 6 of 6 (2 new: a dropped block keeps its bytes; two memories share a block's bytes, the
  guest's writes among them); executor suite 21 of 22 and behavior source suite 1 of 4 (rerun after the fix: unit 6 of 6,
  the 21 fast suite tests 21 of 21; clippy `-D warnings` and rustfmt clean):
  - `agrees_with_precise_instruction_set` (first known behaviors, `CriticalValueCut`, all 4,430 instructions):
    the process aborts in QEMU (`disas_simd_three_reg_same_fp16: insn 0xec01dcd`, SIGABRT) partway through.
  - A probe over the first 2,962 instructions (before the abort): 295 instructions where the stand-in's state
    meets no known behavior. After the fix below, 283; their examples show unicorn runs at EL1: `ERET`, `MRS`/`MSR`
    of EL1 registers and the pointer-authentication branches run where EL0 has them undefined, and `BR` to
    `0x8000000000000000` leaves that program counter where the architecture expects 0. `MRS` of the counter gives
    different values in two runs (164 states), which no known behavior allows; 2 `MSR` states stop with QEMU
    exception 3 (`UnknownStop`).
  - Fixed (in the commit after `ccd4db9da`): a counted turn stops as unicorn fetches the next instruction, so a
    fetch fault at another address than the instruction's belongs to the next one; the instruction completed.
  - `measured_behavior_agrees_with_the_source`, `execution_is_deterministic` and
    `execute_agrees_with_execute_in_place` fail on the same disagreements and the counter.
- Speed (INFERRED, not measured): every turn compares the writable pages of each shared range it maps; a large
  shared mapping costs that much per turn.

X3 is not reached: the instruction agreement over today's full list fails as above (it passed 21 of 21 on the old
ten-instruction list), and the LTP verdicts wait on the kernel track's harness, which does not build (with the
default feature, MEASURED: 22 errors, all in `runtime-single-kernel`; and the harness still drives `next`, `exit`, `event` and
`&mut dyn Cpu`). For the harness: the stand-in is no longer `Send`; `StandIn::run`, `run_on`, `restored`,
`take_refused`, `Refused`, `Unsupported` and the `SPACE_*` constants are kept.

X2 has not started: wasmtime's `CpuSide` implements the runtime's own `Execution` and `Executor`
(`wcontainers_runtime_single_kernel::{Answer, Carry, Execution, Executor, Leaving, RuntimeConfig, StopFlag,
Turn}`, implementation.rs:44), and `runtime-single-kernel` does not compile on this branch (MEASURED: 6 `unsafe`
blocks, `next`/`exit` private, no `event`, no `FallbackObserver` for `LocalSystem`, `send`/`receive` type
parameters). It waits on RT1, or on the runtime track moving those traits.

SECURITY: lines with `unsafe` (raw word, `grep -rn unsafe *.rs`), before (`0ad228c01`) and after: unicorn 34 to 26
(the stand-in's files 8 to 0; the 26 are the harness's: `libc::fork` in driver.rs and the generator's text
handling); wasmtime 62 to 62; simulation 0, conformance 0, random-source 0. No marked exception added, no UNSOUND
mark. Dependencies: unicorn drops `atomic_refcell`; the harness's dependencies become optional; dev-dependencies
`wcontainers-precise-instruction-set(-measured)` and `wcontainers-distinguishing-state-generator-arm64` replace the
input-domain ones; simulation adds dev-dependency `wcontainers-random-source-seeded`; the random-source
implementations add `wcontainers-volatile-bytes` and `wcontainers-byte-source`. No outside crate added. The
hostile-guest suite was not run.
