# Status of the streams track

The plan is docs/streams.md (S1, the stream ends) and docs/embedder-interface.md (S2, the
embedder's hooks, the process handle and the memory primitives). The owner's rulings are in
orchestrator/brief-streams.md and orchestrator/next-embedder-interface.md.

## Where things stand

- S1: complete; its third independent check's findings are settled (see "S1 check findings").
- S2: built in its first run; its first independent check's ten findings settled in its second run
  (see "S2 check findings"); every item of its bar met (see "S2" and "Measurements"). Waiting for
  its next check, but for two runs Docker's stops took from the third run (see "Measurements"):
  the whole suite's verdicts at 565881d2 for the 35 test binaries lost to Docker, and the speed
  comparison over five test binaries (scratch streams/s2b/timing.sh, old build kept in
  streams/s2b/old).
- What comes next: once Docker answers (it was still down at 04:52, after 2 hours),
  `sh streams/s2b/rerun.sh streams/s2b/rerun-2.log`; then build the old binaries in the old
  worktree (its target was removed: `nix develop -c cargo test --no-run -j 4 -p
  wcontainers-kernel-test --test threads --test processes --test memory --test sigframe --test
  shell` inside it) and run `sh streams/s2b/timing.sh` with the old and new deps and crate folders
  (not at the same time as the rerun:
  two test binaries building guests at once hung zig), record both, remove the old worktree
  (`git worktree remove`) and its target. Then S2's independent check. Not built and left open on purpose: export of a process
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
- test host: `hooks::Hooks` (the test's side of every hook and task id call, plus a call between
  steps), `driver::Prepared::{hooks, ceiling}`, `busybox::run_ours_hooked`.

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
- [minor] When the first process ends, every other process is ended first (SIGKILL, as
  `zap_pid_ns_processes`), each with `on_process_exiting`, and every number still carried is freed
  once the container has ended.
- [minor] The order test now has a second program (`paths`): death by a fatal signal, `execve` from
  a second thread (its old number freed after `on_program_starting`), vfork, a group leader reaped
  while its group lives on (its number freed only when the grandchild leaves by `setpgid`, then by
  `setsid`, after a child made in between), and a child whose parent ignores SIGCHLD. Exact order
  at seeds 1 to 3.
- [minor] Speed over more than one test: see "Measurements".
- [minor] embedder.c defines no constant of the test's any more: the buffer's address and pages
  come as arguments, the page size from `sysconf`, and the number of calls is printed by the
  program from its own table.
- [minor] `task_id_allocate`: an answer out of range fails at once (`task id out of range`,
  EAGAIN); each carried answer is counted (`task id still carried`) before the next try. The
  `previous` argument stays (the default counter needs it) and is listed for the owner.
- [minor] `Kernel::watching` is recomputed (any process with watches or a record) when watches
  change, a program starts and a process ends, instead of staying on.

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

## Literals

- Adopted at da21d326 (`init` once on this tree, version 7 of the rules): 7612 baseline lines, of
  them wcontainers-kernel 2269, wcontainers-kernel-test 2434, wcontainers-abi 4, and 11 policy lines
  (the kernel's generated Linux files only). After this run: 3 lines fewer (the moved `"/"` in
  busybox.rs, now `ROOT_DIRECTORY`, and the two `!15` of the frame, now `STACK_ALIGNMENT`).
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
