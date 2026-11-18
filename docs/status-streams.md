# Status of the streams track

The plan is docs/streams.md; the owner's rulings are in the brief (orchestrator/brief-streams.md).

## Where things stand

S1 (stream ends for standard input, output, error and the terminal) is complete after its first
run: built, tested against Docker, the whole suite unchanged test by test, pushed. See "S1, first
run" for the bar item by item.

What comes next: the independent check of S1 (its task lists it). Not built and left open on
purpose: ends made by connect, accept, export or import; `/dev/console` under `-t` (see the
departures).

What exists:
- abi: `End`, `Host::stream_send`, `stream_credit`, `stream_close`; `Event::StreamData`,
  `StreamCredit`, `StreamClosed`, `TerminalResized { end, .. }`. `Host::output`,
  `wcontainers_abi::Stream` and `Event::TerminalInput` are gone; nothing else carries these bytes.
- kernel: `Start::standard` (`wcontainers_kernel::Standard`: `Pipes { input, output, error }` or
  `Terminal { end, columns, rows }`); ends.rs holds each end's kernel side as an ordinary open file
  (pipe reader, pipe writer, pty master) with its send credit; pipes and ptys keep the room they
  promised (`Pipe::promised`, `Pty::promised`); the ends are served at the end of every kernel
  entry (`Kernel::serve_ends` from `settle`); a pipe write sends straight out when the pipe is empty
  and credit allows. The pipe's old "the host reads it" field and its special cases (splice, readers
  count) are gone.
- test host (crates/wcontainers-kernel-test/src/streams.rs): the other sides of the ends, a script
  (wait for output, send, resize, close input, close output) that the Docker reference
  (`reference::run_outside`) follows too, seeded send pieces, slow or absent output credit, a
  rule-breaking mode (`Outside::excess`), and a record of anything the kernel did against the
  credit rules (`Run::broken`).
- tests: crates/wcontainers-kernel-test/tests/streams.rs (17 tests), guest programs stdin.c,
  stdout.c, terminal.c.

## How to run

- The stream tests: `RUST_TEST_THREADS=3 nix develop -c cargo test -j 4 -p wcontainers-kernel-test
  --test streams`. Docker must be running. Our side runs under `SCHEDULE_SEEDS` seeds (3 if unset)
  from `SCHEDULE_SEED` (1 if unset); the reference once.
- Everything: `RUST_TEST_THREADS=3 nix develop -c cargo test --no-fail-fast -j 4 -p wcontainers-abi
  -p wcontainers-kernel -p wcontainers-kernel-test`.

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
  nonblocking (the nonblocking writer met EAGAIN, here and in the reference); output after the
  outside closed its end, SIGPIPE default (killed, 141) and ignored (EPIPE), against a closed pipe
  in the reference: all match. No run had anything in `Run::broken` (the kernel never sent past
  credit, after a close, or credit on an output end).
- Terminal: `-t` without input (isatty, ttyname, size 0 0, settings, pgrp, sid, tcgetpgrp, rdev,
  mode, gid, /proc/self/stat's tty fields, fdinfo flags 02000002); `-it` resize to 100x30 with
  SIGWINCH; canonical reads with echo, erase and kill, ^D at a line's start reads 0, then a raw
  read of 3; ^C and ^Z reaching the program and its child (same group), echoed as ^C and ^Z; a
  busybox shell: `sleep 30` stopped by ^C, then `echo $((6*7))`, then `exit`: all match.
- Rule breaking (no reference): an outside sending 100 bytes past its credit with each piece is
  counted (`stream data past its credit`, warned) and only credited bytes come in; an outside that
  never gives output credit holds a nonblocking writer to the pipe's 65536 bytes, EAGAIN after,
  and nothing is sent.
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

## Departures from the plan

- With `-t`, Docker also bind-mounts the slave at `/dev/console`. Not built: `/dev/console` is
  absent, as without `-t` (ENOENT), so nothing pretends.
- `F_SETPIPE_SZ` on a pipe the outside writes refuses (EBUSY) a size below what it holds plus what
  was promised; Linux refuses only below what it holds, since the shim's bytes not yet written are
  in the shim. A guest writer to that pipe may use only room not promised.
- Bytes the outside sends after the kernel closed an input end are dropped without a count (they
  were within the credit; Linux's shim gets EPIPE for them). Bytes past the credit are counted.
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
  outside's bytes may join; a pty's room toward the slave) less what is outstanding, granted at
  start and again when it has grown by a page, or when nothing is outstanding and any room is free.
- `Next::Finished` can come while output is held for want of credit; the embedder grants credit
  until each output end closes.
- `docker run -t` without `-i` is a terminal end the outside never sends on.
- A start that fails closes every end it named at once (stream_close for each).
- The runtime (on main) asked for `Event::Input` and a flow-control event; this interface covers
  both with `StreamData` and `StreamCredit`.
