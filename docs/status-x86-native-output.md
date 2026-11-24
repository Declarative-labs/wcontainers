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
  its cause and owner. Fifteen independent checks of XN1; XN2 run 1 answered the fifteenth's five minor findings
  in this file (counted speed under "Departures from the plan (XN1)", the sgdt and sidt cases under
  "Coordination", the AMD flag under gap item 2; the counted string's outcome stays gap item 1).
- XN2 (way I, the checked forms): built and measured (runs 1 to 5), see "XN2 run 5" and the runs before it. Run 4
  built the stack form the measurement chose (rsp held as B + the guest's rsp). Run 5 confirmed the speed bars on a
  quiet machine (load 1.4 to 4.3: wrapping 1.087 times the unchecked form, bar 1.10; exact 1.315, bar 1.35), ran
  every way I access shape of plan 7 that needs no executor against the CPU (892 cases a form, 0 failing), and the
  speculation probe with every gadget of plan 7 but `ret` (0 secret reads in the cut forms). Run 6 tried to defeat
  the forms (no hole found; a static test of every encoding family writing rsp added) and measured the benchmark set
  at five code placements: single-placement figures move by up to 40 percent a program (interpret) with the same
  instructions, so the fair figures are over placements: wrapping 1.034, exact 1.293 (counted 1.020 and 1.275).
  What is left of XN2 needs an x86 executor that runs a translation in process (gap item 1): the link,
  register-group and hostile-guest tests in I, the ymm upper halves across a call into the inside, and the X9
  programs. That executor is the native-without-virtual-machine track's W3 (its plan: "both forms on x86-64 Linux",
  after its W2, form I on ARM64), whose own bar already includes "the x86 plan's own way P and way I tests"; at
  runs 7 and 8 (2026-11-24) that track is in W1's round 4, W2 is not built and W3 not started. So these items
  cannot be done by this track before W3 exists, and nothing of this output stands in W3's way (see "XN2 run 7").

## XN2 run 8 (2026-11-24)

The blocker stands unchanged: no commit since run 7 (0d2ddadc6), so crates/executor/implementation/in-process still
does not exist and nothing was built or run.

## XN2 run 7 (2026-11-24)

Nothing to build without the executor. Checked that the blocker stands (docs/status-native-without-virtual-machine.md
at 6aaeec294: form I's crate, crates/executor/implementation/in-process, does not exist; the x86-64 executor, kernel
and busybox suites build no tests on the Xeon, "because they need the x86 native output's way (W3)"), and that no
commit since run 6's tests (d34a4fc4d) touches this track's crates or their dependencies (only the root Cargo.toml's
unsafe list), so run 6's verdicts stand and no run was repeated. Corrected the blocker's name here (W3, not W2) and
gap item 5, which still said `carry_plain` refuses I. For the orchestrator: as with XN1, closing XN2 with the
executor-bound items handed to W3's bar is a ruling only the orchestrator can make; the items are listed above.

## XN2 run 6 (2026-11-24)

No code of the output changed. Attempts to defeat the forms (rented ARM64 machine, the output read back):
- Forms whose operand iced may report as no access (the one place, `carry::access`, sends to the window only what
  iced's `used_memory` names): 49 encodings made in both forms (scratch xn2r6/probe.out, from a test file sent to
  the machine and not kept). None reaches memory uncut: prefetches, `nop` with memory, `cldemote` and the
  reserved-NOP space emit no host instruction; `clflush` is cut and probed as a load; `clflushopt`, `clwb`,
  `movdir64b`, `movdiri`, `enqcmd`, the shadow-stack stores, `monitor`, `umonitor`, `ptwrite`, `clzero`, `tpause`,
  `xsavec`, `xsaves`, `xrstors` and `invlpg` take the Trap route; `xsave`, `lfs`, `lss`, segment
  register moves, `sgdt`, `lar`, `lsl`, `verr` and `str` go to the inside; `cmpxchg16b`, `vmaskmovps`,
  `vpmaskmovq`, `pinsrb`, `lddqu`, `movntdqa` and `vbroadcastss` are cut like any operand.
- Forms that write rsp: the sweep's random bytes seldom make VEX or vector-to-register forms, so a new test,
  `every_form_writing_rsp_keeps_it_in_the_window` (commit d34a4fc4d), makes 63 encodings writing rsp, esp, sp or
  spl (shlx, sarx, shrx, rorx, pdep, pext, bzhi, bextr, mulx with either destination, andn, blsi, blsr, blsmsk,
  movq, vmovq, pextrq, vpextrq, cvttsd2si, vcvttss2si, the mask moves, popcnt, lzcnt, tzcnt, bsr, bsf, crc32,
  movbe, rdrand, rdseed, adcx, adox, cmpxchg, xadd, bswap, xchg, cmov, 32, 16 and 8-bit moves, lea, movzx, movsx,
  movsxd, imul, shld, setz, sbb, arithmetic, shifts, `pop sp`, rdgsbase) followed by `mov rax, [rsp]`, in both
  forms, counted and not, through the sweep's checks: all pass.
- The stack's reach as written: at most 2^31 + 2^16 + 72 bytes past the window's top (`ret 0xffff` at the top, then
  `[rsp + 2^31 - 1]`; a second return reads `[rsp]` first and faults), inside the 2^35 bytes nothing is mapped.

Interpret's wrapping figure (run 5: 1.394 times P uncounted, 0.75 counted), studied (rented x86 machine, Xeon 8488C,
12:13 to 12:20 UTC, load 0.2 to 1.7 until 12:19, `taskset -c 0-9`; scratch xn2r6/interp2.out, interp3.out,
align.out; the benchmark patched only in the machine's copy, xn2r6/counted.diff, to run one count setting):
- perf stat of the whole test process, each way and count its own process, a run with no way subtracted: P
  uncounted 10.5e9 cycles for 42.37e9 instructions, the wrapping form 14.8e9 cycles for 42.38e9 instructions (0.013
  percent more), with equal branches, branch misses, uops issued and executed, uops from the decoded cache and from
  the decoders, decoded-cache misses, machine clears and blocked loads (store forwarding, 4 KiB aliasing). Counted,
  the reverse: P 14.2e9 cycles, the wrapping form 10.7e9, for the same instructions as each other. So the
  difference is the core's back end running the same instructions at different speeds by where they are, which
  none of those counters shows; not the form.
- The pieces' alignment (16 as committed, 32, 64) and the first piece moved by 16 and 48 bytes, interpret alone,
  best of 7: P uncounted 0.346, 0.362, 0.511, 0.355 and 0.516 s; wrapping 0.466, 0.328, 0.330, 0.410, 0.343 s;
  exact 0.614 to 0.618 s at every placement. Wrapping against P: 1.346, 0.906, 0.647, 1.153, 0.664.

The benchmark set at the same five placements (12:20 to 12:44 UTC, best of 5, `taskset -c 0-9`, the machine's
1-minute load 2 to 15 with two spikes to 52 and 58 from other jobs at 12:28 and 12:32, during the 32 and 64 byte
alignments; scratch xn2r6/alignall.out, placements.txt from placements.py), each way's geometric mean time over the
placements against P's:
- Uncounted, wrapping and exact: fib 1.027 and 1.047, sieve 1.032 and 1.187, hash 0.996 and 0.996, sort 1.067 and
  1.263, interpret 1.018 and 1.674, matrix 1.012 and 2.563, walk 1.089 and 1.015 (four placements: a line of the
  last was lost to the load sampler's output), search 1.032 and 1.146. Geometric mean wrapping 1.034, exact 1.293.
- Counted: wrapping 1.020, exact 1.275.
- At each placement alone, uncounted: wrapping 1.099 (16, the committed layout; run 5 gave 1.087 there), 1.031,
  1.027, 0.999, 1.004; exact 1.334, 1.274, 1.340, 1.256, 1.302.
- So the forms' cost on these programs is about +3 percent wrapping and +29 percent exact, with the exact form's
  probe the cost of matrix (2.56) and interpret (1.67) at every placement. Both bars met at every placement.

Checks (rented ARM64 machine, load 12 to 13): every test of tests/native_window.rs passes (11, the gadget printer
ignored; the sweep at seed 0x5eed, 20,000 cases, 80,000 pieces, 0 outside); clippy with warnings denied, `--no-deps
-p wcontainers-x86 --all-targets`, clean.

## XN2 run 5 (2026-11-24)

No code of the output changed. Tests (commit d9621c277):
- `the_window_cases_of_the_plan_match_the_cpu_in_process_x86_64` gains every way I shape of plan 7 that runs without
  an executor: `maskmovq`, `maskmovdqu` and `vmaskmovdqu` with rdi in the data, 2^40 past it, 2^32, 2^32 + 2^35, 2^40
  and -1; `cmpsb` (rsi or rdi), `scasb` and `xlat` at those four starts; the guest's 0x67 strings `stos`, `movs`,
  `cmps`, `scas`, `lods`, `rep stosb`, `rep movsb`, `repe cmpsb`, `repne scasb` and `rep stosq` with esi and edi in
  the data, the upper halves set and rcx 2^32 + 16; `fs:` and `gs:` operands under 0x67 with a base at 2^32, at 2^32 +
  the data, and inside with a 32-bit offset wrapping back into the data, and the plan's `65 67 8b 00` (GS base
  0x300000000, rax 0x100000010); `bt`, `bts`, `btr` and `btc` (the last three also locked) at 16, 32 and 64 bits with
  offsets 0, -1, w, -w - 1, 2^31 - 1, -2^31, 2^35, 2^60 and -2^63 taken at the operand size, from the data's first
  byte, its middle, its last writable bytes and the last bytes of its read-only page; `enter 16, 31` with rsp near
  the data's bottom, at 0x40 and at 2^32; `call [rbx]` and `pushf` with rsp at 2^32, 0 and in the data. The single
  instruction cases have their flags live after them; the exact form takes the flag-free probe whether they are or
  not (run 1), so there is no separate dead-flags case.
- `speculation_gadgets` (native_window.rs) adds a string from rdi (`scasb` against cl, the equality picking a line),
  `xlat`, `pop` after `mov rsp, rbx`, a plain store and `maskmovdqu`, and prints the fixed part's offsets of B and
  -B; its uncut control now drops every cut of a register to its low half (`mov esi, esi`, `mov edi, edi`, the
  stack's `mov r11d, r11d`) other than the probe's own. The probe (scratch xn2r5/specemit.c) puts FS on a thread
  area holding B and -B, sets rdi too, and for a store gadget times the refused address's line, flushed before, and
  a line of the same page that nothing touches. `ret` is not made: its read is the same `[rsp]` access as `pop`'s,
  and its value feeds the lookup, not a register a probe array can be indexed by.

Results (rented x86 machine, Xeon 8488C):
- The window cases (load 3 and 8.5): wrapping 775 as the CPU and 117 the named landing; exact 892 as the CPU (145
  refused by the probe, 3 StackRefused, 17 answered by the inside: the 0x67 strings, `pushf`, `enter`); 0 failing.
- Speculation (processor 5, 12:01 to 12:04 UTC, load 0.9 rising to 46; scratch xn2r5/specemit.out), 100,000 rounds
  each with seeds 0x1234567 and 0xbeef01: every read gadget's cut form read the secret 0 times (load, lods, bt, scas,
  xlat, pop), reading the window's byte instead 99,995 to 100,000 times; every uncut control read it 99,990 to
  99,999 times. The cut store brought the refused address's line into the cache 2 times in each seed and the
  in-window line 97,386 and 99,992 times; the uncut store the refused line 97,338 and 99,992 times. A million rounds
  of the cut store at seeds 0x1234567, 0xbeef01 and 0x51 (xn2r5/specstore.out, load 28 to 15): the refused line 2,
  1 and 5 times, the untouched line 2, 0 and 0: the timing's own false hits, a few in a million, against nearly
  every round when the address is formed. `maskmovdqu` (a non-temporal store) brought no line into the cache in
  either form, the control included, so this probe cannot see it: its cut is shown by the window sweep alone.
- Speed, the benchmark set at full, `taskset -c 0-9`, best of 7 (11:54 to 12:00 UTC, the machine's 1-minute load
  1.4 to 4.3 throughout, sampled every 25 s; scratch xn2r5/bench1.out, bench1-load.txt): against P (the unchecked
  form) wrapping geometric mean 1.087 (counted 1.023), exact 1.315 (counted 1.275), both under their bars (1.10 and
  1.35). Per program wrapping and exact: fib 1.165 and 1.164, sieve 0.991 and 1.171, hash 1.001 and 1.004, sort 1.059
  and 1.246, interpret 1.394 and 1.774, matrix 1.028 and 2.605, walk 1.019 and 0.982, search 1.091 and 1.154. P
  against the host's plain run 1.137 (counted 1.211). So the figures the brief carried (+2 to +4 percent wrapping,
  +11 to +21 percent exact, on other code with the stack exempt) are on real translated programs +9 and +32 percent.
