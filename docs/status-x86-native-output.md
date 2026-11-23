# x86-64 native output: status

Plan: docs/x86-native-output.md. Brief: pyro-parked/wcontainers-scratch/orchestrator/relaunch-agents/
x86-native-output.js. Scratch: pyro-parked/wcontainers-scratch/x86-native-output (probes/, results/, check-xn0/,
check-xn0b/ to check-xn0e/, the five independent checks' probes and outputs, and run4/ to run6/, those runs'
probes).

## Where things stand

- XN0 (the plan, checked before anything is built): done. Run 1 (2026-11-23) wrote the plan; the first independent
  check found seven blocking and eleven minor problems, which run 2 fixed. Run 3 made the second check (scratch
  check-xn0b/): three blocking and ten minor, fixed. The third check (scratch check-xn0c/) found three blocking and
  nine minor, fixed by run 4 (scratch run4/). The fourth (scratch check-xn0d/) found two blocking and six minor,
  fixed by run 5 (scratch run5/). The fifth (scratch check-xn0e/) found three blocking and six minor; run 6 fixed
  all nine (list below), measured in scratch run6/. The run-6 fixes have not had a check of their own.
  Nothing is built; no code of this track exists yet.
- XN1 and XN2: not started. XN1 starts with the reconciliation points under Coordination.

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
- The handler measured was not the one specified (minor): measured again with the handler as specified, all in
  assembly with no call or push on the alternate stack, the image read at its fixed place (2,880 bytes below the
  top on this host) for 832 bytes, every deciding word read-only, the resume through a read-only table (plan 3.7).
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
    while a sibling ORs TF and AC into the copy's rflags and stores handler addresses into its r10 and r11, 3 runs:
    3 to 7 taken in guest code, 55,133 to 63,419 in the tail, deepest frame 3 KiB, flags, r12..r14 and ymm7 kept,
    exit 0; round trip 930 to 942 ns against 1,078 to 1,097 with `rt_sigreturn` (3 rounds of 200,000 in each of 3
    runs).
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
  with one unmasked runs its x87 and MMX instructions in the inside (native.md 3.4 copies x87 arithmetic always,
  which leaves FDP and FOP to the host). FIP is kept by a store a run in the register area, not taken from the
  hardware.

## Coordination with other tracks (read theirs at the start of every run)

- native-without-virtual-machine (docs/native-without-virtual-machine.md, the P/I plan, written the same day as
  this one). Asked of it, to reconcile before XN1 or its W1:
  - x86-64 has no slot table and no lookup cache: the mirror's first level per thread area and second levels per
    code region live in its "our area" [2^44, ~2^47) (its 3.1 still says x86 uses a lookup cache).
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
    offered groups' part of the vector image at its fixed place below the alternate stack's top (learnt from its
    first frame; 2,880 bytes on the Xeon for a process without AMX permission, which the stub never asks for), never
    through `fpregs`, loads a new thread's groups from our kernel's image before its first instruction, and its threads
    have no user shadow stack (refused by name otherwise). I's handler resumes by `rt_sigreturn` with the frame's
    rip set to the lookup's entry; I's routine clears TF and AC before Rust code.
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
  (measured: 198,432 of 200,000 diverted when it does) and has no hardware single step; P's XRSTOR mask is the
  offered groups, never the frame's (measured: the frame's mask let an edited PKRU through); every `popfq` in P
  reads a word of a read-only table, so no sibling store can set TF or AC (measured: clearing a writable word first
  let 6 of 20,000,000 through, the table 0); P's handler unblocks the stop from a read-only set, never the frame's
  mask, and keeps the alternate stack to one frame under a flood of stops (measured: 3 KiB deep at 200,000, with
  the handler as specified, run6/presume.c); P's stub never jumps to a host address a stop may have exposed to a
  sibling: it leaves through a read-only word the embedder writes only for a call return point its map knows;
  P's handler writes nothing of ours on the alternate stack and reads the vector image at a fixed place, never
  through `fpregs`; an `int3` hit on a `jcc` exit chooses only between two host addresses of the map (measured: 0
  wrong arrivals over 16 conditions);
  no code is changed while another processor may run it except by the `int3` protocol
  (measured over 20,000 links with 64 threads running the exit); the code area is never read as data, so it may be
  execute-only; the machine maps each processor's area only for that processor; I's handler restores the host's FS
  before Rust code. Points it hands to
  P: writable pages of the guest's host process, the signal frame and other threads' register areas included, are
  the guest's.
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
./presume 200000 200000` (stops, round trips).
