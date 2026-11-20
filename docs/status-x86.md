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
  what the third check found (see "X2, run 7"). Run 8 (the same day) fixed what the fourth found (see "X2,
  run 8"). Run 9 (the same day) fixed what the fifth found (see "X2, run 9").
  Not built, proposed for X3: the recorded reference results and layer 1 batching of plan 7.3.
- X3 (x86 at look-ahead none, integers): run 1 (2026-11-18) fixed the sixth X2 check's four findings, made
  the description section format 4 (ARM64's output the same apart from it, shown), built `wcontainers-x86`
  (decoder, block, base integer instructions, the SSE2 integer moves and logic a C library's start needs,
  lazy flags, calls and returns, syscall, the 128-by-64 division, the claim table) and x86's layer 1, which
  passes every form at 1,000 cases a form against the i7-8750H; hello world prints in wasmtime. Run 2 (the
  same day) built the length table from a sweep of every map on the reference, ran the kernel-only forms in
  layer 1, ran x86 in Chrome, and measured the decoder's share of the browser build and the long division
  (see "X3, run 2"). Run 3 (the same day) fixed what the independent check of X3 found: cases that reached
  the reference runner's own memory, `may_vary`, the reserved NOPs' prefixes, and five smaller ones, and did
  the first part of the literal sweep (see "X3, run 3"). What is left for later milestones is under "X3:
  what comes next".
- X4 (x86 at full look-ahead): run 1 (2026-11-18) fixed the X3 check's seven findings and a translator bug
  that lost every x86 callee's flags at full look-ahead; hello world and the benchmark set match the
  i7-8750H at none and full. Run 2 (the same day) adopted the literal check, built x86's layer 2 emitter,
  the flag shapes and direct conditions, switch recognition for both compilers, the loop proofs' comparisons
  and bounded loads, `pushf` and `leave`, x86's check programs, and fixed a translator bug that lost every
  register passed into a piece but not out of it across a return (see "X4, run 2"). Run 3 (the same day)
  timed the benchmark set natively, in wasmtime and in Chrome on the x86 machine in alternating rounds,
  built the frame checks plan 3.4 and 5.3 describe for x86 (one check for a run of pushes, none for a call's
  callee, the return address covered), found why interpret is slow, and ran layer 2 at 10,000 again at a
  new seed: X4's pass bar holds (see "X4, run 3"). Run 4 (2026-11-19) fixed what the independent check of X4
  found: switch tables as the compilers really write them for a 32-bit index, the lazy `Shift` shape and
  the flags of rotates and bit tests, the literal check's closing version merged, and the measurements the
  check found missing (see "X4, run 4"); the whole suite's ARM64 tests that need Docker on the Mac could not
  run, because Docker Desktop would not start, and must be run again. Run 5 (2026-11-19) ran those ARM64
  tests on the rented ARM64 machine, worked the literal list down (x86 crate 328 to 187 baseline lines), and
  wrote into plan 5.6 how a rep string's passes let a stop land between any two elements (see "X4, run 5").
  Run 6 (2026-11-19) gave the two ARM64 layer 1 tests their Mac verdict through a Docker engine of its own in
  a Linux VM on the Mac (Docker Desktop still does not start), ran the whole suite on the Mac that way (every
  test passes), ran layer 2 at 10,000 again at a new seed, and worked the literal list down again (x86 crate
  187 to 142 baseline lines) (see "X4, run 6"). Run 7 (2026-11-19) fixed what the independent check of X4
  found: with look-ahead nothing is emitted or worked out for flags nobody reads (it was, for shifts, rotates,
  bit tests and the carry instructions, against what the plan and this file said), x86's "general registers
  only" programs had SSE in them, bare page sizes in files X4 changed; then layer 2 at 10,000 at a new seed
  and the suite (see "X4, run 7"). Run 8 (2026-11-19) fixed what the next check found: the whole workspace
  did not build (the wasmtime runtime's hook), and the orchestrator's question on 5-level paging had no answer
  here (see "For the orchestrator" and "X4, run 8"). Every part of X4's bar holds.
- X5 (every default instruction): run 1 (2026-11-19) fixed the five findings of the last X4 check (Chrome
  runs at once, the matrix slowdown explained, x86 loop-proof checks and a wider proof, x86 versions of
  ARM64's X4 program tests, the block size's literals) and listed what X5 has left to write (see "X5, run
  1"). X5's own work has not started: none of its bar holds yet. Run 2 (the same day) wrote the integer
  forms (bit scans and counts, byte swaps, xadd, cmpxchg and the pair forms, shld and shrd, adcx and adox,
  loop and its kin, lahf and sahf, popf, clflush, the FS and GS base instructions, xlat, movnti, the fences,
  crc32), rep string instructions on both paths, rdtsc and rdtscp from the clock, cpuid from a designed
  table, and the integer, move and blend instructions of SSE2 to SSE4.2 (see "X5, run 2"). Not yet: the
  float instructions, MXCSR, the segment and far-transfer instructions, `enter`, `fxsave`, the string
  compares of SSE4.2, and the rest of the bar. 864 of the 1,295 default legacy forms are written. Run 3
  (2026-11-19) wrote the float forms of SSE to SSE4.1 with MXCSR's fast and exact paths, `ldmxcsr`,
  `stmxcsr`, the prefetches, the not-a-number probe and its fix-ups, and `rcpps`/`rsqrtps` by plan 5.8's
  formula with its test over all 2^32 inputs (see "X5, run 3"): 944 of the 1,295 written. Run 4
  (2026-11-20) wrote SSE4.2's string compares, `enter`, `maskmovdqu` and the descriptor table instructions
  (`lar`, `lsl`, `verr`, `verw`, `sldt`, `str`, `smsw`, `sgdt`, `sidt`), each checked at 1,000 a form against
  the i7-8750H (see "X5, run 4"): 976 of the 1,295 have layer 1 forms. Left: the segment register and
  far-transfer group, and the rest of the bar. Run 5 (2026-11-19, restarted once) wrote the segment register
  and far-transfer group, ran layer 3 Csmith at 1,000 (0 failing), built and ran layer 1's list of unoffered
  forms (2,992 forms, every encoding, 0 failing at 100 a form), measured the not-a-number probe on x86 and ARM
  hosts and the passed-register lists, fixed `wbnoinvd`, and ran every default form on the i7-8750H at 1,000
  a form with 0 failing (see "X5, run 5"). The bar's layer 1 and layer 3 parts hold. Run 6 (2026-11-20)
  fixed the translator bug that held back the faster passed-register lists (a system call's result left
  stale in the thread block was counted clean at a join) and took those lists, marked four more instructions
  as varying between CPUs, ran the ARM64 layer 1 tests on the Mac, ran 100,000 random instructions in no form
  (which found `movbe` with 66 and REX.W, fixed), and ran the bar again with the new lists: every part of X5's
  bar holds (see "X5, run 6").

## X5, run 6 (2026-11-20)

Machines: the rented x86-64 machine for the replay, the fix, lint and layer 2 and 3; the rented ARM64 machine
for formatting and the facts test; the i7-8750H for layer 1's verdicts (`x5/run6/laptop-bg.sh`, 4 at once);
the Mac for git, the literal check and the ARM64 layer 1 tests (Docker Desktop was up again). Logs in scratch
`x5/run6/`.

Done:
- The bug that held back the faster passed-register lists (c9410112). With in = [rsp, rdi, rsi] and out =
  [rsp, rax], rax lives in a local, and the full emitter leaves a system call's result in that local, marked
  stale in the thread block; but the plan's forward analysis (`dirty_after`) took a system call to leave
  nothing stale. So at a loop head reached after a system call, rax counted as clean, and a later fault or exit
  wrote the value from before the call. Found by shrinking seed 9310 case 10 and reading the piece's module
  (scratch `x5/run6/scratch_bounded.rs`, `dump/`): the piece from 0x400134 at bounded 56. Bounded showed it
  because its pieces start at loop heads. One rule, `dirty_after_syscall`, now serves both. ARM64 and the old
  x86 lists pass the result register in, so their output does not change.
- x86 takes the shorter lists (9e175f79), measured fastest in run 5 (20.9 s against 21.6 s for the benchmark
  set in wasmtime); plan 3.4 and section 11 say so. Not timed again after the fix, which changes code only
  after system calls.
- The host-dependence mark reviewed against the instructions written (2c8903ca): `dpps` and `dppd` (where an
  unmasked exception stops one part way, the flags follow the reference's micro-operations), `maskmovdqu`
  (faults under an all-zero mask are implementation dependent in Intel's manual) and `ldmxcsr` (AMD's
  misaligned SSE mode takes MXCSR bit 17, which Intel refuses) were not marked and now are; `facts.rs` checks
  them and four fully defined float forms. Plan 5.2 lists every kind marked. The address width (5-level
  paging) changes faults of every access and is not a per-instruction mark.
- The browser crate's lock gained the x86 crate's `rustc_apfloat` (36a0cc92), which a build there added.
- Found by the 100,000 instructions in no form (below): `movbe` with both 66 and REX.W (`66 48 0F 38 F0/F1`)
  is illegal instruction on the i7-8750H, where iced decodes the 64-bit form and we ran it (576d974f). The
  decoder now takes it as invalid; two hand-made cases built with iced's encoder compare it with the reference;
  the six `movbe` forms again at 1,000 a form, seed 9364: 0 failing.
- The no-form test aborted at 100,000 cases with proptest's "too many local rejects" (its filter throws draws
  away and the limit was proptest's default); it now allows as many per case as the form tests (604f4258,
  `REJECTS_PER_CASE`). The draws are the same.

Measured:
- Layer 2 for x86 with the shorter lists, before the fix: seed 9310 case 10 fails as run 5 said (rax 1 against
  0). With the fix, 3,000 programs at seed 9310, none on the CPU, 96 at once: 0 failing (38 s, load 1 to 20),
  where run 5 had 367 failing.
- The ARM64 layer 1 tests on the Mac, with the fix and the shorter lists (`arm64-l1-mac.log`, load 6 to 8, Docker Desktop):
  `every_form_matches_the_cpu_arm64` 1,215 forms, 64 a form, seed 1, 0 failing;
  `sample_matches_the_cpu_in_chrome_arm64` 1,215 cases, 0 failing (235 s).
- The literal check exits 0 on the Mac.
- On the rented x86-64 machine with the fix and the shorter lists (`batch1-x86m.log`, load 1 to 80, shared with
  another track's jobs; nothing there reads `may_vary`, so the marks committed after do not change these):
  clippy with warnings denied passes for the five crates; the crates' own tests pass; `--test checks x86_64` 2
  passed; `--test programs x86_64` 14 passed (1,468 s); layer 2 for x86 at 10,000 programs, seed 9370, all
  10,000 on the CPU, 1 in Chrome: 0 failing (162 s; 3,457 faulted, 1,240 left to an address at full); layer 2
  for ARM64 at 3,000, seed 9371, none on a CPU: 0 failing; layer 3 Csmith for x86 at 1,000 programs, seed 9372:
  882 compared with Linux, 118 did not end in time, 0 failing (357 s).
- Every default form on the i7-8750H with the fix and the shorter lists (`l1-run6.log` there, copied to scratch
  `x5/run6/l1-run6-laptop.log`): 1,063 forms, 1,000 a form, seed 9360, 4 at once, 0 failing, 3,768 s; the
  laptop's load was 10 to 78 during, from another project's tests there. In the same job the hand-made cases
  and `the_x86_64_reference_refuses_what_linux_refuses` passed. The Chrome sample failed at "run node": the
  job's command lacked the laptop's node and Chrome (now under "How to run"); run again below.
- On the i7-8750H, the rest of layer 1 (`nf6.log`, `l1-movbe.log`, `l1-nf8.log`, `l1-hand9.log` there and in
  scratch, load 1 to 6): the unoffered forms, 2,992 forms, 100 a form, seed 9361: 0 failing (779 s; of the forms
  the reference has it ran 71,400 cases and refused none). The Chrome sample, 3 a form, 3,189 cases, seed
  9363, Chrome 158: 0 failing. The independent check's 100,000 random instructions, in no form, seed 9362: at
  first 1 failing (the `movbe` above), then the abort above; after both fixes 100,000 cases, 0 failing (290 s).
  The hand-made cases with the two new `movbe` cases: pass.
- The suite at 36a0cc92 plus this file, on the rented x86-64 machine (`suite-x86m.log`, 2,597 s, load 5 to 80,
  shared): clippy with warnings denied passes for the five crates; 97 passed, 13 ignored, 29 failed, all
  expected there: the 25 ARM64 tests that need an ARM64 reference, three of the four the 5-level paging changes
  (`every_form`, the Chrome sample, the refusal test; the no-form test at its default 64 cases passed there),
  and `unoffered_forms_are_illegal_x86_64` (that machine has the features). On the rented ARM64 machine
  (`suite-arm.log`, 1,181 s, load 8 to 18), the ARM64 tests of `wcontainers-arm64` and the test crate: 50
  passed, 2 failed, the two layer 1 tests its manual keeps on the Mac, which passed on the Mac above.
- After the suite only the `movbe` decoding, two hand-made cases and the no-form reject limit changed: clippy
  with warnings denied passes for the x86 crate and the test crate (rented x86-64 machine), the x86 crate's
  tests pass there, the literal check exits 0 (the Mac), and the layer 1 runs above cover the change.

Literals: the check exits 0. One bare literal left the test crate (`100` of the reject limit, now
`REJECTS_PER_CASE`); nothing was added.

So X5's pass bar holds with the lists chosen: layer 1 every default form at 1,000 a form (seed 9360) and the
unoffered forms at 100 a form (seed 9361) with 0 failing, layer 3 Csmith at 1,000 (seed 9372) with 0 failing,
the reciprocal formula over all 2^32 inputs (run 3); the passed-register lists are measured and chosen; the
independent check's 100,000 random instructions (seed 9362) give 0 failing; every instruction whose result
may depend on the CPU is marked (plan 5.2 lists them).

What comes next: X5's independent check, then X6.

## X5, run 5 (2026-11-19)

Machines: the i7-8750H for layer 1's verdicts (4 at once, `x5/run5/laptop-bg.sh`); the rented x86-64 machine
(load 3 to 55, shared with another track's jobs) for builds, lint, layer 2, layer 3, the suite and the probe;
the rented ARM64 machine for the ARM64 tests and the probe; the Mac for git, `cargo fmt`, the literal check
and the probe in its Chrome. Logs in scratch `x5/run5/`. The session restarted once part way; its first part
committed 9014abe3 and 066d7c8c without recording them here, so this section records both parts.

Written:
- The segment register and far-transfer group (9014abe3, `crates/wcontainers-x86/src/segments.rs`): the 38
  forms of `mov` to and from segment registers, `push`/`pop` `fs`/`gs`, `lfs`/`lgs`/`lss`, far `call`/`jmp`,
  `retf` and `iret` of every operand size, with ds, es, fs and gs selectors as thread state (the block's
  `reserved1` became `selectors`, no offset moved), the reference runner loading and reading them, and the
  read orders plan 5.3 now records (a return reads the selector's 16 bits alone of its cs and ss slots). A
  far transfer to Linux's 32-bit code stops with general protection, counted and warned
  (`FarTransferTo32Bit`): this phase has no 32-bit mode. Layer 1 on the i7-8750H, the 38 forms, 1,000 a form:
  seed 9201 one failing (`Retfd_imm16`, the selector read, fixed before the commit), seeds 9202 and 9203 0
  failing (`l1-seg2-laptop.log` to `l1-seg4-laptop.log`, 111 s each, load about 1).
- Layer 3 runs Csmith and the hand-picked programs for x86 (066d7c8c).
- Layer 1's unoffered forms (60fe1d5c, 6b54f80d): `GuestTest::unoffered_forms` and `reference_has`, the test
  `unoffered_forms_are_illegal_x86_64` with its knob `WC_LAYER1_UNOFFERED_CASES`, and
  `every_unoffered_code_is_drawn_x86_64`, which fails if a code of the list has no case. The list is every
  `Code` valid in 64-bit mode, of every encoding but MVEX, in the AVX setting's claim group or the
  never-claimed one: 2,993 forms. Left out, each with its reason printed: 13 VMX codes (plan 5.2) and 75 codes
  the translator decodes as another code (48 Knights Corner VEX codes, and the prefetches, MPX, CET, `cldemote`
  and the rest of the reserved-NOP space, which are in the default list as `Reservednop_*`). Cases draw every
  operand kind from iced's tables (ymm, zmm, k and mm registers, vvvv, is4 and is5, VSIB with small index
  elements so every element stays near its base, tiles, implicit rsi/rdi operands, a second immediate) and
  EVEX's mask, zeroing, broadcast, rounding and suppressed exceptions where the form takes them. Our side must
  stop with illegal instruction; where the reference has the feature (AES, PCLMULQDQ, RDRAND, RDSEED,
  CLFLUSHOPT, XSAVEOPT, XSAVEC, INVPCID, XSAVES and the AVX setting's list) its stop is counted and printed,
  not compared. The refactor moved `case`'s first draws into `start` without changing their order, so the
  default forms draw the cases they drew before.
- 4ad0ca98: layer 1 drew `call` and `jmp` through a 16:16 or 16:32 far pointer without the segment forms'
  drawing (it tested a pointer size above 8 bytes, so only the 16:64 forms had it): no selectors placed, and
  no leaving out of transfers to 32-bit code. It now tests iced's far-pointer memory sizes.

Measured:
- Every default form on the i7-8750H at 9014abe3 (`l1-all1-laptop.log`): 1,063 forms, 1,000 a form, seed 9204,
  4 at once, 3,146 s, load 0.5 to 5: 1 failing, `Call_m1616`, a far call to selector 0x20 (Linux's 32-bit
  code), where we give general protection by design and the reference ran it; the case should have been
  left out, and was not because of the drawing fault fixed in 4ad0ca98. In the same job the hand-made cases
  (95) and 64 no-form cases at seed 1 passed, and of `reference_x86_64` 5 tests passed and
  `the_x86_64_reference_runs_programs` failed because the laptop has no zig (it builds its programs), as
  before; its verdict comes from the x86 machine.
- Layer 3 Csmith for x86 on the rented x86-64 machine at 9014abe3 (`l3-csmith-1000.log`): 1,000 programs, seed
  5502, 96 at once, 864 s, load 7 to 10: 879 compared with Linux (10 also in Chrome), 121 did not end on Linux
  in time, 0 failing. Also 300 at seed 9311 with other lists (below): 256 compared, 0 failing.
- The not-a-number probe (the module x86's translator runs at start, the same bytes everywhere, SHA-256
  dba04df9...): wasmtime in the runtime (embedded, release) answers 1 on the x86-64 machine and 0 on the ARM64
  machine; the wasmtime 49.0.2 command answers 1 on x86-64 and 0 on the Mac (Apple M5 Max); Chrome 158 answers
  1 on Linux x86-64 and 0 on Linux ARM64, Chrome 157 0 on macOS ARM64. So x86 hosts take the plain path and ARM
  hosts the fix-ups, whose layer 1 run 4 checked with the answer forced to 0.
- The passed-register lists (`passed.sh`, `passed.log`, rented x86-64 machine, load 3 to 10, `taskset -c 0-9`,
  `perf stat` user-mode instructions, wasmtime, full look-ahead, 3 rounds, order turned each round): five pairs
  of lists, the benchmark set's sum of best times: the plan's starting lists (in rsp, rdi, rsi; out rsp, rax)
  20.9 s, the current (in rsp, rdi, rsi, rdx, rcx, r8, r9, rax; out rsp, rax, rdx) 21.6 s, in four 21.4 s, out
  one 22.2 s, every callee-saved register too 23.2 s. The starting lists ran fewest instructions on six of eight
  workloads (fib 103.0 G against 113.7 G). But with them layer 2 differs at bounded look-ahead: seed 9310, 3,000
  programs, 367 failing (`passed-check.log`); 200 programs, 25 failing, against 0 with the current lists in the
  same job. The starting lists are the first with a register passed out and not in (rax); replay
  `WC_LAYER2_FROM=10 WC_LAYER2_CASES=1 WC_LAYER2_SEED=9310`, rax 1 at bounded 56 against 0. So the current
  lists stay, chosen for now, and the plan (3.4) says why; switching is X6's, with that bug fixed.
- Layer 1's unoffered forms on the rented x86-64 machine as a trial (seed 9320, 10 a form, 96 at once,
  `unoffered-x86m.log`): our side stopped with illegal instruction in every case; 1,827 forms differ, every
  one a feature that machine has and the i7-8750H lacks (AVX-512 and the other EVEX forms, SHA, GFNI, VAES,
  AVX-VNNI, AMX, TSX, WAITPKG, PKU, MOVDIRI, ENQCMD, SERIALIZE and the rest of its manual's list), plus
  `invpcid` and `xsaves`, kernel only there (general protection), now in `REFERENCE_HAS` (the laptop's
  cpuinfo has both). The list's verdict is the laptop's, below.

On the i7-8750H at 4ad0ca98 (`batch5-laptop.log`, load 0.3 to 4, 4 at once):
- The 38 segment and far-transfer forms, 1,000 a form, seed 9330: 0 failing (109 s).
- The unoffered forms, 2,993 forms, 100 a form, seed 9331 (833 s): 1 failing, `Wbnoinvd`. Our side gave
  illegal instruction in every case of every form. The reference refused every form it lacks, as it must;
  of the forms it has it ran 71,400 cases (landed 53,639, memory fault 10,026, general protection 6,179,
  float exception 1,255, stack fault 301) and refused none. `wbnoinvd` is `wbinvd` with an F3 prefix, which a
  CPU without it ignores, so the CPU presented must give general protection: fixed in 50fd0936 (the decoder
  takes it as `wbinvd`), with a hand-made case compared with the reference; then the hand-made cases (96) and
  `wbinvd` and `invd` at 1,000, seed 9334, 0 failing (`wbno-laptop.log`). It now counts among the codes
  decoded as another code (76).
- 10,000 cases in no form, seed 9332: 0 failing (30 s).
- The Chrome sample, 3 a form, 3,189 cases, seed 9333, Chrome 158 there: 0 failing (31 s).
- `the_x86_64_reference_refuses_what_linux_refuses` passed on the laptop in the job above (`l1-all1-laptop.log`).
Every default form once more at 17730368, after the last change to the translator (`final5-laptop.log`):
1,063 forms, 1,000 a form, seed 9340, 4 at once, 0 failing, 4,869 s; the laptop's load was 4 at the start
and up to 61 during, from another project's benchmark running there. The unoffered forms again in the same
job (seed 9341, 100 a form, 913 s): 2,992 forms (`wbnoinvd` now decodes as `wbinvd`), 0 failing; of the
forms the reference has it ran 71,400 cases and refused none.

So X5's pass bar holds by these runs: layer 1 for the default forms at 1,000 a form with 0 failing (seed 9340),
the unoffered forms at 100 a form with 0 failing (seed 9341), layer 3 Csmith at 1,000 programs with 0 failing
(seed 5502), and the reciprocal formula against all 2^32 inputs (run 3). Not done: the passed-register lists
are measured but the faster ones wait on a translator fix (X6), and the ARM64 layer 1 tests were not run on
the Mac after this run's change to the shared layer 1 runner.

The suite at 4ad0ca98 on the rented x86-64 machine (load 8 to 55, shared, 2,714 s, `suite-run5.log`; run 4's
command with `wcontainers-arm64` linted too): clippy with warnings denied passes; 31 tests fail, all
expected there but two: the 25 ARM64 tests that need an ARM64 reference, the four tests its 5-level paging
changes, `unoffered_forms_are_illegal_x86_64` (that machine has the features), and
`csmith_programs_match_linux_x86_64` and `hand_picked_programs_match_linux_x86_64`, which failed because the
ARM64 layer 3 test in the same process could not build its arm64 gcc image there and poisoned the image
lock both share. Fixed in 3d831324; then the whole layer 3 test file there (seed 9350, 96 programs): both x86
tests pass (80 compared, 0 failing), the ARM64 ones fail there as before. On the rented ARM64 machine
(`suite-arm.log`, load about 20, 1,169 s, the ARM64 tests of `wcontainers-arm64` and the test crate): 50
passed, 2 failed, the two layer 1 tests its manual keeps on the Mac; not run on the Mac.

Literals: the check exits 0 (the Mac). The test crate's baseline lost 12 lines (the starting state's numbers,
now named, and the scales); no line was added. Baseline lines: x86 125, interface 28, translator 72, test crate
2,094.

What comes next, in this order:
- Fix the translator's loss of a register passed out and not in at bounded look-ahead (replay above), then
  take the plan's passed-register lists (3% faster on the benchmark set): X6, with bounded look-ahead.
- X5's independent check: 100,000 random instructions of its own seed, every host-dependent instruction
  marked, and the ARM64 layer 1 tests on the Mac (`every_form_matches_the_cpu_arm64` and
  `sample_matches_the_cpu_in_chrome_arm64`, which fail on the rented ARM64 machine as its manual says; the
  shared runner of layer 1 was refactored this run, without a change to what it does, and they were not run
  on the Mac).

## X5, run 4 (2026-11-20)

Machines: the rented x86-64 machine (load 0.3 to 6) for builds, lint, the suite and quick layer 1 runs (its
5-level paging makes every case with an address between 2^47 and 2^56 differ, so its layer 1 runs only show
that nothing else differs); the i7-8750H (load 0.1 to 1.7 before, 4 test processes at once) for layer 1's
verdicts, built and run there (`x5/run4/laptop-bg.sh`); the Mac only for git, `cargo fmt` and the literal
check. Logs in scratch `x5/run4/`.

Written (commit 63fa3b23), each against the i7-8750H:
- `pcmpestri`, `pcmpestrm`, `pcmpistri`, `pcmpistrm` and the REX.W forms (`crates/wcontainers-x86/src/text.rs`):
  specialised on the immediate at translate time; each aggregation is a WebAssembly vector compare per
  element of the first operand (equal any, ranges, equal each, equal ordered with shifted copies of the
  second), the polarity on the lane mask, then the flags (CF, ZF and SF from the lengths, OF the first
  result), ecx or xmm0. Lengths are |eax| or |rax| capped, or up to the first zero element. Layer 1 draws, for
  these forms, short signed lengths in rax and rdx and strings of a few element values (zero among them), or
  of the memory operand's own 16 bytes. Deterministic: not marked as varying.
- `enter` (both operand sizes): push rbp, copy level - 1 frame pointers from below rbp, push the frame, then a
  write check at the final rsp, then rbp and rsp. The reference showed that order (a fault at the final rsp
  comes after the pushes, which stay written; a fault on the push of rbp comes before anything); every access
  of `enter` through the stack segment is a stack fault when not canonical.
- `maskmovdqu`: the upper 8 bytes checked first, then the lower, whatever the mask (plan section 2, run 8);
  each half's address wraps by itself under an address-size prefix (seen: with `67` the upper half's address
  is (edi + 8) mod 2^32), so the halves are two accesses and two stores of the bytes the mask keeps.
- `lar`, `lsl`, `verr`, `verw` (`descriptors.rs`): Linux's user segments as the reference shows them
  (scratch `x5/run4/descriptors.c` and `descriptors.out`, every selector below 0x100 on four processors):
  0x23 (rights 0x00cffb00), 0x2b (0x00cff300, the only writable one), 0x33 (0x00affb00), 0x7b (0x0040f500),
  limits 0xffffffff, any privilege level, nothing else; only ZF changes; the destination only on success.
  `sldt` 0, `str` 0x40, `smsw` CR0 0x80050033 (truncated by the destination). The 0x7b limit is the
  processor's number there (0 to 3 on four processors), so ours is 0 by design and layer 1 reconciles a
  successful `lsl` whose limit is not 0xffffffff to 0. `sgdt` and `sidt` store a zero limit and Linux's UMIP
  dummy bases (the GDT base differed on each processor, and the IDT limit was 0xfff on three and 0xffff on
  one); layer 1 leaves them out with that reason and three designed cases check `lsl` of 0x7b, `sgdt` and
  `sidt`. `lsl`, `sldt`, `str`, `smsw`, `sgdt` and `sidt` are marked as varying between CPUs (`may_vary`); the
  rented machine (UMIP) emulates `sldt`, `str` and `smsw`.

Layer 1 on the i7-8750H: the six string compares, seed 9101, 1,000 a form, 0 failing (`text1`); `enter` and
`maskmovdqu`, seed 9102, 1,000 a form, 0 failing after the fixes above (`enter1`, `mask2`); the 30 forms of this
run together, seed 9104, 1,000 a form, 0 failing, 90.5 s at 4 at once (`desc2`).

What comes next, in this order:
- The segment register and far-transfer group (37 forms: `mov` to and from segment registers, `push`/`pop`
  `fs`/`gs`, `lfs`/`lgs`/`lss`, far `call`/`jmp`, `retf`, `iretq` and their 16- and 32-bit forms), with
  `iretq`'s partly mapped frames. What the reference shows (`descriptors.out`): ds, es, fs and gs are 0, ss
  0x2b, cs 0x33; the loadable selectors are exactly those `verr` accepts (0x20 to 0x33 and 0x78 to 0x7b, any
  privilege level) and null; ss takes 0x2b alone. The design I would build: the block's `reserved1` becomes
  `selectors: [u16; 4]` (ds, es, fs, gs; same size, so no offset moves), a load writes the selector and, for
  fs and gs, a base of 0 (Intel clears the base for a null selector, and every user segment's base is 0);
  layer 1's state gains the four selectors, and the reference runner loads them in its handler before
  `rt_sigreturn` and reads them first thing in its handler after the case (Linux neither saves nor restores
  them on a signal); since `arch_prctl(ARCH_SET_FS)` clears the fs selector, a case with a non-null fs or gs
  selector has that base 0 and the runner sets only the selector. Seen on the reference already (scratch
  `x5/run4/selectors.c` and `selectors.out`): `push fs` writes all 8 bytes, zero-extended, and `pushw fs`
  moves rsp by 2; `mov r64, ss` and `mov r32, ss` zero-extend; `mov gs` with 0x2b or with a null selector (3)
  gives a base of 0 and reads back as loaded; `wrgsbase` afterwards keeps the selector; `arch_prctl(ARCH_SET_GS)`
  makes it 0; `mov ds` with 0x78 loads.
- The never-claimed list in layer 1 at 100 a form (needs case drawing for VEX, EVEX, XOP and 3DNow operand
  kinds: ymm, zmm, k and mm registers, the vvvv and is4 operands, VSIB) with `REFERENCE_HAS`.
- What the probe answers in wasmtime and Chrome on x86 and ARM hosts (the fix-up path itself now passes
  layer 1 with the answer forced to 0).
- Layer 3 Csmith for x86 at 1,000 programs; the passed-register lists; the Chrome sample, the no-form and
  refusal tests against the laptop; 100,000 random instructions of a new seed; then the bar as a whole.

The suite at 63fa3b23 (rented x86-64 machine, load 2 to 12, 2,123 s, `suite-run4.log` and `suite-run4b.log`):
`cargo test --release --no-fail-fast -p wcontainers-guest-cpu -p wcontainers-x86 -p wcontainers-arm64 -p
wcontainers-translator -p wcontainers-translator-test`. Every x86 test passes but the four the machine's
5-level paging changes (`every_form_matches_the_cpu_x86_64`, `bytes_in_no_form_stop_as_on_the_cpu_x86_64`,
`sample_matches_the_cpu_in_chrome_x86_64`, `the_x86_64_reference_refuses_what_linux_refuses`); the 25 ARM64
tests that need an ARM64 reference fail there, as before. On the rented ARM64 machine (load about 30, with the
probe setting below) every ARM64 test passes but the two layer 1 tests its manual keeps on the Mac
(`suite-arm.log`), which were not run on the Mac; nothing ARM64 changed. The laptop's verdict on all default
forms is below.

Every default form on the i7-8750H (`l1-all-laptop.log`, at 63fa3b23): 1,025 forms, 1,000 a form, seed 9105, 4
at once, 2,954 s (load about 4 during): 2 failing, `dpps` and `dppd`, both with an unmasked exception, where
MXCSR's flags differed (one case missing precision, one missing overflow). The exact path filtered operand
exceptions over the whole instruction; the CPU runs a dot product as micro-operations (the multiplications,
then each round of additions), keeps the flags of each, and stops after the first that raises an unmasked
exception (a per-step filter alone then raised flags the CPU did not, at seeds 9106 and 9107). Fixed in
c470ea8d: with it, both forms at 3,000 a form at seeds 9105, 9106, 9107 and 9108: 0 failing (`dp2`). The
other 1,023 forms passed at seed 9105; the whole list was not run again after the fix (only the two forms
changed).

The fix-up path for hosts whose probe answers 0, forced on the i7-8750H with `WC_PROBE_ANSWER=0`: the 76 float
forms, 1,000 a form, seed 9109: 0 failing (`nan0`). What the probe answers in wasmtime and Chrome on x86 and
ARM hosts is still not measured.

Also: `WC_PROBE_ANSWER` (the test runner's `PROBE_ANSWER_SETTING`) makes the harness translate for a host whose
probe gives that answer, for the fix-up path of hosts answering 0 (see "How to run").

Literals: the check exits 0 (the Mac). New code adds no baseline line; `XMM_BYTES` is now one constant of the
x86 crate (it was the SIMD module's and a test's copy), and `tests/reciprocals.rs` names the export through the
translator's `MEMORY`. Baseline lines: x86 125, interface 28, translator 72, test crate 2,106.

## X5, run 3 (2026-11-19)

Machines: the rented x86-64 machine (load 1 to 3) for builds, lint and quick layer 1 runs; the i7-8750H
(load 0 before, 4 to 6 during) for layer 1's verdict, built and run there (`x5/run3/laptop-bg.sh`); the Mac
(load not measured, `mac-gate.sh` said "USE x86-run") only for git, `cargo fmt` and the literal check. Logs
in scratch `x5/run3/`.

Written (commit 2c315d46):
- The float instructions of SSE to SSE4.1 on xmm registers (80 forms with `ldmxcsr`, `stmxcsr` and the
  four prefetches; list in scratch `x5/run3/float-forms.txt`): add, sub, mul, div, sqrt, min, max of every
  size and kind, `cmpps`/`cmpss`/`cmppd`/`cmpsd`, `comis`/`ucomis`, `addsub`, `hadd`, `hsub`, `round`,
  `dpps`, `dppd`, and every conversion between the float sizes, from and to 32- and 64-bit integers.
  `crates/wcontainers-x86/src/float.rs` is the meanings, `exact.rs` the exact path, `host.rs` the probe.
- MXCSR (plan 5.8): controls `Mxcsr`, `MxcsrTracking` (a private word after the block) and
  `FloatException`. The fast path runs when MXCSR's mode is 0x1F80's and tracking is off: WebAssembly vector
  operations, min and max as `pmin`/`pmax` with the operands swapped, x86's integer indefinite where
  WebAssembly saturates. Otherwise the exact path: one call of the float hook per instruction
  (`exact::run`, rustc_apfloat in quadruple precision rounded to odd, then rounded once), with DAZ, FTZ,
  the rounding control, all six flags in x86's order, tininess after rounding, and an unmasked exception
  leaving the destination as it was and stopping with SIGFPE. `ldmxcsr` writes MXCSR (general protection
  for bits above 15) and turns tracking on; `stmxcsr` counts "MXCSR's exception flags read while not kept"
  when tracking is off.
- Not-a-numbers: the probe (`host.rs`, 13 cases: first operand's NaN over a signaling second, default NaN,
  sqrt, nearest, promote and demote keeping the payload) answers whether the engine already gives x86's;
  when not, the fast path fixes every NaN lane up (first operand's quieted, then the second's, else
  0xFFC00000; conversions keep the payload). `PROBE_ANSWERS` is [0, 1]. Which answer wasmtime and Chrome
  give on x86 and on ARM hosts was not measured; I expect x86 hosts to answer 1 and ARM hosts 0.
- `rcpps`, `rcpss`, `rsqrtps`, `rsqrtss` by plan 5.8's formula on every path, marked as varying between
  CPUs (`may_vary`). `tests/reciprocals.rs` runs the emitted formula in wasmtime against the CPU's own
  instructions over all 2^32 inputs of each (`WC_RECIPROCAL_INPUTS` for fewer): 0 differences on the
  rented Sapphire Rapids machine (4,294,967,296 inputs of each, 192 threads, 0.31 s).
- Layer 1 for x86 now draws MXCSR (40 in 100 the default mode, with or without flags; else any mode, 30 in
  100 of those with exceptions unmasked), tracking (50 in 100), float edges in xmm lanes, and for
  `ldmxcsr` a data seed whose operand holds a value it takes (70 in 100). The reference's SIGFPE is
  compared as MXCSR's raised unmasked flags (see "Departures").

Found by layer 1 against the CPU and fixed on the way: an exact zero's sign is the rounding mode's
(x - x is -0 rounding down); DAZ also applies to the operand min and max return for a NaN; dividing a
denormal by zero raises divide by zero alone; an unmasked overflow or underflow also raises precision when
F's precision cannot hold the result, whatever PM says; `dpps`'s and `dppd`'s order of additions (see
"Departures"); the sign of `rcp`'s result; the conversions' range test was inverted; `ldmxcsr` was checked
as a write.

Layer 1 on the float forms: on the rented machine, 1,000 a form, seeds 4404 and 5505 (`l1-float-4404.log`,
`l1-float-5505.log`): every difference is the 5-level paging one (MemoryFault against GeneralProtection or
StackFault), and none other; those forms stop at that first case there, so the verdict is the laptop's.
On the i7-8750H, built and run there, 4 at once: seed 6606 at 1,000 a form, 80 forms, 0 failing, 222 s
(`l1-float-laptop.log`, at the code before its last plumbing change); seed 7707 at 1,000 a form, 80
forms, 0 failing, 281 s (`l1-float-laptop2.log`, at 2c315d46). Then `tests/reciprocals.rs` there: all
4,294,967,296 inputs of `rcpss` and of `rsqrtss`, 0 differences, 11 s on 10 threads. So the float forms
meet the bar at 1,000 a form on the reference. Not yet seen: the fix-up path for engines whose
not-a-numbers are not x86's runs only where the probe answers 0 (expected on the Mac), and no layer 1
float run was made from the Mac this run.

Literals: the check exits 0 (the Mac); six baseline lines deleted (the x86 crate's old probe literal and
hook message, the translator's own `PROBE`, now the interface's one constant, and three MXCSR values in
`reference_x86_64.rs`). Lint: `cargo clippy -D warnings` on the x86, interface, translator and test
crates, all targets, clean (rented machine); `cargo fmt --check` clean.

The suite at the end (2c315d46, rented x86-64 machine, load 3 to 6, `suite-run3.log`): `cargo test --release
--no-fail-fast -p wcontainers-guest-cpu -p wcontainers-x86 -p wcontainers-arm64 -p wcontainers-translator -p
wcontainers-translator-test`. Every x86 test passes but the four the machine's 5-level paging changes
(`every_form_matches_the_cpu_x86_64`, `bytes_in_no_form_stop_as_on_the_cpu_x86_64`,
`sample_matches_the_cpu_in_chrome_x86_64`, `the_x86_64_reference_refuses_what_linux_refuses`), whose
verdicts come from the laptop; the 25 ARM64 tests that need an ARM64 reference fail there, as before.
Not run this run: those four on the laptop in full (only the float forms of the first), and the ARM64
tests on the rented ARM64 machine; nothing ARM64 changed but the interface's one new constant.

What comes next, in this order: the SSE4.2 string compares; the segment, descriptor and far-transfer
instructions with `iretq`'s reference cases; `enter`; `wait`; `maskmovdqu`; layer 1 of the never-claimed
list at 100 a form; layer 3 Csmith at 1,000; the passed-register lists; the Chrome sample against the
laptop; the whole bar (all default forms at 1,000 a form on the laptop, 100,000 random instructions).

## X5, run 2 (2026-11-19)

Machines: the rented x86-64 machine until it was taken away (a spot notice at about 13:53); then the Mac
(Apple M5 Max, load 5 to 13) for builds and the translated side, with the i7-8750H (load 0.1 to 1.9) as the
reference over SSH (`WC_REFERENCE_X86_64`), at -j 4, as `mac-gate.sh` allowed ("MAC ALLOWED", the x86
machine down). Logs in scratch `x5/run2/`. The laptop has no zig, so the program tests cannot run on it
alone.

What was written, each checked in layer 1 against the i7-8750H (all 0 failing):
- Committed in 1ec4684e (written before this run's restart, layer 1 seed 1, 1,000 a form, 67 forms,
  `l1-a-1000.log`): `bsf`, `bsr`, `tzcnt`, `lzcnt`, `popcnt`, `bswap`, `movbe`, `xadd`, `cmpxchg`,
  `cmpxchg8b`, `cmpxchg16b`, `shld`, `shrd`, `adcx`, `adox`, `loop`, `loope`, `loopne`, `lahf`, `sahf`,
  `clflush`, `rdfsbase`, `rdgsbase`, `wrfsbase`, `wrgsbase`.
- rep string instructions (plan 5.6): `rep movs` and `rep stos` in bulk passes of at most 1 MiB
  (`memory.copy`, `memory.fill` for a repeated byte, else a store loop) when DF and TF are clear, the copy
  does not land inside its source and every byte of both ranges is allowed; otherwise one element a pass,
  counted with its reason. `repe`/`repne` `cmps` and `scas` and `rep lods` one element a pass. Every pass
  ends on an element with exact rcx, rsi and rdi and jumps back to the instruction, so the stop check runs
  between any two elements. Layer 1, the 20 string forms with repeat prefixes drawn (counts of zero, past
  the data, overlapping), 1,000 a form, seed 4 (`l1-string-1000c.log`); `programs/x86_64/strings.c` (every
  element size, a copy longer than a pass, backwards, overlapping, compares with their flags, a count of
  zero) matches Linux at none and full and with a stop at every check.
- `popf` (and `popfw`): the six flags, TF, DF, NT and ID; AC is held clear (plan 5.1) and counted; TF is
  kept but not stepped until X6 (see "Departures"). Layer 1, 2 forms, 1,000 a form, seed 5, with AC masked
  as the plan says (`l1-popf-1000.log`).
- `rdtsc` and `rdtscp`: the clock hook's nanoseconds as a 1 GHz counter, ecx 0 for `rdtscp`. Answered by
  design, so excluded from layer 1.
- `cpuid` (plan 5.10): `crates/wcontainers-x86/src/cpuid.rs` is the table, one `answer(avx, leaf,
  sub_leaf)`; the meaning emits a test per leaf and sub-leaf from it. Vendor GenuineIntel, the reference's
  family 6, model 0x9E, stepping 10, the brand "wcontainers x86-64", the default claims or with the AVX
  setting the v3 level and its XSAVE leaf, caches and topology of our own (see "Departures"), leaf 0x15's
  1 GHz counter, invariant TSC, 48-bit linear addresses, leaves past the highest as the highest basic one.
  "a program asked whether AVX is available and was told no" is counted for leaf 1 and leaf 7 sub-leaf 0
  when the setting is off. Checked by `crates/wcontainers-x86/tests/cpuid.rs`, which reads the table back
  through the raw-cpuid crate, and by 59 designed cases in layer 1 (every leaf with each sub-leaf, the next
  and the last, leaves past the highest) that the translated `cpuid` gives the table's answer with the upper
  halves cleared.
- `xlat`: its address takes al zero-extended, in the meaning's address, in its access description (`Extend`
  from 8 bits) and in `memory_fault`, which ignored an index's extension before and so called a fault at a
  non-canonical `xlat` address a page fault; layer 1 builds `xlat` cases (`Xlat_m8`, 1,000, seed 78, 0
  failing). `crc32` (CRC-32C a bit at a time, no table: 5 forms, 1,000 a form, seed 77, 0 failing).
  `movnti`, `lfence`, `mfence`, `sfence` (one thread at a time: nothing to order).
- SSE2 to SSE4.2 integer, move and blend forms on xmm registers: the saturating adds and subtracts, `pavg`,
  `pmullw`, `pmulhw`, `pmulhuw`, `pmaddwd`, `psadbw`, the packs, the shifts by a register, the min and max
  of every width, `pmulld`, `pmuldq`, `pcmpeqq`, `pcmpgtq`, `pshufb`, `pabs`, `psign`, `palignr`, the
  horizontal adds and subtracts, `pmaddubsw`, `pmulhrsw`, `pmovsx`, `pmovzx`, `ptest`, `pblendw`,
  `pblendvb`, `blendps`, `blendpd`, `blendvps`, `blendvpd`, `pextrb/d/q`, `extractps`, `pinsrb/d/q`,
  `insertps`, `phminposuw`, `mpsadbw`, `lddqu`, `movntdqa`, `movntdq`, `movntps`, `movntpd`, `movddup`,
  `movshdup`, `movsldup`, `movhlps`, `movlhps`, `movhps`, `movhpd`, `movlps`, `movlpd`, `movss`, `movsd`.
  135 forms, layer 1 at 100 a form, seed 31: 0 failing (`l1-sse-100b.log`, 102 s); at 1,000 a form, seed
  4242: 0 failing (`l1-sse-1000.log`, 618 s, the Mac at load 7 to 13, the laptop at 0.1 to 1). The first run found the memory sizes of float and packed operands under 16 bytes missing
  (`Float32`, `Packed64_UInt16` and the rest): fixed in `size_of_memory`, checked against iced's sizes.

The gap list (scratch `x5/run2/gaps4.txt`, the default legacy forms iced decodes for Intel in 64-bit mode,
1,295): 864 written. Left: x87 160 and MMX 60 (X7's); the float forms of SSE to SSE4.1 (arithmetic, compares,
conversions, `sqrt`, `min`, `max`, `round`, `dpps`, the SSE3 horizontal ones); `pcmpestri`, `pcmpestrm`,
`pcmpistri`, `pcmpistrm`; `ldmxcsr`, `stmxcsr`; `maskmovdqu` (which bytes the CPU checks when the mask
is partly zero is to be seen on the reference first); `fxsave`, `fxrstor` (with x87, X7); `wait`;
`enter`; the segment and descriptor instructions (`mov` to and from segment registers, `push`/`pop fs/gs`,
`lar`, `lsl`, `verr`, `verw`, `sldt`, `smsw`, `str`, `sgdt`, `sidt`, `lss`/`lfs`/`lgs`) and the far
transfers (`call far`, `jmp far`, `retf`, `iret`), which need the selectors as thread state.

Also found and fixed: SSE4.1's `pextrw` to memory panicked in its meaning (the seed 2611 run found it);
`size_of_memory` knew no float or 2- to 8-byte packed operand (`Float32`, `Packed64_UInt16`, ...).

Literals: the x86 crate's baseline lines 142 to 127 (the new SSE code's sizes come from `size_of`, the shift
kinds from the operand widths, and the old `16`s of `simd.rs` became `XMM_BYTES`); the test crate's 2,111 to
2,109; translator 73 and interface 28 unchanged. `cargo run -q -p wcontainers-literals -- check` exits 0 (on
the Mac).

The suite at the end (50bad34a with this file): on the rented x86-64 machine (a new one, load 1 to 8),
`cargo test --release --no-fail-fast -p wcontainers-guest-cpu -p wcontainers-x86 -p wcontainers-arm64 -p
wcontainers-translator -p wcontainers-translator-test`, 2,045 s (`suite-x86b.log`): every x86 test passes
(the program tests, deep recursion and stops at every check among them) but the three the machine's
5-level paging changes (`every_form_matches_the_cpu_x86_64`, `sample_matches_the_cpu_in_chrome_x86_64`,
`the_x86_64_reference_refuses_what_linux_refuses`); the 32 ARM64 tests that need an ARM64 reference fail
there, as before. Their verdicts: the first and the refusal test pass against the i7-8750H (seed 2611 run
above, and the refusal test on the Mac); the Chrome sample was not run against the laptop this run. On the
rented ARM64 machine (load about 24) the ARM64 tests pass but the two layer 1 tests its manual keeps on the
Mac (`suite-arm.log`), which this run did not run on the Mac; nothing this run changed is ARM64's.
Lint: `cargo clippy -D warnings` on the x86, interface, translator and test crates, all targets, and `cargo
fmt --check`, clean (the Mac).

What comes next, in this order: the float forms with MXCSR's two paths and tracking, the NaN rules and the
probe (plan 5.8); `ldmxcsr`/`stmxcsr`; the reciprocal formula and its 2^32-input test; the SSE4.2 string
compares; the segment, descriptor and far-transfer instructions with the reference cases plan 5.3 asks for
(`iretq`'s partly mapped frames); `enter`; the host-dependence mark reviewed for every new meaning; the
never-claimed list in layer 1 at 100 a form; layer 3 Csmith at 1,000; the passed-register lists; then the
whole bar.

## X5, run 1 (2026-11-19)

Machines: the Mac (load 29 to 44), the rented x86-64 machine (load 1 to 30), the rented ARM64 machine (load
about 36, shared), the i7-8750H (load 0.2 to 0.6). Logs in scratch `x5/run1/`.

The independent check's findings:
- Chrome cases at once overwrote each other's files (minor): fixed in 14c8b424. Each run of the page (a
  blob, a module load, a batch of layer 1 cases) gets its own directory, `target/browser/cases/<guest>/<name>/
  <process>/<call>`, removed when the run ends. Checked at the check's seed: layer 2 x86, seed 8675309, 300
  programs, all on the CPU, 100 in Chrome, 24 at once, on the rented x86-64 machine (`l2-chrome-parallel.log`,
  35 s): 0 failing. So `WC_LAYER2_CHROME` can now be raised with `WC_LAYER2_JOBS` above 1.
- The matrix slowdown of the flags change (minor): explained, not removed. On the i7-8750H (load 0.2 to 0.6),
  `perf stat` of matrix (bench-6) in wasmtime, old (babfb595) and new (49a11539) builds alternating, three
  rounds each (`perf` 5.15's events, user mode): instructions 14.55 to 14.68 G old, 14.84 to 14.97 G new;
  uops from the decoded-uop cache (`idq.dsb_uops`) 13.51 to 13.68 G old against 6.40 to 6.42 G new; uops from
  the legacy decoders (`idq.mite_uops`) 1.45 to 1.48 G old against 8.67 to 8.73 G new. The matrix module's
  bytes are the same in both, so its machine code from wasmtime is the same too, placed elsewhere: in the new
  build the hot loop is fed by the legacy decoders, which is what the i7-8750H (a Coffee Lake, with the
  microcode for the jump-crossing-a-32-byte-boundary erratum) does when a loop's jumps fall badly against 32
  and 64 byte lines. I expect that is the cause (the counters show the decoder switch; which jump and which
  boundary was not looked at). It depends on where wasmtime puts the code, not on what the translator
  emits; the same placement effect can go either way for any workload (search was 17% faster in the same
  pairs). Not fixed: wasmtime offers no loop alignment, and padding our modules to move one loop would be
  shaped around one program. The rented Sapphire Rapids machine, which has no such erratum, was not used to
  confirm it.
- No x86 test checked that the loop proofs take effect (minor): fixed in 75b1d921. x86's checks now hold
  three counted loops, each once within three pages of data (768 turns, `loop not proven` must count 0) and
  once running one page past the blob (2,048 turns, must count exactly 1, status 139 as natively): a count
  down by `dec`/`jnz`, an index up by `inc`/`cmp`/`jb`, and clang -O2's initialising loop (a store through a
  moving base, counted by a value stepped by 3 and compared for inequality with its last value). The last
  was never proven: the proof of an inequality took only steps of 1 and -1. Fixed in adf45969 (the
  translator, so ARM64 too): any step of size up to the proofs' largest is taken when the distance to the
  bound is an exact multiple of it, which makes that meeting the first; otherwise the loop is not proven, as
  before. Steps of 1 and -1 give the same turns as before. Without the fix the stride check fails ("counted
  1 times, must be Exactly(0)"), seen. After it, on the rented x86-64 machine: x86's checks pass, layer 2 x86
  at seed 1919, 2,000 programs, all on the CPU, 24 in Chrome, passes (`x86-proof.log`), and x86's existing
  program tests pass; on the rented ARM64 machine the ARM64 checks, changes, layer 2, look-aheads and
  program tests pass (`arm-proof.log`, 15 program tests among them).
- ARM64's X4 program tests had no x86 versions (minor): written, with x86 sources of the assembly programs
  (`programs/x86_64/deep_small_frames.S`, `big_frame.S`, `callee_moves_sp.S`, `callee_moves_sp_away.S`,
  `callee_misaligns_sp.S`; x86 has no alignment fault, so the last one exits 77 where ARM64's faults).
  `cut_functions_and_split_translations_match_linux` takes its list of programs to split; x86's leaves out
  `differences`, which is ARM64's own program. On the rented x86-64 machine (`x86-programs*.log`): deep
  recursion (at full: left a nearly full stack 530 times, continued 149,990 times for C; 2,680 and 999,640
  for the million small frames), the moved stack, the frame past the budget and the cut and split programs
  pass. Two FAIL and are left failing, recorded here: `programs_match_linux_x86_64` and
  `stops_at_every_check_change_nothing_x86_64` reach `integers`, which uses `bswap` and `bsr`, whose
  meanings are X5's and not yet written (status 132, illegal instruction, at none); their bench part passes
  (77,656 to 116,974 stops). Not written for x86: `unwinding_matches_linux` and
  `stacks_switched_by_the_program_match_linux` (`fibers.c` needs an x86 `switch.h`), `written_code`,
  `differences` and the float programs; they come with the X5 to X7 instructions they need.
- The block size's bare 4 and 16 (minor): fixed in 6a05efe3; the size comes from `size_of::<i32>()` and the
  interface's new `BLOCK_ALIGNMENT` (the bytes of a 128-bit vector).

X5's work left, all of it (the default-claim legacy forms without a meaning, from a scratch test over iced's
`Code`s against `GuestTest::forms`, `gaps.txt`, 2026-11-19): 1,343 default forms, 660 written, 683 not. By
iced feature: x87 (FPU, FPU287, FPU387, with CMOV and SSE3) 160 and MMX 60, both X7's; SSE 71, SSE2 94, SSE3
10, SSSE3 32, SSE4.1 51, SSE4.2 12; INTEL8086 35 (16-bit branches and returns, `lahf`, `sahf`, `popfw`,
`xlat`, `wait`, segment moves, far returns), INTEL186 1, INTEL286 8, INTEL386 56, INTEL486 8, X64 44 (`bsf`,
`bsr`, `bswap`, `cmpxchg`, `xadd`, `shld`, `shrd`, `enter`, `loop` and its kin, `popfq`, `lar`, `lsl`,
`verr`, `verw`, `sgdt`, `sidt`, `sldt`, `smsw`, `str`, far transfers, segment pushes and pops), CPUID 1,
TSC 1, RDTSCP 1, CX8 1, CMPXCHG16B 1, POPCNT 3, LZCNT 3, BMI1 3, ADX 4, MOVBE 6, FSGSBASE 8, FXSR 4, CLFSH 1,
SEP 1, MULTIBYTENOP 3; and the rep prefixes of the string instructions. Then MXCSR's two paths, NaN rules,
CPUID's answers, the reciprocal formula with its 2^32-input test, the host-dependence mark for each meaning,
the never-claimed list in layer 1, layer 3 Csmith for x86, and the passed-register lists.

What comes next: X5 from the integer forms, `bswap`, `bsr` and `bsf` first (they make the two failing tests
pass), then the rest of the list above in that order, each group with its layer 1 forms against the
i7-8750H.

Proof at the end of run 1 (rented x86-64 machine for builds and x86 tests, the ARM64 machine for ARM64
tests): `cargo clippy` with `-D warnings` on the x86, interface, translator and test crates (all targets):
clean; `cargo fmt --check`: clean; `cargo run -q -p wcontainers-literals -- check` on the Mac: exit 0.
Baseline lines: wcontainers-translator-test 2111 (3 fewer), wcontainers-translator 73 (1 fewer),
wcontainers-x86 142, wcontainers-guest-cpu 28. The whole suite was not run: the milestone is not complete.

## For the orchestrator

The brief's question of 2026-11-19 08:20 (which rule defines a guest's behaviour above bit 47, and whether
the rented x86-64 machine should be booted with 5-level paging off). My answer:
- A guest's behaviour is defined by the machine the container sees, never by the host's. The owner ruled the
  address width a property of the container, chosen at start and the same wherever it runs, like the page
  size. An x86-64 container today is a 4-level paging machine: canonical addresses are those whose bits 63 to
  47 are all equal, and an access, push or jump beyond that gives what the i7-8750H gives (general
  protection, `SIGSEGV` `SI_KERNEL` with address 0, or `SIGBUS` through the stack). That is what the
  translator does now, from one constant (`wcontainers_x86::CANONICAL_SHIFT`). The rented machine's answer
  (a page fault at the address) is the answer of a different machine, one with a 57-bit width.
- Linux itself keeps user space to 47 bits on a 5-level machine unless a program asks `mmap` for an address
  above it, so a program sees the same memory on both; only the fault for an address no 47-bit program can
  map differs. Nothing in the brief or plan asks for a 57-bit x86-64 container. If one is ever offered, the
  canonical width becomes part of that container's machine (and of the cache key), and a 5-level CPU is its
  reference.
- So yes: boot the rented machine with `no5lvl`, so it gives the 48-bit answer. The 851,000 layer 1 cases the
  orchestrator compared there showed no other difference, so I expect the four tests to pass there then; that
  is expected, not measured, so run them there once after the reboot before the laptop's verdict is dropped.
  The features it has beyond the laptop (AVX-512, AMX, tpause and the rest in its manual) are not claimed by
  our cpuid either way. Until then the four tests the manual names keep taking their verdict from the laptop,
  as runs 1 to 8 did.
- Added in X5 run 5: layer 1's unoffered forms take their verdict from the i7-8750H whatever its paging, since
  the rented machine has AVX-512, SHA, GFNI, AMX, TSX and more that the i7-8750H lacks, and those forms must
  give illegal instruction on the reference. The i9-9900K desktop (no AVX-512 either) could serve too.

## X4, run 8 (2026-11-19)

Machines: the Mac (load 33 to 51), the rented x86-64 machine (load about 12), the rented ARM64 machine (load
20 to 37, shared), the i7-8750H (load 0.4 to 4.7). Logs in scratch `x4/run8/`.

The independent check's findings:
- The workspace did not build (blocking): fixed in ec1f39b2. `wcontainers-wasmtime` matched
  `HookBody::CodeChanged` as a unit variant; its hook now takes the variant's `line` and drops the pieces of
  that many bytes, where it had a bare 63 and 64 (two baseline lines fewer). Runs 2 to 7 had left it, and
  this file said it was not this track's to fix, against the brief of 08:20; that departure is corrected
  where it stands.
- No "For the orchestrator" answer on 5-level paging (minor): written above.
- The checkout clean at 55bff253, not three uncommitted files (information): so it was; the task text was
  out of date.
- Chrome's interpret at 105 to 143 times native (information): as recorded; the absolute switch table is
  plan 3.4's X9 item.
- The reference runner's xfeatures bug the task names was fixed in X3 run 3 (above, under that run).

Proof (at ec1f39b2's code):
- `cargo check --workspace --all-targets` and `cargo clippy --workspace --all-targets -- -D warnings` on the
  rented x86-64 machine (`workspace.log`): both clean, the whole workspace.
- The runtime's tests (`wcontainers-wasmtime`, `wcontainers-runtime`, `wcontainers-kernel-test`) on the rented
  ARM64 machine (`runtime-arm-all.log`, 2,455 s): 305 passed, 26 failed, 8 ignored. Every failure is one of
  the 25 kernel tests its manual lists as giving their verdict on the Mac only, or `fdinfo`, which its manual
  says Nix's zig cannot build there. Every test of `wcontainers-wasmtime` passed, `code.rs` and `views.rs`
  (which reach the changed hook) among them. On the x86-64 machine the same crate's `surface.rs` fails 5 of 8,
  because Docker there cannot run the ARM64 reference images ("exec format error"): not a verdict.
- The track's suite (run 3's command) on the rented x86-64 machine from a clean build of the track's crates
  (`suite-x86.log`, 201 s): 417 passed, 30 failed, 13 ignored. The 30 are exactly the ones its manual
  names: 25 ARM64 tests (no ARM64 reference there), the baseline test (no .git) and the four 5-level paging
  ones. Those four, and the rest of x86's layer 1 and reference tests, against the i7-8750H from the Mac
  (`laptop-four.log`, 219 s): 10 passed, 0 failed. The literal crate's tests on the Mac, the baseline test
  among them: 19 passed. The ARM64 tests were not run again: nothing they build on changed since run 7's
  suite on the Mac.
- `cargo run -q -p wcontainers-literals -- check`: exit 0; `cargo fmt --check`: clean. Baseline lines of
  this track's crates unchanged from run 7 (wcontainers-x86 142, wcontainers-translator-test 2114,
  wcontainers-guest-cpu 28, wcontainers-translator 74); wcontainers-wasmtime 2 fewer.

What comes next: nothing of X4's bar is open.

## X4, run 7 (2026-11-19)

Machines: the Mac (load 16 to 66), the i7-8750H (load 0.6 to 12.3), the rented ARM64 machine. Logs in scratch
`x4/run7/`.

The independent check's findings:
- Flags nobody reads (blocking): fixed. `Emitter::flags_live` (look-ahead none: always true; full: the
  existing `flags_needed_after` of the instruction) says whether anything may read the flags an instruction
  sets before every one is set again. When it is false, `FullEmitter::set_flags` emits nothing (no copies into
  the shape's locals, no flag state), on both guests, and x86's meanings leave out what they work out only for
  the flags: a shift's or rotate's CF and OF, a bit test's CF, the kept flags of `Each` (rotates, bt and its
  kin, clc, stc, cmc), `inc` and `dec`'s kept CF and their constant, and the high half and overflow of a
  two- or three-operand `imul`. The check's loop (`mov rax,rdi; shr rax,29; add rdi,rax; dec ecx; jnz`) now
  holds for the `shr` only its shift (scratch `shr-dead.wat`). Plan 5.1's "As built" says so.
- `-mgeneral-regs-only` (minor): fixed. zig ignores it for its vectoriser, so x86's "general registers only"
  builds held SSE: bench -O2 254 instructions naming an xmm register, buffers 412, walks 238, tables 67.
  `GENERAL_ONLY` for x86 is now `-mcpu=x86_64-sse-sse2-mmx` (as ARM64's leaves out its vector unit through
  `-mcpu`): 0 in every program at -O2 and -Os, the check's own `check_flags.c` among them (scratch `gen/`).
  So x86's benchmark set is different programs from run 7 on: every x86 benchmark number recorded before
  this run is of the build with SSE.
- Bare page sizes (minor): `full.rs` (the frame check's step and the checks' grouping), `code.rs` (the
  permission checks: offset in the page, the page size, the shift to a page number) and `programs.rs`
  (placement) take them from `wcontainers_abi::PAGE_SIZE`; 14 baseline lines fewer. Values unchanged.
- Cases run (minor): "Cases actually run" now lists runs 4 and 5; run 4's sentence that nothing was worked
  out for flags nobody reads is corrected where it stands.

Measured:
- Output sizes, `measure_x86_64` at babfb595 (before the flags change, same programs) and 49a11539 (after),
  release, on the Mac (`measure-old.log`, `measure-new.log`): look-ahead none is the same byte for byte; at
  full, WebAssembly (gzipped) bench -O2 508,352 to 488,786 (92,224 to 84,551), integers -O2 187,822 to
  177,351, tables -O2 86,805 to 82,319, integers -O0 184,854 to 184,250, tables -O0 133,178 to 132,754, hello
  -O2 119,343 to 118,196.
- User-mode instructions of one wasmtime run of each workload on the i7-8750H (`counts.sh`, both builds,
  twice in opposite order, the two counts of each within 0.02%; `counts-run7.log`, load 2.2 to 3.5): fib,
  sieve, interpret and walk unchanged (within 0.01%); hash 10.658 G to 10.655 G (-0.03%); sort -0.006%;
  search 18.459 G to 18.007 G (-2.4%); matrix 14.610 G to 14.861 G (+1.7%). The matrix module is the same
  bytes at both builds, on the Mac and on the i7-8750H (md5 98a29304, `mx-*.wasm`), so that +1.7% does not
  come from translated code: not explained. Hash's +1.9% of run 4 is not explained by dead flags either:
  removing them moves hash by 0.03% (I expect wasmtime's compiler already dropped them; not checked).
- Tests of the change: the x86 crate's tests; layer 2 x86 at seed 9393, 1,000 programs, every one also on
  the CPU (`l2-seed9393.log`): 0 failing (361 faulted); every x86 test of the test crate against the i7-8750H
  (`x86-tests.log`, layer 1 at its defaults among them): pass.
- X4's layer 2 bar at a new seed after the change (49a11539, seed 5353, 10,000 programs, every one at full in
  both settings, plain and stopping everywhere, bounded, mixed, against none and on the i7-8750H, the first in
  Chrome; `l2-seed5353-10000.log`, 1,154 s, 3 at once; Mac load 22 to 34, i7-8750H 1.2 to 3.6): 10,000 with 0
  mismatches; 3,408 faulted, 1,244 left to an address at full, 24,105 stacks lost and 23,827 pieces
  forgotten. The i7-8750H was unreachable for a few minutes in the middle (ping: host down); the harness
  reconnected and no case failed.
- ARM64 after the change (`set_flags` is the translator's, so ARM64's output at full changes too): the ARM64
  tests of the test crate on the rented machine (`arm-arm64.log`) showed one test target failing, but the
  command kept only the last 400 lines of output, so which one is lost and that run decides nothing; the
  suite on the Mac below runs every ARM64 test, layer 1 among them.

Proof at the end of run 7 (at 49a11539; the Mac, load 26 to 66 from other tracks; the i7-8750H as reference,
load 1.2 to 4.7; Docker through the VM of run 6, with `target/programs` mounted writable):
- Suite (the command of run 3 with `DOCKER_HOST` set, `-j4`, `suite.log`, 2,523 s): 447 passed, 0 failed, 13
  ignored.
- `cargo run -q -p wcontainers-literals -- check`: exit 0. `cargo fmt --check`; clippy with `-D warnings` on
  the x86, interface, translator, ARM64 and test crates (all targets): clean. Baseline lines: wcontainers-x86
  142, wcontainers-translator-test 2114, wcontainers-guest-cpu 28, wcontainers-translator 74.

Timings on the i7-8750H (load 3.5 to 12.3 from other jobs, so far from quiet; one run a round):
- The flags change alone, babfb595 against 49a11539 in wasmtime as alternating pairs (`pairs.sh 5 3 4 6 8`,
  `pairs-run7.log`, load 3.5 to 8.2): the same build's runs spread by up to 1.9 times between rounds (sort
  4.97 to 9.83 s), more than any difference between the builds, so the times decide nothing. Best of 5, old
  then new: hash 0.764, 0.768 s; sort 4.97, 4.98; matrix 1.903, 1.368; search 2.655, 2.471.
- The benchmark set as the general-register programs of this run, at 49a11539, natively, in wasmtime and in
  Chrome 158 in alternating rounds (`time-rounds.sh 3`, `rounds-run7.log`, load 4.4 to 12.3), best of 3, times
  native in brackets: fib native 0.901 s, wasmtime 10.380 (11.53), Chrome 9.760 (10.84); sieve 0.912, 2.802
  (3.07), 3.894 (4.27); hash 0.673, 0.824 (1.22), 1.743 (2.59); sort 0.796, 5.622 (7.06), 6.975 (8.76);
  interpret 0.382, 11.347 (29.73), 61.911 (162.19); matrix 0.214, 1.444 (6.74), 1.264 (5.90); walk 4.942,
  5.568 (1.13), 4.750 (0.96); search 0.319, 1.405 (4.41), 1.697 (5.32). Every output equalled the native one.
  These are other programs than run 3's (no SSE) at a higher load, so they do not compare with run 3's.

What comes next: nothing of X4's bar is open. The VM is stopped and kept as in run 6.

## X4, run 6 (2026-11-19)

Machines: the Mac (load 4 to 7), the i7-8750H (load 1.4 to 4.8), and a Linux VM on the Mac (below). Logs in
scratch `x4/run6/`.

- Docker Desktop on the Mac still says "Docker Desktop is unable to start"; it was left alone. In its place:
  a Linux VM on the Mac's own CPU through Apple's Virtualization framework, made by Lima 2.3.0 (from nixpkgs,
  Apache-2.0, a test tool only), with Docker Engine 29.10.0 inside (Ubuntu 26.04, Linux 7.0.0, 4 cores, 4 GB;
  its `/proc/cpuinfo` says implementer 0x61, Apple). State under scratch `x86/lm`; the Docker socket is
  forwarded to `x86/lm/d.sock`, and the tests reach it by `DOCKER_HOST` (how, under "How to run"). The
  reference runner runs on the M5's cores as under Docker Desktop, so layer 1's verdict is the Mac's.
- `every_form_matches_the_cpu_arm64` and `sample_matches_the_cpu_in_chrome_arm64` on the Mac that way (code of
  ARM64, the translator, the interface and the abi crate unchanged since fc712bf3; `arm64-l1-shown.log`):
  1,215 forms, 64 cases a form, seed 1, 0 failing; in Chrome 1,215 cases, 0 failing. FPSR's write matches on
  the M5, as at fc712bf3.
- Literals, a part of the fix list: x86's conditions are `Condition`, an enum of the sixteen in the order of
  their encoding, with `test()` (what it tests, `Test`) and `negated()`, in place of the numbers 0 to 15 and
  `>> 1`, `& 1` and `| 1` on them; `jcc`, `setcc` and `cmovcc` return it and `Guest::Cond` is it. PF's mask is
  `u8::MAX` and AF's bit the low nibble's width. Baseline lines of wcontainers-x86: 187 to 142. Output sizes of
  `measure_x86_64` (all twelve programs and look-aheads) are the same as run 5's after both changes
  (`measure.log`, `measure2.log`).
- Tests of the condition change against the i7-8750H: layer 1 for every jcc, setcc and cmovcc form (96 forms,
  500 cases a form, seed 6006, `l1-conditions.log`, 1,903 s): 0 failing. The x86 crate's tests, x86's checks
  and programs (`x86-crate.log`, `x86-programs.log`): pass. Layer 2, seed 7171, 1,000 programs, every one also
  on the CPU (`l2-seed7171.log`, 185 s): 0 failing (360 faulted).
- X4's layer 2 bar again on the condition change, at a new seed (c260a579, seed 8282, 10,000 programs, every
  one at full in both settings, plain and stopping everywhere, bounded, mixed, against none and on the
  i7-8750H, the first in Chrome; `l2-seed8282-10000.log`, 1,577 s, 3 at once; Mac load 6 rising to 80 from
  other tracks, i7-8750H 3.3 to 5.3): 10,000 with 0 mismatches; 3,547 faulted, 1,273 left to an address at
  full, 23,244 stacks lost and 23,311 pieces forgotten. The parity and adjust change after it keeps every
  output size and is covered by the suite's layer 1 below.

Proof at the end of run 6 (at 5d83b51b; the Mac, load 30 to 80 from other tracks; the i7-8750H as reference,
load 1.6 to 1.7 at the end; Docker through the VM):
- Suite (the command of run 3 with `DOCKER_HOST` set, scratch `x4/run6/suite.log`): 445 passed, 2 failed, 13
  ignored. The 2 were layer 3's gcc tests, `csmith_programs_match_linux_arm64` and
  `hand_picked_programs_match_linux_arm64`: gcc in the VM could not write its output, because Lima mounts the
  home folder read-only. With `target/programs` mounted writable, `--test layer3` again (`layer3.log`, 545 s):
  3 passed. So every test of the suite passes on the Mac: 447 passed, 0 failed, 13 ignored, the ARM64 tests
  that need Docker among them.
- `cargo run -q -p wcontainers-literals -- check`: exit 0. `cargo fmt --check`; clippy with `-D warnings` on
  the x86, interface, translator and test crates (all targets): clean. Baseline lines: wcontainers-x86 142,
  wcontainers-translator-test 2115, wcontainers-guest-cpu 28, wcontainers-translator 87. The check's list of
  changes that could add lines is the same 10 as before (two starts, eight inits after merges of new rules).
- Left in the x86 crate's literal list: the SIMD lane counts and shifts (`simd.rs`), the register numbers'
  table and operand sizes (`operand.rs`), the guest's bit table (`guest.rs`), the block's layout (`lib.rs`),
  the build script's strings, and the reference runner's own 0x37f, 0x1f80 and 80-bit mask (it depends on no
  x86 crate).

What comes next: nothing of X4's bar is open. The VM is stopped and kept (scratch `x86/lm`, 3.3 GB, plus
Lima's image cache in `~/Library/Caches/lima`, 0.6 GB) for the independent check's Docker tests while Docker
Desktop does not start; `limactl delete x` once it is no longer needed. The rest of "Left for later" under run
4 stands.

Machines: the Mac (load 4 to 8), the i7-8750H (load 1.4 to 3.3), the rented ARM64 machine (Graviton4).
Logs in scratch `x4/run5/`. Docker Desktop on the Mac still says "Docker Desktop is unable to start"; it is the
owner's and was left alone.

- The ARM64 tests the suite could not run on the Mac: run on the rented ARM64 machine at 0f8df9bb (the code of
  5122abd0 plus the plan sentence below), `arm-run <checkout> --name x86 -- 'cargo test -j $ARM_JOBS -p
  wcontainers-translator-test --no-fail-fast -- arm64'` (`arm-arm64.log`, 1,480 s, machine load 7.7 to 15.2):
  50 passed, 2 failed. Of the 25 Mac failures of run 4, 23 pass there: browser 1, checks 2, layer 2 1, layer 3 3
  (the machine's manual says layer 3 gives the Mac's verdicts), programs 13, and layer 1's no-form, hand-made and
  refusal tests. The 2 failures are `every_form_matches_the_cpu_arm64` and `sample_matches_the_cpu_in_chrome_arm64`,
  each failing only on `MSR_SR_systemmove` writing FPSR, the difference of that machine's CPU the manual lists
  (Neoverse V2 keeps FPSR bits 28 and 29); its manual puts layer 1's bars on the Mac. So those two still need
  the Mac's Docker. Nothing in ARM64, the translator or the interface changed since fc712bf3, where the whole
  suite passed on the Mac.
- Plan 5.6 now says how a rep string's passes let a stop land between any two elements (each pass ends on an
  element boundary through the one stop check, and a bulk pass's element count is the one value a limit would
  bound). Nothing of the progress count is built (the translator track builds definition E).
