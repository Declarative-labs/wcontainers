# x86-64 native output: status

Plan: docs/x86-native-output.md. Brief: pyro-parked/wcontainers-scratch/orchestrator/relaunch-agents/
x86-native-output.js. Scratch: pyro-parked/wcontainers-scratch/x86-native-output (each run's probes, logs and
the independent checks' outputs, in the folders named below). This file keeps the current state; the run-by-run
record (the plan's eight checks, XN1 runs 1 to 10 and round 3 runs 2 to 6, finding by finding, with every run's
results and controls) is in git: `git show 5062ac632:docs/status-x86-native-output.md`.

## Where things stand

- XN0 (the plan): done. Eight independent checks; every finding fixed in the plan (the eighth's way I findings
  carried to XN2).
- XN1 (the output with no memory check: the machine way and P): complete for its independent check, closed on the
  orchestrator's ruling of round 3. What is built is the next section; what is not is the gap list, each item with
  its cause and owner. Fourteen independent checks of XN1 so far; round 3 run 7 answered the fourteenth (below).
- XN2 (way I, the checked forms): not started. The eighth check's way I findings (0x67 strings, the exact form's
  0x67 segment operand and `67 d7`, `maskmovq` in the window) and the ninth's crossing of offset 2^32 under 0x67 in
  the exact form are fixed in the plan only; `carry::access` refuses the in-process way by an assertion until then.

## What XN1 built

- Routing (crates/wcontainers-x86/src/native.rs): `route` of every decoded form for each way (the machine, P, I
  with its check) and host (`HostMatches`: paging depth, partial writes, undefined results) and the process's
  `Offer`; the shared `Route::Area`; `native::host_differs` with the counted reason `HostModelResultRouted`;
  `native::question` turns every Kernel-routed form but `syscall` into a `Question` for `answer`. Tests:
  tests/native_routes.rs (a seeded sweep of every decoded form in all ways on two hosts against the plan's rules,
  54 named forms, every call into the inside having a question).
