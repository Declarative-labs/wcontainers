# Native without a virtual machine: status

Plan: docs/native-without-virtual-machine.md. Branch: agents (shared worktree). Scratch:
pyro-parked/wcontainers-scratch/native-without-virtual-machine.

## Where busybox sh's factor of four goes, and why the bar of 1.25 cannot be met by this design (2026-11-24)

The test: busybox `sh` starting 1,000 programs (`/bin/true` in a loop) under form P against the model, release, CPU
41 of Graviton4, /dev/kvm denied, the guest's stubs pinned to the same CPU. Machine load 14 to 33 on 96 cores
during the runs (other tracks' jobs; CPU 41 was not theirs: 2.6% of its samples were other processes). Scratch
w1/account: account.sh, account2.sh, account3.sh (one command each), their outputs account1-3-arm.out, the remote-only
timers diag.py, diag2.py, diag3.py and the sample sorters samples.py and idle.py.

How it was measured. The plain build's second run took 4.75, 4.76, 4.84 and 4.90 ms a program in four runs (the
model 1.08 to 1.19 ms: the reference 0.37 to 0.42 ms, busybox natively on the same host kernel in Docker; plus two
bare stubs made and ended, 0.29 to 0.34 ms each; plus 26 round trips of 2.9 us): 4.0 to 4.5 times the model. Then
timers on the embedder's thread, each instant charged to the innermost timed function and to the kernel exit being
handled, which add up to the whole run (the stubs run on the same CPU, so a turn's time holds the stub's work); they
add about 11% (5.36 ms a program against 4.81). Below, each part's share of the timed run, then that share of the
plain 4.81 ms. Then a cpu-clock profile of CPU 41 with call chains, split by who ran (embedder, stubs, idle, others)
and by host system call, for what happens inside a turn.

The parts, per program:
- Loading translated pieces: 32%, about 1,550 us. 434 pieces: 333 into the exec'd busybox (each exec at a new random
  base, so every piece is moved) and 101 into the fork child (at its parent's base, but its code area starts
  empty). About 3.6 us a piece (timed: finding the kept translation by its key 0.67, writing its moved bytes and
  address places 1.85, placing 0.23, its slot 0.22, last targets 0.13, the walk 0.4), plus writing the changed
  pages into the code area's memfd with the cache maintenance, 0.21 ms a program. The model has none of it.
- Turns: 33%, about 1,590 us. 54 a program: 26 kernel entries (21 system calls, 5 faults the kernel answers), 16
  copy-on-write faults the executor answers itself, 11 stops at a piece not loaded yet, 1 applying a fork child's
  commands. The transition itself is the round trip, 2.9 us (0.16 ms for 54). The rest is host work in the stub:
  34 turns carry 156 commands a program, of which 88 map one slot-table page each and send the code area's
  descriptor again with it (96 descriptors sent a program with 6 file blocks and 2 code areas), 51 protections, 7
  maps, 2 others. Timed, a turn with commands took 47 us (a new memory's) or 62 us (a fork child's), one without
  10 to 12 us. In the profile: the stub's mmap 6.1% of the CPU, its system-call entries and exits 5%, page faults
  3.3%, recvmsg 2.9%, mprotect 2.4%, futex 2.5%; the embedder's sendmsg 3.0%; the guest's own translated code only
  1.8% (about 90 us a program) and the floor 0.8%.
- Stubs: 16%, about 780 us, 2 a program (the fork child's and the exec's), 390 us each: the start 220 (the zygote's
  fork, the stub mapping its pages, installing its filter: the filter's install alone is 3.4% of the CPU, about 80
  us a stub), the end 150 (waiting for its exit: the host tearing its mappings down is 4.7%), the reap 23. The
  model counts two bare stubs made, killed and reaped, 300 us each.
- The kernel reading the whole program file at each exec: 5.5%, about 265 us (two reads of 146 us; the kernel
  track's `loader::program_of`, below under Requests to other tracks).
- The kernel's own work: 5.0%, about 240 us (exec 130, exit 45, every other call under 15).
- The kernel's memory calls: 8.1%, about 390 us: dropping a memory at exit and exec 170 (its code area's records,
  descriptors and mappings), the executor's copy-on-write copies 75 (7 copies of 8 pages), the faults' unmapping of
  shared blocks 36, the kernel's reads and writes of guest memory by position 60, the fork's duplicate 21, the rest
  under 10 each.
- Also seen in the profile: 7.7% of CPU 41's samples idle; a trace of every switch on CPU 41 for 2 s in a later run
  found it never idle, so that is not a wait of ours that recurs. Not explained.

The floors: each part done as well as it can be while every guest process stays a host process with its own code
area, guest memory in a memfd, and the kernel in the embedder.
- Loading: about 0.3 ms. Each new process's code must be written into its own code area; that write is measured
  (0.21 ms timed, about 0.19 plain). The rest is per-piece work (3.4 us) that could be a lookup and a copy
  (expected about 0.25 us a piece, 0.1 ms; not measured).
- Turns: about 0.31 ms beyond the guest's own code: the 26 kernel entries' round trips (0.076, the model's), the 16
  copy-on-write faults' round trips and 7 copies (0.05 and 0.05, measured), no stop for a piece (pieces loaded ahead
  completely), per new process the code area and slot table mapped once with one descriptor each and the file's
  blocks (about 10 mappings and 8 descriptors a program, expected 5 us each, 0.05), and the fork child's first
  touches that a host fork would not take (expected half of the measured 0.16 ms of faults). The guest's own
  mappings and first touches are in the reference too.
- Stubs: the model's 0.62 ms. A real stub's end costs 0.15 ms a program more than a bare one's kill, but that is
  the host tearing down what the program mapped, which Linux's own exit also does.
- The file read: 0 (read the headers only, as Linux does).
- The kernel's own work and memory calls: the model already holds Linux's own fork, exec, exit and wait inside the
  reference, so the floor is ours being as fast as Linux's: nothing beyond the model. Today 0.63 ms against about
  0.31 ms of the reference that is not the guest's code (taking the guest's code as P's measured 90 us).
- Sum: the reference 0.40 + the stubs 0.62 + the turns' 0.31 + loading 0.3 = about 1.63 ms a program, 1.4 times a
  model of 1.15 ms (1.37 to 1.51 over the measured models of 1.08 to 1.19 ms).

So the bar of 1.25 cannot be met by this design: even done as well as it can be, form P starting programs costs
about 1.4 times the model, because every new host process must be given its translated code and its code area's
mappings, which a native exec does not need. The bar this design can meet is about 1.5 times the model. Today it
is 4.0 to 4.5.

The changes of design that would move it, ranked by what they remove, measured in the timed run:
1. One translated image shared by the processes that run the same file at the same base: a fork child maps its
   parent's code area and slot table instead of loading again, and an exec maps the image kept for that file at
   that base. The fork-child half removes 101 loads, 34 slot-page commands with their descriptors and the child's
   code-area set-up: of its 0.35 ms of loads and 0.44 ms of command-carrying turns timed (about 0.7 ms plain, 15%),
   all the loads and most of the turns. The
   exec half removes the other 333 loads and the exec'd process's code-area set-up: about 1.4 ms timed, 1.25 ms plain
   (26%), but only when the exec lands at the same base, so it needs either the kernel to keep a file's base across
   execs (fewer random bases than Linux gives: the owner's ruling) or translated code that does not depend on the
   base (the translator's). With both halves loading's floor goes to a few mappings and the sum to about 1.28 ms,
   1.1 times the model (expected from the measured parts). The question is decision 16 of docs/native.md (loaded
   translations shared between processes) asked for P; until the owner rules it is built behind a private stand-in.
2. Fewer host calls inside turns: the stub keeps the code area's descriptor and maps the slot table whole once, so
   no slot page needs its own command and descriptor. It removes the embedder's 96 descriptor sends a program (0.20
   ms timed), the stub's receives (2.9% of the CPU, about 0.14 ms) and most of its 88 slot mmaps (within its 6.1%
   mmap share, up to 0.29 ms): expected 0.4 to 0.6 ms a program (8 to 12%). No ruling needed; built first.
3. Not ranked: a pool of stubs started ahead takes their start (0.22 ms each) off the program's path only while
   another CPU is free, and the model counts stub creation anyway, so the ratio does not move on one CPU.
What neither removes: the per-piece cost of whatever is still loaded (3.6 us against a floor under 1 us) and the
kernel's whole-file read (the kernel track's). Runs 2 to 4 build 2, then the fork-child half of 1, each measured
before and after, alternating, on a quiet machine.

## Round 4 run 2 (2026-11-24): the account's change 2 built and measured

- Built (commits 7c765bfd0, 2529908da): the code area's memfd is the code area then the whole slot table (2^46 bytes,
  sparse), and a new memory's code is mapped by two commands at once, `MapCode` and `MapSlots`, each sending the memfd
  once; `MapSlots` maps the whole table execute-only, so no slot page needs its own command, descriptor or host
  mapping any more, and "slot pages exhausted" is gone. The filter allows that one mapping exactly (address, length,
  execute-only, shared and fixed, the received fd, offset the code area's size). The embedder maps only the code area
  for cache maintenance, and 2 MiB windows of the slot table only on hosts whose `CTR_EL0` asks for maintenance by
  line (not Graviton4, whose DIC and IDC are set, so that path is not run on our machines). A jump to a page with no
  slot written reads zeros (`udf`, the empty slot) and allocates that page in the memfd; the stop records it and the
  next flush punches it back out if nothing was written there (counter `slot_pages_given_back`; a refused punch is the
  slow path "slot page kept").
- Tests: tests/stub.rs `the_slot_table_is_mapped_whole_once_from_the_code_area_s_memfd` (one `--xs` mapping over
  exactly the table, a slot written by position read back through /proc, an empty slot reads zero) and four new
  refused mappings in the shared-block test (one page of the table, the table readable, from another offset, the code
  area's bytes over it); tests/run.rs `slot_pages_a_jump_reached_and_nothing_wrote_are_given_back` (16 jumps into
  data pages, each page given back at the next turn). Graviton4 with /dev/kvm denied: stub 7, run 11, kernel 12,
  translated 2, floor 6, step 9, busybox 1 passed; conformance 20 of 21, the 21st the known precise-set test at
  0x1000 (owner's item); clippy with warnings denied clean (11:03 to 11:17 UTC, load 7 to 12). x86-64: the machine
  is down ("not set up", 11:12 UTC), so no x86 verdict this run; the library type-checks for x86_64-linux-musl on the
  Mac except an existing musl-only `libc::tgkill` in stub.rs (glibc has it).
- Measured, busybox `sh` starting 1,000 programs, CPU 41 of Graviton4, /dev/kvm denied, before (the committed tree,
  built as speed-before) and after alternated, 7 rounds (scratch w1/r4b-run2, ab1-arm.out and ab2-arm.out), the
  second run of each in ms a program: before 4.25, 4.34, 4.24, 4.99, 4.40, 4.60, 4.97; after 3.71, 3.72, 4.48, 4.36,
  3.87, 4.44, 4.41. Load 8 to 9 for rounds 1 to 3, 14 to 17 for rounds 4 to 7 (other tracks). Paired by round the
  change saves 0.53, 0.62, -0.24, 0.63, 0.53, 0.16, 0.56 ms: median 0.53 ms (12%), mean 0.40 ms, as the account
  expected (0.4 to 0.6). Against the model (1.04 to 1.20 ms in these runs) P is now about 3.4 to 4.0 times it.

## Findings carried from the fourth check of the plan, each settled or open

- The zygote's inherited restartable-sequences registration: settled, plan dec511ab (4.1, 6); code f655e9b0 (the
  zygote unregisters it and proves none is left), tested by tests/zygote.rs's migration test on both machines.
- 3.1's claim that the kernel places mappings low first (K2): settled, plan dec511ab (3.1, 13); K2 landed in the
  kernel and tests/kernel.rs compares addresses, memory size and turns (round 5 run 2); `count_at_every_stop` met
  (round 7 run 2).
- The SVE ABI's promise after a system call: settled, plan dec511ab (3.8 quotes sve.rst); the probe stays as a
  check (ebe9a8e9) and SVE builds are refused (1f5abd8a).
- The zygote's own end: settled, plan dec511ab (4.1, 7); code 924c92c2 forks a new zygote, its stubs ended as
  killed, counted "zygote ended" (tests/host.rs).
- I's checked address and no check of a pair's last byte: settled in the plan, dec511ab (3.2, 7); the code is W2's
  (form I is not built).
- The stack probe fault's counter: settled in the plan, dec511ab (3.2, "stack probe faulted"); the code is W2's.
- The span from the request word to the futex wait: settled, plan dec511ab (3.6); code 5f5feb5e (a stop landing
  there publishes again), tested by tests/stop.rs.
- Aliasing instruction caches: settled, plan dec511ab (3.4); the probe reads `CTR_EL0` (Graviton4: DIC and IDC set,
  L1Ip PIPT).

## Requests to other tracks

- Kernel track: `loader::program_of` (crates/kernel/implementation/linux/loader.rs) copies a regular file whole with
  `file.bytes(cpu)` at every exec; for busybox that is two `shared_read` calls of 146 us each, 5.5% of form P's
  time starting programs (and the same copy under every executor). Wanted: keep the file in `Program` (as the
  backend case keeps its handle), put only its first bytes in `Program::bytes` (Linux's `prepare_binprm` reads 256
  for the `#!` check and the header), and have `file_bytes` read just the range asked through `Regular::read`; the
  segments themselves are mapped from the file's blocks and need no copy.

## Where things stand

- W0 (the plan for both forms) is accepted (runs 1 to 6, four independent checks; its history is in git, commit
  544c6658 and before).
- W1 (form P), run 1 (2026-11-23):
  - The fourth check's eight findings are answered in the plan (commit dec511ab): the zygote's inherited
    restartable-sequences registration (4.1, 6), K2 named as W1's need with `differential` and
    `count_at_every_stop` waiting on it by name (3.1, 13), the SVE ABI's promise (3.8), the zygote's own end (4.1,
    7), I's checked address and no second check of pairs (3.2, 7), the probe fault's counter (3.2), the wait window
    from the request word on (3.6), aliasing instruction caches (3.4).
  - Built (crates/executor/implementation/host-process): the start-up probe (page size, the user-space top by
    fixed mappings, ARM64 `CTR_EL0`, SVE after a system call on its own short-lived thread) and the zygote: forked
    with every signal blocked, its code copied to a memfd and mapped at 2^45 over a reservation, assembly for
    ARM64 and x86-64 that unregisters the inherited rseq area and proves none is left (registering its own), clears
    the robust list and exit-clear word, closes every fd but its socket, unmaps everything else, and answers ping,
    fork, kill and reap over a SOCK_SEQPACKET socket. A forked child closes the zygote's socket, sets
    PR_SET_PDEATHSIG, checks its parent and waits on a futex word: the stub's start, with no body yet.
  - Tests (tests/zygote.rs, 8): the zygote moved 200 times between two CPUs answering after each (the fourth
    check's case); a stub moved, stopped and continued 200 times then killed and reaped as SIGKILL; zygote and stub
    map only the zygote's pages (x86's vsyscall page aside) and hold at most one fd (read from /proc by the test
    only); 64 stubs forked, killed, reaped and gone; stubs die with a killed zygote; 16 zygotes from an embedder
    with 4 busy allocating threads; the probe's values. Passed on Graviton4 (load 17 to 36) and the Xeon (load 0.1
    to 0.6), 5 runs each with /dev/kvm denied (`sudo setpriv --clear-groups`, checked by an open that fails), and
    3 more on x86 with it readable. The tests see the fault: with the unregistration and proof skipped on the
    remote copy, 5 of 8 fail on Graviton4 and 3 of 8 on the Xeon (the migration test on both); the stub test alone
    does not catch it on the Xeon, which is why the zygote's own proof matters (a stub inherits the zygote's state).
    The SVE probe reports Dropped on Graviton4 and Kept when its `svc` is replaced by a `nop`.
  - Probe values: Graviton4 user-space top 2^48, `CTR_EL0` 0xb444c004 (DIC and IDC set, L1Ip PIPT); Xeon 8488C
    top 0xfffffffffff000 (five-level paging).
- W1, run 2 (2026-11-23), built and tested on both machines:
  - The stub's start (zygote/*.rs, stub.rs, pages.rs): a fork request carries the stub's socket, its thread-pages
    memfd and its memory's memfd (`SCM_RIGHTS`; the zygote closes its copies after the fork). The stub puts the
    socket and memory at fixed numbers 8 and 9 (`dup3`), maps all 2^15 control pages read-only and register pages
    read-write (at 2^45 + 2^32) and every signal stack (`MAP_NORESERVE`), closes the pages' fd, names thread 0's
    exit-clear word (`set_tid_address`) and writes its id, then installs its filter. Its start is turn 1 of thread 0:
    the filter waits in that control page, and `fork_stub` returns only once the stub has reported it installed (a
    stub that ends first is reaped and its exit status named, `Unavailable::Zygote(FilterRefused)` and so on).
  - Turns: the embedder writes commands and the resume into the control page, stores the answer word (release) and
    wakes it; the stub applies `Map` (memory memfd, shared, fixed), `Protect` and `Unmap` in order, then reports
    `Done` with the count, or `CommandRefused` with the index and error, in its register page, stores the request
    word and wakes it. The runner waits on the request word with the futex's expected value, 100 ms at a time, and
    on each time-out polls the socket for a hang-up. `end` ends the last thread (`exit`) and waits for the hang-up.
  - The filter (filter.rs, classic BPF written here: no filter crate compares two arguments): the architecture,
    then `futex`, `exit`, `exit_group`; `mmap` only of fd 9, shared and fixed, inside [64 KiB, 2^44) with a 64-bit
    end computed with its carry, never executable, at the offset equal to the address (both words); `mprotect`
    inside the guest area, never executable; `munmap` inside the guest area; anything else kills the process
    (`SECCOMP_RET_KILL_PROCESS`). The rest of section 6's list (clone, recvmsg, close, sched_setaffinity,
    rt_sigprocmask, sigaltstack) joins as the routines that make those calls are built.
  - The window cache (windows.rs): 2 MiB windows of any stub's memfd mapped shared at first touch into one
    reservation, 16,384 at most, evicted by the clock algorithm, copies in and out through raw pointers (a stub may
    write the same bytes meanwhile), `forget` for a memfd about to close.
  - Tests: tests/stub.rs (5 and one measurement): 10,000 turns and thread 0's reported id; maps at the guest's own
    addresses at both edges of the guest area seen through /proc/<pid>/mem with the embedder's bytes, protect and
    unmap seen in /proc/<pid>/maps; a refused command reported after those before it; ten host calls outside the
    filter each kill the stub by SIGSYS (control page made writable, register page unmapped, signal stacks
    replaced, executable protect, executable map, offset not the address, offset equal only in its low word, a map
    across 2^44, a map below 64 KiB, a wrapping length); a stub stopped then killed from outside is seen ended
    within 150 ms. tests/windows.rs (3): writes across window edges survive eviction and are the stub's bytes at the
    guest address; three memories in a cache of two never mix; the plan's case of 64 memories of 2^43 bytes, 512
    seeded touches each (`WCHP_SEED=1`, knobs `WCHP_MEMORIES`, `WCHP_TOUCHES`, 32,768 touches run): 16,384
    windows mapped, 16,383 evicted, at most 16,490 host mappings against 105 before the cache, every word read back
    through the cache and every 61st through /proc. tests/zygote.rs (8) as run 1, the stub now started for real.
  - The process-wide host's private stand-in (host.rs, `HostProcessHost`, in place of the owner's item 8.5): the
    probe and one zygote; a `fork_stub` that finds the zygote gone reaps it, counts and warns "zygote ended", and
    forks a new one (P unavailable for good if that fails); `kill` and `reap` of a stub whose zygote ended answer
    killed by SIGKILL (it died with its zygote). tests/host.rs (1): four stubs end with their killed zygote, the
    next fork comes from a new zygote and answers, the count is 1, a live zygote is not replaced.
  - Verdicts: Graviton4, all 17 tests passed 3 times with /dev/kvm denied (load 9.9 to 22.7), clippy with warnings
    denied clean. Xeon: the same 17 passed 3 times with /dev/kvm denied at 13:47 UTC (load 0.2 to 0.4), clippy clean
    (the machine did not answer SSH from 13:39 to 13:46 UTC).
