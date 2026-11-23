# Native without a virtual machine: status

Plan: docs/native-without-virtual-machine.md. Branch: agents (shared worktree). Scratch:
pyro-parked/wcontainers-scratch/native-without-virtual-machine.

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
  - Fixed (plan 3.6): where the record is missing, the handler saves the general registers from the frame, then
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
    thread waits. Left failing there; a 12,000-round run is below if it finished.
  - Final verdicts after the last code commit, each test binary 3 times with /dev/kvm denied, clippy with warnings
    denied clean on both: Graviton4 (scratch w1/run10/final-arm.log, load 15) 180 of 180; Xeon (19:42 UTC,
    final-x86.log, load 5.9 to 2.8) 156 of 156 (the x86 machine did not answer SSH at 19:40).
  - No new unsafe code (the route is in the zygote's assembly).
- Not built yet (W1's bar is not met): everything that runs translated guest code. The translator's `HostProcess` form and the
  `stop` crate with the slot table's base as a parameter do not exist (asked below), so no executor suite, kernel
  test program, register-isolation test or speed figure can run yet.

## Next

- x86-64's stub answers `Enter` as `BadTurn`; its floor is W3's (plan 3.10).
- When the translator's `HostProcess` form lands: match the stand-ins to it (x28's page is the stub's register
  page and the block the hardware's state here; the translator's `RegisterPage` and its saved link must sit where
  x28 points, and its call codes must be the entries' numbers, 64 today), then resolution through `stop`, the slot
  table's page-by-page mapping (3.1, a command the filter allows in [2^46, 2^47)), and `Cpu::registers` from the
  resolved guest state instead of the block.
- W1, in this order, each tested on both rented machines with /dev/kvm denied: placement's remaining bar items
  (a guest's getppid loop with work between calls within 5% of a free CPU's after its move, and at most one move a
  second with every CPU contended), which need translated code to be the guest; the copy unit measured again on
  translated code (`copy_unit_cost` with real stores), then set. Hand-written pieces stand in for translated code
  until the translator's form exists. Then the `Executor` trait with the `with_parts` loop (4.6), the suites (also
  inside Docker's default profile and under gVisor, as run 5's sandbox.sh does), the register-isolation tests and
  the speed figures.
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

## Asked of other tracks

- For the orchestrator (asked in run 8, still open in run 9): the two tracks W1 waits on look stopped. docs/status-translator.md was last changed
  on 2026-11-20 and docs/status-native.md on 2026-11-21, and no commit has touched the translator, the ARM64 crate
  or the VM inside's `stop` since run 6. Either start them on the two asks below, or let this track write the ARM64
  `HostProcess` form itself (the four places of plan 3.2, in crates/wcontainers-arm64 and the translator) and the
  `stop` crate's split. Until one of those happens, nothing in W1 that runs guest code can be built. The x86 native
  output has its way P already (crates/wcontainers-x86, `Way::Process`); its executor side is W3's (plan 3.10).
- Translator and ARM64 (crates/wcontainers-translator, crates/wcontainers-arm64): a form value given to
  `translate_native` and part of the cache key, with variants `Vm`, `HostProcess`, `InProcess`, that changes four
  places only (plan 3.2):
  - `carry.rs` `access` and the instructions that read or write sp. InProcess emits `ubfx u, a, #32, #24; cbnz u,
    fault` and then the access `[base, wA, uxtw]` (or `add t, base, wA, uxtw` first, for an immediate or a pair),
    plus the stack pointer's rules (a) to (f), including one-instruction pieces with sp in the register block.
  - The jump through a register: the slot table at 2^46 instead of `mirror::MIRROR`, reach 2^44, and InProcess's
    `add t, base, wN, uxtw`.
  - `Arm64::call`: `str x30, [page, #saved_link]; bl entry_<code>` instead of `brk`.
  - The replay check: its stop word from the register page instead of `TPIDRRO_EL0`.
  InProcess also needs two more withheld registers.
- Native (VM) track: `stop::resolve`, `stop::guest_state` and the slot arithmetic in
  crates/executor/implementation/hardware-virtualization-inside/stop.rs (which uses `mirror::MIRROR` at lines 10
  and 296) taking the slot table's base as a parameter, and `stop` and `translation` in a crate that does not pull
  aarch64-paging, arm-gic, smccc, the kernel or the runtime; the monitor's kept translations (`Translator`,
  hardware-virtualization/translate.rs) usable from such a crate.