- Carrying (carry.rs, `Native for X86_64`): three withheld registers with slots, a withheld register named as a
  whole operand read or written in its slot, a looping part's homes (plan 3.4); one place for a guest access,
  `carry::access`; an operand on the guest's `gs:` run on FS holding the guest's GS base, the guest's own bytes
  with 0x65 made 0x64 (plan 3.1); a RIP-relative operand under 0x67 on `[t32]`; calls into the inside (`lea r11,
  [rip]; jmp gs:[entry + 8*kind]`); the count's plain add; direct exits with `condition`, `retarget` and a shared
  `Patch`; FIP stored after each copied x87 instruction and the flag-free check after a copied `fldcw` (call kind
  `X87Control`); a repeated string under the count sent to the inside (`RepeatedStringCounted`); direct transfers
  past user space (`Question::NearBranch`); P's stepping pieces and the replay check before a piece
  (`translate_native_request` with `NativeRequest { look_ahead, exact_replay, stop, step }`, `InsideCall::Step`
  and `Stop`, the stop word in the area's fixed part). Tests: tests/native_output.rs, native_carry.rs,
  native_far.rs.
- The core (builds for `x86_64-unknown-none`): area.rs, strings.rs (`string_passes`), `Condition::holds`, the x87
  unit, transcendental.rs, root.rs, user_segments.rs, and answer.rs: `answer` answers `cpuid`, `xgetbv`, the eight
  transcendentals, the reciprocal estimates, `popf`, `pushf`, the descriptor reads, the segment register moves,
  `push`/`pop` of fs and gs, `lfs`/`lgs`/`lss`, `rdtsc`/`rdtscp`, `wrfsbase`/`wrgsbase`, `retf`, `iret`, far `jmp`
  and `call` through memory, `xsave`/`xrstor`, `enter`, the x87 stores of ST(0), a direct transfer past user space
  and a first-level fault of a transfer through a register or memory (`near_transfer`); `float_status_after` (the
  Float status route's second call); `native::trap` (the Trap route's trap).
- The test crate (crates/wcontainers-translator-test): native layer 1 (src/guest/x86_64/native.rs: each case's
  piece in a reference blob, run five ways: plain, counted with the trap flag stopping it after every host
  instruction, on a stepping piece, on a counted stepping piece behind a replay check that goes on, and behind one
  that stops); native layers 2 and 3 and the benchmark set (native_programs.rs: a program's pieces linked and run
  in a blob that answers system calls and repeated strings); tests/layer1.rs `the_shapes_of_the_plan_*`,
  `the_inside_answers_*`, `the_second_call_*`, `a_condition_holds_*`; tests/layer2.rs
  `branches_into_an_instruction_*` and `repeated_strings_count_*`.

## Round 3 run 7: the fourteenth check's findings, finding by finding

- [blocking] several segment prefixes: the plan, the status and the shapes test said the Xeon takes the last of
  `2e 65 36` (ss) where the meaning and iced take gs. Wrong: the CPU takes gs (the check's prefixes.c), and the
  ninth check's evidence was the reference runner writing into its own memory (GS + offset 0x8C7CBDA6, inside
  `layout::x86_64::RUNNER`). The code was right (the guest's bytes run as written). Fixed: plan 3.1 now says what
  was measured (cs, ds, es and ss overrides ignored, the last of fs and gs wins; iced decodes the same); gap item
  6's prefix half is gone; the shapes test's setg puts its offset at 0x40 with GS at the data, so only a gs access
  lands, and new cases run `8b 01` (with and without REX.W) after `65`, `2e 65 36`, `65 36`, `36 65`, `65 2e`,
  `65 3e`, `65 26`, and `64 65`, `65 64`, `64 2e 65 36`, `65 36 64 2e` with FS 0; each case also asserts how it
  ended (Exit where gs is chosen, Fault where fs is), so a case that passes for the wrong reason fails. Control
  (rented copy only): expecting Exit for `65 64` fails with "segment prefixes ending otherwise: [([65, 64, 8b, 01],
  Some(Fault), Exit)]", the CPU's fault on fs. tests/native_carry.rs's comment corrected likewise.
- [minor] `Call_m1632`: gap item 7 and "Coordination" now name the 32-bit far call through memory beside
  `Call_m1616`, with the check's case. Native layer 1 does not reach it at seeds 13013 and 31122026 (results
  below); why its generator misses the case is not studied.
- [minor] `sgdt` and `sidt`: gap item 7 and "Coordination" now give the stack fault with an rbp base, and the
  single-prefix and no-prefix cases at a non-canonical address, with the check's count (22 and 24 of 2,000,000).
- [minor] this file was 1,784 lines: rewritten to the current state, the history left to git (above).
- [no defect] the gap list item by item: nothing to change but item 6's prefix half.

## Round 3 run 7 results (rented x86 machine, Xeon 8488C, 5-level paging, 2026-11-24 06:51 to 07:00 UTC; scratch xn1r16/)

