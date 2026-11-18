# x86 track status

Plan: docs/phase-2-guest-cpu-and-x86.md. Branch: phase-2-x86. Brief:
pyro-parked/wcontainers-scratch/orchestrator/brief-x86.md.

## Where things stand

- X0 (the plan, and the x86 reference machine proven): done in run 1, 2026-11-17; the first independent
  check's findings fixed in run 2, the second check's in run 3, the third's in run 4, the fourth's in
  run 5, the fifth's in run 6, the sixth's in run 7, the seventh's in run 8, the eighth's in run 9 and
  the ninth's in run 10, the same day (below).
- X1 (the interface, with ARM64 moved onto it): run 1 moved ARM64 and the translator onto the interface;
  run 2 (2026-11-17) fixed what the independent check of X1 found, above all building the whole of section
  3 with the translator's side of every item; run 3 (the same day) fixed what the second check found: a CPU
  without flags, grouping through a segment base, a test guest that now runs, the browser's hook path;
  run 4 (the same day) fixed what the third check found: overlapping instruction streams that meet again,
  half the translate time the move added, an alias of the abi crate. Run 4 said every part of the pass bar
  held, but the float hook program was still slower in wasmtime (never timed again after run 2); run 5 (the
  same day) found both causes of that and removed them, and cut the translate time the move added, but said
  wrongly that none was left; run 6 (the same day) measured it again and corrected that, made the guard read
  the resolved dependencies, and fixed the plan's RISC-V far call (see "X1, run 6").
- X2 (the shared test harness): run 1 (2026-11-17) fixed the four findings of the last X1 check, built the
  generic reference runner and transport and proved the x86 one on the i7-8750H, and moved ARM64's layer 1
  and blobs onto it. Run 2 (the same day) built `GuestTest` with ARM64's part and made the library of the
  test crate generic over it (runner, layer 1, programs, changes, mixed, layer 3, browser), the browser crate
  per guest by a feature, and layer 1's and four other test files written once over `G`. Run 3 (the same
  day) moved layer 2's emitter, the three hand-made corpora and the real binaries into ARM64's part, gave
  flat blobs the guest's own auxiliary entries on both sides, and made `long-runs.sh` run one guest. Run 4
  (the same day) moved ARM64's own program sources into `programs/arm64` and wrote the last ARM64-pinned
  test files (`programs.rs`, `benchmarks.rs`, `output.rs`, `binaries.rs`, `machine-code`) once over `G`
  (see "X2, run 4"). Run 5 (the same day) fixed what the independent check of X2 found (see "X2, run 5").
  Run 6 (2026-11-18) fixed what the second check of X2 found (see "X2, run 6"). Run 7 (the same day) fixed
  what the third check found (see "X2, run 7").
  Not built, proposed for X3: the recorded reference results and layer 1 batching of plan 7.3.

Proof at the end of run 1 (machine: the Mac, Apple M5; x86 machine load 0.1 to 0.5):
- Suite: `WC_REFERENCE_X86_64=tom@192.168.2.240 nix develop -c cargo test -p wcontainers-guest-cpu -p
  wcontainers-arm64 -p wcontainers-translator -p wcontainers-translator-test --no-fail-fast` (scratch
  `x2/suite2.log`): 95 passed (X1's 93 and the two x86 reference tests), 0 failed, 11 ignored (X1's 10 and
  `tests/cases.rs`), 26 minutes 40 seconds, Mac load 18.0 at start and 16.1 at end (X1 run 6: 25 minutes at
  load 13.4 to 22.0). The first run of it (`x2/suite.log`) failed `programs_match_linux_in_chrome` on
  `buffers`, which found the two blob differences above; fixed before the second.
- Clippy with `-D warnings` on the test crate (all targets), the interface, the translator, the browser
  crate and the runner for both CPUs; `cargo fmt --check`: clean.
- origin/phase-1-translator: fetched at the start and end, nothing new since 8b5e420 (merged).

## X2: what comes next

1. The fourth independent check of X2.
2. Plan 7.3's recorded reference results and layer 1 batching, proposed for X3 (see "Departures"); build them
   at X2 only if the orchestrator rules so. The plan's text now says they are not built.
3. For X3: `presented`, `REFERENCE_HAS` and layer 1's rule for an `Err` case (plan 7.2 "As built"). For X4:
   the shared layer 2 model's operations are ARM64's (ccmp's nzcv immediate, ranges ARM64 encodes in one
   instruction), so x86's `assemble` expands them; the X2 check noted it.
4. For X3: the x86 runner reports the signal frame's RF (bit 16) in rflags at every fault stop (seen on the
   i7-8750H: `rep stosb` into a read-only page, rflags 0x10202; trap stops give 0x202). Plan section 2 says RF
   and IF are masked out of the comparison; x86's `reconcile` must do it, or every faulting case differs.

## X2, run 7 (2026-11-18)

The third independent check of X2 found two blocking problems and five smaller ones. What was done:
- Blocking, an ARM64 fact in layer 1's generic no-form test: besides comparing with the reference, it required
  every translated no-form stop to be `Stop::Illegal`, which x86's prefix runs past 15 bytes (general
  protection on the CPU) would fail. The test now only compares, and is renamed
  `bytes_in_no_form_stop_as_on_the_cpu` (`_arm64`). For ARM64 nothing is lost: the reference stops with SIGILL,
  so the comparison already demands `Illegal`. Plan 7.2 "As built" says so.
- Blocking, run 6's claim that a wider read found no CPU facts left was wrong (as run 4's was): it missed the
  test above and smoke's 1024 slots. Run 7's read (below) found two more.
