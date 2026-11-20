# Status of the streams track

The plan is docs/streams.md (S1, the stream ends) and docs/embedder-interface.md (S2, the
embedder's hooks, the process handle and the memory primitives). The owner's rulings are in
orchestrator/brief-streams.md and orchestrator/next-embedder-interface.md.

## Where things stand

- S1: complete; its third independent check's findings are settled (see "S1 check findings").
- S2: built in its first run; its first five independent checks' findings settled in the second,
  fifth, sixth, seventh and eighth runs (sections below); its sixth check's five findings and the
  owner's rulings of the afternoon of 2026-11-19 (verb-first names, `free_task_id` with no default,
  `on_memory_written_by_kernel`, the system call proxy, `WatchesFor` and `AccessKind`) settled in
  the ninth run (see "S2 sixth check findings"); its seventh check's findings settled in the
  tenth run (see "S2 seventh check findings"); its eighth check's findings settled in the
  eleventh run (see "S2 eighth check findings").
- S2 passed its ninth check and is merged (origin/phase-1 at cefb66ab); its four carried findings
  are settled in S3's first run (see "S3").
- S3 (the orchestrator's 2026-11-20 section of the brief): all three parts are built and tested
  (signals as an input, shared memory announced per process with the memory id, and suspend and
  resume by task with a process context, the last in run 2); run 3 made a task taken in its call
  answer as Linux's stop makes it answer (the brief's 03:00 and 04:00 rules) and measured the
  speed with no hook implemented unchanged (+0.05 % user CPU, 12 alternating rounds). The first
  independent check of S3 found four blocking and three minor problems; run 4 settled them (see
  "S3 first check findings"). The second check found three blocking and five minor problems;
  run 5 settled them (see "S3 second check findings"). Run 6 changed no code: on the ARM
  machine the new tests equal the reference, the whole suite's verdicts are unchanged, and the
  speed with no hook implemented is unchanged (see "Measurements", S3 run 6). Run 8 changed no
  code: Docker Desktop on the Mac started containers again at 08:08 Pacific, and the 20 binaries
  holding the manual's 25 Mac-only tests passed there (94 tests, `splice_holds_the_pipe` among
  them), so with the ARM machine's verdicts no test fails by our side (see "Measurements", S3
  run 8). The third check found two blocking and six minor problems; run 9 settled them (see
  "S3 third check findings"), with a new test program (guest/edges.c, tests/edges.rs), the whole
  suite and the Mac-only binaries (no test fails by our side) and the speed with no hook
  implemented (see "Measurements", S3 run 9). Next: S3's next independent check.
