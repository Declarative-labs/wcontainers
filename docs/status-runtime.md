# Runtime status

Plan: docs/runtime.md. Branch: phase-1-runtime. Brief: pyro-parked/wcontainers-scratch/orchestrator/brief-runtime.md.

## Where things stand

- J0 (the plan), J1 (first light) and J2 (processes and the rest of the boundary) are done; their records are
  below.
- J3 run 1 (2026-11-18): the six findings of J2 run 5's check are settled first (their section below); origin/phase-1
  (the kernel through K6b, 2edbc451) is merged (db247ebf), which needed one back end fix (fff0aa77); then J3
  is built: free turns, the library surface on the OCI runtime configuration, OCI images and plain tars, the
  shipped outside, and the command line program. J3's bar passes (below).
- J3 run 2 (2026-11-18): the nine findings of J3 run 1's check are settled (their section below, with the
  bar run again).
- Asked of the kernel track (next heading but one): standard input, and the values `Start` would need to
  honour the fields refused today.
- J3 is complete but for what waits on others: standard input (the kernel's interface), and the fields
  refused until the kernel takes them.
- The kernel test crate changed in J3 only in the merge's settled conflicts (driver.rs, standin.rs, and
  layer1.rs's call of the driver's `back_end`; db247ebf) and in one line each of driver.rs and standin.rs for
  the free kind of `Turn` (5d86b31d, with the runtime crate, since neither builds alone).
- J4a run 1 (2026-11-18): the six open findings of J3's last check are settled first (their section below);
  origin/phase-1 had nothing new to merge (still 2edbc451). Then J4a, as the orchestrator split J4: its bar is
  run (section "J4a run 1" below). Plan section 16's J4 is corrected to J4a and J4b.
- Waiting on the owner, as ruled: J4b (equal counts across look-ahead settings and on the stand-in), the
  stand-in's count and the loop's count of the kernel's changes of pc (the orchestrator confirmed these wait
  for the progress unit); standard input, output and the terminal (the kernel track builds them on the
  general stream the owner is settling; nothing of input is built here, and `process.terminal` and
  `process.consoleSize` stay refused by name).
- The kernel test crate changed in J4a in tests/repeats.rs: added in run 1 (5492ba0a), and in run 2 made to
  compare the files a container left too (b001897c); and in run 2 in the driver: `Injections::wall_start`, the
  wall clock's reading at the start (the machine's clock when none), which repeats.rs gives every run of a
  seed (87319389); in run 3 repeats.rs gained a program whose threads only compute (6d26eb6f), and the
  driver draws bounded look-ahead up to the translator's bound (105c6463) and implements `System` (the
  rename, 5bec4f91, with the runtime crate since neither builds alone). Each its own commit.
- J4a run 2 (2026-11-18): the five findings of J4a run 1's check are settled (their section below): a dropped
  thread's instances are now given to the next thread given its block (the blocking one). origin/phase-1 had
  nothing new to merge (still 2edbc451). The bar is run again (section "J4a run 2").
- J4a run 3 (2026-11-18): the six findings of J4a run 2's check are settled or answered (their section
  below); the owned crates are swept for bare numbers (section "Bare numbers"); the owner's names of
  2026-11-18 are built, each its own commit, and the plan corrected (section "J4a run 3"). origin/phase-1
  had nothing new to merge (still 2edbc451). The bar is run again (section "J4a run 3").
- Also built in J4a run 3, from the brief's later sections: a property of a config.json the specification
  does not define, and every annotation, is ignored and named for a warning (read through `serde_ignored`),
  in place of being refused (section "J4a run 3").
- J4a is complete: its bar passes at full, none and bounded (section "J4a run 3"). Next: J5 (plan section
  16).

## For the orchestrator

- For the kernel track: threads.rs `priority_inheriting_futexes_match_reference` (guest/futexpi.c) compares our
  output line by line with one fresh reference run, but the reference's order of the waiters' "woke" and
  "unlock" lines varies from run to run on one CPU (three orders in 20 runs, J4a run 2; evidence under "J4a
  run 2"). It failed once in J4a run 2's bar at full for that reason alone.

- Disk: the kernel test crate's file backend helper (crates/wcontainers-kernel-test/src/backend.rs, the
  kernel track's code) writes one image tar of about 8.4 MB per backend into
  target/wc-kernel-test/wc-backend-images and removes it only when the `Backend` is dropped. Backends that
  are never dropped (held to the end of a test process, or in a process that was killed) leave theirs. In
  this track's build directory 2,693 had piled up (26 GB, 2026-11-17 to 07:09 on 2026-11-18). J3 run 2
  deleted them. Worth telling the kernel track (other checkouts run the same tests).

## For the translator track

From the bare numbers sweep (J4a run 3): the back end's "code changed" hook needs the guest's code line
size (ARM64 64 bytes, `CACHE_TYPE` in wcontainers-arm64), which the guest interface does not give, so the
back end writes `& !63`; and the memory fault hook's access (0 read, 1 write) has no named constant in the
contract. Nothing in the translator was changed.

Measured in J4a run 1 (this Mac, load 89.8 to 88.1; scratch j4/slowpaths: awk.log, the scratch patch
per-detail.patch that counts every slow path by its detail, applied to a scratch worktree of 42554935 and
removed; the command line program, release, look-ahead full, alpine:3.22's busybox from a plain tar). The
detail of "jump through a register not recognised" is the jump's target (translator full.rs
`unrecognised_jump`). Nothing in the translator was changed.
- `seq 1 100000 | awk '{s+=$1} END {print s}'`: 5,306,710 times, 70 targets, 7.60 s.
- `awk 'BEGIN {for (i = 0; i < 100000; i++) s += length(sprintf("%d", i)); print s}'`: 8,601,492 times, 56
  targets, 7.05 s.
- `seq 1 20000 | awk '{a[$1 % 97] = a[$1 % 97] $1} END {...}'`: 2,621,218 times, 77 targets.
- The targets that account for most of it are the entries of musl's functions in ld-musl-aarch64.so.1
  (mapped at 0xf6f3d000, from /proc/self/maps; symbols by `nm -D`), reached from busybox's calls through
  its procedure linkage table: in the first command `free` (0xf6f6573c) 2,300,049, `malloc` (0xf6f65834)
  800,091, `memset` (0xf6f555a0) 600,090, `strlen` (0xf6f9bc00) 200,110, `strndup` (0xf6f9bdd0) 200,000,
  `strchr` 101,301, `strcpy` 100,054, `strtod` 100,003, `fileno` 100,001; in the second `free` 4,000,060,
  `malloc` 1,300,109, `memset` 1,100,100, `strlen` 300,090. Busybox's own targets come next (0xaa091548,
  offset 0x91548 of /bin/busybox, 200,001 and 600,005 times). The targets being function entries of another
  module suggests (not checked instruction by instruction) that the slow path is mostly busybox's
  procedure linkage stubs, which load the entry and jump through a register.

## Asked of the kernel track

The kernel's defaults as public constants (J4a run 3, the owner's rule on bare numbers): the runtime's base
configuration (spec.rs) must state what the kernel gives a container, and today writes it a second time: the
capability set (kernel credentials.rs), the starting limits (process.rs:41-48), the umask (container.rs:482),
the mounts with their options (container.rs:990-997) and the read-only paths. With them exported from the
kernel, spec.rs imports them. The wait status's parts (task.rs:960) likewise.

CPU time in free turns (J4a run 2): the kernel charges a thread for a free turn the outside's monotonic time
from handing it out to its return (task.rs `account`), so the CPU side's own work in the turn (instantiating a
new block's thread instances, translating code first reached) and the host's preemption of the host thread are
in the guest's CPU clocks, CPU timers and `RLIMIT_CPU` (measurements under "Findings of J4a run 1's check").
Charging the guest's own run only needs the charge to come from outside the kernel: for example `Outside::ran`
in a free turn carrying the host thread's CPU time spent in the guest's code, which the kernel charges in
place of the monotonic difference. This is the kernel's interface (and touches the progress unit the owner is
ruling on), so nothing is built here.

Standard input: by the orchestrator's ruling of 2026-11-18 the owner is settling one general way for a stream
to leave a kernel (an end outside carrying data, a window for flow control, and closed), and the kernel track
builds standard input, output and the terminal on it. The request below is kept as the record of what was
asked; it is not an interim design. What was asked (the brief: settle before J3 fixes the input and output
surface): Today the first process has
/dev/null as descriptor 0, `wcontainers_abi::Event::TerminalInput` exists but the kernel counts it as
unexpected (kernel lib.rs `event`), and `Host` has only `output`. To give a container standard input the
runtime needs, in the kernel and the abi:
- `Start` saying the first process's descriptor 0 is input from outside (as `docker run -i`), in place of
  /dev/null: a pipe whose write end the outside holds;
- an event that hands bytes to that pipe (`Event::Input { bytes }`, or `TerminalInput` given that meaning
  when there is no terminal) and one that closes its write end (end of file);
- flow control: the kernel tells the host how many bytes the pipe takes now and when it has room again (as
  `ConnectionReady` does for connections), so an outside never holds more than the pipe takes;
- for `process.terminal`: `Start` asking for a terminal of a size (descriptors 0 to 2 on a pseudo-terminal
  whose other side is the outside), `TerminalInput` and `TerminalResized` delivered to it, and output on
  `Stream::Output` as the terminal shows it (the abi's comment says so already).
With those the runtime adds an input stream to `System` (a reader thread raising the stop flag as bytes
come) and honours `process.terminal` and `process.consoleSize`. Nothing of input is built in J3; the
surface has no input argument, so nothing pretends.

To honour fields refused today, `Start` would take: the user and group (`process.user.uid`, `gid`), the umask,
the capability sets, the resource limits, no_new_privs, a seccomp filter, sysctl values, the mount list (to
leave a default mount out), the domain name, and the per-process values (`oomScoreAdj`, `ioPriority`,
`scheduler`, `execCPUAffinity`, `personality`, `timeOffsets`); and the address-space size and the memory
size the guest is told (plan section 8), which `Machine` holds once the kernel takes them.

## Findings of J4a run 3's check, and how each was settled (J5 run 1)

- A process's threads are capped by its thread blocks with nothing counted (minor): fixed (9adca9d1). A new
  thread past the last block is still refused as out of memory (the guest's pthread_create gets EAGAIN), now
  counted and warned "no thread block left in the process" with the number of blocks as the detail; a process
  whose memories the host refuses likewise ("the host refused a process's memories"). Unit test
  `a_thread_past_the_blocks_is_counted_and_warned`. The cap (15 MiB / 848 bytes, 18,547 threads at once) is
  not raised: that is the back end's capacity, as the brief's address ceilings are.
- A stale deadline raised the stop flag after an idle (minor): fixed (948628c2). The local system drops a
  turn's deadline when the turn ends (`stopped`), and the timer thread raises the flag only when it is the one
  that clears the deadline it slept towards. Left: a deadline that passes in the instant between the back end
  leaving `run` and `stopped` still raises once (one extra exit at most). Unit tests
  `a_turn_past_its_deadline_is_stopped` and `an_idle_past_an_ended_turns_deadline_raises_nothing`.
- The literal audit's fix list not acknowledged (minor): read now (orchestrator/magic-literals/fix-runtime.md).
  Its one home for shared facts conflicts with two names J4a run 3 added: the back end's `SLOT_BYTES`
  (`size_of::<i32>()`) and `wasm_page()` (from wasmtime's `MemoryType::page_size`) are to become the
  translator's `contract::SLOT_BYTES` / `SlotEntry` and `contract::WASM_PAGE_BYTES`. Neither is on a branch
  this track merges (origin/phase-1 is still 2edbc451), so both stay as they are until they land, as the
  brief says. Of the list, step 1 items 1 and 2 are done in this run (starts.rs uses `PAGE_SIZE` itself;
  `CHUNK` is derived from `CHUNK_BYTES` and `PAGE_SIZE`, 9adca9d1). origin/phase-1-literals exists (its
  L1 is recorded done) but this run's task names only the merge of origin/phase-1, so it is not merged and
  no baseline is made (under "For the orchestrator").
- How to run named the program by its old name (minor): corrected (section "How to run").
- J4a run 3's bar named the wrong commit (minor): corrected; the logs' f353e5be is named.
- Memory grows with live threads times translations (information): no change; it is the plan's one instance
  per thread (translator plan 6).

## Findings of J4a run 2's check, and how each was settled (J4a run 3)

- Plan section 20 said the stand-in counts the placeholder unit and the translator's count comes before J4
  (blocking): corrected (4502981a). It now says the stand-in counts Unicorn's instructions as its progress
  until the owner rules, its count of the placeholder unit is built with J4b, and the translator's count is
  asked for before J4b.
- The repeats never had a turn ended by the seed's limit followed by another thread (minor): repeats.rs now
  also runs the kernel test crate's threadsched program, whose two computing threads share the CPU by turns,
  with and without saves (6d26eb6f): `preempted_threads_repeat_for_a_seed` and
  `preempted_threads_saved_and_brought_back_repeat_for_a_seed`. With the runtime feature each asserts the
  back end warned "left run: another thread runs" with the detail of a leave after a stop, so the seed's turn
  ends decided an interleaving; the plan's J4a bar says so (b5d5efd5).
- Guest CPU clocks include the back end's own work in free turns (minor): not changed in this run, for the
  reason J4a run 2 gave: charging only the guest's own run needs the kernel to take the charge from outside
  (kernel task.rs `account`), which is the kernel's interface and is asked of the kernel track (section
  "Asked of the kernel track"). The check's measurements (mt2, mt4) agree with run 2's.
- New bare literals and no record of the sweep (minor): swept, recorded in section "Bare numbers" below.
  The bounded look-ahead's range is no longer written twice: the kernel test driver and the surface tests
  both draw N from 1 to the translator's `BOUND` (256, its default N; re-exported by the back end), derived
  from the translator rather than a second 64 (a59d5320, 105c6463). image.rs's modes come from the kernel's
  Linux constants (`S_IRWXU` and the rest).
- repeats.rs not listed under Departures (minor): listed now (section "Departures").
- The reference's own varying order in `priority_inheriting_futexes_match_reference` (minor, the kernel
  track's test): already passed on under "For the orchestrator"; nothing to change here.

## Bare numbers (the owner's rule of 2026-11-18; sweep of J4a run 3)

Replaced (a59d5320 in the owned crates, 105c6463 in the driver):
- wcontainers-runtime image.rs: file modes by the kernel's Linux constants (`DIRECTORY_MODE`, `FILE_MODE`,
  `EXECUTABLE`, `PERMISSION_BITS` from `S_I*`); the fields of `/etc/passwd` and `/etc/group` named by place
  (`field::UID` and the rest); the hosts file's loopback addresses from `Ipv4Addr::LOCALHOST` and
  `Ipv6Addr::LOCALHOST`, and the kernel's address and name server formatted by `Ipv4Addr`. spec.rs: its
  tables are slices, so no count is written beside them.
- wcontainers-wasmtime: the slot entry's size `SLOT_BYTES` (`size_of::<i32>()`), the 32 bit state memory's
  size from `u32::BITS`, thread blocks aligned to `size_of::<u128>()`, the WebAssembly page from wasmtime's
  `MemoryType::page_size`, the page map's entry size and bits named, blake3's `OUT_LEN`, the cache file's
  words by `size_of::<u64>()`, the bytes read for a faulting instruction by the guest's `MAX_LENGTH`, the
  system call's arguments taken from the hook's parameters into the abi's `Syscall` (no count written), the
  output form in the cache key named once; starts.rs: words by `size_of::<u64>()`, the instruction alignment
  from ARM64's `Guest::ALIGN`, the relocation type by object's `r_type`. wcon: Docker's exit statuses and the
  wait status's parts named.
- The tests' bounded range (above); the driver's salt for the look-ahead's draw named once.
Left, with why:
- spec.rs writes the kernel's defaults a second time (the capabilities, limits, umask, mounts with their
  options, read-only paths): the kernel holds them as private literals (process.rs:47-48, container.rs:482
  and 990-997, credentials.rs). The fix is the kernel exporting them for the runtime to import; asked of the
  kernel track.
- The back end's code line of the "code changed" hook (`address & !63`, 64 bytes, ARM64's line in `CTR_EL0`,
  wcontainers-arm64 decode_forms.rs `CACHE_TYPE`): the guest interface has no constant for it; for the
  translator track (wcontainers-guest-cpu). The memory fault hook's access encoding (`access == 1` for a
  write): the translator's contract documents it without a constant.
- wcon's wait status constants repeat what the kernel decodes with literals (task.rs:960).
- Named constants that are tuning choices (`CHUNK`, `BLOCKS_BYTES`, `STEP`, `SEARCH_PAGES`, `MOST_LINKS`,
  `CPU_HOOKS`, `CPU_HOOK_ARGUMENTS`) stay as one definition each; the wasmtime hook arities
  (`cpu_hook_func::<G, 2>` to `7`) are one function per arity. Test inputs and expected results are left as
  data.
- Docker's text of `/etc/hosts` beyond the loopback lines is the reference's file, written as it is.

## J4a run 3 (2026-11-18; this Mac, Apple M5 Max, 18 cores; other tracks running)

- The owner's names of 2026-11-18 (brief), each its own commit, the plan corrected (582a1ab0): the shipped
  system `System` became `LocalSystem` (and "the shipped outside" in messages "the local system"); the trait
  the embedder implements `Outside` became `System`, with `Container::system` and `system_mut`, the loop's
  `Leaving::SystemAsked` and its warning "left run: the system asked" (5bec4f91, with the kernel test
  driver's impl); `BackEnd` became `Executor`; `Tuning` became `TranslatorSettings` (its doc now says it is
  how the translator works); `Machine` became `GuestMachine`; the command line program is `wcon`
  (src/bin/wcon.rs; its messages start "wcon:"). `StopFlag` is not renamed, as the brief says. `TaskId`,
  `task_id_allocate` and `task_id_free`: the runtime touches no guest task ids (the kernel allocates them
  inside; the runtime's `ThreadId` and `MemoryId` are the abi's handles of the CPU side's threads and
  memories), so nothing here is named after them.
