# x86-64 native output: status

Plan: docs/x86-native-output.md. Brief: pyro-parked/wcontainers-scratch/orchestrator/relaunch-agents/
x86-native-output.js. Scratch: pyro-parked/wcontainers-scratch/x86-native-output (probes/, results/, check-xn0/,
check-xn0b/ to check-xn0g/, the independent checks' probes and outputs, and run4/ to run8/, those runs'
probes).

## Where things stand

- XN0 (the plan, checked before anything is built): done. Run 1 (2026-11-23) wrote the plan; the first independent
  check found seven blocking and eleven minor problems, which run 2 fixed. Run 3 made the second check (scratch
  check-xn0b/): three blocking and ten minor, fixed. The third check (scratch check-xn0c/) found three blocking and
  nine minor, fixed by run 4 (scratch run4/). The fourth (scratch check-xn0d/) found two blocking and six minor,
  fixed by run 5 (scratch run5/). The fifth (scratch check-xn0e/) found three blocking and six minor; run 6 fixed
  all nine (list below), measured in scratch run6/. The sixth (scratch check-xn0f/) found two blocking and nine
  minor; run 7 fixed all eleven (list below), measured in scratch run7/. The seventh (scratch check-xn0g/) found two
  blocking and seven minor (one of them no defect, a measurement to record); run 8 fixed all nine (list below),
  measured in scratch run8/. The run-8 fixes have not had a check of their own. Nothing is built; no code of this
  track exists yet.
