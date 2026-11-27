# Executor track, X2: the WebAssembly executor on the owner's interfaces

Branch `executor-wasmtime`, cut from `origin/executor-restart` 9aaaf77a3 (2026-11-26). His main `ac696ea19` was already in
it ("already up to date", MEASURED `git merge-base --is-ancestor`); `origin/executor-restart` dd61eb7c merged in
(`ee26ae9f`, no conflict). Every file of his interface folders (`crates/*/interface.rs` and the data files beside it) is
byte-identical to his main (MEASURED: `git diff --stat owner/main HEAD` over those paths is empty). Brief:
`orchestrator/restart-2026-11-25/brief-executor.md`, X2. Scratch: `wcontainers-scratch/executor-wasmtime/`.

Marks: MEASURED (run on a rented machine, the command given), READ (read in a file), INFERRED (not run).

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