- Waiting, not to be built in S3 (the brief's list): the kernel generic over its `System` and
  `ProcessState` (after the kernel track merges); `wake_waiters` and `move_waiters`; the
  scheduling rule for Serial and Parallel tasks sharing memory; moving crates into the layout.
  Also still open: any wire form (export, import), `set_thread_ordering`, the inspection hooks,
  `StopReason`; ends made by connect, accept, export or import; `/dev/console` under `-t`.

## S3

Run 1 (this run) did:
- The carried findings: the interface document says only `/proc/<pid>/cmdline` is open to anyone
  and `environ` needs Linux's `PTRACE_MODE_READ`; a new test,
  `an_environment_read_through_proc_meets_the_watches_of_the_process_it_names` (embedder.c
  `environ`, which runs itself again with a 3-page variable and reads its environment through
  /proc, from itself and from a child), passes beside the command line's; the eleventh run's
  wall times are stated in "Measurements"; the generic kernel is recorded as waiting; the ARM
  machine's full disk is under "For the orchestrator".
- Part 1, signals as an input: `SignalInformation` (the generated `siginfo`),
  `System::on_signal_delivering`, `Process::send_signal`, the counter `signal from the embedder
  not sent`. Firing point and guarantees: docs/embedder-interface.md. A thread now keeps the
  signal it took while it waits on a watch over the signal's frame (`Thread::signal_taken`), so
  the hook fires once per signal. Tests: `every_signal_a_thread_takes_to_act_on_is_announced_first_with_its_record`
  (embedder.c `signals`: queued, blocked then let through, sent by another thread during a read
  with `SA_RESTART`, a fault; one taken by sigwaitinfo and one ignored are not announced; the
  hook's records equal what the handlers saw, and the output equals the reference's) and
  `a_signal_the_embedder_sends_interrupts_a_read_as_one_another_process_sends` (embedder.c
  `interrupted`: the reference's child is interrupted by its parent's sigqueue; ours by the
  embedder at the read's request, while the read waits on a watch the hook left unmet, and
  between steps; same output, one announcement, and a signal to another process's thread and
  one with number 0 counted as not sent).
- Part 2: `MemoryObject` and its four ids, `on_shared_memory_mapping`,
  `on_shared_memory_unmapped`, `Process::memory_id` (sharing.rs; address maps note shared
  mappings going in or out through `MapNotes`, which replaced the shared `outgrown` cell; the
  kernel compares what live processes map with what it announced). Tests:
  `shared_memory_is_announced_per_process_as_it_starts_and_ends_as_proc_maps_shows_it`
  (embedder.c `sharing`: a file mapped read-only by a process and by its forked child, then
  writable by the parent, which announces both earlier mappings then; shared anonymous memory
  cut in two; a System V segment attached and detached; a memory file split by mprotect and
  moved by mremap; a shared mapping replaced by a private one; a child running a program; at
  each step the announced pages and offsets of the process equal its /proc/self/maps in the
  reference, step by step, each end names what was announced, and all end),
  `processes_in_one_memory_have_one_memory_id_and_a_new_program_a_new_one`, and a unit test of
  the piece arithmetic.
- Every hook test's recorder also runs every handle operation inside `on_signal_delivering`.

Run 2 did part 3, suspend and resume by task with a process context (suspend.rs):
- On `Kernel`: `suspend_tasks(cpu, &[TaskId])`, `suspend_process(cpu, TaskId)`,
  `copy_process_context(TaskId)`, `resume_process(ProcessContext, ExecutionConfig)`,
  `resume_tasks(cpu, tasks)`; on the hook's `Process` handle: `suspend_tasks`,
  `suspend_process(self)`, `copy_process_context`. `SuspendedTask` (the `Thread` record, its
  registers, where it was in line) and `ProcessContext` (the `process::Process` record and what
  the embedder was told it maps of shared memory) are the kernel's own types. Firing points,
  guarantees and what the batch means while one thread runs at a time: docs/embedder-interface.md
  "Suspending and resuming tasks".
- A parked call keeps its call slot and runs again when put back; a call waiting in a futex's,
  semaphore set's or message queue's line leaves it and is made again (restart_syscall for a
  timed futex wait), so a wake while away is not lost. Inside a hook, the thread the kernel is in
  the middle of a step for, a task being started by an exec, and the tasks of a process being
  ended are not taken (counted, `task in the middle of a kernel step not suspended`), except the
  thread at `intercept_system_call_request`, which is taken before its call and makes it again.
- New kernel fields: `stepping`, `call_not_begun`, `starting_task`, `ending_process`. `wake_thread`
  and the sharing announcements now pass over a task or process that is not on the kernel; a
  thread ending while its siblings are away hands its children to none.
- Test: `tasks_suspended_from_hooks_and_between_steps_go_on_as_the_reference_runs` (embedder.c
  `suspend`: a reader of an empty pipe, a waiter on a mutex the main thread holds, a timed futex
  waiter, a taker from a System V semaphore; the main thread sleeps twice, then writes the pipe,
  wakes the futex, lets the mutex go and gives to the semaphore, joins, prints). Two plans, each
  in all 24 orders of putting the four tasks back: from hooks (the
  four waiting threads at the second sleep's request, kept away while the main thread wakes
  them; the main thread itself at its write's request, before the call; asking for it again at
  the write's response is refused and counted once), and between steps (one thread at a time and
  a task not on the kernel; every thread; the whole process, its tasks first without the context
  (all handed back), the context, a copy of it again (handed back), then the tasks; the whole
  process moved to a second `Kernel` value, checked there by memory id and size, and brought
  back). The output equals the reference's in every case. With the futex line leaving disabled,
  the hooks plan hangs (the mutex waiter's wake is lost): checked once on the ARM machine.
- Not covered by a test: a message queue waiter taken (it leaves its line as the futex waiter does);
  running a moved process on the second kernel (only its state is checked there). The semaphore
  waiter would also go on correctly on the same kernel without leaving its line (its result is
  kept by its number), so only the futex case is shown to need it.

Run 3 did the brief's rules of 2026-11-20 03:00 and 04:00 (written after run 2 ended):
- A task taken in its call (parked, or woken and not yet run) is treated as Linux treats a signal
  with no handler arriving in that call: the thread's `suspended_in_call` makes `signal_pending`
  true for the call's next run (Linux's freezer wake), cleared once that run has been made, and
  each call answers by its own code; the restart codes then run it again. Waiters in lines a wake
  chooses from leave them when taken: a futex wait (its deadline dropped, so it answers as
  interrupted and `restart_syscall` keeps its time), a semaphore operation (told EINTR, Linux's
  answer after a stop), a message queue send or receive. This replaced run 2's restart from the
  instruction for those three and its "keeps its call and goes on waiting" for the rest.
- The per-call list (restart, go on with the time left, answer what was done, EINTR) is in
  docs/embedder-interface.md, "Suspending and resuming tasks".
- Test: `a_task_taken_in_its_call_answers_as_a_stop_on_the_reference_makes_it_answer`
  (guest/stopped.c, its own program: a child's seven threads wait in a futex wait, a futex wait
  already woken, a 600 ms sleep, a write of twice the pipe's size, a semaphore operation,
  `epoll_wait` and a pipe read; the parent stops the child, changes and wakes the futex and reads
  the semaphore's waiter count while it is stopped, continues it, writes the byte and empties the
  pipe). The reference is stopped and continued with SIGSTOP and SIGCONT; ours is taken off the
  kernel between steps and put back, the parent's `kill` answered 0 and its wait for the stop
  answered with the child (the parent is taken at that wait so the kernel goes idle). Two plans
  (`suspend_process` then `resume_process` and `resume_tasks`; `suspend_tasks` of every thread put
  back in reverse), 3 seeds each. Every answer equals the reference: EAGAIN, 0, 0 having slept the
  whole 600 ms, 65536, EINTR, EINTR, 1; the wake while stopped finds nobody (0), no semaphore
  waiter. With `suspended_in_call` left out of `signal_pending` it fails (the epoll wait never
  ends).
- embedder.c `suspend`'s semaphore taker takes again after EINTR (the reference never sees one).
- stopped.c is its own program because adding it to embedder.c made Nix's zig 0.16.0 on the ARM
  machine emit an object with undefined section symbols (`ld.lld: undefined symbol:` with no
  name), as the manual says it does for fdinfo.c; the Mac's zig was not tried.

Open points for the check and the owner (also in "Departures"):
- A process resumed on a kernel without its parent has no reaper there: its end on that kernel
  is not built (the test does not let it end there).

How S3's tests were run: see "Measurements" (S3 run 1).

## S3 first check findings, settled in run 4

The check (scratch streams/s3check, its own program s3check.c and test s3check.rs) found:
- Blocking, a thread exiting while its siblings are away ended the process. Now a process's
  record keeps its tasks taken off the kernel (`Process::away`: the number, the signals each
  blocks, the signals sent to it since); `exit` and seccomp's KILL_THREAD end the process only
  when no task is on the kernel or away; any end of the process ends its tasks away. Test
  `a_process_whose_main_thread_is_away_goes_on_when_its_other_thread_ends`.
- Blocking, a signal to a process whose tasks are all away was dropped. Now it is kept in the
  process's pending set, as Linux keeps it for a stopped process; a task put back acts on it
  (a task with nothing to do in a call is put among those with a signal to act on). SIGKILL
  ends the process at once, as it ends a stopped one. Test
  `a_signal_to_a_process_whose_tasks_are_away_waits_for_them_and_sigkill_does_not` (SIGTERM and
  SIGKILL while the child's read is away, the pipe closed before it comes back).
- Blocking, a thread's own POSIX timer firing while it is away was lost. Now `tgkill` and its kin
  and a timer aimed at the thread find a task away and keep the signal with its record; it goes to
  the task when it is put back. Test
  `a_threads_timer_firing_while_it_is_away_interrupts_its_read_when_it_is_back`. The check's own
  order (the byte written to the pipe before the task comes back) still answers the byte where
  Linux answers EINTR: the call runs again when the task is put back and finds the byte first.
  Left as a known failure, `a_threads_timer_firing_while_it_is_away_interrupts_its_read_though_the_byte_came`
  (ignored, with the reason; it fails when run: "read 1" against the reference's "read -1
  Interrupted system call").
- Blocking, a copy of a context resumed on a second kernel redirected the original's address map.
  Now the copy has its own map and handler table, no pending signal, and every task of the process
  counted as away from it; an address map notes to every kernel a process using it is on (taking
  a process stops its map noting to the kernel it left unless a process there still uses it). Test
  `shared_memory_is_announced_on_the_first_kernel_after_a_copy_goes_on_a_second`.
- Minor, a parent's wait for a child whose context is away answered ECHILD. The kernel now keeps
  the parent, group and exit signal of a process whose context it gave up (`processes_away`);
  `wait4` and `waitid` wait for it (`WNOHANG` answers 0), reparenting reaches it, and the context
  takes its parent from there when it comes back. Test
  `a_parent_waits_for_a_child_whose_process_is_away`. `kill` of a process away still answers
  ESRCH (written in the document; see "Departures").
- Minor, the handle's `suspend_process` answered a copy that looked like a take. `ProcessContext`
  now says how it was made, `origin()` (`ContextOrigin::{Suspended, Copied}`), an addition to the
  ruled interface listed for the owner. `Kernel::suspend_process` answers none for a process being
  ended. Test `a_hooks_process_whose_thread_is_in_its_step_stays_and_says_its_context_is_a_copy`.
- Minor, the check could not read its whole-suite result: this run's suite is in "Measurements".
The new tests are tests/away.rs with guest/away.c, compared with the reference where nothing is
taken.

## S3 second check findings, settled in run 5

The check (scratch streams/s3check2, its own program s3chk.c and test s3chk.rs) found:
- Blocking, the memory and file blocks of a context away were dropped when the last process on
  the kernel sharing them ended (a forked child's end dropped the parent's program file block; a
  `CLONE_VM` child's end dropped the memory). Now the kernel keeps a weak reference to the map
  of each context taken or copied off it, with its memory, until it is put back here
  (`Kernel::maps_away`, `live_maps_away`): `memory_in_use_elsewhere`, the release of file
  blocks, a file's block moved or cut, `writably_mapped` (memfd seals) and System V
  attachments count those maps. Also found while fixing it: a child that ended while its
  parent's context was away was reaped at once, as if it had no parent; now it stays a zombie for
  the parent, its SIGCHLD is kept (`ProcessAway::signals`) and sent when the parent is back,
  and a parent that does not want its children's zombies reaps them on its return. Tests
  `a_parent_away_while_its_forked_child_ends_goes_on_when_it_is_back` and
  `a_parent_away_while_a_child_in_its_memory_ends_goes_on_when_it_is_back` (away.c `leave`,
  with a memory file shared with the child).
- Blocking, a process taken inside its own `on_shared_memory_mapping` was announced its next
  piece, with a handle that panicked. Now `announce_sharing` keeps what each process was told in
  step piece by piece (updated before each hook), skips a process that left, and stops when a
  hook started a new round; `announce` and `announce_start` fire no hook for a process taken in
  an earlier hook of the same moment (sharing or kernel-write hooks); a watch touch on a process
  that left finds no watch. `resume_process` makes the kernel compare what the process maps with
  what it was told at its next hook, so it is told the rest then. Test
  `a_process_taken_in_its_own_sharing_hook_is_told_the_rest_when_it_is_back` (away.c `pieces`:
  every hook reads `info()`, each end names what was told, all end, the second piece told after
  the process is back).
- Blocking, the test host did not run S3's operations or hooks. The recorder now implements both
  sharing hooks and runs `memory_id`, `copy_process_context`, `suspend_tasks` of none and
  `send_signal` of number 0 in every hook; it accepts `SharedNotWritable` when writing back a
  page of a read-only shared mapping (as documented). New test
  `every_operation_of_the_handle_is_legal_in_the_shared_memory_hooks` (embedder.c `sharing`).
  Taking a process inside a hook is in the suspend tests, as the document now says.
- Minor, `send_signal` to a task away was dropped. Now it is kept as tgkill's is. Test
  `a_signal_the_embedder_sends_to_a_task_away_waits_for_it` (away.c `sent`).
- Minor, a socket wait with `SO_RCVTIMEO` or `SO_SNDTIMEO` restarted after a stop or a handler.
  Now `wait_room` answers EINTR when the socket has a timeout (`sock_intr_errno`). Tests
  `a_socket_wait_with_a_receive_timeout_taken_off_answers_as_a_stop_makes_it_answer` and
  `a_socket_wait_with_a_receive_timeout_is_not_restarted_after_a_handler` (away.c `socket`);
  the table in the interface document says so.
- Minor, the context could carry pieces not yet told: settled with the second finding.
- Minor, no task runs on a second kernel: unchanged, as the ruling asks for one kernel (see
  "Departures"). The first finding's fix also covers a context resumed on a second kernel
  sharing the executor, as far as the first kernel goes: the first keeps the memory while the
  map lives.
- Minor, the reference's verdicts were owed: given in run 6 on the ARM machine, but for the
  Mac-only list (see "Measurements", S3 run 6).
Each new test fails without its fix (checked on this Mac with the expected output written from
the program in place of the reference's: SIGSEGV, "the kernel is idle with nothing to wake it",
the panic at system.rs:341, "read 1" and "recv -1 Resource temporarily unavailable"; scratch
streams/negative.sh and negative.log).

## S3 third check findings, settled in run 9

The check (scratch streams/s3check3, its own program chk3.c and test chk3.rs) found:
- Blocking, a signal sent while a thread waited on a watch outside a call (its own load or store
  met a watch) was acted on only at its next system call; a thread alone that made none never
  acted on it. Now `wake_thread` puts such a thread, when the watches change and a signal is
  pending for it, among those that act on their signals, so it does before it runs the
  instruction again or makes any call, as the document says. Test
  `a_thread_alone_acts_on_a_signal_sent_during_its_watch_wait_once_the_watches_change`
  (tests/edges.rs, guest/edges.c `alone`: the hook log must read watched, accessed, sent,
  unwatched, then at once the delivery, and the output equals the reference's, where the main
  thread sends the signal itself).
- Blocking, an end could cover two announced starts. What the kernel counts as told now stays
  the starts it announced less the ends (the merged picture of what is mapped is no longer put in
  its place), so an end is always the whole or a part of one start: two mappings side by side, a
  mapping and what `mremap` grew it by, end as two hooks. The document says so and no longer
  says "exactly a range". Test
  `every_end_of_shared_memory_lies_in_one_start_and_each_step_matches_proc_maps` (edges.c `ends`:
  side by side, grown, covered in the middle by shared anonymous memory, a hole; for a file and a
  memory file; each end must lie inside one start, and each step's pages equal the reference's
  /proc/self/maps).
- Minor, a read-only mapper was not announced while the only writer's context was away. Now
  `shared_mapped` also counts writable mappings in the maps of contexts off the kernel that still
  live (`live_maps_away`). Test `a_read_only_mapper_is_announced_while_the_only_writer_is_away`
  (edges.c `writer`: the parent is taken off between steps while its child maps the file
  read-only, and put back after the child ends).
- Minor, our /proc/<pid>/maps joins two shared mappings of consecutive pages of a /tmp file made
  side by side; the reference shows two lines. Not changed (the kernel track's map merging, not
  the interface): the reference keeps them apart because /tmp there is on overlayfs, whose mmap
  puts the real file in the mapping after the merge check; for a memory file the reference joins
  them too (edges.c `ends`, steps file-adjacent and memfd-adjacent). The new test compares each
  step page by page and the other lines exactly. Listed for the kernel track below.
- Minor, nanosleep interrupted at once answers the whole request left. Not changed: the time left
  is computed from the embedder's clock (`Host::monotonic_nanoseconds`), read when the call
  checks; the test host's clock is virtual and moves only with instructions run, so no time
  passes between the call's start and its check. An embedder on a real clock answers less, as
  Linux does. Adding time the clock did not show would be faking it.
- Minor, the memory id test could not fail for a forked child given its parent's id, and did not
  cover `CLONE_VM`. New test `a_child_shares_its_parents_memory_id_only_while_it_shares_the_memory`
  (edges.c `memory`: a fork child must differ from its parent, a vfork child and a `CLONE_VM`
  child must equal it, the vfork child's program must be new). The old test stays.
- Minor, no task runs on a second kernel: the document now says what is tested (a second kernel
  value accepts a context and tasks, its state is checked there, they come back) and that no test
  runs a task there; the ruling is one kernel. No test added.
- Minor, the speed comparison's pitfall: recorded under "Measurements" (S3 run 9), and this run's
  comparison builds both binaries alone.
Each of the three new tests of the fixed findings fails without its fix (ARM machine, the kernel
crate's changes stashed, scratch streams/run9/negative1.log): "end 0x0 36000000..36002000 lies in
no one start", "step reader ... left: {} right: {905986048: 0}", and "worker handled 0 capped
1" with the delivery after the worker's next call.

## How to run

- The embedder's tests: `RUST_TEST_THREADS=3 nix develop -c cargo test -j 4 -p
  wcontainers-kernel-test --test embedder` (Docker must be running for the lazy memory test).
  Our side runs under `SCHEDULE_SEEDS` seeds (3 if unset) from `SCHEDULE_SEED` (1 if unset).
- The stream tests: the same with `--test streams`; S3's tests: `--test embedder --test away
  --test edges`.
- The literal check: `RAYON_NUM_THREADS=3 nix develop -c cargo run -q -p wcontainers-literals --
  check` (exit 0).
- Everything: `RUST_TEST_THREADS=3 nix develop -c cargo test --no-fail-fast -j 4 -p wcontainers-abi
  -p wcontainers-kernel -p wcontainers-kernel-test -p wcontainers-literals`.

## S2

What exists (signatures and firing points in docs/embedder-interface.md):
- kernel: `System: Host + Sync` (system.rs) with `allocate_task_id`, `free_task_id` (no default),
  `intercept_system_call_request` and `_response`, and the seven hooks (`on_process_starting`,
  `on_program_starting`, `on_thread_starting`, `on_process_exiting`, `on_process_outgrown`,
  `on_watched_memory_accessing`, `on_memory_written_by_kernel`), all taking `&self`; `TaskId`;
  `SystemCallResponse`; `Process` with `id`, `info`, `execution_config`, `read_memory`,
  `write_memory`, `watch_memory`, `watch_thread_memory` (refused by name), `take_dirty_memory`;
  `ProcessInfo` with `memory_mapped`, `ceiling_distance`, `compute_used`, `parent`;
  `Kernel::process(cpu, id)`; `WatchesFor`, `AccessKind` (with `Execute`), `MemoryError`,
  `ThreadOrdering`, `ReplayDeterminism`, `ExecutionConfig`, `Unsupported`. `Start` has `ceiling`
  and `execution`; `Kernel::start` returns `Result<(), Unsupported>`.
- abi: `SystemCallRequest` (was `Syscall`), `SYSTEM_CALL_ARGUMENT_COUNT`,
  `SYSTEM_CALL_NUMBER_REGISTER`; the stream services are `send_stream`, `credit_stream`,
  `close_stream`.
- abi: `PerThreadWatch`, the subtrait of `Cpu` for watching one thread (declared, implemented by no
  executor).
- watches and the record of written pages live in each `AddressMap` (memory.rs `Watches`), so a
  fork's child inherits watches with the map; the CPU side's permissions are narrowed page by page
  where they are set (`apply_permissions_in`). `syscall/touch.rs` describes what every built call
  touches and what thread ends touch; task.rs walks it before a call, signal.rs before a signal or
  an rseq check; `Guest::accessible` checks each copy against the description and treats a watched
  page as not there; a program's fault on a watched page announces the hook.
- test host: `driver::TestHost<H>`, its state and the test's hooks each behind a `Mutex`: with
  `hooks::NoHooks` (every test but the embedder's) it overrides nothing of `System`, so the kernel's own provided defaults answer; with
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
  Both pass. Under ceilings below 128 MiB and against whole programs: see "S2 seventh check
  findings" (three tests, passing on the ARM machine).
- A hook called while another runs is reported, not waited on:
  `a_hook_called_while_one_runs_is_reported_as_nested`. A successful exec and a handler's return
  answered to the proxy: `a_new_program_and_a_handlers_return_are_answered_as_at_linuxs_exit_stop`.
- Watches met before calls write, inside the hook and left unmet and met later (between steps,
  through `Kernel::process`), the result unchanged:
  `a_watch_is_met_before_a_call_writes_and_the_result_is_unchanged`, 51 children each making one
  call into a watched buffer (the output first compared with the reference's), inputs put there by the parent before the child starts: read, readv
  (two pages), recvmsg (a datagram, which Linux loses at a fault), a handler's frame on an
  alternate stack in the buffer, pread64, getcwd, uname, fstat, pipe2, wait4, getrandom,
  clock_gettime, setitimer, sigaltstack, recvmmsg, msgrcv, sendfile, timerfd_settime,
  timer_settime, pselect6, getsockopt, ioctl FIONREAD, getgroups, prctl, getresuid,
  sched_getaffinity, rt_sigtimedwait, splice, copy_file_range, times, getitimer, statfs,
  readlinkat, capget, epoll_pwait, waitid, getrlimit, fcntl F_GETLK, and the kernel's stores of
  `CLONE_PARENT_SETTID`, `CLONE_CHILD_SETTID` and `CLONE_CHILD_CLEARTID`, and (fifth check) raw
  recvmsg with its header on a page apart from its data (a datagram, and a stream read in part),
  raw recvmmsg with its header and its length on two pages, sched_setattr refused for its size
  and capset refused for its version, and (sixth check) a memory sharer killed by SIGKILL and by
  SIGTERM, its clear-tid word in the buffer, and (eighth check) a vfork child with a clear-tid
  word that runs a program, a vfork child holding a robust lock that runs a program, and
  `process_vm_writev` on the process itself. Each call announced the
  hook in exactly one process, at an address in the buffer, as a write, with the page as it was
  when the process started; the output is the same as with no watch; nothing counted outside a
  description, met at a copy or cut short; and at each process's end every page of the buffer
  whose bytes changed was announced in it or in its vfork child. Passes both ways.
- A fault's span: `an_access_that_ends_before_a_watched_page_does_not_announce_it` (with a
  watch on the store's own page as its control). Passes.
- `on_memory_written_by_kernel` exact: `the_kernels_writes_are_announced_exactly`. The proxy:
  `the_embedder_answers_and_changes_system_calls`. Both pass (see the sixth check's findings).
- `take_dirty_memory` (was `memory_changed`) exact:
  `the_pages_changed_are_exactly_those_a_program_wrote_or_mapped_anew`: a child writes pages 0, 2
  and 5 of a buffer itself and page 7 by read(), and maps new memory with `MAP_FIXED` over page 3
  (writable) and page 4 (read-only); the record over the buffer is exactly [0, 2, 3, 4, 5, 7].
  Passes.
- `memory_watch`'s cost: `watching_a_vast_range_costs_its_mapped_pages_not_its_length`: watching
  and unwatching 0 to 2^48 at a program's start takes under a second (the program's output
  unchanged). Passes.
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

## S2 third check findings, settled in the sixth run

- [blocking] An attribute change dropped watches and the record: `mprotect`, `pkey_mprotect`,
  `mlock`, `mlock2`, `munlock` and `madvise`'s fork advice took their range out of the table
  through the same call as `munmap`, which forgets the pages' watches and record. They now split
  the mappings without forgetting (memory.rs `split_out`); only unmapping and mapping over a page
  end its watch. Test `watches_and_the_record_stay_through_attribute_changes` (embedder.c
  `attributes`: seven changes, each in its own child, over the buffer's first three pages, the
  first written before the change, the second lazy (saved, cleared, watched All) and read after
  it, the third watched for writes and written after it): ours with no hooks prints what the
  reference prints (`<change> 0 read x` for each), the hooked run prints the same, each child meets
  exactly [read of the lazy page, write of the third], and each child's record is exactly pages
  [0, 1, 2]. Without the fix every change printed `read \0` (checked once at seed 1).
- [blocking] `memory_changed` lost pages written before such a change: the same fix; the same
  test's record.
- [blocking] Lazy memory over a page whose attributes change: the same fix; the same test's lazy
  page is read correctly after each of the seven changes.
- [minor] `moved_here` told the CPU side the moved pages' permissions again with nothing watched:
  it now does so only when the process has a watch or a record.
- [minor] The report of which processes share memory, and shared pages marked watched when one
  leaves: not built; named in docs/embedder-interface.md's "Not built" list (it comes with
  `suspend`) and above.
- [minor] A wake made between steps is settled only when the embedder answers `Idle` at time 0:
  the document now says an embedder that resolves watches between steps must answer it at once;
  listed for the owner.
- [minor] `ProcessInfo::compute_used` and `ceiling_distance` checked by value:
  `the_distance_to_the_ceiling_and_the_compute_used_are_the_processes_own` (embedder.c `spend`: a
  child maps 1 MiB, spends CPU time and prints its `CLOCK_PROCESS_CPUTIME_ID`): with and without a
  ceiling, the distance at its end is exactly 1 MiB less than at its start, its compute used at
  its start is zero, and at its end at least what it printed, which is above zero.
- [minor] `getrusage`'s `ru_maxrss` in a forked child (ours above 0, the reference 0): the kernel
  track's; listed for it under "For the orchestrator".

## S2 fourth check findings, settled in the seventh run

- The owner's ruling (`&self`, `System: Sync`), first and as its own commit (2cc763e8): every
  method of `System` and of the abi's `Host` takes `&self`; kernel calls take `&dyn System`; the
  test host keeps its state behind a `Mutex` (`HostState`) and the test's hooks behind another
  (`hooks::Hooks: Send`; the embedder tests' records are `Arc<Mutex<..>>`). Kernel unit tests,
  embedder (15) and streams (20) passed after it (scratch streams/s2e/selfref.log).
- [blocking] A watch set inside `on_process_exiting` was skipped by the thread-end stores that
  came after the hook. `end_process` now does each thread's robust list and `CLONE_CHILD_CLEARTID`
  store first (as Linux's `mm_release` comes before the exit is told), then announces the hook, so
  a watch set then has nothing of the kernel's left to meet. The first process's stores now come
  before the others are killed, as in Linux, and agree with what its walk computed. Test
  `a_watch_set_as_a_process_exits_changes_nothing_the_program_sees` (the 41 calls with a host that
  watches the buffer only from `on_process_exiting`): output the same as with no hooks (`child
  cleartid 1`), nothing counted. With the old order it printed `child cleartid 0` (checked once).
- [blocking] The walk before a call did not start over when a hook raised a watch behind it. Each
  memory's `Watches` counts raised watches (set, or widened to `All`); `Kernel::walk_watched` runs
  a call's walk, a signal's and the rseq check again from the start until a pass meets no raised
  watch. Test `a_watch_raised_behind_a_calls_walk_is_met_before_the_call_acts` (embedder.c
  `scatter`: recvfrom into page 1, its address into page 2, the length from page 3; the host
  watches page 1 and at each hook ends that page's watch and puts `All` on the next): ours with no
  hooks prints what the reference prints (`recvfrom 3 one len 0 next two`), the hooked run the
  same both inside the hook and between steps, hooks exactly [page 1, page 2, page 3, all writes],
  nothing counted. Without the restart it printed `recvfrom 3 two len 0 next ` (checked once).
- [minor] The `&self` ruling: done first (above).
- [minor] Clippy on Linux: `openpty`'s size is passed as `&raw mut size` (Linux takes a constant
  pointer, macOS a mutable one). Clippy with warnings denied passes on the ARM machine and here.
- [minor] `memory_read` of a range that ends before it starts, and `memory_write` past the last
  address, are refused with the new `MemoryError::InvalidRange`; `memory_watch` acts on no page
  for them. Checked inside every hook of the order tests.
- [minor] The walk is a second description of what each call touches, checked only while
  something is watched: kept, and put to the owner (see "For the orchestrator"). What guards it
  today: the 41 calls, the 17 shell scripts under lazy memory and the attribute and move tests
  assert no copy outside a description.
- [minor] Speed: the check's ARM measurement backs the earlier claim; nothing to settle.
- The owner's rename of 2026-11-19: `memory_changed` is `memory_take_dirty` (bf36978b, its own
  commit; the address map's `take_changed` is `take_dirty`).

## S2 fifth check findings, settled in the eighth run

- [blocking] recvmsg's header was described as read only, though the call writes its
  `msg_namelen`, `msg_controllen` and `msg_flags` back after taking the data; the same for
  recvmmsg's headers, `sched_setattr`'s size on E2BIG and (found by the new check) `capset`'s
  version on EINVAL. The walk now meets those fields as writes (`message`, `sched_attributes`,
  `capability_header` in syscall/touch.rs; capget's header now as precisely as capset's), and
  the copy check takes the access into account: a write must lie in a range described as written
  (`Watches::check_described`). With the old descriptions and the new check, the calls test
  counted copies outside the description for recvmsg, recvmmsg, sched_setattr and capset (212,
  243, 274, 91), and with raw calls the blocking datagram recvmsg lost its datagram and waited
  for ever (scratch streams/s2f/unfixed-raw.log). With the fix the 46 calls pass at seeds 1 to 3.
  The C library (musl) copies a msghdr to its stack and back and clears recvmmsg's padding in
  place, so the program's own stores met the header page first; the test makes these calls
  directly. The kernel's write-backs in recvmsg and the mmsghdr size and `msg_len` offset now
  come from the structures (`offset_of!`, `size_of`), three baseline lines fewer.
- [minor] `memory_take_dirty` left out pages replaced by a new mapping: every new mapping (mmap,
  `MAP_FIXED` over mapped pages, heap and stack growth, mremap's growth) is now in the record
  whatever it allows, as Linux marks a new mapping soft-dirty. The record is now a
  `rangemap::RangeSet` (already a dependency of the kernel), so a large new mapping costs one
  range. Test above ([0, 2, 3, 4, 5, 7]).
- [minor] A fork's task id could be left unfreed when the executor failed after it was taken:
  make_child now copies the memory and makes the thread first and takes the number last, as
  `copy_process` takes it after `copy_mm`; a failure frees what the executor made
  (`memory_drop`, `thread_drop`), and make_thread drops its new thread when no number comes. Not
  tested: the stand-in's failures cannot be forced from a test.
- [minor] `memory_watch` cost followed the range's length: ending watches now visits only the
  watched pages of the range, setting them only the mapped ones. Test above.
- [minor] Speed: measured again (see "Measurements", eighth run): the memory test binary's user
  CPU time is about 0.3 % higher from the `&self` commit on, and unchanged by everything after
  it, this run's fixes included. That commit put the test host's state behind a `Mutex` taken on
  every `Host` call (the owner's ruling requires `Sync`); the kernel's no-hook path changed there
  only from `&mut dyn` to `&dyn`. Which of the two the 0.3 % is was not measured.

## S2 sixth check findings, settled in the ninth run

- [blocking] A fatal signal from another process (`kill`, `tgkill`, `tkill`, a group) ended its
  target at once in the sender's call, and the stores at the target's end (its threads' robust
  lists and `CLONE_CHILD_CLEARTID` words) were made with no walk: a watched word was dropped, and
  the hook came after the call, in the sender. Now a fatal signal to a process with watched
  memory is left to the process's own threads, as Linux's threads end themselves on their way
  back (`SIGNAL_GROUP_EXIT`): the process is noted dying (`Kernel::dying`), each thread is woken
  from any wait but a watch's (a stopped process is no longer stopped), and the first to go back
  (`deliver`) walks the ends' stores, waiting on a watch that stays, then ends the process. None
  of its threads runs its program or a call again (`system_call` and `wake_thread` send them to
  their signals first). A process with no watched memory still ends at once, so nothing changes
  without watches. Test: two calls added to the calls test (`kill_sharer` in embedder.c: a
  `CLONE_VM` process of its own waiting in `ppoll` on its own stack, killed with SIGKILL and with
  SIGTERM, its clear-tid word in the buffer), now 48 calls; the calls test also compares the
  program's output with the reference's now. With the old kernel it failed (output differs,
  `copy outside the call's description` and `call short of watched memory` counted for 129,
  kill; scratch streams/s2g/unfixed.log); with the fix it passes at seeds 1 to 3, inside the
  hook and between steps.
- [minor] The owner's rulings of the afternoon of 2026-11-19, applied: methods verb first
  (`read_memory`, `write_memory`, `watch_memory`, `watch_thread_memory`, `take_dirty_memory`,
  `allocate_task_id`, `free_task_id`, `send_stream`, `credit_stream`, `close_stream`);
  `free_task_id` has no default (the test host with no hooks implements it doing nothing);
  `WatchesFor`, `AccessKind { Read, Write, Execute }` (an instruction fetch on an `All` page is
  now `Execute`), `on_watched_memory_accessing(.., kind)`, `watch_memory(range, watches)`;
  `on_memory_written_by_kernel`; the system call proxy. Tests:
  `the_kernels_writes_are_announced_exactly` (embedder.c `kernelwrites`: a child's read into the
  buffer, its clock_gettime, a vfork clone's `CLONE_PARENT_SETTID` word, the vfork child's
  clear-tid word at its end, a killed sharer's clear-tid word, each exactly, each to the process
  the memory belongs to and its thread; then a signal's frame, inside the alternate stack) and
  `the_embedder_answers_and_changes_system_calls` (embedder.c `proxy`: getpid answered by the
  embedder, every write to standard output one byte shorter by a changed request, getuid's
  answer changed). Both pass at seeds 1 to 3.
- [minor] The cost statement: the document now says that every copy, watched or not, takes its
  address map's watches and finds no description and no watched page, and that a thread's way
  back pays one more test (whether a fatal signal waits on watches).
- [minor] The span test had no control: it now also watches the store's own page, which must
  announce exactly the store's address, beside the next page, which must announce nothing.
- [minor] `semctl` GETALL through the copy check: as documented; nothing to settle.

## S2 seventh check findings, settled in the tenth run

- [blocking] An answer of `allocate_task_id` outside 1 to `pid_max` panicked (the refusal went
  through `Counters::refuse`, which only takes reasons of `NOT_BUILT`). Now counted (`task id out
  of range`) and the fork fails with EAGAIN. Test `a_task_id_out_of_range_fails_the_fork_with_eagain`
  (answers 0, then `u32::MAX`, at the shell's fork: `sh: can't fork: Resource temporarily
  unavailable`, counted once).
- [blocking] A ceiling below 128 MiB underflowed the mmap base. The gap below the stack's top is
  now Linux's own (`mmap_base`: 128 MiB, at most five sixths of the top; memory.rs `mmap_gap`,
  with a unit test). Test `a_small_ceiling_is_kept_and_a_first_program_past_it_is_refused`: under
  a 100 MiB ceiling the program runs and maps 4 MiB pieces until the hook fires and mmap fails with
  ENOMEM; under a 16-page ceiling the start fails with ENOMEM.
- [minor] A first program that does not fit was neither announced nor counted (the container just
  died of SIGSEGV). The loader now checks the whole new program (segments, interpreter, vDSO,
  where it will put them; loader.rs `fits_below_ceiling`) before its point of no return: an
  `execve` that does not fit fails with ENOMEM and the old program carries on (announced,
  counted; test `a_new_program_past_the_ceiling_is_refused_before_the_old_one_goes`, guest
  bigdata.c with 128 MiB of zeroed data under the 100 MiB ceiling, and it runs with none), and the
  container's first program is counted and warned and fails the start with ENOMEM, with no hook,
  since its process is announced only once its program is loaded (stated in the document).
- [minor] The cost with no hook implemented (two dynamic calls a system call, a noted range a
  copy): not changed. The owner accepted the kernel-write hook as the zero-cost choice; making
  the kernel generic over `System` is his decision (listed for him). The check measured no
  difference from before S2; the document now says so.
- [minor] The nesting check could hang rather than fail: the test host now takes the test's hooks
  with `try_lock` and panics "a hook nested in another"
  (`a_hook_called_while_one_runs_is_reported_as_nested`, a should-panic test that calls a hook
  while holding them).
- [minor] The plan for shared memory was out of date: the document's "Not built" now names the
  evening ruling's two hooks, `MemoryObject` and the branded ids, the getter for a process's memory
  id that `WorkloadPlacement` needs, `ProcessState` and signals as an input.
- [minor] A successful `execve` had no response: now built rather than listed. A call that
  replaces its thread's registers answers with the new x0, and the embedder's answer goes into
  x0: `execve` answers 0 in the new program after `on_program_starting`, to the thread carrying
  the process's number (a threaded exec included); `rt_sigreturn` answers the x0 it restored
  (task.rs `answer_in_registers`). A call run again after a watch is met is asked again as a new
  call, as before. Test `a_new_program_and_a_handlers_return_are_answered_as_at_linuxs_exit_stop`
  (embedder.c `paths` and `kernelwrites`).
- [minor] A thread waiting on a watch the embedder never resolves cannot be killed: kept, and
  its consequence stated under "Departures" and for the owner.
- [minor] The ninth run's "tree before S2 measured 4 % slower" did not reproduce: the check
  measured the two trees alike (see "Measurements").

## S2 eighth check findings, settled in the eleventh run

- [blocking] `process_vm_readv` and `process_vm_writev` on a task of the caller's own process
  copied the other side with no watch met, no hook and no count. The walk (syscall/touch.rs) now
  meets the other side's segments, tags stripped, as read or written, when the task is the
  caller's own; the copy checks the other side's watches and description as it checks the local
  side's (memory.rs `Watches::copy_reach`, used by `Guest::accessible` too). Tests: the calls
  test's `process_vm_writev` (a Writes watch from the child's start), and
  `a_programs_own_memory_reached_by_the_process_vm_calls_meets_its_watches` (embedder.c `vm`:
  under lazy memory a page of data the program never touches is written by one child and read by
  another; filled inside the hook and between steps, the reference's output). Before the fix
  (kernel changes stashed, ARM machine, scratch streams/s2k/before-fix.log): ours
  "process_vm_writev 8 \nprocess_vm_readv 8 \n" against the reference's "written!" and
  "initiali".
- [blocking] `/proc/<pid>/cmdline` and `environ` read a process's memory with no watch met. A
  read of such a file (`read`, `pread64`, `readv`, `preadv`, `preadv2`, `sendfile`, `splice`:
  touch.rs `read_descriptor`) now meets, before the caller's buffer, the strings it copies in the
  memory of the process the file names, whichever process that is; the hook names that process
  and the reading thread, and a thread waiting on another process's watch waits on that
  process's memory and is woken when its watches change or it ends (or runs a program). The read
  itself treats a watched page as not there and is checked against the description, as a call's
  copies are; the call checks both memories after it runs. A walk starts again when a hook
  raised a watch in any process (the count of raised watches is now the kernel's). Test
  `a_command_line_read_through_proc_meets_the_watches_of_the_process_it_names` (embedder.c
  `cmdline self|parent LONG`, a 12288-letter argument, under lazy memory, filled inside the hook
  and between steps; for `parent` a child reads its parent's command line and the hook names the
  parent with the child's thread). Before the fix: "cmdline 10040 letters 1926" against the
  reference's "letters 10000" (with the earlier 10000-letter argument).
- [blocking] The document's claims: the item saying another process's memory is met at the copy
  with no hook is gone; the document now says `process_vm_*` on another process is refused
  (ENOSYS, counted) and `/proc/<pid>/mem` does not exist, describes both reads above, names the
  thread an access through `/proc` is announced with, and counts the calls tested.
- [minor] A vfork child with `CLONE_CHILD_CLEARTID` that runs a program left the word. `execve`
  now does Linux's `exec_mm_release` after `de_thread`: the caller's robust list (locks marked as
  their owner's death, waiters woken, priority-inheriting locks handed on) and its clear-tid word
  while another process still uses the memory it leaves, then forgets both. The walk of `execve`
  meets these stores (`Ending::Exec`, was `Others`). The robust list at exec was not checked by
  the check; it was missing too. Tests: two calls of the calls test (`vfork exec cleartid`,
  `robust exec owner died`), each matching the reference and announced before the store. Before
  the fix (only the exec store stashed, s2k/before-exec-fix.log): both 0 where the reference has 1.
- [minor] Two dynamic calls through `&dyn System` a system call with no hook implemented, and a
  noted range a copy: carried to the next milestone, as the orchestrator ruled (the kernel becomes
  generic over its `System` when it implements the interface crates). Measured by the eighth
  check (ARM machine, load 12 to 23): user CPU medians of the memory test binary 11.837 s before
  S2 and 11.872 s at its tip (+0.3 %, ranges overlapping). This run adds one flag test to a call
  with no watch anywhere (`Kernel::watching`, already tested) and nothing else to that path.

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

- Sixth run, after the third check's fixes (2e91a1fb): the embedder tests (15, seeds 1 to 3),
  memory and faults passed on this Mac (scratch streams/s2d/affected-1.log, 09:20 to 09:27, load
  68 to 64; embedder 140 s). The ARM machine could not be reached (arm-run: "taken away or is
  down", 09:31 and 09:33), so the whole suite ran on this Mac, the command in "How to run"
  (scratch streams/s2d/suite.log, 09:28 to 10:39, load 67 at the start, 23 at the end): 597
  tests, 591 passed, 0 failed, 6 ignored. `streams/verdicts.py s2b/combined.log s2d/suite.log`:
  every test of the fourth run's whole suite has the same verdict; the six tests added since are
  new and pass.
- Speed with no hook implemented, sixth run (2e91a1fb against 59056384, before S2), this Mac:
  threads, processes, memory, sigframe and shell, both built into this track's one target folder
  (59056384 from a source-only scratch worktree, its five binaries copied aside, then this tree
  rebuilt; S2's counter text found only in the new binaries), so both sides share one guest
  folder; run alternately with 3 test threads inside `nix develop`, a warm-up round and three
  counted (scratch streams/s2d/timing-6.sh, timing-6.log, 10:42 to 11:11, load 29 to 63; summary
  by s2b/timing-summary.py). Medians, old then new, wall and CPU (user plus system): threads 46.6 /
  56.7 s wall, 23.6 / 24.6 s CPU; processes 10.3 / 9.3 s, 1.66 / 1.63 s; memory 58.4 / 63.3 s,
  8.79 / 8.58 s; sigframe 4.4 / 4.7 s, 0.42 / 0.41 s; shell 87.4 / 88.9 s, 8.11 / 8.14 s. All
  passed. Threads' wall looked slower, so three more rounds of it alone, new first (timing-7.log,
  11:12 to 11:16, load 29 to 43): new 43.3, 54.8, 51.7 s, old 46.3, 55.1, 47.8 s; CPU new 22.6,
  23.5, 23.0 s, old 23.9, 22.4, 22.1 s. Over the six rounds: wall medians new 55.3 s, old 47.2 s,
  ranges overlapping (new 43.3 to 66.3, old 46.3 to 56.9); CPU medians new 23.4 s, old 23.2 s. The
  kernel runs inside the test process, so its work is in the CPU time, which is unchanged within
  1 %; the wall difference is not explained by it (the tests also wait on Docker's reference).
  Measured only on this loaded Mac; a quiet machine would settle the threads wall time.

- Seventh run, after the fourth check's fixes (bf36978b; the commits after it are documents):
  the embedder tests (17, seeds 1 to 3), faults, memory, orphans, processes, rseqabort, shell,
  sigframe, threads and zombie passed on this Mac (scratch streams/s2e/fixes-1.log, ending 12:42,
  load 30; embedder 101 s). Clippy with warnings denied over the three crates passed on the ARM
  machine (s2e/clippy-arm-2.log) and here. Whole suite on the ARM machine, the command in "How to
  run" with `-j $ARM_JOBS` (s2e/suite-arm.log, 2312 s, 12:44 to 13:22, machine load 40 falling to
  21): 565 passed, 28 failed, 6 ignored; the 28 are the 26 of the manual's list plus
  `terminal::splice_holds_the_pipe` and the literal check's `the_workspace_keeps_to_its_baseline`,
  as in the fifth run. Their 20 binaries passed on this Mac (s2e/mac-only-4.log, 12:54 to 12:57,
  load 20 to 98; an earlier try, mac-only-3.log, failed at `docker create` when Docker's socket
  closed, nothing of the kernel), and the literal check's tests and the check passed here
  (s2e/literals-mac.log, 13:23, load 115; 7589 baseline lines). Combined (s2e/combined.log): 599 tests, 593 passed, 6 ignored,
  none failed. `streams/verdicts.py s2d/suite.log s2e/combined.log`: every test of the sixth run's
  suite has the same verdict; the two tests added are new and pass. Speed not measured again (the
  fourth check measured it on the ARM machine at 319aeede; the no-hook path is unchanged since).

- Eighth run, speed with no hook implemented, on the ARM machine: the memory test binary
  (tests/memory.rs) built from three trees, each in its own fresh build directory with only that
  binary's guest programs: 319aeede (before the `&self` commit, S2 otherwise built), 2cc763e8
  (the `&self` commit) and 0ea3a743 (this run's fixes), run alternately with 3 test threads
  (scratch streams/s2f/speed.sh; speed-self.log, 23:11, load 24 to 42, and speed-self-3.log,
  23:23, load 32 to 46). User CPU, seconds: 319aeede 11.876, 11.852, 11.879, 11.841, 11.875,
  11.882, 11.874, 11.847, 11.853 (median 11.86); 2cc763e8 11.899, 11.882, 11.907, 11.861, 11.913,
  11.857, 11.933, 11.893, 11.917 (median 11.90); 0ea3a743 11.965, 11.939, 11.909, 11.886, 11.885
  (median 11.91). Wall 15.4 to 16.8 s on all three, overlapping. A first try that ran the
  current tree from the track's shared build directory (speed-self-2.log) showed 13.4 s user and
  0.8 s system: its guest folder held 151 programs from whole suites against 1, the effect found
  in the fourth run; not the kernel.
- Eighth run, tests: the embedder tests (18, seeds 1 to 3) and the kernel's unit tests passed on
  the ARM machine (scratch streams/s2f/embedder-3.log and the vast watch test, 23:05 to 23:10,
  load 16 to 35). Clippy with warnings denied over the three crates passed on the x86 machine
  (s2f/clippy-x86.log, clippy-x86-2.log). The 20 test binaries the manual keeps on the Mac passed
  here, 94 tests (s2f/mac-only.log, 15:11 to 15:13 Pacific, load 8 to 14), and the literal check's
  tests and the check passed here (s2f/literals-mac.log).
- Eighth run, whole suite on the ARM machine at 00b623f4's tree (the command in "How to run"
  with `-j $ARM_JOBS`; scratch streams/s2f/suite-arm.log, 2214 s, ending 00:04 UTC, machine load
  about 20 at the end): 567 passed, 27 failed, 6 ignored. The 27 are the 26 tests of the
  manual's list that differ there and the literal check's `the_workspace_keeps_to_its_baseline`
  (no git there); `terminal::splice_holds_the_pipe` passed there this time. All 27 passed on this
  Mac (above). Combined (s2f/combined.log): 600 tests, 594 passed, 6 ignored, none failed.
  `streams/verdicts.py s2e/combined.log s2f/combined.log`: every test of the seventh run's suite
  has the same verdict, but the dirty record's test, renamed and passing; the vast watch test is
  new and passes.

- Ninth run, speed with no hook implemented, on the ARM machine: the memory and processes test
  binaries built from three trees, each in its own fresh build directory: 59056384 (before S2),
  f5f8b8b9 (the eighth run's end) and this run's tree (a8629893's sources), run alternately
  with 3 test threads, round 0 a warm-up (scratch streams/s2g/speed.sh, speed.log, 01:24 to
  01:29 UTC, machine load 27 to 31). User CPU, seconds, rounds 1 to 5: memory before S2 12.271,
  12.304, 12.297, 12.538, 12.316 (median 12.30); eighth 11.739, 11.785, 11.781, 11.772, 11.782
  (median 11.78); ninth 11.842, 12.159, 11.835, 11.846, 11.841 (median 11.84). Processes: before
  0.339, 0.337, 0.320, 0.293, 0.315; eighth 0.304, 0.303, 0.299, 0.322, 0.332; ninth 0.301, 0.334,
  0.340, 0.299, 0.296 (within noise). Wall, memory: before 15.65 to 16.18 s, eighth 15.12 to
  15.29 s, ninth 15.21 to 15.60 s. So this run's tree costs about 0.5 % more user CPU than the
  eighth's on the memory binary (four rounds of five 0.05 to 0.07 s above, one 0.37 s): with no
  hook implemented every system call now makes two calls of empty provided methods through the
  trait object (the proxy) and every copy into program memory notes its range. Not measured which
  of the two it is. The tree before S2 measured 4 % slower than both here, unlike the eighth
  run's comparison (+0.3 %); not explained (that tree also differs in code merged since). The
  seventh check did not reproduce it (below): read the 4 % as a measurement artifact.
- Seventh check, speed with no hook implemented, ARM machine (its scratch s2check7/head/speed7.sh,
  speed7.log; load 56 falling to 22): memory and processes binaries in fresh build directories
  from 59056384 (before S2) and f7d06fc2, run alternately, rounds 1 to 6. Memory user CPU, before:
  11.897 11.925 11.895 11.914 11.897 11.859 (median 11.897 s); f7d06fc2: 11.865 11.909 11.883
  11.864 11.878 11.873 (median 11.876 s). Processes medians 0.309 and 0.306 s. No difference.
  Not measured by this track; the tenth run's changes add one hook call to `execve` and
  `rt_sigreturn` only, and a check on `execve` under a ceiling below the address space's end.

- S3 run 9, tests (tree a98582a9). ARM machine: fmt and clippy with warnings denied over the
  three crates passed; embedder (35), away (12, the known failure ignored), edges (4) and the
  kernel's unit tests (5) passed at 3 seeds (scratch streams/run9/affected.log, 17:23 UTC, load
  26 to 36). The literal check exits 0 with no marks (x86 machine, with a `git init` of the copy
  for its file list; run9/literals.log). Whole suite, in four pieces each its own job and log
  (run9/suite.sh, suite-1.log to suite-4.log, 17:25 to about 17:55 UTC, load about 25 to 40):
  603 passed, 26 failed, 7 ignored (636 tests: run 6's 632 and the 4 new). The 26 by name
  (run9/failed-arm.txt) are run 6's, less `the_workspace_keeps_to_its_baseline` (it passes now that
  the copy has a git index) and with `fdinfo_matches_reference` (its program does not build on the
  ARM machine's zig, as its manual says; it passed there in run 6). Every one is on the manual's
  list kept on the Mac. The 20 binaries holding them on this Mac (run9/mac-only.log, about 09:27
  to 09:30 Pacific, load 3 to 9): 93 passed, 1 failed: `splice_holds_the_pipe`, whose reference
  echoed twice ("abc\r\nabc\r\n" against ours "abc\r\n"), the reference-side flake seen in runs 3
  and 4; run again three times it passed each time (run9/splice-again.log). So no test fails by
  our side.
- S3 run 9, speed with no hook implemented (ARM machine, about 18:05 to 18:15 UTC, load 18 to
  34; run9/speed.sh, speed.log): the memory test binary of 01290816 (the run's base) and of this
  run's tree, each built ALONE under its own name (`--test memory --no-run`), guest folders
  emptied before the build so each holds only that binary's programs (2 MB), run alternately,
  13 rounds, round 0 a warm-up. User CPU, rounds 1 to 12: base 11.799 11.902 12.066 11.912 11.823
  11.928 11.874 12.057 11.821 11.884 11.782 11.826 (mean 11.890, median 11.879); this tree
  11.895 11.938 11.921 11.957 11.932 11.777 11.971 11.928 11.887 11.861 11.943 11.906 (mean
  11.910, median 11.925), faster in 4 of 12. +0.2 % mean, +0.4 % median, inside each side's
  own spread (11.78 to 12.07); no slowdown shown beyond noise. The changed code runs only when a
  thread waiting on a watch outside a call is woken and when shared mappings change.
- Pitfall found by S3's third check: a test binary built inside the whole-suite build (four
  packages at once) ran bimodally (about 1 s more user CPU in 9 of 16 rounds) against the same
  tree built alone. Compare binaries built the same way, alone, as above; it sits beside S2's
  fourth run's pitfall of unequal guest folders. Run 6's comparison was against run 4's tree,
  not S2's; the check's own comparison against S2 (cefb66ab), both built alone, gave -0.1 %.
- S3 run 8, the Mac-only verdicts (this Mac, tree 24f2b127, code as in run 6): Docker Desktop
  started no container at 07:59, 08:02 and 08:05 Pacific and started one at 08:08 (scratch
  streams/s3g/docker-probe.log). The 20 binaries of s3c/mac-tests.txt (built in run 7, nothing
  rebuilt) ran with 3 test threads, 08:09 to 08:11 Pacific, load 3.8 rising to 13.4 (scratch
  streams/s3h/mac-only.log): 94 passed, 0 failed, `splice_holds_the_pipe` among them, as in run
  4. The literal check exits 0 here (7577 baseline lines, 0 marks) and its crate's tests pass,
  the workspace test among them (s3h/literals-mac.log). With run 6's ARM verdicts (599 passed,
  the 26 failures being these 25 tests and the literal workspace test, 7 ignored): 632 tests,
  none failing by our side, 7 ignored (the known failure among them).
- S3 run 6, tests (ARM machine, tree 8ea3508e, nothing changed since): fmt and clippy with
  warnings denied over the three crates passed; away (12 passed, the known failure ignored) and
  embedder (35 passed) at 3 seeds equal the reference (scratch streams/s3f-arm-away.log,
  s3f/s3f-away-embedder.log, 14:21 to 14:25 UTC, load 16 to 30); the known failure still fails
  when run ("read 1" against "read -1 Interrupted system call"). Whole suite (the command in "How
  to run" with `-j $ARM_JOBS` and 12 test threads; scratch streams/s3f-arm-suite.log,
  s3f/s3f-suite.log, 14:25 to 15:03 UTC, load 39 falling to 17): 599 passed, 26 failed, 7
  ignored. The 26 are the manual's 25 kept on the Mac and the literal workspace test (no .git
  there), s3f/failed-arm.txt; against run 4 (s3d) `fdinfo_matches_reference` and
  `splice_holds_the_pipe` pass now and the seven new tests pass, every other name has its
  verdict. Each of the 26 failures prints the same differences as in run 4, line for line
  (thread numbers and the backtrace note aside), `socket_edges_match_reference` among them, so
  the socket timeout change altered none of them. Their verdicts on the Mac are owed: Docker
  Desktop there still created containers that never started (07:07 and about 07:17 Pacific, `docker
  run alpine:3 echo hi`, removed after).
- S3 run 6, speed with no hook implemented (ARM machine, load 12.4 to 21.9, about 15:19 to 15:26
  UTC; scratch streams/s3f/speed.sh, speed.log): the memory test binary of c22154df (run 4's
  tree, as streams-base) and of 8ea3508e, alternately, 13 rounds, round 0 a warm-up, each with a
  2 MB guest folder holding only that binary's programs. User CPU, rounds 1 to 12: before
  12.051 12.060 11.800 11.840 11.807 12.067 12.794 12.089 11.810 12.140 11.883 11.776 (mean
  12.010, median 11.967); after 12.120 11.836 12.097 11.878 11.806 11.887 12.005 13.027 12.279
  11.811 11.815 12.076 (mean 12.053, median 11.946), faster in 6 of 12. Wall means 15.594 and
  15.648 s. No difference beyond noise. A first try (s3f/speed-unequal-guest.log, 15:06 to
  15:13 UTC) showed +4.5 % user and three times the system time: this tree's guest folder held
  the whole suite's programs (257 MB against 1.7 MB), the pitfall noted under S2's fourth run.
- S3 run 5, tests (this Mac, 05:45 to 06:20 Pacific, load 2 to 3): Docker Desktop started no
  container (58 in "Created", `docker run ... alpine echo hi` never returned, from at least 05:45
  to 06:20; the check saw the same from 04:30), the ARM machine could not be reached
  (`arm-run --status`), and the x86 machine cannot run ARM64 containers (exec format error). So
  no verdict against the reference was produced this run. Instead the away tests were run on our
  side against the output written from the program (scratch streams/positive.sh, positive.log,
  8 seeds): the 11 tests of away.rs that do not need two outputs passed (the kill test was not
  run this way); the sharing-hook operations test ran its 3 seeds without a panic, output not
  compared. Unit tests of abi, kernel and literals, fmt and clippy with warnings denied over the
  three crates passed on the x86 machine (scratch streams/s3e/units.log; the literal workspace
  test fails there for want of .git, as its manual says); the literal check passes on this Mac.
- S3 run 5, speed with no hook implemented (x86 machine, load 70 to 96 on 192 threads, other
  tracks' jobs running, 06:25 Pacific; scratch streams/s3e/speed.log): tests/shell.rs
  `runs_repeat_for_a_seed` (ours only) at c22154df and at 6ac2d63b, alternately, 13 rounds,
  round 0 a warm-up, pinned to cores 0 to 9. User CPU, rounds 1 to 12: before mean 4.685 s
  (median 4.70), after mean 4.480 s (median 4.60), after faster in 6 of 12. Noise at this load is
  about 15 % between rounds, so this says only that no slowdown shows; the memory binary used
  before needs the reference and could not run.
- S3 run 4, tests (tree 814edab1 with its tests and docs): on the ARM machine fmt and clippy with
  warnings denied over the three crates passed, the new tests/away.rs passed with 8 seeds (6
  tests, 48 runs; the known failure ignored, and failing when run with 3 seeds: "read 1" against
  "read -1 Interrupted system call"), the embedder tests (34, 3 seeds) and the kernel's unit
  tests (5) passed; the literal check passes on this Mac. Whole suite on the ARM machine (the
  command in "How to run" with `-j $ARM_JOBS` and 12 test threads; scratch streams/s3d/
  suite-arm.log, s3d/fetched/s3d-suite.log, 11:28 to 12:04 UTC, load 6 rising to 22): 590
  passed, 28 failed, 7 ignored; the 28 are by name run 3's 27 (s3d/failed-arm.txt against
  s3c/failed-arm.txt) and `splice_holds_the_pipe`, whose reference echoed twice (the
  reference-side flake run 3 saw on the Mac; it passed in run 3 on this machine and passes on the
  Mac). The new ignored test is the known failure; the six new tests are the five more passes
  with the one flake fewer. The 20 binaries kept on the Mac (s3d/mac-only.log, this Mac, 03:28 to
  03:30 Pacific, load 4.3 to 7.9): 94 passed, 0 failed, `splice_holds_the_pipe` among them.
  Combined: 625 tests, none failing by our side.
- S3 run 4, speed with no hook implemented (ARM machine, scratch streams/s3d/speed.log, load 8.4
  to 16.7, 12:05 to 12:18 UTC): the memory test binary of cefb66ab (S2 as merged, rebuilt this run
  as streams-base, since the earlier build was gone) and of this run's tree, run alternately, 13
  rounds, round 0 a warm-up. User CPU, rounds 1 to 12: before 11.922 11.942 11.915 11.955 11.971
  11.906 11.917 11.927 11.930 11.905 11.906 11.890 (mean 11.924); after 11.792 11.803 11.844
  11.895 11.807 11.798 11.804 11.807 11.801 11.839 11.794 11.794 (mean 11.815, -0.9 %), faster in
  12 of 12. Wall: mean 15.337 before, 15.188 after (-1.0 %). No slowdown; why the tree is
  faster was not measured (run 3's tree, against the same base, was +0.05 %).
- S3 run 3, tests (tree c626d480 with its docs): the suspend test and the new stop test passed
  on the ARM machine (3 seeds; 48 and 6 runs). Clippy with warnings denied over the three crates
  and fmt passed there; the literal check passes on this Mac. Whole suite on the ARM machine
  (the command in "How to run" with `-j $ARM_JOBS` and 12 test threads; scratch
  streams/s3c/suite-arm.log, s3c/fetched/s3c-suite.log, 09:56 to 10:32 UTC, load 32 falling to
  7): 585 passed, 27 failed, 6 ignored; the 27 failures are by name run 2's 27
  (s3c/failed-arm.txt against s3b/failed-arm.txt), the new test the one more pass. The 20
  binaries kept on the Mac (s3c/mac-tests.txt, s3c/mac-only.log, this Mac, 01:59 to 02:00
  Pacific, load 4): 92 passed, 2 failed, `partial_copies_match_reference` and
  `splice_holds_the_pipe`; run again 4 and 10 times, each failed once more, both on the
  reference's side (the reference's urandom read once came up short, 15 bytes for 16; the
  reference's terminal once echoed twice), and passed otherwise. Combined: 618 tests, none
  failing by our side.
- S3 run 3, speed with no hook implemented (ARM machine, scratch streams/s3c/speed.log, load 6.9
  falling to 4.4, 10:33 to 10:40 UTC): the memory test binary of cefb66ab (S2 as merged,
  streams-base, built in run 2) and of this run's tree (built alone), run alternately, 13
  rounds, round 0 a warm-up. User CPU, rounds 1 to 12: before 11.876 11.868 12.083 12.114 11.866
  11.915 11.914 12.077 12.065 12.112 12.067 12.097 (mean 12.005); after 12.062 11.964 11.967
  12.042 12.053 12.036 12.056 12.044 11.942 11.936 12.047 11.977 (mean 12.011, +0.05 %), slower
  in 5 of 12. Wall: mean 15.309 before, 15.288 after (-0.1 %). No difference beyond noise on a
  quieter machine than run 1's and run 2's (load 10 to 31), whose +0.3 % and +0.5 % were inside
  the base's own swing (11.87 to 12.11 here).
- S3 run 2, tests: the suspend test passed on the ARM machine (3 seeds, then 6; with four
  threads and 24 orders, 3 seeds, 48 runs, at cab6b0bf). Clippy with warnings denied over the
  three crates and fmt passed on the x86 machine; the literal check passes on this Mac (exit 0,
  scratch streams/literals-s3r2.log). Whole suite on the ARM machine at 4d2fb664's tree (the
  suspend test then with three threads; the command in "How to run" with `-j $ARM_JOBS` and 12
  test threads; s3b/suite-arm.log, s3b/fetched/s3b-suite.log, 09:00 to 09:36 UTC, load 10 to
  16): 584 passed, 27 failed, 6 ignored; the 27 are by name run 1's 28 less
  `splice_holds_the_pipe`, which passed there this time (s3b/failed-arm.txt against
  s3a/failed-arm.txt). The 20 binaries kept on the Mac (s3b/mac-only.log, this Mac, 01:00 to
  01:02 Pacific, load 5 to 10): 94 passed, none failed. Combined: 617 tests, 611 passed, 6
  ignored, none failed.
- S3 run 2, speed with no hook implemented (ARM machine, scratch streams/s3b/speed.log, load 9.5
  to 12.7, about 08:54 to 09:00 UTC): the memory test binary of cefb66ab (S2 as merged, an
  archive of it at scratch streams/base-s2) and of this run's tree (8a745148 with its docs),
  each built alone, run alternately 9 rounds, round 0 a warm-up (the base's round 0 took 30.8 s
  wall and 42.2 s user while it was still settling, and is not used). User CPU, rounds 1 to 8:
  11.880 12.082 12.105 11.857 12.083 11.935 11.911 11.861 s before (mean 11.964), 12.042 12.035
  12.058 12.032 11.991 11.978 12.025 12.041 s after (mean 12.025, +0.5 %); the new tree was
  slower in 5 of 8 rounds. Wall: 15.167 15.397 15.374 15.185 15.364 15.269 15.294 15.219 s
  before (mean 15.284), 15.337 15.288 15.280 15.320 15.274 15.212 15.362 15.324 s after (mean
  15.300, +0.1 %), slower in 4 of 8. The base swings between about 11.86 and 12.10 from round
  to round; the new tree sits near 12.03. Not shown to be unchanged: a difference of this size
  is within the base's own swing, but the means moved the same way as in run 1. What part 3
  adds on the no-hook path: two stores around each step and each call's request, one test
  after the request, a store around each process's end.
- S3 run 1, tests: the embedder tests (32, seeds 1 to 3) and the kernel's unit tests (5) passed
  on the ARM machine (scratch streams/s3-sharing-5.log, s3-affected-1.log; there `sigframe`
  fails as the manual lists, and passes on this Mac below). Clippy with warnings denied over
  the kernel passed on the x86 machine and over the kernel test crate on the ARM machine; fmt
  clean; the literal check passes on this Mac. Whole suite on the ARM machine at e713b19b's tree
  with the status edits (the command in "How to run" with `-j $ARM_JOBS` and 12 test threads;
  s3a/suite-arm.log and s3a/fetched/s3a-suite.log, 07:45 to 08:21 UTC, load 14 to 17): 582
  passed, 28 failed, 6 ignored; the 28 are by name exactly the eleventh run's
  (s3a/failed-arm.txt against s2k/failed-arm.txt), and the 6 new tests pass. The 20 binaries
  kept on the Mac (s3a/mac-only.log, this Mac, 23:45 to 23:47 Pacific, load 6 to 12): 94
  passed, none failed. Combined: 616 tests, 610 passed, 6 ignored, none failed.
- S3 run 1, speed with no hook implemented (ARM machine, s3a/speed.log, load 15 to 16, about
  08:25 UTC): the memory test binary at cefb66ab (S2 as merged) and at this run's tree, each
  built alone, run alternately 7 rounds, round 0 a warm-up. User CPU 11.874 11.860 11.983
  11.848 11.769 11.872 s before, 11.913 11.903 11.914 11.880 11.912 11.920 s after (means
  11.868 and 11.907, +0.3 %); wall 15.08 15.13 15.28 15.15 15.02 15.08 s before, 15.24 15.21
  15.31 15.19 15.20 15.16 s after (+0.6 %). The new tree was slower in 5 of 6 rounds by user
  CPU and in all 6 by wall time: small, but in one direction, so not shown to be unchanged. What
  the no-hook path gained: a flag test at every announcement (the proxy's two per call) and when
  the kernel settles, a queue test when it settles, a test per signal taken, and a test of the
  mappings an unmap removes. Not yet looked into further.
- Eleventh run, tests: the embedder tests (27, seeds 1 to 3) passed on the ARM machine at
  93d65247's tree with the `processes` tests (scratch streams/s2k/embedder-4.log, load 12 to 23),
  and the kernel's unit tests (4) at 48c90e4c (embedder-3.log); clippy with warnings denied over
  the three crates passed there and on the x86 machine. Whole suite on the ARM machine at
  48c90e4c's tree (the command in "How to run" with `-j $ARM_JOBS` and 12 test threads;
  s2k/fetched/suite-arm.log, 2222 s, ending about 06:08 UTC, load 29 to 40): 576 passed, 28
  failed, 6 ignored; the 28 are by name exactly the tenth run's (s2k/failed-arm.txt against
  s2j/failed-arm.txt), and the two new embedder tests pass. The 20 binaries kept on the Mac at
  the same tree (s2k/mac-only.log, this Mac, 21:32 to 21:34 Pacific, load 4 to 19): 94 passed,
  `splice_holds_the_pipe` among them. The literal check's tests on this Mac: all passed
  (s2k/literals-mac.log). Combined: 610 tests, 604 passed, 6 ignored, none failed. 93d65247 only
  adds a wake at exec, tested by the embedder and processes tests above.
- Eleventh run, speed (ARM machine, s2k/speed-2.log, load 28 to 44, 06:20 UTC): the memory test
  binary of 1bacb2d4 (S2 before this run) and of 93d65247, each built alone with `cargo test -p
  wcontainers-kernel-test --test memory`, run alternately 7 rounds, round 0 a warm-up: user CPU
  12.224 12.207 12.181 12.225 12.218 11.981 s before, 11.984 11.952 11.959 11.992 11.980 11.983
  s after. No slowdown in user CPU. Wall time was not steady: in rounds 5 and 6 the new
  binary took 37.6 s and 47.9 s against 19.8 s and 21.2 s for the old one, while the machine's
  load rose; user CPU in those rounds was the same (11.98 s both ways), so the wall time is most
  likely the load, not the code, but it was not measured apart from it. A first try (s2k/speed.log) timed the whole suite's build of the binary
  against one built alone: 12.3 s user and 0.88 s system against 11.9 and 0.30, every round; the
  difference is the build (the suite's packages build it differently), not the code, and that
  comparison is not used.
- Tenth run, tests: the embedder tests (25, seeds 1 to 3) and the kernel's unit tests (4) passed
  on the ARM machine; clippy with warnings denied over the three crates passed there. Whole suite
  on the ARM machine at ae37b07c's tree (the command in "How to run" with `-j $ARM_JOBS` and 12
  test threads; scratch streams/s2j/suite-arm.log, 2099 s, ending about 04:20 UTC, load 14 to
  18): 574 passed, 28 failed, 6 ignored. The 28 are the ninth run's 27 (the manual's list and the
  literal check's workspace test, no git there) and `terminal::splice_holds_the_pipe`, failed on
  the reference's side (Docker's tty echoed `abc` once more, as seen before). The 20 binaries
  kept on the Mac (s2j/mac-only.log, this Mac, 19:44 to 19:47 Pacific, load 3 to 7): 94 passed,
  `splice_holds_the_pipe` among them. The literal check's tests on this Mac: all passed
  (s2j/literals-mac.log). Combined: 608 tests, 602 passed, 6 ignored, none failed.

- Ninth run, tests: the embedder tests (20, seeds 1 to 3) on the ARM machine (scratch
  streams/s2g/embedder-2.log, affected-1.log; the nesting checks added to the new hooks last,
  nest.log). Clippy with warnings denied over the three crates passed on the x86 machine
  (s2g/clippy-5.log). Whole suite on the ARM machine at a8629893's tree, before the order tests'
  nesting checks (the command in "How to run" with `-j $ARM_JOBS`; s2g/suite-arm.log, 2139 s,
  ending about 01:58 UTC, machine load 23 to 27): 569 passed, 27 failed, 6 ignored; the 27 are
  by name exactly the eighth run's (the 26 of the manual's list and the literal check's
  `the_workspace_keeps_to_its_baseline`, no git there), and every test that passed then passes
  now, with the two new embedder tests. The 20 binaries kept on the Mac (s2g/mac-only.log, this
  Mac, 17:29 to 18:22 Pacific, load 10 to 23): 92 passed, and in `terminal` two failed on the
  reference's side (`splice_holds_the_pipe`: Docker's tty echoed once more, as seen before;
  `job_control`: the reference container hung for 50 minutes and was removed); `terminal` alone
  again passed all 10 (s2g/mac-terminal.log, 18:23, load 8). The literal check's tests passed
  here (s2g/literals-mac.log). Combined: 602 tests, 596 passed, 6 ignored, none failed.

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
- After the sixth run: unchanged, 7589 lines; the check passes (exit 0).
- After the seventh run: unchanged, 7589 lines; the check passes (exit 0).
- After the eighth run: 7586 lines (recvmsg's three header offsets now `offset_of!`); the check
  passes (exit 0). It reads the checkout's git history, so it runs on this Mac. The test host's clock
  step stays `TestHost::ran` (an associated function taking the locked state) so its `1000` keeps
  its key.
- After the ninth run: 7579 lines (the call number register and the argument count now named in
  the abi; the stand-in, `deliver` and `settle` use them); the check passes (exit 0).
- After the tenth run: 7577 lines (the mmap gap's `128 << 20` now `MMAP_GAP_MIN` with Linux's
  `MAX_GAP` beside it); the check passes on this Mac (exit 0, scratch streams/s2j/literals-2.log;
  it needs the git history, which the rented machines' copies lack).
- After the eleventh run: unchanged, 7577 lines; the check passes on this Mac (exit 0, scratch
  streams/s2k/literals-2.log; the new test's argument length is `3 * PAGE_SIZE`, since 10000 was
  taken for a restatement of another constant).
- After S3's first run: unchanged, 7577 lines (wcontainers-kernel 2247, wcontainers-kernel-test
  2422, wcontainers-abi 3); the check passes on this Mac (exit 0). The new code's sizes and
  offsets are derived (`size_of::<SignalInformation>()`, the word size of `c_int`); the signal
  numbers in the tests are the generated constants.
- After S3's third run: unchanged, 7577 lines (same per crate); the check passes on this Mac
  (exit 0, scratch streams/literals-s3r3.log). The literal check does not read the C guests.
- After S3's fourth run: unchanged, 7577 lines; the check passes on this Mac (exit 0).
- After S3's sixth run: unchanged, 7577 lines; the check passes on this Mac (exit 0, scratch
  streams/s3f/literals.log).
- After S3's fifth run: unchanged, 7577 lines; the check passes on this Mac (exit 0).
- The check's list of changes that could add lines: one, `da21d326f277 the start`.
- Still waiting for their owners: reference.rs `125`, `127` (Docker's failure statuses; the abi's
  `exit` module, runtime track) and tty.rs `"/dev/pts/{index}"` (the kernel track's path separator).

## Licences

No dependency added by this track. The literal check's crate brings syn, proc-macro2, quote, toml,
fnv and rayon (all MIT OR Apache-2.0), a test-time tool of the workspace, not a dependency of
anything shipped.

## Departures from the plan

- S3, suspend and resume (choices where the ruling is silent, for the owner): the kernel's calls
  take the executor (`cpu`) as `Kernel::process` does, and return the concrete `vec::IntoIter`
  (an iterator that borrows nothing of the kernel). A task taken in its call answers as Linux's
  stop makes it answer (the brief's 04:00 rule). The interruption is seen by the call when the
  task is put back, not when it is taken: running a call inside a hook would announce hooks
  inside a hook, and the kernel's calls do not take the `System`. Nothing of the task runs in
  between, so this is Linux's stop arriving at the moment of resumption; waiters in a line a
  wake chooses from (futex, semaphore, message queue) leave it when taken, so a wake while away
  never chooses them. One consequence: a write half done whose pipe is emptied while it is away
  writes more before it answers, where a stop on Linux at the moment of taking would have
  answered the count then (both are answers Linux can give). The orchestrator's 04:00 note lists
  `epoll_wait` among calls that start again; Linux answers EINTR there after a stop (signal(7),
  and the reference agrees), so the kernel follows Linux. Inside a hook, a task the kernel is in the middle of a step for cannot be
  taken; it is left out and counted, except at `intercept_system_call_request` (taken before its
  call). `suspend_process(self)` on the handle, when such a task is its own, leaves the process
  and answers a copy of the context with the tasks taken. `copy_process_context` shares the
  records reached by reference (address map, handler table) with the original. `resume_process`
  also hands back a context whose configuration the kernel cannot run.
- S3, known gaps left open: a process on a kernel without its parent has no reaper there (no
  test lets it happen); a wait met while its task was away is found before a signal sent
  meanwhile (the ignored known-failure test in tests/away.rs); `kill` of a process whose context
  is away answers ESRCH (a signal to a process on no kernel of the embedder's knowing is the
  embedder's to carry), while a wait for it waits as for a stopped child.
- S3 run 5: a context the embedder drops instead of putting back lets go of its file blocks at
  the kernel's next release, but its executor memory is left to the executor: once its map is
  gone the kernel cannot tell whether another kernel still runs in that memory. A dropped context
  also leaves its parent waiting for it. Linux has no such case; the ruling has no call to end a
  context.
- S3 run 5: a SIGCHLD for a parent whose context is away is kept on the kernel it left and sent
  when the context comes back to that kernel; a parent resumed on another kernel is not told.
- S3 run 4: `ProcessContext::origin()` and `ContextOrigin` are additions to the ruled interface:
  the handle's `suspend_process` has no room for none, so a process that cannot leave from inside
  a hook answers a copy, and the embedder has to be able to tell.

- S3, signals (choices where the ruling is silent, for the owner): `on_signal_delivering` fires
  where ptrace's signal-delivery-stop is, for every signal a thread takes off its pending sets to
  act on (handler, default action, an ignoring handler); not for signals a call takes
  (`sigwaitinfo`, `signalfd`), ones dropped at sending as ignored, or a fatal signal that ends a
  process from the sender's side (as Linux, untraced). `send_signal` is sent when the kernel
  next settles (after the hook, before any thread runs), since the handle cannot reach the
  embedder for the hooks a signal can cause (a process's end).
- S3, `SignalInformation` is the generated C union `siginfo`, as ruled. Safe Rust can build one
  whose bytes are partly unwritten (a union member smaller than the record); the kernel reads
  the record whole (`_si_pad`, one `unsafe` read with its reason), so the handle's document asks
  for a record built from zero, as Linux's `clear_siginfo`. For the owner: a record that cannot
  be built partly unwritten (bytes, or an enum by layout) would make this sound by type.
- S3, shared memory (choices, for the owner): the hooks describe what a process maps, compared
  with what was announced, not each call; a source announced once stays announced while any
  process maps it, so every announced mapping is announced ended; a private file mapping is not
  announced (the ruling names `MAP_SHARED`), though it shows other processes' writes to the file
  until it writes a page itself; a fork's child is announced its mappings just after
  `on_process_starting`.

- The literal check is taken as its crate's files from 7668c010, not merged (see above). When
  phase-1 or main carries the check with the runtime adapted to the stream ends, a merge brings
  the same files; the baseline will conflict, and docs/literals.md step 4 says how to settle it
  (leave it, `init`, commit).
- Hooks and task ids are on `wcontainers_kernel::System`, which extends the abi's `Host`, not on
  one trait in the abi: the hooks take `Process`, which holds the kernel. The `&self` ruling names
  `System`; the abi's `Host` methods (the clocks, randomness, warnings, the backend, connections
  and the stream ends) take `&self` too, since the naming rule counts them as the `System`'s
  services and a `Sync` trait whose half takes `&mut self` could not be called from two threads. The rename of `Host`
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
- `memory_take_dirty`'s first answer is every page mapped writable (nothing was recorded before);
  writes by other processes to shared pages are not in a process's record.
- Watches act on whole pages of `PAGE_SIZE`. Left to the copy check (listed with why in
  docs/embedder-interface.md): `semctl` GETALL/SETALL and the IPC `*_INFO` commands,
  `restart_syscall`, a priority-inheriting lock handed on at a thread's end, and thread-end
  stores when the container ends. `process_vm_*` on another process is refused (ENOSYS, counted)
  and `/proc/<pid>/mem` is not built.
- A read of another process's command line or environment through `/proc` announces
  `on_watched_memory_accessing` with that process and the reading thread, a thread of another
  process (the interface did not say which thread; this is the task making the access). For the
  owner.
- The first process's end now kills the others in the kernel (Docker kills them from outside), so
  each is announced and its number freed.
- Task ids: the embedder's id carries no generation (the earlier ruling said it would): the
  embedder hands out every number and hears when each is free, so a number it keeps never names
  another process without its knowing. `task_id_allocate` takes `previous`, the id handed out
  last, which the owner's names do not have: the provided default counter needs it, since a
  trait's default method keeps no state. Both for the owner to confirm.
- `on_memory_accessing` before a call is announced for what the call's arguments name, which can
  be more than it then writes (a short read, `nanosleep`'s remaining time).
- `AccessKind` is the kernel's (the ruled name); the abi's `Access` stays for the executor's
  faults, which `AccessKind` follows one to one.
- `on_memory_written_by_kernel` is announced for every copy into program memory (one routine,
  user.rs `Guest`), not for memory changed by mapping calls (reported by `take_dirty_memory`), a
  new program's stack, vDSO and segment tails written by `execve` (the whole new memory is the
  embedder's to read at `on_program_starting`), or backend pages of a mapped file (written into
  the file all mappers share). The owner named "a page of a mapped file being filled" among his
  examples: for him to confirm. Writes are announced after the call, before the next hook and
  before any thread runs again, not inside the copy (a hook cannot be given the process while a
  call holds it).
- The abi's `Syscall` is renamed `SystemCallRequest` (one type, the ruled name), and the register
  holding a call's number is the abi's `SYSTEM_CALL_NUMBER_REGISTER`; the executors construct it,
  so the runtime track's executor adapts.
- A changed request is written into the thread's registers (as ptrace's change at the
  syscall-entry stop), so a waiting call runs the changed one again and the program sees x1 to x5
  changed after the call. A `Break` answer is not given to `intercept_system_call_response`.
- `watch_thread_memory` is my verb-first form of `memory_watch_thread`; for the owner.
- Watch waits are not ended by signals, as backend waits are not. Consequence: a thread
  waiting on a watch the embedder never resolves cannot be killed, SIGKILL included (it acts once
  the watches change); Linux's nearest wait, a userfaultfd fault, can be killed, and a kernel
  store into such a page is then skipped. An embedder that loses a page leaves an unkillable
  process. Listed for the owner.
- From S1, still standing: `a_terminal_closed_before_the_start_finished` does not guard the
  release it was written for (devpts lookup goes by the master's count; no observation found that
  does); `/dev/console` absent under `-t`; a terminal's space toward the slave is
  held for the embedder's credit; bytes the embedder sends after the kernel closed an input end are
  dropped uncounted; output after the embedder closed its end is compared with a pipe whose reader
  the reference program closed itself.

## For the orchestrator

- Run 6: Docker Desktop on this Mac still starts no container (created, never started, at 07:07
  and about 07:17 Pacific; the daemon answers and one container of another project runs). The ARM
  machine gave every other verdict. Owed on the Mac once Docker starts containers: the 20 test
  binaries holding the manual's 25 Mac-only tests and `splice_holds_the_pipe` (the list in
  scratch streams/s3d/mac-only.log).
- Run 7 (07:22 to 07:58 Pacific, Mac load about 2.5): still none. `docker run`, `docker run -d`
  and `docker start` never return; the container stays "Created" and the VM's log shows the
  attach and wait requests but no start answered. A probe tried every 3 minutes, 9 times from
  07:33 to 07:57 (scratch streams/s3g/docker-probe.sh, docker-probe.log), each "created",
  each removed after. I did not restart Docker Desktop (shared; another project's container
  runs). The 20 binaries are built in this checkout's target (s3g/build.log); once containers
  start: `RUST_TEST_THREADS=3 CARGO_INCREMENTAL=0 nix develop -c cargo test --no-fail-fast -j 4
  -p wcontainers-kernel-test $(cat <scratch>/streams/s3c/mac-tests.txt)`.
- Run 8: Docker Desktop started containers again at 08:08 Pacific (nobody in this track
  restarted it); the Mac-only verdicts are given (all 94 passed). Containers of other tracks
  were still in "Created" at 07:58 (wc-x86-reference-63762-0 and -1, wc-guard-probe and three
  unnamed ones); they are not this track's and were left alone.

- For the owner (S3 run 4): `ProcessContext::origin() -> ContextOrigin { Suspended, Copied }` is
  new (names suggested): through a hook's handle, a process whose own thread is in the middle of
  its step cannot leave, and the ruled `suspend_process(self)` has no `Option` to say so.

- For the owner (S3 run 3, the 04:00 rule): with Linux's stop as the model, a suspension is
  visible to a program in the calls Linux answers EINTR after a stop (`epoll_wait`, `semop`,
  `rt_sigtimedwait`) and in a write half done that answers its count; the 04:00 note listed
  `epoll_wait` among calls that start again, but Linux and the reference answer EINTR. The
  interruption is seen when the task is put back, not when it is taken (see "Departures").
- For the orchestrator: Nix's zig 0.16.0 on the ARM machine also mislinks embedder.c once it
  grows (undefined section symbols, as with fdinfo.c); the new test's program is its own file.

- For the owner (design, found by S2's fourth check): the walk in syscall/touch.rs is a second,
  hand-kept description of what each call touches, beside the call's own copies; a difference is
  seen only while something is watched (counted as `copy outside the call's description`), so a
  new call needs a description and nothing checks it while nothing is watched. It is what makes a
  watch fire before a call has any effect. The other way, Linux's, is to fault inside the copy and
  run the call again, which loses what the call did before the copy (a datagram). Kept until he
  rules.
- For the runtime track: the abi's `Host` methods now take `&self` and `System` requires `Sync`.

- For the kernel track (found by S3's third check): /proc/<pid>/maps shows two `MAP_SHARED`
  mappings of consecutive pages of one file, made side by side, as one line; the reference shows
  two for a file under /tmp (overlayfs there keeps them apart) and one for a memory file. Seen
  in tests/edges.rs `ends`, which compares those steps page by page.
- For the kernel track (found by S2's third check): in a forked child `getrusage(RUSAGE_SELF)`
  reports `ru_maxrss` above 0; the reference reports 0.
- For the owner: a watch resolved between steps through `Kernel::process` wakes its thread only
  once the embedder answers `Next::Idle { until_nanoseconds: Some(0) }` with `Event::Timer`, since
  settling may announce hooks and free task ids, which need the `System`; `Kernel::next` does not
  take it.

- For the owner: `task_id_allocate` takes `previous` (the id handed out last), which the
  owner's names do not have; the provided default counter needs it, since a trait's default
  method keeps no state.
- Names for the owner (suggested): `MemoryError::InvalidRange`; `Process::memory_write(address,
  bytes)`; `ProcessInfo::parent`;
  `ProcessInfo::ceiling_distance` (defined as what new mappings can still take below the ceiling);
  `MemoryError::{NotMapped, NotRead, Executor}`; `Unsupported`; `ExecutionConfig::supported`;
  `Start::ceiling`, `Start::execution`; `task_id_after` (the default counter);
  `PerThreadWatch::set_thread_permissions`.
- Carried to the next milestone (orchestrator, 2026-11-19 night): the system call proxy costs
  every call two calls of empty methods through `&dyn System`, and every copy into program memory
  notes its range for `on_memory_written_by_kernel`. The ninth run measured about 0.5 % user CPU
  on the memory test binary; S2's seventh and eighth checks measured no difference beyond noise
  (+0.3 %). The kernel becomes generic over its `System` in a later milestone, after the kernel
  track has merged (the orchestrator's S3 section moved it out of S3); still carried in S3.
- For the orchestrator (S2's ninth check): the rented ARM64 machine's root disk was full (580 of
  581 GB; /work/target held 555 GB of every track's build folders), so builds there failed with
  "No space left on device" at that time; other tracks' failures then may have been the disk.
- For the owner: a thread waiting on a watch the embedder never resolves cannot be killed (see
  "Departures"); Linux's userfaultfd wait can be, skipping the kernel's store.
- For the owner: a container whose first program does not fit below `Start::ceiling` fails its
  start with ENOMEM, counted, with no `on_process_outgrown` (its process is not announced yet).
- For the runtime track: the abi's `Syscall` is `SystemCallRequest`; `Host::stream_send`,
  `stream_credit`, `stream_close` are `send_stream`, `credit_stream`, `close_stream`;
  `System::free_task_id` has no default.
- From S1 (suggested, unchanged): `End`, `Host::send_stream`, `Host::close_stream`,
  `Event::StreamData`, `Event::StreamCredit`, `Event::StreamClosed`, `Event::TerminalResized`,
  `Start::standard`, `Standard::{Pipes, Terminal}`.
- The literal check: phase-1-kernel merged phase-1-literals directly; this branch took only the
  crate. Whoever merges both into phase-1 resolves the baseline once by `init`.