- Load 24 at the start, 9.8 to 30 during: clippy with warnings denied, `--no-deps`, wcontainers-x86 and
  -translator-test, all targets: clean (without `--no-deps` it stops on two type-complexity findings in
  crates/kernel/interface.rs, not this track's). `cargo test -q -p wcontainers-x86 --test native_carry`: 3 pass.
- The shapes test: 338 as the CPU, 66 known (the 5-level reference), 2 known (`wrfsbase`, `wrgsbase` of 2^47,
  answered by the meaning), every segment-prefix case ending as expected; 7 s.
- `WC_LAYER1_FORMS=Call_m1632,Call_m1616 WC_LAYER1_CASES=3000`, seeds 13013 and 31122026 (load 24): natively
  `Call_m1616` fails at both (item 7), `Call_m1632` passes with 153 and 155 known (5-level); the WebAssembly output's
  layer 1 fails both forms at both seeds; its `Call_m1632` failure at 13013, looked at alone, is the 5-level
  reference, which it has no known kind for (shrunk to `41 ff 1c e0`: the CPU's page fault at 0xffff7fffffffffcd,
  general protection from the meaning), not the selector 0x33 difference.

## XN1 gap list (every item left, with its cause and owner; last corrected by round 3 run 7)

The bar as it stands: native layers 2 and 3 pass against the CPU on the rented Xeon, way P, counted and
uncounted, the count after each system call equal to the WebAssembly output's; native layer 1 passes every form
but one with only the named known kinds (the thirteenth check: 2008 forms, 1,500 cases a form, seed 777, after run
5's stepping and replay changes), and that one, `Call_m1616`, fails at some seeds as the WebAssembly output's layer
1 does, for the meaning (item 7); speed meets the bar uncounted (geometric mean 1.145 against the host in run 5,
1.110 in the ninth check's own measurement). Figures under "Current figures". Section 7's shapes that run without
an executor are tests (the shapes test, the branches into an instruction, the repeated string count); the rest are
listed here with why. Every call into the inside in the machine and P has an answer built by this track
(round 3 run 3) but two: a Kernel-routed form a question for `answer`; a direct `jmp`, `call`, `jcc`, `loop` or
`jrcxz` to a target at or above 2^47 `Question::NearBranch`; a first-level fault of a jump, call or return through
a register or memory `answer::near_transfer` with the kind `native::far_transfer` gives; the Trap route the trap
`native::trap` gives. Native layer 1 fails a call without one, and checks each against the CPU (and the trap and
the Kernel answers also against the meaning). The two without one: `syscall` (the executor's system call), and a
repeated string under the count, for which only the pass rule `string_passes` is built: the inside's run of the
string itself exists only in the test harnesses (item 1). Item 2's forms have no question either. Not built or not
run:

1. Needs an x86 executor that runs a translation. Owner: the native-without-virtual-machine track (its x86 floor,
   or P's executor, crates/executor/implementation/host-process, whose translation runner is ARM64's alone: its
   code-change and fault paths are `cfg(target_arch = "aarch64")`, executor.rs `forget_changed_code`). Cause: the
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
     and a thread with TF running `rep movsb` and ten one-byte instructions (both also need stepping pieces, item
     3).
   - The machine way (Way::Machine): its pieces (the floor's interrupt gates for calls, the copied `syscall`) have
     only the static tests (tests/native_output.rs, native_routes.rs); every native run of the test crate is way P.
   - A 4-level CPU as the reference: the rented Xeon has 5-level paging, so the 66 shapes of transfers to 2^47 up
     to 2^56 and base writes there are known failures (the 5-level reference) and have never been compared with a
     4-level CPU; the i7-8750H (4-level) is reachable only from the Mac. Owner: whoever provides a 4-level x86
     reference to the rented machines.
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
   - The inside's run of a repeated string under the count (the call `carry` emits with
     `NativeReason::RepeatedStringCounted`): the inside runs the guest's string instruction on the hardware and adds
     the passes `string_passes` gives. Only `string_passes` is built (strings.rs, core). The run itself exists only
     in the test harnesses: native_programs.rs (layers 2 and 3, and the rep string count test) runs the instruction
     in its blob; native layer 1's counted run takes the CPU's own outcome for it (native.rs, `on_reference`), so
     there only the state at the call is checked (its summary counts these runs as "a repeated string under the
     count calling the inside correctly (its outcome the CPU's own)"; 33,370 in the tenth check's run). The run
     needs the executor's thread, its signal frame and the guest's memory, so it is the executor's inside.
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
3. Built in round 3 run 5 (filed here by run 4 as this track's XN1 work): the translation side of P's stepping
   pieces and of the replay check. What is left of them is not this track's:
   - Running them: the inside's side of the calls `Step` (the meaning's debug trap after the instruction) and `Stop`
     (the thread stops before the instruction), choosing stepping pieces for a thread whose TF is set (after the
     inside's `popf` or `iret` notes it, `Notes::trap_flag_set`), the replay's recording, setting a thread's stop word
     and loading the stop's piece into one process. Owner: the executor (item 1). Native layer 1 checks the
     translation side against the CPU without one (below).
   - ARM64's `stop_check` (docs/native.md 5.7 gives its sequence). Owner: the ARM64 native back end. Meanwhile the
     shared trait's `stop_check` answers false for a guest that writes none and the translator refuses a stop
     request for it by an assertion, so an ARM64 replay cannot pass its stop silently. ARM64 needs no stepping pieces
     (docs/native.md runs TF by the hardware's single step in every ARM64 way).
   - The guest's TF in a test case: native layer 1's generator keeps TF and AC clear (x86_64.rs, `keep`), as a case
     with either set would need the executor to run (the reference CPU traps after every instruction); the stepping
     piece runs every case instead, the guest's TF standing as the piece's choice.
   The other shared items wait on ARM64's version first. Owner: the ARM64 native back end (shared trait in
   gcpu/native.rs):
   - The trait's `retarget` in place of `link` (x86's is built, run 4, and the native blobs use it).
   - Homes for a loop of several parts (search 1.40 against the host): the translator's loop copy would take a set
     of parts.
4. This track's, after XN2 (not in the bar):
   - The pool for a call through a register (`HostCode::pool`; today the return address is built in rax saved to
     the area, the plan's measured 1.146 form, not the pool's 1.120); it also needs the executor's pool range
     (native-without-virtual-machine 3.4).
   - Counted speed (exact replay): interpret 1.66, the count's adds.
   - The note of an MXCSR read while not kept for `xsave` (the meaning warns; `answer` is not told tracking).
5. Way I (XN2, as written): the eighth check's way I findings (a string with the guest's 0x67 to the inside in
   both checked forms, the exact form's 0x67 segment operand and `67 d7`, `maskmovq` re-encoded in the window), and
   now the ninth check's crossing of offset 2^32 under 0x67 in the exact form (plan 4). Fixed in the plan; their
   code and tests are XN2's. `carry::access` refuses the in-process way by an assertion.
6. Not this track's: the native layer 2 harness past about 10,000 programs in one process (the ninth check: "state memory: mmap failed to reserve 0x104000000 bytes", runner.rs 455/461/1850, the
   WebAssembly runner's reservation, made by the WebAssembly runs that test compares the count with): the cause is
   address space (the tenth check's arithmetic): each run reserves 0x104000000 bytes, an mmap without a high hint
   gets at most 2^47, which holds about 31,500 such reservations, about 10,500 programs at three runs each, where
   the failures start; so the runner's reservations outlive their runs (what keeps them is not found: the state
   memory is a wasmtime SharedMemory held in the store's data). Seen by the two native tests that compare their
   count with the WebAssembly output's over many runs in one process: native layer 2 past about 10,000 programs,
   and the repeated string count test (layer2.rs `repeated_strings_count_as_the_webassembly_output_natively_x86_64`)
   at 12,000 cases (the thirteenth check: seed 1414, 48 at once, the same panic at runner.rs 461; 9,000 cases at the
   same seed pass). Owner: the shared runner's owner (crates/wcontainers-translator-test runner.rs), with this
   track; the recorded ceilings meanwhile are 10,000 programs for layer 2 and 9,000 cases for the string count test,
   nothing above them is tested.
7. The meaning's differences from the CPU. Owner: the x86 track (its meaning, which the inside's answers are; the
   native output follows it, so fixing the meaning fixes both outputs). Recorded under "Coordination".
   - `Call_m1616` and `Call_m1632`, a far call through memory with a 16-bit and a 32-bit offset: the meaning, and
     so the inside's answer, gives general protection for a far call to selector 0x33 where the CPU lands (or
     faults on memory). `Call_m1616`: native layer 1 fails it at some seeds, as the WebAssembly output's layer 1
     does: natively at seed 31122026 (run 5), 13013 (the thirteenth check, 2,000 cases: `66 67 ff 1c 65 68 2a 00
     44`, the CPU lands with the selector 0x33 pushed, ours gives general protection) and both again at 3,000 cases
     (run 7); WebAssembly layer 1 at the same seeds (shrunk at 13013 to `64 66 ff 1c 24`, the CPU's memory fault
     against the meaning's general protection; scratch xn1r15/). `Call_m1632`: the fourteenth check's template
     fuzz, seed 15151589, 1,500,000 cases (its job3-full.log line 372): `ff 58 bf` (call far fword ptr
     [rax-41h]), rax 0x4400195a, rsp 0x44000fb0, GS 0x44000000: the CPU lands at 0x50000003 with rsp 0x44000fa8 and
     `03 00 00 50 33 00 00 00` pushed (return 0x50000003, selector 0x33); ours general protection, rsp unchanged.
     Native layer 1 passes `Call_m1632` at seeds 13013 and 31122026, 3,000 cases (run 7); the WebAssembly output's
     layer 1 fails it at both, at 13013 for the 5-level reference (run 7 results).
   - `sldt`, `smsw`, `str`, `wrfsbase`, `wrgsbase`: native layer 1 counts their differing cases by name as known
     (test crate native.rs `meaning_differs_named`).
   - `sgdt` and `sidt`, which the host kernel emulates under UMIP (the thirteenth check's fuzz, seed 140014: 53
     and 50 of 1.5 million cases; the fourteenth's byte fuzz, seed 15151515: 22 `Sgdt_m1664` and 24 `Sidt_m1664`
     of 2,000,000, all routed Kernel): with more than one segment prefix the reference gives general protection
     where the meaning writes the table; at an address that is not canonical, with one segment prefix or none, the
     reference faults on the address where the meaning gives general protection (`3e 0f 01 0b`, sidt [rbx], rbx
     0x992cc49e4400261e), or, with an rbp base, a stack fault (`40 0f 01 85 dc 7e 16 18`, sgdt [rbp+18167EDCh],
     rbp 0x7e5b20a044003415). Native layer 1's generator makes none of these states.
   - `enter` with rsp off the canonical range under 5-level paging (70 known failures in run 10's test): the
     reference's page fault where the meaning gives general protection.

## Native layer 1's known kinds

The reference's 5-level paging (a page fault where a 4-level CPU and the meaning give general protection);
answered by the meaning where the x86 track records it as differing (`sldt`, `smsw`, `str`, `wrfsbase`,
`wrgsbase`; test crate native.rs `meaning_differs_named`); and two shared with the WebAssembly output's layer 1,
Intel's transcendental result within one unit in the last place and not correctly rounded, and its approximation on
the other side of the correctly rounded result (C1 differs; `TRANSCENDENTAL_DIRECTION`), on F2xm1, Fcos, Fpatan,
Fptan, Fsin, Fsincos, Fyl2x and Fyl2xp1. Cases with TF or AC set are not made (x86_64.rs, `keep`).

## Current figures (rented x86 machine, Xeon Platinum 8488C, 5-level paging)

- Native layer 1, 2008 forms: the thirteenth check, 1,500 a form, seed 777, after run 5's stepping and replay
  changes: every form but `Call_m1616` (gap item 7). Round 3 run 5, seed 2311, 300 a form, five ways each, 32 at
  once (2026-11-24 03:11 to 03:18 UTC, load 13 to 23): 0 failing, 640 known (5 transcendental, 635 the named
  forms); ended Exit 424,952, Fault 132,834, Answered(Answer) 25,032, Answered(Trap) 18,682, FarTarget 726, Lookup
  174; a replay check stopping with the guest as it started in all 602,400. Round 3 run 3, seed 20261124, 1,000 a
  form, 48 at once (load 5 to 45): 0 failing, 2,059 known.
- Native layer 2: round 3 run 5, seed 23, 10,000 programs (load 23 to 26): 0 failing, 3,361 faulted, 114,426 blob
  runs. Ceiling 10,000 programs (gap item 6).
- Native layer 3: round 3 run 5, seed 5, 300 Csmith programs: 264 compared, 36 did not end in time on Linux, 0
  failing; round 3 run 2, seed 1, 1,000 programs: 852 compared, 148 did not end in time, 0 failing.
- The repeated string count test: 9,000 cases at seed 1414 pass (the thirteenth check), 12,000 run out of address
  space (gap item 6).
- Speed, the benchmark set at full, best of 5, times the host's plain run (uncounted, counted). XN1 run 5
  (2026-11-23 16:56 UTC, load 2.6 falling to 1.4): fib 1.25, 1.38; sieve 1.01, 0.99; hash 1.00, 1.00; sort 1.20,
  1.23; interpret 1.23, 1.66; matrix 1.04, 1.11; walk 1.09, 1.05; search 1.40, 1.47; geometric mean 1.145 (the most
  1.395), 1.216 (the most 1.658); again at 17:34 UTC (load 4.0 to 1.6): 1.133 and 1.202. The ninth check (21:10
  UTC, pinned to cores 0-9, load 9 to 27): uncounted fib 1.258, sieve 1.008, hash 1.015, sort 1.377, interpret
  0.982, matrix 1.074, walk 0.881, search 1.395, geometric mean 1.110; counted geometric mean 1.231, worst 1.489
  (search). Single programs move between runs on this shared machine.
- Routing and operand sweeps (XN1 run 1, load 0.1 to 0.7): native_routes seed 0x2b17, 2,000,000 cases
  (15,999,984 routes): 0 broken; native_carry seed 11, 1,000,000 operand shapes: 0 differences from the meaning's
  address. The map sweep (XN1 run 5, ARM64 machine, load 7 to 12): seeds 0x51 and 0x2b17, 300,000 cases each: 0
  broken.
- AMD (XN1 run 10, two c7a.large spot machines, AMD EPYC 9R14, rented for the probe and terminated; load 0.3 to
  0.5): `fstp m80` and `fbstp` leave no partial write (the Xeon: 8 bytes in 2 of 9); `enter 0, 3` and every other
  partial-write probe as the Xeon. The x86 track's flags sweep: of 15,644 distinct cases 9,447 differ from the Xeon,
  all in rflags alone (OF 4,124, AF 6,842, SF, ZF and PF 170 each, the last only in mul, imul, div and idiv):
  shifts and rotates by more than one, mul, imul, div, idiv, tzcnt and lzcnt; and, or, xor, test, the bit tests,
  bsf, bsr and popcnt agree. Scratch xn1r10/; also in docs/status-instruction-design.md, "Data from other tracks".

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
  (item 2); ARM64's `stop_check` answers false and a stop request for ARM64 panics (item 3); a repeated string
  under the count in native layer 1 returns the CPU's own outcome (native.rs `on_reference`; item 1); the runner's
  reservations run out above the recorded ceilings (item 6); `Call_m1616` fails in both outputs' layer 1 at seeds
  13013 and 31122026 (item 7).
- Several segment prefixes: the shapes test asserts how each case ended (gs chosen lands, fs chosen faults).
- The stepping pieces and the replay check (round 3 run 5): tests/native_output.rs
  `a_stepping_piece_leaves_only_through_the_inside`, `the_replay_check_comes_first_and_writes_no_flag`, and every
  piece shape of `pieces_decode_and_match_their_maps` made also with a stop point and as a stepping piece; native
  layer 1 runs every case five ways (summary line "a replay check stopping with the guest as it started").
- The slow path of a form routed for the host (round 3 run 4): tests/native_output.rs
  `a_form_routed_for_the_host_is_counted`.

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
summary lines start "layer 1 natively". Since round 3 run 5 each case runs five ways (plain, counted, stepping,
stepping behind a replay check, stopped at one): 300 a form took 7 minutes at load 13 to 23.

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
- The trait's second type is `Target` (the host's matches and the process's offer), not `HostMatches`.
- A rep string under the count goes to the inside (counted `RepeatedStringCounted`) instead of the inline
  after-code of plan 3.6: correct by construction, slow only with exact replay asked for, and no count stack.
- FIP is stored after each copied x87 instruction, not once a run (the trait's `carry` sees no instructions after
  the current one yet).
- A call through a register builds its guest return address in rax saved to the area (the plan's measured
  stand-in form, 1.146 against the pool's 1.120), until the pool exists (gap item 4).
- `DirectExit::retarget` holds, for x86, the byte offset of the branch's rel32 within it; `retarget` takes the code
  and a distance from the exit's first byte, since the caller knows where an exit starts, not where its branch ends.
- Layer 1, layers 2 and 3 and the speed measurement run in blobs of the test crate's, not under an executor (plan 8
  waits for one): the same translation, way, map, pieces, links and lookup, the blob answering system calls and
  repeated strings, the inside's answers taken from `answer`. Layer 1's every-offset check uses the hardware's TF,
  which P itself never sets; that is the test's stop. A guest system call costs a record's `write` more than under
  P, inside the timed run.
- Two classes of disagreement are counted as known failures by name (the reference's 5-level paging; the meaning's
  named differences on inside-answered forms), and cases with TF or AC set are not made.
- A withheld register named as a whole operand reads or writes its slot directly (plan 3.4 now says so).
- `answer` takes a `Question` (plan 1 and 3.5 give no signature), its memory as a generic `answer::Memory`, and
  reports `Notes`; `Unit::new` takes the constants as a fourth argument. The general register numbers and the table
  of Linux's user segments moved into the core (not in plan 9's list): `answer` needs them without `translate`.
- An operand on the thread area's segment in the machine and P runs on FS holding the guest's GS base (round 3 run
  2): the plan's rebuild on `[t + u]` could not wrap each part of an access past offset 2^32 and left the implicit
  forms with no answer. Plan 3.1 now says this.
- Plan 9's request field `stop: Option<u64>` is `stop: bool`, the check before the piece's first instruction (plan
  3.7 has the inside ask for the stop pc's own piece). `stop_check(way, code)` is a provided method returning
  whether it wrote one (false by default), with `steps(way)` beside it, so no ARM64 code was needed; the translator
  refuses a stop or step request the guest cannot make by an assertion. The request is a new function beside
  `translate_native_for`, whose signature host-process calls.
- Plan 8 lists the replay check and P's stepping pieces as XN1's: their translation side is built (round 3 run 5);
  running them is the executor's (gap item 3).

## Measured (the plan's probes)

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
- x86 track (round 3 run 5, from the twelfth check): `Call_m1616`, a 16-bit far call through memory, differs from
  the reference at seed 31122026: `66 ff 1d 81 19 00 f4` (call far dword ptr [44001988h]), the pointer holding
  offset 0x47ff and selector 0x33: the CPU lands at 0x47ff with rsp 0x44002ec8 and the bytes `07 00 33 00` pushed,
  the meaning gives general protection. WebAssembly layer 1 fails it (`WC_LAYER1_FORMS=Call_m1616
  WC_LAYER1_CASES=3000 WC_LAYER1_SEED=31122026`), and so does native layer 1 now, whose answer is the meaning's;
  it is the meaning's to answer, not a known kind of this track's. Also at seed 13013 (2,000 cases) in both
  layers (gap item 7). The 32-bit form `Call_m1632` differs the same way (the fourteenth check: `ff 58 bf`, call far
  fword ptr [rax-41h], the pointer holding 0x50000003 and selector 0x33: the CPU lands with `03 00 00 50 33 00 00
  00` pushed, the meaning gives general protection).
- x86 track (round 3 run 6, from the thirteenth check): `sgdt` and `sidt`, which the host kernel emulates under
  UMIP, differ from the reference in 53 and 50 of 1.5 million fuzz cases (seed 140014). With more than one segment
  prefix the reference gives general protection where the meaning writes the table (`26 64 65 4a 0f 01 02`, sgdt
  gs:[rdx], GS base 0x44000100, rdx 0x12b1: ours writes 10 bytes at data offset 5041); at an address that is not
  canonical the reference faults on the address where the meaning gives general protection (`65 4e 0f 01 44 6d
  f9`, GS base 0x100000040, rbp 0x44001081, r13 0x83dd1664ac612946); and `64 f2 65 0f 01 0e` (sidt gs:[rsi]): the
  reference gives general protection, ours a memory fault at 0x44003ba5. The fourteenth check adds: at a
  non-canonical address with one segment prefix or none, the reference's fault on the address against the
  meaning's general protection (`3e 0f 01 0b`, sidt [rbx], rbx 0x992cc49e4400261e), or its stack fault with an rbp
  base (`40 0f 01 85 dc 7e 16 18`, sgdt [rbp+18167EDCh], rbp 0x7e5b20a044003415); 22 `Sgdt_m1664` and 24
  `Sidt_m1664` of its 2,000,000 byte-fuzz cases (seed 15151515). They belong beside `sldt`, `smsw` and `str`.
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

No crate added by round 3. What XN1 added or turned on, every one in the lock already:
- iced-x86 1.22.0 (MIT): features `encoder` (run 1), `instr_info` (run 2), and `intel` in tests only.
- fastrand 2.5.0 (MIT OR Apache-2.0): dev-dependency of the x86 crate, for the seeded sweeps (run 1).
- wcontainers-translator (this workspace's): dev-dependency of the x86 crate (run 2).
- libc 0.2.191 (MIT OR Apache-2.0): dependency of wcontainers-translator-test on Linux x86-64 only (native layer 1's
  blob: the signal frame's layout and system call numbers), with the translator's `native` feature there (run 3).
- rustc_apfloat 0.2.3 (Apache-2.0 WITH LLVM-exception), astro-float-num 0.3.7 (MIT), lazy_static 1.5.1 (MIT OR
  Apache-2.0) with `spin_no_std`, which brings spin 0.9.9 (MIT): dependencies of the x86 core (run 6).
- libm 0.2.16 (MIT): dependency of the x86 core, the reciprocal estimates' square root and rounding (run 7).
Licences read from cargo tree on the rented machine.

## Security

- Unsafe code: none in this track's code (carry.rs, native.rs, area.rs, strings.rs, answer.rs, x87.rs, fpu.rs,
  transcendental.rs, the test crate's native.rs, native_programs.rs and the tests; grep finds no `unsafe`); the
  translator and wcontainers-x86 keep the workspace ban; emitting code into a byte buffer needs none, and the test
  crate's blobs are machine code the reference runs. No marked exceptions. (The test crate's tests/reciprocals.rs
  and tests/test_guest.rs hold `unsafe` blocks; neither is this track's.) Imports to vet: none new (the list
  above, each already in the lock).
- `answer` writes only the guest's own block and reaches guest memory only through `answer::Memory`, each access all
  or nothing, so a fault leaves only the parts the reference leaves. The register area's home words and withheld
  slots hold only guest register values; no host address is ever kept there.
- Hostile-guest tests: not run, they need an x86 executor (gap item 1). Round 3 run 7 changed only a test and the
  record.
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
  before Rust code. Points it hands to P: writable pages of the guest's host process, the signal frame and other
  threads' register areas included, are the guest's (so a sibling can change a stopped thread's count c, named in
  plan 5).

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