- Clippy with warnings denied, `--no-deps`, wcontainers-x86 and the test crate, all targets: clean (x86 machine);
  the x86 crate's native_window tests pass (ARM64 machine, load 4).

## XN2 run 4 (2026-11-24)

Built (commits ce33aa912 and 1a683fbfd; plan section 4 "As built (XN2 run 4)" has the sequences):
- The stack form: in I host rsp is B + the guest's rsp. `push`, `pop` to a register, `call`'s push, `ret` and every
  operand `[rsp + d]` with no index and d no lower than one page below run as the guest wrote them; no stack probe is
  left (`window::probe_stack` is gone). An index or a lower d takes the window's form (GS at esp; exact: the offset
  less B, probed). `push m` and `pop m` cut their operand only. An instruction naming rsp as a value (`mov rbp, rsp`,
  `sub rsp, 16`, `lea rax, [rsp+8]`, `pop rsp`, `cmov`, `xchg`, `rdfsbase rsp`, `call rsp`) has it replaced by t or a
  guest register kept in a stand-in slot (carry.rs `StackValue`), loaded with rsp less B where read; where written,
  after the commit point the map holds the guest's rsp there, the exact form probes it, both cut it to 32 bits and
  make rsp `lea rsp, [s + u]` with u the base. `leave` is its own five instructions (six and the probe, exact).
