# x86-64 native output: status

Plan: docs/x86-native-output.md. Brief: pyro-parked/wcontainers-scratch/orchestrator/relaunch-agents/
x86-native-output.js. Scratch: pyro-parked/wcontainers-scratch/x86-native-output (probes/, results/, and check-xn0/
and check-xn0b/, the two independent checks' probes and outputs).

## Where things stand

- XN0 (the plan, checked before anything is built): done. Run 1 (2026-11-23) wrote the plan; the first independent
  check found seven blocking and eleven minor problems, which run 2 fixed. Run 3 (2026-11-23) made the second
  check (probes in scratch check-xn0b/): three blocking and ten minor problems, all fixed in the plan (list below)
  and measured where measurable. The run-3 fixes have not had a third check. Nothing is built; no code of this
  track exists yet.
- XN1 and XN2: not started. XN1 starts with the reconciliation points under Coordination.

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
- Taken from cloud/x86/x86-machine.md, not measured by this track: XSAVE 11,008 bytes, a signal frame's xstate_bv
  0x207, UMIP emulating sldt, str and smsw, split-lock detection.

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

## Coordination with other tracks (read theirs at the start of every run)

- native-without-virtual-machine (docs/native-without-virtual-machine.md, the P/I plan, written the same day as
  this one). Asked of it, to reconcile before XN1 or its W1:
  - x86-64 has no slot table and no lookup cache: the mirror's first level per thread area and second levels per
    code region live in its "our area" [2^44, ~2^47) (its 3.1 still says x86 uses a lookup cache).
  - I on x86: the thread area is on FS (its 3.8's `wrfsbase` swap at entry and exit stays, but the value during guest
    code is our area, not the guest's FS base) and GS is the process's window base; one area per runner, the
    process's first level mapped in at a process switch. The x86 window (2^32 plus 2^35 and two pages below, one
    above) fits its 2^36 slots; the x86 guest ceiling in I is 2^32, not 2^r.
  - Calls into the inside in P and I: `lea t, [rip]; jmp seg:[entry + 8*kind]`, nothing pushed; its stub keeps t in a
    register, as its 3.7 requires; resuming at t is safe because t never sat in guest-writable memory.
  - Signals: resumption at a guest boundary through the lookup, as its 3.3; the rep-string twin is entered that way.
    P's handler never returns by `rt_sigreturn` (its 3.7 says so; the plan's 3.7 has the measurement and the
    jump-out it measured); t and u from a frame copy are never used, so P has no hardware single step (stepping
    pieces instead). In I the handler sets the host's FS base before any Rust code and the area's again before
    guest code; its 3.8 swaps FS at entry and exit only.
  - The stub's routines: t is r11 and c is r10, which a host `syscall` overwrites or takes as an argument.
  - Links in P: the embedder writes the code; a stub thread runs `membarrier(PRIVATE_EXPEDITED_SYNC_CORE)` in the
    stub's process at the embedder's request, once per batch.
  - P on a 4-level host: about 3,000 guest threads per process (32 GiB of first level each).
  - What each way must give: plan section 2 (read-only part of the area, no host address in guest-writable memory).
- KVM back end (the floor): the thread areas at one virtual address mapped per virtual processor through its own
  top-level entry, so no thread reaches another processor's register area (plan 2); guest page tables 4-level.
- kernel: `rt_sigreturn` with cs 0x23 refused by name ("32-bit code segment in sigreturn, not provided", SIGSEGV);
  the kernel's writes into translated pages drop them (plan 3.7).
- x86 and translator tracks: the rep string count (plan 3.6 and 9) changes the WebAssembly output's count if ruled;
  shared code changes (plan section 9) go in small commits that keep ARM64's suites passing.
- instruction design: the x86-64 `ProcessorArchitecture` and precise instruction sets do not exist yet; the route of
  undefined-result and partial-write forms takes the host's match from them once they do.

## For the owner

- How a rep string instruction counts (plan 3.6): today the WebAssembly output counts passes, which depend on data;
  proposed: its elements, as code in plan section 9.
- Split-lock detection on cloud hosts (plan 3.6, 5): a named difference in P and I, or the inside completes the
  access.
- Decision 14 for x86 (plan 10): partial-write and undefined-flag forms routed by the probe's match per host.
- P on a 4-level host: about 3,000 guest threads per process (plan 3.3, 5).

## Licences

No dependency added. XN1 adds iced-x86's `encoder` feature (iced-x86 1.22.0, MIT, already a dependency).

## Security

- Unsafe code: none in this track; none planned (the translator and wcontainers-x86 keep the workspace ban; emitting
  code into a byte buffer needs none).
- Hostile-guest points the plan answers: every host address translated code jumps to is a word of our tables for any
  guest value (the mirror, plan 3.3, measured over 37 targets); no host return address is ever pushed or read back
  from guest-writable memory (plan 3.6); the guest cannot set or read the thread area's segment base (plan 3.1, the
  routing test); prefetches are never copied; strings in I are bounded at their start (plan 4); `xsave` never runs
  with the guest's mask on the host's XCR0; 32-bit code is never entered, sigreturn included; the inside's Rust in I
  cannot clear the guest's ymm upper halves (XSAVEC); P never resumes through a signal frame a sibling can edit
  (measured: 198,432 of 200,000 diverted when it does) and has no hardware single step; the machine maps each
  processor's area only for that processor; I's handler restores the host's FS before Rust code. Points it hands to
  P: writable pages of the guest's host process, the signal frame and other threads' register areas included, are
  the guest's.
- Tests for these: plan section 7 (shapes, way I, routing).

## How to run the probes

`tar cf - <probes> | orchestrator/cloud/x86-run --ssh 'mkdir -p /work/probes/xno && cd /work/probes/xno && tar xf -
&& gcc -O2 -o mirror3 mirror3.c mirror3.S && taskset -c 5 ./mirror3 30 21'`; the same for returns (`returns.c
returns.S`, `./returns 30 21`) and the partial probes (`gcc -O2 -mavx2`, no arguments); `python3 -I shapes86.py <ELF
files>`. Run 3's: `gcc -O2 -mavx2 -mfsgsbase -fno-stack-protector -pthread -o psig psig.c && ./psig 200000` and
`gcc -O2 -mfsgsbase -o wrfs wrfs.c && ./wrfs`, in /work/probes/xn0b.