- Literals, a part of the fix list: the length decoder's encoding bytes (VEX, EVEX, the three-byte escapes,
  the maps), ModRM's fields and modes, and its immediate and displacement sizes are named in `encoding.rs` or
  derived from `size_of`, and its tables are read as an `After` enum at compile time; the length sweep imports
  the prefix, escape and VEX bytes, rip's field and the length limit; the meanings use `W8` to `W64` and
  `SLOT` (`operand.rs`) instead of bare bit and byte counts. Every replacement keeps the value, and the output
  is unchanged: `measure_x86_64` gives the same WebAssembly and gzip sizes, byte for byte, for all twelve
  programs and look-aheads as run 4 (`measure.log`); `lengths_are_the_reference_cpus` (the whole sweep) and the
  x86 programs and checks against the i7-8750H pass (`x86-after-widths.log`). The sweep binary itself was not
  run again (25 minutes on the reference; its probes are the same bytes). Baseline lines: wcontainers-x86 328
  to 187, wcontainers-translator-test 2138 to 2115, guest-cpu 28, translator 87. Left in the x86 crate: the
  SIMD lane counts and shifts (`simd.rs`), the register numbers' table (`operand.rs`), the reference runner's
  own 0x37f, 0x1f80 and 80-bit mask.

Proof at the end of run 5:
- Suite on the Mac (the command of run 3, at 018231ef, Mac load 5.5 to 7.7, the i7-8750H as reference;
  `suite.log`): 422 passed, 25 failed, 13 ignored. The 25 are the same ARM64 tests as in run 4, every one
  panicking because Docker Desktop on the Mac does not start; every other test passes. 23 of them pass on the
  ARM64 machine (above).