- Layer 1 in I (test crate native.rs): the blob starts rsp at B + the case's; a case whose rsp is past 2^32 starts
  at it modulo 2^32 for the reference too (counted, "rsp past the window taken modulo 2^32"); `guest_state` takes rsp
  less B unless the span holds it; the exact form's probe after the commit point of an rsp write is a new ending
  `StackRefused` (the guest as the instruction left it, compared with the CPU); the wrapping form's stops between an
  rsp write and its cut compare rsp modulo 2^32; the second call of a float status instruction faulting in the
  wrapping form counts as the named landing when the exact form agrees with the CPU (`fnstenv [rsp+rax*2]`).
- Layers 2 and 3 and the benchmark blob (native_programs.rs): rsp starts at B + the guest's; the `pushfq` answer is a
  plain `push` of the flags on the guest's rsp.
- The window sweep (native_window.rs) now allows the stack's own accesses and `[rsp + d]` as written and checks every
  host write of rsp: a push or pop, a return's `lea rsp, [rsp + n]` (n at most 2^16 + 8), or `lea rsp, [x + y]` of the
  base loaded from the fixed part and a value cut to 32 bits. New shape tests `the_stack_runs_as_written` and
  `rsp_as_a_value_is_less_b_and_written_back_cut`; `stack_and_withheld_forms_are_made` gains six rsp forms.

Found by layer 1 in I while building (fixed): a `cmov` to rsp was taken as a write alone (t not loaded, so a false
condition wrote junk); the relocation of rsp came after the withheld registers' stores, so a stop between them read
a withheld register's old value (`shlx`, `mulx`, `bextr` naming r10 or r11 beside rsp).

Results (rented x86 machine, Xeon 8488C):
- Window cases of the plan (`the_window_cases`): wrapping 59 as the CPU and 5 the named landing, exact 64 as the CPU
  (load 6).
- Layer 1 in I, seed 5150, 20 cases a form (load 17 to 28): 0 forms differ in either form; exact ended StackRefused
  57 times; 7,692 runs started with rsp taken into the window.
- The window cases again with ten cases for the stack form added (`[rsp - 4096]` at rsp 0, `[rsp + 2^31 - 1]` at
  2^32 - 8, `[rsp + rax*8]` reaching 2^36, `mov rsp, rax` with 2^32 + 16 and inside, `sub rsp, 16` at 8 and inside,
  `pop rsp`, `leave` with rbp inside, at 2^32 - 8 and at 2^40, `ret 0xffff` at 2^32 - 8; load 4): wrapping 66 as the
  CPU and 10 the named landing, exact 76 as the CPU (3 ended StackRefused).
- Layer 1 in I, every form (2,008), 200 cases, both forms (scratch xn2r4/layers.out; 11:08 to 11:20 UTC, load 18 to
  33, 48 jobs): seed 5150 0 forms differ in either form (exact: 452 StackRefused, 51,061 Refused; 77,680 runs started
  with rsp taken into the window); seed 2611 0 forms differ (exact 469 StackRefused).
- Layer 2 in I, seed 23, 10,000 programs (load 28 to 12): 0 failing; 3,361 faulted; 1,038 runs the wrapping form's
  named landing; 227,396 blob runs (the same counts as run 3). Seed 7, 500 programs: 0 failing.
- Layer 3 in I, seed 5, 300 Csmith programs (load 12 to 4): 264 compared, 36 did not end in time on Linux, 0 failing.
- The x86 crate's tests and the window sweep (rented ARM64 machine, load 8): every test passes; seeds 0x5eed 20,000
  cases, 0x2b17, 0x51 and 0xdecaf 400,000 cases each (1,600,000 pieces each, about 377,000 strings): 0 outside.
  Clippy with warnings denied clean for wcontainers-x86, wcontainers-guest-cpu and the test crate.
- Speed, the benchmark set at full, `taskset -c 0-9`, best of 7, each form against P (the unchecked form), two runs
  (scratch xn2r4/bench1.out, bench2.txt):
  - 10:54 to 11:00 UTC, load 9 to 16: wrapping geometric mean 1.091 (counted 1.013), exact 1.323 (counted 1.254).
  - 11:01 to 11:08 UTC, load 6 rising to 36: wrapping 1.065 (counted 1.032), exact 1.280 (counted 1.302); per
    program wrapping and exact: fib 1.173 and 1.178, sieve 0.993 and 1.187, hash 0.999 and 1.002, sort 1.060 and
    1.249, interpret 1.414 and 1.776, matrix 1.055 and 2.603, walk 1.019 and 0.946, search 0.883 and 0.940.
  - A third run (11:28 to 11:37 UTC) is not a measurement of the forms: another job took the load from 3 to 76
    during it, doubling interpret's, matrix's and walk's times between ways (walk 0.59 times the host's); it gave
    wrapping 1.102 and exact 1.359 (scratch xn2r4/bench3.out). A fourth (11:38 to 11:47, load rising to 160 on 96
    cores) is not one either: wrapping 1.140, exact 1.432, search 1.64 and 2.27 (xn2r4/bench4.out). The bars want a
    confirming run on a quiet machine (load under 10) before XN2 is called met.
  - So with the stack as the guest wrote it the earlier figures (+9 to +12 percent wrapping, +40 to +46 exact) become
    +6.5 to +9 and +28 to +32 percent; fib's exact form went from 2.044 to 1.178.
  - interpret's wrapping form is 1.36 to 1.41 times P uncounted but 0.74 to 0.76 times P counted, its uncounted time
    (0.48 to 0.49 s) that of P counted (0.47 s) and its counted that of P uncounted (0.35 s), in both runs: a placement
    effect of the hot loop, I expect, not the form's cost; not studied.
  - The exact form's cost is now the probe of the other accesses: matrix 2.6 (six instructions and a load an access,
    four accesses in its inner loop). A cheaper probe is the next lever if the bar is tightened.

## XN2 run 3 (2026-11-24)