- The owned suite after the sweep and again after the renames (scratch j4run3/owned-1.log, owned-2.log): 121
  passed, 1 ignored (`run_with_the_cache`, run by `a_second_process_starts_from_the_cache`), 0 failed; 106.8 s
  (load 56.2 to 73.1) and 128.5 s (load 73.6 to 95.5).
- Repeats with the new program, at full, 3 runs per seed, seeds 1 to 3, on the tree before the sweep
  (scratch j4run3/rep-full-3.log; 178.6 s with its build, load 31.3 to 35.1): all 5 tests passed. The
  threadsched program's turn counts vary by seed (1669, 1621, 1619; with saves 1614, 1633, 1670) and each
  seed's 3 runs were identical; every seed of it had a leave after a turn ended; threads and futexes had
  none, as the check found (463 and 353 turns every seed).
- Unknown properties of a config.json (the brief, 2026-11-18, later): `spec_from_json` reads with oci-spec's
  types through `serde_ignored` and returns with the configuration the path of each property they do not
  define (named as the refusals name fields, `mounts[2].label`) and of each annotation, since the runtime acts
  on none; the hand-written re-serialise-and-compare is gone. `wcon` warns of each, "wcon: configuration: a
  property the specification does not define, or an annotation, ignored: PATH: COUNT". refused.rs's four
  tests of fields oci-spec does not read are now `ignored!` tests that the container is made and the path
  named, with one for an annotation; the base configuration is accepted with nothing ignored; surface.rs's
  command line test gives `wcon` a configuration with an unknown property and checks the warning. A
  library caller gets the paths; the warning is the caller's (the runtime's warnings go through the
  kernel's `Host::warn`, whose reason and detail cannot carry a path, and the container does not exist yet).
  The owned suite with it (scratch j4run3/owned-4.log; 68.0 s, load 41.9 to 78.2): 122 passed (the new
  annotation test), 1 ignored, 0 failed. (owned-3.log is the same run before spec.rs's own unit test was
  changed from a refusal to an ignored path: that test failed, as it should have.)
- J4a's bar at f353e5be (the logs' headers; the same code as 582a1ab0, which changed only this file), every
  change of this run's code but the unknown properties, which nothing the bar runs reads (the kernel tests read no config.json) (scratch j4/bars.sh bar --nocapture, logs
  j4run3/bar-full.log, bar-none.log, bar-bounded.log; the bar's 13 test files, now 36 tests with the two new
  repeats): full 36 passed (383.7 s with its build, load 93.9 to 64.0); none 36 passed (594.4 s, load 64.0 to
  60.9); bounded 36 passed (435.0 s, load 60.9 to 44.6; N drawn 237, 44, 38, 15, 51, 27 and 202). The repeats
  ran 2 runs per seed, seeds 1 to 3, at each look-ahead alone, all identical; threadsched had a leave after a
  turn ended at every seed and look-ahead.
- Repeats at bounded, 10 runs per seed, seeds 1 to 3 (N 44, 15 and 38), at c981822a (scratch
  j4run3/rep-bounded-10.log; 523.6 s, load 60.5 to 23.9): all 5 tests passed, every seed's 10 runs identical;
  threadsched had a leave after a turn ended at every seed, with and without saves (turns 1669, 1621, 1619;
  with saves 1614, 1633, 1670).
- The threaded program through the surface at 582a1ab0 (j4run3/threaded-none.log, threaded-bounded.log, load
  43 to 47): none passed, bounded (N 114 for `LOOK_AHEAD_SEED` 0) passed; full passed in the owned suite
  (owned-2.log).

## J4a run 2 (2026-11-18; this Mac, Apple M5 Max, 18 cores; other tracks running)

- The bar at 1ebe8e4d (full) and f7777000 (none, bounded; the same code with repeats.rs comparing files):
  full 34 passed (185.7 s with its build, load 62.9 to 39.5; scratch j4/run2-full.log), none 34 passed (152.5 s,
  load 39.5 to 29.2; run2-none.log), bounded 33 passed and 1 failed (163.9 s, load 29.2 to 26.1;
  run2-bounded.log): repeats.rs `futexes_repeat_for_a_seed`, seed 2 (bounded 4), the first run's CPU side
  warnings differed from the second's.
- The cause: the test host's repeatable wall clock started at the machine's clock (driver.rs
  `TestHost::repeatable`), and the futexes program (guest/futexes.c:144-149) adds 20 ms to a
  `CLOCK_REALTIME` reading and carries into the seconds when the nanoseconds pass a second. About one run in
  fifty takes the carry; the output, kernel warnings, counters and turns are the same either way, but at
  bounded look-ahead the other branch shows as different code translated at run time (main+0x968 to
  main+0x9b0 of the program). Scratch j4run2/diffrun.py on the two failures (run2-bounded.log and
  run2-flake-1.log, the second found by running repeats at bounded four times at f7777000) shows the same
  place. Not caused by the instance reuse. Fixed in the driver (87319389): `Injections::wall_start`, which
  repeats.rs reads once per seed and gives every run of it; the clock is an answer from outside, given again,
  as the brief's logging and feeding back of answers would. Then repeats at bounded, 3 runs per seed, 8 times
  (the first with the tests' translation cache emptied): all identical (scratch j4/run2-fixed-1.log to -8.log).
- J4a's bar at 87319389, the code of every change of this run (scratch j4/bars.sh run2final --nocapture):
  - full: 33 passed, 1 failed (239.8 s with its build, load 49.7 to 44.0; j4/run2final-full.log): threads.rs
    `priority_inheriting_futexes_match_reference`, seed 1, where the order of the waiters' lines differed from
    the reference's. Ours was the same run as in every passing run (its CPU side summary: another thread runs
    17, stop checks 33, 96 translated at run time, as in run2-full.log and the reruns); the reference is run
    again by every test and varies itself: `docker run --cpuset-cpus=0` of the same futexpi binary 20 times gave
    three different orders of those lines, 9, 8 and 3 times (load 64.2 to 66.4; j4run2/futexpi-reference.log).
    The test (the kernel track's) compares line by line with one reference run, so it fails whenever the
    reference takes another order than ours. Not changed here; for the kernel track (heading "For the
    orchestrator"). The same test passed in 15 more runs at full of this code: the threads file whole 5 times
    (run2-threadsfile-full-1 to -5.log, 13 passed each), seed 1 alone 6 times and 4 times with the translation
    cache emptied first (run2-futexpi-full-*.log, run2-futexpi-cold-full-*.log).
  - none: 34 passed (322.0 s, load 44.0 to 39.0; run2final-none.log).
  - bounded: 34 passed (376.2 s, load 39.0 to 46.9; run2final-bounded.log).
- Repeats at full, 10 runs per seed, seeds 1 to 3 (f7777000): all identical (30.2 s, load 25.1 to 22.7;
  j4/run2-repeats-full-10.log). Futexes seed 2 at bounded 10 times in one process: identical
  (run2-futexes-b2-10.log).
- The threaded program through the surface: full in the owned suite (j4/run2-owned-1.log: 121 passed, J4a run 1's
  120 and the new unit test, 0 failed, load 29.2 to 76.0); none and bounded 50 pass (run2-threaded-none.log,
  run2-threaded-bounded.log, load 22 to 25).

## Findings of J4a run 1's check, and how each was settled (J4a run 2)

- A dropped thread's instances were never reused (blocking): fixed (1ebe8e4d). A thread's instances import its
  block's address, so they are kept by block (`Process::kept`) with the thread's exact pieces and how many of
  the process's translations they hold. A new thread given that block takes them at its first run, and the
  translations another thread added while the block was free are instantiated in them then. A process
  brought back from a save makes its instances again (its store is new). Test: the back end's unit test
  `a_dropped_threads_instances_serve_the_next_thread`, at full and none: a thread runs and is dropped, another
  thread translates new code, a new thread gets the same block and the same instance and runs the new code
  with nothing translated again. Without the catch-up the test hangs (tried once, the catch-up removed by
  hand). The check's program (its progs/many.c: N threads started and joined one after another, then 64 at
  once) through the command line program at 1ebe8e4d, maximum resident size (`/usr/bin/time -l`; load 45 to
  63; scratch j4run2/many.sh, many-23.log, many-none.log, many-full.log):
  - bounded 23: N=200 99 MB, N=3000 96 MB, N=10000 97 MB (1.98 s); the check measured 183 MB, 1,356 MB and
    4,367 MB (8.65 s).
  - none: N=200 234 MB, N=3000 232 MB (the check: 306 MB and 1,292 MB).
  - full: N=200 480 MB, N=3000 486 MB (the check: 488 MB and 558 MB).
  The output is the check's at every size (`sequential N joined N, total N+128`, status 0).
- The plan said the loop's and the stand-in's counts were built from J2 (minor): plan section 9 now says
  who counts once the owner rules, that none of the three is built, and that the first two wait for the
  ruling too (e027a153).
- The guest's CPU clocks include the back end's own work in free turns (minor): partly settled, the rest
  asked of the kernel track (below). The kernel charges a thread for a free turn the time on the outside's
  monotonic clock from handing it out to its return (kernel task.rs `account`); the back end's own work in
  that time is a cost of the thread as Linux's page faults are, but larger. With instances reused, a thread
  given a dropped thread's block no longer pays for instantiating: the check's sleepcpu at 1ebe8e4d (load 45.8;
  scratch j4run2/sleepcpu-1.log), sleepers 1 and 2 start at 30 and 27 us at full, 377 and 5 us at none,
  247 and 39 us at bounded 23 (Docker 22 us), and are charged 14 to 20 us for the 100 ms sleep. What is left:
  - the first thread of a block pays for its instances (sleeper 0 at start: 17,728 us at full, 64,749 us at
    none, 7,381 us at bounded 23);
  - code first reached in a turn is translated in it (sleeper 0 after the sleep at bounded 23: 78,858 us);
  - the first process's main thread carries the translation of its program (main at start: 362,880 us at
    full, 354,646 us at none, 267,297 us at bounded 23; Docker 11,762 us);
  - the host's preemption of the host thread at this load.
  The check's cputhreads still prints `sleeping thread under 10 ms: 0` at each look-ahead (its sleeping thread
  is the first of its block). Plan section 9 says so.
