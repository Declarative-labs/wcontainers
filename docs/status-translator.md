# Translator status

Plan: docs/phase-1-translator.md. Branch: phase-1-translator.

## Where things stand

M3 (every instruction) is closed out (fourth run, "M3, close-out" below) and waits for its independent check.
Every one of the 1215 forms of the CPU we present translates and matches the CPU in layer 1 (1215 x 1000
cases, seed 5: no mismatch), `programs/differences.c` (section 8) matches Linux at every level, at none and
full, in wasmtime and Chrome, and layer 3 runs Csmith and hand-picked programs against Linux. The long runs to
the plan's pass bars (layer 1 at 10,000 a form, layer 2 at one million programs, layer 3 at 100,000) are not
M3's: the orchestrator runs them once at the end of the translator track, as M4 and M5 still change the
translator. Commands are under "How to run".

M0, M1, M2 and S1 are built and passed their independent checks. Their work is recorded in the sections
below, newest first.

## M3, close-out (fourth run)

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

- Everything: `nix develop -c cargo test -p wcontainers-arm64 -p wcontainers-translator -p wcontainers-translator-test`
  (includes the Chrome test; it needs Google Chrome installed and Docker running).
- Lint the two side packages too: from `crates/wcontainers-translator-test/browser`,
  `nix develop ../../.. -c cargo clippy --target wasm32-unknown-unknown --target-dir ../../../target/browser -- -D warnings`;
  from `.../reference`, `nix develop ../../.. -c cargo clippy --release --target-dir ../../../target/reference -- -D warnings`.
- Layer 3: `WC_LAYER3_CASES=100000 WC_LAYER3_SEED=<n> WC_LAYER3_CHROME=100 WC_LAYER3_JOBS=12 nix develop -c cargo test --release -p wcontainers-translator-test --test layer3 -- --nocapture 2>&1 | grep -v "wcontainers warning"`
  (defaults 4 Csmith programs, seed 1, the first 1 that ends in time also in Chrome at none and full, one at
  a time; the hand-picked programs are 48 builds each run). Needs Docker, Chrome, and network once to make
  the gcc image. At the third run's rate (2000 programs in 14 minutes, 12 at once) 100,000 take about 12
  hours. Replay one program: the same seed with `WC_LAYER3_FROM=<case>` and `WC_LAYER3_CASES=1` (a failure
  prints its case and its Csmith number, `seed * 1000003 + case`).
- Floating point and vector timing: `nix develop -c cargo test --release -p wcontainers-translator-test --test programs floats_speed -- --ignored --nocapture`
  (`WC_BENCH_RUNS`, `WC_BENCH_WORKLOADS` 1 to 4, `WC_BENCH_CHROME=0` as for the benchmarks).
- Every form valid WebAssembly: `WC_VALID_WORDS=<n> nix develop -c cargo test -p wcontainers-translator --test valid`.
- Layer 1, long: `WC_LAYER1_CASES=10000 WC_LAYER1_CHROME=100 WC_LAYER1_SEED=<n> nix develop -c cargo test --release -p wcontainers-translator-test --test layer1 -- --nocapture 2>&1 | grep -v "wcontainers warning"`
  (defaults: 64 cases, seed 1, 1 case a form in Chrome, 1000 cases to a Chrome page; 6075 took 25.5 s).
  Replay a failure: same seed and cases, plus `WC_LAYER1_FORMS=<form name>` (the wasmtime run only). The
  filter drops the runners' warnings (memory faults and stops), one per case.
- Decoding against Arm's forms: `WC_DECODE_CASES` (default 64 per form), `WC_DECODE_WORDS` (random words
  from the whole space, default 200,000), `WC_DECODE_SEED` (default 1).
- Words in no form against the CPU: part of the layer 1 command; `WC_LAYER1_CASES` words.
- Translator proptest: `PROPTEST_CASES`, `PROPTEST_RNG_SEED` (default seed 1).
- Layer 2: `WC_LAYER2_CASES=1000000 WC_LAYER2_SEED=<n> WC_LAYER2_NATIVE=<k> nix develop -c cargo test --release -p wcontainers-translator-test --test layer2 -- --nocapture`
  (defaults 64 cases, seed 1, 8 of them on the CPU; each CPU case starts a container, about 0.3 s).
- Permissions changed at random stops and system calls: `WC_CHANGES_CASES=100000 WC_CHANGES_SEED=<n> WC_CHANGES_PROGRAM_SEEDS=<k> nix develop -c cargo test --release -p wcontainers-translator-test --test changes -- --nocapture 2>&1 | grep -v "^wcontainers warning"`
  (defaults 64 random programs, seed 1, 3 seeds for each C program; since the eleventh run about 33 ms
  a random program at load 16 to 21, as every program now calls a function that makes a system call).
- Benchmarks: `WC_BENCH_RUNS=3 nix develop -c cargo test --release -p wcontainers-translator-test --test benchmarks -- --ignored --nocapture 2>&1 | grep -v "^wcontainers warning"`
  (`WC_BENCH_WORKLOADS=1,2` picks workloads, `WC_BENCH_CHROME=0` leaves Chrome out). About 15 minutes.

## Cases actually run

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

1. M3's independent check.
2. M4 (bounded look-ahead and the rare paths), which also makes the changes test strong after a run's first
   handled fault (see "Departures").
3. At the end of the translator track, the long runs to the pass bars, started by the orchestrator
   (commands under "How to run"; about 12 hours for layer 3 with 12 at once).

## Known gaps (behave as the plan says: illegal instruction, counted, warned)