Built (crates/wcontainers-translator-test, commits 2a0c32b09 and 109720a14):
- Native layer 2's blob (native_programs.rs) runs in any way: `compare`, `run` and `run_timed` take `(Way,
  LookAhead)`. In I the program is at B + its address, the reference's stack copied to B + its address and then
  unmapped (so only the window holds guest memory), the probe's page at B - 2 pages, FS on the area (the guest's FS
  and GS bases kept in its words), GS on B, the fixed part's `window` words set. The blob's routines read the area on
  the way's segment. Its answers in I: a system call on a stack of the blob's own with the guest's flags kept, its
  guest pointers made host addresses around it (`write`, `writev` with each vector's base, `ioctl`'s argument,
  `set_tid_address`; `arch_prctl(ARCH_SET_FS)` writes the area's FS base word; `exit`, `exit_group` plain; any other
  ends the run with a record naming it); `pushfq` (which I sends to the inside) stored at the guest's rsp - 8 in the
  window, a fault there the guest's; a repeated string under the count with rsi and rdi B more around it.
- Tests: tests/layer2.rs `natively_matches_the_cpu_in_process_x86_64` and tests/layer3.rs
  `csmith_programs_match_linux_natively_in_process_x86_64` (both forms, or one with `WC_LAYER2_CHECK` /
  `WC_LAYER3_CHECK`), each at three look-aheads, counted and not. A wrapping run that differs is counted as the named
  landing only when the CPU's run ended by SIGSEGV and the exact form's run of the same program agrees with the CPU
  (the two forms make the same cut accesses; the exact form adds only the probes that refuse addresses at or above
  2^32). tests/benchmarks.rs `measure_benchmarks_natively_x86_64` times P, wrapping and exact (`WC_BENCH_WAYS`),
  each also against P (the unchecked form).

Results (rented x86 machine, Xeon 8488C):
- Layer 2 in I, seed 23, 10,000 programs (load 16 to 27): 0 failing; 3,361 faulted; 1,038 runs the wrapping form's
  named landing; 227,396 blob runs. Seed 7, 500 programs: 0 failing, P's run of the same 500 also 0 failing.
- Layer 3 in I, seed 5, 300 Csmith programs (load 27 to 12): 264 compared, 36 did not end in time on Linux, 0
  failing. Seed 1, 24: 22 compared, 0 failing (P also 0).
- Speed, the benchmark set at full, `taskset -c 0-9`, best of 7, uncounted, each form against P (the unchecked form);
  10:03 to 10:10 UTC, load 3.5 rising to 21 (the machine is shared): fib wrapping 1.355, exact 2.044; sieve 1.030,
  1.228; hash 0.994, 1.002; sort 1.074, 1.334; interpret 1.353, 1.819; matrix 1.010, 2.610; walk 1.104, 1.033;
  search 1.064, 1.227. Geometric mean: wrapping 1.115 (bar 1.10), exact 1.456 (bar 1.35); counted, against P
  counted: 1.006 and 1.368. Against the host's plain run: P 1.122, wrapping 1.251, exact 1.633. An earlier run
  (09:56, load 30 falling to 10, best of 5): wrapping 1.085, exact 1.405. So the earlier +2 to +4 percent
  (wrapping) and +11 to +21 percent (exact), measured with the stack exempt, are corrected to about +9 to +12 and
  +40 to +46 percent on translated programs with every stack access cut and probed. Single programs move by up to
  20 percent between runs on this machine (walk: exact 0.831 and 1.033 of P).
- The stack's share (scratch xn2r3/stackfloor.diff, applied only to the rented copy, never committed): the stack's
  accesses left as the guest wrote them with rsp holding B + the guest's rsp (pushes, pops, calls, returns, `leave`
  and every rsp-based operand; rsp read as a value not rebased, so a floor for that form, not the form). 10:22 to
  10:26 UTC, load 11.5 falling to 5, best of 7: wrapping against P fib 1.000, sieve 0.999, hash 1.001, sort 1.067,
  interpret 0.995, matrix 1.006, walk 1.223, search 0.988, geometric mean 1.032 (counted 1.014). fib and interpret,
  call-heavy, go from 1.355 to 1.000 and 1.353 to 0.995: the cut stack form is most of the wrapping form's cost.
  The exact form under the same patch did not finish (its fib differed or failed; not studied).
- Where the exact form's cost is (scratch xn2r3/dump6-*.txt, matrix's pieces in P and exact): every access
  `[base + index*8]` becomes `lea t; rorx u, t, 32; mov u32, u32; lea u, [u*8]; not u; mov u, gs:[u + 1 - 2 pages];`
  then the cut access, six instructions and a load more per access; matrix's inner loop has four.

The stack form, chosen by this measurement (plan 4: "keeps the cheaper that passes the way I tests"): rsp held as
B + the guest's rsp. Built in run 4 (above), as these points planned it:
1. Every piece in I keeps host rsp = B + guest rsp: `push`, `pop`, `call`, `ret`, `leave` and rsp-based operands run
   as the guest wrote them (no cut, no probe: the page below B and the 2^35 bytes under the probe's page, and the
   page and 2^31 bytes above the window, unmapped); an instruction that reads rsp as a value reads it less B (a
   stand-in loaded `lea s, [rsp + w]` with w = -B from the fixed part); an instruction that writes rsp other than
   by a push or pop (`sub rsp, n`, `and rsp, -16`, `mov rsp, r`, `lea rsp, [...]`, `pop rsp`, `xchg`) has its
   result rebased and cut so that rsp stays in the window: `lea rsp, [B + (result mod 2^32)]` flag-free. Readers of
   a thread's registers in I (the test crate's `guest_state`, later the executor) take rsp less B (a way-level rule,
   so the map needs no `Held` for it).
2. The open point, for the owner or the plan's next check: a guest rsp written outside [0, 2^32). The wrapping form
   cuts it (one more case of its named landing: the value read back differs). The exact form cannot cut it and
   stay exact; the cheapest sound answer is the probe on the written value, so that `mov rsp, rax` with rax at or
   above 2^32 faults at the write, a named difference (the CPU faults only at the next stack access, and a read of
   rsp would give the value). The alternative (the inside keeps the true value and runs the thread until rsp is back)
   costs nothing on the fast path but is a slow path of its own.
3. Layer 1 in I then starts each case with rsp cut or refused as above, and its known kinds gain that difference.
4. Measure again; the floor above says the wrapping form then meets its bar (1.032 before the rebased reads and
   writes); the exact form's remaining cost is the probe of every other access (matrix 2.6), for which the stack's
   share is not yet measured.

## XN2 run 2 (2026-11-24)

Built:
- Strings in I (`window::string`): each start the instruction uses probed in both forms (a start at or above 2^32
  is the inside's, counted by it), `mov esi, esi` / `mov edi, edi`, rdi rebased by B from the area's fixed part
  (new `FixedPart::window` and `window_negated`, read-only to the guest) around the copy, its rsi side on `gs:`;
  the map holds rdi as `Held::Rebased(rdi)` meanwhile (new in wcontainers-guest-cpu, with `Fixup::Offset`; the
  stop crate of arm64 has one `unreachable!` arm for each, commits 6d9ee13bc and fe6c66c38).
- The fixes layer 1 in I found, listed under "Departures from the plan (XN2)": pushes and pops to memory commit at
  their store; a call's lookup before its push; withheld registers' pushes and pops; the high-byte slot.
- Native layer 1 in I (crates/wcontainers-translator-test, `native::compare_in_process`): the blob maps the case's
  data and code at B + their addresses (B = 2^41), the probe's page at B - 2 pages and nothing else within 2^35
  bytes below, FS on the thread area, GS on B, the guest's FS base in the area; a fault's address in the window
  less B; a probe's fault is `Ended::Refused`, the guest as it started, answered by the meaning. Each case runs
  uncounted and counted with a stop after every host instruction (every intermediate stop's registers read through
  the map, `Rebased` and `Offset` included). Known kinds added: the wrapping form's landing modulo 2^32 (the
  reference faulting at or above 2^32), and a refused run's meaning differing from the reference (as the
  WebAssembly output's layer 1 does).

Results (rented x86 machine, Xeon 8488C):
- `the_window_cases_of_the_plan_match_the_cpu_in_process_x86_64` (plan 7's way I cases, 60 in each form at first: one byte
  past the data and across its end, 2^32 - 1 with 8 bytes and 2^32, a SIB index wrapping back into the data and one
  reaching 2^43 past it, `fs:` with negative offsets, `rep movsb` and `rep stosb` with counts 2^20 and 0x1800 up
  and down (DF) from inside the data and down from 0x10 past 0, `movsb`, `lodsb`, `rep stosb` with rsi or rdi at
  2^32, 2^32 + 2^35, 2^40 and -1, `bt`/`bts` with offsets -64, 2^35, 0x200000005 and 5, `push rax` and `call rax`
  with rsp at the data's bottom, 0, 8 and 2^32, `mov rax, [ebx]`, `stosb`, `bt` and `xlat` under 0x67, `maskmovq`
  at the data and 2^40 past it): wrapping 53 as the CPU and 7 the named landing; exact 60 as the CPU; 0 failing
  (load 14).
- Layer 1 in I, every form, 20 cases, seed 5150 (load 8 to 18): wrapping 0 forms differ; exact 1,
  `Fxrstor_m512byte` (`fxrstor [rsp-9]` with rsp past 2^47: reference general protection, the meaning a stack
  fault), which the WebAssembly output's layer 1 fails the same way at that seed; now counted as the meaning's
  difference (filed for the x86 track under "Coordination").
- Layer 1 in I, every form (2,008), 200 cases, seeds 5150 and 2611 (load 12 to 65, 646 s at 48 jobs): exact 0
  forms differ at both seeds (6,904 and 6,840 known cases: the reference's 5-level paging in 1,636 and 1,647 forms'
  worth, the meaning's difference after a refusal or an answer in 19 and 14 forms, Intel's transcendentals in 4);
  wrapping 0 forms differ at 5150, 1 at 2611: `vmaskmovps ymm6, ymm13, [r12+r12*2]` (c4 82 15 2c 34 64), its
  address at 2^64 - 6, the reference's fault at 2 after the 64-bit wrap, ours at 2^32 + 2 (the guard page): the
  named landing, which the classifier then recognised only by the reference's fault address at or above 2^32.
  Fixed in the test: a wrapping failure is the named landing only when the same case's exact form agrees with the
  CPU by a refusal or the inside's answer (the forms' accesses are the same cut instructions otherwise; the gathers,
  which the exact form sends to the inside, are judged that way). Rerun of the wrapping form at both seeds, 200
  cases (load 26 to 40): 0 forms differ; known cases 46,807 and 47,040, the named landing in 1,528 and 1,525 forms'
  worth, the 5-level reference in 96, Intel's transcendentals in 2.
- No regression in P: layer 1 natively for the changed forms (`Call_rm64`, `Call_rel32_64`, `Jmp_rm64`, `Push_r64`,
  `Push_r16`, `Pop_r64`, `Pop_rm64`, `Pop_rm16`, `Pushq_imm8`; 300 cases, seed 5150): 0 failing; the shapes, the
  inside's answers and the second call (7 tests) pass on the x86 machine; every test of wcontainers-x86,
  wcontainers-guest-cpu and wcontainers-executor-arm64-stop passes on the ARM64 machine, and clippy with warnings
  denied is clean for those and the test crate.
- The window cases rerun with `fxsave64` across 2^32 with and without 0x67 added: wrapping 57 as the CPU and 7 the
  named landing, exact 64 as the CPU.
- The speculation probe made from the emitted code (scratch xn2r2/specemit.c fed by the test
  `speculation_gadgets`, which prints the exact form's pieces of `movzx eax, byte [rbx]; shl eax, 12; mov eax,
  [rax+100000h]`, the same after `lodsb`, and `bt [rax], rcx; setc al; ...` up to the piece's first call, and an
  uncut control with each access's 0x67 and `mov esi, esi` taken out; output xn2r2/specemit.out): processor 5,
  load 34 to 60, 100,000 rounds each with seeds 0x1234567 and 0xbeef01: the cut forms read the secret 0 times
  in all six runs (the in-window byte 98,034 to 99,729 times); the uncut controls read it 99,121 to 99,770 times.
- The window sweep (rented ARM64 machine, now with strings, about one in six instructions, and direct branches):
  seed 0x5eed 20,000 cases (19,062 strings); seeds 0x2b17, 0x51, 0xdecaf 400,000 cases each (about 377,000
  strings each): 0 outside.

Left after run 2, where it stands after run 3: 1. speed per workload, measured (run 3; the stack form chosen, not
built); 2. layers 2 and 3 in I, done (run 3; the link and stop tests in I can use the same blob);
3. Register-group and hostile-guest tests in I, and the X9 programs: an executor (gap item 1).

## XN2 (run 1, 2026-11-24)

Built (crates/wcontainers-x86/src/carry/window.rs, reached only through `carry::access`, the one place):
- Every guest access in I is on GS (the window's base B) at a 32-bit address, in both forms. Wrapping: the
  instruction re-encoded with the address-size prefix and `gs:`, no instruction added. Exact: the flag-free probe
  of plan 4 (`rorx u, x, 32; mov u32, u32; lea u, [u*8]; not u; mov u, gs:[u + 1 - 2 pages]`, `area::PROBE_PAGE`)
  on the address the access reaches, then the cut access; x is the operand's base where it is the whole operand,
  else t from one `lea` (or `mov t, imm64` for an absolute address). Under the guest's own 0x67 the address is
  below 2^32 already: no probe, the cut access alone (the CPU wraps each part of it as the meaning does).
- The stack: `push` as a store at `gs:[esp - n]` then `lea rsp, [rsp - n]`, the commit point (a stop between them
  repeats the push: the same value at the same place); `pop`, `leave` and `ret` load into t from `gs:[esp]` or
  `gs:[ebp]`, rsp's move the commit point, t then holding the popped register for the map; `push m` and `pop m`
  through t (the popped value kept in the area word `kept_t` while t and u form and probe the destination, which an
  rsp base forms after rsp moves, as the CPU does); the exact form probes rsp - n by a `lea` into u (so rsp below 8
  faults), rsp or rbp itself for a read. A call through a register or memory pushes before the lookup's first
  level in I, so the push's probe takes u and no second stand-in is needed.
- `xlat` on `gs:[ebx + al]` (exact: probe on rbx + al; `67 d7` no probe, the wrapped address); `maskmovq`,
  `maskmovdqu`, `vmaskmovdqu` on `gs:[edi]` (exact: probe on rdi, none under 0x67).
- Bit tests with a register offset, exact form: A computed in u without a flag (64: `mov u32, 6; sarx u, r, u`; 32:
  `sarx` and `movsxd`; 16: `movsx u, r16` with t borrowed as the count, t kept in `kept_t` when it holds the
  operand's address), `lea u, [x + u*w/8]`, the probe on u, the access cut on t32 with its own offset register.
  Under the guest's 0x67: none, or on a segment the base added after the wrapped sum, as the meaning adds it.
- `fs:` and `gs:` operands: `lea t, [offset]` (32-bit under 0x67), `mov u, fs:[the guest's base word]` (new
  `RegisterArea::fs_base` in I, `gs_base`), `lea t, [t + u]`, the probe on t in the exact form, the access on
  `gs:[r11d]`; `rdfsbase` and `rdgsbase` read the words. The CPU's segment choice among several prefixes is iced's.