- `tests/smoke.rs`: the slots of its one executable page are `PAGE / G::ALIGN` (was a fixed 1024, 4096 / 4).
- `src/changes.rs` (found in this run's read): the frames the changes test leaves alone started at the stack
  pointer's page, which on x86 misses the 128-byte red zone a leaf function uses below rsp. They now start at
  `sp - G::RED_ZONE`, as the translator's frame checks do, and the "page below sp's" pointer is the page below
  the frames. ARM64's `RED_ZONE` is 0, so its cases are unchanged. Plan 7.2 says so.
- `src/runner.rs` (found in this run's read): `code_at`, the bytes given to `G::memory_fault`, took a fixed 16;
  it now takes `G::MAX_LENGTH`.
- The x86 runner's RF in rflags: noted for X3 under "what comes next" (no x86 `GuestTest` compares yet).
- ARM64's `AUXILIARY` doc comment names the bits 0x183 holds: fp, asimd, crc32, atomics.
- The SSH reference's runner is now kept under its SHA-256 and its content is checked on the machine with
  `sha256sum` before it is used, and each upload is checked before it is renamed into place; a damaged or
  wrong file is replaced, and a failed upload removes its part file. `sha2` 0.10.9 is added (it was already in
  the lock); `blake3` stays for the lists compared between commits. Plan 7.2 and 7.3 say so.
- Plan 7.3's recorded results and layer 1 batching: still proposed for X3 and not ruled on (see "Departures").
- The check's `ldaxr` at DATA + 1 is by design (ARM64 presents a CPU without FEAT_LSE2), nothing to do.

This run's read for CPU facts outside the per-CPU part, and its limits: grep of `src/` (outside `src/guest/`),
`tests/` (outside the `_arm64` wrappers and ARM64's own `decoding.rs`, `valid.rs`, `test_guest.rs`) and
`browser/` for fixed 4s, 16s, 0x1000 and 4096, `Stop::`, `Exit::` and `Fault::` kinds, fixed field indices,
register names, HWCAP and ARM64 names; then reading each hit, `src/layer1.rs` whole and the stack and fault
parts of `src/changes.rs` and `src/runner.rs`. Left as they are: 4096 as Linux's page size, the exit status
of each signal (same numbers on both), the 16-byte stack alignment at entry and the 16-byte `CodeBytes`
buffer (the longest instruction of every guest fits), `smoke`'s test that features not offered stop with
illegal instruction (the brief says so for x86 too). Not read line by line: `src/layer2.rs` (its model is
ARM64's by a recorded departure), `src/binaries.rs`, `src/programs.rs`, `src/browser.rs` and
`browser/runner.js` beyond the grep. So this is not a proof that none is left; X3's x86 `GuestTest` is what
will show the rest.

Proof of run 7 (the Mac, Apple M5):
- origin/phase-1-translator: nothing new at the start (2026-11-18 01:20).
- After the changes (`x2/run7/affected.log`, 01:24 to 01:31, Mac load 13.9 to 79.7): changes 3 passed; layer 1 5
  passed (1215 forms, 64 cases each, seed 1, 0 failing; 64 no-form cases; 105 hand-made cases; the Chrome
  sample); smoke 6 passed.
- The SSH transport on the i7-8750H (`x2/run7/reference2.log`, Mac load 26.7, x86 load 5.5 to 6.8 from another
  track's builds): 4 passed. The first try (`reference.log`) failed all four: the content check quoted the
  part file's name in single quotes, so `$$` did not expand; fixed before the second. Then the kept runner was
  damaged on purpose (one byte appended, still executable, 864113 bytes); the next run replaced it (864112
  bytes, its SHA-256 again the name), passed, and left no part file.

The second independent check of X2 found two blocking problems and four smaller ones (and a measurement).
What was done about each:
- Blocking, an ARM64 fact in the generic harness: `Binary::in_code` kept a real binary's function start only
  at a multiple of 4. It now takes `G::ALIGN` (`in_code::<G>`, and `unwind_starts::<G>` that calls it), as the
  code range two lines above already did. ARM64's starts are unchanged (its `ALIGN` is 4). A wider read
  for others (grep of `src/`, `tests/`, `browser/runner.js` and the scripts for 4-byte steps, ARM64 names,
  register numbers and HWCAP): the 4s left are i32 slot and flag words of the lookup array and the state
  memory, the eh_frame entry's length word and `gap / 4`, none a CPU fact; `elfconv.sh` is ARM64's own
  comparison tool (elfconv converts only ARM64). (Wrong: the third check found layer 1's no-form test
  demanding illegal instruction and smoke's 1024 slots; run 7 found two more. See "X2, run 7".)
- Blocking, the x86 runner ignored a failed `arch_prctl` and answered the case as run. Now `Machine::enter`
  returns `Result<(), &'static str>`; x86 sets FS and GS first, before anything else changes, and on a
  refusal puts FS back and changes nothing. The case is answered with a new status, `RESULT_REFUSED` (2) and
  the reason, which the harness gives as `CaseRan::Refused` (layer 1 fails with "the reference cannot start
  this case"). New test `the_x86_64_reference_refuses_what_linux_refuses`: FS at 2^63 and at 2^47, GS at 2^47
  and at 2^47 - 4096 refused; after them `mov rax, fs:[0]` with FS DATA + 0x40 reads the seed's word and a
  GS of 2^47 - 8192 runs and reads back. Plan 7.2 (trait, wire), 7.3 and 7.4 (bases drawn below 2^47 - 4096)
  say so.
- zig: every zig command now runs with `ZIG_GLOBAL_CACHE_DIR` and `ZIG_LOCAL_CACHE_DIR` at the checkout's
  `target/zig-cache`, so no other checkout's zig holds its locks, and is killed and fails with a message
  after 10 minutes (`wait-timeout`). The first run fills the cache (a C library per target).
- Plan 7.3 and `Stop::Alignment`: x86's `BUS_ADRALN` gives `si_addr` 0, so x86's stop is `Alignment(0)`.
  New hand-made case: `mov rax, [rbx]` with AC set and rbx DATA + 1 gives SIGBUS `BUS_ADRALN`, rip CODE,
  address 0 on the i7-8750H.
- TF in the reported rflags: the runner now reports the frame's TF only when the stop is `TRAP_TRACE` (signal
  and code both checked: SIGSEGV's `SEGV_ACCERR` is also 2) and iced decodes `popf` or `iret` (no other user
  instruction writes TF), else the case's starting TF. New cases: `popf` of a word with TF and of one
  without (TF as the word's), `popf` from an unmapped stack (SIGSEGV, TF clear, rsp unchanged), `nop` (TF
  clear). Writing them found a real fault in the runner: a `popf` that set AC killed the case child with
  SIGBUS, because Linux starts the handler with the case's AC and the handler's own unaligned accesses then
  fault. The read-back handler now clears AC with NT and ID, first thing. Plan sections 2, 7.2 (`may_differ`,
  `leave`) and 7.3 say so.
- Suite time: measured again below, with no zig stall.

Proof of run 6 (the Mac, Apple M5):
- origin/phase-1-translator: nothing new at the start (2026-11-18 00:10).
- x86 reference tests on the i7-8750H (`x2/run6/reference2.log`, Mac load 59 to 55, x86 load 4.3 just
  before, another track builds there): 4 passed, 43 hand-made cases (38 before, 5 new).
- `binaries` and `programs` after the changes (`x2/run6/affected.log`, 00:13 to 00:19, Mac load 72 to 67):
  binaries 2 passed, 1 ignored; programs 15 passed, 2 ignored, 320 s, with the zig cache new and empty.
- Same cases: `same-cases.sh d27cd70 1 2 3 8807` (`WC_OUTPUT_ROOT` scratch `x2/same`, log
  `x2/run6/same-cases.log`, Mac load 86 to 94): layer 1 1215 forms and layer 2 64 programs the same for each
  seed. Nothing this run changed draws cases.
- Suite at 3cfa9e1 (`WC_REFERENCE_X86_64=tom@192.168.2.240`, scratch `x2/run6/suite.log`): 78 passed (run 5's
  77 and the refusal test), 0 failed, 11 ignored, 23 minutes 34 seconds, Mac load 54.1 at start and 34.3 at
  end (run 5: 24 minutes 7 seconds at 42.0 to 43.8, and 27 minutes 10 seconds at 37.9 to 64.5). No zig
  build stalled.
- Clippy with `-D warnings` on the test crate (all targets) and on the runner for both CPUs, `cargo fmt
  --check`: clean.
- origin/phase-1-translator: fetched again at the end (00:44), nothing new.

## X2, run 5 (2026-11-17)

The independent check of X2 found two blocking problems and six smaller ones. What was done about each:
- Blocking, five items of plan 7.2's `GuestTest` missing and unrecorded. Built: `form_group` (ARM64 its
  group in Arm's list; layer 1's summary adds a line counting failing forms by group, after the line
  `long-runs.sh` parses) and `offered()` and `told()` (ARM64: `OFFERED_FEATURES`, and the abi's HWCAP bits
  by name; `offered_features_are_the_ones_programs_are_told_arm64` in `tests/layer1.rs` replaced
  `offered_features_are_the_ones_the_kernel_reports_arm64` of `decoding.rs`). Deferred and now recorded in
  plan 7.2 "As built" and "Departures": `presented`, `REFERENCE_HAS` and the `Err` rule (X3), and
  `Stop::SingleStep` (X6), each because nothing would reach it before then.
- Blocking, the x86 runner was a static PIE. Now built with `-C relocation-model=static`: `file` says "ELF
  64-bit LSB executable, x86-64, statically linked"; on the i7-8750H its maps show the image at
  0x60000000-0x6009b000 and the heap at 0x84172000 (plan 7.2 corrected; all guest addresses of the tests
  are below 0x5800_0000).
- The plan's text on batching, isolation and recorded results (section 4 item 12, 7.2, 7.3) now says what is
  built and that the rest is proposed for X3; the model is not recorded with results, and 7.2 says so.
- The SIGSYS window in 7.3 is `[CODE, CODE + 18)`, as 7.2 and the runner have it.
- A lost connection in the middle of a run: `Reference::request` starts the runner again and resends, at
  most five times a request, with a warning. New test `the_x86_64_reference_comes_back_after_its_connection_is_lost`
  cuts the connection twice (`Reference::cut` kills the transport's child) and gets the same answer each
  time. Its first run found a race: three tests uploading a new runner at once wrote the same part file,
  and one start failed with "Text file busy" (then retried and passed). Each upload now writes a part file
  named by the remote shell's pid; with the runner removed from the machine first, the three tests uploaded
  at once again and no start failed (scratch `x2/run5/reference.log`, `after-fixes.log`).
- `long-runs.sh` removes, after a stopped chunk, only the `wc-x86-` containers whose test process (the id in
  the name) has ended, never another live run's.
- Layer 2's model being ARM64's is noted under "what comes next" for X4 (already a recorded departure).
- `tests/cases.rs` now also writes layer 1's no-form cases (`no-form-<seed>.txt`, case texts) and the
  hand-made cases (`hand.txt`: name, compared, stop, fields, case text); `same-cases.sh` compares them when the
  earlier commit writes them. d27cd70 does not, so its no-form words were drawn by a scratch test added to
  its kept copy and removed after (`x2/run5/old-no-form-*.txt`), and compared with the words of this tree's
  lists by `x2/run5/compare-no-form.sh`. The hand-made cases are fixed values, not drawn; d27cd70 built them
  inline in four tests, so they have no list to compare with; from now on `hand.txt` covers them.

Proof of run 5 (the Mac, Apple M5):
- Same cases after the changes: `same-cases.sh d27cd70 1 2 3 7717` (scratch `x2/run5/same-cases.log`, Mac
  load 40.5 to 50.0): layer 1 1215 forms and layer 2 64 programs the same for each seed. No-form words
  against d27cd70 (`compare-no-form.sh`): the same for seeds 1, 2, 3 and 7717 at 64 and at 1000 cases, every
  line's state the same one (sp DATA + 0x1000, pc CODE, the rest 0, the data seed the seed).
- The x86 reference tests (3: the manual's 38 cases, 2 blobs, the reconnection) passed twice outside the
  suite on the i7-8750H (its load 3.7 to 5.1; another track builds there).
- Suite at 995e949 (`WC_REFERENCE_X86_64=tom@192.168.2.240`, scratch `x2/run5/suite.log`): 77 passed (run 4's
  76 and the reconnection test), 0 failed, 11 ignored, 24 minutes 7 seconds, Mac load 42.0 at start and 43.8
  at end (run 4: 24 minutes 12 seconds at 18.3 to 48).
- origin/phase-1-translator: nothing new at the start; at the end three commits (to 11cc5ad: the runner's
  stop hook reads only the pc, its counters in `FxHashMap`, `rustc-hash` added). Merged in 4bebb56: the
  conflicts were in `runner.rs`, now generic, so the stop hook reads the pc at `G::PC_OFFSET` and the maps
  stay keyed by reason numbers. After it `smoke`, `checks`, `changes` and `programs` passed (6, 2, 3, 15;
  `x2/run5/merge.log`, load 38.7 to 60.0), then the suite again: at 4bebb56 (`x2/run5/suite2.log`) 77
  passed, 0 failed, 11 ignored, 27 minutes 10 seconds, Mac load 37.9 at start and 64.5 at end.
- Clippy with `-D warnings` on the test crate (all targets) and on the runner for both CPUs, `cargo fmt
  --check`: clean.

## X2, run 4 (2026-11-17)

- origin/phase-1-translator: fetched at the start and the end, nothing new since 8b5e420.
- `programs/arm64/`: `syscall.h` (`syscall3` with `svc` and the numbers `SYS_write`, `SYS_writev`, `SYS_exit`,
  `SYS_exit_group`, used by bench, calls, buffers, integers, tables, walks), `switch.h` (coroutines' context
  switch, `start_context`, and `CALL_ON_STACK` for fibers), the five `.S` tests, `written_code.c` and
  `differences.c`. `programs::source::<G>` looks in `programs/<NAME>` first, then the shared folder, and
  `build_with` puts the guest's folder on the include path. `elfconv.sh` copies `syscall.h` beside bench.c
  (not run this time: it needs elfconv's 6.1 GB image).
- Written once over `G`, each with `_arm64` tests: `programs.rs` (its 15 tests as before),
  `benchmarks.rs`, `output.rs`, `binaries.rs`, and `machine-code` (its program stays ARM64's). Instruction
  starts and counts come from `boundaries::<G>` and slots from `G::ALIGN`, where they were `/ 4`.
  `GuestTest::PROBE_ANSWERS` gives every probe answer: `output.rs` translates for each, and `differences`
  now matches Linux for all four answers (was one), with ARM64's counter checks on the same host as before.
  `decoding.rs` and `valid.rs` and the HWCAP check are named as ARM64's own.
- `machine-code` printed `8 + i` as a piece's function index, stale since phase one; it now reads the index
  from the module's lookup table (integers -O2: the first piece is function 25).
- `same-output.sh` gives a commit from before `PROBE_ANSWERS` ARM64's four answers; it no longer adapts
  `output.rs` for commits from before the interface (their lists are kept, under scratch `x1/same/lists`).
- Bench bytes after the move, against 32631fc (this run's start): `same-output.sh 32631fc5 bench` (scratch
  `x2/run4/same-output.log`, Mac load 20.5 to 31.9): the same, 505 modules. After `output.rs` became
  generic, `bench,musl` (`same-output-2.log`, load 21.9 to 22.9): bench the same, 505 modules; musl the
  same, 120 modules.
- Same cases: `same-cases.sh d27cd70 1 2 3 6151` (scratch `x2/run4/same-cases.log`, Mac load 17.7 to 27.5):
  layer 1 1215 forms and layer 2 64 programs the same for each of the four seeds.
- `tests/programs.rs` before the commit (`x2/run4/programs.log`, load 20 to 29): 15 passed, 2 ignored.
- Suite at 7bcb0d6 (`WC_REFERENCE_X86_64=tom@192.168.2.240`, scratch `x2/run4/suite.log`): 76 passed, 0
  failed, 11 ignored, 24 minutes 12 seconds, Mac load 18.3 at start, 38 to 48 part way, 23.1 at end (run 3:
  76, 0, 11 in 20 minutes 50 seconds at 14.4 to 21.1; X1 run 6: 25 minutes at 13.4 to 22.0). The x86
  reference's two tests passed on the i7-8750H (its load 2.9 just after; another track's builds run there).
- Clippy with `-D warnings` on the test crate (all targets) and `cargo fmt --check`: clean. The reference
  runner and the browser crate did not change.
- Read of the harness for ARM64 facts outside the per-CPU part: none left in `src/` beyond `src/guest/`,
  `layout.rs`'s per-CPU code areas (shared with the runner by plan) and layer 2's model, whose operations
  are ARM64's by plan 7.2 "As built". In `tests/`, ARM64 appears only in the `_arm64` wrappers, the
  ARM64-own tests above and the host counters of `differences_match_linux_on_every_host_arm64`. (Wrong:
  `Binary::in_code` still kept a start only at a multiple of 4; the second X2 check found it, run 6 fixed it.)
## X2, run 3 (2026-11-17)

- origin/phase-1-translator: fetched at the start, nothing new since 8b5e420.
- The Mac's disk filled (116 MB free) part way through, which stopped every command and Docker Desktop. To
  go on I removed the cargo target folders of the finished X1 checks in this track's scratch
  (`check-x1*/target*`, `suite-target`, `head-target`, about 19 GB; their logs and lists are kept) and
  started Docker Desktop again (`open -a Docker`); the disk then had 17 to 62 GB free.
- Layer 2: the model stays in `src/layer2.rs`, drawing registers from `G::MODEL` (`Model`: the 26 registers
  in today's order, those a system call sets, the one every function keeps); the dynasm emitter moved
  unchanged to `src/guest/arm64/assemble.rs` behind `GuestTest::assemble`; `tests/layer2.rs` and the layer 2
  part of `tests/cases.rs` are over `G`. Layer 2's "every word is defined" check walks the program by
  `G::decode` and asks `G::excluded`.
- The corpora: `GuestTest::smoke() -> Smoke`, `checks() -> Vec<Check>`, `changes() -> Vec<Change>`;
  ARM64's programs moved unchanged to `src/guest/arm64/corpora.rs`. `tests/smoke.rs` keeps its six tests
  (each `_arm64`); `tests/checks.rs` is now `checks_match_linux_arm64` (all 49 programs of the 11 tests
  before, each on its own thread) and `failing_at_an_entry_in_chrome_arm64`; `tests/changes.rs`'
  hand-made tests are `hand_made_changes_match_none_arm64` (all 8 programs of the 7 tests before).
- `GuestTest::AUXILIARY`: a flat blob's own auxiliary entries (ARM64 AT_HWCAP 0x183, AT_HWCAP2 0, as
  before, so the same stack bytes); the blob record now carries them to the runner. `layout.rs` lost
  `BLOB_HWCAP`.
- Real binaries per guest: `binaries::sources::<G>()` (musl's path from `G::MUSL_LOADER`), files under
  `target/binaries/<NAME>` fetched for the guest's Docker platform, and `Binary::read` checks
  `G::ELF_MACHINE`. M5's nine files moved to `target/binaries/arm64`; `same-output.sh` links an older
  commit's copy to that folder.
- `long-runs.sh`: `WC_LONG_GUEST` (default arm64) picks the tests by their suffix; each guest has its own
  ledger and logs under `<WC_LONG_DIR>/<guest>/`; its replay lines name today's tests (one named
  `every_covered_form_matches_the_cpu`, gone since run 2); stopped containers are `wc-x86-`.
- Same cases after the layer 2 move: `same-cases.sh d27cd70 1 2 3 4127` (scratch `x2/run3/same-cases-1.log`,
  Mac load 14.1 to 17.7): layer 1 1215 forms and layer 2 64 programs the same for each of the four seeds.
- Tests run (Mac load 20 to 35, scratch `x2/run3/corpora.log`): layer 2 at the defaults (64 programs, 8 on
  the CPU, 1 in Chrome, 0 failing), smoke (6), checks (2, 49 programs), changes (3; the random part made
  the same 1031 changes as before, so the same programs). An earlier layer 2 run failed its 8 native cases
  only because Docker was down (`x2/run3/layer2.log`). After the auxiliary change: `programs` (15), `browser`
  (1), `reference_x86_64` (2, the new runner uploaded to the i7-8750H and the 38 cases and 2 blobs
  passing), `binaries` (2) and `smoke` (6) pass (`x2/run3/aux.log`, `binaries.log`).
- Suite at 3e4420d (scratch `x2/run3/suite.log`, Mac load 14.4 at start, 21.1 at end): 76 passed, 0 failed,
  11 ignored, 20 minutes 50 seconds (X1 run 6: 25 minutes at load 13.4 to 22.0; run 2: 22 minutes). 16
  fewer than run 2's 92 because checks' 12 tests are now 2 and changes' 9 are now 3, over the same
  programs. Clippy with `-D warnings` on the test crate (all targets) and the runner for both CPUs, `cargo
  fmt`: clean. origin/phase-1-translator fetched at the end: nothing new since 8b5e420.

## X2, run 2 (2026-11-17)

- origin/phase-1-translator: fetched at the start, nothing new since 8b5e420.
- `src/guest/mod.rs`: `GuestTest` and the generic `Encoded`, `State`, `Case`, `Signal`, `Stop`, `Outcome`,
  `Designed`, `StandIn`; `src/guest/arm64.rs`: ARM64's part, with layer 1's forms, value strategies, exclusions,
  the reference's stops, what a faulting access leaves open and the FPSR rule, all moved out of
  `src/layer1.rs` unchanged. The `Cases`/`ArmLanded` adapter and `AARCH64` are gone.
- Generic over `G`: `Machine<G>` and its host (thread state, stand-in numbers, slots by `G::ALIGN`, one
  thread module and probe per guest kept by type, since a static in a generic function is shared), layer 1
  (the filler written when not 0), programs (C and Rust targets, general-register flags), changes, mixed (random
  starts from instruction boundaries found by `G::decode`, which for ARM64 draw the same), layer 3 (its gcc
  image per guest platform), browser.
- The browser crate: the guest by a feature (`arm64`), built per guest under `target/browser/<name>`; the
  contract gives `pcOffset` and `flagsWordOffset`; the page takes the stand-in's numbers from its settings
  and steps slots by `align`.
- `relocation(kind)` (taking `object`'s relocation type, not a `u32`) and the code range of a real binary by
  `G::ALIGN` in `src/binaries.rs`.
- Tests written once over `G` with an `_arm64` wrapper each: `layer1.rs`, `cases.rs` (layer 1 part),
  `browser.rs`, `float_hook.rs`, `lookaheads.rs`, `layer3.rs`.
- The page starts a blob from `initial_block(pc, sp)`, a new export of the browser translator that gives
  `G::initial_block`'s bytes (it wrote ARM64's pc and sp by their offsets). Found by the suite's first run
  this run: with the offsets gone from the contract, the page started blobs with sp 0, and
  `failing_at_an_entry_in_chrome` caught it (no "left to an address"). Also fixed: the page's stand-in
  numbers were shadowed by its `standIn` function.
- Tests run before the commits (Mac, load 14.8 to 17.4): `tests/layer1.rs` (1215 forms at 64 cases, 0
  failing; 105 hand-made cases; 64 no-form cases; 1215 Chrome cases), `smoke`, `changes`, `lookaheads`: all
  pass (scratch `x2/run2/layer1.log`); after the page fix, `checks` (12) and `browser` (1) pass.
- Suite at ade0f6a (scratch `x2/run2/suite2.log`, Mac load 16.9 at start, 20.5 at end): 92 passed, 0
  failed, 11 ignored, 22 minutes. Run 1 had 95: ARM64's four hand-made layer 1 tests are now one, over the
  same 105 cases. Clippy with `-D warnings` on the test crate (all targets) and the browser crate for wasm32,
  `cargo fmt`: clean. origin/phase-1-translator fetched at the end: nothing new.
- Same cases: `same-cases.sh d27cd70 1 2 3 5309` (scratch `x2/run2/same-cases-1.log`, Mac load 15.7 to 19.5):
  layer 1 1215 forms and layer 2 64 programs the same for each of the four seeds.

## X2, run 1 (2026-11-17)

The X1 check's four findings, all fixed first:
- The dependency guard now runs `cargo tree --target all -e normal,build,dev`. Shown to fail with the check's
  example (a wasm32-only `cpu = { package = "wcontainers-arm64" }` in the translator) and with the same alias
  as a dev-dependency, then put back. Plan 3.1 and the X1 pass bar say so.
- `tests/output.rs` translates the benchmark set and `musl` for each of ARM64's four probe answers (h0 to h3
  in each line's name), the other binaries for this machine's answer. Compared with 2274314 (before the move)
  by `same-output.sh 2274314 bench,musl`: bench the same, 505 modules; musl the same, 120 modules (Mac, load
  10.7 to 11.3). The benchmark set gives the same bytes for all four answers (it has no float operations the
  answer changes); musl gives different bytes for each, so the four meanings are covered.
- `HookBody::CodeChanged { line }`: the hook drops the pieces of the `line` bytes holding its address; ARM64
  64. Both runners step over the line by `ALIGN`. The plan's RISC-V `fence.i` gets a `HookBody` variant of its
  own when the RV64GC crate is built (plan 3.2, 3.5).
- `HookType::MAX_PARAMS` (4) and `MAX_RESULTS` (2) in the interface, checked at compile time by both runners
  (the browser crate now has the check too). Eight parameters were tried first: the float hook in Chrome got
  about 15 percent slower (old 0.98 and 1.64 s, new 1.15 and 1.88 s at none and full, best of 3, two rounds,
  Mac load 15.9 to 16.8; scratch `x2/chrome-hook-8args.log`), so the limit is 4, as the browser passed before.
  With 4, old 0.938 and 1.611 s, new 0.965 and 1.666 s (best over 3 alternating rounds, load 14 to 34,
  `x2/chrome-hook.log`): the same code as before, within noise.

The shared harness so far:
- `tests/cases.rs` writes what layer 1 and layer 2 draw for a seed (a hash a form, a line a program), and
  `same-cases.sh <commit> <seed>...` runs each commit's own copy and compares. Committed (d27cd70) before any
  harness change, so it is the "before". Seeds 1, 2 and 3 at 64 cases a form and 64 programs: 1215 forms and
  64 programs each, the same (at d27cd70 against itself, which shows the command). At the end of the run, with
  the reference moved: seeds 1, 2, 3 and 9071 the same, 1215 forms and 64 programs each.
- The reference runner is one program with a per-CPU `Machine` (`reference/src/aarch64.rs`,
  `reference/src/x86_64.rs`): the wire of plan 7.2 (hello with field names and CPU model, case and blob
  records with statuses), the seccomp filter with the arch check, cases in a long-lived child replaced when a
  case ends it, each blob in a child with its stop record (stored into a shared page), its output through
  pipes and no descriptor but 0, 1 and 2 (the first suite run found both: `buffers` writes a faulting buffer,
  which a file cuts short and a pipe refuses, and writes to fd 7), timed runs. x86: the trap flag in the frame, `int3`
  filler over 32 KiB either side, NT and ID through the handler's own rflags, x87 rotated by TOP, XSAVE
  components with `xstate_bv`, FS and GS by raw `arch_prctl`, rep strings and gathers stepped (iced decides
  which, before the case runs).
- `src/reference.rs`: `Reference::start(&Target)` runs the runner in Docker here or over SSH (ControlMaster,
  upload to `~/wcontainers-x86/reference/<hash>` when missing, five tries), `case` and `blob`. ARM64's layer 1
  (`Cases`) and every native blob run (`programs::native`, `native_seconds`) now use it: one container per
  runner, shared through a pool, not one container per blob.
- `tests/reference_x86_64.rs`: 38 hand-made cases, each checked by hand against Intel's manual, and two blobs,
  all passing on the i7-8750H (x86 load 0.0 to 0.2). Among them every stop of plan 7.3 (TRAP_TRACE, `mov ss`
  held back to the filler at CODE+3, `int3`, `int 3`, `int1` as TRAP_BRKPT, `syscall`, `int 0x80` and
  `sysenter` as SIGSYS, `pushf` with NT and ID, `ret` to a non-canonical address as general protection with
  rip and rsp unchanged, `mov rax, [rbx]` general protection, `mov rax, [rbp]` and `push rax` stack faults as
  SIGBUS SI_KERNEL, page faults, divide error, `ud2`), x87 written and read back through TOP, ymm upper halves,
  FS and GS bases, `rep stosb` stepped to its end, a `call far` through selector 0x33 (pushes 4-byte CS and
  eip), and a `jmp` beyond the code area. Found: `sysenter` with rbp 0 gives SIGSEGV, not SIGSYS (its 32-bit
  entry reads the caller's stack at rbp first); the case sets rbp, as X0's experiment did.

## X1, run 6 (2026-11-17)

The fifth check of X1 found one blocking problem and four smaller ones. What was done about each:
- Blocking, run 5's "no more cycles than before the move" at translate time does not hold. Measured again
  (scratch `x1/run6/count.sh`: python and busybox at full, bounded 256 and bounded 4096, one thread,
  `/usr/bin/time -l`, order flipped each round; Mac, load 12.3 to 21.2):
  - HEAD against old (2274314), 2 rounds (`count-a.log`): cycles 118.80 G against 122.06 G, then 125.53 G
    against 122.87 G (ratios 0.973 and 1.022).
  - HEAD with one change tried (a function's instruction lengths kept as `u8`, the walk back over
    instructions stepping by `ALIGN`), 8 rounds against old (`count-a8.log`): ratio of cycles a pair 0.992,
    1.015, 1.019, 1.026, 1.035, 1.041, 1.042 and 1.000; best old 121.37 G, best new 125.60 G (1.035);
    instructions old 471.3 to 475.8 G, new 458.9 to 462.1 G. No better than HEAD, so it was dropped.
  - With the check's 7 pairs (1.001 to 1.050): the move costs ARM64 about 0 to 5 percent more cycles a pair at
    translate time, about 2 to 3 percent on average, while it retires 2.4 to 3.5 percent fewer instructions.
  - Where it goes, by `sample` of one run of each (`old.sample`, `new.sample`, `kids.py`): the walk that finds
    functions (`Finder::explore`) has about 13 percent more samples of its own, the rest is spread thin; its
    calls to decode cost less than before. Not found further. The output is the same bytes, so the guest's
    run time is untouched. Plan 3.1 now gives the measured range; run 5's sentence above was wrong.
- The licence entry for `wasmparser` now says it is a dev-dependency of the test crate.
- The guard: a new test, `the_translator_depends_on_no_guest_cpu_crate`, runs `cargo tree --offline
  --all-features -p wcontainers-translator -e normal,build` and fails if any `wcontainers-*` crate but the abi
  and the interface is in it. Shown to fail with the check's example (root `cpu = { package =
  "wcontainers-arm64", ... }` under `[workspace.dependencies]`, `cpu.workspace = true` in the translator),
  then put back. Plan 3.1 and the X1 pass bar say so.
- RISC-V's `auipc ra, hi; jalr ra, lo(ra)`: the plan claimed something the interface does not do. Now the
  RV64GC crate decodes the pair as one 8-byte instruction whose flow is a call with its target, which the
  interface already carries, so the translator treats it as any direct call (MAX_LENGTH 8 for RV64GC; a jump
  to the `jalr` alone decodes it alone, by the overlap rule). `built_address` no longer mentions `jalr`.
  Plan 3.4 (Control flow, Addresses the code builds) and 3.5.
- "Cases actually run" now lists X1's seeded random tests and their counts.

Proof after run 6 (machine: the Mac, Apple M5): the translator's source, the ARM64 crate and the runners are
unchanged since run 5 (this run changed a test, a doc comment and the documents), so run 5's byte-for-byte
comparison stands and was not repeated. origin/phase-1-translator: fetched at the start and end of the run,
nothing new (its last commit, 8b5e420 at 11:05 the same day, is merged).
- Suite: `cargo test -p wcontainers-guest-cpu -p wcontainers-arm64 -p wcontainers-translator -p
  wcontainers-translator-test` at dd1b4e9 (the sources of this run's end; `x1/run6/suite.log`): 93 passed (the
  new guard test among them), 0 failed, 10 ignored (the measuring ones), Chrome and Docker included, 25
  minutes (load 13.4 at start, 22.0 at end).
- Clippy with `-D warnings` on the translator and the interface crate (all targets), `cargo fmt --check` on
  the four crates: clean.

## X1, run 5 (2026-11-17)

The fourth check of X1 found one blocking problem and one smaller one. Both were measured first, by
instructions retired (`/usr/bin/time -l`), which barely move between runs, with times beside them.
- Blocking, the float hook program slower in wasmtime. Scratch `x1/run5/float-count.sh` runs the float hook
  test binary of the old tree (2274314) and of the checkout, `WC_FLOAT_RUNS=5`, alternating. Old 313.0 to
  313.4 G instructions, HEAD 321.1 to 321.5 G. With `WC_FLOAT_STEPS=1` both are 192.9 G, so all 8 G is in the
  hook calls (about 6 percent of their instructions). Variants built only to measure, then dropped:
  - the old hook body put back in the new runner: 313 G, so nothing outside the hook path costs more;
  - typed `func_wrap` with `run_hook` and the 36-byte copies: 319 G;
  - no copies, the bytes handed in place: HEAD's path 315.8 G and typed 313.1 G;
  - in place with the arguments read with the types written out: 313.9 G.
  So the copies of the state bytes cost 6 G and the per-call loop over the hook's types about 2 G.
  Fixed so: the host runner hands `run_hook` the hook's bytes in place, as a slice of the state memory (no
  copy and no `Arc` clone). Each hook's closure is `cpu_hook::<I>` for its index, made at compile time from
  `REGISTER_CPU_HOOK`, so the hook's types and range are constants there. Result 312.6 to 313.7 G against
  old 313.0 to 313.1 G (`float-count-final.log`). Times, old against new alternating, order flipped each
  round, best of 3 a run, 8 rounds (`float-alt2.log`, load 21.7 to 28.6):
  - none: best old 0.6719 s, new 0.6597 s;
  - full: best old 0.9330 s, new 0.9221 s;
  - new was faster in 4 of the 8 pairs at each look-ahead.
  (With the copies gone but the closure not yet per index, 10 rounds of one run each, `float-alt1.log`, were
  too noisy at that load to read.) The browser's path copies the bytes, as it must. It was already equal in
  Chrome (run 3) and is unchanged.
- Translate time: the rest was found. Run 3 made the flag-state sets `u128`, and two loops
  (`write_back_flags`, `push_flag`) tested all 128 bits where they had tested 32. Now `state_ids` walks only
  the set bits. Scratch `x1/run5/count.sh`, python and busybox at full, bounded 256 and bounded 4096, one
  thread, 3 rounds of old, HEAD before this and new (load 17.4 to 26.5):
  - instructions: old 471.7 to 472.5 G, HEAD 468.9 to 470.1 G, new 459.6 to 460.2 G;
  - best cycles: old 128.68 G, HEAD 132.84 G (1.032), new 128.34 G (0.997).
  So the move now costs ARM64 nothing measurable at translate time, and about 2.6 percent fewer
  instructions. The plan (3.1, 3.4 Hooks) says so. (Wrong: the best of 3 rounds hid the spread; run 6
  measured 0 to 5 percent more cycles a pair.)

Proof after run 5 (machine: the Mac, Apple M5; loads as given):
- Byte for byte against 2274314 with `same-output.sh` at 34bfa5c (`x1/run5/same-small.log`, `same-big.log`):
  bench 127 modules, busybox 34, musl 30, python 34, rg 34, git 34, java 40, go 28, all the same (load 23.1 at
  the end); node 62 modules (alone, 13:46 to 13:51, load 20.7 to 46.5) and bun 74 (alone, 13:51 to
  13:57, load 46.5 to 28.2), the same.
- Suite: `cargo test -p wcontainers-guest-cpu -p wcontainers-arm64 -p wcontainers-translator -p
  wcontainers-translator-test` at 34bfa5c (`x1/run5/suite.log`): 92 passed, 0 failed, 10 ignored (the
  measuring ones), 29 result lines, Chrome and Docker included, 25 minutes (load 26.2 at start, 23.0 at end).
- Clippy with `-D warnings` on the four crates (tests and bins) and on the browser crate for wasm32, and
  `cargo fmt --check`: clean.
- Benchmark run times: not timed again. The modules are the same bytes, and the runner's change touches only
  a hook of the guest's own, which the benchmarks never call (they run in the default rounding mode), so run
  2's figures stand.
- origin/phase-1-translator: fetched at the end of the run, nothing new (its last commit, 11:05 the same
  day, is already merged).

## X1, run 4 (2026-11-17)

The third check of X1 found one blocking problem and five smaller ones. What was done about each:
- Blocking, two overlapping streams that meet again panicked (`at[&t]`, "no entry found for key", in
  `Finder::explore`): an offset that two instructions fall into was joined to one of their blocks, so the
  other's fall-through had no block to go to. Now such an offset leads a block of its own; the cut keeps
  the furthest end of the earlier instructions, so the look back over `MAX_LENGTH` offsets runs only where
  instructions overlap, never for ARM64. `before` also answers `None` there (two instructions fall in), as
  at a jump target. Two test guest cases: a jump into `inc r0`'s second byte that ends where the `inc` ends,
  and a jump past an instruction's first byte (x86's `je 1f; lock; 1: cmpxchg`). Without the fix both fail
  with the check's panic; with it the test guest passes 16 of 16. Plan 3.4 says so.
- Translate time: measured, not only expected. python and busybox at full, bounded 256 and bounded 4096, one
  thread, old (2274314) and new alternating (scratch `x1/run4/translate-time.sh`, `count.sh`; Mac):
  - before this run's change, 6 rounds (load 26.6 to 57.2): best old 28.18 s user, new 28.97 s (1.03);
    new slower in all 6.
  - `/usr/bin/time -l` counts: instructions retired the same within their run-to-run spread (old 470.2 to
    475.8 G, new 468.4 to 476.0 G), but more cycles. ARM64's small per-instruction queries (`decode`, `flow`,
    `accesses`, `reg_bit` and so on, 16 of them) were not inlined across the crate boundary (no LTO in
    release). Marked `#[inline]`, 3 rounds of old, without and with (load 19.9 to 25.4): best cycles old
    115.15 G, without 121.44 G (1.055), with 117.76 G (1.023); with them 1.008 to 1.028 a round.
  - 6 rounds of wall time with them (load 21.1 to 33.1): best old 26.63 s user, new 27.83 s; too noisy at
    this load to say more than the cycle counts do.
  So the move costs ARM64 about 1 to 3 percent of translate time, as cycles; the output is the same bytes.
  The plan (3.1) now says this instead of "nothing beyond what ARM64 pays today". Not found: where the rest
  comes from (the cycle counts move by about 2 percent between runs of one binary).
- The guard missed `use wcontainers_abi as abi;`: in Rust source every `wcontainers_abi` not followed by
  `::PAGE_SIZE` or `::Permissions` is now named, whatever follows (`as`, `{`, `;`); the manifest is checked
  for crate names only. Tested with the alias and with `extern crate wcontainers_abi as abi;`.
- `ReturnAddress::Register` no longer carries a register bit nobody read (plan 3.2 and 3.5: the meaning
  writes the link register; RV64GC's `jalr` may link to any register).
- The test guest's programs now run at none, bounded 1, 5, 17, 64, 256, 4096 and full (the comparison's
  list, and 5), plan section 9 says so.
- The check's confirmation of run 3's proof: nothing to do.

Proof after run 4 (machine: the Mac, Apple M5; loads as given):
- Merged origin/phase-1-translator (8b5e420, which changed only docs/status-translator.md) at 0ef82c6;
  fetched again at the end of the run: nothing new.
- Byte for byte against 2274314 with `same-output.sh`, at 0ef82c6 (the overlap fix; `x1/run4/same-all.log`):
  bench 127 modules, busybox 34, musl 30, python 34, rg 34, git 34, java 40, go 28 (2 minutes, load 22.8 to
  18.4), node 62 (alone, 4 minutes), bun 74 (alone, 6 minutes, load 36.4 to 26.4): all the same.
  Again at bc66759 (with the inline hints; `x1/run4/same-final.log`): every set the same, the same module
  counts (small sets 2 minutes, load 28.0 to 58.4; node 4 minutes; bun 6 minutes, load 27.3 to 39.3).
- Suite: `cargo test -p wcontainers-guest-cpu -p wcontainers-arm64 -p wcontainers-translator -p
  wcontainers-translator-test` at bc66759 (`x1/run4/suite.log`): 92 passed, 0 failed, 10 ignored (the
  measuring ones), 29 result lines, Chrome and Docker included, 25 minutes (load 39.3 at start, 43.5 at end).
- Clippy with `-D warnings` on the four crates (tests and bins) and on the interface and ARM64 crates
  without `translate`, and `cargo fmt --check`: clean.
- Benchmark run times: not timed again; the modules are the same bytes and the host runner is unchanged
  since run 2's timing, so run 2's figures stand.

## X1, run 3 (2026-11-17)

The second check of X1 found three blocking problems and five smaller ones. What was done about each:
- Blocking, a CPU without flags made invalid modules: the "flags in locals" flags-word function was made
  with an empty body for empty `FLAGS`. Now `FLAGS_WORDS` is 0 and no flags-word function, flag state local
  or flags-word type is made when `FLAGS` is empty, and a return's compare skips the mask when
  `FLAGS_WANTED` is 0. The test guest has a variant without flags (`Test<false>`, conditional jumps on a
  register being zero, `FLAGS_WANTED` 0, `UNREACHABLE_RETURN` 1, like RV64GC); its translations validate
  and run.
- Blocking, grouping by segment unreachable: `MemoryAccess::plain` no longer rejects a segment, so
  `fs:[r + d]` accesses group under (segment, base, write), and `fs:[d]` with no base or index under
  (segment, none, write). The loop proofs still leave segment accesses out (they bound a register's values,
  not a segment base's), and kept runs of pages are keyed by segment too. Shown by the test guest: two pairs
  of segment loads have no exact check at full look-ahead (4 at none), and a pair whose second load reaches
  an unreadable page faults exactly, with the first load done.
- Blocking, the status file's departure about `before` walks contradicted the source: the sentence is gone
  (the walks were built in run 2).
- The test guest now runs: it moved to the test crate (`tests/test_guest.rs`), where a small runner of its
  own (one thread, the program's pieces placed first, later code at look-ahead none, the exact piece at
  bounded 1, every hook call recorded) runs eleven programs in wasmtime at none, bounded 1, 5, 256 and full,
  for speed and size, with and without flags, and checks registers, pc, memory and hook calls: a trap leaving
  pc after it; a call pushing its return address, a red-zone store and load, a register passed out only, the
  flags in the block after a return; a flag kept by `inc`; a jump into the middle of an instruction; the
  segment groups and their exact faults; a hook writing a register back and reading one; a raised stop seen
  before the next instruction; a system call's registers and the CPU's own slow path; a run-time range;
  a switch to a case and to an address not in its table; two accesses of one instruction and the second's
  exact fault. With the two blocking fixes taken out, 11 of its 14 tests fail (the flagless modules invalid;
  4 exact checks where 0 are expected).
- Flag states are bits of a `u128` (was `u32`), so a CPU may have 126 shapes, checked at compile time.
- The browser runner's `Cpu` hooks give every result (the contract lists each hook's result types; i32 as a
  number, two results as an array), copy the hook's bytes with two `set`s between views made once per hook
  (the one at `cpu_state` again after the cdylib's memory grows), and allocate only the BigInts of i32
  arguments. Timed in Chrome with the new `float_hook_time_in_chrome` (`tests/float_hook.rs`, same program
  and knobs as the wasmtime one), old (2274314's copy) and new alternating, best of 3 a run, 4 rounds each
  (scratch `x1/run3/chrome-alternate.sh`, Mac):
  - with run 2's path (a fresh array, four BigInts and three views a call; 36 bytes copied by views made
    each call), load 38 to 46: old none 0.90 to 1.0 s, full 1.6 to 1.7 s; new none 1.3 to 1.4 s, full
    2.0 s. New over old about 1.44 (none) and 1.25 (full). (This run's first printing divided seconds by
    1000; the log `chrome-alternate.log` shows 0.0009 and so on, read as 0.9 s.)
  - with views made once and `set`, load 19.7 to 35.8: best old none 0.9184 s, full 1.6141 s; best new none
    0.9099 s, full 1.6082 s (`chrome-alternate2.log`). New over old 0.99 and 1.00.
- The bottom loop's misaligned-target fault: not an interface item; the plan now says why (3.4, and
  Departures).
- `dyn` in the translator: the plan's sentence now says nothing of a guest is reached through `dyn`, and
  names the loop-proof module's own `&dyn Fn` closures (translate time only).
- Also: a compile-time check that a CPU tracks at most 8 flags (the flag locals' fixed array).

Proof after run 3 (machine: the Mac, Apple M5; loads as given):
- Byte for byte against 2274314 with `same-output.sh` (every look-ahead, each kind of start alone), at the
  tree of e0adef3 (`x1/run3/same-final.log`): bench 127 modules, busybox 34, musl 30, python 34, rg 34, git
  34, java 40, go 28 (2 minutes, load 22.7 to 27.9), node 62 (alone, 4 minutes), bun 74 (alone, 7 minutes,
  load 24.6 to 25.6): all the same. Also bench, busybox, musl and python after the first two fixes alone
  (`same1.log`): the same.
- Suite: `cargo test -p wcontainers-guest-cpu -p wcontainers-arm64 -p wcontainers-translator -p
  wcontainers-translator-test` at the tree of e0adef3 (`x1/run3/suite.log`): 90 passed, 0 failed, 10 ignored
  (the measuring ones), 29 result lines, Chrome and Docker included, 26.5 minutes (load 24.7 at start,
  56.7 during, 32.4 at end). Clippy with `-D warnings` on the four crates (tests and bins) and on the browser
  crate for wasm32, and `cargo fmt --check`: clean.
- Benchmark run times: not timed again. The modules are the same bytes as before the move and the host
  runner (`src/`) and the ARM64 crate are unchanged since run 2's timing, so run 2's figures stand.
- The float hook's instructions and times, old against new: build the float hook test binary in both trees
  (`cargo test --release --no-run -p wcontainers-translator-test --test float_hook`), copy each into scratch
  `x1/run5/`, then `SIDES="old=<old binary> new=<new binary>" nix develop -c <scratch>/x1/run5/float-count.sh
  <rounds>` (instructions retired, cycles, best times; `WC_FLOAT_RUNS`, `WC_FLOAT_STEPS` as for the test) or
  `.../float-alt.sh <rounds>` (times, order flipped each round). The side named `old` runs in the old tree.
- The float hook in Chrome: old and new equal (above); in wasmtime it is run 2's figure (the host path is
  unchanged).
- origin/phase-1-translator had nothing new to merge (fetched at the end of the run; last commit there
  05:34 the same day, already merged).

## X1, run 2 (2026-11-17)

The check's findings and what was done about each:
- Blocking, interface built in part: every item of plan section 3 now exists (`ID`, `MAX_LENGTH`,
  `bit_reg`, `initial_block` in `Personality`, `probe_module`, `host_from_probe`, `host_key`, `offer_key`,
  a CPU's `Reason`/`REASONS` from 64, `TrapKind` 4 to 7, `check`'s `which`, `check_range`, `warn`,
  `raise_stop`, `RETURN_ADDRESS`, `RED_ZONE`, `CODE_CHANGES`, `STEPS`/`step`/`StepAt`/`Step`,
  `HookType::registers`, `Index::BitOffset`/`Lanes`, `AccessBytes::Counted`/`Masked`, `Class::Segment`,
  the `before` walks), and the translator does its side of each: `PASSED_IN` and `PASSED_OUT` may differ
  (the compile-time equality check is gone), check plans per (offset, access), groups keyed by segment
  too, red-zone frames, a `Stack` return address moved past in `ret` and expected back after `call`
  (emitter and frame analysis), hook registers written back and reloaded, `check_range`, `warn` with
  sites by number, `raise_stop` with a stop check before the next instruction, and the walk back for
  `built_address` and `jump_table`. The runners count a CPU's reasons by number, probe through
  `Guest::probe_module`, start blob threads from `initial_block`, and stop with x86's faults as
  `Stopped::Fault`. `tests/test_guest.rs` in the translator is a guest made for the test, shaped like
  x86-64 where ARM64 differs (1 to 3 byte instructions, return address on the stack, red zone 128,
  passed in [r7, r6, sp] and out [r0, sp], an `inc` keeping C, an FS base, a trap with pc after it, two
  accesses in one instruction, a range check, its own slow path, a hook on registers, a raised stop, a
  jump into the middle of an instruction, a switch found by walking back); its translations at none,
  bounded 1, 5, 256 and full, both settings, both offers, and its thread module validate.
- Blocking, the float hook 1.5 times slower: `HookType::state` names the bytes a `Cpu` hook works on
  (ARM64's float hook: 36 bytes, FPSR through FPSR tracking); the host runner registers the hook with
  `func_new_unchecked`, arguments and state in fixed arrays sized by the largest hook, the state copied
  with one `copy_nonoverlapping` each way; the browser copies only those bytes too. Timed with
  `tests/float_hook.rs` (`programs/rounding.c`, 200,000 turns a mode, three modes, output equal to the
  native run), in the 2274314 copy and the checkout alternating, one run each per round, on the Mac:
  - first fix (36-byte copy, 1 KiB buffer zeroed per call), 3 rounds, load 27.8 to 28.6: best old none
    0.6884 s, full 0.9012 s; new 0.7506 s, 0.9948 s (about 1.09 and 1.10).
  - with the buffer sized to 36 bytes and the copy by `copy_nonoverlapping`, 3 rounds old first (load 23.1
    to 23.9): old 0.7250 / 0.9355 s, new 0.7309 / 0.9577 s; 4 rounds new first (load 34.9 to 41.8): old
    0.7505 / 0.9836 s, new 0.7812 / 1.0015 s. New over old 1.01 to 1.04 (none) and 1.02 (full), against
    1.5 before the fix.
  - an experiment registering the float hook with a typed `func_wrap` instead (not kept): old 0.7065 /
    0.9610 s, new 0.7590 / 1.0030 s (load 33.9 to 34.7), no better, so the registration is not the rest.
  What is left, about 8 ns of about 200 ns per hook call, is expected to be the generic hook path (36 bytes
  copied each way where the old code moved 16, and the arguments read by type); not measured further.
- Blocking, the browser did not ask `memory_fault`: the cdylib exports `cpu_memory_fault(address,
  length)`; runner.js copies the state and the instruction's bytes to `cpu_state` and stops with the
  answer (page fault, general protection or stack fault).
- `guest_free.rs` now allows only `wcontainers_abi::PAGE_SIZE` and `wcontainers_abi::Permissions`, by full
  path; a second test shows the check catches `RegisterBlock`, a `use` group and a bare `use`.
- `int 4` gets `TrapKind::Overflow` (7), so the kernel tells it from general protection (trap number 4,
  no RF); plan 3.2, 5.2, 6.
- The plan's contradictions fixed (header, 3.2's code block with `Personality`, the kernel's wording,
  `Class` and `Ty`), and this file's "X0, run 1" heading restored.
- `flags_needed_after` finds its place by binary search (a block's offsets are in order).
- The comparison command covers every look-ahead (none, bounded 1, 17, 64, 256, 4096, full), and for a
  binary each kind of start alone at full and bounded; both sides run the current `tests/output.rs` (for
  a commit before the interface its calls are spelled the old way by sed in the script).
- The move in one commit: history is not rewritten; run 2's work is in separate commits.
- zerocopy's licence recorded below.
- `TrapPc::After` and the `no_std` cores: the test guest's break leaves pc after it; `tests/no_std.rs` in the
  test crate builds both cores for `wasm32v1-none` with `translate` off.

Proof after run 2 (machine: the Mac, Apple M5; loads as given):
- Byte for byte against 2274314 (before the move), with the widened `same-output.sh` (every look-ahead,
  and each kind of start alone at full and bounded), at the tree of 46143f4: bench 127 modules, busybox 34,
  musl 30, python 34, rg 34, git 34, java 40, go 28, node 62 (alone, 8 minutes), bun 74 (alone, 13
  minutes): all the same (load 39 to 79). The later commits change only the runner and documents.
- The intentional ARM64 output changes stay at X3 and X6 (see Departures), so there is one comparison.
- Suite at 46143f4's tree (before the last runner changes): 74 passed, 0 failed, 8 ignored, 26 test
  binaries, Chrome and Docker included. After them: smoke, programs, checks, no_std, the translator's
  tests and the browser lint passed. The whole suite again at c6949d5 (the final code): 78 passed, 0
  failed, 9 ignored (the measuring ones), 29 test binaries (load 31 at start, 44 at end).
- Clippy with `-D warnings` on the four crates (tests and bins) and the browser crate for wasm32: clean.
- Benchmark run times, wasmtime, every check, old (2274314's copy) and new alternating, best of 5 (load
  35 to 57): fib 2.504 / 2.402 s, sieve 0.670 / 0.686, hash 0.584 / 0.584, sort 1.246 / 1.269, interpret
  0.661 / 0.650, matrix 0.294 / 0.285, walk 2.305 / 2.340, search 0.257 / 0.242 (old / new); new over old
  0.94 to 1.02, the modules being the same bytes.
- origin/phase-1-translator had nothing new at the end of run 2 (fetched; already merged at 9626b63).
- Not done: Chrome timings (as in run 1); the float hook's last 1 to 4 percent (above).

## X1, run 1 (2026-11-17)

Done:
- Merged origin/phase-1-translator (22 commits) first.
- `crates/wcontainers-translator-test/same-output.sh <commit>` with `tests/output.rs`: one command that
  builds the earlier commit from its own copy (scratch), writes every module's length and BLAKE3 hash on
  both sides (benchmark set: whole program and the 8 workloads; the 9 M5 binaries; full and bounded
  look-ahead 256; speed and size; the thread module) and compares the lists with `cmp`. Committed before
  any interface change (2274314), which is the "before" of every comparison below.
- The X0 review's open findings settled in the plan: `TrapPc` (a trap may leave pc after the instruction:
  x86 `int3`, `int 3`, `int1`, `int 4`); the `no_std` core and `translate` feature layering of the
  interface and of each CPU crate (plan 3.1, 6); rdpmc kernel only and VMX designed SIGILL and excluded from
  layer 1 (5.2, 7.4); `int 4` a trap with rip after (5.2, 6); segment loads write the FS/GS base (5.5,
  3.4); `sgdt`, `sidt`, `lsl 0x7b` designed answers and excluded (5.10, 7.4); the recorder runs with the
  translation's host and offer (3.1, built so); X1's dependency check (valid.rs moved, 9).
- `crates/wcontainers-guest-cpu`: core (`Personality`, `Registers`, `Bit`, `TrapKind`, `MemoryFault`)
  `no_std`; `Guest`, `Emitter`, the plain types, `Reason` and the generic effects recorder behind the
  `translate` feature (default on).
- `wcontainers-arm64`: `Arm64` with its `Personality` in a `no_std` core; `Guest` in `src/guest.rs`
  (shapes, completing, stack moves, switch tables, built addresses, loop-proof facts, `run_hook` for the
  float hook); meanings bound on `Emitter<Arm64>`; FPSR tracking and exclusive-monitor offsets moved here
  from the contract.
- `wcontainers-translator`: generic over `G: Guest`, depends on the interface only; blocks keep their
  instruction offsets (variable lengths), lookup and switch tables by `ALIGN`, hooks and type indices after
  the guest's hooks, trap pc from `TrapPc`. `tests/guest_free.rs` fails if its manifest or sources name a
  CPU crate.
- Test crate and browser cdylib name `Arm64`. Host runner: guest hooks registered by `HookBody`, the float
  hook through `Arm64::run_hook`; the memory fault hook asks `Arm64::memory_fault`. Browser: `cpu_state`,
  `cpu_hook`, `cpu_hook_result`, `cpu_hook_reason` exports, runner.js copies the state around them; the
  contract JSON lists the guest's hooks and `align`.
- `rust-toolchain.toml` gains the target `wasm32v1-none` (no standard library). Both cores build for it with
  `--no-default-features`; the control (interface with `translate`) fails on `can't find crate for std`.

Proof so far (machine: the Mac, Apple M5; load 18 to 42 while it ran):
- Byte for byte, at 7178588 (the move) against 2274314 (before it): `same-output.sh` gives "the same" for
  bench (37 modules: thread module, whole program and 8 workloads, full and bounded, speed and size),
  busybox, musl, python, rg, git, java, go (4 modules each), node (6) and bun (8); Node and Bun each alone.
  Each side of the "before" was also compared with itself first (same lists), so the output is
  deterministic. After merging origin/phase-1-translator again (9626b63, which changed only
  docs/status-translator.md) bench, busybox, musl and python were compared again: the same. The other
  binaries were not run again, as no input of the translator changed.
- No intentional change of ARM64's output in X1 (the stop check's raised branch changes at X6), so there is
  one comparison, across the whole move.
- Suite: `cargo test -p wcontainers-guest-cpu -p wcontainers-arm64 -p wcontainers-translator -p
  wcontainers-translator-test` at the tree of 7178588: 74 passed, 0 failed, 8 ignored (the measuring ones),
  22 test binaries, Chrome tests included (load 17.4 at start, 22.4 at end). Clippy with `-D warnings` on
  the four crates (tests and bins) and on the browser crate for wasm32: clean.
- `cargo tree -p wcontainers-translator`: wcontainers-abi and wcontainers-guest-cpu only.
- Run time, wasmtime, every check, best of 5 with old (2274314's copy) and new (the checkout) alternating
  one run each (scratch `x1/bench-alternate.sh 5`, outputs in `x1/bench/`; Mac load 16 to 27): fib 2.307 /
  2.170 s, sieve 0.640 / 0.642, hash 0.580 / 0.578, sort 1.107 / 1.081, interpret 0.530 / 0.535, matrix
  0.238 / 0.233, walk 2.262 / 2.240, search 0.245 / 0.232 (old / new). New over old 0.94 to 1.01; the
  modules are the same bytes, so these differences are the machine's noise.
- The move landed as one commit (7178588) after the comparison tool's own commit: the translator could not
  be made generic piece by piece and still build, so the steps were checked in the working tree (byte for
  byte on bench, busybox and musl as soon as it built) rather than committed one by one.

Left for X1 after run 1: nothing of the pass bar as it then stood. The independent check of X1 then found
the interface built only in part, a slower float hook and the browser not asking `memory_fault`; run 2
below.

## X0, run 1 (2026-11-17)

Done:
- Wrote the plan from the brief and the seam map. Appendix A maps every kind b and c place of the seam
  map's translator and tests areas (41 and 60 places) to the section that covers it.
- iced-x86: 1.22.0, MIT (crate `Cargo.toml` and `cargo info`). Builds for wasm32-unknown-unknown from a
  `#![no_std]` library with `default-features = false` and `no_std`, `decoder`, `instr_info`, `encoder`,
  `op_code_info`. Size it adds to a stripped wasm32 release cdylib: decoder only 227,228 bytes raw, 60,705
  gzip -9, 47,058 brotli -q 11; with `instr_info` 353,374 / 82,721 / 64,947. Today's browser translator
  (ARM64) for scale: 2,170,386 / 412,059 / 312,302. Measured on the Mac (Apple M5), load 15 to 28.
- Fused multiply-add: WebAssembly's relaxed madd may fuse or not; the specification's rule quoted in the
  relaxed SIMD overview makes the choice fixed per environment, so the phase-one probe can rely on it. No
  scalar form. Plan: FMA claimed only with the AVX setting, software where the host does not fuse.
- How others stop an x86 CPU after one instruction: sandsifter (trap flag, and page-end placement), RISU
  (a UD1 operation after each instruction), debuggers (trap flag). Chosen: the trap flag set in the signal
  frame the reference runner returns through.
- Experiment 1: `hello.c` built with `zig cc -target x86_64-linux-musl -static` on the Mac, copied with
  scp, run over SSH on the x86 machine; output and exit status 7 came back.
- Experiment 2: `stepper.c` runs one instruction from given registers by the trap flag and reads every
  register back. 22 cases, all as the Intel manual says (plan section 2 lists them and what they teach
  the reference runner).
- Measured: a new SSH connection about 344 ms (Mac load 57). The figure of 410 ms for 1,000 stepper
  processes recorded here had no method written down and could not be repeated; withdrawn in run 3 (see
  there).

Scratch (not in the repository), all under pyro-parked/wcontainers-scratch/x86:
- `iced-size/` size builds (base, dec, info, nostd); `dis/` an iced disassembler used to check encodings;
  `reference-proof/` hello.c, stepper.c, cases.sh, cases2.sh, cases.out, cases2.out; `sources/` the
  relaxed SIMD overview and sandsifter's injector.c and licence; `target-browser/` and `browser-build.log`
  the browser translator size build.

## X0, run 2 (2026-11-17): the independent check's findings, all fixed in the plan

- Blocking, the bottom loop's marker: a matched `ret` could reach `UNREACHABLE_RETURN & !FLAGS_WANTED`
  on x86 and RV64. Now each CPU makes it unreachable before the compare: ARM64 by its existing untag, x86
  by a canonical check in the meanings of `ret` and of every jump or call through a register or memory
  (four operations each, cost measured at X4), RV64GC by marker 1 (no `jalr` target has bit 0 set). Plan
  3.2, 3.4 (T14), 3.5, 5.3, 12.
- Blocking, interface not in full: added `Guest::Offer` and `offer_key` (the AVX setting), passed to
  `translate` and read through `Emitter::offer()`; added `Guest::run_hook`, `HookBody` and `HookDone`, so
  the software float runner (ARM64 `exact::run`, x86 float and x87) ships behind the interface and no
  runner names a CPU function; the browser exports `cpu_hook`. Added appendix B: every interface surface
  item of the translator (24) and tests (25) areas and where it is covered.
- `mov ss` holds the trap flag's stop back one instruction; the runner reads a `SI_KERNEL` stop at CODE +
  length + 1 as "landed at CODE + length" (plan 2, 7.3).
- `int 0x80` runs a 32-bit system call on the reference; this phase gives general protection, as a kernel
  without 32-bit emulation does (expected from Linux's source, not seen), counted and warned, excluded
  from layer 1 with that reason. `int1` maps to `TRAP_BRKPT` (plan 5.2, 6, 7.3, 7.4).
- The `syscall` experiment's description corrected (no `TRAP_TRACE` at all; the filler stopped it).
- Lookup array figure: bun's 52,050,800 bytes of code need about 208 MB of slots (was a wrong 38 MB).
- Write protection of translated pages now follows the permission contract: other threads stopped first
  (free with one host thread), pieces recheck after calls, and a page in a live stack frame makes the
  threads concerned give up their WebAssembly frames, counted "code translated from a live stack frame".
- Decoder only has no `encoding()` or `cpuid_features()`: a build script makes a per-`Code` table (claim
  group, VEX) from iced's own `instr_info` as a build-dependency, checked by a test.
- X1 pass bar: the builder adds `WC_BENCH_DUMP` to tests/benchmarks.rs first, so the benchmark modules can
  be compared with `cmp`.
- CPUID: `rdpid` is not claimed (the i7-8750H lacks it); the "asked about AVX" counter firing in nearly
  every glibc program is stated as intended, with the reason.
- x87: precision control by rustc_apfloat semantics of our own (precision 66, 53, 24, extended exponent
  range) and rounding to odd; transcendentals with `astro-float`.
- The x86 machine is shared at times (another track's `~/ordering-cost` wasmtime job, load about 1.0 seen);
  Rust was already on it.

Checked by looking, run 2:
- Repeated on the reference (x86 load 0.07): `mov ss` (SI_KERNEL, CODE+3), `int 0x80` (ran, CODE+3),
  `int 0x81` (SIGSEGV SI_KERNEL), `int1` (TRAP_BRKPT, CODE+1), `int 3` (SI_KERNEL, CODE+2), `syscall`
  (ran, SI_KERNEL CODE+3).
- astro-float 0.9.6, MIT (`cargo info`). It builds for wasm32-unknown-unknown with std. (Run 2 wrote that
  it does not build without std; run 3 found it does once `lazy_static`'s `spin_no_std` is on, see
  there.) What it adds to a stripped wasm32 cdylib (opt-level s,
  LTO; sin, atan, log2, exp, add at 128 bits) against the same cdylib without it: 142,688 bytes raw (160,136
  against 17,448), 40,947 brotli -q 11 (46,678 against 5,731). Mac load 16.6 to 20.7.
- rustc_apfloat 0.2.3's `ieee::Semantics` is a public trait that can be implemented outside the crate.
  Semantics with 15 exponent bits and precision 66 and 24 work. Rounding to odd then once to 24 bits gave
  the right results in both directions (scratch `apfloat-pc`).
- iced 1.22.0: `Code` has 4,936 values (`iced_constants.rs`); `encoding()` and `cpuid_features()` are behind
  `instr_info`.

## X0, run 3 (2026-11-17): the second independent check's findings, all fixed in the plan

Each finding was first repeated on the reference (x86 load 0.00 to 0.89, all results as the check
reported), then fixed:
- Blocking, `call` to a non-canonical target: the CPU writes the return address at rsp - 8 first, then
  gives general protection with rsp unchanged; a page fault on the push wins; `call [m]` reads its target
  before the push. Also seen for a direct `call rel32` from code mapped at 0x7fffffff8000 (`stepper3.c`).
  The plan now has the meaning do the CPU's steps in the CPU's order (read target, push, canonical check,
  move rsp) and the emitter's `call` only makes the call (plan 3.3, 3.4 T14, 4 decision 2, 5.3, 5.4).
- r11 after `syscall` carries the stepping's TF (0x3C3 from 0x2C3): added to the per-case masks (7.2).
- An access running from a canonical start into the non-canonical range is general protection: new
  `Guest::memory_fault(address, code) -> MemoryFault`, which the runner calls before telling the kernel;
  x86 answers from the access's size, no cost on any access (3.2, 3.4, 5.4, 6).
- `rcpps`, `rcpss`, `rsqrtps`, `rsqrtss`: matched bit for bit by a closed formula in f64 vector
  arithmetic, no table and no slow path. Found and checked on the reference over all 2^32 inputs of each,
  zero differences, in MXCSR 0x1F80 and 0xFFC0 (5.8).
- `div` and `idiv` of a 128-bit dividend: inline fast path when rdx is rax's extension, inline long
  division otherwise, divide error rules; not a slow path, cost measured at X3 (5.2).
- The "AVX instruction reached" counter is keyed on the AVX setting's claim group, not on VEX; the build
  table no longer needs a VEX bit (2, 5.2, 5.10, X8).
- Round trips re-measured with the method written down (2 item 3, 7.3).
- FPU claimed by default is now stated in the plan as a reading of the brief, with the reason (5.10),
  and listed below for the owner.
- astro-float and `no_std`: hooks are a runner's job, never the kernel's, so the CPU crates stay `std`
  and the kernel never depends on one (3.1, 3.2, 3.4, 6). astro-float does build without std once
  `lazy_static`'s `spin_no_std` is on (scratch `x87-nostd`: a `no_std` cdylib with its own panic handler
  links with it and fails on a duplicate `panic_impl` without it).
- Interface gaps: `zerocopy` added to the interface's dependencies; `jump_table` returns `Vec<u64>`;
  `Value`, `Induction`, `Comparison`, `Relation`, `Extend`, `NarrowWrite`, `MemoryFault` defined;
  `counting_exit` and `unsigned_bound` became one `branch_comparison`; `narrow_load` became `loaded_max`;
  `index_extension` dropped (the index's `Extend` is in `MemoryAccess`); `NARROW_WRITE` is
  `[Option<NarrowWrite>; 3]`; the effects recorder and `Registers` live in the interface crate (3.1, 3.2,
  3.4, appendix B).
- RISU checked in its source (mirror github.com/rth7680/risu, commit eed2249; git.linaro.org gave 504):
  `risu_i386.c` marks operations with `UD1 r, eax` and steps over its 3 bytes. Licence EPL-1.0, read only.

Measured in run 3:
- New process on the x86 machine: `bash -c 'time (for i in $(seq 1000); do ./stepper 90; done >
  /dev/null)'` 0.93, 1.25, 1.25 s; the same over `./hello` 0.86, 0.88, 0.94 s (x86 load 0.89, governor
  powersave). About 1 ms a process.
- New SSH connection from the Mac, `time ssh tom@192.168.2.240 true`, five runs: 0.24 to 0.32 s (Mac load
  9.2).
- Exhaustive runs on the x86 machine: `approx` 24 s, `approx_formula` 73 s (x86 load 0.02),
  `approx_mxcsr` 145 s (load 0.66).

## X0, run 4 (2026-11-17): the third independent check's findings, all fixed in the plan

- Blocking, code written without announcement missed two write paths: the kernel's own writes into
  guest memory (`Cpu::write`, `zero`, `shared_write`, which ignore permissions) and writes through another
  mapping of a shared page (memfd or shm mapped twice, .NET's W^X double mapping, another process). The
  plan now keeps code marks per backing page in the CPU side (which owns memories, permissions and shared
  blocks), protects every alias of a shared page, and has those three `Cpu` methods take the same
  drop-and-unprotect step before writing. No kernel change: the kernel stays CPU-free. Built at X10 with
  three tests; until then x86 runs only under the stand-in, which maps nothing twice (plan 5.7, 6, 9, 12).
- Blocking, DF among the lazily tracked flags made the per-shape liveness test never true. DF (and ID)
  now live in a control `Rflags`; `FLAGS` is the six arithmetic flags; `flags_needed_after` becomes a
  test per flag (`pending` set), which on ARM64 gives exactly today's answers because every ARM64 setter
  sets all four. "Plain computation" is a CPU function `Guest::plain` (ARM64 today's list), not derived
  from effects, because a derived rule would let ARM64 look past `crc32` and change its output (plan
  3.2, 3.4, 3.5, 5.1, 5.9).
- Blocking, `GuestTest` had no Rust shape. Section 7.2 now gives the trait in full (names, forms, `cases`
  as one strategy per CPU so ARM64's seeds give the same cases, `presented`, `excluded`, `no_form`,
  `offered`/`told`, `designed`, `FIELDS`, `to_state`/`from_state`, `may_differ`, `reference_stop`,
  `auxiliary`, `STAND_IN`, `PROGRAMS`, the corpora, layer 2's `MODEL_REGISTERS`, `Extra` and `assemble`),
  its plain types, the reference runner's per-CPU `Machine` trait, the wire format, the transport
  (`Reference::<G>`, Docker or SSH, same records) and the browser contract's per-CPU numbers.
- Address limit: the x86 personality now has `TASK_SIZE_MAX` 0x7ffffffff000 (plan 5.4, 6, X10).
- Interface gaps: `locals_of` (inverse of `with_locals`) and `complete`; `Bit` has its bit number;
  `Target`, `Access`, `Locals` defined; the flags word holds only the tracked flags, so bits 1 and 9 need
  no interface item; `jump_table` takes the jump's offset and a `before` walk, and ARM64 keeps reading its
  window from the blob exactly as today.
- Far transfers designed (plan 5.3) from new reference runs; `memory_fault` now takes the block and picks
  the faulting access by the reported address, since one instruction's two accesses may differ in size.
- x87 `fsqrt`: rustc_apfloat 0.2.3 has no sqrt (its lib.rs "Future work"), ieee-apsqrt 0.1.1 only
  32/64/128-bit IEEE; now astro-float's sqrt at 128 bits toward zero, exactness by squaring at 256 bits,
  round to odd at 66 bits, then once to 24/53/64 (plan 5.8).
- The reference runner now writes the x87 component too (`xstate_bv` bit 0), proven by `stepper4`.
- Found while checking, added to the plan: with AC set Linux gives `SIGBUS BUS_ADRALN` on a misaligned
  user access. The plan does not provide alignment checking: `popf` and `iretq` leave AC clear (the 386
  test reads it as absent), counted and warned (plan 5.1).

Checked on the reference in run 4 (x86 load 0.00 to 0.09; 36.5 from another job during the AC cases,
nothing timed):
- `cases4.sh` (8 x87 cases with `stepper4`), `cases5.sh` (9 far-transfer cases), and 4 AC and `pushfq`
  cases (`cases6.out`, run as `./stepper3 488b07 rdi=0x20000101 rflags=0x40202` and so on), all as
  recorded in plan section 2, run 4.

## X0, run 5 (2026-11-17): the fourth independent check's findings, all fixed in the plan

Each was first repeated or widened on the reference (x86 load 3.6 to 5.4 from another job, nothing timed):
- Blocking, encodings past 15 bytes: they give general protection, not `SIGILL`, whatever the body is
  (valid, an invalid opcode, an invalid lock): `cases7.sh`. iced reports them as `INVALID` length 15, like
  a 15-byte invalid one. Plan 5.2 now decides it: for `INVALID` at length 15, `decode` drops the prefix
  bytes that change no length (keeps the last 66, 67, F2/F3 and a REX before the opcode), decodes the
  rest and adds the count; over 15 is `GeneralProtection`, otherwise `Undefined`. iced's count after an
  opcode it calls `INVALID` is not the CPU's (`cases8.sh`: `0F 24` counts no ModRM, `0F 38 FF` counts
  one), so the x86 crate gets a table of what the CPU counts after each such opcode, started from a sweep
  of 1,024 opcodes (`sweep15.sh`, 24 count nothing, 539 at least one byte) and completed at X3 (plan 5.2,
  9). `no_form` and 7.4 now say so; X3's pass bar gains 10,000 no-form cases.
- `built_address` now takes `at, i, blob, code, before` like `jump_table`; ARM64 reads the word at
  `at - 4` from the blob whenever it lies in code, exactly as full.rs:561-581, so its output cannot
  change; RV64GC walks `before` (plan 3.2, 3.4).
- `clflush`: a checked one-byte read, then nothing (reference: page fault unmapped, general protection
  non-canonical; `prefetchnta` never faults) (plan 5.10).
- `Stop::Breakpoint(si_code)`, with `GuestTest::breakpoint_code(detail)` for the translated side; ARM64's
  `brk` gives `TRAP_BRKPT` (1), seen in Docker with `brk.c` (plan 7.2, 7.3).
- `Locals` holds 9: x86's shift by cl keeps value, count, result and six old flags, one local each, which
  keeps the per-flag liveness (plan 3.2, 5.1).
- Header now says four checks; 7.3's connection cost now quotes what was measured. Run 5 measured five
  new SSH connections at 0.92 to 1.19 s (x86 load 6.9 from another job, Mac load 27.6) and then 0.27 to
  0.48 s (Mac 28.2); the fourth check saw 0.24 to 0.40 s at Mac load 34.
- This file's list of what is on the x86 machine is corrected (the cases outputs are only on the Mac).

## X0, run 6 (2026-11-17): the fifth independent check's findings, all fixed in the plan

Each was first repeated and widened on the reference (`cases9.sh`, `cases10.sh`, x86 load 0.00 to 0.12,
nothing timed):
- Blocking, the stack fault: a non-canonical access through the stack segment is #SS, which Linux sends
  as `SIGBUS` `SI_KERNEL` address 0, not general protection. Widened: only base rsp or rbp counts (r12
  and r13 give general protection), `fs:` and `gs:` make it general protection, `cs:`, `ds:`, `es:` and
  `ss:` change nothing, every implicit stack access (push, pop's read, call, ret, pushfq, push fs, enter,
  leave) counts, `pop [rbx]` gives either by which address is bad, and an alignment check comes first
  (a misaligned non-canonical `movaps [rsp]` is general protection). Plan: `MemoryFault` gains `Stack`;
  x86 `memory_fault` picks it by one test on the faulting access (no segment, base rsp or rbp); `Stop`
  gains `StackFault`; section 6's signal list, 7.3's mapping, 5.3's call and ret steps, 4 decision 16,
  layer 1's operand drawing and the X2 and X3 pass bars carry it. `TrapKind` does not grow: no meaning
  raises a stack fault (the reason is in plan 3.2).
- Found while fixing it: `Base::Segment` could not express `fs:[rdx + rax * 4]` (a segment base with a
  base register). `MemoryAccess` now has its own `segment: Option<R>` and `Base` is `Register` or `Pc`;
  the check analysis keys on (segment, base), which for ARM64 (segment always `None`) is today's key.
- MMX writes set the x87 register's sign and exponent to 0xFFFF (seen for four writes; reads leave
  them). Plan 5.8: eight i32 controls `X87High(n)` over bytes 8 to 11 of each slot, written with one
  `set_control` by every MMX meaning that writes mmN.
- The longest valid rest after the dropped prefixes is 15 bytes, not 14 (plan 5.2).
- iced's sizes are now given as totals, with the 257 / 232 / 178-byte base subtracted for what it adds
  (plan 2; the two stripped files measured again).
- `SHAPES` for ARM64 is today's ten shapes in `shapes()` order, and the flags-word functions keep the
  generic "flags in locals" one first at `F_FLAGS_WORD` (plan 3.4, 3.5).
- Cross-references fixed (RISC-V is 3.5, the reasons are 3.4) and section 13 lists all five checks'
  folders on the x86 machine.

## X0, run 7 (2026-11-17): the sixth independent check's findings, all fixed in the plan

Each was first repeated and widened on the reference with `stepper5` (`cases11.sh`, `cases12.sh`,
`cases13.sh`, x86 load 0.00, nothing timed):
- Blocking, NT: `popf` and `iretq` keep NT (bit 14) in user mode, `pushf` stores it, `syscall` and fault
  frames keep it, and `iretq` with NT set is general protection before it reads its frame. Plan 5.1:
  `Control::Rflags` holds DF, NT and ID; `iretq` tests NT first (5.3); the kernel's personality puts NT
  in signal frames and keeps NT and ID at sigreturn as Linux does (6).
- Found while checking: `iretq` reads its frame out of order (rflags, then CS, SS before rsp), so a page
  fault names the slot the CPU names. Plan 5.3: five checked reads in the CPU's order; where rip's read
  falls is settled at X5 before the meaning is written.
- Entering NT and ID: proven by setting them in the handler's own rflags just before `rt_sigreturn`,
  which keeps non-`FIX_EFLAGS` bits from its caller. Plan 7.3 (`Machine::enter`, the one-instruction
  steps) and 7.4 (starting rflags: six flags, DF, NT, ID drawn; AC, TF, IOPL, RF, VIF, VIP clear).
- MXCSR and lazy binding: `ldmxcsr` turns tracking on; `fxrstor`, `xrstor` and sigreturn restore without
  it, so glibc's lazy binding trampoline keeps the fast path. `float_slow` is mode bits not 0x1F80 or
  tracking (3.4, 5.8). Confirmed on the reference: the loader has `fxsave`/`fxrstor` and
  `xsavec`/`xrstor`.
- Reading MXCSR's flags while not kept: `stmxcsr` and the `fxsave`/`xsave` family call the float hook
  with `ReadStatus`, counted "MXCSR's exception flags read while not kept" (5.8). `may_differ` now says
  what ARM64's layer 1 does today: untracked default-mode flags must equal the case's starting ones; the
  x87 clause is gone (x87 status is always kept). A signal frame's MXCSR is not counted, as ARM64's FPSR
  today; listed in plan 12.
- VEX or EVEX after 66, F2, F3 or REX: the CPU counts the whole VEX or EVEX instruction (16 bytes give
  general protection, 15 `SIGILL`, also EVEX on this CPU without AVX-512). Plan 5.2 drops those prefixes
  too when the next byte is C4, C5 or 62 (67 stays); 7.4's no-form cases draw such runs.
- `sysenter`: the reference filter now also traps every `AUDIT_ARCH_I386` call wherever made (proven:
  `SIGSYS` with rip in the vDSO). Its meaning is general protection like `int 0x80` (a kernel without
  32-bit emulation loads a null `IA32_SYSENTER_CS`; expected from Linux's source, not seen); SEP claimed;
  excluded from layer 1 with `int 0x80`'s reason and checked by the designed answers (2, 5.2, 5.10, 6,
  7.2 to 7.4, X2, 12).

## X0, run 8 (2026-11-17): the seventh independent check's findings, all fixed in the plan

Each was first repeated and widened on the reference with `stepper6` (scratch `run8/`: `cases14.sh`,
`cases15.sh`, `smc.c`, `copyuser.c`; x86 load 0.00 to 0.16, nothing timed):
- Blocking, code written on its own page: `smc` prints `eax 1 sum 20100` (the check's result). Plan 5.7:
  the step also sets a one-shot mark; the bottom loop runs the faulting instruction once from a
  look-ahead-none piece that is not entered in the lookup and protects no page, and the next instruction
  is translated afresh. Any stop clears the mark; each repeat clears one page's mark, so it ends. Decision
  4.6 and the X6 pass bar (three such programs, among them `smc.c`) say so.
- Blocking, `bt`/`bts`/`btr`/`btc` and `xlat`: `MemoryAccess::index` is now `Index` (`Register` with
  `Extend` from 8 to 64 bits, `BitOffset { reg, width }`, `Lanes` for VSIB). Seen: offsets read signed at
  the operand's width (64, 32, 16), bits above it ignored, rbp base gives the stack fault. The check
  analysis and loop proofs use only accesses ARM64 has today (index `None` or a 32/64-bit `Register`,
  `Fixed` bytes); every other access is checked by its meaning each time (plan 3.2, 3.3, 3.4, 5.4).
- Blocking, gathers and masked moves: `AccessBytes` gains `Masked { element, count }` (and `Counted`
  names its element). Seen and designed (plan 5.4): gathers go in lane order, a fault leaves the earlier
  elements gathered and their mask lanes clear and names the faulting element's first byte not allowed;
  `vmaskmov`/`vpmaskmov` never fault on masked-off elements, are all or nothing, a load reports its first
  byte not allowed and a store the first selected byte when it is not allowed, else the last selected byte
  (two one-byte checks); `maskmovdqu` ignores the mask for faults, stores nothing on a fault, and reports
  as if it checked rdi + 8 first, then rdi. `memory_fault` works out each access's real range.
- `Machine::unfinished` now takes the instruction's bytes and steps on only for a rep string instruction
  or a gather (which under the trap flag stops once, with RF, before its fault), up to `Machine::limit`;
  a branch to CODE is finished (plan 5.6, 7.2, 7.3).
- gzip figures: run 1's were Apple gzip storing the file names (12 bytes more than the check's names).
  The plan now gives `/usr/bin/gzip -9 -n -c`: decoder 60,682, with `instr_info` 82,697, base 208,
  browser 412,037; GNU gzip 1.14 `-n`: 60,831 and 82,541. Brotli is Homebrew's 1.2.1, figures unchanged
  (Mac load 20.4).
- Kernel side, copy routines: each CPU gets its own measured model. x86 (Linux 7.3.4, ERMS, no FSRM):
  pipe `read`, `/dev/zero` `read` and `uname` write every byte up to the page not allowed, for buffers
  ending 1 to 389 bytes before it (plan 6, X10 repeats it).

## X0, run 9 (2026-11-17): the eighth independent check's findings, all fixed in the plan

Repeated and widened on the reference first where the finding was about the CPU (scratch `run9/`:
`cases16.sh` 39 cases and `xacquire.out` 2, with run 8's `stepper6`; x86 load 0.02 to 0.06, nothing timed):
- Blocking, features not claimed whose bytes run on a CPU without them: the whole reserved-NOP space
  (`0F 0D`, `0F 18` to `0F 1F`: `endbr64`, `endbr32`, `rdssp`, `cldemote`, MPX, `prefetchw`,
  `prefetchwt1`, `prefetchit0`) runs as a NOP on the reference with any address, never faulting; a lock
  prefix there gives SIGILL. Plan 5.2: iced decodes with `FORCE_RESERVED_NOP`, which makes all of them
  `Reservednop_*` Codes (feature `MULTIBYTENOP`, claimed) with one meaning that does nothing; ignored
  prefixes (`xacquire`, `xrelease`, `bnd`, `notrack`) are ignored, as seen. 5.10 states the rule. Layer 1
  (7.4, 7.2 `presented` and the new `REFERENCE_HAS`) splits forms by the decoded Code's features and runs
  the not-offered list on the reference too: compared where the reference lacks the feature (TSX, WAITPKG,
  CLWB, PTWRITE, SERIALIZE, MOVDIRI all SIGILL there), designed answer where it has it (AES, PCLMULQDQ,
  RDRAND, RDSEED, CLFLUSHOPT, XSAVEOPT, XSAVEC, the AVX group when off). ARM64's tests stay as they are.
- `Guest::shape` now returns `Option<Self::Shape>`, as today's `Shape::of` (3.2, 3.4).
- Code written on its own page: the plan now reuses phase one's exact path (`UNWIND_GO_TO_EXACT`,
  `EXACT_SLOT`, `load_code(pc, 1)`). At X6 the memory fault hook returns the unwind payload (ARM64's
  runners always `UNWIND_LEAVE`; ARM64's output loses one `i32.const` per fault site), and the CPU side
  keeps a per-thread exact mark that makes that one `load_code(A, 1)` translate without protecting (5.7,
  decision 6, X6).
- Berkeley SoftFloat: considered and not used, with the reason in plan 5.8. softfloat-sys 0.1.4 and
  softfloat-rs 0.1.0 do not build here: their build scripts test the building machine with `cfg!` and
  panic off x86-64 ("build rules are not implemented"), and softfloat-sys's `c99` dependency runs CMake,
  which fails with the Nix shell's CMake. The pure-Rust crates lack 80-bit precision control.
  SoftFloat's C does compile for wasm32 with zig (`extF80_sqrt.c`, seen), so vendoring it is possible
  but would put C into the Rust project's x86 crate and every build of it.
- `cpu_hook`'s buffer: the cdylib exports `cpu_state() -> *mut u8`, `threadStateSize` bytes allocated once;
  runner.js's per-call copy, index and results are spelled out (3.4 Hooks).
- Code area: `FILLER` and a new `CODE_SPAN` are per guest in `layout.rs`; the translated side writes the
  filler over the code area when it is not 0 (x86: 0xCC over 64 KiB, ARM64 nothing), and layer 1 draws
  rip-relative operands into it (7.2, 7.4).
- RISU's operation number is in ModRM's r/m field (plan 2, checked in scratch `sources/risu/risu_i386.c`).
- iced without std: proved by a `no_std` cdylib with its own panic handler that links (scratch
  `run9/iced-strict-nostd`), with a control using `std` that fails on a duplicate `panic_impl`
  (`run9/iced-strict-std`) (plan 2).

## X0, run 10 (2026-11-17): the ninth independent check's findings, all fixed in the plan

Repeated and widened on the reference first (scratch `run10/`: `tf.c`, `rf.c`, with `stepper6` for
`umip.out` and `branch-unmapped.out`; x86 load 0.00 to 0.24, nothing timed):
- Blocking, the trap flag had no state: TF set by a program now behaves as seen with `tf.c` (popf or
  iretq setting TF: the next instruction runs, then `TRAP_TRACE`; popf clearing TF still stops after
  itself; `syscall` with TF: the instruction after it runs before the stop; `int3` and a fault give only
  their own signal; TF kept across `rt_sigreturn`; handlers start with TF clear; `pushfq` stores TF).
  Plan 5.1: TF is bit 8 of `Control::Rflags`; while it is set the stop flag stays raised and a new
  `Guest::step` (default `Off` for ARM64 and RV64GC) runs each instruction alone through phase one's exact
  path and answers `Trap` at the next stop check. New `Emitter::raise_stop` (only x86's `popf` and `iretq`
  call it), `StepAt` and `Step` types, x86 private words `Step`, `StepAt`, `StepHeld` (`mov ss`). The stop
  hook returns the unwind payload from X6, like the memory fault hook. `SingleStep` is no longer a
  `TrapKind` (no meaning raises it). rep string instructions take the bulk path only with DF and TF clear.
  One known difference recorded: a gather faulting part way under TF (designed-answers case at X8).
  Section 6: TF from the sigreturn frame goes into the control and steps; RF is put in fault frames only
  (seen with `rf.c`) and not kept at sigreturn (no program can see it). Tests: X6 trap-flag blobs compared
  with the reference (the blob runner now records a fatal signal's rip and registers, 7.3); X10 runs
  `tf.c` and `rf.c` against the reference's output.
- `CODE_SPAN`'s comment: a branch beyond the code area stops by `TRAP_TRACE` at the target before any
  fetch (repeated: `jmp rel32`, `jmp rax` to two unmapped addresses), so on x86 it matters only for fall
  through and rip-relative operands.
- SSE3 (`fisttp`) and CMOV (`fcmovcc`, `fcomi`, `fcomip`, `fucomi`, `fucomip`) contain x87 instructions:
  plan 5.10 extends the FPU reading to them, with the reason.
- The claim lists now name iced's own `CpuidFeature` values (plan 5.10 gives both lists), with the
  baseline names `INTEL8086` to `INTEL486`, `X64`, `CPUID`, `PAUSE`, `MULTIBYTENOP`, and `FPU287`,
  `FPU387`, `CLFSH`, `CMPXCHG16B`, `SEP`, `SYSCALL`. The claim table gains a fourth value, "kernel only",
  for every `is_privileged()` Code (plan 2, 5.2). Counted in `run10/feat.out`. `sldt`, `str`, `smsw`
  (baseline `INTEL286`) run in user mode on the reference (no UMIP).
- Layer 1's forms now cover every encoding but MVEX (and the `KNC` VEX Codes, which iced decodes only with
  its `KNC` option), split into default, AVX setting, never-claimed and kernel-only lists by the decoded
  Code's claim group; the never-claimed list (EVEX, XOP, 3DNow and the rest) runs on the reference at 100
  cases a form; X5's pass bar includes it (plan 7.4, 9).
- The plan's header now counts nine checks.

## How to run

- X2's proofs: same cases, `WC_OUTPUT_ROOT=<scratch>/x2/same nix develop -c
  crates/wcontainers-translator-test/same-cases.sh d27cd70 1 2 3` (knobs `WC_CASES_LAYER1`, `WC_CASES_LAYER2`;
  layer 1's no-form and hand-made lists are compared only against commits from 995e949 on, which write them;
  against d27cd70 the no-form words by `<scratch>/x2/run5/compare-no-form.sh` after drawing them as its
  header says);
  the x86 reference, `WC_REFERENCE_X86_64=tom@192.168.2.240 nix develop -c cargo test -p
  wcontainers-translator-test --test reference_x86_64 -- --nocapture`; the float hook in Chrome old against
  new, `<scratch>/x2/chrome-hook.sh <old tree> <rounds>`.
- The suite now needs `WC_REFERENCE_X86_64=tom@192.168.2.240` (the x86 reference test fails without it).
- Per-guest tests are named with the guest's suffix: `floats_speed_arm64`, `switching_speed_arm64`
  (`--test programs`), `measure_benchmarks_arm64`, `measure_calls_between_modules_arm64` (`--test
  benchmarks`), `measure_binaries_arm64` (`--test binaries`), `write_output_arm64` (`--test output`); the
  decoder's and validity tests are ARM64's own (`..._arm64`).
- Long runs of one guest: `WC_LONG_DIR=<dir> WC_LONG_GUEST=arm64 nohup nix develop -c
  crates/wcontainers-translator-test/long-runs.sh > <log> 2>&1 &` (ledger `<dir>/arm64/ledger.txt`).
- M5's real binaries are now under `target/binaries/arm64`.
- Lint the runner for both CPUs: from `crates/wcontainers-translator-test/reference`, `nix develop ../../.. -c
  cargo clippy --release --target <aarch64|x86_64>-unknown-linux-musl --target-dir ../../../target/reference
  -- -D warnings`.

- X1's byte-for-byte proof: `WC_OUTPUT_ROOT=<scratch>/x1/same nix develop -c
  crates/wcontainers-translator-test/same-output.sh 2274314 bench,busybox,musl,python,rg,git,java,go`, then
  the same with `node` and with `bun`, each alone (many GB). The earlier commit's lists are kept under
  `<root>/lists/<commit>`, so later runs only translate the current tree; delete them when
  `tests/output.rs` changes. `WC_OUTPUT_LOOK_AHEADS=full,bounded` (labels of `LOOK_AHEADS`) limits the
  look-aheads. The binaries come from `target/binaries` (copied from the translator worktree's M5 files).
- The float hook's time: `nix develop -c cargo test --release -p wcontainers-translator-test --test
  float_hook -- --ignored --nocapture` (knobs `WC_FLOAT_STEPS`, `WC_FLOAT_RUNS`); old against new by
  running the same file in a copy of the earlier commit (`<root>/trees/<commit>` of the comparison).
- The test guest (both variants, validated and run): `nix develop -c cargo test -p wcontainers-translator-test
  --test test_guest`.
- The float hook in Chrome: `nix develop -c cargo test --release -p wcontainers-translator-test --test
  float_hook -- --ignored --nocapture float_hook_time_in_chrome`; old against new alternating:
  `<scratch>/x1/run3/chrome-alternate.sh <rounds>` (it copies nothing; put the same `tests/float_hook.rs`
  into the earlier commit's copy first).
- The suite of what this track owns: `nix develop -c cargo test -p wcontainers-guest-cpu -p wcontainers-arm64
  -p wcontainers-translator -p wcontainers-translator-test` (Chrome and Docker needed), and the browser
  crate's lint as in docs/status-translator.md.
- Translate time, old against new: build both sides' `tests/output.rs` (the comparison does), then
  `<scratch>/x1/run4/translate-time.sh <rounds>` (wall and user seconds) or `<scratch>/x1/run5/count.sh
  <rounds>` (instructions retired and cycles from `/usr/bin/time -l`; edit the binaries' paths at its top),
  or `<scratch>/x1/run6/count.sh <rounds> old=<binary> new=<binary>` (the same, any sides, order flipped each
  round).
- The no-std check is `tests/no_std.rs` of the test crate (part of the suite); by hand: `nix develop -c
  cargo build -p wcontainers-guest-cpu -p wcontainers-arm64 --no-default-features --target wasm32v1-none
  --target-dir <scratch>/x1/nostd-target`.

- Run 10's experiments: `nix develop -c zig cc -target x86_64-linux-musl -static -O1 -fno-stack-protector
  -o <scratch>/run10/tf <scratch>/run10/tf.c` (and `rf` likewise), copy both to
  `~/wcontainers-x86/proof10/` and run each there; `umip.out` and `branch-unmapped.out` are `stepper6` in
  `~/wcontainers-x86/proof8` with `0f00c0`, `0f00c8`, `0f01e0`, and `e900000010`, `ffe0
  rax=0x700000000000`, `ffe0 rax=0x7ffffffff000`. iced's feature names: `nix develop -c cargo run
  --release --manifest-path <scratch>/run10/feat/Cargo.toml --target-dir <scratch>/run10/feat/target`.

- Run 9's experiments: copy run 8's `stepper6` and `run9/cases16.sh` to `~/wcontainers-x86/proof9/` and
  run `./cases16.sh` there. Decoding: `nix develop -c cargo build --release --manifest-path
  <scratch>/run9/dis/Cargo.toml --target-dir <scratch>/run9/dis/target`, then `dis9 <hex> ...`. iced
  without std: `cargo build --release --target wasm32-unknown-unknown` of `run9/iced-strict-nostd` (links)
  and `run9/iced-strict-std` (must fail). SoftFloat: the same build of `run9/sf` (fails, as recorded).

- Run 8's experiments: build `stepper6.c`, `smc.c`, `copyuser.c` in scratch `run8/` like the steppers
  below, copy them and `cases14.sh`, `cases15.sh` to `~/wcontainers-x86/proof8/` and run each there.
  `NOTF=1`, `STEPS=n` and `DUMP=offset` in the environment change how `stepper6` runs (its header says
  how). gzip sizes: `/usr/bin/gzip -9 -n -c <file> | wc -c` on scratch
  `iced-size/target/wasm32-unknown-unknown/release/size_*.stripped.wasm`.

- Run 7's experiments: build `stepper5.c` like the steppers below, copy `stepper5`, `cases11.sh`,
  `cases12.sh` and `cases13.sh` to `~/wcontainers-x86/proof/` and run each there. `SECCOMP=1` in the
  environment installs the filter with its arch test.

- Run 6's experiments: copy `stepper4`, `cases9.sh`, `cases10.sh` from scratch `reference-proof/` to
  `~/wcontainers-x86/proof/` and run each there (`ssh tom@192.168.2.240 'cd ~/wcontainers-x86/proof &&
  ./cases9.sh'`).

- Run 5's experiments: copy `stepper4`, `cases7.sh`, `cases8.sh`, `sweep15.sh` from scratch
  `reference-proof/` to `~/wcontainers-x86/proof/` and run each there. `sweep15.iced` adds to each line
  of `sweep15.out` iced's decoding of the opcode followed by eight `cc` bytes (scratch `dis` tool);
  `sweep15.classes` keeps the `INVALID` ones, as "more" where the boundary gives general protection and
  the opcode alone does not, else "none". `brk.c`: `nix develop -c zig cc -target aarch64-linux-musl
  -static -O2 -o brk brk.c`, then
  `docker run --rm --name wc-x86-brk -v <scratch>/reference-proof:/p:ro alpine:3.22 /p/brk`.
- Run 4's experiments: `python3 -I <scratch>/scripts/make_stepper4.py <scratch>/reference-proof/stepper3.c
  <scratch>/reference-proof/stepper4.c`, build it as below, copy `stepper4`, `cases4.sh`, `cases5.sh` to
  `~/wcontainers-x86/proof/` and run the two scripts there.
- Run 3's experiments: build `stepper3.c`, `approx.c`, `approx_dump.c`, `approx_formula.c`,
  `approx_mxcsr.c` the same way (without `-fno-stack-protector` for the approx ones), copy to
  `~/wcontainers-x86/proof/`, run each (`CODE_BASE=0x7fffffff0000 ./stepper3 e8fb7f0000` for the direct
  call). astro-float without std: `nix develop -c cargo build --release --target wasm32-unknown-unknown
  --manifest-path <scratch>/x87-nostd/Cargo.toml --target-dir <scratch>/x87-nostd/target`.
- Experiments of X0: build `nix develop -c zig cc -target x86_64-linux-musl -static -O2
  -fno-stack-protector -o <scratch>/reference-proof/stepper <scratch>/reference-proof/stepper.c` from the
  checkout root; copy `stepper` and `cases.sh` to `tom@192.168.2.240:wcontainers-x86/proof/`; run
  `ssh tom@192.168.2.240 'cd ~/wcontainers-x86/proof && ./cases.sh'`.
- Size of the decoder: `nix develop -c cargo build --release --target wasm32-unknown-unknown
  --manifest-path <scratch>/iced-size/<base|dec|info>/Cargo.toml --target-dir <scratch>/iced-size/target`,
  then `wasm-tools strip` and gzip -9 / brotli -q 11.

## The x86 machine

- Intel i7-8750H, 6 cores / 12 threads, 15 GB, AVX2, FMA, FSGSBASE, no AVX-512; Ubuntu 20.04.3 with a
  mainline Linux 7.3.4 kernel; reached as `ssh -o BatchMode=yes tom@192.168.2.240`.
- Installed on it by this track: nothing. Already there before this track and not used by it: Rust
  (`~/.rustup` dated 2026-11-14, rustc 1.100.0). Files of this track: `~/wcontainers-x86/proof/` (hello,
  stepper, stepper3, stepper4, stepper5, the scripts cases.sh, cases2.sh, cases4.sh, cases5.sh, cases7.sh,
  cases8.sh, sweep15.sh, cases9.sh, cases10.sh, cases11.sh, cases12.sh, cases13.sh, and approx,
  approx_dump, approx_formula, approx_mxcsr with approx.out, approx_formula.out and approx_mxcsr.out) and
  `~/wcontainers-x86/proof8/` (stepper6, smc, copyuser, cases14.sh, cases15.sh and their outputs) and
  `~/wcontainers-x86/proof9/` (stepper6, cases16.sh, cases16.out) and `~/wcontainers-x86/proof10/` (tf,
  rf, tf.c, rf.c, tf.out, rf.out). The
  cases scripts' outputs before run 8 are kept only in the Mac's scratch `reference-proof/`. The
  independent checks' files are in `~/wcontainers-x86/check` and `check2` to `check9`. Installed in runs
  2 to 10: nothing.
- Added in X2: `~/wcontainers-x86/reference/<hash>`, the reference runner the harness uploads (one file per
  build of it).
- Not always idle: another track's atomic-cost measurement (`~/ordering-cost`, wasmtime) runs there at
  times. The independent check saw load about 1.04. Run 2 saw 0.07. Record `/proc/loadavg` with every
  timing there.

## Cases actually run

- X0: 22 single-instruction cases on the reference CPU in run 1, 6 more in run 2, 21 more in run 3
  (hand-written, no seed); in run 3 also every one of the 2^32 inputs of `rcpss` and of `rsqrtss`, three
  times (analysis, formula in MXCSR 0x1F80, formula in 0xFFC0); in run 4, 21 more hand-written cases; in
  run 5, 21 hand-written cases (`cases7.sh`, `cases8.sh`) and the 1,024-opcode sweep twice over (each
  opcode at the boundary and alone, 2,048 runs), plus one ARM64 `brk` in Docker; in run 6, 46 hand-written
  cases (`cases9.sh` 38, `cases10.sh` 8); in run 7, 45 hand-written cases (`cases11.sh` 15, `cases12.sh`
  18, `cases13.sh` 12); in run 8, 41 hand-written cases (`cases14.sh` 24, `cases15.sh` 17), one run of
  `smc` and 42 copy cases (`copyuser`: 14 buffer placements for each of three calls). `cases14.sh` ran
  twice: the first run did not pass its environment settings to `stepper6` and was discarded; in run 9,
  41 hand-written cases (`cases16.sh` 39, the two `xacquire` runs 2); in run 10, one run each of `tf`
  (8 trap-flag scenarios in 9 runs, 20 stops) and `rf` (8 signal frames), and 6 `stepper6` cases.
- X2 run 6: the x86 reference's 43 hand-made cases, 2 blobs, the reconnection case and the refusal test
  (4 refused cases and 2 that run), twice outside the suite (the first stopped at the `popf` that set AC,
  above) and in the suite run.
- X2 run 5: `same-cases.sh d27cd70` for seeds 1, 2, 3 and 7717 at 64 cases a form and 64 programs, both
  sides; `tests/cases.rs` at 1000 cases a form and 1 program for the same four seeds (this side only, for the
  no-form lists); d27cd70's no-form words at 64 and 1000 for the same seeds (scratch test); the x86
  reference's 38 cases, 2 blobs and the reconnection case (3 `nop` runs, 2 cuts) twice outside the suite and
  in each suite run; the suite twice at the defaults (seed 1), before and after the merge.
- X2 run 4: `tests/cases.rs` for seeds 1, 2, 3 and 6151 at 64 cases a form and 64 programs, both sides;
  `tests/programs.rs` once and the suite once at the defaults (seed 1); the x86 reference's 38 cases and 2
  blobs in the suite.
- X2 run 3: `tests/cases.rs` for seeds 1, 2, 3 and 4127 at 64 cases a form and 64 programs, both sides;
  layer 2 twice at the defaults (seed 1, 64 programs; the first run's 8 native cases met Docker down);
  changes at the defaults; the x86 reference's 38 cases and 2 blobs once outside the suite; the suite once.
- X2 run 2: `tests/cases.rs` for seeds 1, 2, 3 and 5309 at 64 cases a form and 64 programs, both sides;
  layer 1 once at the defaults (seed 1: 64 cases a form over 1215 forms, 1 a form in Chrome, 64 no-form
  cases, 105 hand-made); the suite at the defaults below (one run stopped part way, once more to the end).
- X2 run 1: the suite twice at the defaults below (seed 1); `tests/cases.rs` for seeds 1, 2, 3 and 9071 at
  64 cases a form and 64 programs, on both sides of each comparison; 38 hand-made x86 cases and 2 blobs, four
  times (twice while fixing the `sysenter` case, which the first stopped at, and in each suite run).
- X1: each run of the suite (one in each of runs 1 to 6, the last listed under that run) ran the seeded
  random tests at their defaults, all with seed 1: layer 1 at `WC_LAYER1_CASES` 64 a form (and
  `WC_LAYER1_CHROME` 1), decoding at `WC_DECODE_CASES` 64 a form and `WC_DECODE_WORDS` 200,000, layer 2 at
  `WC_LAYER2_CASES` 64 programs (`WC_LAYER2_NATIVE` 8, `WC_LAYER2_CHROME` 1), layer 3 at `WC_LAYER3_CASES` 4,
  changes at `WC_CHANGES_CASES` 64 programs and `WC_CHANGES_PROGRAM_SEEDS` 3, look-aheads at
  `WC_LOOKAHEAD_CASES` 4, validity at `PROPTEST_CASES` 256 (`PROPTEST_RNG_SEED` 1) and `WC_VALID_WORDS` 16
  (that one from its own fixed seed).
  No run raised a knob. The test guest's programs and the byte-for-byte comparison are not random.

## Measurements

All in the run sections above (X0 runs 9 and 10 timed nothing; X1's benchmark times are in run 2, the
float hook's in runs 2, 3 and 5, translate time in runs 4, 5 and 6).

## Licences of what this track uses

- Added at X1, all already in the workspace: `blake3` (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH
  LLVM-exception) as a dev-dependency of the test crate; `wasm-encoder` (Apache-2.0 WITH LLVM-exception OR
  Apache-2.0 OR MIT) as a dependency of the test crate and of the interface crate; `zerocopy` 0.8
  (BSD-2-Clause OR Apache-2.0 OR MIT) as a dependency of the interface crate and of the browser crate;
  `wasmparser` 0.263 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT) as a dev-dependency of the test
  crate (the test guest's validity test; it was the translator's until the test guest moved in run 3).

- iced-x86 1.22.0: MIT. Since X2 a dependency of the reference runner when built for x86-64 (decoder and
  `instr_info`, to tell rep string instructions and gathers); X3 adds it to the x86 crate.
- `blake3` is since X2 a normal dependency of the test crate (the hashes in the lists compared between
  commits; it was also the reference runner's content hash until X2 run 7).
- `sha2` 0.10.9 (MIT OR Apache-2.0): since X2 run 7 a normal dependency of the test crate (the reference
  runner's SHA-256, checked on the SSH machine by `sha256sum`); it was already in the lock.
- `wasmparser` 0.263 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT) is since X2 run 4 a normal
  dependency of the test crate (was a dev-dependency): `machine-code` reads a module's lookup table with it.
- The toolchain gains the `x86_64-unknown-linux-musl` target (for the reference runner).
- `rustc-hash` 2.1.3 (Apache-2.0 OR MIT): a normal dependency of the test crate, added by the translator track
  (its runner's counters), merged in X2 run 5.
- `wait-timeout` 0.2.1 (MIT/Apache-2.0): since X2 run 6 a normal dependency of the test crate (bounds each zig
  build); it was already in the lock through proptest's `rusty-fork`.
- astro-float 0.9.6: MIT, with astro-float-num 0.3.7 and astro-float-macro 0.4.6 (both MIT), lazy_static,
  itertools, either (all MIT or Apache-2.0, already common). Planned for X7 (x87 transcendentals); not yet a
  dependency.
- rustc_apfloat 0.2.3: Apache-2.0 WITH LLVM-exception, already a dependency of wcontainers-arm64.
- sandsifter: BSD-3 (Battelle); read only, nothing copied.
- RISU: EPL-1.0; read only, nothing copied.
- gex: AGPL-3.0; not read, nothing used.
- Looked at in run 9 and not used: softfloat-sys 0.1.4 (BSD-3-Clause, bundles Berkeley SoftFloat 3,
  BSD-3), softfloat-rs 0.1.0 (BSD-3-Clause), softfloat 1.0.0 (MIT OR Apache-2.0), machina-softfloat 0.1.2
  (MIT), era2m_softfloat 0.2.1 (BSD-3-Clause-Clear). Nothing copied.

## Departures from the plan

X1 (each also written into the plan's text in the same commit):
- The kernel's side: the plan had the interface crate need std and the kernel never depend on a CPU crate,
  against the brief ("the kernel is written once against the interface"). As the orchestrator ruled, the
  interface and each CPU crate now have a `no_std` core (`Personality`: register block and offsets now, the
  rest of the personality at X10) and a translation half behind the default feature `translate`; the kernel
  will depend on both with it off (plan 3.1, 6). `X86Block` lives in x86's core, not the abi.
- Each CPU's test support (`GuestTest`, plan 7.2) lives in the test crate, not in the CPU crate, though the
  brief says each CPU crate supplies everything specific to its CPU: it needs test-only dependencies
  (wasmtime, Docker and SSH transport, dynasm, iced's assembler) that a shipped CPU crate must not carry,
  and it is used only by the tests. The CPU crate still supplies everything the translator, runners and
  kernel use.
- Run 1 built only the interface items ARM64 uses and put the rest off; run 2 built them all (plan 3.2,
  "Built whole at X1"), as the plan reviewed before X1 had it. Three uses that change ARM64's output stay
  with the milestone whose x86 code needs them, each its own commit with its own byte-for-byte comparison:
  the description section's guest and offer bytes (X3) and the stop and memory fault hooks returning the
  unwind payload (X6, which the runners' `step` and code-change handling need). The first frame check also
  covering a `Stack` return slot waits for X4, where it is measured (gated on `ReturnAddress::Stack`).
- No `NARROW_WRITE` item: nothing would read it. The proofs use a 32-bit induction only through a 32-bit
  read, where zero and sign extension agree, and x86's merging writes are never inductions (plan 3.2).
- `HookType` gains `state: StateBytes`, the bytes of the thread state a `Cpu` hook works on, and
  `run_hook` gets only those, so a hook costs its work and not a copy of the whole state (plan 3.2, 3.4).
- `Guest::STEPS` (default false) beside `step`, so runners copy nothing for a CPU that cannot step.
- `TrapKind::Overflow` (7) for x86 `int 4` (plan 3.2, 5.2, 6).
- `initial_block` is in `Personality`, not `Guest`: the kernel makes threads too.
- A CPU's own slow paths are numbered from `GUEST_REASONS` (64); the translator's `Sites` has room for
  16 of them; `Reason::name_of::<G>` names any number.
- The runners' stops for faults the abi does not name yet are the test crate's `Stopped::Fault`; the
  abi's `Exit` grows them at X10 (the abi is not this track's until then).
- Signatures changed while building, to keep ARM64's bytes: `trap_kind` returns `TrapPc` too;
  `temporaries(flags)` added (logic takes no temporaries today); `run_hook` takes the hook's index;
  `data_address` and `branch_target` push rather than work in place; `branch_comparison` gets each path
  instruction's effects and the branch's offset; `loaded_max(i)` returns the register and the bound. The runtime's `Reason`
  list moved into the interface (the contract re-exports it and `TrapKind`).
- The comparison is one command (`same-output.sh`) writing lengths and BLAKE3 hashes of every module, not
  the `WC_BENCH_DUMP` knob and stored files: Bun alone is about 4 GB a variant, and the lists cover full and
  bounded look-ahead too. Equality of two 256-bit hashes is taken as byte for byte equality.
- `tests/valid.rs` moved from the translator crate to the test crate (it draws ARM64's forms), so the
  translator has no dependency on a CPU crate at all, dev-dependencies included (X0 finding).
- Run times: wasmtime only, old and new alternating, best of 5 (the orchestrator's X1 task), not wasmtime
  and Chrome best of 3 as the plan had.
- The X1 pass bar gains a test guest, the `no_std` build as a test, the float hook timing and the wider
  comparison (plan 9). The test guest lives in the test crate (`tests/test_guest.rs`), not the translator's
  tests, since run 3 makes it run its translations in wasmtime, which the test crate already has; it has a
  variant without flags shaped like RV64GC.
- No interface item says whether a misaligned jump target faults: the bottom loop tests `ALIGN` as before,
  and RV64GC's own meanings never make an odd target (`jalr` clears bit 0), so such an item would have no
  CPU that needs it (plan 3.4).
- The translator's flag states are bits of a `u128`, so a CPU has at most 126 shapes, checked at compile
  time (plan 3.4).
- A segment access with no base or index (`fs:[d]`) groups under the segment alone (plan 3.4).
- `ReturnAddress::Register` has no register bit (the plan had `Register(u32)`): nothing reads it, since a
  call's meaning writes the link register itself (plan 3.2).
- The test guest runs at the comparison's look-aheads and bounded 5 (plan 9).
- The host runner hands a hook of the guest's own its state bytes in place, not a copy, and makes each such
  hook's closure for its index at compile time. The plan had the runner copy the named bytes; the copies
  and the per-call lookup of the hook's types cost the float hook program 8 G instructions (run 5; plan 3.4
  Hooks).
- RV64GC's MAX_LENGTH is 8, not 4: its crate reads a far call's `auipc ra; jalr ra` as one instruction, so
  the translator treats it as a direct call with nothing more in the interface (plan 3.4, 3.5).
- No intentional change of ARM64's output at X1: the stop check's raised branch changes at X6 (plan 5.1),
  so the whole move is compared byte for byte against one "before".

X2 (each also written into the plan's text in the same commit):
- `HookBody::CodeChanged` carries its line size; RISC-V's `fence.i` gets its own variant later (plan 3.2, 3.5).
- A hook takes at most 4 parameters (`HookType::MAX_PARAMS`): eight cost ARM64's float hook about 15 percent
  in Chrome (plan 3.2).
- The transport is `Reference::start(&Target)`, a value of names and fields, not `Reference::<G>`, so the x86
  reference is proven before an x86 `Guest` exists; `GuestTest` will name its `Target` (plan 7.2).
- The seccomp window is given as its end as seccomp reports it (ARM64 8, x86 18), not 4 and 16 (plan 7.2).
- `Machine` works on raw frame pointers, keeps and restores the runner's frame itself, gains `AUDIT_ARCH` and
  `blob_stop`, and `limit` (with iced) decides before the case which instructions may be stepped. Since run
  6 x86's `leave` also decodes, after the runner's own thread pointer and flags are back, to tell `popf` and
  `iret` (plan 7.2).
- Run 6: `Machine::enter` can refuse (x86: a thread pointer `arch_prctl` refuses), answered as `RESULT_REFUSED`
  and given as `CaseRan::Refused`, never as a stop; `leave` gets the instruction, the start state and the
  `siginfo`, and x86 reports TF as the instruction left it instead of masking it, so plan section 2's mask of
  TF in rflags is gone (TF stays masked only in the copies an instruction makes); the read-back handler
  clears AC too; x86's `Alignment` holds 0, as Linux reports #AC (plan 2, 7.2, 7.3, 7.4).
- Isolation: one long-lived case child, replaced when a case ends it, instead of a fork per batch; batching
  and the recorded results of plan 7.3 are not built yet, and plan 4 (item 12) and 7.3 now say so.
- Both runners are static executables that are not position independent: x86's is built with `-C
  relocation-model=static`, since Rust's musl target for x86-64 builds a static PIE that Linux loads at a
  random high address (found by the X2 check; until run 5 its `--image-base` did nothing). It is linked at
  1.5 GiB, ARM64's at 2 GiB (plan 7.2).
- A request whose connection is lost (written or answered after the transport's child ended) starts the
  runner again and is sent again, at most five times a request; before, only `start` retried (plan 7.2).
  An upload writes a part file of its own, so runs uploading at once never write into a started runner.
- Containers this track starts are named `wc-x86-...` (were `wc-translator-...`).
- ARM64's blobs go through the wire to one long-lived container per runner, not a container per blob.
- `GuestTest` as built differs from plan 7.2's listing where building showed it must (plan 7.2, "As built
  at X2"): the names group is `REFERENCE: Target`; `Form` is `Clone`; `cases(form)` takes no offer
  (`DEFAULT_OFFER` until X8); `no_form(seed)` gives whole cases; `hand_cases()` stands for `designed` and
  also holds the hand-made compared cases; `reconcile` stands for `may_differ` (a mask cannot say "equal to
  the start" or "the reference's stored bytes do not count"); `track`, `PC_FIELD`, `SP_FIELD`,
  `GENERAL_FIELDS` and `NAME` added.
- ARM64's four hand-made layer 1 tests (alignment faults, vector registers, the step instructions' edges,
  designed answers) are one test, `hand_cases_do_what_they_must_arm64`, over the same 105 cases.
- A Chrome instruction case carries `MAX_LENGTH` bytes, the instruction then the filler (ARM64: the 4-byte
  word, as before).
- The gcc image of layer 3 is per guest (`wc-x86-gcc-arm64`, built for the guest's Docker platform), so it no
  longer shares `wc-translator-gcc` with the translator track.
- The browser translator exports `initial_block(pc, sp)` (plan 7.2 had the contract carry offsets), so the
  page never writes a register by its offset.
- Layer 2 (run 3, plan 7.2 "As built"): `MODEL: Model` in place of `MODEL_REGISTERS` (the model draws the
  registers a system call sets and the one kept across calls too); `Extra` and `extra()` wait for x86's
  layer 2, where their content is known; the shared model's operations are ARM64's, which another CPU's
  `assemble` maps to its nearest forms.
- The corpora are typed (`Smoke`, `Check`, `Change`), not `Snippet` and `Expect`. ARM64's 11 checks tests
  and 7 hand-made changes tests are one test each over the same programs, each program on its own thread.
- `auxiliary(offer)` is `AUXILIARY`, and the blob record carries the entries to the runner (plan 7.2's
  wire).
- The real binaries are kept per guest under `target/binaries/<guest>` (were `target/binaries`).
- Proposed: plan 7.3's recorded reference results and layer 1 batching are built at X3, with x86's layer 1,
  the first run whose reference is remote and many cases a form; X2's pass bar does not name them. Not yet
  ruled on.
- Programs (run 4, plan 7.2 "As built"): no `PROGRAMS` constant, the folder is `programs/<NAME>`, looked in
  first and on the include path; `switch.h` (file-level assembly, `start_context`, `CALL_ON_STACK`) in place
  of `switch.S`, so each program stays one source file; no `start.S` (the programs start from a C `_start`).
- Of plan 7.2's `GuestTest`, built in run 5: `form_group(&form)` (layer 1's summary counts failing forms
  by group) and `offered()` and `told()` without an offer (one test written once replaced ARM64's own HWCAP
  check). Not built at X2, recorded in plan 7.2 "As built" (found missing and unrecorded by the X2 check):
  `presented(code, offer)`, `REFERENCE_HAS` and layer 1's rule for an `Err` case of a feature not offered,
  which come with x86's layer 1 at X3, since ARM64 would answer `Ok` to every instance and give an empty
  list, leaving the rule as code no case reaches; and `Stop::SingleStep`, which comes with X6, where the
  translated side first stops by the guest's own trap flag.
- `GuestTest::PROBE_ANSWERS` (not in the plan's listing): every answer the probe can give. `output.rs` and the
  `differences` program run for each; `differences_match_linux_on_other_hosts` (one host) became
  `differences_match_linux_on_every_host_arm64` (all four, with ARM64's counter checks on the host it had).
- Tests of one CPU's own types stay that CPU's and are named with its suffix: `decoding.rs`, `valid.rs`,
  `blobs_are_told_the_hwcap_of_the_abi_arm64`.
- `machine-code` numbers a piece's function by the module's lookup table, not 8 + its position (stale since
  phase one: the pieces start after the translator's own functions and the guest's hooks).
- The SSH reference keeps its runner under its SHA-256 (the plan said "content hash"), so the machine can check
  the content with `sha256sum` rather than trust the name (X2 run 7).
- The browser page reads the pc by `pcOffset` and nothing else of the register block (it read x0 to x30 and
  sp, unused); `syscall` offsets, `codeChanges` and `traps` of plan 7.2's contract are not added, since the
  page gets the call's number and arguments from the hook and traps by `trapKinds` already.

## Decisions taken in the plan that the owner may want to hear about

- The AVX setting claims the whole x86-64-v3 level (AVX, AVX2, FMA, F16C, BMI2, XSAVE), not AVX alone.
- CPUID claims vendor GenuineIntel with the reference CPU's family and model, and a brand string of our own.
- FPU is claimed by default although x87 always runs in software (counted). That is the plan's reading
  of the brief's claim rule ("only features every instruction of which maps onto WebAssembly without a
  software slow path"), not its letter: x86-64 has no CPU without x87, the ABI puts `long double` there
  without asking CPUID, and the brief itself plans x87 as a counted slow path (plan 5.10). MMX is
  offered (baseline).
- Every x86 `call` to a non-canonical target writes the return address before its general protection,
  because the i7-8750H does so (Intel's pseudo-code says otherwise).
- 64-bit `div` and `idiv` with a full 128-bit dividend run an inline long division that is not counted as
  a slow path; its cost is measured at X3.
- `rcpps` and `rsqrtps` reproduce Intel's approximations exactly by a formula found on the i7-8750H
  (other CPUs are expected, not seen, to give other approximations; the reference decides).
- The lookup array costs four bytes of state memory per byte of x86 code.
- Switch tables with absolute entries (non-PIE code) and tables over the br_table limit go through the
  lookup, counted; revisited only if X9 shows them common.
- The legacy vsyscall page is not provided.
- `int 0x80` (the 32-bit system call gate) gives general protection, as a kernel without 32-bit
  emulation does, until 32-bit mode is built.
- Every x86 `ret` and every jump or call through a register or memory pays a canonical-address check.
  It is about four operations each.
- astro-float adds about 41 KB brotli to the x86 browser build (scratch measurement).
- `TrapKind` (and later the abi's `Exit`) grow divide error, general protection and float exception; the
  abi's `Exit` also gains single step (from the stepping of section 5.1, not a `TrapKind`), and it and the
  runners' memory faults gain x86's stack fault (SIGBUS), which the
  i7-8750H gives for a non-canonical access through rsp or rbp.
- Alignment checking (rflags AC) is not provided: `popf` leaves AC clear, as on a CPU without it, counted
  and warned. A real x86 Linux would fault misaligned accesses while AC is set.
- A far transfer to Linux's 32-bit code segment (selector 0x23) gives general protection, counted, until
  32-bit mode is built.
- `sysenter` also gives general protection (as `int 0x80`), while SEP is claimed: the instruction exists,
  the kernel has no 32-bit calls behind it, as on Linux without 32-bit emulation.
- Code written on the page that writes it costs one fault, one step and two translations per write (the
  faulting instruction runs once from an unprotected one-instruction piece), counted with "code written
  without announcement".
- Accesses that are not base plus scaled index (`bt` with a register offset, gathers, masked moves, 8-
  and 16-bit indexes such as `xlat`'s) are checked every time by their own meanings and never grouped or
  proven; expected to cost little, measured at X5 and X8.
- The kernel's copy routines get a measured model per CPU; x86's writes every byte up to the fault.
- Glibc programs without `BIND_NOW` fire the counter "MXCSR's exception flags read while not kept" at each
  lazy binding (the loader's `fxsave`), warning once; a state restore never moves a program off the fast
  path, only `ldmxcsr` or a non-default mode does.
- x86 instructions of features not claimed do what they do on a CPU without the feature: the reserved-NOP
  space (CET's `endbr64` and `rdssp`, `cldemote`, MPX, `prefetchw`) runs as NOPs, so CET-built programs run;
  the rest gives illegal instruction.
- Berkeley SoftFloat is not used for x87 precision control and `fsqrt`, though it does exactly that job:
  no crate wrapping it builds for these targets, and vendoring its C would bring C into the x86 crate. The
  plan keeps rustc_apfloat and astro-float with a little glue.
- At X6 the memory fault hook returns the unwind payload, which changes ARM64's output by one instruction
  per fault site (behaviour unchanged).
- x86 layer 1's code area is 64 KiB around the instruction, filled with `int3` on both sides; ARM64's stays
  256 MiB of zeros.
- A program's own trap flag is held and stepped exactly: each instruction then runs alone through the exact
  path, counted ("trap flag set by the program" and "left to address"). From X6 the stop hook returns the
  unwind payload, which changes ARM64's output inside each stop check's raised-flag branch (behaviour
  unchanged). One known difference: a gather faulting part way under TF gets no extra SIGTRAP first.
- SSE3 and CMOV are claimed although each holds a few x87 instructions (`fisttp`; `fcmov`, `fcomi`), which
  run in software like all x87, under the same reading as FPU.
- Layer 1 runs the never-claimed forms (EVEX, XOP, 3DNow and the rest) on the reference at 100 cases a
  form, not 1,000, since an instruction the CPU lacks stops before touching its operands.
