# Status of the streams track

The plan is docs/streams.md; the owner's rulings are in the brief (orchestrator/brief-streams.md).

## Where things stand

S1 (stream ends for standard input, output, error and the terminal): built in the first run;
the second run fixed everything the independent check found (see "S1, second run").

What comes next: the independent check of S1 again. Not built and left open on purpose: ends
made by connect, accept, export or import; `/dev/console` under `-t` (see the departures).

What exists:
- abi: `End`, `Host::stream_send`, `stream_credit`, `stream_close`; `Event::StreamData`,
  `StreamCredit`, `StreamClosed`, `TerminalResized { end, .. }`. `Host::output`,
  `wcontainers_abi::Stream` and `Event::TerminalInput` are gone; nothing else carries these bytes.
- kernel: `Start::standard` (`wcontainers_kernel::Standard`: `Pipes { input, output, error }` or
  `Terminal { end, columns, rows }`); ends.rs holds each end's kernel side as an ordinary open file
  (pipe reader, pipe writer, pty master) with its send credit. A pipe knows the end that reads it
  (`Pipe::outside_reader`, set when the end is attached), the input credit outstanding
  (`Pipe::promised`) and an inbox for the outside's bytes that found no room (at most the credit);
  a pty keeps the room it promised (`Pty::promised`). The ends are served when the kernel settles
  (`Kernel::serve_ends`), only after an end's event or a wake on a channel in `Ends::watched`
  (the channels of the files held, removed when a file is let go). A pipe write sends straight out
  when the pipe is empty and credit allows. The pipe's old "the host reads it" field and its
  special cases (splice, readers count) are gone.
- test host (crates/wcontainers-kernel-test/src/streams.rs): the other sides of the ends, a script
  (wait for output, send, resize, close input, close output) that the Docker reference
  (`reference::run_outside`) follows too, seeded send pieces, slow or absent output credit, a
  rule-breaking mode (`Outside::excess`), and a record of anything the kernel did against the
  credit rules (`Run::broken`).
- tests: crates/wcontainers-kernel-test/tests/streams.rs (19 tests), guest programs stdin.c,
  stdout.c, terminal.c.

## How to run

- The stream tests: `RUST_TEST_THREADS=3 nix develop -c cargo test -j 4 -p wcontainers-kernel-test
  --test streams`. Docker must be running. Our side runs under `SCHEDULE_SEEDS` seeds (3 if unset)
  from `SCHEDULE_SEED` (1 if unset); the reference once.
- Everything: `RUST_TEST_THREADS=3 nix develop -c cargo test --no-fail-fast -j 4 -p wcontainers-abi
  -p wcontainers-kernel -p wcontainers-kernel-test`.

## S1, second run: the independent check's findings

- Rule-breaking outside (blocking): the test host now sends all of its credit plus the excess each
  time and takes exactly the credit off its own count (`Outside::excess`), so every such send is
  past the kernel's credit by exactly the excess, at every seed. The test checks that the count of
  `stream data past its credit` equals the number of sends past credit, and that the program read
  exactly the bytes sent within credit (`Run::sent_within_credit`, `Run::sends_past_credit`).
  Passed at seeds 1 to 3 and 40 to 52.
- `children_inherit_the_standard_descriptors` (blocking): busybox `head -c 10` takes a stdio
  buffer, so what the next reader saw depended on how the seed cut the input. It now uses `dd bs=1
  count=10`, which takes exactly 10 bytes. The rule is written in docs/streams.md: a program compared
  with Docker must not show where its reads end. Passed at seeds 1 to 3 and 40 to 52.
- Output after the outside closed (blocking): the first process is init, which a default SIGPIPE
  never kills, so the old "default" case could not show a kill. The test now runs the writer both as
  the first process and as a shell's child (`sh -c '/wc/stdout closed ... ; echo status $? >&2'`).
  As the child with SIGPIPE default it is killed (status 141); ignored, it gets EPIPE (status 0); as
  the first process both get EPIPE. Each matches the reference. The first run's status wrongly said
  the "default" case was killed.
