# Executor track, X2: the WebAssembly executor on the owner's interfaces

Branch `executor-wasmtime`, cut from `origin/executor-restart` 9aaaf77a3 (2026-11-26). His main `ac696ea19` was already in
it ("already up to date", MEASURED `git merge-base --is-ancestor`); `origin/executor-restart` dd61eb7c merged in
(`ee26ae9f`, no conflict). Every file of his interface folders (`crates/*/interface.rs` and the data files beside it) is
byte-identical to his main (MEASURED: `git diff --stat owner/main HEAD` over those paths is empty). Brief:
`orchestrator/restart-2026-11-25/brief-executor.md`, X2. Scratch: `wcontainers-scratch/executor-wasmtime/`.

Marks: MEASURED (run on a rented machine, the command given), READ (read in a file), INFERRED (not run).

## X2 continued, 2026-11-26 afternoon (newest first; the sections after this one are the morning's)

His main `ac696ea19` was already in the branch (MEASURED: `git merge-base --is-ancestor FETCH_HEAD HEAD` after
`git fetch <his checkout> main`). Merged: `origin/executor-restart` 914b4e9d1 (`18a5611a2`) and `origin/arm64-instructions`
31c296c51 (`6473ee15b`), no conflict. Every owner interface file is byte-identical to his main (MEASURED: `git diff
FETCH_HEAD HEAD` over every folder with an `interface.rs` on his main, leaving out `implementation/` and `conformance/`,
is empty; `crates/kvm`, `crates/linux-socket-options` and `crates/wasmtime` have an `interface.rs` his main lacks: API
crates of the tracks, not his).

### READY FOR A CONTAINER: `9802b9416`

hello and busybox run on this executor at `9802b9416` (MEASURED, Graviton4, throwaway tree = this commit merged with
`origin/kernel-port` 406fba566, the harness pointed at `WasmtimeExecutor` and one line in the kernel; patch:
`wcontainers-scratch/executor-wasmtime/hello-tree-throwaway.patch`; `cargo test --release -p wcontainers-executor-unicorn
--features runtime --test hello --test busybox`, log `fetched/run6-arm/hb3.log`; `counterclock` at `e4a430b6c`, log
`fetched/run5-arm/hb2.log`):

| Test | Result |
|---|---|
| `hello_matches_reference` | passes |
| busybox | 3 passed, 1 failed, 1 ignored: `system` differs in 2 of 35 commands, the same two host-specific ones as on the stand-in (`/proc/config.gz`, `/proc/sys` values) |
| `counter_never_goes_backwards` | fails, as the ruling makes it until the kernel answers the trap: `running.rs:172` "an execution ended on an exception no exit names: SystemAccessTrapped(SystemAccess { op0: 3, op1: 3, crn: 14, crm: 0, op2: 0, rt: 23, direction: Read })" |