- `cargo run -q -p wcontainers-literals -- check`: exit 0. `cargo fmt --check`; clippy with `-D warnings` on
  the x86 and test crates (all targets): clean.

What came next for X4 (done in run 6): the Mac's verdict of `every_form_matches_the_cpu_arm64` and
`sample_matches_the_cpu_in_chrome_arm64`.

## X4, run 4 (2026-11-19)

Machines: the Mac (Apple M5 Max, load 40 to 90) and the i7-8750H (load 3.8 to 10.3 from other tracks' jobs).
Logs in scratch `x4/run4/`.

The independent check's findings, each fixed:
- Switch tables (blocking): gcc and clang put a zero-extending `mov r32, s32` of the index between the guard
  and the table load for a switch on a 32-bit value; the guard search stopped at it, so every such switch
  went through the lookup. `habits::jump_table` now follows the index through that move (`zero_extended`).
  Layer 2's emitter writes both compilers' real forms (gcc: table address, `mov t32, t32`, `notrack jmp`;
  clang: `mov u32, t32` into the register the entry is loaded into, then the table address).
  `programs/x86_64/switch_tables.c` holds gcc 9.4's and clang 10's own output as they wrote it;
  `compilers_switches_match_linux_x86_64` checks it against the i7-8750H at none and full and that full
  counts no "jump through a register not recognised" at every level and in both settings. Without the fix
  it fails (1,400 such jumps at -O0).
- Lazy flags of shifts (blocking): shl, sal, shr and sar keep a `Shift` shape (three locals, or eight when the
  count may be zero), so their PF, ZF and SF are worked out only when read. (This said too that nothing was
  worked out for flags nobody reads; that was wrong until run 7: CF and OF were still worked out, and the
  shape's locals and the flag state written, for flags nothing read.) Rotates, bt and its kin, clc, stc and cmc hand the six flags over each in a local
  (`Flags::Each`) instead of packing a word and unpacking it. How this departs from plan 5.1's text is under
  "Departures", X4; plan 5.1's "As built" paragraph is rewritten. Layer 2's emitter now also writes shifts
  and rotates by immediates 0 to 64, bt, bts, btr and btc, and clc, stc and cmc.
- Literal check: merged origin/phase-1-literals at its closing tip 7668c010 and ran its init once (6e7d8d8d,
  the commit after the merge, which changed one line). Baseline lines of this track's crates now:
  wcontainers-x86 328 (11 fewer: `Flag::bit` and the flags table derive from `CF` to `OF`, the shape table's
  size from its one width's list, `with_locals` matches slices, the shift count's mask is one function),
  wcontainers-guest-cpu 28, wcontainers-translator 87, wcontainers-translator-test 2138. Left: the reference
  runner's own 0x37f, 0x1f80 and 80-bit mask (it depends on no x86 crate), as recorded at X4 run 1.
- The canonical-address check's cost: measured (below; plan 12 updated).
- Size and compile time of x86 output at full: measured (below). The `measure` test now runs at none and full
  for each guest (`measure_x86_64` added).
- fib's per-call costs: measured (below); still a design question, not changed.
- Plan 7.4's rep strings in layer 2: recorded as left to X5 (plan 7.4 and "Departures", X4).
- The stale proof paragraph of X2 run 1 is gone from "Where things stand".

Also: `programs/x86_64/shift_flags.c`, three functions of this run with flags of shifts, rotates and bit tests
live across calls (a shift by cl of 0 keeping a compare's flags, by 1, and by 32 which masks to 0; shr and
sar by immediates; a rotate, a bt, and a callee's stc and cmc under a sub's ZF). Native output on the
i7-8750H, as worked out by hand first: `kept 0 1 1 0 1 0`, `kept 1 1 0 0 0 1`, `kept 32 1 1 0 1 0`,
`shifted 1 0 1 1 1 0`, `turned 1 0 1 1 1 0 1`. `shift_flags_across_calls_match_linux_x86_64` matches it at
none and full in both settings.

Measured:
- Layer 2 x86, seed 4, 500 programs, every one also on the CPU, after the switch change (Mac load 60 to 80):
  0 failing (162 faulted). Seed 5, 1,000 programs, after the shape change and the new emitter forms (Mac load
  38 to 72): 0 failing (351 faulted, 130 left to an address at full, 2,341 stacks lost, 2,346 pieces forgotten).
- Output at none (from every instruction) and full (from the entry), translated for speed, best of 5,
  `measure_x86_64` on the Mac at load 7 (`measure.log`; sizes the same in an earlier run at load 60 to 90):
  bench -O2, 7,868 bytes of code (3,595 gzipped): none 390,793 bytes of WebAssembly (49.7 a byte of code;
  46,680 gzipped, 13.0 a gzipped byte), translated in 1.91 ms, compiled by wasmtime in 42.8 ms; full 641,282
  bytes (81.5; 106,716 gzipped, 29.7), 5.01 ms, 36.1 ms. hello -O2 with the C library, 13,583 bytes (8,880):
  none 716,610 (52.8; 88,246, 9.9), 3.43 ms, 85.9 ms; full (what the entry reaches) 119,343 (8.8; 20,844, 2.3),
  1.37 ms, 5.7 ms. integers -O2, 3,693 bytes: none 178,041 (48.2), 0.90 ms, 20.9 ms; full 187,822 (50.9), 2.50
  ms, 15.1 ms. tables -O2, 1,758 bytes: none 86,126 (49.0), 0.49 ms, 8.6 ms; full 91,416 (52.0), 1.14 ms, 8.5
  ms. The -O0 builds are in the log.
- Timings of the shape change and of the canonical check, in wasmtime on the i7-8750H, three builds
  alternating one run each a round, 5 rounds (`variants.log`, `variants.sh`; load 3.8 to 8.9, with this run's
  layer 1 on the same machine for part of it): the spread between rounds (interpret 11.2 to 29.8 s) was far
  larger than any difference between the builds, so the times decide nothing. Counted instead as user-mode
  instructions of one run (`perf stat -e instructions:u`, `counts.sh`; repeated runs agree to 0.01%):
- The shapes (b1ccb5ba against c72c85c5): hash 10.507 G to 10.710 G (+1.9%, not explained: its loop's
  `shr $29` flags are never read, and the WebAssembly of that loop at full, read in run 4 (scratch
  `hashloop.wat`, from `scratch_dump.rs`), holds for each shift only its result, CF and OF and three copies into
  the shape's locals that nothing reads, besides the flags state constant every flag setter stores), sort 54.72 G to 53.74 G (-1.8%), the others within 0.15%.
- The canonical-address check (c72c85c5 against the same with the check emptied): fib 1.4%, sort 2.0%,
  interpret 1.4%, search 4.4% of their instructions; sieve, hash and matrix move by up to 1.7% the other way,
  which is the noise of code layout.
- fib's per-call costs (c72c85c5 against builds without each, wrong in general and made only to count): the
  flags word packed into the block before every call is 12.1% of fib's instructions and 9.7% of sort's; the
  passed-in-only registers stored at each return and loaded after each call are 6.5% of fib's and 3.6% of
  sort's. Interpret and search: 1.4% each, or less. Doing without them needs to know whether a callee reads
  the flags or those registers (a callee's own liveness at entry); that stays a design question for the
  translator.

- Layer 1 for every shift, rotate, bt, bts, btr, btc, clc, stc and cmc form (123 forms), 1,000 cases a form,
  seed 4405, after the shape change (`l1-shapes.log`, 2,017 s, Mac load 7 to 90, i7-8750H load 4 to 14): 120
  forms with 0 mismatches; 3 (Rol_rm8_1, Ror_rm8_imm8, Sar_rm32_1) stopped when SSH to the x86 machine timed
  out five times; run again alone with the same seed (`l1-shapes-again.log`): 0 mismatches.
- X4's layer 2 bar after this run's changes, at a new seed (d4ad56d2, seed 31337, 10,000 programs, every one
  at full in both settings, plain and stopping everywhere, bounded, mixed, against none and on the i7-8750H,
  the first in Chrome; `l2-seed31337-10000.log`, 2,247 s, 3 at once): 9,997 with 0 mismatches; 3,483 faulted,
  1,257 left to an address at full, 23,568 stacks lost and 23,506 pieces forgotten. Cases 676, 3192 and 5711
  stopped in the same SSH outage; each run again alone (`l2-seed31337-again.log`): 0 failing. So 10,000 with 0
  mismatches.

Proof at the end of run 4 (the Mac, load 7 to 10; the i7-8750H as reference, load 4.6 to 6.9):
- Suite (the command of run 3, scratch `x4/run4/suite.log`, at 5122abd0's code): 422 passed, 25 failed, 13
  ignored. All 25 failures are ARM64 tests whose reference runs in Docker on the Mac: Docker Desktop there
  says "Docker Desktop is unable to start" (`docker info`), so every one panics with "the reference in
  Docker: the reference did not answer" (289 such panics, plus layer 3's "the gcc image did not build").
  Docker is the owner's and was left alone. This run changed no file of the translator, ARM64 or interface
  crates (`git diff --stat fc712bf3..HEAD`), so ARM64's output cannot have changed; those 25 must be run
  again once Docker works. Every x86 test, the literal check's tests and the ARM64 tests without Docker
  passed. The runner panic "mid > len" in the log is the refusal test's own.
- `cargo run -q -p wcontainers-literals -- check`: exit 0. `cargo fmt --check` and clippy with `-D warnings` on
  the x86, translator and test crates (all targets): clean.

Left for later (in order; none is in X4's bar):
1. interpret's absolute switch table (plan 3.4's decision, for X9).
2. The flags word handed to every callee and the passed-in-only registers (measured above: 12.1% and 6.5% of
   fib's instructions): a callee's liveness at entry would let both go; a translator design question.
3. hash's 1.9% more instructions after the shape change, not explained.
4. `programs_match_linux` for x86 still reaches `bsr` and `bswap` (X5's list); layer 3 is X5's.

## X4, run 3 (2026-11-18)

Machines: the Mac (Apple M5 Max, load 90 to 110) and the i7-8750H (load 2.8 to 10.5 from other tracks'
jobs, recorded beside every run in the logs). Logs in scratch `x4/run3/`.

Done:
- The benchmark set timed on the x86 machine natively, in wasmtime and in Chrome, in alternating rounds
  (native, wasmtime, Chrome, then again; one run each a round, so a change of load falls on all three):
  `time-blob rounds` and `time-rounds.sh` (see "How to run"). Chrome there is 158, unpacked into the track's
  folder with Node and playwright-core (see "The x86 machine" and "Departures", X4).
- Frame checks as plans 3.4 and 5.3 describe them for x86, which were not built: a move down makes no check
  when the frame already covers its red zone; the check of a run of moves down (a prologue's pushes) reaches
  down for all of them, so the run makes one check, not one a push; a call that pushes its return address
  checks no frame for the callee (each callee checks its own); a check made where nothing is known covers
  the return address at the stack pointer too, so the `ret` that reads it is unchecked. One function
  (`frame_checked`) decides each check and the frame it leaves for both the forward pass and the check
  plans. fib's function went from four frame checks and a checked `ret` to one check. Layer 2 x86 at seed 3,
  1,000 programs, every one also on the CPU: 0 failing (336 faulted, 116 left to an address, 2,386 stacks
  lost and 2,402 pieces forgotten in mixed runs). ARM64's layer 2, changes, checks, programs, look-aheads
  and the test guest pass.
- X4's layer 2 bar again after the frame change, at a new seed (3542d8e, seed 7777, 10,000 programs, every
  one at full in both settings, plain and stopping everywhere, bounded, mixed, against none and on the
  i7-8750H, the first in Chrome; `l2-x86-seed7777-10000.log`, 2,532 s, 3 at once, Mac load 72 to 110): 0
  failing; 3,481 faulted, 1,257 left to an address at full, 23,662 stacks lost and 23,404 pieces forgotten.
- Why interpret is slow: its switch is clang's absolute table (`jmp [T + r*8]`, the benchmarks are built
  without PIC at a fixed address), which plan 3.4 sends through the lookup because the translator never sees
  the load address; every dispatch is counted as "jump through a register not recognised" (240,004 at each
  of two jumps a run). That is plan behaviour, measured on real binaries at X9 for a decision; here it costs
  interpret 33 times native in wasmtime and 166 to 184 in Chrome.
- fib: per call the rest is the flags word packed into the thread block before the call (about 45
  operations with parity), the passed-in-only registers stored at the return and loaded after the call,
  and the callee-saved registers kept in the thread block across the call. Not changed.
- Literal check: exit 0; baseline lines of this track's crates now wcontainers-x86 339, wcontainers-guest-cpu
  28, wcontainers-translator 87, wcontainers-translator-test 2139 (4 fewer: the runner's name and its
  `serve` command named once in `reference.rs`); the load probe, written three times in tests, is one
  library function (`programs::load`) that reads `/proc/loadavg` on Linux as plan 7.5 says.

Measured, the benchmark set (bench -O2, SCALE=1, each workload alone) translated at full for speed with every
check, best of 5 alternating rounds on the i7-8750H, pinned off cores 2 and 8, times native in brackets:
- Before the frame checks (5e89009, `rounds-5e89.log`, load 3.2 to 10.5): fib native 0.721 s, wasmtime
  13.633 (18.90), Chrome 13.456 (18.66); sieve 0.727, 1.988 (2.74), 2.567 (3.53); hash 0.579, 0.693 (1.20),
  1.408 (2.43); sort 0.624, 5.042 (8.08), 5.578 (8.94); interpret 0.345, 11.324 (32.80), 57.236 (165.76);
  matrix 0.130, 0.927 (7.14), 1.000 (7.71); walk 3.924, 4.571 (1.17), 4.028 (1.03); search 0.201, 1.423
  (7.09), 1.347 (6.70).
- After (3542d8e, `rounds-3542.log`, load 2.8 to 8.0): fib 0.717, 8.522 (11.88), 8.255 (11.51); sieve 0.685,
  2.205 (3.22), 2.604 (3.80); hash 0.579, 0.686 (1.18), 1.413 (2.44); sort 0.624, 4.595 (7.36), 5.427 (8.69);
  interpret 0.343, 11.169 (32.56), 63.282 (184.49); matrix 0.136, 1.077 (7.94), 1.095 (8.07); walk 4.025,
  4.444 (1.10), 3.933 (0.98); search 0.201, 1.313 (6.54), 1.418 (7.06).
- Every run's output equalled the native one (the tool asserts it).
- The frame checks alone, old (5e89009) and new (3542d8e) alternating in wasmtime, one run each a round, 5
  rounds, i7-8750H at load 6.2 to 14.0 (`old-new.log`, `old-new.sh`): fib old 13.72 to 15.48 s, new 9.33 to
  11.31 (best 13.72 against 9.33, 32% less); sort old 5.10 to 6.39, new 5.43 to 7.73 (old faster in four
  pairs of five, though the new code makes fewer checks: within this load's noise, not explained); search old
  1.47 to 2.17, new 1.32 to 1.73.
- Between the two rounds runs the load differed (above), so their other differences are not the change's.
- Not measured: the same set in Chrome and wasmtime on the Mac (plan 7.5's second part; the Mac's load of
  about 100 makes any time there meaningless today).

Proof at the end of run 3 (the Mac, load 70 to 110; the i7-8750H as reference):
- Suite (`WC_REFERENCE_X86_64=tom@192.168.2.240 nix develop -c cargo test -p wcontainers-guest-cpu -p
  wcontainers-arm64 -p wcontainers-translator -p wcontainers-translator-test -p wcontainers-x86 -p
  wcontainers-literals --no-fail-fast -- --test-threads 3`, scratch `x4/run3/suite.log`): 397 passed, 0
  failed, 13 ignored.
- `cargo run -q -p wcontainers-literals -- check`: exit 0. `cargo fmt --check` and clippy with `-D warnings`
  on the x86, interface, translator and test crates (all targets): clean.
- X4's pass bar as plan 9 writes it: layer 2 at 10,000 programs with zero mismatches against the CPU and
  against none (seed 7777, after this run's translator change); the benchmark set the same at none and full
  (`benchmarks_match_linux_x86_64`) and timed on the x86 machine natively, in wasmtime and in Chrome (above).

Left for later (in order; none is in X4's bar):
1. interpret's absolute switch table (plan 3.4's decision, for X9) and fib's per-call costs (flags packed
   for the callee, passed-in-only registers) are the two large slow spots; both are design questions, not
   bugs.
2. The canonical-address check's cost (plan 12, "measured at X4") is not measured.
3. `programs_match_linux` for x86 still reaches `bsr` and `bswap` (X5's list); layer 3 is X5's.

## X4, run 2 (2026-11-18): partly done

Machine for everything below: the Mac (Apple M5 Max, load 20 to 90 during the run) with the i7-8750H as the
x86 reference (load 1.0 to 1.4 from other tracks' jobs). Logs in scratch `x4/run2/`.

Done:
- Literal check adopted: merged origin/phase-1-literals (f561838a) and ran its `init` in the merge commit
  (0f3b3905; see "Departures", X4). Baseline lines of this track's crates at adoption: wcontainers-x86 339,
  wcontainers-guest-cpu 28, wcontainers-translator 87, wcontainers-translator-test 2146 (9172 in all, the
  other tracks' included). After this run: 3 fewer in the test crate (two panic texts and the `"layer2"`
  name), none added; every new line of code this run names its values. `check` lists six changes that could
  add lines: 0f3b3905 and 68214b2e (the starts: this branch's merge and the literals branch's own) and
  89f88281, 7119e3f3, c3bf56d4, 31ec396f (the literals branch's new rules).
- The brief's runner bug (fp + 472 read as the FP area's size) was already fixed at X3 run 3 (b4a8cda); no
  recorded reference existed to be affected.
- The uncommitted flag shapes of run 1 (`flags::SHAPES`, `shape`, `with_locals`, `condition_direct` for a
  plain difference and the logic instructions) were checked and committed (78a77dd).
- Translator bug: x86 is the first guest whose `PASSED_IN` is larger than `PASSED_OUT`. A callee's writes to
  rdi, rsi, rcx, r8 and r9 were lost on return at every look-ahead (they live in parameters, which liveness
  does not follow, and a return passes out only rsp, rax and rdx). Now a return stores the registers passed in
  only into the block and a call reloads them (`code::passed_in_only`); for ARM64 the list is empty, so its
  output is unchanged. Found by layer 2's first cases (a function that makes the model's system call).
- `pushf`/`pushfq` (needed for the dump) and `leave` have meanings; layer 1 passes for `Pushfq`, `Pushfw`,
  `Leaveq`, `Leavew` at 1,000 cases a form (seed 4404). Layer 1's reconcile clears the runner's own trap flag
  in the image `pushf` stores, as it already did for r11 after `syscall`.
- x86's layer 2 emitter (`src/guest/x86_64/assemble.rs`, iced's `CodeAssembler`, feature `code_asm` of the
  existing iced dependency): every model operation, plus read-change-write stores, lods and stos with the
  direction flag both ways, push and pop of registers and memory, calls and jumps through memory, partial
  register writes (8-bit low and high, 16-bit, 32-bit), inc and dec (CF kept), cmov, setcc, the divisions
  (divisor made odd, and positive when signed, so they never trap), shifts and rotates through cl,
  red-zone saves in leaf functions, gcc's (`notrack jmp`, sum in the index) and clang's (sum in the table
  register) position-independent switch tables, and `pushfq` for the dump. The model's registers for x86 are
  now rax, rcx, rdx, rbx, rsi, rdi, r8 to r11 (r12 to r15 are the emitter's scratch and counters).
- Switch recognition (`habits::jump_table`): the walk back from the jump crosses a conditional branch's
  fall-through now (the translator's `before` stopped at every block end, so it never reached the guard;
  ARM64 does not use `before`). `branch_comparison` (cmp or sub with a register or a non-negative immediate,
  test r, r, dec before je or jne) and `loaded_max` (movzx, mov r32 of a double word) are built.
- x86's `checks()`: both switch forms taken through their tables, which must count "jump through a register
  not recognised" zero times (passes, also before the walk fix it failed), and a group failing across the
  blob's end on a function's first instruction (none, full, Linux and Chrome agree).
- `programs/x86_64/flags_across.c`: three functions of this run with flags live across calls (through a
  callee that leaves them, set in part by a callee with sub then inc, and a loop driven by a callee's dec),
  equal to the i7-8750H at none and full in both settings (`flags_across_calls_match_linux_x86_64`); run
  natively by hand its output is `through 42 05`, `halves 01 00 01 00 01 00`, `loop 06 02`, as worked out.
- `measure_benchmarks_x86_64` (ignored, as ARM64's): native on the x86 machine, wasmtime and Chrome here.

Measured:
- Layer 2, seed 1: 1,000 programs, each also on the CPU, 0 failing (342 faulted), twice (before and after
  the recognisers), 108 s each with 3 at once.
- Layer 2, seed 2026, 10,000 programs, each also on the CPU, one in Chrome (`l2-2026-10000.log`, 1,196 s,
  Mac load 80 to 113): 1 failing, case 7461, status 135 (SIGBUS) at none, and the same on the CPU: the model
  makes a pair loaded into one register two by taking the next register number, which for rbx is rsp, so
  `pop rsp` set a random stack pointer. The emitter now takes the next of the model's registers (bdcc84c);
  case 7461 then passes against none and the CPU.
- Layer 2, seed 2026, 10,000 programs again after that fix (bdcc84c, `l2-2026-10000-b.log`, 1,312 s, 3 at
  once, Mac load about 90): every program at full (both settings, plain and stopping everywhere), bounded,
  mixed, on the i7-8750H, and the first in Chrome: 0 failing; 3,368 faulted, 1,191 left to an address at
  full, 23,893 stacks lost and 23,762 pieces forgotten in mixed runs. This is X4's layer 2 bar.
- The benchmark set (bench -O2, SCALE=1, each workload alone), x86-64 guest, translated at full for speed
  with every check. Natively on the i7-8750H (best of 3, through the harness; its load 1.3 to 1.5 from other
  tracks' jobs on the two cores left to them) and in wasmtime on the same machine (best of 3, `time-blob`
  built there, pinned off cores 2 and 8, load 1.3 to 1.7), times native: fib 0.700 s native, 10.918 s
  wasmtime (15.6); sieve 0.671, 2.004 (3.0); hash 0.553, 0.736 (1.33); sort 0.609, 5.008 (8.2); interpret
  0.339, 10.994 (32.4); matrix 0.127, 0.909 (7.2); walk 3.679, 4.318 (1.17); search 0.194, 1.363 (7.0).
  Outputs equal the native ones (`time-wasmtime.out` there, `measure-x86.log` here). The same translations
  in wasmtime on the Mac (Apple M5 Max, load 96 to 115, so slow by an unknown amount): fib 5.057 s, sieve
  0.901, hash 0.542, sort 2.133, interpret 5.334, matrix 0.377, walk 2.996, search 0.419. fib and interpret
  are 15 to 32 times native: not looked into yet (calls and returns, and the passed-in-only registers stored
  at every return, are the first suspects). Chrome on the x86 machine is not measured yet (plan 7.5).

Proof at the end of run 2 (the Mac, load 90 to 113; the i7-8750H as reference):
- Suite of what this track owns, with the literal check (`WC_REFERENCE_X86_64=tom@192.168.2.240 nix develop
  -c cargo test -p wcontainers-guest-cpu -p wcontainers-arm64 -p wcontainers-translator -p
  wcontainers-translator-test -p wcontainers-x86 -p wcontainers-literals --no-fail-fast -- --test-threads 3`,
  scratch `x4/run2/suite.log`, about 50 minutes): 397 passed, 0 failed, 13 ignored. The two runner panics in
  the log are the refusal test's own.
- `cargo run -q -p wcontainers-literals -- check`: exit 0. Clippy with `-D warnings` on the x86, translator
  and test crates (all targets), `cargo fmt --check`: clean. `cargo check --workspace` fails only in
  `wcontainers-wasmtime` (see "Departures", X4).

Not done (in order):
1. The benchmark set in Chrome on the x86 machine (it has Chrome; the browser harness runs Chrome only on
   the machine the tests run on, so it needs the test crate run there, with zig and a reference: it has no
   zig, Docker or SSH key to itself). wasmtime there is done with `time-blob` (below). Then why fib and
   interpret are so slow.
2. Layer 3 (Csmith) for x86.
3. `programs_match_linux` for x86 still reaches `bsr` and `bswap` in `integers` (X5's list).
4. The first frame check covering a `Stack` return slot (plan 3.4) is not measured.
5. The passed-in-only registers are stored at every return and loaded after every call; liveness of
   parameters would let most of that go. Cost not measured.

## X4, run 1 (2026-11-18): partly done

Fixed first, the independent check of X3's seven findings (all minor):
- Plan 7.2 said `presented`, `REFERENCE_HAS` and layer 1's `Err` rule came at X3; they come with X5's
  never-claimed list (plan and "Departures", X3, now say so).
- "Departures", X3, now lists every default-claimed instruction X3 left without a meaning (shld, shrd, push
  and pop of fs and gs, mov of segment registers, the far transfers, enter, lahf, sahf, popf, popcnt, lzcnt,
  tzcnt, movbe, adcx, adox, crc32, cmpxchg8b and 16b, the fs and gs base instructions, rdtsc, cpuid, the
  fences, movnti, besides those it named).
- `tests/no_std.rs` builds the x86 crate's core too.
- `may_vary` is true for the kernel-only instructions a kernel can grant to user code (rdpmc, in, out, ins,
  outs, cli, sti); `tests/facts.rs` checks four of them.
- Literals: the FXSAVE area is a typed struct (`wcontainers_x86::Fxsave`), so MXCSR's and the xmm registers'
  offsets are field offsets and the harness reads fields, not byte ranges; `FCW_AT_START`, `MXCSR_AT_START`,
  `X87_REGISTER_BITS`, `ARITHMETIC_FLAGS`, `MODE_BITS` (iced's bitness), `RSP_NUMBER` and the registers
  `RBP`, `R8`, `R9`, `R10` are named and used (`PASSED_IN`, `PASSED_OUT`, `SYSCALL`, `rsp_based`, DF's shift
  in the string instructions); the harness names `EDGE_BYTES`, `AIMED_BIT_OFFSETS`, `DRAWN_REX_W` and the
  unit test's draw count; the reference runner takes the legacy FP area's size and MXCSR's place from libc's
  `user_fpregs_struct`. Left in the runner: its own 0x37f and 0x1f80 (it has no dependency on the x86 crate).
- The reference server ends when no request comes for 15 minutes (`IDLE_MILLISECONDS`), so a runner whose
  connection died without closing does not stay on the x86 machine; its case child ends with it.
- x86's `smoke()` panic names X5.
- The decoder's share of the browser build is recorded as a lower bound (plan 12 too).

Built and found:
- A translator bug that broke every x86 program at full look-ahead whose caller reads flags a callee left:
  a return tested the caller's flags-wanted bit by wrapping `L_RETURN & FLAGS_WANTED` to i32, which drops
  x86's bit 62, so a callee never wrote its flags back. Now `code::push_flags_wanted` wraps only when the
  bit is in the low word (ARM64's bit 0: the same bytes as before) and compares otherwise. Found by hello
  world's final registers at full against none; `programs/x86_64/flag_returns.c` (flags handed back after a
  direct call, through a register and memory, nested, after a tail jump, before pops) fails before the fix
  and passes after (`flag_returns_match_linux_x86_64`).
- x86 at full look-ahead today: `hello_matches_linux_x86_64` (every optimisation level, none and full in
  both settings) and `benchmarks_match_linux_x86_64` (the benchmark set at none and full, both settings,
  against the i7-8750H) pass. `programs_match_linux` is not run for x86: `integers` at -O0 reaches `leave`,
  which has no meaning yet (X5's list above).
- `machine-code` takes the guest and the library (`machine-code x86_64 bench -O2 none <out> SCALE=1
  WORKLOAD=2`), for reading x86's WebAssembly.
- Literal check: main has no `wcontainers-literals` yet, and `docs/literals.md` on `phase-1-literals` (third
  version) says a track adopts the check by merging main once main has it, never the literals branch
  directly; so nothing was merged (see "Departures", X4).

What comes next for X4, in order:
1. x86's layer 2 emitter (`GuestTest::assemble`, with iced's `code_asm`), mapping every model operation to
   x86 forms that have meanings (plan 7.4), then layer 2 at 10,000 programs against the CPU and against none.
2. Lazy flags as plan 5.1 has them: the `Shift` and `Bits` shapes, `SHAPES` for the flags-word functions,
   `condition_direct` (cmp or test plus jcc as one comparison), then reading the WebAssembly of two
   benchmarks' hot loops for flag updates emitted and never read.
3. `jump_table` for both compiler styles (gcc and clang PIC tables: `lea r, [rip + T]; movsxd r2, [r + i *
   4]; add r2, r; jmp r2`; absolute tables stay counted), `branch_comparison` and `loaded_max` for the loop
   proofs, the first frame check covering the return slot (plan 3.4).
4. Layer 3 (Csmith) for x86 against the i7-8750H at every look-ahead.
5. The benchmark set timed on the x86 machine: natively (the harness already times a blob there), and in
   wasmtime there, which needs the test crate (or a small runner) built on that machine; record its load.
   Then report beside ARM64's figures in docs/status-translator.md.
6. The independent check's items: new seeds for layers 2 and 3, the benchmarks run again, the fast call path
   read for any way to return to the wrong place.

## X3: what comes next

1. The second independent check of X3.
2. Plan 7.3's recorded reference results and layer 1 batching (still proposed, not ruled).
3. For X4: the shared layer 2 model's operations are ARM64's; x86's `assemble`, `smoke`, `checks` panic
   with the milestone that builds them. A cached FS base at full look-ahead must be reloaded after a system
   call (the stand-in's `arch_prctl` writes it in the block).
4. The literal fix list (`fix-x86.md`) from its WIDTH part on, and `layout.rs`'s page size (see "X3, run
   3"). ARM64's layer 1 cases are not checked against its runner's range as x86's are: its loads and stores
   take their base from the data area (or sp, also there) and an index within 64 of zero, and its literal
   loads reach at most 1 MiB from the code area, so those cannot reach 0x8000_0000. Not checked: system
   instructions that take an address in a random register (`dc zva` and the cache maintenance ones), which
   could reach it. The runner's start check holds for ARM64 too.
5. For X5: `xsetbv` is in the AVX setting's group and has no meaning yet (with the setting on it would give
   illegal instruction, where a CPU with XSAVE gives general protection). The never-claimed list in layer 1
   (100 cases a form) and the designed answers of the VMX forms (illegal instruction, excluded from layer 1
   as plan 7.4 says) come with X5. Prefixes a CPU without a feature ignores, so that the bytes run as an older
   instruction (`F3 0F 09`, `wbnoinvd`, is `wbinvd` and general protection on the reference), are X5's
   never-claimed list to find.

## X3, run 3 (2026-11-18)

The independent check of X3 found eight problems; all are fixed but the last, which is a part of the literal
fix list (below):
- Layer 1 at seed 5150 failed `Btr_rm64_r64`: a bit offset moved the access into the reference runner's own
  image (linked at 0x6000_0000), where the CPU read the runner. Each runner's place is now one range in
  `layout.rs` (`RUNNER`, the image base and 1.25 GiB after it), which the runner checks at start (it caught at
  once that the i7-8750H put the heap 0x3b5c_9000 bytes after the image, so the range is not 512 MiB), and
  x86's generator drops every case, in a form or in no form, whose accesses (iced's `used_memory` from the
  case's registers, plus a bit offset's move) reach it. Three quarters of the bit offsets are now aimed with
  their operand. A unit test replays the check's two cases.
- `Guest::may_vary` was false for bt and its kin, `int 0x80`, `sysenter`, never-claimed `Code`s and invalid
  encodings. Now true for each (see "Departures"); plan 3.2 describes the item; `tests/facts.rs` in the x86
  crate checks every kind.
- The reserved NOP forms' cases now draw `66`, `F2` and `F3` in any order and REX.W (kept when the
  translator's decoder still gives a reserved NOP), so endbr64, rdsspq and MPX's encodings are compared; a
  unit test checks the draws reach each prefix. `endbr64` is also a hand case compared with the reference.
- `pop [esp + d]` under 67 added the stack move after wrapping; now before (a hand case compared with the
  reference: `67 8f 84 24 f8 ef ff bb` at rsp 0x4400_1000 faults at 0).
- `stack_move` of `add rsp, [m]` read an immediate from a memory operand; now `Unknown`.
- Multi-byte and reserved NOPs described a 4-byte read-write access; now none. `movapd` and `movupd` stores
  are writes.
- `jrcxz` and `jecxz` are built (the Jcc page); the others the check listed stay X5's (see "Departures").
- The eager flags of bt, clc, stc and cmc are now named under "Departures" and in plan 5.1.

Also done:
- The brief's bug in the x86 reference runner: it read `_fpx_sw_bytes.xfeatures` (fp + 472) as the size of
  its frame's FP area, so it kept and put back only a few bytes of its own FP state around each case. It now
  reads `extended_size` through a `#[repr(C)]` copy of Linux's struct, with a bound check against its buffer.
  No recorded reference exists to be affected; what it failed to put back was the runner's own MXCSR and
  vector registers, and every x86 case so far starts with the same MXCSR and x87 control word as the runner.
- Literal sweep, part 1 (page and address width) of `fix-x86.md`: `wcontainers_x86` gains
  `VIRTUAL_ADDRESS_BITS`, `USER_END`, `USER_TOP` (from `wcontainers_abi::PAGE_SIZE`; the x86 crate now
  depends on `wcontainers-abi`), `CANONICAL_SHIFT` and `is_canonical`, used by `memory_fault` and the
  canonical check of branch targets (the emitted bytes are the same), by x86's harness and by the reference
  tests; `layer2.rs` and `smoke.rs` use `PAGE_SIZE`. A new `wcontainers_x86::encoding` names the legacy
  prefixes, REX and the 0F escape (used by the decoder and the harness), and the crate names the interrupt
  vectors (`BREAKPOINT_VECTOR`, `OVERFLOW_VECTOR`, `LINUX_32_BIT_SYSTEM_CALL`). Left: `layout.rs`'s 4096s
  (the reference runner includes the file and has no `wcontainers-abi`), and every later part of the list
  (WIDTH, RFLAGS, FXSAVE, REG, the rest of ENC, COND, SIMD, LINUX, FIELDS and on). Signal and si_code
  numbers stay retyped in x86's harness until the orchestrator rules on the x86-64 system call source (the
  list's LINUX step). `phase-1-literals` has no check crate yet, so nothing was merged.

Proof of run 3 (the Mac, Apple M5; scratch `x3/run3/`):
- Layer 1, every x86 form (705: run 2's 703 with `jrcxz` and `jecxz`), 1,200 cases a form, seed 5150 (the
  check's seed), 4 at once, at 65ffbd7 (the runner's FP-area fix, b4a8cda, was written while it started, so
  its runner may be either; neither changes a case's result) (`l1-5150-1200.log`, 14:13 to 15:12, 3,485 s; Mac load 75 at the
  start, 64 at the end; the i7-8750H's load 6.2 before from other tracks' builds): passed, zero mismatches.
  The check's own run of this seed failed `Btr_rm64_r64`.
- Cases in no form, 10,000 at seed 5150 (`noform-5150-10000.log`, 352 s, Mac load 113 at the end): passed.
- The x86 crate's tests (tables 4, lengths 2, facts 3) and the harness's two new unit tests: passed.
- Suite of what this track owns at 9c77810 (`WC_REFERENCE_X86_64=tom@192.168.2.240 nix develop -c cargo test
  -p wcontainers-guest-cpu -p wcontainers-arm64 -p wcontainers-translator -p wcontainers-translator-test -p
  wcontainers-x86 --no-fail-fast`, `suite.log`, 15:13 to 16:00, Mac load 56 at the start, 124 at its
  highest, 72 at the end): 99 passed, 0 failed, 12 ignored. Both runners passed their start check (ARM64's in
  Docker, x86's on the i7-8750H).
- Clippy with `-D warnings` on the x86, test and interface crates (all targets) and on the reference runner
  for both CPUs; `cargo fmt --check`: clean.

## X3, run 2 (2026-11-18)

What was done:
- The length of every encoding iced calls invalid, cannot finish, or decodes as a never-claimed `Code` now
  comes from a table (`decode.rs`, `ONE_BYTE` and `TWO_BYTE`, plus the rules for the 0F38 and 0F3A maps, VEX
  and EVEX map fields, and 66, 67 and REX.W). The table comes from a sweep on the i7-8750H made by
  `x86-lengths` in the test crate: 300 lines (escape and filler) of 256 opcodes, 76,100 lengths, kept as
  `crates/wcontainers-x86/tests/lengths.txt`. It was run twice: the second run added XOP's `8F` and the 3DNow
  suffixes, and its 294 shared lines were the same as the first run's. `tests/lengths.rs` checks every recorded
  length against `decode`, and checks the table against iced's lengths of valid instructions where the
  sweep cannot measure (an instruction that faults however short it is). What the sweep found is in plan 5.2.
  The old rule (decode again without the repeat prefix or 66, and VEX by its fields) was wrong for many
  invalid opcodes: `06` was counted as 2 bytes where the CPU counts 1, and 0F3A as no immediate. A
  never-claimed instruction of another maker is counted by Intel's map: 3DNow's `0F 0F` with nothing after,
  XOP's `8F` as a ModRM, SSE4a's `66 0F 78` with no immediates.
- `decode` no longer returns `None` when iced runs out of bytes on an encoding the CPU counts as shorter
  (`4A D6`: iced wants more after `D6`, the CPU counts 2 bytes). This made layer 1's no-form cases panic in
  the translator once the table was in.
- Kernel-only `Code`s are layer 1 forms (49 of them; their cases got `dx`, control and debug registers,
  `ins`, `outs` and `out imm8, al`). Layer 1 decided their faults: all general protection but `rsm`, which is
  illegal instruction (#UD outside system management mode). A kernel-only `Code` whose features the CPU we
  present does not have (VMX, SVM, SMAP, SGX, XSAVES, INVPCID, CET and the rest) is now in the never-claimed
  group, which gives illegal instruction as on a CPU without the feature. Only `MSR`, `RDPMC` and `SMM`
  (`KERNEL_FEATURES`) join the default list for kernel-only instructions.
- The stand-in's thread-pointer call is now data (`StandIn::set_base`: the number, each code's block offset,
  the limit). The wasmtime and Chrome stand-ins both read it. `GuestTest::stand_in_call` is gone.
- Chrome: `sample_matches_the_cpu_in_chrome_x86_64` passes (654 forms, 1 a form, seed 1), and so does
  `hello_matches_linux_in_chrome_at_none_x86_64` (-O0, -O1 and -Os blobs; -O2 and -O3 build the same bytes
  as -O1).
- `programs/x86_64/syscall.h` and `divide.c`, and `measure_long_division_x86_64` (ignored, not a check).

Measured (plan 5.2 and 12 say so too):
- Long division, best of 5 (`x3/run2/divide.log`): 50 million `div r64` with a high half: native 21.27 ns a
  division on the i7-8750H (load 0.16 before, 1.26 after), 47.75 ns at look-ahead none in wasmtime on the
  Mac (load about 38). With a zero high half: native 6.60 ns, ours 34.35 ns. The long path's own cost: native
  14.66 ns, ours 13.39 ns, so about the CPU's own. The two sides ran on different machines.
- The decoder in the browser build, from the link map of the release x86 build (`--Map`, at 5450cf9 with the
  table; `x3/run2/x86-browser.map`, `scripts/mapsize.py`): the build is 872,638 bytes with every custom
  section stripped (265,961 gzip -9, 203,533 brotli -q 11), or 1,136,812 with `wasm-tools strip`, which keeps
  the name section (299,537 and 229,131). iced-x86's own functions and statics in it are 156,416 bytes
  (115,635 of code, 40,781 of data), 36,056 gzip and 27,565 brotli compressed alone. Mac load 22.9.
  A lower bound on what the decoder adds: the sum is of link-map symbols named for iced, so iced code inlined
  into `wcontainers_x86`'s own functions is not counted.

Proof of run 2 (the Mac, Apple M5; the i7-8750H's load given with each):
- Kernel-only forms, 49 forms, 1,000 cases a form, seed 1, 4 at once (`x3/run2/kernel-l1-1000.log`): passed.
- Cases in no form, seeds 1 to 6 at 64 cases each (`x3/run2/noform-*.log`): all passed. All six had failed
  in run 1, and failed again on the panic above before it was fixed.
- Cases in no form, 10,000 at seed 1 (`x3/run2/noform-10000b.log`, 541 s, Mac load about 40): passed, zero
  mismatches. Half are prefix runs of 13 to 16 bytes. The first try (`noform-10000.log`) never ended: a
  prefix run had moved a jump's target onto its own start, and our side, which runs a branch into its own
  bytes, looped there. Such cases are now left out, as layer 1's forms already left out branches into their
  own bytes (1a0a6b1).
- Layer 1, every x86 form (703: run 1's 654 and the 49 kernel-only), 1,000 cases a form, seed 2, 5 at once,
  at db9d971 (`x3/run2/l1-seed2-1000.log`, 09:59 to 10:40, 2,457 s; Mac load 38 to 77; the i7-8750H's load
  1.3 just before): passed, zero mismatches.
- Chrome, both guests at once (`x3/run2/chrome-both.log`, `chrome-l1-both.log`): the suite's first run of
  the Chrome tests failed for both guests, because two guests' Chrome runs wrote the same case directory
  (fixed in e51676b: each guest has its own). After the fix, ARM64's programs and x86's hello world in Chrome
  passed together (466 s), and so did both guests' layer 1 samples in Chrome (1 a form, seed 1).
- The x86 crate's tests: tables 4, lengths 2 (76,100 lengths), all passed.
- Suite of what this track owns at db9d971 (`WC_REFERENCE_X86_64=tom@192.168.2.240 nix develop -c cargo test
  -p wcontainers-guest-cpu -p wcontainers-arm64 -p wcontainers-translator -p wcontainers-translator-test -p
  wcontainers-x86 --no-fail-fast`, scratch `x3/run2/suite.log`, 10:20 to 11:07, Mac load 38 to 77): 91
  passed, 3 failed, 12 ignored. The three failures were the Chrome tests of both guests running at once
  (fixed in e51676b, above). The two targets they are in were then run again at the end: browser (both
  tests passed, above) and layer 1 (all 10 passed at 1a0a6b1, its defaults: seed 1, 64 cases a
  form, 1 a form in Chrome, 64 in no form; `x3/run2/layer1-final.log`, 11:33 to 12:07, 1,988 s).
- Clippy with `-D warnings` on the test and x86 crates (all targets); `cargo fmt --check`: clean.

## X3, run 1 (2026-11-18)

The sixth independent check of X2 found four smaller problems. What was done:
- The status file's run 6 account and its proof sat inside run 7's section: run 6 has its own section now,
  and "Cases actually run" has runs 7 and 9.
- The x86 runner's step limit took ecx or rcx by looking for a 0x67 byte; it now reads iced's operand kinds
  (`MemorySegESI`, `MemoryESEDI`), which say the address size of the decoded instruction.
- The SSH runner's directory is now the target's own (`Target::remote_directory`: x86-64's
  `wcontainers-x86/reference`, ARM64's `wcontainers-arm64/reference`); plan 7.2 says so.
- RF in the reference's rflags at fault stops: x86's `reconcile` clears it (and the runner's own TF in r11
  after `syscall`, which the CPU copies from rflags).

What was built (all at look-ahead none):
- The description section is format 4 (guest byte and offer key after the format byte; `runner.js` reads
  it). Shown on ARM64: with each module's description section written back as format 3 (after checking
  guest 1 and offer 0), `same-output.sh 072a2136 bench,busybox,musl,rg,git,python,java,go` from a worktree of
  941bc688 gave "the same" for all eight sets (505, 34, 120, 34, 34, 34, 40, 28 modules; scratch
  `x3/run1/format4-proof.log`, 06:00 to 06:23, Mac load 35.7 to 48.1). Node and Bun were not run (many GB).
- The interface gains `Guest::may_vary` (the brief's "mark what depends on the host CPU"): x86 says true for
  the instructions whose undefined flags are fixed to the reference's; ARM64 says true for every
  instruction (not audited, so nothing is said to be the same on every CPU); the test guest false.
  `Index::BitOffset` gains the operand's own index (`bt [rbx + rsi * 4], rcx`), which plan 3.2 lacked.
- `wcontainers-x86`: `X86Block` (plan 5.9) in the `no_std` core; iced-x86 `no_std` + `decoder` behind
  `translate`; the build script writes each `Code`'s claim group from iced's `instr_info` (4,936 bytes;
  tested against iced for every `Code`); meanings for mov, movzx, movsx, movsxd, lea, xchg, the eight ALU
  operations, inc, dec, neg, not, cbw to cqo, push, pop, call, ret, jmp, jcc, cmovcc, setcc, the shifts and
  rotates, mul, imul (all forms), div, idiv (64-bit with the 128-by-64 long division), movs, stos, lods, cmps
  and scas without rep, bt and its kin, clc, stc, cmc, cld, std, syscall, nop and the reserved NOPs, int3,
  int1, int n, ud0 to ud2; and the SSE2 moves, logic, adds, compares, interleaves, shuffles, shifts by an
  immediate and masks that musl's start and string routines reach (see "Departures"). Kernel-only `Code`s
  give general protection and never-claimed ones illegal instruction (counted); AVX-setting ones warn
  "an AVX instruction was reached while AVX is not offered".
- `X86_64::memory_fault`: an access whose first or last byte is not canonical gives the stack fault through
  rsp or rbp, else general protection; the kernel's half is a page fault (the plan said otherwise; fixed).
- x86's `GuestTest`: forms are every legacy `Code` valid in 64-bit mode whose meaning is written, as the
  translator's decoder gives it (654 forms); cases from iced's `OpCodeInfo` operand kinds, built with iced's
  creators and `Encoder`, memory operands aimed at the data area or its edges, the last 64 bytes below 2^47 or
  non-canonical (through every base register, rsp and rbp among them, `fs:` and `gs:`, rip-relative, 67),
  half the dividends as compilers leave them; cases in no form (random invalid bytes, and prefix runs of 13 to
  16 bytes before an instruction of a form or a VEX or EVEX start).
- The stand-in answers x86's `arch_prctl(ARCH_SET_FS or GS)` (`GuestTest::stand_in_call`), as musl sets its
  thread pointer that way. `match_linux_at` runs a program at chosen look-aheads; x86's hello world runs at
  none (`hello_matches_linux_at_none_x86_64`).

What the reference CPU does where Intel's manual leaves it open, found here (plan 5.1 says so):
- OF after a shift or rotate is a count of one's, for every count, except `rol` and `ror` of a register by
  an immediate above one, which keep OF (of memory they do not). AF is clear after the shifts and the logic
  instructions. After `mul` and `imul`, ZF and AF are clear and SF and PF follow the low half. `bt` and its kin
  change only CF; `div` and `idiv` change no flag. `shl` of 8 or 16 bits by more than the width leaves CF clear.
  Gathered from layer 1's failures and a sweep of every shift and rotate at every width, counts 0 to 31,
  register forms by cl and by an immediate (scratch `x3/run1/shifts.txt`, `shifts-imm.txt`, 1,120 and 640
  cases, analysed by `scripts/shifts.py`, `shifts2.py`).
- `cmps` reads [rdi] before [rsi]. A 67 prefix wraps a bit offset's address with the operand's, before the
  segment base. An access that wraps past 2^64 is a page fault at its first byte.
- Invalid encodings: iced stops early, so every invalid encoding now gets the CPU's length (plan 5.2 says
  so), with an opcode made invalid by its mandatory prefix counted with its ModRM, and VEX and EVEX counted by
  their fields; the last rule is not yet right for every VEX and EVEX encoding (item 1 of "what comes next").

Proof of run 1 (the Mac, Apple M5; the i7-8750H's load 0.8 to 1.2 at the start of the long runs):
- Layer 1, 542 forms (before bt and the SSE2 forms), 1,000 cases a form, seed 1, 6 at once
  (`x3/run1/l1-1000.log`, 06:41 to 07:05, Mac load 55.8 to 54.6): 536 passed; the six failing were fixed
  after it (a branch into the instruction's own bytes, which our side runs, now left out like a jump to
  itself; a full-address operand drawn at random, which reached the reference runner's own memory, now aimed;
  `push [m]` under 67 wrapping the stack address; `int 0x80`, now excluded as plan 7.4 says), each form then
  passing 1,000 cases alone.
- Layer 1 after those fixes, every form (bt and the SSE2 forms with them; 654 forms in the seed 7 run of the
  same list, the passing run prints no count), 1,000 cases a form, seed 1, 6 at once, at e39a581
  (`x3/run1/l1-1000b.log`, 07:07 to 07:46, 2,312 s, Mac load 69.4 to 34.6, x86 load 7.1 at the start from
  another track's builds): passed, zero mismatches.
- Bit tests at 512 cases a form and the division forms at 256, alone: passed. The x86 crate's table tests: 4
  passed.
- Hello world at -O0, -O1, -O2, -O3 and -Os (three distinct blobs), at none in wasmtime, against the i7-8750H
  (`x3/run1/hello.log`): matches.
- The browser crate builds for x86 (feature `x86_64`; the host answer is now worked out inside the generic
  `translate_for`, as x86's is `()`). Sizes, release, `wasm-tools strip`, Apple gzip `-9 -n` and brotli `-q
  11` (07:11, Mac load 64.4): x86 1,135,906 bytes raw, 298,876 gzip, 229,169 brotli; ARM64 2,345,937,
  447,975 and 327,252. The x86 build holds iced's decoder (plan section 2 measured it alone at 226,971 raw,
  46,880 brotli) and fewer meanings than ARM64's so far; what the decoder adds to a complete x86 build is
  measured again when its meanings are complete. Clippy with `-D warnings` on both builds: clean.
- Suite of what this track owns at 6ac366b (`WC_REFERENCE_X86_64=tom@192.168.2.240`), in two parts: `cargo
  test` of the interface, ARM64, translator, test and x86 crates (scratch `x3/run1/suite.log`, from 07:47,
  Mac load about 35 to 45) ran every target up to and including layer 1 and then stopped writing with no
  result and no process left (the detached shell it ran in was gone; not seen why); the targets after layer 1
  and the x86 crate's were then run alone (08:58 to 09:11, Mac load 16.9 to 44.9). All passed except the
  known failure `bytes_in_no_form_stop_as_on_the_cpu_x86_64` (item 1 of "what comes next"): layer 1 8 of 9,
  layer 2 1, layer 3 3, programs 16 (2 ignored), the x86 reference 6, smoke 6, the test guest 16, validity 7,
  binaries 2, changes 3, checks 2, browser 1, decoding 5, lookaheads 1, no_std 1, the x86 tables 4, the
  translator's 4 and ARM64's 3.
- Clippy with `-D warnings` on the five crates (all targets) and the browser crate for both guests; `cargo
  fmt --check`: clean.

## X2: what was left for later (carried into X3 above)

1. The sixth independent check of X2.
2. Plan 7.3's recorded reference results and layer 1 batching, proposed for X3 (see "Departures"); build them
   at X2 only if the orchestrator rules so. The plan's text now says they are not built.
3. For X3 (moved to X5, see "Departures", X3): `presented`, `REFERENCE_HAS` and layer 1's rule for an `Err`
   case (plan 7.2 "As built"). For X4:
   the shared layer 2 model's operations are ARM64's (ccmp's nzcv immediate, ranges ARM64 encodes in one
   instruction), so x86's `assemble` expands them; the X2 check noted it.
4. For X3: the x86 runner reports the signal frame's RF (bit 16) in rflags at every fault stop (seen on the
   i7-8750H: `rep stosb` into a read-only page, rflags 0x10202; trap stops give 0x202). Plan section 2 says RF
   and IF are masked out of the comparison; x86's `reconcile` must do it, or every faulting case differs.

## X2, run 9 (2026-11-18)

The fifth independent check of X2 found one blocking problem and five smaller ones. What was done:
- Blocking, the wasmtime runner put an address's slot entry at `slots_at + (address - table_base)` bytes,
  right only when `ALIGN` is 4; for x86 (`ALIGN` 1) the entries of a blob with more than one slot would
  overlap and be read from elsewhere. The layout is now one function of the translator's contract,
  `contract::slot_entry::<G>(offset)` = `4 * (offset / ALIGN)`, which the wasmtime runner and the test
  guest's own runner (`ALIGN` 1) both use; `runner.js` already did the same sum. Proof that the test guest
  checks it against the translated lookup: with the function changed back to the old sum, 10 of its 16
  tests fail (`x2/run9`, by hand); with the new one all 16 pass (`x2/run9/test_guest.log`). ARM64's
  entries are where they were (`4 * (offset / 4)` is `offset` for every 4-aligned offset).
- Plan 7.2's browser contract now lists what `contract()` gives (`trapKinds` as names, `guestHooks`,
  `hookParams`, ...) and says why the call's block offsets, `codeChanges` and trap numbers are not in it.
- `long-runs.sh` builds each commit into `<WC_LONG_DIR>/target/<commit>`, as `same-cases.sh` and
  `same-output.sh` do since run 8. A long run restarted on this commit builds once into its new directory;
  old `<WC_LONG_DIR>/target` contents can be deleted when no run uses them.
- A failure of the reference runner itself (a panic of the serving process, such as on a request it cannot
  read) is now sent as the request's result with a new status, `RESULT_RUNNER_FAILED`, carrying the panic
  message, before the runner ends with 101; the harness fails the run with "the reference runner failed:
  <message>" and does not reconnect. A case child's death stays `RESULT_FAILED` (the runner goes on); a
  stream that ends with no result is still a lost connection. New tests
  `the_reference_reports_its_own_failure_arm64` (layer 1's file) and
  `the_x86_64_reference_reports_its_own_failure` send a blob request cut short; both pass, the x86 one on the
  i7-8750H (`x2/run9/reference.log`: 6 passed).
- The local reference's image and layer 3's gcc image are Alpine 3.24.2 by digest
  (`sha256:294b683c...`, a multi-platform index; 3.24.2 is what `alpine:latest` was here); the gcc image is
  named `wc-x86-gcc-<guest>-<first 12 hex of the SHA-256 of its recipe>`, so the first layer 3 run builds it
  once. `apk add gcc musl-dev` still takes the 3.24 branch's current packages when the image is built; the
  image is then kept.
- The x86 runner's RF in rflags at fault stops: already item 4 of "what comes next", for X3's `reconcile`.
- From this run's read of `src/runner.rs` whole (named not read in run 8): the block area before the
  permission table is a fixed 4096 bytes in both runners; each now checks the guest's thread state fits
  (a `const` assertion in wasmtime's, a thrown error in `runner.js`). `runner::thread_state_size` duplicated
  the contract's and is now the contract's. Left as they are: the stand-in's error numbers (9, 14, 22, 38:
  the same on 64-bit ARM64 and x86-64 Linux), fds 1 and 2, the permission table's bytes. Still not read line
  by line: `src/layer2.rs`, `src/binaries.rs`, the rest of `src/programs.rs` and `src/changes.rs` (a grep of
  them for fixed sizes found only 64-bit pointer steps and Linux's signal numbers; layer 2's shared model is
  ARM64's, item 3 of "what comes next").

Proof of run 9 (the Mac, Apple M5):
- origin/phase-1-translator: nothing new at the start (04:12); at the end (05:21) two commits, merged (layer
  3's rule that a faulting program's output may differ as a blob and as an ordinary executable).
- The x86 reference on the i7-8750H (`x2/run9/reference.log`, 04:15, x86 load 1.9): 6 passed, the new
  failure test among them.
- Same cases (`x2/run9/same-cases.log`, root scratch `x2/run9/same`, 04:20 to 04:22, Mac load 29.5 to 91.9):
  against c1153e5, seed 31337, layer 1 (1215 forms), layer 2 (64), no-form (64) and hand-made (105) the same.
- Clippy with `-D warnings` on the four crates (all targets), the browser crate (wasm32, `arm64`) and the runner
  for both CPUs; `cargo fmt --check`: clean.
- Suite at 7157b52 (`WC_REFERENCE_X86_64=tom@192.168.2.240`, scratch `x2/run9/suite.log`): 82 passed (run 8's 80
  and the two runner failure tests), 0 failed, 11 ignored, 30 minutes 14 seconds, Mac load 58.3 at start and
  34.7 at end, x86 load 3.6 at start (another track's builds) (run 8: 26 minutes 42 seconds at 30.0 to 86.0). A
  first run of it was stopped by this session's 30-minute limit on background commands after 45 tests, none
  failing (`x2/run9/suite-cut.log`).
- After the merge: layer 3 (`x2/run9/layer3-merged.log`, 05:21 to 05:28, Mac load 29.2 to 23.2): 3 passed,
  default case counts; its first run built the pinned gcc image.

## X2, run 8 (2026-11-18)

The fourth independent check of X2 found two blocking problems and three smaller ones. What was done:
- Blocking, Chrome's layer 1 held two ARM64 facts and the plan said what the page did not do: the page never
  wrote the filler (right only because ARM64's is 0), and its lookup covered `MAX_LENGTH / ALIGN` slots (1 for
  ARM64, 15 for x86), so an x86 case falling through to CODE + L would have run the padding's `int3` where
  wasmtime and the CPU land at CODE + L. Now each Chrome case carries its length; the page translates the
  instruction's own bytes (as `Machine::load(code, 0..len)` does), covers one slot (as `Machine::new(.., CODE,
  1, ..)`), and fills the code area once from the new `fill` setting when the filler is not 0 (as wasmtime
  does per case). `code_area::<G>()` in `layer1.rs` gives both sides the range. Shown for ARM64 only (its
  cases are unchanged: one slot, 4 bytes, filler 0); x86's Chrome layer 1 at X3 is what runs it for x86.
- Blocking, layer 3 judged a blob's signal ending by `docker run --platform` of the ordinary executable on
  the Mac, bypassing the transport (for x86, Docker's emulation). The runner has a new request,
  `RECORD_PROGRAM`: the executable's bytes, run from a memory file by `fexecve` in a child of its own, with
  the blob's name as `argv[0]`, no environment, empty input, no signal blocked, SIGPIPE at its default (Rust
  ignores it in the runner and exec keeps that) and the blob's 20-second `alarm`, which carries over exec;
  its result is a blob's with no stop. `layer3::ordinary` sends it through `with_reference`
  (`programs::native_program`). The blob's run and the program's share `run_child` (input, pipes, fork,
  wait, time) and `standard_files` in the runner. New tests: `ordinary_executables_run_on_the_reference_arm64`
  and `the_x86_64_reference_runs_programs` (hello world's output and status 3, a write through a null pointer
  139, a loop ended by the limit 142), through `programs::check_ordinary_programs`.
- Minor, the browser glue passed ARM64's `()` as the offer: the interface's `Offer` is now `Default` (its
  default is what the CPU claims by default), the glue translates through a generic `translate_for::<C>` with
  `C::Offer::default()`, and `GuestTest::DEFAULT_OFFER` is gone (every use is `G::Offer::default()`).
- Minor, `same-cases.sh` and `same-output.sh` built every earlier commit into one target directory, where
  cargo could take one commit's build for another's (copies from `git archive` carry the commit's times). Each
  commit now builds into `<root>/target/<commit>`. Shown: in one new root, `same-cases.sh d27cd70 271828 4242`
  then `same-cases.sh 995e949 271828` both pass (the check's same pair failed to compile before).
- Three comments in generic code said Docker where they meant the shell's 128 plus the signal; reworded.

This run's read for CPU facts outside the per-CPU part: `browser/runner.js`, `src/browser.rs`, `src/layer1.rs`,
`src/layer3.rs`, `tests/layer3.rs` and `src/reference.rs` whole, and a grep of `src/` and `tests/` for Docker.
Left as they are: Chrome's stop kinds as 128 plus Linux's signal (the same on every guest), the stand-in's
16-byte `iovec` and `IOV_MAX` (64-bit Linux's on every guest), the state memory's 4-byte words, `binaries.rs`
taking files from images with Docker (not a reference judgement), and layer 3's gcc in Docker for the guest's
platform (compiling only, as plan 7.4 says). Not read line by line: `src/layer2.rs`, `src/binaries.rs`,
`src/programs.rs` beyond what this run changed, `src/changes.rs` beyond run 7's part, `src/runner.rs`. So this
is not a proof that none are left.

Proof of run 8 (the Mac, Apple M5):
- origin/phase-1-translator: nothing new at the start (2026-11-18 02:43).
- The x86 reference on the i7-8750H (`x2/run8/reference.log`, 02:52 to 02:53, Mac load 20.5 to 37.2, x86 load
  2.9 just before, another track's builds): 5 passed, 43 hand-made cases, the new program test among them.
- After the changes (`x2/run8/affected.log`, 02:52 to 03:04, Mac load 19.9 to 44.6): browser 1 passed; layer 1
  5 passed (1215 forms, 64 cases each, seed 1, 0 failing; 64 no-form cases; 105 hand-made; Chrome 1215 cases, 0
  failing); smoke 6 passed; `ordinary_executables_run_on_the_reference_arm64` passed (20.6 s, the loop's limit).
  The page's fill was changed to "when the filler is not 0" while this ran; the suite below runs the final
  page.
- Same cases (`x2/run8/same-cases.log`, root scratch `x2/run8/same`, 02:52 to 02:55, Mac load 21.1 to 39.1):
  against d27cd70 seeds 271828 and 4242, layer 1 1215 forms and layer 2 64 programs the same; against 995e949
  seed 271828, layer 1, layer 2, no-form (64) and hand-made (105) the same.
- Clippy with `-D warnings` on the four crates (all targets), the browser crate (wasm32, `arm64`) and the runner
  for both CPUs; `cargo fmt --check`: clean.
- Suite at 8e74a45 (`WC_REFERENCE_X86_64=tom@192.168.2.240`, scratch `x2/run8/suite.log`): 80 passed (run 7's 78
  and the two program tests), 0 failed, 11 ignored, 26 minutes 42 seconds, Mac load 30.0 at start and 86.0 at
  end, x86 load 3.7 at start (run 7: 29 minutes 35 seconds at 73.3 to 64.2).
- origin/phase-1-translator: fetched again at the end (03:32), nothing new.

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
illegal instruction (the brief says so for x86 too). Also read: `binaries.rs`'s sources and start finder,
`programs.rs`'s placement and compiler arguments (nothing of ARM64's: the target comes from the guest, the
alpine paths are the same on both). Not read line by line: `src/layer2.rs` (its model is ARM64's by a recorded
departure), the rest of `src/binaries.rs` and `src/programs.rs`, `src/browser.rs` and `browser/runner.js`
beyond the grep. So this is not a proof that none is left; X3's x86 `GuestTest` is what
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
- Same cases: `same-cases.sh d27cd70 1 2 3 9321` (`WC_OUTPUT_ROOT` scratch `x2/same`, log
  `x2/run7/same-cases.log`, 01:31 to 01:33, Mac load 73.3 to 87.1): layer 1 1215 forms and layer 2 64 programs
  the same for each seed. The no-form lists are not compared (d27cd70 does not write them); nothing this run
  changed draws them.
- Suite at 78bcabc (`WC_REFERENCE_X86_64=tom@192.168.2.240`, scratch `x2/run7/suite.log`): 78 passed, 0 failed,
  11 ignored (as run 6), 29 minutes 35 seconds, Mac load 73.3 at start and 64.2 at end, with `same-cases.sh`
  beside it for its first two minutes (run 6: 23 minutes 34 seconds at 54.1 to 34.3). The longest parts are
  machine-bound and the load was higher; nothing this run changed runs more cases or slower code.
- Clippy with `-D warnings` on the test crate (all targets), `cargo fmt --check`: clean.
- origin/phase-1-translator at the end (02:01): two commits (to d98f6bf: its long run gives zig a cache of its
  own). Merged, leaving out its one line in `long-runs.sh` (`export ZIG_GLOBAL_CACHE_DIR=$dir/zig-cache`): in
  this tree `programs.rs` sets zig's cache for every zig command to the tree's own `target/zig-cache`, which
  overrides the export, so the line would be a second mechanism with no effect. Only its status file changed
  otherwise.

## X2, run 6 (2026-11-18)

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

- Layer 1's verdicts on the i7-8750H (X5): `<scratch>/x5/run4/laptop-bg.sh <name> '<command>'` copies the
  working tree there and starts the command detached (4 build jobs, under `choom`, off cores 2 and 8), log in
  `~/wcontainers-x86/logs/<name>.log` there; e.g. `WC_LAYER1_FORMS=<codes> WC_LAYER1_CASES=1000
  WC_LAYER1_SEED=<n> WC_LAYER1_JOBS=4 cargo test --release -p wcontainers-translator-test --test layer1
  every_form_matches_the_cpu_x86_64 -- --nocapture`. `WC_PROBE_ANSWER=<answer>` makes the harness translate
  for a host whose probe gives that answer (one of `PROBE_ANSWERS`), so the not-a-number fix-ups that hosts
  answering 0 need run on any host. The script is copied to `x5/run5/laptop-bg.sh`; it replaces the copy of
  the tree there, so never start it while another of its jobs runs. The Chrome sample there needs
  `PATH=$HOME/wcontainers-x86/node/bin:$PATH WC_PLAYWRIGHT_CORE=$HOME/wcontainers-x86/playwright-core
  WC_CHROME_EXECUTABLE=$HOME/wcontainers-x86/chrome/opt/google/chrome/chrome` in the command (without them it
  fails at "run node").
- Layer 1's unoffered forms (X5): `WC_LAYER1_UNOFFERED_CASES=100 WC_LAYER1_SEED=<n> WC_LAYER1_JOBS=4 cargo
  test --release -p wcontainers-translator-test --test layer1 unoffered_forms_are_illegal_x86_64 --
  --nocapture` on the i7-8750H (the rented machine has most of these features); `WC_LAYER1_FORMS` limits it.
  `every_unoffered_code_is_drawn_x86_64` lists the codes left out and why.
- The not-a-number probe on a host: copy scratch `x5/run5/scratch_probe.rs` into the test crate's `tests/`,
  `WC_PROBE_OUT=<file> cargo test --release -p wcontainers-translator-test --test scratch_probe --
  --nocapture` (wasmtime's answer, and the module written to the file), then `node x5/run5/probe.mjs <file>`
  for Chrome's (`WC_PLAYWRIGHT_CORE` from the Nix shell) and `wasmtime run --invoke probe <file>`.
- The passed-register lists: `x5/run5/passed.sh <rounds>`, from an x86-run copy on the rented x86-64 machine
  (it builds a copy of `time-blob` for each pair of lists).

- Docker on the Mac without Docker Desktop (X4 run 6), for the ARM64 tests whose reference runs in Docker:
  `export LIMA_HOME=<scratch>/x86/lm` and Lima from `nix build nixpkgs#lima`; `limactl start x` (made with
  `--vm-type=vz --cpus=4 --memory=4 --disk=30 template:docker`; its rootless setup failed for want of the
  archive over http, so inside it `docker` and `containerd` were unmasked and started as root and the socket
  opened to its user); then forward the socket, `ssh -F $LIMA_HOME/x/ssh.config -o ControlMaster=no -o
  ControlPath=none -N -L $LIMA_HOME/d.sock:/var/run/docker.sock lima-x &`, and run the tests with
  `DOCKER_HOST=unix://$LIMA_HOME/d.sock`. `limactl stop x` when done; `limactl delete x` frees its disk.

- Two builds as alternating pairs on the x86 machine (X4 run 7): `~/wcontainers-x86/pairs.sh <rounds>
  <workloads>` times `variants/old` and `variants/new` in wasmtime, one run each a round, the order flipped
  every round. Copy each commit with `git archive` to its own tree there and touch its sources before building
  it into a shared target folder (the archives' file times are the commit's, and cargo otherwise keeps the
  first build).
- Comparing builds on the x86 machine (X4 run 4): put each build's `time-blob` in `~/wcontainers-x86/variants/`
  (built from a `git archive` of its commit in `~/wcontainers-x86/tree`, `-j 4`, under `choom -n 600`), then
  `~/wcontainers-x86/counts.sh "<workloads>" <variant>...` counts user-mode instructions of one wasmtime run
  each with the kernel-versioned `perf` in /usr/lib/linux-tools (the unversioned `perf` does not run there),
  and `~/wcontainers-x86/variants.sh <rounds> <workloads>` alternates times (scratch `x4/run4/` holds both and
  `patch-variants.py`, which makes the timing-only builds without the flags hand-over or the passed-in-only
  registers).

- x86 benchmarks on the x86 machine (X4 run 3), natively, in wasmtime and in Chrome in alternating rounds,
  one command there once the blobs and the tree are in place: `~/wcontainers-x86/time-rounds.sh <rounds>
  [workloads]` (scratch `x4/run3/time-rounds.sh`; it runs `time-blob rounds x86_64 <rounds> <blobs>` off
  cores 2 and 8 with the machine's Node, playwright-core and Chrome; output and load per run, then each
  one's best and times native). Setup: blobs by `time-blob dump x86_64 bench -O2 <file> SCALE=1
  WORKLOAD=<k>` here into `~/wcontainers-x86/blobs/bench-<k>.blob`; `git archive HEAD | ssh ... 'tar x -C
  ~/wcontainers-x86/tree'`; there `. ~/.cargo/env && cargo build --release -p wcontainers-translator-test
  --bin time-blob --target-dir ~/wcontainers-x86/target`. `time-blob native|wasmtime|chrome x86_64 <blob>
  <runs>` times one way alone.
- x86 (X4): layer 2 against the i7-8750H and against none, `WC_REFERENCE_X86_64=tom@192.168.2.240
  WC_LAYER2_CASES=10000 WC_LAYER2_NATIVE=10000 WC_LAYER2_JOBS=3 WC_LAYER2_SEED=<n> nix develop -c cargo test
  -p wcontainers-translator-test --test layer2 full_matches_none_and_the_cpu_x86_64 -- --nocapture`; the
  checks, `--test checks x86_64`; flags across calls, `--test programs flags_across_calls_match_linux_x86_64`;
  timings, `WC_BENCH_VARIANTS=1 WC_BENCH_CHROME=0 ... --release --test benchmarks measure_benchmarks_x86_64 --
  --ignored --nocapture` (native on the reference, wasmtime here); wasmtime on the x86 machine: `cargo run
  --release --bin time-blob -- dump x86_64 bench -O2 <file> SCALE=1 WORKLOAD=<k>` here for each workload, copy
  the files and a `git archive` of the tree to `~/wcontainers-x86/{blobs,tree}` there, build `time-blob`
  there (`cargo build --release -p wcontainers-translator-test --bin time-blob --target-dir
  ~/wcontainers-x86/target`), and run scratch `x4/run2/time-wasmtime.sh 3` there. Scratch tools of
  run 2 (one layer 2 case disassembled and run, a blob dump, the missing meanings of the shared programs) are
  in scratch `x4/run2/tools/`: copy one into `tests/` to run it.
- x86 (X3): layer 1 against the i7-8750H, `WC_REFERENCE_X86_64=tom@192.168.2.240 WC_LAYER1_CASES=1000
  WC_LAYER1_JOBS=6 nix develop -c cargo test -p wcontainers-translator-test --test layer1
  every_form_matches_the_cpu_x86_64` (`WC_LAYER1_FORMS`, `WC_LAYER1_SEED` as for ARM64); cases in no form,
  `bytes_in_no_form_stop_as_on_the_cpu_x86_64`; hello world, `--test programs hello_matches_linux_at_none_x86_64`;
  the crate's own tests, `nix develop -c cargo test -p wcontainers-x86`. In Chrome: `--test layer1
  sample_matches_the_cpu_in_chrome_x86_64` and `--test browser hello_matches_linux_in_chrome_at_none_x86_64`.
  The length sweep (about 25 minutes, 6 connections): `WC_REFERENCE_X86_64=tom@192.168.2.240 nix develop -c
  cargo run -p wcontainers-translator-test --bin x86-lengths -- <output> [connections]`, then copy the output
  over `crates/wcontainers-x86/tests/lengths.txt`. The long division: `--test benchmarks
  measure_long_division_x86_64 -- --ignored --nocapture` (`WC_BENCH_RUNS`, default 3), only while the x86
  machine's load is below 2. The scratch tools of run 1 (shift
  sweeps, a hello debugger, a module dump) are in scratch `x3/run1/scratch_*.rs`: copy one into the test
  crate's `tests/` to run it.

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
- Added in X3 run 2: `~/wcontainers-x86/proof/len.sh` (a length probe over the old `stepper4`, for quick
  looks; the table's sweep uses the harness's runner instead).
- Added in X4 run 2: `~/wcontainers-x86/tree` (a `git archive` of this branch; its build folder
  `~/wcontainers-x86/target` was deleted at the end of the run),
  `~/wcontainers-x86/blobs` (the eight benchmark blobs), `time-wasmtime.sh` and its `time-wasmtime.out`,
  `proof10/flags_across`. Installed: nothing.
- Added in X4 run 3, all under `~/wcontainers-x86`: `node` (Node.js 24.22.3), `playwright-core`, `chrome`
  (Chrome 158 unpacked from `google-chrome-stable_current_amd64.deb` with `dpkg-deb -x`; the system's Chrome
  115 is untouched), `time-rounds.sh` (copy in scratch `x4/run3/`) and its logs `rounds-*.log`. Installed
  system-wide: nothing.
- Added in X4 run 7, all under `~/wcontainers-x86`: `tree-old` (babfb595) beside `tree` (49a11539), `variants/old`
  and `variants/new` (their `time-blob`), `build-variants.sh`, `pairs.sh` and its `pairs-run7.log`,
  `counts-run7.log`, `rounds-run7.log`, `wasm-mode.py` (a scratch mode that writes a blob's modules, patched
  into both trees' `time-blob` only there) and `mx-*.wasm`; `blobs/` now holds the general-register blobs
  and `blobs-sse/` the earlier ones. The build folders `target` and `target-w` were deleted at the end.
- Added in X2: `~/wcontainers-x86/reference/<hash>`, the reference runner the harness uploads (one file per
  build of it).
- Not always idle: another track's atomic-cost measurement (`~/ordering-cost`, wasmtime) runs there at
  times. The independent check saw load about 1.04. Run 2 saw 0.07. Record `/proc/loadavg` with every
  timing there.

## Cases actually run

- X5 run 4: every default form on the i7-8750H at 1,000 a form, seed 9105 (1,025,000 cases); `dpps` and
  `dppd` at 3,000 a form at seeds 9105 to 9108 twice (before and after the last fix); the 76 float forms at
  1,000 a form, seed 9109, with the probe answer forced to 0; layer 1 on the i7-8750H at 1,000 a form: 6 forms at seed 9101, 3 at seed 9102 (twice, before and
  after the `maskmovdqu` fix), 21 at 300 a form at seed 9103 (twice), 30 at seed 9104; quick runs of the same
  forms on the rented machine.

- X5 run 1: layer 2 x86 at seed 8675309, 300 programs (100 in Chrome, 24 at once), and at seed 1919, 2,000
  (24 in Chrome), every one also on the CPU; the ARM64 layer 2, changes and look-ahead tests once at their
  defaults (seed 1) on the rented ARM64 machine; x86's checks three times (once without the proof fix).

- X4 run 8: the track's suite once on the rented x86-64 machine at its defaults (seed 1); the x86 layer 1
  tests and the x86 reference's tests once at their defaults against the i7-8750H; the runtime's and the
  kernel test crate's tests once on the rented ARM64 machine (see "X4, run 8").
- X4 run 7: layer 2 x86 at seed 9393, 1,000 programs, and at seed 5353, 10,000, every one also on the CPU;
  every x86 test of the test crate once at its defaults; the ARM64 tests of the test crate once on the rented
  machine and the suite once on the Mac, at their defaults (see "X4, run 7").
- X4 run 6: layer 1 x86 at seed 6006, 500 cases a form, the 96 conditional forms; layer 2 x86 at seed 7171,
  1,000 programs, and at seed 8282, 10,000, every one also on the CPU; layer 1 ARM64 at the defaults (64 a
  form, seed 1) twice on the Mac; the suite once at its defaults (see "X4, run 6").
- X4 run 5: none of its own (the ARM64 tests of the test crate once on the rented ARM64 machine at their
  defaults, seed 1; the suite once on the Mac at its defaults); `lengths_are_the_reference_cpus`, x86's
  programs and checks against the i7-8750H, and `measure_x86_64` once each after the literal changes.
- X4 run 4: layer 1 x86 at seed 4405, 1,000 cases a form, the 123 shift, rotate, bit test and carry forms
  (3 of them again alone at the same seed after an SSH outage); layer 2 x86 at seed 4, 500 programs, at seed
  5, 1,000, and at seed 31337, 10,000 (3 of them again alone after the outage), every one also on the CPU;
  the instruction counts of the eight workloads for three builds, and 5 rounds of three builds in wasmtime on
  the x86 machine; the suite once at its defaults (see "X4, run 4").
- X4 run 3: layer 2 x86 at seed 3, 1,000 programs, every one also on the CPU; at seed 7777, 10,000 (see "X4,
  run 3"); the tests of layer 2 (both guests at their defaults, seed 1), changes, checks, programs,
  look-aheads and the test guest once after the frame change; the suite at the end; the benchmark set 5
  rounds natively, in wasmtime and in Chrome on the x86 machine twice (before and after the frame change) and
  hash once while trying the tool; fib, sort and search 5 rounds old against new.

- X4 run 2: layer 2 x86 at seed 1, 16 programs (four times while fixing), 64 (twice), 1,000 (twice, every
  program also on the CPU); at seed 2026, 10,000 (see "X4, run 2"); layer 1 at seed 4404, 1,000 cases a form,
  for `Pushfq` and `Pushfw` (four times while fixing the reconcile) and with `Leaveq` and `Leavew` once; the
  checks and the flags program once each after their last change; hand programs of the scratch tool.

- X3 run 3: twelve forms (the bit tests, `pop`, three reserved NOPs, `jrcxz`, `jecxz`) at 1,200 cases a form,
  seed 5150 (twice: the first ended on the runner's start check, see "X3, run 3"); every x86 form (705) at
  1,200 cases a form, seed 5150; 10,000 cases in no form at seed 5150; the suite.
- X3 run 2: the length sweep twice (294 and 300 lines of 256 probes, each probe 4 or 5 runs on the
  reference, about 380,000 runs); the kernel-only forms at 50 cases (twice, while fixing) and at 1,000
  (seed 1); cases in no form at 64 for seeds 1 to 6 (twice), and 10,000 at seed 1; layer 1 in Chrome at 1 a
  form (seed 1); hello world in Chrome once; the long division measurement once at best of 1 and once at
  best of 5; layer 1 at 1,000 cases a form, seed 2; the suite.

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
- X3 run 1: x86 layer 1 at seed 1: 8 cases a form (542 forms), 16, 64 (542 forms), 1,000 (542 forms), 1,000
  again (every form, about 654,000 cases); at seed 7: 32 cases (654 forms); single forms re-run at 256 to 1,000 while fixing
  (rotates, divisions, bit tests, the six of the first 1,000 run); cases in no form at 500, 1,000 and 2,000
  for seed 1 and 2,000 for seeds 2 to 6, each stopping at its first failure; the scratch sweeps of shifts and
  rotates (1,120 and 640 cases, seed 12345); hello world once per fix and in the suite; the x86 reference's
  tests once and in the suite; x86's 6 hand-made cases twice and in the suite; the suite's layer 1 at its
  defaults (seed 1, 64 a form) for both guests.
- X2 run 9: the x86 reference's tests (6, the runner failure test among them) once alone and in the suite;
  `same-cases.sh c1153e5 31337` (layer 1 1215 forms at 64 a form, layer 2 64 programs, 64 no-form cases, 105
  hand-made, both sides); the test guest's 16 tests twice (with the old slot sum, 10 failing, and with the
  new); the suite once at the defaults (seed 1), and a first suite run cut after 45 tests; layer 3 after the
  merge at its default 4 cases.
- X2 run 7: layer 1 at the defaults (seed 1) twice (once alone, once in the suite); changes and smoke at the
  defaults; `same-cases.sh d27cd70` for seeds 1, 2, 3 and 9321 at 64 a form and 64 programs, both sides; the
  x86 reference's 4 tests twice (the first failing all four at the content check) and in the suite.
- X2 run 8: layer 1 at the defaults (seed 1) twice (once alone, once in the suite); `same-cases.sh` for seeds
  271828 and 4242 against d27cd70 and 271828 against 995e949 at 64 a form and 64 programs; the x86
  reference's 43 cases, 2 blobs, 3 programs, the refusal and reconnection tests, once alone and in the suite;
  ARM64's 3 programs, once alone and in the suite.
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
- X5 run 2, layer 1 against the i7-8750H (`WC_LAYER1_JOBS` 3, translated side on the Mac): the 67 integer
  forms at 1,000 a form, seed 1; the 20 string forms at 1,000, seed 4; `popfw`/`popfq` at 1,000, seed 5;
  135 SSE integer, move and blend forms at 100 (seed 31) and 1,000 (seed 4242); `crc32` 5 forms at 1,000
  (seed 77); `xlat` at 1,000 (seed 78); `pextrw` 4 forms at 1,000 (seed 79); 145 forms at 200 (seed 8080)
  after the literal changes; every x86 layer 1 test at 100 a form (seed 2611: 919 forms, 100 cases in no
  form, 88 hand-made cases). All 0 failing but the two `pextrw` forms to memory in the seed 2611 run
  (fixed, then seed 79) and the Chrome sample, which cannot run on the Mac (a page resource 404: it runs on
  the rented x86-64 machine).
- X5 runs 4 and 5, layer 1 against the i7-8750H at 1,000 a form unless said: every default form at seeds
  9105 and 9204 (1,025 and 1,063 forms), the run's new forms at seeds 9101, 9102, 9104, 9201 to 9203, 9330 and
  9334, `dpps`/`dppd` at 3,000 at seeds 9105 to 9108, the 76 float forms with the probe forced to 0 at seed 9109;
  the unoffered forms at 100 a form (2,993 forms, seed 9331); 10,000 no-form cases (seed 9332); the Chrome
  sample at 3 a form (seed 9333). Layer 3 Csmith for x86 at 1,000 programs (seed 5502, 879 compared) and 300
  (seed 9311). Layer 2 for x86 at 3,000 (seed 9310) with the shorter passed-register lists only.
- X5 run 6: layer 1 against the i7-8750H, every default form at 1,000 a form (seed 9360), the unoffered forms at
  100 a form (seed 9361), 100,000 cases in no form (seed 9362), the Chrome sample at 3 a form (seed 9363), the six
  `movbe` forms at 1,000 (seed 9364); ARM64 layer 1 on the Mac at 64 a form (seed 1). Layer 2 for x86 at 3,000
  (seed 9310) and 10,000, all on the CPU (seed 9370); for ARM64 at 3,000 (seed 9371). Layer 3 Csmith for x86 at
  1,000 (seed 9372).

## Measurements

All in the run sections above (X0 runs 9 and 10 timed nothing; X1's benchmark times are in run 2, the
float hook's in runs 2, 3 and 5, translate time in runs 4, 5 and 6).

## Licences of what this track uses

- Added at X5 run 3: `rustc_apfloat` 0.2.3 (Apache-2.0 WITH LLVM-exception), already a dependency of the
  ARM64 crate, now of `wcontainers-x86` (the exact path of SIMD floating point).
- Added at X1, all already in the workspace: `blake3` (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH
  LLVM-exception) as a dev-dependency of the test crate; `wasm-encoder` (Apache-2.0 WITH LLVM-exception OR
  Apache-2.0 OR MIT) as a dependency of the test crate and of the interface crate; `zerocopy` 0.8
  (BSD-2-Clause OR Apache-2.0 OR MIT) as a dependency of the interface crate and of the browser crate;
  `wasmparser` 0.263 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT) as a dev-dependency of the test
  crate (the test guest's validity test; it was the translator's until the test guest moved in run 3).

- iced-x86 1.22.0: MIT. Since X2 a dependency of the reference runner when built for x86-64 (decoder and
  `instr_info`, to tell rep string instructions and gathers). Since X3 a dependency of `wcontainers-x86`
  (`no_std` and `decoder` only, behind `translate`; with `std` and `instr_info` as its build script's
  dependency and its tests'), and of the test crate (`std`, `decoder`, `encoder`, `op_code_info`, `instr_info`,
  `intel`: forms, cases, disassembly; since X4 also `code_asm` and the `block_encoder` it needs, for layer 2's
  emitter, the same crate and licence). It brings `lazy_static` (MIT/Apache-2.0) and `spin` (MIT) without
  std.
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
- raw-cpuid 11.6.0: MIT (its Cargo.toml), with bitflags 2 (MIT OR Apache-2.0, already in the workspace).
  Since X5 run 2 a dev-dependency of `wcontainers-x86` only: the CPUID test reads our table back through it.
- Test tools on the x86 machine since X4 run 3, none a dependency of anything: Node.js 24.22.3 (MIT, the
  official linux-x64 tarball, checked against its SHA-256), playwright-core 1.63.0 (Apache-2.0, a copy of
  the Nix shell's), Google Chrome 158.0.8127.36 (Google's terms; unpacked, not installed).
- Lima 2.3.0 (Apache-2.0, from nixpkgs) with an Ubuntu 26.04 cloud image and Docker Engine 29.10.0
  (Apache-2.0) inside it, on the Mac since X4 run 6: a test tool only, a dependency of nothing.
- sandsifter: BSD-3 (Battelle); read only, nothing copied.
- RISU: EPL-1.0; read only, nothing copied.
- gex: AGPL-3.0; not read, nothing used.
- Looked at in run 9 and not used: softfloat-sys 0.1.4 (BSD-3-Clause, bundles Berkeley SoftFloat 3,
  BSD-3), softfloat-rs 0.1.0 (BSD-3-Clause), softfloat 1.0.0 (MIT OR Apache-2.0), machina-softfloat 0.1.2
  (MIT), era2m_softfloat 0.2.1 (BSD-3-Clause-Clear). Nothing copied.

## Departures from the plan

X5 run 4:
- `wait` (`fwait`) is left to X7 with x87: its whole meaning is the check for a pending unmasked x87
  exception, which only x87 instructions raise and which needs X7's x87 state and fault path.
- `enter` checks the write at the final stack pointer after its pushes, not before: the reference does so.
- Layer 1 reconciles `lsl` of the per-CPU segment to processor 0 (plan 5.10 asked to leave it out of the
  comparison; reconciling keeps every other selector of `lsl` compared).

X5 run 3:
- An unmasked SIMD floating point exception reports MXCSR's raised unmasked flags as its detail, not the
  `si_code` (the interface's doc says so now). The CPU's part of #XM ends at MXCSR; Linux works the `si_code`
  out from MXCSR (`fpu__exception_code`), and so will the kernel's x86 personality at X10. The x86 crate
  cannot import Linux's `FPE_*` for the browser build (linux-raw-sys has no wasm32 module, and depending on
  the kernel for its generated copy flipped the literal check's owner of an unrelated ARM64 constant).
  Layer 1 maps the reference's SIGFPE to MXCSR's raised unmasked flags, which it compares in full anyway.
- `stmxcsr` with the flags not kept counts "MXCSR's exception flags read while not kept" with `warn` in the
  meaning, not through a `ReadStatus` call of the float hook: the same count with no hook call.
- The float hook takes the destination xmm register from the block (its number in the operation word, all
  xmm registers declared as the hook's registers), because a hook may have at most four parameters.
- `dpps` and `dppd` add in the CPU's order, seen on the reference with not-a-numbers in several lanes: each
  lane adds its neighbour (the neighbour first in `dpps`'s first step, its own first in `dppd`), then
  `dpps` adds the other pair's sum to its own; not the manual's one sum broadcast.
- The prefetches are written as hints with no access (never a fault), as on the CPU.

X4 (each also written into the plan's text where the plan is wrong):
- Run 6: the Mac's Docker verdicts (ARM64 layer 1 and the suite) came from Docker Engine in a Lima VM on the
  Mac's own CPU, because Docker Desktop does not start; the reference still runs on the M5's cores, in the
  same image, so the verdict is the Mac's. Its kernel is Ubuntu's 7.0.0, not Docker Desktop's.
- The literal check was adopted in run 2 by merging origin/phase-1-literals directly, as the brief's newest
  section and the task say, against `docs/literals.md`'s step 1 (merge main once main has it; main does not
  yet). The merge brought the other tracks' work that branch holds (kernel, runtime, wasmtime crates). One of
  them, `wcontainers-wasmtime` (the runtime track's), did not build on this branch: it matched
  `HookBody::CodeChanged` as a unit variant, which this track's interface gives the line size as a field.
  Runs 2 to 7 left it, wrongly: the brief of 2026-11-19 08:20 says to fix it here. Run 8 fixed it in that
  crate (its hook takes the line size from the variant instead of a bare 64), the one edit this track made
  outside what it owns.
- Layer 2's model registers for x86 changed (rbx kept, r12 to r15 scratch); no x86 program had been
  generated before, so nothing to keep.
- The translator's walk back for `jump_table` crosses a conditional branch's fall-through (plan 3.2 says
  "`None` at the start of a block that more than one edge enters", which this now matches).
- `pushf` and `leave` are built at X4 (X5's list): the dump and gcc's -O0 frames need them.
- Run 4: plan 5.1's nine-local `Shift` shape is built with three locals (result, CF, OF) when the count is
  known not to be zero and eight when it may be (the count and PF, AF, ZF, SF as they were besides): CF and OF
  are chosen at the shift between their new and earlier values, which saves two locals and the work at each
  read. Plan 5.1's `Bits` shape is not used for bt and its kin, clc, stc and cmc, nor for rotates: they change
  only CF (and OF), so they hand the six flags over each in a local (`Flags::Each`), which needs no shape and
  no flags-word function. `Bits` comes with bsf, bsr, popcnt, lzcnt and tzcnt at X5 (plan 5.1 says so now).
- Run 4: plan 7.4 says x86's layer 2 emitter adds string instructions with and without rep; it has them
  without rep only (lods and stos, both directions). Rep strings are X5's instructions (plan 9), so their
  layer 2 form comes with them at X5.
- Run 4: the `measure` test (programs.rs) measures at full from the entry besides none from every
  instruction, and runs for x86 too (`measure_x86_64`), so that output size and compile time at full are
  measured for each guest (plan 9's preface).
- `machine-code` takes the guest and the library as arguments (it read ARM64 programs only).
- Run 3: the benchmark set is timed in Chrome on the x86 machine with Chrome 158 unpacked from Google's
  package into `~/wcontainers-x86/chrome`, not the system's Chrome 115, which lacks 64-bit memories; the
  browser runner (`run.mjs`) launches the executable `WC_CHROME_EXECUTABLE` names when it is set (its one
  use: a machine whose installed Chrome is too old), else the installed Chrome as before. Not upgrading the
  system's Chrome keeps the owner's own browser and profile untouched.
- Run 3: a reference whose CPU is this machine's runs its runner directly when this machine is Linux (Docker
  only elsewhere, as on the Mac for ARM64); that is what an SSH reference already does on the far machine, and
  it lets `time-blob` time natively on the x86 machine itself.
- Run 3: layer 3 (Csmith) for x86 is left to X5, whose pass bar names it (X4's bar names layer 2 and the
  benchmarks only); x86 still lacks meanings Csmith's programs reach (bsr, bswap and the rest of X5's list).
- Run 3: the frame check of a move down reaches down for the moves down that follow in its block, and a
  frame known below the red zone is kept on a move down (dropped on a move up, as before). ARM64's output
  changes where a function moves its stack pointer down twice in one block (one check instead of two).

X3 (each also written into the plan's text where the plan is wrong):
- Some SSE2 instructions (the moves, logic, adds, compares, interleaves, shuffles, shifts by an immediate and
  masks on xmm registers, `simd.rs`) are built at X3, not X5: musl's start and string routines reach them, and
  the X3 pass bar asks for hello world. They are layer 1 forms like any other and pass at 1,000 cases a form.
- Flags as built (plan 5.1): no flags-word functions yet (`SHAPES` empty, `condition_direct` false), and the
  shifts and rotates, `bt` and its kin, `clc`, `stc` and `cmc` work their flags out at once into a packed
  word (the last seven pack the five flags they keep with the new CF, which works out every flag of the
  instruction before them): this costs run speed until X4. The lazy `Shift` and `Bits` shapes and direct
  conditions come with X4's flags liveness.
- `jrcxz` and `jecxz` are built at X3 (the manual puts them on the Jcc page). Every other default-claimed
  instruction without a meaning stays X5's "every default instruction", among them some inside X3's own
  categories: `shld` and `shrd` (shifts), push and pop of fs and gs (push and pop), mov to and from a segment
  register (moves), and the far call, far jmp, `retf` and `iretq` that plan 5.3 puts in layer 1's forms.
  The rest: `loop` and its kin, `xlat`, `bswap`, `bsf`, `bsr`, `enter`, `leave`, `pushfq`, `popfq`, `lahf`,
  `sahf`, `cmpxchg`, `cmpxchg8b`, `cmpxchg16b`, `xadd`, `popcnt`, `lzcnt`, `tzcnt`, `movbe`, `adcx`, `adox`,
  `crc32`, the fs and gs base reads and writes, `rdtsc`, `cpuid`, the fences, `movnti` and the rep string
  instructions. Until then each gives illegal instruction, counted as "unsupported instruction reached".
  (The independent check of X3 found this list understated; X4's own additions are under X4.)
- `presented(code, offer)`, `REFERENCE_HAS` and layer 1's rule for an `Err` case (plan 7.2 "As built at X2"
  had them at X3) come with X5's never-claimed list in layer 1: X3's layer 1 draws only forms whose meaning
  is written, so no case reaches them before. Plan 7.2 says so now.
- `Guest::may_vary` (added to the interface at X3) is in plan 3.2 now. x86's answer is true for an invalid
  encoding, every `Code` outside the default claims (illegal instruction by design where a CPU with the
  feature runs it), a default `Code` with no meaning yet, `int 0x80` and `sysenter`, and the instructions
  with undefined flags (bt and its kin among them); false for kernel-only `Code`s of claimed features, an
  instruction over 15 bytes, and the reserved NOPs (NOPs while the kernel leaves CET and MPX off).
- The reference runner's place is one range per CPU in `layout.rs` (`RUNNER`), which the runner checks at
  start, and x86's case generator drops cases that reach it (plan 7.2). Seen at once: the i7-8750H put the
  runner's heap 0x3b5c_9000 bytes after its image (x86-64 Linux randomises the heap by up to 1 GiB).
- Plan 5.4's canonical rule was wrong for the kernel's half: an access there is canonical and a page fault on
  the reference. Plan 5.2's length rule ran only for 15-byte `INVALID`s; it now runs for every `INVALID`.
  `Index::BitOffset` gained the operand's own index (plan 3.2).
- `GuestTest`'s `StandIn` gains `set_base` (the system call that sets a thread pointer in the block: x86's
  `arch_prctl`), read by the wasmtime and the Chrome stand-ins alike, and `Target` its runner's remote
  directory.
- Plan 5.2's length rules were replaced by a table from a sweep of every map on the reference (run 2). The
  table also counts never-claimed `Code`s (iced's lengths for AMD's 3DNow, XOP and SSE4a are not what the
  Intel reference counts). The sweep is a program of the test crate (`x86-lengths`), not a script: it goes
  through the harness's reference transport. Its output sits beside the x86 crate's tests.
- Kernel-only `Code`s of features not presented are never claimed, not "kernel only" (plan section 2).
  Plan 5.2 said every kernel-only `Code` was "kernel only" whatever its feature, which would have given
  general protection for VMX, SMAP or CET instructions, which a CPU without those features does not have.
- `told()`, `assemble`, `smoke`, `checks` and `changes` of x86's `GuestTest` panic naming the milestone that
  builds them; no x86 test calls them yet.
- The iced memory sizes and register numbers the meanings need are a small map in the crate (iced has them
  only with `encoder` or `instr_info`, which the shipped decoder leaves out for size); a test checks the map
  against iced's own answers.

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
  (the offer's default until X8: the interface's `Offer` is `Default`, run 8); `no_form(seed)` gives whole cases; `hand_cases()` stands for `designed` and
  also holds the hand-made compared cases; `reconcile` stands for `may_differ` (a mask cannot say "equal to
  the start" or "the reference's stored bytes do not count"); `track`, `PC_FIELD`, `SP_FIELD`,
  `GENERAL_FIELDS` and `NAME` added.
- ARM64's four hand-made layer 1 tests (alignment faults, vector registers, the step instructions' edges,
  designed answers) are one test, `hand_cases_do_what_they_must_arm64`, over the same 105 cases.
- A Chrome instruction case carries its length (a byte), then `MAX_LENGTH` bytes, the instruction then the
  filler (ARM64: the 4-byte word, as before). The page translates the instruction's own bytes, its lookup
  covers the instruction's address alone, and it fills the code area with the filler once a page from the
  page's `fill` setting when the filler is not 0 (the plan had the fill sent with the settings but did not
  say the page fills once, nor that the lookup and the translated bytes must match wasmtime's). So Chrome holds what wasmtime and the CPU hold (run 8; before it, x86 cases that fall through
  would have run the padding's `int3`).
- Layer 3's check that a blob's signal ending is the program's own runs the ordinary executable on the
  guest's reference through the transport (a new wire record, `RECORD_PROGRAM`), not `docker run` on the
  Mac, which for x86 would have been Docker's emulation (run 8). The program starts under the blob's name
  with no environment (Docker's had its path and a shell's environment) and ends at the blob's 20-second
  limit by SIGALRM, status 142 (was `timeout -s KILL`, 137); a blob that does not end in time is never
  compared, so no comparison changes.
- `GuestTest::DEFAULT_OFFER` is gone: the interface's `Offer` is `Default`, its default what the CPU claims
  when the container asks for nothing more, and the tests and the browser glue use `G::Offer::default()`
  (the browser glue passed ARM64's `()` itself; run 8).
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

- The array of slots' layout is one function of the contract, `contract::slot_entry::<G>(offset)` (4 bytes
  per `ALIGN` bytes), used by the wasmtime runner and the test guest's own runner; the plan had the layout
  only in words (X2 run 9).
- The wire has a fourth result status, the runner itself failed (`RESULT_RUNNER_FAILED`, sent by the serving
  process's panic hook before it ends with 101), so a runner's failure is never taken for a lost connection
  (plan 7.3, X2 run 9).
- A local reference and layer 3's gcc image use Alpine 3.24.2 by digest, not `alpine:latest`; the gcc image
  is named by the SHA-256 of its recipe, so a changed recipe makes a new image (plan 7.3, X2 run 9).

- CPUID's topology (X5 run 2). The brief asks that the processor-count and topology leaves agree with the
  CPU count the kernel reports, which may differ between machines, with no count fixed in translated code.
  The plan named no shape. Chosen: every logical processor is a package of its own (leaf 1: one logical
  processor, APIC identifier 0, no HTT; leaf 0xB: one SMT level and one core level of one processor each;
  leaf 4: each cache shared by one processor). That agrees with any count, so nothing is read at run time.
  The kernel's /proc/cpuinfo should say the same (each CPU its own physical id) at X10.
- `popf` that sets TF (X5 run 2): until X6 builds stepping (plan 5.1), the bit is kept and shows in `pushf`,
  but no single step is taken; counted and warned "popf set the trap flag: single steps are not taken". The
  plan's `raise_stop` path comes with X6.
- Unreachable forms (X5 run 2): iced decodes `0F 18 /0` to `/3` (prefetch) and `0F 1F /0` (the multi-byte
  NOP) as reserved NOPs with the decoder options x86 uses, which is what the CPU does with them (nothing, no
  fault), so `Prefetch*` and `Nop_rm*` never reach a meaning and are counted as reserved NOPs. The 16-bit
  near branches, `retnw` and `ud0` without ModRM exist only in iced's AMD decoding (iced's documentation:
  Intel ignores 66 on those branches), so the gap list counts only forms with `intel_decoder64()`. Taken from
  iced, not checked separately on the reference.
- The passed-register lists (X5 runs 5 and 6): the plan's starting lists measured fastest; with them layer 2
  failed at bounded look-ahead, from a translator bug that run 6 fixed (a system call's result in a local),
  and x86 now uses them (plan 3.4).
- The never-claimed list (X5 run 5): plan 7.4's `GuestTest::REFERENCE_HAS` is the method `reference_has(form)`
  on the trait, filled from x86's constant `REFERENCE_HAS` (feature names, as the claim lists are); the list
  also holds the AVX setting's forms (setting off), and so is called the unoffered forms. `invpcid` and
  `xsaves` were added to it: the reference has them and gives general protection (kernel only), where the
  CPU presented lacks them. ARM64's `unoffered_forms` is empty: ARM64 draws no forms of features it does not
  offer yet.

## Decisions taken in the plan that the owner may want to hear about

- X3: ARM64's `may_vary` answers "may vary" for every instruction until its meanings are audited one by one;
  a native output would copy no ARM64 instruction through until then.
- X3: x86's undefined flags follow the i7-8750H (Coffee Lake); another CPU of the same architecture may
  differ, which `may_vary` records for those instructions.

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