- W1, run 3 (2026-11-23), built and tested on both machines:
  - Threads (stub.rs, zygote/*.rs): a `NewThread` command of a turn makes a host thread (`clone` with 4.1's flags
    plus `CLONE_PARENT_SETTID`, so the new index's exit-clear word holds the id before the call returns), on its
    own signal stack as its stack, with x16 / r9 its register page and x15 / r8 the value its answer word holds,
    which the embedder put in the command so no turn is missed; the maker writes the id to the new register page.
    The new thread names its signal stack (`sigaltstack`), unblocks the stop and waits. The embedder holds indices
    (`Stub::new_thread`, `release_thread`, `end_thread`): free means not held and its exit-clear word zero;
    `thread_gone` waits on that word. The filter allows `clone` with exactly those flags. tests/threads.rs (4): 8
    threads take 1,000 turns each, each waits on its own signal stack (its sp from /proc's syscall line) and
    reports the id /proc lists; an index let go while its thread runs is not reused, one whose thread is gone is;
    an index past the last is refused (`BadTurn`); all 32,768 indices made (32,767 threads in rounds of 64) and the
    next `new_thread` is `None`.
  - The stop (3.6): the kernel's first real-time signal (`STOP_SIGNAL`, 32), sent by `tgkill` (`StopTarget`, from
    any thread). The stub installs its handler before its filter (`SA_SIGINFO | SA_ONSTACK | SA_NODEFER |
    SA_RESTORER`, mask the stop; the restorer is our own `rt_sigreturn`, never reached, refused by the filter) and
    thread 0's signal stack. A turn with commands blocks the stop before the first and unblocks it after the last
    (two `rt_sigprocmask`); a turn without pays nothing. The handler never returns: it resets sp to the thread's
    signal stack top, takes the interrupted pc from the frame only to choose a window, and jumps (through an
    unblock whose next instruction is in the same window) to: publish again (from the unblock before the exit's
    stores to the end of the wake), the wait (from the wait to the instruction that takes the turn, and the jump
    back to it), the thread's end (its `exit`), or else, after the commands, a `Stopped` exit with the count
    applied. `Resume::Spin` stands in for guest code. The stub counts where each stop landed in its register page
    (untrusted, for tests). tests/stop.rs (3): 1,000 spinning turns on each of two threads, with and without
    commands, each ends `Stopped` with its count; 1,000 stops to a waiting thread then 100 turns each answered once;
    20,000 seeded turns (`WCHP_SEED=1`, knob `WCHP_ROUNDS`) of four kinds, one making a thread, under a second
    thread sending stops with random pauses: every turn ended once with its full count, each thread made exactly
    once and ended, and every stop sent landed once (Graviton4: 20,161 sent, 7,809 ended a turn, 709 in a publish,
    11,643 in the wait; Xeon: 23,268 = 8,552 + 1,114 + 13,602); a thread told to end under stops ends (no stop
    landed in its `exit` in either run).
  - Faults: the stub installs one handler for SIGSEGV and SIGBUS (flags as the stop's) and unblocks both before its
    filter (a fault while blocked kills outright); new threads inherit that. `Resume::Load` and `Resume::Store`
    (one access each, address and value in the control page) stand in for guest code: a fault at either access is
    the turn's `Fault` exit with the host's address, signal and code; a fault anywhere else in the stub is ours and
    ends it (`ZygoteEnd::StubFault`, "stub fault"). tests/touch.rs (3): stores and loads at both edges of the guest
    area, seen in the memfd; 100 rounds of an unmapped store and load (`SEGV_MAPERR`) and a store to a read-only
    page (`SEGV_ACCERR`), the stub answering after; the escape attempts: the embedder's heap, stack and code
    addresses are unmapped in the stub (load and store fault, the embedder's value unchanged), its control page and
    the zygote's code are readable and refuse writes, and its register and signal-stack pages are writable (the
    plan's section 12 item). Not tested: a fault of the stub's own (nothing provokes one without corrupting it).
  - Verdicts (14:13 to 14:15 UTC): all 27 tests of the 8 test binaries passed 3 times with /dev/kvm denied on
    Graviton4 (81 of 81, each binary's run first proving the device cannot be opened; load 14 to 16) and on the Xeon
    (81 of 81; load 18 to 24, other tracks' jobs), clippy with warnings denied clean on both. The ended-thread checks first failed 2 of 3 runs on
    each machine: the host lists a thread in /proc until it reaps it, a moment after clearing its exit-clear word;
    the tests now wait up to 1 s for the list (the product never reads /proc).
- W1, run 4 (2026-11-23), built and tested on both machines:
  - The `Cpu` calls (executor.rs, `HostProcessExecutor`, made from an `Arc<HostProcessHost>`, which now holds the one
    window cache): every method of `wcontainers_abi::Cpu`. A memory is a stub whose whole guest area is its memfd
    mapped with no access; `set_permissions` records the kernel's permissions by range (rangemap) and queues an
    `mprotect` for the stub's next turn; `read`, `write` and `zero` (a punched hole) go through the window cache;
    registers live in the register page (offset 128, as the floor will save them); `code_changed` has nothing to
    drop yet. A memory keeps one spare host thread (its stub's first, or the last dropped), so a memory with no
    guest thread still has a stub thread to apply commands; a further `thread_new` is a turn of one of its threads
    carrying the `clone` command.
  - Copy-on-write (3.5): one owner per private page. A fork lends the source's data extents (`SEEK_DATA` on its
    memfd) and its own borrowings' owners to the child, takes write from every writable range of the source's view
    by a round trip before it returns, and starts the child's view with no access. A fault the kernel's
    permissions allow is the executor's: the page is copied from its owner (any access) or given to its first
    borrower (a write; the rest then borrow from that one), and the widest range of one state around it gets the
    kernel's permissions at the next resume. `Cpu::read` of a borrowed page reads the owner's. An owner dropped
    while lending keeps its memfd, not its process, until nothing is borrowed from it (departure below).
  - Shared blocks: one memfd per block; `shared_map` gives up the range's private pages and queues `MapShared`,
    whose memfd the embedder sends over the stub's socket (`SCM_RIGHTS`) just before the turn whose page holds the
    command; the stub receives it with `MSG_DONTWAIT` (a missing one is refused with EAGAIN, never waited for),
    maps it (the lowest descriptor, 0) and closes it. The filter allows exactly that: `recvmsg` on fd 8 with
    `MSG_DONTWAIT`, `close` of fd 0, `mmap` of fd 0 at any offset under the same bounds as the memory's. A fork child
    maps its parent's blocks; `shared_unmap` copies the block's bytes into the memory's own; `zero` ends the sharing.
  - Slices (3.6): a turn's wait carries its slice; when it passes the runner sends the stop, again every 100 ms, and
    after 10 s (`STOP_IGNORED`) the zygote kills the stub ("thread ignored its stop", counted, warned).
  - Tests: tests/memory.rs (10): the kernel's reads, writes, zeros and permissions against loads and stores, the
    edges of the guest area refused; registers per thread, a dropped thread's index made again; a fork's exact copy
    counts (one copy per event, none where the page is already the toucher's); forking twice before a copy and a
    child forking before copying (four processes, every word checked by load and by `read`); an owner ended while
    lending, its borrowers and a grandchild still reading its pages, nothing copied for it; forged fault reports
    (every access at the page and outside it, three times) doing only what real accesses would, then C's copy equal
    to the fork's instant; a queue of 190 commands applied in two rounds; a block shared by two processes and a fork
    child, then unmapped and zeroed; one block mapped 10,000 times (`WCHP_MAPS`) leaving 2 descriptors in the stub;
    a seeded model (`WCHP_SEED`, `WCHP_STEPS`, default 30,000 steps) of up to six processes with forks, ends, the
    kernel's writes and zeros, loads, stores and permission changes, every process's copy of the touched page
    checked after each step. Seeds 1 to 5 at 100,000 steps each passed on Graviton4 (about 3,800 forks, 19,500
    copies, 1,200 retired owners per seed). The model catches a fork without its write protection within 13 to 77
    steps (seeds 1 to 3), the fork tests too; at 3,000 steps checking only the touched word it did not, hence the
    current default and checks. tests/stub.rs (+1): a block not sent is refused with EAGAIN; one sent is mapped and
    read; mapped executable, over the signal stacks or below the guest area, the stub is killed by SIGSYS.
    tests/slice.rs (3): slices of 1, 5 and 20 ms end the spin as stopped within 100 ms of the slice and a turn back
    in time sends none; a stub held stopped by the host for 350 ms is sent its stop at least 4 times, the turn ends
    once, and every stop sent lands (one in the turn); a stub held stopped is killed after 10 s, 95 to 102 stops
    sent.
  - Placement (4.3): `HostProcessHost` keeps the table of the process's allowed CPUs (`sched_getaffinity` at its
    start, read again when a pin is refused), each with its runners held and its last contended time. A runner (the
    thread calling the executor) takes the CPU held by the fewest not found contended in the last second and pins
    itself; a turn of a thread last placed elsewhere carries a `Place` command (the stub's `sched_setaffinity` of
    itself, the filter allowing only pid 0 and the 1,024-bit set's size). Every 10 ms of turn time the runner reads
    its own thread's CPU clock and each stub's process CPU clock (nix's `clock_getcpuclockid`); a share under 3/4
    marks its CPU contended and moves it to the CPU the table gives now. Counted: moves, "placement without samples"
    (clocks refused, warned).
  - A slice's stop waits until the thread has taken its turn (the stub writes the turn it took, `TAKEN`, to its
    register page; the runner looks every 1 ms, at most 100 ms): the placement test first showed a 1 ms slice on a
    contended CPU running 104 ms, the stop having landed in the wait before the turn began; after the change the
    same test moved after 2 turns in 6.9 to 7.0 ms (3 runs, Graviton4, load 17 to 18).
  - tests/placement.rs (2): two containers' runners in one embedder take different CPUs and each guest thread is
    pinned to its runner's; with a thread spinning on the runner's CPU the runner moves (after 2 turns) and the guest
    thread follows. The executor tests now share one host per test process, as an embedder does.
  - tests/embedder.rs (2): a stub killed from outside while its thread spins ends the turn as `StubEnded` within
    150 ms of the kill, and once dropped it is reaped (no zombie); an embedder (this test binary run again as one,
    with a host and three guest processes) killed by SIGKILL leaves its zygote and every stub gone (or a zombie of
    init's) within 5 s. tests/threads.rs: the stack-pointer check read /proc's syscall line while a thread had just
    published and was not yet in its wait ("running"), which failed once on the Xeon; it now reads until the thread
    is in a call.
  - Final verdicts (15:13 to 15:15 UTC, scratch w1/run4/embedder2-arm.log and embedder2-x86.log): all 45 tests of the
    12 test binaries passed 3 times with /dev/kvm denied on Graviton4 (135 of 135, load 7 to 9) and on the Xeon (135
    of 135, load 13 to 26 from other tracks), clippy with warnings denied clean on both.
  - Verdicts with placement (14:57 to 14:59 UTC, scratch w1/run4/placement-arm.log and placement-x86.log): all 43
    tests of the 11 test binaries passed 3 times with /dev/kvm denied on Graviton4 (129 of 129, load 9 to 10) and on
    the Xeon (129 of 129; load 0.2 at the start, 712 at the end from another track's 32-job run), clippy with
    warnings denied clean on both.
  - Verdicts before placement (14:48 to 14:50 UTC, scratch w1/run4/slice-arm.log and slice-x86.log): all 41 tests of the 10 test
    binaries passed 3 times with /dev/kvm denied on Graviton4 (123 of 123, load 8 to 10) and on the Xeon (123 of
    123, load 1.8 to 3.7), clippy with warnings denied clean on both.
- W1, run 5 (2026-11-23), built and tested on both machines:
  - Host mappings counted up front (4.2, view.rs): each memory keeps its stub's guest-area mappings as its queue
    leaves them (neighbours of equal permission and backing merged, as the host merges them) plus the stub's five
    own (`OWN_MAPPINGS`); a change of c commands is refused as `OutOfMemory` unless the count plus 2c stays within
    the host's `vm.max_map_count` (the probe reads it once; Linux's default, warned, where unreadable). Counted
    "host mappings exhausted". Tests (tests/memory.rs): the count equals /proc's after every load and store of the
    seeded model, after 10,000 block maps and in the shared-block test; alternating permissions page by page up to
    the limit (1,048,576 on both machines) is refused once, every change before it applied by the host and none
    refused there, the count then within 2 of the limit. A view that ignores protections fails 4 of them.
  - Inside Docker's default seccomp profile (busybox image, the Nix store mounted read-only, an unprivileged user,
    CapEff 0, Seccomp 2, no /dev/kvm, no network): all test binaries pass on both machines unchanged.
  - Under gVisor (runsc release 20261109.0, `runsc do`, systrap platform, no KVM), found and handled:
    - `rseq` answers ENOSYS, which the zygote's proof took for a registration left over; the zygote now takes
      ENOSYS as no restartable sequences at all (both architectures' assembly).
    - `SEEK_DATA` on a memfd answers EINVAL, so every fork answered `OutOfMemory`; a fork now lends every private
      page of the guest area there (slow path, counted "memory extents unknown", warned). The first version also
      lent the shared ranges, which the shared-block test caught under gVisor.
    - `fallocate`'s hole punch answers EOPNOTSUPP, so `zero` and `shared_map` answered `OutOfMemory`; zeros are now
      written instead (slow path, counted "hole punch refused", warned). With both, all 11 memory tests pass under
      gVisor on Graviton4 (the mapping-limit test returns at once there: gVisor's limit is 2^31 - 1, past
      `WCHP_MAPPING_LIMIT_MAX`, default 2^22).
    - On x86-64 gVisor maps its own `[usertrap]` page at 0x63000 in every process (syscall patching) and refuses
      to unmap it, so the zygote reports `UnmapRefused` and P is unavailable, as it should be; with
      `--systrap-disable-syscall-patching` the suites run.
    - Not ours to fix, recorded in plan 6: the systrap platform kills a process at about its 4,097th thread
      (`every_index_held_refuses_the_next_thread` fails there); `sched_setaffinity` is accepted but does not pin
      (`a_runner_whose_cpu_another_program_uses_moves_and_its_thread_follows` fails there); /proc's per-thread
      `syscall` file is missing (the test-only read of `new_threads_take_turns_on_their_own_pages_and_signal_stacks`
      fails there); glibc registers no restartable sequences (`the_forking_thread_has_a_registration_to_drop`, a
      test precondition, fails there). The 20,000-turn stop test passes but takes 369 s against about 2 s on the
      host (at load 46 it passed 900 s and was stopped); beside it, `stops_sent_to_a_waiting_thread_are_stale`
      failed once on the Xeon (a stop refused while the other test's stops wait in the queue) and passes alone.
  - The copy unit (3.5) is the constant `COPY_UNIT`, still one page; measured from 1 to 512 pages (plan 11): in
    order 20 µs a store at one page against 7.8 at 8 pages, scattered 25 against 36. Kept at one page until the
    same measurement runs on translated code; 8 pages is the candidate.
  - Final verdicts (16:55 to 17:00 UTC, scratch w1/run5/final2-arm.log and final2-x86.log, after the last code
    commit): all 46 tests of the 12 test binaries 3 times with /dev/kvm denied: Xeon 138 of 138 (load 1.3 to 7.6);
    Graviton4 137 of 138 (load 14 to 20), clippy with warnings denied clean on both. The one failure is a known
    intermittent one, left failing: `stops_landing_anywhere_never_lose_repeat_or_split_a_turn` ends with "thread 1
    did not end under stops" (thread 1 told to end while another thread sends it stops back to back, given 1 s);
    alone it failed 1 of 30 runs on Graviton4 (load 13). Its code is run 3's (nothing this run touches the stub's
    stop or end paths), so it is older than this run and earlier rounds missed it. Not yet known whether the
    thread livelocks under the flood or its end is lost; the product sends a stop at most every 100 ms.
  - Docker's default profile (scratch w1/run5/sbfinal-*.out, before the hole-punch fix, which only gVisor
    reaches): all 46 tests pass on both machines.
- W1, run 6 (2026-11-23):
  - Found why a thread told to end under back-to-back stops sometimes did not end within 1 s: the stop was a
    real-time signal, so every stop sent was queued, and the thread handled them one at a time, unblocking the stop
    before going back to its wait, so it went no further while any was pending. Reproduced on Graviton4 (load 4 to
    10, the test pinned to two CPUs, 1 of 60 runs; scratch w1/run6/repro.sh with stop-diag.rs): 768,008 stops
    queued (`SigQ` of the 1,430,995 allowed), the thread ended 443 ms after the sender stopped. Unpinned at load 10
    it did not fail in 90 runs.
  - The stop is now `SIGUSR1`, a standard signal: stops sent while one is pending are that one, so a burst holds a
    thread only while it lasts, never fills the user's `RLIMIT_SIGPENDING` (shared with the embedder's whole user),
    and `tgkill` never answers EAGAIN (plan 3.6, 3.7). Tests changed with it: the end under stops floods for 1 s or
    until the thread is gone, then requires it gone within 100 ms of the last stop (under contention one run in 60
    held the thread for the whole flood, 61,827 stops, and it ended after); the main flood now requires every stop
    accepted; the slice test's held stub now requires the stops sent while it was held to land as one (fewer landed
    than sent) instead of each landing. Under the same contention 200 runs passed with each signal (the real-time
    one held a thread through a flood of 159,178 and still drained within 100 ms, so the new test does not by itself
    catch the queueing; the slice test's assertion does).
  - Verdicts after the change (17:07 to 17:09 UTC, scratch w1/run6/coalesce-arm.log and coalesce-x86.log): all 46
    tests of the 12 test binaries 3 times with /dev/kvm denied, 138 of 138 on Graviton4 (load 3.4 to 4.6) and on
    the Xeon (load 27 to 30 from other tracks), clippy with warnings denied clean on both.
  - Sandboxes after the change (17:09 to 17:12 UTC, scratch w1/run6/sb-arm.out and sb-x86.out, run 5's
    sandbox.sh): inside Docker's default profile every test binary passes on both machines; under gVisor the same
    four tests fail as in run 5, for the reasons recorded there and in plan 6, and nothing else. Under gVisor the
    stop binary now takes 71 s on Graviton4 against run 5's 369 s.
  - Form I's bounds-check bypass on ARM64, measured as a host program of I's own sequences (scratch
    w1/run6/specarm.c, plan 11 and 3.2): twelve access shapes, each behind I's own check and behind a slow check,
    cut and uncut. Behind the slow check the uncut control read the outside byte in about 98% of tries for the nine
    shapes that load speculatively, the cut access never (0 or 1 in about 1,000, as untouched lines) and its own
    in-window byte instead; behind I's own check nothing leaked even uncut. Exclusives, `LDADD` and `SWP` do not
    load speculatively at all here, so the test cannot show their cut. The harness had to defeat the branch
    predictor learning the original proof's fixed pattern, and the prefetchers (one line timed per round, a new
    secret each round).
- W1, run 7 (2026-11-23), the floor on ARM64 (the x86-64 machine did not answer SSH from 17:50 to about 17:55 UTC,
  then did):
  - Nothing the translator's form or the stop crate needed had landed (no commit to the translator, ARM64, the VM
    inside or its status since run 6), so this run built the stub's side of the floor and the code area, which
    need neither, with pieces the tests write in the native output's own encodings (wcontainers-arm64's encoder, a
    dev-dependency) standing in for translated code (plan 3.3 and 3.4, "as built").
  - The code area (code.rs, layout.rs): one memfd per stub, [2^45 + 128 KiB, 2^45 + 128 MiB) in the stub, so every
    piece reaches the floor's entries with one `bl`; the embedder writes through its own mapping and then cleans and
    invalidates by `CTR_EL0`'s lines where IDC and DIC ask (the probe's `code_lines`), each ended by `dsb ish`.
    `Command::MapCode` maps it in the stub `PROT_EXEC` alone, received like a shared block; the filter allows that
    one call exactly (address, length, protection, flags, fd 0, offset 0). The view counts its one mapping.
  - The enter and the floor (zygote/aarch64.rs): `Resume::Enter` loads every register of the thread's block (now the
    hardware's state: general, sp, NZCV, v0 to v31, FPSR, FPCR, `TPIDR_EL0`) but x23 (the code's address) and x28 (the
    register page, the native output's withheld registers), refusing an address outside the code area (`BadTurn`).
    64 floor entries at the start of the zygote's code (`FLOOR_ENTRIES`) save every register and publish `Called`
    with the entry's number. A stop or a fault (SIGSEGV, SIGBUS and now SIGILL and SIGTRAP, all on the fault handler)
    in the code area saves every register from the signal frame (`fpsimd_context` checked by tag and size; a page
    in x28 that is not a register page ends the stub as "stub fault") and publishes `Interrupted` with the host pc,
    or `Fault`. A stop in the enter's loads ends the turn `Stopped` with the block untouched; a stop in an entry or
    the save finishes it from the frame (counted in the register page, `STOPS_IN_CALL`).
  - tests/floor.rs (6 and one measurement; seeded, `WCHP_SEED`, `WCHP_TURNS`, `WCHP_ROUNDS`, `WCHP_STOP_PAUSE`):
    pieces that change every register (32 vector loads from the thread's own data page, every general register but
    x1, x23 and x28 incremented, sp moved, NZCV, FPSR, FPCR and `TPIDR_EL0` set), then call, spin or fault; the
    saved block must equal a model of the pieces. 1,000 calls of each kind and the last entry from the code area's
    far end; 100 rounds of a stop and of four faults (unmapped load and store, `udf` as SIGILL `ILL_ILLOPC`, `brk`
    as SIGTRAP `TRAP_BRKPT`) with every register; the code area neither readable nor writable from the code
    (`SEGV_ACCERR`); an enter outside the code area refused. Register isolation (brief-native.md's last section):
    thread 0 holds its values in the hardware when it makes 7 threads, each of which then reads zeros at its first
    instruction; then 4,000 seeded rounds over the 8 threads, each going on from the registers it last saved, each
    reading back exactly its own. A stop flood (20,000 rounds of a piece 64 times longer, re-entered at every stop
    with the registers saved there, as a resumed thread would be): 10,968,293 stops sent, 37,183 landed in the code
    at every kind of instruction and resumed exactly, 37,615 in the enter, 2,077 in an entry or the save, and every
    call's block equal to the model. tests/stub.rs: six forged code-area maps (readable, writable, partial, another
    offset, execute-only in the guest area or past the code area) each kill the stub by SIGSYS.
  - The tests see the faults (scratch w1/run7/mutate.sh, mutate2.sh, outputs mutate2-arm.out, mutate3-arm.out): each
    of nine breaks of the remote copy fails at least one test: v0 and v1 not loaded; a stop in the save not
    finished from the frame; the page taken from x28 after the save; the entry's number taken from the block
    during the stores; a stop in the code taken for one in the stub; the frame's last vector, or its FPSR and FPCR,
    not saved; v30 and v31 not saved at a call; the flags not loaded. Before the pieces changed every register
    three of these went unseen (the block already held the values), hence the scramble pieces.
  - Measured (Graviton4, CPU 41, load 1.3 to 1.5, scratch w1/run7/callrt-arm.out, 3 runs of 21 alternating batches of
    10,000): a call from code, with every register loaded and saved, 2,777 to 2,782 ns median against a turn without
    code 2,700 to 2,742 ns: 35 to 80 ns more per kernel entry.
  - Verdicts (17:53 UTC, scratch w1/run7/floor-arm.log): all 13 test binaries 3 times with /dev/kvm denied on
    Graviton4, 156 of 156 passed (load 2.9 to 6.6), clippy with warnings denied clean; on the Xeon (17:57 UTC,
    scratch w1/run7/floor-x86.log) 138 of 138 (the floor test builds no tests there), load 17 to 23, clippy clean.
- W1, run 8 (2026-11-23, 18:00 to 18:40 UTC):
  - Still nothing from the translator's form or the stop crate (no commit to the translator, ARM64, the VM inside
    or their status files since run 6; docs/status-translator.md last changed 2026-11-20, docs/status-native.md
    2026-11-21: those tracks look stopped, see "Asked of other tracks").
  - Built the bar's "a stop landing at every instruction" item for the stub's own code, on both machines
    (tests/step.rs, 4 tests on ARM64, 3 on x86-64): the test traces one stub thread (ptrace, the test's tool alone;
    nix's `ptrace`), steps it from its wait one instruction at a time to learn the turn's path, then runs the turn
    again once per instruction of it, letting the thread go with the stop delivered exactly there (the step's
    SIGTRAP replaced by the stop at detach), the pc at every step checked against the first run. Turns: commands
    with a `clone` among them that report back (all applied or none, the thread made exactly once, it answers, the
    page's permissions as the count says); commands then a spin (a stop that lands in the wait changes nothing, and
    the runner's next stop ends it); a thread's end (it ends, or reports `Stopped` before taking its end and ends
    when told again); on ARM64 an enter into a piece calling a floor entry (the block untouched, the stop's pc with
    the loaded registers, or the call's save). Each stop sent raises exactly one of the stub's landing counts,
    read once the thread is back in its wait.
  - Paths and where their stops landed (the same on every run): ARM64 commands 134 instructions (106 ended the
    turn, 8 in the publish, 20 in the wait), spin 124 (111, 13 in the wait), end 24 (13 before the end was taken, 8
    in the wait, 3 in the end), enter and call 164 (72 turn, 69 in the entry or the save, 8 publish, 15 wait);
    x86-64 commands 128 (101, 8, 19), spin 122 (109, 13), end 25 (14, 8, 3).
  - The test sees the faults (scratch w1/run8/mutate-step.sh, mutate-step-arm.out): with the stop handler's wait,
    end or publish window each sent to the turn's path instead, at least one test fails.
  - Two things about the tool, not the product, found on the way: a thread stepped into a host call that blocks
    and let go while in it takes a step trap when the call returns (ARM64, Linux 7.0; setting its registers back or
    resuming it unstepped first does not clear it), so the test's path stops before the futex wait instead of
    stepping into it (it decodes the instruction at the pc: wcontainers-arm64's decoder, iced-x86 on x86-64); and a
    stop that lands after the request word is stored is counted after the turn's end can be read, so counts are
    read once the thread waits again.
  - Verdicts (18:33 to 18:36 UTC, scratch w1/run8/final-arm.log and final-x86.log, after the last code commit): all
    14 test binaries 3 times with /dev/kvm denied, each run first proving the device cannot be opened: Graviton4
    168 of 168 (load 5.0 to 5.8), Xeon 147 of 147 (load 0.5 at the start, 38 at the end from other tracks), clippy
    with warnings denied clean on both. Not rerun inside Docker's profile or gVisor this run.
  - No new unsafe code (the step test has none).
- W1, run 9 (2026-11-23, 18:25 UTC on):
  - Still nothing from the translator's form or the stop crate. Since run 8 nobody has touched the translator, the
    ARM64 crate or the VM inside. The native output's trait now takes a way of running (`Native::Way`, commit
    0b3267fc, before run 6), but ARM64's is `()`, with one way only. The `HostProcess` way of plan 3.2 is still
    to be written there.
  - Built the handler's own instructions (tests/step.rs, 4 more tests on ARM64, 3 on x86-64). The test sends a
    stop at every instruction of the turn's path. It steps the thread into that stop's handler and on to the
    instruction after the unblock the handler leaves by, or to the `exit` it ends by. Then, for each different way
    through the handler, it runs the turn again once per handler instruction, sending a second stop there the way
    the runner sends it (`Stub::stop`, the thread stopped by the tracer). With `WCHP_TAIL_EVERY=1` it does this
    for every first stop's handler, not just one per way. The second stop must land just after the unblock, in the
    window the handler chose:
    - a first stop in the wait leaves a second in the wait;
    - a first stop in a turn, a publish or a call leaves a second in the publish;
    - a first stop in the thread's end goes on to its `exit` with the second still pending, and the thread ends.
    Every turn still ends as the single-stop test requires.
  - A test-tool fact: a signal given at the tracer's let-go while the thread sits at its handler's first
    instruction is dropped by the host. That stop reports the step into the handler, and its restart signal is
    ignored. So the second stop is sent by `tgkill`, as the runner sends it, not given at detach.
  - A second test-tool fact: a first stop given while the thread blocks it is taken at the unblock as a stop of
    the tracer's own, so the tool gives it again there.
  - A third: a failing test left its thread traced, and the zygote's reap then waited forever for the stub. The
    test now waits for its traced threads after the kill.
  - Graviton4 (19:00 UTC, load 4.7 to 6.3, scratch w1/run9/step5-arm.out): all 8 step tests passed in 25 s, and
    clippy with warnings denied was clean. Second stops sent:
    - turn of commands: 4 ways through the handler, 224 second stops;
    - spin: 3 ways, 175;
    - end: 3 ways, 172;
    - enter and call: 10 ways, 1,515.
  - The tests see the fault (scratch w1/run9/mutate-tail.sh, mutate-tail-arm.out). With the stop unblocked at the
    handler's start, before it chooses a window, all 4 new tests fail (a second stop counted in the turn).
  - Final verdicts (about 19:02 to 19:05 UTC, scratch w1/run9/final-arm.log and final-x86.log, after the last code
    commit). All 14 test binaries ran 3 times with /dev/kvm denied, each run first proving the device cannot be
    opened. Clippy with warnings denied was clean on both machines.
    - Graviton4: 180 of 180 passed (load about 5).
    - Xeon: 156 of 156 passed (load 0.9 to 9.7).
    - Not rerun inside the sandboxes after the change: the run above used run 8's code.
  - No new unsafe code.
  - Sandboxes rerun on run 8's committed code (18:26 to 18:31 UTC, scratch w1/run9/sb-arm.out and sb-x86.out, run
    6's sandbox.sh; Graviton4 load 4.5 to 6.0). These are the first sandbox runs of the floor (run 7) and step
    (run 8) tests.
    - Docker's default profile: every test binary passes on both machines, the floor's 6 and the step test's
      included.
    - gVisor: run 5's four known failures again.
    - gVisor: all of tests/step.rs fails on both machines. That is the test tool's own read of /proc's per-thread
      `syscall` file, which gVisor lacks (as in run 5), not the product.
    - gVisor, new: 4 of the 6 floor tests fail on Graviton4. These are every test with a stop or a fault in the code
      area: a fault there ends the stub as `StubFault` (exit 12), and a spin there "ignored 10,000 stops", ending
      `Stopped` each time and never `Interrupted`.
    - gVisor: calls from code into the floor's entries pass (1,000 of each kind), so the code area is mapped and
      runs.
    - Both symptoms fit the signal frame's pc not pointing into the code area under gVisor's systrap. That is not
      yet checked. P under gVisor on ARM64 therefore cannot run translated code until this is settled (Next).
- W1, run 10 (2026-11-23, 19:06 UTC on):
  - Still nothing from the translator's form or the stop crate (no commit to the translator, ARM64 or the VM inside
    since run 9).
  - Found why stops and faults in the code area failed under gVisor on ARM64 (scratch w1/run10/gframe.c and its
    outputs gframe*-arm.out): the frame's pc is right, but gVisor's frame has no FP/SIMD record (`fpsimd_context`;
    the first record's magic reads 0) and the handler starts with v0 to v31, FPSR and FPCR zeroed; gVisor keeps
    them itself until the handler returns. The save took the missing record for a broken frame and ended the stub.
  - Fixed first for exits from code (plan 3.6): where the record is missing, the handler saves the general registers from the frame, then
    returns through it (`rt_sigreturn`) into its own next routine (pc, sp, pstate, x9, x10 and x27 rewritten, the
    stop added to the frame's mask), which stores the FP/SIMD registers the host restored and goes on to the exit.
    The stub counts each in its register page (`VECTORS_BY_RETURN`); the embedder's `Stub` adds it up when it reads
    an exit from code (`Stub::vectors_by_return`) and warns once per guest process ("signal frame without FP/SIMD
    registers"). The ARM64 stub filter now allows `rt_sigreturn` (plan 6); it gives an escaped stub nothing new,
    since `rt_sigprocmask` and `sigaltstack` are already allowed as they come.
  - The test: `a_stop_or_a_fault_in_translated_code_saves_every_register` now also requires that every exit from
    code took the route or none did. Graviton4 natively: 0 of 700. Under gVisor: 702 of 702, every register block
    equal to the model (it failed before). With the route forced on every exit on Linux (the remote copy's `cbz w7`
    made a `b`), every floor test but that count passes (701 of 700 before the count was fixed to count re-entries),
    so the route also holds where the host restores from the frame.
  - Found next, and fixed: gVisor also keeps that state for good when a handler never returns. A 12,000-round
    stop flood under gVisor grew its sentry to 377 GB of memory until the host's out-of-memory killer ended it
    (20:20 UTC on Graviton4, the machine's dmesg; runsc exit 137; nothing else was killed). So now every handler
    that goes on in the stub leaves through one routine (`.Lwchp_leave`, target in x16): a jump where the frame held
    the record, else a return through the frame with every general register and sp as the handler holds them,
    pstate zero and the stop added to the mask, into the same target. Stops and faults in translated code return
    into the routine that stores the FP/SIMD registers. Handlers that end the thread or the stub need none. The
    frame's kind is read once at each handler's entry (`wchp_frame_kind`, x15).
  - Under gVisor after the change (scratch w1/run10/gleave-arm.out, gleave2-arm.out): the 2,000-round stop flood
    takes 20 s against 269 s before, and the sentry's memory stays at 47 MB (largest of its samples every 5 s). It
    still fails its own coverage check (3,815,032 stops sent, 1,880,016 landed in the wait, 17 in the publish, 1 in
    the code, none in the enter or the save), left failing there as above.
  - Forced on every handler on Linux (the remote copy's `cbnz w3` made a `nop`; scratch w1/run10/leave1-arm.out):
    the floor (stop flood at 2,000 rounds), step, stop, slice, touch, threads and memory tests all pass, 701 of 701
    exits from code by the return. Natively every test binary passes, clippy clean.
  - New gVisor limit, recorded in plan 6: an execute-only mapping can be read there (scratch w1/run10/xonly.c:
    the read succeeds, the write faults `SEGV_ACCERR`). So under gVisor translated code can read its code area,
    and `translated_code_can_neither_read_nor_write_the_code_area` fails there on its load (left failing).
  - New decision for the owner (plan 12, item 14): the frame of the return route lies on a signal stack the guest's
    other threads can write, so under gVisor on ARM64 a racing sibling could rewrite its pc and run at a pc of its
    choosing in the stub, bounded by the filter and by gVisor. Linux never takes the route.
  - The stop flood under gVisor (scratch w1/run10/gflood2000-arm.out, 2,000 rounds, 269 s): every call's block
    equal to the model, but it fails its own coverage check, "no stop landed in the save, the enter or the code":
    of 42,391,328 stops sent, 41,340,295 landed in the wait, 1,412 in the publish, 6 in the code and none in the
    enter or the save. Under gVisor a stop takes far longer to arrive than the piece runs, so stops land where the
    thread waits. Left failing there.
  - Sandboxes on Graviton4 after the code commit (19:50 to 20:10 UTC, scratch w1/run10/sb-arm.out, load 12 to 18):
    inside Docker's default profile every test binary passes. Under gVisor: run 5's four known failures and the
    step test's /proc read, as in run 9; the floor binary as a whole passed its 900 s limit (the full 20,000-round
    stop flood), so its verdicts under gVisor are the separate runs above (calls, stops, faults and register
    isolation pass; the code-area read and the flood's coverage fail).
  - Sandboxes after the leave routine (20:38 to 20:48 UTC, scratch w1/run10/sbfinal-arm.out and sbfinal-x86.out;
    Graviton4 load 12 to 15, Xeon 7.3 to 7.8): inside Docker's default profile every test binary passes on both
    machines. Under gVisor on both: run 5's four known failures and the step test's /proc read, nothing else; on
    Graviton4 the floor binary now finishes there (186 s): its calls, stops, faults and register isolation pass
    (4), and the two failures named above (the code-area read, the flood's coverage) are its only ones.
  - Final verdicts after the last code commit (20:34 to 20:37 UTC, scratch w1/run10/final2-arm.log and
    final2-x86.log), each test binary 3 times with /dev/kvm denied, clippy with warnings denied clean on both:
    Graviton4 180 of 180 (load 12 to 22), Xeon 156 of 156 (load 2.2 to 7.4). Before the leave routine (final-*.log):
    the same counts. The x86 machine did not answer SSH at 19:40 and around 20:15.
  - No new unsafe code (the route is in the zygote's assembly).
- W1 round 3, run 1 (2026-11-23, 21:00 UTC on), with the orchestrator's leave to change the translator's ARM64 way
  and the stop crate (both written up under "Changes to the translator and the stop crate" below):
  - The fourth check's eight findings were already answered in run 1 (commit dec511ab; plan 3.1, 3.2, 3.4, 3.6,
    3.8, 4.1, 6, 7, 13); nothing new to fix.
  - The ARM64 native output has a process way (`wcontainers_arm64::Way::Process`, beside `Machine`, the default):
    the slot table at 2^46 and a reach of 2^44 for exits and jumps through a register, and calls into the stub's
    floor entries instead of `brk` (one branch in line to out-of-line code that keeps x30 in the register page's
    new `link` word and calls the entry by `blr`; plan 3.2 item 3 says how and why). Its constants are
    `wcontainers_arm64::way`'s; host-process's layout asserts them.
  - Stop resolution is its own crate, crates/executor/implementation/arm64-stop, with the way as a parameter
    (`resolve(way, piece, taken, ...)`, `interrupted(way, ...)`); the VM inside's `stop` module reads its syndrome
    into `Taken` and calls it with `Way::Machine`.
  - The stub's register page now starts with the native output's register page (128 bytes), so x28 is one address
    for both; the stub's own words follow, the block at 256. Floor entries are 16 bytes: they keep x23 in the page
    (`CALL_SCRATCH`) before naming themselves in it, and the embedder puts it back into the block on a `Called`
    exit, since a jump's last-target call leaves its target in x23. A stop at an entry's first word keeps x23 from
    the frame.
  - tests/translated.rs (2): code the translator made for the process way names the slot table, its reach and the
    floor entries and no `brk`, a target past 2^44 calls the floor, x30 is the page's from the store on at a call;
    and a guest `mov x0, #7; mov x30, #0x1234; svc #0`, translated, runs in a stub, calls the system call's entry,
    keeps the guest's x30, and resolves through the stop crate to `Stop::Syscall` at the instruction after the
    `svc` with every guest register (the withheld ones from the page). First translated guest code run under P.
  - Verdicts so far (Graviton4, 21:13 to 21:17 UTC, load 7 to 13): arm64, translator, guest-cpu and stop crate
    tests pass; the VM executor's exits, native, floor and registers tests pass; host-process's floor (6), step (8)
    and translated (2) tests pass; clippy with warnings denied clean on the five crates.
  - Final verdicts after the code commits (21:18 to 21:22 UTC, scratch w1/r3run1/final1-arm.log and
    final1-x86.log, run 10's both.sh), each test binary 3 times with /dev/kvm denied, clippy with warnings denied
    clean on both: Graviton4 186 of 186 (load 3 to 9), Xeon 156 of 156 (load 2 to 20; the translated and floor
    tests build none there).
  - The VM executor after the commits (21:23 to 21:29 UTC, Graviton4 with KVM, load 8 to 17, scratch
    w1/r3run1/way3-arm.out): exits, native (32,562 cases, 0 failing), floor (22), registers, host_branches,
    interrupts and probe pass. The kernel test fails at its blocks program, "memory fault" 8 natively against 10
    on the WebAssembly side: `dc cvau` of an unreadable page runs on Graviton4 and faults on the M5, the hardware
    difference the native track recorded (docs/status-native.md, N4 run 20; that comparison needs the Mac). Not
    caused here: the machine way's code is unchanged and the native test's 32,562 cases agree. busybox and speed
    were not run.
  - No new unsafe code (the entries' change is in the zygote's assembly; the stop crate has none).
- W1 round 3, run 2 (2026-11-23, 21:55 UTC on), translated guest programs under form P:
  - The slot table (layout.rs, filter.rs): [2^46, 2^47), one 4 KiB page at a time. A slot page's bytes are the
    next free page of the code area's memfd past the code area (128 MiB of them, `SLOT_PAGES_SIZE`); the stub maps it
    by `Command::MapSlots` (received like a shared block, `PROT_EXEC` alone). The filter allows exactly that: one
    page of fd 0, execute-only, shared and fixed, its address in the table, its offset in the slot pages. A page is
    never given to another slot page, so a mapping in the stub is never stale; when all are in use the process is
    ended, counted "slot pages exhausted". The ARM64 stub's stop and fault handlers take a pc in the table as
    translated code's (an empty slot's `udf` or an unmapped page: the stop crate's "landed there").
  - Faults in translated code report the frame's `esr_context` (the handler walks the frame's records, register
    page word `EXIT_SYNDROME`, `Stub::fault_syndrome`), so a write is told from a read; where the frame has none
    (gVisor) write is taken from the page's permissions, counted "fault without syndrome".
  - The loader and run loop (native.rs, ARM64): guest code is translated for `Way::Process` in the embedder
    (`translate_native_for`, look-ahead full, exact count), kept by guest address and bytes for every memory of
    the executor, placed in the memory's code area, its slot filled, its exits and table entries linked both ways,
    as the VM inside does. Every exit is resolved with the stop crate: calls at the `blr` (block pc less 4), faults
    by their syndrome, stops as `Interrupted`. A jump's last target and an announcement of changed code are
    resolved at the call's word in line (found from the out-of-line call's closing `b`), not at its `blr`. Pages
    translated code came from lose write in the view; the first write, the kernel's `write`, `zero`, a shared map
    or unmap over them, and permissions without execute drop their pieces. `Cpu::registers` and `set_registers`
    on ARM64 are the guest's own state (vectors, FPSR, FPCR, `TPIDR_EL0` included); the register page holds the
    hardware's only while code runs.
  - The kernel's loop drives it through the runtime's `Execution` (execution.rs, the one module that implements an
    executor trait). A counted turn ends at the first exit at or after its count, or when a slice of one nanosecond
    per unit of its limit sends the stop (a host process cannot be stopped at an exact count): counted "turn limit
    passed". The trait of crates/executor/interface.rs on this branch is the single-instruction one; it is not
    implemented yet (Next).
  - tests/run.rs (4): a loop, a call and a return, a store and a load, getpid's return and exit through
    `Execution::run`, every register and the memory checked; a fork child storing to a page lent by its parent
    (its own copy; the parent's unchanged until it runs); 50 calls through a register (slots, then last targets);
    a loop with no exits ended 20 times by its slice and going on each time.
  - tests/kernel.rs (7), the kernel test programs in the kernel's loop with the seeded test policy, under form P and
    under the WebAssembly output (wasmtime's `CpuSide`), the same seed (`WCHP_KERNEL_SEED`, `WCHP_KERNEL_SEEDS`,
    1 seed run): hello (its output and status equal), getppid with 10,000 calls, forks (fork, exec itself,
    SIGUSR1, SIGTERM: equal, and its own lines and exit 3), mappings, jumptables: equal status and output. blocks and
    code: equal status; their output differs exactly where the VM form's kernel test records the WebAssembly side's
    known faults (a zeroed block's fault at the block's start; a function's old code run after its second line is
    rewritten, form P printing the Linux answer "7 8" against "7 7"). Not compared against the Docker reference
    here (the VM test does that through the Unicorn crate's reference).
  - Found by these tests: a last target made at the `blr` wrote its words over the out-of-line code (returns looped
    forever); an announcement resolved at the out-of-line `ldr` re-ran the announcing instruction forever.
  - Register isolation on translated threads (tests/run.rs, brief-native.md's last section): a thread holding its
    own values in every general and vector register and `TPIDR_EL0` is stopped at a system call; a new thread of
    the same process stores every register it has and finds zeros (but the x2 it was given); the first then goes on
    and stores exactly its own. With the vectors not carried into the block (the remote copy changed), it fails.
  - Final verdicts, every test binary 3 times with /dev/kvm denied (scratch w1/r3run2/both.sh and nokvm.sh, which now
    keeps PATH and HOME across sudo so the kernel test finds zig): Graviton4 222 of 222 (17 binaries, 23:05 to
    23:11 UTC, load 15 to 22, after the last code commit 1b7e9062), clippy with warnings denied clean; Xeon 156 of
    156 (22:52 UTC, load 29 to 42, commit 15b49cf1; nothing x86 builds changed after it), clippy clean. The Xeon
    then stopped answering SSH ("not set up", 23:05 UTC), so the second round there did not run. The first
    Graviton4 round failed only the kernel test, whose zig sudo's PATH did not have.
  - No new unsafe code (the code area's one write is the same, moved into `write_file`).
- W1 round 3, run 3 (2026-11-23 23:00 to 2026-11-24 01:30 UTC), Graviton4 (the Xeon did not answer SSH for most of it;
  both machines dropped SSH for minutes at a time):
  - Busybox (tests/busybox.rs, plan 13's N3 item): all 56 commands of the kernel test crate's busybox `text` and
    `files` lists and the VM form's own bzip2 pipe run as a container's first process in the kernel's loop under
    form P (root in memory, alpine:3.22's files, seeded policy, seed 1) and give the reference's output, exit code and
    changed files (Docker on Graviton4, `Reference::check` of wcontainers-executor-unicorn, a new dev-dependency).
    Passed first time, /dev/kvm denied (the docker group kept for the reference only), 29.8 s; 23.5 s after the
    resume fix below. tests/kernel_host (the run in the kernel's loop) and tests/guest_build (zig builds) are shared
    test modules now; the kernel test also checks the program's file is still in the root.
  - Sandboxes with translated code (scratch w1/r3run3/sandbox.sh, sb1-arm.out, sb1log/log; Graviton4, load 2 to 19):
    inside Docker's default profile every binary passes, the run (5) and translated (2) tests of translated guest
    code included, and the kernel test (7) once the sources are mounted read-only (speed1-arm.out: the first run
    mounted only /nix and the build folder, so zig found no source). The busybox test is left out of the sandboxes:
    its reference is Docker itself. Under gVisor: run 5's known failures and the step test's /proc read as before,
    the run and translated tests pass, and in the kernel test hello and getppid pass; forks, code and mappings fail
    on the WebAssembly side of the comparison (wasmtime's memory reads /proc/self/pagemap, which gVisor lacks; form
    P had finished its run), blocks and jumptables pass the 300 s limit (which side was not found).
  - Interrupted threads resume where they stopped (native.rs, `Guest::interrupted`): after a stop in translated code
    with `Resume::Continue`, the thread goes on at its host pc in the same loaded piece (checked by its guest start,
    base and code), instead of loading a piece from the guest pc the stop landed at. Before, each stop at a new pc
    made a translation from there: busybox `sh` starting 1,000 programs made 243,918 translations (speed2log), bench's
    matrix ran 3.97 times its plain time. `set_registers` drops it.
  - The kernel-entry count (`Counters::kernel_entries`, every exit given to the kernel but a turn's end: plan 7's k,
    standing in for the owner's `ProcessInfo::kernel_entries`) and `slow_paths()` for the measurements.
  - Per exit, two clock reads and an allocation fewer: `Stub::wait_within` reads the clock only after a wake that did
    not publish the turn, and `take_warnings` returns at once when nothing is new (profile below).
  - Speed (tests/speed.rs, ignored tests, scratch speed1.sh to speed3.sh, Graviton4, CPU 41, /dev/kvm denied):
    - Debug builds overstate the embedder's share (getppid 3,620 to 3,672 ns, speed1-arm.out); every figure below is
      a release build.
    - getppid round trip (translated `svc` loop, each call answered at once by the test's loop, timed at every exit),
      before the per-exit cuts (speed2log/log, load 4.7 to 8.4): median of batch means 3,048 to 3,059 ns against W0's
      futex slot's 2,395 to 2,413 ns alternating in the same run, 1.26 to 1.28 times (bar: under 1.25); per call
      median 3,022 to 3,031 ns, 99% 3,620 to 3,654 ns, largest 197 to 214 µs, none over 1 ms in 1,000,000 (3 runs).
      A profile of CPU 41 (check3-arm.out, 3,000,000 calls, load 10.7): 33% the host's task switch, 11% its unlock,
      6.7% `gup_fast_fallback` (the futex word is in shared memory), the embedder's own code about 14%, the vDSO clock
      about 4%. After the cuts and the resume fix (speed3-arm.out, load 11.8 to 12.7): 3,021 to 3,060 ns against the
      slot's 2,373 to 2,396 ns, 1.26 to 1.29 times, still over the bar; per call median 3,001 to 3,021 ns, 99% 3,556
      to 3,667 ns, largest 43 to 44 µs, none over 1 ms. The cuts did not move the median measurably.
    - getppid in the kernel's loop with the seeded policy (whole runs of 1,000,000 calls against none): 3,481 ns per
      call, 3,441 after the cuts (debug 4,322).
    - bench.c's eight workloads (SCALE 1), whole runs, medians of 3, before the resume fix (speed2log/log): form P
      against plain fib 1.317, sieve 1.014, hash 1.002, sort 1.360, interpret 1.318, matrix 3.965, walk 0.998, search
      1.161, geometric mean 1.350; the WebAssembly output (wasmtime, look-ahead full, in the same loop) 24.997, which
      is far from the engine track's figures and not examined here (its run includes compiling at look-ahead full and
      the seeded policy's counted turns). After the resume fix (speed3-arm.out): the same within 1.5% (matrix 4.014,
      geometric mean 1.349), so stops were not matrix's cost. Not yet known: whether it is translation time at
      look-ahead full, the count's adds or the probes of accesses that may cross a page; a profile of matrix is next.
    - busybox `sh` starting 1,000 programs, before the resume fix: 14.15 s first, 8.55 s on the same executor again
      (translations kept), 26,027 kernel entries each; the reference 0.32 s; process creation and end 296 µs; model
      6.60 s; first run 2.15 times the model (bar: 1.25). After the fix: 13.54 s then 7.83 s, model 6.70 s, 2.02 times,
      and still 243,918 translations: alpine's busybox is position-independent and the kernel places each exec at a
      new random base, so translations kept by guest address never match again. Keeping them by bytes and moving them
      (`wcontainers_arm64::carry::movable(Way::Process)`, which no P loader uses yet) is what this bar needs.
  - Final verdicts of this run's code (commits 0932c4a3 to a8f480ad), on a tree of 3f7cb472 (the commit before the
    owner's merge) with this crate's files, every test binary 3 times with /dev/kvm denied (the docker group kept),
    each run first proving the device cannot be opened, clippy with warnings denied clean (scratch w1/r3run3/
    final2-arm.log, lx/log): Graviton4 57 runs, 225 tests passed, none failed (01:08 to 01:15 UTC, load 10.5 to
    12.9); Xeon 57 runs, 156 passed, none failed (load 26 to 30 from other tracks).
  - No new unsafe code.
  - The VM executor's own speed test on the same machine with KVM, right after P's figures (speed3-arm.out, its
    look-ahead 256, 3 pairs, load 11.7 to 12.6): native against plain, median cycles, fib +41.3%, sieve +5.4%, hash
    +2.8%, sort +40.4%, interpret +35.7%, matrix +41.6%, walk +1.9%, search +32.3%, calls +22.5%, geometric mean
    +23.96% (its bar +21.2%: fails here); its system call 472 ns, the kernel's share 204 ns. Beside it form P's whole
    runs against plain (wall, start and translation included): fib +32.7%, sieve +1.1%, hash +0.3%, sort +35.9%,
    interpret +31.0%, matrix +301%, walk -1.1%, search +15.6%. So P runs the compute and memory-bound workloads
    (sieve, hash, walk) at the plain speed and as fast as or faster than the VM form elsewhere, but for matrix; P's
    system call is 6.4 times the VM form's (3,021 against 472 ns).
  - Not done: the executor suite (see "For the owner").
- W1 round 3, run 4 (2026-11-24, 01:25 to 02:40 UTC), Graviton4:
  - The executor interface (execution.rs, still the one module that implements an executor trait): `HostProcess`
    implements `Executor` as the owner's interface stands at b01ba0d8, ARM64 only. `HostProcessMemory` is the
    `MemorySpace` (a stub and its memfd; `borrow` gives `HostProcessBytes`, read and written through the window
    cache), `HostProcessInterrupter` the `ExecutionInterrupter`. The executor's state is shared by the executor and its
    memories (`Rc<RefCell<..>>`: `HostProcessExecutor` is not `Send`), never held while a trap handler runs.
    - `execute` returns a future that runs to its end at its first poll (the guest runs in another process, and
      nothing in the interface wakes a waiting future when it answers) on a host thread of the memory's stub (its
      spare, made and dropped per execution), answering system calls, faults and interrupts through the handler.
      Ends: a system call answered terminate stops at the `svc` with its syndrome (the immediate read from memory);
      a fault answered terminate is the abort at the access, a translation fault where the page has no permission;
      `brk`, undefined instructions and alignment faults end it with their exception; a stub that ended is an SError
      (For the owner). The guest's counter reads the host's monotonic clock (no clock is given).
    - The interrupter sets a request and sends the stop to the run's thread; the run takes a request before each
      entry and at each stop. (As built here an interrupt sent between the run's internal turns was lost, found by
      W1's independent check; fixed in round 6, below.) tests/interrupt.rs (2): a guest counting for ever, interrupted from another
      thread 1 and 5 times 50 ms apart, answered continue then terminate: each interrupt reaches the handler, the
      guest ends in its loop with its count, no exception.
    - `map_from` makes the other space's private pages a block both map (copied once, then shared); `unmap_from`
      zeroes the mapped parts; `copy_from` copies private bytes (only the memfd's data extents and borrowed pages),
      maps the same blocks and copies permissions; a block lives while some memory maps it. Only whole pages are
      mapped or copied (others refused as invalid addresses). A copy at the same addresses does not yet lend pages
      as `memory_duplicate`'s fork does (Next).
    - `execute_single_instruction_in_place`: a second executor whose pieces are a stretch of one instruction
      (`LookAhead::Bounded(1)`), never linked or given a slot, so the run ends at its first landing; one memory per
      state, made of the deltas, writable pages compared before and after.
    - Warnings print when a path's count reaches a power of two (the interface has no channel for them).
  - Found by the suite: P's fault handling took a `brk` syndrome for "other" (the process way's misalignment check
    calls by `brk #5`), now a call as in the VM inside; a cache-maintenance fault is a read, as the VM inside takes it.
  - A thread's vectors now stay in its register page between turns (read only when the kernel reads the registers):
    every exit copies 128 words fewer.
  - Executor suite (tests/conformance.rs, the committed suite with its executions polled to the end, see below):
    17 of 18 pass. `agrees_with_precise_instruction_set` fails at its first state: the input domain's code page is at
    0x1000 (For the owner). With the domain's code moved to 64 KiB in the remote copy only, 2 of the generated
    instructions still disagree with the translator's precise set, both process-way translation gaps (section on
    the translator below): `StrImmediateUnsignedOffset64` 180 of 1,728 states, `StpSignedOffset64` 3,474 of 8,640.
    Every other instruction agrees on every state.
  - Verdicts (02:20 UTC, scratch w1/r3run4/run2-arm.out, load 15 to 20): every other test binary passed with
    /dev/kvm denied (18 binaries, 78 tests), clippy with warnings denied clean on all targets. Not run here: kernel,
    busybox and speed, which need the WebAssembly executor and the Unicorn crate; at that time the Unicorn crate (and
    first the WebAssembly one) did not build against b01ba0d8 in the shared tree, and the committed suite does not
    build against it (it calls `execute` as returning the state). The runs here overlay, in the remote copy only, the
    committed suite with each `execute` polled to its end, and leave out the tests that need what does not build
    (scratch w1/r3run4/fixes.sh, drop-unicorn.sh).
  - The kernel test with only busybox's Unicorn left out (02:45 UTC, kernel-arm.out, /dev/kvm denied, load 8 to 10):
    all 7 programs give the WebAssembly output's status and output under form P after this run's changes.
  - Xeon (02:40 UTC, run2-x86.out, /dev/kvm denied, load 40 to 52 from other tracks): every binary that builds there
    passed (59 tests; the ARM64-only ones build none), clippy with warnings denied clean on all targets.
  - getppid round trip after the vectors change (release, CPU 41, /dev/kvm denied, load 9 to 11, speed4-arm.out),
    3 rounds alternating with W0's futex slot: median of batch means 3,130, 2,976 and 3,001 ns against the slot's
    2,397, 2,427 and 2,397: 1.31, 1.23 and 1.25 times; per call median 3,024, 2,949, 2,934 ns against 2,325, 2,352,
    2,319 (1.25 to 1.30). Not clearly under the bar of 1.25; before the change 1.26 to 1.29.
  - No new unsafe code.
- W1 round 3, run 5 (2026-11-24, 02:05 to 02:45 UTC):
  - The shared tree built again at the start (the suite following b01ba0d8, Unicorn conformed, others' edits in
    progress included): every test binary 3 times with /dev/kvm denied, with no overlay, kernel, busybox (its Docker
    reference) and conformance included (scratch w1/r3run5/all1-*.out, run 4's nokvm.sh): Graviton4 285 passed, 3
    failed (02:10 UTC, load 8.7 to 17.8), the 3 being `agrees_with_precise_instruction_set` at the input domain's
    0x1000 (For the owner); Xeon 156 of 156 (02:10 UTC, load 37 at the start, 494 at the end from other tracks).
  - The two process-way gaps the precise-set test showed, fixed in host-process (native.rs):
    - A store that may cross a page is probed by an atomic add of zero, whose syndrome says read where a read would
      fault too; a fault of a plain store (`str`, `stp` and their vector forms, read from the guest's instruction)
      is now always a write, as Linux reports it.
    - Linux puts no syndrome of its own in a `brk`'s signal frame (it records one for aborts and undefined
      instructions only, so a frame can carry an earlier fault's): a SIGTRAP in translated code is now read from the
      loaded word at the pc. The misalignment check among a pair's probes (sp not a multiple of 16 where the pair may
      cross a page) calls by `brk #5`, which before ended the process as "stub exit not understood".
    - tests/run.rs (+1): a store crossing into the unmapped page after the data faults there as a write and stores
      nothing; `stp x0, x0, [sp, #8]` with sp 8 past a multiple of 16, in a page and at a page's end, is an
      alignment fault at sp. With either fix undone in the remote copy, it fails (scratch w1/r3run5/mutate4-arm.out).
    - With the input domain's code at 64 KiB (remote copy only), the precise-set test now passes: every generated
      instruction on every state agrees (scratch w1/r3run5/treeA-arm.out; before: `str` 180 of 1,728 and `stp` 3,474
      of 8,640 states disagreed).
  - The owner's volatile bytes (45dc7425, merged at 02:20 UTC): `VolatileBytes` is its own crate and `read_to` and
    `write_from` are required; host-process imports it from there, and gives a sink all the bytes in one write and
    a source one read into all of them, through a buffer (execution.rs).
  - After that merge the shared tree does not build host-process at all: the kernel (and the ICMP and raw socket
    implementations) do not yet follow the volatile bytes, and host-process's library depends on the kernel through
    the runtime crate. So this run's code was verified on two scratch trees, each the commit before the merge
    (a7b0b008) with the executor suite's executions polled to their end:
    - tree A, with this run's native.rs and tests/run.rs (scratch w1/r3run5/treeA): every test binary 3 times with
      /dev/kvm denied; Graviton4 285 passed, 3 failed (02:27 UTC, load 4.1 to 4.5), the 3 being the precise-set test
      at 0x1000, once per round; Xeon 156 of 156 (02:27 UTC, load 24 to 48). Then the precise-set test with the code
      at 64 KiB: passes.
    - tree B, tree A with the volatile bytes crate and the executor's memory space of 45dc7425 and this run's
      host-process (scratch w1/r3run5/treeB): clippy with warnings denied clean on all targets; the conformance
      binary 17 of 18 (the 0x1000 one), interrupt 2 of 2, run 6 of 6 (Graviton4, 02:33 UTC, load 5 to 8). The
      kernel, busybox and speed tests were left out there (they need the WebAssembly and Unicorn executors, which
      did not follow the merge in that tree).
  - The rented ARM64 machine's disk filled at 02:16 UTC (581 GB used of 581); this track's build folder was 69 GB,
    and removing its incremental data (19 GB) let the runs go on. The machine's /work is no longer the 5 TB stripe
    arm-machine.md names.
  - No new unsafe code.

- W1 round 4, run 1 (2026-11-24, 03:05 UTC on; the owner's executor changes of 2026-11-23 merged):
  - Conforming: host-process already followed the interface on this branch (`execute` returning a future and its
    `HostProcessInterrupter`, `HostProcessMemory` as the `MemorySpace`, `HostProcessBytes` as the `VolatileBytes` of
    crates/volatile-bytes, `ProcessorStateOf`); nothing in it had to change. The in-process executor (form I) does not
    exist yet (W2), so there is nothing of it to conform. The test host (tests/kernel_host) was conformed to the
    volatile bytes by the callers' job (c07e21cd).
  - The executor suite on form P (tests/conformance.rs, the shared suite as committed at 79441339, no overlay): 20 of
    21 pass on Graviton4 with /dev/kvm denied (03:08 UTC, load 4 to 10); the one failing is
    `agrees_with_precise_instruction_set` at the input domain's 0x1000 (For the owner), as before.
  - The zygote's signal mask around its fork is set through nix's `pthread_sigmask` (safe), two unsafe uses fewer
    (28 in the library). tests/zygote.rs and every other binary that builds pass (below).
  - `MemorySpace::copy_from` at the same addresses now lends the source's private pages as a fork does (the source's
    writable ranges lose write in its view by a round trip before the call returns); a copy within one space goes
    through a memory of its own first, so overlapping ranges read the bytes as they were (before, the destination was
    zeroed first and an overlapping copy lost them). tests/copy.rs (2): the source's store after the copy is a
    resolved fault that copies the page, each side then stores its own value and keeps it; a copy onto itself changes
    nothing, one page up and one page down give the shifted pages. With the source's loss of write undone (remote copy
    only), the first fails (scratch w1/r3run6/mutate-copy-arm.out).
  - Verdicts with /dev/kvm denied, each binary once, each run first proving the device cannot be opened (scratch
    w1/r3run6/t1-arm.out, t1-x86.out, copy4-arm.out): Graviton4, every binary that builds (the remote copy leaves out
    the kernel, busybox and speed tests and their WebAssembly and Unicorn dev-dependencies, run 4's drop-wasm.sh,
    because wasmtime does not yet build against the volatile bytes crate: its conforming job's): all pass but the
    precise-set test above; Xeon: all 156 pass. Clippy with warnings denied clean on the library on both and on every
    target on Graviton4.
  - Why bench's matrix runs 4 times plain under P (scratch w1/r3run6/perf-matrix.sh, perf-matrix3-arm.out, dump.sh,
    dump4-arm.out; Graviton4, release, load 17 to 28; the remote copy alone leaves out the WebAssembly and Unicorn
    dev-dependencies): matrix 115 ms plain, 474 ms under P (4.12; 3.90 and 4.04 in other runs). perf: 88% of the
    samples in the code area, 74% on two instructions of one 11-instruction loop. That loop is matrix's innermost
    (`ldr x6; ldr x7; subs x18, x18, #1; add; add; madd; b.ne`, 6 instructions plainly), whose counter is the guest's
    x18. x18 is one of the native output's three withheld registers (the count's, `thread_state::COUNT`), so the
    translated loop keeps it in the register page: `ldr x23, [x28]; subs x23, x23, #1; str x23, [x28]` every
    iteration, a dependency carried through memory. The machine way withholds the same registers, so the VM form pays
    it too wherever a hot loop names x18. Not the stops, the count's adds or the page-crossing probes (the aligned copy
    of the loop has none). The fix is the translator's register choice (docs/native.md 3.3), shared with the VM form:
    for example a piece that names a withheld register in a loop renames it to a host register the piece leaves free,
    loaded at the piece's entry and stored at its exits. Not built: it changes both ways, so it is for the native
    track to rule (the ask is under "Changes to the translator and the stop crate"). A rebuild of bench with
    `-ffixed-x18` to confirm it did not take effect (zig's C compiler still used x18).
  - Final verdicts after the last code commit (5a9ade5e), every binary 3 times with /dev/kvm denied, each run first
    proving the device cannot be opened (scratch w1/r3run6/final-arm.out, final-x86.out, about 03:30 UTC):
    Graviton4 (load 5 to 21; the remote copy leaving out kernel, busybox and speed as above) 57 runs, 276 passed and 3
    failed, the 3 being the precise-set test at 0x1000 once per round; Xeon (load 17 to 23) 66 runs, 156 passed, none
    failed.
  - Not done this run: the kernel, busybox and speed tests on the shared tree (they wait on wasmtime).
  - No new unsafe code; two uses fewer.
- W1 round 4, run 2 (2026-11-24, 03:30 UTC on), Graviton4:
  - Conforming: nothing new to follow since run 1 (host-process already follows the 2026-11-23 interfaces; form I
    is W2's).
  - Translations move (native.rs): kept by the code they were made from (the rest of its page), its offset in the
    page, and the page only where the code is outside `wcontainers_arm64::carry::movable(Way::Process)` (4 GiB to
    under 2^44 less a branch's reach); a load at another page clones the kept one and moves it there
    (`Translation::moved`), as the VM form's translator does. Pieces now hold a `Translation`, not a `HostCode`.
    This is busybox `sh`'s missing piece (a position-independent program at a new base at every exec). tests/run.rs
    (+1): a program naming its own addresses (`adr`, `bl`, a loop) runs at 8 GiB, then at three other movable bases
    with no new translation (registers, link and data checked at each), then under 4 GiB with one. With the move
    left out in the remote copy, the test fails (scratch w1/r4run2/mutate-move-arm.out).
  - The thread past the last index through the `Cpu` calls (tests/run.rs +1): 32,768 threads made in one memory,
    the next `thread_new` is `OutOfMemory` and counted (`threads_exhausted`), the last made runs guest code, and one
    let go makes room for exactly one more (2.1 s).
  - Forged exits as translated guest code after two forks (tests/forged.rs, 3; plan 13): A forks B and C, A's store
    gives the page to B, which lends it to C; B's code writes a forged exit kind (all eight, 0 and 9), value, detail,
    syndrome, request word and thread id into its register page, runs on past the embedder's 100 ms look at the
    request word, then stores to the page. 80 cases. Whatever the forgery: C's copy is the page at the fork, A keeps
    its store, both still run and read their own, B's page is the fork's or its own store, and the runner returns
    (a watchdog kills B after 20 s and fails the case). Seen (scratch w1/r4run2/forged5-arm.out): a forged stop is a
    stop (B runs on and exits); a forged fault is a fault for B; every other kind ends B ("stub exit not
    understood"). Also a store to its own control pages (three of them) is the guest's write fault at that address,
    and its thread runs on.
  - Found by it and fixed (stub.rs): a forged `CommandRefused` whose error number is outside 1 to 4,095 panicked the
    embedder in rustix's `Errno::from_raw_os_error` (forged4-arm.out); it is now a forged exit, which ends the stub.
    Not asserted: that no host code runs that we did not translate (nothing observes it from outside).
  - The shared tree built every host-process test binary again at about 03:45 UTC (with other jobs' uncommitted
    wasmtime and Unicorn edits in it). Every binary once with /dev/kvm denied, no overlay, kernel (7 of 7) and
    busybox (Docker's reference) included (03:50 to 03:53 UTC, load 15 to 22, scratch w1/r4run2/all2-arm.out): all
    pass but the known `agrees_with_precise_instruction_set` at 0x1000. An earlier attempt (all1-arm.out) lost its
    job after busybox's reference failed with "docker could not run the container"; Docker ran again a minute later.
  - The x86-64 machine was not reachable for this run (x86-run: "the machine answers but is not set up (no /work,
    Docker or Nix)"), so nothing ran there; this run's code changes on x86-64 are stub.rs's refusal check only.
  - busybox `sh` showed the movable range alone did nothing (scratch w1/r4run2/diag1-arm.out): every translation was
    under 4 GiB (busybox at 0xab0e_7000 and up, musl near 0xf7xx_xxxx), because the kernel now places a process's
    mappings low first (`first_tier_top`), below `movable(Way::Process)`. So a kept translation now also moves
    between pages in [128 MiB + a page, the movable range's end) when every guest address it names (its places, exit
    targets and table entries) is under 2^44 both before and after (`moves`, native.rs). Proved against translating
    there: a seeded test of random code naming addresses (the VM form's generator), pages under 128 MiB, under 4 GiB
    and movable (`WCHP_MOVED_CASES`, default 2,000; `WCHP_SEED`): seeds 1 to 3 at 100,000 cases each, about 43,000
    moved each (208,000 address places), none differing; with the check reduced to "both pages above the first" it
    fails at once (moves2-arm.out). tests/run.rs's moving test adds two bases under 4 GiB.
  - Speed after it (release, CPU 41, /dev/kvm denied, scratch w1/r4run2/speed1-arm.out, speed-bb1-arm.out; load 15
    to 30 from other tracks):
    - getppid round trip, 3 rounds against W0's futex slot in the same run: 2,948 / 2,400, 3,009 / 2,377 and 3,013 /
      2,431 ns median of batch means, 1.23, 1.27 and 1.24 times (bar under 1.25: not met in every round); per call
      median 2,917 to 2,930 ns; 1 call over 1 ms in the first round's 999,999 (1.65 ms, at load 30), none in the others.
    - getppid in the kernel's loop 3,346 ns per call.
    - bench.c against plain (medians of 3): fib 1.318, sieve 1.015, hash 1.003, sort 1.361, interpret 1.293, matrix
      3.845, walk 0.999, search 1.161, geometric mean 1.342 (WebAssembly 25.4).
    - busybox `sh` starting 1,000 programs: translations 243,918 to 1,052, first run 13.96 s to 9.30 s, the second
      9.0 to 9.35 s (moved translations are cloned at each load, not kept). The model is now 1.0 to 1.36 s (no
      translation time left), so the first run is 9.25 times it (bar 1.25: far from met).
  - Where busybox `sh`'s 10 ms per program goes (remote-only timing, timing-bb3-arm.out, 2,000 programs): creating,
    forking and dropping memories 1.45 ms (drop 0.72, of which waiting for the stub's end 0.33); loading 415 pieces
    1.2 ms and placing and linking them 0.7 ms (each process loads every piece again into its own code area); the
    rest, about 6.6 ms, in turns (guest code, its faults and the kernel's work). A profile (perf-bb1-arm.out) has the
    host's task switches first (11%), then hashing the translation keys (3%, the whole rest of a page per lookup) and
    cloning moved translations (4%). Next for this bar: one code area shared by an executor's memories, so a fork or an
    exec of the same program places nothing again, then a profile of the turns.
  - Final verdicts after the last code commit (341f0ef8), shared tree, every binary once with /dev/kvm denied, each
    run first proving the device cannot be opened (04:20 to 04:23 UTC, Graviton4, load 6 to 14, scratch
    w1/r4run2/final-arm.out): 23 binaries, 101 tests passed, 1 failed (the precise-set test at 0x1000), the kernel's
    7 and busybox included; clippy with warnings denied clean on every target. One round only, not three; x86-64 not
    run (machine not set up).
  - No new unsafe code.
- W1 round 5, run 1 (2026-11-24, 04:40 UTC on):
  - Conforming: nothing new since round 4 (no interface commit touched the executor, memory space or volatile bytes
    after 79441339); form I does not exist yet.
  - Three rounds with /dev/kvm denied from committed code (2086b90e, a `git archive` tree in scratch w1/r5run1/tree,
    build folder nwvmc), every binary, each run first proving the device cannot be opened (nokvm3-arm.out,
    nokvm3-x86.out): Graviton4 318 passed, 3 failed, the 3 being `agrees_with_precise_instruction_set` at 0x1000 once
    a round, kernel and busybox included (load 3 to 250 from another track's job); Xeon 156 passed, none failed (load
    2 to 15). This is the "every binary 3 times on both machines" item of round 4.
  - Where busybox `sh`'s turns went (remote-only counters, turns-bb.sh, turns1-arm.out; Graviton4, load 14 to 33): 533
    turns a program, of which 390 were a thread stopping at an empty slot for a piece to be loaded and 100 a jump's
    first miss making its last target; 26 system calls and 21 faults. Each new memory loads every piece it runs with a
    round trip of its own (about 10 us each with the load), which the model does not count.
  - Tried and measured slower, not kept: copying the parent's code area, slots and records to a fork child at the fork
    (copy_file_range of about 900 KB and the records' clone, 0.8 ms a fork; ab1-arm.out, three alternating rounds:
    9.64, 9.58, 9.11 s copying against 9.14, 8.61, 8.42 s not).
  - Built: pieces loaded ahead (native.rs `load`). Each memory records, for its first piece and each piece it then had
    to load, how far from it the next piece it had to load was, and which last targets it made for that piece's jumps
    (call offset in the piece, target relative to the piece). When a later memory loads the same first piece (the same
    kept translation, so the same code), each load is followed by the pieces and last targets recorded after it, as
    long as each piece's page is executable for the guest and its code is that of a kept translation (no translating
    ahead); at the first that is not, it stops, and the next stop goes on from there. A memory's last piece records
    that nothing followed when the memory is dropped. Counted: `pieces_loaded_ahead`, `last_targets_made_ahead`.
    Keyed by the first piece so a fork child and an exec of the same busybox do not load each other's runs (keyed by
    piece alone, busybox `sh` loaded 1,035 pieces a program for 416 needed; turns4-arm.out). Relative offsets make it
    hold across the kernel's random bases within one mapping; a jump across two mappings at independent bases stops
    the chain there.
  - Tests (tests/run.rs +2): a program of eight pages, each its own piece, run at a base, again there and at another
    base (every later page and the code after its getpid loaded at the first stop, 8 pieces), with one page's code
    changed (its own add runs; 6 loaded ahead: the two pages before it, then the four after the page after it), and
    with a later page left readable only (the guest's execute fault there, nothing loaded past it); a call through a
    register whose target is read from data, run three times in fresh memories, the third with another target: the
    last targets made ahead in the second and third (at least 2 each), each sum right (the third's wrong guess goes
    where it jumps). With the permission check skipped ahead, with nothing loaded ahead, and with no last target
    made ahead (remote copy only, mutate-ahead.sh, mutate-ahead3-arm.out), each fails.
  - Speed after it (release, CPU 41, /dev/kvm denied, speed1-arm.out; Graviton4, load 12 to 18): busybox `sh` starting
    1,000 programs 4.90 s then 4.75 s (9.30 s before), model 1.12 s, 4.36 times the model (9.25 before); turns 59 a
    program (533 before; targets1 run). getppid round trip against W0's futex slot in the same run: 2,947 / 2,388,
    2,924 / 2,406, 2,965 / 2,412 ns median of batch means, 1.234, 1.215, 1.229 times (bar under 1.25: met in all three
    rounds this time; unchanged code on that path, so this is the machine's quieter load); per call median 2,890 to
    2,908 ns, none over 1 ms in 999,999 calls. getppid in the kernel's loop 3,352 ns. bench against plain: fib 1.334,
    sieve 1.019, hash 1.002, sort 1.361, interpret 1.334, matrix 3.770, walk 1.002, search 1.163, geometric mean
    1.347 (WebAssembly 25.4).
  - Verdicts after it, shared tree, every binary once with /dev/kvm denied (ahead-all1-arm.out, after the first change;
    all-speed1-arm.out after the second, 05:50 UTC, load 14 to 32): all pass but the known precise-set test (108
    passed, 1 failed); clippy with warnings denied clean on every target of the crate (`--no-deps`: the kernel
    interface's two `type_complexity` findings stop a clippy run that lints dependencies, the owner's file). Xeon,
    every binary once with /dev/kvm denied after the second change (all1-x86.out, load 27 to 35): 52 passed, none
    failed; clippy clean.
  - Placement's two measured items, built as ignored measurements of tests/speed.rs on translated code (a getppid loop
    with about 10 us of work between calls, one memory kept across the phases; knobs `WCHP_CALLS`, `WCHP_WORK`,
    `WCHP_SET`, `WCHP_SECONDS`): `placement_after_a_move_runs_as_on_a_free_cpu` and
    `placement_with_every_cpu_contended_moves_at_most_once_a_second`. Building them found two faults of the rule
    (executor.rs, host.rs), now fixed: a window holding a stub that ended decided nothing (a runner of short-lived
    processes, each loop a new memory, never moved: 0 moves in 10 s with a program spinning on its CPU); the stub's
    clock is now read just before it ends and its time kept for the window. And with every CPU contended the runner
    took the CPU held by the fewest, moving every window (about 50 moves a second; 420 to 513 in 10 s); a runner
    now moves only to a CPU not found contended in the last second, and at most once a second (with the first
    alone, 30 in 10 s as the marks expired one by one). The plan's 4.3 says so.
  - Measured after the fixes (placement.sh, placement4-arm.out; Graviton4, 3 rounds, /dev/kvm denied, load 13 to
    14): with a program spinning on the runner's CPU it moved once, after 9 to 15 ms, and its loop then ran 14,385 to
    14,394 ns a call against 14,232 to 14,248 ns on that CPU once the program ended (1.0098 to 1.0108 times; bar
    1.05) and 14,399 to 14,504 ns on the first CPU before (0.992 to 0.999 times); with all four CPUs of its set each
    running a spinning program, 10 moves in 10.0 s in each round (bar: at most one a second). tests/placement.rs and
    tests/host.rs pass. Seen on the way: the same loop in a new memory each time ran either 14.4 or 16.5 us a call,
    so the comparison now keeps one memory (where its pieces lie in the code area may be the cause; not measured).
  - The copy unit on translated stores (tests/speed.rs `copy_unit_on_translated_stores`, a fork child's loop storing
    once to each of its parent's 16,384 written pages in order, then to every 16th in a seeded order; unit.sh edits
    the unit in the remote copy; unit1-arm.out, Graviton4 CPU 41, load 12 to 15, 3 rounds each): in order 18.2 to
    18.9 us a store at one page, 4.1 to 4.3 at 8, 2.3 to 2.5 at 64, 2.1 to 2.2 at 512; scattered 23.4 to 24.3,
    34.8 to 35.0, 31.0 to 31.5, 27.4 to 27.5 us (2,058 host mappings in the child at 1 and 8 pages, 12 from 64).
    busybox `sh` (unit-bb.sh, alternating, 3 rounds): 4.52 to 4.56 s at 8 pages against 4.67 to 4.78 s at one. Set
    to 8 pages (`COPY_UNIT`, now public), the plan's candidate. The three copy-counting tests of tests/memory.rs
    write one page per unit (`unit`); a new one checks that a store copies the unit's eight lent pages and the
    other seven are then the child's (it fails at one page). Every binary once with /dev/kvm denied after it:
    Graviton4 108 passed, 1 failed (the known precise-set test; unit3-all2-arm.out), Xeon 53 passed
    (unit3-all-x86.out); clippy clean on both.
  - Final verdict after the last code commit (de4a1037), shared tree, Graviton4, every binary once with /dev/kvm
    denied plus the unit test (final-arm.out, load 10 to 15): 110 passed, 1 failed (the known precise-set test);
    clippy clean. No new unsafe code this run.
  - Verdicts after the placement fixes, shared tree, every binary once with /dev/kvm denied: Graviton4 108 passed, 1
    failed (the known precise-set test; all3-arm.out), Xeon 52 passed, none failed (all2-x86.out); clippy with
    warnings denied clean on both.
- W1 round 5, run 2 (2026-11-24, 06:10 UTC on), Graviton4 (the Xeon answered "not set up": no /work, Docker or Nix):
  - Conforming: nothing new. No commit since eb5b1bcc touched the executor, memory space or volatile bytes interfaces
    (the last that did, 9b572652 and 167cffce, are conformed to since round 4); host-process has no register_tool
    lines; form I does not exist yet. The getters entry (22eaab2d) is the sockets' and not on `agents` yet.
  - Verdict at the start, shared tree at eb5b1bcc, every binary once with /dev/kvm denied (verdict1-arm.out, load 16
    to 20): 109 passed, 1 failed, the known precise-set test (the executor suite's other 20 pass).
  - K2 is in the kernel in the form P needs (KF, first run): mappings are placed in tiers lowest first, and the first
    tier's top is the lower of 4 GiB and the container's ceiling, which the embedder may give in `Start::ceiling`
    with the memory size in `Start::memory_bytes`. So the kernel test now gives both outputs the WebAssembly output's
    ceiling (0xFF00_0000) as ceiling and memory size, and compares wait status, output and root files of every
    program, and the turns of hello, getppid and a new program, `layout` (tests/guest/layout.c: sysinfo's memory, the
    stack, break, heap and mmap addresses, and /proc/self/maps), which are equal (kernel1-arm.out). With form P
    given its own ceiling instead (remote copy only, mutate-layout.sh), the layout test fails on the memory size
    (mutate-layout2-arm.out).
  - Built `a_stop_at_every_counted_event_changes_nothing` (tests/kernel.rs): jumptables, layout and forks under P
    with every turn ending after one counted event (the policy's `turn_events`) give the same status, output and root
    files as without, in 298, 215 and 184 turns against 65, 20 and 38 (kernel2-arm.out, 9 of 9 kernel tests passed
    with /dev/kvm denied, clippy with warnings denied clean on every target). What is not built: the count at each
    stop compared with the WebAssembly output's. That output ends a limited turn at its own stop checks, not at the
    count's definition (the VM form's kernel test says the same), so the two counts are not the same quantity; the VM
    form compares its count across look-aheads, which P cannot (it translates at one look-ahead).
  - Where the getppid round trip goes (perf.sh, perf1-arm.out; CPU 41, 3,000,000 calls, 3,053 ns median of batch
    means with perf recording, load 8 to 13): the host kernel 78% of the samples in the top 90 lines (task switch
    36%, the futex's page lookup for a shared word about 8%, the stack offset's random draw 1.6%), the embedder's own
    code 11% (`run_translated` 2.5%, `give` 1.7%, `wait_within` 1.0%, `turn_with`, `set_registers_but_vectors`,
    `run_turn`, `stop::resolve`, `translated_page` under 1% each), clock reads in the vDSO 3.2% (two of the three
    a round trip are placement's turn timing, one the test's), memcpy 2.2%, the stub's own code about 3%. So at most
    about a sixth of a round trip (about 500 ns) is ours to cut; the rest is the host's switch.
  - Final verdict after the last code commit (9593baba), shared tree, every binary once with /dev/kvm denied
    (final-arm.out, load 8 to 12): 111 passed, 1 failed (the known precise-set test). No new unsafe code this run.
- W1 round 5, run 3 (2026-11-24, 06:30 UTC on), scratch w1/r5run3, both machines:
  - Conforming: nothing left. The notice's executor, memory space, interrupter and volatile bytes changes (167cffce,
    1dd2684e, 9b572652) were conformed to in round 3 runs 4 and 5 and round 4; host-process writes no register_tool
    lines; form I (crates/executor/implementation/in-process) does not exist yet (W2). The executor suite runs on P
    in tests/conformance.rs and tests/interrupt.rs: 20 of 21 pass, the precise-set test fails as before (For the
    owner).
  - The round trip's per-exit register copies (round 5 run 2's profile): a translated thread's registers but its
    vectors are now written and read word by word (`Stub::set_scalars`, `Stub::scalars`), with no 800-byte register
    block built, zeroed and copied at each exit. Measured A/B (ab-rt.sh, ab-rt1-arm.out; Graviton4 CPU 41, /dev/kvm
    denied, load 2 to 9, 5 rounds alternating with W0's futex slot): before 2,928 to 2,983 ns, after 2,904 to 2,975
    ns, slot 2,394 to 2,409 ns. No measurable difference (means 2,957 and 2,950 ns); kept as the smaller work. Every
    one of the ten runs is under the bar: 1.215 to 1.241 times the slot before, 1.213 to 1.241 after.
  - Verdicts from 9de60558, every binary with /dev/kvm denied, each run first proving the device cannot be opened:
    Graviton4 3 rounds (verdict1-arm.out, verdict2-arm.out; load 3 to 7), 111 passed and 1 failed (the known
    precise-set test) in each; Xeon 2 rounds (verdict2-x86.out; load 23 to 81, other tracks' jobs) 53 passed in each,
    and 1 round before the change (verdict1-x86.out) 53 passed. Clippy with warnings denied clean on every target on
    both. No new unsafe code.

- W1 round 6 (2026-11-24, 07:00 UTC on), scratch w1/r6run1, Graviton4 at load 1.2 to 3.3, every run with /dev/kvm
  denied. The independent check's findings, each fixed or recorded:
  - An interrupt sent while the run was between its internal turns (a piece being translated or loaded, the thread
    not yet in its turn) landed in the stub's wait and was lost; the guest then ran for ever. Fixed: each turn of an
    execution waits with the run's request (`StopWhen`, stub.rs): once it is set the stop is sent as soon as the
    thread has taken the turn, and while the turn is not yet taken the wait looks again every millisecond. A stop
    that lands before the code runs (`TurnEnd::Stopped`) now leaves the thread where it was to enter, as an
    interrupt there does; before, the run went back to the state of its last exit. New test
    `an_interrupt_at_any_early_moment_reaches_the_handler` (tests/interrupt.rs, knobs `WCHP_SEED`,
    `WCHP_INTERRUPT_CASES`): one to three interrupts after seeded pauses of 0 to 300 us, spun, the guest's add
    seeded so its code is translated anew, every eighth case a new executor; each must reach the handler within 2 s.
    Run: 3 seeds of 1,000 cases, none lost (interrupts4-arm.out); with the fix undone in the remote copy
    (undo-fix.patch), seed 1 at 60 cases lost an interrupt in 18 cases (interrupts5-arm.out). Pauses of 0 to 3 ms
    slept, as first written, lost none even with the fix undone (interrupts1, 2): the window is tens of us.
  - `a_stop_at_every_counted_event_changes_nothing` stopped at no counted event: P ends a turn limited to one
    counted event at the next exit or where the slice's stop lands. Renamed
    `turns_limited_to_one_counted_event_change_nothing` and described as what it is; plan 13 and the gap list
    say `count_at_every_stop` is not built and what it needs. Its seeds are now compared each with its own run
    (it compared every seed with the first seed's, so the case knob failed on `layout`).
  - `blocks` and `code`, where the WebAssembly output is known to print otherwise, now compare form P's status and
    output with the program run on the host itself (tests/kernel.rs `on_this_host`): equal (check1-arm.out).
  - Register isolation's second part: `at_the_moment_a_thread_enters_its_code_it_holds_its_own_registers_or_zero`
    (tests/step.rs) steps an enter to its code's first instruction and reads x0 to x30, sp, pc, NZCV, v0 to v31, FPSR
    and FPCR from outside (ptrace), for random values, then none, then other values on one host thread: each holds
    exactly its own block with the floor's two named registers (check1-arm.out). TPIDR_EL0 has no register set in
    nix and is checked by read-back only.
  - Verdicts from this round's tree, every binary once with /dev/kvm denied, each run first proving the device
    cannot be opened (all1-arm.out, all1-x86.out): Graviton4 113 passed, 1 failed (the known precise-set test), load
    13 to 25; Xeon 53 passed, load 96 to 117 (other tracks' jobs). Clippy with warnings denied clean on every target
    on both. No new unsafe code.
  - Recorded, not changed: the plan's bar change of 944f8917 (Departures), the 150 ms allowed for a killed stub's end
    (Departures), `Execution::restored` not built (gap list, For the owner), the precise-set test (as before).
- W1 round 7 run 1 (2026-11-24), `count_at_every_stop` built (scratch w1/r7run1):
  - Conforming: nothing new (start-arm.out: the suite and the interrupter as round 6, the precise-set test failing as
    known).
  - The exact executor (plan 3.6): `HostProcessExecutor::exact(host)`, a stand-in for the embedder's choice for a
    container whose policy gives count limits (For the owner). Every piece is one instruction, never linked and never
    given a slot (`lone`, what the single-instruction executor already used), so every way out lands on an empty slot
    and a limited turn ends at the first instruction's end where the count reached the limit, with no slice: a turn
    per guest instruction, the slow path "turn with a count limit run in steps", counted per limited turn. The
    ordinary executor is unchanged (limits passed, counted). The single-instruction path now gives `run_translated`
    its start count as the limit instead of a special case.
  - Every return from translated code is counted and its count and pc hashed (`positions()`, rustc-hash's FxHasher,
    as the VM inside's `positions`), so the two compare.
  - Tests (tests/kernel.rs), one per program of the VM form's set, each with every turn limited to one counted event:
    status and output as the run without limits, no turn past its limit, returns and hash equal to the VM executor's.
    bench (built with the VM test's `-mcpu=generic+reserve_x18`) 177,072 returns, hash 10937141521840235265: EQUAL;
    calls 3,004, 18260131204484228776: EQUAL; jumptables: P 5,482 returns (hash 590264501367005601), the VM 5,484:
    FAILS, kept failing as a known failure. The VM's three figures were measured again this run on Graviton4 with KVM
    (vm1-arm.out, the same as N4 runs 15 to 21; that run's VM kernel test then fails later at `blocks`, "memory
    fault" 8 against 10, the recorded hardware difference). Ceiling none or the WebAssembly output's gives P the same
    figures.
  - Diagnosis so far (diag-count.py patches the remote copy only): at every instruction run under the exact executor
    in the three programs, P's count rose by exactly the definition's events of that instruction
    (`counted` of the shared finder; 0 mismatches), and jumptables has 72 exits on P's side, every one a system call,
    a fault or the one illegal instruction. So the two missing returns are exits the VM side gives that P does not (or
    P handles inside), not a count difference; which (the VM's code-changed path or its page faults are the likely
    places: the program rewrites code it ran) is not found.
  - Verdicts from this run's tree, every binary once with /dev/kvm denied, each first proving the device cannot be
    opened (all1-arm.out, Graviton4, load 19 to 25): 115 passed, 2 failed (the known precise-set test, and jumptables'
    count above); clippy with warnings denied clean on every target on Graviton4 and on the Xeon. No new unsafe code.

- W1 round 7 run 2 (2026-11-24, 08:20 UTC on), scratch w1/r7run2, Graviton4, every run with /dev/kvm denied:
  - Conforming: nothing new (no change to the executor, memory space or volatile bytes interfaces since round 4).
  - jumptables' two missing returns found (p-pos.sh and vm-pos.sh print every return's count, pc and exit on each side
    in a remote copy; vm.pos and p.pos): the program's two jumps to a misaligned target (`br` to an odd offset). The VM
    executor's single step ends the limited turn after the `br`, at the target (count 3707 and 3929), and its next turn
    gives the alignment fault; form P returned the fault at once, from the jump's exit. The VM's is the definition (the
    hardware takes the fault at the target's fetch), so P now ends a limited turn there too: an alignment stop at the
    thread's own pc is an instruction's end, like a landing (native.rs). Everything else on both lists is equal (the
    system calls' stack addresses differ, waiting on K2, as recorded). `count_at_every_stop`: bench, calls and
    jumptables now all equal the VM executor's returns and hash (jumptables 5,484, 17339527552300285948).
  - `differential` built (tests/differential.rs): the VM form's differential test under P. Every covered form's layer 1
    cases (proptest's ChaCha from the seed and the form's name, as the VM's), each drawn load or store's edge case
    (across a refusing page, tagged, bit 55, misaligned), the words layer 1 leaves out (register collisions and swept
    forms), every float status word, and each drawn exclusive with its partner, run under the exact executor (lone
    pieces) and the ordinary one (full look-ahead) on one memory laid out as layer 1's (the data area, and the code
    area of 2 x 128 MiB holding `udf #0` apart from the words), a new thread per case; compared whole (stop,
    registers, vectors, FPSR and FPCR, changed data bytes) with the WebAssembly output, and the count each made with the
    counting rule. A case whose float flags are kept has its thread write FPSR first (`msr fpsr, x0`), as a program
    does. Knobs `WCHP_DIFFERENTIAL_CASES` (16), `WCHP_SEED`, `WCHP_DIFFERENTIAL_FORMS`, `WCHP_DIFFERENTIAL_JOBS` (3).
    Seed 0x2545f4914f6cdd1d at 16 cases: 1,215 forms, 8,114 edge cases, 4,528 left-out words, 96 float status words,
    384 pairs (550 storing), 65,124 runs, none failing, 68 s (load 5 to 7).
  - At 64 cases, seeds 1 to 3 (diff-seeds-arm.out), it found a bug of the process way: `br`, `blr` and `ret` through
    x30, or `blr` through a withheld register (x18, x23), to a target no piece can be at (past the reach, or
    misaligned) stopped at a host address, or left x30 unchanged after `blr` (2 to 14 of 128 runs of BR, BLR and RET
    per seed). The call into the floor is a `blr`, which overwrites x30: the call's code keeps the guest's x30 in the
    page and says so for the guest's registers, but the jump's target was still read from x30, and a fix-up giving x30
    the return address was overridden by the page's old x30 (diag-br.sh prints each lone turn's way out). Fixed in the
    process way's call (wcontainers-arm64 carry.rs, below under the translator changes); after it seeds 1 to 3 at 64
    cases pass: 251,344, 251,218 and 251,476 runs, none failing, 137 to 139 s each (xlink-arm.out, load 16 to 31).
  - The test sees faults (mutate-diff.sh, remote copy): FPCR not taken from `set_registers` fails it (every form with
    a nonzero FPCR), the stack-alignment syndrome taken as unknown fails it (LD1R and others). Two breaks it does not
    see: a data abort's alignment status ignored (the native output checks alignment itself where the form needs it,
    so the path is not reached on Graviton4), and the PC-alignment syndrome taken as unknown (the stop crate then
    answers a landing at the misaligned target, and P's load of that pc gives the same alignment fault).
  - Final verdicts from this run's tree, every test binary once with /dev/kvm denied, each first proving the device
    cannot be opened (scratch w1/r7run2/all-arm.out, all-x86.out): Graviton4 (load 17 to 74) every binary passes but
    the known precise-set test (20 of 21 in conformance); the Xeon (load 36 to 54) 50 passed, none failed (the
    translated-code binaries build no tests there). Clippy with warnings denied clean on every target on both. An
    earlier attempt failed to build for a few minutes on the kernel track's commit in progress (388f03d7), not ours.
    No new unsafe code.

- W1 round 8 run 1 (2026-11-24, 09:20 UTC on), scratch w1/r8run1, every run with /dev/kvm denied:
  - Conforming: nothing new in the executor, memory space or volatile bytes interfaces. The owner's notice of
    2026-11-24 (unsafe is priority one: no new unsafe, and any use a safe library function does the same goes) was
    followed first: ten of the library's 28 uses are gone (Unsafe code, below), 18 left (16 build on ARM64, 13 on
    x86-64), each listed under "Needs a primitive" by its operation. No use added.
  - The window cache is gone (Departures): files.rs reads and writes a stub's memfd or a shared block's by position
    (rustix's `pread`, `pwrite`, retried on EINTR, a range past the file's end `FAULT`); a fork's copy-on-write
    moves a copy unit at a time through one buffer. The code area is written by `pwrite` and mapped (memmap2's
    `MmapRaw`, ARM64 only) for cache maintenance by address; the thread pages are mapped by `MmapRaw`.
    tests/files.rs replaces tests/windows.rs: writes across huge-page edges are the stub's bytes at the guest
    address (through /proc) and read back, a write by another path is read back, three memories never mix, a read
    past the memfd's end is refused, and 64 memories of 2^43 bytes with 512 seeded touches each add no host mapping
    to the embedder (64 allowed for tests running beside it).
  - Verdicts from this tree (all1-arm.out, all1-x86.out): Graviton4 (load 26 to 39) 117 passed, 1 failed (the known
    precise-set test); the Xeon (load 45) 54 passed (the translated-code binaries build no tests there). Clippy with
    warnings denied clean on every target on both.
  - Cost, measured (busybox `sh` starting 1,000 programs, release, CPU 41, /dev/kvm denied; "before" is commit 5c049141
    with the window cache, built from a scratch tree, alternated with this tree): with every code-area write its own
    `pwrite` it was 6.29 to 6.66 s against 5.46 to 5.82 s (load 25 to 37; ab-arm.out): the embedder made about 1,680
    `pwrite`s and 470 `pread`s a program (count-arm.out, strace over 100 programs run twice), most of them 4 and 16 bytes
    into the code area (links and slots). So code-area writes are now staged (code.rs): pieces, slots and links go
    into the executor's own copy of each page written, and every page changed reaches the memfd before the memory's
    next turn (`run_turn`), a run of consecutive pages per `pwritev`, then the cache maintenance over it. After that
    (ab-bb-arm.out, 4 rounds, load 17 to 20): 5.07 to 5.34 s against the window cache's 4.76 to 4.93 s, about 7%
    slower, 4.29 to 5.01 times the model against 4.20 to 4.43; 72 `pwritev` and 67 `pwrite` a program left, and the
    470 `pread`s of guest memory (the kernel's reads, and a fork's copies). The guest's own code is not affected
    (bench's workloads in the same run, before only: geometric mean 1.348 times plain, WebAssembly 25.4).
    The kernel's reads of guest memory are the rest of the cost; reading them through a mapping is what needs a
    primitive.
  - Verdicts after staging (stage-arm.out, stage-x86.out): Graviton4 (load 22 to 44) files, memory, floor, step,
    translated, run, kernel, busybox, interrupt, copy, forged and differential pass, conformance 20 of 21 (the known
    precise-set test); the Xeon (load 14 to 32) the 54 tests of its 13 binaries pass. Clippy clean on both. A later
    build failed for some minutes on the unicorn executor's edit in progress (driver.rs, not ours), so the last A/B
    ran the binaries already built.

- W1 round 8 run 2 (2026-11-24, 10:10 UTC on), scratch w1/r8run2, every run with /dev/kvm denied:
  - Conforming: nothing new in the executor, memory space or volatile bytes interfaces since round 4; form I does not
    exist yet. No unsafe code added or touched (18 in the library, as round 8 run 1 left them).
  - Verdict at the start, shared tree at ac39cbbd (verdict1-arm.out, Graviton4, load 15 to 23): every binary passes but
    the known precise-set test (20 of 21 in conformance); clippy with warnings denied clean.
  - Where busybox `sh` goes now (perf1-arm.out, CPU 41, load 24 to 26): `load_piece` 25.9% of the samples with its
    callees; of that, a kept translation moved to another page was cloned whole and moved (`Translation::moved` 2.8%,
    the spans' clone 1.4%) and then dropped with its memory (2.7%), at every exec, since the kernel places busybox at
    a random base each time. The kernel's own exec reads the whole program file at each exec (`loader::program_of`,
    `Regular::read` through `shared_read`, 7 to 8%: the kernel track's, and the same copy under the WebAssembly
    output).
  - Built: a piece loaded at another page than its kept translation's now writes only the moved bytes (the kept
    translation's bytes with its address places written for the new address, `moved_bytes`, the same words
    `Translation::moved` writes); the loaded piece keeps the kept translation and how far it moved, and the whole
    moved translation is made only when a stop resolves a position in it (`Loaded::code`, a `OnceCell`). Exits and
    table targets are moved as they are linked. The unit test of moving now also checks, at every case that moves,
    that `moved_bytes` equals the moved translation's bytes (2,000 cases, seed 1, 829 moved with 3,966 address
    places). Measured (moved1-arm.out, busybox `sh` starting 1,000 programs, release, alternating 4 rounds, load 18
    to 30): before 4.67 to 4.97 s, after 4.32 to 4.89 s (means 4.74 and 4.49).
  - Built: one load and the pieces it loads ahead read each guest page once (a map of the pages read, local to the
    load), instead of one `pread` of the rest of the page per piece. Measured (pages1-arm.out, ab-file.sh, the same
    test, 4 rounds, load 21 to 35): before 4.46 to 4.98 s, after 4.33 to 4.82 s, lower in every round (means 4.73 and
    4.58). Profile after both (perf2-arm.out): `load_piece` 19.7%, `memory_drop` 4.1% (6.9% before).
  - busybox `sh` against the model in this run's "after" figures: 3.78 to 4.44 times (bar 1.25).
  - Verdicts after both changes (moved1-arm.out, pages1-arm.out): Graviton4 the unit tests, run, kernel, translated,
    differential (65,124 runs, none failing), forged, step, floor, busybox, interrupt, copy and memory pass,
    conformance 20 of 21 (the known precise-set test); clippy with warnings denied clean. The Xeon from the same tree
    (verdict1-x86.out, load 3 to 19): its 54 tests of 13 binaries pass (the translated-code binaries build none there).

- W1 round 9 run 1 (2026-11-24, 10:40 UTC on; the orchestrator's round 4 run 1), scratch w1/account, Graviton4:
  - Conforming: the executor suite's run moved to conformance_run.rs with its `path` line (8d1ba890), as the notice
    of 2026-11-24 asks; built and run with /dev/kvm denied: 20 passed, 1 failed (the known precise-set test);
    clippy with warnings denied clean on every target (rename-arm.out). No interface change touched the executor,
    memory space or volatile bytes since round 4. No unsafe code added or touched.
  - The measured account of busybox `sh` against the model, at the top of this file. Nothing else built.

## W1 not complete: the gap list

W1 was reported complete at round 3 run 5 and round 5 run 3 so that its independent check could run; that check
found it not met, and it is not reported complete now. These items of plan 13's bar are not met, each with what is
known:
- Speed: the getppid round trip met its bar in rounds 5 run 1 and run 3 on a quiet Graviton4 (1.213 to 1.241 times W0's
  futex slot in ten runs at load 2 to 9; bar under 1.25) but not at higher load before (1.23 to 1.31 in rounds 3 and 4,
  load 9 to 30), and the independent check measured 1.238, 1.276 and 1.249 at load 9: at the margin, not met; about five sixths of a round trip is the host's task switch (round 5 run 2's profile). Not met: busybox `sh` starting 1,000
  programs: translations move across execs (round 4 run 2) and pieces and last targets are loaded ahead (round 5 run
  1): 4.36 times the model (bar 1.25; 9.25 before), 4.29 to 5.01 since round 8 run 1 read guest memory by position
  instead of through mapped windows (Departures), 3.78 to 4.44 since round 8 run 2 loaded moved pieces by their bytes
  alone and read each page once a load (the account at the top of this file: the design's floor is about 1.4 times
  the model, so this bar cannot be met by it; about 1.5 can); per program about 1.45 ms of memory calls (about 0.6 ms of it the
  model's process creation), about 1.6 ms loading 440 pieces into the process's own code area, 59 turns. bench's matrix runs 3.8 to 4.0 times plain under P (other
  workloads 0.999 to 1.36): its inner loop's counter is the guest's x18, a withheld register kept in the register
  page (round 4 run 1); the translator's register choice is the fix. No round trip over 1 ms in 1,000,000 on an idle
  machine (run 3; one of 1.65 ms at load 30 in round 4 run 2).
- The executor suite's precise-set test fails as committed: its input domain puts code at 0x1000, below
  `vm.mmap_min_addr` (For the owner). Every other test of the suite passes, and with the code at 64 KiB this one
  passes too (run 5).
- K2 is in the kernel (KF): with the WebAssembly output's ceiling and memory size given to both, addresses, memory
  size, mappings, root files and the turns of one-thread programs equal the WebAssembly output's (round 5 run 2,
  tests/kernel.rs); hello, getppid, forks, mappings, jumptables and layout equal whole, blocks and code differ only
  where the WebAssembly side has its recorded faults and equal the program run on the host (round 6); busybox's 56
  `text` and `files` commands give the reference's output (run 3). Turns limited to one counted event change
  nothing (round 6's name). `differential` is built (round 7 run 2): every form's layer 1 cases, edges,
  left-out words, float status words and exclusive pairs equal the WebAssembly output's under both of P's look-aheads,
  with the count the rule's; met for what it compares. `count_at_every_stop` is met (round 7 run 2): bench, calls and
  jumptables equal the VM executor's returns and hash of positions.
- `Execution::restored` (the runtime's save of a process) is `unimplemented!` under P: a save panics. Not built;
  For the owner.
- x86-64: the executor, kernel and busybox suites build no tests on the Xeon (0 tests in conformance, kernel, run,
  interrupt, copy, forged, floor, translated and busybox), because they need the x86 native output's way (W3).
- Adversarial cases not built: the fork while a sibling is in a system call and another writes (needs several guest
  threads running at once, which today's kernel loop never does); one P and two I containers in one embedder is
  W2's. Built in round 4 run 2: forged exits as translated guest code after two forks (tests/forged.rs) and the
  thread past the last index refused and counted through the `Cpu` calls (tests/run.rs); the kernel's own `clone`
  answer to it is not tested here.
- Placement's two measured items are met on Graviton4 (round 5 run 1: 1.010 times the free CPU's loop after the move,
  10 moves in 10 s with every CPU contended); not measured on the Xeon, where translated code is W3's.
- The copy unit is 8 pages, measured on translated stores (round 5 run 1).
- The shared tree builds every host-process test binary again (round 4 run 2, with the conforming jobs' wasmtime and
  Unicorn edits uncommitted in it): kernel and busybox pass with /dev/kvm denied, once on Graviton4; the three rounds
  on both machines from committed code are still to run (the Xeon was not set up during round 4 run 2).
- gVisor: P runs there with known failures (plan 6; Decision 14 is with the owner); in its kernel test, blocks and
  jumptables pass the 300 s limit (which side was not found).
- x86-64: everything that needs translated code is W3's (the x86 native output's way); on the Xeon the 156 tests of
  the stub, zygote, stops, threads, memory, placement and embedder pass with /dev/kvm denied.

Met, each with its run (register isolation now with its from-outside part, round 6; the interleaving is seeded
random, not every order): the zygote's migration and no restartable sequences left (run 1); the stop landing at every
instruction of the stub's windows and handler (runs 8, 9); the control and register pages' escapes and the filter's
kills (runs 2, 3, 7); a stub killed from outside and the embedder killed (run 4); threads made and ended under
random stops (run 3); 10,000 maps of one block holding no fd (run 4); 64 memories of 2^43 bytes (run 2, through the window cache; since
round 8 run 1 read and written by position, adding no mapping); register isolation on stub pieces and on translated threads (runs 7 and round 3 run 2); the SVE probe
(run 1); Docker's default profile (runs 5, 9, 10 and round 3 run 3); form I's bounds-check bypass as a host program
(run 6); every slow path counted and warned.

## Next

- The owner's rule of 2026-11-24 comes first: add no unsafe; the 18 uses left wait for the primitives (Needs a
  primitive); when they land, each use here becomes a call of one.
- W1 is not complete (gap list). In order: busybox `sh`'s bar, the round trip's margin, the adversarial fork with a
  writing sibling, `Execution::restored`; x86-64 and form I are W3's and W2's.
- busybox `sh`'s bar: the account at the top of this file replaces what follows in this item. Next: the stub keeps
  the code area's descriptor and maps the slot table whole once (its change 2), then a fork child maps its parent's
  code area and slot table (the fork-child half of its change 1, behind a private stand-in for decision 16), each
  measured before and after, alternating. Older notes: the loads (440 a program; left after round 8 run 2:
  the key's hash, comparing the rest of the page with the kept code, writing the moved address places, placing, the
  slot; perf2-arm.out) and the kernel's read of the whole program file at each exec (7 to 8%, the kernel track's
  `loader::program_of`), and the memory calls
  (new, fork and drop beyond the model's process creation, about 0.9 ms a program), then the turns that are left (59 a
  program, of which 26 system calls and 21 faults). Sharing code areas between memories is decision 16's question
  (the owner's). A fork child copying its parent's code was measured slower (round 5 run 1). Scratch
  w1/r5run1/turns-bb.sh counts turns, landings, loads and last targets in a remote copy (its native.rs patches follow
  the code as of 9dc2f309; one failing assertion leaves the native counters at zero).
- The other bars: the round trip (1.213 to 1.27 times the slot across runs, under the bar when the machine is quiet);
  round 5 run 2's profile puts all of ours at about a sixth of a round trip, and cutting the register block's copies
  (round 5 run 3) changed nothing measurable; placement's two clock reads, `give` and `stop::resolve` are what is
  left of ours; bench's matrix (3.8 times plain) needs the withheld
  x18 renamed in hot loops (round 4 run 1, the native track's ruling first).
- Then: the gVisor kernel test's blocks and jumptables (which side hangs).
- x86-64's stub answers `Enter` as `BadTurn`; its floor and translated code are W3's (plan 3.10).
- W2's part of W1's bar, form I's bounds-check-bypass test on ARM64 (one per access shape), is measured as a host
  program (run 6); at W2 the same gadgets run as translated guest code. I's stop is a real-time signal (plan 3.6)
  and queues as P's did: W2 must bound it (send only when none is pending, or a standard signal the embedder leaves
  free).
- The adversarial fork with a sibling writing in a loop meanwhile (plan 13) needs a second thread running during a
  `Cpu` call, which today's kernel and its one-thread `with_parts` loop never do (3.9): built with parallel threads.
- In this shared worktree a `[[test]]` entry in the manifest must never land before its file exists: a missing
  test file stops every cargo command of every track (runs 2 and 3 did this once; the runtime and x86 tracks
  recorded it).
- I's reservation (W2) must leave out the zygote's pages [2^45, 2^45 + 128 KiB), or the zygote's start is refused
  ("reservation refused") in an embedder that made an in-process host first.

## How to run

- The account (scratch w1/account): each of account.sh, account2.sh and account3.sh is one `arm-run` command with
  its Python helpers sent as base64 in environment variables (DIAG, DIAG2, DIAG3, SAMPLES, IDLE; the lines used are
  in this run's outputs). The timers patch the remote copy only and are lost at the next copy.

- Since round 8 run 2: scratch w1/r8run2. check.sh is round 6's; perf-bb.sh profiles CPU 41 during busybox `sh` and
  keeps the speed test it built as speed-before; ab-file.sh (`FILE=<path> BEFORE=<base64 of its committed text>`, then
  `set -- <rounds>`) builds the speed test with that one file as committed, then as sent, and alternates busybox `sh`.

- Since round 8 run 1: scratch w1/r8run1. check.sh is round 6's (`set -- <binary>[:<filter>] ...`); before.sh builds
  the speed test of the tree sent (send the scratch tree `before`, a git repository of commit 5c049141, under `--name
  nwvm`) and keeps it as speed-before; ab-files.sh (`set -- <rounds>`) builds this tree's and alternates busybox `sh`
  and bench's workloads, ab-bb.sh busybox `sh` alone, ab-nobuild.sh the same with the binaries already built;
  count.sh counts the embedder's `pread`, `pwrite` and `pwritev` by fd under strace during busybox `sh` (100 programs).

- Since round 7 run 2: scratch w1/r7run2. `cargo test -p wcontainers-executor-host-process --test differential` (knobs
  above); diff.sh runs clippy and it with /dev/kvm denied (`EXTRA_ENV='WCHP_DIFFERENTIAL_CASES=16'` before it),
  diff-seeds.sh three seeds at `CASES`, mutate-diff.sh its breaks. p-pos.sh and vm-pos.sh print every return of
  jumptables under P's exact executor and the VM executor (send vm-pos.sh with `--name nwvmvm`; it needs KVM).
- Since round 7 run 1: scratch w1/r7run1. check.sh is round 6's; `set -- kernel:count_at_every_stop` runs the three
  count tests. vm-positions.sh runs the VM executor's kernel test with KVM (send under another name, e.g. `--name
  nwvmvm`) and prints its stops and hashes. diag-count.py patches a remote copy's native.rs to check the count at every
  lone instruction against the definition and print every exit (vm-positions and diag runs: vm1-arm.out, p*-arm.out).
- Since round 6: scratch w1/r6run1. check.sh (`set -- <binary>[:<filter>] ...`) runs clippy, then those test
  binaries with /dev/kvm denied and their results shown; interrupts.sh runs the early-interrupt test for 3 seeds at
  `CASES`, then applies `PATCH` (base64 of undo-fix.patch, the fix reversed) in the remote copy and runs it again;
  nokvm.sh is round 5 run 2's writing into /work/target/nwvm; x86.sh is clippy alone.
- Since round 5 run 3: scratch w1/r5run3. before.sh keeps a release build of tests/speed.rs as
  /work/target/nwvm/speed-before; ab-rt.sh (`set -- <rounds>`) runs clippy, builds this tree's, and alternates the
  getppid round trip of both with W0's futex slot on CPU 41 with /dev/kvm denied. nokvm.sh is round 5 run 2's.
- Since round 5 run 2: scratch w1/r5run2. kernel.sh runs clippy on every target, then the kernel test with /dev/kvm
  denied and its output shown (`FILTER=<name>` for one test); mutate-layout.sh gives form P its own ceiling in the
  remote copy, which must fail the layout test; perf.sh profiles CPU 41 during the round trip by process, binary and
  symbol. nokvm.sh is round 5 run 1's (send with `set -- <rounds>`).
- Since round 5 run 1: scratch w1/r5run1. nokvm.sh is round 4's writing into /work/target/nwvmc, run on `tree` (a
  `git archive` of a commit made a git repository, so `arm-run`/`x86-run` take it) for verdicts from committed code;
  mutate-ahead.sh runs tests/run.rs, then breaks loading ahead three ways in the remote copy, each of which must fail;
  ab-ahead.sh (and ab-copy.sh for the fork copy, no longer in the code) alternate busybox `sh` with and without a change
  (`set -- <rounds>`). The speed figures are still w1/r4run2/speed.sh; nokvm.sh ends with `exit`, so send speed.sh as
  its own job. placement.sh runs clippy, then the two placement measurements `set -- <rounds>` times.
- Since round 4 run 2: scratch w1/r4run2. nokvm.sh (run 4's) runs every binary with /dev/kvm denied; speed.sh is
  run 3's speed3.sh without the VM part, speed-bb.sh busybox `sh` alone; forged.sh runs tests/forged.rs (its
  `--nocapture` prints what each forged exit did); moves2.sh runs the moving test at 100,000 cases for three seeds,
  then with the check removed (must fail); perf-bb.sh profiles CPU 41 during busybox `sh`; timing-bb.sh times the
  memory calls, loads and placings in a remote copy; diag.sh prints every translation's pc (remote copy);
  wasm-overlay.sh makes the committed wasmtime executor follow the volatile bytes crate in a remote copy (not needed
  while the shared tree builds). The new unit test: `cargo test -p wcontainers-executor-host-process --lib` (knobs
  `WCHP_MOVED_CASES`, `WCHP_SEED`).

- Since round 3 run 5: scratch w1/r3run5. agrees.sh runs the executor suite's precise-set test with the input
  domain's code at 64 KiB (remote copy only); mutate.sh runs tests/run.rs, then undoes each of run 5's two fixes in
  the remote copy, each of which must fail it; suite-overlay.sh imports `VolatileBytes` from its crate in the remote
  copy of the suite; drop-kernel.sh leaves out the tests that need the kernel. treeA and treeB are the scratch trees
  of the commit before the volatile-bytes merge (git repositories of their own, so `arm-run` takes them as
  checkouts; run them with `--name nwvm` to reuse the build folder).

- Since round 3 run 4: tests/conformance.rs runs the executor suite on form P, tests/interrupt.rs the interrupter.
  Scratch w1/r3run4: nokvm.sh (as before, but it fails when no test binary was built); fixes.sh overlays, in the
  remote copy only, the committed suite with each `execute` polled to its end and leaves out the tests that need the
  WebAssembly executor and Unicorn; drop-unicorn.sh leaves out only busybox's; run2.sh (all binaries, KVM denied, then
  the precise-set test with the domain's code at 64 KiB), run4.sh, speed4.sh (round trip against the futex slot)
  send them inline through `arm-run`.

- Since round 3 run 3: tests/busybox.rs needs Docker on the machine (its reference), so scratch w1/r3run3/nokvm.sh
  keeps the docker group when it denies /dev/kvm (`setpriv --groups=<docker>`), and both.sh takes `CHECKOUT=<tree>`.
  The speed figures are ignored tests of tests/speed.rs (`getppid_round_trip`, `getppid_in_the_kernel`,
  `bench_workloads_plainly_under_p_and_under_web_assembly`, `busybox_sh_starting_programs`; knobs `WCHP_CALLS`,
  `WCHP_PER_BATCH`, `WCHP_ROUNDS`, `WCHP_PROGRAMS`), run in release: scratch w1/r3run3/speed3.sh (sent inline through
  `arm-run`) runs them on CPU 41 with W0's slot alternating, then the VM executor's speed test. perf.sh profiles CPU
  41 during the round trip. sandbox.sh is run 10's with the sources mounted into Docker and the busybox test left out.

- W1's tests, on either rented machine: `arm-run <checkout> --name nwvm -- 'cargo test -p
  wcontainers-executor-host-process -j 12 -- --test-threads 3'` (or `x86-run`). With /dev/kvm denied: build with
  `--no-run`, then `sudo setpriv --reuid=$(id -u) --regid=$(id -g) --clear-groups -- <test binary> --test-threads 3`
  for each test binary: scratch w1/run3/nokvm.sh does it all, each run first proving /dev/kvm cannot be opened,
  and exits 1 if any run failed (send it inline: `arm-run <checkout> --name nwvm -- "set -- 3; $(grep -v '^#!'
  nokvm.sh)"`, or `x86-run`); outputs w1/run3/arm-nokvm.out and x86-nokvm.out. Knobs: `WCHP_TURNS`, `WCHP_SEED`, `WCHP_MEMORIES`,
  `WCHP_TOUCHES`, `WCHP_STOPS`, `WCHP_ROUNDS`.
- Since run 4: scratch w1/run4/both.sh <tag> runs clippy and nokvm.sh's three rounds on both machines at once and
  fetches each job's own log as <tag>-arm.log and <tag>-x86.log, printing the counts (the streamed output can miss
  lines; the fetched log does not).
- Since run 10: scratch w1/run10/vectors.sh (sent inline through `arm-run`) runs the floor test natively, under
  gVisor and with the return route forced (remote copy patched), printing how many exits took it; gframe.c and
  xonly.c (built with gcc there, sent as base64 through `arm-run --ssh`, see gframe*-arm.out) show a host's signal
  frame and whether an execute-only mapping reads; gfloor.sh and gflood.sh run the floor test, or its stop flood
  with fewer rounds (`set -- <rounds>`), under gVisor. both.sh and sandbox-both.sh are run 9's writing into run10.
- Since run 9: tests/step.rs's `a_second_stop_*` tests and the end one's second-stop twin. Knob `WCHP_TAIL_EVERY=1`
  sends second stops in every first stop's handler, not one per way through it (a long run). Scratch
  w1/run9/both.sh and sandbox-both.sh are run 8's and run 6's, writing into run9.
- Since run 8: `cargo test -p wcontainers-executor-host-process --test step` (knob `WCHP_STEP_STRIDE`, default 1: every
  instruction; `--nocapture` prints each path's length and where its stops landed); scratch w1/run8/both.sh is run
  7's writing into run8, mutate-step.sh (sent inline through `arm-run`) breaks the stop handler three ways and runs
  tests/step.rs after each, each of which must fail. The test needs ptrace of a descendant (Yama's scope 1 allows
  it; both machines have 1).
- Since run 7: scratch w1/run7/both.sh <tag> is run 6's writing into run7; mutate.sh (sent inline through `arm-run`)
  breaks the remote copy's floor nine ways and runs tests/floor.rs after each, each of which must fail.
- Since run 6: scratch w1/run6/both.sh and sandbox-both.sh are run 4 and run 5's, writing into run6; flood.sh and repro.sh rerun the stop flood with either signal ($PIN to pin it), spec.sh runs specarm.c (form I's speculation, `specarm <rounds> <seed> [gadget]`).
- Since run 5: scratch w1/run5/sandbox-both.sh <tag> runs clippy, then sandbox.sh on both machines: every test
  binary inside Docker's default profile and under gVisor (downloads runsc 20261109.0 into
  /work/target/nwvm/gvisor, checked against its sha512), writing <tag>-arm.out and <tag>-x86.out; summary.py <log>
  prints one line per sandbox and binary. gdiag.sh and gtrace.sh run one binary or test under gVisor with its
  output or its failing host calls. unit.sh <u...> measures the copy unit (it edits `COPY_UNIT` in the remote copy).
- The fault-visibility runs: scratch keep-rseq.py and keep-rseq-x86.py patch the remote copy (at
  /work/nwvm-w1 on each machine), then the same test command; the next sync restores the tree.

## How to run the measurements

- Scratch w1/run2/: `rt.sh <cpu> <rounds>`, sent inline through `arm-run` / `x86-run --name nwvm` (it writes itself
  to /work/target/nwvm/rt.sh; see arm-rt.out for the command), alternates W0's `/work/nwvm-w0r3/slot futex 10000 21
  <cpu> <cpu> -` with the ignored test `turn_round_trip_on_one_cpu` (knobs `WCHP_CPU`, `WCHP_PER_BATCH`,
  `WCHP_BATCHES`). Outputs arm-rt.out, x86-rt.out, and the KVM-denied verdicts arm-run-nokvm.out.

- Scratch w0/run5/: `run5.sh <tag>` in /work/nwvm-w0r5 (sent as a tar through `arm-run --ssh` / `x86-run --ssh`,
  started detached); builds check5.c with gcc and writes <tag>-check5.out. `./check5 rt <ro|rw> <cpu> <work>
  <per batch> <batches>`, `./check5 alone <cpu> <work> <per batch> <batches>`, `./check5 filter`. The filter lines
  inside the round-trip section of arm5-check5.out and x865-check5.out come from a build with a wrong jump; the
  section "filter (fixed build)" at their end is the valid one. `spcost5.sh` reruns I's check campaign in
  /work/nwvm-w0r5/spcost (needs /work/nwvm-w0/bin/clang; send w0/spcost/src and ported; the fault program needs
  round2-copy too and was not built in run 5).

- Scratch w0/run4/: `run4.sh <tag>` in /work/nwvm-w0r4 (sent as a tar through `arm-run --ssh` / `x86-run --ssh`,
  started detached); builds place.c (and sve.c on ARM64) with gcc and writes <tag>-place.out. `./place fixed
  <kcpu> <gcpu> <work> <calls>`, `./place alone <cpu> <work> <calls>`, `./place rule <cpu,cpu,...> <work> <calls>`.
- Scratch w0/run3/: `run3.sh <tag>` in /work/nwvm-w0r3 on a rented machine (sent as a tar through `arm-run --ssh` /
  `x86-run --ssh`); builds slot.c with gcc and writes <tag>-slot.out. `./slot futex <per batch> <batches> <cpu_k>
  <cpu_g> <spin cpus or ->` (-1: unpinned), `./slot sigmask`, `./slot window`.
- Scratch w0/: `run2.sh <tag>` in /work/nwvm-w0 (probe.c, fallback.c; `SPCOST=1` adds the stack-guard campaign,
  ARM64 only, with the Nix store's clang 21 linked as /work/nwvm-w0/bin/clang). probe.c's futex and zygote waits
  have the lost-wakeup fault: its unpinned runs can hang; use slot.c for the futex slot.
- spcost/ is the cloud ARM64 harness (costs-cloud/arm-recheck/native) with two stack modes added to
  src/instrument2elf.py ("probe", "check") and two variants (X32-p, X32-c) in src/buildlx.sh.

## Measurements

All in plan section 11, with machine and load. Run 2 (w0/run2/; Graviton4 at load 14 to 27, Xeon 8488C at 0.05 to
0.13, both bare metal): futex slot on one CPU 2.29 to 2.37 / 1.89 to 1.96 µs, two CPUs 4.58 to 4.83 / 4.02 to
4.16 µs; zygote child 80 / 95 µs; stop signal to handler 1.68 to 1.78 / 1.75 to 1.90 µs; a fault handled in process
1.31 / 1.33 µs; code area homes in Docker as plan 3.4. Form I's check on Graviton4 (load 5.4 to 13.7, 18 pairs):
X32-e +21.8%, X32-p +21.4%, X32-c +21.7% wall, so f = 0.214. Run 3 (w0/run3/; Graviton4 at load 3.8 to 7.4, Xeon at
0.7 to 1.6): idle one CPU 2.41 to 2.46 / 1.98 to 1.99 µs (two clock reads per round trip included), two CPUs 4.6
to 4.8 / 4.2 to 4.5 µs, unpinned 4.5 to 4.9 / 4.4 to 4.9 µs with every run finished; busy one CPU median 3.7 to
3.9 / 2.7 to 3.1 µs, mean 6.0 to 7.0 / 4.7 to 5.7 µs; busy two CPUs about 2 ms on both; rt_sigprocmask 125 /
87 to 89 ns; 2 MiB window map, touch, unmap 8.5 to 8.6 / 4.8 to 5.2 µs. Crossover with one-CPU placement:
90,000 to 94,000 kernel entries per second on Graviton4 idle. Run 3's busy crossover (31,000 to 36,000) is
withdrawn: it left out that I's work slows on a busy CPU too. Run 4 (w0/run4/; 2026-11-23 11:52 UTC, Graviton4 at
load 7.4 to 8.2, Xeon at 2.7 to 10.4): with 10.8 µs of work between calls c = 2.60 to 2.63 µs on a free CPU
(Graviton4), c_busy 6.11 to 6.23 µs beyond the work alone on a CPU shared with a spinner (work alone slowed 2.00
times); Xeon c 2.9 to 4.5 µs (clock speed varies), c_busy 4.3 to 5.1 µs; CPU share 1.000 free, 0.505 to 0.559
contended; the placement rule's one move 21 to 22 / 118 to 137 µs, then 13.37 to 13.39 / 18.10 to 18.29 µs per
call; SVE p0 and FFR zero after `getppid`. Crossover with work between calls 81,000 to 82,000 (Graviton4, free),
69,000 to 70,000 (every CPU contended); Xeon 25,000 to 52,000 and 38,000 to 66,000. Run 1's outputs and run 2's
unpinned "did not finish" runs give no figure. Run 5 (w0/run5/; 2026-11-23 12:07 to 12:20 UTC, Graviton4 at load
16 to 21, then 5.8 to 6.7 for the campaign, Xeon at 0.1 to 0.4): the stub's answer word on a read-only page, back
to back 2.36 to 2.40 against 2.33 to 2.36 µs read-write (Graviton4), 1.93 to 1.98 against 1.88 to 1.94 µs (Xeon);
with work between calls c = 2.50 to 2.53 / 2.02 to 2.18 µs, no read-only difference; the filter's two-argument
`mmap` rule works on both; I's check X32-p +21.1% wall, X32-e +21.8% (3 rounds of 6 pairs, 9 programs, outputs equal
the plain build's). Crossover with work between calls now 81,000 to 86,000 (Graviton4), 50,000 to 75,000 (Xeon,
idle).

W1 run 2 (2026-11-23 13:38 to 13:39 UTC, w1/run2/): the stub's turn with no commands, embedder thread and stub
pinned to CPU 41, alternating with W0's slot in the same run, three rounds of 21 batches of 10,000, median of batch
means: Graviton4 2,703 to 2,718 ns against the slot's 2,354 to 2,365 ns (load 7.2), Xeon 2,235 to 2,253 ns against
2,019 to 2,040 ns (load 0.3); 1.15 and 1.10 times the slot. (The slot's figure includes two clock reads per round
trip; the turn's does not.) This is the transport alone; the bar's figure is the guest's getppid, which needs
translated code.

W1 run 1 (2026-11-23 12:55 to 13:20 UTC): no timings yet, only the probe's values and the test verdicts above
(Graviton4 at load 13 to 36 while other tracks ran, Xeon at 0.1 to 0.6).

## Changes to the translator and the stop crate (for the native, translator and x86 native output tracks)

Made by this track on 2026-11-23 with the orchestrator's leave; each commit kept the ARM64 translator suites and
the VM executor's tests passing on Graviton4 (verdicts under round 3 run 1 above).
- ASKED, not made (round 4 run 1): a guest loop that names a withheld register (x18, x23 or x28) keeps it in the
  register page, a load and a store per use; in bench's matrix the innermost loop's counter is x18 and the loop runs 4
  times plain under P (both ways withhold the same registers, so the VM form pays it in such loops too). Proposed:
  rename a withheld register a hot piece names to a host register the piece leaves free, at the piece's entry and
  exits. For the native track to rule, since it changes the machine way as well.
- 89a6818e: `Native::Way` for ARM64 is `wcontainers_arm64::Way` (`Machine`, the default, as before; `Process`).
  `translate_native` keeps making the machine way's code. `Holds::EntrySlot` (crates/wcontainers-guest-cpu) carries
  its slot table's base, so `write_place` needs no way; x86 never uses it. `RegisterPage` has a `link` word
  between `saved` and the exclusive monitor (`word` reads it). `encode::form::SVC` is new. The VM executor's tests
  name `Way::Machine` where they called `route(&(), ..)`, and the runner's page sets `link: 0`; nothing else of
  theirs changed.
- 718e5eb3: crates/executor/implementation/arm64-stop (package wcontainers-executor-arm64-stop, no_std) holds what
  hardware-virtualization-inside/stop.rs held, with `resolve(way, piece, taken, hardware, page, float, memory)`
  and `interrupted(way, ..)`; `Taken` says what the exception was. The inside's `stop` module re-exports it and keeps
  `resolve(piece, exception, ..)` and `interrupted(piece, ..)` as before, reading the syndrome into `Taken`; its
  callers did not change. It depends on the abi, arm64 (without translate) and guest-cpu only.
- Round 3 run 2: host-process loads pieces itself (native.rs), as the VM inside's native.rs does, since P's
  translator runs in the embedder and needs no link. In the process way a call's `blr` is out of line, so the
  stop crate's `Resolved::LastTarget { call }` and `Resolved::CodeChanged { then }` name the out-of-line `blr` and
  the `ldr` after it; host-process takes them to the call's word in line. If the stop crate is to answer in line
  itself it needs the piece's host words; not changed there.
- Still asked of the native (VM) track, not done here: `translation` and the loading of pieces with their slots
  (hardware-virtualization-inside/native.rs) and the monitor's kept translations (hardware-virtualization/
  translate.rs) usable from a crate without the VM's dependencies, so P loads pieces the same way.
- Round 3 run 4, found by the executor suite's precise-set test (with the input domain's code moved to 64 KiB),
  not fixed: in the process way a store that may cross a page is probed with a load first, so a store to a page that
  allows no access faults as a read (the syndrome's WnR clear); Linux reports a write. Example: `str x1, [x0]` with
  x0 = 0x11ffc and 0x11000 unmapped: expected ESR 0x92000047, P gives 0x92000007 (180 of 1,728 such states; `stp`
  3,474 of 8,640). The probe would need to fault as a store, or the stop crate to take the access from the guest
  instruction. Also `stp` with its base near the top of the address space reaches the process way's misalignment
  check, which still calls by `brk #5`; P now takes that as a call, but the stop crate finds no access operand there
  and P ends the process ("stub exit not understood"), so those states fail too.
- Round 3 run 5, fixed in host-process only (the stop crate is unchanged): the precise-set gaps above. For the VM
  form: the probe in front of a store that may cross a page is an atomic add of zero, whose syndrome has WnR clear
  where a read would fault too, so the VM inside should give the same answer for the same states (a store faulting
  as a write whatever the probe's syndrome says), unless it already takes write from the guest instruction; the
  machine way's `brk` syndrome comes from the hardware, so the second fix (P reading a SIGTRAP's call from the loaded
  word) does not concern it.
- 9d7c1d79 (W1 round 7 run 2), crates/wcontainers-arm64/src/carry.rs, the process way only: its call into the floor
  (`process_call`) now also moves a resume held in x30 (`Resume::Through` or `Slot` of `Held::Register(30)`) to the
  page's `link` word after storing x30 there, and does not relocate the guest's x30 to that word where the span's
  fix-ups give x30 its whole value (`Constant`, `Address` or `Move`), so the fix-up's value stands. Found by
  host-process's differential test (above). Verified on Graviton4 (xlink-arm.out, xlink-vm-arm.out): the arm64,
  translator, guest-cpu and stop crate tests pass; the VM executor's exits, native (32,562 cases, 0 failing), floor,
  registers, host_branches, interrupts and probe tests pass with KVM; host-process's differential, floor, step,
  translated, run and kernel tests pass with /dev/kvm denied. The machine way is unchanged (its call is one `brk`).
- For the translator track, not done: the InProcess way (plan 3.2 item 1 and two more withheld registers), W2's.
- Kernel: K2 (placement low first, memory size at start) as docs/native.md asks, now W1's need too: until it
  lands W1's bar is equal verdicts, and `differential` and `count_at_every_stop` wait on it by name; several threads running at once
  for `ThreadOrdering::Parallel`; calling the stopper (plan 8 item 3) from `suspend_tasks`, `suspend_threads`,
  signals to a running thread and group stops, if the owner takes it; `ProcessInfo::kernel_entries` if he takes it.
- Every executor track, if the owner takes plan 8 item 3: a `ThreadStopper` of its own.
- x86 track: the x86-64 native output with its `Way` (XN1, XN2), which plan 3.10 takes as it stands.

## For the owner

- Decision 10 under P (round 7 run 1): a turn limited to a count ends exactly only under `HostProcessExecutor::exact`,
  a stand-in constructor for the embedder's choice (the VM inside's `exact_replay` is its twin), which runs every guest
  instruction as a turn. The ordinary executor ends such a turn
  past its count, counted "turn limit passed", as before. Whether the choice is the embedder's, the policy's or a
  translated compare at counted events (option B) is decision 10.
- `Execution::restored` (runtime single-kernel: a new executor holding the same memories, for a copy of a process
  made at a save) is `unimplemented!` under P: a save panics. P's memories are stub processes held by one executor;
  a restored executor would share the stubs and code areas through the host. Not built in W1.
- Interface changes, as code in plan section 8:
  - `Executor::runner_handle` with `Executor::run_thread`. Today `KernelRunner::run` is `unimplemented!` for want
    of it, for every executor. The alternative is a handle type in `Runner::run`.
  - `ProcessInfo::kernel_entries`, the evaluator's input.
  - `Executor::Stopper` with `ThreadStopper::stop(thread)`: settled for one execution by the owner's
    `ExecutionInterrupter` (167cffce), which P implements (`HostProcessInterrupter`).
  - `StandaloneExecutorHost`: the process-wide part both executors need, made once, making one executor per
    container.
    Built meanwhile as the private stand-in `HostProcessHost` (crates/executor/implementation/host-process/host.rs),
    the one place his ruling changes.
- The executor interface as of b01ba0d8, found while implementing it for form P (round 3 run 4):
  - No constructor: `HostProcess::new(Arc<HostProcessHost>)` stands in (as the WebAssembly executor's `new` does);
    a `StandaloneExecutor` trait would settle it.
  - `MemorySpace::zero`, `set_permissions` and `unmap_from` can only fail with an invalid address, yet P may need
    one more host mapping for any of them and the host caps them (`vm.max_map_count`). Example: setting alternate
    pages read-only then read-write one page at a time past 1,048,576 changes (the limit on both machines). P panics
    there ("the host refused a change to a guest memory"); an out-of-memory error, as `map_from` and `copy_from`
    have, would let the kernel answer ENOMEM as Linux does.
  - `execute` has no ending for "the guest's process is gone" (a stub killed from outside, or one that ignored its
    stop for 10 s). P returns the state with an SError exception; a third ending, or an error, would say it plainly.
  - No clock is given to `execute`: a guest's counter read under P is the host's monotonic clock, not the system's.
  - The execution future: P's runs to its end at its first poll, since its guest runs in another process and the
    interface gives no way to wake a waiting future from that process's answer.
  - A memory belongs to the executor that made it (its stubs are that executor's); using it with another panics. The
    interface does not say whether that is allowed.
  - Views of guest memory (the owner's ruling of 2026-11-23): P adds none. `HostProcessBytes::read_to` and
    `write_from` move the bytes through a heap buffer of their whole length (one more copy and one allocation per
    call). The view that would remove it is a slice of the stub's memfd where the window cache maps it in the
    embedder, in windows.rs under one more `#[expect(unsafe_code)]`:
    `core::slice::from_raw_parts(at as *const u8, length)` (the span `read` copies from today) handed to
    `ByteSink::write` (and `from_raw_parts_mut` to `ByteSource::read`), valid while the cache's lock keeps the window
    mapped. It is not sound as Rust: the guest's other threads may write those bytes meanwhile, and a `&[u8]` promises
    they do not change. It would also split bytes that cross a 2 MiB window into several writes, which the contract's
    one write forbids. So the buffer stays unless the sink and source take volatile access.
- The executor conformance suite runs on form P and passes but for one test (round 4 run 1: 20 of 21), which cannot
  pass on it as it stands (plan 8): its one state generator,
  wcontainers-instruction-input-domain-arm64, places every state's instruction at 0x1000 (`CODE_ADDRESS`) and its
  data pages at 0x2000 to 0x4000, below the host's `vm.mmap_min_addr` (32,768 on Graviton4, 65,536 on the Xeon), so
  an unprivileged host process cannot map them; P runs guest addresses as host addresses. Example: the first state
  of `AddShiftedRegister64` has pc 0x1000. A generator whose pages lie at or above 64 KiB, or one the executor's
  address space places, would let every executor run it.
- Decisions in plan section 12:
  - names;
  - I's wrapping check: about 10 points faster, but an access outside the window lands in the process's own
    memory where Linux faults;
  - P's guest can read and write its stub's own pages where Linux faults;
  - PAuth and MTE not offered under P and I;
  - a clock the system publishes;
  - changing the form at an exec;
  - per-thread watches under P, by a process per guest thread;
  - I's runner threads holding off the embedder's SIGCANCEL and SIGSETXID while they run;
  - I's 4 GiB per guest process, against larger windows at one more instruction per access;
  - I's guests sharing one process with no line between them against branch target injection and cache timing;
  - P's defence in depth behind translation: an escaped stub can write pages it lends to fork children before they
    copy them, and shared blocks; close it with a sealed file per fork, or accept;
  - hosts with pages larger than 4 KiB: the ruling of 2026-11-18 followed by split host pages, built at W4 after
    both forms work on 4 KiB hosts;
  - P under gVisor on ARM64 (run 10): stops and faults return through a signal frame that the guest's other
    threads can write, so a racing sibling can escape translation into its stub (bounded by the filter and gVisor);
    accept there, or make P unavailable on such a host.

- Filled in by W1 run 1, where the plan was silent (the fourth check's minor point): when the zygote itself ends,
  every P guest process of that embedder ends as killed by SIGKILL (its stubs die with it), counted "zygote
  ended", and a new zygote is forked from the embedder on the next `memory_new`; if that fork fails, P is
  unavailable from then on. The alternative was to fail every later P container of the process. Plan 4.1.

## Departures from the plan

- W1 round 8 run 1: the window cache of plan 4.2 is gone. The embedder reads and writes the stubs' memfds by position
  (files.rs, rustix's `pread` and `pwrite`) and maps no guest memory; the code area is written by `pwrite` and mapped
  only for its cache maintenance. Five of the crate's unsafe uses went with the cache and five more with memmap2
  (the owner's rule of 2026-11-24). Plan 4.2 says so now; tests/windows.rs is tests/files.rs (the same checks less
  the cache's bound, plus a read past a memfd's end refused, and 64 memories of 2^43 bytes adding no mapping).

- W1 round 7 run 2: tests/differential.rs carries copies of the VM executor's test helpers that draw the cases
  (`edge`, `across`, `collisions`, `swept`, `paired` and the WebAssembly side of a pair), because they live in that
  crate's test files; the arm64 crate's `words_of` is included by path as the VM's test does. One shared home for them
  (the translator test crate) would be the clean form; that crate is not this track's.

- W1 round 5 run 2 (commit 944f8917) changed plan 13's W1 bar without recording it here: the two tests the fourth
  check listed as waiting on K2 (`differential`, `count_at_every_stop`) were written up as "not built" with a
  replacement that did not stop at counted events. Round 6 restores them as unmet items of the bar (plan 13, gap
  list) and changes one thing in it, recorded here: `count_at_every_stop` compares with the VM executor's count,
  not the WebAssembly output's, since only the VM executor counts the same quantity (the native output's count
  definition); the WebAssembly output ends a limited turn at its own stop checks.
- Plan 13's bar says a stub killed from outside ends its process as SIGKILL within 100 ms of its death; the tests
  (tests/stub.rs, tests/embedder.rs) allow 150 ms, because the runner looks whether the stub ended every 100 ms
  (`LOOK_FOR_END`, the same interval as the stop's resend) and then needs to be scheduled. Meeting 100 ms exactly
  needs a shorter look interval (one more wake per long turn); not changed.
- Plan 3.8's floor assertion at the moment a thread is about to run is built as a test that reads the registers from
  outside at that moment (tests/step.rs, round 6), with no test build of the floor; the interleaving of the
  read-back tests is seeded random (4,000 rounds over 8 threads), not every order of a bounded search.

- W1 round 3 run 5: `HostProcessBytes::read_to` and `write_from` copy through a buffer as long as the bytes (one
  allocation per call), since a borrowed range of a stub's memory can span windows and shared blocks; lending the
  window's own mapping where the range lies in one window is the faster form, not built. W1 is reported complete
  with the gap list above instead of with every bar met, as the orchestrator asked for this run.

- W1 round 3 run 3: the busybox test (and the kernel test, now) runs with /dev/kvm denied but the docker group kept,
  since its reference runs in Docker; the earlier test binaries' runs cleared every group. The executor suite is not
  run (For the owner). The round trip's bar is measured with each call answered by the test's own loop, not by the
  kernel (the kernel's own share is measured beside it as `getppid_in_the_kernel`).

- W1 round 3 run 2: slot pages are pages of the code area's memfd past the code area, given out in order and never
  reused, not a memfd of their own: the embedder writes them through the code area's own mapping (and its cache
  maintenance), and a stub's mapping of a slot page can never show another page's slots. A counted turn is not
  stopped at its exact count (plan 4.6 assumed the VM form's stepping, which a host process has no way to do): it
  ends at the first exit past the count or by a slice, counted. Translations are kept in the executor by address
  and bytes (the process way's code depends on nothing else), so a fork child loads its parent's without
  translating again. Code a program writes is caught as plan 3.4 says for private pages; a shared block's page
  written by another process is not yet write-protected there (only the announcement and `code_changed` drop it).

- W1 round 3 run 1: the ways are named `Machine` and `Process`, as the x86 native output names its own, not `Vm`
  and `HostProcess`. A process-way call is one in-line branch to out-of-line code with an absolute `blr` to the
  entry, not `str x30; bl entry`: a `bl` would need relocating wherever a piece is loaded, and one in-line word
  keeps a jump's last-target layout the same in every way (plan 3.2 says so now). Entries keep x23 first (16 bytes
  each), since a last-target call leaves its target there. No replay check exists in any way yet, so item 4 of
  plan 3.2 changed nothing.

- W1 run 8: the stops at every instruction are forced by tracing the thread from the test and stepping it there
  (exhaustive and replayable, every instruction in turn), not by a seeded delay, which cannot aim at one instruction.

- W1 run 7: the code area is mapped by a command after the stub's start (received like a shared block), not at its
  start, so a stub that never runs code maps none; it is execute-only, one per stub, just under 128 MiB, beside the
  zygote's code where the floor's entries are. The register block in the register page is the hardware's state
  (x23 and x28 the floor's), not the guest's, so the kernel's side will read guest registers through resolution.

- W1 run 6: P's stop is `SIGUSR1`, a standard signal, not the kernel's first real-time signal, so stops coalesce
  (plan 3.6 and 3.7 say so now). Form I's speculation test was run ahead of W2 as a host program of I's own
  sequences, since the brief makes it part of W1's bar (plan 13 lists it now).

- W1 run 5: the up-front mapping rule leaves a margin of two splits per command, so at the limit it also refuses a
  change that would only merge (Linux would make it); the probe reads /proc/sys/vm/max_map_count, the one /proc
  read (plan 6 says so now). Under gVisor a fork lends the whole guest area, since the host does not say which
  pages hold data.

- W1 run 4: the runner is whichever thread calls the executor, pinned from its first turn for the executor's
  life (the plan's runner threads come with 4.6's loop); a guest thread's move is a command of its own turn, so a
  turn that moves pays the two mask calls of a turn with commands. A slice's stop waits for the thread to take its
  turn (plan 3.6 says so now).
- W1 run 4: an owner that ends while it lends pages keeps its memfd (its stub ended and reaped) until no page is
  borrowed from it, instead of copying each into a borrower: nothing is copied, as on Linux (plan 3.5 says so now).
  A `Cpu::read` of a borrowed page reads the owner's instead of copying it. Views may be stricter than the rules
  allow and are put right at the executor's own faults; a fork protects every writable range of the parent, not
  only its lent pages (fewer commands, one fault more on a page the parent writes that was not lent). A
  `thread_new` past a memory's spare is its own round trip, not a command riding on the next resume. A fork
  refused part-way (a command the host refuses) drops the child and answers `OutOfMemory`.

- W1 run 3: P's stop signal is the kernel's first real-time signal, fixed, not chosen from the embedder's
  dispositions: it is only ever sent to a stub, which holds no C library (plan 3.6 and 3.7 say so now; I's choice is
  unchanged). Both architectures name our own restorer (`SA_RESTORER`), since the stub has no vDSO. A thread's
  exit-clear word is also set by `CLONE_PARENT_SETTID`, so an index is never free between the command and the new
  thread's start. A stop in the stub's own code outside the named windows (resume dispatch, the spin) ends the turn
  as `Stopped`; a malformed turn reports `BadTurn` with the stop blocked throughout.

- W1 run 2: the window cache evicts by the clock algorithm, an approximation of least recently used whose touch is
  one store (plan 4.2 says so now); a stub's start is a turn the embedder waits for (plan 4.1 says so now); the
  filter allows only the calls the stub's routines make so far, and grows with them.
- W1 run 2: until the stop handler is built, a stub keeps the zygote's mask (every signal blocked); SIGKILL and the
  filter's SIGSYS end it regardless.

- The zygote is forked with every signal already blocked by the forking thread (and its mask restored after),
  rather than blocking them as its first act, so no handler of the embedder can run in the child (plan 4.1 says
  so now).

- The brief's "Linux hosts on ARM64 and x86-64 first" is built in two steps: ARM64 at W1 and W2, x86-64 at W3,
  because x86's native output (the x86 track's XN1, XN2) does not exist yet and both forms run only translated code.
  x86's side of both forms is decided now (plan 3.10, from docs/x86-native-output.md).
- The owner's 2026-11-18 ruling that a back end gives the guest 4 KiB pages on whatever the host has is followed
  (plan section 6, split host pages), but built last, at W4: until then both forms refuse to start on a host whose
  pages are larger than 4 KiB (Apple silicon, 64 KiB-page ARM64 kernels), named. Cloud ARM64 Linux machines run
  4 KiB pages by default, so W1 to W3 cover the case the ruling of 2026-11-23 is about first.

## Needs a primitive

The owner's rule of 2026-11-24 (no new unsafe; unsafe only in low-level primitives behind traits of their own). Each
operation this crate still does with unsafe code, for the primitives' design
(pyro-parked/wcontainers-scratch/executor-unsafe-primitives; its survey-process.md lists them line by line as of
37f1b84b, before round 8 run 1 removed ten):
- Map at a fixed address with `MAP_FIXED_NOREPLACE` and unmap on drop: the zygote's reservation, the probe's one-page
  search of the user-space top.
- Views of a shared mapping as atomics of one size per offset: the thread pages (`&AtomicU32`, `&AtomicU64` at an
  offset; a `#[repr(C)]` page of atomics would make one size per field certain).
- Cache maintenance after writing code, by address range of a mapping (`dc cvau`, `dsb ish`, `ic ivau`, `dsb ish`).
- Read a system register: `CTR_EL0`.
- Send a signal to one thread of another process: `tgkill(pid, tid, signal)`.
- Fork a multithreaded process whose child only jumps into code of its own: `fork`, then the jump with five
  arguments in registers, never returning; the zygote's assembly and the slice of its machine code between two labels.
- glibc's `__rseq_offset` and `__rseq_size`.
- The SVE probe: set p0 and FFR, make a system call, store both.
Not needed by anything built: a view of guest bytes lent to a byte sink or source (For the owner, views of guest
memory); `read_to` and `write_from` keep their buffer.

## Unsafe code

The crate denies `unsafe_code`; each use carries its own `#[expect(unsafe_code, reason)]`. Since W1 round 8 run 1:
18 in the library (16 build on ARM64, 13 on x86-64), 1 in its tests; round 8 run 1 removed 10 (the owner's rule of
2026-11-24: no new unsafe, and any a safe library function does the same goes): the window cache's five (its
reservation, its window maps and its two copies; the embedder now reads and writes the stubs' memfds with rustix's
safe `pread` and `pwrite`), the code area's map, unmap and slice (memmap2's safe `MmapRaw`, words written by
`pwrite`), the thread pages' map and unmap (`MmapRaw`). Left:
- code.rs: cache maintenance by address and its barriers (`dc cvau`, `ic ivau`, `dsb ish`; no crate wraps them as
  safe calls), ARM64 only, one marked function.
- stub.rs: glibc's `tgkill` for the stop (rustix's `tkill` is unsafe too and reaches any thread of the host; nix has
  none for one thread of another process; rustix's safe `pidfd_send_signal` on a thread's pidfd would do it, but
  `PIDFD_THREAD` needs Linux 6.9 and is not known in gVisor).
- zygote.rs: the zygote's assembly, one module per architecture (it runs with none of the embedder's memory, so
  it refers to nothing outside itself); glibc's `__rseq_offset` and `__rseq_size` (no crate has them); the two
  labels around the assembly; the slice of its code between them; the reservation's `mmap` and `munmap`;
  `fork` (the signal mask around it is set through nix's safe `pthread_sigmask`);
  the jump into the zygote's code in the child, one per architecture.
- probe.rs: `mrs ctr_el0` (no crate wraps it); the fixed `mmap` and `munmap` of the user-space search; the SVE
  probe's assembly (set p0 and FFR, `svc`, store them).
- pages.rs: the two views of the thread pages' mapping as 32-bit and 64-bit atomics (the stub writes the same pages
  from its own process).
- tests/zygote.rs: glibc's `__rseq_size`, to show the test has a registration to drop.
- The stub's routine that receives a shared block's memfd and maps it is in the same assembly (no new use); it
  reads the control message the host kernel wrote on its own stack, checks its length, level and type, and maps
  only what the filter allows.
- The floor's entries, its save and the enter (run 7) are in the same assembly (no new use); the save from a signal
  frame reads the frame's general registers and its `fpsimd_context` (tag and size checked) on the thread's own
  signal stack, and writes them only to the register page that x28 names after checking it is one.
- The stub's handlers live in the zygote's assembly (no new use): they read the interrupted pc, and for a fault the
  signal's code and address, from the frame on the thread's own signal stack, which a guest thread of the same stub
  could write; that reading only chooses among our own targets or is reported to the kernel as the guest's fault.
- The return route for frames without FP/SIMD registers (run 10) is in the same assembly (no new use): it writes
  the general registers, sp, pc, pstate and mask of the frame at x2 on the thread's own signal stack and returns
  through it; plan 12 item
  14 is the one way it is weaker than the rest.
Constants no crate defines, named with their source: glibc's rseq signatures (`RSEQ_SIG`), Linux's
`RSEQ_FLAG_UNREGISTER` and the original 32-byte `struct rseq`, `CTR_EL0`'s fields, Linux's `struct fpsimd_context`
and `FPSIMD_MAGIC` (arch/arm64/include/uapi/asm/sigcontext.h); in tests/floor.rs the `msr` encodings of NZCV,
FPSR, FPCR and `TPIDR_EL0` (Arm ARM).

## Dependencies and licences

Since W1 round 8 run 1: memmap2 0.9.12 (MIT OR Apache-2.0, already in the lock file) for the thread pages' and the
code area's mappings in the embedder, made and unmapped by its safe `MmapRaw`.

Since round 7 run 2, for the differential test only (ARM64): proptest 1.12.0 (MIT OR Apache-2.0, already in the lock
file, the VM executor's tests' version) to draw layer 1's cases.

Since round 7 run 1, ARM64 only: rustc-hash 2.1.3 (Apache-2.0 OR MIT, already in the lock file, the VM inside's
version) for the hash of positions, so it equals the VM executor's.

Since round 3 run 5, ARM64 only: wcontainers-volatile-bytes, wcontainers-byte-sink and wcontainers-byte-source, all
this workspace's (the owner's interfaces the memory space's bytes implement).

Since round 3 run 3, for the busybox and speed tests only (ARM64): wcontainers-executor-unicorn (this workspace's:
the reference image, the busybox command lists and the Docker reference), which brings unicorn-engine 2.1.5
(GPL-2.0) as a test-only dependency, as the VM executor's tests already do; never a dependency of the library.

Since round 3 run 2, ARM64 only: wcontainers-translator (`native`), wcontainers-guest-cpu,
wcontainers-executor-arm64-stop and wcontainers-runtime-single-kernel (without `std`), all this workspace's; for the
kernel test only, this workspace's kernel, seeded policy, system, clock, random source, readiness, wasmtime executor
and translator test crates, and zig's C compiler from the Nix shell as a tool (the VM form's tests use it alike).

host-process, Linux only: nix 0.31.4 (MIT; `process` and `time` only) for a stub's process CPU clock, which rustix
lacks for another process; rangemap 1.8.0 (MIT OR Apache-2.0) for permissions, page states and shared ranges by
range, zerocopy 0.8.63 (BSD-2-Clause OR Apache-2.0 OR MIT) for a register block as bytes (both already in the
lock file), rustix 1.1.6 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT), libc 0.2.191 (MIT OR
Apache-2.0) for the signal mask, `fork`, `tgkill`, the layouts of a signal frame and the wait-status macros, linux-raw-sys 0.12.1 (Apache-2.0 WITH
LLVM-exception OR Apache-2.0 OR MIT) for Linux's numbers in the assembly (since run 5 also its `errno` feature, for
ENOSYS), wcontainers-abi; for its tests only, fastrand 2.5.0
(Apache-2.0 OR MIT), nix's `ptrace` and `signal` features (since run 8, the step test's tracing), on x86-64 since
run 8 iced-x86 1.22.0 (MIT; the step test's decoding of the instruction at a pc) and, on ARM64 since run 7,
wcontainers-arm64 (this workspace's, for the floor test's pieces and since run 8 the step test's decoding). All were already in the lock file. Measurement tools only, never dependencies: gcc 13.3 and clang 21 on the rented
machines; gVisor's runsc 20261109.0 (Apache-2.0) and the busybox image (GPL-2.0, run only as a container base) for the
sandbox runs.