- The plan's J4a item read as if outside events in general raised the flag (minor): the plan's J4a now says
  what is built: the stop flag raised from another host thread, whose one source in the shipped outside today
  is its slice timer; a file backend and the network (J5) and input (the kernel track's streams) raise it the
  same way once they exist (e027a153).
- repeats.rs left out the files a container left (minor): it now compares each file's path, mode, owner,
  device and a hash of its contents too (b001897c).

## Open findings of J3's last check, and how each was settled (J4a run 1)

- Mounts checked as a set (minor): fixed (d33aa0a5). The list must be the kernel's six once each in the
  kernel's order; the first `mounts[n]` that differs is refused, naming the order and what another order or a
  repeat would take (the kernel taking its mounts at start). A list lacking one is still refused at `mounts`.
  Tests: refused.rs `kernel_mounts_in_another_order` (`mounts[1]`) and `a_kernel_mount_repeated`
  (`mounts[6]`); surface.rs `the_guest_sees_the_honoured_values` now also checks that `/proc/mounts` lists them
  in the configuration's order.
- A program not in PATH ran from the working directory (minor): fixed. A name with no slash found in no
  directory of `PATH` makes no container: `Error::NotFound(name)`, shown as runc words it ("executable file
  not found in $PATH"); the command line program exits 127 for it, as Docker does. A path with a slash still
  goes to the kernel. Tests: surface.rs `a_program_not_in_the_path_is_not_found` (the check's case: `PATH=/nonexistent`,
  cwd `/bin`, `ls`; and Docker exits 127), the unit test `programs_are_found_through_path_and_links`.
- No /etc/resolv.conf from an OCI image (minor): fixed. At start the root gets the files Docker writes over
  its mounts in every container, from either form of image: `/etc/hostname` (the host name), `/etc/hosts` (as
  Docker writes it, the kernel's made-up address for the host name), `/etc/resolv.conf` (the kernel's name
  server, 127.0.0.11), mode 0644, root's, dated 0; `/etc` is made when the image has none. Not `/.dockerenv`
  (Departures). Test: surface.rs `dockers_files_are_written_from_either_image` (mode, the host name file
  equal to `uname -n`, the hosts file's last line naming it, a name server line; the same as `docker run`
  from the OCI image and the plain tar).
- An empty host name (minor): taken as none, as runc does, so the guest sees `localhost`. Test: surface.rs
  `an_empty_host_name_is_localhost`.
- The surface tests ran at full only (minor): fixed (42554935). They read `LOOK_AHEAD` as the kernel tests do;
  bounded draws N from `LOOK_AHEAD_SEED` alone (blake3 of it, 1 to 64; 50 for the default 0), and the command
  line program is given the same `--look-ahead`. All 13 pass at none (229.9 s, load 107.0 to 117.6;
  scratch j4/surface-none.log) and bounded 50 (164.3 s, load 117.6 to 101.3; surface-bounded.log); full in
  the whole suite below.
- `Container::step` and `Container::idle` untested (minor): surface.rs `a_container_is_stepped_from_the_callers_loop`
  drives the check's command with a step and idle loop, at least one idle step, the reference's output and
  status 4 (full: 31 steps, 1 idle; none 33 and 1; bounded 50: 67 and 1).

## J4a run 1 (2026-11-18; this Mac, Apple M5 Max, 18 cores; other tracks running, load 47 to 140)

Built: nothing new was needed in the back end or the loop for J4a's tests: threads, futexes, timers and
free turns were in place from J1 to J3, and J4a's bar passed at 68013b31 before any change of this run (scratch
j4/first-full.log, first-none.log, first-bounded.log: 31 passed at each, 213.0 s, 319.3 s and 546.4 s, load
47.7 to 103.1). Added:
- The kernel test crate's tests/repeats.rs (5492ba0a, its own commit): `threads` and `futexes` under each seed
  of `schedule_seeds`, and `threads` with 3 saves, each run `REPEAT_RUNS` times (2 if unset) and compared whole:
  output, the kernel's warnings, the CPU side's warnings, counters, saves and the turn count. It prints each
  seed's turn count and a fingerprint.
- surface.rs `a_threaded_program_matches_the_reference` (42554935), with tests/programs/threaded.c built by zig:
  four threads taking turns in order through a mutex and a condition variable (200 turns) and adding 20,000
  each under the mutex; a thread that spins with no system call until the main thread, back from a 5 ms
  sleep, sets a flag (only the outside's timer ends its free turn); a POSIX timer signal sent to one chosen
  thread (`SIGEV_THREAD_ID`) waiting in `sigsuspend`, while the main thread spins with the signal unblocked;
  then one thread calls `exit(3)` while another waits in `read` on a pipe and the main thread in
  `pthread_join`. Run in free turns through the public surface from the plain tar with the program added, and
  compared with `docker run` of the same binary (output and status 3). Passes at full, none and bounded 50.

Repeats (scratch j4/repeats.sh NAME LOOK RUNS):
- full, 10 runs per seed, seeds 1 to 3: all identical (52.6 s, load 93.6 to 112.5; repeats-full-10.log;
  threads 463 turns for each seed, 463 to 465 with saves; futexes 353).
- none, 3 runs: identical (20.7 s, load 112.5 to 103.1; repeats-none-3.log). bounded, 3 runs (N 11, 4 and 10
  for seeds 1 to 3): identical (28.8 s, load 103.1 to 113.6; repeats-bounded-3.log).
- The stand-in, 3 runs: identical (42.6 s, load 113.6 to 101.2; repeats-standin-3.log).
- Turn counts are compared only within one setting. They happen to be equal or close across settings here
  (futexes seed 1: 353 at full, 356 at none), which J4b would make a rule.

J4a's bar at fc43c6fc (the code of every change of this run; scratch j4/bars.sh final --nocapture, logs
final-full.log, final-none.log, final-bounded.log): the 11 tests named, the whole of shell
(`runs_repeat_for_a_seed` among its 3) and repeats.rs, 34 tests in all, pass at each look-ahead:
- full: 34 passed, 0 failed, 411.3 s with its build (load 91.4 to 78.6);
- none: 34 passed, 237.4 s (load 78.6 to 56.3);
- bounded: 34 passed, 315.8 s (load 56.3 to 61.9); N drawn from each run's seed (60 for seed 0, 11, 4 and
  10 for seeds 1 to 3, 13, 7 and 51 for the shell's seeds 7 to 9).
The surface's threaded program at full is in the whole suite of what this track owns at fc43c6fc (scratch
j4/owned-final.log; 52.6 s with its build, load 61.1 to 88.4): runtime 7 unit; back end 3 unit, cache 3,
code 4, faults 2, fork 5, refused 72, surface 13 (and the cache run's child, ignored when run alone), turns
5, views 6: 120 passed, 0 failed. Its cache pair: first 8.07 s (both ranges and 98 pieces made), second
0.77 s (both ranges and 94 pieces from disk, 21 made).

The CPU side's warnings per test file at full (final-full.log by j2/scripts/warnings.py --targets; scratch
j4/final-warnings-full.txt; repeats.rs's lines run its programs 18 times):
- clocks: 1 runs; continued after stack loss 13, jump through a register not recognised 1064, left run: the call waits 12, left run: the container finished 1, progress counted by stop checks 13, translated at run time 6
- cpulimit: 1 runs; continued after stack loss 10, jump through a register not recognised 21, left run: another thread runs 4, left run: new registers 2, left run: the container finished 1, progress counted by stop checks 7, translated at run time 21
- cputime: 1 runs; continued after stack loss 8, jump through a register not recognised 57222, left run: the call waits 3, left run: the container finished 1, loop not proven 9, progress counted by stop checks 4, translated at run time 23
- deadlines: 1 runs; continued after stack loss 79, jump through a register not recognised 85, left run: another thread runs 47, left run: the container finished 1, loop not proven 9, progress counted by stop checks 48, translated at run time 102
- lockwaits: 1 runs; continued after stack loss 223, jump through a register not recognised 175, left run: another thread runs 70, left run: new registers 3, left run: the call waits 22, left run: the container finished 1, progress counted by stop checks 96, translated at run time 184
- priorities: 1 runs; continued after stack loss 27, jump through a register not recognised 237, left run: another thread runs 12, left run: the container finished 1, progress counted by stop checks 13, translated at run time 55
- repeats: 18 runs; continued after stack loss 5614, first write to a shared page 72, jump through a register not recognised 4140, left run: another thread runs 2702, left run: new registers 30, left run: the call waits 250, left run: the container finished 18, left run: the outside asked 18, left to an address 12, loop not proven 144, memory fault 6, progress counted by stop checks 3024, translated at run time 1702
- rseqabort: 4 runs; continued after stack loss 27, jump through a register not recognised 72, left run: another thread runs 12, left run: new registers 8, left run: the container finished 4, memory fault 1, progress counted by stop checks 25, translated at run time 53
- schedattr: 1 runs; continued after stack loss 5, jump through a register not recognised 950, left run: another thread runs 2, left run: the container finished 1, loop not proven 552, progress counted by stop checks 3, translated at run time 27
- shell: 38 runs; continued after stack loss 6733, exact floating point requested 2, jump through a register not recognised 2445894, left run: another thread runs 1700, left run: new registers 118, left run: the call waits 191, left run: the container finished 38, left to an address 38, loop not proven 1109231, memory fault 1106, progress counted by stop checks 3153, translated at run time 4890
- threads: 39 runs; continued after stack loss 5225, first write to a shared page 54, jump through a register not recognised 8391, left run: another thread runs 2477, left run: new registers 39, left run: the call waits 256, left run: the container finished 39, left run: the outside asked 50, left to an address 9, loop not proven 9375, memory fault 27, progress counted by stop checks 2888, translated at run time 2916
- timers: 12 runs; continued after stack loss 922, jump through a register not recognised 1737, left run: another thread runs 118, left run: new registers 123, left run: the call waits 171, left run: the container finished 12, left run: the outside asked 66, loop not proven 81, memory fault 3, progress counted by stop checks 493, translated at run time 730
- turns: 4 runs; continued after stack loss 195, first write to a shared page 2, jump through a register not recognised 537, left run: another thread runs 882, left run: the container finished 4, loop not proven 27, progress counted by stop checks 886, translated at run time 207

## Findings of J3 run 1's check, and how each was settled (J3 run 2)

- Capabilities left out were taken as Docker's (blocking): fixed (3b81f5b4). A set left out is empty in the
  runtime specification, so a configuration without `process.capabilities`, or without its `bounding`,
  `effective` or `permitted`, is refused by that name (the kernel gives the 14). Leaving out `inheritable`
  or `ambient` is accepted: empty is what the kernel gives. Tests: refused.rs
  `process_capabilities_absent`, `process_capabilities_{effective,permitted,bounding}_absent`.
- Fields oci-spec does not model were dropped (blocking): fixed. The new `spec_from_json` (runtime crate) reads
  a `config.json`, compares its JSON with what oci-spec read (re-serialised), and refuses the first field it
  did not read, by its path (`hostnmae`, `process.futureProcessField`, `linux.resources.memory.limitt`). Nulls
  count as absent. The command line program and every refusal test now go through it. Tests: refused.rs
  `a_field_oci_spec_does_not_read` and the process, linux and mount ones; spec.rs
  `a_whole_configuration_is_read_with_nothing_unread` (a `runc spec` style configuration with resources,
  seccomp, devices, masked and read-only paths: nothing is reported as unread). The check's
  cli-unknown.json now exits 125 with `hostnmae is not supported: a field this runtime does not know`.
  A `Spec` built in Rust cannot hold an unknown field, so `Runtime::container` keeps taking a `Spec`.
- `root.path` was listed as honoured but never read (blocking): it is now refused (`root.path`, unless empty),
  with what honouring it would take. The root's files are the `Image` argument, and `base_spec()` has no
  `root`. Reading a host directory as a root needs the shipped outside's file backend (J5), because an
  unpacked directory on macOS keeps neither the files' owners nor their devices. Test: refused.rs
  `root_path`. Plan section 14 corrected. A `config.json` written by `runc spec` is now refused at
  `root.path` first.
- The conversion left out the implicit annotations (minor): fixed. `os`, `architecture`, `variant`,
  `os.version`, `os.features` (comma-joined, as ExposedPorts), `author` and `created` go to their
  `org.opencontainers.image.` annotations. Labels are applied last, so a label overrides any implicit one,
  StopSignal included. Test: image.rs `implicit_annotations_give_way_to_labels`.
- With no `User`, root had no groups and no HOME (minor): fixed, and the same now holds for a plain tar
  (`Image::spec` of a tar).
  - Groups: the user is root, its group and then the groups `/etc/group` lists it in, in file order, as
    Docker gives them (`groups=0(root),0(root),1(bin),...`).
  - A user named without a group gets the same form. A numeric `User`, or one naming a group, gets no
    additional groups, as conversion.md says (the code had given a numeric user the groups listing its
    name).
  - HOME: as runc does, a process whose environment has no `HOME` gets its user's home directory from the
    root's `/etc/passwd` (`/` when none). This is done at start (`Runtime::container`), so a configuration
    built by hand gets it too.
  - Test: surface.rs `the_user_and_environment_match_the_reference` runs `id; echo "home=$HOME"; env | sort`
    from the OCI image and from the plain tar, both matching `docker run`.
- An image without Env lost PATH (minor): fixed. The image's Env is kept whole, and the base `PATH` is added
  when it has none (`with_defaults`). Tests: image.rs `the_image_configuration_is_converted` (PATH added)
  and `implicit_annotations_give_way_to_labels` (the image's own PATH kept).
- Three refusals had no test (minor): added (refused.rs `uid_mappings`, `gid_mappings`,
  `a_gid_mapped_mount`).
- The `oci-layout` file was read by hand (minor): now `oci_spec::image::OciLayout::from_reader`.
- The honoured values were not checked against the guest (minor): surface.rs
  `the_guest_sees_the_honoured_values`. It runs the base configuration with the umask and all 16 limits
  written out (accepted, so they equal the runtime's constants), and checks the following:
  - the umask;
  - every line of `/proc/self/limits`;
  - all five capability sets of `/proc/self/status`, against the masks computed from the configuration;
  - `/proc/mounts`: each mount's source, type and options, and nothing beyond the root, the six mounts and
    the read-only paths.
  This found a value the configuration did not state: the kernel mounts `/proc/sys` again read-only (as
  Docker's `readonlyPaths` does). So `base_spec()` now carries `linux.readonlyPaths: ["/proc/sys"]`.
  Exactly that list is honoured; any other list, or none, is refused by name (refused.rs
  `linux_readonly_paths`, `linux_readonly_paths_absent`).
- The check's probe (scratch check-j3/probe) was not run again. Each of its configurations that showed a
  problem is now a test in refused.rs or surface.rs.

J3's bar run again, as the whole suite of what this track owns (3b81f5b4, the test build;
`cargo test -j 4 -p wcontainers-runtime -p wcontainers-wasmtime -- --test-threads 3 --nocapture`; 64 s with
its build, load 31.5 to 56.9; scratch j3run2/owned-1.log):
- Results: runtime 7; back end 3 unit, cache 3, code 4, faults 2, fork 5, refused 70 (69 refusals and the
  base configuration accepted), surface 8 (the cache run's child ignored when run alone), turns 5, views 6.
  113 passed, 0 failed.
- The surface tests cover the pipeline from the OCI image and from the plain tar, the user and environment,
  the honoured values, the busy process and the command line program from both images. All match the
  reference.
- The cache pair, each run a new process: the first, with an empty cache, made both ranges and 104 pieces
  in 7.805 s. The second found both ranges and 85 pieces on disk and made 7, in 0.346 s. One pair only, so
  the times show the order of the difference, not a figure.
- The test images left under target/tmp/wc-surface (one set per test process) were deleted after the run.

## Findings of J2 run 5's check, and how each was settled (J3 run 1)

- Layer 1's second kernel difference, the unix sequenced packet send after the peer closed with unread data
  (minor): recorded here, and it is gone with the merged kernel. With J2's kernel (36d20013) seed 6022140857
  with 40,000 cases gave `sendmmsg ... ours -32 theirs -104` on both the back end and the buffer CPU (the
  check's logs). The kernel track's ed9cea02 ("let a local sequenced packet send take its pending error
  after copying and before reading its control messages"), merged here with K6b, settles it: the same seed
  and count pass over the back end at fff0aa77 (scratch j3/m-layer1-40000-6022140857.log, 664.8 s, load 28.7
  to 34.2), and so does the buffer CPU's (no feature; 655.0 s, load 34.2 to 34.4;
  j3/m-layer1-buffer-40000-6022140857.log).
- The copy cost was one low run (minor): J2 run 4's paragraph now gives the range of repeated runs with their
  loads (below): `memory_duplicate` 74.2 µs median at load 29.7 (this track's run 4) and 85.5 to 124.6 µs
  medians in the checks' repeats at loads 41 to 52 (check-j2-run3, check-j2-run4, check-j2-run5/cost).
- The warnings per test were J2 run 1's (minor): the list at the merged tree, sharedviews and the newer
  reasons included, is in J3's section below (from J2's bar at full with the back end's reports, fff0aa77).
- The loop's count of the kernel's changes of pc (minor): put to the orchestrator above.
- The stand-in writes part of an access that crosses into an inaccessible page and reports `si_addr` inside
  the access (information): recorded. Unicorn's limitation, for the kernel test crate; the back end does
  what the reference does (the check's zzcheck5 at full, none and bounded).
- Run 4's passing layer 1 log does not show its count or seed (minor): they are in j2run4/all.sh, which ran
  it: `LAYER1_CASES=10000 LAYER1_SEED=2026111804 LAYER1_BACKEND_CASES=32 LAYER1_CACHED_CASES=32`. J3's run
  scripts (scratch j3/run.sh, j3/trun.sh) write every layer 1 knob into each log's header.

## J3 run 1 (2026-11-18; this Mac, Apple M5 Max, 18 cores; other tracks running)

Merged: origin/phase-1 at 2edbc451 (the kernel through K6b) into this branch (db247ebf). The driver's three
conflicts were settled keeping both sides: the copy's report carries the kernel track's turn count and this
track's CPU warnings; the save moments use the kernel track's `save_gap` in this track's `Injecting`; the
stand-in is made with the container's `address_space` (the kernel track's `StandIn::with_space`), while the
back end keeps its own ceiling from its output's widths whatever the container asks (only runtimes.rs asks
for 16 GiB; J6's).

The merged kernel reads a backend file's pages when first touched (kernel f45c8e0a): a mapped page stays
without access until a fault fills it. The back end translated a code range whole at first use, so pages
not yet filled were translated from zeros and ran as undefined instructions (busybox from the backend root
printed nothing: scratch j3/bar-full.log, `shell` 1 passed 2 failed). Fixed (fff0aa77): a range is
translated in runs of pages that allow running code, each run once (`CodeRange::covered`); a page that is not
executable yet faults when reached and its run is translated when it may run. Tested by tests/code.rs
`a_page_that_comes_later_faults_then_runs_{full,none}` (failed before the fix with `IllegalInstruction`).

J2's bar on the merged tree with the fix (scratch worktree j3/tree at fff0aa77, its own target; j3/merged.sh):
- full 47 passed, 2 ignored, 359.0 s with its build (load 17.5 to 62.0; j3/m-bar-full.log, with the back
  end's reports); none 47 passed, 2 ignored, 222.6 s (load 62.0 to 36.0; m-bar-none.log); bounded 47
  passed, 2 ignored, 94.4 s (load 36.0 to 28.7; m-bar-bounded.log). The 47 are J2's 46 and the kernel
  track's new test in processes; ignored as before: align's exclusive load (the translator's known failure)
  and memfd's huge pages.
- Layer 1: seed 6022140857 with 40,000 sequences passes over the back end and on the buffer CPU (above;
  a first try in the checkout, j3/layer1-40000-6022140857.log, was ended by a signal 15 from outside after
  its build, with no verdict); a
  new seed, 10,000 sequences with seed 2026111831 and 32 on each file backend, 3 passed in 167.8 s (load 34.4
  to 31.5; j3/m-layer1-10000-2026111831.log, whose header names every knob).

- The stand-in: the whole kernel test crate without the feature on the same tree (j3/m-verdicts.log and
  m-verdicts.txt; 2416.8 s, load 31.5 to 56.0, the K6b runtimes most of it): 217 passed, 1 failed, 106
  ignored. The failure is `backend::transfers_match_reference` on the reference's side: the same standard
  output, and the reference's standard error held a message of its own backend helper ("wc-backend:
  EncodeMessage(... ENOENT ...)"); run again alone it passes (j3/m-transfers-rerun.log, load 50.6). Not
  compared line by line with the kernel track's own verdicts, which this track does not keep.

Built in J3:
- Free turns (5d86b31d): `Turn` is `Counted { limit }` or `Free`; `StopFlag` (runtime crate) raises the
  running thread's flag from any host thread, or the next one's; the back end enters it for each `run`, sets
  the flag word only for counted turns, and in a free turn the stop hook hands `Exit::Stop` with no progress
  and warns no "progress counted by stop checks". The kernel test crate's driver and stand-in change one
  line each for the enum (the same commit, since neither builds alone). Tested by tests/turns.rs
  `a_free_turn_ends_when_the_flag_is_raised_{full,none}` (a raise from another thread after 50 ms ends a
  `b .` loop; a raise before the turn ends it at once).
- The library surface (23cdf0ad, 9f02646b, dcca457c; plan section 14 has it as built): `Runtime`,
  `RuntimeSettings`, `BackEnd`, `Machine`, `Container` (`step`, `idle`, `run`, `start_failure`), `Error`
  (`Refused { field, reason }`), `Image` (`tar`, `oci`, `spec`), `base_spec`, `System` (the shipped outside)
  and `Warned`, in crates/wcontainers-runtime; `Tuning` and `BackEnd for CpuSide<G>` in
  crates/wcontainers-wasmtime; no type of ours repeats one of oci-spec's (the configuration is
  `oci_spec::runtime::Spec`, images are read with `ImageIndex`, `ImageManifest`, `ImageConfiguration` and
  `Descriptor`).
- The refused fields and what each would take: plan section 14 lists them; tests/refused.rs has one test for
  each (56 refusals, among them a configuration without `mounts` or without `linux`, and the base
  configuration accepted: 57 passed). Fields the kernel would need to
  take at start are listed above under "Asked of the kernel track".
- The command line program (dcca457c): crates/wcontainers-wasmtime/src/bin/wcontainers.rs, calling only the
  library.

J3's bar (crates/wcontainers-wasmtime/tests/surface.rs, refused.rs; Docker's `alpine:3.22` linux/arm64):
- The pipeline `cat /etc/os-release | grep ^ID= | tr a-z A-Z; ls /bin | sort -r | head -3; echo one two three
  | awk '{print $3, $1}' | sed s/three/3/; seq 1 100 | grep 7 | wc -l; echo to error >&2; exit 7` through
  the public surface matches `docker run` (output, error, status 7) from the OCI image (`docker save`
  unpacked) and from a plain tar (`docker export`); the fields honoured beyond the program (host name,
  environment, working directory, additional groups, annotations) behave so; the command line program
  gives the same from both and exits 125 naming `process.terminal`; each refused field is refused by name;
  a process that never makes a system call (`while :; do :; done &`) is preempted by the outside's timer, so
  the shell goes on as on the reference.
- A second run in a new process from the disk cache (`a_second_process_starts_from_the_cache`, the pipeline
  from the OCI image, each run a new test process, the test build; dcca457c, load 34.2 to 63.4 over the
  suite; scratch j3/owned-1.log): the first, with an empty cache directory, made both ranges' translations
  and 92 pieces at run time, 9.61 s from reading the image to the end; the second found both ranges and 92
  pieces on disk and made 2 pieces (where a free turn stopped elsewhere), 0.268 s. One pair, so the times
  only show the order of the difference.
- The whole suite of what this track owns (`cargo test -p wcontainers-runtime -p wcontainers-wasmtime`, the
  test build): at ef6c7173 (the plan's text uncommitted) runtime 4 passed; back end 3 unit, cache 3, code 4,
  faults 2, fork 5, refused 57, surface 6 (and the cache run's child, ignored when run alone), turns 5, views
  6: 95 passed, 0 failed, 42.6 s with its build (load 31.8 to 47.5; scratch j3/owned-2.log). Its cache pair:
  first 7.38 s (both ranges and 88 pieces made), second 0.500 s (both ranges and 73 pieces from disk, 20
  pieces made). The earlier run at dcca457c: 92 passed (owned-1.log; the pair above).
- hello and busybox at ef6c7173: with the feature 5 passed, 1 ignored (root_file_system, the known
  failure), 100.1 s with its build (load 42.8 to 49.5; j3/hello-busybox-runtime.log); on the stand-in the
  same, 60.3 s (load 49.5 to 39.9; j3/hello-busybox-standin.log).

Measured: what counting at every stop check costs against free turns (release; scratch j3/cost/cost.sh and
counting_cost.rs, copied into the back end's tests for the run and removed; an outside that wraps `System` and
counts every turn with no limit, against `System`'s free turns; one runtime, a warm-up run of each, then 11
alternating pairs; dcca457c with docs uncommitted; load 38.0 to 39.7; j3/cost/cost-1.log):
- `echo hello` (busybox, a whole container from start to finish): free median 4.92 ms (min 4.57), counted
  5.30 ms (min 4.86), counted over free median 1.09;
- a busybox shell loop of 20,000 (`i=$((i+1))`): free median 263.6 ms (min 248.4), counted 484.3 ms (min
  450.9), counted over free median 1.81.
- So a hook call at every stop check nearly doubles a loop-bound program's time; the tests' counted turns
  are for repeatability only (plan section 9).

The CPU side's warnings per test, J2's bar at full on the merged tree (fff0aa77; per test file, its runs summed;
j3/m-warnings-full.txt from j3/m-bar-full.log by j2/scripts/warnings.py --targets):
- align: 1 runs; progress counted by stop checks 1
- faults: 8 runs; jump through a register not recognised 1, left run: the container finished 1, memory fault 4, progress counted by stop checks 8
- loader: 1 runs; left run: the container finished 1, progress counted by stop checks 1
- loaderrors: 3 runs; continued after stack loss 8, jump through a register not recognised 312, left run: another thread runs 1, left run: the container finished 3, loop not proven 8301, memory fault 4, progress counted by stop checks 8, translated at run time 34
- mapflags: 2 runs; jump through a register not recognised 3197, left run: the container finished 2, loop not proven 8, progress counted by stop checks 2, translated at run time 37
- memfd: 1 runs; jump through a register not recognised 1079, left run: the container finished 1, progress counted by stop checks 1, translated at run time 17
- memory: 12 runs; continued after stack loss 570, engine stack nearly full 8, jump through a register not recognised 283, left run: code changed under it 1, left run: the container finished 5, left to an address 2016, loop not proven 316, memory fault 2035, progress counted by stop checks 2040, translated at run time 83
- orphans: 1 runs; continued after stack loss 43, jump through a register not recognised 22, left run: another thread runs 18, left run: new registers 5, left run: the call waits 1, left run: the container finished 1, progress counted by stop checks 25, translated at run time 53
- privatemap: 1 runs; continued after stack loss 10, first write to a shared page 2, jump through a register not recognised 220, left run: another thread runs 4, left run: the container finished 1, left to an address 2, memory fault 13, progress counted by stop checks 18, translated at run time 33
- processes: 4 runs; continued after stack loss 1385, first write to a shared page 6, jump through a register not recognised 738, left run: another thread runs 676, left run: new registers 8, left run: the call waits 6, left run: the container finished 4, left to an address 2, loop not proven 1024, memory fault 12, progress counted by stop checks 706, translated at run time 246
- procsignals: 8 runs; continued after stack loss 1129, jump through a register not recognised 291, left run: another thread runs 511, left run: new registers 69, left run: the call waits 41, left run: the container finished 8, left run: the outside asked 14, loop not proven 18853, progress counted by stop checks 643, translated at run time 346
- script: 8 runs; continued after stack loss 7, jump through a register not recognised 812, left run: the container finished 7, left to an address 1, loop not proven 3811, memory fault 35, progress counted by stop checks 42, translated at run time 43
- scriptline: 1 runs; jump through a register not recognised 701, left run: the container finished 1, progress counted by stop checks 1, translated at run time 9
- sharedmap: 1 runs; first write to a shared page 10, jump through a register not recognised 943, loop not proven 312, memory fault 1, progress counted by stop checks 1, translated at run time 45
- sharedviews: 4 runs; code changed after it ran 4, continued after stack loss 12, first write to a shared page 8214, jump through a register not recognised 42, left run: another thread runs 4, left run: the container finished 4, left to an address 8, loop not proven 16411, progress counted by stop checks 8, shared page written through another page 8197, translated at run time 87, write next to another view made one instruction at a time 7
- shell: 38 runs; continued after stack loss 6733, exact floating point requested 2, jump through a register not recognised 2445894, left run: another thread runs 1700, left run: new registers 118, left run: the call waits 191, left run: the container finished 38, left to an address 38, loop not proven 1109231, memory fault 1106, progress counted by stop checks 3153, translated at run time 4890
- sigframe: 1 runs; continued after stack loss 34, jump through a register not recognised 712, left run: new registers 23, left run: the container finished 1, left to an address 7, loop not proven 639, memory fault 5, progress counted by stop checks 32, translated at run time 40
- sysvshm: 1 runs; continued after stack loss 20, first write to a shared page 4, jump through a register not recognised 1715, left run: another thread runs 11, left run: the container finished 1, left to an address 2, loop not proven 9, progress counted by stop checks 12, translated at run time 94
- terminal: 10 runs; continued after stack loss 1028, exact floating point requested 2, jump through a register not recognised 9879, left run: another thread runs 220, left run: new registers 44, left run: the call waits 94, left run: the container finished 10, left to an address 1, loop not proven 16232, memory fault 146, progress counted by stop checks 514, translated at run time 851
- unknown: 2 runs; jump through a register not recognised 80, left run: the container finished 2, loop not proven 32, progress counted by stop checks 2, translated at run time 18
- write: 2 runs; jump through a register not recognised 93, left run: the container finished 2, progress counted by stop checks 2, translated at run time 13
- zombie: 1 runs; continued after stack loss 25, jump through a register not recognised 519, left run: another thread runs 12, left run: the container finished 1, loop not proven 461, progress counted by stop checks 13, translated at run time 47

## Where J2 stood at its end

- J0 (the plan) and J1 (first light: hello and busybox's single programs) are done; their record is below.
- J2 run 1 (2026-11-18): the findings of J1 run 2's check are settled first (next section); then J2 is built:
  the cache's disk level with the translator's build identity, fork with the exact copy
  (`memory_duplicate`), `restored()` for the WebAssembly back end, layer 1 over the back end's memories,
  and the fix that made signals from faults work (registers the kernel sets after an exit handed on after
  the thread left were never written). J2's bar passes at full, none and bounded (below).
- Not built in J2, by the brief: the stand-in's count of the guest's own transfers and, with it, the loop's
  count of the kernel's changes of pc (the plan builds them together; both wait for the owner's ruling on
  the progress unit). The kernel's clock method has not landed: origin/phase-1 (2edbc451, "merged the
  kernel through k6b") and origin/phase-1-kernel (9568c985) still keep `monotonic` on the crate-private
  `Context`, so the clock hook still answers `Host::monotonic_nanoseconds` (see J2 run 3).
- origin/phase-1 is not merged into this branch in J2 (its K6b merge, 2edbc451, came after J2 started); J2's
  bars and checks all ran on the kernel at 36d20013.
- J2 run 2 (2026-11-18): the seven findings of J2 run 1's check are settled (next section) and the bars
  run again on the fixed tree (J2 run 2 below).
- J2 run 3 (2026-11-18): the five findings of J2 run 2's check are settled (next section). One stays a
  known failure, recorded: after a save is brought back on the WebAssembly back end the counter can go
  back, until the kernel makes its clock public (the kernel track's code).
- J2 run 4 (2026-11-18): the three findings of J2 run 3's check are settled (next section). The blocking
  one, a block of shared pages shown twice in one memory, is fixed exactly, also for a ring of one page
  mapped twice side by side; the new kernel test `sharedviews` joins J2's bar. The bars, layer 1 with a new
  seed, the stand-in's verdicts, the Linux check and the costs were run again on the fixed tree (J2 run 4
  below). Nothing of J2 is left open but what was before: the progress count (the owner's ruling) and the
  counter after a restore (the kernel's clock method).
- J2 run 5 (2026-11-18): the two findings of J2 run 4's check are settled (next section): the build identity
  now hashes every file of each hashed package's directory but its tests, so the ARM64 decoder's forms
  table is in it; the reference's stale code in one process is recorded.
- J2 is complete: every part of plan section 16's J2 that the brief lets be built now is built, its bar
  passes at full, none and bounded, layer 1 passes 10,000 seeded sequences over the back end, the million
  is one command (not run here; the orchestrator's), and system call and fork cost are measured with both
  sides of the copy. One test the stand-in cannot run still fails with the runtime: align's unaligned
  exclusive load, which the translator faults as Unicorn does; with the feature it is ignored as the
  translator's known failure. The other, align's misaligned stack pointer, runs and passes with the runtime.
  Layer 1 does not pass every seed: the kernel's `getcpu` (kernel track's code) differs from Linux with a bad
  cpu pointer and a good node pointer, on the stand-in too; and with J2's kernel a unix sequenced packet send
  after the peer closed with unread data gave EPIPE where Linux gives ECONNRESET (seed 6022140857, 40,000
  cases, on the stand-in too), which the kernel at K6b fixes (J3 run 1).

## Findings of J2 run 4's check, and how each was settled (J2 run 5)

- The build identity left out the ARM64 decoder's forms table (blocking): fixed (01ca6858). build.rs hashed
  and watched only each workspace crate's manifest and `src/`, and wcontainers-arm64/src/forms.rs includes
  `../arm/a64-forms.txt`, which decides which instructions are undefined. Now each hashed workspace crate
  gives every file of its package directory (cargo's own bound of what a package is built from: a crate
  cannot be packaged with an include from outside it), but the directories never built into a dependent
  (`tests`, `benches`, `examples`, `target`); each top-level entry is watched, a directory with every file
  in it. Checked (scratch j2run5/identity.sh, identity.log; load 57.5 to 52.1): a comment line appended to
  the forms table reran the script and changed the identity (4b9be68c... to 832345c4...); removing it gave
  4b9be68c... back; touching wcontainers-arm64/tests/exact.rs reran nothing; touching src/decode.rs reran
  the script with the identity unchanged. The output names arm/ once and no tests directory. The identity
  differs from run 4's (b09d7e24...), since the table is now in it, so every translation in a cache
  directory is made again once. Plan section 7 corrected. The back end's tests: 24 passed
  (j2run5/backend.log, load 41.7 to 35.3). J2's bar at full at 01ca6858, every translation made again under
  the new identity: 46 passed, 0 failed, 2 ignored (the same two), 222.0 s with its build (load 34.3 to 62.6;
  scratch j2run4/run5-full.log). The none and bounded bars, layer 1, the stand-in and the costs were not run
  again: the change is the identity's text alone, which only names cache files.
- The reference runs stale code in one process too (minor): recorded beside the child's case (J2 run 4's
  findings below, and Departures). The check's program (scratch check-j2-run4/zzcheck4b.c, part
  "codering") writes code through a writable `MAP_SHARED` view of a memfd that sits next to its executable
  view, from a `str` run in the executable view, and syncs with `__builtin___clear_cache` on the executable
  address. The reference (`alpine:3.22`, linux/arm64 in Docker on this Mac; `CTR_EL0` 0x99444c004, DIC 0,
  IDC 1, so `ic ivau` runs) gives `code ring: 0 0 22 33` six runs of six (check-j2-run4/ref/): the old
  instruction at the first rewrite, the new one after. Our back end gives `0 11 22 33` at full, none and
  bounded, what the architecture requires; the stand-in `0 0 0 0`. So the reference is not an oracle for code
  written through one view and run through another just after the sync, which matters for J6's JIT tests:
  such a test compares with what the architecture requires, or rewrites before the reference's first run.
  Nothing changed in the runtime.

## Findings of J2 run 3's check, and how each was settled (J2 run 4)

- One memory showing the same block's page at two places read stale bytes (blocking): fixed (2fa76074,
  88ecb185). Each memory held one copy per place shown, and a write through one place reached the others
  only at the next exit. Now, while the guest writes through one place, the memory's other places of that
  page carry a new lock, the "view" lock, that takes reading as well as writing; reaching one of them
  faults to the back end, which makes the copies first (counted "shared page written through another
  page"). `load_code` makes them first too, before it reads code. A page whose neighbouring page shows the
  same block's page (a ring of one page mapped twice side by side) could not take a write that crosses
  from one into the other, since each copy would hold only its part: such a write is made one instruction
  at a time (counted "write next to another view made one instruction at a time"). Both pages are
  unlocked; the slot of the instruction's address is pointed at its own piece in the exact slot and the
  slot of the next address (from `Guest::decode`'s length) is emptied, so the lookup runs that instruction
  alone and then calls `load_code`, where the step ends and the copies are made, each page giving the
  bytes it changed against the block. Also fixed on the way: `ic ivau` on a page that shows a block now
  drops that line's pieces wherever the block is shown, in every memory (a parent ran its old code after a
  child rewrote shared code). A first try that refused, as not supported, any block's page shown next to
  itself writable is not kept: `mmap` places two one-page mappings of one file side by side, so it refused
  common programs.
  Tests: the back end's tests/views.rs (6: write through one place and read through the other, rings of
  two pages and of one page with a store across the seam, each at full and none), and the kernel test crate's
  guest/sharedviews.c with tests/sharedviews.rs (a7197a7b, 27ff0eac, its own commits): "data" (the
  check's two views, and a child writing through one view), "ring" (two pages), "ring1" (one page), "code"
  (code written through a writable view and run through an executable one; shared code rewritten by a
  child). With the runtime all four pass at full, none and bounded (scratch j2run4/views-runtime-5-*.log;
  load 20.3 to 22.9). On the stand-in "code" fails (`code views: 1 1 1` against `1 2 3`, Unicorn's stale
  code, the check's second finding), so without the feature it is ignored as the stand-in's known failure;
  the other three pass (views-standin-4.log, load 22.1). Before the fix the runtime failed "data" and "ring"
  as the check found, and "code" died with SIGILL (views-runtime-old.log: exit 132).
  The reference's child, running code it had just rewritten in shared memory and synced with
  `__builtin___clear_cache` (which runs `dc cvau`, `ic ivau` and `isb` as `CTR_EL0` asks;
  j2run4/sharedviews.dis), ran the old code (the check's "child 77"), while the parent then ran the new.
  Both back ends run the new code there, as the architecture says. That case is left out of sharedviews (the
  child exits with what it ran before rewriting) and recorded here, unexplained. J2 run 4's check found the
  same in one process with no fork (J2 run 5's findings above).
- The stand-in runs stale code in two cases (minor, information for the kernel test crate): recorded; with
  the shared code part changed as above, the stand-in now fails only the code views line, and "code" is
  ignored without the feature for it.
- The build identity parsed the lock file and manifests by hand (minor): build.rs now reads the workspace
  with `cargo metadata --no-deps` (the cargo_metadata crate: each member's manifest path and its
  dependencies with their kinds, as cargo reads them) and the lock file with the cargo-lock crate
  (`Lockfile::load`, `Dependency::matches`), and walks normal and build dependencies from the translator and
  every guest crate (a workspace crate built on the guest interface and not on the translator), hashing the
  sources of each workspace crate (since run 5 every file of its package directory but tests) from its
  manifest's own directory (no assumption that directory and
  package names agree) and the locked entry (name, version, source, checksum) of every other. It hashes the
  same 54 packages as before (4 workspace crates, 50 others; listed once with a build warning, then removed).
  The check's suggestion, cargo's full resolved graph (`cargo metadata` without `--no-deps`), was built
  first (df7cab06) and replaced: it needs every package of the whole workspace present, so on a fresh
  registry the back end's build failed with `--offline` ("failed to download adler2", scratch
  j2/linux/linux-check-6.log) and would fetch from the network without it. With the lock file read as
  it is, nothing is fetched: the Linux check passes (linux-check-7.log, 24 tests, load 47.0 to 43.7).
  Checked again: touching a kernel test file rebuilt nothing; touching an arm64 source reran the script.

## J2 run 4: the bars on the fixed tree (this Mac, Apple M5 Max; other tracks running)

The back end of every run below is 88ecb185's (later commits change only the build identity's way of
finding its crates, which hashes the same 54 packages, and docs); logs in scratch j2run4/, made by
j2run4/all.sh at 27ff0eac with the docs uncommitted.
- J2's bar with sharedviews (j2run4/bar.sh): full 46 passed, 2 ignored, 92.7 s (load 23.2 to 35.3;
  bar-full.log); none 46 passed, 2 ignored, 195.0 s (load 35.3 to 50.6; bar-none.log); bounded 46 passed, 2
  ignored, 151.2 s (load 50.6 to 74.1; bar-bounded.log). The 46 are run 3's 42 and sharedviews' 4; ignored
  as before: align's exclusive load (the translator's known failure) and memfd's huge pages.
- Layer 1 over the back end with a new seed: 10,000 sequences, seed 2026111804, 32 on each file backend, 3
  passed in 172.7 s (load 74.1 to 34.2; layer1-10000-2026111804.log).
- The stand-in: the whole kernel test crate without the feature, 568.3 s (load 34.2 to 20.3;
  verdicts.log, verdicts.txt): 195 passed, 0 failed, 6 ignored. Against run 3's verdict lines the
  differences are sharedviews' four (three ok, "code" ignored as the stand-in's known failure) and
  `threads_match_reference_on_backend_root`, which failed on the reference's side in run 3 and passes here.
- The back end's own tests on this Mac: 24 passed (backend-2.log, load 18.9 to 20.3). On Linux (Docker,
  linux/arm64): 24 passed, the paged out page "present 0, swapped 1" (j2/linux/linux-check-7.log, a scratch
  worktree of 27ff0eac with the cargo-lock build script, 5979be8b's; load 47.0 to 43.7); its target,
  registry and image are deleted. At 5979be8b the back end's tests on this Mac: 24 passed (backend-3.log,
  load 28.6 to 26.2).
- System call and fork cost again, since every exit's copy step changed (release; j2/cost/runtime_cost.rs
  and copy_cost.rs copied in for the run and removed, the same method as runs 1 and 2; scratch
  j2run4/cost/cost.sh, at 5979be8b; cost-1.log and copy-1.log): `getppid` ours 155.0 ns an iteration (min
  140.2), the loop with no call 7.8 ns, reference 140.3 to 158.0 ns (load 41.2); fork, `_exit` and
  `waitpid` with 448 heap pages ours 230.8 µs (min 207.9), reference 175.4 to 284.2 µs (load 41.2 to 39.2);
  fork then rewriting the 448 pages ours 355.3 µs (min 328.4), reference 634.6 to 745.5 µs; the rewrite
  alone ours 14.0 µs, reference 4.5 to 7.8 µs (load 39.2 to 37.7). Both sides of the copy on the back end
  alone (J0's busybox sh layout, 446 pages written, medians of 15): this run, at load 29.7,
  `memory_duplicate` 74.2 µs (min 65.2), the parent's first writes after it 214.2 µs, the same writes with no
  copy 18.4 µs. Repeated by the checks at higher load, the copy's medians range 85.5 to 124.6 µs (min 87 to
  94 at load 49 to 52) and the first writes 249 to 327 µs (check-j2-run3: 85.5 and 103.0 µs;
  check-j2-run4/cost-copy*.log: 103.1 to 124.6 µs; check-j2-run5/cost/copy-1..4.log: 98.5 to 123.3 µs, first
  writes 249.3 to 327.2 µs, no copy 18.5 to 29.5 µs, load 49.1 to 51.5). So the copy is about 75 to 125 µs on
  this Mac, depending on load. Run 2's figures: 163 ns, 319 µs; the copy 71 to 95 µs after the records
  change.

## Findings of J2 run 2's check, and how each was settled (J2 run 3)

- The counter goes backwards after a save is brought back (blocking): partly fixed, the rest a known failure
  recorded here and listed for the owner. Cause: the test host's clock reads from 0 again after each save
  (driver.rs `TestHost::moved`, on purpose: a host that started later), the kernel carries its own clock on
  through `monotonic_offset` (kernel lib.rs:410), and the clock hook answers `Host::monotonic_nanoseconds`
  without that offset, which only the kernel holds and keeps crate-private. The runtime cannot compute it
  from anything public, so the exact fix is the kernel's one method (plan section 20), the kernel track's
  code, not changed here. Done now: the slow path the plan named was not built; it is (f685bfdf): a back
  end made by `restored()` counts and warns every counter read, "counter read without the kernel's offset",
  sent to the outside at the read. Tested by tests/turns.rs `counter_reads_after_a_restore_are_warned`
  (and not warned on a back end never restored). The check's program is now the kernel test crate's
  guest/counterclock.c with tests/counterclock.rs (2dcdad96, a test file and a guest program of that crate,
  its own commit): with no saves it passes on both; across 3 saves it passes on the stand-in and fails with
  the runtime (`seed 1: ours "counter backwards 3"`, `theirs "counter backwards 0"`; 3963 reads warned), so
  with the feature it is ignored as the back end's known failure, naming the reason (scratch
  j2run3/counter-standin.log, counter-runtime.log with `--include-ignored`; load 47.9 to 42.8). Plan
  sections 4, 11 and 16 J2 corrected (7b3c359c): they said the hook answers from the kernel's clock.
- The build identity hashed only wasm-encoder's locked entry, by splitting the lock file's text (minor):
  build.rs now reads Cargo.lock with toml_edit and follows it from the translator and every guest crate,
  through each workspace crate's normal and build dependencies (from its manifest) and each other crate's
  locked dependencies, hashing the sources of every workspace crate reached and the name, version, source
  and checksum of every other crate reached (6270d0f0). Today it reaches 4 workspace crates (translator,
  guest interface, abi, arm64) and 50 locked crates, among them yaxpeax-arm, yaxpeax-arch, rustc_apfloat,
  petgraph and wasm-encoder (listed once with a build warning while writing it, then removed).
- The build script watched the whole crates tree (minor): it now watches the lock file (a crate added to or
  removed from the workspace changes it), each crate's manifest, and the hashed crates' sources (6270d0f0).
  Checked: adding a file under crates/wcontainers-kernel-test/tests and guest/ rebuilt nothing; a line added
  to crates/wcontainers-arm64/src/lib.rs reran the script and rebuilt the back end.
- The status file's stale parenthesis about origin/phase-1 (minor): corrected above.
- A cache file refused was not counted (minor): a file that exists but cannot be read, is not whole, or that
  wasmtime refuses is now a counted, warned slow path, "translation in the cache's directory refused", with
  the host's error number as the detail (`u64::MAX` when there is none) and the time spent; the translation
  is made again and written in its place (f685bfdf). The not-kept slow path's name is now a constant
  too. Tested by tests/cache.rs `a_file_that_is_refused_is_a_slow_path` (every file cut in half: made again
  and warned; then a new compiler finds it on disk).

## J2 run 3: the bars on the fixed tree (this Mac, Apple M5 Max; other tracks running)

The code of every run below is f685bfdf's back end with 2dcdad96's test (later commits change only docs).
- J2's bar (scratch j2/bar.sh): full 42 passed, 2 ignored, 129.9 s (load 30.9 to 70.5; j2/run3-full.log, at
  7b3c359c); none 42 passed, 2 ignored, 218.6 s (load 70.5 to 92.1; run3-none.log, at 01ea3121); bounded 42
  passed, 2 ignored, 140.6 s (load 92.1 to 54.5; run3-bounded.log, at 01ea3121). Ignored as in run 2:
  align's exclusive load (the translator's known failure) and memfd's huge pages.
