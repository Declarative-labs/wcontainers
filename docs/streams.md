# Streams: standard input, output, error and the terminal

The plan of the streams track, rewritten on 2026-11-25 for the owner's committed interface. The track's brief is
orchestrator/restart-2026-11-25/brief-streams.md; where this plan and the brief disagree, the brief wins. The
credit design this page held before (`stream_credit`, `Event::StreamData`, `Event::StreamCredit`,
`Event::StreamClosed`, abi `End`) is replaced by the interface below.

## The interface

`StreamTransport` (crates/system/interface.rs:189-204) is part of the embedder's `System`:

- `type Endpoint: Ord`: the embedder's name for one side of a byte stream whose other side it holds.
- `send_stream(peer, payload)`: the kernel hands bytes to the embedder, which takes a prefix and says how many.
- `receive_stream(peer, into)`: the embedder writes a prefix of what it has into `into` and says how many.
- `close_stream(peer)`: the kernel lets go of the endpoint; it calls nothing on it after.
- `Readiness<Endpoint>` (crates/readiness/interface.rs): `wait_until_ready` ends when the endpoint is ready for
  what was asked (`READABLE`, `WRITABLE`), or with an unconditional event (`ERROR`, `HANG_UP`: the other side has
  closed, and what it sent can still be read).

The runtime names the endpoints of the first process's descriptors 0 to 2 in `ContainerConfig`
(`standard_input`, `standard_output`, `standard_error`, crates/runtime/interface.rs:53-55).

## What the kernel relies on, and the check that holds it

The kernel's side is crates/kernel/implementation/linux/ends.rs (the kernel track's). It relies on exactly this;
each line is a function of crates/system/conformance/suite.rs, run by `stream_tests!` on a transport and the two
endpoints of one stream it gives (what is sent at `sending` arrives at `receiving`).

| The kernel relies on | Suite function |
|---|---|
| A send takes a prefix, says how many, and the bytes arrive in order, from a slice or from bytes that are not one | `send_and_receive_through_buffers`, `send_and_receive_bytes_that_are_not_a_slice` |
| A receive gives a prefix of what is held, never more than asked, and keeps the rest | `receive_part_and_keep_the_rest` |
| With nothing to give and the other side held, a receive says `WouldBlock`, never `Ok(0)` (the kernel reads `Ok(0)` as the end) | `a_receive_with_nothing_sent_would_block` |
| Once the other side closed, the endpoint reports `HANG_UP`, still gives what was sent, then ends with `Ok(0)` or an error other than `WouldBlock` | `closing_the_sending_endpoint_ends_the_stream_after_what_was_sent`, `closing_the_sending_endpoint_with_nothing_sent_ends_the_stream` |
| A send that took less than it was given is followed, once there is room, by a `WRITABLE` report that wakes the wait | `a_send_without_room_is_ready_after_a_receive` |
| Waits on an endpoint behave as the readiness suite requires (not before ready, woken once, dropped waits withdrawn, failures end every wait) | the readiness suite's six checks, through `receiving_made_readable_by_a_send` and `receiving_hung_up_by_closing_the_sending_endpoint` |

What the interface leaves open stays open (the owner's rule: silence means unspecified): whether a send takes
everything at once, whether a receive fills its buffer, `Ok(0)` or `Err(Closed)` at the end, what a send does after
the receiving side was closed, and what calls on an endpoint after `close_stream` do.

The suite checks itself: its test module runs every check on a correct loop with no limit and on one holding a page,
and fifteen loops each with one flaw, every one of which a named check catches by its message.

## Where each piece lives

| Piece | Where | Track |
|---|---|---|
| The interface | crates/system/interface.rs, crates/readiness/interface.rs | the owner |
| The stream suite | crates/system/conformance/suite.rs | streams |
| The kernel's side: pipes and a pseudo-terminal whose other side is an endpoint | crates/kernel/implementation/linux/ends.rs, syscall/pipe, syscall/tty | kernel |
| The Docker comparisons of descriptors 0 to 2 and the terminal | unicorn harness tests/streams.rs, tests/terminal.rs | kernel |
| The runtime's endpoints (`LocalSystem`) and seeded-test-policy | crates/runtime/implementation/single-kernel, crates/system/implementation/seeded-test-policy | runtime |
| The native executors' `LinkHost` | crates/executor/implementation/hardware-virtualization-inside | native |

Each implementation runs `stream_tests!` once it builds on main (the brief's ST4).

## Linux behavior to match, and the Docker command that shows it

The kernel track keeps these; they are unchanged by the interface. On ARM64 Linux Docker, `alpine:3.22`:
- Without `-i`: 0 is `/dev/null` (fdinfo flags 0400002), 1 and 2 are pipes (flags 01):
  `docker run --rm alpine:3.22 sh -c 'ls -l /proc/self/fd; cat /proc/self/fdinfo/0'`.
- With `-i`: 0 is a pipe (flags 00, mode `fifo 600`): `echo hi | docker run --rm -i alpine:3.22
  sh -c 'ls -l /proc/self/fd; cat /proc/self/fdinfo/0'`.
- With `-t`: 0 to 2 are one open file of `/dev/pts/0` (fdinfo flags 02000002), devpts mounted with
  `gid=5,mode=620,ptmxmode=666`, the first process its session's leader with it as controlling terminal
  (`/proc/self/stat`: tty_nr 34816, tpgid 1), stty settings `icrnl ixon opost onlcr isig icanon iexten echo echoe
  echok echoctl echoke`: `docker run --rm -t alpine:3.22 sh -c 'ls -l /proc/self/fd; stty -a; cat /proc/self/stat'`.
- A closed reader: `docker run --rm alpine:3.22 sh -c 'trap "" PIPE; (sleep 1; echo x; echo $? >&2) | true'` prints 1.

## Open

- A terminal's size change has no path through the committed interfaces (the brief's question 1); the size at
  start comes from the spec's `process.consoleSize`.
- `type Endpoint: Ord` is used by no caller, and the kernel needs `Send + Sync + 'static` (the brief's question 2).
- The linter's `conformance_not_run` asks each suite function to take one value; a `StreamTransport` alone gives
  no endpoint, so the stream checks take the transport and two endpoints (the brief's question 3).
- Not built, not closed off: endpoints made by connect, accept, export or import.