- Kernel: K2 (placement low first, memory size at start) as docs/native.md asks, now W1's need too: until it
  lands W1's bar is equal verdicts, and `differential` and `count_at_every_stop` wait on it by name; several threads running at once
  for `ThreadOrdering::Parallel`; calling the stopper (plan 8 item 3) from `suspend_tasks`, `suspend_threads`,
  signals to a running thread and group stops, if the owner takes it; `ProcessInfo::kernel_entries` if he takes it.
- Every executor track, if the owner takes plan 8 item 3: a `ThreadStopper` of its own.
- x86 track: the x86-64 native output with its `Way` (XN1, XN2), which plan 3.10 takes as it stands.

## For the owner

- Interface changes, as code in plan section 8:
  - `Executor::runner_handle` with `Executor::run_thread`. Today `KernelRunner::run` is `unimplemented!` for want
    of it, for every executor. The alternative is a handle type in `Runner::run`.
  - `ProcessInfo::kernel_entries`, the evaluator's input.
  - `Executor::Stopper` with `ThreadStopper::stop(thread)`. Without it nothing can stop a thread that is running
    on another runner; `Interrupter` has no implementation for the same reason.
  - `StandaloneExecutorHost`: the process-wide part both executors need, made once, making one executor per
    container.
    Built meanwhile as the private stand-in `HostProcessHost` (crates/executor/implementation/host-process/host.rs),
    the one place his ruling changes.
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

## Unsafe code

The crate denies `unsafe_code`; each use carries its own `#[expect(unsafe_code, reason)]`. 30 in the library (28
build on ARM64, 25 on x86-64), 1 in its tests:
- code.rs (run 7): the code area's `mmap` where the host chooses and its `munmap`; the embedder's write into it (a
  slice of its own mapping, which only it writes and a stub only runs); cache maintenance by address and its
  barriers (`dc cvau`, `ic ivau`, `dsb ish`; no crate wraps them), ARM64 only.
- stub.rs: glibc's `tgkill` for the stop (rustix's `tkill` is unsafe too and reaches any thread of the host; nix has
  none for one thread of another process).
- zygote.rs: the zygote's assembly, one module per architecture (it runs with none of the embedder's memory, so
  it refers to nothing outside itself); glibc's `__rseq_offset` and `__rseq_size` (no crate has them); the two
  labels around the assembly; the slice of its code between them; the reservation's `mmap` and `munmap`;
  `sigfillset` with `pthread_sigmask`, then `fork`, then `pthread_sigmask` again (rustix has no signal mask call);
  the jump into the zygote's code in the child, one per architecture.
- probe.rs: `mrs ctr_el0` (no crate wraps it); the fixed `mmap` and `munmap` of the user-space search; the SVE
  probe's assembly (set p0 and FFR, `svc`, store them).
- pages.rs: the thread pages' `mmap` where the host chooses and its `munmap`; the two views of that mapping as
  32-bit and 64-bit atomics (the stub writes the same pages from its own process).
- windows.rs: the reservation's `mmap` and `munmap`; mapping a window, or inaccessible memory, over one slot of
  it; the copy out of a window and the copy into one (raw copies, since a stub may touch the bytes meanwhile).
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
  six words and the mask of the frame at x2 on the thread's own signal stack and returns through it; plan 12 item
  14 is the one way it is weaker than the rest.
Constants no crate defines, named with their source: glibc's rseq signatures (`RSEQ_SIG`), Linux's
`RSEQ_FLAG_UNREGISTER` and the original 32-byte `struct rseq`, `CTR_EL0`'s fields, Linux's `struct fpsimd_context`
and `FPSIMD_MAGIC` (arch/arm64/include/uapi/asm/sigcontext.h); in tests/floor.rs the `msr` encodings of NZCV,
FPSR, FPCR and `TPIDR_EL0` (Arm ARM).

## Dependencies and licences

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
