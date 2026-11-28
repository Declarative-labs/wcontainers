# Executor track, X2: the WebAssembly executor on the owner's interfaces

Branch `executor-wasmtime`, cut from `origin/executor-restart` 9aaaf77a3 (2026-11-26). His main `ac696ea19` was already in
it ("already up to date", MEASURED `git merge-base --is-ancestor`); `origin/executor-restart` dd61eb7c merged in
(`ee26ae9f`, no conflict). Every file of his interface folders (`crates/*/interface.rs` and the data files beside it) is
byte-identical to his main (MEASURED: `git diff --stat owner/main HEAD` over those paths is empty). Brief:
`orchestrator/restart-2026-11-25/brief-executor.md`, X2. Scratch: `wcontainers-scratch/executor-wasmtime/`.

Marks: MEASURED (run on a rented machine, the command given), READ (read in a file), INFERRED (not run).

## X2, 2026-11-27 night: the check's five surviving plants, cargo-mutants over the window work (newest)

His main moved during the run to `b9cab24fe` (14 commits: Verus specifications for byte sink, byte source and volatile
bytes, the capability table, kernel-core, memory shard). Merged normally (`5a499dbfa`); the five conflicts were all in
volatile-bytes implementation files his main rewrote, taken as his. Every file beside each `crates/*/interface.rs` of
his main is byte-identical to his main after the merge (MEASURED: `git diff` per file, empty). Followed in `273ff83b9`:
`VolatileBytes` now asks `read_at`/`write_at` (and gives `read`/`write`), so the WebAssembly, simulation and stand-in
byte views implement them, and the wasmtime crate lends a host memory's bytes at offsets directly (his `PointerBytes`
no longer has `part`). Commits this run: `7f00de2db` .. `f7c400c42`, all pushed.

### The five plants the check left alive, each now killed by a test stating the right value

Tests in `tests/windows.rs` (MEASURED x86 at `51ba2acf6`, `xw-plants/run.sh`: the check's own patches applied one at a
time, executor and wasmtime-direct tests without the full list):