- The x87 and FXSAVE images and environments (`fxsave`, `fxrstor`, `fnsave`, `frstor`, `fnstenv`, `fldenv`), which
  the CPU makes in parts each at its own 32-bit address: the exact form also probes the last byte (the ninth
  check's `fxsave64` at 0xffffffd6), except under the guest's 0x67, where the parts wrap as the meaning's do.
- An instruction naming ah, ch, dh or bh whose address the exact form puts in t moves it into a free register of
  the first eight (kept in the first stand-in slot); an absolute `mov` (moffs) takes its ModRM form.
- Routes in I: `enter` and `pushf` go to the inside (Kernel; their questions exist), with the 0x67 strings,
  segment-prefixed implicit forms and exact gathers as before.

Tests (crates/wcontainers-x86/tests/native_window.rs, rented ARM64 machine, the x86 machine unreachable all run:
SSH banner timeouts at 08:35 and 08:40 UTC): `every_guest_access_is_cut_to_the_window`, a seeded sweep of
instruction mixes in both forms, counted and not, decoding every piece and failing any host access to memory that
is not the area on FS, the lookup's second level, the probe, or a GS access at a 32-bit address (strings and direct
branches are not made: strings are not built, and a branch can land on one inside another instruction); six shape
tests (plain, stack and its map, implicit, bit tests including `67 64 0f a3 08`, segment operands, images in
parts). Results (load 15 to 51): seed 0x5eed 20,000 cases, 79,388 pieces; seeds 0x2b17, 0x51 and 0xdecaf 400,000
cases each, 1,589,408, 1,589,296 and 1,589,232 pieces: 0 outside. `cargo clippy --no-deps -p wcontainers-x86
--all-targets -D warnings` clean; every test of the crate passes. The test crate was not rebuilt for x86-64 (its
x86 parts build only there; `RegisterArea` gained two words at its end, built with `..Default`).

