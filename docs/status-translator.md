# Translator status

Plan: docs/phase-1-translator.md. Branch: phase-1-translator; since the relaunch of 2026-11-25, translator-port.

## Relaunch of 2026-11-25 (branch translator-port, from main at 9fe98a6a6)

Brief: pyro-parked/wcontainers-scratch/orchestrator/restart-2026-11-25/brief-translator.md. Results and logs in
scratch `translator-port/`.

Nothing lost: b069b3d8 brings the five crates from origin/agents at 198bb11d6 by path, with the lock file and the
x86 native output plan and status; `git diff 198bb11d6` of the five folders is empty, and 3a5fda94, c027d2e0 and
e8f158ec are in it. 62abb61d prunes the lock file to this branch's crates.

Baseline (62abb61d, rented x86-64 machine unless said):
- `cargo check --all-targets --all-features` of the five packages: exit 0 on both machines. Clippy with warnings
  denied, `--no-deps`: exit 0. The browser translator for x86-64 did not build (iced's `Instruction` has no
  `Display` without its formatter, in `carry.rs` and `carry/window.rs`), so 10 browser and binaries tests failed.
- Layer 2 and the other tests that translate from several threads at once hung on both machines: the
  translator's first translation in a process held its pool's lock while the new pool's threads started, and
  the waiting thread ran another case of its own pool, which asked for the same lock (gdb, all threads in
  `on_translator_threads`).
- `unsafe` lines outside comments and attributes (the root manifest's command of 198bb11d6): arm64 0, x86 0,
  guest-cpu 0, translator 0, translator-test 15 (runner.rs 8, test_guest.rs 2, reciprocals.rs 1, the guest
  program programs/panics.rs 4), and 16 `#[unsafe(no_mangle)]` in browser/; reference/ 86.
- `cargo reviewable check` (the installed linter's commit 59ad72ce, no model key on the machine): 12,049 findings
  in the five crates: translator-test 4,583, x86 3,144, arm64 2,858, translator 951, guest-cpu 513.

TR1 (de124d4a, 19afeb11, f25b550c, 937e6f10):
- The state memory is reached only through a small module of WebAssembly's own loads and stores (words, bytes,
  a fill, and the two atomic stores) in a store of its own (`src/state_memory.rs`, a203bd58), with wasmtime's
  safe calls; hooks and steps work on a copy and write back only the bytes they changed. No executor is needed.
  A first form with a scratch memory of its own (de124d4a) reserved 4 GiB more for each machine and ran out of
  address space in x86-64's layer 2 (462 of 10,000 cases "mmap failed to reserve"; the baseline at the same seed
  had 0); a203bd58 removes it. The guest hooks use `func_new` with `Val`; the
  reciprocal test asks the CPU through safe_arch 1.2.0 (Zlib OR Apache-2.0 OR MIT); the browser translator's
  exports are `#[wasm_bindgen]` (0.2.130, MIT OR Apache-2.0), named by wasm-bindgen-cli-support 0.2.130 (MIT OR
  Apache-2.0) and loaded by the page without wasm-bindgen's JavaScript; the browser crate forbids unsafe code.
- The stop raiser from another thread sets the limit to 0 when the count is kept (it stored u32 1 in the u64
  limit): `stop_raised_while_running_with_the_count_kept_*` failed before (count 0, not 1) and passes.
- The translator makes its pool with no lock held: `tests/translator_threads.rs` timed out after 120 s before and
  passes in 0.15 s. The x86 carrying code's two panic messages use `Debug`, so the browser build compiles.
- translator-test is on `[lints] workspace = true`. Left: programs/panics.rs, a Rust guest program that rustc
  builds for the guest (cargo never compiles it in the workspace), needs `unsafe impl GlobalAlloc` and an
  unmangled `main`, since the stand-in kernel has no mmap and std's own start asks for more calls.
- The root manifest of main has no TODO list of crates, so there was no line to remove.

TR1's measurements (rented machines, 12 jobs; "the baseline" is 62abb61d with the two fixes above, as the bare
baseline hangs and its browser build fails):
- check, clippy `--no-deps` with warnings denied (all targets and features), the browser crate's wasm32 clippy for
  both guests and the reference's clippy: exit 0 on both machines.
- `unsafe` outside comments: 0 in the five crates' compiled files; programs/panics.rs 4; reference/ 86.
- The whole suite of the five packages, test by test, against the baseline: every outcome the same on both
  machines (ARM64 machine 51 failed, x86-64 machine 36, all the other guest's reference tests and the known
  machine differences); the three new tests pass; no test removed or ignored (295 to 298, 313 to 316 listed).
  The native output tests (x86 crate's native_* and the arm64 way) pass as at the baseline.
- ARM64's output lists, every look-ahead, speed and size, the benchmark set and the nine real binaries (10
  lists, 965 modules): the same sha256 for every list as the baseline's.
- Layer 2, 10,000 programs, seed 25112502, all on the CPU, at a203bd58: ARM64 0 failing (3,392 faulted, 519 left
  to an address); x86-64 0 failing (3,442 faulted, 1,104 left), the same counts as the baseline's run.
- Layer 3, cases 0 to 1,199, seed 1: ARM64 at a203bd58 1,023 compared, 177 did not end in time, 0 failing;
  x86-64 at a203bd58 1,023 compared, 177 did not end, 0 failing.
- Layer 1 on the rented CPUs is not a verdict: ARM64 (1,000 cases a form, seed 25112503) fails only
  `MSR_SR_systemmove`, the Graviton's FPSR bits 28-29; x86-64 (64 a form, seed 1) fails 1,126 of 2,008 forms on
  the 5-level paging and feature differences of the Sapphire Rapids machine. Both lists are the baseline's, form
  for form. Layer 1's verdicts were the Mac's and the i7-8750H's, which this brief does not use.
- `cargo reviewable check` (59ad72ce): 12,072 findings in the five crates (baseline 12,049): 25 more names and
  summaries without a model answer (no key on the machine), 1 more `stray_source` (state_memory.rs), 3 fewer
  others.

TR1 under the orchestrator's two rulings (2026-11-25, 1b3b7578 to d714af58; 9 jobs; results in scratch
`translator-port/` `exp-entry/`, `tr1c/`, `tr1d/`, `tr1e/`, `l1/`):
- Ruling 1, the entry symbol: done with no unsafe attribute. The Rust program is built as a library whose public
  `entry` keeps a global symbol, found in the rlib (object's archive reader), and a root of two lines links it
  with `-C link-arg=--defsym=main=<that symbol>`. Tried on the ARM64 machine (exp1-arm.log): one binary crate
  with a `#[used]` static and `--defsym` fails (rustc makes `entry` a local symbol: "symbol not found"); the
  library and `--defsym` runs (exit 7 as asked); an assembly stub branching to the library's symbol runs too;
  `global_asm!` with `sym` is flagged by `-F unsafe_code` ("using this macro is unsafe even though it does not
  need an `unsafe` block"), as is edition 2021's plain `#[no_mangle]`; std's own start makes ppoll,
  rt_sigaction, sigaltstack, mmap and mprotect before `main`, which the stand-in does not answer. panics.rs is the
  only Rust program in programs/.
- Ruling 1, the allocator: not done; its four lines stay. Without them the program's own values are all on the
  stack, but the standard library allocates every panic's message and exception and stdout's buffer through
  the system allocator, musl's malloc, which takes pages by mmap: natively the heap-free program makes 34 mmap
  and 32 munmap calls and prints the same output ("caught 32, dropped 380"); on the stand-in (no mmap, a break
  that never moves) it ends at once with "memory allocation of 8 bytes failed" and status 139 (ARM64,
  unwinding_matches_linux_arm64, exp3-arm.log). Removing them needs either an anonymous mmap and a moving break
  in both stand-ins (runner.rs and browser/runner.js, with the pages' permissions), or no Rust unwinding test.
- The cases, before and after (the same program text below the entry): 32 rounds, round i diving i mod 9 frames
  (0 to 8) and panicking with "bottom 0" at the bottom; each caught by `catch_unwind`, its message read from the
  payload, every frame's guard dropped on the way; the output 32 lines "bottom 0 at i" and "caught 32, dropped
  380"; at -O0 and -O2, position independent builds left out as before (rustc's aarch64 musl target makes them
  fixed). unwinding_matches_linux passes on both machines at d714af58.
- Ruling 2, layer 1's failing forms, tip against the baseline, form by form (scratch `layer1_lists.py`,
  lists in `l1/`): ARM64 machine, 1,215 forms, 64 cases a form, seed 1: `["MSR_SR_systemmove"]` at both, equal;
  in Chrome 1 a form: `["MSR_SR_systemmove"]` at both, equal. x86-64 machine: 1,126 of 2,008 forms at both,
  equal (none only before, none only after); the unoffered forms 1,820 of 2,986 at both, equal; in Chrome 41 of
  2,008 at both, equal. "Baseline" is 62abb61d with the two fixes; the bare 62abb61d gives the same two
  non-Chrome lists on each machine (its browser build fails, so it has no Chrome list).
- The whole suite at 1b3b7578 against the baseline, test by test: ARM64 machine every outcome the same (51
  failed, all the other guest's reference and the Graviton's FPSR), 3 tests added (295 to 298); x86-64 machine
  the same but for layer 3's three x86-64 Csmith tests, which failed on a race of this harness: the three run the
  same cases at once and Csmith wrote each case's source to one shared path, so one compiled a file another was
  writing ("undefined symbol: main"). 9a184968 writes each to a file of its own and moves it into place; the
  layer 3 test binary then gives the baseline's outcomes (x86-64 3 passed, ARM64's 3 failing for want of a
  reference, as before).
- check and clippy with warnings denied over all targets and features, the browser crate's clippy for both
  guests and the reference's clippy: exit 0 on both machines; fmt exit 0 at d714af58.
- ARM64's ten output lists (965 modules): the same sha256 for every list as the baseline's.
- `cargo reviewable check` (59ad72ce): 12,070 in the five crates (12,072 at 2e027019, baseline 12,049).
- `unsafe` outside comments: arm64 0, x86 0, guest-cpu 0, translator 0, translator-test 4 (programs/panics.rs,
  the allocator; 5 before, with the attribute); reference/ 86.
- So TR1's bar does not hold by this measurement: the four allocator lines remain. TR2 is not started.

## For the orchestrator

- C1 touched two places outside this track's crates, to keep the workspace building: wcontainers-wasmtime's
  one `translate` call and one `thread_module` call now pass `ProgressCount::Absent` (its behaviour is
  unchanged). The runtime must pass `Kept` when `ReplayDeterminism::Exact` (or a host with one thread) asks
  for it, to both, and make it part of its cache key; the description section's last byte says which.
  Names to rule on: `ProgressCount { Absent, Kept }`, `contract::progress_limit_offset`,
  `contract::progress_count_offset`; `translate` now has eight parameters, and grouping look-ahead, optimise
  and the count into a `TranslationOptions` would be the owner's call.
- C1 measured the leaf-call stall on this Mac (one test binary, 4 jobs, about two minutes a run, load 3 to
  7): it is a property of Apple's core, which no rented machine has.
- C1's choice of emission (see "C1"): the count as the pieces' last parameter (E-param), passed through the
  thread block only on tail calls off the hot path. E-local cost leaf calls 4.4 times on Apple's core; plain
  E-param cost hash 1.67 times on the Graviton (a spill), which the block on rare tail calls removes (1.00).
  The price: about 1 percent on the M5 and x86 means, and search on the Graviton 1.18 to 1.40, from where
  its unchanged loop lands, not from its code. Please rule if you see it otherwise.
- Wasmtime's ARM64 code is sensitive to where a hot loop lands: search's inner loop is the same 28
  instructions at two commits, and costs 1.18 or 1.40 times its count-free form on the Graviton, 0.99 or
  1.06 on the M5, with the loop at 0x1c30 or 0x1c1c. Any change to code before a loop can move it so;
  the translator does not choose loop placement today.

