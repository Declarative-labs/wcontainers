# Status of the streams track

The plan is docs/streams.md (S1, the stream ends) and docs/embedder-interface.md (S2, the
embedder's hooks, the process handle and the memory primitives). The owner's rulings are in
orchestrator/brief-streams.md and orchestrator/next-embedder-interface.md.

## Where things stand

- S1: complete; its third independent check's findings are settled (see "S1 check findings").
- S2: built in its first run (see "S2"). The whole suite's result is under "Measurements".
- What comes next: S2's independent check. Not built and left open on purpose: export of a process
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
  where they are set (`apply_permissions_in`). The kernel's copies (`Guest::accessible`) treat a
  watched page as not there; `syscall/touch.rs` walks what a described call will touch before it
  runs; a signal's frame is walked before the signal is taken; a program's fault on a watched
  page announces the hook.
- test host: `hooks::Hooks` (the test's side of every hook and task id call, plus a call between
  steps), `driver::Prepared::{hooks, ceiling}`, `busybox::run_ours_hooked`.

The bar, item by item (tests/embedder.rs, guest/embedder.c; this Mac, seeds 1 to 3 unless said):
- Each hook's firing point and order over fork, exec, thread creation and exit:
  `life_cycle_hooks_fire_in_order_over_fork_exec_threads_and_exit` (a thread, a forked child that
  runs /bin/true, the exit): exactly starting(1), program(1), thread(1, 2), free(2), starting(3,
  parent 1), program(3), exiting(3), free(3), exiting(1). Passes.
- Task ids from the embedder: `task_ids_come_from_the_embedder_and_go_back_once_nothing_carries_them`:
  a pool from 1001 reusing freed numbers first; the thread's number, freed when it ended, becomes
  the child's; no number freed twice. Passes.
- Trying to make a hook nest: the recorder runs every operation of the handle (info, read,
  write, watch, the record of written pages, the refused per-thread watch) inside every hook and
  asserts no hook is running when one starts, and no task id is freed inside a hook. Passes.
- `on_process_outgrown`: `a_request_past_the_ceiling_is_announced_and_fails_as_out_of_memory`: with
  a ceiling of 1 GiB on a 4 GiB address space, a program mapping 64 MiB at a time is announced once
  with 64 MiB, the distance left below 64 MiB, the mmap fails with ENOMEM, counted once; without a
  ceiling nothing is announced and it maps more. Passes.
- Watches inside read, readv, recvmsg and a signal's frame write, met inside the hook and left
  unmet and met later (between steps, through `Kernel::process`), the result unchanged:
  `a_watch_is_met_before_a_call_writes_and_the_result_is_unchanged`, twelve children each making
  one call into a watched buffer: read, readv (two pages), recvmsg (a datagram, which Linux loses
  at a fault), a handler's frame on an alternate stack in the buffer, pread64, getcwd, uname,
  fstat, pipe2, wait4, getrandom, clock_gettime. Every child's call announced the hook, at an
  address in the buffer, as a write, with the buffer still holding its old bytes at the hook; the
  output is the same as with no watch; nothing was met at a copy or cut short. Passes both ways.
- `memory_changed` exact: `the_pages_written_are_exactly_those_a_program_wrote`: a child writes
  pages 0, 2 and 5 of a buffer itself and page 7 by read(); the record over the buffer is exactly
  [0, 2, 5, 7]. Passes.
- Lazy memory from the two primitives: `lazy_memory_runs_the_shell_scripts_as_the_reference_does`:
  every new program's writable pages are saved, cleared to zero and watched (All); each is filled
  from the saved copy on first touch; a fork's child fills from its parent's copy (found by
  `ProcessInfo::parent`). The 17 scripts of tests/shell.rs that compare with the reference give
  the reference's output, error and exit status, pages were filled in each, and no page was found
  written past its watch. Passed at seed 1 (17.2 s, load about 100); the whole suite runs it at
  seeds 1 to 3.
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

## S1 check findings (the third check), settled this run

- Literal check not adopted: the brief now rules to take it from origin/phase-1-literals 7668c010.
  A merge of that branch would bring main's runtime crate, which still implements the removed
  `Host::output`, and make kernel-test (which then depends on it) fail to build; this track may
  not edit that crate. So the check's crate was taken from 7668c010 as it is (same files), `init`
  run once, and the check is in the bar. See "Departures" and "Literals".
- Vague words: renamed on every line this track wrote. `Pipe::outside_reader` is `reading_end`,
  `room_outside` is `free_bytes_for_end`, `from_outside` / `to_outside` are `receive_from_end` /
  `send_to_end`, `promise_outside` is `credit_for_end`, `woken_by_outside` is `woken_by_end`,
  `resize_from_outside` is `resize_from_end`, `grant` is `credit`; the test's `Outside` is
  `StreamClient` (it plays Docker's client), `run_outside` is `run_with_client`; comments say
  "embedder" (kernel and abi) or "client" (tests). Linux's own `write_room` and `no_room` stay, as
  does the plain preposition in "a writer outside Linux".
- Credit on an end the kernel only receives on is now counted (`event with no waiter`, detail the
  end): ends are named with their direction at start. Test `credit_on_the_input_end_is_counted`
  (a new `Step::Credit`). Passes.
- The reference's race in `a_shell_on_the_terminal`: the ^C is now sent after `start`, printed by
  the job itself once it is the foreground group (`sh -c 'echo st""art; exec sleep 30'`).
  `interrupt_and_suspend_reach_the_foreground_group` waits for `ready`, which its program prints
  after setting up, so it is left as it was.
- `a_terminal_closed_before_the_start_finished` now also checks that the slave's devpts node is gone
  (exit 8 if `/dev/pts/0` exists). A run with the release replaced by a drop still passes: devpts
  lookup goes by the master's count too, so this test still does not guard the release. No
  observation found that does; recorded, not hidden.
- The third salt is `SIGNALS_SALT`.
- A pipe write to standard output now sends what credit allows in one `stream_send` (a page that
  cannot be read whole still ends the write unsent). Not measured.

## Measurements

- Third run's fixes, stream tests (20 tests, seeds 1 to 3): passed, 27.2 s, load 73 to 82 (21:15).
- Regression subset after the core change (streams, busybox, shell, processes, sigframe, threads,
  memory, faults, saving; seeds 1 to 3; load 120 to 150, 21:38): all passed but
  `a_terminal_is_resized`, whose reference side failed: the Docker client never forwarded the resize
  (`the reference never showed "size 100 30"`). It passed at 21:15 and in every earlier run. Not
  this track's kernel (the reference is Docker); recorded as a reference-side flake under load.
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
- The ceiling: low-first placement and the executor's ceiling are not on origin/phase-1, so the
  ceiling given at start is the end of every new address map: placement stays top-down from it, the
  stack at its top. A request whose placement finds no room is announced as outgrown. brk and stack
  growth running into mappings are not announced (they fail as before).
- `memory_changed`'s first answer is every page mapped writable (nothing was recorded before);
  writes by other processes to shared pages are not in a process's record.
- Watches act on whole pages of `PAGE_SIZE`; a call the touch table does not describe meets a watch
  at its copy, as described in docs/embedder-interface.md, counted.
- `Access` is the kernel's own two-variant enum; the abi's `Access` (with `Execute`) stays for
  faults. Running code on a watched page is reported as `Read`.
- Watch waits are not ended by signals, as backend waits are not.
- From S1, still standing: `/dev/console` absent under `-t`; a terminal's space toward the slave is
  held for the embedder's credit; bytes the embedder sends after the kernel closed an input end are
  dropped uncounted; output after the embedder closed its end is compared with a pipe whose reader
  the reference program closed itself.

## For the orchestrator

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
