# Translator status

Plan: docs/phase-1-translator.md. Branch: phase-1-translator.

## For the orchestrator

- T3 (this milestone): every bar is met (see "T3"; the fourth run answered the independent check's
  findings): layers 1, 2 and 3 reran on the merged code with no mismatch, and again on the final code (fourth
  run); the awk count; the literal check passing with a smaller baseline (8,113 lines after the init at the
  milestone's start, 8,159 at the merge before it; 7,660 now), now checked with the branch's real history; the
  benchmark set's speed unchanged against 97fff482 (the merge) in wasmtime and Chrome. Against f6e760ae (the
  proof's code, before the merge) the independent check measured fib in Chrome 1.6 percent slower in 4 of 4
  rounds, with the translator's output for the benchmark set byte for byte the same at both: that comes from
  main's refactor of the test harness (runner.js, the browser translator), not from the translator; see the
  fourth run. So in Chrome the bar holds against the merge, not against f6e760ae. The fifth run answered the
  second independent check: a jump through a register into another part of a cut function is now counted.
- The P2 check's `wcontainers-abi` constants: this track added `PAGE_SHIFT` and `PAGE_MASK` to wcontainers-abi
  (06187cfd), as fix-translator.md says, but magic-literals/README.md gives that stage to the runtime track.
  They are defined exactly as the README's stage 1 writes them; when main's own arrive, keep main's lines in
  the merge. Taking them out here would add their literals back to the baseline, which the check refuses.
- The Chrome runner (runner.js, test tooling only) changes V8's speed on the translated code by up to 5 percent
  depending on how its `run` function takes its arguments (an extra destructured parameter, or a wrapper that
  reads a property inline, slows interpret by 4.6 percent; a module variable does not). Measured, not
  understood; recorded so nobody reads such a difference as the translator's.
- FPSR (the P2 check's first finding): a program whose code reads FPSR now has its flags kept exact from its
  start, decided by `wcontainers_arm64::reads_float_status(code)` and applied by whoever starts the thread
  (`FLOAT_TRACKING` set to 1 before it runs). This track's runners do it; the runtime's CPU side
  (wcontainers-wasmtime, not this track's) must call it for each program it starts, or a program that reads
  FPSR first still gets 0 there. Musl's dynamic C library holds `fetestexcept`, so applied to every loaded
  object it would make every dynamically linked musl program take the exact floating point path: the runtime
  should decide on the main executable and what it imports, and the owner may want to rule on that trade.
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

M0 to M5, S1, P1 and P2 are done (P2: the long run reached the plan's bars on 2026-11-19 13:52; see "P2").
T3 is done (pending its independent check): main merged (the translator on the shared guest interface), the
literal check adopted and the baseline down to 7,660 lines, the P2 check's findings answered, the register
jump slow path fixed (busybox awk: millions to single figures), FPSR read first fixed; layers 1 and 2 rerun in
full on the merged code with no mismatch, layer 3 to 5,287 compared with none, and all three again on the
final code with a new seed; the benchmark set's speed unchanged against the merge. The fourth and fifth runs
answered the independent checks' findings. See "T3" and "T3, what is left".

## T3: main's refactor, the slow path, the literal sweep

Done, in order (commits on phase-1-translator):
- 6a981a30 merged origin/main (the translator generic over `wcontainers-guest-cpu`, arm64 behind it). Two
  conflicts, both in the test crate: the runner's counters (main's reason numbers, kept in this branch's Fx
  maps) and its pc-only read at a stop; `programs::ran`'s statuses (main's new x86 faults, written with this
  branch's named signals). Nothing in the translator or arm64 crates conflicted.
- 65ba8f8a merged origin/phase-1-literals and ran its init once (8159 to 8113 lines on the merged tree).
- 97fff482 the P2 check's findings on the test harness:
  - Layer 2 now compares every program with the real CPU (finding 5). `native` runs blobs through one
    reference container per job (`wcontainers-reference blobs`: a request per blob on stdin, each blob in a
    child of its own with an empty stdin and pipes for its output, as in a container of its own), instead of
    a container per program. Measured on the rented ARM64 machine, 12 jobs, load 26 to 34: layer 2, 1,000
    programs, seed 900, was 28 s with 8 on the CPU and 70 s with all 1,000 on the CPU through a container
    each; through the server, all 1,000 on the CPU, 25 s. `WC_LAYER2_NATIVE` now defaults to every case and
    the driver sets it to every case.
  - Layer 3 replays exactly, apart from which programs end within Linux's 20 s alarm (finding 4): Chrome runs
    the first of every `cases / WC_LAYER3_CHROME` cases (when it ends in time), not the first to finish; the
    bounded N and mixing seed come from FxHash of the blob's bytes (rustc-hash, pinned by Cargo.lock), not
    std's DefaultHasher. Which programs are compared still depends on how fast the reference runs (a 20.2 s
    program on the Mac ended in time on the Graviton): that is the alarm's nature, recorded, not changed.
  - The rented machine's P2 ledger lines say `WC_LAYER3_JOBS=6` in their knobs although they ran at 12 or
    48 (their `ran=` field says so): those knobs were kept so the Mac's driver would skip the chunks.
- b4384452 busybox `awk`'s "jump through a register not recognised" (below).
- The FPSR fix and the literal steps 1 and 2 (below).

Busybox `awk` (Alpine 3.22, ARM64, run by `wcon` at full look-ahead on the rented x86 machine, which only
translates and runs WebAssembly; exact counts by target from a scratch counting patch on the job's copy,
`translator/t3/detail/apply.py`; commands in `translator/t3/awk.sh`). What it did not recognise:
- Calls through the procedure linkage table: each stub ends `br x17` into another module (`free`, `malloc`,
  `memset` and so on in musl). A jump through a register to outside its own function is a tail call; it now
  goes through the lookup like a call through a register, with no warning. Only a jump to inside its own
  function (a switch table not recognised) is the slow path, counted and warned (a range check on the target).
- awk's evaluator's switch, whose table address the function computes once and keeps in a stack slot
  (`str x0, [sp, #0x30]`, later `ldr x1, [sp, #0x30]`): the switch habit now follows a 64 bit load from
  `[sp, #n]` back to the nearest store to that slot with sp unchanged between (`habits::kept_in_frame`).
- Cases of a recognised switch that lie in another piece of a function cut in parts: the jump goes to them as
  a tail call through the lookup, with no warning; other targets still take the counted path.
- Counts, before (97fff482) and after (both changes), by the exact count:
  `seq 1 100000 | awk '{s+=$1} END {print s}'` 5,306,745 to 5; `awk 'BEGIN {for (i = 0; i < 100000; i++) s
  += length(sprintf("%d", i)); print s}'` 8,601,492 to 0; `seq 1 20000 | awk '{a[$1 % 97] = a[$1 % 97] $1}
  END {...}'` 2,621,222 to 4. Outputs unchanged. (wcon's own summary prints a counter's value when it last
  warned, so it shows slightly different figures, such as 5,306,726.)
- Test: `programs::only_jumps_inside_their_function_are_unrecognised` (`programs/register_jumps.S`: a tail
  call through a register, a jump into its own function, and a switch whose table address is kept in the
  frame, 1,200 rounds each): the count must equal the rounds of the jump inside. It failed on the code
  before each of the two changes and passes after (rented ARM64 machine).

FPSR read first (the P2 check's first finding): see "For the orchestrator". New test
`programs::float_status_read_first_matches_linux` (`programs/float_status.c`: flags read through mrs and
fetestexcept after 1/0, overflow, underflow and sqrt(-1), never cleared; every level, every look-ahead, and
Chrome).

Reruns on the merged code (commit 97fff482, whose translator and arm64 crates are the merge's):
- Layer 2, rented ARM64 machine, 12 jobs, load 26 to 32: 100,000 programs, seed 300 (four runs of 25,000 from
  cases 0, 25,000, 50,000 and 75,000), every one also on the real CPU, 5 a run in Chrome: 0 failing; 34,217
  faulted, 7,450 left to an address at full; 580 to 638 s a run. Same as before the merge: no mismatch.
- Layer 3, same machine and job, seed 300, chunks of 500 from case 0, 5 a chunk in Chrome: chunks 0 to
  11 (cases 0 to 5,999) passed: 5,287 compared (52 also in Chrome), 713 did not end within Linux's 20 s alarm,
  0 failing (392 to 3,270 s a chunk, load 7 to 42); ended 2026-11-20 04:28 UTC. Log: arm-run job translator/20261120-010956-2486238 (`arm-run --fetch translator
  /work/jobs/translator/20261120-010956-2486238/log <dir>`), each chunk's output in the build directory's
  `t3/layer3-<k>.log`.
- Layer 1, the Mac, full bar (10,000 cases a form, 100 a form in Chrome), seed 300, 4 jobs, load 5 to 20: the
  driver (`WC_LONG_DIR=.../translator/t3/layer1 WC_LONG_SEED=300 WC_LONG_LAYERS=1 WC_LONG_JOBS=4
  WC_LONG_LIMIT=180`, on 65ba8f8a) passed all 10 chunks: 12,150,000 cases (121,500 in Chrome), 0 mismatches,
  854 to 970 s a chunk; ledger `translator/t3/layer1/ledger.txt`. Same verdicts as before the merge.

Speed pairs (benchmark set, full look-ahead, every check, best of 3, wasmtime and Chrome; 97fff482's test
binary against this branch's, alternating, order flipped each round, 4 rounds; rented ARM64 machine, load 22
to 32; script `translator/t3/pairs.sh`, log `t3/pairs-1.log`, summary by `t3/pairs-summary.py`): every
workload in wasmtime and every one in Chrome but one within 0.2 percent (new over old 0.999 to 1.002).
Interpret in Chrome is 4.8 percent slower in every round (2.35 s against 2.46 s). Bisected: the translator's
output is byte for byte the same (the benchmark set and thread module, `tests/output.rs`, at b4384452 and
after), and each of the three translator changes removed alone leaves it slow, while b4384452 is fast; the
cause is the two-line change to the test runner's `runner.js` in 06187cfd (the `keepFloatFlags` parameter and
its store at the thread's start), which runs once. Why V8 runs the translated interpreter slower for it is not
known; keeping the flag outside `run`'s scope did not help.

Second run of T3 (2026-11-19 19:36 to 20:30 local):
- The Chrome interpret regression: 4fe2dd6e gives `run` the thread's start as one object (`start.sp`,
  `start.keepFloatFlags`), so its destructured parameter list has its old length. Why V8 cared is still not
  known (runner.js variants with one binary, `t3/js-variants-2.log`: swapping two parameters, or passing the
  flag through a wrapper, were as fast as before; only the added parameter was slow).
- Speed pairs again, 55fc5e54 (the start object and this run's literal edits) against 97fff482 (whose old
  side was rebuilt from a clean copy, `t3/old-97fff482`, with only 7f8df4cf's load reading added so it runs on
  Linux), 4 alternating rounds, rented ARM64 machine, load 10 to 39, `t3/pairs-4.log`; translator output for
  the benchmark set and thread module byte for byte the same as before (`OUTPUT SAME` in that log). New over
  old: wasmtime 0.999 to 1.004 on every workload; Chrome 0.998 to 1.002 on sieve, hash, interpret (2.352 s
  against 2.354 s: fixed), matrix, search; but fib 1.007 (slower in all 4 rounds, 4.886 to 4.901 s against
  4.852 to 4.859 s), sort 1.010 (slower in 3 of 4) and walk 1.005 (slower in 3 of 4) in Chrome.
  With one binary (55fc5e54's) and runner.js as committed or without the store of the starting flag
  (`t3/js-variants3.sh`, 4 alternating rounds, load 9 to 16, `t3/js-variants-3.log`), fib, sort and walk in
  Chrome take the same time (fib 4.880 to 4.887 s against 4.881 to 4.896 s; sort 3.261 to 3.267 against
  3.263 to 3.264): the store costs nothing, and what is slower than 97fff482 is the new shape of `run`'s
  parameters, as with interpret before. So the speed bar is not met in Chrome by 0.5 to 1 percent on three
  workloads. Next to try: leave `run`'s parameter list exactly as at 97fff482 (with `sp`) and read the flag
  in a wrapper, `function run(setup) { return runWith(setup, setup.keepFloatFlags); }` (with the old binary
  that wrapper kept interpret at its old speed, `t3/js-variants-2.log`, variant "late"), then pair all eight
  workloads again.
- busybox awk on the final code (47cb2450), `wcon` at full look-ahead on the rented x86 machine, load 25 to 27,
  `t3/awk-final.log`: outputs as before; wcon's summary (the counter when it last warned) 4, none and 3 for
  the three commands, as at b4384452.
- Literals: 55fc5e54 (look-ahead codes and the description's version named once in the translator and used by
  the test side; the frame and per-function estimates named as tuning numbers; the permission table's eight
  pages a load and repeated byte derived; the thread module's labels and locals derived; the register set's
  bit count from `Registers`), 47cb2450 (the stand-in's error numbers, `UIO_MAXIOV` and `iovec` and layer
  1's signals from linux-raw-sys; see "Departures"). 4fe2dd6e had removed three "sp" literals without their
  baseline lines; they went with 55fc5e54. Baseline 7,944 lines at the start of this run, 7,906 now (arm64
  1,375, translator-test 1,360, translator 7: its six `Means::name` texts, which the fix list rules are not
  violations, and the stack alignment; guest-cpu 28, shared, left as it is). The check passes (rented x86 machine,
  `t3/lint.sh`).
- Layer 1's tests on the Mac on 47cb2450 (`cargo test -j 4 -p wcontainers-translator-test --test layer1 --
  --test-threads 3`, load 5 to 6, 210 s, `t3/suite/mac-layer1.log`): 7 passed.
- The whole suite of the four crates on 47cb2450, debug, `cargo test -j 12 --no-fail-fast -p wcontainers-arm64
  -p wcontainers-translator -p wcontainers-translator-test -p wcontainers-guest-cpu`, rented ARM64 machine
  (arm-run name translator-suite, 04:05 to 04:31 UTC, load 10 to 22 on 96 cores,
  `t3/suite/arm-suite-47cb2450.log`): 97 passed, 10 ignored (marked so in the code), 2 failed, both layer 1's
  and both `MSR_SR_systemmove` (the Graviton's FPSR keeps bits 28 and 29: the known reference difference;
  layer 1's verdict is the Mac's, above: 7 passed). So every test passes where its verdict counts.

Third run of T3 (2026-11-19 20:30 to 22:00 local):
- Chrome's speed: runner.js again takes the setup exactly as at 97fff482 (with `sp`), and the starting float
  flag comes through a module variable set from the message (5c08f25b). Variants with one test binary, 3
  alternating rounds, rented ARM64 machine, load 9 to 14 (`t3/js-variants-4.log`, `t3/js-variants-5.log`):
  interpret in Chrome 2.353 to 2.356 s for 97fff482's runner.js, the module variable and a wrapper with a
  destructuring `const`, but 2.462 to 2.465 s for a wrapper reading `setup.keepFloatFlags` inline; fib, sort
  and walk the same in all.
- Speed pairs, 5c08f25b's test binary against 97fff482's, alternating, 4 rounds, rented ARM64 machine, load 6
  to 18 (`t3/pairs-6.log`): new over old in Chrome 0.999 to 1.001 on every workload but walk, 0.974 (faster);
  in wasmtime 0.998 to 1.001 on every workload but fib, 1.006, from one round (4.907 s against 4.802 s; the
  other three within 3 ms). Fib alone again, 6 rounds, load 5 to 9 (`t3/pairs-7-fib.log`): wasmtime 1.000
  (new faster in 4 of 6), Chrome 0.999. Translator output for the benchmark set and busybox byte for byte the
  same as at 91616539 (`tests/output.rs` on the rented x86 machine, lists in `t3/out`). It is not the same as
  97fff482's (corrected in the fifth run, from the independent check's lists): b4384452 put a range check at
  every unrecognised jump site, so four of the benchmark set's modules differ (the benchmark program and its
  WORKLOAD5 build, full, for speed and for size: 295,182 to 295,230, 124,539 to 124,571, 64,800 to 64,848 and
  35,076 to 35,108 bytes). These pairs compared that changed code with 97fff482's, not identical code.
- Literals, fix list steps 6, 13, 16, 21 and 22 in the arm64 crate, each output-neutral: c3cf197e (float byte
  sizes from `size_of`, the formats' fraction and exponent widths from rustc_apfloat, the float hook's operand
  count named once and its parameter list built from it), c94a9289 (the float hook's operation code as a
  struct of named bytes, `u32::from_le_bytes`, its kinds a `strum::FromRepr` enum, its flag bits from the
  rounding table's length; new test `every_operation_code_decodes_to_its_operation`), d1f3a5fb (the scalar
  reciprocal estimates' widths and biases derived from the formats, Arm's estimate width and the square root
  estimate's three biases named; new against old on 50,000,000 random singles and doubles, equal), 38c86b88
  (AdvSIMD register and element sizes), 2bbbf961 (the vector estimate tables from the same names, and one
  `width` instead of two). Baseline 7,906 to 7,660 lines (arm64 1,129). New dependency: strum 0.27.2 (MIT),
  in wcontainers-arm64's `translate` feature; it was already in Cargo.lock through another crate.
- Layer 1 on the Mac (load 5 to 7): the 12 estimate forms (FRECPE, FRSQRTE, FRECPX, URECPE, URSQRTE, all
  shapes) at 20,000 cases each, seed 301, 20 each in Chrome: passed (`t3/layer1-estimates.log`, 206 s);
  layer 1's seven tests on 5c08f25b: 7 passed (223 s, `t3/suite/mac-layer1-5c08f25b.log`).
- The whole suite of the four crates on 5c08f25b, debug, as in the second run, rented ARM64 machine (05:25 to
  05:49 UTC, load 5 to 41, `t3/suite/arm-suite-5c08f25b.log`): 98 passed, 10 ignored, 2 failed, both layer 1's
  `MSR_SR_systemmove` (the Graviton's FPSR difference); layer 1's verdict is the Mac's, above: 7 passed.

Fourth run of T3 (2026-11-19 23:00 local to 2026-11-20), the independent check's findings:
- The other hosts test (blocking): since 06187cfd differences.c reads FPSR, so on a host without fused
  multiply-add and Arm's not-a-numbers it ran only the exact path. Building it without FPSR reads showed why
  no C test program could avoid them: zig 0.16 links C library code nothing calls (`hypotl`, which calls
  `fmal`, which calls musl's fenv functions), so every C program was taken as reading FPSR. 59cb1416 links C
  test programs with `--gc-sections` (one named option in `programs.rs`; `flat.ld` keeps libc++'s override
  section), gives differences.c a `WITHOUT_FENV` build, and the test runs both builds: the plain one on the
  exact path as before, the other with "software fused multiply-add" counted and Linux's output, which
  covers the fix-up on the fast path. (A first try asserted "left to an address" at full for both; the
  software fused multiply-add is called in place, so that holds only on the exact path.)
- A jump through a register to its own function's entry is a tail call to itself, no slow path (c2bafa58: the
  range check also tests the target against the entry, in a function's own piece only, not a loop piece).
  `register_jumps.S` gained three such calls a round: the test fails without the change (4,800 counted
  against the 1,200 expected) and passes with it. The check's program `check_calls.c` (tail calls through a
  table of three functions, -O2, full): 0 counted (11,752 at 48c65565, as the check measured). busybox awk,
  `wcon` on the rented x86 machine, load 26 to 28 (`t3/awk-r4.log`): outputs as before; wcon's summary 3,
  none and 2 for the three commands (4, none and 3 before). The single figures vary from run to run (the
  independent check, at the same code: exact counts 2, 0 and 4, summary 1, none and 3; b4384452: exact 5, 0
  and 4), so the remainder is a range: exact 2 to 5, 0, and 4; summary 1 to 4, none, 2 to 3.
- Switch cases in another part of a cut function (b4384452's third part) now have a test (486ee285,
  `programs/cut_switch.c`: 48 cases of 64 branches each, every case twice, -O0 and -O2, full for speed and
  size): no slow path, Linux's output. The check's `check_big.c` gives 0 counted and still 1,622 translations
  at run time: see "Known gaps".
- The plan now has "Two workarounds for what WebAssembly lacks" (4117dcd3), as the brief asks.
- The literal check now runs with the branch's real history (`t3/lint.sh` sends the branch as a git bundle to
  the rented x86 machine and the copy's HEAD is put at the branch's tip): "15 changes on this branch's
  first-parent history; growth is checked", status 0, on this run's commits. The baseline is 7,660 lines; it
  was 8,159 at the merge (fc5f55ca) and 8,113 after the init (65ba8f8a), not 7,944 as the third run wrote
  (that was the second run's start).
- The translator's output (tests/output.rs on the rented x86 machine, `t3/r4/out`) against the third run's:
  busybox the same; the benchmark set differs in four modules, from the new entry test at a jump through a
  register: the benchmark program and its WORKLOAD5 build, full, each for speed (295,230 to 295,240 and 64,848
  to 64,858 bytes) and for size (124,571 to 124,591 and 35,108 to 35,128).
- Which commit each rerun proves. Merged code: layer 2, 100,000 programs, and layer 3, 5,287 compared, at
  97fff482; layer 1, 10,000 cases a form, at 65ba8f8a (see above). Final code, this run, at 4117dcd3 (the
  translator as at c2bafa58), new seed 400:
  - Layer 1 on the Mac, 1,000 cases a form (1,215 forms) and 10 a form in Chrome, 4 jobs, load 6 to 10, 866 s:
    0 failing; the words in no form, 1,000: passed (`t3/r4/layer1-mac.log`).
  - Layer 2 on the rented ARM64 machine, 25,000 programs, every one on the real CPU, 5 in Chrome, 12 jobs, load
    10 to 17, 606 s: 0 failing (8,329 faulted, 1,781 left to an address at full) (`t3/r4/rerun.log`).
  - Layer 3 on the same machine, 1,000 programs from case 0, 100 in Chrome, 12 jobs, load 14 to 19, 838 s: 866
    compared (82 also in Chrome), 134 did not end in time on Linux, 0 failing.
  - Layer 3 again, 4,500 programs from case 1,000, 45 in Chrome, 12 jobs, load 10 to 18, 5,537 s: 3,937
    compared (42 also in Chrome), 563 did not end in time on Linux, 0 failing (`t3/r4/layer3b.log`). With the
    1,000 before: 4,803 compared on the final code, 0 failing.
- The whole suite of the four crates at 4117dcd3, debug, rented ARM64 machine, 07:18 to 07:43 UTC, load 7 to
  20 (`t3/r4/suite.log`): 99 passed, 10 ignored, 2 failed, both layer 1's `MSR_SR_systemmove` (the Graviton's
  FPSR difference); layer 1's verdict is the Mac's, above.
- Speed pairs over the whole milestone: the proof's code (f6e760ae, the check's tree with only its
  `/proc/loadavg` reading added, built as translator-pre) against 4117dcd3, 4 rounds, order rotated, rented
  ARM64 machine, load 10 to 46 (`t3/r4/pairs.log`, script `t3/r4/pairs.sh`). Head over pre: wasmtime 0.993
  to 1.002 on every workload; Chrome 0.999 to 1.002 on sieve, hash, sort, interpret, walk and search, but fib
  1.017 (4.861 s against 4.781 s, slower in 4 of 4) and matrix 1.010 (0.296 s against 0.293 s, slower in 4 of
  4). The fib step is the merge's: the check measured the merge (97fff482) at the same 1.6 percent over
  f6e760ae with the translator's output for the benchmark set byte for byte the same at both, so it comes
  from main's test harness in Chrome (runner.js and the browser translator page), not from translated code;
  against the merge, earlier runs measured every workload within 0.2 percent. Not found what in the harness
  V8 is sensitive to.
- Not changed, recorded: the test harness takes the guest's Linux numbers from linux-raw-sys of the host (see
  "Departures"), against the README's hard case 2, until a crate with the guest's own Linux numbers exists.

Fifth run of T3 (2026-11-20 02:00 to 04:00 local), the second independent check's findings:
- A jump through a register into the middle of another part of a cut function (finding 3) was taken as a tail
  call: the range check used the piece's own offsets. Now each part knows the function it was cut from
  (`Whole`: its entry and span, recorded where the cut entries are made) and the range check and the entry
  test use that function, in its parts and its loop pieces alike (e26b3c23). `register_jumps.S` gained `far`,
  a function of `FUNCTION_BLOCKS` and then `FUNCTION_BLOCKS / 8` blocks (so cut in two) that jumps through a
  register into its second part: the test expects 2 x 1,200 counted; without the change it fails (1,200),
  with it it passes (rented ARM64 machine). `FUNCTION_BLOCKS` is now public so the test can import it.
- Looking into the 1,622 translations at run time of the check's `check_big.c` showed that the cause is not
  what the fourth run wrote: the switch has no bound check, so it is never recognised (see "Known gaps").
  `cut_switch.c`'s `cut` was static and had the same shape at -O2, so at -O2 the test did not reach a cut
  function at all (1,190 translations at run time, 0 counted). `cut` is no longer static; the compiler then
  checks the bound, the function is cut and its cases in other parts are entries: 3 translations at run
  time (the C library's), 0 counted, Linux's output, at -O0 and -O2, for speed and for size.
- The record (findings 1 and 4) corrected in 12d1ecb7: the third run's pairs did not compare identical code
  with 97fff482 (four modules differ, from b4384452); the fourth run's output change was in four modules, not
  one; the awk remainder is a range.
- The speed bar against f6e760ae (finding 2): the check measured, head over f6e760ae, fib in Chrome 1.017,
  search in Chrome 1.004 (slower in 4 of 4, also 1.004 against the merge) and interpret in wasmtime 1.003
  (slower in 4 of 4); every other workload 0.994 to 1.002. As the fourth run says, the fib step is the
  merge's test harness, not the translated code; the bar holds against the merge, not against f6e760ae in
  Chrome.
- The translator's output (tests/output.rs, rented x86 machine, `t3/r5/out-r5`) against the fourth run's:
  busybox the same; the benchmark set differs in two modules, full for speed (295,240 to 295,260 and 64,858 to
  64,878 bytes: two loop pieces' unrecognised jump sites gain the entry test); full for size the same.
- busybox awk on e26b3c23 (`wcon`, rented x86 machine, load 29 to 44, `t3/r5/awk-r5.log`): outputs as before;
  wcon's summary 3, none and 3, within the range above.
- Speed pairs, e26b3c23 against the code before it (the rented machine's copy of 4117dcd3, whose translator is
  c2bafa58's, the same as e3254f67's), 4 rounds, order rotated, rented ARM64 machine, load 4 to 13,
  `t3/r5/pairs-rerun.log`, script `t3/r5/pairs.sh` (it now builds each side's benchmarks binary and runs the
  one cargo names, not the first in the folder). New over old: wasmtime 1.000 to 1.002 on every workload but
  matrix, 1.005; Chrome 0.999 to 1.002 on every workload but matrix, 1.010. Matrix's means come from single
  rounds (Chrome 0.305 against 0.293 s in round 1, the other three rounds equal to the millisecond; wasmtime
  0.485 against 0.480 s in rounds 2 and 3, equal in 1 and 4). Matrix alone, 6 rounds, same sides,
  load 6 to 13 (`t3/r5/matrix-suite.log`): wasmtime 1.000, Chrome 0.999, every round within 5 ms (wasmtime) and
  1 ms (Chrome) of the other side. So unchanged on every workload in both engines.
- Reruns on e26b3c23, new seed 500:
  - Layer 1 on the Mac, 1,000 cases a form and 10 a form in Chrome, 4 jobs, load 2 to 9, 838 s: 7 passed
    (`t3/r5/layer1-mac.log`).
- The whole suite of the four crates at e26b3c23 (the code as committed now), debug, rented ARM64 machine,
  11:14 to 11:38 UTC, load 7 to 13 (`t3/r5/matrix-suite.log`): 99 passed, 10 ignored, 2 failed, both layer 1's
  (`sample_matches_the_cpu_in_chrome`, `every_covered_form_matches_the_cpu`), whose verdict is the Mac's
  (above: 7 passed). The log kept only test names, so the forms that failed there were not seen this time;
  in the second to fourth runs they were `MSR_SR_systemmove` (the Graviton's FPSR keeps bits 28 and 29).
  - Layer 2 on the rented ARM64 machine, 25,000 programs, every one on the real CPU, 5 in Chrome, 12 jobs, load
    13 to 19, 601 s: 0 failing (8,600 faulted, 1,921 left to an address at full) (`t3/r5/pairs-rerun.log`).
  - Layer 3 on the same machine, 1,000 programs from case 0, 100 in Chrome, 12 jobs, load 8 to 19, 917 s: 868
    compared (86 also in Chrome), 132 did not end in time on Linux, 0 failing (same log).

T3, what is left:
- For the independent check: layers 1 and 2 and layer 3 with a seed other than 300, 400 and 500 (layer 1 on
  the Mac: `WC_LAYER1_CASES WC_LAYER1_CHROME WC_LAYER1_SEED`; layers 2 and 3 on the ARM64 machine, as in "How
  to run"), the awk counts (`t3/awk.sh` on the x86 machine after building `wcon`; exact counts need the
  counting patch `t3/detail/apply.py`), and the pairs (`t3/r5/pairs.sh 4 <label>:<source>:<target> ...`,
  uploaded to the ARM64 machine, each side's tree and build directory named; it builds each side's binary).
- Beyond the milestone's bar: recognise a switch with no bound check from the index's range in the code
  before it (see "Known gaps"; the check's `check_big.c` is the measure, 1,622 translations at run time).
- Beyond the milestone's bar, the literal fix list goes on (arm64's decoders, fix list steps 17 to 20, need the
  arm-forms generator to emit Arm's field and name constants), with the open homes named under "Departures":
  ARM64 system call and auxiliary vector numbers, the stack alignment, runner.js's look-ahead codes.

The P2 check's other findings, answered here: the Graviton's DC ZVA fault address (finding 2) is under "Known
gaps"; the square root forms at 7,000 cases on the final code (finding 3) are covered by this milestone's
layer 1 rerun at 10,000 cases a form on the merged code, and P2's headline is now worded as mixing the two
commits; the loosened layer 3 guard (finding 6) needs no change.

## P2: the long runs to the pass bars

The P1 check's findings, each fixed or answered:
- A chunk stopped at the time limit or failing was never run again on that commit. Now only a chunk that
  passed on an accepted commit is skipped; one that failed or was stopped runs again on the next start (its
  old line stays in the ledger, so a retry is visible). Checked on a made-up ledger (pass, fail, stopped,
  pass on another commit: only the first held) and by a real run with `WC_LONG_LIMIT=1` (4 chunks stopped,
  no container left).
- Layer 3's bar is now counted in programs compared with Linux, not programs made: the driver runs chunks
  until the programs compared reach the bar (at most twice the chunks the bar would take if every program
  were compared, so a broken reference cannot run for ever). The ledger's `cases` for layer 3 is the programs
  compared; `sample` gives those also in Chrome, those made and those that did not end on Linux in time.
- Layer 3's chunks are 500 programs, not 1,000, so a chunk stays well inside the 90 minute limit at high
  load (the check measured 5.1 s a program at load 55 to 113: about 43 minutes for 500, extrapolated).
- One unreadable unwind entry (a bad CIE pointer) ended all reading of `.eh_frame`. Now the entries are walked
  by their lengths (gimli's reader) and each function's entry parsed alone (`fde_from_offset`), so a bad one
  is skipped, counted and warned; reading stops only where a length cannot be read. New test
  `a_bad_unwind_entry_loses_only_itself` (musl with one CIE pointer broken: failed before with 11 of 22
  starts, passes now). On seven good binaries (busybox, musl, rg, git, python, go, java) the starts found are
  the same as before, means by means.
- Finding 6 of the M5 check (the two lookup designs' memory as process memory): not measured, on purpose. The
  old design cannot hold Node or Bun in Chrome (over Chrome's 10,000,000 table entries), so no memory figure
  could change the choice; the old design is gone from the tree, and bringing it back from 0d312ab to run Node
  would take the cores the long runs hold for a figure with no use.
- `origin/phase-1` moved to df81eff ten seconds after P1's merge; among the shared and translator files it
  differs from this branch by one line of `Cargo.lock`. Not merged again (nothing needs it); for the
  orchestrator.
- "How to run" for layer 3 brought up to date.

The driver also changed in one more way: while it ran, each chunk's `cargo test` built whatever was in the
checkout, and the tests build the reference runner and the browser translator from the working tree, so work
on a fix in the checkout would have run in the next chunk under the old commit. Now the driver refuses a tree
with uncommitted changes, exports the commit (`git archive`) to `WC_LONG_DIR/trees/<commit>`, and builds and
runs there (target directory `WC_LONG_DIR/target`, shared by every copy). To run a fix, stop the driver,
commit, and start it again with `WC_LONG_ACCEPT`. Checked: layer 1 at 1 percent from the copy (seed 31,
commit a31bb6f) passed, 121,500 cases and 1,215 in Chrome, 70 s, load 10 to 11; the copy built its own
reference runner and browser translator.

The run (started 2026-11-16 15:57, commit c38b4ad, base seed 100, all three layers at 100 percent, 6 at once):
- Directory: `/Users/thomaseaton/Documents/GitHub/pyro-parked/wcontainers-scratch/translator/p2/long`
  (`ledger.txt`, `driver.log`, `logs/`, `trees/`, `target/`).
- Alive? `pgrep -fl long-runs.sh` and look for the line `bash .../long-runs.sh` (any shell whose command
  text holds the name matches too, a watcher's included; never start a second driver beside it). What it is doing: `tail -3
  .../p2/long/driver.log`. Done so far: `cut -f1,2,4,6,7,10 .../p2/long/ledger.txt`.
- Restart after a stop or a fix (from the checkout root, all committed):
  `WC_LONG_DIR=/Users/thomaseaton/Documents/GitHub/pyro-parked/wcontainers-scratch/translator/p2/long WC_LONG_SEED=100 WC_LONG_ACCEPT=c38b4ad WC_LONG_LIMIT=180 nohup nix develop -c crates/wcontainers-translator-test/long-runs.sh >> .../p2/long/driver.log 2>&1 &`
  It skips every chunk that passed on c38b4ad or later and runs the rest. To stop it: `pkill -f long-runs.sh`,
  then stop its `cargo test` and remove its containers (`docker ps -aq --filter name=wc-translator- | xargs
  docker rm -f`).
- Restarted 2026-11-16 16:44 on 92b0866 (the square root fix below) with `WC_LONG_ACCEPT=c38b4ad`: layer 1
  chunks 1, 2 and 4 kept from c38b4ad, chunk 3 run again. Chunk 5 had started on c38b4ad and was stopped
  (no ledger line). Restarted again 2026-11-16 22:59 on 445e3cf (the layer 2 generator fix), same command;
  it was stopped 2026-11-17 20:34 after layer 3 chunk 55 was stopped at the limit (failure 3 below).
  Restarted 21:21 on de8f4a2 with `WC_LONG_LIMIT=180` added (a chunk may take 3 hours); chunk 55 runs again
  first. Stopped 2026-11-18 01:27 with chunk 57 hung (failure 4 below) and restarted 01:29 on 7ec7920 (the
  run's own zig cache), same command. Stopped 04:32 after chunk 71 and restarted on 33afa1e (failure 5
  below), same command. Stopped 08:12 after chunk 85 and restarted on 9d54ef3 (failure 6 below), same
  command. Stopped 14:58 after chunk 114 (chunk 115 had just started, no line) and restarted on 6b74146
  (failure 7 below), same command, chunk 113 first. Stopped 18:50 during chunk 127 (no line) after chunk
  126 failed on Docker's outage, and restarted 18:51 on 99eda5e (failure 8 below), same command. Stopped
  21:16 after chunk 135 (chunk 136 had just started, no line) and restarted 21:17 on 6a705f8 (failure 9
  below), same command, chunk 133 first. Stopped 2026-11-19 02:07 during chunk 153 (no line): Docker's
  Linux machine stopped again at 02:05 ("the virtualization process exited without being asked to"), and
  this time Docker Desktop did not come back by itself but showed its error (offering Quit or Reset).
  Docker is shared, so it was left alone and the driver stopped, rather than let it run through chunks
  that can only fail. See "Docker outages" below for what to do. To stop it right after a chunk ends: the scratch script
  `.../p2/stop-after-chunk.sh` (waits up to 9 minutes for a ledger line, then stops the driver, its test
  and containers). One look at the progress, waiting up to 9 minutes for a new line: `.../p2/look.sh`;
  `.../p2/watch.sh` waits 9.5 minutes, or less if a chunk does not pass, the driver ends or Docker stops
  answering, and also shows how long the running chunk's log has been quiet. Use `WC_LONG_ACCEPT=c38b4ad
  WC_LONG_LIMIT=180` on any later restart too. Each restart appends to `driver.log`; the code it runs is the copy in
  `trees/<commit>`, so commits in the checkout do not reach it until it is restarted.
- Docker outages: Docker's Linux machine stopped on its own three times (2026-11-18 18:14, back by
  18:18; 2026-11-19 02:05, back at 02:39 when someone started it again; 02:52, still down at 03:15). The
  VM's logs end with no reason given and the host showed no memory pressure; the cause is not known and
  is outside this track. `resume.sh` restarted the driver at 02:42 on e8677a0 (only this file changed
  since 6a705f8) and stopped it at 02:53 (chunk 153 again, no line). Since 99eda5e a case whose container Docker could not run fails
  with "Docker failed" and is never compared, so an outage fails a chunk; it does not pass a wrong one.
  When `docker version` answers again: check no `wc-translator-` container is left, then restart the
  driver with the command above (it runs the failed or stopped chunks again first). Do not quit, restart
  or reset Docker Desktop from this track: other tracks' containers run in it.
- The scratch script `.../p2/resume2.sh` does that by itself for both the long run and the confirmation run,
  for a day from its start (2026-11-19 12:48, log `.../p2/resume.log`): when Docker has not answered for a
  minute it stops each driver with its whole process tree, and when Docker has answered for two minutes it
  starts each stopped driver again with its restart command (the long run's above, the confirmation's under
  "Confirmation run"; not on a checkout with uncommitted changes); a run is left alone once its driver logs
  "long runs ended". It never touches Docker. It replaces `resume.sh` (02:29 to 12:47), which watched the
  long run only and, on stopping it, would also have killed the confirmation run's test by name. Is it
  running: `pgrep -fl '^bash resume2\.sh$'`. Before stopping a driver by hand, stop it too
  (`stop-after-chunk.sh` does so first), or it starts the driver again two minutes later. Match the driver as `pgrep -f '^bash [^ ]*/long-runs\.sh$'`: the
  looser pattern above also matches any shell whose command text holds the name.
- Rented ARM64 machine (AWS Graviton4, Linux 7.0.0-1017-aws; manual in the orchestrator's scratch
  `cloud/arm-machine.md`): from 2026-11-19 03:00 a separate agent of the orchestrator ran layer 3 chunks
  there from the top down, at the driver's commit e8677a0, and appended passed ones to the same ledger in
  the driver's format with an 11th field `ran=rented ARM64 Linux machine ...` (the driver skips them; their
  `seconds` and `load` are that machine's). Its notes, logs and table: scratch `translator/p2/arm`
  (`README.md`, `chunks.tsv`). It first ran chunk 152 there with the Mac's verdicts in every field (not in
  the ledger). In the ledger from there: chunks 198, 199 and 200 (441, 448 and 426 compared, 0 mismatches).
  Chunk 197 was lost when the spot machine was taken away at 04:08 (no ledger line; the driver runs it when
  it gets there). A second machine came up about 05:00 and that agent's loop went on from chunk 197 down,
  at the driver's commit since 05:01 (398df57).
- Chunks above 200 on the rented machine, run by this track: from 06:21, scratch `translator/p2/arm-up`
  (`up.sh`, `run-chunk.sh`: the top-down agent's chunk script with the track name `translator` and its own
  logs, `up.log`, `chunks.tsv`, `logs/`) runs chunks 201 upward under arm-run name `translator` at 12 jobs,
  from its own export of 398df57 (identical to the other agent's), with the same ledger format and mark. It
  stops when the programs compared, counting each chunk up to 200 not yet passed at 440, reach 100,000, or
  when the Mac's driver has reached its next chunk. Its zig cache was warmed first (24 cases, seed 9, one
  job, passed, not counted). Running: `pgrep -fl '^bash up.sh$'`. All three runners at once:
  `.../p2/w3.sh` (waits like `watch.sh`).
- The second machine went away at 08:48 during chunks 181 (top-down) and 212 (upward), both lost with no
  ledger line. By then chunks 182 to 197 and 201 to 211 had passed there. `arm-up/up-resume.sh` (detached,
  20 hours from 08:52, log `up-resume.log`) warms the cache and starts `up.sh` again when a machine answers;
  the top-down loop is the other agent's.
- Confirmation run, layers 1 and 2, started 2026-11-19 09:30 on the Mac beside the long run, on 1f9c030
  (its crates are 6a705f8's; a later fix would mean running it again): `CARGO_BUILD_JOBS=4
  WC_LONG_DIR=.../p2/confirm WC_LONG_PERCENT=5 WC_LONG_SEED=200 WC_LONG_LAYERS=1,2 WC_LONG_JOBS=3
  WC_LONG_LIMIT=180` with the driver command above (3 jobs: what the Mac allows beside the long run's 6).
  Layers 1 and 2 passed (607,500 cases, 825 s; 2 x 25,000 programs, 1,517 and 1,539 s; load 44 to 89).
  Layer 3 of it started 10:44 on 8c7a4b5 (status-only change since 1f9c030) after its cache was warmed
  (`confirm/warm.log`), same command with `WC_LONG_LAYERS=3` and no `WC_LONG_ACCEPT` (so it accepts 8c7a4b5
  on; a restart needs `WC_LONG_ACCEPT=8c7a4b5`; the restart command, from the checkout root:
  `CARGO_BUILD_JOBS=4 WC_LONG_DIR=.../p2/confirm WC_LONG_PERCENT=5 WC_LONG_SEED=200 WC_LONG_LAYERS=3
  WC_LONG_JOBS=3 WC_LONG_LIMIT=180 WC_LONG_ACCEPT=8c7a4b5 nohup nix develop -c
  crates/wcontainers-translator-test/long-runs.sh >> .../p2/confirm/driver.log 2>&1 &`): about 37 minutes a chunk at 3 jobs, load 19 to 49. Its
  chunks from 12 downward run on the rented machine once the long run's runners there end
  (`arm-confirm/down.sh 12`, log `down.log`; `run-chunk.sh` there is the long run's with seed
  200, `WC_LAYER3_JOBS=3` in the knobs, the confirmation ledger and an export of 8c7a4b5). `resume.sh` and `watch.sh` now tell the long run's
  driver from this one by its working directory (`lsof`), so neither takes one for the other.
- Docker on the Mac came back at 04:58 (someone started it); `resume.sh` restarted the driver at 05:01 on
  398df57 (only this file changed since 6a705f8); chunk 153 ran first and passed.
- Disk: each copy's `trees/<commit>/target/programs` keeps every built program, about 330 MB a chunk
  (the copy of 445e3cf had 19 GB after 54 chunks). After a restart, delete `trees/<old commit>/target` of
  every copy no longer running (done 2026-11-18 18:50: 48 GB freed; sources and logs kept).
- Layer 3's progress in programs compared: `awk -F'\t' '$1=="layer=3" && $10=="result=pass" {split($4,c,"=");
  s+=c[2]} END {print s}' .../p2/long/ledger.txt`. Any chunk that did not pass: `grep -v result=pass
  .../p2/long/ledger.txt` (each is one of failures 1 to 10 below).
- After the last fix: the confirmation, 5 percent of each layer on the final commit with base seed 200 (not
  used before): the same command with `WC_LONG_PERCENT=5 WC_LONG_SEED=200` and a new `WC_LONG_DIR`.
- Expected length (from P1's 1 percent rates at load about 30, not measured): about 50 hours, more at higher
  load.

Progress (from the ledger):
- Layer 1 done, 2026-11-16 15:55 to 18:11: 10 of 10 chunks passed, 1,215 forms x 10,000 cases (12,150,000)
  and 100 a form in Chrome (121,500); chunks 1, 2 and 4 on c38b4ad, 3 and 5 to 10 on 92b0866; 7,253 s of
  chunks (576 to 881 s each), load 10 to 30 for most of it, up to 189 (another job on the machine) in chunks
  9 and 10. Chunk 3 failed once on c38b4ad (failure 1 below) and passed on 92b0866. So on the final code the
  ledger holds 7,000 cases a form, not 10,000, for the five square root forms 92b0866 changed (FSQRT_S, _D,
  _H, _asimdmisc_R, _asimdmiscfp16_R): the layer's total mixes the two commits. T3's layer 1 rerun ran every
  form at 10,000 cases on the merged code.
- Layer 2 done, 2026-11-16 18:11 to 2026-11-17 02:37: 40 of 40 chunks passed, 1,000,000 programs, 1,000 also
  on the CPU and 200 in Chrome (at none, and full in both settings); chunks 1 to 21 and 23 on 92b0866, 22
  and 24 to 40 on 445e3cf; 29,589 s of chunks (628 to 1,230 s each), load mostly 8 to 35, peaks of 111 and
  64 from other jobs. Chunk 22 failed once on 92b0866 (failure 2 below) and passed on 445e3cf.
- Layer 3 started 2026-11-17 02:37 (chunks of 500 until 100,000 compared), on 445e3cf. At 05:56: 12 chunks
  passed, 5,246 compared (of 6,000 made; 12.6 percent did not end on Linux in time), 60 also in Chrome, 0
  mismatches; 573 to 2,384 s a chunk (most 620 to 770; chunk 8 took 2,384 s and chunk 11 1,263 s at load
  30 to 130, no failure), load 12 to 130. At about 440 compared in about 700 s a chunk, the rest (about 216
  chunks) is expected to take about 42 hours more (an extrapolation).
- Layer 3 at 2026-11-17 11:05: 30 chunks passed, 13,128 compared, 0 mismatches, still on 445e3cf; chunks
  took 573 to 2,639 s (median 709 s), load 14 to 80. From 05:33 to 11:05 it passed 18 chunks, about 18
  minutes a chunk with the gaps between chunks; at that pace the rest (about 199 chunks) is about 60 hours
  (an extrapolation).
- Layer 3 at 2026-11-18 10:25: 92 chunks passed, 40,326 compared, 460 also in Chrome (5 a chunk), 0
  mismatches of the translator; by commit: 445e3cf 54 chunks, de8f4a2 2, 7ec7920 14, 33afa1e 14, 9d54ef3 8.
  Five non-pass lines in the whole ledger, each run again and passed (layer 1 chunk 3, layer 2 chunk 22,
  layer 3 chunks 55, 70 and 84; layer 3 chunk 57 hung and has no line, failures 3 to 6). Load at chunk
  starts 7 to 104. From 2026-11-17 05:33 to 2026-11-18 10:25 (about 29 hours, with about 5 hours lost to
  the stopped, hung and killed chunks) layer 3 went from 12 to 92 chunks; at about 16 minutes a chunk the
  remaining about 136 chunks are expected to take about 36 hours (an extrapolation).
- Layer 3 at 2026-11-18 18:50: 125 chunks passed, 54,740 compared, 0 mismatches of the translator; by
  commit as above plus 9d54ef3 to chunk 114 and 6b74146 for 113 and 115 to 125. Chunks on 6b74146 took
  738 to 1,879 s at load 37 to 76. The remaining about 45,260 compared are about 103 chunks; at about 18
  minutes a chunk that is about 31 hours (an extrapolation).
- Layer 3 at 2026-11-19 02:07: 152 chunks passed, 66,505 compared, 0 mismatches of the translator. Passed
  chunks by commit: 445e3cf 54, de8f4a2 2, 7ec7920 14, 33afa1e 14, 9d54ef3 29, 6b74146 12, 99eda5e 9,
  6a705f8 18 (chunks 133 and 136 to 152). On 99eda5e and 6a705f8 chunks took 671 to 1,819 s at load 25
  to 152. The remaining about 33,500 compared are about 77 chunks; at about 16 minutes a chunk (the pace
  from 21:17 to 02:05) that is about 21 hours once Docker is back (an extrapolation).
- Layer 3 at 2026-11-19 04:15: 155 chunks passed, 67,820 compared, 0 mismatches of the translator: chunks
  1 to 152 on the Mac as above, and 198 to 200 on the rented machine. Docker on the Mac down since 02:52,
  the rented machine gone since 04:08, so nothing runs.
- Layer 3 at 2026-11-19 12:05: 214 chunks passed, 93,686 compared, 0 mismatches of the translator, no new
  non-pass line since failure 9. Not yet passed: 174 to 181 (the Mac's driver is in 174, on 398df57) and
  223 upward (`arm-up/up.sh` is in 223). Mac load 32 to 45. From 12:05 `arm-up/after-up.sh` waited for
  `up.sh` to stop at the expected bar, to run `gap.sh 181` then.
- At 12:44 something outside this track sent SIGTERM to the Mac's `arm-run` client of chunk 225 (and to two
  of this session's own waiting shells at the same moment; who is not known). The job itself ran on there, so
  `run-chunk.sh` saw no summary and recorded chunk 225 as `fail` with `cases=?` in `arm-up/chunks.tsv` only
  (not in the ledger), and `up.sh` stopped when arm-run refused chunk 226 beside the running job. Not a
  verdict on any program. `arm-up/chain.sh` (log `chain.log`, from 12:45) attaches to that job and parses
  its log as `run-chunk.sh` does (`harvest.sh`), then runs `up.sh 226`, then `gap.sh 181`, then the
  confirmation's `down.sh 12`, one after another (`after-up.sh` is stopped). `gap.sh 181` runs chunks 181
  downward on the rented machine until the next one is one the Mac's driver has started or finished (one
  translator job there at a time, as the manual asks). Its ledger lines are marked "run downward from chunk
  181 by the translator track after its upward run" (script `run-chunk-gap.sh`, `run-chunk.sh` with only
  that mark changed); log `gap.log`. Is it running: `pgrep -fl 'chain.sh|harvest.sh|up.sh|gap.sh|down.sh'`.
- The owner's rule of 2026-11-19 13:50 (no building or testing on the Mac; the brief's last section): from
  13:47 `resume2.sh` and `arm-up/chain.sh` are stopped (so nothing starts a Mac driver again; `gap.sh 181`
  ran on to its end). Chunk 181 passed on the rented machine at 13:47, so the Mac's long-run driver, in chunk
  180 then, was left to finish that chunk and then finds the bar reached and ends by itself. The Mac's
  confirmation driver was left to finish its chunk 6 and is then stopped by `p2/stop-confirm-after-chunk.sh`
  (log `stop-confirm.log`: waits for its ledger line, stops the driver's whole tree, removes the containers of
  the test it had just begun). The confirmation's remaining chunks run at once on the rented machine:
  `arm-confirm/par.sh 10 7 8 9 10 11 12` (log `par.log`; `run-chunk-par.sh` is `run-chunk.sh` with the first
  chunk syncing the export of 8c7a4b5 and the others running beside it with `--no-sync`, 10 jobs each, and the
  guard reading only the Mac's ledger lines, since its driver is stopped).
- Layer 3 done, 2026-11-17 02:37 to 2026-11-19 13:52: chunk 180 passed on the Mac (436 compared, 719 s, on
  4e67cd6, only this file changed since 6a705f8's crates), and the driver then skipped 181 to 229 (passed on
  the rented machine) and ended by itself: "layer 3: 100261 programs compared in 229 chunks". In all: 229 of
  229 chunks passed, 114,500 programs made, 100,261 compared with Linux, 14,239 not compared because they
  did not end on Linux within its 20 s alarm, 1,145 also in Chrome, 0 mismatches. 180 chunks ran on the Mac
  (178,642 s of chunks, 573 to 4,963 s each, load at chunk start 5 to 152 on 18 cores), 49 on the rented
  ARM64 machine (181 to 229; 36,183 s, 127 to 4,449 s each, 40 at 12 jobs and 9 at 48, load at start up to
  55 on 96 cores). Passed chunks by commit (all layers): c38b4ad 3, 92b0866 29, 445e3cf 72, de8f4a2 2,
  7ec7920 14, 33afa1e 14, 9d54ef3 29, 6b74146 12, 99eda5e 9, 6a705f8 18, e8677a0 3, 398df57 69, 4e67cd6 5;
  each is c38b4ad or a later commit of this branch. Since c38b4ad the translator and arm64 crates changed
  only in 92b0866 (the square root, failure 1); every later commit changed the test crate (its runner's
  stop hook in de8f4a2, the generator, the guards, the driver) or this file.
- Ledger check at the end (`p2/ledger-check.sh`): its nine non-pass lines (layer 1 chunk 3, layer 2 chunk
  22, layer 3 chunks 55, 70, 84, 113, 126, 133, 176) each have a later pass of the same chunk and knobs;
  layer 3's passed chunks are 1 to 229 with no gap and no chunk counted twice. Wall clock of the whole run:
  2026-11-16 15:55 to 2026-11-19 13:52, about 70 hours, Docker outages, stopped and hung chunks included.
- Confirmation run done, 2026-11-19 09:30 to 15:05 (base seed 200, 5 percent of each layer, ledger
  `p2/confirm/ledger.txt`): layer 1 607,500 cases (1,215 forms x 500, 6,075 in Chrome) and layer 2 50,000
  programs on the Mac, on 1f9c030; layer 3 12 chunks passed, 6,000 programs made, 5,239 compared with Linux
  (bar 5,000), 60 also in Chrome, 0 mismatches, on 8c7a4b5 (chunks 4 and 5 on 4e67cd6, where the
  watcher restarted the driver; all these commits have 6a705f8's crates).
  Layer 3 chunks 1 to 5 ran on the Mac (3 jobs, 1,446 to 2,359 s, load 10 to 100); its chunk 4 failed once on
  Docker's outage of 12:45 (failure 10) and passed again. The Mac's confirmation driver was stopped at 13:54
  in chunk 6 (owner's rule of 13:50; no ledger line, its containers removed), and chunks 6 to 12 ran at once
  on the rented machine at 8 jobs each (`arm-confirm/par.sh`, whose export was first made a git checkout,
  which arm-run now asks for): 625 to 896 s each, except chunk 7, 4,354 s, where two workers spent most of
  an hour each in the "full, stopping everywhere" runs of long programs (sampled with perf: the runner's
  stop hook under `translated_stopping_everywhere`, the slow setting of failure 3, not a hang); it passed
  with 457 compared. `ledger-check.sh` on the confirmation ledger: the one non-pass line is followed by a
  pass of the same chunk.
- The whole suite of the three crates on the final code (4e67cd6; the crates are the same at d5dcb8a and
  later, only this file changed), debug build, `cargo test -j 24 --no-fail-fast -p wcontainers-arm64 -p
  wcontainers-translator -p wcontainers-translator-test`, on the rented ARM64 machine (arm-run name
  `translator-suite`, 2026-11-19 21:48 to 22:15 UTC, load 7 to 37 on 96 cores; log in scratch
  `p2/suite/arm-suite-4e67cd6d.log`): 74 passed, 7 ignored (marked so in the code: benchmarks 2, binaries 1, lookaheads 2, programs 2), 2
  failed, both in `layer1` and both `MSR_SR_systemmove` (`MSR FPSR` keeping bits 28 and 29 on Graviton4:
  the known reference difference under "Known gaps", which is why layer 1 bars on the Mac). `layer1` on
  the Mac (`cargo test -j 4 -p wcontainers-translator-test --test layer1 -- --test-threads 3`, 14:04 to
  14:08, load 3 to 31 on 18 cores, log `p2/suite/mac-layer1.log`): 7 passed. So every test of the three
  crates passes where its verdict counts.
- After the run: the run's build folders (`long/target`, `long/zig-cache`, `long/trees/*/target`) were
  deleted; sources, logs and the ledger are kept.
- 46 to 82 of each chunk's 500 programs (9 to 16 percent) do not end on Linux within the reference's 20 s
  alarm and are not compared (counted in `sample`). Across chunks 1 to 30 the count does not follow the load
  at the chunk's start (47 at load 65, 82 at load 13), so it is Csmith's programs that loop or run long,
  not the load.

What is left of P2:
1. The independent check: read the ledger against this account; rerun every failed program and twenty
   passing ones from their seeds (each ledger line's knobs give its seed and range; `WC_LAYER3_FROM=<case>
   WC_LAYER3_CASES=1` with the line's `WC_LAYER3_SEED` replays one Csmith program, and the failing cases are
   named under "Failures found by the run"); confirm no test or guard was weakened to pass (the three guard
   changes, failures 5, 7 and 9, are departures recorded under "Departures from the plan").
2. Then, as the brief asks (not part of P2): merge origin/main and rerun layers 1 and 2 and a sample of layer
   3 on the merged code; adopt the literal check from origin/phase-1-literals; rename what this track owns
   from `...Settings` to `...Config`.

Failures found by the run, each with its cause and fix:
1. Layer 1 chunk 3 (seed 100002, c38b4ad): `FSQRT_D_floatdp1`, the root of a denormal (0x19) rounding up
   (FPCR 0x2400000) gave one ulp too many and raised inexact; Linux gives the exact root 0x1e84000000000000.
   Cause: `ieee-apsqrt` runs its Newton steps in the rounding mode asked for and calls a root exact only when
   its square comes out equal, so with rounding up it settles one ulp above any exact root (the root of 9.0
   gave 3 plus an ulp). The earlier scaling of denormals had hidden one case of it. Fix (92b0866): the
   square root is the host's root, correctly rounded to nearest, moved one ulp for the directed modes by the
   sign of the remainder m - r * r (apfloat's fused multiply-add, exact for m scaled into 1 to 4);
   `ieee-apsqrt` is gone. Infinity is answered before the root, as zero is. Test:
   `wcontainers-arm64` `tests/exact.rs` `square_roots_round_as_asked` (exact squares k^2 * 2^j, normal and
   denormal, and their neighbours, every mode, single and double, checked by exact squares in quad
   precision, and infinities): 50,807 wrong before, 0 after. The failing form and the other two square root
   forms at 1,000 cases seed 100002, and at 10,000 cases seeds 5001 to 5003: 0 failing (load 16 to 24).
2. Layer 2 chunk 22 (seed 100, case 525010, 92b0866): Linux ended the program with SIGILL, every translation
   ran it to the end. The CPU stopped at `ldp q15, q15, [x14], #-64` (found with a temporary SIGILL report
   in a scratch copy of the reference runner): a pair loaded into one register, which Arm leaves constrained
   unpredictable and this CPU treats as undefined. Cause, in the test's generator, not the translator:
   `through` moved a transfer register off a written-back base for vector accesses too, though a vector
   register is never the base, so q14/q15 through x14 became q15/q15. Fix (445e3cf): vector accesses are
   left as `fitted` made them; and layer 2 now checks every word of each program against layer 1's
   `excluded` rules before running it (this check failed on the case before the fix). A scratch sweep of
   50,000 programs of seed 100: 170 such words before the fix, none after, and no other word left out.
   About 0.3 percent of layer 2's programs held such a word before; only the CPU sample could notice, and
   chunks 1 to 21 and 23 (passed before the fix) compared those translations with each other only. With the
   fix, cases from 525000 make some programs differently than before. Restarted 22:59 on 445e3cf, same
   command; chunk 22 runs again.
3. Layer 3 chunk 55 (seed 100, cases 27000 to 27499, 445e3cf): stopped at the 90 minute limit, not a
   mismatch. The case still running was case 27441 (Csmith 100027741, gcc -O0, position independent): the
   only two cases with no program built were 27442 and 27443, the next in the same worker's run, and a
   sample of the test showed one thread in guest code under `Machine::run` for over an hour. Replayed alone
   in a scratch copy of 445e3cf, each setting timed (load 25 to 95): Linux 20.2 s with Docker (so about 19
   s of the 20 s alarm), none 1,106 s, full for speed 119 s, full for size 174 s, full for speed stopping
   everywhere 851 s, all with Linux's output; the rest (size stopping everywhere, bounded, mixed) was not
   timed. So it is a program that runs almost to the alarm on Linux, and its eight translated runs together
   take more than 90 minutes. A sample of the stopping run showed most of the time in the runner's stop
   hook, not in the guest: a full register snapshot to read the pc (`snapshot`, memset, memmove) and SipHash
   for the two slow path maps. Change (de8f4a2): the stop hook reads only the pc, and the runner's slow path
   maps are `rustc-hash` maps; what is counted and warned is unchanged. Same case, full for speed stopping
   everywhere: 353 s after against 851 s before (one run each, load 40 to 95, so the ratio is rough). The
   chunk limit was raised to 180 minutes (`WC_LONG_LIMIT=180`) so such a case can finish; nothing in the
   test was changed. Smoke, changes, programs, layer 2 (default) and layer 3 (12 cases, seed 7) pass on
   de8f4a2. Chunk 55 runs again on de8f4a2: passed, 429 compared, 4,963 s, load 41 to 63.
4. Layer 3 chunk 57 (cases 28000 to 28499, de8f4a2): hung, not a mismatch. At 01:22, 2 h 26 m in, the test
   used no CPU; its five unfinished workers were each in a `zig cc` started 22:59 to 23:02 that had used no
   CPU since, every thread waiting in `flock` (sampled). All five, and one `zig cc` of the other half
   (started 23:17), held the same manifests of zig's shared cache (`~/.cache/zig/h`, libc's crt, ubsan_rt and
   compiler_rt entries) open; three of those manifests were rewritten at 23:30, so those entries were being
   rebuilt. Cause: zig processes rebuilding the same entries of one cache at once deadlocked on its manifest
   locks (a lock cycle between processes; the other half's `zig cc` ended by itself as soon as ours were
   killed, so it was in the same cycle). Why the entries were rebuilt at 23:00 is not known (nothing of this
   half touches the cache). Not the translator, the reference or the test. Change (in the driver,
   `long-runs.sh`): each run's zig uses its own cache, `WC_LONG_DIR/zig-cache`, so other jobs on the machine
   cannot rebuild entries under it. The driver and the hung chunk were stopped 01:27 (no ledger line for
   chunk 57), the run's cache was warmed by one layer 3 run of 24 cases with one job (seed 9, every compiler,
   level and position, not counted), and the driver restarted; chunk 57 runs again: passed on 7ec7920, 445
   compared, 739 s, load 45 to 58.
5. Layer 3 chunk 70 (cases 34500 to 34999, 7ec7920): case 34780 (Csmith 100035080, clang -Os, fixed
   position) failed the test's own guard, before any translation ran: Linux ended the blob with SIGSEGV
   (139, the same four times) and the program built as an ordinary executable also with SIGSEGV, but they
   wrote different lines: the blob `...checksum after hashing O error : 8612052E`, the executable
   `...checksum after hashing H<garbage> : 32BE545C` (both the same on two more runs each). The program is
   broken as clang -Os builds it: built by gcc at -O0, -O1, -O2 and -Os it runs past 20 s without a fault;
   as clang -Os builds it, it prints a name through a wild pointer (and hash lines it should not print at
   all without an argument, so memory was overwritten) and then faults, so what it prints depends on where
   things are, which differs between the blob and the executable. The translator agrees with Linux on the
   blob at none, full for speed and full for size, each plain and stopping everywhere (replayed in a
   scratch copy). Change (33afa1e, a departure, see "Departures from the plan"): when Linux ends the blob by
   a signal, the guard now asks only that the ordinary executable end by the same signal, not that it also
   wrote the same; the blob is then compared with the translator as every other case. The guard still
   fails a case whose blob and executable end differently. The case replayed on 33afa1e with
   `WC_LAYER3_FROM=34780 WC_LAYER3_CASES=1`: compared, also in Chrome, passes. The driver was stopped after
   chunk 71 and restarted 04:32 on 33afa1e, same command; chunk 70 runs again. Chunk 70 passed on 33afa1e, 461
   compared (the case above among them), 670 s, load 32 to 104.
6. Layer 3 chunk 84 (cases 41500 to 41999, 33afa1e): recorded as failed after 814 s with no result. Its log
   stops in the middle of the run with no panic, no test summary and no message from cargo, which cargo
   prints whenever the test process ends by a signal or an error; there is no crash report and nothing in the
   system log. So cargo itself, or the `timeout` above it, was killed from outside this run at 08:00 (this
   half killed nothing then); by whom is not known. Not a mismatch. The driver now writes each chunk's exit
   status to `driver.log` (9d54ef3), so a kill shows as such next time. Stopped after chunk 85 and restarted
   08:12 on 9d54ef3, same command; chunk 84 runs again first.
7. Layer 3 chunk 113 (cases 56000 to 56499, 9d54ef3): case 56418 (Csmith 100056718, clang -Oz, position
   independent) failed the guard, before any translation ran: Linux ended the blob with SIGSEGV (139, four
   times of four), the ordinary executable with SIGBUS (135). Replayed alone the case passed (the
   executable ended with SIGSEGV), so the executable's ending varies: built the same way in scratch and run
   200 times in Docker it ended 188 times with SIGSEGV and 12 with SIGBUS (load 44 to 57). Under gdb (with
   address randomisation left on) a SIGBUS run stopped at `ldr xzr, [x11, x8, lsl #3]`, a read through a
   wild index, at 0x15000 past the executable's base, a page of its file mapping past the end of the
   70,640 byte file (SIGBUS); where the read lands depends on the randomised layout, and elsewhere it hits
   no page (SIGSEGV, as in the blob, which is anonymous memory with no file behind it). Not the
   translator: it matched Linux on the blob in every setting on the replay. Change (a departure, see
   "Departures from the plan"): the guard takes SIGSEGV and SIGBUS as one ending (`same_ending`, test
   `a_memory_fault_ends_the_same_by_either_signal`, which did not build before); any other pair of endings
   still fails. Layer 3 at 8 cases (seed 7) and the case itself pass on the change.
8. Layer 3 chunk 126 (cases 62500 to 62999, 6b74146): the reference, not the translator. Docker's Linux
   machine stopped at 18:14 (Docker's log: "The guest OS stopped the virtual machine"; it came back by
   18:18; who or what stopped it is not known, this half did not). From then each worker's builds failed
   (`gcc failed`, a panic for that case), and `native` took Docker's own failure for Linux's ending
   (status 125 from Docker, or 1 when it could not reach its daemon, with no output), so the case was
   reported as a mismatch ("ours checksum ..., Linux "" status 125"). Worse, a program that never ends,
   which Linux's 20 s alarm normally keeps out of the comparison, then ran translated without end: at
   18:49 the test's six workers were all in guest code with no new line since 18:16, so the chunk would
   have run to the 180 minute limit. Its test was stopped (ledger: fail, `cases=?`, exit status 101) and
   the driver with it. Change (99eda5e, in the tests only, no translator code): every reference
   container (the blob's run, the ordinary executable's run, the timing run) goes through one primitive,
   `programs::run_container`, which fails the case when Docker made no container (no `--cidfile` written)
   or the container did not start (Docker's own status 125 to 127 with its "Error response from daemon"
   message); a program's own ending, even 125, is still its result. Tests (`tests/programs.rs`):
   `a_container_docker_does_not_make_fails_the_case`, `a_container_that_does_not_start_fails_the_case`,
   `a_programs_own_ending_is_its_result` (pass; Docker's behaviour in each was checked by hand first: no
   daemon gives status 1 and no id file, a refused mount 125 and no id file, a missing command 127 with
   the id file and the message). Layer 3 at 8 cases (seed 7, 7 compared, 1 in Chrome) passes on it, 23 s,
   load 54 to 62. Chunks from 126 on run on 99eda5e; the translator is the same as on 6b74146. Chunk 126
   passed on it (437 compared, 938 s).
9. Layer 3 chunk 133 (cases 66000 to 66499, 99eda5e): case 66482 (Csmith 100066782, clang -O3, fixed
   position) failed the guard, before any translation ran: Linux ended the blob with SIGILL (132, four
   times of four), the ordinary executable with SIGSEGV (139); the same on a replay. Built as the guard
   builds it (with `-g`, in scratch `p2/c133`) and run under gdb in Docker, the executable prints a hash
   line through a wild pointer and then jumps into the global `g_868` (pc 0x103940c, `udf #68` there),
   data that an ordinary executable cannot run: SIGSEGV. The blob is one region that can be read, written
   and run (`placement`, and the reference runner maps it so too), so there the same jump runs the data
   and stops at the undefined word: SIGILL. Not the translator: once compared it matches Linux's SIGILL
   in every setting and in Chrome. Change (a departure, see "Departures from the plan"): the guard takes
   every ending of a program gone astray (SIGSEGV, SIGBUS, SIGILL, SIGTRAP) as one (test
   `a_program_gone_astray_ends_the_same_by_any_of_its_signals`, which replaces
   `a_memory_fault_ends_the_same_by_either_signal`). With it, Linux's signal numbers in the test crate's
   files touched come from `linux-raw-sys` (0.12.1, Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT,
   already in the workspace for the kernel test crate) and the 128 of a signalled status is one constant,
   `programs::SIGNALLED` (`layer1.rs` and `browser.rs` still have their own copies: for the literal check
   after P2). The case passes on the change, compared and in Chrome, 40 s at load 92 to 102.
10. Layer 3 chunk 176 (cases 87500 to 87999, 398df57) and the confirmation run's layer 3 chunk 4 (seed 200,
   cases 1500 to 1999, 8c7a4b5): the reference, not the translator. Docker Desktop on the Mac went away at
   about 12:45 on 2026-11-19 (its socket gone; its back end started again at 12:46:55 and answered at 12:47,
   restarted by the orchestrator's watcher). Both tests ended at 12:46:26 with exit status 143 (SIGTERM from
   outside this track; at 12:44 two of this session's waiting shells and the rented machine's `arm-run`
   client of chunk 225 were also sent SIGTERM), with no summary: ledger `fail`, `cases=?`. The next chunks
   (177; confirmation 5) started at 12:46:26 with Docker down and failed case after case at `run_container`'s
   "Docker failed" and `gcc failed` (the guard of failure 8 working: nothing was compared). Both drivers and
   their whole process trees were stopped at 12:47 (no line for 177 or confirmation 5), the three
   `wc-translator-` containers left removed, and both restarted by `resume2.sh` once Docker had answered
   for two minutes, on the checkout's head (only this file changed since 6a705f8's crates), so 176 and 177
   and confirmation 4 and 5 run again. No code changed.

## P1: close-out of M5 and the long-run driver

Merge: `origin/phase-1` merged into `phase-1-translator` (a fast forward to 7bf2809; the integration branch
already held this branch). The shared crate's two changes from the kernel half (`CounterReading::detail`, set to
0 by the runner, and `Host::output(Stream, bytes)`) need nothing more here: the three crates build and lint, and
their whole suite passed after the merge (debug, `RAYON_NUM_THREADS=4`, `--test-threads=4`: 69 passed, 7 ignored,
16.7 minutes, load 26 to 36).

The independent check's findings (`m5-check-findings.json`), each fixed:
- The size setting now runs in Chrome in the normal tests: `tests/browser.rs` runs every program at full and at
  bounded made at run time in both settings; layer 3's Chrome sample, layer 2's new Chrome sample, the
  unwinding, stack switching, written code, deep recursion and cut function tests and `checks.rs`'s Chrome test
  run full in both settings. `browser::translated_with` takes the setting with the look-ahead.
- Layers 2 and 3 compare full with none in registers and memory, plain and stopping at every check, in both
  settings, every case. The programs tests (`match_linux`, `every_look_ahead_matches`, stops at every check)
  do the same.
- A normal test, `tests/binaries.rs` `small_binaries_translate_and_load`, translates busybox and musl whole in
  both settings, validates every module, loads them in wasmtime and in Chrome (a refusal fails), and checks that
  every piece is counted once by the means that found it (13.6 s in release, load 37).
- "Found by the translator" is now split by means: `Translation::found`, by `Means` (given, call targets, built
  addresses, followed (look-ahead none only), kept reducible, cuts for the translator's own limit or an
  engine's). The old figure counted the reducibility and size cuts as found: busybox 3,100 "found" were 2,000
  call targets, 130 built addresses and 970 entries kept reducible. Output bytes are unchanged.
- Reading `.eh_frame` counts and warns of every entry it cannot read (`Binary::unwind_errors`, printed by
  `measure_binaries`; the normal test requires 0 for busybox and musl). All nine binaries: 0.
- The elfconv comparison is one command in the repository:
  `nix develop -c crates/wcontainers-translator-test/elfconv.sh` (see "How to run").
- The engines' figures and the static counters are taken again on the final code (below), replacing the stale
  ones.
- Not done: the two lookup designs' memory as process memory (finding 6, not blocking); still worked out from
  table sizes only.

Each binary, final code (`measure_binaries`, release, `RAYON_NUM_THREADS=6`, nothing else of this half
running; the seven small ones together at load 21 to 31, then Node alone at load 21 to 31, then Bun alone at
load 23 to 31). Bytes of WebAssembly (the same as the fourth run's, byte for byte in size), translation time,
pieces by means (given / call targets / built addresses / kept reducible / cuts), wasmtime's compile time and
machine code, Chrome making the modules compiled at first call / at once and how much its processes grew
(noise about 100 MB; python's first call figure came out at -120 MB). Instances took at most 17 ms in wasmtime.
- busybox: speed 32,472,392, 0.24 s, 420 / 2,000 / 130 / 970 / 0, wasmtime 2.33 s to 51.2 MB, Chrome 0.01 /
  0.06 s, 60 / 342 MB; size 13,533,418, 0.29 s, wasmtime 1.11 s to 27.5 MB, Chrome 0.00 / 0.03 s, 43 / 198 MB.
- musl: speed 14,546,689, 0.08 s, 1,513 / 250 / 30 / 31 / 0, 0.99 s to 22.6 MB, 0.00 / 0.03 s, 39 / 194 MB;
  size 6,996,354, 0.05 s, 0.57 s to 13.5 MB, 0.00 / 0.02 s, 31 / 139 MB.
- rg: speed 137,298,326, 0.55 s, 5,073 / 154 / 22 / 714 / 0, 16.03 s to 188.4 MB, 0.03 / 0.42 s, 175 / 1,314
  MB; size 41,973,706, 0.32 s, 4.52 s to 72.1 MB, 0.01 / 0.10 s, 81 / 642 MB.
- git: speed 107,719,042, 0.55 s, 6,084 / 245 / 5 / 2,975 / 0, 8.08 s to 169.2 MB, 0.03 / 0.17 s, 150 / 985 MB;
  size 43,777,767, 0.35 s, 3.82 s to 87.7 MB, 0.01 / 0.09 s, 88 / 547 MB.
- python: speed 137,603,117, 0.56 s, 5,878 / 1,263 / 72 / 655 / 374, 10.20 s to 210.3 MB, 0.03 / 0.22 s, -120
  / 1,336 MB; size 57,678,120, 0.43 s, 4.37 s to 112.7 MB, 0.01 / 0.12 s, 109 / 794 MB.
- java: speed 543,298,928, 2.38 s, 46,890 / 242 / 131 / 2,000 / 0, 43.59 s to 867.4 MB, 0.11 / 0.89 s, 660 /
  4,644 MB; size 235,237,352, 1.91 s, 20.01 s to 519.3 MB, 0.06 / 0.45 s, 350 / 2,471 MB.
- go: speed 347,710,233, 1.10 s, 17,595 / 57 / 2,106 / 1,101 / 0, 24.51 s to 546.0 MB, 0.07 / 0.47 s, 426 /
  2,949 MB; size 148,076,420, 0.91 s, 11.39 s to 315.6 MB, 0.04 / 0.25 s, 224 / 1,619 MB.
- node: speed 2,515,099,638 in 3 modules, 12.85 s, 99,365 / 498 / 2,009 / 18,685 / 29, wasmtime 189.74 s to
  3,810.2 MB (alone: so the third run's 194 s was not slowed by Chrome; the first run's 89.6 s was on other
  code), Chrome 0.46 s at first call (2,938 MB), compiled at once the page crashed; size 895,334,387 in 1
  module, 7.32 s, 80.86 s to 1,895.3 MB, Chrome 0.21 / 2.77 s, 305 / 8,781 MB.
- bun: speed 3,761,902,687 in 4 modules, 17.69 s, 64,343 / 819 / 2,215 / 10,312 / 65,849, wasmtime 340.33 s
  to 5,532.9 MB, Chrome 0.71 s at first call (4,154 MB), compiled at once the page crashed; size 1,058,185,079
  in 2 modules, 8.98 s, 139.54 s to 2,117.5 MB, Chrome 0.22 / 4.04 s, 1,441 / 7,724 MB.
- Places that may take each slow path, speed / size (left to an address; jump through a register not
  recognised; stopped by the flag; memory fault; loop not proven; engine stack nearly full; unsupported
  instruction reached where any):
  busybox 37,300 / 21,950; 528 / 459; 15,318 / 8,004; 63,243 / 35,589; 346 / 326; 5,932 / 3,342.
  musl 11,567 / 7,473; 57 / 45; 7,310 / 3,788; 21,020 / 11,175; 414 / 377; 2,668 / 1,384.
  rg 109,780 / 43,806; 510 / 473; 30,807 / 14,625; 204,956 / 76,469; 2,403 / 2,262; 10,912 / 5,044.
  git 145,137 / 82,366; 341 / 317; 37,113 / 19,990; 235,899 / 126,213; 896 / 827; 15,163 / 8,931.
  python 113,812 / 68,542; 2,766 / 2,531; 31,049 / 17,229; 292,111 / 169,776; 1,822 / 1,774; 13,245 / 7,849.
  java 466,074 / 312,032; 3,579 / 3,212; 114,920 / 61,903; 1,143,350 / 693,667; 4,786 / 4,367; 51,636 /
  32,337; 8 / 6.
  go 232,053 / 148,546; 391 / 286; 71,564 / 46,397; 883,594 / 553,930; 3,315 / 3,135; 28,393 / 18,144; 40 / 39.
  node 1,885,890 / 1,047,843; 8,143 / 6,025; 484,150 / 240,461; 5,036,524 / 2,393,326; 25,426 / 22,657;
  185,164 / 98,982; 102 / 99.
  bun 3,455,257 / 1,268,611; 8,913 / 6,346; 629,509 / 308,535; 6,234,847 / 2,258,415; 32,887 / 30,158; 257,328
  / 136,310; 54 / 52.

elfconv against ours on the final code, with the one command (`elfconv.sh`, best of 3, wasmtime only; load 26
to 59 while it ran). Times native, each against its own native build (elfconv's programs are static glibc builds
by clang 16, ours flat blobs by zig's clang, so only the ratios compare): fib elfconv 9.00 / ours 5.70; sieve
1.12 / 1.46; hash elfconv's program fails ("No lifted instruction at (0x40103c) was executed") / 0.99; sort
7.44 / 3.39; interpret 16.56 / 3.70; matrix 1.30 / 3.03; walk 0.95 / 1.00; search 1.50 / 1.95. As in M5:
elfconv ahead on loops over memory, well behind on calls and the interpreter, and wrong on hash. The script now
also compares each elfconv program's output with the native one. Not compared in Chrome.

### P1, the long-run driver

One command, `crates/wcontainers-translator-test/long-runs.sh` (how to start it under "How to run"). What it
does:
- Each layer is cut into chunks, each one test run with 6 jobs: layer 1 in ten seeds (`SEED*1000+k`) of 1,000
  cases a form, 10 a form in Chrome each; layer 2 in 40 chunks of 25,000 programs by case offset
  (`WC_LAYER2_FROM`), 25 of each on the CPU and 5 in Chrome; layer 3 in 100 chunks of 1,000 Csmith programs by
  case offset, 10 of each in Chrome. `WC_LONG_PERCENT` scales every count (at 1 percent each layer is one
  chunk).
- New knobs it needed: `WC_LAYER1_JOBS`; layer 2's `WC_LAYER2_FROM`, `WC_LAYER2_JOBS` and `WC_LAYER2_CHROME`
  (layer 2 had no Chrome sample; its cases are now each made from the seed and the case number, so a failing
  case replays alone). Each layer's cases run on one pool of threads (`src/jobs.rs`, rayon) that the translator
  and wasmtime share, so a chunk keeps to its jobs: with separate threads and the translator's pool at one
  thread, layer 2 ran at one core, and at the default pool size it would take three times the jobs.
- One line a chunk is appended to `ledger.txt` in `WC_LONG_DIR` (tab separated key=value: layer, chunk, knobs,
  cases, sample on the CPU and in Chrome, mismatches, seconds, load at start and end, commit, result; a failing
  or stopped chunk adds what failed, a replay command and its log). Each chunk's log is kept under `logs/`.
- Run again, chunks the ledger holds for an accepted commit (`WC_LONG_ACCEPT` up to HEAD; default HEAD alone)
  are skipped; a run on a tree with changes to the crates is recorded with `-changed` and never skipped. A chunk
  past `WC_LONG_LIMIT` minutes (default 90) is stopped, recorded as `stopped`, its reference containers removed,
  and the run goes on. Do not change the crates while it runs: each chunk's `cargo test` would build the change.

Proven at 1 percent (seed 7, commit c999db7, the whole run 1,758 s, load 24 to 31; ledger and logs in
scratch `p1/long2`):
- layer 1: 1 chunk, 1215 forms x 100 cases (121,500) and 1215 in Chrome, 0 mismatches, 60 s, load 26 to 27.
- layer 2: 1 chunk, 10,000 programs, 10 on the CPU and 2 in Chrome, 0 mismatches, 260 s, load 27 to 31.
- layer 3: 1 chunk, 1,000 Csmith programs: 888 compared, 112 did not end on Linux in time, 10 also in Chrome,
  0 mismatches, 1,437 s, load 24 to 31.
- Run again: all three skipped, in 2 s. A layer 3 chunk with `WC_LONG_LIMIT=1` was stopped after 60 s,
  recorded as stopped with its replay command, and left no container.
- So the whole run, from these rates at load about 30 (expected, not measured): layer 1 about 1.7 hours (ten
  chunks of about 10 minutes), layer 2 about 7.2 hours (40 chunks of about 11 minutes), layer 3 about 40 hours
  (100 chunks of about 24 minutes): about 49 hours in all. Each chunk is well under the 90 minute limit.

## M5, fourth run

Translation speed, profiled again on Bun (macOS `sample`, speed setting, translation only,
`RAYON_NUM_THREADS=6`):
- Half the time was one thread in `slice::contains`: a switch table's targets were kept once by searching
  the list for each entry (`habits::jump_table`), and the addresses a function builds were each searched
  for in every table's targets (`Finder::explore`). Both are now sets. Bun has a table of 360,445 entries
  (left unrecognised later, as before), searched again each time the function was explored.
- Then: loop pieces were found on one thread, and for each loop the set of the function's instructions
  was made even when nothing used it (only stretches do). Now found per function on every core, the set
  made once and only for stretches. The checks for irreducible loops and cuts after each round also run
  on every core.
- Then: the translator's maps used std's SipHash; now `rustc-hash` (already a dependency) everywhere in
  `full.rs` and `full/proof.rs`.
- Bun for speed, best of each step (load 26 to 46): 72.74 s (third run) / 33.71 s (sets) / 30.11 s
  (loop pieces on every core) / 18.69 s (fast hash; 21.84 s again). Output the same bytes: total sizes the
  same for Bun, and every module byte for byte the same as before for busybox, musl, rg, python and go in
  both settings (`cmp` of `WC_BINARIES_DUMP`).
- What is left of Bun's time is mostly making and emitting its 3.9 GB in parallel; a split translation is
  still emitted twice (each module's code depends on which pieces it holds).

Size, what is free (section 11, step 1). The meanings' own waste, each change measured by itself on the
benchmark set (bytes of WebAssembly, speed / size setting) and run speed in both engines:
- An addition or subtraction that sets no flags is a plain `add` or `sub` (it was `a + (b ^ -1) + 1`
  through three locals, as the flag setting form needs), adding zero is a copy, and a `mov` or `mvn` (an or
  or exclusive or with the zero register) is the copy alone. Benchmark set 312,148 / 133,089 to 297,773 /
  125,813 (-4.6% / -5.5%); musl for size 7,313,689 to 7,121,191 (-2.6%).
- A load or store with no write back lets the check untag the address in the local it was made in (it was
  copied to a second local first), and an address the code makes from its own place (`adr`, a literal
  load, the link register) adds the offset once (it added the instruction's offset, then the rest).
  Benchmark set 297,773 to 295,287 (speed); musl for size 7,121,191 to 7,012,251 (-1.5%).
- Run speed, speed setting, the three alternated over three rounds (sort, interpret, matrix; best of 3
  each; load 26 to 65), base / first / both: wasmtime sort 1.078 / 1.085 / 1.078, interpret 0.514 / 0.511
  / 0.512, matrix 0.222 / 0.227 / 0.226; Chrome sort 1.068 / 1.069 / 1.069, interpret 0.581 / 0.574 /
  0.581, matrix 0.156 / 0.155 / 0.156. The whole set once each (load 20 to 50), base / first, speed:
  wasmtime fib 2.254 / 2.139, sieve 0.640 / 0.613, hash 0.576 / 0.539, walk 2.213 / 2.143, search 0.238 /
  0.250; size (where loops run with their checks): wasmtime fib 2.784 / 2.702, sieve 1.283 / 1.269, sort
  1.608 / 1.273, interpret 0.673 / 0.603, matrix 0.303 / 0.336, search 0.408 / 0.280; Chrome sieve 0.668 /
  0.668, matrix 0.783 / 0.774, search 1.445 / 1.421. No loss shows; kept in both settings.
- Both together on the binaries (translation only, load about 35), speed / size: busybox 32,574,096
  (-3.6%) / 13,592,810 (-4.9%); musl 14,572,720 (-3.5%) / 7,012,251 (-4.1%); rg 137,534,970 (-4.1%) /
  42,064,664 (-5.1%); git 108,125,485 (-4.0%) / 44,008,090 (-5.7%); python 137,874,944 (-3.5%) /
  57,840,097 (-4.7%); java 544,231,926 (-3.2%) / 235,856,675 (-4.6%); go 348,302,497 (-3.0%) / 148,452,670
  (-4.3%). Gzipped about 10% less.
- The bit that tells a callee the caller wants the flags is added into the return address constant (it
  was an `or` after the add; instructions are 4 byte aligned, so the bit is clear).

Each binary on this run's final code, translation only (`measure_binaries`, `WC_BINARIES_ENGINES=0`,
`RAYON_NUM_THREADS=6`, one run, load 30 to 62). Bytes of WebAssembly (bytes a byte of code), gzipped, and
translation time, speed / size:
- busybox 32,472,392 (46.1), 6,103,289, 0.18 s / 13,533,418 (19.2), 3,012,555, 0.11 s.
- musl 14,546,689 (39.3), 2,763,532, 0.08 s / 6,996,354 (18.9), 1,413,770, 0.05 s.
- rg 137,298,326 (67.9), 22,141,772, 0.51 s / 41,973,706 (20.7), 7,555,446, 0.29 s.
- git 107,719,042 (49.4), 18,926,550, 0.47 s / 43,777,767 (20.1), 9,368,089, 0.34 s.
- python 137,603,117 (46.5), 21,758,085, 0.58 s / 57,678,120 (19.5), 10,426,703, 0.56 s.
- java 543,298,928 (41.2), 81,396,412, 2.48 s / 235,237,352 (17.8), 40,699,810, 2.02 s.
- go 347,710,233 (59.3), 47,273,601, 1.14 s / 148,076,420 (25.3), 24,231,195, 0.87 s.
- bun 3,761,902,687 in 4 modules (72.3), 558,499,321, 24.97 s / 1,058,185,079 in 2 modules (20.3),
  176,121,046, 10.92 s (third run: 72.74 s / 52.08 s).
- node 2,515,099,638 in 3 modules (62.9), 380,762,969, 14.74 s / 895,334,387 in 1 module (22.4),
  155,537,459, 8.11 s (third run: 18.79 s / 9.27 s).
- The engines' figures on this code are under "P1" (taken again there, alone).

What a call between two modules costs, again on this run's code (`measure_calls`, best of 3, load 24 to 32):
wasmtime speed 2.81 ns, size 2.20 ns; Chrome speed 2.71 ns (0.135 s in one module, 0.406 s a module a
piece), size 2.75 ns. Chrome's figures now agree across two runs (third run 2.65 and 2.75 ns).

Cases run on this run's code: after the translation speed change, smoke, checks, programs and layer 2 at
their defaults (release); after the size changes, smoke, checks, programs, layer 2, look-ahead, changes and
layer 1 at 200 cases a form (release; layer 1 at 300 a form after the first size change), all passing.
Last, every test of the three crates on the final code (debug, `RAYON_NUM_THREADS=4`, Chrome and layer 3's
defaults included): 69 passed, 7 ignored (the measurements), 16 minutes, load 34 to 61. No long random run
was made this run.

M5, fourth run, what comes next (the milestone's parts are all built and recorded; these are refinements):
1. Size: slow paths still write back their registers in place, the floating point ones with every live
   vector register (`v128.store` is 1.4% of musl's instructions, size setting). A shared exit per function
   would need every register's local to hold its value at every exit; not tried.
2. Size mode: a proven loop with loops inside it still has only its checked copy.
3. A split translation is emitted twice (Node and Bun only), as each module's code depends on which pieces
   it holds.
4. (Done in P1: the engines' figures on this code, taken alone.)

## M5, third run

Size, slow paths out of line. Built and measured as free first, then found not to be free, so it is a
size trade (section 11), not a change for both settings:
- What it is: a write back on a path the thread never comes back on (a memory fault, leaving to exact code,
  an unsupported instruction) stores the passed registers and pc through a function each module holds once
  (`F_SAVE`, pc as a blob offset), and works out the flags word through a module function for each shape of
  kept flags and one for flags in locals (`flags_word_functions`). The same work before a call, a jump to
  another piece, a return, a stop check or a system call is always in place. The depth check at a piece's
  entry uses `F_SAVE` in both settings (busybox speed 33,934,500 / 33,789,527, -0.4%).
- Size, measured each by itself in the speed setting (translation only; load 120 to 260, which does not
  change sizes): `F_SAVE` busybox 33,934,500 / 30,794,415 (-9.3%), musl 15,157,213 / 14,038,301 (-7.4%), rg
  143,614,742 / 134,177,916 (-6.6%), git 112,987,851 / 101,607,621 (-10.1%), python 143,036,317 / 131,149,290
  (-8.3%); then the flags word functions busybox 28,679,642 (-6.9%), musl 13,154,114 (-6.3%), rg 125,371,053
  (-6.6%), git 95,563,100 (-5.9%), python 121,691,140 (-7.2%). Slow paths went from 33.4% to 24.3% of musl's
  instructions.
- Cost: the benchmark set in the speed setting showed nothing (best of 3, load 17 to 22: wasmtime fib 2.379,
  sort 1.163, interpret 0.567, matrix 0.253, sieve 0.640), because its hot loops there run their copies without
  checks. In the size setting, where loops run with their checks, sieve went from 0.910 (the run's start
  commit, same tree, load 14) to 1.25 to 1.41 (several runs, load 14 to 40); with these paths in place again
  0.827, with only the flags word in place 1.025. Calls on a path that ends in `unreachable` still slow the
  engine's code for the loop around them (the register allocator does not know the path is cold). Checked
  loops exist in the speed setting too (loops not proven), so it is made only for size.
- Tried and dropped: a register's `local.set` followed by its `local.get` made a `local.tee`: 0.3% (busybox
  24,543,043 / 24,463,163), not worth the state it needs.

Size mode, the exact permission test (run only when the fast one fails) is a call to a module function
(`F_PERMISSION`) instead of in place. Size, from the size setting with the slow paths out of line: busybox
15,986,588 / 13,626,082 (-14.8%), musl 7,445,171 / 6,678,359 (-10.3%), rg 46,019,767 / 40,599,353 (-11.8%),
git 52,999,449 / 44,966,585 (-15.2%), python 68,029,417 / 57,266,627 (-15.8%), java 284,163,910 / 238,563,513
(-16.0%), go 184,241,116 / 149,926,790 (-18.6%) (load 12 to 22). Its cost was measured in the speed setting,
alternating (best of 3, load 17 to 25, in place / called): wasmtime fib 2.379 / 2.876, sort 1.163 / 1.292,
interpret 0.567 / 0.647, matrix 0.253 / 0.352 (1.39 times); Chrome interpret 0.597 / 0.807 (1.35 times), sort
1.236 / 1.432, matrix 0.201 / 0.184, fib 2.321 / 2.209. The call returns into the piece, so the engine keeps
every value live across it.

Size mode, a proven loop with no loop inside it keeps its copy without checks (what trade 1 took away). The
proof and both copies sit in a `loop`; a stop at the loop's own jump back in the unchecked copy loads the
registers back and branches there, so the proof is made again with the registers as they are (it depends only
on them at the loop's header). A loop with loops inside it still has only its checked copy, since a stop at an
inner loop's jump back cannot carry on in the outer loop's checked copy without a loop piece. Size, from the
line above: busybox 13,626,082 / 13,980,688 (+2.6%), musl +6.3%, rg +7.2%, git +2.2%, python +4.4%, java
+2.3%, go +2.4%. Speed (size setting, wasmtime, best of 3, load 20 to 40): matrix 0.770 without / 0.306 to
0.350 with; search 0.328 with (0.478 at trade 2). Tests: `tests/checks.rs` now runs every case at full in both
settings (the loop not proven counts hold in both), and `tests/changes.rs` translates for size when the plan's
seed is odd.

Translation speed: profiled Node with macOS `sample` (speed setting, translation only). It ran on one thread;
finding functions (`Finder::explore`, with a SipHash map of decoded instructions) took about half, planning
and emitting the rest. Now (with `rayon` and `rustc-hash`, both established crates): functions are found in
rounds, every entry of a round explored at once; exploring decodes on the fly and keeps nothing, and the
instructions of the functions found are decoded once afterwards into a map with a fast hash; plans, loop piece
plans and emitting run on every core (on wasm32, where threads cannot start, rayon runs them on the calling
thread). Same pieces and same bytes for all eight binaries tried. Measured with `RAYON_NUM_THREADS=6` (this
machine's limit for one job), load 21 to 55: Node 92.98 s before (one thread) / 35.37 s with planning and
emitting in parallel / 19.42 s with exploring too (2.06 MB of code a second); busybox 0.67 / 0.25 s, rg 2.63
/ 0.95 s, java 12.16 / 3.76 s, go 5.49 / 1.73 s. What is left on one thread: the rounds' bookkeeping, the
check for irreducible loops and cuts, and a translation split into modules is emitted twice (Node: 3
modules).

Static counters: each code module now carries how many places in its code may take each slow path
(`CodeModule::sites`, by reason; floating point paths, decided by a hook at run time, are not counted), and
`measure_binaries` prints them. The figures below are stale (the size setting's were taken before later changes
of this run); the current ones are under "P1". Speed / size settings (load 22 to 11):
- busybox: left to an address 37,300 / 21,944; jump through a register not recognised 528 / 459; stopped by the
  flag 15,318 / 7,662; memory fault 63,243 / 35,346; loop not proven 346 / 0; engine stack nearly full 5,932 /
  3,342.
- musl: 11,567 / 7,465; 57 / 45; 7,310 / 3,396; 21,020 / 10,911; 414 / 0; 2,668 / 1,384 (same order).
- rg: 109,780 / 43,750; 510 / 472; 30,807 / 12,322; 204,956 / 75,843; 2,403 / 0; 10,912 / 5,044.
- git: 145,137 / 82,347; 341 / 317; 37,113 / 19,126; 235,899 / 125,274; 896 / 0; 15,163 / 8,931.
- python: 113,812 / 68,483; 2,766 / 2,529; 31,049 / 15,406; 292,111 / 167,952; 1,822 / 0; 13,245 / 7,849.
- java: 466,074 / 311,644; 3,579 / 3,212; 114,920 / 57,069; 1,143,350 / 687,197; 4,786 / 0; 51,636 / 32,337;
  unsupported instruction reached 8 / 6.
- go: 232,053 / 148,498; 391 / 286; 71,564 / 43,105; 883,594 / 552,437; 3,315 / 0; 28,393 / 18,144; unsupported
  instruction reached 40 / 39.
- (The size setting has fewer places because a function has no loop pieces, which repeat their loop's code.)

Each binary in each setting with both engines, on the final code of this run, superseded by "P1" (`measure_binaries`, release,
`RAYON_NUM_THREADS=6`, one run, load 24 to 36). Bytes of WebAssembly; translation time; wasmtime's compile time
and bytes of machine code; Chrome's time to make the modules compiled at first call / at once, and how much
the resident memory of Chrome's processes grew from before the worker started to after the modules and their
instances were made, at first call / at once. The difference of the two is Chrome's memory for the code its
baseline compiler made (noise about 100 MB: git's first-call figure came out at -88 MB). Instances took at most
11 ms everywhere.
- busybox: speed 33,789,527, 0.25 s, wasmtime 2.28 s to 50.7 MB, Chrome 0.01 / 0.06 s, 62 / 352 MB; size
  14,295,760, 0.13 s, 1.14 s to 27.2 MB, 0.00 / 0.03 s, 41 / 206 MB.
- musl: speed 15,102,460, 0.10 s, 1.02 s to 22.5 MB, 0.00 / 0.03 s, 40 / 205 MB; size 7,313,689, 0.06 s, 0.62
  s to 13.4 MB, 0.00 / 0.02 s, 32 / 143 MB.
- rg: speed 143,403,426, 0.97 s, 13.41 s to 187.5 MB, 0.03 / 0.22 s, 182 / 1,375 MB; size 44,311,195, 0.37 s,
  4.62 s to 72.1 MB, 0.01 / 0.10 s, 84 / 655 MB.
- git: speed 112,629,083, 0.77 s, 8.16 s to 167.1 MB, 0.03 / 0.18 s, -88 / 1,030 MB; size 46,661,991, 0.41 s,
  3.90 s to 86.7 MB, 0.01 / 0.10 s, 90 / 561 MB.
- python: speed 142,820,698, 0.95 s, 10.77 s to 209.3 MB, 0.03 / 0.23 s, 190 / 1,374 MB; size 60,678,646, 0.55
  s, 4.85 s to 112.0 MB, 0.02 / 0.13 s, 107 / 830 MB.
- java: speed 562,179,318, 3.52 s, 44.13 s to 863.4 MB, 0.11 / 0.96 s, 679 / 4,789 MB; size 247,101,181, 2.34
  s, 18.91 s to 516.8 MB, 0.06 / 0.46 s, 363 / 2,538 MB.
- go: speed 358,903,616, 1.52 s, 24.26 s to 543.5 MB, 0.07 / 0.49 s, 437 / 3,009 MB; size 155,086,202, 1.00 s,
  13.08 s to 313.7 MB, 0.03 / 0.28 s, 233 / 1,654 MB.
- node (load 33 to 43): speed 2,605,496,053 in 3 modules, 18.79 s, wasmtime 194.03 s to 3,795.4 MB (beside
  Chrome; 89.6 s in the first run), Chrome 0.73 s at first call (3,028 MB, mostly the module bytes it keeps);
  compiled at once Chrome's page crashed again. Size 941,973,129 in 1 module (23.5 a byte of code), 9.27 s,
  wasmtime 74.00 s to 1,886.0 MB, Chrome 0.26 / 2.83 s, 387 / 5,498 MB. Static counters, speed / size: left to
  an address 1,885,890 / 1,047,843; jump through a register not recognised 8,143 / 6,025; unsupported
  instruction reached 102 / 99; stopped by the flag 484,150 / 240,461; memory fault 5,036,524 / 2,393,326; loop
  not proven 25,426 / 22,657; engine stack nearly full 185,164 / 98,982.
- bun (load 34 to 43): speed 3,918,787,999 in 4 modules (75.3 a byte of code), 72.74 s (still slow: see next),
  wasmtime 373.90 s to 5,507.2 MB (beside Chrome), Chrome 1.11 s at first call (4,312 MB); compiled at once its
  page crashed. Size 1,115,588,488 in 2 modules (21.4 a byte), 52.08 s, wasmtime 102.08 s to 2,095.0 MB, Chrome
  0.31 / 4.61 s, 1,498 / 3,871 MB. Static counters, speed / size: left to an address 3,455,257 / 1,268,611; jump
  through a register not recognised 8,913 / 6,346; unsupported instruction reached 54 / 52; stopped by the flag
  629,509 / 308,535; memory fault 6,234,847 / 2,258,415; loop not proven 32,887 / 30,158; engine stack nearly
  full 257,328 / 136,310.
- So all nine load in both engines in both settings; Chrome compiling everything at once refuses (the page
  crashes) only Node and Bun for speed, whose baseline code would be about 20 GB.
- Chrome's eager timings do cover compiling every function: its memory grows with the module's size, 7 to 9
  bytes for each byte of WebAssembly (V8's baseline code is about 5 to 6.5 times wasmtime's machine code),
  within the synchronous `new WebAssembly.Module`. Compiled at first call, it holds only the module.
- (wasmtime's compile times here are about twice the second run's: it compiled beside Chrome's runs and the
  other half's tests; not re-taken alone.)

What a call between two modules costs in Chrome: the Chrome runner can now split a translation (`splitBytes`
in its setup, `browser::repeated_split`). `measure_calls_between_modules` (best of 3, the program run twice in
its instances, load 17 to 20): speed 0.135 s in one module, 0.400 s a module a piece, 2.65 ns more a call;
size 0.152 / 0.427 s, 2.75 ns. wasmtime in the same run: speed 2.41 ns, size 2.35 ns.

Cases run on this run's code (all passing): after each change, the changes, checks, layer 2 (300 cases),
look-ahead, programs, smoke, Chrome and validity tests at their defaults (release). Last, every test of the
three crates on the final code (debug, `RAYON_NUM_THREADS=4`, Chrome and layer 3's defaults included): 69
passed, 7 ignored (the measurements), 16 minutes, load 26 to 38. No long random run was made this run. Bun's
translation alone again: 76.73 s (load about 30).

M5, third run, what comes next (most important first; the milestone's parts are all built and recorded):
1. Bun still translates slowly (72.74 s for speed, 52 s for size, against Node's 18.79 s for less code): not
   profiled after the parallel work. Likely the rounds of cuts (a function of more than 2048 blocks makes
   every function be explored again) and a split translation being emitted twice.
2. Size: nothing free was found this run. Left to try: the meanings' own waste (copies between locals, a
   register's set then get), measured in both settings on loops that run with their checks (sieve in the size
   setting shows what the benchmark set in the speed setting hides).
3. Size mode: a proven loop with loops inside it still has only its checked copy.
4. Chrome's own timings of calls between modules and of memory are from one run each.

## M5, second run

Lookup design (plan section 5), both built and measured, compact chosen:
- Built: the lookup table now holds one slot per piece, and an array of slots in the state memory (one i32
  per 4 bytes of guest address from `table_base`, 0 for no piece) gives each address its slot. A lookup is
  the old range and alignment test, one i32 load from the array, and `table.get`. Slot 0 stays empty (so an
  address with no piece finds none) and slot 1 (`EXACT_SLOT`) holds the exact path's one-instruction piece,
  in place of the old last slot. Code modules put their pieces in order from `slot_base` (one element
  segment); the runner gives each module its slots, grows the table and writes the array from
  `CodeModule::pieces` (the Chrome runner reads the same list from the module's description section). The
  array is per process and the table per thread, so a module gets the same slot base in every thread. New
  imports `slots` and `slots_cover` (contract.rs).
- Measured (benchmark set, -O2, full, best of 3, release; the old design run from a copy of commit 0d312ab,
  rounds alternated, load 30 to 57; seconds, old round 1 / round 2 against new round 1 / round 2):
  wasmtime fib 2.246 / 2.418 against 2.253 / 2.400; sieve 0.642 / 0.648 against 0.660 / 0.649; sort 1.133 /
  1.215 against 1.162 / 1.318; interpret 0.525 / 0.561 against 0.551 / 0.626; matrix 0.237 / 0.251 against
  0.249 / 0.262; walk 2.092 / 2.145 against 2.235 / 2.222; search 0.240 / 0.200 against 0.231 / 0.227.
  Chrome fib 2.015 / 2.179 against 2.007 / 2.126; sort 1.072 / 1.170 against 1.109 / 1.237; interpret 0.585 /
  0.604 against 0.589 / 0.637; the others within 0.01 s. Every difference is inside this machine's 10 to 15%
  noise; the best of each points to the compact design costing up to a few percent on calls through
  registers (sort 2.6%, interpret 5% in wasmtime). Not settled.
- Memory: the old table had a slot (a pointer, 8 bytes in both engines) per 4 bytes of code; Node's
  10,003,026 slots were 80 MB in every thread. Now the table has one slot per piece (Node 120,586) per thread,
  and the array 4 bytes per 4 bytes of code once per process (40 MB for Node), in a shared memory where
  untouched pages cost nothing. Measured only as these sizes, not as process memory.
- Chosen: the compact design. It is the only one within Chrome's 10,000,000 table entries for Node and Bun,
  and no speed difference could be shown. The old design is gone (one primitive).

What a call between two modules costs (`measure_calls_between_modules`, `programs/calls.c` -O2, 100 million
direct calls of a small function, the program in one module against every piece in a module of its own;
best of 3, wasmtime): 2.60 ns more a call (0.208 s against 0.468 s; native 0.047 s; load 30 to 32), again
2.95 and 2.91 ns in a later run (speed and size; load 25). Not measured in Chrome: the Chrome runner cannot
split a translation into modules yet.

Size, first what is free (section 11, step 1), each measured by itself (translation only, release, bytes of
WebAssembly; before / after):
- Flags written back: the NZCV word was built flag by flag, and for flags whose state is known only at run
  time each of the four flags had its own choice between states, each working out the kept comparison again.
  Now the word is worked out once for each state. busybox 38,916,859 / 33,934,500 (-12.8%), musl 17,145,765 /
  15,157,213 (-11.6%), rg 160,947,565 / 143,614,742 (-10.8%), git 126,677,601 / 112,987,851 (-10.8%), python
  161,387,477 / 143,036,317 (-11.4%), java 639,562,571 / 563,342,889 (-11.9%), go 408,425,275 / 359,573,773
  (-12.0%); the benchmark set 360,994 / 312,018 (load 34 to 40). Run speed (benchmark set, best of 3, load
  27 to 33, against the lookup rounds above): fib 2.405, sieve 0.650, sort 1.092, interpret 0.547, matrix
  0.234, search 0.232 in wasmtime; Chrome fib 2.123, sort 1.079, interpret 0.603, matrix 0.168: no loss.
  Kept in both settings.
- Where the rest goes (the benchmark set after the change, `wasm-tools print` and a script in scratch,
  `m5r2/slow.py`): 28% of the instructions are in slow paths that end in `unreachable` (623 of them, about 70
  instructions each: registers written back, the flags word, the passed registers and pc, the hook call);
  `main` alone is 32,440 of 154,391 lines of text. Not reduced yet.

The setting (section 11): `Optimise::{Speed, Size}`, a parameter of `translate` (also of `translate_omitting`
and `translate_in_modules`), written in the description section (format version 3: a byte after the
look-ahead). Speed is the default and gives up nothing. The runners take it (`Machine::optimise_for`, used
for code made at run time too; the Chrome runner's setup `optimise`). Mixed runs (layers 2 and 3, the program
tests) now load full look-ahead translations in both settings, a bounded one in a random setting, and make
code at run time in a random setting, so pieces of both settings meet in every mixed run. The size mode,
one trade at a time:
- Trade 1, no copy of a proven loop without its checks (every loop keeps only its checked copy). Size:
  busybox 33,524,703 (-1.2% from speed), musl 14,671,148 (-3.2%), rg 139,745,007 (-2.7%), git 111,660,457
  (-1.2%), python 140,363,235 (-1.9%), java 555,484,478 (-1.4%), go 355,632,306 (-1.1%); benchmark set
  284,057 (-9%). Speed (benchmark set, best of 3, load 35 to 52, against speed's figures above): wasmtime
  matrix 0.637 s (2.7 times slower), search 0.418 (1.8 times), interpret 0.595 (9%), others the same; Chrome
  matrix 0.251 (1.5 times), search 0.362 (1.5 times), sieve 0.623 (18%). A poor trade on its own; kept
  because trade 2 needs it (a loop without its own loop piece cannot carry on after a stop where its proof is
  made again).
- Trade 2, no loop pieces: a stop at a jump back carries on in place (what irreducible loops already did).
  Size, from trade 1: busybox 19,077,708 (-43%; 27.1 bytes a byte of code), musl 8,646,183 (-41%), rg
  53,558,478 (-62%), git 62,767,687 (-44%), python 80,524,723 (-43%), java 337,272,362 (-39%), go
  222,990,220 (-37%); the benchmark set 130,762 (-54%) (load 42 to 49). Gzipped 7.9 to 12.8 bytes a gzipped
  byte of code, against 14.2 to 25.9 for speed. Speed (best of 3, load 21 to 28): wasmtime about as trade 1
  (sieve 0.803, matrix 0.507, search 0.478, interpret 0.639; fib, hash, sort, walk unchanged); Chrome much
  slower where a long loop runs in a function entered once, because V8 only tiers a function up when it is
  called again: matrix 1.360 s (8 times speed's 0.168), search 2.170 (9 times), hash 1.453 (2.5 times),
  sieve 0.918 (1.7 times), interpret 0.800 (1.3 times); fib, sort, walk unchanged.
- Translation is faster in size mode (busybox 0.50 s against 0.59 s; rg 1.53 against 2.31).

elfconv (the benchmark to beat), set up and compared on the benchmark set:
- Set up from its published image `ghcr.io/yomaytk/elfconv:arm64` (6.1 GB, built 2026-07-05; remill, LLVM 16,
  WASI SDK 24, Emscripten), not built from source. Its lifter accepts only statically linked glibc programs (it
  looks for glibc's `_start` pattern to find `main`), so the flat blobs of `bench.c` cannot be given to it.
  Each workload was built inside the image as an ordinary static glibc program with clang 16 (`-O2 -static`,
  bench.c's own `_start` renamed away) and converted with `TARGET=aarch64-wasi32 ./exe.sh` (about 55 s a
  workload, 5.7 MB of WebAssembly each). Scripts in scratch: `m5r2/elfconv/convert.sh` and `time.sh`.
- Run in wasmtime 50 (compiled ahead with `wasmtime compile`, best of 3, so the time includes wasmtime
  starting and instantiating, a few milliseconds), and the same glibc executables natively in Docker (best of
  3), load 24 to 30. Times native, each against its own native build (ours: speed setting, wasmtime, from the
  run above; our blobs are built by zig's clang without a C library, elfconv's by clang 16 with glibc, so the
  machine code differs and only the ratios compare): fib elfconv 8.7 (3.60 s / 0.414), ours 6.0; sieve 1.05
  (0.48 / 0.459), ours 1.44; hash elfconv failed ("No lifted instruction at (0x40103c) was executed", no
  output; native 0.640 s); sort 6.9 (2.42 / 0.353), ours 3.2; interpret 14.8 (4.14 / 0.279), ours 3.7; matrix
  1.17 (0.11 / 0.094), ours 3.0; walk 0.84 (2.29 / 2.728), ours 0.96; search 1.17 (0.17 / 0.145), ours 1.9.
- So elfconv is ahead on loops over memory (sieve, matrix, search, walk: LLVM -O3 optimises the lifted
  loops, and it has no permission checks) and well behind on calls, calls through registers and the switch
  interpreter (fib, sort, interpret). It also gave a wrong result where we do not (hash). Not compared in
  Chrome (its browser output needs Emscripten's page).

Measured with the engines, each binary in each setting (`measure_binaries`, release, one run each; load 25
to 52). Bytes of WebAssembly, then wasmtime's compile time and bytes of machine code (the memory it uses for
compiled code), then Chrome making the modules compiled at first call / at once by its baseline compiler.
Instances took at most 18 ms in wasmtime and 177 ms in Chrome everywhere.
- busybox: speed 33,934,500, 1.07 s, 50.8 MB, 0.01 / 0.06 s; size 19,077,708, 0.65 s, 29.6 MB, 0.00 / 0.04 s.
- musl: speed 15,157,213, 0.46 s, 22.6 MB, 0.00 / 0.03 s; size 8,646,183, 0.28 s, 13.4 MB, 0.00 / 0.02 s.
- rg: speed 143,614,742, 5.20 s, 187.7 MB, 0.03 / 0.21 s; size 53,558,478, 3.67 s, 73.8 MB, 0.01 / 0.10 s.
- git: speed 112,987,851, 3.59 s, 167.4 MB, 0.02 / 0.17 s; size 62,767,687, 2.03 s, 95.3 MB, 0.02 / 0.10 s.
- python: speed 143,036,317, 4.48 s, 209.5 MB, 0.03 / 0.22 s; size 80,524,723, 2.66 s, 121.6 MB, 0.02 / 0.13 s.
- java: speed 563,342,889, 19.7 s, 863.9 MB, 0.11 / 1.04 s; size 337,272,362, 13.1 s, 531.9 MB, 0.07 / 0.62 s.
- go: speed 359,573,773, 13.3 s, 543.8 MB, 0.07 / 0.55 s; size 222,990,220, 9.8 s, 341.2 MB, 0.05 / 0.30 s.
- node: speed 2,608,644,243 in 3 modules (65.2 a byte of code), translated in 61 to 66 s; Chrome now makes its
  modules (0.53 to 0.76 s at first call, instances 26 to 33 ms): the compact lookup's table fits. Compiled at
  once, Chrome's page crashed ("Target crashed"; two runs), so 2.6 GB is past what one page can compile
  eagerly here; wasmtime was not reached in these runs (the first run's figure: 89.6 s to 4.04 GB). Size:
  1,253,423,350 in 2 modules (31.3 a byte), translated in 48 s; Chrome 0.28 s at first call and 2.57 s at once
  (instances 24 and 177 ms); wasmtime 49.2 s to 1.88 GB of machine code.
- bun (load 33 to 54): speed 3,922,908,246 in 4 modules (75.4 a byte), translated in 139 s; Chrome makes its
  modules at first call in 1.06 s (instances 34 ms); compiled at once its page crashed; wasmtime 154.8 s to
  5.51 GB of machine code. Size 1,408,316,068 in 2 modules (27.1 a byte), translated in 86 s; Chrome 0.32 s at
  first call and 4.30 s at once (instances 31 and 193 ms); wasmtime 61.7 s to 2.06 GB.
- So all nine binaries now load in Chrome (compiled at first call) in both settings; compiled at once, Node
  and Bun load only for size.
- A crashed page is now reported as a refusal, so the measurement goes on to wasmtime.
- Chrome's memory for compiled code is not measured (nothing reads V8's code space yet); wasmtime's is the
  machine code bytes above.

Cases run on this run's code (all passing): after each change, the program, layer 2, look-ahead, changes,
Chrome, smoke and checks tests at their defaults (release; mixed runs now hold pieces of both settings).
Last, every test of the three crates on the final code (debug, Chrome and layer 3's defaults included): 69
passed, 7 ignored (the measurements), 13.5 minutes, load 36 to 55. No long random run was made this run.

M5, what comes next (most important first):
1. Size, free reductions not yet done: slow paths are 28% of the output (each writes back its registers, the
   flags word, the passed registers and pc, inline): a shared function per module for the part that is the
   same everywhere (passed registers, pc, the hook call, the unwind). Then the meanings' own waste (a move
   that is an `or` with zero, constants extended at run time, copies between locals), each measured.
2. Size mode: a way for a proven loop to keep its unchecked copy without a loop piece (a stop at its jump
   back going to a re-proof at the loop's entry), which would win back trade 1's loss.
3. Records for each binary in each setting with both engines (started at the end of the second run; see
   "Measured with the engines" below if it finished), Node and Bun in Chrome with the compact lookup, and the
   memory each engine uses for compiled code (wasmtime: machine code bytes; Chrome: not measured, nothing
   in the runner reads V8's code space yet).
4. A call between modules in Chrome (the Chrome runner needs to split translations).
5. Translation speed on the largest binaries (Node 65 to 99 s, Bun 137 to 165 s): not profiled again in
   this run.
6. Static counters for each binary: not recorded yet.
7. Check that Chrome's eager-compile timings cover compiling every function.

## M5, first run

Fixes from the check of M4's second run:
- Code the program writes while running now runs (the JIT pattern). The stand-in's load code hook translates
  from guest memory as it is at that moment, not from the file's bytes: from the blob's code range when the
  address is in it, otherwise from the run of pages that allow running code around the address (within what
  the lookup table covers). Both runners' tables now cover the whole blob, not only its code. Test
  `tests/programs.rs` `written_code_and_null_calls_match_linux` (`programs/written_code.c`: writes `add`,
  `mul`, calls them in a loop, then writes `sub` over `add` and calls it again; -O0 and -O2 against Linux at
  none, full, bounded and mixed, and in Chrome at full and bounded). Found on the way: the C library's
  `__clear_cache` (zig's compiler-rt) leaves out the `dsb ish` after `ic ivau`, and real ARM64 Linux then ran
  the old code (printed 1499000, not 1498000); the program uses Arm's full sequence itself.
- A jump or call to an address with no code (null or not allowing running code) is now a memory fault
  (`Exit::MemoryFault`, access `Execute`, pc and address the target), counted and warned as "memory fault",
  SIGSEGV (status 139) as on Linux, in both runners. Test: the same one, with `programs/null_call.c` (Linux
  139, "before"; ours the same at every look-ahead and in Chrome, one memory fault counted). `Stopped::NoCode`
  is left only for an address that allows running code but has no slot in the runner's table (layer 1 uses a
  one-slot table for this, so control that lands outside its instruction stops there; layer 1 maps an execute
  fault to "landed", as its reference does with SIGSEGV at pc).
- The changes test now keeps "may run code" as placed on every page (the stand-in drops no pieces when a page
  stops allowing it, plan section 7), so its changes still touch reading and writing only.
- The share of the changes test's run at full (M4 second run): the figure still comes from the instrumented
  copy in scratch; its patch is now kept at `pyro-parked/wcontainers-scratch/translator/m4r2-share/share.patch`
  (apply to commit 94db52f). It is approximate: a block's whole length is counted when it is entered, even if
  it faults or leaves early. Not put in the repository: a per-block counter in the translator for one figure
  would be measuring-only code with no other use.
- Pieces made at run time from the middle of a function (after a lost stack, a go to address, a forgotten
  piece) lose what full pieces recognise: switch tables and loop proofs. Counted and warned, never hidden, but
  slower: the check measured `mine_deepthrow -O0` with 273,040 "jump through a register not recognised" at
  full against 4,095,513 at Bounded(7) and Bounded(256), and 1,456,194 "loop not proven" in Chrome at
  Bounded(256), none at full. Recorded under "Known gaps"; `measure_bound` times only bench.c.
- Chrome's figure for translating and loading one piece at run time now includes the engine's compile: the
  measurement runs Chrome three ways (V8 flags given only to the measuring run). Load 8 to 10, release:
  bench -O2 at bounded 256, 26 pieces: wasmtime 2572 us a piece; Chrome 543 us compiling at first call (not
  timed), 692 us with its baseline compiler eagerly (`--no-wasm-lazy-compilation`), 3915 us with its optimising
  compiler alone (`--no-liftoff` too). hello -O2, 55 pieces: wasmtime 3669 us; Chrome 729, 924 and 6947 us.
  At bounded 16 in wasmtime: bench 128 pieces at 1051 us, hello 127 at 975 us. (bench made 15 pieces at 256
  in M4's figure and 26 now; not looked into.)
- The Chrome runner now refuses (throws) to put code made at run time over a piece its table holds, as the
  wasmtime runner asserts.

What M5's first run built:
- `src/binaries.rs` (test crate only; `object` 0.40 and `gimli` 0.34, both established crates): each binary is
  taken once from its Docker image into `target/binaries` and read as the translator's input: the blob is the
  whole loaded image (every loadable segment in place); the code range is from the first section of
  instructions to the end of the last (the runnable segment also holds headers and read-only data, which would
  only grow the lookup table); starts by means: the entry point and DT_INIT/DT_FINI; function symbols of both
  symbol tables; every FDE of `.eh_frame`; the init, preinit and fini arrays (through their relocations);
  addresses in data that point into code (what RELATIVE, ABS64, GLOB_DAT and JUMP_SLOT relocations and RELR
  places hold; in a file that is not moved, every aligned word of its data segments).
- Translations can be several code modules (`Translation { modules }`). The translator emits every piece for
  one module first; if that passes an engine limit (1 GiB a module less 64 MiB, a million functions less a
  thousand), it groups pieces in order and emits each group for a module of its own, where direct calls to
  pieces of other modules go through the lookup (the dispatcher). A piece's loop pieces stay in its module. A
  function whose body passes 7,654,321 bytes or 50,000 locals is cut in two (more starts) and all is done
  again. Test-only entry `translate_in_modules` (measure feature) splits small programs into tiny modules:
  `cut_functions_and_split_translations_match_linux` runs hello, differences, exceptions and many_blocks in
  modules of 4 KiB and 64 KiB against Linux and none (registers and memory).
- A function of more than `FUNCTION_BLOCKS` (2048, a new tuning number) blocks is cut in parts: blocks in depth
  first order, cut into runs, and every place control passes from one run to another made an entry. Test
  `programs/many_blocks.c` (one function of thousands of blocks) at every look-ahead, mixed, and in Chrome.
- Emitting a function no longer recurses down chains of merges: the subtree a node's code ends with is
  emitted by a loop. Bun overflowed the test thread's 2 MiB stack before; a chain of 4000 branches now
  translates on a 256 KiB stack (`valid.rs` `long_chains_translate_on_a_small_stack`; before the change 1000
  branches needed between 512 KiB and 1 MiB).
- Translation speed: making loop pieces cloned the set of every entry once per loop (quadratic). Profiling
  Bun showed it on top; fixed. Java went from 15.8 s to 12.2 s, Node (Alpine's package) from 42.7 s to 27.8 s
  (load 20 to 60, so partly noise); Bun did not finish in 3 minutes before.
- `tests/binaries.rs` `measure_binaries` (one command under "How to run"): for each binary its code and starts,
  the translation at full (time, pieces, found by the translator, modules, size raw and gzipped), every module
  validated (wasmparser), loaded in Chrome (made and instantiated, compiled lazily and eagerly), and compiled
  and instantiated in wasmtime with the bytes of machine code it made.

Measured (release; one run each, load in brackets; "a byte" is per byte of ARM64 code, gzip default level).
"Chrome" is Chrome 157 making the modules and their instances: compiled at first call (V8's default, so the
time is mostly not compiling) and compiled at once by its baseline compiler (`--no-wasm-lazy-compilation`).
wasmtime 50 compiles with Cranelift on all cores at once; "machine code" is the size of what it made.
- busybox (Alpine 3.22): code 704,368 bytes (176,092 instructions); starts 420 (entry 3, symbols 7, unwind 4,
  init/fini 2, data 412); translated in 0.64 s (1.1 MB/s); 3520 pieces, 3100 found by the translator; 1 module,
  38,951,871 bytes (55.3 a byte), 8,247,353 gzipped (19.5 a gzipped byte); wasmtime compile 1.17 s to 54.3 MB
  of machine code, instantiate under 1 ms (load 60 to 70); Chrome 0.01 s at first call, 0.07 s at once,
  instances 1 ms (load 56).
- musl (`ld-musl-aarch64.so.1`): code 370,304 (92,576 instructions); starts 1513 (symbols 1483, unwind 22, data
  15); 0.31 s; 1824 pieces, 311 found; 17,166,135 bytes (46.4), 3,604,109 gzipped (16.9); wasmtime 0.51 s,
  23.8 MB; Chrome 0.00 s and 0.04 s.
- rg: code 2,023,452 (505,863); starts 5073 (unwind 5070, data 1803); 2.44 s (0.83 MB/s); 5963 pieces, 890
  found; 161,005,794 bytes (79.6), 29,171,765 gzipped (30.5); wasmtime 6.69 s, 199.3 MB; Chrome 0.03 s and 0.29 s
  (load 90).
- git: code 2,181,904 (545,476); starts 6084; 2.08 s; 9309 pieces, 3225 found; 126,781,906 bytes (58.1),
  25,384,531 gzipped (20.0); wasmtime 3.99 s, 177.5 MB; Chrome 0.03 s and 0.25 s.
- python (`libpython3.13.so.1.0`): code 2,959,628 (739,907); starts 5878 (symbols 1471, unwind 5538, data
  4059); 2.81 s; 8242 pieces, 2364 found; 161,447,763 bytes (54.6), 29,420,342 gzipped (21.6); wasmtime 5.53 s,
  223.1 MB; Chrome 0.04 s and 0.33 s. Peak memory of the whole measurement of rg, git and python: 7.7 GB.
- java (`libjvm.so`): code 13,193,824 (3,298,456 instructions); starts 46,890 (symbols 46,874, unwind 46,871,
  init/fini 790, data 19,348); 13.25 s (1.0 MB/s); 49,263 pieces, 2373 found; 1 module, 640,062,103 bytes
  (48.5), 110,653,245 gzipped (20.4); wasmtime 30.5 s to 925.0 MB of machine code, instantiate 4 ms; Chrome
  0.16 s and 1.31 s, instances 5 and 18 ms; peak memory 9.3 GB (load 90 to 97).
- node (`node:24-alpine3.22`): code 40,012,104 (10,003,026 instructions); starts 99,365 (symbols 99,247,
  unwind 96,688, init/fini 162, data 24,539); 98.7 s (0.41 MB/s; 65.1 s in an earlier run at load 60); 120,586
  pieces, 21,221 found; 3 modules, 2,990,303,191 bytes (74.7), 529,078,428 gzipped (30.7); wasmtime 89.6 s to
  4.04 GB of machine code, instantiate 11 ms; peak memory 22.2 GB; Chrome refused it: its table of 10,003,027
  entries is past V8's 10,000,000 (load 55 to 100).
- bun (`oven/bun:1.3-alpine`): code 52,050,800 (13,012,700 instructions); starts 64,343 (symbols 551, unwind
  63,291, init/fini 5, data 15,375); 165.3 s (0.31 MB/s; 136.8 s in an earlier run); 143,538 pieces, 79,195
  found; 5 modules, 4,452,427,570 bytes (85.5), 770,736,310 gzipped (35.2); wasmtime 169.8 s to 5.88 GB of
  machine code, instantiate 13 ms; peak memory 27.7 GB; Chrome refused it (table of 13,012,701 entries)
  (load 85 to 100). Before the switch table limit wasmtime refused it: one `br_table` of 360,445 targets.
- go (the `go` command): code 5,860,244 (1,465,061 instructions); starts 17,595 (symbols 14,259, no unwind
  tables, data 6482); 5.32 s (1.1 MB/s); 20,859 pieces, 3264 found; 1 module, 408,650,746 bytes (69.7),
  64,816,902 gzipped (25.2); wasmtime 13.2 s to 578.6 MB, instantiate 1 ms; Chrome 0.07 s and 0.60 s; peak
  memory 6.4 GB (load 98 to 84).
- Every module of all nine validates (wasmparser) and wasmtime compiles and instantiates it; Chrome loads
  seven of nine (all but Node and Bun, refused for the table's size only). The figures above for Chrome
  compiling at once look fast (161 MB in 0.29 s); whether V8's synchronous compile waits for every function
  was not checked.
- Where the bytes go (rg and busybox, before the module split): loop pieces are 45% (busybox) and 64% (rg) of
  the code; the largest piece is 4.76 MB (rg, a function of 12,966 instructions, 367 bytes an instruction).
  Pieces carry much that the engine removes anyway (copies between locals, constant flag parts, a value pushed
  and dropped) and every check's slow path inline. The size work of section 11 has not started.
- Chrome allows 10,000,000 table entries. With one slot per 4 bytes of code, Node's 10,003,026 instruction
  slots cannot be one table, so the compact lookup design is needed.

Cases run on this run's code (all 0 mismatches): layer 2, 2000 cases seed 501, 10 on the CPU, each also at a
bounded N and mixed (671 faulted, 161 left to an address at full, 6572 stacks lost and 6472 pieces forgotten;
release, 104 s, load 24 to 40); `tests/changes.rs`, `tests/lookaheads.rs`, `tests/checks.rs`, layer 1 and
`valid.rs` at their defaults (release); the written code, null call, cut and split tests (release). Last,
every test of the three crates on the final code (debug, Chrome and layer 3's defaults included): 69 passed,
6 ignored (the measurements), 21.5 minutes, load 39 to 99 (the large binaries were measured beside it).

M5, what comes next (most important first):
1. The compact lookup design (section 5: one slot per piece, and an array in memory from code offset to
   slot), built beside the present one and measured on the benchmark set (calls through registers: interpret,
   sort) and in memory; then choose against the engine limits. Node and Bun cannot be loaded in Chrome without
   it. Questions to settle: where the array lives (state memory, per process or per thread; the table is per
   thread instance), who writes it (a code module's start function, or the runner), and pieces made at run
   time (the table must grow).
2. Size (section 11), in its order. First what is free in both modes, each measured by itself on the nine
   binaries and the benchmark set: what the engine removes anyway (copies between locals, `x | 0` for moves,
   flag parts that are constant, values pushed and dropped); loop pieces that hold more than their loop (the
   code after a loop up to the next loop header is copied into every loop piece that reaches it); slow paths
   shared per piece instead of inline at every check. Then the size mode, one trade at a time (for example no
   loop pieces, carrying on in place after a stop; slow paths out of line).
3. For each binary in each mode: output size, the memory each engine uses for compiled code (wasmtime's is
   recorded now as bytes of machine code; Chrome's is not measured yet), and what a call between two modules
   costs (a direct call becomes a call through the lookup; not measured yet).
4. elfconv (the benchmark to beat): not set up yet. It builds with remill and LLVM in Docker; how long that
   takes is not known yet.
5. Translation speed: Node 65 s and Bun 137 s (single thread, 0.4 to 0.6 MB of code a second, against 1 MB/s
   on small binaries). Not yet profiled after the fix above; the module split emits everything twice when it
   splits, and finding functions explores every entry again each round.
6. Counters for each binary: nothing runs yet, so only what translation can count. Not recorded yet.
7. The translator's input for a program's own pieces past the code range (PLT stubs are in it; data words
   that look like code in fixed files make starts that are wrong guesses): look at how many pieces those
   are.

## M4, second run

Fixes from the check of the first M4 run:
- How much of the changes test's run is at full after a handled fault, measured. A copy of the tree in
  scratch (`m4r2-share/tree`, not in the repository) adds to every block of a full or bounded piece, and to
  every piece of look-ahead none, a count of its instructions in the state memory, kept by look-ahead, and
  the changes test prints the full machine's counts (its tallies are the same as without the counts, so the
  run is the same). Seed 21, 1000 random programs and the C programs (3 seeds; release, load 36 to 41):
  random programs, all runs: 67.5% of instructions at full (344,132 of 509,493), 32.3% at bounded, 933 at
  none (the instructions run alone); the runs with at least one handled fault: 60.1% at full (248,314 of
  413,182). C programs (36 runs, every one with a handled fault): 57.4% at full (16,484,157 of 28,733,683),
  42.6% at bounded, 31 at none. What runs at bounded is the rest of the functions in progress at each fault
  (the C programs fault inside long loops of main, so main's rest runs at bounded); every call through the
  table and every direct call between full pieces still reaches a full piece. Before M4 that rest ran at
  none and took over the table entries of every function it reached.
- Events in mixed runs now happen at random points over the whole run: a mixed run stops at every stop
  check (the stop hook raises the flag again each time), and events are injected at random stops among
  them, the first within 8 stops and each later gap drawn up to a quarter longer than the one before, so a
  run has about 4.5 events for each doubling of its stops, spread through it. Each event is a lost stack or
  a forgotten piece, half and half. A run whose mixing seed is a multiple of 4 stops nowhere and has nothing
  injected, so pieces of every look-ahead also meet on their fast paths. The deep recursion test now runs
  mixed too (seeds 0 to 2, as every program test).
- Programs that switch stacks themselves, tried (`programs/fibers.c`: a fiber on its own stack switched with
  setjmp and longjmp, from the check; `programs/coroutines.c`: two coroutines switched by a hand-written
  context switch as coroutine libraries do, yielding from inside a recursion). Both at -O0 and -O2 match Linux
  at none, full, bounded and mixed, and at full in Chrome (`stacks_switched_by_the_program_match_linux`).
  What a switch costs (`switching_speed`, 2,000,000 round trips, -O2, best of 3, load 39 to 62): fibers
  native 0.031 s, wasmtime 0.291 s (9.3 times), Chrome 0.851 s (27 times), about 65 ns more per switch than
  native in wasmtime and 205 ns in Chrome; coroutines native 0.067 s, wasmtime 2.59 s (38 times), Chrome
  10.96 s (163 times), about 316 ns and 1362 ns more per switch. The plan expected each switch to take "go to
  address". It does for the hand-written switch (12,004 "left to an address" for 6002 switches at 3000
  round trips, each switch a return that does not go where its caller expects, plus one or two "continued
  after stack loss"). With setjmp and longjmp it does not: musl's longjmp ends in `br x30`, so each switch is
  a "jump through a register not recognised" (6002 at 3000 round trips), which carries on through the table
  without leaving the WebAssembly stack, and the stack fills and is left to the bottom loop now and then (19
  "engine stack nearly full").
- A function whose own frame estimate at full is past the stack budget no longer leaves at its entry for
  ever: the entry compares the callers' depth (the parameter), not the callers' depth plus its own frame, with
  the budget, so a piece called from the bottom loop (depth 0) always runs, and what it calls with its own
  frame on top leaves as before. Test `a_frame_past_the_budget_matches_linux` (`programs/big_frame.S`: 150
  loops through x0 to x27, so thousands of locals, calling itself three deep; none, full, bounded and mixed
  against Linux, and at full "engine stack nearly full" counted): before the fix it ran for ever at full
  (stopped after 120 s), after it passes. Bounded and none were not affected (none checks the parameter).
- The wasmtime runner sets no limit on instances, tables and memories in its store (wasmtime's default is
  10,000 instances), so runs with many lost stacks, forgotten pieces or pieces made at run time go on.
- The first run's final suite had 3 ignored tests, not 2 (`measure_translating_at_run_time` was new).
- "address above the host's cap" (`Reason::AddressAboveHostCap`) is never raised by the translator, the
  runners or the hooks: it counts the kernel refusing a request for address space above what the host gives
  (plan section 6: mmap, brk and the like), and the stand-in kernel has no such request. A guest access to an
  address past guest memory is not a request: it reads the always-zero byte after the permission table and
  is a memory fault with exact registers, as an access to an unmapped page is on Linux. With 32 bit memory
  hosts (not built, see "Known gaps") the translator would raise it too.
- The stack budget check: the kept check (in the stop check's branch) measured no difference from no check
  at all in wasmtime and 4% in Chrome, both inside this machine's 10 to 15% noise, so no other way of
  knowing (a counter in memory or a global, which also needs a decrement at every return) can be shown
  cheaper here; not built.
- The default N against others, measured (`measure_bound`): the benchmark set (`bench.c`, SCALE=1, -O2)
  translated at bounded look-ahead from its first instruction, every other piece made at run time, best of 3
  in wasmtime (release, load 87 to 110, so the differences under about 15% are not settled): N 16, 20.3 s
  (0.47 s of it making 133 pieces); N 64, 10.5 s (35 pieces, 0.19 s); N 256, 8.2 s (17 pieces, 0.13 s);
  N 1024, 8.5 s (12 pieces, 0.08 s); full, 8.7 s. So N 256 runs as fast as full here and well ahead of 64;
  kept. Only one program; what larger programs reached at run time do at each N is not measured.
Cases run on the final code (all 0 mismatches): the program tests touched (deep recursion, the big frame,
unwinding, switched stacks; each also mixed with seeds 0 to 2; debug, 191 s, load about 50). Events in those
mixed runs are now spread over the run, for example exceptions -O2 stopped 4,146,830 times with 26 stacks
lost and 30 pieces forgotten, deep -O2 151,321 stops, 18 lost and 24 forgotten, jumps -O0 5916 stops, 13
and 14 (the check saw 2 to 8 stops in a run of its programs). Layer 2, 1000 cases seed 31, 10 on the CPU (release,
68 s, load 103 to 57): 357 faulted, 81 left to an address at full, 3110 stacks lost and 3105 pieces forgotten
in mixed runs. Layer 3, 40 Csmith programs seed 15, 2 at once, 1 also in Chrome, and the hand-picked
programs: 34 compared, 6 did not end in time on Linux (208 s, load 103 to 76; how much of that the stops at
every check add was not measured). `tests/lookaheads.rs`, `tests/changes.rs` and `tests/smoke.rs` at their
defaults (release). Last, every test of the three crates on the final code (debug, Chrome and layer 3's
defaults included): 66 passed, 5 ignored (the measurements), 12.2 minutes, load 77 to 85.

## M4, first run

Fixes from the check of M3's fifth run:
- FRSQRTS on the fast path gave the default NaN for the smallest denormal times an infinity: the branch for an
  overflowed fused sum (-a/2 * b + 1.5) is now taken only when b is finite; with b infinite the infinite sum is
  exact. Test `tests/layer1.rs` `step_edges_match_the_cpu` (FRECPS and FRSQRTS, 2s, 4s, 2d, s, d, denormals
  against infinities, flags kept and not, against the CPU): failed before the fix, passes after. Layer 1 on the
  four FRECPS/FRSQRTS forms: seed 4711 at 400 cases (the check's failing replay) and seed 4712 at 5000 cases: 0
  failing (release, load 41 to 72).
- The fifth run's flake "after the fix" count (about 10,200 fresh-container runs) rests on the leftover file
  names of the workers' last runs (run-1-3416.bin, run-3-3422.bin), not on a summary the script wrote: the
  workers were stopped before fresh.sh printed DONE, and it prints a line only for a bad run, so the three logs
  are empty. The count is plausible but not recorded directly.
- The scratch copy m3-close/tree had a .git file pointing at the real worktree's git directory; removed. Copies
  of the tree for deliberate breaks must leave out .git (copy with rsync --exclude .git).
- The check found that the random changes test does catch the break "kept runs allow 32 bytes" (seed 4713,
  within 781 programs, shrinking included); the fifth run's note said only seed 1 at 1000 missed it.

What M4 built:
- Bounded look-ahead, `LookAhead::Bounded(n)`: one piece per start, holding what the code reaches from it in
  its first n instructions (the next instruction is followed first), with everything full look-ahead does
  inside it (registers in locals, kept flags, blocks and loops, loop pieces kept within the stretch, proofs,
  kept runs, jump tables, direct calls). It leaves where the stretch ends through the lookup. Blocks that would
  make a stretch irreducible are left out of it. `Bounded(1)` is the instruction's piece of look-ahead none
  and nothing else. The default N, `BOUND`, is 256 (a tuning number, not measured against others).
- Code reached at run time with no piece is translated at `LookAhead::AT_RUN_TIME` (bounded, the default N)
  from that address and put in the table at that address only, and only where the table holds no piece (the
  wasmtime runner asserts it). So carrying on after a fault, a go to address, an unrecognised jump or a lost
  stack never takes over the table entry of a function translated at full.
- The exact path (a frame or group check that fails, and floating point's slow path at full) runs the one
  instruction from its piece of look-ahead none, which the load code hook puts in the lookup table's last
  slot, a slot for no address; the hook takes a second argument for it (`exact`). The thread then carries on
  through the table from the next instruction.
- The bottom loop and "go to address" were there since M0; now the bottom loop passes an expected return
  address no return can reach (`BOTTOM_RETURN`, i64::MIN: return targets are untagged), and a return that
  does not go where its caller expects, when that caller is the bottom loop, sets pc and returns to it
  instead of throwing. Each such return is counted and warned as "continued after stack loss": a function
  that was in progress carries on without its WebAssembly frame (after a lost stack, a go to address, or a
  stack left for being nearly full).
- Deep recursion: every piece takes one more parameter, an estimate of the engine stack its callers use
  (`frame_bytes`: 64 bytes, 8 per parameter and per i32/i64 local, 16 per vector local). A piece passes its
  own estimate on to what it calls and its parameter to what it tail calls. At the entry of a piece that
  calls or leaves to others, in the same branch as the entry stop check, a depth past `STACK_BUDGET` (128 KiB,
  a tuning number) leaves to the bottom loop (`UNWIND_DEEP`, counted and warned as the new reason "engine
  stack nearly full", 13) with exact registers and pc at the entry, so the WebAssembly stack starts empty
  again; the frames given up continue one by one as they return to the bottom loop. Look-ahead none checks
  at every piece's entry. The engine's own stack overflow is never relied on.
- The stack is lost: a runner can make the thread leave at a stop and carry on from its registers in a fresh
  thread instance (new lookup table, the code translated ahead of time instantiated again, nothing translated
  since kept): `Paused::lose_stack` in the wasmtime runner.
- Across the layers (`src/mixed.rs`): a run with the program's code at none, at bounded of a random N from its
  first instruction and random other starts, and at full, each address's piece picked at random among them;
  code at run time at a random look-ahead (1, a random N, or the default); and at random stop checks the stack
  lost, an address forgotten ("no piece yet"), and the stop flag raised again at random. Layer 2 runs every
  case at bounded of a random N (ahead of time and at run time) and mixed; layer 3 runs every Csmith and
  hand-picked program at a bounded N and mixed; `tests/lookaheads.rs` runs C programs at random N with every
  piece but the first made at run time; the changes test (permissions changed at random points, faults) now
  carries on at bounded after each handled fault. All against none in the same engine (registers and memory),
  and layer 2 and 3 against Linux.
- Programs with setjmp and longjmp (`programs/jumps.c`), C++ exceptions (`programs/exceptions.cpp`, zig's
  libc++ and libunwind) and Rust panics (`programs/panics.rs`, the standard library, catch_unwind): at -O0 and
  -O2, against Linux at none, full and bounded, mixed with events injected (two seeds), and at full in Chrome
  (`tests/programs.rs` `unwinding_matches_linux`). Built with `flat.ld`, which now keeps the unwind table's
  index in a section of its own (the unwinder finds it through its program header; before, it was merged
  into read-only data and C++ and Rust crashed), places thread local data, and keeps libc++'s replaceable
  operators (`__lcxx_override`) in the code with their bounds.
- Deep recursion against Linux on an 8 MiB stack (blobs now say their stack size, `Blob::stack_bytes`, and the
  reference maps it): C 150,000 calls deep at -O0 and -O2, and a million calls deep through 8 byte frames
  (`programs/deep_small_frames.S`), at none, full and bounded in wasmtime and at none and full in Chrome
  (`deep_recursion_matches_linux`). The million-deep run at full left a nearly full stack 1831 times and
  continued 999,726 frames from the bottom loop; the whole test took 3.2 s in a debug build before Chrome
  was added (load about 60).
- Adding code to a running program, proven in wasmtime (every look-ahead test above) and in Chrome
  (`tests/browser.rs` now runs every program at bounded from its first instruction, every other piece added
  while it runs, and checks some were).

The changes test after a handled fault (the check's hand-over): before M4 the run carried on at none from the
faulting instruction and that code took over the table entries of every function it reached. Now it carries
on in pieces of bounded look-ahead put only at their own addresses (asserted on every load), so calls through
the table, and every direct call between full pieces, still reach full pieces; what runs at bounded is the
rest of each function that was in progress at the fault, and what was in progress at a lost stack. What
share of the run's instructions ran at full was not measured (nothing counts instructions by piece). Tally at
full (debug, load 50 to 90): 64 random programs seed 1, 181 faults handled, 133 pieces made at run time and 39
instructions run alone after a check failed; the C programs, 1483 faults handled, 434 pieces made at run time,
31 run alone; 1000 random programs seed 21 (release), 3294 faults handled, 2057 pieces made at run time, 933
run alone. The same 1000 programs made 18,470 changes against 45,316 in the fifth run: changes are made at
the full run's stops, and after a fault it no longer stops at every instruction as none did.

Deliberate break: a return to the bottom loop that returns before setting pc, at full: layer 2's mixed runs
fail on seed 1 within 64 cases (a stack lost at a stop).

Cases run on the final code (release unless said; all 0 mismatches): layer 2, 2000 cases seed 2, 20 on the
CPU, each case also at bounded of a random N and mixed (641 faulted, 133 left to an address at full, 1815
stacks lost in mixed runs; 128 s, load 87 to 46); the changes test, 1000 random programs seed 21 and the C
programs (3 seeds) (39 s, load 113); layer 3, 200 Csmith programs seed 14, 3 at once, 2 also in Chrome,
each also at a bounded N and mixed: 173 compared, 27 did not end on Linux in time (336 s, load 87 to 30);
`tests/lookaheads.rs`, 40 random N for each of five C programs, seed 2 (35 s, load 35 to 31). Last, every test of
the three crates on the final code (debug, Chrome and layer 3's defaults included): 64 passed, 3 ignored (10
minutes, load 27 to 34). The unwinding and deep recursion tests: as above.

Measured (release, best of 5 unless said; load in brackets):
- The stack budget check, on the call benchmarks (fib, sort), every check against the same without it.
  As its own branch at the entry (best of 3, load 47 to 82): fib wasmtime 2.268 s against 2.151 (5% slower),
  Chrome 2.049 against 1.980 (3.5%); sort 1.141 against 1.139 and 1.080 against 1.104 (no difference). In the
  stop check's branch, kept (load 70 to 101): fib wasmtime 2.262 against 2.276 (no difference), Chrome 2.099
  against 2.021 (4%); sort 1.153 against 1.145 and 1.083 against 1.078 (no difference). Identical runs vary
  by 10 to 15% on this machine, so differences under that are not settled. A counter in memory or in a global
  (which needs a decrement at every return too) was not built or measured.
- Translating and loading one piece while a program runs (translation, the engine's compile and
  instantiation, as the runner's counter times it; load 82 to 101): at bounded 256, bench 15 pieces, 4.1 ms
  each in wasmtime and 0.87 ms in Chrome; hello 52 pieces, 6.7 ms in wasmtime and 0.71 ms in Chrome. At
  bounded 16 in wasmtime: 1.1 ms and 0.95 ms. Chrome compiles lazily, so its compile is paid later, at
  first call, and is not in its figure. An earlier run (load 44 to 50) gave 5.3 and 3.4 ms in wasmtime and
  0.89 and 0.74 ms in Chrome at 256.

## M3, close-out, second run (fifth run)

Fixes from the check of the fourth run:
- Layer 2 let accesses reach below its area, which since the fourth run is in the blob after the code and the
  switch tables: a group from near the area's start pre-indexed down by up to 64 bytes an access, and a walk
  of Q register pairs rounded its step after fitting it, which made a negative step longer (-17 became
  -32). Stores then overwrote the program's code or tables, and Linux and ours ran garbage. Now a group (and
  the accesses around a call) starts as far into the area as its accesses reach below their start
  (`lowest`), and the pair's step is rounded towards zero. The area now ends on a page boundary after zeros,
  and layer 2 checks after every program that the blob before the area is as built (`area_offset`). With
  the generator fix undone, that check failed seed 4 within 1000 cases; with it, the checker's seed 4243 at
  6000 cases and seeds 1, 2, 3, 4, 5, 6, 7 pass (under "Cases actually run").
- A loop of structure loads and stores post-indexed by their size (`ld1 {v0.16b-v3.16b}, [x1], #64` and the
  like) is now proven: `memory_access` gives them `PostIndex` by their size, so the proof sees the base as
  an induction, groups keep following the base, and a pop by one at sp moves the frame. Post-indexing by a
  register stays an offset of 0 with the base written, as before. Tests: `tests/checks.rs`
  `structure_loops_are_proven` (ld1 of four, st4, ld4r; failed before: "loop not proven" 0 for a loop
  running off the blob, now 1) and `structure_frames_off_the_stack` (frames made by st1 at sp and half
  popped by ld4 post-indexed, 1000 turns and 20,000 that run off the stack, against Linux).
- The reference runner's own failure (status 101 and its message on stderr) now fails the case in
  `programs::native`; before, it was only caught by differing from our result.
- Status corrections: the flake loop gave 4 failures in 20,000 runs, not 5; the summary no longer cites the
  first M3 run's layer 1 run as the current code's.

- Live fault fixed: a kept run allowed for accesses of at most 32 bytes (`LONGEST_ACCESS`), but LD1 to LD4
  and ST1 to ST4 of three and four Q registers access 48 and 64 bytes, so such an access starting in the
  last 63 bytes of a kept run ran onto the next page unchecked. Now each kept run allows for the longest
  access that shares it (`kept_longest`, the largest `memory_access().bytes` among them), and the access
  check asserts its size fits; `permission_exact` asserts a length of at most a page (the table's bits
  reach only the next page). The other places a size is used already took the true size: groups (at most a
  page in all), the frame check (a page at a time), proofs over a loop's range (`len` of the access or
  group). Tests written first, both failed before the fix: `tests/checks.rs`
  `structure_loops_running_onto_a_forbidden_page` (ld1, st1, ld4, st4 of four, ld1 and st3 of three
  registers, steps 16, 32, 64, against Linux; full faulted two turns late) and `tests/changes.rs`
  `longest_access_onto_a_page_changed_at_a_stop` (a proven loop of 64 byte loads and stores, the page after
  made to allow nothing at its first stop; full ran to the end). `denied_at_the_first` now stops full at
  every check only up to the change, so kept runs live after it.
- Layer 2 now makes vector accesses: groups and walks load and store one Q register, a pair (LDP/STP Q)
  and structures of three and four registers (LD1/ST1 of several, LD3/ST3, LD4/ST4, base only or
  post-indexed), walks also index a Q register by a register; accesses through sp include LDP/STP Q and an
  LD4 at sp. With the kept-run fix undone, layer 2 failed on seeds 1, 2 and 3 within 1000 cases.
- A loop whose proof fails at its entry is counted and warned as the new reason "loop not proven" (12,
  `Reason::LoopNotProven`, at the loop's address): one warn hook call where the copy with checks runs, once
  per entry, nothing per access. Test: `tests/checks.rs` `loop_not_proven_is_counted` (a loop proven, 0;
  one running off the blob, 1). A kept run that misses is not counted (ruling; see "Departures").
- The changes test now changes pages of memory on the stack's side that is no frame: layer 2's area moved
  from main's frame into the blob (main keeps a frame of the same size for its accesses through sp), so
  groups, walks, kept runs and proofs through x29 and x18 have changes made under them. Tally, 1000 random
  programs, seed 21: 45,358 changes, 4926 faults handled (the eleventh run: 2000 programs, 88,517 changes,
  5418 faults). Deliberate breaks, 1000 cases, first failure counting shrinking (run in a copy of the tree,
  scratch `m3-close/breaks.sh`, load 25 to 110): proof 156, 53, 95 (seeds 1 to 3; before 231, 248, 139);
  kept run 221, 257, 143 (before 254, 325, 283); seed 1 only: groups not ended at a system call 625, loops
  with a system call counted quiet 171, frame kept below sp 346, groups not ended at a call 180, loops with a call counted quiet 42. The new break "kept runs allow 32
  bytes" passed 1000 programs of the changes test on seed 1 (it is caught by layer 2 and both directed tests).
- The reference flake is found and fixed: it was the runner, not Linux. The runner is a static executable
  at 0x200000, and Linux puts its heap (brk) at a random place up to 1 GiB after it. Now and then that place
  is inside the range a blob is then mapped to, so `map` failed (`MAP_FIXED_NOREPLACE`: "File exists") and
  the runner panicked. With `panic = "abort"`, musl's abort raises SIGABRT, which the first process of a
  container ignores, and then ends itself by a fault, so Docker reported status 139, as if the guest had
  ended by SIGSEGV. Evidence (scratch `m3-close/flake`, the blob of Csmith 2000136, a runner printing the
  file's size and hash, and at a fault the address, registers, words at pc and /proc/self/maps): 20,000
  runs in four containers (load 26 to 92): 4 failed, status 134 (the runner not the first process), each the
  map panic with the heap inside 0x400000 to 0x455000; about 10,000 runs each in a fresh container as layer
  3 runs them (five at once, load 49 to 117): 3 failed, status 139, the same panic. Fixes: the runner is
  linked at 2 GiB (`--image-base=0x80000000` in `reference/.cargo/config.toml`), above every address a test
  gives the guest, so its heap never reaches a blob or a stack; and a panic of the runner now ends it with
  status 101 (a panic hook calling `_exit`) after its message on stderr, never a signal a guest could end
  with, and `programs::native` fails the case on that status with that message (fifth run). After the fix (the same printing runner, linked at 2 GiB): about 10,200 runs, each in a fresh
  container, three at once (load 27 to 113), all ended 0 with the checksum; 300 runs in one container, the
  same. While looping, a script of mine ran two workers on one output file, which lost output and looked
  like a Docker fault; it was not.
- Vector and floating point registers in locals, measured on their own: `floats.c` now has four workloads
  (`WORKLOAD` 1 to 4: an n-body step in double precision, a single precision matrix product with the sum
  innermost, the product in i, k, j order which clang vectorises, and sums of absolute differences of
  bytes, also vectorised), longer at SCALE=1, and `floats_speed` times each in wasmtime and Chrome. Best of
  3, seconds at full (load 67 to 90 for both): with locals / with v0 to v31 read and written in the thread
  block (scratch `m3-close/no-vector-locals.py`): n-body wasmtime 0.83 / 1.45, Chrome 0.55 / 1.60; matrix
  wasmtime 1.11 / 1.11, Chrome 1.23 / 1.66; vector matrix wasmtime 0.62 / 0.83, Chrome 0.37 / 0.80; byte
  differences wasmtime 0.90 / 1.40, Chrome 0.75 / 1.20. Locals kept. Times native with locals (native
  0.43, 0.24, 0.17, 0.25 s): wasmtime 1.9, 4.6, 3.6, 3.6; Chrome 1.3, 5.1, 2.1, 3.0. The scalar matrix
  product at 4.6 to 5 times native is the slowest; not looked into.

## M3, third run

- Layer 3's long run is one command that runs `WC_LAYER3_JOBS` cases at once (default 1), each case still
  picked and replayed by its number alone; a line every thousand cases shows progress. Measured: 2000 Csmith
  programs, seed 11, 12 at once, in 852 s (0.43 s a program; load 15 to 70, the full test suite ran beside it
  for part of the time), against 3.8 s a program one at a time in the first run. At that rate 100,000 take
  about 12 hours. Most of a program's time is Docker (gcc and the reference each start a container) and the
  20 seconds Linux is given before a program that does not end is dropped (12% of programs).
- A layer 3 blob that Linux ends by a signal is now compared (ours must end the same way, at none and full)
  when the program built as an ordinary static Linux executable (`layer3::ordinary`, same compiler, level and
  position, no `flat.ld`, 20 seconds) ends with the same status and output; otherwise the case fails, and the
  report says what three more runs of the blob on Linux gave and how the ordinary executable ended. Found by
  Csmith 12001408 (seed 12, case 1372, clang -O2, position independent), which faults on Linux as an ordinary
  executable too, at -O2 also with an unlimited stack (at -O0 it ran for minutes): the program's own
  undefined behaviour or a compiler fault, not ours. Replayed after the change: compared and matching Linux
  (status 139, no output) at none and full, in wasmtime and Chrome. Csmith 12003654 (clang -O3) and 12004814
  (clang -O1, position independent) were the same kind and match under the rule too (replayed).
- The reference flake came back once: Csmith 12008340 (seed 12, case 8304, clang -O0, position
  independent): Linux ended the blob by SIGSEGV with no output, then three more runs of the same blob gave
  status 0 and the checksum. Load was 30 to 300 during that run. With the first run's 2000136 (also clang
  -O0) that is 2 in about 12,300 programs on Linux, each time on the reference's side only. Not understood:
  the blob mode maps fixed addresses with `MAP_FIXED_NOREPLACE` (an overlap would panic, not fault), and
  nothing in it depends on time but the 20 second alarm. The case fails, as it should; under the new rule the
  ordinary executable decides, so a flake like this can only fail a case, never pass a wrong one. At the
  pass bar's 100,000 programs, expect about 16 such failures, each to be replayed with `WC_LAYER3_FROM`.
- Instructions of features we do not offer now decode as `Unsupported`, not `Undefined`: both raise illegal
  instruction at the instruction, but these are counted and warned as "unsupported instruction reached"
  (the project's rule for anything unsupported). Two kinds: words of a form in Arm's list whose condition
  holds on a CPU with every feature but not on ours (`forms::of_feature_not_offered`: the encryption
  extensions, scalable vectors, pointer signing loads and branches, FlagM, MTE and so on), and encodings inside
  a form we present that Arm's decode makes undefined without a feature (64 bit PMULL, half precision fixed
  point conversions, the SSBS, DIT, TCO, RNDR, RNDRRS and CNTVCTSS registers, MSR to SSBS, DIT, SVCR and TCO,
  and the MTE and persistence cache operations). The second kind is one list in layer 1
  (`layer1::of_feature_not_offered`), which layer 1 leaves out of the comparison (the reference CPU has
  most of these features) and which `every_offered_form_is_translated` checks the decoder against both ways.
  Words in no form at all stay undefined and uncounted, as on any CPU. New tests:
  `features_not_offered_are_unsupported` (AES, SHA, PMULL, SVE and the second kind),
  `words_of_forms_not_offered_are_undefined_or_unsupported`, and `smoke`'s
  `instructions_not_offered_are_illegal_and_counted` (end to end: illegal instruction at the word, registers
  kept, counted once; a word in no form not counted). Layer 1 on the 13 forms concerned, 1000 cases each,
  seed 8: 0 mismatches.

## M3, second run

- Floating point's slow path at full leaves the piece. `Emitter::float_mode` now returns whether the meaning
  writes both paths (`float::paths` is the one place meanings use it). At none it pushes the mode and the
  slow path is in place, as before. At full the instruction tests the mode (kept in a local) and, when it is
  set, writes back and tail calls the code module's new function `exact` (`F_EXACT`, so pieces start at 12),
  which saves the passed registers and `pc`, warns "left to an address", asks `load_code` for code at none
  there (replacing a piece there, as after `UNWIND_GO_TO_EXACT`) and dispatches to it. The WebAssembly stack
  is kept, unlike the throwing `UNWIND_GO_TO_EXACT`. Nothing of the instruction may be written before the
  test; the full emitter asserts it (`every_form_is_valid` runs every form at full through it). The software
  fused multiply-add on hosts that need it stays in place (it is not a slow twin but the only path there).
  `differences_match_linux_on_other_hosts` now also asserts that full leaves the piece for it.
- Measured (`floats_speed`, wasmtime, full, best of 3): 4.41 times native before (load 6), 2.43 to 2.45
  after (three runs, load 4 to 5; again 2.42 to 2.45 at load 20 to 23). The first run's experiment with the
  slow path made `unreachable` gave 2.5, so the test that stays costs little.
- Vector registers in locals at full, measured alone (an experiment not kept: at full, vector registers read
  and written in the thread block, as at none): `floats_speed` 3.84 to 3.85 times native, against 2.42 to
  2.45 with locals (best of 3 each, load 17 to 23). Locals kept.
- Position independent blobs are now loaded away from where they are linked (`programs::MOVED`, 0x23456000;
  `Blob::load`). Layer 3 builds them as static position independent programs (`-static-pie -fPIE -z
  norelro`; `flat.ld` keeps their dynamic sections), whose start (musl's `rcrt1`) relocates itself. It finds
  where it is through `AT_PHDR`, so `flatten` places the ELF file's program headers after the blob (read with
  the `object` crate) and both runners and the reference give `AT_PHDR`, `AT_PHENT` and `AT_PHNUM`
  (`layout::initial_stack`; the reference takes two more arguments). Every C program blob now gets them, as
  Linux gives them. `AT_BASE` alone does not work: Alpine's musl then reads an ELF header at that address.
  Layer 3 now also fails a program that Linux ends by a signal, so a blob that did not relocate itself cannot
  match by faulting the same way on both sides.
- Chrome samples for both layers. Layer 3: `WC_LAYER3_CHROME` Csmith programs (default 1) also run in
  Chrome at none and full and are compared with Linux. Layer 1: `sample_matches_the_cpu_in_chrome` runs
  `WC_LAYER1_CHROME` cases a form (default 1, from `WC_LAYER1_SEED`) in Chrome and compares each with the
  CPU through the same comparison as the wasmtime run. The browser runner got an instruction case mode
  (`browser::cases`): many one-instruction cases one after another in the same page, memory and instances,
  each with its thread state and data area given and read back, ending at a system call. A page runs 1000
  cases (`CASES_A_PAGE`); a page given 6075 at once crashed. Found on the way: the page read the thread state
  after a system call's result had gone into x0; it now keeps the state from when the thread stopped, as the
  wasmtime runner does.
- The layer 3 failure of the first run (Csmith 2000136, clang -O0, SIGSEGV once on Linux): its blob ran 160
  more times on the reference, 8 at a time at load 16: 160 good. Not reproduced, not understood.

## M3, first run

What is built:
- Decoding by Arm's forms (`wcontainers-arm64/src/decode_forms.rs`, `decode_simd.rs`): `forms::form(word)`
  gives the form a word is on the CPU we present, and its name and fields give the instruction. The first
  path (yaxpeax-arm) keeps the families it had. A word of an offered form that neither path gives an
  instruction for is undefined (a reserved field value; layer 1 checks each against the CPU), no longer
  "unsupported".
- New instructions: CRC32; single instruction atomics (every `memop`, CAS, CASP); exclusives (LDXR, STXR and
  pairs, plain and ordered) as a compare and exchange against the remembered address, size and value;
  LDAR and STLR; CLREX; system registers FPCR, FPSR, TPIDRRO_EL0, the counter (CNTVCT_EL0 through a clock
  hook, CNTFRQ_EL0 1 GHz), CTR_EL0, DCZID_EL0 and the identity registers as Linux answers them for the
  CPU we present (MIDR 0x410fd083, a generic Arm core; ID_AA64ISAR0 shows CRC32 and atomics; PFR0 FP and
  vectors without half precision; every other identity register zero; CRm 0 other than MIDR, MPIDR, REVIDR
  and CRm 1 raise illegal instruction, as Linux does); DC ZVA (64 bytes), DC CVAU/CVAC/CIVAC (a read check),
  IC IVAU (a read check and the new "code changed" hook); MSR to PSTATE fields is undefined (none is open
  to EL0 on the CPU we present under Linux). A system call clears the exclusive monitor.
- Scalar floating point (every floatdp1/2/3, compare, conditional compare, select, immediate, conversions
  to and from integers and fixed point, FCVT between half, single and double), and every AdvSIMD form: one
  `Instruction::Simd { op, shape, .. }` covers the same, misc, shift, three-different, by-element, across
  and scalar pairwise groups; plus permutes, TBL/TBX, scalar DUP and LD1-LD4/ST1-ST4 (multiple, one lane,
  replicate, post-indexed). Meanings: `simd.rs` (integer: WebAssembly SIMD where it has the operation, else
  element by element on i64; FPSR.QC is always kept), `float.rs` and `float_simd.rs` (floating point),
  `memory.rs` (atomics, exclusives, cache maintenance, structures). The old VectorBinary/Unary/Across are
  gone; their forms are in `Simd`.
- Floating point has two paths. Fast: WebAssembly's own operations (exact for numbers, as FPCR is at its
  default whenever it runs). Slow: `exact.rs`, Arm's rules (not-a-number choice, flush to zero, default
  not-a-number, alternative half precision, underflow when the exact result is tiny, overflow when rounding
  gives the largest number, the estimates) over `rustc_apfloat` 0.2.3 and `ieee-apsqrt` 0.1.1 for the arithmetic,
  through the new `float` hook, one call per element. The slow path runs when FPCR is not zero or the
  program has written FPSR (a CPU-side word, `FLOAT_TRACKING`, is set then; until then the cumulative flags
  are not kept), and always for fused multiply-adds on a host whose relaxed multiply-add does not fuse.
  The runner counts it as "software fused multiply-add" (FPCR zero, flags not kept) or "exact floating point
  requested". Reading FPSR while its flags are not kept is counted and warned too.
- The start-up probe (`wcontainers-arm64/src/host.rs`): a module whose one export answers whether relaxed
  multiply-add fuses and whether 34 not-a-number cases come out as Arm gives them. Its answer
  (`wcontainers_arm64::Host`) is a new parameter of `translate` and part of the cache key. On a host whose
  answer is no, a fix-up after each arithmetic operation puts Arm's not-a-number in. Conversions between
  precisions always work out the not-a-number themselves. On this Mac (wasmtime 50) both answers are yes.
- The translator's contract: three new hooks (`float`, `clock`, `code_changed`), and 32 bytes of the
  CPU side's own after the abi's thread block (`THREAD_STATE_SIZE`: the float tracking word and the
  exclusive monitor). Both runners give them; the browser translator exports `probe_module`, `float`,
  `float_status` and `float_reason` for the page's hooks.

Tests:
- Layer 1 covers every offered form (`covered` is now "the form has a presented word"). Cases carry FPSR and
  FPCR both ways (the reference sets them in the signal frame): half the cases in the fast mode, the rest
  with flags kept, half of those with FPCR's bits random. In the fast mode the cumulative flags of FPSR are
  not compared (they are not kept there by design), QC is. New value strategies: single, double and half
  precision edge values and not-a-numbers with payloads.
- `tests/browser.rs` runs `differences` too: matches Linux in Chrome at none and full (the page's float hook
  calls the browser translator's `float`).
- `tests/layer1.rs` `designed_answers_and_faults`: the values we answer by design, FPCR keeping only its
  bits, and alignment faults of unaligned atomics, load-acquire and exclusives.
- `tests/decoding.rs` `every_offered_form_is_translated`.
- `tests/valid.rs` `every_form_is_valid`: 16 words of every form, at none and full, on a host needing every
  fix-up: valid WebAssembly (relaxed SIMD now in the validator's features).
- `wcontainers-arm64/tests/exact.rs`: the software path on known cases.
- `programs/differences.c` (`tests/programs.rs`): division by zero and overflow, every rounding conversion of
  edge values, fused multiply-add single and double, not-a-number bits through add, multiply, divide, max,
  minnm and fmadd, the four rounding modes, each exception flag through fenv, flush to zero, LSE atomics,
  CASP, an LDAXR/STLXR loop, a failing STXR after CLREX, C11 atomics, the counter, DCZID, DC ZVA, cache
  maintenance and IC IVAU, pointer signing and branch marks, printf of numbers, qsort, strtod and vector
  loops. Built at the five levels (3 distinct blobs), at none and full: output equal to Linux, full equal
  to none in registers and memory. `differences_match_linux_on_other_hosts`: -O0 and -O2 translated for a
  host that does not fuse and needs the fix-up: output equal to Linux, both slow paths counted.

Layer 3 (`src/layer3.rs`, `tests/layer3.rs`): Csmith 2.4 programs (`--max-funcs 8 --max-block-depth 4`)
and the hand-picked programs (hello: printf; differences: number printing, qsort, strtod and section 8),
built with clang (zig cc) or gcc 15.2 (Alpine's, in the Docker image `wc-translator-gcc`, made once by the
test) at -O0, -O1, -O2, -O3, -Os and -Oz, as flat blobs with fixed position code (`-fno-pic`) or position
independent code (`-fPIE`, data through the global offset table), linked at the fixed load address. At none
and full: output and status equal to Linux, full equal to none in registers and memory. A Csmith program
that does not end on Linux in 20 seconds (the reference's blob mode now has that alarm) is not compared,
and counted. `flat.ld` keeps unwind tables as read-only data now, as gcc's start files refer to them.
(Since the second run position independent blobs are loaded away from their link address; see there.)

Measured (floating point, `tests/programs.rs` `floats_speed`, ignored, one command; `programs/floats.c`
at -O2 SCALE=1: an n-body step in double precision and a single precision matrix product; wasmtime, full,
best of 3, load 13 to 31): 4.9 times native as first built (native 0.101 s), 4.3 to 4.5 with the float mode
kept in a local at full (kept). Experiments not kept, each one run at load 15 to 165: with the mode test
taken out, 2.3; with the test kept but the slow path made `unreachable`, 2.5; with one hook call in the
slow path instead of one a lane, 4.2. So the cost is any call on the slow path inside the hot function, not
the test itself: the next step is a slow path that leaves the piece (a tail call, or carrying on at none
from the instruction) instead of calling the hook in place. Vector registers are locals at full since M2;
how much locals gain for them alone is not measured (there is no variant without).

Found by layer 1 and fixed on the way (each was a wrong meaning): selects with their operands swapped in
saturating 64 bit add and subtract, signed shifts by register, half precision signs, rounding to infinity
on overflow, polynomial multiply; a mask built as `cond - 0`; the denormal normalisation of FRSQRTE; FRECPX
of denormals; unsigned saturation of unsigned 64 bit sources; apfloat not raising overflow when rounding
gives the largest number; AHP not-a-numbers keeping their sign. Found by `differences.c`: the saturation
local reused uncleared at full (meanings must clear the locals they read; fresh locals at none hid it).

Layer 1 exclusions added, each with its reason in `excluded`: store exclusives whose status register is
also transferred or the base, and load exclusive pairs into one register (constrained unpredictable);
half precision encodings of the fixed point conversions and 64 bit PMULL (FEAT_FP16 and FEAT_PMULL, which
the reference CPU has); system registers and PSTATE fields of DIT, TCO and CNTVCTSS (features the
reference CPU has); MTE and DC CVAP/CVADP cache operations (likewise); registers we answer by design
(CNTFRQ, CNTVCT, CTR, DCZID, the identity registers). Atomics, ordered accesses and exclusives get aligned
bases, as the reference CPU runs unaligned ones (it has FEAT_LSE2, and even its exclusives do not fault);
the faults are checked by `designed_answers_and_faults`. The reference CPU's answers that settled these
were probed once (scratch `m3/`): it runs DIT, TCO, CFINV/XAFLAG/AXFLAG, CNTVCTSS and the MTE cache
operations, reads CTR_EL0 as 0x9444c004 (IDC set, TminLine 9), CNTFRQ as 24 MHz, MIDR as 0x610f0000.

## S1, close-out, third run (eleventh run)

- Fixed a missed fault at full: when sp moved back up (`add sp, sp, #N`), `full.rs` kept the area the
  pop left below sp as a frame known to be allowed, so later accesses there (`ldur x9, [sp, #-8]`) were
  not checked, also after a system call or stop that changed that page. Memory below sp is no function's
  frame, so the plan's exception does not cover it. Now `shift` drops what is left below sp. The contract's
  text says so too ("at or above the stack pointer"). Tests in `tests/changes.rs`:
  `below_sp_after_a_pop_across_a_syscall` and `below_sp_after_a_pop_in_a_loop` (change at a stop). Both
  fail without the fix.
- Made the changes test strong at calls, the half of the rule the tenth run left weak. With breaks F
  (groups not ended at a call, `checks`) and G (loops holding a call counted as quiet, `quiet`), the old
  generator passed 1000 cases on some seeds. Changes:
  - every layer 2 function now saves and restores x28, x16 and x18 (its frame's first 32 bytes, so
    accesses through sp start at 32 and frames are at least 544 bytes). A group through x28 can then hold
    a call (`Chunk::Call` with `around`, half of calls), and loops counting in x16 (`Chunk::Loop`) and
    walks through x18 whose base moves can call each turn (4 in 10 of them);
  - a "writer" function that makes a system call and returns. Calls go to it or to a numbered function,
    half and half. So calls are in every program, and a call often leads to a change. Before the writer,
    F and G were caught late or not at all: F at 1942 (counting shrinking) on seed 1, passed 1000 on seed 2,
    caught at 552 on seed 3; G passed 1000 on seed 1, caught at 2268 and 780 on seeds 2 and 3;
  - a new layer 2 shape, `Chunk::Below`: move sp to its page's start, make and pop a frame there, then
    access below sp straight away, after a system call, or in a loop. `changes.rs` now also picks the
    page below sp's page as a page near a pointer;
  - two directed tests, `group_across_a_call` and `loop_holding_a_call`, where the callee makes the
    system call. They fail at once under F and G.
  Old layer 2 seeds give other programs again.
- The random test, 1000 cases each, seeds 1, 2 and 3, against all seven breaks. Numbers are the case of
  the first failure, counting shrinking. Patches are in scratch `s1-close3/break.py`, run with
  `breaks.sh`, at load 10 to 104. Every break was caught on every seed:
  - A (proof survives a stop): 231, 248, 139.
  - B (kept runs survive a stop): 254, 325, 283.
  - C (groups not ended at a system call): 479, 475, 305.
  - D (loops with a system call counted as quiet): 263, 282, 270.
  - E (frame kept below sp): 54, 216, 92.
  - F (groups not ended at a call): 176, 378, 162.
  - G (loops with a call counted as quiet): 169, 285, 332.
  F and G on the checker's seeds 4242 and 777, which passed before: F 265 and 428, G 141 and 403.
- Not changed: a failed proof and a kept run that misses are still not counted (see "Departures"); the
  orchestrator should rule on it.

## S1, close-out, second run (tenth run)

- Fixed a missed fault at full: the bounded index proof (`proof.rs`, `index_bound`) took the last
  instruction writing the index register to be a narrow zero extending load and so bounded it by 255 or
  65535, without asking whether the register was the load's destination. A post- or pre-indexed load
  writes back its base, so `ldrb w9, [x14], #1` passed as a bound on the pointer x14 and the loop's copy
  without checks read a page that allows nothing. Now the load must load into that register; the same
  fix covers the 32 bit comparison path. Test: `tests/checks.rs`, `index_written_back_by_a_narrow_load`
  (both forms, against Linux, none and full); it fails without the fix (full exits 0 where Linux and none
  get SIGSEGV). Nothing in the generators made this shape; not added to them.
- `tests/changes.rs` was not strong at system calls: two breaks of the system call half of the rule were
  not caught (C: groups not ended at a system call, `full.rs` `checks`; D: loops holding a system call
  counted as quiet, so they get kept runs and proofs). Causes found: the layer 2 area (and so every
  access through x29) is on the stack, which changes leave alone (frames); a group was never split by a
  system call; no loop over memory held a system call; changes at system calls came only one in `every`;
  and 3 runs in 4 raised the flag at every pause, so after each system call the stop check forgot the
  kept runs anyway. Changes made:
  - every system call now makes a change (stops still one in `every`);
  - near-register changes pick among registers that point at pages a change may touch (not frames), and
    pick the register's own page half the time, otherwise the page before or one of the two after;
  - the plan's `stop_everywhere` became `stops`, the chance that the full run raises its flag again at
    each pause (1, 1, 1, 0.5, 0.1 or 0), so some system calls are followed by a stop and some are not;
    the runner's `Paused` gained `raise_stop_flag`;
  - layer 2: a group may hold a system call between its accesses or just after them (half of groups),
    a walk may hold one after its access each turn (one in four), and the plain system call op is twice as
    common. The registers the call sets (x0, x1, x2, x8) are kept off the group's base and the walk's
    index and step. Old layer 2 seeds give other programs again.
  - two directed tests in `tests/changes.rs` (`group_across_a_syscall`, `loop_holding_a_syscall`): a
    page made to allow nothing at the first system call, full must fault as none does. Each fails at once
    under its break.
- The random test against all four breaks, 1000 cases each, seeds 1, 2, 3, first failure counting
  shrinking (patches and `breaks.sh` in scratch `s1-close2/`, run in a copy of the tree, load 5 to 29):
  every one caught on every seed. A (proof carried on in place after a stop): 200, 136, 138. B (kept runs
  survive a stop that carries on in place): 619, 114, 60. C: 408, 403, 316. D: 184, 80, 76. Before this
  run C and D passed 1000 to 5000 cases. In between (before walks held system calls) D passed 1000 cases
  on seed 3; and with the flag raised at every pause in only 1 run in 4, A and B took longer (B passed
  1000 on seed 1), which is why `stops` is 1 in half the runs.
- Not counted: a proof that fails (the loop runs its copy with every check) and a kept run that misses
  (the access is checked exactly). Both are the ordinary checked path that every loop took before S1, not
  a slow twin, so no `Reason` was added; a counter on them would need a host call on every loop entry
  whose proof fails or every page a walk crosses, in code S1 measured for speed. Noted under "Departures".
- The shared crate's change in b0df319 is now recorded (see "Shared crate").

## S1, close-out (ninth run)

- The rule the dropped checks rest on (now in `contract.rs`'s documentation, "When the permission table
  may change"): what a piece has found out about page permissions (a check another covers, a kept run,
  a proof over a loop's range, a bounded index) holds only until the next point where its thread can stop:
  a stop check whose hook runs, a system call, or leaving `run`. The one exception is the plan's own: a
  frame checked when sp moved down holds until the next function entry, for memory at or above sp only
  (since the eleventh run; before, the code also kept what a pop left below sp, see the third run). So whoever changes a process's
  page permissions must first bring every other running thread of that process to a stop (raise its flag
  and wait for its hook, or for it to leave `run`), as a real kernel interrupts the other processors
  before it changes a page table. With one host thread this happens by itself; the later host-threads
  work must do it.
- The section 9 test, `tests/changes.rs` with `src/changes.rs`. The run at full makes changes and records
  where; the run at none, stopping at every instruction, makes the same changes at the same points and
  must end the same. A change is up to three pages, each set to nothing, read, write, read and write, or
  everything; 60% are the page a register points at, the one before it or the two after it (so they land
  on what a loop's proof or a kept run covers, and on the page after the one being accessed), the rest
  any page of the blob, the page after it, the stack below sp or the page below the stack. Pages from
  sp's page to the top of the stack are left alone (frames, the plan's exception). Changes came at one
  pause in 1, 4, 32 or 256 (one in 16 or 1024 for the C programs), at stop checks (the full run stopped
  at every check in 3 cases in 4) and after system calls; the tenth run changed this (see above). A fault on such a page is handled as a program's
  handler would: the page is made readable and writable and the thread carries on from the faulting
  instruction, up to 64 times a run, so one run meets many faults and every one is compared: why the
  thread came back (fault address and kind), every register, then output, status and every byte of
  those pages at the end. A system call point is found again by its count; a stop point by how often
  the thread had come back before it plus its registers and a hash of that memory (a handled fault
  returns to the same registers and memory, which first matched the wrong stop; found by the test
  itself). Programs: layer 2's random programs, and walks (-O0, -O2), integers, tables and bench (-O0,
  -O2, the small sizes), each with several seeds.
- Layer 2's generator has two new shapes for it: a system call op (a write of nothing to standard
  output, so stops and checks after a system call are exercised mid-function and the CPU agrees), and
  a lookup loop that goes back to its head through a switch whose cases are the head and the way out (a
  switch with a backward case, so a stop carries on in place inside a loop with kept runs). Before the
  second, no test program reached that path. Old layer 2 seeds now give other programs.
- Shown to fail by breaking the code on purpose: (A) a proven loop's copy without checks carrying on in
  place after a stop at its jump back, instead of going to the loop piece that proves again (patch
  `s9/break-proof.patch`): with the final test, caught at the first failing case on seeds 1, 2 and 3
  (within 20, 6 and 28 cases counting shrinking; full ran past a fault none took, or faulted later).
  (B) a stop that carries on in place leaving the kept
  runs alive (`full.rs` `stop_check`, patch `s9/break-kept-run.patch` in scratch): not caught by 3000
  cases before the backward switch shape existed (that path was never reached), caught within 232, 122
  and 154 cases (seeds 1, 2, 3) after.
- The switch that leaves checks out (`Omit`, `translate_omitting`) exists only with the translator
  crate's `measure` feature; `translate` always translates with every check. Only the test crate and the
  browser translator turn the feature on. Cargo merges features in one build, so a crate built in the
  same build as the test crate sees `Omit` too, but must still call `translate_omitting` by name.
- Search with only the permission check left out (5.6 times native in wasmtime, against 2.0 with no
  check and 2.0 with every check), looked into for about half an hour, best of 3 to 5, load 6 to 13:
  reproduced (0.66 to 0.73 s against 0.24 for no check and for every check). It is not a cost of any
  check. The hot loop's machine code is the same as every check's proven copy (both hold the stop flag
  load), 19 to 22 instructions a turn; no slow path is taken (no counter moves); `sample` puts 65% of
  the samples on the instruction after the loop's first data dependent branch. Changing only what the
  piece does at its entry (a few stores to unused state memory, an experiment not kept, patch
  `s9/search/pad.patch`) changes how Cranelift allocates registers in that function: 10 of 11 such
  builds stay at 0.66 to 0.73 s and one runs at 0.245 s, with the loop's running sum kept on the stack
  instead of in a register. So it is an artefact of how Cranelift compiled that one function in that
  variant, together with this CPU; why that code is three times slower was not traced (no hardware
  counters here).
- Output size, bench -O2 SCALE=1, all workloads, 4836 bytes of code, at full, each S1 commit built in a
  scratch clone (`s9/size.sh`; bytes of WebAssembly, then gzipped at the default level):
  - before S1 (d0e700d): 111,975 (23.2 per byte of code), 27,653 gzipped.
  - registers in the thread block (5ab1eed): +27,411 to 139,386 (28.8). Every register outside the
    locals is a load or store with its own offset, instead of a global.
  - throwing through a helper, the module's own globals (2300a89): +316.
  - re-entering the piece at its loops after a stop (7e8be55, replaced later): -4,322.
  - kept page in loops (2382c4a): +3,381.
  - the flags bit in the return address (87bb69e): +3,092 to 141,853 (29.3).
  - loop pieces (af2566e): +188,027 to 329,880 (68.2), 69,297 gzipped. The largest by far: each loop
    gets a piece that holds the loop and what follows it up to another loop's header, so code is copied
    once per loop that reaches it.
  - kept runs of up to eight pages and the write back before loops (1f7dfee): +10,708.
  - the table's place from the module's globals in a kept run's check (86f24d5): -140.
  - the proof over a loop's range and its copy without checks (dc7af11): +8,254; outer loops proven too
    (c7b40ab): +1,690.
  - two argument registers instead of four (93b955e): -10,479.
  - the lookup in place at calls through a register (ceaf05f): +541; the switch's case from the
    register (bd4adcf): +54.
  - bounded indices proven (ce0fb6b): +13,382 to 353,890 (73.2), 78,378 gzipped. Unchanged since.
  So of the growth from 23.2 to 73.2 bytes per byte of code, loop pieces are about 78%, registers in the
  block about 11%, proofs (both kinds) about 10%, kept runs about 6%, and the rest small or negative.
  Nothing was changed for size.

## S1: run speed at full look-ahead

Tools for it: `cargo run --release -p wcontainers-translator-test --bin machine-code -- bench -O2 <dir>
SCALE=1 WORKLOAD=<n>` writes the code module and wasmtime's compiled form (`objdump -d` it; function 8 is
the first piece, the tool prints which is which). For V8's code: `nix shell nixpkgs#nodejs_latest -c node
--print-wasm-code --no-liftoff --no-wasm-lazy-compilation -e 'new WebAssembly.Module(require("fs").readFileSync("<file>.wasm"))'`
(node 26, V8 14; Chrome's own build has no disassembler). The benchmarks take `WC_BENCH_VARIANTS`,
`WC_BENCH_REPEAT` (Chrome runs the program that many times in the same instances, each timed) and
`WC_CHROME_JS_FLAGS`.

Done so far, each measured (wasmtime unless said; times native, every check / no check; Apple silicon,
load in brackets; the machine is shared, so native times themselves moved by up to 50% between runs):
- Step 1, registers in the thread block. Registers that are not in locals are read and written at their
  fixed place in the thread block (`state.rs`), not in imported globals. The thread module has no
  registers, `save_state` and `load_state` are gone (the block always holds the registers when a hook runs
  or `run` returns), runners read and write the block. Machine code (wasmtime): the block's place plus the
  state memory's base sits in one register for the whole piece and every register is `ldr/str [xN, #off]`,
  no bounds check, nothing reloaded; V8: the same, the base reloaded from the instance once per loop turn
  (one load), no bounds check. Before / after (load 8 to 12): fib 9.4 / 8.2, sieve 3.2 / 2.2, hash 1.33 /
  0.98, sort 5.5 / 5.5, interpret 8.6 / 8.6, matrix 7.7 / 7.1, search 8.5 / 5.5. Chrome (second run in
  the same instance): fib 6.7 / 5.9, sieve 2.7 / 2.1, matrix 6.7 / 5.3, search 6.8 / 4.6.
- Calls, first part. Machine code of fib showed every piece saving all ten callee-saved general registers
  and eight vector registers on entry: in wasmtime a function containing `throw` does that. Pieces now
  call a small function of the code module that throws. Pieces also read the six imported values from the
  module's own immutable globals (one load each instead of two in wasmtime). Together with step 2's
  setting: fib 8.2 to 6.3, sort 5.5 to 4.1, sieve 2.2 to 2.0, search 5.5 to 4.75 (load 6 to 10).
- Step 2, guest memory. The runner's wasmtime engine has `memory_may_move(false)` (the contract now says
  the guest memory's base must never move): the base is loaded once per piece and the bound is a constant,
  so each access is an add, a compare and a select (wasmtime's Spectre guard) before it; before, each access
  loaded base and length from memory. V8 keeps base and size in registers already: one compare and branch
  per access. Measured and not kept: wasmtime with its heap Spectre guard turned off changed sieve, matrix
  and search with no checks by 0 to 6% (load 8 to 11; it is a host security setting, so left on). A 32 bit
  guest memory (addresses wrapped after the permission check, which already faults on anything above the
  table) removes the engine's check entirely: 10 to 27% faster in wasmtime (sieve every check 0.89 to 0.79
  s, matrix 0.66 to 0.43 s, search with no checks 0.24 to 0.17 s, fib 2.5 to 2.2 s; load 8 to 10). Not
  taken: the plan gives a process 64 bit memory where the host has one; a 4 GiB cap is the owner's call.
- Step 3, Chrome's compilers: confirmed. Running each workload several times in the same instances
  (load 8 to 11): matrix with no checks 11.9 times native the first run, then 2.0, 1.96, 1.88; search
  10.7 then 1.90; with every check 19.4 then 6.7 (matrix) and 19.7 then 6.8 (search). Sieve and fib
  were the same every run (their loops' functions are entered many times). With
  `--js-flags=--no-liftoff` the first run is as fast as later ones (matrix 1.85, search 1.63 with no
  checks). Tried and not working: a `metadata.code.compilation_priority` section (the compilation hints
  proposal) asking for every function optimized at once: no change, with no flag or with
  `--wasm-compilation-hints`; `--experimental-wasm-compilation-hints` is not a flag in Chrome 157.
  What an ordinary page can do, built: after the stop hook says carry on at a jump back, the thread goes
  on in a new call, which runs whatever code V8 has for the function by then. The page
  (`browser/run.mjs`) raises the stop flag at 1, 2, 4, ... 1024 ms after the guest starts. Result, every
  check, first run in Chrome (load 13 to 34): matrix 16.7 to 3.7 times native, search 17.0 to 3.8, hash
  2.9 to 1.0; later runs the same as the first. (With the stop checks left out there is nothing to
  re-enter, so "no check" still shows the old first run.) The first run's version entered the function
  itself again and went to the loop through the header of each loop around it (a word in the thread
  block's padding said which loop); in wasmtime those extra ways into loops cost matrix half its time.
  Second run, replaced by loop pieces (`full.rs`, `pieces`): for each loop header of a function, a piece
  that starts there and holds that loop and what follows it up to the header of any other loop, where it
  goes on in that loop's piece. Only a stop check that carries on at a jump back enters one (a direct
  `return_call`), so a loop piece is not in the lookup table and may rely on what its function knows at
  the loop (the frame and the alignment of sp); the function keeps a single way into each loop. Where a
  loop piece would need more entries to stay reducible, none is made and the stop check carries on in
  place, loading registers back. Measured (load 15 to 27, same session): wasmtime matrix every check 0.75
  s to 0.41 s (9.5 to about 5 times native), sieve 0.75 to 0.66 to 0.71, search 0.44 (unchanged); Chrome
  first run matrix 3.4, search 2.1, sieve 1.33 times native (load 25 to 45), the second run the same.
  Size, bench at -O2 SCALE=1: 141,853 to 329,880 bytes of WebAssembly (68 per byte of code), wasmtime
  compile 31 to 35 ms. A first version, whose loop pieces ran on to the end of the function, made
  1,306,873 bytes (one copy of the rest of the function per loop). The thread block's word 804 is no
  longer used. Found by the new test below: an entry check that went on in a new call never made progress
  when the flag was raised at every check, so the entry check carries on in place. Breaking the change
  on purpose (each stop going on in the loop piece of another loop) fails
  `stops_at_every_check_change_nothing`.
- Step 5, the stop check. In wasmtime the stop check now costs about 10% on sieve (1.71 against 1.55 with
  no stop check, load 10 to 23) where it doubled it before; matrix still pays more (0.59 against 0.34 s,
  same run; the second run traced this to the re-entry paths, see step 3). Carrying on no longer reloads
  registers anywhere but after a switch's jump back, at a loop with no loop piece, and after a system
  call: at loops the thread goes on in a loop piece (which loads what the loop needs at its entry), and
  at the entry check, which comes before any register but the passed ones is loaded, nothing is reloaded. This relies on the abi: after `Exit::Stop`, `Resume::Continue` says nothing has
  changed, and the kernel plan's uses of the hook (deliver a signal, switch threads) replace the registers
  (`Resume::FromRegisters`), which is the hook's "leave" answer.
- Step 6, permission checks over a whole loop. An access checked exactly inside a loop that makes no call
  and no system call keeps the page it last found allowed (the fast test: that page and the next allow
  the access); it checks again only when the access is on another page. Accesses of one kind through one
  base register in one loop share one kept page; it is forgotten when the loop is entered from outside,
  on re-entry, and when a stop check carries on in place. Faults stay exact (the full check runs on every
  new page). Measured (wasmtime, load 10 to 19): search 3.9 to 3.4, sieve 1.71 to 1.57, matrix 7.3 to
  6.2; interpret did not move (8.6 to 9.0, noise). The page is also forgotten on entering a loop piece.
  Since the third run a run of up to eight pages is kept instead of one page; see "S1, third run".
- Calls, second part. Before, a piece wrote its flags back at every return (packing NZCV from the lazy
  operands, about 12 instructions). Now the caller sets the low bit of the expected return address
  (`contract::FLAGS_WANTED`) when the flags may be read after the call (the rest of its block is looked
  at: anything but plain arithmetic before the next flag setting counts as reading); the piece writes the
  flags back at a return only then. `BOTTOM_RETURN` is now -2 (even) and the bottom loop passes it with
  the bit. fib, wasmtime, every check: 3.09 to 2.48 s, 6.9 to 6.25 times native (load 11 to 14). Writing
  flags back before a call is still needed: the callee may fault before it sets flags, and the fault must
  see the caller's flags.
- `Omit` and `translate_omitting` are public only with the translator crate's `measure` feature, which only
  the test crate and the browser translator turn on.
- New tests: `tests/programs.rs` `stops_at_every_check_change_nothing` (bench and integers at -O0 and -O2,
  the stop hook raising the flag again before it says carry on, so the thread stops at every check, about
  100,000 to 260,000 stops per program: output equal to Linux, registers and memory equal to none); layer
  2 runs every case a second time at full stopping at every check; `tests/checks.rs`
  `loops_running_onto_a_forbidden_page` (loads and stores with steps 1, 8, 4095, 4096, 4097 bytes running
  off the blob onto a page that allows nothing). Each was checked by breaking the code on purpose: a kept
  page two pages ahead fails the checks test; never asking for the flags after a call fails layer 2 within
  2000 cases (seed 9).

S1 measurements at the end of the first run (commit 413f126, `tests/benchmarks.rs`, best of 3, load 4 to 12;
times native; every check / no stop check / no permission check / no check; Chrome first run in the same
instance, the second run within 2% unless said):
- fib (native 0.40 s): wasmtime 6.38 / 5.52 / 5.69 / 4.96; Chrome 5.63 / 4.92 / 5.17 / 4.73.
- sieve (0.45 s): wasmtime 1.69 / 1.45 / 1.43 / 1.21; Chrome 1.38 / 1.33 / 1.03 / 0.94.
- hash (0.58 s): wasmtime 1.01 / 1.00 / 0.99 / 0.99; Chrome 1.00 / 1.82 then 0.99 / 0.99 / 1.78 then 0.98.
- sort (0.34 s): wasmtime 4.21 / 4.10 / 3.69 / 3.40; Chrome 3.39 / 3.37 / 3.06 / 2.95.
- interpret (0.15 s): wasmtime 8.79 / 8.90 / 2.79 / 2.87; Chrome 8.92 / 8.72 / 2.48 / 2.32.
- matrix (0.08 s): wasmtime 9.26 / 5.97 / 3.67 / 2.57; Chrome 3.29 / 14.5 then 4.06 / 2.17 / 12.7 then 1.91.
- search (0.12 s): wasmtime 3.46 / 4.27 / 2.03 / 2.08; Chrome 3.04 / 13.9 then 2.89 / 1.99 / 11.1 then 1.90.
- Size: bench at -O2: 138,761 bytes of WebAssembly (28.7 per byte of code), 31,094 gzipped (10.8).
- Experiments after it, not kept: (a) the stop checks without the paths into nested loops (every re-entry
  at the start, which is wrong but never taken in wasmtime): matrix 0.39 s against 0.75 s, sieve 0.70
  against 0.75 (load 4 to 9). So in wasmtime the paths into loops cost matrix about half its time. (b)
  re-entering only loops inside no other loop, a stop at a nested loop marking the piece to re-enter at
  the next outer jump back: wasmtime matrix 0.50 s, sieve 0.80, search 0.46; Chrome first run matrix 9.4,
  search 5.5, sieve 1.8 times native (load 11 to 14), much worse in Chrome, so reverted.

S1 measurements at the end of the second run (commit af2566e, `tests/benchmarks.rs` with
`WC_BENCH_VARIANTS=1,5`, best of 3, load 8 to 19; times native, every check / no check; Chrome first run
in the instance). Native times were 15 to 35% slower in this session than in the first run's (fib 0.54
s, sieve 0.54, hash 0.76, sort 0.46, interpret 0.19, matrix 0.10, search 0.12), so ratios may read low:
- fib: wasmtime 5.95 / 4.73; Chrome 4.31 / 3.88.
- sieve: wasmtime 1.44 / 1.19; Chrome 1.37 / 0.97.
- hash: wasmtime 0.99 / 0.97; Chrome 0.96 / 1.69 (no check has no stop to move to V8's optimised code).
- sort: wasmtime 3.92 / 3.64; Chrome 3.50 / 2.77.
- interpret: wasmtime 9.29 / 2.95; Chrome 9.13 / 2.34.
- matrix: wasmtime 4.88 / 2.60; Chrome 2.62 / 11.6 (as hash).
- walk: 0.6 to 0.85 everywhere. search: wasmtime 3.67 / 2.07; Chrome 2.91 / 11.0 (as hash).
- Size: 329,880 bytes of WebAssembly (68.2 per byte of code), 69,102 gzipped (24.1), wasmtime compile 37
  ms.
- Aims met with every check in this run: sieve and hash in both engines; matrix and search in Chrome. Not
  met: fib (aim 4), sort (2.5), interpret (3) in both; matrix and search (3) in wasmtime. For none of
  these does the status file show yet that the rest cannot be removed; see below.

S1, second run, what the machine code and experiments show (wasmtime's code from the `machine-code`
tool; A/B runs alternate the two builds in one session, because native times alone moved by up to 35%
between sessions at load 10 to 45):
- Matrix (wasmtime): with loop pieces the inner loop (7 guest instructions, two loads through x0 and x1
  with a register index) is about 45 instructions a turn. About 30 values are live in it: 19 guest
  registers, and of ours the permission table's place and length, two kept pages, the thread block, the
  guest memory's base, its bound and a zero for wasmtime's Spectre guard, and the operands kept for the
  flags. That is more than the 26 or so registers Cranelift hands out, so it spills: at the jump back 8
  stores and 3 loads to its own stack each turn. Registers the loop loads anew every turn (x6, x7) stay
  alive across the jump back only because the next turn's fault paths write them back. Not yet tried:
  making slow paths not need those registers in registers (e.g. writing such registers to the block as
  they are set inside the loop, or one shared exit per loop).
- Interpret: its stack accesses are `[x9, w10, sxtw #3]` through a copy of sp with a register index, so
  giving x9 the frame's knowledge would not drop them (the index is unknown). Each turn of the dispatch
  makes three kept-page checks (bytecode, jump table byte, stack), each a shift, a compare with a spilled
  kept page and a branch; the untagging of the switch's target and a 75 entry `br_table` are on the path
  too. Why removing the permission check takes it from about 9 to about 3 times native is still not
  explained; it needs an instruction-level profile (`sample` gives none; Instruments' counters or `perf`
  in a Linux VM).
- fib: per call about 150 instructions in wasmtime against 19 natively. In the code: Cranelift's own
  save of ten callee-saved registers, loads of the two imported memories' bases (two loads each) and the
  fixed values, the entry stop check, loading x19, x20, x29, x30 from the block and storing them back
  before every call, the frame check of the new frame (about 10), the alignment check, four guest stack
  stores and loads each with a 64-bit bounds check (add, compare, select), NZCV worked out and stored
  before the call (about 17), and the return check with x30 untagged (4). Measured, A/B, load 10 to 19,
  best of 3, seconds: leaving out the flags written before calls 2.39 / 2.64 (base) against 2.64 / 3.09;
  leaving out the frame check 2.42 / 2.80; comparing x30 before untagging it in the return check 2.73 /
  2.69 (new) against 3.04 / 2.73 (old). None measurable, so none kept. With no check at all fib is 4.7 to
  5.0 times native, so the aim of 4 needs a cheaper call even without checks: the likely cost is the
  chain of stores and loads through the thread block that every call makes for the guest's callee-saved
  registers (caller writes x19 to the block, callee reads it there to save it on the guest stack, and the
  same back). Next to try: pass x19 to x30 as parameters and results (the plan's tuning number); it
  changes the convention of every look-ahead, the dispatcher and both runners.
- Sort: the same costs per call as fib, plus a call through a register (the dispatcher's table lookup)
  to the comparison at every step.
- 64 bit guest memory: a 32 bit memory measured 10 to 27% faster in wasmtime (see step 2) and frees the
  bound and Spectre-guard registers in every loop; the owner's call.

S1, third run (commits 1f7dfee, 86f24d5):
- Interpret's permission cost found and removed. Counting kept-page misses (a counter in the thread
  block's padding, not committed) showed every one of interpret's 1.17 billion checked accesses missing:
  the kept page needed the page after it to allow the access too, and the interpreter's stack sits on the
  top page of the stack and its byte code and jump table on the last page of the blob, each followed by a
  page that allows nothing. So the fast test failed every time and the exact test ran. Matrix had 56
  million misses (its column walk moves to a new page every 5.3 turns), sieve 53 million.
- What is kept now (`code.rs` `keep_pages`, `full.rs` `check`): per kept key two locals, the first
  address of a run of pages that allow the access and the room in it. The check is `address - low <u
  room` (a subtract, a compare and a branch, as before); nothing kept is room 0, which nothing passes.
  After an access passes the full check (fast or exact), the run is that page and the pages after it that
  allow the same kind, up to eight, found with one 8 byte load of the table and a count of trailing zeros;
  room is the run's length less 31 (the longest access is 32 bytes), so an access that starts in it ends in
  it. Faults stay exact: anything outside the run takes the full check. The kept run is forgotten where the
  kept page was. Misses after it: interpret 4,003, matrix 11 million, search 14 thousand.
- Registers that are dirty when a loop is entered and that its own code neither reads nor writes are
  written back to the thread block just before the loop, so the loop's slow paths no longer keep them
  alive (`Plan::written_before`; the backward analysis counts them as needed on the edges that enter the
  loop, so the locals hold their true values there). Matrix 0.643 to 0.621 s, search 0.45 to 0.43 s, others
  within noise (A/B in one session, two rounds each, best of 2, load 8 to 11).
- The check of a kept run reads the table's place and length from the module's globals instead of the
  piece's locals, so loops do not keep them in registers for a check that rarely runs. A/B, two rounds,
  load 8 to 11: matrix 0.597 to 0.555 s, search 0.42 to 0.35 s, interpret and sieve unchanged.
- Tried and not kept: one kept page with the meaning "this page allows it" and a test that the access
  does not run past the page (one local): A/B against the runs, load 7 to 14, two rounds: matrix 0.48 / 0.55
  against 0.62 / 0.53 s, search 0.42 / 0.42 against 0.41 / 0.35, interpret 0.76 / 0.76 against 0.67 / 0.59,
  sieve 0.82 / 0.77 against 0.83 / 0.66. Writing the slow paths' pc from the load address global instead
  of the local: matrix 0.547 to 0.594 s, interpret 0.57 to 0.61 (load 6 to 10), so reverted.
- Checked by breaking on purpose: a run one page too long fails `tests/checks.rs`
  `loops_running_onto_a_forbidden_page`; leaving out the stores before loops (analysis unchanged) fails
  layer 2 within 300 cases.
- Benchmarks after 1f7dfee (`WC_BENCH_VARIANTS=1,5`, best of 3, load 6 to 18; times native, every check /
  no check; Chrome first run): fib wasmtime 6.67 / 5.12, Chrome 5.63 / 4.67; sieve 1.58 / 1.20, Chrome 1.39
  / 0.99; hash 0.96 / 0.93, Chrome 0.91 / 1.17; sort 4.11 / 3.33, Chrome 3.31 / 2.88; interpret 3.82 / 2.90,
  Chrome 3.87 / 2.04; matrix 7.74 / 2.65, Chrome 4.87 / 11.3; walk 0.8 to 1.0; search 2.61 / 1.55, Chrome
  2.12 / 7.26. Size: 340,588 bytes of WebAssembly (70.4 per byte of code), 70,878 gzipped.
- After 86f24d5, every check only (best of 3, load 24 to 50, so noisier): sieve wasmtime 1.61, Chrome 1.43;
  hash 1.00 / 1.00; interpret 4.12 / 4.25; matrix 7.26 / 3.99; search 3.00 / 3.06.
- Against the second run: interpret 9.3 to about 3.8 to 4.1 in both engines; search in wasmtime 3.7 to
  2.6 to 3.0; matrix worse, wasmtime 4.9 to about 7 and Chrome 2.6 to about 4 to 5 (native matrix 0.076 to
  0.080 s in both sessions). Matrix's inner loop spills (see the second run's notes); two locals per kept
  key instead of one add to it. The one-local variant above did not measure better for matrix within
  the noise, and is worse for interpret and search.
- Calls (fib, sort), why passing more registers cannot help in wasmtime: a test module
  (`pyro-parked/wcontainers-scratch/translator/s3/cc/cc.wat`, a function of 14 i64 parameters and 14
  results, compiled by wasmtime 49) shows Cranelift passes only the first four parameters in registers (x4
  to x7, after the two instance pointers in x2 and x3) and the rest on the stack, and returns six results in
  registers and the rest through memory. Our pieces already pass six (x0 to x3, sp, the expected return), so
  sp and the expected return already go through the stack on every call; x19 to x30 as parameters would
  be stores and loads to the stack instead of to the thread block, the same work. fib's remaining cost
  per call in wasmtime (machine code of `fib`, function 9): the engine's stack limit check (5
  instructions), its save and restore of ten callee-saved registers (12 instructions; they hold the
  instance, the thread block, the state memory's base, the load address, the permission table's place and
  length, the guest memory's base and the expected return, all live across the guest's call), the stack
  arguments, the four guest stack stores and loads each with a 64 bit bounds check (add, compare, select:
  24 instructions; a 32 bit guest memory would remove them, the owner's call), and x19, x20, x29, x30
  through the thread block (about 15 loads and stores). With every check off fib is 5.1 times native in
  wasmtime and 4.7 in Chrome in this run, so the aim of 4 is out of reach without a cheaper call
  convention in the engine or a 32 bit guest memory. Tried and not kept: in pieces that call, reading
  every fixed value from the module's globals at each use instead of copying them into locals. fib 2.42 /
  2.41 s against 2.41 / 2.46, sort 1.44 / 1.48 against 1.50 / 1.47 (A/B, load 70 to 160, so only a rough
  answer); the machine code shows why: the piece still saves all ten callee-saved registers, because
  Cranelift merges the reads of an immutable global into one value that lives across the calls anyway.

S1, fourth run (no code change kept; measurements and two experiments):
- Where each check's cost sits now, wasmtime, commit 5cac63e, best of 3, load 12 to 17 (every check / no
  stop check / no permission check / no return check / no check, times native): matrix 7.39 / 5.14 / 2.72 /
  7.62 / 2.66; interpret 3.78 / 3.69 / 2.88 / 4.09 / 2.80; fib 6.25 / 6.33 / 6.35 / 6.80 / 6.01; sort 4.05 /
  3.74 / 3.31 / 3.56 / 2.95; sieve 1.51 / 1.45 / 1.25 / 1.58 / 1.24 (native sieve 0.442 s). So in matrix and
  interpret almost all of the checks' cost is the permission check; in fib the checks cost about 4% and the
  call itself is the cost.
- Matrix in numbers: n = 96, 300 rounds, 265 million turns of the 6 instruction inner loop; native 0.29 ns a
  turn, no check 0.77 ns, every check 2.1 ns. Kept-run misses (11 million, one per 24 turns) account for
  about 2 instructions a turn; the rest is the two kept-run checks per turn and the spills they cause (the
  inner loop is about 56 instructions in wasmtime, 12 of them stores to its own stack and 10 loads, see the
  second run's notes). The way to the aim is the proof over the loop's range (below): with it the loop is
  the no-permission loop, 2.7 in wasmtime.
- Tried and not kept: (a) registers the header of a loop sets before reading them (x6, x7 in matrix) written
  back at every jump back and before the loop, so they are no longer alive across the jump back. The
  machine code shows the two stores and x6, x7 gone from the spills, but the loop still spills the same
  amount (other values took the registers). A/B, two rounds alternating, load 5 to 19, seconds: matrix 0.665
  / 0.554 against 0.609 / 0.552, sieve 0.884 / 0.672 against 0.756 / 0.676, search 0.444 / 0.350 against
  0.354 / 0.348, interpret 0.816 / 0.600 against 0.601 / 0.565. No gain. (b) The kept runs in mutable globals
  of the module instead of locals (so they are not loop values): matrix 0.529 s (unchanged), sieve 1.92,
  search 3.54, interpret 4.40 times native (all worse, load 6). Patch of (a):
  `pyro-parked/wcontainers-scratch/translator/s4/writeback.patch`. (c) The flags left out of the write back
  before calls (wrong, only to measure): fib every check / no check, seconds, wasmtime 2.75 / 2.14 against
  2.40 / 2.00 with them, Chrome 2.21 / 1.96 against 2.15 / 1.88; sort wasmtime 1.52 / 1.15 against 1.42 /
  1.15, Chrome 1.11 / 0.99 against 1.16 / 0.98 (one round each, load 17 to 26). No gain in either engine,
  so the flags are not where fib's time goes.
- fib against the toy, measured again today in wasmtime 50 CLI (toy modules from translator-bench/out,
  compiled at opt-level 2, process start included, best of 3, native 0.365 s, load 7.5 to 12): toy
  `args4+check` (return check only, 32 bit memory, registers in module globals, flags never carried across
  calls, no permission check) 1.25 s, 3.4 times native; `args4+check+flag@loops+callers` (adds the stop check
  at loops and at entries of callers) 1.48 s, 4.05; `locals+check` 1.53 s against `locals+check+mem64` 1.60
  s (the same with a 64 bit memory: 4% slower). So in wasmtime even the toy's call shape is at 4 once it has
  the stop check the plan requires, before a 64 bit memory, permission checks or flags kept across calls.
  Ours: 6.0 with no check, 6.25 with every check (this session). The toy figures in Chrome (2.55 to 2.72 times
  native, `out/poll-results.txt`, load 16) were not measured again.

S1, fifth run (commits dc7af11, 98f61b7, c7b40ab, 68cff43):
- The proof over a loop's range (`full/proof.rs`), the fourth run's plan for matrix. A loop that makes no
  call and no system call and holds no switch is looked at over its own blocks (not those of loops inside
  it). A register written there at one instruction only, by adding a constant or a register the loop does
  not write (add/sub, or pre/post indexing), moves by a fixed step at most once a turn. An exit at a block
  every turn passes, a compare (subs/cmp with an immediate or a register the loop does not write) or
  cbz/cbnz on such a register, bounds the turns: "not equal" with step 1 or -1 (modulo the width), or an
  ordered condition (lo, ls, hi, hs, lt, le, gt, ge) stepping toward the bound. Each access checked exactly or
  as a group whose base and index are such registers (one of them moving, or neither) covers a range of
  addresses over the turns. At the loop's entry the translated code works out the turns and every range,
  and looks up every page of each range in the permission table (eight table bytes at a time). When all
  allow the access it runs a copy of the loop with no check for those accesses; otherwise the loop runs with
  its checks, so faults stay exact. Anything that could wrap (a 32 bit index or counter passing its limit,
  steps above 2^24, more than 2^32 turns, addresses outside the table) makes the proof fail, never pass.
- Where the copy lives matters for wasmtime. With both copies in one function, matrix stayed at 4.5 to 5.4
  times native: Cranelift's register allocator moved values that live through the loop (x0 to x3, sp, the
  expected return, kept runs of outer loops) between registers and its stack at every turn (7 to 10 loads
  and stores a turn, seen in the machine code), even though the copy itself is 15 instructions. Trapping in
  the checked copy (only to measure) gave 2.88. So the checked copy is now only in the loop's loop piece: the
  function runs the proof and either the copy without checks or a tail call into the loop piece, which holds
  both copies. A stop at a jump back now writes back and goes straight on in the loop piece, whose entry
  check calls the stop hook (loop pieces always check the flag at entry), so the loop has no call that
  returns into it. Registers a loop neither reads nor writes are no longer kept in locals through it: they
  are loaded from the thread block on each edge leaving it (the dirty ones are already written back before
  it). In this case Cranelift forwarded the stored value past the loop anyway, so that part did not change
  matrix, but it is kept as the cleaner rule (it did not slow anything measured).
- Loops holding other loops are proven too (sieve's outer loop): only accesses and register changes in the
  loop's own blocks count; the copy is used only when every loop inside has a loop piece, so a stop
  anywhere inside carries on where a proof is made again.
- Measured, wasmtime, best of 3 (times native, every check): matrix 7.4 (fourth run) to 3.00 and 3.09
  (load 5 to 8; 2.63 with no check), sieve 1.69 to 1.42 with outer loops proven (load 8), search 2.9 to 1.75
  to 2.0, hash 1.0. Chrome (first run in the instance, load 5 to 9): matrix 2.11, search 1.73, sieve 1.32,
  hash 1.00.
- Full benchmark run at 98f61b7 (before outer loops were proven), best of 3, load 4 to 12, every check / no
  check: fib wasmtime 6.02 / 5.19, Chrome 5.16 / 4.71; sieve 1.69 / 1.25, Chrome 1.32 / 0.99; hash 0.98 /
  0.97, Chrome 1.00 / 1.59 (no check has no stop to move to V8's optimised code); sort 4.12 / 3.34, Chrome
  3.31 / 2.80; interpret 3.81 / 2.92, Chrome 4.18 / 2.19; matrix 3.00 / 2.63, Chrome 2.11 / 11.3; walk 0.92
  to 0.97; search 1.75 / 1.81, Chrome 1.73 / 9.2. Size: 348,702 bytes of WebAssembly (72.1 per byte of
  code), 74,679 gzipped, wasmtime compile 28 ms.
- Seen and not explained: search with only the permission check left out is 5.8 times native in wasmtime,
  also at the fourth run's commit 50908cb (so not from this run), while with no check at all it is 1.8. Only
  a measuring variant; its machine code for the inner loop is tight, so the time is elsewhere.
- Tests: layer 2 has a new chunk, a walk: a counted loop (every counter form above, 32 and 64 bit, the
  compare before or after the change) over memory through x18, moving its base by indexing or an add, or an
  index read as uxtw, sxtw or lsl, shifted or not, stepping by an immediate or a register, starting near
  2^32 or 2^31 so some wrap, in the area or walking forward onto the page after the blob. `programs/walks.c`
  (nested counted loops, one running onto the page after the blob) matches Linux at every level, in
  wasmtime and Chrome, and full matches none. Checked by breaking on purpose: a proof that always passes
  fails layer 2 and `walks.c`; a range one turn short fails layer 2 within 15 cases (seeds 41, 42).

- After c7b40ab (outer loops proven), every check, best of 3, load 7.5 to 10.6, wasmtime / Chrome first run:
  sieve 1.44 / 1.15, matrix 2.94 / 2.02, search 1.70 / 1.88, hash 0.97 / 0.97, interpret 3.74 / 3.89. Size
  350,392 bytes of WebAssembly (72.5 per byte of code).

S1, sixth run (commits 93b955e, ceaf05f; A/B runs alternate the two builds in one session, best of 3):
- Fewer argument registers passed (the plan's tuning number, `code::ARGUMENTS`). The machine code showed
  wasmtime passing only the first four parameters of a piece in registers (sp and the expected return went
  through its stack on every call), and V8 giving back only two results in registers (x2, x3 and sp went
  through its stack on every return; V8 also has no callee-saved registers, so every value live across a
  call is spilled and reloaded). Now x0 and x1 are passed; x2 and x3 live in locals like x4 and up, written
  back and loaded only where needed. Seconds, every check, four against two, three rounds at load 10 to 26:
  fib wasmtime best 2.40 against 2.09, Chrome 2.03 against 1.92; sort wasmtime 1.39 against 1.26, Chrome
  1.12 against 1.09. One argument register against two (two rounds, load 8 to 25): fib the same (2.00 /
  2.16 against 2.07 / 2.15 s), sort worse (wasmtime 1.27 / 1.38 against 1.26 / 1.28, Chrome 1.17 / 1.20
  against 1.10 / 1.15), so two. Interpret pays a little for it (four against two, two rounds, load 7 to 11:
  wasmtime 0.551 / 0.553 against 0.572 / 0.575 s, Chrome the same); matrix unchanged.
- Calls and jumps through a register look up the piece in place (`full.rs` `look_up`, `call_looked_up`) and
  call it with `call_ref`, going through the dispatcher only when the table has no piece there. Before, every
  such call was a call of the dispatcher, which tail called the piece. Sort (its comparison is called through
  a register), two rounds, load 10 to 16: wasmtime 1.34 / 1.32 to 1.155 / 1.25 s, Chrome unchanged (1.15 /
  1.20 to 1.14 / 1.20).
- Tried and not kept: the dispatcher writing `pc` only on its slow path (sort unchanged, 1.265 / 1.265
  against 1.265 / 1.254 s, load 14 to 24).
- Full benchmark run at ceaf05f (`WC_BENCH_VARIANTS=1,3,5`, best of 3, load 6 to 13; times native, every
  check / no permission check / no check; Chrome first run in the instance): fib wasmtime 5.15 / 4.52 / 3.86,
  Chrome 4.74 / 4.58 / 4.25; sieve 1.47 / 1.19 / 1.19, Chrome 1.17 / 0.97 / 0.95; hash 0.96 / 0.96 / 0.99,
  Chrome 0.97 / 0.97 / 1.54 (no check has no stop to move to V8's optimised code); sort 3.21 / 2.64 / 2.49,
  Chrome 3.15 / 2.92 / 2.51; interpret 4.08 / 3.24 / 3.00, Chrome 4.40 / 2.40 / 2.29; matrix 2.90 / 2.58 /
  2.52, Chrome 2.10 / 2.25 / 12.2 (as hash); walk 0.78 to 0.91; search 2.06 / 5.78 / 2.02, Chrome 1.94 / 2.03
  / 10.7. Size: 340,454 bytes of WebAssembly (70.4 per byte of code), 75,696 gzipped, wasmtime compile 24
  ms. Interpret in Chrome reads 4.2 to 4.4 in every session of this run, with two or four arguments alike,
  against 3.89 in the fifth run's last measurement.

S1, seventh run (commit bd4adcf; A/B runs alternate the two builds in one session, in a clone in scratch so
the checkout stays free: `pyro-parked/wcontainers-scratch/translator/s7/ab.sh`):
- A switch's case is worked out from the jump's register as it is, and the target is untagged only on the
  path where no case is taken (`full.rs` `table_jump`). A tagged value never lands on a case (a top byte that
  is not zero puts it far above every case; one with bit 55 set is unchanged by untagging), so it still
  reaches the lookup untagged. wasmtime's dispatch in interpret loses the four untagging instructions (seen in
  the machine code); V8 still works out the untagged value before its jump table. A/B interpret, every
  check, three rounds, best of 3, load 7 to 18, seconds, before against after: wasmtime 0.606 / 0.584 /
  0.599 against 0.569 / 0.838 (load 17) / 0.593; Chrome 0.623 / 0.681 / 0.642 against 0.630 / 0.647 /
  0.654. Kept: less work, about 2% in wasmtime, no change in Chrome.
- Tried and not kept: the module's own copies of the fixed values made mutable and read again into their
  locals after every call, so that no engine keeps them across a call. wasmtime still saves all ten
  callee-saved registers (Cranelift saves every one the function uses anywhere); V8 spills two values at
  entry instead of five. A/B, three rounds, best of 3, load 9 to 17, before against after: fib wasmtime
  2.073 against 2.143 s, Chrome 1.914 against 1.964; sort wasmtime 1.134 against 1.100, Chrome 1.079 against
  1.064. Mixed and within noise. Patch: `s7/mutable.patch`.
- The floor of a call under the plan's rules (`s7/floor`: `fib.wat`, `variants.py`, `chrome.mjs`). fib as
  bench.c's -O2 code has it, written by hand in WebAssembly with only what the plan requires of a full piece
  and nothing of ours: x0, x1 and sp passed and the expected return as a parameter; x19, x20, x29, x30 in
  the thread block at the piece's edges and around the call; a 64 bit guest memory; the stop check at entry
  and at the jump back; the frame and alignment check when sp moves down; NZCV packed into the block
  before the call; sp compared after the call; the return check with x30 untagged. Then the same with one
  part left out (only to measure). fib(42), native 0.39 to 0.41 s in this session:
  - wasmtime 49 (CLI, memory may not move, as the runner), best of 3, load 9 to 15: every part 1.86 to 1.91
    s (4.6 to 4.8 times native); without the stop checks 1.78; without the frame and alignment check 1.73;
    without the flags 1.74; without the return check 1.75; with a 32 bit guest memory 1.90 (no gain here);
    without x19 and x20 through the block around the call 1.85; with no check at all 1.57 (3.9). Its machine
    code has the same engine part as ours: the stack limit check and the save of all ten callee-saved
    registers on every call.
  - Chrome 157, four runs in one instance, load 6 to 12: every part 1.72 s (4.3); any one check left out
    1.31 to 1.37; 32 bit memory 1.83; no check 1.26. V8 inlines this fib into itself (its code holds the
    callee's body at the call) as deep as its size budget allows, so one check more or less moves it by
    25%. With `--js-flags=--no-wasm-inlining`: every part 2.30 s (5.7), no frame check 1.88, no check 1.62
    (4.0).
  - Ours in the same session (full benchmark below): wasmtime 2.06 s (5.22), Chrome 1.90 s (4.82).
  - So in wasmtime the aim of 4 for fib with every check is below what the plan's call shape itself costs:
    4.7 with every part, 3.9 with no check at all. Ours is about 10% above that floor. In Chrome the floor is
    4.3 when V8 inlines the recursion and 5.7 when it does not; whether V8 inlines any of ours was not
    checked.
- Full benchmark at bd4adcf (`WC_BENCH_VARIANTS=1,3,5`, best of 3, load 6 to 11; times native, every check /
  no permission check / no check; Chrome first run in the instance): fib wasmtime 5.22 / 4.58 / 3.90, Chrome
  4.82 / 4.60 / 4.36; sieve 1.40 / 1.20 / 1.19, Chrome 1.18 / 0.98 / 0.95; hash 0.98 / 0.99 / 0.99, Chrome
  0.99 / 0.99 / 1.45 (no check has no stop to move to V8's optimised code); sort 3.29 / 2.68 / 2.52, Chrome
  3.30 / 2.76 / 2.54; interpret 3.86 / 2.68 / 2.59, Chrome 4.31 / 2.43 / 2.03; matrix 3.11 / 2.56 / 2.50,
  Chrome 2.07 / 2.06 / 11.0 (as hash); walk 0.88 to 0.94; search 1.86 / 5.57 / 1.97, Chrome 1.86 / 1.94 /
  10.1. Size: 340,508 bytes of WebAssembly (70.4 per byte of code), 75,704 gzipped, wasmtime compile 24 ms.
  Raw output: `s7/bench-bd4adcf.txt`.

S1, eighth run (commits ce0fb6b, 33dea4b; A/B runs alternate the two builds in one session, in clones in
scratch: `pyro-parked/wcontainers-scratch/translator/s8/ab.sh`, best of 3 unless said):
- Bounded indices proven (`full/proof.rs` `index_bound`). An access whose base the loop does not write and
  whose index does not move by a fixed step is proven when the index is bounded where it is read: set
  earlier in its block by a load of at most 4 bytes that zero extends, or, when the block does not set it,
  bounded by an unsigned comparison with a constant (lo or ls) on the only edge into the block; for a 32 bit
  comparison and a 64 bit read, what set the register before the comparison must be such a load. The range
  is the base up to the base plus the largest index shifted, and needs no counting exit (accesses that move
  still need one). Loops holding a switch are proven too, unless a case of the switch jumps back, because a
  stop there carries on in place without proving again. In interpret this proves the switch's table byte
  (`cmp w14, #0xb; b.hi; ldrb w15, [x8, x14]`): the dispatch makes two kept-run checks instead of three
  (seen in wasmtime's code: the table byte's load keeps only the engine's own bound).
- Measured first by leaving that one check out (only to measure; two rounds, load 5 to 8): interpret
  wasmtime 0.558 / 0.560 against 0.531 / 0.566 s, Chrome 0.632 / 0.629 against 0.548 / 0.549. The proof
  itself, two rounds, load 8 to 16: interpret wasmtime 0.587 / 0.558 to 0.529 / 0.523 s, Chrome 0.668 /
  0.638 to 0.570 / 0.573; matrix and search unchanged.
- Tests: layer 2 has a new chunk, a lookup: a counted loop whose access is a base plus an index loaded each
  turn from a byte table in the blob or worked out (the counter exclusive-ored in), with or without a guard
  (b.hi or b.hs after a 32 or 64 bit compare), read as uxtw, sxtw or lsl, shifted or not, near the end of
  the blob or in the area, sometimes followed by a switch on a byte of the table; in "tight" cases the
  largest index's access starts exactly on the page after the blob. Checked by breaking on purpose: a range
  covering only the first entry fails layer 2 (seeds 81, 82, 300 cases each); a range one entry short
  passed 1200 cases before the tight cases were added and fails within 400 cases after (seeds 81 to 83).
- Tried and not kept: (a) in `keep_pages`, the count of trailing zeros worked out with a count of leading
  zeros: V8 (node 26) calls out of line for i64.ctz on arm64 (`wasm::word64_ctz` in its code), which spills
  the loop around the slow path; with clz the call is gone and V8's hit path in interpret's dispatch is
  shorter, but A/B two rounds, load 5 to 9, sieve, interpret, matrix and search did not move beyond noise
  (interpret Chrome 0.706 / 0.638 against 0.641 / 0.633 s). Patch `s8/clz.patch`. (b) Only to measure: the
  exact check without its clamp to the table's length and the table's offset (3 of its instructions):
  sort, wasmtime, three rounds, load 14 to 17, best 1.115 against 1.092 s, about 2%. So in sort the exact
  check costs its table load and test, not those. (c) Only to measure: the proof at a loop's entry without
  its table lookups (ranges still worked out): matrix wasmtime 0.240 / 0.241 against 0.224 / 0.231 s,
  Chrome 0.162 / 0.161 against 0.157 / 0.158; sieve wasmtime 0.644 / 0.633 against 0.551 / 0.624 (load 7
  to 17, noisy). Matrix's inner loop is entered 2.8 million times and proven each time; keeping what was
  proven across entries (forgotten at stops, like a kept run) would save at most these few percent.
- fib in Chrome: V8 does not inline our fib piece (its code, node 26 with `--no-liftoff`, keeps both
  recursive calls as calls; the piece is about 1,480 lines of V8 machine code), while it inlined the
  hand-written floor. Ours (4.76) is below that floor without inlining (5.7), and the floor with inlining
  is 4.3, above the aim of 4.
- Full benchmark at 33dea4b (`WC_BENCH_VARIANTS=1,3,5`, best of 3, load 6.5 to 12; times native, every
  check / no permission check / no check; Chrome first run in the instance): fib wasmtime 5.22 / 4.53 /
  3.87, Chrome 4.76 / 4.58 / 4.29; sieve 1.43 / 1.21 / 1.21, Chrome 1.21 / 0.99 / 0.96; hash 0.97 / 0.99 /
  0.98, Chrome 0.98 / 1.01 / 1.51 (no check has no stop to move to V8's optimised code); sort 3.30 / 2.65 /
  2.51, Chrome 3.22 / 2.74 / 2.55; interpret 3.51 / 2.67 / 2.60, Chrome 3.92 / 2.39 / 2.03; matrix 3.06 /
  2.68 / 2.64, Chrome 2.11 / 2.08 / 11.4 (as hash); walk 0.92 to 0.96; search 2.00 / 5.63 / 2.00, Chrome
  1.99 / 2.02 / 10.5. Size: 353,890 bytes of WebAssembly (73.2 per byte of code), 78,146 gzipped,
  wasmtime compile 24 ms. Raw output: `s8/bench-33dea4b.txt`.

S1, what remains after the eighth run (aims with every check, in both engines):
- Met: sieve, hash and search in both engines, matrix in Chrome. Matrix in wasmtime sits at its aim (2.90 to
  3.11 over the last three runs, native 0.076 to 0.082 s), so noise puts it either side; the proof at each
  entry of its inner loop is a few percent of it (see (c) above), the next thing to try for it.
- fib (aim 4): wasmtime 5.22, Chrome 4.76. Shown in the seventh run: the plan's call shape written by hand
  is 4.7 times native in wasmtime with every part the plan requires (3.9 with none; ours with no check is
  3.87, the same), and 4.3 in Chrome only when V8 inlines it (5.7 when not; V8 does not inline ours). So 4
  cannot be reached in either engine without breaking a rule (registers outside the passed ones in the
  thread block at calls, the stop check at entries, the frame check, exact flags at a fault in the callee,
  the return check). Ours is about 10% above the floor in wasmtime; where that goes is not traced.
- Sort (aim 2.5): wasmtime 3.30, Chrome 3.22; no check 2.51 / 2.55, so even with no check it is at the aim,
  and with no check our call shape matches the hand-written floor in fib in wasmtime (3.87 against 3.9). The permission checks cost 0.65
  of a native run: per comparison the callee's two loads are exact table checks, because the plan relies
  on nothing proven before a call after it. The check's clamp and table offset are 2% of it (b above); the
  rest is the table load and test each access needs. No floor written by hand for sort itself yet.
- Interpret (aim 3): wasmtime 3.51, Chrome 3.92; no permission check 2.67 / 2.39. Left: two kept-run checks
  per dispatch (a subtract, a compare and a branch, with the kept run spilled to the stack in both
  engines), on the byte code (an index from data: pc is incremented or loaded from the byte code) and on
  the interpreter's stack (an index from data, sign extended). Neither index has a bound the code shows, so
  neither can be proven ahead; the kept run is the cheapest exact check found. Not measured: a hand-written
  floor of the dispatch with those two checks.
- Not explained, only in a measuring variant: search with only the permission check left out is 5.6 times
  native in wasmtime, against 2.0 with no check.
- 32 bit guest memory: the owner's call; it made no difference to the hand-written fib in either engine.
- Not run yet: the pass bars for layer 1 and layer 2 (long runs for the orchestrator).

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
  - `crates/wcontainers-arm64/arm/a64-forms.txt`: all 4429 A64 encodings from Arm's specification
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
    layer 1 covers, are translated now (`wcontainers-arm64/src/vector.rs`), and layer 1 carries v0 to v31
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

- M2, full look-ahead (`wcontainers-translator/src/full.rs`, `LookAhead::Full`):
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
  - Jump tables (`wcontainers-arm64/src/habits.rs`): a `br` whose target is a base built by `adr` plus a
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
  for days): `WC_LONG_DIR=<scratch directory> nohup nix develop -c crates/wcontainers-translator-test/long-runs.sh > <log> 2>&1 &`.
  Knobs (`WC_LONG_PERCENT`, `WC_LONG_LAYERS`, `WC_LONG_SEED`, `WC_LONG_ACCEPT`, `WC_LONG_JOBS`,
  `WC_LONG_LIMIT`) and the ledger's fields are described at the top of the script and under "P1, the long-run
  driver" and "P2". To carry on after a stop, run the same command again; to keep chunks that passed on an
  older commit, set `WC_LONG_ACCEPT=<that commit>`. It refuses uncommitted changes and runs its own copy of
  the commit. The run under way and its exact command are under "P2".
- The benchmark set against elfconv: `nix develop -c crates/wcontainers-translator-test/elfconv.sh` (needs
  Docker and the image `ghcr.io/yomaytk/elfconv:arm64`; converted programs kept in `target/elfconv`;
  `WC_BENCH_RUNS` best of, default 3).
- Everything: `nix develop -c cargo test -p wcontainers-arm64 -p wcontainers-translator -p wcontainers-translator-test`
  (includes the Chrome test; it needs Google Chrome installed and Docker running).
- Lint the two side packages too: from `crates/wcontainers-translator-test/browser`,
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

1. T3's independent check (see "T3, what is left").
2. The progress count (definition E, the brief) as its own milestone.
3. The literal fix list: arm64's decoders and forms (steps 17 to 20) and the test crate's remaining lines
   (orchestrator's `magic-literals/fix-translator.md`).
4. Refinements of M5 under "M5, fourth run, what comes next".

## Known gaps (behave as the plan says: illegal instruction, counted, warned)

- T3: a switch whose code has no bound check before its table (the compiler proved the index's range, as for
  `k % 48` or an argument of a static function whose every caller it sees) is left unrecognised, as the
  table's length cannot be known. Its cases are never found, so each is made at run time once (bounded
  look-ahead, counted "translated at run time", warned), and the jump to them goes through the lookup with no
  count at each run, because the target lies past the explored function (only the code before the jump).
  Measured with `cut_switch.c`'s `cut` static (as at 486ee285), -O2, full: 1,190 translations at run time, 0
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
  to 59cb1416). Layer 3's Csmith builds (`layer3.rs`, their own command) are left as they are: they still
  link that code, so their floating point (Csmith's programs have little) takes the exact path.
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
  calls through function pointers, common in the C library, paid a host call each time).
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