What a container needs from the other tracks (none is this branch's):

- Kernel (`origin/kernel-port`): (1) `running.rs` `to_processor` names the new field: `process_state_controls: 0,` (the
  field arrived with arm64-instructions 58791651d; without it the kernel does not build on this branch). (2) Answer
  `Exception::SystemAccessTrapped(SystemAccess { op0: 3, op1: 3, crn: 14, crm: 0, op2, rt, direction: Read })` by
  emulating the read, as Linux's `do_el0_sys` handlers do: op2 2 (CNTVCT_EL0) or 1 (CNTPCT_EL0) write the kernel's clock
  in ticks at its frequency to `rt` (31 is the zero register: nothing written), op2 0 (CNTFRQ_EL0) writes that frequency;
  then pc + 4 and run on, no signal. Any other trapped access: SIGILL, ILL_ILLOPC, as Linux. Today `exit_of` panics on it.
  Then `counter_never_goes_backwards_across_saves` can lose its `runtime` ignore. (3) The harness's `runtime` feature names
  `WasmtimeExecutor` as `Back` (driver.rs, in the patch above). `handle_interrupt` needs no change: its `Terminate`
  already ends the slice.
- Runtime: build the executor with `WasmtimeExecutor::new(WasmtimeTranslatorConfig::default())`, which now bounds runs.

### What changed (commits on this branch)

| Commit | What |
|---|---|
| `492c943d8` | translator, guest-cpu, arm64: (1) a memory fault reports the address as the program computed it, tag included (`push_fault`, code.rs and full.rs: the fault address plus the distance between the program's address and the untagged one); (2) `Access::Modify` for an atomic read-modify-write: checked for read and write, a fault is a read where the page cannot be read, a write where it can (CAS, CASP, LDADD to SWP); (3) the ARM64 `Offer` (was `()`): `CounterAccess::Read` (default, key 0, today's output) or `Trapped`, under which a read of CNTVCT_EL0, CNTPCT_EL0 or CNTFRQ_EL0 raises the new `TrapKind::SystemAccess` (detail: the word); (4) MDCCSR_EL0, DBGDTR_EL0 and DBGDTRRX_EL0 read zero and DBGDTR/DBGDTRTX writes do nothing (MDSCR_EL1.TDCC clear) |
| `b547d187f` | executor: translates under `Offer { counter: Trapped }` and `ProgressCount::Kept`; a `SystemAccess` trap ends the run with `SystemAccessTrapped` at the MRS; the hook untags a fault address for pages and handler and keeps the tag in `DataAbort`; `WasmtimeTranslatorConfig::slice` (default `DEFAULT_SLICE`, 2^22 progress events): when the count reaches the slice's end, the stop hook calls `handle_interrupt`, as for an interrupt from outside; the stop flag is now the progress limit (raised = 0) |
| `33e3f9f78` | executor: at most 65,536 kept translations (each wasmtime module holds mappings; the full list hit the 1,048,576 map limit: "unable to make memory executable ... Cannot allocate memory (os error 12)" on both machines) |
| `6e358a5c3` | stand-in: `process_state_controls` reported clear (TODO marked, for X3) so the unicorn crate builds after the merge |
| `321fe3abe`, `6d3e773dc` | an alignment trap of an exclusive, acquire, release or atomic access carries the computed address; the executor reports it as `DataAbort { AlignmentFault, access, address }` (Write for STXR/STLXR/STLR, Read otherwise, as the list), no longer as a stack pointer fault |
| `e4a430b6c` | DC ZVA checks one byte at the register's address, so its fault reports that address, as the list |
| `9802b9416` | IC IVAU never faults at EL0 (the list: it never does) and hands the hook its untagged address |

For the translator track (origin `translator-port`), the translator-side commits are `492c943d8`, `321fe3abe`,
`e4a430b6c` and `9802b9416`: every module with a checked access changes (one more local and its copy per check; the fault path), so
TR1's WebAssembly hashes must be taken again; the default offer keeps key 0, so the description section is unchanged.
`translator-test` was adapted (the `()` offers, the new trap kind as SIGILL; its offer reads the counter, so none
traps). Its runner now gets tagged fault and alignment addresses; Linux gives `si_addr` untagged by default, so its
memory-fault and alignment stops should untag before comparing (INFERRED: layer 1 showed no such case, below).
The native track's `hardware-virtualization` tests pass `()` as the ARM64 offer in three places (tests/carry.rs:65,
178; tests/registers.rs:85): `wcontainers_arm64::Offer::default()`.

### The bar now (MEASURED unless marked)

| Run | ARM64 (Graviton4) | x86-64 |
|---|---|---|
| executor suite without the full-list tests | 23 of 23 | 23 of 23 |
| `tests/slice.rs`, the ten slice instructions | 10 of 10 (was 6) | 10 of 10 |
| `tests/time_slice.rs` (new: slices answered continue change nothing, terminate ends at the slice, no slice asks nothing, counter reads trap at their MRS) | 4 of 4 | 4 of 4 |
| volatile-bytes 5, `wcontainers-wasmtime-direct` 5 | pass | pass |
| clippy `-D warnings` all targets (arm64, guest-cpu, translator, translator-test, executor-wasmtime); rustfmt | clean | |
| trap cost (`tests/trap_cost.rs`, fastest of 5 runs of 1,000,000 system calls) | 82.8 ns at `6d3e773dc` (83.1 at `b547d187f`; 80.2 before) | 78.9 ns at `6d3e773dc` (74.6 before) |
| determinism probe (`tests/determinism.rs`, every listed instruction) | 0 instructions give two states (was 1, MRS of the counter) | 0 |
| translator crates' tests (`-p wcontainers-arm64 -p wcontainers-guest-cpu -p wcontainers-translator`) | all pass | |
| translator-test `valid`, `progress_count`, `smoke`, `checks`, `test_guest`, `decoding`, `layer1` | 65 pass; the 7 x86-64 ones need a reference machine; `sample_matches_the_cpu_in_chrome_arm64` needs the browser; `every_form_matches_the_cpu_arm64`: 1 form differs, MSR FPSR (reference FPSR 0x80000000, translated 0), a path these commits do not touch (INFERRED pre-existing, not rerun on the parent) | |