- Bare literals (blocking): the new `(125..=127)` is gone. reference.rs now checks Docker's own
  failure statuses in one function (`assert_docker_ran`) instead of three copies. The literal stays
  until the abi's `exit` module lands: the literal README gives that module to the runtime track
  (stage 1), and it is on no branch yet. In the files this track touched, these are now named once in
  kernel-test: `/wc` (`reference::GUEST_DIRECTORY`, with `in_guest_directory` and
  `guest_directory_entry`), `/bin` (`reference::IMAGE_BINARIES`, `image_command`), `linux/arm64`
  (`reference::PLATFORM`), the driver's seed salts (`SCHEDULE_SALT`, `SAVES_SALT`), and the save
  shift (`u32::BITS`). See "Literals" below for what remains.
- Writing one's own input pipe (minor): fixed rather than kept as a departure. Credit no longer
  reserves room in the input pipe. A guest writer uses the pipe as on Linux, `F_SETPIPE_SZ` is
  Linux's rule again (EBUSY only below what the pipe holds), and the outside's bytes that find no
  room wait in the pipe's inbox (at most the credit given). They go in as readers make room, and a
  close from outside lets go of the writer only once they are in. New test
  `a_program_writing_its_own_input_pipe` (the check's command, with `cat` after a second write):
  matches Docker.
- Costs on unrelated pipes and wakes (minor): a pipe write now reads one field of the pipe it
  already holds (`outside_reader`) instead of walking the ends. A channel is removed from `watched`
  when its file is let go, so a wake compares against at most the live ends' channels (3 today).
  Making the wake itself free would need a mark that the wake path can reach from a channel; not
  done, and not measured.
- Outside closing before the start made the file (minor): `Kernel::attach_end` now lets go of the
  file through the kernel's release, as a close does. New test
  `a_terminal_closed_before_the_start_finished` (root on the backend, the outside closing at once;
  the test host now runs the script's first steps that wait for nothing at `begin`): `read` sees end
  of file and the shell exits 7. A run with the old drop showed that this test passes even without
  the release, because the slave sees no master by count. The release is still what marks the pty
  closed and wakes its waiters.
- Reference runner (minor): an Expect that times out now removes the container (`docker rm -f`)
  and kills the client before it panics. `run_outside` sets `DOCKER_CLI_HINTS=false`, so the client
  adds no "What's next" text to a terminal after a failed run.
- Stale comment in fs/mod.rs (minor): rewritten.
- `priority_inheriting_futexes_match_reference` (minor, not this track's code): the check saw it
  fail once in the whole suite at a load of 90 to 110, and pass 3 of 3 alone. The reference printed
  two lines in the other order. This is a race in the reference, recorded here, and not changed:
  it belongs to the kernel track.

## S1, first run

The bar, item by item (this Mac's Docker, linux/arm64, alpine:3.22; ours under seeds 1 to 3, the
reference once; tests/streams.rs, 17 tests, 30.8 s for the 16 before the backend one, load 55 to
65):
- `docker run -i`: a pipeline reading all its input (`sort | uniq -c | ...`, 200 kB);
  end of input as end of file (read 0 twice, poll POLLHUP; also with no input at all); 1 MiB to a
  reader of 512-byte reads that sleeps every 32 reads, every byte counted and hashed, and the
  kernel never promised more than the pipe's size (the program's F_GETPIPE_SZ) at once; a reader
  that closes 0 after 100 bytes while 1 MiB still comes (the kernel closes the input end); input
  arriving while blocked in read, poll, select and epoll (sent only when the container waits);
  children inheriting 0 to 2 (`read`, `head`, `cat | wc` in a shell); redirecting them away
  (`exec 3<&0 </dev/null`, `exec 1>&2`); input to a container whose start still waits on the
  backend: all match.
- Output: 8 MiB to an outside that takes 64 KiB only when the container waits, blocking and
  nonblocking (the nonblocking writer met EAGAIN, here and in the reference): matches. Output
  after the outside closed: see the second run. No run had anything in `Run::broken` (the kernel never sent past
  credit, after a close, or credit on an output end).
- Terminal: `-t` without input (isatty, ttyname, size 0 0, settings, pgrp, sid, tcgetpgrp, rdev,
  mode, gid, /proc/self/stat's tty fields, fdinfo flags 02000002); `-it` resize to 100x30 with
  SIGWINCH; canonical reads with echo, erase and kill, ^D at a line's start reads 0, then a raw
  read of 3; ^C and ^Z reaching the program and its child (same group), echoed as ^C and ^Z; a
  busybox shell: `sleep 30` stopped by ^C, then `echo $((6*7))`, then `exit`: all match.
- Rule breaking (no reference): see the second run for the outside that sends past its credit (the
  first run's version went past it only at some seeds). An outside that never gives output credit
  holds a nonblocking writer to the pipe's 65536 bytes, EAGAIN after, and nothing is sent.
- Whole suite at 6ca181b8 (this checkout's build, scratch streams/mine-suite.log,
  `RUST_TEST_THREADS=3 cargo test --no-fail-fast -j 4 -p wcontainers-abi -p wcontainers-kernel -p
  wcontainers-kernel-test`, 52 minutes with the build, load 66.7 at the start and 66.1 at the end):
  232 passed, 0 failed, 6 ignored. Compared test by test with the run before this work
  (streams/verdicts.py base-suite.log mine-suite.log): every one of the 220 earlier tests has the
  same verdict; the new ones are the 17 stream tests and the abi's one unit test (the abi was not in
  the earlier command).
- Layer 1: its default runs passed in that suite (`sequences_match_reference`, 64 sequences, seed
  0; `sequences_on_backend_match_reference` and `sequences_on_cached_backend_match_reference` at
  their default counts).
- clippy with warnings denied and fmt: clean on the abi, the kernel and the test crate
  (`cargo clippy --all-targets -- -D warnings`), and `cargo check --workspace --all-targets` builds.
- No bare literal added: `git diff 2edbc451 -- crates/wcontainers-kernel crates/wcontainers-abi`,
  every added line with a digit read by eye: only 0 (as a value or a count), `.0` field access, and
  named constants (`NULL_PATH`, `PTMX_PATH`, `NO_THREAD`, `pipe::MODE` from Linux's `S_IFIFO |
  S_IRUSR | S_IWUSR`, replacing the bare `0o600` that was there; the `/dev/pts/{index}` format now
  in one function). In the test crate the ends' numbers, `UNLIMITED`, `MOST_AT_ONCE`, `BUSY_TURNS`
  and the reference's waits are named; the rest are test inputs.

Found on the way: the first whole-suite run (stopped; scratch streams/mine-suite-1-stopped.log)
failed every run with the root on the backend: the output credit came before the start had made
the ends and was lost. Fixed by naming the ends when `start` is called (6ca181b8), with a test.

## Measurements

- Whole suite before this work, at 2edbc451 (a clean copy with its own build, scratch
  streams/base-suite.log, `RUST_TEST_THREADS=3 cargo test --no-fail-fast -j 4 -p wcontainers-kernel
  -p wcontainers-kernel-test`): 214 passed, 0 failed, 6 ignored, 61 minutes with the build, load
  42.9 at the start and 53.8 at the end.
- Nothing about speed was measured. Expected only: a write to standard output costs what it did
  (sent straight out while the pipe is empty and credit lasts), and each kernel entry pays one flag
  test for the ends unless a wake touched their pipes or terminals.

## Licences

No dependency added.

## Literals

The track's fix list is orchestrator/magic-literals/fix-kernel.md, which covers the same crates as
the kernel track. This track takes only the lines in the files it touches, so that the merges stay
small. Left as they are, each until its owner's constant is on a branch this one merges:
- `125..=127`, Docker's run-failure statuses: one place in reference.rs (`assert_docker_ran`), and
  `container.rs`'s `start_failed` (125, 126, 127). Owner: the abi's `exit` module (runtime track,
  stage 1 of the literal README).
- driver.rs's `wait_status & 0x7f`. Owner: the kernel's public `wait` module (kernel track, stage 4).
- Other copies of `/wc`, `linux/arm64` and the Docker command words are in kernel-test files this
  track did not change (backend.rs, image.rs, layer1.rs, ltp.rs, runtimes.rs, busybox.rs). They can
  use the names above when the kernel track works its list. `ENVIRONMENT`'s `HOSTNAME=wc-kernel`
  restates `HOSTNAME`; not changed.

The literals check (branch phase-1-literals) was not merged in this run. Its last commits (16:22
to 16:33) changed its rules minutes before this run began. Its status shows no checked milestone
yet. So the baseline is not made here.

## Departures from the plan

- With `-t`, Docker also bind-mounts the slave at `/dev/console`. Not built: `/dev/console` is
  absent, as without `-t` (ENOENT), so nothing pretends.
- A terminal's room toward the slave is still held for the outside's credit, so `TIOCSTI` from
  inside may find less room than on Linux while credit is outstanding. Pipes no longer hold room
  (second run).
- Bytes the outside sends after the kernel closed an input end are dropped without a count (they
  were within the credit; Linux's shim gets EPIPE for them). Bytes past the credit are counted.
  Bytes waiting in a pipe's inbox when its last reader goes are dropped the same way.
- Output after the outside closed its end cannot be shown by Docker (its daemon reads every
  container's output for the log), so the reference runs the same program on a pipe whose reader
  it closed itself (stdout.c `closed self`), and ours with the outside closing (`closed outside`).

## For the orchestrator

Names for the owner (all suggestions but `stream_credit`, which the owner chose):
- `End` (`wcontainers_abi::End(u32)`).
- `Host::stream_send`, `Host::stream_close` (beside the owner's `stream_credit`).
- `Event::StreamData`, `Event::StreamCredit`, `Event::StreamClosed`, and
  `Event::TerminalResized { end, columns, rows }` (the old event, given the end).
- `Start::standard` and `wcontainers_kernel::Standard::{Pipes, Terminal}`.

Decided here, not ruled by the brief:
- The embedder names the ends made at start; the kernel will name the ones it makes later, from
  numbers no live end has.
- A terminal's size changes through its own event (`TerminalResized` with the end), the brief's
  "plus its size".
- Closing: the outside's close frees the end at once (the kernel lets go of its file and says
  nothing more); the kernel's close means it sends nothing more, and the outside then frees the end
  by closing it. An input end is closed by the kernel when no reader is left; an output end when
  no writer is left and all it held is sent; a terminal end when the slave has no opener and all the
  master held is sent. The outside closing a terminal end closes the master (the slave hangs up).
- Credit the kernel gives: the room it has (a pipe's free pages plus the room in a last page the
  outside's bytes may join; a pty's room toward the slave) less what is outstanding (and, for a
  pipe, what waits in its inbox), granted at start and again when it has grown by a page, or when
  nothing is outstanding and any room is free. A pipe's room is not held for the credit: a writer
  inside may take it, and the outside's bytes then wait in the inbox, at most the credit.
- `Next::Finished` can come while output is held for want of credit; the embedder grants credit
  until each output end closes.
- `docker run -t` without `-i` is a terminal end the outside never sends on.
- A start that fails closes every end it named at once (stream_close for each).
- The runtime (on main) asked for `Event::Input` and a flow-control event; this interface covers
  both with `StreamData` and `StreamCredit`.