- The back end's own tests on this Mac: 18 passed (load 54.5 to 60.2; j2run3/backend.log). On Linux (Docker,
  linux/arm64, 7.0.14-linuxkit; scratch j2/linux/check.sh): 18 passed at 7b3c359c, the paged out page
  "present 0, swapped 1" (load 32.0 to 93.2; j2/linux/linux-check-5.log); its target, registry and image
  are deleted.
- The stand-in: the whole kernel test crate without the feature, 540.1 s (load 60.2 to 69.0;
  j2run3/verdicts.log, verdicts.txt): 191 passed, 1 failed, 5 ignored. Against run 2's verdict lines the
  differences are the two new counterclock tests (ok) and `threads_match_reference_on_backend_root`, which
  failed on the reference's side (its run gave "exec /wc/threads: no such file or directory", exit 255;
  ours matched the usual output) and passes when run again alone (load 55.6 to 49.2;
  j2run3/threads-rerun.log). The driver did not change in this run.
- Not run again, since nothing they depend on changed: layer 1 (it uses the back end's memories and copies,
  not the clock hook or the cache) and the system call and fork costs (the clock hook's added test is on
  counter reads only, and the call and fork paths did not change).

## Findings of J2 run 1's check, and how each was settled (J2 run 2)

- Fork copy not exact on Linux after the host process forked (blocking): `/proc/self/pagemap` was opened
  once per process and kept, and an open page map stays bound to the process that opened it. memory.rs
  `copy_pages` now takes every run of one copy and opens the page map for each copy (9ea8a8aa). New test
  tests/fork.rs `a_duplicate_made_after_the_host_process_forked_is_exact` (duplicate once, fork, then in the
  copy write 256 pages of a new memory and duplicate it). On Linux it fails on the code before the fix
  (`status 0x100`; scratch j2/linux/linux-before-fix.log, the test copied into a worktree of a3bb4728) and
  passes at the fix (linux-check-2.log, e98abb5e) and after the records change (linux-check-3.log, 7679fe4b).