- FPSR's cumulative exception flags are kept only once the program has written FPSR (or set FPCR); a
  program that reads them without ever writing gets the flags it had, counted and warned as "exact
  floating point requested". A thread made by the kernel (fork, a new thread) starts with them not kept.
- CASP of 64 bit pairs is a plain load, compare and store: exact with one thread, not atomic with more
  (WebAssembly has no 128 bit compare and exchange).
- The "code changed" stand-in forgets the lookup table's pieces of the 64 byte line only; full pieces that
  hold the code inline elsewhere keep it. Code that changes is out of phase one.
- The float hook's counters carry the operation as their detail, not the address (registers are not saved
  at that hook).
- Flat blobs carry their program headers after the image (since M3's second run), but the image has no
  PT_PHDR and `flat.ld` places no thread local sections, so thread local data is not supported in test
  programs.
- At full, a floating point instruction that must take the slow path leaves for code at none from that
  instruction, translated at run time, and the rest of that function call runs at none. The runner's code
  translated at run time then holds the table's place at every address it reaches (the M2 departure on code
  translated at run time), so calls through the table into those functions run at none afterwards too.
  Exact either way; bounded look-ahead (M4) is the plan's answer.
- Only 64 bit guest memory. 32 bit memory hosts need the high-bit check (section 6).
- The WebAssembly stack overflowing on deep guest recursion is not handled, at either look-ahead: every
  guest call is a WebAssembly call at none too. The engine stops the guest ("call stack exhausted"), the
  registers are lost and the runner panics; nothing is counted or warned. The independent check found it
  with recursion 50,000 deep in 16 byte frames (800 KB of a 1 MiB stack), which Linux runs. The plan's way
  out is the "stack is lost" path (M4): count call depth and, past a limit, leave to the bottom loop and
  carry on from the guest's own stack. Not built; its cost on calls is unmeasured.
- Not yet in the across-layer tests: pieces of different look-aheads mixed at random, and injected rare
  events other than the stop flag (stack lost, missing piece) (M4 per the plan's milestones). Faults with
  the table changed at random stops and system calls are in `tests/changes.rs` since S1's close-out;
  bounded look-ahead does not exist yet, so only full is compared with none there.
- A test whose guest never ends hangs rather than fails (nothing bounds how long a translated run may
  take); the hang this run fixed showed as layer 2 and `tests/checks.rs` running for ever.
- The full piece of a function keeps every local it may need; very large functions could pass V8's 50,000
  locals or function size limits. Not checked yet (M5).
- Finding functions explores every entry again when entries are added; fine for the test programs, not
  measured on large binaries (M5).
- Stand-in brk never moves the break.
- A stop flag raised from outside is tested in wasmtime only (`tests/smoke.rs`, before a run and by a
  second thread mid-loop). The Chrome runner has the same hook code.
- Slow paths taken inside translated code (left to an address) and hook calls that do no work are counted
  with zero time; only the runner's own work is timed.
- Layer 1 checks only words the CPU we present has. Whether undefined words really raise SIGILL on a CPU
  without the feature is checked against Arm's list, not against a CPU (the reference CPU has newer
  features than the one we present).

## Departures from the plan

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
  registers as none, the handling is there so a run meets many faults. The test is weaker than its tally of
  faults suggests: after the first fault it handles, the run carries on from the faulting instruction
  through code translated at none, which replaces the table's pieces of every address it reaches, and the
  functions on the stack resume at none too. So mostly the changes before a run's first fault test full
  pieces; changes and faults after it mostly compare none with none. Bounded look-ahead (M4) is the fix.

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
- A frame or group permission check that fails leaves to the instruction and carries on at none from
  there ("left to an address"), instead of working out the exact fault in the piece. Exact either way.
  For this the contract has a third unwind payload, `UNWIND_GO_TO_EXACT`, and the load code hook may be
  asked for code at an address that has a piece (it puts one translated at none there, unless the one
  there already is). After that the table holds none code at that address, so calls through the table run
  it at none; direct calls still reach the full piece, whose check fails and leaves again each time
  (counted and warned each time; the runner does not translate again).
- The permission check of none changed shape too (fast test, then exact test), so both look-aheads share
  it; layer 1 passed after the change.
- Without the permission check (`Omit::permissions`, only for measuring) addresses are not untagged either,
  because untagging is part of the check's slow path now.
- The table byte after the last page must read as "nothing allowed"; the runners size the state memory for
  it. Pages past the table are clamped to that byte.
- A recognised jump table that reaches a target not in its table counts as "jump through a register not
  recognised" (it goes through the lookup like one).
- Every executed jump through a register that is not recognised calls the warn hook (counted per
  execution, as every slow path is). Tail calls through function pointers (common in the C library) pay a
  host call each time.
- Layer 2 compares against the real CPU only for the first `WC_LAYER2_NATIVE` cases, because the
  reference runs one blob per container; full against none is compared for every case.
- Layer 2 writes `mrs`/`msr nzcv` as raw words, because dynasm has no name for the register.
- Code translated at run time is at look-ahead none, and its pieces replace whatever the table held for
  every address it reaches (including full pieces). Correct; after a "go to address" later calls through
  the table may reach none pieces. So one "left to an address" (a group or frame check that fails, a
  tagged pointer in a group, a callee that moves sp, a jump through a table to a target not in it) runs
  the rest of that function, and what it reaches, at none. The independent check measured this (one
  unrecognised jump taken twice: 3 translations at run time). Left as is in M2: bounded look-ahead (M4)
  is the plan's answer, and its pieces will not need to replace full ones.
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