Carried findings (the eighth and ninth checks), where each stands:
- [blocking] a string with the guest's 0x67 in I: routed to the inside (Kernel) in both forms, never re-encoded;
  the inside's run of it is the executor's (gap item 1's string run). Not yet tested by a run in I.
- `maskmovq` in the window: built (above), shape test `implicit_operands_are_cut`.
- The exact form's 0x67 segment operand: the base added after the wrap, then the probe (`a_segment_operand_*`, and
  the bit test's case).
- `67 d7`: the wrapped address, no probe (shape test).
- Crossing offset 2^32 under 0x67 (`fxsave64`): see the images in parts above.
None of these has yet run against the CPU in I: that needs the harness below.

Not built yet, in order (as run 1 left it; items 1 to 3 are done in run 2):
1. Strings in I (`carry/window.rs` refuses them by an assertion): the probe of rsi and rdi, `mov esi, esi` and
   `mov edi, edi`, rdi rebased by the window's base and back (plan 4). The map then needs plan 9's `Held::Rebased`
   (shared crate wcontainers-guest-cpu); adding the variant breaks the exhaustive match in
   crates/executor/implementation/arm64-stop/implementation.rs (`held`), another track's file, so it is to be
   agreed through the coordination section first. The fixed part also needs B and -B (plan 4's `fs:[base]`).
2. The in-process harness in the test crate: a blob with the window at B (guard page above and below, the probe's
   page of zeros, 2^35 bytes reserved without access), the area on FS, guest data at B + its address; native layer
   1 in I against the CPU (faults with exact registers, the guest's fault address from the meaning), and plan 7's
   way I cases (one byte past a mapping, 2^32 - 1 with 8 bytes, a far SIB, `fs:` with a negative offset, the strings
   with rcx large, rsi and rdi at 2^32, 2^32 + 2^35, 2^40 and -1, the bit-test offsets, a push at the stack's
   bottom, `call r/m` with rsp at 2^32 and 0).
3. The speculation test with these sequences (run8/specx3.c's gadgets made from the emitted code).
4. Speed per workload, each form against the unchecked one (bar 1.10 wrapping, 1.35 exact), and the stack form
   measured against rsp held as B + rsp (plan 4).
5. Link, register-group and hostile-guest tests in I, and the X9 programs: an executor (gap item 1).

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
   forms a chip's set holds is that design's to say (the fifteenth check confirmed it: with the flag false, `21
   c8`, and, is routed Kernel with no question, as are `Shl_rm64_imm8`, `Imul_r64_rm64`, `Idiv_rm32`,
   `Tzcnt_r64_rm64` and `Shld_rm64_r64_CL`). (Partial writes differ on AMD too, `fstp m80` and `fbstp`;
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
   - Counted speed (exact replay): interpret 1.66, the count's adds (the fifteenth check: geometric mean 1.214,
     interpret 1.645). Plan 8's speed bar does not limit itself to uncounted runs; holding it uncounted only is
     a departure, listed under "Departures from the plan (XN1)".
   - The note of an MXCSR read while not kept for `xsave` (the meaning warns; `answer` is not told tracking).
5. Way I (XN2, as written): the eighth check's way I findings (a string with the guest's 0x67 to the inside in
   both checked forms, the exact form's 0x67 segment operand and `67 d7`, `maskmovq` re-encoded in the window), and
   now the ninth check's crossing of offset 2^32 under 0x67 in the exact form (plan 4). Fixed in the plan, built and
   run against the CPU in XN2 (runs 1 to 5: the window cases, native layer 1 in I, the speculation probe). Left:
   the 0x67 strings' run by the inside in I is the executor's (item 1); the window cases run them in the test's
   harness.
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

## For the independent check of XN2

Ways to try to defeat each form (commands under "How to run XN2's tests"):
- The one place: every host instruction that may reach guest memory goes through `carry::access`, which in I sends
  it to `window::access` when iced's `used_memory` names an access (`window::reaches_memory`). A form iced reports
  as no access is therefore copied: prefetches, `nop` with memory, `cldemote` and the reserved-NOP space, all
  routed Nothing (no host instruction). Run 6 printed both forms of 49 such and nearby encodings (scratch
  xn2r6/probe.out): `clflush` is cut and probed like a load; `clflushopt`, `clwb`, `monitor`, `umonitor`,
  `ptwrite`, `movdir64b`, `movdiri`, `enqcmd`, the shadow-stack stores, `xsavec`, `xsaves`, `xrstors`, `clzero`,
  `tpause` and `invlpg` take the Trap route; `xsave`, `fxsave`, `lfs`, `lss`, segment register moves, `sgdt`, `lar`, `lsl`,
  `verr` and `str` go to the inside.
- Static: tests/native_window.rs, the seeded sweep (`X86_WINDOW_SEED`, `X86_WINDOW_CASES`) reads every piece back
  and fails any host access not on GS at a 32-bit address, the area on FS, the lookup or the probe, and any host
  write of rsp that is not a push or pop, a return's bounded move or `lea rsp, [B + a cut value]`;
  `every_form_writing_rsp_keeps_it_in_the_window` does the same for 63 encodings writing rsp, esp, sp or spl
  (the VEX and vector-to-register forms, which random bytes seldom make).
- Against the CPU: the window cases (892 a form) and native layer 1 in I with a new seed (`WC_LAYER1_SEED`);
  layers 2 and 3 in I.
- Speculation: the gadget printer and scratch xn2r5/specemit.c; add a gadget to `speculation_gadgets` as a guest
  sequence, the probe takes it from the printed line.
- Reach of the stack as written: rsp holds B plus a guest rsp of at most 2^32 + 2^16 + 8 (after `ret 0xffff` at
  the top; a second return reads `[rsp]` first and faults), so `[rsp + 2^31 - 1]` reaches at most 2^31 + 2^16 + 72
  bytes past the window's top, inside the 2^35 bytes the slot keeps unmapped (plan 4 and 6).
- What is not covered without an executor: the link, register-group and hostile-guest tests in I, the ymm upper
  halves across a call into the inside, the X9 programs (gap item 1); `maskmovdqu` in the speculation probe (no
  leak to see even uncut); `ret` as a gadget (its read is `pop`'s).

## How to run XN2's tests

Run 3's (rented x86 machine): `WC_LAYER2_CASES=10000 WC_LAYER2_SEED=23 WC_LAYER2_JOBS=48 cargo test -q -p
wcontainers-translator-test --test layer2 natively_matches_the_cpu_in_process -- --nocapture` (summary "layer 2
natively in process"; `WC_LAYER2_CHECK=wrapping` or `exact`); `WC_LAYER3_CASES=300 WC_LAYER3_SEED=5
WC_LAYER3_JOBS=48 ... --test layer3 csmith_programs_match_linux_natively_in_process -- --nocapture`; speed:
`WC_BENCH_RUNS=7 taskset -c 0-9 cargo test -q -p wcontainers-translator-test --test benchmarks
measure_benchmarks_natively_x86_64 -- --ignored --nocapture` (`WC_BENCH_WAYS=process,wrapping,exact`; lines "times
the unchecked form's"). Run 3's stack floor patch (scratch xn2r3/stackfloor.diff) no longer applies: run 4 built that
form. Scratch xn2r4/run.sh sends one command to the x86 machine, retrying while its SSH refuses. Speed over code
placements (run 6): scratch xn2r6/alignall.sh, sent as one command (`echo <its base64> | base64 -d >
target/alignall.sh && bash target/alignall.sh`), edits the machine's copy of native_programs.rs to each placement
(`PIECE_ALIGN` 16, 32, 64; `PIECES` 16 and 48 bytes later), runs the benchmark at each and puts the file back;
`python3 -I xn2r6/placements.py <its output>` gives the means over placements.

`orchestrator/cloud/arm-run <checkout> --name x86-native-output -- 'cargo test -q -p wcontainers-x86 --test
native_window -- --nocapture'` (any Linux machine: the tests decode, they run no guest code); the sweep takes
`X86_WINDOW_SEED` and `X86_WINDOW_CASES` (decimal, or hex with 0x for the seed); summary line "seed ...: N cases, N
pieces, ... 0 outside".

Run 2's tests on the rented x86 machine (its own reference): `x86-run <checkout> --name x86-native-output --jobs 48
-- 'WC_LAYER1_CASES=200 WC_LAYER1_SEED=5150 WC_LAYER1_JOBS=48 cargo test -q -p wcontainers-translator-test --test
layer1 every_form_matches_the_cpu_in_process_x86_64 -- --nocapture'` (both forms, or `WC_LAYER1_CHECK=wrapping` or
`exact`; summary lines "layer 1 in process, the ... form"); `cargo test -q -p wcontainers-translator-test --test
layer1 the_window_cases -- --nocapture` (seconds; summary "the window's cases"). The speculation probe:
`cargo test -q -p wcontainers-x86 --test native_window speculation_gadgets -- --ignored --nocapture | grep -E
'^(area|load|lods|bt|scas|xlat|pop|store|maskmov) ' > gadgets.txt` (any machine; the `area` line first), then on the
x86 machine scratch xn2r5/specemit.c built with `cc -O2 -mno-red-zone -fno-stack-protector -mfsgsbase` and run as
`taskset -c 5 ./specemit 100000 <seed> < gadgets.txt` (copies in /work/probes/xn2r5 there).

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
- Plan 8's speed bar (geometric mean at most 1.25, no program above 1.6) is held for uncounted runs only. Counted
  (exact replay) the count's plain adds take interpret above 1.6 (1.658 in run 5, 1.645 in the fifteenth check's
  run: Xeon 8488C, cores 0-9, best of 5, load 18.6 falling to 6.5; geometric mean 1.214). Lowering it is gap item
  4's; the guest's own run, which replay does not ask for, is the one the bar was set for.

## Departures from the plan (XN2)

- The exact form uses the flag-free probe for every access, not `cmp t, fs:[limit]; jae out` where the flags are
  dead: a fault taken after that `cmp` would show the guest the check's flags, not its own, and faults must carry
  exact registers (plan 8). So no limit word exists; the ceiling is 2^32 by the probe itself. Its cost is XN2's
  speed measurement to show.
- (Run 1 made a push commit at rsp's move after its store; native layer 1 in I, run 2, showed a stop between them
  sees the stored bytes with the push not done. Run 2 follows the plan: the store is the commit point, rsp's move
  plan 9's `Fixup::Offset`, now in wcontainers-guest-cpu. A pop to memory commits at its store the same way.) A pop
  to a register, `leave` and `ret` load into t and commit at rsp's move with t relocated.
- `pushf` and `enter` go to the inside in I (plan 4 builds `pushf` on the count stack): both are rare in compiled
  code, and the inside's answers exist.
- A call through a register or memory in I: the exact form's probe of rsp - 8 comes before the lookup's first
  level (u is free there), then the first level, then the push from the rax stand-in as in P, so no second
  stand-in (plan 4 counted one). Run 1 had put the first level after the push, which layer 1 found: a target past
  user space then faulted with the call's push done.
- A withheld register's push in I is t's, loaded from its slot; its pop writes the slot (the window's pop), never
  the slot form `pop fs:[slot]`, whose implicit stack access would not be cut (found by layer 1 in run 2: `pop r11`
  read the host's rsp, outside the window).
- An instruction naming ah, ch, dh or bh whose address the window puts in t takes the second stand-in slot: the
  first may hold the register `carry::on_legacy` took for a RIP-relative operand (found by layer 1 in run 2, `add
  dh, fs:[rip+d]`).
- A 16-bit bit test borrows t as the count instead of a stand-in v; bit tests need no v in any width (the probe
  runs in u in place).
- The exact form also probes the last byte of the forms the CPU makes in parts (answering the ninth check).
- (Run 4) The stack: rsp held as B + the guest's rsp, the stack's accesses as the guest wrote them, chosen by run 3's
  measurement as plan 4 asked; the run 1 and 2 forms of pushes and pops above (commit points, fix-ups, the probe of
  rsp - n, a call's probe before the lookup) are gone with it. `leave` commits at rbp's write with the guest's rsp in
  u, not at rsp's move. A guest rsp written outside [0, 2^32) is cut (wrapping) or faults after the write (exact),
  named in plan 5; a thread starts with rsp in [0, 2^32], and a case of layer 1 past it starts modulo 2^32.

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

- native-without-virtual-machine and the ARM64 native back end (asked in XN2 run 1, no answer by run 2, so made
  in run 2 as proposed): wcontainers-guest-cpu's `Held` has `Rebased(u8)` (the guest's value is the host register
  less the window's base B, rdi while a string runs in I) and `Fixup` has `Offset { register, by }` (rsp after a
  push's store, plan 9); crates/executor/implementation/arm64-stop/implementation.rs has one `unreachable!` arm
  for each (ARM64 code makes neither), commits 6d9ee13bc and fe6c66c38, each alone. Its tests still build (clippy
  of the crate with all targets on the ARM64 machine); replace the arms with that track's own answer if wanted.
- x86 track (XN2 run 2): `fxrstor [rsp-9]` (0f ae 4c 64 f7) with rsp past 2^47, layer 1 seed 5150 at 20 cases: the
  reference gives general protection (the misaligned operand), the meaning a stack fault (the address); the
  WebAssembly output's layer 1 fails it the same way (`WC_LAYER1_FORMS=Fxrstor_m512byte WC_LAYER1_CASES=20
  WC_LAYER1_SEED=5150`). In I the exact form's probe refuses that address and the meaning answers, so native layer 1
  in I counts it as the meaning's difference. Way I's area also
  gains `RegisterArea::fs_base` (the guest's FS base) and `kept_t`, and reads the window's page of zeros at B - 2
  pages (`area::PROBE_PAGE`); I's runner maps that page readable and the 2^35 bytes below it without access, as plan
  4's guards say.

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
  The fifteenth check (seed 16161616, 3,000,000 byte-shaped cases: only `Sgdt_m1664` 29 and `Sidt_m1664` 31 fail,
  all routed Kernel, ours equal to the meaning, no new kind): `26 2e 36 45 0f 01 8c 3b b1 09 d2 6b` (three
  segment prefixes: the reference's general protection, ours writes 10 bytes at data offset 11871); `26 4d 0f 01
  04 12` (sgdt [r10+rdx], r10 0xd5311133c6d1170b, rdx 0x25ef5f42d75d30c9: the reference's memory fault at
  0xfb2070769e2e47d4, ours general protection); `2e 3e 41 0f 01 87 b8 2d 0c cd` (the reference's general
  protection, ours a memory fault at 0x4400338a).
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
- Way I's stack form (built in XN2 run 4): rsp held as B + the guest's rsp, the cheaper by far. A guest rsp written
  outside the 4 GiB window is cut by the wrapping form and faults at the write in the exact form, both named
  differences (plan 5). Example: `mov rsp, rax` with rax 2^32 + 16, then `mov rbx, rsp`: the CPU gives rbx 2^32 + 16
  and faults only at a later push; the wrapping form gives 16, the exact form a SIGSEGV at the `mov rsp, rax` with
  the instruction done. Open for the executor: an answer of the inside (`popf`, `enter`, `pushf` in I, a signal
  return) that leaves rsp at or above 2^32 in the exact form has no host form to resume in; proposed, the same
  fault after the answer.

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

- XN2 run 6: no `unsafe` and no import added (one test in native_window.rs; the probes and patches are scratch).
  Tried to defeat both forms: 49 encodings whose operand iced may call no access (cache-line, prefetch, monitor,
  shadow-stack, direct-store, xsave and segment forms) none reaching memory uncut, and 63 encodings writing rsp
  (VEX, vector-to-register and the rest) none leaving rsp outside the window's rule; no hole found. Hostile-guest
  suite in I: not yet (needs an executor, gap item 1).
- XN2 run 5: no `unsafe` and no import added (test cases and a gadget printer only; the C probe is scratch). The
  isolation was tried against the CPU through every way I access shape of plan 7 that runs without an executor (892
  cases a form, none reaching outside the window), and the speculation probe now covers a string from rdi, `xlat`,
  `pop` and a plain store beside run 2's three: 0 secret reads in the cut forms, the uncut controls reading it in
  nearly every round. Not covered by it: `maskmovdqu` (no control leak to see) and `ret` (not made). Hostile-guest
  suite in I: not yet (needs an executor, gap item 1).
- XN2 run 4: no `unsafe` added (grep of carry.rs, carry/window.rs, native_window.rs and the test crate's native.rs and
  native_programs.rs finds none); no import added. The isolation now rests also on rsp: every host write of it is a
  push or pop, a return's bounded move, or the cut `lea rsp, [s + base]`, which the window sweep checks on every
  piece; an `[rsp + d]` made as written reaches at most one page below the window (guard) or 2^31 + 2^17 above it
  (unmapped by the slot's layout). Two holes found and fixed while building, both by layer 1 in I, neither an escape:
  a `cmov` to rsp with a false condition wrote t's junk (inside the window, cut), and a stop between rsp's write and a
  withheld register's store read that register's old value.

- XN2 run 2: no `unsafe` added (grep of carry.rs, carry/window.rs, native_window.rs and the test crate's native.rs
  finds none); the C speculation probe is scratch, never built into anything shipped. The in-process way's
  isolation: one window escape found and fixed by native layer 1 in I, a withheld register's pop written as the
  slot form `pop fs:[slot]`, whose implicit read of the stack was on the host's rsp, not cut to the window (the
  window sweep had not drawn it). Every guest access either form emits is cut to 32 bits on GS (the sweep, now with
  strings: 0 outside in 1,220,000 cases), and the speculation probe made from the emitted code read the secret 0
  times in 600,000 rounds where the uncut control read it about 99 percent of the time. The window base B and -B
  are words of the fixed part, which the guest cannot write; the guest's FS base, which it can (area word), only
  ever enters an address that is then cut. Hostile-guest suite in I: not yet (needs an executor, gap item 1).
- Unsafe code: none in this track's code (carry.rs, carry/window.rs, native.rs, area.rs, strings.rs, answer.rs,
  x87.rs, fpu.rs, transcendental.rs, the test crate's native.rs, native_programs.rs and the tests, native_window.rs
  included; grep finds no `unsafe`, XN2 run 1); the
  translator and wcontainers-x86 keep the workspace ban; emitting code into a byte buffer needs none, and the test
  crate's blobs are machine code the reference runs. No marked exceptions. (The test crate's tests/reciprocals.rs
  and tests/test_guest.rs hold `unsafe` blocks; neither is this track's.) Imports to vet: none new (the list
  above, each already in the lock).
- `answer` writes only the guest's own block and reaches guest memory only through `answer::Memory`, each access all
  or nothing, so a fault leaves only the parts the reference leaves. The register area's home words and withheld
  slots hold only guest register values; no host address is ever kept there.
- Hostile-guest tests: not run, they need an x86 executor (gap item 1). XN2 run 1 adds way I's static check that
  every guest access either form emits is on GS at a 32-bit address (4.8 million pieces, 0 outside); no way I
  access has yet run against the CPU (XN2's harness, item 2 of "Not built yet"). The register area's new words
  hold the guest's FS base and a guest address t held (`kept_t`), never a host address.
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