- getcpu with a bad cpu pointer and a good node pointer (minor): not fixed here. It is the kernel track's
  code (crates/wcontainers-kernel/src/syscall/system.rs `getcpu`, still the same on origin/phase-1-kernel at
  940e9868), not the back end's, and the buffer CPU fails the same seed the same way. Linux writes the node
  and then returns EFAULT; ours returns at the first failed write. Reproduce: `LAYER1_CASES=20000
  LAYER1_SEED=918273645 cargo test -p wcontainers-kernel-test --test layer1 -- sequences_match_reference`
  (with or without `--features runtime`). Listed for the owner.
- align under the feature (minor): with `--features runtime` the misaligned stack pointer test runs (not
  ignored) and passes; the unaligned exclusive load is ignored as the translator's known failure, named so
  (e98abb5e, a kernel test crate test file, its own commit). Without the feature both keep Unicorn's
  reasons. `--include-ignored` with the feature: stack pointer ok, exclusive load FAILED (scratch
  j2run2/align-runtime.log, load 47.9 to 53.3).
- The build identity named only the ARM64 guest (minor): build.rs now hashes the translator, the guest
  interface, the abi, and every guest crate of the workspace, found from the manifests (a crate that
  depends on `wcontainers-guest-cpu` and not on `wcontainers-translator`; today that finds
  `wcontainers-arm64`), and reruns when a crate is added (1bee906a). The manifests are read with toml_edit.
- Fixed cost of every fork (minor): the per-page records (permissions, write locks, may hold data) are one
  record per page, kept in chunks of 4096 pages that copies share until one side changes one, and the
  permission table is copied only for the records' chunks (7679fe4b). Measured (release, scratch
  j2run2/cost/pairs.sh, binaries of the commit before and after, five alternating pairs; Apple M5 Max, load
  45.5 read at the start and the end; pairs-1.log): a memory with two written pages duplicated, median of
  41: before 69.9 to 75.7 µs, after 29.4 to 37.1 µs; `memory_new` alone before 27.8 to 32.7 µs, after 16.7 to
  20.8 µs. J0's busybox sh layout, median of 15: before 109.5 to 151.0 µs, after 70.7 to 95.2 µs; the
  parent's first writes after it before 185.7 to 268.5 µs, after 194.9 to 269.1 µs (unchanged). Tested by
  the records' unit test in lib.rs and every fork test.
- A cache directory that cannot be written panicked (minor): writing a file is now a counted, warned slow
  path, "translation not kept in the cache's directory", with the host's error number as the detail; the
  partial file is removed and the run goes on (9ea8a8aa). tests/cache.rs gives the compiler a path under a
  file.
- The Linux check's command was not in its log (minor): scratch j2/linux/check.sh SOURCE LOG runs it and
  writes the docker command into each log's header ("How to run").
- Found in this run, not by the check: a save's copy could hang for ever. The first bounded bar of run 2
  (scratch j2/run2-bounded-hung.log, 7679fe4b) stopped in procsignals' `injected_signals_change_nothing`:
  the deepest copy waited on a mutex while dropping the back end it was brought back from (`*cpu =
  cpu.restored()` in the driver), inside rayon's `ThreadPool::drop` waking threads that are only in the
  parent; a parent thread held that lock at the fork (stacks: j2run2/sample-*.txt). The compiler now drops
  its pool only in the process that made it, and leaves it in a copy (a7c310e9). New unit test
  `a_compiler_copied_by_fork_is_dropped_there` (400 forks while the pool's threads wake): before the fix it
  hung and failed at its 20 s limit (j2run2/drop-before-fix.log); after it, four runs pass in about 2.4 s
  each (drop-after-fix.log). The full and none bars that had passed at 7679fe4b are kept in
  j2run2/superseded/; the bars were run again at the fix (below).
- Found next, by the bars at a7c310e9: the copy could still hang, now inside `Compiler::new` in the copy,
  compiling the thread module: wasmtime registering its unwind tables waited on libunwind's lock
  (j2run2/sample2-62043.txt; j2run2/superseded/run2b-none-hung.log). Another test thread of the same test
  binary (procsignals runs its four tests three at a time) held that lock at the fork. Any lock another
  thread holds at a fork is lost to the copy, so this is the tests' save, not the back end: the driver
  now makes a run that saves hold a process-wide lock alone, and every other run share it, while it has
  a back end (44ff7829, the driver, with the feature only). A saving run then never overlaps another run
  of its binary; the other tests of the binary wait for it.

## J2 run 2: the bars at c2539861 (this Mac, Apple M5 Max; other tracks running)

- J2's bar (scratch j2/bar.sh): full 42 passed, 2 ignored, 86.2 s (load 98.5 to 96.8; j2/run2c-full.log);
  none 42 passed, 2 ignored, 83.1 s (load 96.8 to 82.1; run2c-none.log); bounded 42 passed, 2 ignored,
  168.3 s (load 82.1 to 103.0; run2c-bounded.log). The 42 are run 1's 41 and align's stack pointer test;
  ignored are align's exclusive load (the translator's known failure) and memfd's huge pages.
- procsignals, the bar's test that saves, five more times at none (j2run2/procsignals-repeat.sh 5 none):
  4 passed each time, 5.3 to 6.8 s each (load 62.4 to 87.1; j2run2/procsignals-none-1..5.log).
- Layer 1 over the back end with a new seed: 10,000 sequences, seed 2026111802, 32 on each file backend,
  3 passed, 179.0 s (load 103.0 to 62.4; j2/layer1-10000-2026111802.log).
- The back end's tests on Linux (Docker, linux/arm64, 7.0.14-linuxkit): 16 passed at fbcf5e37, whose
  back end is c2539861's (load 58.2 to 91.8; j2/linux/linux-check-4.log); the paged out page read
  "present 0, swapped 1" and was copied exactly.
- The back end's own tests on this Mac: all pass at c2539861.
- The stand-in unchanged: the whole kernel test crate without the feature at 0b4ffb30 (code as c2539861),
  609.3 s (load 90.2 to 54.7; j2run2/verdicts.log): 190 passed, 0 failed, 5 ignored, and the sorted verdict
  lines (j2run2/verdicts.txt) are identical to run 1's (j2/verdicts.txt).
- System call and fork cost again, end to end (release, j2/cost/runtime_cost.rs copied in for the run and
  removed, the same method as run 1; c2539861; load 101.8 to 96.5; j2run2/cost/cost-3.log): `getppid` ours
  162.9 ns an iteration (min 147.3), the loop with no call 9.6 ns, reference 143.3 to 212.2 ns; fork, `_exit`
  and `waitpid` with 448 heap pages ours 319.0 µs (min 249.1), reference 312.1 to 694.8 µs; fork then
  rewriting the 448 pages ours 656.8 µs (min 398.8), reference 790.7 to 1395.3 µs; the rewrite alone ours
  13.9 µs, reference 4.4 to 5.8 µs. Run 1 measured fork 338.8 and 739.7 µs; at this load one run does not
  separate the records change (about 40 µs a fork, measured apart above) from the variation.


## J2 (run 1, 2026-11-18)

Built (crates/wcontainers-wasmtime unless named):
- The cache's disk level: `Compiler::new(Some(directory))`, one file per key (the translator's build identity
  from build.rs over the sources of the translator, guest interface, ARM64 guest and abi crates and their
  locked wasm-encoder; the back end; the guest; the host probe's answer; the offer; look-ahead and
  optimisation; the memory widths; the code's hash; the starts), written whole by rename, loaded with
  wasmtime's own compatibility check. `Translated::origin` says memory, disk (with its time) or made (with
  translating and compiling times). Tested by tests/cache.rs.
