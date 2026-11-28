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

## X3 continued: the stand-in at EL0 over today's whole list (2026-11-26)

Merges (normal, no conflict but the lock file, MEASURED `git merge`): `d355cb977` and `b44485e28`
`origin/arm64-instructions` 44e944a2f then f7f80cae6 (SVE, Advanced SIMD, the rest family); `7fe9b48b1`
`origin/translator-port` 659f657d2; `82e225ca2` `origin/runtime-restart` 93637d42e; his main `ac696ea19` was
already in. Every file of his 29 interface folders is byte-identical to his main (MEASURED, `git diff --quiet` on
each file at the top of each folder). `19515bf25`: the lock file as cargo writes it after the merges.

What changed in the stand-in (`218b82d95`, `f159f4e59`, `c84cbab9e`):

- The guest runs at EL0 under Linux's controls: PSTATE EL0t, SCTLR_EL1 UCI, UCT, DZE set and nTWI clear,
  CPACR_EL1 FPEN and ZEN open, CPTR_EL3 EZ set (unicorn's model has EL3, which traps SVE otherwise), TCR_EL1 T0SZ 16
  and TBI0. Written once per unicorn instance, kept in the context each turn restores.
- The predicate registers and FFR reach the processor state: unicorn's interface has none, so a single
  instruction runs between helper code on a scratch page after the state's pages (LDR/STR predicate, RDFFR,
  WRFFR), the page allowing everything for the helpers and nothing for the instruction. A whole execution keeps
  the predicates it started with (no page to pass them through).
- A stop QEMU raises as undefined: WFI is `WaitTrapped` (nTWI clear), any other system-class word is refused as
  `Unsupported::TrapOrUndefined` (unicorn gives no syndrome to tell a trap from undefined), anything else is
  `UndefinedInstruction`. The unallocated FP16 three-same words QEMU's decoder asserts on (one of them ended the
  last run, `0x0ec01dcd`) are refused as `EmulatorAborts` before unicorn sees them. A data access to a tagged
  address whose untagged page allows it is refused as `TaggedAccess`, a branch to a tagged address as
  `TaggedBranch`. A fault status reads the untagged address. Every exception but a supervisor call leaves the
  program counter on its instruction (unicorn left it after a faulting SVE store).