- XN1 (the output with no memory check): complete for its independent check (closed by round 3's first run,
  2026-11-23, on the orchestrator's ruling): what is built is below run by run, what is not is the gap list under
  "XN1 gap list", each item with its cause and owner. Runs 1 to 10 in detail: done: the eighth check's five
  findings answered in the plan; the shared `Route::Area`; the core's `string_passes`; the route of every decoded
  form per way with its static routing test; the 0x67 segment-operand finding's code with its test (run 1). Run 2:
  the trait takes the way and the target (shared, ARM64 one of each); `Native for X86_64` for the machine and P,
  every route carried (below); the guest's FIP stored after each copied x87 instruction; the flag-free check
  after a copied `fldcw` with the new call kind `X87Control`; a rep string under the count run by the inside;
  `DirectExit::condition`; the position map's operand with segment and address width; `Condition` and its
  plain `holds` in the core; the pre-executor bar's encoding round trips and call-position map checks. Not yet:
  the replay check (`stop_check`), the x87 unmasked mode and the inside's side of `X87Control`, `answer` and the
  x87 unit's move into the core, the pool, `retarget`/`Patch` and the `int3` link protocol, stepping pieces, the
  differential test of `holds` against the meaning's WebAssembly.
  No x86 executor can run a translation yet (P's executor, crates/executor/implementation/host-process, was
  committed by its track during this run without a translation runner; no x86 floor), so the bar's layers,
  programs and speed wait on one (plan 8).
  Run 3: layer 1 runs natively after all, without an executor: the test crate's new
  src/guest/x86_64/native.rs puts each case's piece (way P) in a reference blob that sets up the case's memory,
  the thread area and the first level, loads the case's state and enters the piece; the guest's state at the stop
  is read through the position map and compared with the CPU. Each case runs uncounted, and counted with the trap
  flag stopping it after every host instruction (state, data and count checked at each stop: the every-offset test
  for layer 1's pieces). It found five defects of the output, fixed (below). 2008 forms, 1,000 cases a form, seed
  5150: 0 failing (results below). Layers 2 and 3, the X9 programs and speed still need an executor (they run
  many pieces, links, lookups and system calls).
  Run 4: layers 2 and 3 and the benchmark set run natively too, without an executor: the test crate's new
  src/guest/x86_64/native_programs.rs makes a program's pieces (way P) from its start, the targets of their exits
  and the return points of their calls, lays them out linked to each other (`carry::retarget`, a shared `Patch`)
  and fills the lookup's second level with them, and runs them in a reference blob that also answers the guest's
  system calls (the real call, from the guest's registers) and its repeated strings under the count (the
  instruction itself, the count taking the meaning's passes by `string_passes`); a lookup or exit that finds no
  piece ends the run, which is made again with that piece. Output and status are compared with the CPU's, and with
  the count, the count after each system call with the WebAssembly output's. Results below. The benchmark set's
  times against the host showed the withheld registers named by hot loops (matrix 6.2 times the host's): a guest
  instruction naming one now reads or writes its slot directly where the memory form acts the same (plan 3.4),
  which native layer 1 checked (it found two classes where it does not, kept out and named in a test). Of the
  bar: layers 1 (2008 forms, 1,000 a form), 2 (10,000 programs) and 3 (1,000 Csmith programs) pass natively against
  the CPU, the count after every system call equal to the WebAssembly output's; speed fails (1.27 against 1.25,
  matrix 2.8 against 1.6); the X9 programs, replay, register groups and hostile guests wait on an x86 executor.
  Run 5: a loop's homes (plan 3.4): a straight-line part looping to itself that names withheld registers runs its
  copy with them renamed to registers it never names. Speed now meets the bar uncounted (geometric mean 1.145, the
  most search 1.395); layers 1 (2008 forms, 1,000 a form), 2 (10,000) and 3 (1,000) pass again with it; the sweep
  now follows every reachable host instruction's moves against the position map (results below). Left: the replay
  check, `answer` and the core moves, the pool, the trait's `retarget`, and what waits on an x86 executor.
  Run 6: the x87 unit, transcendental.rs and root.rs are in the x86 core (plan 9), which builds for
  `x86_64-unknown-none`; `answer` (x86/answer.rs, core) answers `cpuid`, `xgetbv` and the eight transcendental
  instructions, each checked equal to the meaning's WebAssembly case by case (native layer 1 for the
  transcendentals, a new test for `cpuid` and `xgetbv`, which layer 1 leaves out as the CPU's own table differs).
  Run 7: `answer` takes the guest's memory (`answer::Memory`) and forms a memory operand's address from the block
  (`answer::address` over the shared `Operand`); it now also answers the reciprocal estimates (legacy and VEX),
  `popf`, the descriptor reads (`lar`, `lsl`, `verr`, `verw`, `sldt`, `str`, `smsw`, `sgdt`, `sidt`), the segment
  register moves, `push`/`pop` of fs and gs, and `lfs`/`lgs`/`lss`, each equal to the meaning (native layer 1 now
  compares 82,000 answered cases; a new test 19,427 edge cases).
  Run 8: `answer` also answers `rdtsc`/`rdtscp` (through `answer::Clock`), `wrfsbase`/`wrgsbase`, `retf`, `iret`, far
  `jmp`/`call` through memory, and `xsave`/`xrstor` (with `Memory::check` for their order of faults), each equal to
  the meaning in two new edge tests; `lfs`/`lgs`/`lss` now wrap the selector's address by itself under 0x67, as
  the meaning does (an unchecked-form 0x67 defect of run 7's answer, found while building the far branches). Still
  to answer: the partial-write and undefined-result forms (for hosts whose probe finds them different).
  Run 9: the Float status route's second call is built (`answer::float_status_after`, `native::float_status`): a
  save's image gets FIP from the register area and FDP, FOP, the selectors and MXCSR's mask from the block as the
  meaning writes them, a load leaves the image's pointers in the block, `fnsave` clears them, `ldmxcsr` turns
  tracking on. Native layer 1 now makes the call wherever a run reaches it, and a new test runs every save and
  load form after x87 instructions (one piece for the stretch) against the meaning (results below).
  Run 10: the partial-write forms are answered (`Question::Enter`, `Question::X87Store` for every x87 store of
  ST(0) to memory), with an answered x87 instruction setting the block's FIP; a new test checks them against the
  meaning and runs them natively through the inside for a host that differs. The partial-write probes ran on AMD
  (EPYC 9R14): `fstp m80` and `fbstp` leave no partial write there, so such a host routes them Kernel (results
  below).
  Round 3 run 2 (after the ninth check, the first independent check of XN1): an operand on the guest's `gs:` runs
  on FS holding the guest's GS base (plan 3.1), so every Kernel route without an answer is gone; every guest access
  is written through `carry::access`; section 7's shapes that need no executor and the rep string count test are
  tests; a jump where nothing is mapped ends as the guest's fault in the native program harness (the two Csmith
  programs). Finding by finding under "Round 3 run 2".
- XN2: not started. The eighth check's way I findings (0x67 strings, the exact form's 0x67 segment operand and
  `67 d7`, `maskmovq` in the window) are fixed in the plan only; their code and tests are XN2's. `carry` refuses
  the in-process way by an assertion until then.

## XN1 gap list (round 3, 2026-11-23: every item left, with its cause and owner; corrected by run 2 of round 3)

The bar as it stands: native layers 1, 2 and 3 pass against the CPU on the rented Xeon, way P, counted and
uncounted, the count after each system call equal to the WebAssembly output's (figures under "Round 3 run 2
results"); speed meets the bar uncounted (geometric mean 1.145 against the host in run 5, 1.110 in the ninth check's
own measurement). Section 7's shapes that run without an executor are tests now (round 3 run 2, below); the rest
are listed here with why. Every Kernel-routed form in the machine and P now has an answer: a question for
`answer`, or (a repeated string under the count) the inside's run of it on the hardware with `string_passes`; the
only exceptions are item 2's. Not built or not run:

1. Needs an x86 executor that runs a translation. Owner: the native-without-virtual-machine track (its x86 floor,
   or P's executor, crates/executor/implementation/host-process, which has no translation runner yet). Cause: the
   blobs of the test crate answer only system calls and repeated strings, from the guest's registers, in one
   thread, with no signal delivery to the guest and no control of the guest's page permissions.
   - The X9 real-program set natively (threads, signals, `execve`, our kernel) with the WebAssembly verdicts.
   - Replay against a recorded run, register-group tests (plan 3.1, brief-native.md's last section), hostile-guest
     tests (plan 7), the mirror's memory on busybox and python, links' batches and `int3` traps, P's stub with its
     flags table (plan 3.6: the table is the stub's read-only page, the output's `popfq` reads it there).
   - Section 7's shapes that need threads, signals, another process or page permissions: code reading and storing
     into its own bytes (same and sibling thread), writes to code through a shared mapping and through the kernel,
     the `futex` wait over rewritten, unmapped and re-protected code, a signal handler editing rip, r10, r11, r15
     and rflags, a jump to the vsyscall page and the vDSO, `int 0x80`, `sysenter` and a far transfer to 0x23 as
     the kernel delivers them, a stop inside `rep movsb`, `rep stosq` and `repe cmpsb`, an instruction straddling
     into a page that cannot run code (the blob maps the guest image readable and writable only, so there is no
     page that the CPU's run and ours both see as not executable), `popf` setting TF before a `ret` and a `jmp r`,
     and a thread with TF running `rep movsb` and ten one-byte instructions (both on stepping pieces, item 3).
   - The start-up probe that fills `HostMatches` (paging depth, partial writes, undefined results) and the floor's
     entry rule for MXCSR (plan 3.5).
   - The inside's side of the `X87Control` call and of the `x87_unmasked` mode (the call is emitted after every
     copied `fldcw`; with no inside a guest that unmasks an x87 exception is not run natively). The core's half of
     it, x87_answer.rs (plan 9: the plain-Rust twin of fpu.rs's instruction-level part), is this track's and comes
     with that mode, after XN2; so does the check of a save's FDP and FOP other than zero, which only an unmasked
     exception can set.
   - The FS window of an operand on the guest's `gs:` (plan 3.1): a stop, fault or signal inside it finds the
     guest's FS base in u through the position map (guest register 16, `carry::FS_BASE_NUMBER`); P's handler and
     the machine's floor must take it from there and put it back on FS before resuming the guest elsewhere.
     Native layer 1's every-offset run checks the map at every host instruction of the window; the handler's
     side is that track's.
2. Undefined results on AMD hosts. Owner: the instruction behavior design (docs/status-instruction-design.md:
   each chip recorded with its own pinned behaviors, a start-up probe choosing the set). Cause: measured in run 10
   (EPYC 9R14 against the Xeon's sweep): 9,447 of 15,644 flags-sweep cases differ, in rflags alone, on shifts and
   rotates by more than one, mul, imul, div, idiv, tzcnt and lzcnt (data recorded there, raw in scratch xn1r10/).
   Nothing is built for it here on purpose: no inline flags and no inside answer of these forms. `route` copies
   every form of `native::undefined_result` where `HostMatches::undefined_results` is true (the Xeon, the
   i7-8750H); where it is false it sends all of them Kernel, for which `native::question` gives no question
   (`answer` has no twin of these meanings), so a host with it false cannot run them natively yet. That one flag
   also covers and, or, xor, test, the bit tests, bsf and bsr, which the EPYC sweep found equal to the Xeon: which
   forms a chip's set holds is that design's to say. (Partial writes differ on AMD too, `fstp m80` and `fbstp`;
   the inside's answer for them is built and tested, run 10.)
3. Needs ARM64's version first. Owner: the ARM64 native back end (shared trait in gcpu/native.rs).
   - The replay check: `stop_check` in the trait, `InsideCall::Stop`, `stop: Option<u64>` in
     `translate_native_for`, `Step` and P's stepping pieces (so every case with TF or AC set, which native layer 1
     counts as not run, and the rep string test leaves TF out). ARM64 has no replay check yet. Natively a stop is
     the host's signal; the output's only part in it is the count's plain add.
   - The trait's `retarget` in place of `link` (x86's is built, run 4, and the native blobs use it).
   - Homes for a loop of several parts (search 1.40 against the host): the translator's loop copy would take a set
     of parts.
4. This track's, after XN2 (not in the bar):
   - The pool for a call through a register (`HostCode::pool`; today the return address is built in rax saved to
     the area, the plan's measured 1.146 form, not the pool's 1.120); it also needs the executor's pool range
     (native-without-virtual-machine 3.4).
   - Counted speed (exact replay): interpret 1.66, the count's adds.
   - The note of an MXCSR read while not kept for `xsave` (the meaning warns; `answer` is not told tracking).
   - `enter` with rsp off the canonical range under 5-level paging (70 known failures in run 10's test): the
     reference's page fault where the meaning gives general protection, the x86 track's meaning to answer; and the
     x86 track's known meaning differences (`sldt`, `smsw`, `str`, `wrfsbase`, `wrgsbase`, under "Coordination").
5. Way I (XN2, as written): the eighth check's way I findings (a string with the guest's 0x67 to the inside in
   both checked forms, the exact form's 0x67 segment operand and `67 d7`, `maskmovq` re-encoded in the window), and
   now the ninth check's crossing of offset 2^32 under 0x67 in the exact form (plan 4). Fixed in the plan; their
   code and tests are XN2's. `carry::access` refuses the in-process way by an assertion.
6. Not this track's: the meaning's choice among several segment prefixes (`2e 65 36`: the CPU takes the last,
   ss; the meaning and iced take gs). Owner: the x86 track. The native output now runs such an instruction's own
   bytes, so it is the CPU's unless the instruction also names a withheld register (then iced re-encodes it with
   one prefix, as the meaning reads it). And the native layer 2 harness past about 10,000 programs in one process
   (the ninth check: "state memory: mmap failed to reserve 0x104000000 bytes", runner.rs 455/461/1850, the
   WebAssembly runner's reservation, made by the WebAssembly runs that test compares the count with): the cause
   is not found yet (a reservation kept past its run, or the machine's map limit); seen only by the native test,
   since no other test runs that many programs in one process. Owner: this track, with the shared runner's
   owner; 10,000 at a seed is the recorded bar meanwhile.

## Round 3 run 2: the ninth check's findings, finding by finding

- [blocking] 0x67 `gs:` access across offset 2^32 (fxsave64): the plan's rebuild `lea u32, [...]` then `[t + u]`
  wrapped only the start. Now the instruction runs as the guest wrote it on FS, FS holding the guest's GS base for
  it (`carry::enter_guest_segment` and `leave_guest_segment`: `rdfsbase u; mov t, gs:[guest GS base]; wrfsbase t;
  <the guest's bytes, 0x65 made 0x64>; wrfsbase u`), the position map holding the guest's FS base in u meanwhile;
  the CPU wraps each part. Tests: the check's case and its seed-4 case in tests/native_carry.rs (bytes) and the
  new layer 1 shapes test (against the CPU), and native layer 1 with the check's seed 31337 at 1,500 a form.
- [blocking] two Csmith programs at seed 4: both are UB programs whose `main` falls into a `ret` reading the zero
  stack, so the guest jumps to 0 and Linux kills it with SIGSEGV; the WebAssembly runner gives the same. The native
  program harness ended with an error at a lookup of an address with no code. A target outside the program's image,
  where nothing is mapped, now ends the run as the guest's fetch fault (SIGSEGV), as the inside gives it.
- [blocking] Kernel routes with no answer: with the FS window, strings, `xlat`, `maskmovq`, `maskmovdqu` and
  `vmaskmovdqu` on `gs:`, bit tests with a register offset on `gs:` under 0x67 and gathers on `gs:` under 0x67 are
  the Area route (the CPU's own access), and `stos`, `scas` with a `gs:` prefix are copied (es:rdi ignores it).
  A rep string under the count is the inside's run of it on the hardware with `string_passes` (as the program
  blobs do), so native layer 1 now counts those runs apart ("run by the inside on the hardware") and fails any
  other call into the inside whose instruction `native::question` does not answer (`syscall` excepted).
- [blocking] section 7's shapes and the rep string test: new tests, tests/layer1.rs
  `the_shapes_of_the_plan_match_the_cpu_natively_x86_64` (ret, jmp r and call r, plain and 66-prefixed, to 2^47,
  2^48, each of bits 48 to 63 over a valid target, 0xfff0000000401000, 2^63, -1; 66-prefixed direct jmp and call;
  the base instructions and `mov r, gs`; `rdsspq`, `incsspq`; the implicit `gs:` forms; gs67 both ways;
  RIP-relative on `gs:` with and without 0x67; the check's fxsave64, setg and bit-test cases), tests/layer2.rs
  `branches_into_an_instruction_match_the_cpu_natively_x86_64` (a direct jump into a `mov`'s immediate, and a
  jump through a register to the same) and `repeated_strings_count_as_the_webassembly_output_natively_x86_64`
  (seeded, `WC_STRINGS_SEED`, `WC_STRINGS_CASES`, `WC_STRINGS_JOBS`; every kind, size, rep and repne, 0x67, DF,
  rcx 0, 1, 7 and the cap and either side, rdi - rsi 0, 1, an element, 10, the length, -10, past the cap; the
  check's 91-pass case first). The shapes that need threads, signals or page permissions are on the gap list,
  item 1, with why.
- [blocking] one place for a guest access: `carry::access(way, code, written, resume)`; the copied bytes, the
  rebuilt instructions, the return's read, the calls' pushes and the jump through memory's load go through it. The
  slot form of a withheld register reaches the register area, not guest memory. Plan 4 names it so.
- [minor] RIP-relative bit test under 0x67 (Rebase): the access is now on `[t32]` (r11d) with the 0x67 kept, so the
  CPU wraps the bit offset; the check's case with rax 2^35 and 2^63 is in the shapes test.
- [minor] layer 2 past 10,000 programs: not fixed, recorded (gap item 6) with the 10,000 bar.
- [minor] several segment prefixes: the native output now takes the CPU's choice (the guest's bytes); the
  meaning's is the x86 track's (gap item 6); the check's case is in the shapes test.
- [minor] one `undefined_results` flag: recorded exactly in gap item 2, and the contradicting line under "For the
  independent check" corrected; no route changed (the chip sets are the design's).
- [minor] stale departure note: replaced.
- [minor] speed figures shifting between runs: recorded beside run 5's under "Measured" (the check's 21:10 UTC run,
  load 9 to 27: uncounted geometric mean 1.110, worst search 1.395; counted 1.231, worst 1.489).

## The independent check's findings carried into XN1 (the eighth check)

All five were answered in XN1 run 1 (see "What XN1 run 1 changed"); this round found nothing new to change:
- 0x67 strings in way I (blocking): plan 4 sends them to the inside in both checked forms; code at XN2 (way I).
- A segment operand with the guest's 0x67: unchecked form built in run 1 as `lea u32, [base32 + index32*scale +
  disp]` then `[t + u]`, which the ninth check showed wraps only the start of an access; replaced in round 3 run 2
  by the FS window (the guest's own access on FS holding its GS base), tested by tests/native_carry.rs with the
  check's gs67 case (reaches 0x300000010, as the reference), its fxsave64 and setg cases (bytes) and a seeded sweep
  of operand shapes against the meaning's address, and by the shapes test against the CPU; the exact form and
  `67 d7` are XN2's.
- `maskmovq`: named beside `maskmovdqu` in plan 3.1, 4 and 7; the machine and P route it Kernel on the area's
  segment (native.rs `implicit`), checked by tests/native_routes.rs; I's re-encoding is XN2's.
- The rep string count: named as a second implementation of the meaning's rule with its full rule (plan 3.6);
  `string_passes` in the core, tested by `cargo test -p wcontainers-x86 --lib strings` and by native layer 2's
  count after each system call against the WebAssembly output.
- The unmasked x87 mode's plain-Rust twin: named as new code (x87_answer.rs, plan 3.1 and 9); gap list item 1.

## Round 3 run 2 results (rented x86 machine, Xeon 8488C, 5-level paging, 2026-11-23; logs scratch xn1r11/)

- 21:55 UTC, load 2.6 before: clippy with warnings denied, `--no-deps`, on wcontainers-x86 and -translator-test (all
  targets): clean. `cargo test -q -p wcontainers-x86`: every test passes.
- The shapes test: 108 cases as the CPU, 66 known (the 5-level reference: transfers to 2^47 up to 2^56 and the
  base writes there), 2 known (`wrfsbase` and `wrgsbase` of 2^47, answered by the meaning, which refuses what the
  5-level CPU takes); 1 s.
- Native layer 3, the check's seed 4, 300 programs, 48 at once (21:55, load 2.7): 268 compared at three look-aheads
  with and without the count, 32 did not end in time on Linux, 0 failing (4000130 and 4000302 among the compared).
- Native layer 1, the check's seed 31337, 2008 forms, 1,500 cases a form, 32 at once (21:56 to 22:10, load up to
  52 with other tracks' jobs): passes; 3,283 known failures, the same two named kinds; the inside's answer equal
  to the meaning's in 247,558 runs; the second float status call in 14,744; a repeated string under the count run
  by the inside on the hardware in 19,913; ended: Exit 2,124,033, Fault 664,510, Answered(Answer) 125,279,
  Answered(Trap) 93,553, FarTarget 3,712, Lookup 913.
- The inside's answers and the second call against the meaning (22:10, load 3): 6 tests pass.
- Native layer 2, the check's seed 77, 10,000 programs, 32 at once (22:11, load 6 to 21): 0 failing, 3,465 faulted,
  113,606 blob runs.
- The branch shapes as programs: pass at the three look-aheads, counted and not.
- The repeated string test, seed 1, 40 cases and the check's (22:20, load 11.7): 0 failing. Control (remote copy
  only): `string_passes` giving one bulk pass more is caught, 8 of the 40 failing. Seed 2, 1,000 cases and the
  check's, 32 at once (22:23, load 14): 0 failing, 8 s.
- Native layer 3, seed 1, 1,000 programs, 48 at once (22:23 to 22:27, load 17 to 42): 852 compared, 148 did not end
  in time on Linux, 0 failing.
- The literal check: no finding in a line this run wrote (three in the shapes test named without restating);
  older findings in carry.rs and the test crate's native.rs remain.

## XN1 closing run results (rented x86 machine, Xeon 8488C, 2026-11-23; no code changed since run 10)

- 20:32 UTC, load 0.2 before, 3.5 after: clippy with warnings denied, `--no-deps`, on wcontainers-x86 and
  -translator-test (all targets) and the core without `translate`: clean. `cargo test -q -p wcontainers-x86`: every
  test passes (the same counts as run 10). The `the_inside_answers_*`, `the_second_call_*` and `a_condition_holds_*`
  tests: 7 pass, the same case counts as run 10 (enter and the x87 stores 3,494, natively 732 and 70 known).
- 20:34 UTC, load 7.9 after: native layer 1 with a new seed, 2008 forms, 100 a form, seed 777, 32 at once: passes,
  269 known failures (the same two named kinds), 16,514 answers equal to the meaning's, 65 s. Output scratch
  xn1close/.

## For the independent check of XN1

- Rebuild from clean on the rented x86 machine: `x86-run <checkout> --name <fresh name> --jobs 32 -- 'cargo clean
  && cargo test -q -p wcontainers-x86 && ...'` with the commands under "How to run XN1's tests".
- Native layer 1 with another seed: the command there with `WC_LAYER1_SEED=<seed>` (5150 was this track's).
- The copy-through decision against the CPU: native layer 1 runs every form's Copy, Rebase and Area routes on the
  CPU; tests/native_routes.rs checks the route of every decoded form against the plan's rules (`X86_ROUTE_SEED`).
- The FS window of a `gs:` operand (plan 3.1): tests/native_output.rs `the_area_segment_runs_on_fs_with_the_guests_base`
  reads it; the dumper below prints it for any piece.
- The emitted code of a system call, a fault and a stop: scratch run9/dump.rs (sent into the copy as a test file
  of the test crate) prints a benchmark's pieces, host code beside guest; a system call in P is a call into the
  inside (`lea r11, [rip]; jmp gs:[entry + 8*kind]`, carry.rs `call`), a guest fault is the host's own fault on
  the copied access read back through the position map, a stop is the host's signal with the count's plain add
  (carry.rs `count`) as the output's only part.
- Each gap-list item: the X9 programs have no runner (item 1); `HostMatches` with `undefined_results` false (an
  AMD host) routes every form of `native::undefined_result` Kernel, for which `native::question` gives no question
  (item 2); `translate_native_for` has no stop argument (item 3).

## What XN1 run 10 changed

- `answer`: `Question::Enter { size, level, bytes }` (each push written by itself, the copies read from rbp down,
  the final stack pointer checked for a write, rbp, bp alone for 16 bits, and rsp changed only at the end, as the
  meaning's `enter`) and `Question::X87Store { code, at, bytes }` (the pending check, the unit's store with the
  operand's address as its data pointer, then the value in the meaning's two parts and the status and tag words
  taken only once memory took it; nothing written when the unit says NOT_STORED). `native::question` gives them for
  `enter` and every memory form of `fst`, `fstp`, `fist`, `fistp`, `fisttp` and `fbstp` (`fpu::store_code`). The
  route is unchanged: Kernel for `enter` with a level, `fstp m80` and `fbstp` where `HostMatches::partial_writes`
  is false.
- An answered x87 instruction that is not a control one (the transcendentals too) sets the block's FIP to its
  address, as the meaning does (plan 3.5: the inside carries the register area's FIP into the block and back).
- Shared once in the core: the hook's value split (`x87::VALUE_LOW`, `VALUE_HIGH`, which fpu.rs reads) and where a
  store's second result holds the status and tag words (`STORED_STATUS_SHIFT`, `STORED_TAGS_SHIFT`);
  `meaning::ENTER_LEVELS` is crate-wide.
- Test crate: `answered_with_pointers` (the x87 pointers after an answer) beside `answered`; the state's `FCW` and
  `FTW` fields are public. tests/layer1.rs `the_inside_answers_enter_and_the_x87_stores_as_the_meaning_x86_64`:
  `enter` of 64 and 16 bits, five sizes, nine levels (0 to 4, 31 to 33, 255), six stacks (writable, pushes and the
  final check meeting the data's start, copies meeting it, read-only, off the canonical range); the 14 store forms
  with ten ST(0) values (one, a fraction, a large negative past 10^18, past every format, tiny, quiet and
  signalling NaN, infinity, an unnormal, empty) on 16 places across the writable data's end, the data's end and
  the canonical range's, [ecx] with rcx's upper half set and fs:[ecx] passing 2^32, under four control words, and
  with an invalid operation pending. Each against the meaning, FIP, FDP and FOP included; `enter` with a level,
  `fstp m80` and `fbstp` (but fs:[ecx], whose base is not canonical) also run natively, uncounted and counted, for
  a host with `partial_writes` false, which must reach the inside unchanged and equal the CPU.

## XN1 run 10 results

- Rented x86 machine (Xeon 8488C, 5-level paging), 20:12 UTC, load 1.0 before, 8 after: clippy with warnings
  denied, `--no-deps`, on wcontainers-x86 and -translator-test (all targets) and the core without `translate`:
  clean. The new test: 3,494 cases equal to the meaning's, pointers included; natively through the inside 732
  equal to the CPU, 70 known failures, every one `enter` with rsp off the canonical range (the 5-level reference's
  page fault where the meaning gives general protection); 19 s.
- Regressions (20:17 to 20:27 UTC, load 8.5 before, up to 16 with other tracks' jobs): the x86 crate's tests and
  the `the_inside_answers_*`, `the_second_call_*` and `a_condition_holds_*` tests pass (7, the same counts); native
  layer 1, 2008 forms, 1,000 a form, seed 5150, 32 at once: 0 failing, the same 2,594 known failures, 165,030
  answers equal to the meaning's, the second call in 9,904 runs, 557 s. The literal check: no finding in a line this
  run wrote (three in the new test removed); the older strings of the test crate's native.rs remain.
- Controls (rented copy only, scratch xn1r10/controls-enter.sh, 20:15 UTC, load 8.5): all 11 caught: the final
  check left out, rbp's upper bits lost under 16 bits, one copy too many, the frame's own push left out, an 80-bit
  value's last two bytes not written, the status word or the tag word not taken, a NOT_STORED value written, the
  data pointer zero, FIP not set, the pending check left out.
- AMD (scratch xn1r10/amd-probe.sh: a c7a.large spot machine, AMD EPYC 9R14, rented for the probe and terminated,
  checked by tag; 20:15 UTC, load 0.5): the three partial-write probes (probes/partial86.c, partial3.c,
  check-xn0/partial2.c). `fstp m80` and `fbstp`: 0 of 9 cases leave a partial write (the Xeon: 2 of 9, the first 8
  bytes); `enter 0, 3`: 6, 14 and 22 bytes, as the Xeon; `mov`, `movdqu`, `vmovdqu`, `fxsave64`, `xsave64`,
  `fnsave`, the masked stores and the loads: none, as the Xeon. Output scratch xn1r10/amd-c7a.large.out.
- AMD again (scratch xn1r10/amd-flags.sh, a second c7a.large, terminated and checked by tag; 20:19 UTC, load 0.3):
  the x86 track's flags sweep (x86/probes/fixed/proof8 stepper6 and flags-sweep.sh) against its Xeon output
  (fixed-sapphire/flags-sweep.out): of 15,644 distinct cases 9,447 differ, all in rflags alone (OF 4,124, AF
  6,842, SF, ZF and PF 170 each, the last only in mul, imul, div and idiv); shifts and rotates by more than one,
  mul, imul, div, idiv, tzcnt and lzcnt differ; and, or, xor, test, the bit tests, bsf, bsr and popcnt agree.
  Comparison scratch xn1r10/amd-flags-compare.out, raw amd-flags-sweep.out.

## What XN1 run 9 changed

- `answer::float_status_after(status, block, tracking, memory) -> Result<Notes, Fault>` (core) and `FloatStatus`
  (`LoadMxcsr`, `StoreMxcsr`, `Image { restore, wide, at }`, `Environment { restore, registers, wide, at }`), with
  `native::float_status(i)` giving it; `Notes::mxcsr_flags_read_not_kept`. Plan 3.5 says what the block it takes
  holds and what the caller does after.
- The environment's layout (`x87::Environment`, `ENVIRONMENT_32`, `ENVIRONMENT_16`, its reserved halves) moved from
  fpu.rs into the core; fpu.rs reads it.
- Test crate: native.rs makes the second call at a run's FloatAfter (`float_status_after`, FIP from the register
  area, counted in `SECOND_CALLS`, which layer 1's summary line prints); `run_with_pointers` (a piece of a given
  look-ahead, returning FIP, FDP and FOP after the second call) beside `run`. tests/layer1.rs
  `the_second_call_writes_and_takes_the_x87_pointers_as_the_meaning_x86_64`: fxsave, fxsave64, fnstenv and fnsave
  in both sizes and their six loads, after none, `fld1`, `fld1; fadd`, `fld qword [rdx]`, `fld1; fnstcw` and
  `fld1; fnclex` (loads after the first two), on [rcx], [rip + disp32], fs:, gs: (the area's segment in P) and
  [ecx]; outcome and pointers against the meaning, uncounted and (one instruction) counted; saves with no prior
  against the CPU too. Load images keep the control word's reserved bits as the CPU does: the meaning keeps all 16
  bits of a loaded control word where the CPU keeps 0x1f3f and reads bit 6 as one (seen: image 0xc3ff, CPU 0x37f,
  meaning 0xc3ff), and clears neither ES nor B; that is the meaning's, the x86 track's to answer, and not this
  call's.

## XN1 run 9 results (rented x86 machine, Xeon 8488C, 5-level paging)

- Clippy with warnings denied, `--no-deps`, on wcontainers-x86 (all targets, and without `translate`) and
  -translator-test (all targets): clean.
- The new test (19:42 UTC, load 2.3): 300 runs equal to the meaning's, pointers included; the 30 saves with no x87
  instruction before equal to the CPU's (uncounted and counted).
- Controls (rented copy only, scratch xn1r9/controls-float.sh, 19:45 UTC): caught: FIP left out of an environment
  save, FIP left out of an image save, a 32-bit image load's pointers not cut to 32 bits, `fnsave` keeping the
  pointers, FOP loaded with more than its 11 bits, the 32-bit environment's reserved half written zero, MXCSR's
  mask written zero. Not caught: FOP left out of the environment's selector word (FOP is zero in every save here,
  item 7 of the next steps).
- Native layer 1 with the second call (19:44 to 19:56 UTC, load 10.9 before, up to 14.5 during with other
  tracks' jobs): 2008 forms, 1,000 a form, seed 5150, 32 at once: 0 failing, the same 2,594 known failures; the
  second call made in 9,904 runs; 165,030 answers equal to the meaning's; 694 s.
- The x86 crate's tests and the four `the_inside_answers_*` tests (19:57 UTC): all pass, the same case counts.
- The literal check: no finding in a line this run wrote; one earlier line of tests/layer1.rs (`USER_DS | 0x100`)
  now writes its bit by name. The error-message strings of the test crate's native.rs remain.

## What XN1 run 8 changed

- `answer(question, block, offer, constants, memory)`: `memory` is `impl Memory + Clock`; `answer::Clock` gives the
  inside's nanoseconds; `Memory::check(at, bytes, write)` says whether a range may be reached without reaching it;
  `Notes::transferred` (a far transfer set rip; the caller leaves it); `Fault::FarTransferTo32Bit` (general
  protection counted by name, the meaning's `FarTransferTo32Bit` warning).
- New questions: `TimeStamp` (`rdtsc`, `rdtscp`: edx:eax the clock, ecx 0), `WriteBase` (`wrfsbase`, `wrgsbase`:
  the register at its width, general protection where not canonical), `FarReturn` (`retf`, `retf imm16`: cs then
  rip read, the selector tested at privilege level 3 exactly, the target's canonical test, rsp past both slots and
  the release), `InterruptReturn` (`iret`: NT first, then rflags, cs, rip, cs tested, ss, rsp, ss tested, the
  target's test, rflags as `popf` loads them), `FarBranch` (far `jmp`/`call` through memory: the pointer, the
  selector with any privilege level, a call's cs then return address written each faulting by itself, then the
  target's test, as the meaning orders them), `XState` (`xsave`, `xrstor`, plain and 64: the meaning's order of
  faults, then each component RFBM names, XSTATE_BV from the components in use, `xrstor`'s header and MXCSR
  refusals, TOP placing the registers, a component XSTATE_BV leaves out put in its initial configuration).
- Fixed: `FarPointer` (`lfs`, `lgs`, `lss`) read the selector at the offset's address plus its bytes; under 0x67
  the selector's own address wraps within 32 bits before the segment base, as the meaning's `far_pointer` makes it.
  Run 7's edge test had no 0x67 case; the new test has one for each width (`64 67 0f b4 01`).
- Shared once in the core: the frame's slots (`answer::FrameSlot`, segments.rs reads it), the XSAVE constants
  (`XSTATE_BV`, `XSAVE_HEADER_ZERO`, `XSAVE_ALIGNMENT`, `XSTATE_BYTES`, `HEADER_KEPT`, `IMAGE_SLOT`; fpu.rs reads
  them), `cpuid::xsave_bytes` is a `const fn`; popf's rflags load is `answer::load_rflags`, which `iret` uses too.
  `segments::operand_bytes` is crate-wide for `native::question`.
- Test crate: `CaseMemory` is the clock (`ANSWER_CLOCK`, 3 * 2^32 ns) and checks ranges; `answered` leaves rip
  where a far transfer set it. tests/layer1.rs: `answered_as_the_meaning` (one case against the meaning),
  `the_inside_answers_far_transfers_bases_and_the_clock_as_the_meaning_x86_64` (`retf` and `retf 0x10` at three
  widths, `iret` at three widths with five rflags, four ss and NT set, far `jmp`/`call [rcx]` at three widths, each
  with seven selectors and six targets and five stacks that fault, the 0x67 fs wraps of the far branches and `lfs`,
  `wrfsbase`/`wrgsbase` of ecx and rcx with eight values, `rdtsc`/`rdtscp` against the meaning with the clock
  taken out: 3,145 cases) and `the_inside_answers_xsave_and_xrstor_as_the_meaning_x86_64` (the four forms, eight
  RFBMs, five mixes of status word, registers, xmm and upper halves, seven areas with XSTATE_BV, header, MXCSR and
  the reserved byte set, five places that fault and two 0x67 fs places: 1,148 cases). The guest module's `FSW` and
  `R0` are public for it.

## XN1 run 8 results (rented x86 machine, Xeon 8488C, 5-level paging)

- Clippy with warnings denied on wcontainers-x86 (all targets, and the core without `translate`) and
  -translator-test, `--no-deps`: clean; the core builds for `x86_64-unknown-none`; the x86 crate's tests pass.
- Native layer 1 with the far transfers and base writes answered (19:08 to 19:17 UTC, load 0.4 before, up to 25
  during with this run's other jobs): 2008 forms, 1,000 a form, seed 5150, 32 at once: 0 failing, the same 2,594
  known failures; 165,030 answers equal to the meaning's (127,030 in run 7); 531 s. Again with `xsave`/`xrstor`
  answered and the XSAVE constants shared (19:18 to 19:28 UTC, load 2.6 before, up to 16 during): the same, 526 s.
- Edge tests: far transfers, bases and clock 3,145 cases; xsave and xrstor 1,148; with run 7's 19,427 and
  cpuid's 154, all equal (again on the committed code, 19:20 UTC: 4 tests, 59 s).
- The literal check: no finding in a line this run wrote (three it found in answer.rs and fpu.rs during the run,
  two names defined twice and one repeating the abi crate's `WORD`, were removed). The error-message strings that
  earlier runs wrote in carry.rs and the test crate's native.rs remain.
- Controls (each in the rented machine's copy only): caught by the far-transfer test: the selector's address not
  wrapping under 0x67 ("[64, 67, ff, 29]"), `retf imm16` not releasing, a far call writing rip's slot before cs's,
  `wrfsbase` of ecx taking rcx's upper half, a return taking any privilege level; by the xsave test: TOP not placing
  the registers, the last-byte check left out, `xrstor` refusing a reserved MXCSR bit when no SSE or AVX is named,
  the initial x87 header 0, and (once a case had registers with the status word initial) the registers left out
  of the x87 component's in-use test. Not caught, and not seeable in a one-instruction case here: `iret` reading
  the rsp slot after testing ss (the rsp slot lies below ss's, and the data has no hole that makes one readable
  and the other not), and `xrstor` keeping the reserved byte after the abridged tag (the reference's state has no
  field for it; a later save would show it).

## What XN1 run 7 changed

- `answer(question, block, offer, constants, memory) -> Result<Notes, Fault>`: `answer::Memory` (read or write a
  range all or nothing, else the first address it may not reach), `Fault::Memory(address)` (the caller classifies it
  by the guest's `memory_fault`, page, stack or general protection, as every fault), `Notes` (a struct: AVX asked
  while told no, TF set by `popf`, AC asked for, a `mov` to ss holding a trap back) in place of `Note`;
  `answer::address(block, &Operand)` (base, index extended and shifted, the sum modulo 2^address_bits, then the
  segment base; rip is the instruction's own address) and `Place` (a general register with its width, a vector
  register, or memory).
- New questions: `Reciprocal` (rcpps, rcpss, rsqrtps, rsqrtss and their VEX forms: legacy keeps the upper halves,
  VEX.128 clears them, the scalar forms merge the other lanes, a legacy packed memory operand not 16-aligned is
  general protection before the read) by `answer::estimate`, the meaning's formula (`float::reciprocal`) in plain
  Rust with libm's `sqrt` and `rint`; `PopFlags`; `Descriptor` (with the table in x86/user_segments.rs, core, which
  descriptors.rs and segments.rs now read too); `SegmentRead`, `SegmentWrite` (no cs: iced decodes a `mov` to cs as
  no instruction), `SegmentPush`, `SegmentPop`, `FarPointer` (the offset read, then the selector after it, each
  faulting by itself; then the load, then the register).
- Shared constants named once: the float format constants and the estimate's bit counts in answer.rs (float.rs reads
  them), `QUIET32` and `DEFAULT32` in lib.rs (host.rs re-exports them), `POPPED_CONTROL` in answer.rs (meaning.rs
  reads it). `native::operand` (the memory operand as the shared `Operand`) is factored out of `stopping`, which
  uses it unchanged.
- Test crate: native.rs's `answered` runs `answer` over the case's own memory (`CaseMemory`: layer 1's data area and
  code area with their permissions, layer1.rs's `regions`, `code_area`, `start_data` and `changed` now crate-wide)
  and compares the data changed too; tests/layer1.rs
  `the_inside_answers_reciprocals_flags_and_descriptors_as_the_meaning_x86_64` (new): every exponent of both signs
  with eight fractions (interval edges, the quiet bit, the largest) through the eight reciprocal forms; `popf` and
  `popfw` of each bit, none and all, with DF, NT and ID clear and set before, and from read-only, unmapped and
  non-canonical stacks; `lar`/`lsl` at three widths, `verr`, `verw` of selectors 0 to 255 with the upper bits clear
  and set; `sgdt`, `sidt`, `smsw`, `sldt`, `str`, `lar` with memory operands in writable, read-only, partly mapped
  and non-canonical places, and `smsw`, `sldt`, `str` to eax, rax and ax; every segment move, push, pop, `lfs`,
  `lgs`, `lss` with selectors 0 to 0x80 and two more.

## XN1 run 7 results (rented x86 machine, Xeon 8488C, 5-level paging)

- Clippy with warnings denied on wcontainers-x86 (with and without `translate`) and -translator-test, all targets,
  `--no-deps`: clean. The x86 crate's tests: all pass.
- Native layer 1 with the reciprocals, `popf` and descriptor answers (18:23 UTC, load 0.3 before, up to 17 during):
  2008 forms, 1,000 a form, seed 5150, 32 at once: 0 failing, the same 2,594 known failures; 88,693 cases answered
  by the inside, of which 82,000 had a question and the answer equal to the meaning's (16,000 in run 6); 608 s.
  Again with the segment questions too (18:45 UTC, load 4.9 before, up to 18 during): 0 failing, the same 2,594
  known failures, 127,030 answers compared equal; 563 s.
- The new edge test: 19,427 cases equal (12,593 before the segment questions); with cpuid's test, 2 tests, 38 s.
- Controls, each in the rented machine's copy only, each failing the edge test: rsqrt's exponent parity not
  inverted ("[0f, 52, c1]: the answer ..."); `verw` taking every segment as writable; `popf`'s written bits not
  limited to the operand's size (failed only once the starting rflags had ID set, so the test sets it); ss accepting
  any selector ("[8e, d1]"); fs' base not cleared by a load (failed only once the cases started with nonzero bases,
  so they do: "[8e, e1]").

## What XN1 run 6 changed

- Core moves (plan 9; commits 28efa220, 1a84b81c): rustc_apfloat and astro-float-num are dependencies of the core,
  and lazy_static 1.5 with `spin_no_std` (already in the lock, its spin 0.9.9 too); x87.rs, transcendental.rs and
  root.rs build without `translate`; `x87::transcendental` and `above_exact` (they take iced's `Mnemonic`) and
  `inexact_power_product` (only `above_exact` uses it) stay behind it. `Unit::new` takes `&mut Consts` and the
  transcendental functions take it; the translate half keeps the per-thread cache (`with_constants`) around the x87
  hook and `above_exact`. The core builds for `x86_64-unknown-none` (cargo build, no error). The general register
  numbers `RAX` to `R15` moved from operand.rs (translate) to the core's lib.rs; operand.rs re-exports those it uses.
- `answer` (x86/answer.rs, core): `answer(Question, &mut X86Block, Offer, &mut Consts) -> Result<Note, Fault>`;
  `Question::{Cpuid, Xgetbv, X87(Code)}`, `Fault::{GeneralProtection, FloatException(raised)}`,
  `Note::AvxAskedAndToldNo` (the meaning's warning, for the inside to count). The translate half's
  `native::question(&Instruction)` gives the question; fpu.rs's `code` and `transcendental_code` build the unit's
  code once for the meaning and the question. Rip is the caller's to move on `Ok`.
- Tests: native layer 1 computes every answered case's outcome by `answer` where it has a question and requires it
  equal to the meaning's (`ANSWERS_CHECKED` in the summary line); tests/layer1.rs
  `the_inside_answers_cpuid_and_xgetbv_as_the_meaning_x86_64` (every leaf with each sub-leaf of its table and two
  past, leaves above the highest, `xgetbv` of four registers, upper halves set): 154 cases. Controls (remote copy
  only): the sub-leaf echo left out: the cpuid test fails; the pending exception check left out: native layer 1 of
  `fsin` and `fpatan` fails ("the inside's answer ... differs from the meaning's FloatException").

## XN1 run 6 results (rented x86 machine, Xeon 8488C, 5-level paging)

- Commits 1a84b81c to e034c9ef (other tracks' edits in the tree), 17:44 to 18:09 UTC: clippy with warnings denied on
  wcontainers-x86 (with and without `translate`) and -translator-test, all targets, `--no-deps`: clean;
  `cargo build -p wcontainers-x86 --no-default-features --target x86_64-unknown-none`: builds; the x86 crate's
  tests: all pass (the transcendental ones with the constants passed in).
- WebAssembly layer 1, 2008 forms, 300 a form, seed 5150, 32 at once (load 1.3 before, 23 after): 1587 forms fail
  on this machine, the same list after the core move and after the code's sharing (scratch run11-wasm-failing-
  before.txt and -after.txt); none is an x87 register form, and the x87 memory forms fail by 5-level paging (e.g.
  `fld dword [r13]`: the reference a page fault at 0xffff7ffffffffff3, the meaning general protection), as
  cloud/x86/x86-machine.md says layer 1 does here.
- Native layer 1, 2008 forms, 1,000 a form, seed 5150, 32 at once (load up to 23): 0 failing, the same 2,594 known
  failures as runs 3 to 5; 88,693 cases answered by the inside, of which the 16,000 transcendental runs (8 forms,
  1,000 cases, uncounted and counted) had `answer`'s outcome equal to the meaning's; 547 s.
- `the_inside_answers_cpuid_and_xgetbv_as_the_meaning_x86_64`: 154 cases, equal.
- `a_condition_holds_as_the_meaning_says_x86_64` (new, tests/layer1.rs: `setcc al` under WebAssembly for the 16
  conditions on all 32 combinations of CF, PF, ZF, SF and OF against `Condition::holds`): 512 cases, equal; with
  `holds` reading ZF for parity in the remote copy it fails ("Parity with 0x4").
- The literal check now reports 10,623 lines over the workspace, among them strings (error messages) in carry.rs and
  the test crate's native.rs that earlier runs wrote; none in a line this run wrote. Left for the run that takes
  the literal check up again.

## What XN1 run 5 changed

- Shared (commit bd1fdd92): `Native::test_loop_invariant` takes the way (ARM64's is `()`), since x86's moves reach
  the area on the way's segment; `HostCode::relocate_throughout` relocates a guest register from the current
  instruction's first byte, the count's add written before `carry` included. ARM64's tests, the translator's native
  tests and hardware-virtualization's build pass on the ARM64 machine.
- x86 (commits 55013584, 7ce8976f): `LoopInvariant` is `carry::Homes`. `Homes::of(part)` pairs every withheld
  register the part names with a general register the part never names, implicitly either (never rsp or rbp), or
  gives none where too few are free. `enter_homes` (the loop's "test"): for each pair the home's own value to its
  word of the register area (new `RegisterArea::homes`, `HOME_VALUES`), the withheld value into the home, then a
  `jmp rel32` to the copy (the trait's branch, its offset that of the rel32). In the copy each instruction is
  carried renamed (iced re-encodes it) and its map holds each withheld register in its home and each home's own
  value in its word, throughout; Kernel, Trap, FloatStatus, Area, a repeated string, `fldcw` and a RIP-relative
  operand naming a high byte are carried with the homes left around them; the branch back's not-taken way and
  every exit's call into the inside (after the linkable `jmp`, so a link skips it) leave the homes first.
- Tests (crates/wcontainers-x86/tests/native_output.rs): `a_loop_holds_its_withheld_registers_at_home` (the copy
  names no withheld register and reaches no area word; its map holds them in their homes); the sweep's
  `map_follows_the_moves`: from the piece's start along every branch and link, it follows each move between a host
  register and the area, gives a register or area word any other instruction writes the guest value the map gives
  it there (every other copy of that value then stale), and at each instruction checks every guest register is
  where the map says. It does not see a map wrong only where an instruction writes. An exit's call is no longer
  taken as one that comes back (the code after it is another way's).
- Controls (each in a remote copy only): the leave's store left out: native layer 2 237 of 2,000 failing, and
  the sweep and the named test fail; the enter's load left out, and `relocate_throughout` replaced by `relocate`:
  the sweep and the named test fail. Layer 3 at 32 programs did not see the first.

## XN1 run 5 results (rented x86 machine, Xeon 8488C, 5-level paging; sweeps on the ARM64 machine)

- Commit 7ce8976f (other tracks' edits in the tree), 17:05 to 17:38 UTC: clippy with warnings denied on
  wcontainers-x86, -guest-cpu, -arm64, -translator-test and the translator with `native`: clean; the x86 and
  guest-cpu crates' tests: all pass; native layer 2, seed 1, 10,000 cases: 0 failing, 3,432 faulted as on the CPU,
  115,358 blob runs (load 5 before); native layer 3, 1,000 Csmith programs, seed 1: 852 compared, 148 did not end in
  time on Linux, 0 failing; native layer 1, 2008 forms, 1,000 a form, seed 5150: 0 failing, the same 2,594 known
  failures as runs 3 and 4 (load up to 20, this run's 32 jobs).
- Sweep with the map check (ARM64 machine, load 7 to 12): seeds 0x51 and 0x2b17, 300,000 cases each (1,200,000
  pieces each): 0 broken; before the map check, seeds 0x51, 0x2b17, 0x5eed, 1,000,000 cases each: 0 broken.
- Benchmarks at full, best of 5, times the host's plain run (uncounted, counted), 16:56 UTC, load 2.6 falling to
  1.4: fib 1.25, 1.38; sieve 1.01, 0.99; hash 1.00, 1.00; sort 1.20, 1.23; interpret 1.23, 1.66; matrix 1.04, 1.11;
  walk 1.09, 1.05; search 1.40, 1.47; geometric mean 1.145 (the most 1.395), 1.216 (the most 1.658). With homes only
  in r8 to r15 (the first version, 16:48 UTC): matrix 1.75 (its inner loop names all three withheld registers and
  leaves only r13 of those free), geometric mean 1.218. Again at the end of the bar's runs (17:34 UTC, load 4.0
  falling to 1.6, bar.out in scratch run10/): fib 1.24, 1.37; sieve 1.01, 0.99; hash 1.00, 1.00; sort 1.21, 1.24;
  interpret 1.23, 1.65; matrix 1.04, 1.12; walk 1.01, 0.94; search 1.38, 1.48; geometric mean 1.133 (the most 1.384),
  1.202 (the most 1.652). Scratch run10/ also holds the piece dumper (dump.rs, which now follows return points too)
  and matrix's and search's listings (matrix-dump.txt).

## What XN1 run 4 changed

- Shared: `Patch` and `PATCH_BYTES` in gcpu/native.rs (plan 9); x86's `carry::retarget(code, exit, distance)` gives
  the patch that points an exit at code `distance` bytes from its first byte (its branch's rel32, or a `jmp rel32`
  over a settling add with `nop`s after), and `link` writes it.
- crates/wcontainers-translator-test/src/guest/x86_64/native_programs.rs (new, Linux x86-64 only): `run`,
  `run_timed` and `compare` (above). The blob's routines: the entries for Exit and Answer and one for every other
  kind (which ends the run, named); a system call site found by its return point flag-free (`lea`, `jrcxz`), which
  records the site and the count on standard error, loads the guest's r10 from the area where it is withheld, makes
  the call, stores r11 (the flags) there and gives rcx the next pc; a repeated string site, which records rcx, rsi and rdi before, runs the guest's own bytes, and records them after
  (DF read from rdi's direction, a host process keeping TF off); a fault in the pieces or a string's copy is the guest's (the default action put back, the
  instruction run again, so the blob ends by the same signal), at host pc 0 a lookup miss (an empty second-level
  word, the target in t), anywhere else the blob's own (named). The blob starts the program with the registers
  and flags the reference gives a blob, read at its start. Tests: tests/layer2.rs
  `natively_matches_the_cpu_x86_64` (layer 2's programs, the same cases as layer 2's), tests/layer3.rs
  `csmith_programs_match_linux_natively_x86_64`, each at none, a bound and full, uncounted and counted;
  tests/benchmarks.rs `measure_benchmarks_natively_x86_64` (ignored, a measurement).
- Controls (each made in the rented machine's copy, 100 layer 2 cases): the guest's r11 not stored after a system
  call: 31 failing; a repeated string's passes counted whole (not less the one the add counted): 3 failing (those
  with a rep string under the count); the site's lead not taken off the count: 61 failing.
- The slot form (carry.rs `on_slot`, plan 3.4). Native layer 1 at 100 cases a form found 12 forms where it is wrong,
  in two classes: `rol`/`ror r/m, imm8` (OF, which Intel leaves undefined, differs between the memory and register
  forms on the reference) and `pextrb`/`pextrw`/`extractps` and their VEX forms (memory of one byte, the register
  written whole). Both are kept out (undefined-result forms, and memory of another size than the register), and
  named in tests/native_output.rs `a_withheld_operand_is_its_slot`; the sweep accepts the slot form.
- A stand-in is loaded only where the guest reads the withheld register and stored back only where it writes it
  (carry.rs `reads_writes`); checked by native layer 1 (200 a form, 0 failing) and layer 2 (1,000 cases, 0
  failing), and the named tests count the moves.
- The withheld registers are named once: the stand-ins and both native blobs follow `COUNT`, `T`, `U` and
  `WITHHELD` (no hard-coded r10, r11 or r15 left in carry.rs).

## XN1 run 4 results (rented x86 machine, Xeon 8488C, 5-level paging)

- Native layer 2, seed 1, 500 cases, 32 at once (load 1.3 after): 0 failing, 5,676 blob runs, 129,298 pieces, 167
  ending by SIGSEGV as on the CPU, 97 s. Before the string sites, 21 of 500 failed, all at a repeated string under
  the count, and one on rcx clobbered by the site's search (fixed).
- Native layer 3, 8 Csmith programs (seed 1), 8 at once (load 260 from other users at the start, 67 after): 0
  failing, 366 blob runs, 79,082 pieces.
- Native layer 1 with the slot form: 2008 forms, 200 cases a form, seed 5150, 32 at once (load 9 to 13): 0 failing.
- The bar's runs at commit 1876776c (with other tracks' edits in the tree), 15:51 to 16:08 UTC, load 1.8 before, up
  to 20 during (this run's 32 jobs): native layer 2, seed 1, 10,000 cases, each at none, its bound and full,
  uncounted and counted: 0 failing, 3,432 ending by SIGSEGV as on the CPU, 115,358 blob runs, 2,671,676 pieces, 129
  s; native layer 3, 1,000 Csmith programs (seed 1): 852 compared the same way, 148 did not end in time on Linux, 0
  failing, 251 s; native layer 1, 2008 forms, 1,000 cases a form, seed 5150: 0 failing, the same 2,594 known
  failures as run 3, 601 s; the x86 and guest-cpu crates' tests: all pass.
- Again at commit 34e663bf (the lighter stand-ins in), 16:17 to 16:32 UTC, load 0.6 before, up to 19 during: clippy
  with warnings denied on wcontainers-x86, -guest-cpu, -translator-test and -arm64 clean; the x86 and guest-cpu
  crates' tests pass; native layer 1 (2008 forms, 1,000 a form, seed 5150) 0 failing, 2,594 known failures, 551 s;
  native layer 2 (seed 1, 10,000 cases) 0 failing, 118 s; native layer 3 (1,000 Csmith programs, seed 1) 852
  compared, 0 failing, 252 s.
- The first 10,000-case run found 2 failures, both the blob's own: a repeated string's record pushed the flags
  below an rsp at the stack's bottom (cases 1409, 9636); the record now reads DF from rdi's direction instead and
  touches no stack, and both pass. That run took over an hour at load 0.5 because every guest fault started the
  host's crash reporter (apport); the blob now makes itself non-dumpable (500 cases: 97 s before, 23 s after).
- The benchmark set at full, best of 5 (bench-run4a.out and bench-run4b.out in scratch; load 1.5 to 5), times the
  host's plain run (uncounted, counted): before the slot form fib 1.24, 1.16; sieve 1.01, 0.98; hash 1.02, 1.01;
  sort 1.28, 1.27; interpret 1.23, 1.66; matrix 6.20, 6.20; walk 1.04, 1.07; search 4.13, 4.08; geometric mean
  1.643, 1.687. With it: fib 1.25, 1.38; sieve 1.01, 0.99; hash 1.00, 1.00; sort 1.24, 1.27; interpret 1.23, 1.65;
  matrix 3.83, 3.81; walk 1.06, 1.04; search 1.98, 2.10; geometric mean 1.408, 1.489. With stand-ins loaded only
  where read and stored only where written (bench-run4c.out, 16:13 UTC, load 7.4 falling to 2.0): fib 1.25, 1.38;
  sieve 1.01, 0.99; hash 1.00, 1.00; sort 1.21, 1.24; interpret 1.22, 1.66; matrix 2.80, 2.87; walk 0.92, 1.02 (the
  host's own walk varies from 2.4 to 2.6 s); search 1.44, 1.49; geometric mean 1.274, 1.365. The bar (1.25, none above
  1.6) is not met. The blob times its run as the reference times a blob (from before the process is made to its
  end), so the prologue's few system calls and each guest system call's record are inside the time.

## Withheld sets (XN1 run 4 experiment, scratch run9/sets.sh and sets.out)

Each set (c, t, u) put into the rented machine's copy, checked by native layer 1 (30 cases a form, seed 5150) and
layer 2 (32 cases), all 0 failing, then the benchmark set at full uncounted, best of 3 (load 1.3 to 3.1), times the
host's: fib, sieve, hash, sort, interpret, matrix, walk, search; geometric mean (counted in brackets). Made with
the slot form but before stand-ins skipped their needless loads and stores.
- r10, r11, r15 (today's): 1.25, 1.01, 1.01, 1.23, 1.22, 3.82, 1.04, 1.86; 1.392 (1.476).
- r13, r14, r15: 2.26, 1.00, 1.00, 1.42, 1.23, 1.06, 1.04, 1.04; 1.208 (1.188).
- r12, r13, r15: 1.24, 1.01, 1.00, 1.50, 1.22, 2.85, 1.06, 1.05; 1.280 (1.313).
- r12, r14, r15: 2.26, 1.01, 1.00, 1.45, 1.23, 2.82, 0.97, 1.05; 1.358 (1.404).
- r8, r9, r15: 1.25, 2.78, 1.00, 1.29, 2.46, 3.20, 0.85, 0.78; 1.486 (1.573).
No set meets the bar (1.25 and none above 1.6): every register is some workload's hot one (leaf loops take r8 to
r11 first, recursion keeps r14 across its calls: fib's `push`/`pop` of the slot chain through memory). The set
stays r10, r11, r15 until the X9 programs' dynamic count exists (Go keeps r14 for its goroutine, static counts put
r12 and r13 above r10 and r11 over 13 files, plan 3.4): choosing by eight small kernels alone would be shaping the
output around them. The way past it is a piece's own homes: where a piece names a withheld register, the piece
keeps c, t and u in the registers it names least and that register's guest value in the area, swapped at its
entry and exits (a loop inside the piece pays once); that is the next speed step.

## What XN1 run 3 changed

- crates/wcontainers-translator-test/src/guest/x86_64/native.rs (new, Linux x86-64 only) and the test
  `every_form_matches_the_cpu_natively_x86_64` in tests/layer1.rs (`WC_LAYER1_CASES`, `WC_LAYER1_SEED`,
  `WC_LAYER1_JOBS`, `WC_LAYER1_FORMS` as layer 1's). A case's instruction is translated at look-ahead none for P
  (`translate_native_for`, `Way::Process`, the reference's `HostMatches`: 5-level, partial writes and undefined
  results as the meaning's, since this CPU is the reference) and run as a reference blob at 2^43: its prologue maps
  the data and code areas, the area (register area with the guest's r10, r11, r15 and GS base; the fixed part's
  entries pointing at unmapped addresses, one a kind; the first level's 2^15 lower-region words pointing into an
  unmapped second level at 2^44), installs a handler on its own stack, loads the state (`xrstor`, `popfq`, moves)
  and jumps to the piece. The handler writes the frame, the data area and the register area. How a run ends: an
  exit (the Exit call, also X87Control and FloatAfter; FloatBefore's entry is `jmp t`, a host process holding the
  float status in hardware), a fault in the piece, a lookup miss at the second level (by its host instruction),
  a call the inside answers or a first-level miss (the state must be the case's start, then the meaning's outcome
  stands for the inside's). The native outcome is reconciled as the reference's is (both are a CPU seen through a
  frame) and compared with the reference. Two classes are counted as known failures, not passes: the reference's
  5-level paging (a page fault where the 4-level meaning gives #GP, x86-machine.md) and a meaning-answered case where
  the meaning differs from the reference exactly as the WebAssembly layer 1 does (checked: `smsw [r15d]` fails
  there the same way). Cases with TF or AC set are not run (P keeps both off the hardware; stepping pieces not
  built), counted.
- Each case runs twice: without the count, and with it and TF set after the prologue, the handler returning at
  every trap in the piece: at each stop the state read through the map must be the start (before the commit point)
  or the final one (after), the data likewise, the count (r10 less the lead) 0 or the events done. A control (the
  expectation flipped in the machine's copy) fails 3 of 4 forms at host offset 0.
- Defects found by it and fixed in carry.rs and native.rs, with a named test (`what_layer_1_found_on_the_cpu`):
  a `gs:` operand based on rsp or rbp rebuilt as `[rsp/rbp + t]` took the stack segment's #SS where the guest
  takes #GP (139 of the first failures): t is now the base; a stand-in for a withheld base could be rbp
  (`cmpxchg8b [r15]`), and the high-byte form's address register too: never rbp; a `pop m` based on rsp on `gs:`
  formed its address before the pop: the pop's size is added; float status instructions (`ldmxcsr`, `fxsave`,
  `fnstenv`, ...) on `gs:` used the hardware's GS, our area: rebuilt as Area now; a bit test with a register offset
  on `gs:` under 0x67 lost the 32-bit wrap of the offset's part (the carried 0x67 finding's other case in the
  unchecked form): answered by the inside.

## XN1 run 3 results (rented x86 machine, Xeon 8488C, 5-level paging)

- Native layer 1, 2008 forms (every form layer 1 has), 1,000 cases a form, seed 5150, 32 at once, uncounted and
  counted (before the stepping check), load 0.3 before: 0 failing, 511.5 s; 2,594 known failures (91 form lines with
  the 5-level class, 11 with the meaning class: sldt, smsw, str (the host kernel's UMIP emulation), wrfsbase and
  wrgsbase; 11 transcendental forms as WebAssembly's). Ended (uncounted run): exit 1,416,015, fault 437,876,
  answered 88,693, trap 62,329, first-level miss 2,495, second-level miss 592.
- With the stepping check (commit a4551138), 2008 forms, 1,000 cases a form, seed 5150, 32 at once, load 25 to 34
  during it (this run's own jobs and other users): 0 failing, 511.3 s, the same 2,594 known failures and endings;
  no case had TF or AC set, so none was left unrun. Also seed 7, 8 cases a form: 0 failing. The x86 crate's and
  guest-cpu's tests and the translator's native lib tests in the same job: all pass.
- clippy with warnings denied on wcontainers-x86 and wcontainers-translator-test (all targets, `--no-deps`):
  clean. x86 crate's native tests: routes 2, output 18, carry 2: pass.

## What XN1 run 2 changed

- Shared (commit 0b3267fc): `Native` gains `type Way` and `type Target`; `route`, `carry`, `call`, `exit` take
  the way (and `route`, `carry` the target). `translate_native_for` takes both; `translate_native` stays for a
  guest whose way and target are `Default` (ARM64's are `()`), so the hardware-virtualization executor's calls
  are unchanged; its tests' `Arm64::route` calls were updated. ARM64's clippy, tests and the translator's native
  tests passed on the ARM64 machine; hardware-virtualization's lib and tests build.
- Shared (commit 179ca957): `Fixup::X87Pointer(u64)` (moved with the code like `Fixup::Address`; the ARM64
  inside's stop.rs takes it as nothing, since ARM64 never writes it) and `NativeReason::RepeatedStringCounted`.
- x86/native.rs: `Target { host, offer }` and `impl Native for X86_64`. x86/carry.rs: every route for the machine
  and P. Copy as the guest's bytes; an instruction naming r10, r11 or r15 re-encoded on stand-ins chosen by iced's
  register use (implicit ones included), saved to and loaded from the register area by `seg:` moves (counted
  `WithheldRegisterNamed`); Rebase (`mov t, imm64` an address place, `lea` likewise, `mov t32, t32` under 0x67);
  Area (the run-1 forms, a RIP-relative operand on the area through u, a `moffs` move through u, `rdgsbase` from
  the area); an instruction naming ah, ch, dh or bh whose operand is rebuilt takes its address in a register of
  the first eight (REX would forbid the byte register); Kernel, Trap, FloatStatus through `call` (machine `int
  32 + code`, P `lea t, [rip]; jmp qword ptr gs:[entries + 8 * code]`); Transfer: direct `jmp`/`call` by rel32
  exits (`call` pushes the guest return address from `mov t, imm64`), `jcc` rel32 linkable in place when the count
  is not ahead, else through a settling exit, `loop`/`jrcxz` by their rel8 over a short jump to an exit,
  returns and jumps and calls through a register or memory by the two-level mirror of plan 3.3; targets at or above
  2^47 answered by the inside. The count is `lea r10, [r10 + k]` with a 32-bit displacement, so a link's `jmp
  rel32` fits over a settling add (the rest filled with `nop`). area.rs: the register area (GS base, withheld
  values, stand-in values, FIP), the fixed part (entries), the first level at two pages.
- After a copied x87 instruction that is not a control one (fpu.rs's own `control`), `mov t, imm64; mov
  gs:[fip], t` with `Fixup::X87Pointer` over it.
- After a copied `fldcw` (commits 6855550d, 161ece86): `fnstcw` into the register area, `movzx`, `not`, `pext`
  of the six masks, rcx borrowed into u for `jrcxz`, and a call of the new kind `InsideCall::X87Control` when one
  is clear; no arithmetic flag written, nothing relocated at the call. The ARM64 inside's stop.rs and the
  hardware-virtualization test take the kind as unexpected. The SDM leaves C0 to C3 undefined after `fnstcw`:
  a host that changed them would change the guest's condition codes after a `fldcw` (not measured; the meaning
  leaves them).
- `DirectExit::condition` (commit 97b3f852): set by x86's linkable `jcc` exits from iced's condition code,
  `None` elsewhere and on ARM64; `branch_exit` takes it.
- x86/condition.rs (core, commit 5053d6bf): `Condition` and `Test` moved from flags.rs, with `ALL`, `from_code`
  and `holds`, unit-tested on all 16 conditions and 32 flag combinations against the SDM's table.
- The position map's `Operand` gains `segment` (`Fs`, `Gs`) and `address_bits` (commit 47b3dfc2; ARM64 flat, 64);
  x86's `stopping` gives `Access` for every memory operand but `lea`'s.
- A gather whose vector index is on the area's segment under 0x67 goes to the inside (the 32-bit `lea` cannot
  take a vector index); found by reading the carrying code, now a named test.
- The sweep also checks that nothing is relocated at any call into the inside and nothing relocated or pending
  at its return point.
- Tests (crates/wcontainers-x86/tests/native_output.rs): a seeded sweep of instruction mixes with branches, calls
  and returns, translated in both ways with and without the count, decoded back by iced: every piece decodes
  whole, every span, exit and address place is at an instruction start of its kind, no host `call`/`ret`, no
  RIP-relative access but P's `lea t, [rip]`, every copied instruction's bytes unchanged, every stand-in form the
  same code naming no withheld register; and twelve named shapes (return, call through a register, direct call,
  a loop linked in place, stand-ins, `gs:` rebuilt and `fs:` copied, `moffs` on `gs:`, RIP-relative, `syscall`
  per way, a trap's map, the count, FIP, a rep string). native_carry.rs now reads the translated piece.

## XN1 run 2 results (these tests run no host code, so they hold on either machine)

- Rented x86 machine (Xeon 8488C, load 0.08 before, 1.36 after), at commit ab6fc245 plus other tracks' edits:
  clippy with warnings denied on wcontainers-x86, wcontainers-guest-cpu, wcontainers-translator (native, all
  targets, `--no-deps`): clean; `cargo test -p wcontainers-x86 -p wcontainers-guest-cpu`: all pass (x86 lib 9,
  native_output 17, native_carry 2, native_routes 2, the others as before); the translator's native lib tests 7.
  The x86 machine did not answer SSH for part of the run, so the work in between was built and tested on the
  ARM64 machine.
- Rented ARM64 machine (Graviton4, load 10 to 36 from other tracks): the same clippy plus wcontainers-arm64 clean;
  `cargo check -p wcontainers-x86 --no-default-features`, hardware-virtualization's lib and tests and its inside:
  build; `cargo test -p wcontainers-guest-cpu -p wcontainers-arm64` and the translator's native tests: pass, after
  each shared change.
- native_output sweep: seed 0x5eed 20,000 cases (80,000 pieces), seed 0x2b17 500,000 cases (2,000,000 pieces,
  5,705,008 spans, before the fldcw and call-map checks), seed 0x51 4,000,000 cases (16,000,000 pieces, 45,570,238
  spans, release, all checks): 0 broken. native_routes seed 0x51, 2,000,000 cases: 0 broken. native_carry seed 11,
  1,000,000 cases: 0 differences.

## Departures from the plan (XN1 run 7)

- `answer` takes the guest's memory as a generic `answer::Memory` (a trait of the core's own, the plan gives none)
  and reports `Notes`, a plain struct, since `popf` may warn of two things at once.
- The table of Linux's user segments moved into the core (x86/user_segments.rs), not in plan 9's list: the
  descriptor and segment answers need it without `translate`.

## Departures from the plan (XN1 run 6)

- `answer` takes a `Question` (plan 1 and 3.5 give no signature): the inside learns the instruction from the
  position map's guest pc, and the translate half's `native::question` turns a decoded instruction into one, so the
  core needs no decoder. `Unit::new` takes the constants as a fourth argument (plan 9 says the transcendental
  functions take them; the unit is how they are reached).
- The general register numbers moved into the core (not in plan 9's list): `answer` needs them without `translate`.

## Departures from the plan (XN1 run 4)

- Layers 2 and 3 and the speed measurement run in a blob of the test crate's, not under P's executor: the same
  pieces, links and lookup, with the blob answering the guest's system calls directly and its repeated strings
  under the count by running the instruction and counting the meaning's passes offline (`string_passes`), where the
  inside would add them to c. A guest system call costs a record's `write` more than under P, inside the timed run.
- `retarget` takes the code (the settling add's length) and a distance from the exit's first byte, not from the end
  of its branch: the caller knows where an exit starts, not where its branch ends.
- A withheld register named as a whole operand reads or writes its slot directly (plan 3.4 now says so), a form the
  plan did not have; measured, and checked by native layer 1.

## Departures from the plan (XN1 run 3)

- Layer 1 runs natively before any executor exists (plan 8 waits for one), through a reference blob the test crate
  builds: the same translation, way and map an executor would use, the inside's answers taken from the meaning
  where the state at the call is the case's start. Its every-offset check uses the hardware's TF, which P itself
  never sets; that is the test's stop, not a way of running.
- Two classes of disagreement are counted as known failures by name (the reference's 5-level paging; the meaning's
  own difference from the reference on inside-answered forms), and cases with TF or AC set are not run natively.

## Departures from the plan (XN1 run 2)

- The trait's second type is `Target` (the host's matches and the process's offer), not `HostMatches`, since the
  route needs both.
- A rep string under the count goes to the inside (counted `RepeatedStringCounted`) instead of the inline
  after-code of plan 3.6: correct by construction, slow only with exact replay asked for, and no count stack. The
  after-code remains the plan's form if the inside's cost matters.
- FIP is stored after each copied x87 instruction, not once a run (the trait's `carry` sees no instructions
  after the current one yet).
- A call through a register builds its guest return address in rax saved to the area (the plan's measured
  stand-in form), until the pool exists.
- `DirectExit::retarget` holds, for x86, the byte offset of the branch's rel32 within it, until the shared field
  changes as plan 9 says.

## What XN1 run 1 changed

- The eighth check's findings, in the plan: (1, blocking, way I) a string with the guest's 0x67 goes to the inside
  in both checked forms, since its rdi side cannot be rebased (plan 4, routing test, way I tests with str67.c's
  case); code at XN2. (2) a segment operand with the guest's 0x67: the offset part is wrapped by `lea u32, [base32 +
  index32*scale + disp]` before the segment base is added (`[t + u]`), in the machine and P (plan 3.1) and in the
  exact form (plan 4), `67 d7` by a 32-bit `lea`; the wrapping form needed nothing (the sum modulo 2^32 is the
  same). Code for the machine and P done (`carry::area_operand`), the exact form's at XN2. (3) `maskmovq`
  named beside `maskmovdqu`: Kernel on the area's segment in the machine and P, re-encoded in I, in the routing
  test's list (plan 3.1, 4, 7). (4) the rep string count is a second implementation of the meaning's rule: named,
  with the full rule (each pass's own length, the cap, one element a pass for compares, `lods`, 0x67, DF and TF),
  its closed form (the check's case 91 passes), `string_passes` in the core, the after-code tested against the
  WebAssembly output, and no cost estimate (plan 3.6, 7, 9). (5) the unmasked x87 mode needs a plain-Rust twin of
  fpu.rs's instruction-level part: named as new code, x87_answer.rs (plan 3.1, 9).
- `Route::Area` added to gcpu/native.rs (ARM64's carrying code takes it as unreachable), as plan 9 now says.
- x86/strings.rs (core): `StringKind` and `BULK_STRING_BYTES` moved there from meaning.rs, and `string_passes`.
- x86/native.rs: `Way` (machine, process, in process with its check), `HostMatches` (paging depth, partial writes,
  undefined results), `route` for every decoded form.
- x86/area.rs (core): the register area's first word, the guest's GS base; x86/carry.rs: the withheld registers
  and `area_operand`, built on iced's encoder (feature `encoder` added).
- Tests: tests/native_routes.rs (a seeded sweep of instruction-shaped bytes, every decoded form in all four ways on
  two hosts against the plan's rules, and 54 named forms); tests/native_carry.rs (the check's gs67 case, and a
  seeded sweep of operand shapes, both address sizes, comparing the address the host code reaches, run through
  iced's `virtual_address`, with the meaning's).

## XN1 results (rented x86 machine, Xeon 8488C, load 0.1 to 0.7, commit of this run plus other tracks' edits)

- clippy with warnings denied on wcontainers-x86, wcontainers-guest-cpu, wcontainers-arm64 (all targets, `--no-deps`,
  since the kernel interface crate, not ours, has two type-complexity findings): clean. `cargo check -p
  wcontainers-x86 --no-default-features` (the core alone, which the kernel takes): builds.
- `cargo test -p wcontainers-x86 -p wcontainers-guest-cpu`: 37 tests, 0 failed.
- native_routes: seed 0x5eed 200,000 cases and seed 0x2b17 2,000,000 cases of instruction-shaped bytes, each
  decoded form in 4 ways on 2 hosts (15,999,984 routes in the second: Copy 4,003,401, Nothing 107,840, Rebase
  64,464, Transfer 750,824, Kernel 820,723, FloatStatus 10,576, Trap 9,740,928, Area 501,228): 0 broken; 54
  named forms right.
- native_carry: the gs67 case reaches 0x300000010 (and without 0x67 0x400000010); operand shapes, seed 0x5eed
  100,000 cases and seed 11 1,000,000 cases: 0 differences from the meaning's address.
- While this run worked, crates/executor/implementation/host-process (another track's, uncommitted) named a
  missing test file, which stops every cargo command in the tree; the runs above put an empty file there in the
  rented machine's copy only.

## How to run XN1's tests

`orchestrator/cloud/x86-run <checkout> --name x86-native-output -- 'cargo test -q -p wcontainers-x86 --test
native_routes --test native_carry -- --nocapture'`; the sweeps take `X86_ROUTE_SEED`, `X86_ROUTE_CASES`,
`X86_CARRY_SEED` and `X86_CARRY_CASES` (decimal, or hex with 0x for the route seed); `cargo test -p
wcontainers-x86 --lib strings` for the pass rule.

Native layers 2 and 3 and the benchmark set (rented x86 machine): `WC_LAYER2_CASES=10000 WC_LAYER2_SEED=1
WC_LAYER2_JOBS=32 cargo test -q -p wcontainers-translator-test --test layer2 natively_matches_the_cpu_x86_64 --
--nocapture` (summary line "layer 2 natively"; `WC_LAYER2_FROM` replays a case); `WC_LAYER3_CASES=<n>
WC_LAYER3_JOBS=<j> cargo test ... --test layer3 csmith_programs_match_linux_natively_x86_64 -- --nocapture`;
`WC_BENCH_RUNS=5 cargo test ... --test benchmarks measure_benchmarks_natively_x86_64 -- --ignored --nocapture`.
Scratch run9/ holds the piece dumper used to read the benchmark's code (dump.rs, sent into the copy as a test
file) and the withheld-set experiment (withheld.sh, sets.sh, sets.out).

Round 3 run 2's tests (rented x86 machine, seconds each but the strings at 1,000 cases): `cargo test -q -p
wcontainers-translator-test --test layer1 the_shapes_of_the_plan -- --nocapture`; `... --test layer2
branches_into_an_instruction`; `WC_STRINGS_CASES=<n> WC_STRINGS_SEED=<s> WC_STRINGS_JOBS=<j> ... --test layer2
repeated_strings_count -- --nocapture` (summary line "repeated strings natively"). Do not name a shell variable
STRINGS in a script run under the Nix shell: stdenv sets it to the `strings` tool.

Native layer 1 (one command, on the rented x86 machine, which is its own reference; about 9 minutes without the
stepping run at 32 jobs): `x86-run <checkout> --name x86-native-output --jobs 32 -- 'WC_LAYER1_CASES=1000
WC_LAYER1_SEED=5150 WC_LAYER1_JOBS=32 cargo test -q -p wcontainers-translator-test --test layer1
every_form_matches_the_cpu_natively_x86_64 -- --nocapture'`; `WC_LAYER1_FORMS=<names>` for some forms. The
summary lines start "layer 1 natively".

The inside's answers against the meaning (rented x86 machine, about a minute): `cargo test -q -p
wcontainers-translator-test --test layer1 the_inside_answers -- --nocapture` (four tests: cpuid and xgetbv; the
reciprocals, flags, descriptors and segment registers; far transfers, bases and the clock; xsave and xrstor).
Scratch run9/controls.sh and controls-xstate.sh are run 8's controls, sent inline with `--no-sync`.

The second call against the meaning (rented x86 machine, seconds): `cargo test -q -p wcontainers-translator-test
--test layer1 the_second_call -- --nocapture`. Scratch xn1r9/controls-float.sh is run 9's controls, sent the same
way.

## Departures from the plan (XN1)

- `route` takes the process's `Offer` beside the way and the host's matches (plan 9 gives `route(way, i, host)`),
  since a form of a feature the process was not told of takes the Trap route and the offer says which.
- An operand on the thread area's segment in the machine and P runs on FS holding the guest's GS base (round 3 run
  2, the ninth check's 0x67 crossing): the plan's rebuild on `[t + u]` could not wrap each part of an access past
  offset 2^32 and left the implicit forms with no answer. Plan 3.1 now says this.

## What run 8 changed (seventh check), finding by finding

- Speculation in way I (blocking): the exact form's access is now the wrapping form's, cut to the window by its
  addressing mode (address-size prefix, `gs:[t32]`), after either check; the bit tests are re-encoded the same way
  (the hardware wraps t32 plus the offset's part, which is the checked A); strings cut rsi and rdi with `mov esi,
  esi` and `mov edi, edi` after the probe and before the rebase; `maskmovdqu` and `xlat` take the address-size
  prefix in both forms; `pushf` reads the flags on the count stack and pushes them through `access`, so no rsp
  rebase remains. A new Speculation item says what this covers and why, the routing test fails on an uncut access,
  section 7 has the bounds-check-bypass test for every access shape and XN2's bar includes it. Measured
  (run8/specx3.c, two seeds, 100,000 rounds each): the uncut live-flags forms read the secret 99,997 to 99,999
  times, the uncut dead-flags forms 4 to 7, every cut form 0 (plan 4, 7, 8).
- The no_std core (blocking): `Condition`, the x87 unit and the transcendental answers are translate-only today and
  transcendental.rs uses std; section 9 now lists their move into the core as shared-code changes (a new
  condition.rs, the x87 unit, transcendental.rs and root.rs moved, the `Mnemonic` helpers left behind `translate`,
  the `thread_local!` cache passed in as `&mut Consts`). Measured (run8/nostd): rustc_apfloat 0.2.3 and
  astro-float-num 0.3.7 build for `x86_64-unknown-none` only with lazy_static's `spin_no_std` feature, so the core
  gains that dependency line (plan 1, 3.1, 3.2, 3.5, 9).
- SA_RESTORER (minor): P's handlers are installed with it and a never-reached restorer; why (raw `rt_sigaction`, the
  check's bt16.c) and that this track's probes went through glibc (plan 3.7).
- The unmasked x87 mode across saves (minor): saves in that mode take the control and status words from the block,
  loads put them back there and keep the hardware's masked, the thread or child stays in the mode; new shapes for
  a switch, a returning handler and `clone` (plan 3.1, 7).
- X87Control's two positions (minor): before an MMX instruction it resumes at that instruction, after a `fldcw` at
  the next; the map records per position and XN1's map test checks both (plan 3.7, 7).
- The guest's address-size prefix on a bit test (minor): A is formed with a 32-bit `lea`, wrapping as the meaning
  does; the check's case (`67 0f a3 08`, eax 0xfffffff0, ecx 0x100, reaching 0x10) is a way I test and was run in
  run8/btcut.c (plan 4, 7).
- c from the frame's copy in P (minor): named as P's difference, changing only that process's counts (plan 3.7, 5).
- The stale coordination item (minor): removed; the P/I plan's 3.10 already takes the mirror with no slot table.
- The 16-bit bit-test sequences (no defect): the check's measurement recorded in plan 4.

## What run 7 changed (sixth check), finding by finding

- Bit tests in way I's exact form (blocking): `bt`, `bts`, `btr`, `btc` on memory with a register offset reach A = t
  + (w/8) * (offset SAR log2 w); the check and the probe are now made on A, computed flag-free in u (`mov u32, 6;
  sarx; lea` for 64-bit, with `movsxd` for 32-bit, `movsx` and a count register for 16-bit), the instruction
  re-encoded on `gs:[t]`. The wrapping form needed nothing. The guards claim and the routing test now name every
  form whose reach is not its operand's address. Measured (run7/btseq.c): 4,000,000 seeded cases, the sequence gave
  the meaning's A every time with no flag changed, and `lock bts` on `gs:[t]` set exactly the bit at B + A. New way
  I tests with offsets up to 2^60 (plan 4, 7).
- Saved return points across a dropped piece (blocking): every drop converts, before the piece's map goes and with
  the memory's other threads stopped, each saved t in the piece (the machine's `iretq` frame, P's request record,
  I's thread record) into its guest pc and count, and the thread leaves through the lookup; space is reused only
  after that. Per way how the stop covers a thread in the inside (floor lock, embedder's records, I's drop gate).
  New shape tests: a thread in a `futex` wait while its page is rewritten, unmapped, unprotected, replaced and the
  space reused (plan 3.2, 3.6, 3.7, 7).
- The vector image's place (minor): rdx (the context the host kernel passes) plus a fixed delta, not a place below
  the stack's top. Measured (run7/altdelta.c): 448 bytes for rsp on the program's stack and for 128 rsp values
  inside the alternate stack, where the distance from the top ranged over -68,544 to -68,672. A guest rsp near the
  alternate stack's bottom killed the process by SIGSEGV: named for P (plan 3.7, 5).
- The run-6 handler probe (minor): it never loaded the copy's r10 and r11, so that part of its hostile test could not
  fail. Measured again (run7/presume7.c) restoring all sixteen registers and leaving through a read-only word to a
  lookup entry reading the pc from the register area: same results, 924 to 931 ns (plan 3.7).
- The `fldcw` check in P (minor): the register-area word is sibling-writable in P, so the race is not closed there;
  named as P's difference, inside the guest's own process (plan 3.1, 5).
- The `jcc` condition (minor): `Condition::holds` in plain Rust for the floor and I, the flags table and a 16-entry
  `jcc` table in P's assembly, a differential test of the three against the meaning (plan 3.2, 7, 9).
- MMX in the unmasked x87 mode (minor): the register stack stays in the hardware (control word masked there), x87
  instructions go to the x87 unit, MMX instructions stay copied after an X87Control call that raises a pending #MF
  (plan 3.1, 9).
- XSAVE routing (minor): keyed on the offer's XSAVE and AVX bits apart; XSAVE without AVX is offered with the
  576-byte image (plan 3.1).
- The inside-call kinds (minor): `X87Control`, `Step` and `Stop` given as code, `ALL` 11 (plan 9).
- The handler's choice on an `int3` hit (minor): it also reads the copy's rip and `si_code`, which siblings can
  write; why that is harmless is stated (plan 3.2).
- The vDSO (minor): no x86-64 vDSO exists (the kernel's is ARM64's one-function image); asked of the kernel track,
  with the P/I plan's "clock_gettime is a system call" as the case without one (plan 3.7).
- The header (minor): run 7, six checks.

## What run 6 changed (fifth check), finding by finding

- A link's `int3` hit on a `jcc` exit (blocking): the handler now does what the branch would have done: a `jmp`
  exit resumes at its unlinked path, a `jcc` exit evaluates its condition on the interrupted rflags (the meaning's
  condition function) and resumes at the unlinked path or after the `jcc`, as Linux's `int3_emulate_jcc`;
  `DirectExit` gains `condition: Option<u8>` (plan 3.2, 9). Measured (run6/jcc16.out): all 16 conditions, seeded
  flags, 20,000 links each, 16 and 4 workers: 0 wrong arrivals; the check's form (always the unlinked path) had
  50,983 and 142,005.
- The flags table in every way (blocking): it is P's alone. In the machine and I the carrying code's `popfq` reads
  the word its `pushfq` wrote, in memory no other thread reaches, so the hardware TF (the guest's there) survives a
  rep string count; a new test steps a thread through `rep movsb` and ten one-byte instructions in each way (plan
  3.6, 7). I's routine's last `popfq` puts the guest's TF and AC back.
- t after a stop in P's stub (blocking): every way out of the stub is `jmp qword ptr gs:[resume]`, a word of the
  thread area's read-only part that only the embedder writes, after checking in its position map that t is the
  return point of a call of the kind asked, in a live piece of this process; a t from the handler's copy is checked
  the same way, so a sibling can choose only another call return point of its own process, where the guest's state
  is its registers and register area alone (no fix-up pending, nothing relocated at any call; the FIP store comes
  before a call; XN1's map test checks it). Other words the stub uses after a stop come from the read-only part
  again. A new hostile-guest test edits the copy's t in each step of the stub (plan 2, 3.6, 3.7, 7).
- UMIP (minor): the host's kernel emulates all five of `sgdt`, `sidt`, `sldt`, `str`, `smsw` for a 64-bit process
  (the check's umip.c); the routing was already Kernel (plan 3.1).
- Linux's function names (minor): `smp_text_poke_batch_add`, `smp_text_poke_batch_finish`,
  `smp_text_poke_int3_handler` (plan 3.2).
- The XSAVE family (minor): Kernel only with the AVX setting (`xsave`, `xrstor`); without it they are outside the
  offer (XSAVE is in the setting's group), and `xsavec`, `xsaveopt` are outside it always: Trap (plan 3.1).
- The check after `fldcw` (minor): `fnstcw` into a register-area word, then the test on that word, so a sibling's
  store to the operand cannot hide an unmasked exception (plan 3.1).
- The handler measured was not the one specified (minor): measured again, all in assembly with no call or push on
  the alternate stack, the image read at a fixed place below the top (replaced in run 7 by rdx plus a delta), every
  deciding word read-only, the resume through a read-only table; it did not load the copy's r10 and r11 (run 7's
  probe does) (plan 3.7).
- rflags in the count stack word (minor): the word sits at a fixed offset of the register page, so the map names
  it with the existing `Held::Page`, and rflags gets the x86 crate's map number 16; the passes are added by one
  `lea` and `RepPasses` covers the span up to it (plan 3.6, 9).

## What run 5 changed (fourth check), finding by finding

- P's `popfq` from a word siblings can write (blocking): every `popfq` of the carrying code and the stub now reads
  a word of the flags table, a read-only page of 512 words indexed by `pext` of the value read once into a
  register (CF PF AF ZF SF DF OF NT ID; TF and AC clear in every word). Measured (run5/flags.out): with a sibling
  storing 0x40302 into the count-stack slot, the old form set TF and AC 6 times each in 20,000,000 rounds, the
  table form 0 in each of 3 runs; it costs about 0.9 ns a round more (9.12 to 9.18 against 8.25 to 8.26) (plan
  3.6).
- Stops nested in P's resume (blocking): the handler now follows the P/I plan's 3.6: installed with `SA_NODEFER`
  and `sa_mask` the stop signal, the stub thread blocking all but the stop and the synchronous faults, rip
  classified by the position map or the stub's own ranges (the tail takes the exit already written, from the
  copy), rsp moved off the alternate stack before the unblock. Measured: 200,000 stops flooded while another
  sibling ORs TF and AC into the copy, 3 runs: 53,145 to 62,085 taken in the tail, the deepest frame 3 KiB, the
  guest loop's flags, r12..r14 and ymm7 kept; a round trip 948 to 953 ns against 1,073 to 1,080 with
  `rt_sigreturn` (plan 3.7).
- The mask from the frame (minor): the handler unblocks the stop signal alone, its set a word of the stub's
  read-only page; the P/I plan's 10-second kill stays for a stub that blocks it all the same (plan 3.7).
- SIGTRAP and SIGBUS in P (minor): a SIGTRAP is a link's `int3` only with `SI_KERNEL` at an exit of the batch; in P
  TF and AC are never set, so `TRAP_TRACE` or `BUS_ADRALN` is a fault of ours, the thread ended, counted
  "unexpected host trap" (plan 3.2).
- FIP, FDP, FOP (minor): the hardware's are never read. FIP is stored in the register area after a run's last
  non-control x87 instruction, with a fix-up `X87Pointer` before it; a copied `fldcw` is followed by a flag-free
  check (or a call where the flags are live), and a thread with an unmasked x87 exception runs its x87 and MMX
  instructions through the meaning's x87 unit in the inside, which sets FDP and FOP (plan 3.1, 7, 9).
- The rep string twin (minor): it adds nothing to c, does not store rcx again, and counts the whole instruction's
  passes from the original's word; a fault inside counts the passes completed before the faulting element (plan
  3.7).
- P's link syncs (minor): registration at the stub's start named; the membarrier is a command to a stub thread in a
  kernel entry, or to one the embedder stops first. Measured across processes (run5/plink.out): 14.92 to 79.79 µs
  a link with a stub thread waiting (0 to 64 running threads), 15.81 to 231.34 µs when one must be stopped first
  (1 to 64) (plan 3.2).
- Run 4's trap counts (minor): the count was not atomic, so run 4's figures were lower bounds, and the check is the
  workers' `ud2` with exit 0, not the printed line. Re-run with an atomic count (run5/patch2.c): 0, 115,878,
  321,673 and 1,191,515 traps (plan 3.2).

## What run 4 changed (third check), finding by finding

- Calls read the code area (blocking): the guest return address is pushed from a pool, a read-only,
  non-executable range at one fixed distance from the code area, so P's execute-only code area holds. Measured
  (run4/calls-resume.out): fib(30) through `call r`, pool outside execute-only code 1.120
  against plain call/ret, pool in the code page 1.119, stand-in register 1.146, `push imm32` plus high-half store
  3.271 (plan 2, 3.2, 3.3).
- Unsynchronized cross-modifying code (blocking): links are now made by Linux's `int3` protocol in batches, three
  syncs a batch (membarrier SYNC_CORE in P and I, interrupts in the machine), new code reached only after the first
  sync; an `int3` hit resumes at the exit's unlinked path. Measured (run4/patch.out): 0 to 64 threads running the
  exit while it was linked 20,000 times, every turn on a target (checked by each worker's `ud2`, exit 0), every
  trap resumed; one membarrier 0.65 to 5.88 µs, one link 1.90 to 69.05 µs (plan 3.2).
- The machine's area in the lower half (blocking): put back where docs/native.md has it, the upper canonical half,
  user-accessible pages, guest ceiling 2^47; its readable pages named in plan section 5 as native.md 7.2's
  difference (decision 4) (plan 2, 5, 6). So it is no departure.
- Minor: `exit` takes the way and `retarget` (returning a `Patch`) replaces `link`; `Held::InScratch` dropped for
  the existing `Relocated` (plan 9). `syscall` per way, and the floor writes the guest's r11 slot with rflags (plan
  3.1, 3.6). XRSTOR's mask is the offered groups, measured with the full handler (943 to 950 ns against 1,080 to
  1,088; a PKRU edit in the frame left PKRU unchanged with mask 7 and killed the process with the frame's mask)
  (plan 3.7). t may be read from the copy as a guest value only (3.7). TF and AC cleared by I's routine before
  Rust code and by P's stub when it puts rflags back (3.6; P's form replaced by the flags table in run 5). The rep
  string pass count until ruled: inline, out of line, `pushfq` on the area's count stack, a `RepPasses` fix-up; cost expected, measured at XN1 (3.6, 9).
  Register-group tests per way and a new P thread's vector registers loaded by XRSTOR (3.6, 7, 8). Shadow stacks: P
  refuses a stub thread with one, I resumes by `rt_sigreturn` (3.1, 3.7). The exactness figure re-measured with
  exactly 2^15 first-level words (21 targets fault on the first level, 15 on the second, t kept at all) (3.3).

## What run 3 changed (second check), finding by finding

- P resumed through `rt_sigreturn` (blocking): a sibling thread can rewrite the frame's rip on the alternate stack;
  measured 198,432 of 200,000 signals returned to the sibling's address. Now P's handler resumes by its own jump
  (mask by `rt_sigprocmask`, vector state by `XRSTOR` from its copy, since a handler starts with the vector state
  reset); measured working and cheaper, 925 against 1,089 ns a round trip (plan 3.7).
- Guest TF in P (blocking): the silent resume mid-sequence would take t and u from that editable frame, and a
  chosen u sends the second-level `jmp` anywhere. P now keeps the hardware TF clear and runs a stepping thread on
  stepping pieces (the request's `step`; plan 3.6, 9).
- The machine's per-processor areas (blocking): mapped at distinct addresses in a process's tables, a thread of
  process A could write the register area of a thread of process B on another processor. Now one virtual address
  mapped per processor through its own top-level entry (plan 2).
- Minor: I's host signal handler starts with FS on the thread area (measured), so it sets the host's FS first
  (plan 3.7); `wrfsbase` is not copied in P on a 5-level host, which accepted 2^47, 2^50 and 0xff00000000000000
  (measured, plan 3.1); the reserved-NOP space (`rdsspq` and the rest) is Nothing, never Copy; strings and `xlat`
  on the area's segment go to the inside; `jrcxz` is rel8, so the replay check's layout is given, and the check
  sits before its run's count add; way I's `call r/m` takes stand-ins, `pushf` rebases rsp, `push m`/`pop m` go
  through t; the stub moves t (r11) and c (r10) before a host `syscall`; host requirements named (BMI2, FSGSBASE,
  XSAVEC); a sibling's store into another thread's register area named for P (plan 5); the vDSO clock's cost and
  I's first-level remap with busy runners given to XN1 and XN2 as measurements; sigreturn's other cs values.

## What run 2 changed (first check), finding by finding

- Top-bits bound wrapped (blocking): replaced. The first level is indexed by bits 32..63 (2^32 words, 32 GiB reserved
  per thread area, 256 KiB mapped), so every target at or above 2^47 faults on the first-level read with t intact;
  no bound, no add (plan 3.3). Measured exact over 37 targets and cheaper than run 1's form.
- Kernel-half targets (blocking): the inside now answers by the meaning's 4-level rule: not canonical is general
  protection at the transfer with rsp not popped; a canonical upper-half target, vsyscall included, is a page fault
  at the target with rip there and the pop or push done (plan 3.3, 3.7). The first-level read is placed before the
  commit (and before a call's push) so nothing is to undo.
- FS/GS bases (blocking): the thread area is on GS in the machine and P and on FS in I; the guest's FS is the
  hardware's in the machine and P, so `fs:`, `rdfsbase`, `wrfsbase` are copied there. The area segment's operands
  and base instructions are routed (`rd*base` inline from the register area, `wr*base` to the inside, selector reads
  to the inside); the static routing test is per way with this plan's classes and fails if any is copied (plan 2,
  3.1, 7). This also removes run 1's extra load on every guest `fs:` operand in the machine and P (the dynamic count
  the check asked for is no longer leaned on).
- Strings in I (blocking): the start (rdi and rsi) is bounded by the flag-free probe before rdi is rebased; a run
  from inside the window faults on one guard page; `maskmovdqu` is re-encoded on `gs:` (plan 4).
- Host return address on a guest-writable stack in P (blocking): the call into the inside in P and I is `lea t,
  [rip]; jmp seg:[entry + 8*kind]`, nothing pushed; the routine keeps t in a register (P) or its own memory (I)
  (plan 3.6).
- Replay (blocking): stop word in the area's read-only part, `stop` in the translation request, a flag-free check
  with rcx borrowed into t and `jrcxz`, rep strings run pass by pass by the inside in a replay piece (plan 3.7, 9).
- Partial writes (blocking): `enter` with a level, `fstp m80` and `fbstp` do leave partial writes (measured again);
  they are routed by the probe's match like undefined flags; the masked stores measured whole (plan 3.6).
- Minor: XSAVE sizes corrected (832 and 576 bytes against 11,008); the register-count reasoning restated with the
  files that disagree; the partial-write results now in results/partial.out; Rust in the inside in I saves the
  offered groups with XSAVEC; the P/I plan reconciled (below); the area's fields are disp32 offsets from the segment
  base, no address place; the stack cost in I declared unmeasured and given to XN2 with the alternative; self-modifying
  code with other threads, locking order, the kernel's writes, shared mappings and unmap or execute loss; sigreturn
  with cs 0x23; pass bars with figures; the miss routine in empty second-level entries and the memory bound.

## Measured

- Speed again, by the ninth check (rented x86 machine, Xeon 8488C, 2026-11-23 21:10 to 21:12 UTC, pinned to cores
  0-9, best of 5, load 9 to 27, the code of run 10): uncounted fib 1.258, sieve 1.008, hash 1.015, sort 1.377,
  interpret 0.982, matrix 1.074, walk 0.881, search 1.395, geometric mean 1.110, worst 1.395 (search); counted
  geometric mean 1.231, worst 1.489 (search). Single programs move between runs on this shared machine (run 5 had
  sort 1.20 and interpret 1.23); the bar holds in both.

All on the rented x86 machine (Xeon Platinum 8488C, Sapphire Rapids, 96 processors, Linux 7.0.0-1018-aws, 5-level
paging, AT_HWCAP2 0x2 = user FSGSBASE, vsyscall execute-only, mmap_min_addr 65536), probes copied there with
`x86-run --ssh`, timings pinned to processor 5.
- Run 1, static shapes (probes/shapes86.py, results/shapes86.out; 13 files, 19.2 million instructions; load 1.3 to
  2.8): RIP-relative `lea` 24.5 and other 22.0 per 1000; `fs:` 1.22 (glibc 12.7); `gs:` 0.00; rep strings 0.22;
  r8..r15 named per 1000: r10 13.9, r15 15.0, r11 17.1, r9 19.1, r14 21.3, r13 21.8, r8 28.6, r12 30.8, with files
  that disagree (libm r15 2.9, libz r15 50.7, the Go daemon reversing r12..r15); memory operands where the flags may
  be live 20.9%.
- Run 1, return lookups (probes/returns.S and returns.c, fib(30), 3 runs of 21 alternating rounds, `rdtsc`; load 1.4
  to 7.2; results/returns.out, returns2.out): against plain call/ret, two-level mirror (16-bit index) 1.120 to 1.133,
  cache with `cmp` 1.17 to 1.39, flag-free cache 1.52 to 1.62.
- Run 2, exact lookups (probes/mirror3.S and mirror3.c, results/mirror3.out; load 9.4 at the start, 2.4 a minute
  later): over 37 targets, the 32-bit index, the `pext` index and the saved-and-bounded form reach no wrong piece and
  keep the target at every fault; the 16-bit index reaches a wrong piece for 4 (bits 61..63). fib(30), 3 runs of 21
  rounds: 32-bit index 1.065 to 1.070, 16-bit 1.112 to 1.117, `pext` 1.119 to 1.123, saved-and-bounded 1.265 to
  1.272.
- Run 2, partial writes (probes/partial86.c, partial3.c and check-xn0/partial2.c, results/partial.out; load 2.4):
  none for `mov` 8 bytes (21 cases), `movdqu` (45), `vmovdqu` 32 bytes (93), `fxsave64` (93), `xsave64` (24),
  `fnsave` (107), `vmaskmovps` xmm and ymm (15, 31), `vpmaskmovq` ymm (31), `maskmovdqu` (15); loads changed no
  register (45, 93). Partial: `enter 0,3` (6, 14, 22 of 8, 16, 24 bytes), `fstp m80` (8 bytes before the fault, 2 of 9
  offsets), `fbstp` (1 byte, 2 of 9).
- Run 3, signals (check-xn0b/psig.c, psig.out; load 0.17, `gcc -O2 -mavx2 -mfsgsbase -fno-stack-protector`,
  main thread on processor 5, sibling on 6): a sibling storing one address into the frame's rip slot in a loop
  diverted 198,432 of 200,000 `rt_sigreturn`s; ymm7 (all ones before) read zero at handler entry and all ones after
  `rt_sigreturn`; resuming by jump (`rt_sigprocmask`, `XRSTOR` with the frame's xfeatures) kept ymm7 and handled all
  200,000; a `tgkill` to itself and back, 3 rounds of 200,000: 1,088 to 1,089 ns with `rt_sigreturn`, 923 to 927 ns
  by jump; the FS base at handler entry is the one set before the signal.
- Run 3, FS bases (check-xn0b/wrfs.c, wrfs.out; load 0.05): `wrfsbase` accepts 2^46, 2^47, 2^50,
  0xff00000000000000, 0xffff800000000000, 0xfff0000000000000 and faults on 2^56 and 2^57 (5-level canonical).
- Run 4 (scratch run4/; load 0.04 to 0.12; `gcc -O2`, one pinned processor where timed):
  - calls.c and calls.S, calls-resume.out: the 32-bit-index lookup with 2^15 first-level words mapped, 37
    targets, none wrong, t kept at every fault, 21 on the first level (all at or above 2^47), 15 on the second.
    fib(30), every guest call `call r`, 21 rounds, median against plain call/ret: pool in the code page
    (read-execute) 1.119, pool in a read-only page outside execute-only code 1.120, stand-in 1.146, `push imm32` and
    `mov dword [rsp+4]` 3.271.
  - resume.c, calls-resume.out: P's full resume by jump (frame copied, mask 7, flags with TF and AC cleared, every
    general register and rsp from the copy, jump through a table word): ymm7 and r12..r14 kept; 943 to 950 ns
    against 1,080 to 1,088 with `rt_sigreturn`, 3 rounds of 200,000; a PKRU edit to 0x55555555 in the frame left
    PKRU 0x55555554 with mask 7, and killed the child by SIGSEGV with the frame's own mask.
  - patch.c, patch.out: the `int3` link protocol with membarrier SYNC_CORE, 20,000 links, threads on their own
    processors running the exit: 0, 4, 16, 64 threads: membarrier 0.65, 1.48, 2.15, 5.88 µs; one link 1.90, 6.03,
    16.48, 69.05 µs; at least 110,263, 309,928 and 1,150,833 traps resumed (the count was not atomic: lower
    bounds); every turn on one of the targets (checked by each worker's `ud2` at its end and exit 0; the printed
    line is unconditional).
- Run 5 (scratch run5/; load 0.3 to 0.5 before each, `gcc -O2 -pthread`, timed parts on one pinned processor):
  - flags.c, flags.out, 3 runs: the count stack with a sibling storing 0x40302 into its slot, 20,000,000 rounds:
    `andq` and `popfq` of that word set TF 6, 6, 6 and AC 6, 6, 6 times; the flags table 0 and 0 each run; without
    the sibling 8.25 to 8.26 ns a round against 9.12 to 9.18. P's resume (tail off the alternate stack, unblock from
    a read-only set, XRSTOR mask 7, the table, registers, jump), 200,000 stops flooded by one sibling while another
    ORs TF and AC into the copy's rflags, 128 KiB alternate stack: 2 to 4 taken in guest code, 53,145 to 62,085 in
    the tail, deepest frame 3 KiB, the guest loop's flags (CF PF ZF OF DF NT ID set, TF AC clear) and r12..r14 and
    ymm7 kept. A round trip `tgkill` to itself, 3 rounds of 200,000 a run: 948 to 953 ns, `rt_sigreturn` 1,073 to
    1,080.
  - patch2.c, plink.out (run 4's patch.c with an atomic trap count; the probe's own 64 threads raised the load to
    10 to 15): membarrier 0.74, 1.50, 2.11, 5.88 µs; one link 1.97, 5.91, 16.69, 69.99 µs; traps 0, 115,878,
    321,673, 1,191,515; exit 0.
  - plink.c, plink.out: P's links across processes (embedder writes a shared memfd, the stub's threads run it
    read-execute, three futex requests a link, 20,000 links): with a stub thread waiting, 0, 4, 16, 64 running
    threads, 14.92, 19.37, 38.83, 79.79 µs a link (traps 0 to 1,422,111); with none waiting, each request stopping
    a running thread whose handler makes the call, 1, 4, 16, 64 threads, 15.81, 29.84, 83.96, 231.34 µs (traps
    67,487 to 4,394,533); every stub exited 0 with every turn checked.
- Run 6 (scratch run6/; `gcc -O2 -pthread`, timed parts on one pinned processor):
  - jcc16.c, jcc16.out (load 0.13 before, 1.76 after the probe's own 16 workers): a `jcc` rel32 exit of each of
    the 16 conditions relinked 20,000 times by the `int3` protocol, workers loading seeded arithmetic flags each
    turn, the handler evaluating the condition from the frame's rflags: 16 workers (seed 0x5eed) 320,571 to 321,559
    traps a condition, 4 workers (seed 0x2b17) 91,297 to 93,199; 0 wrong arrivals in all 32; exit 0.
  - presume.c, presume.out (load 0.03 before, 0.11 after): P's handler as the plan specifies it (assembly, no call
    or push, image at its fixed place, measured 2,880 bytes below the alternate stack's top, 832 bytes, XRSTOR
    mask 7, the flags table, deciding words read-only, resume through a read-only table): 200,000 stops flooded
    while a sibling ORs TF and AC into the copy's rflags and stores handler addresses into its r10 and r11 (which
    this probe never loaded, so that edit could not matter), 3 runs: 3 to 7 taken in guest code, 55,133 to 63,419
    in the tail, deepest frame 3 KiB, flags, r12..r14 and ymm7 kept, exit 0; round trip 930 to 942 ns against 1,078
    to 1,097 with `rt_sigreturn` (3 rounds of 200,000 in each of 3 runs).
- Run 7 (scratch run7/; load 2.0 to 3.2 from other users of the machine, Linux 7.0.0-1018-aws):
  - btseq.c, btseq-altdelta.out (`gcc -O2 -mno-red-zone -mbmi2 -mfsgsbase`, processor 5): way I's exact bit-test
    address, 2,000,000 cases with seed 0x5eed and 2,000,000 with 0x2b17 (half 64-bit, half 32-bit; offsets small,
    negative, bit 62 set, any): 0 differences from the meaning's A, flags changed 0 times, `lock bts` on `gs:[t]`
    landed at exactly B + A's bit in every case; exit 0.
  - altdelta.c, same file: the vector image at the context (rdx) + 448 bytes for rsp on the program's stack and for
    rsp at top - 64 KiB - k, k = 0..127 (the same delta each time; from the top -68,544 to -68,672); rsp 512 bytes
    above the alternate stack's bottom: the process killed by signal 11.
  - presume7.c, presume7.out (`gcc -O2 -pthread -fno-stack-protector -mfsgsbase`, main thread on processor 5):
    the handler restoring all sixteen general registers (r10 and r11 included) and leaving by `jmp` through a
    read-only word to a flag-free lookup entry that reads the copy's pc from the register area at `gs:8`; 200,000
    stops flooded while a sibling ORs TF and AC into the copy's rflags and stores handler addresses into its r10 and
    r11, 3 runs: 8 to 10 taken in guest code, 54,952 to 57,568 in the tail, deepest frame 3 KiB, the guest loop's
    flags, r12..r14 and ymm7 kept, exit 0; round trip 924 to 931 ns against 1,069 to 1,079 with `rt_sigreturn` (3
    rounds of 200,000 in each of 3 runs).
- Run 8 (scratch run8/, btcut-specx3.out; load 0.1 to 0.8 before and after each, processor 5):
  - btcut.c (`gcc -O2 -mno-red-zone -mbmi2 -mfsgsbase`): 2,000,000 cases with seed 0x5eed and 2,000,000 with
    0x2b17 (16, 32, 64-bit offsets, A in a 1 MiB buffer at B, t at or above 2^32 in 625,255 and 625,142): `lock
    bts` on `gs:[t32]` with the offset register missed exactly B + A's bit 0 times; the 32-bit `lea` A against the
    meaning's wrapped A 0 wrong, flags changed 0 times; `67 0f a3 08`'s case reached bit 0 of B + 0x10; exit 0.
  - specx3.c (`gcc -O2 -mbmi2 -mfsgsbase`), 100,000 rounds each, seeds 0x1234567 (secret 0xc7) and 0xbeef01 (0xc1),
    rounds where the secret's byte (or bit) was read: `cmp; jae; gs:[t]` 4 and 7, `gs:[t32]` 0 and 0; probe then
    `gs:[t]` 99,999 and 99,999, then `gs:[t32]` 0 and 0 (the window's byte 99,999 and 99,996); `bt r, gs:[t]` 7 and
    5, `bt r, gs:[t32]` 0 and 0; probe then `lods gs:[rsi]` 99,997 and 99,997, with `mov esi, esi` first 0 and 0;
    exit 0.
  - nostd/ (a no_std crate using astro-float-num 0.3.7 without default features and rustc_apfloat 0.2.3, built in
    the checkout's Nix shell, Rust 1.99.0, `cargo build --release --target x86_64-unknown-none`): fails in
    lazy_static (E0463, no std) as the x86 crate takes them today; builds with `lazy_static` and its
    `spin_no_std` feature added (tree: either, itertools 0.10.5, lazy_static 1.5.1, spin 0.9.9, smallvec, bitflags).
- Measured by the seventh check (check-xn0g/): specx.c, specx2.out (the uncut forms leaking, the cut one not; the
  figures are in plan 4); bt16.c, bt16.out (the 16-bit sequences, 10,000,000 cases, 0 wrong; raw `rt_sigaction`
  without `SA_RESTORER` killed by signal 11).
- Measured by the fifth check (check-xn0e/run.out): UMIP emulation of all five of sgdt, sidt, sldt, str, smsw in a
  64-bit process (none raised a signal); a `popfq` of the guest's word with TF set gave 10 steps over ten `nop`s,
  of a flags-table word 0.
- Taken from cloud/x86/x86-machine.md, not measured by this track: XSAVE 11,008 bytes, a signal frame's xstate_bv
  0x207, split-lock detection.

## Departures from docs/native.md (the ARM64-first plan's x86 parts)

- The thread area on GS in the machine and P (as native.md) but on FS in I, where GS holds the window; the guest's
  FS base in the register area in I only.
- A stop inside a rep string resumes at the instruction's twin instead of a DR0 breakpoint, which P and I cannot set
  without perf events.
- Jumps through a register and returns through a two-level mirror with a 32-bit first-level index, not a compared
  cache (measured above).
- The partial-write forms and undefined-flag forms routed by the probe's match on the host, not a Split route.
- The XSAVE family goes to the inside (native.md copied it between two float calls): the host's XCR0 sizes the image.
- Three withheld registers, not two (the mirror needs a second scratch).
- The replay check is `jrcxz` with rcx borrowed into t (native.md left x86's form open, "chosen at N9").
- Guest single step in P by stepping pieces with the hardware TF clear (native.md 3.4 steps in hardware, which P
  cannot resume from safely); the machine and I keep native.md's form.
- `wrfsbase` routed to the inside in P on a 5-level host (copied elsewhere in the machine and P).
- Links by Linux's `int3` protocol in batches of three syncs, not one store (native.md 3.2 relies on ARM64's
  allowed concurrent change of a branch, which x86 does not give); a hit on a `jcc` exit evaluates the exit's
  condition, so `DirectExit` gains `condition`.
- The flags table in P only; the machine and I put rflags back from the word they saved, keeping the guest's TF.
- A call's guest return address from a read-only pool outside the code area, so the code area is never read as
  data (P maps it execute-only).
- A rep string's pass count computed inline after the instruction until the count is ruled (native.md section 4
  does not count rounds).
- x87 arithmetic is copied only while every x87 exception is masked; a copied `fldcw` is checked, and a thread
  with one unmasked runs its x87 instructions in the inside and its MMX instructions after a pending-exception
  call (native.md 3.4 copies x87 arithmetic always, which leaves FDP and FOP to the host). FIP is kept by a store a
  run in the register area, not taken from the hardware.
- Saved return points converted when their piece is dropped (native.md does not say what a thread waiting in the
  inside resumes at after its piece goes).

## Coordination with other tracks (read theirs at the start of every run)

- native-without-virtual-machine (docs/native-without-virtual-machine.md, the P/I plan, written the same day as
  this one). Asked of it, to reconcile before XN1 or its W1:
  - Way I's guarantee against speculation (its 3.2, 9 and W2's test, which its 3.10 takes for x86 "as it stands"):
    holds for x86 since run 8, every access in both forms being cut to 32 bits (plan 4, Speculation; measured).
    Before run 8 the exact form's access was on 64-bit `gs:[t]` and leaked (the seventh check's specx.c).
  - `SA_RESTORER` on x86-64: its 3.6 now says it; plan 3.7 says the same for P's stub.
  - I on x86: the thread area is on FS (its 3.8's `wrfsbase` swap at entry and exit stays, but the value during guest
    code is our area, not the guest's FS base) and GS is the process's window base; one area per runner, the
    process's first level mapped in at a process switch. The x86 window (2^32 plus 2^35 and two pages below, one
    above) fits its 2^36 slots; the x86 guest ceiling in I is 2^32, not 2^r.
  - Calls into the inside in P and I: `lea t, [rip]; jmp seg:[entry + 8*kind]`, nothing pushed. In I the routine
    keeps t in the inside's memory and jumps back to it. In P (changed in run 6, replacing "the stub keeps t in a
    register" of its 3.7) the stub hands t to the embedder in the request and leaves only by `jmp qword ptr
    gs:[resume]`, a word of the thread area's read-only part the embedder writes after its position map shows t
    is a call return point of the kind asked in a live piece of the process (else the lookup's entry, or the thread
    ended "call from an unknown host address"); a t from a stop's copy is checked the same way, and every other word
    the stub uses after a stop (its pages, the answer word, the futex address) is read again from the read-only
    part. A pure answer the stub makes itself may jump back to t only when no stop landed in it.
  - Signals: resumption at a guest boundary through the lookup, as its 3.3; the rep-string twin is entered that way.
    P's handler never returns by `rt_sigreturn` (its 3.7 says so; the plan's 3.7 has the measurement and the
    jump-out it measured); t from a frame copy is read only as a guest value and u never, so P has no hardware
    single step (stepping pieces instead). In I the handler sets the host's FS base before any Rust code and the
    area's again before guest code; its 3.8 swaps FS at entry and exit only.
  - The stub's routines: t is r11 and c is r10, which a host `syscall` overwrites or takes as an argument.
  - New in round 3 run 2: an operand on the guest's `gs:` runs inside a window where FS holds the guest's GS base
    (`rdfsbase u` ... `wrfsbase u`, plan 3.1); the position map gives the guest's FS base there as held in u (guest
    register 16, `carry::FS_BASE_NUMBER`). P's handler and the machine's floor take the guest's FS base from the
    map at a stop, fault or signal and put it on FS before resuming the guest anywhere else; nothing of ours reads
    FS there. It needs user FSGSBASE, which the probe already requires in the machine and P.
  - Links in P: the embedder writes the code by the `int3` protocol (plan 3.2); the stub's first thread registers
    `MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_SYNC_CORE` before any guest code; each of a batch's three syncs is a
    command (its 4.2) to a stub thread in a kernel entry, or to one the embedder stops first, which runs
    `membarrier(PRIVATE_EXPEDITED_SYNC_CORE)` and answers (measured 14.92 to 231.34 µs a link, run5/plink.out);
    the stub's SIGTRAP handler takes an `int3` hit (`SI_KERNEL`, at an exit of the batch) as the exit's branch
    would: a `jmp` exit at its unlinked path, a `jcc` exit by its condition on the copy's rflags, at the unlinked
    path or after the `jcc`; any other SIGTRAP or an alignment SIGBUS is a fault of ours.
  - The code area may stay execute-only (its 3.4): translated code reads no data from it; it needs a pool range,
    read-only and not executable, at one fixed distance from the code area.
  - The stub's handlers follow its 3.6 as written (mask, `SA_NODEFER`, the tail taking the exit already written;
    run 4's x86 text restored the frame's mask and is replaced), are assembly that makes no call or push on the
    alternate stack, unblock the stop signal from a read-only set, and move rsp off the alternate stack before the
    unblock. The stub has a read-only page holding the flags table and that set, and puts rflags back only by
    `popfq` of a table word (plan 3.6), restores vector state with the offered groups as the XRSTOR mask, reads the
    offered groups' part of the vector image at rdx (the context address) plus a fixed delta (learnt from its first
    frame; 448 bytes on the Xeon, run7/altdelta.c; a place below the stack's top is wrong once a guest rsp points
    into the alternate stack), never through `fpregs`, loads a new thread's groups from our kernel's image before its first instruction, and its threads
    have no user shadow stack (refused by name otherwise). I's handler resumes by `rt_sigreturn` with the frame's
    rip set to the lookup's entry; I's routine clears TF and AC before Rust code.
  - Saved return points (plan 3.7, added in run 7): a drop converts every saved t in a dropped piece into its guest
    pc and count before the piece's map goes; in P the embedder validates each t at the request (recording that
    pc and count) and answers a thread whose piece was dropped with the lookup's entry; in I a thread leaves the
    inside only past the memory's drop gate, which the 4.2 stop waits on. Its 3.3 rule ("never at a host pc in the
    middle of a piece") is kept for every resume after a drop or a stop; the jump back to t is only for a live
    piece.
  - P on a 4-level host: about 3,000 guest threads per process (32 GiB of first level each).
  - What each way must give: plan section 2 (read-only part of the area, no host address in guest-writable memory).
- KVM back end (the floor): the thread areas at one virtual address mapped per virtual processor through its own
  top-level entry, so no thread reaches another processor's register area (plan 2); guest page tables 4-level; our
  area in the upper canonical half on user-accessible pages, guest ceiling 2^47 (plan 6); links synced by an
  interrupt to every processor running the process, three times a batch, and vector 3 in translated code resumed
  as the exit's branch would (a `jcc` exit by its condition on the saved rflags, plan 3.2); the carrying code's
  `popfq` reads its own saved word, not P's flags table, so the guest's TF holds (plan 3.6); on a copied `syscall` the floor writes rcx and the guest's r11 slot (plan
  3.6).
- kernel: `rt_sigreturn` with cs 0x23 refused by name ("32-bit code segment in sigreturn, not provided", SIGSEGV);
  the kernel's writes into translated pages drop them (plan 3.7). Asked in run 7: what an x86-64 program gets as
  its vDSO. Today vdso.rs builds only ARM64's one-function image and loader.rs hands it out through
  `AT_SYSINFO_EHDR`; an x86-64 guest needs either no vDSO (glibc's `clock_gettime` then a system call, as the P/I
  plan assumes) or an x86-64 one (plan 3.7).
- x86 track (run 3): on the rented Xeon, the meaning differs from the reference for some cases of `sldt`, `smsw`,
  `str` (which the host kernel emulates under UMIP; e.g. `66 67 41 0f 01 24 e7`: the reference faults at
  0x43ffffff having changed the byte at DATA + 0x2fff, the meaning at 0x44000000 with nothing changed), `wrfsbase`
  and `wrgsbase`; WebAssembly layer 1 fails the same cases (checked for
  smsw), so they are its to answer; native layer 1 counts them by name.
- x86 and translator tracks: the rep string count (plan 3.6 and 9) changes the WebAssembly output's count if ruled;
  shared code changes (plan section 9) go in small commits that keep ARM64's suites passing. The x86 crate's core
  gains `Condition`, the x87 unit, the transcendental answers and their dependencies (plan 9), so the kernel,
  which depends on the core, compiles them too.
- instruction design: the x86-64 `ProcessorArchitecture` and precise instruction sets do not exist yet; the route of
  undefined-result and partial-write forms takes the host's match from them once they do. Run 10's AMD data is in
  docs/status-instruction-design.md, "Data from other tracks" (written by this track's closing run).

## For the owner

- How a rep string instruction counts (plan 3.6): today the WebAssembly output counts passes, which depend on data;
  proposed: its elements, as code in plan section 9.
- Split-lock detection on cloud hosts (plan 3.6, 5): a named difference in P and I, or the inside completes the
  access.
- Decision 14 for x86 (plan 10): partial-write and undefined-flag forms routed by the probe's match per host. Run
  10 measured that it matters: on an AMD EPYC 9R14 `fstp m80` and `fbstp` leave no partial write where the Xeon
  leaves 8 bytes, so on AMD hosts those two take the inside (now built) unless the guest is to see AMD's behaviour.
  Larger: AMD's undefined flags differ from the meaning's (Intel's) in 9,447 of 15,644 sweep cases (shifts,
  rotates, mul, imul, div, idiv, tzcnt, lzcnt; results always equal). Natively on AMD, either the guest sees AMD's
  undefined flags there (a precise set per container, the instruction design's), or each such form with a live
  undefined flag goes to the inside or inline flags (shifts are hot). One concrete example: `rol al, 2` (c0 c0 02) of al 1
  with rflags 0xad7 before leaves 0xad6 on the Xeon and 0x2d6 on AMD (OF), which a following `jo` reads.
- P on a 4-level host: about 3,000 guest threads per process (plan 3.3, 5).

## Licences

XN1 run 7 made libm 0.2.16 (MIT, already in the lock) a dependency of the x86 core: the reciprocal estimates' square
root and rounding, correctly rounded as WebAssembly's, without std.

XN1 run 6 made rustc_apfloat 0.2.3 (Apache-2.0 WITH LLVM-exception) and astro-float-num 0.3.7 (MIT) dependencies of
the x86 core, and added lazy_static 1.5.1 (MIT OR Apache-2.0) with `spin_no_std`, which brings spin 0.9.9 (MIT);
all three were in the lock already.

XN1 run 3 made libc 0.2.191 (MIT OR Apache-2.0, already in the lock) a dependency of wcontainers-translator-test on
Linux x86-64 only (the signal frame's layout and system call numbers of native layer 1's blob), and turned on the
translator's `native` feature there. No new crate.

XN1 run 2 turned on iced-x86's `instr_info` feature (for the stand-ins' register use) and, in tests only, `intel`
(listings), both of iced-x86 1.22.0 (MIT, already a dependency), and made wcontainers-translator (this
workspace's) a dev-dependency of the x86 crate. No new crate.

XN1 run 1 turned on iced-x86's `encoder` feature (iced-x86 1.22.0, MIT, already a dependency) and added fastrand
2.5.0 (MIT OR Apache-2.0, already in the lock for wcontainers-arm64's tests) as a dev-dependency of the x86 crate,
for the seeded sweeps. Plan
section 9 moves rustc_apfloat 0.2.3 (Apache-2.0 WITH LLVM-exception) and astro-float-num 0.3.7 (MIT), both already
dependencies, into the x86 core and adds `lazy_static` 1.5 (MIT OR Apache-2.0) for its `spin_no_std` feature, which
brings spin 0.9.9 (MIT); licences read from cargo tree on the rented machine.

## Security

- XN1 run 10's answers (answer.rs, x87.rs, fpu.rs, native.rs, meaning.rs, the test crate's native.rs, x86_64.rs
  and layer1.rs): no unsafe code, no new crate. `enter` and the x87 stores reach guest memory only through
  `answer::Memory`, each access all or nothing, so a fault leaves only the parts the reference leaves.
- XN1 run 6's core moves and `answer` (answer.rs, x87.rs, transcendental.rs, fpu.rs, native.rs, lib.rs, operand.rs,
  and the test crate's native.rs and layer1.rs): no unsafe code; no new crate (lazy_static and spin were in the
  lock already). `answer` writes only the guest's own block and reads nothing outside it.

- XN1 run 5's loop homes (carry.rs, area.rs, native.rs, the shared native.rs files and native_output.rs): no
  unsafe code, no new import. The register area's home words hold only guest register values, like the withheld
  slots, so a sibling that writes them changes only the guest's own state; no host address is ever kept there.
- Unsafe code: none in this track (XN1 run 3's native.rs in the test crate holds none: the blob is machine code in
  a byte buffer the reference runs; XN1 run 2's carry.rs, native.rs, area.rs and native_output.rs hold none;
  XN1 run 1's x86/native.rs, carry.rs, area.rs, strings.rs and two test files hold
  none; grep finds no `unsafe` in them); none planned (the translator and wcontainers-x86 keep the workspace ban;
  emitting code into a byte buffer needs none). No imports to vet beyond fastrand, a test-only dependency.
- Hostile-guest points the plan answers: every host address translated code jumps to is a word of our tables for any
  guest value (the mirror, plan 3.3, measured over 37 targets); no host return address is ever pushed or read back
  from guest-writable memory (plan 3.6); the guest cannot set or read the thread area's segment base (plan 3.1, the
  routing test); prefetches are never copied; strings in I are bounded at their start (plan 4); `xsave` never runs
  with the guest's mask on the host's XCR0; 32-bit code is never entered, sigreturn included; the inside's Rust in I
  cannot clear the guest's ymm upper halves (XSAVEC); P never resumes through a signal frame a sibling can edit
  (measured: 198,432 of 200,000 diverted when it does) and has no hardware single step; P's XRSTOR mask is the
  offered groups, never the frame's (measured: the frame's mask let an edited PKRU through); every `popfq` in P
  reads a word of a read-only table, so no sibling store can set TF or AC (measured: clearing a writable word first
  let 6 of 20,000,000 through, the table 0); P's handler unblocks the stop from a read-only set, never the frame's
  mask, and keeps the alternate stack to one frame under a flood of stops (measured: 3 KiB deep at 200,000, with
  the handler as specified, run6/presume.c); P's stub never jumps to a host address a stop may have exposed to a
  sibling: it leaves through a read-only word the embedder writes only for a call return point its map knows;
  P's handler writes nothing of ours on the alternate stack and reads the vector image at rdx plus a fixed delta,
  never through `fpregs` (measured constant for 129 interrupted rsp values); its exit restores all sixteen
  registers and jumps only through read-only words (measured with a sibling storing handler addresses into the
  copy's r10 and r11, run7/presume7.c); an `int3` hit on a `jcc` exit chooses only between two host addresses of
  the map (measured: 0 wrong arrivals over 16 conditions); way I's exact form checks the address a bit test
  reaches, not its operand's (checking the operand alone let the check's probe set a host byte 1 GiB away; the new
  sequence measured over 4,000,000 seeded cases); every way I access in both forms is cut to the window by its
  addressing mode, so a speculative access past either check reaches nothing outside it (measured: the uncut
  live-flags forms read a byte outside 99,997 to 99,999 times in 100,000, every cut form 0, run8/specx3.c);
  P's handlers carry `SA_RESTORER`, without which the host kernel kills the stub at its first signal;
  no thread resumes at a host address in a piece dropped since its call (plan 3.7, tests in plan 7);
  no code is changed while another processor may run it except by the `int3` protocol
  (measured over 20,000 links with 64 threads running the exit); the code area is never read as data, so it may be
  execute-only; the machine maps each processor's area only for that processor; I's handler restores the host's FS
  before Rust code. Points it hands to
  P: writable pages of the guest's host process, the signal frame and other threads' register areas included, are
  the guest's (so a sibling can change a stopped thread's count c, named in plan 5).
- Tests for these: plan section 7 (shapes, way I, routing).

## How to run the probes

`tar cf - <probes> | orchestrator/cloud/x86-run --ssh 'mkdir -p /work/probes/xno && cd /work/probes/xno && tar xf -
&& gcc -O2 -o mirror3 mirror3.c mirror3.S && taskset -c 5 ./mirror3 30 21'`; the same for returns (`returns.c
returns.S`, `./returns 30 21`) and the partial probes (`gcc -O2 -mavx2`, no arguments); `python3 -I shapes86.py <ELF
files>`. Run 3's: `gcc -O2 -mavx2 -mfsgsbase -fno-stack-protector -pthread -o psig psig.c && ./psig 200000` and
`gcc -O2 -mfsgsbase -o wrfs wrfs.c && ./wrfs`, in /work/probes/xn0b. Run 4's, in /work/probes/xn0r4: `gcc -O2
-mbmi2 -o calls calls.c calls.S && taskset -c 5 ./calls 30 21`, `gcc -O2 -mavx2 -fno-stack-protector -o resume
resume.c && ./resume 200000`, `gcc -O2 -pthread -o patch patch.c && ./patch 20000`. Run 5's, in /work/probes/xn0r5:
`gcc -O2 -pthread -fno-stack-protector -o flags flags.c && ./flags 20000000 200000 200000` (rounds, stops, round
trips), `gcc -O2 -pthread -o plink plink.c && ./plink 20000`, `gcc -O2 -pthread -o patch2 patch2.c && ./patch2
20000`. Run 6's, in /work/probes/xn0r6: `gcc -O2 -pthread -o jcc16 jcc16.c && ./jcc16 20000 16 0x5eed` (links,
workers, seed; exit 1 on any wrong arrival), `gcc -O2 -pthread -fno-stack-protector -o presume presume.c &&
./presume 200000 200000` (stops, round trips). Run 7's, in /work/probes/xn0r7: `gcc -O2 -mno-red-zone -mbmi2
-mfsgsbase -o btseq btseq.c && taskset -c 5 ./btseq 2000000 0x5eed` (cases, seed; exit 1 on any difference), `gcc
-O2 -o altdelta altdelta.c && ./altdelta`, `gcc -O2 -pthread -fno-stack-protector -mfsgsbase -o presume7
presume7.c && ./presume7 200000 200000`. Run 8's, in /work/probes/xn0r8: `gcc -O2 -mno-red-zone -mbmi2 -mfsgsbase -o
btcut btcut.c && taskset -c 5 ./btcut 2000000 0x5eed` (cases, seed; exit 1 on any difference), `gcc -O2 -mbmi2
-mfsgsbase -o specx3 specx3.c && taskset -c 5 ./specx3 100000 0x1234567` (rounds, seed), and in nostd/ `nix develop
<a checkout copy on the machine> -c cargo build --release --target x86_64-unknown-none`.