- `memory_duplicate`: the parent's written shared pages copied first, then every run of pages that may hold
  data (a per-page bit) copied by memory.rs `copy_pages` (macOS `mach_vm_copy`; Linux `/proc/self/pagemap`
  present or swapped pages with `memcpy`; other hosts `memcpy`), with the parent's per-page record, blocks
  shown, permission table and slot entries of every code range; the child keeps the parent's translations at
  the same slot bases. Tested by tests/fork.rs (exact, its own afterwards, the block still shared, the
  child runs the parent's translation without translating) and memory.rs's unit test.
- `restored()`: the same copy into a back end with a new compiler (its own pool) and nothing translated;
  thread blocks copied. Tested by tests/fork.rs, also in a fork copy of the test process with rayon's
  global pool started, where it compiles new code and runs it (this test hung for 120 s and failed with the
  probe compiled outside the back end's pool, which was the bug).
- Registers set by the kernel in answer to an exit handed on after the thread left (a fault's signal) are
  written before the thread runs again (tests/faults.rs); before, sigframe and privatemap died at their
  first handled fault.
- The kernel test crate (its own commits): fa0b5bac gives the tests' back end a cache directory,
  target/wc-kernel-test/translations, and reports each translation's origin; 0b6fefdb runs layer 1 over
  the WebAssembly back end with the feature (layer1.rs's CPU type and the driver's `Back` and `back_end`
  made crate-visible).

The bar (scratch j2/bar.sh NAME full|none|bounded: the 21 listed tests with `--features runtime`, three at a
time; this Mac, other tracks running):
- full: 41 passed at 0b6fefdb, 70.1 s (load 48.8 at the start, 51.2 at the end; j2/full-2.log);
- none: passed at 0b6fefdb, 226.0 s (load 58.3 at the start; j2/none-1.log);
- bounded: passed at 116b8410 (code as 0b6fefdb, only tests and docs after), 116.5 s (load 88.1 to 81.3;
  j2/bounded-2.log); N is drawn per run from its seed and `LOOK_AHEAD_SEED` 0.
- Ignored in these tests, all as with the stand-in: align's two (known stand-in failures, Unicorn's
  alignment checks) and memfd's huge pages (not built in the kernel). Run with `--ignored` and the
  feature: align's misaligned stack pointer passes against the
reference; align's unaligned exclusive load fails (ours exit 135, `SIGBUS`; the reference runs it): the
translator raises an alignment trap for an unaligned exclusive load, as Unicorn does. That is the
translator's code, not changed here; the test stays ignored with the stand-in's reason (j2/ignored-full.log,
load 77.0).
- Earlier runs, kept as evidence: j2/full-1.log (before the fixes: five failures, a cache file race,
  the held registers, and procsignals' copies hanging on the probe); j2/bounded-1-stale-build.log
  (procsignals hung: its test binaries were built while the source held, for the 120 s check above, the
  probe outside the pool).

The stand-in unchanged: the whole kernel test crate without the feature at 116b8410 (only this file
changed), 550.9 s (load 61.8 to 41.2; j2/verdicts.log): 190 passed, 0 failed, 5 ignored, and its sorted
verdict lines (j2/verdicts.txt) are identical to J1 run 2's (j1run2/verdicts.txt).

Layer 1 over the back end (`LAYER1_CASES`, `LAYER1_SEED`; scratch j2/layer1.sh CASES SEED [BACKEND_CASES],
one command; the long run is `j2/layer1.sh 1000000 <seed>`): 10,000 sequences with seed 2026111801, and 32 on the file
backend and 32 on the cached backend, all match the reference: 3 passed in 177.3 s (commit 116b8410, only
this file changed; load 65.5 at the start, 77.0 at the end; j2/layer1-10000-2026111801.log). Earlier: 500
with seed 1 in 9.0 s (load 20.6). About 18 ms a sequence, so the million is expected (not measured) to take about five hours.

Linux (Docker `rust:1.99-slim`, linux/arm64, kernel 7.0.14-linuxkit, 4 CPUs, swap 4 GB; the back end's
tests at 0c084598; load 72.5 to 75.0; j2/linux/linux-check.log): all 13 pass: host-supplied memory, blocks
of shared pages, the fork copy through `/proc/self/pagemap`, a back end brought back in a fork copy. In
`copy_pages`'s test the page given `MADV_PAGEOUT` read "present 0, swapped 1" and was copied exactly. That
run did not test a copy made after the host process forked, which was not exact (fixed in run 2, above).

The CPU side's warnings per test in J2 run 1 (per test file, its runs summed; j2/warnings-full.log at full,
made by j2/scripts/warnings.py --targets; J3 run 1 has them for the final tree):
Commit 116b8410 with only this file changed; 57.7 s, load 71.4 to 65.5. "progress" is runs of `run` (warned
once each); "finished", "another thread", "the call waits", "new registers", "the outside asked" and "code
changed under it" are the leaves of `run`. align has no runs (both its tests are ignored).
- faults: 8 runs; jump through a register 1, finished 1, memory fault 4, progress 8
- loader: 1 runs; finished 1, progress 1
- loaderrors: 3 runs; stack loss 8, jump through a register 312, another thread 1, finished 3, loop not proven 8301, memory fault 4, progress 8, translated at run time 34
- mapflags: 2 runs; jump through a register 3197, finished 2, loop not proven 8, progress 2, translated at run time 37
- memfd: 1 runs; jump through a register 1079, finished 1, progress 1, translated at run time 17
- memory: 12 runs; stack loss 570, engine stack nearly full 8, jump through a register 283, code changed under it 1, finished 5, left to an address 2016, loop not proven 316, memory fault 2035, progress 2040, translated at run time 83
- orphans: 1 runs; stack loss 43, jump through a register 22, another thread 18, new registers 5, the call waits 1, finished 1, progress 25, translated at run time 53
- privatemap: 1 runs; stack loss 10, first write to a shared page 1, jump through a register 220, another thread 4, finished 1, left to an address 2, memory fault 13, progress 18, translated at run time 33
- processes: 3 runs; stack loss 1367, first write to a shared page 6, jump through a register 392, another thread 670, new registers 8, the call waits 6, finished 3, left to an address 1, loop not proven 506, memory fault 7, progress 694, translated at run time 203
- procsignals: 8 runs; stack loss 1129, jump through a register 291, another thread 511, new registers 69, the call waits 41, finished 8, the outside asked 14, loop not proven 18853, progress 643, translated at run time 346
- script: 8 runs; stack loss 7, jump through a register 812, finished 7, left to an address 1, loop not proven 3811, memory fault 35, progress 42, translated at run time 43
- scriptline: 1 runs; jump through a register 701, finished 1, progress 1, translated at run time 9
- sharedmap: 1 runs; first write to a shared page 7, jump through a register 943, loop not proven 312, memory fault 1, progress 1, translated at run time 45
- shell: 38 runs; stack loss 6439, exact floating point requested 2, jump through a register 2028137, another thread 1699, new registers 119, the call waits 198, finished 38, left to an address 4, loop not proven 1109896, memory fault 570, progress 2624, translated at run time 2285
- sigframe: 1 runs; stack loss 34, jump through a register 712, new registers 23, finished 1, left to an address 7, loop not proven 639, memory fault 5, progress 32, translated at run time 40
- sysvshm: 1 runs; stack loss 20, first write to a shared page 4, jump through a register 1715, another thread 11, finished 1, left to an address 2, loop not proven 9, progress 12, translated at run time 94
- terminal: 10 runs; stack loss 1028, exact floating point requested 2, jump through a register 9879, another thread 220, new registers 44, the call waits 94, finished 10, left to an address 1, loop not proven 16232, memory fault 146, progress 514, translated at run time 851
- unknown: 2 runs; jump through a register 80, finished 2, loop not proven 32, progress 2, translated at run time 18
- write: 2 runs; jump through a register 93, finished 2, progress 2, translated at run time 13
- zombie: 1 runs; stack loss 25, jump through a register 519, another thread 12, finished 1, loop not proven 461, progress 13, translated at run time 47

System call and fork cost (this Mac; release build; scratch j2/cost/: cost.c, runtime_cost.rs and
copy_cost.rs, copied into the crates' tests/ for the run and removed; logs cost-1.log, cost-2.log, copy-1.log;
commit 116b8410, only this file changed). Ours is the host's time of a whole run with N iterations less one
with none, over N, median of 7 alternating pairs, on the kernel tests' outside: counted turns, so every stop
check is a hook call (the plain loop below). The reference is the program's own clock, 5 runs, in Docker's
Linux VM on this Mac. Measured:
- `getppid` in a loop: ours 227.7 ns an iteration (min 153.3; load 64.8) and again 163.2 ns (min 152.9; load
  42.7); the same loop with no call 9.2 ns and 8.8 ns, so the call itself about 155 to 218 ns. Reference
  139.9 to 146.2 ns, again 151.2 to 334.7 ns. unkernel 56 ns (the brief's figure). J0 expected about 130 ns.
- fork, the child's `_exit`, the parent's `waitpid`, with 448 heap pages written once before the loop: ours
  338.8 µs (min 323.7; load 64.6) and again 739.7 µs (min 499.4; load 45.3); reference 178.8 to 212.4 µs,
  again 238.8 to 980.0 µs. unkernel 20 µs. The two runs of ours differ twofold under this load.
- both sides of the copy, on the back end alone (copy_cost.rs: a process laid out as J0's busybox sh case,
  four ranges, 11,200 KB given access, 446 pages written; medians of 15, two runs; load 44.3 and 41.7):
  `memory_duplicate` 125.1 and 131.0 µs (min 104.4 and 122.1), all of it: the new store, guest and state
  memories, the per-page record's copy and `mach_vm_copy` of every page given access; the parent's first
  writes to its 446 pages after it 220.5 and 236.5 µs, against 10.8 and 11.2 µs for the same writes with no
  copy (each first write a fault the host settles, about 0.5 µs a page).
- the same end to end in a program: fork then rewriting the 448 pages 421.5 µs (load 64.6), again 514.1 µs;
  the rewrite alone 13.0 and 11.6 µs; reference 547.0 to 658.5 µs and 731.8 to 1056.3 µs, rewrite alone 3.5
  to 4.5 and 4.0 to 5.1 µs. Under this load the end-to-end difference does not separate the first writes
  from the fork's own variation; the back end's figures above do.
- Expected only, not measured apart: most of ours' fork beyond the copy is instantiating the child's code
  modules at its first run; J7 measures where it goes and brings it down.

## Findings of J1 run 2's check, and how each was settled (J2 run 1)

- Hand-rolled RELR and symbol counts: starts.rs now reads `RELR` with `object::read::elf::RelrIterator` and
  counts symbols with `HashTable` / `GnuHashTable::symbol_table_length`, over the bytes from the table to the
  end of its loaded segment's file part (ea2392dd). The hand-written decoder and its own test are gone.
  The starts by source are unchanged: busybox `text` at full (j2/starts-check.log, load 37.5 to 35.3) gives
  musl 1538 starts (symbols 1611, unwind 22, relocations 45) and busybox 559 (relocations 607), as in J1.
- The exact slot's piece not timed: it is now counted "translated at run time" with its time (b6f2415a).
  The run-time piece's time now starts at that piece, not at the hook's entry, so a whole range's first
  translation in the same call is not counted into it.
- Any overlapping notice marked code changed: a range now keeps the hash of the bytes it was last translated
  from, carried across notices of the same range; it is "changed" only when its bytes at first use differ
  from that hash, or when a notice for a different extent finds the old range's bytes changed (b6f2415a).
  tests/code.rs: a permission-only notice leaves it unchanged; new bytes written between two notices, as a
  program making code does, count once and run.
- Warnings per test: J1's are now listed per test from run 2's log at a572aa76 (busybox, one test at a time,
  j1run2/busybox-leaves.log; summed per test, j2/j1-warnings-per-test.txt):
  - busybox::text, 50 runs: progress counted by stop checks 350, loop not proven 29,588, jump through a
    register not recognised 41,316, continued after stack loss 230, translated at run time 423, memory fault
    250, left run: the call waits 50, the container finished 50;
  - busybox::files, 60 runs: progress 1489, loop not proven 38,754, jump through a register 3,681,846, stack
    loss 2370, translated at run time 643, memory fault 300, the call waits 1129, finished 60;
  - busybox::system, 66 runs: progress 593, loop not proven 47,364, jump through a register 144,070, stack
    loss 499, translated at run time 682, memory fault 384, the call waits 137, another thread runs 6,
    finished 66, left to an address 2;
  - busybox::one_cpu, 1 run: progress 6, loop not proven 555, jump through a register 28, stack loss 1,
    translated at run time 5, memory fault 5, finished 1.
  J2's are in the J2 section, per test file.
- Plan inaccuracies: section 14 now says wasmtime 50 brings its own object 0.40 and gimli 0.33 (two versions
  of each in a build), and names `process.user.umask` (86f98584).
- Build output left in scratch: j1/after/src/target and j1/baseline/src/target (1.6 GB each, the tests'
  `target/wc-kernel-test` under the copies' sources) are deleted. Each run now deletes every target folder
  under its scratch copies, `src/target` included.
- busybox awk's "jump through a register not recognised" (information for J6 and J7): recorded here; the
  check measured 26,679,107 in one awk run at full. Nothing changed in J2.

## J1 record

- J0 (the plan) is done: docs/runtime.md. J1 was built in run 1 (2026-11-17); run 2 (2026-11-18) fixed the
  findings of J1's independent check (below) and ran the bars again on the fixed, committed tree.
- First light, 2026-11-17 23:2x: a program ran through both halves for the first time. The static hello
  passes against the reference with the kernel deciding and the translator's WebAssembly, run by wasmtime,
  running its instructions, at full, none and bounded look-ahead:
  `nix develop -c cargo test -j 4 -p wcontainers-kernel-test --features runtime --test hello -- --test-threads 3`
  (and with `LOOK_AHEAD=none`, `LOOK_AHEAD=bounded` in front). Pushed at that point.
- Busybox's single programs pass at full look-ahead (`--test busybox`: text, files, system, one_cpu; the
  known failure root_file_system stays ignored as with the stand-in), each command against the reference with
  the root in memory and on the backend: 4 passed, 1 ignored, 66.9 s (load 10.5 at the start, 26.2 at the end;
  scratch j1/busybox-full-1.log).
- Built in J1:
  - crates/wcontainers-runtime: `Stepper::step` and `drive` (the one loop), `Execution` (with `Failure` and
    `restored`), `Outside` (with `End`; `turn`, `ran`, `arrived`, `stopped`, `idle`, `cpu_warning`),
    `Answer`, `Carry` (with `Leaving`), `Turn` (counted turns only; free turns come with J3's timer), `Step`, `Ending`.
  - crates/wcontainers-wasmtime: `Compiler` (engine with host-supplied memories and Unix signal traps, the
    thread module, the back end's own rayon pool, the probe, the in-memory cache) and `CpuSide<G>` (every
    `Cpu` method but `memory_duplicate`, `run` with carrying on in place and leaving, the hooks, translation
    at first use from the kernel's starts and the image's own, shared blocks by copying with the "shared
    pages" write lock).
  - The kernel test crate (the plan's one change outside the runtime's crates, its own commits): bdf240e1
    moves the driver onto `drive` (the stand-in implements `Execution`, `TestHost` implements `Outside`
    holding the driver's per-turn work); 2c219b7f adds the `runtime` feature and `LOOK_AHEAD`; later commits
    add `LOOK_AHEAD_SEED` and the back end's report on the standard error.
- J1's step 1 checks, all held:
  - seeded runs on the stand-in, at the merge commit b4de3ffa and on the moved driver (scratch
    j1/standin-check/before.txt and after.txt): hello seeds 1 and 5, turns seed 1, the shell's four jobs seeds
    7, 8 and 9 give the same output, exit status, kernel warnings, counters and turn counts (3, 3, 58, 591, 596,
    582). The only difference is the count of root files of the hello and turns runs (664 against 524): the
    program's directory is the container's /wc, and the merge commit's copy held 142 built test programs
    against 2 in this checkout; the shell runs, whose root is the image, agree.
  - the whole kernel test crate without the feature (`cargo test -p wcontainers-kernel-test --no-fail-fast --
    --test-threads 3`): at b4de3ffa 189 passed, 1 failed, 5 ignored (load 63.6 at the end;
    j1/baseline/verdicts-full.log); at 3c2c89a5, whose code without the feature is bdf240e1's, 190 passed, 0
    failed, 5 ignored (load 94.6 at the start, 25.9 at the end; j1/after/verdicts-full.log). The sorted verdict
    lines differ in one line: `queued_signals_match_reference`, FAILED at the merge commit because the
    reference's own count differed ("sigqueue stopped after 3" where it gives 5, other tracks' containers
    sharing Docker's pending-signal limit), passes on the moved driver, and passes again at the merge commit
    when rerun alone (j1/baseline/userlimits-rerun.log, load 28.5). So no verdict changed with the driver.
- J1 was complete: every part of plan section 16's J1 is built, its bars pass (hello at full, none and
  bounded; hello and busybox at full, and also at bounded and none), step 1's checks hold, and what it records
  is below. The scratch copies' build output (j1/baseline/target, j1/after/target, a zig cache) is deleted;
  their sources and logs stay.

- Run 2's bars, at a572aa76 with only this file uncommitted (scratch j1run2/bars.sh; this Mac, other tracks
  running): hello passes at full, none and bounded (N 60) (load 77.1 to 76.6); hello and busybox pass at full
  in 43.3 s (load 76.6 to 90.4), bounded in 44.8 s (90.4 to 68.8) and none in 59.0 s (68.8 to 54.1); busybox 4
  passed, 1 ignored (root_file_system, the known failure) each time.
- Run 2's verdicts of the whole kernel test crate without the feature at a572aa76 (460 s, load 54.1 to 52.8;
  j1run2/verdicts.log and verdicts.txt): 190 passed, 0 failed, 5 ignored, and the sorted verdict lines are
  identical to run 1's after the move (j1/after/verdicts-after.txt). Against the merge commit the one
  difference is still `queued_signals_match_reference` (the reference's own flake, above).
- Leaving `run`, counted from run 2 (busybox at full, one test at a time, 177 runs: the root in memory and on
  the backend; 64.4 s, load 48.3 to 45.7; j1run2/busybox-leaves.log): "the call waits" in 70 runs (1, 3, 887 a
  run: min, median, max; 887 in `cmp /bin/busybox /bin/sh` on the backend, its file calls parking), "another
  thread runs" in 6 (1 each), "the container finished" in every run (1), "new registers" and "the outside
  asked" in none. Runs of `run` ("progress counted by stop checks"): 3, 6 and 893.

## Findings of J1's check, and how each was settled (run 2)

- Clock reads held a counted turn open (blocking): the clock hook reset the turn's count, so a guest reading
  the counter in a loop never reached its limit and never got back to the kernel. The back end now keeps the
  turn's count and the part already told by clock reads apart (`Data::report` for a read, `Data::end_turn` at
  an exit), and a write to a locked page no longer restarts the turn's count. Tested by
  crates/wcontainers-wasmtime/tests/turns.rs (a spin on `mrs cntvct_el0` in a counted turn of 1000, at full
  and none; both failed with "the turn never ended" with the old hook put back, and pass now). The check's own
  program (alarm, then a spin on the counter) passes against the reference with `--features runtime` at full,
  58 s with build (load 67.4 at the start, 88.7 at the end; scratch j1run2/spin-full.log; the program and test
  were copied in for the run and removed).
- Leaving `run` was not counted (blocking): the loop now says why a thread leaves (`Carry::Leave(Leaving)`),
  and the back end counts and warns each leave from a hook, with the call number as the detail (`u64::MAX`
  after a stop): "left run: the call waits", "another thread runs", "new registers", "the container
  finished", "the outside asked". A leave after a fault or trap is not counted again, since that thread is
  out of the guest already (plan sections 5 and 10).
- The guest's own hooks went through `Val` slices and a `Vec` per call (minor): now `Func::new_unchecked`
  with a closure made per index at compile time (`cpu_hook_func::<G, I>`), as the translator's runner does.
  Not timed; no J1 bar is float-heavy.
- J1's text still listed the stop-check counting cost (minor): moved to J3 in the plan (section 16 J1 and
  J3, section 19) and under Departures.
- Plan disagreeing with what was built (minor): the header, J1's `set_permissions` bullet (the "shared
  pages" lock), and section 5, which now names layer 1 (itself the CPU, calling the kernel directly) and
  tests/standin.rs (the stand-in alone) as the tests below the loop.
- RELR split (minor): section 7 records the check's count for busybox and musl (the same starts at three load
  addresses, relocated or not: 558 and 1537); git and rg are counted when they first run.
- Loose evidence (minor): hello's pass at full in run 1 is j1/hello-full-2.log (hello-full-1.log is the
  first, failing attempt); j1/hello-full-1/2.log, busybox-full-record.log and busybox-bounded-none-1.log ran
  on a tree with uncommitted changes, so the commit they measured is not named. Run 2's bar logs
  (j1run2/*.log) each start with the commit and `git status`.

## Findings of J0's last check, and how each was settled

- Shared block calls in J1 (blocking): moved into J1 and built (plan sections 8 and 16). Building them showed
  this Mac's pages are 16 KiB against the guest's 4 KiB, so blocks are kept by the copying method with a write
  lock, not by host mappings (plan section 8, and Departures below). Section 8's sentence about guest memory
  holding everything the kernel loaded is corrected: file mappings, the ELF images among them, show blocks and
  the fork copy skips them.
- The user surface (blocking): plan section 14 now takes `oci_spec::runtime::Spec`, the image-spec
  conversion, every field honoured or refused by name with the list of refused fields and what each would
  take, how we run (`Tuning`, suggested) and what machine it sees (`Machine`, suggested, with one `Offer`
  field per architecture) apart; names as approved; the command line program as J3's last, thin part;
  section 20 asks only what is still open (the two names, the program's name, the OCI image format).
- Progress with two meanings in J1 (minor, ruled interim): recorded under Departures; `Turn` and progress
  kept; no test compares progress between back ends or look-aheads; the loop's count of the kernel's
  changes of pc moves to J2 with the stand-in's own count, since both change the stand-in's unit.
- RELR measured on 5 of 9 binaries: plan sections 7 and 18 now give java's and node's `RELA` (from the
  check's log), bun's neither, go not read; the design reads both.
- Load of the system call figures: section 18 now names measure-translator.log, load 39.78.
- Exec and the count: section 9 says exec is not counted and the count does not survive it (a new thread's
  block starts at zero); a new thread's first `FromRegisters` is not counted.
- How a generic outside rebuilds the back end after a save: `Execution::restored()` (plan section 5); the
  test outside's `stopped` calls it. Built for the stand-in; for `CpuSide` it panics naming J2, where it is
  built with the fork copy (no J1 test saves).
- The cache key leaving out the abi and guest interface crates: section 7's build identity now covers the
  translator, `wcontainers-guest-cpu`, the guest crate, `wcontainers-abi` and wasm-encoder's locked version;
  built with the disk level (J2), since the in-memory level lives inside one build.
- Data words read two ways: an image's `RELR` words are now read one way for the whole image (relocated
  when more of them, less the load offset, fall in the image's span); `RELA` starts come from addends. Built
  in crates/wcontainers-wasmtime/src/starts.rs.
- Line offsets: RNDR at decode_forms.rs:253 and the clock hook at guest.rs:434-441, in both places.

## How to run

The one build directory of this track is the checkout's own `target/` (CARGO_TARGET_DIR unset); the tests'
translation cache is in it (target/wc-kernel-test/translations; delete it to make every translation again;
J3 run 1 deleted its 15 GB of old identities' files, and target/debug/incremental). A run that must not see
edits made while it runs uses a scratch `git worktree` (J3: j3/tree, its own target, deleted at the end of the
run). Scratch is pyro-parked/wcontainers-scratch/runtime; J1's logs are in its `j1/` and `j1run2/`, J2's in
`j2/` to `j2run5/`, J3's in `j3/`.
- J4a's bar at one look-ahead: `LOOK_AHEAD=<full|none|bounded> nix develop -c cargo test -j 4 -p
  wcontainers-kernel-test --features runtime --no-fail-fast --test threads --test turns --test timers --test
  deadlines --test cputime --test cpulimit --test clocks --test lockwaits --test rseqabort --test priorities
  --test schedattr --test shell --test repeats -- --test-threads 3` (the whole of shell, which holds
  `runs_repeat_for_a_seed`); logged, scratch `j4/bar.sh NAME LOOK [test arguments]`, all three
  `j4/bars.sh PREFIX [--nocapture]`. The threaded program through the surface:
  `LOOK_AHEAD=<l> nix develop -c cargo test -j 4 -p wcontainers-wasmtime --test surface -- --test-threads 3
  a_threaded_program` (zig from the Nix shell builds it). A seed repeated N times: `REPEAT_RUNS=<n>
  LOOK_AHEAD=<l> nix develop -c cargo test -j 4 -p wcontainers-kernel-test --features runtime --test repeats
  -- --test-threads 3 --nocapture` (`SCHEDULE_SEED`, `SCHEDULE_SEEDS` choose the seeds; leave out the feature
  for the stand-in); logged, scratch `j4/repeats.sh NAME LOOK|standin RUNS`.
- The surface tests at a look-ahead: scratch `j4/surface.sh NAME full|none|bounded`.
- Reusing a dropped thread's instances: `nix develop -c cargo test -j 4 -p wcontainers-wasmtime --lib --
  --test-threads 3 a_dropped`. Resident size against thread count through the command line program: scratch
  `j4run2/many.sh LOOK COUNT...` (first copy `target/debug/wcon` into j4run2/cli as `wcontainers`, the name
  many.sh calls, and the check's check-j4a/img/with-progs.tar into j4run2; both copies were deleted at the
  end of the run). The check's CPU clock programs: `target/debug/wcon --tar check-j4a/img/with-progs4.tar
  --look-ahead <l> -- /wc/sleepcpu` (or `/wc/cputhreads`). Repeats at bounded ROUNDS times, the first with the translation cache emptied: scratch
  `j4run2/flake.sh PREFIX ROUNDS`.
- J3's surface, refusals, back end and runtime tests (Docker with `alpine:3.22` for linux/arm64):
  `nix develop -c cargo test -j 4 -p wcontainers-runtime -p wcontainers-wasmtime -- --test-threads 3`
  (`--nocapture` prints the two cache runs' times and each refusal's reason). Logged: scratch
  `j3/run.sh NAME full test ...` (commit, status, layer 1 knobs and load in the header).
- The command line program: `cargo build -p wcontainers-wasmtime --bin wcon`, then
  `target/debug/wcon --oci <layout directory> [--cache <directory>] -- sh -c '...'` or `--tar <file>`.
- What counting at every stop check costs: scratch `WC_COST_LAYOUT=<layout> j3/cost/cost.sh NAME`
  (`WC_COST_PAIRS`, default 11).
- J2's bar, layer 1 and the stand-in's verdicts on a scratch worktree: scratch `j3/merged.sh` (after
  `git worktree add --detach j3/tree <commit>`).
- J2's bar since run 4 (with sharedviews): scratch `j2run4/bar.sh NAME full|none|bounded [test arguments]`
  (log j2run4/NAME.log); run 4's bars, layer 1 and the stand-in's verdicts in one command:
  `j2run4/all.sh`. Before run 4: scratch `j2/bar.sh NAME full|none|bounded [test arguments]` (log j2/NAME.log); by hand:
  `LOOK_AHEAD=<l> nix develop -c cargo test -j 4 -p wcontainers-kernel-test --features runtime --no-fail-fast
  --test shell --test script ... --test unknown -- --test-threads 3` (the 21 tests of plan section 16 J2).
- Layer 1 over the back end, one command: scratch `j2/layer1.sh CASES SEED [BACKEND_CASES]`, or
  `LAYER1_CASES=<n> LAYER1_SEED=<s> nix develop -c cargo test -j 4 -p wcontainers-kernel-test --features
  runtime --test layer1 -- --test-threads 3`. The orchestrator's long run: `j2/layer1.sh 1000000 <seed>`.
- The back end on Linux: scratch `j2/linux/check.sh <checkout> <log name> [cargo test arguments]`; it
  writes the commit, the load and the docker command in the log's header:
  `docker run --rm --platform linux/arm64 --cpus 4 -v <checkout>:/src:ro -v <here>/target:/target -v
  <here>/registry:/usr/local/cargo/registry -e CARGO_TARGET_DIR=/target -e
  RUSTUP_TOOLCHAIN=1.99.0-aarch64-unknown-linux-gnu -w /src rust:1.99-slim sh -c 'uname -a; swapon --show;
  cargo test --locked -j 4 -p wcontainers-wasmtime -- --test-threads 3 --nocapture'`. Delete
  j2/linux/target and j2/linux/registry after the run, and the image (`docker rmi rust:1.99-slim`).
- That the build identity follows the hashed packages' files and not their tests: scratch
  `j2run5/identity.sh` (it appends a line to the forms table and removes it with `git checkout`).
- procsignals, the bar's test that saves, N times at a look-ahead, each with a 15 minute limit: scratch
  `j2run2/procsignals-repeat.sh N none`.
- Do not edit the crates while a bar runs: a later cargo invocation of the same run rebuilds from what is
  on disk (that is how j2/bounded-1-stale-build.log hung). Since J2 run 3 the back end's build script reruns
  only for the lock file, a manifest or the translator's and guests' sources, so a new test file alone no
  longer rebuilds the back end.
- The bar, first step: `nix develop -c cargo test -j 4 -p wcontainers-kernel-test --features runtime --test
  hello -- --test-threads 3`, again with `LOOK_AHEAD=none` and with `LOOK_AHEAD=bounded` in front.
- The bar: `nix develop -c cargo test -j 4 -p wcontainers-kernel-test --features runtime --test hello --test
  busybox -- --test-threads 3` (full; the same with `LOOK_AHEAD=bounded` and `LOOK_AHEAD=none` also pass).
- The back end's own tests (turns, code notices, faults, the disk cache, fork and bringing back, the page
  copy): `nix develop -c cargo test -j 4 -p wcontainers-runtime -p wcontainers-wasmtime -- --test-threads 3`.
- Run 2's bars and verdicts, one command, logs beside it: scratch j1run2/bars.sh.
- `LOOK_AHEAD` is `full` (also when unset), `none` or `bounded`; at `bounded` the container's N is drawn from
  1 to the translator's `BOUND` (256; 1 to 64 before J4a run 3) from the run's seed and `LOOK_AHEAD_SEED` (0 if unset) and printed ("look-ahead bounded N"). hello
  and busybox run with seed 0, so `LOOK_AHEAD_SEED` is the knob that changes N for them.
- If `zig cc` hangs (all of this machine's zig processes waited on the shared cache's lock for over 30
  minutes on 2026-11-17), give the run a cache of its own: `ZIG_GLOBAL_CACHE_DIR=<scratch>/zig-cache`.
- With the feature, each run prints the back end's report on the standard error ("cpu side: ..."): its
  translations, how many were made at run time, translating and compiling time, cache hits, the run's time,
  and per range its starts by source. `--nocapture` shows it.
- The stand-in unchanged: `nix develop -c cargo test -j 4 -p wcontainers-kernel-test --no-fail-fast --
  --test-threads 3`, its `test ... ok/FAILED/ignored` lines sorted and compared with the merge commit's
  (scratch j1/baseline/verdicts-before.txt). The seeded runs' check is the scratch test
  j1/standin-check/zz_standin_check.rs, copied into the crate's tests/ for the run and removed after; it prints
  hello (seeds 1 and 5), turns (seed 1) and the shell's four jobs (seeds 7, 8, 9): output, warnings,
  counters, turn counts.
- J0's measurements (scratch, not in the repository) are in pyro-parked/wcontainers-scratch/runtime:
  - `base/` is the trial merge's tree (main 36d20013 with 84e9a937) plus three scratch files:
    `base/crates/wcontainers-translator-test/programs/syscall_loop.S`,
    `base/crates/wcontainers-translator-test/tests/runtime_measure.rs` (tests a to g) and
    `base/crates/wcontainers-kernel-test/tests/runtime_measure.rs` (the kernel's own share). Its target
    directory is deleted; a rerun rebuilds it.
  - `measure.sh`, `measure2.sh` and `measure3.sh` run them; logs `measure-*.log`, `forksize/sizes.txt`,
    `retired-gzip.log`; `forkcopy/forkcopy.c` and its log; `check-j0-run2/relr.py` and `relr.log`;
    `ceiling/reserve.c` and `ceiling/reference-maps.log`. measure2.out's first host memory run exited 101;
    its logs were overwritten by later runs started by hand (plain-3, plain-4, pooling-1, plain-5, pooling-2,
    plain-6, then measure-hostmemory.log); `measure-none.log` is measure3.sh's.

## J1 measurements (2026-11-17, this Mac: Apple M5, 18 cores; wasmtime 50.0.1; the tests' build, opt-level 3 for dependencies and 1 for the workspace)

Load is `sysctl vm.loadavg` at the start and end of each run; other tracks were running. Logs in scratch j1/.
- hello at full, none and bounded (N 60 from seed 0): passes; one translation of its one page of code from 2
  starts (entry 1, unwind 1, ELF data words 3 before merging), 1 piece at full and bounded, 18 at none; made in
  17 to 22 ms translating and 1.2 to 2.3 ms compiling (hello-full-2.log, hello-none.log, hello-bounded.log;
  load 13.2 to 17.1).
- busybox, full, one command at a time (busybox-full-record.log and its summary; load 31.9 at the start, 33.2
  at the end; 178 runs, two per command, the root in memory then on the backend):
  - first use of each range, translated and compiled while the first command ran: musl's code (0xa2000 bytes)
    0.169 s translating and 0.787 s compiling to 2219 pieces from 1538 starts; busybox's (0xd2000 bytes) 0.324 s
    and 1.764 s to 4368 pieces from 559 starts. M5's runner made 1824 and 3520 pieces from the files.
  - starts by source before merging (`Sources`): musl entry 1, init and fini 2, dynamic symbols 1611, unwind
    table 22, relocations 45; busybox entry 1, init and fini 2, symbols 7, unwind 4, relocations 607 (RELR read
    one way, as relocated), init arrays 2.
  - a second container in the same process (cache hit for every range): run time median 5.6 ms (min 4.6, max
    247.9, 112 runs that made nothing), from the kernel's start to its finish.
  - pieces translated at run time per run: median 5, max 51 (0 for the runs where every address already had
    one).
  - the CPU side's warnings, by reason, in how many of the 178 runs and their counts per run (min, median,
    max): progress counted by stop checks 178 (1, 6, 893: runs of `run`, warned once each); loop not proven
    177 (506, 559, 1647); jump through a register not recognised 177 (13, 39, 1,838,650, the most in `cmp
    /bin/busybox /bin/sh` on the backend, 1075 ms); continued after stack loss 175 (1, 1, 1777); translated at
    run time 177 (2, 5, 51); memory fault 177 (2, 5, 10); left to an address 2 (1). First write to a shared
    page, code changed after it ran and code outside an announced range: none. Per command: the summary file
    (j1/busybox-full-record-summary.txt), made by j1/scripts/summarise.py from the log.
- busybox at bounded (N 60) and at none, three tests at a time (busybox-bounded-none-1.log): pass, 37.0 s
  (bounded; load 19.7) and 55.8 s (none; load 24.9 to 32.6). At none the first two runs, side by side, each
  made the whole of musl and busybox: 1.05 and 1.07 s translating, 18.7 and 19.5 s compiling.
- hello and busybox at bounded with `LOOK_AHEAD_SEED=7` (N 13) and `31` (N 27): pass, 57.7 s and 48.1 s
  (load 75.2 to 64.7; bounded-seed-7.log, bounded-seed-31.log).
- Not measured, and why: what counting every stop check costs against full speed needs a turn that counts
  nothing, which comes with J3's free turns; making a per-process 4 GiB state memory alone (it is inside the
  4.6 ms of the fastest cache-hit run).

## J0 measurements (2026-11-17, this Mac: Apple M5, 18 cores; wasmtime 50.0.1; release)

Load is `sysctl vm.loadavg` (1, 5, 15 minutes) at the time; other tracks were running. Plan section 18 has the
same figures with the log of each.
- Reference resident and anonymous memory (`smaps_rollup`, Docker ARM64 Linux; load 29): busybox sh 892 /
  48 kB; python 3.13 8592 / 3364 kB; node 22 48204 / 7268 kB; java 21 44656 / 25064 kB.
- New 4 GiB wasmtime memory plus a copy into it (median of 15; load 39.8): 48 KB 6.4 + 4.0 µs; 3364 KB 6.3 +
  167.8 µs; 7268 KB 9.0 + 387.6 µs; 25064 KB 11.6 + 1587.7 µs; 48204 KB 15.6 + 3229.6 µs.
- Fork copy on macOS (median of 15; load 27.7): `mach_vm_copy` against `memcpy` of the written bytes: busybox
  sh layout (11,200 KB mapped, 1,784 KB written) 19.1 against 115.6 µs; python-sized 197.9 against 610.6 µs;
  java-sized 1292.4 against 3354.6 µs; node-sized 483.0 against 3521.2 µs; exact in all 60 rounds. The parent
  rewriting its written pages after the copy: 266.3 µs, 1575.6 µs, 8916.5 µs, 9576.1 µs, more than `memcpy` of
  the written set in every case. 2000 copies in a row: mean of the first ten 20.3 µs, of the last ten 18.0 µs,
  exact.
- System call in the translator's runner, full look-ahead (load 39.8): in the hook 10.5 ns (median of 7
  alternating pairs of 1,000,000 calls; min 9.4); leaving `run` and back 300.8 ns (median of 5 of 100,000; min
  285.7). Again at load 26.5 (measure-none.log): leaving 286.3 ns (min 283.5).
- Kernel's own share (layer 1 buffer CPU; load 64.8): `getppid` 116.3 to 120.0 ns (5 x 1,000,000); fork, exit
  and wait 2.42 to 2.65 µs (5 x 20,000).
- Real binaries, full, speed, default allocator, 8 runs (load 41.5 to 60.6; instantiation median of 3 and
  bringing back median of 4 in each run): busybox translate 0.15 to 0.35 s, compile 1.12 to 2.08 s,
  instantiate 0.43 to 0.60 ms, from a serialised file 2.77 to 3.59 ms; musl 0.06 to 0.16 s, 0.51 to 0.99 s,
  0.27 to 0.42 ms, 1.19 to 1.67 ms; libpython (1 run, load 57.1) 0.46 s, 6.60 s, 1.19 ms, 12.56 ms.
- Look-ahead none, whole binary from M5's starts (load 27.1 to 26.2): busybox 135,972 pieces, compile 8.45 s,
  peak 3,120 MB; musl 85,954, 5.09 s; libpython 703,235, 113.92 s, peak 16,918 MB. Reading M5's starts from
  the file: 0.07, 0.08 and 1.37 ms.
- Relocations: busybox, musl, git and rg keep all relative relocations in `RELR` (1074, 80, 4253, 14,146
  places); python in `RELA` (12,063).
- Pooling allocator, 2 runs interleaved with 4 default runs (load 54.2 to 63.7): busybox 0.60 and 0.55 ms
  against 0.44 to 0.60 ms; musl 0.36 and 0.35 ms against 0.33 to 0.42 ms: no difference.
- BLAKE3 of code (median of 21; load 40.3): busybox 0.300 ms, musl 0.159 ms, libpython 1.275 ms.
- Host-supplied memory (macOS; load 59.2): one 1 MiB shared memory object mapped into two wasmtime guest
  memories; 1000 of 1000 stores seen across; mapping 0.6 µs (median of 50).
- Instructions retired against time, `gzip -c` of one 20 MB file, 8 runs (load 88.45, read once after them): 2,716,771,743 to
  2,721,493,991 instructions (0.17% apart); 0.25 to 0.29 s.

- Address room (macOS 27.2; load 38.34): one process can reserve 2^47 bytes in all (largest single
  reservation 130,578 GiB). The reference (Docker ARM64, `alpine:3.22`; load 41.28) puts its stack at
  0xffff_d4a3_b000: 48 bit addresses.

Expected only (not measured): a system call answered at once about 130 ns end to end (10.5 + 116 to 120); a
shell's fork about 0.75 ms, nearly all of it instantiating busybox's and musl's modules; rebuilding a child's
array of slots in microseconds.

## Departures from the phase-one plans and the brief

- The container's address-space size is the guest's Linux size (2^48 for ARM64), not the kernel plan's 4 GiB,
  as the brief's late section directs; the memory size told is a separate value given in `Start` (proposed
  4 GiB). Each back end has its own address ceiling, computed from the output's widths (0xFF00_0000 for ARM64
  today). Until the kernel holds the size and places low first, it lays processes out inside
  `Cpu::address_space` as today, so the stand-in (4 GiB) and the WebAssembly back end (0xFF00_0000) lay out
  and tell sizes differently in J1. Listed for the owner in plan section 20.
- Today's translator contract keeps the permission table and the array of slots in a 32 bit state memory,
  so its output does not yet give a process the largest address space the host offers (translator plan
  section 6). The plan asks the translator track for a wider form; nothing in the runtime changes but its
  layout code.
- Fork copies exactly with the host's own copy, which on macOS copies each page on first write (the kernel
  plan's expectation for wasmtime) and on Linux copies the present and swapped pages at once.
- Starts are M5's list read from the image in memory at first use, not from the file: no `.symtab`, and the
  address first reached is not a start (plan section 7).
- The kernel test driver's own loop is replaced by the runtime's `step` and `drive`; its per-turn work moves
  into the test host's `Outside`. One loop instead of two (plan section 5).
- The brief's layer 1 bar ("with the translator behind the boundary") is read as the WebAssembly back end's
  memories, tables and copies in place of the buffer CPU, since layer 1 runs no instructions (plan section 1).
- Until the translator emits the guest-defined progress count, the WebAssembly back end counts stop checks in
  its place (named, counted and warned). J4 is split by the orchestrator's ruling of 2026-11-18: J4a (threads,
  turns, timers, repeating a seed at one look-ahead) is built on stop checks; left out of J4's bar, as J4b, is
  only what needs a guest-defined count: the same count at every exit for a seed across look-ahead settings
  and on the stand-in (plan sections 9 and 16).
- The root gets Docker's three files (`/etc/hostname`, `/etc/hosts`, `/etc/resolv.conf`) at start, though no
  OCI field asks for them: the reference is Docker, which writes them over its mounts, and the kernel says a
  host writes them from its made-up network. `/.dockerenv` is not written (it marks the Docker engine). The
  files are dated 0 so that a run repeats.
- Look-ahead none is not used for the K6b runtimes (J6): libpython at none took 113.9 s to compile and 16.9 GB.
- The benchmark margin is checked on instructions retired, with times reported beside it (plan section 15;
  listed for the owner).
- wasmtime's engine uses Unix signals for traps on macOS, not its default Mach ports, so a process copy (the
  tests' saves) can still run WebAssembly (plan section 11).
- Blocks of shared pages are kept by the copying method on every host, not by host mappings as plan section 8
  first said: this Mac's pages are 16 KiB (`sysctl hw.pagesize`) and the guest's 4 KiB, and the kernel maps
  and unmaps blocks a 4 KiB page at a time. Each memory holds a copy; pages that show a block carry the
  "shared pages" write lock; the guest's first write faults to the back end (counted "first write to a
  shared page"), and written pages are copied into the block and every other memory before the kernel sees
  anything. Exact while one guest thread runs at a time, with the view lock on the memory's other places of
  a page being written and a write next to another view of its page made one instruction at a time (plan
  section 8; J2 run 4).
- Progress has two meanings in J1, accepted by the orchestrator until the owner rules on the count: the
  stand-in counts its instructions and the WebAssembly back end counts stop checks passed (named, counted and
  warned once per run: "progress counted by stop checks"). The translator is not changed for it. No test
  compares progress between the two back ends or between look-aheads; that waits for the translator's
  guest-defined count. The loop's count of the kernel's changes of pc moves to J2 with the stand-in's own
  count, since either alone changes the stand-in's results.
- At bounded look-ahead N is drawn once per container from the run's seed (and `LOOK_AHEAD_SEED`), not per
  range from the range's start as plan section 15 first said: the look-ahead is a setting of the container,
  and drawing it per range would put a test's behaviour in the shipped back end (plan section 15 corrected).
- The private words of a thread are zeroed at every `FromRegisters`, the contract's state of a new thread, so
  the back end never names ARM64's words; the plan first said the monitor is cleared at every exit and float
  tracking set to "tracked" (plan sections 4 and 9 corrected).
- A dropped thread's block was reused by the process's next thread from J1, but its instances only from J4a
  run 2 (1ebe8e4d), after J4a's first check found a process's store growing with every thread it had started.
- `Outside` has six methods, not five: `cpu_warning` carries the back end's slow paths apart from the
  kernel's `Host::warn` (plan section 5 corrected). `Execution` has `restored()` (finding of J0's check 4).
- J1's step 1 was checked by the seeded runs before the commit was made, and by the whole-suite diff after it
  (the whole suite took 8 to 25 minutes at this load); the commit was pushed with first light, before the
  whole-suite diff ended, as the J1 task asked for a push as soon as hello passed.
- Leaving `run` is counted for five reasons, not the plan's first three: the loop's two other leaves (the
  container finished, the outside asked) cost the same and are named the same way (plan section 10).
- The change that names why a thread leaves touched the stand-in's `run` (one pattern, `Carry::Leave(_)`) in
  the same commit as the runtime crate (61c63df6), since either alone does not build; it is the one kernel
  test crate change of run 2.
- What counting every stop check costs against full speed was listed for J1 but is measured in J3: it needs
  a turn that counts nothing, which J3's free turns bring.
- The RELR split is counted for busybox and musl only (by J1's check); git and rg when they first run.
- `zig cc` runs for the checks used a zig cache in scratch (`ZIG_GLOBAL_CACHE_DIR`), because every zig process
  on the machine waited on the shared cache's lock for over 30 minutes; it changes nothing built.

- Not built in J2, by the brief: the stand-in's count of the guest's own transfers, and the loop's count of
  the kernel's changes of pc that plan section 16 builds with it; both wait for the owner's ruling on the
  progress unit.
- The fork copy copies pages that show a block of shared pages with the rest, and copies the permission
  table and the slot entries, where plan section 8 first said to skip the first and rebuild the others: after
  the written pages' copies are made all of them hold exactly what the child needs, and the host's copy
  shares pages instead of writing each (plan section 8 corrected).
- The check that a copy made at a save compiles new code is the back end's own test (a fork copy of its test
  process, rayon's global pool started first), not a kernel test: the kernel tests' copies find their
  translations in the cache's directory and need not compile (plan section 16 J2 says so).
- Layer 1 over the back end changed layer1.rs (its CPU type) as well as the driver; both are the kernel test
  crate's planned change (plan section 14), made as its own commit (0b6fefdb).
- J2's bar runs three tests at a time; the per-test warnings come from a separate run at full with
  `--nocapture`, summed per test file because lines of tests running together interleave.
- align.rs (a kernel test crate test file, not the driver) changed in J2 run 2 so that its ignores depend on
  the feature (e98abb5e): the plan's rule that a test the stand-in cannot run must pass with the runtime
  needs the stack pointer test to run with it.
- The kernel's `getcpu` difference that layer 1 finds with seed 918273645 is left to the kernel track: this
  track does not change the kernel.
- The check's counter program was added to the kernel test crate (guest/counterclock.c,
  tests/counterclock.rs, 2dcdad96), not only the driver: it is the test of the clock hook's known failure,
  ignored with the feature until the kernel's clock method lands, and it runs on the stand-in.
- After a save is brought back on the WebAssembly back end the counter is the host's clock, not the kernel's,
  so it can go back (a known failure, counted and warned); the plan's design needs the kernel's clock method,
  which the kernel track has not made public.
- J2 run 4 added a guest program and test file to the kernel test crate (guest/sharedviews.c,
  tests/sharedviews.rs, their own commits), not only the driver, and added `sharedviews` to J2's bar (plan
  section 16 J2): no listed test showed one block at two places of one memory. Its "code" test is ignored
  without the feature as the stand-in's known failure (Unicorn runs stale code), the plan's rule for a
  test the stand-in cannot run; with the runtime it passes against the reference.
- sharedviews' child does not run the code it rewrote in shared memory: on the reference it ran the old
  code after `__builtin___clear_cache`, which the architecture does not allow and neither back end does
  (recorded in J2 run 4's findings). It also runs stale code in one process, every time, when code written
  through a writable view is run through the executable view next to it just after the sync (J2 run 5's
  findings): for such code the tests compare with what the architecture requires, not with the reference.
- A write next to another view of its block's page is single-stepped through the exact slot and the
  lookup, which the translator's contract gives for one instruction run exactly; the plan had no such
  case (section 8 now has it).
- With the feature, a kernel test run that saves holds the test process alone while it has a back end
  (44ff7829): the save forks a test process whose other threads run other tests, and a lock one of them
  holds at the fork (libunwind's, wasmtime's) never comes free in the copy. Plan section 11 said the copy
  holds only the forking thread; it did not say the other threads' locks are lost with them.

- J3 merged origin/phase-1 (the kernel through K6b) at the start of its run, as the task allowed; the driver's
  conflicts were settled keeping both sides' tests working (J3 run 1).
- Free turns changed `Turn` into an enum; the kernel test crate's driver and stand-in change one line each in
  the same commit as the runtime (5d86b31d), since neither builds alone.
- `Outside` has a seventh method, `stop_flag`, with a default that does nothing, for an outside that counts
  every turn (plan section 5 says six).
- `Machine` holds only the CPU feature claim: the address-space size and memory size the plan put in it wait
  for the kernel to take them at start; a field that did nothing would pretend.
- The back end the runtime makes is a trait of the runtime crate (`BackEnd`), with the back end's own
  `Tuning`; the plan's sketch named `Tuning` and `Machine` as runtime types with the translator's look-ahead,
  which the runtime crate cannot name without depending on the translator.
- The cost of counting at every stop check is measured on busybox's `echo` and a busybox shell loop, not the
  static hello: the surface runs the alpine image, which has no hello.
- `a_second_process_starts_from_the_cache` asserts every range's translation comes from disk in the second
  process, not every translation: pieces translated at run time where a free turn stopped mid-way differ
  between runs (the timer's moments are real time), so some are made again; both counts are printed.
- A configuration with no `hostname` gives the container `localhost` (runc keeps the host's, which the
  container never sees); listed for the owner in plan section 20.
- The mounts are honoured only when they are exactly the kernel's six with Docker's options, and the
  namespaces only when they are exactly the kernel's six kinds: the kernel makes them whatever the
  configuration says, so anything else would not be honoured.

- J4a added a test file to the kernel test crate, tests/repeats.rs (5492ba0a, b001897c, 87319389, 6d26eb6f),
  not only the driver: the bar's seeded repeats need the driver's `Run` compared whole, which no kernel test
  did. The driver gained `Injections::wall_start` for it (87319389).
- The rename of `Outside` to `System` changed the kernel test driver in the same commit as the runtime crate
  (5bec4f91), since neither builds alone.
- The tests draw bounded look-ahead from 1 to the translator's `BOUND` (256), not 1 to 64 as plan section 15
  first said: the range is then derived from the translator, not a second constant (J4a run 3; plan
  corrected). The N a seed gives changed with it.

## Dependencies

Added in J4a run 3 (licence read from the registry's manifest on 2026-11-18): serde_ignored 0.1.14 (MIT OR
Apache-2.0) to wcontainers-runtime, for the properties of a config.json the specification does not define;
its dependencies serde and serde_core were in the lock.

Added in J1, licences read from each crate's manifest in the registry on 2026-11-17:
- wcontainers-runtime: none beyond the workspace's own crates.
- wcontainers-wasmtime: wasmtime 50.0.1 (Apache-2.0 WITH LLVM-exception); wasm-encoder 0.263.0 (Apache-2.0
  WITH LLVM-exception OR Apache-2.0 OR MIT) for the hook types the guest interface names; rustix 1.1.6
  (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT) for mappings and the host page size; blake3 1.8.7
  (CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception) for the cache key; object 0.41.0 (Apache-2.0 OR
  MIT) for ELF layouts; gimli 0.34.0 (MIT OR Apache-2.0) for the unwind table index; rayon 1.12.0 (MIT OR
  Apache-2.0) for the back end's pool; zerocopy (the workspace's).
- wcontainers-kernel-test: wcontainers-runtime, and, behind the `runtime` feature, wcontainers-wasmtime and
  wcontainers-arm64. The crate stays GPL-2.0-only and unshipped; nothing shipped depends on it.
- Added in J2 (licences read from the registry's manifests on 2026-11-18): mach2 0.6.0 (BSD-2-Clause OR MIT
  OR Apache-2.0), on macOS only, for `mach_vm_copy`; blake3 1.8.7 also as a build dependency of
  wcontainers-wasmtime (the build identity); libc 0.2.191 (MIT OR Apache-2.0) as a dev-dependency only, for
  the test that forks its process.
- Added in J2 run 2 (licence read from the registry's manifest on 2026-11-18): toml_edit 0.25.19 (MIT OR
  Apache-2.0), a build dependency of wcontainers-wasmtime only, to find the workspace's guest crates.
  Replaced in J2 run 4 by two build dependencies only (never in a shipped binary), licences read from the
  registry's manifests on 2026-11-18: cargo_metadata 0.23.1 (MIT; already in the lock for the kernel test
  crate; with camino, cargo-platform, semver, serde_json, thiserror, each MIT OR Apache-2.0) and cargo-lock
  11.1.0 (Apache-2.0 OR MIT), which brings 31 crates new to the lock: toml, url, idna, percent-encoding,
  form_urlencoded, utf8_iter and displaydoc (MIT OR Apache-2.0), synstructure (MIT), and the ICU crates (icu_*, zerovec, zerotrie, yoke, tinystr,
  litemap, writeable, potential_utf, zerofrom; Unicode-3.0).
- Added in J3 (licences read from cargo metadata on 2026-11-18), to wcontainers-runtime: oci-spec 0.10.0
  (Apache-2.0; default features off, `runtime` and `image`), which brings const_format and
  const_format_proc_macros 0.2.37 (Zlib), darling, darling_core and darling_macro 0.20.11 (MIT),
  derive_builder, derive_builder_core and derive_builder_macro 0.20.2 (MIT OR Apache-2.0), getset 0.1.7
  (MIT), ident_case 1.0.1 (MIT/Apache-2.0), konst 0.2.20 and konst_macro_rules 0.2.19 (Zlib), strsim 0.11.1
  (MIT), strum and strum_macros 0.27.2 (MIT), unicode-xid 0.2.6 (MIT OR Apache-2.0); and, already in the
  lock: tar 0.4.46 (MIT OR Apache-2.0), flate2 1.1.11 (MIT OR Apache-2.0), sha2 0.10.9 (MIT OR Apache-2.0),
  getrandom 0.3.4 (MIT OR Apache-2.0), serde_json 1.0.154 (MIT OR Apache-2.0). To wcontainers-wasmtime:
  pico-args 0.5.0 (MIT) and wcontainers-arm64 (the workspace's) for the command line program; tar and
  serde_json as dev-dependencies.