| Plant | Caught by (right value from the design) |
|---|---|
| t01 no clamp of the window number | `an_address_at_or_above_two_to_the_48_faults_at_that_address`: bits 48-55 set fault at that address, Read and Write |
| t14 an unplaced window shows window 0 | `a_window_never_given_a_place_faults_at_the_address` (and t01's test) |
| t03 fast test accepts every place | 6 tests: one load, two loads, a store, a loop, a 64/68 KiB stepped loop in window 1 past 64 KiB, each reading or writing its own word (word = address / 8) |
| t05 loop proof bound | the two window-1 loop tests |
| t20 `Space::bytes` not split at a window end | `a_borrow_across_a_window_end_past_the_64_kib_after_it_holds_each_address_once` (written whole and read whole) |
| t04 kept-run room (the check's "could not show it matters") | the 64 KiB stepped loop: a step of 64 KiB takes an address as computed to the place of the one before |

t06 (group exact check), t07 and t09 (range check placing) no longer apply: the code is gone. A translation with address
windows now asserts it is optimised for speed (the only way the executor translates), so the module's exact function
never gets an address as computed; `check_range` asserts no windows (only x86-64 and the test guest call it).

### cargo-mutants (27.1.0, x86 machine, never in a lock file)

Scope: every line of hand-written Rust changed since the list merge `1242fecd5` in translator, arm64, guest-cpu, the
executor's non-test files and wasmtime-direct (`--in-diff`, 537 mutants at the first run). Tests: executor and
wasmtime-direct packages, release, without the full list (`--skip agrees --skip behavior_source`).

| Run | Tip | Caught | Missed | Timeout (detected) | Unviable |
|---|---|---|---|---|---|
| run 2 | `51ba2acf6` | 316 | 132 | 20 | 69 |
| run 3 | `cf3ad2520` | 343 | 56 | 25 | 68 |
| run 4 (ARM64; the x86 machine was taken away twice) | `a6dc5e219` | 342 | 26 | 18 | 96 |
| run 6: the 31 of run 4's unviable whose build hit the shared machine's full disk, and the copy lines | `b3c370030` | 35 | 6 | 9 | 1 |
| run 7: `copy_from` after its run condition lost its redundant half | `06ebe56d5` | 12 | 1 | 0 | 0 |
| run 8: the lines the merge changed (WebAssembly bytes, direct crate) | `273ff83b9` | 22 | 3 | 0 | 2 |
| run 9: direct crate after a suite check of the whole length and an empty range | `f7c400c42` | 15 | 0 | 0 | 2 |

Run 7's one survivor is `run.end == page` -> false: no run is merged, so each page is copied alone (equivalent).

Between runs, each survivor got a test stating the right bytes, registers or fault (`tests/space_changes.rs`, 24 tests;
`tests/windows.rs` 19; `crates/arm64/tests/offer.rs`; a growth test in the wasmtime suite), or its code was shown
unreachable and removed: `SPACE_PAGES`, `any_code`, `drop_code_locked`'s unused result, `zero`'s fallback after a hole
punch that cannot fail on a memory file, `copy_file_data`'s retry of a short write and its read-of-zero check, the
step's stop count (now a flag), the run merging in `write_permission_bytes` (now one write per window).

Real defects of coverage the survivors showed, all now tested: no test changed run code (borrow write, copy, map,
unmap, zero, losing execute, a store by another run); no test pushed a frame onto a page that forbids writing (deleting
the frame check passed every test); no test ran two threads on one space (every thread sharing block 0 passed); no
test used a rounding mode (the float hook); no test copied or mapped across window ends or over mapped pages.

Survivors left that change only speed or memory, never a result (equivalent mutants; INFERRED from the code, each
named):

| Mutant | Why no result changes |
|---|---|
| scratch reuse guard -> false; `windows_given` -> 0 | every single instruction gets a new scratch space |
| `GUEST_GUARD_BYTES` `<<` -> `>>` | a guard of 0: wasmtime then bounds-checks each access itself |
| `WINDOW_PAGES` `/` -> `%`; `keep_pages` -> () | the fast test or the kept run never accepts, so every access takes the exact path |
| `bits() \| MAPPED`, `PUNCH_HOLE \| KEEP_SIZE` `\|` -> `^` | disjoint bits |
| `zero` guard `cleared == bytes` -> false | a whole own page is written with zeros instead of given back |
| run-merge conditions in `show_locked` and `copy_from` that only stop merging | each page mapped or copied by its own call |
| `copy_file_data` `hole - from` -> `/`; `copy_own_locked` `- target` -> `/ target` | reads or copies in smaller pieces |

Six survivors lie in code only the offer, x86-64 or the test guest reach, so the executor's tests cannot see them; they
are a committed plant list, `xw-plants/plants6.sh` style, run by hand on ARM64 at `b3c370030` against
`crates/arm64/tests/offer.rs` and `crates/translator-test/tests/test_guest.rs` (MEASURED, baseline both 0):

| Plant | offer | test_guest |
|---|---|---|
| arm64 `address_windows` -> true | 101 caught | 0 |
| `Guest::address_windows` default -> true | 0 | 101 caught |
| `NoneEmitter::check_range` -> 0, -> 1 | 0 | 101 caught, 101 caught |
| `FullEmitter::check_range` -> 0, -> 1 | 0 | 101 caught, 101 caught |

So after runs 4, 6 and 7 and the plants, every survivor left is one of the 21 equivalent mutants in the table above
(plus `copy_own_locked`'s chunk bound `- target` -> `/ target`, which only copies a page at a time). The
behavior-source suite and the full list were left out of the mutant runs for time (the suite alone passes 400 s); the
Step-mode stop count they reach became a flag, so it has no arithmetic left to mutate.

### Bar

| Machine, tip | fmt (7 crates) | clippy | translator, arm64, guest-cpu | executor suite / windows / space changes / direct | full list |
|---|---|---|---|---|---|
| ARM64 `51ba2acf6` | 0 | 0 | pass | 23 / 15 / - / 5 | "5 passed; 0 failed ... 6903.42s", FULL 0 |
| x86 `b3c370030` | 0 | 0 | pass | 23 / 19 / 25 / 5 | "5 passed; 0 failed ... 9073.98s", FULL 0 |
| x86 `273ff83b9` (after his main) | 0 | 0 | pass | 23 / 19 / 25 / 6 | "5 passed; 0 failed ... 7768.26s", FULL 0 |
| ARM64 `f7c400c42` (after his main) | 0 | 0 | pass | 23 / 19 / 25 / 6 | "5 passed; 0 failed ... 5958.07s", FULL 0 |

Cost per access, fastest of 5 runs of 50M rounds (MEASURED):

| Loop | ARM64 `06ebe56d5` | x86 `273ff83b9` |
|---|---|---|
| one word below 4 GiB | 0.54 ns | 0.00 ns |
| one word, window 1 | 2.83 ns | 2.73 ns |
| one word, top page | 2.75 ns | 2.73 ns |
| each word of a page, below 4 GiB | 0.63 ns | 1.06 ns |
| each word of a page, window 1 | 3.95 ns | 3.54 ns |
| each word of a page, top page | 3.93 ns | 3.63 ns |
| trap | 80.4 ns | 68.1 ns |

No change against the last run (0.54 / 2.80 / 2.76 / 3.94 ns ARM64): the reworked fast and exact paths cost nothing.

### The check's nonblocking list

1. Code above 4 GiB: still only data there; unchanged, in Next.
2. Containers never reach above 4 GiB (the kernel harness passes SPACE_32_BIT): unchanged, the kernel track's file.
3. t04, t06: t04 is now caught (the 64 KiB stepped loop); t06, t07, t09 are gone with their code.
5, 6. The list branch: not touched this run.
7. Linter at `51ba2acf6` (reviewable-rust da55b9d7, no key, whole workspace, x86): 105,916 findings in the workspace,
   none on a line the window work added in translator, arm64 or guest-cpu (MEASURED: findings matched against the
   added lines of `1242fecd5..HEAD`, 0 of 132 in those crates). The check's +28 were on lines that existed before.
9. `map_over`'s doc says readable again. The constants that exist only for the literal rule stay: the rule asks for them.

### Next

1. Speed outside window 0 (kept runs and group checks for placed addresses): changes window-0 code, so a fresh full list.
2. Code above 4 GiB: waits on a design choice (windows for the array of slots).
3. The 243 linter findings that wait on model answers.
4. The `Executor` shape he is deciding: every caller of `execute` here is already a loop of "run until the next trap, then
   ask the handler".

## X2, 2026-11-27: the list merged, a 2^48 space, the linter, the presented chip, the costs

His main `ac696ea19` is still in the branch (MEASURED: `git fetch <his checkout> main`, FETCH_HEAD `ac696ea19`, nothing to
merge). Every owner interface file is byte-identical to his main (MEASURED: `git diff ac696ea19 HEAD` over each
`crates/*/interface.rs` of his main is empty). Commits, all pushed: `1242fecd5` merges `arm64-instructions` 079919923;
`b9257a686` adds the access cost test; `732842ebf` the translator's address windows; `733297805` the executor's windows;
`dac03cd41` their tests; `5c7f30eb7` the linter work.

### 1. `arm64-instructions-for-wasmtime` merged into `arm64-instructions`

`arm64-instructions` 39ec6ca78 -> `233a8e39b` (normal merge of aba673597, "merge arm64-instructions-for-wasmtime into
arm64-instructions"), `4caea397f` (the build script as rustfmt wants it), `079919923` (the Apple M5 and Neoverse V2 chip
crates name no reservation granule: `reservation_granule_bytes: None`, as their other unmeasured choices). Pushed.

MEASURED on Graviton4 at `079919923` (log `fetched/list-merge/bar.log`, `tests.log`):

| Run | Result |
|---|---|
| `cargo fmt --check` of the arm64, Apple M5, Neoverse V2, collected and measured crates | 0 (build.rs:231 fixed) |
| clippy `-D warnings`, same crates, all targets | 0 |
| `cargo check` of the Apple M5 and Neoverse V2 crates | 0 (they did not build before) |
| whole list through the owner's `measure_instructions` (`this_chip_measures_as_its_baked_sample`, `CriticalValueCut`) | passes in 285 s: the fresh measurement of every listed instruction equals the baked Neoverse V2 sample, and `measure_instructions` returns no error, so no instruction disagrees; the sample holds all 4,430 listed instructions (`every_arm64_sample_holds_each_listed_instruction...` passes) |
| planted-bug harness (`planted_bugs`, one core) | 669 of 671 caught in 99 s, the same as at 39ec6ca78 (missed: 508 pointer authentication, 3618 SVE as before) |
| tests of the five crates, `--no-fail-fast` | all pass except `agrees_with_recordings` in Apple M5 and Neoverse V2, which compares their ten hand-written instructions with a recording of all 4,430; it failed the same way when those crates were restored (orchestrator log line 2078) |

`cargo fmt --check` of the whole workspace still differs in other tracks' files (x86 crates, direct3d-12, volatile-bytes
tests, two owner interface files `precise-instruction-set/interface.rs` and `privilege-level/interface.rs`); none is
mine and none was touched.

### 2. The top page of a 2^48 space: address windows

A space has the 2^48 bytes of a 48-bit virtual address. The design (contract in `crates/translator/src/contract.rs`,
"Address windows"):

- The addresses are cut into windows of 4 GiB (65,536 windows). The guest memory (the WebAssembly memory) holds eight
  of them, each at a place followed by 64 KiB whose first page shows the next window's first page. Window 0 is always
  at place 0; any other window gets a place when one of its addresses is first given bytes or permissions.
- The window table (65,537 i64s in the state memory, ending where the permission table starts) gives each window's
  place; a window with none gives a place past the permission table, whose pages allow nothing.
- What the generated code does to an address: nothing new on the fast path. The fast permission test reads the
  permission byte of the address as computed, but only within window 0's pages; any other address fails it. Its slow
  path untags the address as before and then places it: `place = table[min(address >> 32, 65536)] + (address &
  0xffff_ffff)` (one i64 load from the state memory, nine more operations), checks the place exactly, and the access
  uses the place. A fault is reported at the address as the program computed it. Group checks, frame checks, kept runs
  and loop proofs (which use the address as computed) are limited to window 0, so an address outside it never skips its
  check. An access across the end of a window reaches the next window's first page through the page after the place.
- The switch is the ARM64 offer: `Offer { addresses: Addresses::Windows, .. }`, which this executor gives
  (`Guest::address_windows`, default false). With the default offer the translator's output is byte-identical
  (MEASURED x86-64: `WC_OUTPUT_SETS=bench` output test, 505 modules, sha256 `62764764...2396` at `b9257a686` and with
  the change).
- The executor keeps page records, backings and permission bytes by place, splits every range at window ends, and
  borrows in one part per window. A ninth window is refused (`InvalidAddressError` from `set_permissions` and `borrow`,
  `OutOfMemory` from `map_from` and `copy_from`). Code still runs only below the array of slots' cover (just under
  4 GiB): the list's unexecutable state stays the one past it. The exclusive monitor's address is given back as the
  address, not the place.

What it costs per access (`tests/access_cost.rs`, fastest of 5 runs of 50,000,000 rounds, minus the same loop without
the accesses; MEASURED at `b9257a686` before and `dac03cd41` after):

| Loop | ARM64 before | ARM64 after | x86-64 before | x86-64 after |
|---|---|---|---|---|
| load, add, store one word, below 4 GiB (0x20000) | 0.54 ns | 0.54 ns | 0.00 ns | 0.00 ns |
| the same at 0x1_0001_0000 (window 1) | refused | 2.80 ns | refused | 2.76 ns |
| the same on the top page (0xffff_ffff_f000) | refused | 2.76 ns | refused | 2.76 ns |
| each word of a page in turn, below 4 GiB | 0.45 ns | 0.63 ns | 0.78 ns | 0.91 ns |
| the same in window 1 | refused | 3.96 ns | refused | 3.57 ns |
| the same on the top page | refused | 3.94 ns | refused | 3.55 ns |
| trap cost (`tests/trap_cost.rs`) | 80.8 ns | 80.8 ns | 73.6 ns | 70.9 ns |

Below 4 GiB the code is unchanged (the before and after rows differ within the noise of a loaded machine, the ARM64 one
at load 95 to 125). Outside window 0 every access takes the slow path, about 2.8 ns more; a kept run or group check
there would remove most of it, which is the next speed step (it changes window 0's code too).

The bar at `dac03cd41` (MEASURED):

| Run | ARM64 (Graviton4) | x86-64 |
|---|---|---|
| executor suite without the full-list tests | 23 of 23 | 23 of 23 |
| `tests/windows.rs` (top page, across a window's end both ways, faults, monitor, copy and map, ninth window) | 7 of 7 | 7 of 7 |
| slice 1, time slice 4, volatile-bytes 5, `wasmtime-direct` 5 | pass | pass |
| full list (`agrees` and `behavior_source`, `CriticalValueCut`) | **0 disagree**: 5 of 5 (`agrees_with_precise_instruction_set` and the behavior-source suite 4 of 4) in 6,491 s at load 84 to 125 | **0 disagree**: 5 of 5 in 4,754 s |
| containers through the kernel harness (tree = `dac03cd41` + `origin/kernel-port` f02018c89, `--features runtime`) | hello 1 of 1; busybox 3 of 4, the same 2 of 35 host-specific commands (`/proc/config.gz`, `/proc/sys`); counterclock 2 of 2 | not run |

At the tip `5c7f30eb7` (the linter work only) on x86-64: fmt 0, clippy `-D warnings` 0 over the translator, guest-cpu,
arm64, x86, executor and wasmtime crates, the suite 23 of 23 and windows 7 of 7, the translator's and arm64's tests pass
(MEASURED, `fetched/run10-x86/20261127-143058-20768.log`). The generator's states hold no address above 4 GiB (the slice
test: 78,836 states, 0 refused, before and after), so the full list does not reach outside window 0; `tests/windows.rs`
does.

Logs: `fetched/run10-arm/`, `fetched/run10-x86/`.

### 3. The linter (`cargo reviewable check`, reviewable-rust da55b9d7, no model key)

Findings in my two crates, MEASURED on Graviton4 (whole workspace run, findings by path). Before: 409 at `53cbce1e9`
(161 waiting on the model); 463 on the windows work before this pass (`fetched/lint10/`). After, at `5c7f30eb7`
(`fetched/lint12/`):

| Rule | At `5c7f30eb7` | Why left |
|---|---|---|
| `summary_unanswered`, `name_unanswered` | 243 | the model's answers (no key); the new code added names and summaries |
| `magic_literal` | 0 (was 75, then 87) | |
| `number_constant_without_link`, `verb_not_first`, `literal_restates_constant`, `function_summary_not_third_person` | 0 | tuning numbers link to the table below |
| `summary_not_one_line`, `summary_too_long`, `summary_starts_with_name`, `comment_too_long` | 13 | 11 in `crates/wasmtime/interface.rs` and 2 SAFETY comments of marked uses in the direct crate: the API crate's interface states the contract its marked uses rely on, so its wording is for the owner's review of that crate |
| `public_outside_interface` | 26 | owner question 5 (how an executor is built: `new`), derives a type adds beyond its trait, and the direct crate's types whose fields must stay private (they hold the host memory) |
| `conformance_not_run` | 11 | the executor suite's `tests!` takes four values and the behavior-source and volatile-bytes suites run beside it: the suite's shape, not this crate's |
| `constant_defined_twice` | 14 | each test file is its own crate and cannot name another's constant; four name other crates' constants |
| `implementation_folder`, `interface_item`, `comment_budget_exceeded` | 4 | `VolatileBytes` implemented outside its interface's folder (the API crate shape), the API crate's re-export and its comments |
| **all** | **311** (executor 239, `crates/wasmtime` 72) | 243 wait on the model, 68 on the reasons given |

The translator's own new lines (`crates/translator`, the address windows) were not counted here.

### Tuning numbers

Each tuning number's link points here. They were chosen, not measured, unless the row says so.

| Constant | Value | Why |
|---|---|---|
| `HOST_WINDOWS` | 8 | Linux places a program's text and heap, its mappings and its stack in three or four windows; eight is 32 GiB of host addresses per space |
| `KEPT_TRANSLATIONS` | 2^16 | a process may map only so many; the full list makes millions of single-instruction modules |
| `DEFAULT_SLICE` | 2^22 events | a few milliseconds of a short loop |
| `SCRATCH_STEPS` | 2^10 | bounds a scratch store's instances and slots |
| `BLOCKS_BYTES` | 1 MiB | thread blocks of a space |
| `CHUNK_PAGES`, `COPY_BYTES` | 2^12, 1 MiB | record chunk and copy buffer sizes |
| `PAGES_BEHIND`, `PAGES_AHEAD` | 1, 4 | the code a run-time translation sees around its start |
| `CPU_HOOK_ARGUMENTS` | 8 | room for the guest hooks' arguments; a test checks each fits |
| wasmtime suite `PAGES`, `GUARD_PAGES` | 4, 2 | small memories for the API crate's tests |

### 4. The chip this executor presents

READ from `crates/arm64/src/forms.rs` (`OFFERED_FEATURES`), the list's `PRESENTED_SYSTEM_REGISTERS` and the behaviors the
full list measured; the ID register fields are INFERRED from the Arm ARM.

| Feature | Presented | Decided by | ID register field | HWCAP |
|---|---|---|---|---|
| FEAT_FP, FEAT_AdvSIMD | yes | translator | PFR0.FP = 0, PFR0.AdvSIMD = 0 (no half precision) | `fp`, `asimd` |
| FEAT_CRC32 | yes | translator | ISAR0.CRC32 = 1 | `crc32` |
| FEAT_LSE | yes | translator | ISAR0.Atomic = 2 | `atomics` |
| FEAT_LSE2 | no | a misaligned ordered or atomic access is an alignment fault | MMFR2.AT = 0 | no `uscat` |
| FEAT_FP16 | no | the half-precision fixed-point conversions are undefined; FPCR keeps no FZ16 | PFR0.FP and AdvSIMD = 0 | no `fphp`, `asimdhp` |
| FEAT_AES, FEAT_PMULL | no | PMULL of 64-bit elements is undefined | ISAR0.AES = 0 | no `aes`, `pmull` |
| FEAT_SSBS | no | MRS and MSR of SSBS are undefined | PFR1.SSBS = 0 | no `ssbs` |
| FEAT_DIT | no | MRS and MSR of DIT are undefined | PFR0.DIT = 0 | no `dit` |
| FEAT_MTE | no | MRS and MSR of TCO are undefined | PFR1.MTE = 0 | no `mte` (HWCAP2) |
| FEAT_RNG | no | MRS of RNDR and RNDRRS is undefined | ISAR0.RNDR = 0 | no `rng` (HWCAP2) |
| AArch32 | no | FPSR's N, Z, C, V read zero | PFR0.EL0 = 1 | none |
| SHA1/2/3, SM3/4, SVE, PAuth, BTI, DotProd, FCMA, JSCVT, LRCPC, BF16, I8MM | no | not offered | 0 | none |
| CTR_EL0 | 0x8444_C004 | MRS CTR_EL0 | DIC = 0, IDC = 0 | |
| DCZID_EL0 | block of 64 bytes (BS = 4) | DC ZVA | | |
| TPIDRRO_EL0 | 0 | MRS | | |
| FPCR, FPSR bits held | 0x07C0_0000 (AHP, DN, FZ, RMode); 0x0800_009F (QC and the cumulative flags) | MRS and MSR | | |
| Addresses | 48 bits (this section's windows) | | MMFR0.PARange is the kernel's | |

For the kernel track: the call of the owner's interfaces that carries this chip is `BehaviorSource::measure_instruction`
(`crates/behavior-source/interface.rs`, which `Executor` extends): measured on this executor, MRS gives the
`SystemRegisterChoices` above, a misaligned LDAR the LSE2 answer, SCVTF (fixed point, half) the FP16 answer, PMULL
(64-bit) the PMULL answer. The kernel turns them into `ProcessorFeatures::Arm64(Arm64Features)`
(`crates/kernel/arm64_features.rs`) and passes it as `supported` to `ProcessorInputs::choose_processor_features`
(`crates/system/interface.rs`). No owner call returns `Arm64Features` from an executor directly.

### 5. The trap cost and a kernel call end to end

| | ARM64 (Graviton4) |
|---|---|
| one trap, answered from inside the run (`tests/trap_cost.rs`) | 80.8 ns |
| one `getpid` end to end through the kernel on this executor (throwaway `callcost` guest and test in the kernel harness, 200,000 and 400,000 calls, the difference, median of 5) | 3,629 ns (3,544 ns before the windows, at `b9257a686`) |
| the same on the stand-in | 21,042 ns (21,004 ns) |

The kernel's own handling, not the trap, is most of a call: 81 ns of the 3.6 microseconds. Logs:
`fetched/run10-arm/callcost-before.log`, `containers-after.log`. The guest and test are in
`wcontainers-scratch/executor-wasmtime/callcost/` (not committed: the harness is the kernel track's).

### Next

X2's bar holds by my measurement at `dac03cd41` on both machines; the independent check has not run. Left: the 243 linter findings that wait
on the model's answers; a kept run outside window 0 (speed); code above 4 GiB (the array of slots covers only the first
4 GiB). X3 (the stand-in) waits on nothing from this branch.

## X2, 2026-11-26 evening: the orchestrator's three rulings (newest first; the sections after this one are older)

His main `ac696ea19` is still in the branch (MEASURED: `git fetch <his checkout> main`, FETCH_HEAD `ac696ea19`,
`git merge-base --is-ancestor FETCH_HEAD HEAD`). Merged: `origin/arm64-instructions` 39ec6ca78 (`46d0dc453`), and the
branch `arm64-instructions-for-wasmtime` made for ruling 2, at 70a17afc3 (`bb11b9c05`) and bd9d83dd8 (`53cbce1e9`).
Every owner interface file is byte-identical to his main (MEASURED: `git diff FETCH_HEAD HEAD` over the 30 folders with an
`interface.rs` on his main, leaving out `implementation/` and `conformance/`, is empty).

### READY FOR A CONTAINER: `53cbce1e9`

MEASURED, Graviton4, throwaway tree = `53cbce1e9` merged with `origin/kernel-port` 353987bad (no conflict, no patch: the
kernel track's harness already runs the kernel on this executor, and d77525c9d answers the trapped registers);
`cargo test --release -p wcontainers-executor-unicorn --features runtime --test hello --test busybox --test counterclock`;
logs `fetched/run9-arm/hb9.log`, `hb9b.log`:

| Test | Result |
|---|---|
| `hello_matches_reference` | passes |
| busybox | 3 passed, 1 failed, 1 ignored: `system` differs in 2 of 35 commands, the same two host-specific ones as before (`/proc/config.gz`, `/proc/sys` values) |
| counterclock | 2 of 2 (`counter_never_goes_backwards` failed at `9802b9416` until the kernel answered the trap) |

### What changed (commits on this branch)

| Commit | What |
|---|---|
| `cd2b7fc8e` | ruling 1, translator side: the ARM64 offer `KeptRegisters::Trapped` (was `CounterAccess::Trapped`) raises `TrapKind::SystemAccess` for every access EL1 keeps from EL0 as Linux sets it: DAIF, DAIFSet and DAIFClr (SCTLR_EL1.UMA clear), SCXTNUM_EL0 (TSCXT set), the timers, reads of the activity monitors, the counter, and the whole identity register space (FEAT_IDST), including the ones Linux gives no value; DC CVAU is never checked (the list: it never faults at EL0) |
| `52406c3bc` | ruling 1, executor side: translates under `KeptRegisters::Trapped`; a trapped MSR (immediate) reports its op0; a fault of DC CVAC or CIVAC is the list's `Undecoded` cache maintenance syndrome, not a `DataAbort` |
| `bb11b9c05` | ruling 2: merges 70a17afc3, the list's behaviors of a chip without FEAT_LSE2 (a misaligned ordered or atomic access is an alignment fault), without FEAT_FP16 (the half-precision fixed-point conversions) and without FEAT_PMULL (PMULL of 64-bit elements) |
| `53cbce1e9` | ruling 2: merges bd9d83dd8, the list's MRS and MSR (register) of a chip without FEAT_SSBS, FEAT_DIT, FEAT_MTE, FEAT_RNG, FEAT_FP16 and AArch32 |

What the list lacked and why, rows and the manual's words: `wcontainers-scratch/executor-wasmtime/NEEDED-FROM-ARM64-LIST.md`
(sections 1 to 4). Branch `arm64-instructions-for-wasmtime` (two commits on 39ec6ca78, pushed), for the orchestrator to merge
into `arm64-instructions`. MEASURED for bd9d83dd8 on Graviton4: clippy `-D warnings` on the arm64, measured and collected
crates clean; tests of `processor-architecture-arm64` (49 + 3 + 1), `collected` (3 + 3), `measured` (4 + 3) and
`behavior-source-recording` (10 + 4) pass; `this_chip_measures_as_its_baked_sample` passes (the Neoverse V2 still
measures as its sample: the new behavior is listed third).

### For the kernel track: what a program on this executor must be told

It runs on a chip without FEAT_LSE2, FEAT_FP16, FEAT_PMULL, FEAT_SSBS, FEAT_DIT, FEAT_MTE and FEAT_RNG. The identity
registers it now traps (the kernel answers them) and AT_HWCAP/AT_HWCAP2 must say so: ID_AA64MMFR2_EL1.AT 0 (no
HWCAP_USCAT); ID_AA64PFR0_EL1.FP and AdvSIMD 0 (no HWCAP_FPHP, HWCAP_ASIMDHP) and DIT 0 (no HWCAP_DIT);
ID_AA64ISAR0_EL1.AES below 2 (no HWCAP_PMULL) and RNDR 0 (no HWCAP2_RNG); ID_AA64PFR1_EL1.SSBS 0 and MTE 0 (no HWCAP_SSBS,
HWCAP2_MTE). INFERRED from the list; not checked against `d77525c9d`'s answers.

### The bar now (MEASURED at `53cbce1e9` unless marked)

| Run | ARM64 (Graviton4) | x86-64 |
|---|---|---|
| executor suite without the full-list tests | 23 of 23 | 23 of 23 |
| `tests/slice.rs`, the ten slice instructions | passes (10 of 10) | passes |
| `tests/time_slice.rs` | 4 of 4 | 4 of 4 |
| volatile-bytes 5, `wcontainers-wasmtime-direct` 5 | pass | pass |
| trap cost (`cargo test --release --test trap_cost -- --ignored`, fastest of 5 runs of 1,000,000 system calls) | 81.0 ns (82.8 at `6d3e773dc`, 80.2 before X2's trap changes) | 72.9 ns (78.9 at `6d3e773dc`, 74.6 before) |

Logs: `fetched/run9-arm/`, `fetched/run9-x86/` (`cost9.log`, `rest9.log`, `full9.log`).

Full list (`cargo test --release --test conformance -- --test-threads 6 agrees behavior_source`, `CriticalValueCut`):

| Commit | ARM64 | x86-64 |
|---|---|---|
| `9802b9416` | 143 instructions disagree | 143 |
| `52406c3bc` (rulings 1 and the first list commit) | 2 disagree: MsrRegister 4,833 of 118,437 states, Mrs 1,120 of 130,737; behavior source 3 of 4 (65 min; log `fetched/run8-arm/full8.log`) | the same 2 and the same counts (60 min; `fetched/run8-x86/full8.log`) |
| `53cbce1e9` | **0 disagree**: `agrees_with_precise_instruction_set` passes, and the behavior-source suite 4 of 4 (`measured_behavior_agrees_with_the_source` passes too); 5 of 5 in 94 min (`fetched/run9-arm/full9.log`) | the same: 5 of 5 in 104 min (`fetched/run9-x86/full9.log`) |

The 2 at `52406c3bc`, by encoding (MEASURED: a throwaway test over every generated state of the two, not committed):
under the Neoverse V2 choices, MRS and MSR of DIT, SSBS and TCO are undefined here and complete in the list; MRS of RNDR
and RNDRRS are undefined; MRS CTR_EL0 reads 0x8444_C004 (the list had 0xB444_C004 in the generic MRS, though its own
`MrsCtrEl0` already had the presented value); TPIDRRO_EL0 reads 0; MSR FPCR keeps no FZ16 and MSR FPSR no N, Z, C or V.
Every one is a chip without those features, which the list did not have: bd9d83dd8 adds it. The same test at `53cbce1e9`:
the new behavior meets every state the executor ran of both instructions (no encoding shows a disagreeing state).

`36f4725b4` (merges aba673597) changes only the comments of the new list items, for the linter; the full list was run
at `53cbce1e9`.

### The linter (`cargo reviewable check`, reviewable-rust da55b9d7 built on the machine, no model key)

MEASURED at `53cbce1e9`, findings by path over the whole workspace's run (102,561 in all): `crates/executor/implementation/wasmtime`
307, `crates/wasmtime` 102 (of these 409: 161 need the model's answer, `summary_unanswered` 111 and `name_unanswered` 50;
then `magic_literal` 75, `summary_not_one_line` 44, `summary_too_long` 32, `public_outside_interface` 26,
`number_constant_without_link` 18, `function_summary_not_third_person` 18, `conformance_not_run` 11, and 25 more of
nine kinds). The list's `system_register.rs` went from 117 to 111 with aba673597; the 6 left on the lines I added are 4
`public_outside_interface` (every field of that crate's types is `pub`) and 2 that need the model's answer. Working the
409 down is not started: it is the linter part of X2's bar.

### Next

X2's agreement bar is met by measurement at `53cbce1e9` on both machines: 0 disagreeing instructions on the full list, the
executor suite 23 of 23 plus the 2 full-list tests, the behavior-source suite 4 of 4. `e8806f744` changes only
function summaries (fmt, clippy and the suite without the full list rerun on x86: clean, 23 of 23). Left of X2: (1) the linter's 409 findings in the two crates (mine; 161 wait on a model key or the orchestrator's answers);
(2) "the top page of a 2^48 space accepted": a space is still 4 GiB, a change of the translator's contract (translator
track); (3) the independent check of this section. X3 (the stand-in) is on `executor-restart`; it waits on nothing from
this branch beyond `53cbce1e9`.

## X2 continued, 2026-11-26 afternoon

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
- 1: Sys. At `e4a430b6c` its first differing state was IC IVAU (`d50b7520`) of an address above 48 bits, which the list
  says never faults and the translator checked: fixed in `9802b9416`.

The DAIF and ID-register causes are one question, for the orchestrator: should the executor trap every register the
list says EL1 traps (DAIF, DAIFSet/Clr, SCXTNUM_EL0, the timers, the activity monitors, the ID space), as it now does
the counter, so that the kernel answers them as Linux does (ID registers emulated, the rest SIGILL)? It is the same
offer switch in the translator (`CounterAccess::Trapped` would grow into "EL1 traps as Linux sets it"); it needs the
kernel to answer `SystemAccessTrapped` first, since a program that reads MIDR_EL1 (glibc does, given HWCAP_CPUID)
would then stop the kernel where today it reads Linux's value.

Re-measured at `9802b9416` (MEASURED, both machines, logs `fetched/run7-arm/full7.log`, `fetched/run7-x86/full7.log`):
33,322,792 states executed, 0 refused, 143 instructions disagree on each machine (49 and 50 min); the behavior-source
determinism tests pass on both. The causes are the four above; Sys's first differing state is now DC CVAC (`d50b7a20`)
of an unmapped address: the list gives the cache maintenance abort as `Undecoded { syndrome: 0x92000147 }` (EC 0x24,
CM and WnR set, a level 3 translation fault), the executor a `DataAbort`. Its fix is two small parts: the executor
reports a fault of DC CVAC or CIVAC as that `Undecoded` exception (mine, next), and DC CVAU, which the list says never
faults, stops being checked (translator: `CleanCache` does not tell CVAU from CVAC today, decode_forms.rs:191).

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
