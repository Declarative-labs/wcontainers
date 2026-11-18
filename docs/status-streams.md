# Status of the streams track

The plan is docs/streams.md; the owner's rulings are in the brief (orchestrator/brief-streams.md).

## Where things stand

S1 (stream ends for standard input, output, error and the terminal), first run: built and tested.
See "S1, first run" for the bar item by item.

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
- tests: crates/wcontainers-kernel-test/tests/streams.rs (16 tests), guest programs stdin.c,
  stdout.c, terminal.c.

## How to run

- The stream tests: `RUST_TEST_THREADS=3 nix develop -c cargo test -j 4 -p wcontainers-kernel-test
  --test streams`. Docker must be running. Our side runs under `SCHEDULE_SEEDS` seeds (3 if unset)
  from `SCHEDULE_SEED` (1 if unset); the reference once.
- Everything: `RUST_TEST_THREADS=3 nix develop -c cargo test --no-fail-fast -j 4 -p wcontainers-abi
  -p wcontainers-kernel -p wcontainers-kernel-test`.

## S1, first run

The bar, item by item (all on this Mac's Docker, linux/arm64, alpine:3.22):
- (filled in below as each part is run)

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