Full list (`cargo test --release --test conformance -- --test-threads 6 agrees behavior_source`, `CriticalValueCut`):

| Run | ARM64 | x86-64 |
|---|---|---|
| `agrees_with_precise_instruction_set` | fails at `e4a430b6c`: 33,322,792 states executed, 0 refused; 143 instructions disagree (538 this morning); 51 min | fails at `6d3e773dc`: the same 143 (classified the same, 131 + 11 + 1); 63 min |
| `next_state_after_an_error_executes_as_on_a_fresh_source` | passes | passes |
| `execute_agrees_with_execute_in_place` | passes (failed before: the counter) | passes |
| `execution_is_deterministic` | passes (failed before: the counter) | passes |
| `measured_behavior_agrees_with_the_source` | fails: "an instruction shows no one known behavior" (the disagreeing ones below; the 4 with no behavior now have one) | fails, the same |

So the behavior-source suite passes 3 of 4 (was 1 of 4).

Disagreeing instructions by cause at `e4a430b6c` on ARM64, first differing state of each (`scripts/classify.py` in scratch), 143 in all:

- 130: FEAT_LSE2. The list lets LDAR, STLR, CAS, CASP and the LSE atomics lie anywhere inside 16 bytes (a Neoverse V2;
  `Alignment::WithinSixteenBytes`, no other choice), so a state with x = 0x2001 on an unmapped page is a translation
  fault there; the translator requires natural alignment and gives an alignment fault (WebAssembly atomics trap when
  misaligned). Casp32, Caspl32, Caspa32, Caspal32, Stlrh, Ldarh, Stlr32, Ldar32, Stlr64, Ldar64, Cash, Caslh, Casah, Casalh
  and the rest of the CAS, CASP, LDADD to SWP, LDAR and STLR family. Either the list adds the choice of a chip before
  FEAT_LSE2 (an alignment fault, as `MisalignedExclusive::AlignmentFault` does for the exclusives: arm64-instructions
  track), or the translator implements LSE2 (a misaligned access inside 16 bytes done without a WebAssembly atomic:
  translator track, a question of its concurrency contract).
- 11: the translator takes as undefined what the list defines:
  - ScvtfScalarImmediate, UcvtfScalarImmediate, FcvtzsScalarImmediate, FcvtzuScalarImmediate and the four
    VectorImmediateImmediate forms (first word `5f1fe440`): immh = 001x, half precision, FEAT_FP16. The list's rows name
    only FEAT_AdvSIMD, so the half-precision encodings get no "without the feature" behavior, which rows naming FEAT_FP16
    get; the translator offers no FP16 anywhere (decode_simd.rs: "Half precision needs FEAT_FP16, which we do not
    offer").
  - PmullAsimddiffL, size 11 (`0ee4e040`): FEAT_PMULL, which the translator does not offer either; same list cause.
  - Both: the arm64-instructions track gives these encodings the behavior of a chip without FEAT_FP16 or FEAT_PMULL.
  - MsrRegister (`d51b4220`, MSR DAIF) and MsrImmediate (`d50340df`, MSR DAIFSet): the list has EL1 trap them
    (SCTLR_EL1.UMA clear: `SystemAccessTrapped`); the translator says undefined. The MDCCSR/DBGDTR cause of this morning
    is gone (`492c943d8`).
- 1: Mrs (`d5380000`, MRS MIDR_EL1): the list has EL1 trap every ID register read (FEAT_IDST); the translator answers
  in place with the values Linux's emulation gives.
- 1: Sys (`d50b7520`, IC IVAU of an address above 48 bits): the list says it never faults, the translator checked it;
  fixed in `9802b9416`, re-measure below.

The DAIF and ID-register causes are one question, for the orchestrator: should the executor trap every register the
list says EL1 traps (DAIF, DAIFSet/Clr, SCXTNUM_EL0, the timers, the activity monitors, the ID space), as it now does
the counter, so that the kernel answers them as Linux does (ID registers emulated, the rest SIGILL)? It is the same
offer switch in the translator (`CounterAccess::Trapped` would grow into "EL1 traps as Linux sets it"); it needs the
kernel to answer `SystemAccessTrapped` first, since a program that reads MIDR_EL1 (glibc does, given HWCAP_CPUID)
would then stop the kernel where today it reads Linux's value.

The re-measure at `9802b9416` (both machines) is running; its result follows here when in.

### Next

X2's remaining bar (full-list agreement) waits on: the list track for the FP16 and PMULL feature choices and the
LSE2 choice, or the translator track for LSE2; the orchestrator's ruling on the EL1-trapped registers (DAIF, the ID
registers). X3 (the stand-in) waits on the kernel and runtime compiling on this base: the kernel needs the one line above.