- Please rule how T3's speed bar ("the benchmark set's speed is unchanged or better") is read. Against the
  merge (97fff482) it holds: every workload in both engines within 0.994 to 1.004. Against f6e760ae (the
  proof's code, before the merge) it does not: the fourth independent check measured fib in Chrome 1.7
  percent slower (4 of 4 rounds), interpret in wasmtime 0.5 to 0.6 percent slower and sort in Chrome 0.2 to
  0.4 percent slower (4 of 4 each), everything else 0.995 to 1.001 (rented ARM64 machine, 12 jobs, load 3
  to 32, scratch `check-t3d/pairs-summary.txt`). Interpret in wasmtime is also 0.1 to 0.4 percent slower than
  the merge in 4 of 4 rounds. This track's own pairs had listed search in Chrome and matrix in wasmtime as
  slower too and missed sort in Chrome; the check's figures are the better ones. The fib step comes from the
  merge (main's test harness): the translator's output for the benchmark set is the same at both commits.
- No container of this track is left on the Mac's Docker (`docker ps -a` shows none named translator,
  2026-11-20 10:40 local); the layer 1 containers an earlier entry named are gone.
- The P2 check's `wcontainers-abi` constants: this track added `PAGE_SHIFT` and `PAGE_MASK` to wcontainers-abi
  (06187cfd), as fix-translator.md says, but magic-literals/README.md gives that stage to the runtime track.
  They are defined exactly as the README's stage 1 writes them; when main's own arrive, keep main's lines in
  the merge. Taking them out here would add their literals back to the baseline, which the check refuses.
- The Chrome runner (runner.js, test tooling only) changes V8's speed on the translated code by up to 5 percent
  depending on how its `run` function takes its arguments (an extra destructured parameter, or a wrapper that
  reads a property inline, slows interpret by 4.6 percent; a module variable does not). Measured, not
  understood; recorded so nobody reads such a difference as the translator's.
- FPSR: a program whose code reads FPSR has its flags kept exact from its start, decided by
  `wcontainers_arm64::reads_float_status(code)` and applied by whoever starts the thread (`FLOAT_TRACKING`
  set to 1 before it runs). This track's runners do it; the runtime's CPU side (wcontainers-wasmtime, not
  this track's) must call it for each program it starts, or a program that reads FPSR first still gets 0
  there. Musl's dynamic C library holds `fetestexcept`, so applied to every loaded object it would make every
  dynamically linked musl program take the exact floating point path: the runtime should decide on the main
  executable and what it imports, and the owner may want to rule on that trade.
- No `...Settings` type is in this track's crates (`TranslatorSettings` is the runtime's, in
  wcontainers-wasmtime), so the rename to `Config` has nothing to do here.
- The literal check was adopted by merging origin/phase-1-literals (7668c010) directly, as the brief says,
  not through main as docs/literals.md step 1 says; init ran once, in the commit right after the merge
  (65ba8f8a). Changes that could add lines to the baseline, as the check lists them: fc5f55ca (the start),
  65ba8f8a (the init after the baseline arrived by a merge), 68214b2e (the start), 89f88281, 7119e3f3,
  c3bf56d4, 31ec396f, 0f187563 (new rules, for literals already in the tree).
- Docker's VM stopping (brief, 2026-11-19 05:10): no test of this track can power off, reboot or kill the
  first process of Docker's VM. Every container it starts (`programs::run_container`, `reference.rs`,
  `layer3.rs`, `binaries.rs`) is a plain `docker run` with Docker's default capabilities, no
  `--privileged`, no `--cap-add`, no host namespaces, and (except `binaries`' image set-up) `--network
  none`; the programs run are Csmith programs (libc's output and exit), generated instruction blocks (whose
  only system calls are the harness's own write and exit) and the binaries test's ordinary commands.

## Where things stand

M0 to M5, S1, P1, P2 and T3 are done. C1 (the progress count in the WebAssembly output) is done and waits
for its independent check: see "C1". The run-by-run records of every milestone up to T3 (each measurement with its machine and load, each
independent check's findings and what was done) are in this file's history:
`git show 26e3d848:docs/status-translator.md`. What is done is listed under "Done".

## T3 in short (corrected by the fourth independent check)

- Main merged (6a981a30: the translator generic over `wcontainers-guest-cpu`), the literal check adopted
  (65ba8f8a), layers 1, 2 and 3 rerun on the merged code with no mismatch (layer 1 on the Mac at seeds 300
  and 600, layer 2 100,000 programs at seed 300, layer 3 5,287 compared at seed 300; layers 2 and 3 again at seed 600 in the eighth run).
- Busybox awk's "jump through a register not recognised" fell from millions to a small remainder that
  differs between runs of the same code. Exact counts measured on the three commands of `t3/awk.sh` (seq|awk
  sum, sprintf loop, array concat), all with the same outputs as before: 5,306,745, 8,601,492 and 2,621,222
  at 97fff482; since 40fd0c67, 8, 2 and 7 (sixth run) and 11, 2 and 9 (fourth independent check, rented x86
  machine, load 7 to 18). Five more commands of that check: 61,857,670 to 2; 13,466,548 to 5; 4,862,634 to 7;
  6,185,776 to 2; 10,752,258 to 2. So "single figures" is not always true; the remainder is up to about a
  dozen and varies with what is made at run time.
- Which look-ahead counts that slow path: only the full emitter (`FullEmitter`, which serves full and
  bounded look-ahead) recognises switches and emits the count (`full.rs`, two sites). Look-ahead none
  recognises nothing and never counts it: every jump through a register there simply goes through the
  lookup (the plan's rule for none). The sixth run's record said a stretch at none counts every jump within
  the code; that was wrong for none, and holds only for stretches made by the full emitter (bounded, and
  pieces made at run time at full). Measured by the check: `c4_tailptr` (18,000 register tail calls within
  the code) counts 0 at none.
- FPSR read first fixed (see "For the orchestrator"); the plan has "Two workarounds for what WebAssembly
  lacks".

## C1: the progress count in the WebAssembly output

First run (2026-11-20, from 10:40 local), then the second (from 12:20 local, below).

Done:
- The fourth T3 check's findings: `Omit::bits`/`from_bits` exist only with the `measure` feature (clippy on
  the translator alone now passes, and the lint script checks it alone too); the register jump test's first
  layout is back as a known failure (`a_tail_call_to_a_function_only_data_names_is_not_unrecognised`,
  ignored with its reason, 3,600 against 2,400 on the rented ARM64 machine); the speed ruling, the awk
  figures, look-ahead none and Docker corrected above; this file cut from 3,630 lines to about 1,050 (the
  per-run history is in git, see "Where things stand").
- The count, definition E (`code::progress_event`, from `Guest::flow`: a direct jump with target at or
  below itself, taken or not, every call, return and jump through a register or memory), counted once per
  instruction at its first way out (`jump`, `jump_if`, `call`, `ret`), after its memory checks. Asked for by
  `translate(.., ProgressCount::Kept, ..)` (a new parameter after `Optimise`; the description section ends
  with it, version 4). Limit at 800, count at 808 (`contract::progress_limit_offset`/`progress_count_offset`,
  from `Guest::STOP_FLAG_OFFSET`). Stop check: `push_stop_wanted` (flag, or count >= limit); raising the stop
  writes limit 0. First form, "E-local" (844c553d, replaced, below): at full and bounded a piece kept the
  count in a local loaded at entry, written back with the registers and loaded again after a call; at none
  every event added to the block.
- Runner: `Machine::keep_progress_count`, `set_progress_limit`, `progress_count`; `Paused::progress_count`,
  `set_progress_limit`, `carry_on_at_none` (leave and carry on with pieces made at none only), `end_run`.
  `progress::reach` reaches a position (see the plan, "The progress count").
- Tests (`tests/progress_count.rs`, rented ARM64 machine, release, 4 test threads, load 12 to 30): hand count
  of `progress_events.S` (5 rounds: 4 x 5 + 1 = 21) at none, bounded 256 and a drawn N, full, speed and size:
  passed. The count at every system call and at the end is the same at those eight and twice each, for 8
  programs (progress_events, leafcalls -O0/-O2, bench SCALE=0 -O0/-O2, jumps, tables, exceptions): passed.
  Every stop of bounded and full stopping at every check is a position of none's run, in order (3
  programs): passed. One stop a program and look-ahead (seed 1), logged at each of the eight, reached from
  each of the eight: passed (469 s with two stops each). Csmith (layer 3's cases, seed 1, 12 programs): 11
  compared, 1 reached 2^32 events at full and was left out, 0 failing.
- The benchmark set's counts are the prototype's exactly (fib 1,300,483,329, sieve 879,049,018, hash
  409,802,086, sort 477,041,232, interpret 758,494,538, matrix 268,224,251, search 901,000,429): an
  independent cross-check of definition E.
- Form "E-param" (731fd2d8), the one kept: the count is the pieces' last parameter (`Layout::L_COUNT`) and
  last result, in the local of that parameter, so no call stores or loads it; the thread block gets it only
  where registers are saved (`save`, `save_passed`, `depth_check`, the dispatcher and exact paths), and the
  bottom loop passes it in and stores it back. `thread_module::<G>(ProgressCount)` now takes the choice (the
  piece type differs). At none too the count is that local. E-local (844c553d) is gone.
- Cost against today's output, `measure_progress_count` (full, speed, wasmtime, best of 5, alternating,
  with over without), E-local first, then E-param:
  - ARM64, this Mac (Apple M5 Max), load 3 to 7 (run here because the stall is a property of Apple's core;
    no rented machine has one): fib 1.071 / 1.051, sieve 1.053 / 1.023, hash 0.994 / 1.016, sort 1.022 /
    1.020, interpret 1.078 / 1.070, matrix 1.040 / 1.051, walk 1.019 / 1.000, search 0.968 / 0.972; leaf
    calls below 4.447 / 1.109, above 3.732 / 1.101. Geometric mean of all ten 1.356 / 1.040.
  - ARM64, rented Graviton4, load 12 to 43: fib 1.154 / 1.048, sieve 1.152 / 1.156, hash 1.000 / 1.675 (9
    rounds again: 1.675), sort 1.019 / 0.982, interpret 1.076 / 1.067, matrix 0.988 / 0.989, walk 0.997 /
    0.995, search 1.471 / 1.177 (again 1.180); leaf calls 1.074 / 1.150, 1.083 / 1.126. Geometric mean 1.094
    / 1.123.
  - x86-64, rented machine, load 2 to 50: fib 1.052 / 1.043, sieve 0.979 / 0.950, hash 0.999 / 1.000, sort
    1.035 / 1.030, interpret 0.988 / 0.984, matrix 0.948 / 1.009, walk 0.997 / 0.991, search 1.118 / 1.109;
    leaf calls 1.066 / 1.070, 1.072 / 1.121. Geometric mean 1.024 / 1.029.
  - The leaf-call stall (E-local on the M5): a leaf call stored the count, the leaf loaded it, added and stored
    it, and the caller loaded it again, two store-to-load round trips a call, which Apple's core pays for
    heavily and the Graviton hardly. E-param removes it (4.4 to 1.11 times).
  - Hash on the Graviton under E-param: wasmtime's register allocator spilled the hash value in the inner
    loop (`ldur x4, [sp]` ... `stur x4, [sp]` each turn), a store-to-load chain the M5 does not feel. Removed
    in the second run (below).
  - E-param was chosen over E-local: its worst case (1.675, one program on one host) is far below E-local's
    (4.4, every leaf call on Apple's core).
- Tests on E-param (rented ARM64 machine, release, 4 threads): all five of `progress_count` passed (268 s).
- Layer 1 on the Mac on 7d66c447 (seed 1200, 1,000 cases a form, 10 a form in Chrome, 4 jobs, load 6 to 10,
  888 s, `c1/mac-layer1.log`): 1,215 forms, 0 failing; 12,150 cases in Chrome, 0 failing; 1,000 words in no
  form illegal as on the CPU; 7 passed.

- The long checks on 7d66c447 (E-param), rented ARM64 machine, 12 jobs, load 20 to 29, 19:34 to 19:59 UTC
  (`c1/checks.sh`, results in `c1/checks-2/c1`):
  - Csmith, layer 3's cases 0 to 999 of seed 1 (`WC_PROGRESS_CSMITH=1000`): 852 compared (the count at every
    system call and at the end the same at none, bounded 256 and a drawn N, full, speed and size, and full
    twice; results the same as without the count), 148 reached 2^28 events at full and were left out, 0
    failing (1,018 s).
  - Layer 2, 10,000 programs, seed 1100, every one on the real CPU: 0 failing (3,466 faulted, 748 left to an
    address at full; 244 s).
  - `progress_count`: 5 passed, 1 ignored (the measurement) (249 s).
  - The first try (E-local, limit 2^32) was stopped after an hour: some Csmith programs at none ran very
    long below that limit.

- The whole suite of the four crates on 25eb5eae (release, `--no-fail-fast`, rented ARM64 machine, 24 jobs,
  12 test threads, 20:00 to 20:23 UTC, load 8 to 22, `c1/suite.log`): every test binary passed but layer 1's,
  where `every_covered_form_matches_the_cpu` and `sample_matches_the_cpu_in_chrome` failed, as in every
  Graviton run since T3 (the Graviton's FPSR difference; this log's filter kept only the panics' lines, so
  the form was not read from it). Layer 1's verdict is the Mac's, above: 7 passed. progress_count 5 passed,
  programs 22 passed (3 ignored, one being the register jump known failure), layer 3 3, layer 2 1, changes
  9, checks 12, browser 1, binaries 2, smoke 6, test_guest 16, valid 7, decoding 6.

Second run (2026-11-20, from 12:20 local, machines' clocks in UTC):
- Csmith on the first run's code (3848a3f3), layer 3's cases 0 to 1,199 of seed 1 (`WC_PROGRESS_CSMITH=1200`,
  rented ARM64 machine, 24 jobs, load 5 to 32, 646 s, `c1/checks-3/c1`): 1,023 compared, 177 reached 2^28
  events at full and were left out, 0 failing.
- The hash spill, found with the machine code tool (`c1/mc3`, `c1/mc3.sh`) by editing the hash piece's
  WebAssembly by hand and compiling it with the runner's engine: the spill goes when the count is not passed
  to the piece's tail calls that hand the loop on (to its checked copy, to a stop), and stays when the count
  is moved to a register parameter or reloaded after a stop. So the emitted form now passes the count
  through the thread block on tail calls off the hot path (`FullEmitter::rare_arguments`: to a stop, to a
  loop's checked copy when its proof fails, to exact code), and code that carries on after a stop or a
  system call loads it from the block (`reload`). Hot paths (calls, returns, gotos, lookups) are unchanged.
  Hash's inner loop then has no stack access (ef3c2b22).
- Its cost against today's output, paired runs of the same tool (best of 5, alternating), first the first
  run's E-param, then this form, each measured twice on each host (the second, paired run shown; on the
  Graviton the two agreed within 0.02 for every program, on the M5 within 0.03, on the busy x86 machine up
  to 0.06, in sort and hash; logs `c1/measure-*`):
  - Graviton4, 12 jobs, load 7 to 14: fib 1.046 / 1.048, sieve 1.163 / 1.137, hash 1.674 / 0.999, sort 0.982
    / 0.982, interpret 1.068 / 1.070, matrix 0.988 / 0.989, walk 0.983 / 0.987, search 1.182 / 1.404, leaf
    calls 1.179 / 1.193 and 1.124 / 1.124. Geometric mean 1.125 / 1.087.
  - M5 (this Mac, 4 jobs, load 5 to 7): fib 1.055 / 1.042, sieve 1.014 / 1.044, hash 1.012 / 1.008, sort
    1.028 / 1.028, interpret 1.064 / 1.071, matrix 1.050 / 1.071, walk 1.017 / 1.006, search 0.986 / 1.060,
    leaf calls 1.093 / 1.091 and 1.100 / 1.105. Geometric mean 1.041 / 1.052.
  - x86-64 rented machine, load 20 to 59 (busy): fib 1.026 / 1.039, sieve 0.956 / 0.970, hash 1.048 / 1.004,
    sort 0.969 / 1.075 (first pair 1.030 / 1.032), interpret 0.970 / 1.004, matrix 0.990 / 1.007, walk 1.026
    / 1.035, search 1.115 / 1.155, leaf calls 1.042 / 1.032 and 1.102 / 1.084. Geometric mean 1.023 / 1.039.
  - Search: its inner loop is the same 28 instructions in both forms (`c1/mc4h` against `c1/mc4`), at 0x1c30
    before and 0x1c1c now; its count-free time is the same in both. So its 1.18 to 1.40 on the Graviton (and
    0.99 to 1.06 on the M5) is where the loop lands, which this change moves but does not choose. Measured
    alone, not proven by moving the loop by hand.
  - Kept: this form, for removing a spill from a hot loop by construction; the owner may rule otherwise.
- The machine code tool names each piece by its real function index (it printed 8 + i, now
  `contract::first_piece_function`, 27 for hash where it said 10), reads the program's symbols with the
  `object` crate instead of `nm`, names its strings once, and compiles a module given as `<x.wasm>
  <out.cwasm>`, for trying another emission by hand (c5267646).
- The plan's "The progress count" now says how the count travels (it still described E-local).
- The native track's index of forms (brief, 08:40): applied as its own commit (71e9be93). Each form goes
  into every bucket whose key agrees with its fixed key bits, walking the subsets of its free bits, in the
  same order as before, so the buckets are the same lists; checked by layer 1 on the Mac and the decoding
  tests (below).
- Closing checks on 71e9be93 (this form and the forms index):
  - Rented ARM64 machine, 12 jobs, 20:56 to 21:28 UTC, load 3 to 18 (`c1/checks4.sh`, results
    `c1/checks-4`): `progress_count` 5 passed, 1 ignored (the measurement), 245 s: the hand count, the count
    the same at none, bounded 256 and a drawn N and full, speed and size, twice, for its 8 programs, the stops
    of bounded and full on none's run, and every logged stop reached from all eight. Decoding 6 passed.
    Layer 2, 10,000 programs at a new seed 1300, every one on the CPU: 0 failing (3,468 faulted, 740 left to
    an address at full). Csmith, layer 3's cases 0 to 1,199 of seed 1: 1,023 compared, 177 left out at 2^28
    events, 0 failing (1,427 s).
  - Layer 1 on the Mac (seed 1300, 1,000 cases a form, 10 a form in Chrome, 4 jobs, 3 test threads, load 3
    to 9, 828 s, `c1/mac-layer1-2.log`): 1,215 forms, 0 failing; 12,150 cases in Chrome, 0 failing; 1,000
    words in no form illegal as on the CPU; 7 passed.
  - The whole suite of the four crates (release, `--no-fail-fast`, rented ARM64 machine, 12 jobs, 12 test
    threads, 20:56 to 21:24 UTC, load 5 to 21, `c1/suite-2.log`, whole output in `c1/suite-2`): every test
    binary passed but layer 1's, whose two random tests fail on `MSR_SR_systemmove` alone (64 cases a form
    and 1 a form in Chrome, seed 1), the Graviton's FPSR difference; layer 1's verdict is the Mac's, above.
    progress_count 5, programs 22 (3 ignored), layer 3 3, layer 2 1, changes 9, checks 12, browser 1,
    binaries 2, smoke 6, test_guest 16, valid 7, decoding 6, exact 4, guest_free 4, lookaheads 1, no_std 1,
    omit 1, seeded 1.
- C1's bar is met by these runs; what is left is its independent check and the owner's rulings.

Next (in order):
1. The owner's ruling on the emission and the names (see "For the orchestrator").
2. For the independent check: other programs and a new seed (`WC_PROGRESS_SEED`, `WC_PROGRESS_CSMITH_SEED`);
   hand-count `progress_events.S` or a program of its own; `measure_progress_count` on each host.
3. The literal fix list.

How to run C1's checks: `cargo test --release -p wcontainers-translator-test --test progress_count`
(knobs at the top of the file); the cost: `WC_BENCH_RUNS=5 cargo test --release -p
wcontainers-translator-test --test progress_count measure_progress_count -- --ignored --nocapture`.

## Done

- `wcontainers-arm64`: our own `Instruction` type; `decode` wraps yaxpeax-arm 0.5.0 and settles from the raw
  word what its operands leave open (shifted against extended add, register-offset index fields, system
  register identity) and the reserved encodings it accepts (32 bit shift amounts of 32 or more, extend
  amounts above 4, reserved logical bit masks, EXTR lsb of 32 or more in 32 bit). `emit` holds the meaning of
  every base integer instruction once, as WebAssembly, against the `Emitter` trait. `Instruction::flow`
  says where control can go. `forms` reads Arm's list of every A64 form (`arm/a64-forms.txt`) and evaluates
  each form's condition (features and field limits) against the CPU we present (`OFFERED_FEATURES`: FP,
  AdvSIMD, CRC32, LSE). `decode` asks the forms first: a word in no form of that CPU is `Undefined`
  before yaxpeax-arm sees it (yaxpeax-arm accepts some words that are in no form, such as MOVZ W with
  hw=2 and conditional selects with wrong fixed bits). A word of that CPU the decoder does not translate
  is `Unsupported`. The forms are indexed by bits 31 to 21, built once on first use. RPRFM is the one form
  Arm carved out of a hint space, so without its feature it is a PRFM hint and does nothing.
- Alignment faults: a load or store with SP as its base and SP not a multiple of 16, and a jump to an
  address that is not a multiple of 4, stop with `Exit::AlignmentFault` (Linux: SIGBUS, BUS_ADRALN, at SP or
  at the target). Prefetches are not checked, as on the CPU.
- `wcontainers-translator`: `translate(blob, code range, starts, LookAhead::None)` and `thread_module(slots)`.
  `contract` is the module contract: import names, hooks, trap kinds, warning reasons, permission table bits,
  description section. Look-ahead none follows direct control flow from the starts; every instruction reached
  is its own piece.
- Piece convention: params x0 and x1 (`code::ARGUMENTS`, four up to S1's sixth run), sp, expected return
  address; results x0, x1, sp. Everything else in the thread block (since S1). Returns check x30 against the expected address, else leave through "go to
  address" (an exception the bottom loop catches). Since S1 the expected address's low bit
  (`contract::FLAGS_WANTED`) says whether the caller reads the flags after the call.
- Lookup: one table slot per 4 bytes of code (design one of the two). The second design is not built.
- Permission check (one byte per page, with next-page bits) and stop check are already in look-ahead none.
- `wcontainers-translator-test`:
  - `runner`: wasmtime runner, one thread, the hooks, the stand-in for write, writev, exit, exit_group, brk,
    and translation at run time when control reaches an address in a loaded blob with no piece. Every slow
    path goes through one function: counted by reason, timed where the runner does the work (translation at
    run time), and warned once per address (or system call number) when its count there reaches
    `contract::WARNING_THRESHOLD` (1 for now). Warnings go to stderr and `Machine::warnings`; counters read
    as abi `CounterReading`s.
  - `reference/`: native reference runner (own small package, built with rust-lld for
    aarch64-unknown-linux-musl, run in Docker as `alpine:latest`). Enters a case through a rewritten signal
    frame, so every register is exact; catches system calls from the test page with seccomp.
  - `crates/arm64/arm/a64-forms.txt`: all 4429 A64 encodings from Arm's specification
    (2026-09 release), made by `cargo run -p wcontainers-translator-test --bin arm-forms -- <Instructions.json> <out>`.
  - Layer 1: `tests/layer1.rs`. 266 forms: every base integer form M0 covers, including the five whose
    condition only limits fields (LDRB/STRB/LDRSB register, PRFM register). SP is misaligned in a quarter of
    cases. A second test checks fixed alignment cases against the CPU and checks they reach the fault.
  - `tests/decoding.rs`: words of every form in Arm's list that the CPU we present does not have decode as
    undefined (pointer signing branches and loads, FlagM, SB, CSSC, MTE and the rest); the offered features
    match the abi's HWCAP; random words from the whole 32 bit space: every word we translate is of a form
    layer 1 covers (or the RPRFM hint carve-out); 12 words in no form that yaxpeax-arm accepts are undefined.
  - Layer 1 also runs random words in no form of Arm's list (for any feature) on the CPU and through ours:
    both raise illegal instruction.
  - Stand-in write and writev read the permission table: a buffer (or writev's vector array) with any byte
    on a page that does not allow reading gives -EFAULT and writes nothing, as Linux does for a pipe (Docker
    gives the reference a pipe). writev takes its vector count as 32 bits, as Linux does, and gives -EINVAL for
    more than 1024 vectors and for a length that is negative as a signed number. `programs/buffers.c` checks all of these against Linux, in wasmtime and
    in Chrome (`flat.ld` now defines `__blob_end`, so it can reach the page after the blob).
  - Stop hook stand-in: a flag raised from outside is cleared, counted ("stopped by the flag"), warned, and
    the thread carries on (hook returns 0), as the plan says. The runners raise the flag themselves to make
    the thread leave after exit or exit_group (and after every system call in layer 1); that is not counted.
    `tests/smoke.rs` raises it before a run and checks the result and the count.
  - `tests/programs.rs`: `programs/integers.c` and `programs/tables.c` (only zeroed data, a jump table)
    built at -O0, -O1, -O2, -O3, -Os as flat blobs; output and exit status identical to real ARM64 Linux.
    zig cc gives the same bytes at -O1, -O2 and -O3 for both, so that is 3 distinct blobs per program, 6 in
    all. The linker script follows zeroed data with one byte so a flat copy keeps it.
  - M1, `programs/hello.c`: printf through the C library (musl as zig builds it, its own crt1.o `_start`,
    init and fini arrays placed by `programs/flat.ld`), exit status 3. Built at all five levels (-O1, -O2,
    -O3 give the same blob, so 3 distinct). Stdout, stderr and exit status byte-identical to real ARM64
    Linux in wasmtime (`tests/programs.rs`) and in Chrome (`tests/browser.rs`, which also runs integers and
    tables at every distinct level).
  - Starting stack (`layout::initial_stack`): argc 1, argv "blob", empty environment, auxv with AT_PAGESZ,
    AT_HWCAP (= abi HWCAP, checked by a test), AT_HWCAP2 0, AT_RANDOM, AT_EXECFN. Shared by both runners and
    the native reference, so all three start the blob identically.
  - The C library reaches SIMD and floating point loads and stores and some vector data processing
    (54 forms: MOVI, FMOV, ORR, DUP, CMEQ, UMAXV, EXT, CNT and others; the list is `VECTOR_FORMS` in
    `src/layer1.rs`). These, and every SIMD and floating point load and store form in the load/store groups
    layer 1 covers, are translated now (`arm64/src/vector.rs`), and layer 1 carries v0 to v31
    both ways. Layer 1 covers 399 forms in all.
  - Hello uses exactly these system calls: set_tid_address and ioctl (answered "not implemented", counted
    and warned, as the plan says; musl carries on), writev, exit_group.
  - Chrome runner: `browser/` is the translator built for wasm32 (its own workspace), `browser/runner.js` a
    module worker doing what the wasmtime runner does (hooks, stand-in system calls, counters, warnings,
    translation at run time in the page), every name and number read from the translator's contract
    except where the thread block and permission table sit in the state memory, which the runner chooses.
    `browser/run.mjs` drives the installed Chrome headless through playwright-core from the Nix shell,
    serving the files from https://wcontainers.test with cross-origin isolation headers.
    `src/browser.rs` builds it and reads the result. Ran in Chrome 157.

Found and fixed in the second run: the misaligned SP and jump faults above; `translate` overflowed on a
branch whose target wraps below the blob (found by layer 1, regression test in `tests/valid.rs`); flat blobs
lost zeroed data when a program had no initialised data.

Found and fixed in M1: tests failed now and then with empty output from the native side. Cargo replaces
the built reference binary on every build, even with nothing to do, and every native run called cargo, so
a container started while another test's build was running could find no binary. The reference is now
built once per test process and copied to `target/reference/run/<pid>/`. Builds also work under names of
their own and each native run reads its own copy of the blob. After the fix: 15 runs of
`tests/programs.rs` in a row passed (it had failed three times in about 25 runs before).

Found and fixed in the third M1 run (from the independent check): yaxpeax-arm accepted about 0.45% of all
32 bit words that are in no Arm form as base integer instructions, and we ran them where the CPU raises
SIGILL (fixed by asking the forms first). The stand-in write and writev ignored the permission table and
wrote out unreadable buffers; a length of -1 crashed the wasmtime runner. The stop hook counted every
exit as a stop and always made the thread leave. The description section had a hand-written LEB128
encoder (now wasm-encoder's `Encode`). The permission byte was built with a fixed shift (now from the
contract's bits, in both runners).

- M2, full look-ahead (`translator/src/full.rs`, `LookAhead::Full`):
  - Finding functions: the starts, targets of `bl`, addresses built by `adr` or by `adrp` with the `add`
    right after it (except those a jump table of the same function goes to), and extra entries that keep
    every function's control flow reducible (a block entered by a jump back from outside its loop becomes
    an entry of its own, tried again until none is left). A function is everything reachable from its
    entry without passing another entry; reaching another entry is a tail call. The same instruction may be
    in more than one piece.
  - One piece per function. Blocks are structured after Ramsey's "Beyond Relooper" (dominators from
    `petgraph`): loops for loop heads, blocks for merge points, everything else placed inline.
  - Registers (x4 to x30, v0 to v31) are locals; x0 to x3 and sp stay parameters and results. A forward
    analysis says which locals may be newer than their globals ("dirty"); a backward one says which must
    hold their true value, counting every register that may be dirty at a block's start as needed there, so
    no write back ever stores a stale local. Locals are loaded at the entry when needed, written back
    before calls, system calls, slow paths and leaving the piece, and loaded again after a call when
    needed.
  - Flags are kept as the last comparison: a flag setting the meaning makes once (`Effects::flags`) is kept
    as its operands in locals shared by every setter of the same shape, and a condition on it is one
    WebAssembly comparison (`condition_direct` in the arm64 crate: a subtraction gives eq, ne, hs, lo, hi,
    ls, ge, lt, gt, le straight from its operands; a logical operation gives the sign conditions). Where
    the flags are is known per point (flag locals, the globals, or one shape); where paths with different
    shapes meet, a local says which, and readers choose by it. After a call the flags are in the globals,
    so nothing is loaded.
  - Direct calls are WebAssembly calls; calls through a register and calls to code with no piece go through
    the dispatcher (the lookup). Jumps that leave the piece are tail calls, direct or through the lookup.
    Returns keep the check of M0.
  - Jump tables (`arm64/src/habits.rs`): a `br` whose target is a base built by `adr` plus a
    shifted entry loaded from a table built by `adr`/`adrp`, with the bound from the nearest flag setting
    subtraction of an immediate and `b.hi`/`b.hs` before it (looking up to 64 instructions back). The
    targets are read from the blob and become blocks of the function; the jump is a `br_table` on the
    address actually computed at run time, so a wrong guess or a changed table only takes the lookup. An
    unrecognised `br` leaves through the lookup and is counted ("jump through a register not
    recognised"), as is a recognised one going somewhere not in its table.
  - Stop check at every jump back and at the entry of every piece that calls or leaves to another piece. It
    reads the flag with a plain load (see "Departures"); when the hook says carry on, registers are loaded
    back from the globals just written rather than kept across the call. The check after a system call
    stays (it is how the kernel makes a thread leave).
  - Permission check with the drops the plan allows. The fast test is one clamped byte load and an AND
    (this page and the next both allow it); only when that fails is the exact test made (does the access
    really reach the next page, and where it faults). An address the table allows has a clear top byte, so
    untagging (Top Byte Ignore) moved into the check's slow path. Drops: (1) the stack frame: when the
    stack pointer moves down by a known amount, the pages of the new part are checked once for reading and
    writing, and accesses through sp inside the known frame have no check, across calls too; a forward
    analysis carries the frame between blocks. (2) A group: accesses through one base register in one
    block, with the base unchanged or moved by a known amount by its own pre/post indexing, and touching at
    most 4096 bytes in all, are checked once at the first of them. If a frame or group check fails, the
    thread leaves to the instruction with the unwind payload `UNWIND_GO_TO_EXACT`: the bottom loop counts
    "left to an address" and asks the load code hook for code at that address even though the table may
    hold a piece there (the failing one, when the instruction is a function's entry). The runner translates
    at none from there, unless it already did (it keeps the set of addresses it translated from), so the
    instruction runs from code that checks each access exactly, and faults land at the same instruction
    with the same registers as at none. Nothing proved before a call or system call is relied on after it,
    except the frame.
  - The stack alignment check is left out where the stack pointer is known to be a multiple of 16 (checked
    by an earlier access and moved only by multiples of 16 since); a new `Emitter::stack_aligned` asks.
  - A call after which a frame or the alignment is known compares the stack pointer the callee gives back
    with the one it was given. If they differ, the thread leaves to the instruction after the call ("left
    to an address") and carries on at none, which knows nothing about sp. Calls with nothing known after
    them have no compare.
  - The immutable imported values a piece uses (load address, thread block, permission table place and
    length) are copied into locals at its entry, so the engine can keep them in registers instead of
    reloading them through the import on every check (wasmtime loads an imported global through a
    pointer).
  - `translate_omitting` with `Omit { stop, permissions, returns }` leaves checks out, only to measure them
    (since S1 public only with the crate's `measure` feature, which only the test tools turn on).
- `wcontainers-arm64` additions: `Effects` and `Registers` (what an instruction reads and writes, found by
  running its meaning against a recording emitter, so there is no second copy of instruction logic),
  `Instruction::memory_access`, `habits::jump_table`, `condition_direct`, `untag` (now public; the check
  calls it), `Flags::Add::subtrahend` (the value subtracted, for direct comparisons).
- Tests for M2:
  - `tests/programs.rs`: integers, tables, buffers, hello and `programs/bench.c` at every distinct
    optimisation level, at none and full: output and status equal to Linux, and at full the final
    registers and every reachable byte of memory equal to none.
  - `programs/bench.c`: the toy's four workloads (fib, sieve, hash, sort through a callback) and four of our
    own (a byte code interpreter on a switch, a matrix product, a linked list walk, a byte search).
    `WORKLOAD` picks one, `SCALE=1` gives the toy's sizes for timing.
  - `tests/browser.rs`: the same programs (and bench) in Chrome at none and full, output equal to Linux.
  - `tests/layer2.rs` with `src/layer2.rs`: random programs assembled with `dynasm` (dynasmrt 5.1.0): loops
    with counters, calls with bounded depth, bounded recursion, tail calls, calls and jumps through
    registers, compiler-style jump tables, flags carried across instructions (cmp, ccmp, csel, mrs/msr
    nzcv), loads and stores in a 512 byte area. Each ends by writing its area, x0 to x15, x19 to x28 and
    NZCV. Full against none: registers, output and all reachable memory; the first few also against the
    real CPU. Breaking one direct comparison on purpose was caught within 218 cases and shrunk. Since the
    third M2 run: every function call (recursive ones too) makes a frame with sp, from 528 bytes to 1.5 MiB
    (some run off the 1 MiB stack, some reach below it but are used only at the top), at the instruction a
    recursive `bl` enters, so frame checks fail on entries; accesses through sp in the frame; groups of up
    to four accesses (every size, pairs, offset, pre- and post-indexing) through any register, from the
    area or from just before the end of the blob, which is followed by a page that allows nothing (the blob
    ends with a page and more of zeros so these reach no code). About 12% of programs fault (SIGSEGV),
    about 11% leave to an address at full. With the old leave-to (before this run's fix) a 500 case run
    hung (killed after 240 s); with the fix it passes in 14 s.
  - `tests/smoke.rs`: a flag raised by another host thread while a translated loop runs is seen inside the
    loop, at none and full.
  - `tests/programs.rs` also runs hand-written assembly programs (`programs/*.S`; a program may be C or
    assembly): callees that give sp back moved down the stack, moved onto a page that allows nothing, and
    no longer a multiple of 16. Each matches Linux (exit status 77, SIGSEGV, SIGBUS) at none and full, and
    full matches none in registers and memory. With the compare after the call taken out, the second gives
    77 at full where Linux faults, so the test catches it. A program killed by a fault now reports 128 plus
    the signal's number, as Docker does for the native run, instead of panicking.
  - `tests/checks.rs`: frame and group checks failing, on a function's first instruction too: recursion
    with 4 KiB frames made by the entry running off the stack (SIGSEGV), a function whose first two loads
    are a group reaching a forbidden page, a frame reaching below the stack used only in its mapped part
    (exit 0) and then once below it, groups crossing onto a forbidden page, an alloca loop running off the
    stack. Linux, none and full agree, and full matches none in registers and memory; the entry cases also
    run at full in Chrome. Before the fix the three entry cases hung at full.
  - `tests/benchmarks.rs` (ignored, one command, see "How to run"): native and wasmtime and Chrome timings
    with and without each check, and the translation's size. Native runs are timed by the reference itself
    (`wcontainers-reference time`), from making a child process to its end, so the reference's start-up is
    not counted (it was before, by busybox `time` in 10 ms steps). Matrix, the shortest: 0.093 s native
    either way, so the earlier ratios stand.

## How to run

- The long runs to the plan's pass bars, resumable, one command (from the repository root, detached; it runs
  for days): `WC_LONG_DIR=<scratch directory> nohup nix develop -c crates/translator-test/long-runs.sh > <log> 2>&1 &`.
  Knobs (`WC_LONG_PERCENT`, `WC_LONG_LAYERS`, `WC_LONG_SEED`, `WC_LONG_ACCEPT`, `WC_LONG_JOBS`,
  `WC_LONG_LIMIT`) and the ledger's fields are described at the top of the script and under "P1, the long-run
  driver" and "P2". To carry on after a stop, run the same command again; to keep chunks that passed on an
  older commit, set `WC_LONG_ACCEPT=<that commit>`. It refuses uncommitted changes and runs its own copy of
  the commit. The run under way and its exact command are under "P2".
- The benchmark set against elfconv: `nix develop -c crates/translator-test/elfconv.sh` (needs
  Docker and the image `ghcr.io/yomaytk/elfconv:arm64`; converted programs kept in `target/elfconv`;
  `WC_BENCH_RUNS` best of, default 3).
- Everything: `nix develop -c cargo test -p wcontainers-arm64 -p wcontainers-translator -p wcontainers-translator-test`
  (includes the Chrome test; it needs Google Chrome installed and Docker running).
- Lint the two side packages too: from `crates/translator-test/browser`,
  `nix develop ../../.. -c cargo clippy --target wasm32-unknown-unknown --target-dir ../../../target/browser -- -D warnings`;
  from `.../reference`, `nix develop ../../.. -c cargo clippy --release --target-dir ../../../target/reference -- -D warnings`.
- Layer 3: `WC_LAYER3_CASES=100000 WC_LAYER3_SEED=<n> WC_LAYER3_CHROME=100 WC_LAYER3_JOBS=6 nix develop -c cargo test --release -p wcontainers-translator-test --test layer3 -- --nocapture 2>&1 | grep -v "wcontainers warning"`
  (defaults 4 Csmith programs, seed 1, the first 1 that ends in time also in Chrome at none and at full in
  both settings, one at a time; the hand-picked programs are 48 builds each run). Programs that do not end on
  Linux in time (11 to 13 percent) are made but not compared. Needs Docker, Chrome, and network once to make
  the gcc image. Keep `WC_LAYER3_JOBS` at 6 or below: wider runs beside the other half's tests pushed the
  load past 300. For the bar use the driver; P1's 1 percent run took 1,437 s for 1,000 programs at 6 at once
  (load 24 to 31), the P1 check 762 s for 150 (load 55 to 113). Replay one program: the same seed with
  `WC_LAYER3_FROM=<case>` and `WC_LAYER3_CASES=1` (a failure prints its case and its Csmith number,
  `seed * 1000003 + case`).
- Floating point and vector timing: `nix develop -c cargo test --release -p wcontainers-translator-test --test programs floats_speed -- --ignored --nocapture`
  (`WC_BENCH_RUNS`, `WC_BENCH_WORKLOADS` 1 to 4, `WC_BENCH_CHROME=0` as for the benchmarks).
- Every form valid WebAssembly: `WC_VALID_WORDS=<n> nix develop -c cargo test -p wcontainers-translator --test valid`.
- Layer 1, long: `WC_LAYER1_CASES=10000 WC_LAYER1_CHROME=100 WC_LAYER1_SEED=<n> nix develop -c cargo test --release -p wcontainers-translator-test --test layer1 -- --nocapture 2>&1 | grep -v "wcontainers warning"`
  (defaults: 64 cases, seed 1, 1 case a form in Chrome, 1000 cases to a Chrome page; 6075 took 25.5 s).
  `WC_LAYER1_JOBS` forms at once (default 1), each job with its own reference container.
  Replay a failure: same seed and cases, plus `WC_LAYER1_FORMS=<form name>` (the wasmtime run only). The
  filter drops the runners' warnings (memory faults and stops), one per case.
- Decoding against Arm's forms: `WC_DECODE_CASES` (default 64 per form), `WC_DECODE_WORDS` (random words
  from the whole space, default 200,000), `WC_DECODE_SEED` (default 1).
- Words in no form against the CPU: part of the layer 1 command; `WC_LAYER1_CASES` words.
- Translator proptest: `PROPTEST_CASES`, `PROPTEST_RNG_SEED` (default seed 1).
- Layer 2: `WC_LAYER2_CASES=<n> WC_LAYER2_SEED=<s> WC_LAYER2_FROM=<first case> WC_LAYER2_JOBS=6 WC_LAYER2_NATIVE=<k> WC_LAYER2_CHROME=<c> nix develop -c cargo test --release -p wcontainers-translator-test --test layer2 -- --nocapture`
  (defaults 64 cases, seed 1, from case 0, one at a time, the first 8 on the CPU, the first 1 in Chrome; each
  CPU case starts a container, about 0.3 s, and each Chrome case Chrome three times). Since P1 case `i` is made
  from the seed and `i` alone, so a failure replays with `WC_LAYER2_FROM=i WC_LAYER2_CASES=1`; seeds of earlier
  runs give other programs than they did then.
- Bounded look-ahead of random N against none (C programs, every piece but the first made at run time):
  `WC_LOOKAHEAD_CASES=<n> WC_LOOKAHEAD_SEED=<s> nix develop -c cargo test --release -p wcontainers-translator-test --test lookaheads -- --nocapture`
  (defaults 4 a program, seed 1). Layers 2 and 3 also run every case at bounded look-ahead of a random N and
  mixed with events injected (`src/mixed.rs`): layer 2's N and mixing seed come from its proptest case, layer
  3's from a hash of the blob, so the same knobs replay them.
- Time to translate and load one piece at run time, wasmtime and Chrome:
  `nix develop -c cargo test --release -p wcontainers-translator-test --test lookaheads measure_translating -- --ignored --nocapture`.
- The cost of the stack budget check: the benchmarks with `WC_BENCH_VARIANTS=1,5` (every check, no depth check).
- What a switch of stacks costs (fibers and coroutines against native, wasmtime and Chrome):
  `nix develop -c cargo test --release -p wcontainers-translator-test --test programs switching_speed -- --ignored --nocapture`.
- The default N against others (the benchmark set made at run time at N 16, 64, 256, 1024, and full):
  `nix develop -c cargo test --release -p wcontainers-translator-test --test lookaheads measure_bound -- --ignored --nocapture`.
- Permissions changed at random stops and system calls: `WC_CHANGES_CASES=100000 WC_CHANGES_SEED=<n> WC_CHANGES_PROGRAM_SEEDS=<k> nix develop -c cargo test --release -p wcontainers-translator-test --test changes -- --nocapture 2>&1 | grep -v "^wcontainers warning"`
  (defaults 64 random programs, seed 1, 3 seeds for each C program; since the eleventh run about 33 ms
  a random program at load 16 to 21, as every program now calls a function that makes a system call).
- Real binaries, as a check (busybox and musl, both settings, in the normal suite):
  `nix develop -c cargo test --release -p wcontainers-translator-test --test binaries`.
- Real binaries (M5): `WC_BINARIES=busybox,musl nix develop -c cargo test --release -p wcontainers-translator-test --test binaries measure_binaries -- --ignored --nocapture 2>&1 | grep -v "^chrome:"`
  (names: busybox, musl, rg, git, node, python, bun, java, go; all when unset; each is taken from its Docker
  image into `target/binaries` once, which needs network). Node and Bun take minutes each and 15 to 20 GB of
  memory (wasmtime compiling several GB of WebAssembly); run them alone.
- What a call between code modules costs (wasmtime and Chrome): `nix develop -c cargo test --release -p wcontainers-translator-test --test benchmarks measure_calls -- --ignored --nocapture`
  (`WC_CALLS`, default 100 million; `WC_BENCH_RUNS`; `WC_BENCH_CHROME=0` leaves Chrome out).
- The translator uses every core; on this machine set `RAYON_NUM_THREADS=6` for measurements and long runs.
- Size only, no engines: `WC_BINARIES_ENGINES=0` on the real binaries command; `WC_OPTIMISE=speed` or `size`
  for one setting (both by default); `WC_BINARIES_DUMP=<dir>` writes each module.
- Benchmarks: `WC_BENCH_RUNS=3 nix develop -c cargo test --release -p wcontainers-translator-test --test benchmarks -- --ignored --nocapture 2>&1 | grep -v "^wcontainers warning"`
  (`WC_BENCH_WORKLOADS=1,2` picks workloads, `WC_BENCH_CHROME=0` leaves Chrome out, `WC_OPTIMISE=size`
  translates for size). About 15 minutes.

## Cases actually run

- T3, fourth run (final code, 4117dcd3, seed 400): layer 1 on the Mac, 1,215 forms x 1,000 cases, 10 a form
  in Chrome, 1,000 words in no form; layer 2, 25,000 programs, all on the CPU, 5 in Chrome; layer 3, 1,000
  programs and then 4,500 more from case 1,000, 4,803 compared (see "T3", fourth run); the whole suite of the
  four crates (debug, defaults); the changed program tests; busybox awk's three commands; 4 rounds of speed
  pairs. Earlier T3 runs' cases are in the "T3" section.
- P1, release unless said: after the merge, every test of the three crates (debug, 69 passed, 7 ignored).
  After the changes: layer 2, 64 cases seed 1 (8 on the CPU); 64 cases seed 1 with 4 jobs and 1 in Chrome;
  600 cases seed 1 with 6 jobs, 2 on the CPU, 1 in Chrome (19 s, load 20 to 22); layer 1, 1215 forms x 16
  cases, seed 1, 6 jobs (12 s); layer 3, 4 Csmith programs seed 1 and the hand-picked builds, then 24 seed 5
  with 6 jobs, 2 in Chrome (46 s, load 27); checks, Chrome and programs tests; the new binaries test: all
  passing. The 1 percent run of the long-run driver (seed 7) above: 121,500 layer 1 cases, 10,000 layer 2
  programs, 1,000 Csmith programs, 0 mismatches. Last, every test of the three crates on the final code
  (debug, `RAYON_NUM_THREADS=4`, `--test-threads=4`, Chrome and the defaults of every layer included): 70
  passed, 7 ignored (the measurements), 20.5 minutes, load 20 to 43.

- M3 close-out, second run (fifth run), release: layer 2 with the area check, the generator fixed and the
  translator before the structure proof change: seed 4, 1000 cases (none on the CPU; with the generator fix
  undone it failed, "a store below the area"); seed 4243, 6000 cases, 60 on the CPU (1993 faulted, 392
  left to an address at full, 129 s); seeds 1, 2, 3, 5, 2000 cases each, 60 on the CPU: 0 mismatches (about
  48 s each; load 43 to 261 over these runs). With the current translator: seeds 6 and 7, 2000 cases each,
  40 on the CPU: 0 mismatches (46 s and 41 s, load 95 to 111); the changes test, 1000 random programs seed
  21 and the C programs (3 seeds): 0 mismatches (45,316 changes, 4921 faults handled; 36 s, load 109 to
  115). `tests/checks.rs` in full: pass. Layer 1 on the current code, 1215 forms x 300 cases, seed 13, and 1 a form in Chrome: 0
  failing (548 s, load 64 to 115). Last, every test of the three crates passes (debug build, Chrome tests
  and layer 3's defaults included; 59 passed, 2 ignored; 6.9 minutes, load 58 to 64).

- M3 close-out, release unless said: layer 2, 500 cases seed 121 (40 on the CPU), 2000 seed 122 (20 on the
  CPU), 1000 seed 123 (30 on the CPU), 1000 seed 124 (40 on the CPU), all with vector accesses: 0
  mismatches (seed 124: 341 faulted, 61 left to an address at full; 22 s, load 83 to 90). The changes test,
  300 random programs seed 1 and the C programs (3 seeds), then 1000 random programs seed 21: 0 mismatches.
  `tests/checks.rs` and `tests/changes.rs` in full: pass. Every test of the three crates passes (debug build, Chrome tests
  and layer 3's defaults included; 57 passed, 2 ignored; 8.7 minutes, load 49 to 93). Layer 3 after the
  reference fixes, 200 Csmith programs seed 13, 3 at once, 2 also in Chrome: 174 compared and matching, 26
  did not end on Linux in time, 0 failing (290 s, load 50 to 113). `floats_speed` (all four workloads, both
  engines): output equal to Linux.

- M3, third run, release: layer 3, 2000 Csmith programs, seed 11, 12 at once, 10 also in Chrome: 1751 compared
  and matching, 249 did not end on Linux in 20 seconds, 0 failing (852 s, load 15 to 70). Layer 1, 1215 forms
  x 64 cases, seed 9, with 100 cases a form in Chrome (121,500 Chrome cases): 0 mismatches (829 s, load 19 to
  74). Layer 1, the 13 forms whose decoding changed, 1000 cases each, seed 8: 0 mismatches. Decoding,
  `WC_DECODE_CASES=2000`: all pass. Layer 3, 10,000 Csmith programs, seed 12, 12 at once, 20 also in Chrome,
  and the 48 hand-picked builds: 8780 compared and matching, 1216 did not end on Linux in time, 4 failing,
  hand-picked all matching (4423 s, load 26 to 308). Of the 4, three were programs that fault as ordinary
  executables too (compared and matching after the rule above, replayed one by one) and one the reference
  flake (12008340). Every test of the three crates passes (debug build, Chrome tests included; 8.6 minutes,
  load 17 to 67), and layer 3 at its defaults after the last change (69 s, load 14 to 17).

- M3, second run, release, after its changes: layer 1, 1215 forms x 200 cases, seed 6: 0 mismatches
  (243,000 cases, 292 s, load 15 to 21). Layer 3, 100 Csmith programs, seed 3 (moved position independent
  blobs among them; 1 also in Chrome), and the 48 hand-picked builds: 86 compared and matching, 14 did not
  end on Linux in 20 seconds, 0 failing (382 s, load 11 to 21). Every test of the three crates passes (debug
  build, Chrome test and layer 3's defaults included; load 8 to 28). Layer 3, 8 Csmith programs, seed 4,
  all 8 also in Chrome at none and full: 0 failing (38 s in all, load 10 to 18). Layer 1 in Chrome: 1215
  forms x 1, seed 1, and 1215 x 5, seed 7 (6075 cases, 25.5 s, load 13 to 14): 0 mismatches against the CPU.
  Last, every test of the three crates passes (debug build, Chrome tests included; 6.2 minutes, load 15 to 21).

- M3, layer 1, release: 1215 forms x 100 cases, seed 2: 0 mismatches (121,500 cases, 177 s, load 18 to 21).
  1215 x 1000, seed 3 (1,215,000 cases, 27 minutes, load 12 to 23): 4 forms failing, all in the software
  path or its rules: underflow is raised when the exact result is tiny (before rounding, as Arm's
  `FPRoundBase` does), not after; `ieee-apsqrt` rounds a denormal input wrongly under a directed rounding
  (now the root of x * 2^64, scaled back); the software FRSQRTS halved an operand whose half is denormal.
  Fixed; those 12 forms at 1000 seed 3 and the whole layer at 100, seed 4: 0 mismatches (153 s, load 19 to 23).
  Then 1215 x 1000, seed 5: 0 mismatches (1,215,000 cases, 26 minutes, load 17 to 24). Every test of the
  three crates passes after it (debug, Chrome and layer 3's defaults included).
- M3, layer 3, release: 4 Csmith programs seed 1 and the 48 hand-picked builds (hello and differences x
  clang, gcc x six levels x two positions): all match (59 s, load 5 to 6). 300 Csmith programs, seed 2
  (18.8 minutes, load 5 to 30): 256 compared and matching, 43 did not end on Linux in 20 seconds, 1
  failing: Csmith 2000136 (clang -O0), where Linux gave SIGSEGV and no output and ours printed the
  checksum. The same blob on the reference afterwards: 15 runs of 15 print that same checksum, as does the
  program built as an ordinary Linux executable, with a 1 MiB stack too. So the failure looks like a flake
  of the reference's blob mode under load, not understood; the test is left as it is (it fails when that
  happens) and this is recorded. A first 300 run hung: the reference is the first process in its container,
  which SIGALRM's default action never kills; the blob mode now ends from a handler.

- M3, release: layer 1, 1215 forms x 16 cases, seed 1: 0 mismatches (after the fixes above; 8 cases a form
  first gave 104 failing forms, then 17). Debug run of every test: 1215 forms x 64, seed 1, 0 mismatches
  after the last three fixes; those forms then 200 cases each, 0 mismatches. Load 4 to 6.

- Layer 1: 261 forms x 64 cases, seed 1: 0 mismatches.
- Layer 1: 261 forms x 1000 cases, seed 2: 0 mismatches (261,000 cases). This was before SP could be
  misaligned in cases; the misaligned SP bug was there and not found.
- After the fixes: 266 forms x 64 cases, seed 1: 0 mismatches. 5 newly covered forms x 500 cases, seed 1:
  0 mismatches.
- Decoding: 4429 forms x 64 words, seed 1, and 4429 x 2000 words, seed 2: 0 failures.
- M1, with the vector forms and the UNKNOWN-after-fault rule: 399 forms x 1000 cases, seed 2, release
  build: 0 mismatches (399,000 cases).
- Third M1 run, after the decoder asks the forms first: random words, 40,000,000, seed 2026, release:
  every translated word is of a form layer 1 covers. Words in no form on the CPU: 2000, seed 1, and 1000,
  seed 3: all illegal on both sides. The same test with the old decoder failed within 2000 words.
- Third M1 run, after the decoder change: layer 1, 399 forms x 1000 cases, seed 3, release build: 0
  mismatches (399,000 cases). This confirms the second run's seed 2 result with a new seed.
- Pass bar of the plan (10,000 per form) not yet run. One command for it is under "How to run".
- Layer 2 (M2): 64 cases, seed 1, 8 on the CPU: 0 mismatches. 2000 cases, seed 7, 40 on the CPU, release:
  0 mismatches (22 s at load 3 to 4). Second M2 run, after this run's changes: 3000 cases, seed 11, 20 on
  the CPU, release: 0 mismatches (35 s at load 10 to 16). Pass bar (one million) not run.
- Layer 2, S1 (every case also run at full stopping at every check), release: 500 cases seed 3, 2000 seed
  5 (after kept pages), 2000 seed 9 (after the flags bit): 0 mismatches. Not yet the pass bar.
- Layer 2, second S1 run, after loop pieces: 2000 cases, seed 13, 10 on the CPU, release: 0 mismatches,
  308 faulted, 220 left to an address at full (37 s at load 22 to 57).
- Layer 2, third S1 run, after kept runs and the write back before loops: 2000 cases, seed 17, 10 on the
  CPU, release: 0 mismatches, 315 faulted, 211 left to an address at full (32 s at load 7 to 8).
- Layer 2, third M2 run, with frames, sp accesses, groups and faults: 500 cases, seed 1, 60 on the CPU,
  release: 0 mismatches, 61 faulted, 40 left to an address. 20,000 cases, seed 21, 40 on the CPU,
  release: 0 mismatches, 2949 faulted (SIGSEGV, same at none, full and on the CPU where run), 2131 left
  to an address at full; 265 s at load 17 to 19. Found on the way (a generator bug, not ours): a store
  with write back whose transfer register is its base is CONSTRAINED UNPREDICTABLE, and the reference
  CPU raises SIGILL for it; the generator now keeps such registers apart, as layer 1 does.

- Layer 2, fifth S1 run (with walks), release: 1000 cases seed 41, 40 on the CPU: 0 mismatches, 325
  faulted; 1000 seed 43 (after outer loops): 0 mismatches, 379 faulted; 500 seed 31 (before walks): 0
  mismatches; 5000 seed 47, 20 on the CPU, after outer loops: 0 mismatches, 1774 faulted, 380 left to an
  address at full (77 s, load 8 to 10). Not yet the pass bar.
- Layer 2, seventh S1 run, release: 2000 cases seed 71, 10 on the CPU: 0 mismatches, 666 faulted, 152 left
  to an address at full (27 s, load 8 to 10). Not yet the pass bar.
- Layer 2, eighth S1 run (with lookups), release: 300 cases seed 81, 12 on the CPU; 600 seed 84, 40 on the
  CPU; 2000 seed 85, 10 on the CPU: 0 mismatches (seed 85: 658 faulted, 111 left to an address at full, 36
  s at load 7 to 15). Not yet the pass bar.
- Layer 2, sixth S1 run, release: 300 cases seed 61 and 2000 seed 63, 10 on the CPU (two argument
  registers): 0 mismatches, 674 faulted; 2000 seed 65, 10 on the CPU (lookup in place): 0 mismatches, 668
  faulted, 150 left to an address at full (30 s, load 13). Not yet the pass bar.

- S1 close-out, release: layer 2, 2000 cases seed 91, 40 on the CPU (with the new system call and
  backward switch shapes): 0 mismatches, 704 faulted, 132 left to an address at full (35 s, load 4 to 8).
  `tests/changes.rs`, 5000 random programs seed 11 and the C programs with 6 seeds each (72 runs): 0
  mismatches; 205,661 changes made, 11,178 faults handled and compared (108 s, load 7.5 to 9). Not yet a
  pass bar (the plan's one million random programs applies across layers 2 and 3).
- S1 close-out, second run, release, after the fix and the new shapes: layer 2, 2000 cases seed 101, 10 on
  the CPU: 0 mismatches, 715 faulted, 127 left to an address at full (37 s, load 17 to 20).
  `tests/changes.rs`, 3000 random programs seed 12 and the C programs with 4 seeds each (48 runs): 0
  mismatches; 142,304 changes made, 10,437 faults handled and compared (78 s, load 14 to 18). Every test
  of the three crates passes (debug build, Chrome test included).
- S1 close-out, third run, release, with the new shapes: layer 2, 1000 cases seed 111, 40 on the CPU:
  0 mismatches, 352 faulted, 97 left to an address at full (25 s, load 33 to 37). `tests/changes.rs`,
  2000 random programs seed 13 and the C programs with 3 seeds each (36 runs): 0 mismatches; 88,517
  changes made, 5418 faults handled and compared in the random programs (66 s, load 16 to 21).
  Every test of the three crates passes (debug build, Chrome test included; load 3 to 10).
- T3, third run: layer 1, the 12 estimate forms at 20,000 cases each (240,000), 20 each in Chrome, seed 301,
  the Mac; the scalar estimate functions new against old on 50,000,000 random inputs (rented x86 machine);
  the speed pairs 4 rounds of all eight workloads and 6 of fib.
- T3, eighth run, rented ARM64 machine: the three register jump tests (default sizes); layer 2 seed 600, 25,000
  programs, all on the CPU, 5 in Chrome; layer 3 seed 600, 1,000 programs (884 compared, 90 in Chrome); speed
  pairs, 4 rounds of all eight workloads; layer 1 seed 600 on the Graviton, 1,215 forms at 1,000 cases each
  (1,215,000), 10 each in Chrome (comparison only); the whole debug suite at default sizes, twice.

## Measurements

- Layer 1, 261 forms x 64 cases (debug build, opt-level 1): 93 s at load 26 to 34, about 5.6 ms per case.
- Layer 1, 261 forms x 1000 cases: 1334 s at load 39 to 73, about 5.1 ms per case.
- Layer 1, 399 forms x 1000 cases, release build: 780 s at load 22 to 29, about 2.0 ms per case. At that
  rate the 10,000 per form pass bar is about 2.2 hours.
- Layer 1, 399 forms x 1000 cases plus 1000 words in no form, release build: 480 s at load 9 to 18 (other
  tests ran beside it), about 1.2 ms per case.
- Look-ahead none, every instruction a start, release build, best of 5, load 53 to 88 (gzip at its default
  level; the ratio is gzipped WebAssembly over gzipped code):
  remeasured in the third M1 run, load 7.4 to 8.6:
  - integers -O0: 2896 bytes of code (1674 gzipped), translated in 0.50 ms (5.8 MB/s), 122,224 bytes of
    WebAssembly (42.2 per byte), 14,277 gzipped (8.5), wasmtime compile 20 ms.
  - integers -O2: 3428 bytes (1953), 0.52 ms (6.6 MB/s), 106,915 bytes (31.2), 15,495 gzipped (7.9), 25 ms.
  - tables -O0: 2368 bytes (1289), 0.43 ms (5.5 MB/s), 97,480 bytes (41.2), 11,619 gzipped (9.0), 15 ms.
  - tables -O2: 1552 bytes (1204), 0.25 ms (6.2 MB/s), 46,621 bytes (30.0), 8,036 gzipped (6.7), 10 ms.
  - hello -O2 (with the C library): 19,752 bytes (12,748), 3.0 ms (6.6 MB/s), 683,475 bytes (34.6),
    89,761 gzipped (7.0), 132 ms.
  - Sizes are unchanged by the third run's fixes. Code modules have imported every vector global since M1
    (468 bytes each). Before M1 integers and tables were 468 bytes smaller (121,756 / 106,447 / 97,012 /
    46,153); integers grew from 113,270 / 105,767 (first run) mostly by the SP alignment checks.
- The forms index (built on the first decode): about 10 ms once, then about 22 ns per word for "is this
  word an instruction of the CPU we present" (1,000,000 random words in 22 ms), release, load 7.5.
- hello in wasmtime translates at run time 10 times per run (pieces reached through registers); those 10
  take 0.58 to 0.79 s in all in a release build, load 21 to 31. That is wasmtime compiling one small
  module per piece; look-ahead (M2, M4) is what removes it.
- Chrome test (hello, integers, tables, 9 distinct blobs, a fresh headless Chrome for each): 23 to 29 s in
  all over 6 runs, load 22 to 33. With buffers too (12 blobs): 28 s, one run, load 4 to 8.
- Run time against elfconv: not measured; elfconv is not set up.
- M2, full look-ahead, `tests/benchmarks.rs`, release build, best of 2, Apple silicon Mac, wasmtime 50
  (crate) and Chrome 157 headless; native is the reference runner in Docker timed by busybox `time`
  (10 ms steps). Load 3.4 to 8 during the run. Times are seconds, then times native, in the order: every
  check / no stop check / no permission check / no return check / no check.
  - fib(42), native 0.38: wasmtime 4.38 (11.5) / 3.10 (8.2) / 4.31 (11.3) / 4.15 (10.9) / 2.76 (7.3);
    Chrome 3.39 (8.9) / 2.85 (7.5) / 3.29 (8.7) / 3.28 (8.6) / 2.59 (6.8).
  - sieve, native 0.43: wasmtime 1.54 (3.6) / 0.73 (1.7) / 0.76 (1.8) / 1.59 (3.7) / 0.53 (1.2);
    Chrome 1.43 (3.3) / 0.73 (1.7) / 0.85 (2.0) / 1.26 (2.9) / 0.43 (1.0).
  - hash, native 0.55: wasmtime 1.04 (1.9) / 0.57 (1.0) / 0.58 (1.05) / 1.03 (1.9) / 0.55 (1.0);
    Chrome 1.79 (3.3) / 1.08 (2.0) / 1.67 (3.0) / 1.78 (3.2) / 0.98 (1.8).
  - sort, native 0.34: wasmtime 2.18 (6.4) / 2.20 (6.5) / 2.11 (6.2) / 2.22 (6.5) / 1.62 (4.8);
    Chrome 1.76 (5.2) / 1.66 (4.9) / 1.63 (4.8) / 1.73 (5.1) / 1.57 (4.6).
  - interpret, native 0.14: wasmtime 1.43 (10.2) / 2.06 (14.7, an outlier against the others) / 0.65 (4.6)
    / 1.43 (10.2) / 0.41 (2.9); Chrome 1.31 (9.3) / 1.17 (8.3) / 0.42 (3.0) / 1.30 (9.3) / 0.34 (2.4).
  - matrix, native 0.07: wasmtime 0.66 (9.4) / 0.38 (5.5) / 0.35 (5.0) / 0.68 (9.7) / 0.20 (2.9);
    Chrome 1.59 (22.7) / 0.99 (14.1) / 1.39 (19.8) / 1.58 (22.5) / 0.95 (13.5).
  - walk (memory latency), native 2.19: about 0.9 to 1.1 everywhere.
  - search, native 0.11: wasmtime 1.42 (12.9) / 0.48 (4.4) / 0.62 (5.6) / 1.35 (12.3) / 0.24 (2.2);
    Chrome 2.53 (23.0) / 1.65 (15.0) / 2.27 (20.6) / 2.52 (22.9) / 1.48 (13.5).
  - After keeping fewer flag operands (last commit of this run), wasmtime, load 5.7 to 7.8: sieve 3.8 /
    1.6 / 1.6 / 3.6 / 1.3, hash 2.1 / 1.1 / 1.1 / 1.9 / 1.0, search 9.3 / 3.8 / 5.2 / 9.3 / 2.0 times
    native. Within noise except search.
  - Against the toy's figures in the plan: the toy had loops at about 1.3 times native with registers in
    locals and no checks; we have 1.0 to 1.3 on sieve and hash with no check, so the shape holds. The toy's
    checks cost 5 to 15% (stop, tightest loop) and 0 to 24% (permissions, wasmtime) each; ours each alone
    cost 0 to 40% on sieve and hash, but together they double the loops in both engines. Call heavy code
    (fib, sort) is 5 to 7 times native even with no check; the toy's call figures were relative (about 20%
    better with 4 passed registers), not against native. Matrix and search in Chrome are 13 times native
    with no check, which wasmtime is not (2 to 3): unexplained.
  - Size, bench at -O2 SCALE=1 (all workloads), measured at commit 1852235 (before the last commit of the
    first M2 run): 4836 bytes of code (2867 gzipped), translated at full in 2.4 ms (2.0 MB/s), 88,244 bytes
    of WebAssembly (18.2 per byte of code), 16,400 gzipped (5.7 per gzipped byte), wasmtime compile 30 ms,
    load 7. The toy made about 5 bytes per byte of ARM64. Keeping fewer flag operands (b58212a) grew it to
    110,768 bytes (22.9 per byte), 26,896 gzipped (9.4); found by the independent check, not explained yet.
- Second M2 run, after the compare of sp after calls and the fixed values in locals (commit 9f4124e),
  `tests/benchmarks.rs`, best of 2, same order as above (every check / no stop / no permission / no
  return / no check), times native. Load 6.3 to 8.6 up to sort, then 10 to 25 (other work on the Mac), so
  the later workloads are noisier.
  - Size: 111,975 bytes of WebAssembly (23.2 per byte of code), 27,571 gzipped (9.6), translated in 2.6 ms,
    wasmtime compile 31 ms, load 6.6.
  - fib: wasmtime 12.2 / 8.4 / 10.1 / 10.2 / 8.2; Chrome 10.1 / 8.8 / 9.6 / 9.6 / 7.7. A run of the same
    code at load 4 to 7 gave 10.2 with every check in wasmtime, so fib did not change beyond noise.
  - sieve: wasmtime 3.5 / 1.6 / 1.4 / 3.4 / 1.3; Chrome 2.9 / 1.5 / 1.9 / 2.7 / 1.1. (3.2 with every
    check in a wasmtime-only run at load 7.)
  - hash: wasmtime 1.33 / 1.00 / 0.99 / 1.48 / 0.95; Chrome 2.9 / 1.9 / 2.7 / 2.9 / 1.8. Every check in
    wasmtime went from 1.9 to 2.0 times native down to 1.33 to 1.35 (three runs) with the fixed values in
    locals.
  - sort: wasmtime 7.0 / 7.4 / 6.5 / 8.2 / 5.1; Chrome 5.5 / 5.3 / 5.1 / 5.3 / 4.7.
  - interpret (load 16 to 25): wasmtime 9.5 / 9.9 / 4.6 / 10.8 / 3.3; Chrome 10.1 / 9.4 / 2.9 / 10.1 / 2.5.
  - matrix (load 13 to 21): wasmtime 9.8 / 6.5 / 5.5 / 8.6 / 2.9; Chrome 21.5 / 13.3 / 18.9 / 21.2 / 11.9.
    A wasmtime-only run at load 7 gave 7.7 with every check (10.2 before the fixed values in locals).
  - walk: 0.8 to 0.95 everywhere (memory latency; the native run is slower here, unexplained).
  - search (load 8 to 11): wasmtime 9.1 / 3.2 / 6.1 / 9.6 / 2.2; Chrome 20.4 / 14.4 / 17.7 / 19.8 / 10.5.

- Third M2 run, wasmtime, best of 3, load 14 to 17: sieve native 0.449 s, every check 3.23 times
  native, no stop check 1.49; matrix native 0.079 s, every check 8.06, no stop check 5.20. An experiment
  not kept, same conditions: with the registers not loaded back after the stop hook says carry on (kept in
  locals instead), every check gave sieve 2.90 and matrix 7.34 times native, about 11% faster. Not kept
  because the plan says the code loads its registers back after the hook; the gain says the stop check's
  cost is mostly elsewhere (the write back of every dirty register on its slow path, through one
  imported global pointer per register; see "Next").

## Next

1. C1, the progress count (see "C1").
2. The literal fix list: arm64's decoders and forms (steps 17 to 20) and the test crate's remaining lines
   (orchestrator's `magic-literals/fix-translator.md`).
3. Refinements of M5 ("M5, fourth run, what comes next", in the file's history).

## Known gaps (behave as the plan says: illegal instruction, counted, warned)

- T3: a tail call through a register to a function that nothing names as a start (no start given, no direct
  call to it) and that lies before the next certain start is counted "jump through a register not
  recognised", although it is a tail call: since 40fd0c67 a function's whole reaches up to the next certain
  start, and such a jump has the same shape as a computed goto (an address loaded from data, then `br`), so
  the two cannot be told apart from the code. The output is right; the counter over-reports and the warn hook
  is called at each such jump. Stripped binaries keep this: busybox and the C library have no symbols and
  almost no unwind tables (plan, "Finding functions"), so nothing names a function reached only through a
  table of function pointers. The typical case is a tail-calling interpreter's handlers: the fourth check's
  `c4_tailptr.c` (six handlers in a table, each ending `return ops[*pc](...)`) counted 15,001 of its 18,000
  tail calls at -O1 to -Os, full look-ahead (0 at -O0). Kept as a known failure:
  `programs::a_tail_call_to_a_function_only_data_names_is_not_unrecognised` (`register_jumps.S` with
  `STEP_AFTER_CALLER`, the target right after its caller), ignored with this reason; it counts 3,600 against
  2,400 (C1 run, rented ARM64 machine). Recognising it would need starts read from data (words pointing at
  code); not done.

- T3: a switch whose code has no bound check before its table (the compiler proved the index's range, as for
  `k % 48` or an argument of a static function whose every caller it sees) is left unrecognised, as the
  table's length cannot be known. Its cases are never found, so each is made at run time once (bounded
  look-ahead, counted "translated at run time", warned), and the jump to them goes through the lookup, counted
  "jump through a register not recognised" each time it runs (since 40fd0c67; before it, from b4384452, not
  counted, because the target lies past the explored code). A computed goto (a threaded interpreter's
  dispatch) is the same slow path: its labels are not found, made at run time, and every dispatch is counted
  (`unexplored_jumps.c`). Measured with `cut_switch.c`'s `cut` static (as at 486ee285), -O2, full: 1,190 translations at run time, 0
  counted unrecognised; with `cut` not static the compiler checks the bound, the switch is recognised and 3
  remain (the C library's, as at -O0). The independent check's `check_big.c` (`switch (k % 48)`, 1,622 at run
  time) is expected to be this case; not measured separately. Recognising it needs the index's range from the
  code before the jump (an `and` mask, a remainder); not done. (Earlier runs put this down to switch cases in
  another part of a cut function: wrong, such cases are entries of their part.)
- T3: a function cut in two because it is too large for an engine (`halves`, a second plan with more starts)
  becomes separate functions for the unrecognised jump's range check, unlike a function cut for
  `FUNCTION_BLOCKS`, whose parts share their whole function's span.
- M5: pieces made at run time from the middle of a function (after a lost stack, a go to address or a
  forgotten piece) do not recognise switch tables or prove loops the way the function's full piece does: much
  slower on unwinder-heavy code (figures under "M5, first run"), counted and warned.
- M5: code the program writes and calls directly is translated from memory when reached; a piece of
  look-ahead none or full that holds written code inline (reached by a direct branch inside a stretch made
  from the run of runnable pages) keeps the old code after `ic ivau`, as the "code changed" gap below says.
- M5: a switch table of more than 65,520 slots (an engine limit for `br_table`) is left unrecognised (counted
  as "jump through a register not recognised"); Bun has one of 360,445.
- M5: translated for size, a long loop in a function entered once stays in V8's baseline code in Chrome (no
  loop pieces to carry on in after a stop), up to 9 times slower than speed on the benchmark set. That is the
  trade the size setting makes; measured under "M5, second run".

- FPSR's cumulative exception flags are kept from the start for a program whose code reads FPSR (see "For
  the orchestrator": the runner decides, by `reads_float_status`), otherwise once it writes FPSR. Code that
  reaches an FPSR read without that (a library loaded later, code written at run time) gets the flags kept so
  far, counted and warned as "exact floating point requested". A thread made by the kernel (fork, a new
  thread) starts with them as the runner sets them.
- CASP of 64 bit pairs is a plain load, compare and store: exact with one thread, not atomic with more
  (WebAssembly has no 128 bit compare and exchange).
- The "code changed" stand-in forgets the lookup table's pieces of the 64 byte line only; full pieces that
  hold the code inline elsewhere keep it. Code that changes is out of phase one.
- The float hook's counters carry the operation as their detail, not the address (registers are not saved
  at that hook).
- Flat blobs carry their program headers after the image, but the image has no PT_PHDR, so musl finds
  thread local data (placed by `flat.ld` since M4) only in a blob that is not moved.
- At full, a floating point instruction that must take the slow path runs alone from its piece of look-ahead
  none (the table's last slot), then the function carries on in pieces of bounded look-ahead made from the
  next instruction. Exact either way.
- Only 64 bit guest memory. 32 bit memory hosts need the high-bit check (section 6).
- A test whose guest never ends hangs rather than fails (nothing bounds how long a translated run may
  take); the hang this run fixed showed as layer 2 and `tests/checks.rs` running for ever.
- Finding functions explores every entry again when a round of cuts adds entries (functions of more than
  `FUNCTION_BLOCKS` blocks); measured on the large binaries under "M5, fourth run".
- Stand-in brk never moves the break.
- A stop flag raised from outside is tested in wasmtime only (`tests/smoke.rs`, before a run and by a
  second thread mid-loop). The Chrome runner has the same hook code.
- Slow paths taken inside translated code (left to an address) and hook calls that do no work are counted
  with zero time; only the runner's own work is timed.
- Layer 1 checks only words the CPU we present has. Whether undefined words really raise SIGILL on a CPU
  without the feature is checked against Arm's list, not against a CPU (the reference CPU has newer
  features than the one we present).
- Layer 1 differs on the rented Graviton4 in two forms (found by the orchestrator's setup): `MSR FPSR`
  keeps bits 28 and 29 there, and `CPP RCTX` runs there. Both are reference differences, not something
  left undefined: the CPU we present is 64 bit only at EL0 (`ID_AA64PFR0_EL1` EL0 field 1), where FPSR's
  bits 31 to 28 are reserved, and keeps only `FPSR_BITS` (reads zero, writes ignored, one of the two
  behaviours Arm allows a reserved bit, and the M5's); it has no `FEAT_SPECRES` (not in
  `OFFERED_FEATURES`), so `CPP RCTX` is an undefined instruction, as on the M5. The Graviton has
  `FEAT_SPECRES` and keeps those bits, so layer 1 runs on the Mac. Read from the code, not run. The P2
  check ran it there (seed 4244, 2,000 cases a form) and found a third: `DC ZVA` on an address that faults
  reports the address in the register there, while the M5 and the translator report the start of the 64
  byte block (shrunk case: x0 = 1, Linux `MemoryFault(1)`, ours `MemoryFault(0)`). Two Arm CPUs differ here,
  so taken as a choice the architecture leaves open (not checked against the Arm ARM's text): a reference
  difference, and the translator keeps the M5's answer.

## Departures from the plan

- C1: the count is classified from `Guest::flow` (the shared guest interface) rather than per ARM64
  instruction, so every guest gets definition E with no code of its own; it is added at the instruction's
  first way out (`jump`, `jump_if`, `call`, `ret`), after its own memory checks, so an instruction that
  faults before it runs is never counted. A jump through memory (x86) counts as a jump through a register.
- C1: the cost was measured with this track's `measure_progress_count` (wasmtime only), not the prototype's
  harness; Chrome was not measured (the bar names hosts, not engines). The browser runner keeps the count
  absent.
- C1: the Csmith check runs layer 3's programs without Linux (the count's sameness needs no reference;
  layer 3 already compares their output with Linux), and leaves out programs whose count passes 2^28 at full
  look-ahead (counted and printed), as layer 3 leaves out those Linux does not end in 20 s.

- T3: layer 2's rerun on the merged code is 100,000 programs (10 percent of the bar), every one compared with
  the real CPU, rather than the million: that is 100 times the CPU comparisons of P2's whole layer 2, and the
  milestone asks for reruns, not the bar. Layer 1's rerun is the full bar (10,000 cases a form) on the Mac.
- T3: `differences_match_linux_on_other_hosts` runs differences.c twice: as it is, which reads FPSR, so its
  flags are kept from its start and its software operations are counted as "exact floating point requested"
  (asserted: none counted as "software fused multiply-add"); and built with `WITHOUT_FENV` (no rounding modes,
  no flags), which never reads FPSR, so the software fused multiply-add and the fast path's not-a-number fix-up
  run and must give Linux's output, with "software fused multiply-add" counted (fourth run; between 06187cfd
  and 59cb1416 only the first ran, and the fix-up was compared with Linux nowhere).
- T3: C test programs (`programs::build_with`, `Library::C`) are linked with `--gc-sections`: zig 0.16 keeps C
  library code nothing calls (`fmal`, `hypotl`, ...), and `fmal` calls musl's fenv functions, which read FPSR,
  so every C program was taken as reading FPSR and ran all its floating point on the exact path (from 06187cfd
  to 59cb1416). Layer 3's Csmith clang builds were left as they were until 0b62e4ad, so from 06187cfd to
  then a Csmith program that kept musl's fenv code (8 of 48 in a sample) ran its floating point on the exact
  path; since 0b62e4ad they link with `--gc-sections` too (0 of 48).
- T3: busybox awk's counts were measured with `wcon` (the runtime track's command line program) on the rented
  x86 machine: the translator's output does not depend on the host, and that program is what runs a real
  container; this track's own binaries test only translates and loads.
- T3: the test harness's stand-in kernel and layer 1 take Linux's error numbers, `UIO_MAXIOV`, `iovec`'s
  layout, the signals and `BUS_ADRALN` from linux-raw-sys, already a dependency of the test crate, although
  the fix list says to use the kernel's generated ARM64 copy: these values are the same on the x86-64 and
  ARM64 hosts the harness runs on. Depending on wcontainers-kernel was tried and dropped: the literal check
  then reads every test literal against the kernel's constants too, and nine unrelated test values (300,
  8192, 0x80000, ...) became new findings. The stand-in's ARM64 system call numbers and the starting stack's
  auxiliary vector numbers (`layout.rs`, also compiled into the native reference runner, which has only
  libc) stay in the baseline until there is a home both sides can import (orchestrator's call: a small
  crate for the kernel's generated Linux module, as the fix list suggests).
- T3: the stack alignment (16) is still a literal in full.rs and layout.rs: its named home is a new constant
  on the shared `Guest` trait in wcontainers-guest-cpu, which the x86 track also implements; left for a
  change agreed on main.
- T3: the look-ahead codes and the description section's version are named in the translator
  (`LookAhead::code`, `LookAhead::from_code`) and used by browser.rs and the browser translator; runner.js
  still writes 0 for look-ahead none in two places and skips the description's 3 header bytes by number,
  left alone in this run because V8's speed on interpret moved with a two-line runner.js change (see
  "Speed pairs").

- P2: layer 3's bar of 100,000 Csmith programs is read as 100,000 compared with Linux; programs that do not
  end on Linux in time are made and counted apart, not compared, so more than 100,000 are made.
- P2: the long-run driver runs an exported copy of one commit in the run's directory rather than the
  checkout, so a fix being worked on cannot leak into chunks recorded under an older commit.
- P2: layer 3's guard for blobs that Linux ends by a signal was narrowed (33afa1e): the program built as an
  ordinary executable must end by the same signal, but may have written something else before, since a
  program that faults may print through a wild pointer whose target depends on the layout (failure 5 of
  P2). This lets one more kind of case be compared; it skips nothing, and a case whose two builds end
  differently still fails.
- P2: the same guard takes SIGSEGV and SIGBUS as one ending, a memory fault (`same_ending`, failure 7 of
  P2): which of the two a stray access raises depends on what lies at its address, and an ordinary
  executable has file pages past its file's end (SIGBUS) where a blob has none.
- P2: the same guard then took SIGILL and SIGTRAP into that one ending, "gone astray" (`same_ending` over
  `ASTRAY`, failure 9 of P2): a stray jump into data faults (SIGSEGV) in an ordinary executable, whose
  data cannot be run, but runs the data in a blob, which is one region that can be (`placement`), so it
  ends by whatever the data decodes to. Any other pair of endings (a clean exit against a fault, a fault
  against SIGKILL or SIGABRT) still fails. These three are the only changes to a test's checks in P2; in
  each the blob is still compared with the translator exactly, and only cases the guard stopped before
  any comparison are let through to it.
- P2: the long-run driver gives each run its own zig cache (`WC_LONG_DIR/zig-cache`): zig processes
  sharing `~/.cache/zig` with other jobs deadlocked on its locks (failure 4 of P2).

- P1: the long-run driver is a shell script (`long-runs.sh`), not Rust: it only runs test commands one after
  another and keeps a ledger.
- P1: layer 2 now runs a sample in Chrome (the task for P1 asks for one; the plan asks for a sample of each
  layer in Chrome). Its cases are each made from the seed and the case number, so a layer can be cut into
  chunks by case offset and run at once; the seeds of earlier runs no longer give the programs they gave.
- P1: the plan's pass bar for layer 1 counts 10,000 cases a form; the driver runs them as ten seeds of 1,000
  cases a form, which is 10,000 different cases a form.
- M5, fourth run: the translator's maps use `rustc-hash` (an established crate, already a dependency)
  instead of std's SipHash: keys are guest offsets, and translation speed is a goal while hash flooding is
  not a risk here (a hostile binary can only slow its own translation).
- M5, third run: section 11 asks first for what makes the output smaller at no cost in run speed. The slow
  paths out of line (shared functions for the write back on paths that never come back) were built as that,
  and measured free on the benchmark set in the speed setting, but they slow loops that run with their checks
  (sieve in the size setting 0.83 to 1.3 s), so they are a size trade. Nothing found this run is free; the
  speed setting's output is as before (less 0.4% from the depth check's shared save).
- M5, third run: the translator uses every core (rayon). Tuning the number of threads is the host's
  (`RAYON_NUM_THREADS`), not a setting of the translator. The measurements here ran with 6.
- M5, second run: the compact lookup's array of slots is in the state memory, per process, written by the
  runner (later the kernel) from each code module's list of pieces; the table is per thread and grows; slot 0
  is always empty and slot 1 holds the exact path's piece (the plan says only "an array in memory from code
  offset to slot").
- M5, second run: size mode's trades are no proven loop copies and no loop pieces; the first costs much in
  loops and saves little, but the second needs it.

- M5: Node is taken from the official `node:24-alpine3.22` image (Node 24.16.0, 125 MB, 40 MB of code, 10.0
  million instructions), not Alpine's own `nodejs` package (Node 22, 51 MB, 25 MB of code): the plan's figures
  (9.13 million instruction slots) are of a Node of that size. The other kinds: Python is
  `libpython3.13.so.1.0` of `python:3.13-alpine3.22` (the interpreter is in it), Bun `oven/bun:1.3-alpine`,
  Java `libjvm.so` of `eclipse-temurin:21-jre-alpine`, Go the `go` command of `golang:1.25-alpine3.22`.
- M5: the code range given for a binary is from its first section of instructions to the end of its last,
  not the whole runnable segment (which also holds headers and read-only data); the blob is the whole loaded
  image, so jump tables and constants in read-only data are there.
- M5: a translation is one or more code modules (`Translation { modules }`); pieces of one module reach
  another's through the lookup. Grouping is in order of entry; a module holds at most 1 GiB less 64 MiB of
  code and a million functions less a thousand. A test-only entry (`translate_in_modules`, measure feature)
  splits small programs to test it.
- M5: a new tuning number, `FUNCTION_BLOCKS` (2048): a function with more blocks is cut in parts at places
  control passes between runs of its blocks in depth first order. Bodies or locals past the engine's limits
  for one function cut it in two again. Not measured for speed: cut parts tail call each other.
- M5: the stand-in's load code hook translates from guest memory as it is, from the blob's code range or, for
  other addresses, from the run of pages that allow running code; a jump to a page that does not is a memory
  fault with access `Execute`. The runners' tables cover the whole blob. The changes test keeps "may run code"
  as placed.

- M4, second run: "the stop flag is raised at a random check" (section 9) is done as a stop at every stop
  check of a mixed run, with the stack lost or a piece forgotten at random stops among them; the only way to
  reach a random check of a long computation without a system call is for the thread to stop at each one.
  One mixed run in four (by seed) stops nowhere instead.
- M4, second run: the stack budget is compared with the depth of a piece's callers, not with that plus the
  piece's own frame, so the frame of the piece in progress may sit above the budget (otherwise a piece whose
  own frame is past the budget could never run).
- M4: every piece takes one more parameter than the plan's (passed registers and the expected return
  address): the estimate of the engine stack its callers use, for leaving deep recursion to the bottom loop.
  The plan names no mechanism; this one needs no memory access and nothing at returns.
- M4: a return to the bottom loop that does not go where it expects returns to it (pc set) instead of
  throwing, and is counted as "continued after stack loss" whatever lost the frame (a lost stack, a go to
  address, a stack left for being nearly full). The plan counts "continued after stack loss" only for the
  first.
- M4: one new reason, "engine stack nearly full" (13), for leaving to the bottom loop at the stack budget.
- M4: the lookup table has one slot more than the addresses it covers, holding the one-instruction piece of
  the exact path, and the load code hook takes `exact` to fill it; the table's address slots are only ever
  filled where empty, so code made at run time never replaces a piece.
- M4: `Bounded(1)` is defined as the one instruction's piece of look-ahead none (seeing one instruction is
  what none is); `LookAhead::None` still translates every instruction the starts reach, for the reference.
- M4: a runner's reference machine at none translates code reached at run time at none (whatever its
  starts reach), so the reference stays at none; other machines use bounded. The Chrome runner makes run time
  pieces at the default N only.
- M4: the C++ and Rust test programs bring their own allocator over a static area, because the stand-in has
  no mmap (and its brk never moves); the Rust program is `#![no_main]`, so the C library calls its `main`
  directly, because the standard library's own start makes system calls the stand-in does not answer (poll,
  sigaction) and aborts on "not implemented". They are built at fixed positions only (musl finds thread
  local data only in a blob that is not moved).
- M4: blobs carry a stack size and the reference runner takes it as an argument (`<stack bytes>` after the
  stack top); 1 MiB as before unless a test asks for more (deep recursion: 8 MiB, Linux's default).

- M3, third run: layer 3 compares a blob that Linux ends by a signal (ours must end the same way) when the
  program built as an ordinary Linux executable ends the same way; Csmith's programs are meant to have no
  undefined behaviour, but some fault on Linux all the same.

- M3, second run: at full the floating point slow path leaves the piece by a tail call to code at none
  from the instruction (counted as "left to an address" besides the float hook's own count), instead of
  calling the software path in place; a call in place doubled floating point code's time.
- M3, second run: flat C blobs carry the ELF file's program headers after the image, and runners give
  `AT_PHDR`, `AT_PHENT` and `AT_PHNUM`, so a static position independent program can relocate itself when
  loaded away from its link address. This is test tooling standing in for the kernel phase's ELF loading.
- M3: decoding of the new families goes by Arm's forms (names and fields from the machine readable
  specification already in the repository), not yaxpeax-arm, whose AdvSIMD and system decoding leaves out
  or misnames too much; yaxpeax-arm still decodes what it did.
- M3: the plan's "a program that changes the mode gets floating point done in software" is done per
  operation at run time (each floating point instruction tests FPCR and the tracking word), not by
  translating again; and status flags are kept from the first write of FPSR, not from the start.
- M3: the estimate instructions (FRECPE, FRSQRTE, FRECPX) have their table algorithm twice: as WebAssembly
  for the fast path and in `exact.rs` for the slow one, which must also flush and raise flags. Every other
  meaning is once as WebAssembly, with the software floating point library behind the slow path.
- M3: the CPU side keeps two things of its own after the abi's thread block (the float tracking word and
  the exclusive monitor), in the state memory; the kernel never sees them. A monitor lost at a stop only
  makes a store exclusive fail, which programs retry.
- M3: we answer CNTFRQ_EL0 as 1 GHz (the counter is the clock in nanoseconds) and CTR_EL0 with IDC and
  DIC clear, so programs that write code tell us with IC IVAU.

- A kept run that misses is not counted or warned: it is the ordinary exact check, not a slow path
  (orchestrator's ruling, M3 close-out). A loop whose proof fails is counted ("loop not proven").

- S1 close-out: in `tests/changes.rs` changes never touch the pages from sp's page to the top of the stack,
  because the plan lets a frame's check hold until the next function entry ("What still gets through"), so
  full and none may differ there by design. Memory there that is no frame is still covered: layer 2's area
  is in the blob since the M3 close-out, so its pages change. The page below sp's page is changed (often,
  since the eleventh run), and with it memory a popped frame left below sp. Faults the test handles make
  the page readable and writable; the plan's section 9 asks only that faults reach the hook with the same
  registers as none, the handling is there so a run meets many faults. Before M4 the test was weaker than its
  tally of faults suggests: after the first fault it handled, the run carried on through code translated at
  none, which replaced the table's pieces of every address it reached. Since M4 the run carries on in pieces
  of bounded look-ahead put only at their own address, so every function's full piece stays in the table;
  see "M4, first run".

- Two kinds of module per thread: a thread module (lookup table, unwind tag, bottom loop) and code modules
  (pieces only, importing the thread module's table). The plan has every module export run and save/load;
  here only the thread module exports run, and since S1 there is no save or load state at all (see S1), so
  code loaded later and split modules are all the same kind.
- A second memory, "state", shared, holds thread blocks (stop flag at 800) and the permission table, so the
  guest cannot reach them.
- The permission check and the stop check are in look-ahead none from M0, so layer 1 can compare faults
  exactly. In none every piece checks the flag at entry (atomic load) and after every system call.
- Top Byte Ignore is part of the meanings: data addresses ignore the top byte when bit 55 is clear (Linux
  turns it on for user space); branch targets through registers get the same treatment. Costs a few
  instructions per access; to be measured at M2.
- LDAR/STLR are reported unsupported rather than translated as plain accesses: they need an alignment fault.
- Layer 1 excludes: CONSTRAINED UNPREDICTABLE encodings (write back to a transfer register, load pair into
  one register), branches to themselves (never finish), and hints whose meaning differs on a CPU with pointer
  signing or later features (the reference CPU has them, the CPU we present does not; we run them as
  nothing, as the plan says). System register moves are limited to NZCV and TPIDR_EL0.
- The alignment check for jumps is in the thread module's bottom loop and in each code module's dispatcher,
  where a target with no piece is handled, so it costs nothing on the normal path.
- A slow path warns at its first occurrence per address (`WARNING_THRESHOLD` = 1); no measurement yet says
  a higher threshold is needed.
- The forms list moved to `wcontainers-arm64` (it is ARM64 knowledge the decoder now uses);
  `wcontainers-translator-test` keeps the tool that makes it.
- Arm's site refuses automated downloads (403/404), so the specification came from the kernel.org mirror of
  Arm's BSD release (kernel.googlesource.com/pub/scm/linux/kernel/git/maz/AARCHMRS, release 2026-09), kept in
  pyro-parked/wcontainers-scratch/translator/arm-mrs. Only the derived forms list, with Arm's notice, is
  committed.
- Layer 1's knobs are WC_LAYER1_CASES, WC_LAYER1_SEED and WC_LAYER1_FORMS (one proptest runner per form).
- Test programs are built with `-mcpu=generic-neon-fp_armv8`: zig ignores `-mgeneral-regs-only`.
  (Programs with the C library are built for zig's default CPU, as the library is.)
- Vector instructions brought forward from M3: the C library's printf path reaches SIMD loads and stores
  and the 54 vector data processing forms above, so hello cannot run without them. Each is proven by
  layer 1 like the base forms.
- Words of a translated vector form whose arrangement Arm marks reserved decode as undefined.
- Layer 1, on a load or store that faults on both sides: the registers the reference loaded and the
  memory it stored count as UNKNOWN (Arm's rule), and are taken as ours, provided ours changed no memory.
  The reference CPU writes part of a 32 byte pair that crosses onto a faulting page; ours writes none.
- The browser translator is a separate package (`browser/`, own workspace and Cargo.lock) because it
  builds for wasm32. It reuses `wcontainers-translator` unchanged.
- Flake: added `WC_PLAYWRIGHT_CORE` = nixpkgs `playwright-driver`, so Node can drive the installed Chrome
  without npm. Chrome itself is the one installed on the Mac (channel "chrome").
- Stand-in write and writev give -EFAULT for a buffer that is only partly readable. That is what Linux
  does for a pipe within a page; for a pipe write that spans pages, or a regular file, Linux writes the
  readable start and returns a short count. The stand-in's output is a pipe in the reference, and the
  real kernel replaces the stand-in.
- The runners make the thread leave after exit, exit_group (and, in layer 1, after any system call) by
  raising the stop flag themselves; the stop hook then returns 1 without counting. A flag raised from
  outside is counted and the thread carries on.
- A word of the CPU we present that yaxpeax-arm rejects or that we do not translate is `Unsupported`, also
  where Arm's decode pseudocode makes it UNDEFINED (that is not in the forms list): both raise illegal
  instruction, but such a word is counted as "unsupported instruction reached" too.
- Full look-ahead finds functions only from the code (starts, `bl` targets, built addresses). It does not
  take a jump that leaves its function as a new entry unless something else makes it one; such code is
  copied into the jumping piece instead (correct, larger).
- Stop flag read with a plain `i32.load`, not an atomic load. An atomic load cost the same here, but a
  plain one is what the plan measured; an engine may in principle keep a plain load out of a loop, so
  `tests/smoke.rs` checks that a flag raised by another thread mid-loop is seen, at none and full, in
  wasmtime. Not yet checked mid-loop in Chrome.
- After a system call, and after the stop hook says carry on at a switch's jump back, registers are loaded
  back from the thread block instead of being kept across the call (it keeps registers out of
  callee-saved registers). Elsewhere since S1 a piece does not reload after carrying on: see S1.
- A frame or group permission check that fails leaves to the instruction (`UNWIND_GO_TO_EXACT`, counted
  as "left to an address") instead of working out the exact fault in the piece. Since M4 that one
  instruction runs alone from its piece of look-ahead none, which the load code hook puts in the lookup
  table's last slot (a slot for no address), and the thread carries on through the table from the next
  instruction, in pieces of bounded look-ahead. The table's pieces are never replaced for it.
- The permission check of none changed shape too (fast test, then exact test), so both look-aheads share
  it; layer 1 passed after the change.
- Without the permission check (`Omit::permissions`, only for measuring) addresses are not untagged either,
  because untagging is part of the check's slow path now.
- The table byte after the last page must read as "nothing allowed"; the runners size the state memory for
  it. Pages past the table are clamped to that byte.
- A recognised jump table that reaches a target not in its table counts as "jump through a register not
  recognised" (it goes through the lookup like one).
- An executed jump through a register that no table explains calls the warn hook (counted per execution, as
  every slow path is) when its target lies inside its function, in its own part or another of a function cut
  in parts, other than the function's entry. A jump to outside it, or to its entry, is a tail call through
  the lookup, not counted (since b4384452, c2bafa58 and e26b3c23; before, every one was counted, and tail
  calls through function pointers, common in the C library, paid a host call each time). "Inside" is from
  the function's lowest explored instruction up to the next start given or direct call's target (40fd0c67),
  so a tail call to a function known only by its address (a pointer in data) that lies just after the
  jumping function is counted as the slow path too: the count errs towards counting. A stretch (look-ahead
  none or bounded, and every piece made at run time) counts every jump through a register within the code
  other than to its own start, as it knows no function's bounds.
- Layer 2 compares against the real CPU only for the first `WC_LAYER2_NATIVE` cases, because the
  reference runs one blob per container; full against none is compared for every case.
- Layer 2 writes `mrs`/`msr nzcv` as raw words, because dynasm has no name for the register.
- Code translated at run time (M4): the load code hook puts one piece, of bounded look-ahead from the
  address asked for, at that address only, and only where the table holds none (the wasmtime runner
  asserts it). A runner's reference machine at none translates code reached at run time at none.
- Vector registers, FPSR and FPCR at the start of a flat blob: the runners and the reference start v0 to
  v31 at zero, as Linux does at exec. The reference leaves FPSR and FPCR as its own process had them.

- S1, approved by the orchestrator: registers that are not in locals live in the thread block in the state
  memory, at their places in `RegisterBlock`, not in the instance's globals (the plan: "registers live in
  the instance's globals"). There is no save or load state: the block always holds the registers when a
  hook runs or `run` returns, and a runner sets registers by writing it.
- S1: after the stop hook says carry on at a jump back, the thread goes on in a new call of a loop piece
  of that loop (or of the piece itself when the loop starts at its entry) instead of going on in place
  (the plan: "loads its registers back and continues"); at the entry check it goes on with nothing
  reloaded. Both rest on `Resume::Continue` meaning nothing changed. Loop pieces are pieces the lookup
  table does not hold (the plan has every piece in the table): only stop checks of their own function
  enter them, so they may rely on what that function knows at the loop. They are not listed in the
  description section either.
- S1: the browser page raises the stop flag at 1, 2, 4, ... 1024 ms after the guest starts, so V8's
  optimised code is used in long loops of functions entered once. A real browser host needs the same (or
  any other early raises); wasmtime needs none.
- S1: the plan's "whole function reasoning" for permission checks is a kept run of up to eight allowed
  pages per base register and kind in loops without calls (see step 6 and the third run), and a proof over
  the loop's range at its entry that picks a copy of the loop without those checks: for counted loops,
  accesses that move by a fixed step (fifth run); for any loop without calls, accesses whose index a
  comparison on the edge into the block, or the narrow load that set it, bounds, such as a switch's table
  (eighth run). Loops whose switch can jump back are left out, because a stop there carries on in place.
- S1: a proven loop's copy with its checks is only in its loop piece; the function tail calls into the loop
  piece when the proof fails. Loop pieces check the stop flag at their entry, and a stop check at a jump back
  to a loop with a loop piece writes back and tail calls it without calling the stop hook itself.
- S1: registers a loop neither reads nor writes are kept in the thread block through it and loaded on the
  edges leaving it (not for loops holding a switch).
- S1: registers dirty at a loop's entry that the loop does not touch are written back before it (the plan
  writes registers back only around calls, system calls, slow paths and the piece's edges).
- S1: the low bit of a piece's expected return address says whether the caller reads the flags after the
  call (`FLAGS_WANTED`); `BOTTOM_RETURN` changed from -1 to -2.
- S1: pieces throw through a small function of their module (wasmtime makes a function that throws save
  every callee-saved register), and read the imported values from the module's own globals.
- S1: the contract requires a guest memory whose base never moves (the runner's wasmtime engine has
  `memory_may_move(false)`).

## Shared crate (wcontainers-abi)

- T3: added `PAGE_SHIFT` (`PAGE_SIZE.trailing_zeros()`) and `PAGE_MASK` (`PAGE_SIZE - 1`) beside
  `PAGE_SIZE`, exported from the crate root, as the literal fix list's step 1 asks (additive; nothing else
  changed).

- P1: no change by this half. The merge of `origin/phase-1` brought the kernel half's two changes
  (`CounterReading::detail`, `Host::output(Stream, bytes)` in place of `Host::terminal_output`); the runner
  already sets `detail` to 0, as it counts by reason only, and nothing here implements `Host`.

- M5, second run: no change. The array of slots, its imports and the exact slot are in the translator's
  contract (`contract.rs`).

- M5: no change. A jump to code that may not run uses the existing `Access::Execute`.

- M4: no change. The new unwind payload, hook argument, reason and the piece's depth parameter are in the
  translator's contract (`contract.rs`), not in the abi.

- M3 close-out: no change. The new reason "loop not proven" is in the translator's contract
  (`contract::Reason`), not in the abi.
- M3, third run: no change.
- M3, second run: no change.
- M3: no change. The CPU side's own words (float tracking, exclusive monitor) are after the abi's block, in
  the translator's contract (`THREAD_STATE_SIZE`), not in the abi.

- Added `Exit::AlignmentFault { address }` (one new variant, additive): a load or store through an SP that
  is not a multiple of 16 (`pc` at the instruction, `address` = SP), or a jump to an address that is not a
  multiple of 4 (`pc` = `address` = the target). The kernel should deliver SIGBUS, BUS_ADRALN, si_addr =
  `address`, as Linux does. The kernel half must add a match arm for it. Needed because the independent
  check found the translator carrying on where Linux faults; without it the CPU side cannot report the
  fault. Later LDAR/STLR and exclusives on misaligned addresses will use it too.
- Changed once after M2 (commit b0df319, "made the abi layout test pass the linter"): in `registers.rs`'s
  test module, `assert!(STOP_FLAG_OFFSET + 4 <= THREAD_BLOCK_SIZE)` inside the layout test became a const
  assertion next to it (`const _: () = assert!(...)`), because clippy with warnings denied failed on it
  (`assertions_on_constants`). Only a test changed; no type, value or layout. Earlier versions of this
  file wrongly said nothing in the shared crate changed in S1.
- S1, first run: the CPU side kept one word for itself in the thread block's padding (offset 804). Since
  the second run's loop pieces it does not: the CPU side uses no byte of the block outside the abi's
  layout.