- The stand-in may move between threads (the kernel's bounds: `C: Send`, `GuestMemory: Send`): every unicorn
  instance of a stand-in lives on one host thread of its own (`engine.rs`), the stand-in, its spaces and tables
  are handles that send each call there. A table only maps or copies from a space of the same engine. Single
  instructions reuse one cleared memory per engine. `PerThreadWatch` is implemented (refuses: the kernel watches
  one thread itself before each turn, executing.rs:686-697).
- The executor suite's agreement test (`crates/executor/conformance/suite.rs`): a state the executor gives an
  error for is a refusal, counted and listed on the standard error per instruction, not compared; instructions
  with no known behavior are listed as not compared; the test fails if no state executes. It shares the
  instructions among 12 executors on their own threads (`AGREEMENT_EXECUTORS`). The stand-in's run takes as its
  set the first known behavior every executed state meets (the chip picks one), measured the same way in
  parallel, and names two refused states for the error-recovery test.
- The kernel harness no longer pulls in wasmtime without the `runtime` feature (`5bfd6d5ad`).

MEASURED, Graviton4, `-j 12` (logs: `executor-restart/x3-finish/`):

| Run | Result |
|---|---|
| probe, every listed instruction, one by one (`probe1-out.txt`, 1,406 s) | 4,430 listed, 4,426 with a behavior, 25,783,206 states, no host abort |
| stand-in unit tests | 7 of 7 (new: the stand-in moves to another thread) |
| executor suite on the stand-in, all but the behavior source module | 21 of 22 (the agreement test fails) |
| agreement test (`bar2-unicorn-tests.txt`, 46 min) | 24,505,841 states executed, 1,277,365 refused in 707 instructions, 509 instructions disagree |
| executor suite on the simulation | 25 of 26: `measured_behavior_agrees_with_the_source` (the 4 instructions with no behavior) |
| rustfmt `--check`, clippy `-D warnings` (stand-in lib and suite; simulation and suite all targets) | clean |

What the 509 disagreements are (READ from the probe's first diff of each):

- 209 instructions, one state each, the list knows only "undefined" (`Unimplemented`, no model yet) and unicorn
  runs them: SVE dot products, indexed FMLA/FMUL/FCMLA, FADDV and the other reductions, FTMAD, FRECPE, the gathers
  and scatters, LDFF1 and LDNF1, ADR, the SM3 and SM4 instructions. They wait on the arm64 track's models.
- Tagged addresses: the model applies TBI0 to data and branches; unicorn faults on a tagged data address even with
  TBI0 written (bar 1 without it and bar 2 with it gave the same counts), so those states are refused
  (`TaggedAccess`, most of the 1.28 million) and the rest disagree (SVE loads and stores at a tagged base,
  LDR/STR predicate and vector). Open: why QEMU's TBI does not apply here (MMU off is my guess, INFERRED).
- Exclusive stores (STXR, STLXR, STXP, STLXP: 10) to a misaligned address: unicorn completes and fails the
  store, the model expects an alignment fault.
- Chip differences of unicorn's `max` model: MSR FPCR keeps more bits than a Neoverse V2 (1,440 states); MRS of
  the cache type reads 0x8444c004, which MRS's two behaviors do not list (83 states, only `MrsCtrEl0` has it);
  FCVTZS (scalar, fixed point, half) writes 32 bits where the model writes 16; FCMLA by element sets IDC in FPSR;
  FJCVTZS sets Z where the model leaves NZCV 0; XPACI and XPACD strip at bit 55 differently (6 states each).
- The refusals besides tags: MRS 124,014 and MSR 100,077 states (EL1 traps them, unicorn says undefined), WFET,
  WFIT, DSB nXS, the hint space (1 each), FAMAX, FAMIN and FSCALE half (the decoder assertion, 1 each).

Four mandatory instructions still have no known behavior on `origin/arm64-instructions` f7f80cae6: `Hint`,
`MsrImmediate`, `Sys`, `Sysl` (MEASURED: the probe's NOBEHAVIOR lines).

X3 IS NOT REACHED: the agreement test fails on the 509 instructions above, and the LTP verdicts wait on the
kernel track's harness, which does not build (MEASURED, `cargo check -p wcontainers-executor-unicorn --lib` with a
throwaway fill of the kernel's two new processor fields: 86 errors, in driver.rs, hooks.rs, ltp.rs, layer1.rs,
replay.rs and the kernel's own system.rs and container.rs: the old drive `next`/`exit`/`event`, `&mut dyn Cpu`
where the kernel now takes `&mut GuestMachine`, the `Hooks` methods' new type parameter, the runtime's removed
`drive`, `Ending` and `System` methods, `FallbackObserver` for `TestHost`). One of them is the kernel's own:
`running.rs:203` builds `Processor` without `predicate_registers` and `first_fault_register` since the arm64
merge (this branch's kernel crate does not compile, MEASURED). For the kernel track's harness, from the stand-in:
it is `Send` again, its handles and `Unsupported` variants are above; `StandIn::run`, `run_on` (asserts the space
is the stand-in's), `restored` (same engine), `take_refused`, `Refused` and `SPACE_*` are kept.

## X2: not started (what it needs, READ)

The wasmtime executor implements the runtime's own `Execution` and `Executor`, which `runtime-restart` keeps
unused (its status, RT1). After the merges its lib fails with 76 errors (MEASURED, with the kernel field filled):
19 `expect(unsafe_code)` under the workspace's `forbid`, 14 unsafe blocks, 4 unsafe impls, and the old
`Executor`/`MemorySpace`/`Processor` shapes. Plan for the trap path without unsafe and without leaving the
translated code: wasmtime's `async` feature runs the guest on a fiber (`Func::call_async`), each trap hook is a
`Func::wrap_async` whose future hands the trap to `run`'s loop and waits for the answer, so the handler is asked
from inside the run while the guest's stack stays on the fiber, and the store's data owns its state (no erased
lifetimes, implementation.rs:1812-1829). The custom memory (memory.rs, 39 lines) and the deserialize cache
(implementation.rs:308) go; fork in the tests (3094) stays a stop item. Not measured yet.

SECURITY: lines with `unsafe` (`git grep -n unsafe`, raw word) before (`62d137472`) and after (`c84cbab9e`):
unicorn 26 to 26 (all in the harness files: `libc::fork` in driver.rs, the generator's text handling; the
stand-in's files 0 to 0), wasmtime 62 to 62, simulation 0, conformance 0. No marked exception added, no UNSOUND
mark, no lint lowered, every crate keeps `[lints] workspace = true`. Dependencies: unicorn adds dev-dependency
`wcontainers-distinguishing-state-generator` (workspace); no outside crate added. The hostile-guest suite was
not run (none exists; the thread-test design review is owed to the owner first).

## X3 continued: the four waiting items, the tag cause, the set-aside list (2026-11-26, afternoon)

His main `ac696ea19` was already merged (MEASURED, `git merge-base --is-ancestor`). `2b552c0b4` merges
`origin/arm64-instructions` `26a3d7ce0`; all 77 files at the top of his interface folders are byte-identical to his
main after it (MEASURED, `git diff --quiet` each).

Which commit carries which (all pushed to `origin/executor-restart`):

| Item | Commit |
|---|---|
| (1) failing test first: `two_views_of_one_page_show_each_store` in the executor suite | `fca4d2f8f` |
| (1) the repair: stand-in `memory.rs` and the simulation's `memory.rs` | `79ddc9200` |
| (2) `executor_and_its_memory_move_between_threads`: bounds `Chip: Send`, `GuestMemory: Send`, `PageTable: Send`, `Interrupter: Send` fail the build, then a guest runs on another thread | `490b89187` |
| (3) `PerThreadWatch` on the stand-in (refuses; the kernel watches one thread itself) | `218b82d95`, pushed before this run |
| (4) `refused_in_a_copy` removed | `dd61eb7cb` |

(1) MEASURED before the repair (`x3-shared/run1-before-fix.txt`): the new test failed on the stand-in AND on the
simulation (x3 read the pattern, not the stored value, with no trap between). Causes: the stand-in copies shared pages
into each unicorn instance per turn, so a second view in the same space was stale within a turn, and after a turn it
was marked current anyway; the simulation's copy of a space followed a write at one address only. Repairs: a unicorn
store hook, present only while a space has two views of the same shared bytes, copies each store to the other views
(unicorn's write also drops code translated from them); a view whose bytes another view wrote is copied in again
before the next turn; the simulation takes a new copy after a write to a frame another page of the space shows.
After: stand-in unit 7 of 7, fast suite 22 of 22; simulation 9 of 9 and 22 of 22 (`run2-after-fix.txt`).

(2) READ: unicorn-engine 2.1.5's `Unicorn` holds `Rc<UnsafeCell<..>>` (lib.rs:163), so neither `Arc` nor a lock
can make it `Send` without unsafe. The stand-in already is `Send` the safe way (since `218b82d95`): every unicorn
instance lives on one engine thread and the stand-in, its spaces and tables are handles. The kernel-port branch still
has the earlier `Rc` stand-in (`ccd4db9da`, MEASURED `git merge-base`); merging this branch brings the `Send` one.

### The tagged data address fault: the cause (MEASURED, a probe in the stand-in's memory tests, not kept)

TCR_EL1 read back 0x2000000010 (TBI0 set), yet `LDR x1, [x0]` with x0 = 0x0100_0000_0001_1000 faulted at the tagged
address. Turning the MMU on with an identity map gave prefetch aborts on every fetch until SCR_EL3.RW was set:
unicorn starts with SCR_EL3 = 0, so QEMU takes EL1&0 as an AArch32 regime and its MMU-off path skips TBI. With RW set
(MMU still off) the tagged loads at tags 0x01 and 0xFF returned the stored value. The stand-in now sets SCR_EL3.RW
(`8f95ed70b`). Two effects of QEMU's own translation are handled there too: an address past its 44 physical bits
raises an abort with no address, and a fault's address loses its tag; so a turn that faults runs its instruction once
more with unicorn's virtual TLB (`add_tlb_hook`), which gives a refusing page a page of its own that nothing maps, and
the fault hook names the guest's address. Single instructions rerun from their saved state; a long turn reruns in
place. The `TaggedAccess` and `TaggedBranch` refusals are gone.

The 27 first-fault gathers of elements wider than a byte end the whole process in unicorn (SIGSEGV; MEASURED, each
instruction alone, `x3-shared/firstfault-crash.txt`); the stand-in refuses their words as `EmulatorAborts` before
unicorn runs them (single instructions only; a whole execution reaching one still crashes).

### The agreement test over today's list (MEASURED, Graviton4, release, `x3-shared/run11-suite.txt`)

| Run | Result |
|---|---|
| probe, every listed instruction (`probe7-out.txt`, 1,893 s) | 4,430 listed, 4,430 with a behavior (the 4 mandatory ones have theirs now), 33,324,598 states, 798,029 refused in 39 instructions (EmulatorAborts 456,465, TrapOrUndefined 341,564), 0 tag refusals (1,277,365 before), 353 instructions no single known behavior explains (813 before the tag-address work, 509 at the last run) |
| chip, the same 353 (`chip1.txt` to `chip3.txt`, the measured set's `default_behaviors` on KVM) | every modeled one: 0 states disagree with the model; SM3 and SM4: undefined on the chip |
| stand-in suite without the behavior source module (`--release`) | unit 7 of 7; suite 24 of 24 |
| agreement test | 32,526,569 states executed; 798,029 refused, of 39 instructions; 353 set aside; 0 instructions disagree |
| simulation suite without the behavior source module (`--release`) | unit 9 of 9; suite 24 of 24 |
| clippy `-D warnings` (stand-in lib and suite; simulation and suite all targets), rustfmt | clean |

The set-aside list is in `unicorn/conformance_run.rs`: 353 instructions by name in 13 families, each with its
measurement; `SET_ASIDE_COUNT` is 353 and the list's type holds that length; the suite fails on a set-aside
instruction that agrees. Families: vector loads across a fault 79, first-fault loads 16, SVE stores across a fault
66, pointer strip 3, SYS 1, MSR/MRS 2, misaligned atomics 124, exclusive stores 10, exclusive loads 8, ordered
accesses 24, signed byte atomics 8, floating point 3, SM3/SM4 9. The suite takes the list as an optional fifth value of
`tests!`; the simulation passes none.

Not run: the behavior source module on the stand-in (each test measures the whole list on one thread, hours).

For the ARM64 instruction track (MEASURED, debug build): `sve_remaining_elements.rs:21` panics with "attempt to negate
with overflow" (`-(1_i64 << 63)` for a 64-bit element); the release runs above do not check overflow.

X3 IS NOT REACHED: the agreement holds by the list above, but the bar also needs the kernel track's harness to give
LTP 4,169 of 4,226 on this stand-in, and that harness lives on `kernel-port` (it does not build on this branch). Next:
X3's LTP part, waiting on the kernel track merging this branch into `kernel-port` and running its harness.

SECURITY: lines with `unsafe` (`git grep -n unsafe`, raw word) before (`9aaaf77a3`) and after (`b6256aaf4`): unicorn
29 to 29 (all in the harness's files: driver.rs 8, generate.rs 8, ltp.rs 8, backend 5; the stand-in's files 0 to 0),
wasmtime 62 to 62 (untouched, X2 is another agent's), simulation 0, conformance 0. No marked exception added, no
UNSOUND mark, no lint lowered, every crate keeps `[lints] workspace = true`. No dependency added. The hostile-guest
suite was not run (none exists; the thread-test design review is owed to the owner first).

## X3 repair, the two rulings, and the kernel track's stand-in failures (2026-11-26, evening)

His main is still `ac696ea19` (MEASURED: `git fetch` of his checkout, FETCH_HEAD an ancestor of the branch). After the
merge below, all 100 files at the top of his 30 interface folders are byte-identical to his main (MEASURED,
`git diff --quiet ac696ea19` on each). The two `interface.rs` not on his main (`crates/kvm`, `crates/linux-socket-options`)
are other tracks'.

Which commit carries which (all pushed to `origin/executor-restart`):

| Item | Commit |
|---|---|
| `StandIn::refused_in_a_copy` put back (origin's `kernel-port` driver.rs:1717 calls it; `dd61eb7cb` was wrong) | `914b4e9d1` |
| merge of `origin/kernel-port` `406fba566` (normal, no conflict) | `1d777839b` |
| time slice: the stand-in and the simulation end an execution's slice with `handle_interrupt`; suite test | `30ca724a8` |
| the guest's counter read traps on the stand-in (CNTKCTL_EL1 = 0); a trapped MRS/MSR ends the run as the list names it | `c4113f822` |
| code the kernel wrote over is what runs next (runners codewrite) | `af5897634` |
| view stores no longer written into views the guest may not store to (host SIGSEGV in privatemap) | `20d552cdf` |

MEASURED: with `1d777839b`, `cargo test --no-run -p wcontainers-executor-unicorn --lib --bins --tests` (the kernel's
harness, default features) exits 0 on ARM64: the branch merged with `kernel-port` builds again.

### Time slice (ruling: no interface change)

- `StandInConfig { slice_instructions }` and `SimulationConfig { slice_instructions }`, set as the executor is built
  (`StandIn::configured`, `Simulation::configured`), default `DEFAULT_SLICE_INSTRUCTIONS` = 1,000,000. Not
  `TranslatorConfig`: the kernel's `ExecutionConfig { translator: () }` is written in the harness (driver.rs:1416,
  the kernel track's file) and nothing hands it to an executor, so it stays `()`.
- The stand-in passes what is left of the slice to unicorn's exact instruction count; a stop with no interrupt raised is
  the slice's end and calls `handle_interrupt`, as an interrupt from outside does. The simulation counts each instruction.
- Suite: `a_guest_that_loops_forever_is_interrupted_by_its_slice` (a count-down from `u64::MAX`, no interrupter called;
  fails if the handler is not asked within `SLICE_BOUND_INSTRUCTIONS` = 2^26, or not within 600 s). Every executor runs it
  through `tests!`; the wasmtime executor (`executor-wasmtime`) must pass it too. `interrupt_after_the_end_does_nothing`
  now asks that both executions get the same number of interrupts (their own slices), not zero.
- MEASURED (ARM64, release): stand-in unit 8 of 8, suite 24 of 24 without the agreement and behavior source modules;
  simulation unit 9 of 9, suite 24 of 24; rustfmt and clippy `-D warnings` clean (stand-in lib and suite, the whole
  harness with all targets, simulation and suite all targets).
- The default, by the kernel suite's bounded targets (MEASURED, two full copies, slice 1,000,000 and 100,000, three
  targets at a time, 2,400 s each): cpulimit, cputime and deadlines reach the bound at BOTH slices (exit 124 after
  2,400 s, 100% CPU). Cause (READ): the kernel's runner goes on to the next turn after a slice's end without returning
  (executing.rs:585-586, "one for a slice's end goes on to the next turn"), and the harness's repeatable clock moves only
  when the runner returns (driver.rs:1762-1766, `arrived_live`), so no deadline or CPU limit ever comes. That is the
  kernel track's: either the runner returns `Interrupted` at a slice's end, or the clock moves by the slice. The other
  targets: see the table below. 1,000,000 stays the default: no measurement told the two apart.

### The guest's counter (ruling: no interface change)

- The stand-in writes CNTKCTL_EL1 = 0 (EL0PCTEN and EL0VCTEN clear), so QEMU traps a read of CNTVCT_EL0, CNTPCT_EL0 or
  CNTFRQ_EL0 at EL0. Unicorn gives no syndrome, so an MRS or MSR (register) QEMU raises as undefined is classified by the
  arm64 list's own `simulate_system_register_move`: `SystemAccessTrapped { op0 3, op1 3, crn 14, crm 0, op2 2, rt, Read }`
  for the virtual counter, `UndefinedInstruction` where the list says undefined, a refusal where the list lets it complete.
- The list already lists CNTFRQ/CNTPCT/CNTVCT_EL0 as `ReadTrapped` (READ: system_register.rs:163-166), so nothing in the
  instruction list changes.
- New unit test `a_counter_read_traps` (MEASURED: passes).
- FOR THE KERNEL TRACK: counterclock still fails (MEASURED: 0 of 2), now in the kernel: running.rs:172 panics,
  "an execution ended on an exception no exit names: SystemAccessTrapped(..)". The kernel must answer that exception
  by emulating the read from its clock (write Xt, step the program counter) as the ruling says.

### Code written over (runners codewrite)

Unicorn's `mem_write` keeps the blocks it translated from the bytes written. Every write into unicorn's pages
(the kernel's writes, own pages copied back on remapping, shared pages copied in) now drops those translations
(`ctl_remove_cache`). MEASURED: runners 4 of 4 (was 3 of 4).

### Views (privatemap's host SIGSEGV, found on the way)

MEASURED: the privatemap test process died with SIGSEGV in unicorn's `store_memop`, writing one byte below the page
stored to (gdb). Also at `1d777839b`, so older than today's work. Cause: the store hook copied a store into another view
with `mem_write`; to a region that refuses stores unicorn makes the region writable and back, which rebuilds its memory map
and empties the TLB while the store still holds an entry. A version that stopped unicorn after the store to copy outside
the hook spent minutes rebuilding the memory map at each store (MEASURED, gdb: `flatview_update`). Now the hook copies only
into views the guest may store to; a read-only view takes the bytes before the next turn, as `copy_shared_out` already
marks it stale. MEASURED: privatemap 1 of 1, sharedviews 3 of 3, suite `two_views_of_one_page_show_each_store` passes.
INFERRED limit: a read through a read-only view of bytes stored through a writable view of the same process in the same
turn, with no trap between, sees the old bytes.

### LTP's hangs and the python SIGSEGVs: the cause is the kernel's (FOR THE KERNEL TRACK)

Found with getrandom01 on `kernel-port`'s harness (a throwaway trace in a scratch copy, not committed): after its TPASS
lines the test's child waits on a private futex at 0xf7a122a8 with value 2 (glibc's lock), and later its parent waits on
the same word. The stand-in showed that page in BOTH processes as the same shared block, libc's file at offset 0x1b2000,
readable and writable. ld.so maps libc's text from the file (0xf7860000, 0x1be010 bytes), then its bss over the text's tail
with `mmap(0xf7a12000, .., MAP_FIXED | MAP_ANONYMOUS)`. The kernel's `AddressMap::insert` (memory.rs:1479-1486) removes the
old file mapping but never calls `unmap_shared` for it, as `unmap` does (memory.rs:1513); `clear_used` then calls
`MemorySpace::zero`, which by the interface only writes zeros (through the mapping, into libc's page cache), and the bss
page stays a view of libc's file. So every process's bss page there is one page: locks, malloc state and libc's file
content are shared across processes (INFERRED: also the python SIGSEGVs). The old `Cpu::zero` ended sharing; the
interface's `zero` does not.

The fix, for the kernel track (scratch `executor-restart/x3-ltp/kernel-insert-unmaps-file-pages.patch`, 6 lines): in
`insert`, call `cpu.unmap_shared(memory, old.start, old.end - old.start)` for each removed `Backing::File` mapping
before `clear_used`. MEASURED with it applied in a scratch copy of this branch:

| Run | Without (kernel-port's own run) | With the 6 lines |
|---|---|---|
| LTP, the kernel track's 30 | 6 match, 24 hang | 30 of 30 verdicts match the reference |
| sharedmap | 1 of 2 | 2 of 2 |
| python, one seed (`SCHEDULE_SEEDS=1`) | 0 of 3 (exit 139 at seed 1 of each, MEASURED on `c4113f822` with the code-write fix, three seeds asked) | system program and language test modules pass; system test modules did not end in 5,400 s |
| LTP, all 4,226 (`LTP_JOBS=12`, limit 600 s) | not run (the 30 above hang already) | 4,144 verdicts match, 82 differ: 25 hang, 8 slower than the limit, 32 wrong answers, 17 parts not built |

The 82 (scratch `executor-restart/x3-ltp/ltpall/compare.txt`): hangs include sched starvation and hackbench01/02,
msgstress01, dirtyc0w, inotify09, timerfd_settime02 and DIT000-002; wrong answers are mostly sysfs entries skipped (13)
and a few syscalls (clock_gettime04, mlock201, mincore03, creat05, vma01). None was looked at further.

### The bounded targets (MEASURED, at `c4113f822`, without the kernel fix above; two copies, slice 1,000,000 and 100,000)

| Target | slice 1,000,000 | slice 100,000 |
|---|---|---|
| cpulimit, cputime, deadlines, edges, processes, repeats, threads, timers, turns | 2,400 s bound (exit 124), each | the same |
| counterclock | 0 of 2: kernel panic, `SystemAccessTrapped` (running.rs:172) | the same |

### The stand-in's whole suite at `20d552cdf` (MEASURED, release, 4,407 s job)

| Run | Result |
|---|---|
| stand-in unit tests | 8 of 8 |
| executor suite without the behavior source module | 25 of 25, the agreement test among them |
| agreement test | 32,748,140 states executed; 576,458 refused, of 39 instructions (798,029 before); 353 set aside; 0 instructions disagree |
| refusals of MRS / MSR (register) | 720 of 130,377 / 1,800 of 117,357 (124,014 / 100,077 before: the trapped ones now compare as `SystemAccessTrapped`) |
| simulation unit / suite without the behavior source module | 9 of 9 / 25 of 25 |

### Milestones

- X3's executor part holds by the measurements above (suite 25 of 25, agreement 0 disagree, 0 `unsafe` in the stand-in's
  files).
- X3 IS NOT REACHED: the LTP bar (4,169 of 4,226 at line 1444) is 4,144 even with the kernel fix, and 6 of 30 without it
  on the harness as `kernel-port` has it. Next: X3's LTP part again, waiting on the kernel track (1) taking the 6-line
  `insert` fix, (2) emulating the trapped counter read, (3) a runner that returns, or a clock that moves, at a slice's end.
- The wasmtime executor (another agent's) must pass `a_guest_that_loops_forever_is_interrupted_by_its_slice`.

SECURITY: lines with `unsafe` (`git grep -n unsafe`, raw word) before (`1070da05d`) and after (`20d552cdf`): unicorn 29 to
13 (the merge of `kernel-port` removed the harness's `libc::fork`; the 13 are the harness's: backend/Cargo.toml 3,
backend/src/main.rs 2, generate.rs 8, all the kernel track's; the stand-in's files 0 to 0), wasmtime 62 to 62 (another
agent's), simulation 0 to 0, conformance 0 to 0. No marked exception added, no UNSOUND mark, no lint lowered, every crate
keeps `[lints] workspace = true`. No dependency added. The hostile-guest suite was not run (none exists; the thread-test
design review is owed to the owner first).

## X3 check repair: the counter read, the whole behavior source suite, LTP on kernel-port's harness (2026-11-27)

### What the check found and the repair (READ: arm64 list `system_register.rs:148-149` on `origin/arm64-instructions`)

- The list names `("DAIF", at(3, 3, 4, 2, 1), A::Trapped)` (SCTLR_EL1.UMA clear): at EL0, MRS X0, DAIF traps to EL1. Since
  `c4113f822` the stand-in executes it as that trap (`SystemAccessTrapped`), so it is no state the stand-in cannot execute.
  Repair `5923767b8`: the unexecutable list names WFET X1 (`0xD5031001`) instead, which unicorn stops on as undefined with
  no syndrome to tell a trap from an undefined instruction.
- Why the behavior source module did not finish in 1,500 s: each of its tests runs every generated state (33 million at
  0.68 ms each, about six hours a pass, MEASURED) on one thread. The cut generator is the cause; `5923767b8` gives the
  module a `Sampled` generator (at most 256 states of each instruction, 596,536 states, MEASURED 34 s for a measuring
  pass) and does not run `measured_behavior_agrees_with_the_source` on an executor with instructions set aside (its
  agreement already checks every other instruction).
- Found on the way, mine: the merge of the arm64 list broke the simulation (`RegisterValueExpectation` became
  `UncomparedBits`; HINT, MSR immediate gained feature-set behaviors, so the first behavior of HINT is a chip's without
  FEAT_PAuth and disagreed with XPACLRI's on hint 7). `310aa6eb3`: the simulation draws uncompared bits from its random
  source; its tests take for each instruction the first behavior that agrees with the more specific encodings' first
  behaviors. The stand-in reported SSBS, DIT and TCO clear whatever the state held (96 MSR immediate states disagreed):
  `b828bde70` carries them through (TODO kept: an MSR immediate write of them is not seen), and HINT, which now agrees,
  leaves the set-aside list (353 to 352).

### MEASURED (Graviton4, release, `-j 12`; job x3b/20261127-075340, synced at the tree of `b828bde70`, pushed)

| Run | Result |
|---|---|
| fmt; clippy `-D warnings` (stand-in `--lib --test conformance`, simulation, conformance) | 0; 0; 0 |
| behavior source, each test alone | next_state 95 s ok; execute_agrees 358 s ok; deterministic 360 s ok; measured 51 s FAILED as expected (`--include-ignored`: set-aside instructions show no one known behavior) |
| stand-in unit + whole executor suite | 8 of 8; 28 passed, 1 ignored (the measured test), 0 failed, 4,478 s |
| simulation unit + suite | 9 of 9; 29 of 29 |

Logs: scratch `executor-restart/x3-bsrc/run-b82/`.

### LTP on kernel-port's harness (MEASURED, job x3ltp/20261127-061003 at `ef8ca97de` = kernel-port `353987bad` merged; LTP_JOBS 8, LTP_LIMIT 900 as for the 4,169 runs; load 45 to 156)

- The kernel track took the AddressMap fix (`517923bf4`, `unmap_shared` in `insert`) and the counter emulation (READ).
  The 30: 30 of 30 verdicts match (was 6 match, 24 hang). The 24 hangs' cause was the kernel's `insert` (found here
  yesterday, applied by the kernel track); none hangs now.
- All 4,226: 4,168 match, 58 differ. THE BAR (4,169) IS MISSED BY ONE.
- Against the 4,169 run `ours-20260930-a9f8631e.tsv`: 4 now pass that timed out (ADSP037, 038, 073, 074); 5 now differ that
  matched: clock_gettime04 (`CLOCK_REALTIME(gettimeofday)` readings 15 to 17 ms apart), futex_wait05 ("slept for too
  long"), mtest05, shm_test01, hackbench01 (900 s limit).
- Rerun of those 5 alone (load 42 to 58): futex_wait05 passes; the other 4 fail the same. A probe with the stand-in's
  translation drop on writes removed (`af5897634`) gives the same 4: not that. INFERRED: the kernel's new time model or the
  trapped counter read (each vDSO clock read is now a trap); for the kernel track.

### Milestones

- X3's executor part HOLDS (above): suite 28 + 1 ignored, behavior source three of three that run, 0 `unsafe` in the
  stand-in's files.
- X3 IS NOT REACHED: LTP 4,168 of 4,226 (bar 4,169). Next: the kernel track finds why clock_gettime04, mtest05,
  shm_test01 and hackbench01 slowed since `a9f8631e`; then a full LTP run on a pushed tip.

SECURITY: lines with `unsafe` (`git grep -w unsafe`) before (`922f20713`) and after (`b828bde70`): unicorn 6 to 6 (all in
the kernel track's harness files), the stand-in's files 0 to 0, simulation 0 to 0, conformance 0 to 0. No marked exception,
no lint lowered, every crate keeps `[lints] workspace = true`. No dependency added by this track (the lock's changes come
from the kernel-port merge). All owner interface files byte-identical to his main `ac696ea19` (3 interface files added by
other tracks exist only here). The hostile-guest suite was not run (none exists; the thread-test design review is owed
to the owner first).

## X3 run 28: no test switched off, node's stall, the LTP set of verdicts, the linter (2026-11-27)

Commits (pushed): `5bc2ee60f`, `db4051178`, `a0f622a9c`, `cf2140cdc`, `432fff607`, `d567c1a91`. His main `ac696ea19` was
already merged; every owner `interface.rs` is byte-identical to it.

- No test switched off. `measured_behavior_agrees_with_the_source_but_set_aside` (behavior-source conformance) takes the
  set-aside list: each instruction set aside measures as its first known behavior and is not checked, so the two
  consistency checks (everything at once against one at a time, the count) still run over every instruction. A state the
  source refuses is left out and counted; where only refused states tell behaviors apart (Ldff1wGatherExtendedOffset),
  the first that agrees is taken. `Sampled` keeps, besides its spread, the first state that tells each two behaviors
  apart (instruction 2295 was indistinguishable on the plain spread). The old function is unchanged for other callers.
- Node's stall: unicorn's write to a region that refuses stores makes it writable and back, each change rebuilding its
  memory map. The stand-in now takes into unicorn only the shared pages changed since its copy (a change version per
  page), opens such a region once for all its writes, maps a region writable while its bytes go in, and restores a
  remapped region's shared bytes from host memory instead of recopying whole ranges. `copy_from` looks up shared ranges
  only for pages that show them.

### MEASURED (Graviton4)

| Run | Result |
|---|---|
| node `start_up_matches_reference` (debug, harness) | 43 s at `db4051178`, 22 s and 25 s after `432fff607` (was over 600 s) |
| behavior source group, the kernel check's command (`timeout 1500 ... behavior_source -- --test-threads 4`) | 4 of 4 in 578 s at `cf2140cdc`; `execution_is_deterministic` included |
| measured test planted: set-aside branch taken by every multi-behavior instruction (first, last behavior) | fails at instruction 43 and at instruction 0 ("disagrees with the source") |
| whole executor suite at `432fff607` (release) | stand-in 8 of 8; 29 passed, 0 failed, 0 ignored, 6,909 s; simulation 9 of 9, 29 of 29 |
| harness memory tests at `432fff607` | sharedmap, sharedviews, privatemap, memory, processvm, memfd, faults, standin, hello, zombie, orphans pass; mapflags, sysvshm, threads, busybox `system` fail as in fails16; processes `killed_children_end...` failed once under load, 6 of 6 alone |
| fmt; clippy `-D warnings` | 0; 0 |

### LTP (job x3n-ltp/20261127-141608 at `432fff607`, LTP_JOBS 8, LTP_LIMIT 900, load 115 to 180)

- 4,226 compared: 4,166 match, 60 differ (bar 4,169).
- Against `ours-20260930-a9f8631e.tsv`: gained ADSP037, 038, 073, 074; lost mtest05, shm_test01, hackbench01 (900 s
  timeouts) and six timer tests ("slept for too long": epoll_pwait03, futex_wait05, nanosleep01, poll02, pselect01_64,
  select02). The six pass alone (load 31 to 58, 42 to 49 s each): load, not code. clock_gettime04 now passes (4.7 to
  10.7 s alone; it failed at 27.6 s before the memory change).
- Executor speed, not kernel behavior, for the three (perf, 200 s each): mtest05 95% in `set_permissions` -> `remap`
  -> unicorn map and unmap (phys_page_set_level 55%, phys_page_compact 16%); shm_test01 memcpy 41% and memcmp 15% in
  `run_loaded`'s copy of shared pages out and in; hackbench01 28% in the kernel's `ppoll`, 24% in the stand-in's
  `copy_from` per fork with shared-range lookups. All three passed at `a9f8631e`, before `ccd4db9da` gave unicorn the
  bytes (no `mem_map_ptr`, no unsafe). Alone with a 3,600 s limit at `d567c1a91` (load 31 to 58): mtest05 passes in
  1,604 s, hackbench01 in 2,042 s, shm_test01 still times out at 3,600 s (they took 486, 466 and 103 s at `a9f8631e`).

### Linter (installed `59ad72ce`, built on the machine; no key there, so `*_unanswered` cannot be answered)

My files' findings: 753 at `922f20713`, 766 at `aabf8dc2e`, 764 at `d567c1a91`. Worked off: every finding the last run
and this one added except the unanswered ones and these, which need a decision: `Sampled` implements another crate's
trait with pub fields (`implementation_folder`, 3 `public_outside_interface`; a named generator implementation would
have to pass the generator suite); `conformance_not_run` for the new suite function and for the suite's arm 3 (the
fixed `tests!` shape wants every suite function to take one value; its other functions already do not).

## X3 run 33: the unicorn API crate, the three slow LTP tests (2026-11-27/28)

His ruling (an outside-API crate for unicorn inside the stand-in's folder, host memory owned by the direct crate, our
side only through `VolatileBytes`) is built:

- `crates/executor/implementation/unicorn/unicorn`: `interface.rs` (the trait `Unicorn<Data>` and THE MAPPING RULE at
  the top), `implementation/direct` (2 marked `unsafe` uses: the pointer promise of each host memory, and
  `mem_map_ptr`; `[lints.rust] unsafe_code = "deny"`), `implementation/recording`, `implementation/hostile` (its guest
  writes every byte a writable mapping shows before each run), `conformance/suite.rs` (21 tests, the mapping rule's
  hostile-guest test `a_guest_writes_only_bytes_a_writable_mapping_shows` among them). The engine's handle never
  leaves `direct`, hooks included (they get the data and a `Requests` value), so no copy of it outlives the memory.
- The stand-in (memory.rs) maps each host memory once with every permission and translates every guest address
  through unicorn's TLB hook from the page table (THE SPACE'S RULE at the top of memory.rs; its own hostile-guest test
  `a_hostile_guest_writes_only_what_the_page_table_lets_it`). No copy between turns, no map or unmap at a permission
  change, a TLB flush instead.

MEASURED on the Graviton4 (all at pushed commits; details in the run's report):
- mtest05 alone: 1,604 s before, 26.6 s at 74b80ae01. shm_test01 alone: over 3,600 s before, 1,252 s. hackbench01
  alone: still over 1,800 s.
- Harness memory tests pass as before (sharedmap 1.3 s, memory 7.7 s, faults 7.4 s, processes 8/8).
- Executor suite on the stand-in: 28 of 30; `agrees_with_precise_instruction_set` and
  `measured_behavior_agrees_with_the_source` fail on 13 branch instructions (Br, Blr, Ret and their PAC forms), 1 to
  16 states each: a branch to the top of the address space (0xffff_ffff_ffff_ffff) stops with the program counter at
  0xffff_ffff_ffff_fffb under the TLB hook, where QEMU's own translation gave the target. Open (see next).
- Simulation suite 30 of 30 (x86), with the new suite test `copy_from_over_written_pages_leaves_the_source_bytes`.
- Fix after the suite run (2b9b1a819, 0de965246): a faulting fetch leaves the program counter at the address
  fetched; an upper-half branch target faults there (unit test). The top-of-space case stays open.
- Full LTP at pushed 0de965246 (LTP_JOBS 8, LTP_LIMIT 900, load 47 to 226, 9,321 s): 60 of 4,226 differ from the
  reference, as in run 28. Against ours-20260930-a9f8631e.tsv: gained ADSP037, 038, 073, 074 and mtest05 now matches
  again; lost shm_test01 and hackbench01 (900 s timeouts), DOR003 (900 s timeout) and eight timer tests that fail under
  load (epoll_pwait03, epoll_wait02, futex_wait05, nanosleep01, poll02, prctl09, pselect01, select02).
- X3 is NOT reached. Next: hackbench01 and shm_test01 (the stand-in's share of their profiles is now small: the
  engine thread's channel round trips and the kernel's own paths), the branch at the top of the address space, the
  cargo-mutants run over memory.rs (started twice on the x86 machine, which was taken away both times), and the full
  LTP at load under about 60.