## What was built

- `crates/wasmtime` (new, the outside-API crate for wasmtime, in the layout of `linux-process` on `native-restart`):
  `interface.rs` is one safe trait `Wasmtime`; `implementation/direct` holds every `unsafe` use, each one marked line;
  `conformance` is its suite. The trait has seven methods: `with_host_memory` (the engine takes every linear memory
  from us), `new_host_memory` and `map_file` (a reservation with memory files mapped over it, which every store's guest
  memory is), `host_memory_bytes` (its bytes, only as `VolatileBytes`, through `volatile-bytes-pointer`'s
  `PointerAndLength`), `new_memory` (a store's memory over a host memory), and `call_with_trap_handler` with
  `with_trap_handler` (the trap handler of a run, reachable from the hooks the translated code calls, since a store's
  data must be `'static`).
- `crates/executor/implementation/wasmtime` rewritten on the owner's `Executor`, `Execution`, `MemorySpace`, `PageTable`
  and `BehaviorSource` (`WasmtimeExecutor`, `WasmtimeMemory`, `WasmtimePageTable`, `WasmtimeBytes`,
  `WasmtimeInterrupter`, `WasmtimeExecution`). 0 lines with `unsafe`, under the workspace's `forbid`.
  - A space: 4 GiB of guest addresses in a sealed memory file, mapped into one host memory; `map_from` maps the other
    space's file pages over it, `unmap_from` maps its own file back after punching the pages to zero, `copy_from` copies
    only the data extents of the other file (`SEEK_DATA`), carries its mappings and permissions. A state memory per
    space (blocks, the permission table, the array of slots), reached only through WebAssembly's own loads and stores
    (the module of the translator's test runner).
  - `run` is the loop "run until the next trap, then ask the handler": the hooks (system call, stop, memory fault, load
    code) ask the handler from inside the run; the guest's WebAssembly frames stay where they are. Retry of a fault runs
    the instruction again alone (`UNWIND_GO_TO_EXACT`). The interrupter raises the thread's stop flag with
    WebAssembly's own atomic store. Code is translated where a thread reaches it (bounded look-ahead), kept for every
    executor of the process by the hash of its bytes, dropped when written over (host writes, guest writes to a page
    that holds code, `ic ivau`, a lost execute permission, map/unmap/copy over it).
  - Threads of one space on several host threads: a change that no piece in progress may run past (a load, a
    narrowing, dropped code, a mapping) raises every other thread's stop flag and waits until none runs guest code, as
    the translator's contract asks; a thread that saw such a change leaves and runs again from its registers.
  - The behavior source runs one instruction on a scratch space per executor (cleared after each, made anew every 1,024).
- The old crate's files that served the runtime's own `Cpu`/`Execution`/`Executor` (the deserialize cache, the custom
  memory, the CLI, the ELF start reader, 12 test files) are gone.

## The bar (MEASURED unless marked)

| Run | ARM64 (Graviton4) | x86-64 |
|---|---|---|
| `wcontainers-wasmtime-direct` conformance | 5 of 5 | 5 of 5 |
| executor suite without the two full-list tests | 23 of 23 | 23 of 23 |
| volatile-bytes suite | 5 of 5 | 5 of 5 |
| clippy `-D warnings` all targets, the four new crates; rustfmt | clean | |
| trap cost (`tests/trap_cost.rs`, release, fastest of 5 runs of 1,000,000 system calls) | 80.2 ns | 74.6 ns |

The full-list tests (`cargo test --release --test conformance -- --test-threads 6 agrees behavior_source`, the set
measured from this executor by `list_translator_behaviors`, `CriticalValueCut`, all 4,430 listed instructions; logs
in scratch `fetched/full2-arm.log`, `full2-x86.log`):

| Run | ARM64 | x86-64 |
|---|---|---|
| `agrees_with_precise_instruction_set` | fails: 25,783,206 states executed, 0 refused; 538 instructions disagree (1,542,679 of their 4,885,104 states); 4 not compared (no known behavior) | the same 538, the same counts |
| `next_state_after_an_error_executes_as_on_a_fresh_source` | passes | passes |
| `execute_agrees_with_execute_in_place`, `execution_is_deterministic` | fail (the counter, below) | fail |
| `measured_behavior_agrees_with_the_source` | fails (the 4 with no behavior) | fails |
| time | 732 s | 878 s |

Before the step fix of `4bae8d25` the ARM64 run had 541: BR, BLR and RET to a misaligned address ended their own step
with the next fetch's alignment fault.

### The ten slice instructions (`tests/slice.rs`, `CriticalValueCut`, 78,836 states, both machines the same)

6 of 10 show one known behavior that keeps the translator's choices over every state: ADD, ADDS, CSEL, BRK, RDVL
(undefined), MRS CTR_EL0 (`CACHE_TYPE`). 4 do not: LDR (unsigned offset), STR, STP, LDR (post-index): 2,120, 4,524,
11,286 and 1,824 states disagree, and every one of these 19,754 states agrees with a behavior once its fault address
gets back the tag bits the behavior keeps. The translator reports the address of a faulting tagged access with its
whole top byte cleared (the ARM64 guest untags before the permission check and hands the hook the untagged address);
the known behaviors keep the top byte (`KeepsTopByte`) or clear only its top nibble (`ClearsTopNibble`). That is the
translator's to change (or a behavior the list lacks), not the executor's: the hook never sees the tagged address.
0 states refused. 4.3 s on ARM64 in release, 7.1 s in a test build.

### hello and busybox through the kernel (throwaway, not committed)

The kernel track's harness (`crates/executor/implementation/unicorn`, its files) is hard-wired to the stand-in. In a
throwaway worktree (this branch merged with `origin/kernel-port` 36bf10ee), its `runtime` feature was pointed at
`WasmtimeExecutor` (driver.rs 30 lines, layer1.rs 8; patch: `wcontainers-scratch/executor-wasmtime/kernel-harness-on-wasmtime.patch`):

- `hello_matches_reference`: passes on this executor.
- busybox: 3 passed, 1 failed, 1 ignored — the same as on the stand-in in the same tree: `system` differs in two
  commands on both (`/proc/config.gz` is absent in the reference container; `/proc/sys` `protected_*` and
  `perf_event_paranoid` are the rented host's). A first run on this executor also failed `files` on a timestamp's minute
  rolling over between ours and the reference; it passed on rerun.
- The merged tree's stand-in path does not build as it is: `origin/kernel-port`'s driver.rs still calls the
  stand-in's `refused_in_a_copy`, which `origin/executor-restart` dd61eb7c removed (for the kernel track).

## What each waiting track can start from

- Translator, TR4: from `4bae8d25` (or this status's commit). The measured set is in `conformance_run.rs`
  (`list_translator_behaviors`). The disagreements below are the translator's list to work through.
- Runtime, RT2; native, R2 and R3: the executor half of X2 is from `4bae8d25`. Construction is an inherent
  `WasmtimeExecutor::new(WasmtimeTranslatorConfig)` (open question 5); every executor of the process shares one engine,
  so a space made by one runs on any other.
- x86 front end, XF4: the executor is ARM64 only (`Arm64<User>`, the `wcontainers-arm64` guest); an x86-64 one needs
  the guest made a parameter of `engine.rs`, `thread.rs` and the block conversion. XF4 can start reading from `4bae8d25`.
- Kernel track: hello and busybox run on this executor with the patch above.

## Not reached, and why

- The executor suite's full-list agreement and the behavior-source suite: see the table above. Of the disagreeing
  538 instructions, the first differing state of each (`scripts/classify.py` in scratch over both logs):
  - 359: the fault address of a tagged access, as in the slice (the ST1 to ST4 and LD1 to LD4 families, the exclusives,
    the loads and stores of every addressing form).
  - 168: the LSE atomics (CAS, CASP, LDADD, LDCLR, LDEOR, LDSET, LDSMAX, LDSMIN, LDUMAX, LDUMIN, SWP in every size and
    ordering) report a fault on an unmapped page as a write; the list's behavior has a read.
  - 11: the translator takes as undefined what the list defines: MSR (register) and MRS of the debug registers
    (`d5130400` MSR op0=2 op1=3 CRn=0 CRm=4; `d5330100` MRS MDCCSR_EL0), SCVTF, UCVTF, FCVTZS and FCVTZU with fixed
    point (scalar and vector), and PMULL of 64-bit elements (`0ee4e040`).
  None is the executor's: in each the hooks hand on what the translated code reports.
- The behavior-source suite's determinism (`execution_is_deterministic`, `execute_agrees_with_execute_in_place`):
  `tests/determinism.rs` (ARM64, release): one instruction gives two states, `Mrs`, 80 states, all reads of the
  counter, which the clock hook answers with the host's monotonic nanoseconds. The executor interface has no clock the
  kernel could answer, so a read of the counter is the host's; the list's MRS behaviors leave it decided.
- `measured_behavior_agrees_with_the_source` fails at the 4 instructions with no known behavior (Hint, MsrImmediate,
  Sys, Sysl), as on the simulation.
- The brief's "top page of a 2^48 space accepted": not lifted. A space is 4 GiB: the translator's contract puts the
  array of slots (one 32-bit entry for each instruction address it covers) and the permission table (one byte for each
  page) in a 32-bit state memory, so code above about 4 GiB less 2 MiB cannot be placed. The kernel's first tier is
  4 GiB, below which it places the stack, the mmap base and programs (READ: kernel memory.rs `PLACEMENT_TIERS`).
  Lifting it is a change of the translator's contract.
- Several threads of one space at once run through the gate above, which no suite test exercises yet: the thread-test
  design review is owed to the owner first, so no such test was written.

## SECURITY

Lines with `unsafe` (raw word, `grep -rn unsafe`), before (`9aaaf77a`) and after:

| Crate | Before | After | Marked uses |
|---|---|---|---|
| `wcontainers-executor-wasmtime` | 62 (all code) | 1 (a comment in Cargo.toml), 0 in code | none: `forbid` from the workspace |
| `wcontainers-wasmtime` (interface) | new | 2 (doc comments), 0 in code | none: `forbid` |
| `wcontainers-wasmtime-conformance` | new | 0 | none: `forbid` |
| `wcontainers-wasmtime-direct` | new | 30 (28 outside comments) | 10, each `#[expect(unsafe_code, reason = ..)]` with a SAFETY comment: `unsafe impl LinearMemory` (2), `unsafe impl MemoryCreator`, the reservation's `mmap_anonymous`, the file's fixed `mmap`, `mprotect` of a memory the engine grows, `munmap`, `PointerAndLength::new`, the handler's lifetime erased (`transmute` of a `NonNull<dyn TrapHandler>`), and lending it back (`NonNull::as_mut`) |

- The direct crate's manifest has its own `[lints.rust] unsafe_code = "deny"` (an outside API's `implementation/direct`
  with marked exceptions); every other crate keeps `[lints] workspace = true`. No lint lowered, no linter exclusion, no
  UNSOUND mark.
- Bytes a guest can change are reached only through `VolatileBytes` (`volatile-bytes-pointer`); the state memory only
  through WebAssembly loads and stores.
- New dependencies: none from outside the workspace. The new crates use wasmtime 50.0.1, wasm-encoder 0.263.0, rustix
  1.1.6, blake3 1, thiserror 2 and zerocopy, all already in the lock file. The executor crate dropped aarch64-cpu,
  libc, mach2, rayon, object, gimli, pico-args, tempfile, postcard, serde, serde_bytes, tar, serde_json, flate2, zstd,
  sha2, build-rs, cargo_metadata, cargo-lock, and the paths to `runtime-single-kernel` and `docker`.
- The hostile-guest suite was not run: none exists; the thread-test design review is owed to the owner first.
- Other tracks' crates that named the old executor and no longer build against it (they did not build before either,
  through the runtime): `hardware-virtualization`'s tests (dev-dependency) and the unicorn harness's `runtime` feature.
