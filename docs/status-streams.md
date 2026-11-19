# Status of the streams track

The plan is docs/streams.md (S1, the stream ends) and docs/embedder-interface.md (S2, the
embedder's hooks, the process handle and the memory primitives). The owner's rulings are in
orchestrator/brief-streams.md and orchestrator/next-embedder-interface.md.

## Where things stand

- S1: complete; its third independent check's findings are settled (see "S1 check findings").
- S2: built in its first run; its first independent check's ten findings settled in its second run
  (see "S2 check findings"); every item of its bar met, the whole suite at 565881d2 and the speed
  over five test binaries finished in the fourth run (see "Measurements"); its second independent
  check's twelve findings settled in the fifth run (see "S2 second check findings").
- What comes next: S2's third independent check. Not built and left open on purpose: export of a process
  (`suspend`, `resume`, `SuspendedProcess`, `ExportedProcess`; where they attach is in
  docs/embedder-interface.md), `set_thread_ordering`, the inspection hooks, `StopReason`; ends made
  by connect, accept, export or import; `/dev/console` under `-t`.

## How to run

- The embedder's tests: `RUST_TEST_THREADS=3 nix develop -c cargo test -j 4 -p
  wcontainers-kernel-test --test embedder` (Docker must be running for the lazy memory test).
  Our side runs under `SCHEDULE_SEEDS` seeds (3 if unset) from `SCHEDULE_SEED` (1 if unset).
- The stream tests: the same with `--test streams`.
- The literal check: `RAYON_NUM_THREADS=3 nix develop -c cargo run -q -p wcontainers-literals --
  check` (exit 0).
- Everything: `RUST_TEST_THREADS=3 nix develop -c cargo test --no-fail-fast -j 4 -p wcontainers-abi
  -p wcontainers-kernel -p wcontainers-kernel-test -p wcontainers-literals`.

## S2

What exists (signatures and firing points in docs/embedder-interface.md):
- kernel: `System: Host` (system.rs) with `task_id_allocate`, `task_id_free` and the six hooks,
  all with provided defaults; `TaskId`; `Process` with `id`, `info`, `execution_config`,
  `memory_read`, `memory_write`, `memory_watch`, `memory_watch_thread` (refused by name),
  `memory_changed`; `ProcessInfo` with `memory_mapped`, `ceiling_distance`, `compute_used`,
  `parent`; `Kernel::process(cpu, id)`; `Watch`, `Access`, `MemoryError`, `ThreadOrdering`,
  `ReplayDeterminism`, `ExecutionConfig`, `Unsupported`. `Start` has `ceiling` and `execution`;
  `Kernel::start` returns `Result<(), Unsupported>`.
- abi: `PerThreadWatch`, the subtrait of `Cpu` for watching one thread (declared, implemented by no
  executor).
- watches and the record of written pages live in each `AddressMap` (memory.rs `Watches`), so a
  fork's child inherits watches with the map; the CPU side's permissions are narrowed page by page
  where they are set (`apply_permissions_in`). `syscall/touch.rs` describes what every built call
  touches and what thread ends touch; task.rs walks it before a call, signal.rs before a signal or
  an rseq check; `Guest::accessible` checks each copy against the description and treats a watched
  page as not there; a program's fault on a watched page announces the hook.
- test host: `driver::TestHost<H>`: with `hooks::NoHooks` (every test but the embedder's) it
  overrides nothing of `System`, so the kernel's own provided defaults answer; with
  `Box<dyn hooks::Hooks>` (the test's side of every hook and task id call, plus a call between
  steps) it forwards them. `driver::Prepared::{hooks, ceiling}`, `busybox::run_ours_hooked`.

The bar, item by item (tests/embedder.rs, guest/embedder.c; this Mac, seeds 1 to 3 unless said):
- Each hook's firing point and order over fork, exec, thread creation and exit:
  `life_cycle_hooks_fire_in_order_over_fork_exec_threads_and_exit` (a thread, a forked child that
  runs /bin/true, the exit): exactly starting(1), program(1), thread(1, 2), free(2), starting(3,
  parent 1), program(3), exiting(3), free(3), exiting(1), free(1). And
  `life_cycle_hooks_fire_in_order_over_signals_threaded_exec_vfork_groups_and_unwanted_children`
  (see "S2 check findings"), 38 events in exact order. Both pass.
- Task ids from the embedder: `task_ids_come_from_the_embedder_and_go_back_once_nothing_carries_them`:
  a pool from 1001 reusing freed numbers first; the thread's number, freed when it ended, becomes
  the child's; no number freed twice. Passes.
- Trying to make a hook nest: the recorder runs every operation of the handle (info, read,
  write, watch, the record of written pages, the refused per-thread watch) inside every hook and
  asserts no hook is running when one starts, and no task id is freed inside a hook. Passes.
- `on_process_outgrown`: `a_request_past_the_ceiling_is_announced_and_fails_as_out_of_memory`: with
  a ceiling of 1 GiB on a 4 GiB address space, a program mapping 64 MiB at a time is announced once
  with 64 MiB, the distance left below 64 MiB, the mmap fails with ENOMEM, counted once; without a
  ceiling nothing is announced and it maps more. `a_fixed_request_past_the_ceiling_...`: a page at
  1.25 GiB with `MAP_FIXED` and then `MAP_FIXED_NOREPLACE` is announced twice with a page each and
  both fail, counted as outgrown and not as above the address space; with no ceiling both map.
  Both pass.
- Watches met before calls write, inside the hook and left unmet and met later (between steps,
  through `Kernel::process`), the result unchanged:
  `a_watch_is_met_before_a_call_writes_and_the_result_is_unchanged`, 41 children each making one
  call into a watched buffer, inputs put there by the parent before the child starts: read, readv
  (two pages), recvmsg (a datagram, which Linux loses at a fault), a handler's frame on an
  alternate stack in the buffer, pread64, getcwd, uname, fstat, pipe2, wait4, getrandom,
  clock_gettime, setitimer, sigaltstack, recvmmsg, msgrcv, sendfile, timerfd_settime,
  timer_settime, pselect6, getsockopt, ioctl FIONREAD, getgroups, prctl, getresuid,
  sched_getaffinity, rt_sigtimedwait, splice, copy_file_range, times, getitimer, statfs,
  readlinkat, capget, epoll_pwait, waitid, getrlimit, fcntl F_GETLK, and the kernel's stores of
  `CLONE_PARENT_SETTID`, `CLONE_CHILD_SETTID` and `CLONE_CHILD_CLEARTID`. Each call announced the
  hook in exactly one process, at an address in the buffer, as a write, with the page as it was
  when the process started; the output is the same as with no watch; nothing counted outside a
  description, met at a copy or cut short. Passes both ways.
- A fault's span: `an_access_that_ends_before_a_watched_page_does_not_announce_it`. Passes.
- `memory_changed` exact: `the_pages_written_are_exactly_those_a_program_wrote`: a child writes
  pages 0, 2 and 5 of a buffer itself and page 7 by read(); the record over the buffer is exactly
  [0, 2, 5, 7]. Passes.
- Lazy memory from the two primitives: `lazy_memory_runs_the_shell_scripts_as_the_reference_does`:
  every new program's writable pages are saved, cleared to zero and watched (All); each is filled
  from the saved copy on first touch; a fork's child fills from its parent's copy (found by
  `ProcessInfo::parent`). The 17 scripts of tests/shell.rs that compare with the reference give
  the reference's output, error and exit status, pages were filled in each, and no page was found
  written past its watch, and no copy was outside its call's description. Passes at seeds 1 to 3
  (the nine embedder tests together 26.6 s, load about 55, 01:01).
- The whole suite with no hook implemented, and its speed: see "Measurements".
- The literal check: passes (see "Literals").

Found on the way, fixed:
- A kernel copy that noted a page as written told the CPU side only over its unaligned range, which
  the executor rounds inward, so the page stayed write-protected and the program's next store was
  a SIGSEGV (`reapply` now covers whole pages).
- The stand-in (unicorn) reports an atomic's store to a write-protected page as a read, and a
  store's fault at its first byte even when it reaches into the next page. A fault on a page whose
  reading is allowed is taken as a write, and a fault's span (64 bytes, four vector registers) is
  checked; the next page is noted only once the first may be written, so the record stays exact.
- A thread woken between steps (a watch met through `Kernel::process`) was picked before the kernel
  settled it; `Kernel::next` now answers `Idle` at time 0 first.

## S2 check findings (the first check), settled in the second run

- [blocking] Calls outside the description table did their work, met the watch at the copy and
  ran again. Every built call that touches program memory is now described (syscall/touch.rs:
  strings, `execve`'s strings, every structure, `ioctl` by encoding and the old terminal requests,
  `futex`, `prctl`, `seccomp`, IPC by command), and every copy a call makes while anything is
  watched is checked against its description (`copy outside the call's description`, counted).
  The calls test now makes 41 calls (the check's 14 among them: setitimer, sigaltstack, recvmmsg,
  msgrcv, sendfile, timerfd_settime, timer_settime, pselect6, getsockopt, FIONREAD, getgroups,
  prctl, getresuid, sched_getaffinity), each the first writer of its watched page, with the page
  unchanged at the hook and nothing counted outside a description; the lazy memory test asserts no
  copy outside a description over the 17 scripts. A description removed on purpose (times) makes
  the calls test fail on that counter (checked once, restored). The calls left to the check are
  listed, with why, in docs/embedder-interface.md.
- [blocking] The kernel's own stores: `CLONE_PARENT_SETTID` is now stored before
  `on_process_starting`; `CLONE_CHILD_SETTID` is met by the clone's walk; a thread's end
  (robust list and `clear_child_tid`) is walked by `exit`, `exit_group`, `execve` (the other
  threads) and before a fatal signal is acted on; rseq's accesses are walked before a handler and
  at preemption (the preemption check moved into `deliver`, behind a per-thread `rseq_event`, so a
  thread that waits on a watch checks its sequence when it goes on). Tested by three calls (raw
  vfork clones with each flag). A vfork parent going on after its child stores nothing more, so its
  clone is not walked again.
- [blocking] The ceiling: every map keeps the executor's address space as its end (the width);
  the ceiling is a separate field. Fixed requests (`MAP_FIXED`, `MAP_FIXED_NOREPLACE`,
  `MREMAP_FIXED`), `brk` growth and program segments that reach past it are announced as outgrown;
  a hint above it is placed below. Test `a_fixed_request_past_the_ceiling_is_announced_and_fails_as_out_of_memory`.
  The stack's top and the mmap base stay at the ceiling (see "Departures").
- [minor] A fault's span is looked at only when the CPU side allows the access all of the faulting
  page (no watch, no held-back write, no copy-on-write, no backend page not read). Test
  `an_access_that_ends_before_a_watched_page_does_not_announce_it` (the check's case).
- [minor] When the first process ends, every other process is killed (SIGKILL, as
  `zap_pid_ns_processes`), each with `on_process_exiting` (after the first's, see the second
  check), and every number still carried is freed once the container has ended.
- [minor] The order test now has a second program (`paths`): death by a fatal signal, `execve` from
  a second thread (its old number freed after `on_program_starting`), vfork, a group leader reaped
  while its group lives on (its number freed only when the grandchild leaves by `setpgid`, then by
  `setsid`, after a child made in between), and a child whose parent ignores SIGCHLD. Exact order
  at seeds 1 to 3.
- [minor] Speed over more than one test: see "Measurements".
- [minor] embedder.c's constants: the buffer's address and pages come as arguments, the page size
  from `sysconf`, and the number of calls is printed by the program from its own table (the size
  `outgrow` maps was still a literal on both sides; see the second check).
- [minor] `task_id_allocate`: an answer out of range fails at once (`task id out of range`,
  EAGAIN); each carried answer is counted (`task id still carried`) before the next try. The
  `previous` argument stays (the default counter needs it) and is listed for the owner.
- [minor] `Kernel::watching` is recomputed (any process with watches or a record) when watches
  change, a program starts and a process ends, instead of staying on.

## S2 second check findings, settled in the fifth run

- [blocking] mremap dropped watches and left moved pages out of the record. A watch now belongs to
  its page's bytes: a move's walk meets the pages it takes along as reads (an `All` watch fires
  where the page is, so lazy memory fills it before it moves), the remaining watches go along to
  the new place, and the moved pages are noted written there (as Linux's soft-dirty bits). The
  same gap was in `madvise`: `MADV_DONTNEED`, `MADV_DONTNEED_LOCKED` and `MADV_REMOVE` are met as
  writes and their pages noted written, as are the old pages of `MREMAP_DONTUNMAP`; a fork's
  child's pages wiped by `MADV_WIPEONFORK` lose their watches. Test
  `watches_and_the_record_follow_pages_moved_or_dropped` (a child moves 4 watched pages with
  `MREMAP_FIXED`, sums them, writes the last, drops the next page of the buffer): the same output
  as with no hooks (`dropped 0`, as Linux), hooks exactly [read at pages 0, 1, 2 where they were,
  write at the carried page where it landed, write at the dropped page], record exactly the 4
  moved pages where they landed and the dropped one.
- [blocking] The first process's end: the document said the others were announced first; the
  code (and Linux, whose first process kills the rest after its own memory is gone) announces the
  first, then each other. The document now says so. Test
  `the_first_process_is_announced_ending_before_the_others_it_takes_along`: exactly starting(1),
  program(1), starting(2, parent 1), exiting(1), exiting(2), free(1), free(2), exit status 3.
- [blocking] brk past the ceiling could never be announced: the stack (just below the ceiling)
  was met first. Growth ending above the ceiling is now outgrowing it, checked before what lies in
  the way. Test `heap_growth_past_the_ceiling_is_announced_and_fails_as_out_of_memory` (the buffer
  unmapped, brk grown by 1 GiB): with a 1 GiB ceiling announced once with 1 GiB, refused, counted
  once; with none it grows.
- [blocking] The size `outgrow` maps was a literal in embedder.c and `PIECE` in the test: it is now
  an argument, `PIECE` its one definition.
- [minor] `on_process_outgrown`'s moment: every place that decides it does so before the request
  has any effect (`MREMAP_FIXED`'s ceiling check moved before it unmaps the destination), and the
  thread gets its result only after the hook returns, so a later `suspend` in the hook can run the
  call again. `find_free` notes nothing without a ceiling, and the note is taken after every call.
- [minor] The cost of watching is confined: a call or signal walks its description only when its
  own process has watched pages (`Kernel::watched`, after the kernel-wide flag). Not measured.
- [minor] The test host's defaults: with no hooks the host is `TestHost<NoHooks>`, which overrides
  nothing of `System`, so the whole suite runs the kernel's own provided defaults; only the
  embedder's tests go through `Hooks`, whose no-op methods are the tests' convenience.
- [minor] The two task id departures (no generation, the `previous` argument) are now under
  "Departures".
- [minor] The walk announces what a call's arguments name, not what it writes: kept (nothing else
  is known before the call runs), now said in the document and under "Departures".
- [minor] `memory_write` into a shared page the process may not write is refused
  (`MemoryError::SharedNotWritable`), as Linux's `check_vma_flags` refuses a forced write there.
  Test `a_read_only_shared_page_is_not_written_by_the_embedder` (a read-only shared memfd page):
  refused, the file and the mapping still read `x`.
- [minor] The first program is told apart by the kernel having no first process yet
  (`Kernel::init`), not by `MemoryId(0)`; the placeholder id stays only until that program's
  memory exists, and nothing reads it.
- [minor] `/proc/self/status` printing one `Vm` line where the reference prints 12: outside this
  track (the kernel track's `/proc`, counted as `proc fields not tracked`); left to it.

## Measurements

- Third run's fixes, stream tests (20 tests, seeds 1 to 3): passed, 27.2 s, load 73 to 82 (21:15).
- Regression subset after the core change (streams, busybox, shell, processes, sigframe, threads,
  memory, faults, saving; seeds 1 to 3; load 120 to 150, 21:38): all passed but
  `a_terminal_is_resized`, whose reference side failed: the Docker client never forwarded the resize
  (`the reference never showed "size 100 30"`). It passed at 21:15 and in every earlier run. Not
  this track's kernel (the reference is Docker); recorded as a reference-side flake under load.
- Whole suite with no hook implemented (the test host forwards every hook to `hooks::Defaults`,
  which does what `System`'s defaults do), at 0c569e7d, the command in "How to run" (scratch
  streams/s2/suite.log, 70 minutes with the build, 21:55 to 23:05, load 92 at the start and 133 at
  the end): 579 passed, 1 failed, 6 ignored. `streams/verdicts.py run3-suite.log s2/suite.log`:
  every one of the 240 tests of S1's last suite has the same verdict but `busybox::system`, whose
  `ls -l /dev/null` shows the minute the container started: the reference's said 05:59 and ours
  06:00 (UTC). It passed alone at 22:34 (74.9 s, load 118). The four stream tests that the
  comparison shows as gone are the renamed ones, passing under their new names. New and passing:
  `credit_on_the_input_end_is_counted`, the six embedder tests (the lazy memory test at seeds 1 to
  3 among them, 19.0 s for the six), and the literal check's own tests. The commits after
  0c569e7d add the abi's `PerThreadWatch` declaration, two kernel unit tests (`system::tests`,
  passing: refused orderings by name, the default counter's wrap) and documents.
- Whole suite at 565881d2 (third run, scratch streams/s2b/suite.log, 01:10 to 02:08, load 62 at the
  start): 492 passed, 93 failed, 6 ignored, but not a verdict on the kernel: Docker Desktop stopped
  at 02:04 ("Docker Desktop is unable to start", for every track) and every test started after it
  failed at `docker create` or at the reference; `python::system_test_modules_match_reference`
  failed when its backend container died at 02:04:20; the two `devopen` tests failed because two
  `zig cc` guest builds hung for 19 minutes with every thread waiting (zig 0.16, the shared
  ~/.cache/zig, while a second test binary also built guests) and were killed. The 35 test binaries
  with failures are to be rerun by scratch streams/s2b/rerun.sh LOG (python last and alone).
- First rerun (02:40 to 02:55, load 7 rising to 62, scratch streams/s2b/rerun.log): the two
  `devopen` tests and `python::system_program_matches_reference` passed; Docker stopped again at
  02:51 (its VM log: "multiplexer: read vsock:57: no buffer space available"; the 02:04 stop was
  "vpnkit-data.sock: connection reset by peer" with port forwards of other projects open) and
  everything after failed at `docker create`. Both stops came while python tests of this track and
  of the kernel track were running; whether those contribute is not known.
- Second rerun (fourth run, the same tree as 565881d2, scratch streams/s2b/rerun-2.log, this Mac,
  04:59 to 05:42; load 6 at the start, 73 when python began at 05:03, 39 at the end): the 34 test
  binaries passed (90 tests), then python alone (3 tests, 2323 s). With the first rerun and the
  suite's earlier tests (`cat s2b/suite.log s2b/rerun.log s2b/rerun-2.log`, later verdicts winning):
  585 passed, 0 failed, 6 ignored. `streams/verdicts.py s2/suite.log s2b/combined.log`: every test
  has the verdict it had at 0c569e7d but `busybox::system`, failed there (the minute flake above)
  and passing now; new and passing: the three embedder tests and two `system::tests` unit tests
  added since. The whole suite's verdicts with no hook implemented are unchanged.
- Speed with no hook implemented: tests/shell.rs `runs_repeat_for_a_seed` (ours only, four
  background jobs of `seq 1 3000 | wc -l`, run at several seeds), its own test binary built at
  59056384 (before S2, in a scratch worktree, since removed) and at 0c569e7d, run alternately
  (scratch streams/s2/timing.sh, timing.log, timing-2.log). Before: 2.48, 1.92, 2.43, 3.64, 2.11,
  2.05, 2.00, 1.97 s; after: 2.62, 2.05, 2.21, 2.07, 1.92, 2.28, 1.97, 1.96 s; load 156 falling to
  127 (21:59 to 22:01). Medians 2.08 s before and 2.06 s after: unchanged within this machine's
  noise. Expected, not measured: a process with no watch and no record pays one flag test per call
  and per fault (`Kernel::watching`), and none per page permission set.
- Speed with no hook implemented over five whole test binaries (fourth run, this Mac): threads,
  processes, memory, sigframe and shell, built before S2 (59056384, in a scratch worktree, since
  removed) and at 565881d2's tree, each binary run alone with 3 test threads, old and new
  alternately, a warm-up round not counted and three rounds counted, inside `nix develop`
  (scratch streams/s2b/timing-2.sh, timing-5.log, timing-summary.py; 06:23 to 06:47, load 33 to
  101). Wall and CPU (user plus system, the test's child processes included: Docker's client and
  zig) medians, old then new: threads 41.4 / 40.5 s wall, 23.4 / 25.6 s CPU (rounds 22.9, 23.4,
  24.3 against 22.8, 25.6, 25.6); processes 8.7 / 8.5 s, 1.63 / 1.61 s; memory 44.5 / 45.8 s,
  7.88 / 7.97 s; sigframe 3.9 / 4.0 s, 0.44 / 0.44 s; shell 58.2 / 73.8 s wall (old's rounds 58,
  54, 73; new's 75, 60, 74: the reference's Docker runs under the load), 8.29 / 8.27 s CPU. All
  passed. Unchanged within this machine's noise.
- A first try of the same (timing-4.log, 05:55 to 06:11, load 27 to 72) showed threads 60 %
  slower, and it was not the kernel: the tests put the whole guest program folder
  (target/tmp/guest) into each container, and the new build's held 151 programs (257 MB, from the
  whole suite) against the old build's 20 (30 MB). A sample of `threads_survive_saving` showed the
  same samples in the kernel and the stand-in on both sides (StandIn::run 22 and 22) and the extra
  time in reading files. With the same folder on both sides (timing-5) the difference went. For
  later measurements: compare builds with the same guest folder.
- Zig: `zig cc` from Homebrew outside `nix develop` hung (three guest builds of one test binary
  sleeping 10 minutes at the link); the timing now runs inside `nix develop`, where it did not.

- Fifth run, after the second check's fixes (4d7e959e): the embedder tests (13, seeds 1 to 3) and
  streams, memory and faults passed on this Mac (scratch streams/s2c/affected-1.log, 08:03 to
  08:07, load 40 to 28; embedder 82 s). Whole suite on the ARM machine (`cargo test --no-fail-fast
  -j $ARM_JOBS` over the four crates; scratch streams/s2c/suite-s2c.log, 2257 s, ending 08:45,
  machine load 61): 561 passed, 28 failed, 6 ignored. 26 of the failures are the manual's list of
  tests whose reference differs there, plus `terminal::splice_holds_the_pipe` (the reference's tty
  there echoes once more: theirs "abc\r\nabc\r\n", ours "abc\r\n") and the literal check's
  `the_workspace_keeps_to_its_baseline` (the copy there has no git). On this Mac the 20 binaries
  holding them passed, 94 tests (scratch streams/s2c/mac-only.log, 08:09 to 08:13, load 24 to 17),
  and the literal check's tests passed. So 595 tests: 589 passed, 6 ignored, none failed. Speed was
  not measured again: with no hook the path is the same one flag test per call, and the host with
  no hooks now forwards nothing.

## Literals

- Adopted at da21d326 (`init` once on this tree, version 7 of the rules): 7612 baseline lines, of
  them wcontainers-kernel 2269, wcontainers-kernel-test 2434, wcontainers-abi 4, and 11 policy lines
  (the kernel's generated Linux files only). After this run: 3 lines fewer (the moved `"/"` in
  busybox.rs, now `ROOT_DIRECTORY`, and the two `!15` of the frame, now `STACK_ALIGNMENT`).
- After the fifth run: 7589 lines; wcontainers-kernel 2257, wcontainers-kernel-test 2423,
  wcontainers-abi 4. The fifth run named what the driver's renamed functions held (the notices'
  salts and gap, the chance of a second answer in a turn, a turn's time ranges, the backend's
  root) and kept the clock step in `TestHost::ran` (an associated function now, so the generic host
  keeps its key). Its `1000` (picoseconds in a nanosecond) stays: as a named constant the check
  takes three test inputs of 1000 elsewhere in the crate for restatements of it, and as a mark it
  says 1000 has a name (`AUDIT_GET`).
- The check's list of changes that could add lines: one, `da21d326f277 the start`.
- Still waiting for their owners: reference.rs `125`, `127` (Docker's failure statuses; the abi's
  `exit` module, runtime track) and tty.rs `"/dev/pts/{index}"` (the kernel track's path separator).

## Licences

No dependency added by this track. The literal check's crate brings syn, proc-macro2, quote, toml,
fnv and rayon (all MIT OR Apache-2.0), a test-time tool of the workspace, not a dependency of
anything shipped.

## Departures from the plan

- The literal check is taken as its crate's files from 7668c010, not merged (see above). When
  phase-1 or main carries the check with the runtime adapted to the stream ends, a merge brings
  the same files; the baseline will conflict, and docs/literals.md step 4 says how to settle it
  (leave it, `init`, commit).
- Hooks and task ids are on `wcontainers_kernel::System`, which extends the abi's `Host`, not on
  one trait in the abi: the hooks take `Process`, which holds the kernel. The rename of `Host`
  into `System` and of `Cpu` into `Executor` is left to the runtime track's merge.
- `Process` has no executor parameter (`Process<'_, E>` in the rulings) and `ExecutionConfig` no
  `translator` field: the kernel reaches the executor as `dyn Cpu`. `Kernel::process` takes the
  executor (`cpu`), which the kernel does not own.
- `memory_read` returns an owned copy, not pieces of borrowed bytes: the `Cpu` trait reads by copy.
  `memory_write(address, bytes)` takes the address, not a range, since the range is the bytes'.
- `ProcessInfo::parent` is added (Linux's `getppid`, published to anyone): an embedder that keeps
  its own picture from the hooks needs a new process's parent at `on_process_starting`, and the
  lazy memory test needs it to hand a child its parent's saved pages. For the owner.
- The ceiling: low-first placement and the executor's ceiling are not on origin/phase-1
  (unchanged at eb515b7a on 2026-11-19), so the ceiling is given at start and kept beside the
  width in every map. Assumed: the executor cannot back any address at or above the ceiling, so
  anything that would reach past it is outgrown (fixed requests, brk growth, segments, placements
  that find no room), and, with top-down placement, the stack's top and the mmap base are put at
  the ceiling so placed memory stays below it. The width (the highest address a fixed request may
  name) stays the executor's address space. Stack growth is downward and never reaches the
  ceiling.
- `memory_changed`'s first answer is every page mapped writable (nothing was recorded before);
  writes by other processes to shared pages are not in a process's record.
- Watches act on whole pages of `PAGE_SIZE`. Left to the copy check (listed with why in
  docs/embedder-interface.md): `semctl` GETALL/SETALL and the IPC `*_INFO` commands,
  `restart_syscall`, a priority-inheriting lock handed on at a thread's end, another process's
  memory (process_vm_readv/writev, /proc/<pid>/mem: the copy stops short at a watched page with no
  hook, until inspection is built), and thread-end stores when the container ends.
- The first process's end now kills the others in the kernel (Docker kills them from outside), so
  each is announced and its number freed.
- Task ids: the embedder's id carries no generation (the earlier ruling said it would): the
  embedder hands out every number and hears when each is free, so a number it keeps never names
  another process without its knowing. `task_id_allocate` takes `previous`, the id handed out
  last, which the owner's names do not have: the provided default counter needs it, since a
  trait's default method keeps no state. Both for the owner to confirm.
- `on_memory_accessing` before a call is announced for what the call's arguments name, which can
  be more than it then writes (a short read, `nanosleep`'s remaining time).
- `Access` is the kernel's own two-variant enum; the abi's `Access` (with `Execute`) stays for
  faults. Running code on a watched page is reported as `Read`.
- Watch waits are not ended by signals, as backend waits are not.
- From S1, still standing: `a_terminal_closed_before_the_start_finished` does not guard the
  release it was written for (devpts lookup goes by the master's count; no observation found that
  does); `/dev/console` absent under `-t`; a terminal's space toward the slave is
  held for the embedder's credit; bytes the embedder sends after the kernel closed an input end are
  dropped uncounted; output after the embedder closed its end is compared with a pipe whose reader
  the reference program closed itself.

## For the orchestrator

- For the owner: `task_id_allocate` takes `previous` (the id handed out last), which the
  owner's names do not have; the provided default counter needs it, since a trait's default
  method keeps no state.
- Names for the owner (suggested): `Process::memory_write(address, bytes)`; `ProcessInfo::parent`;
  `ProcessInfo::ceiling_distance` (defined as what new mappings can still take below the ceiling);
  `MemoryError::{NotMapped, NotRead, Executor}`; `Unsupported`; `ExecutionConfig::supported`;
  `Start::ceiling`, `Start::execution`; `task_id_after` (the default counter);
  `PerThreadWatch::set_thread_permissions`.
- From S1 (suggested, unchanged): `End`, `Host::stream_send`, `Host::stream_close`,
  `Event::StreamData`, `Event::StreamCredit`, `Event::StreamClosed`, `Event::TerminalResized`,
  `Start::standard`, `Standard::{Pipes, Terminal}`.
- The literal check: phase-1-kernel merged phase-1-literals directly; this branch took only the
  crate. Whoever merges both into phase-1 resolves the baseline once by `init`.
