# Streams: standard input, output and error

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

## No terminal on the interface (the owner's ruling of 2026-11-22)

A terminal's size is not on any interface, and no terminal is in the kernel by default
(orchestrator/open-with-owner.md:980-982, interface-changes.md:231-232). A container that wants a terminal runs a
small program of its own that holds the pseudo-terminal; the window size, and any change of it, travels inside the
stream to that program, as ssh and telnet carry it. So `StreamTransport` carries bytes only, nothing calls the
kernel for a resize, and the suite checks no terminal. Who builds that program inside the container is not
assigned yet (written in the track's status for the orchestrator).

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
| The kernel's side: descriptors 0 to 2 as pipe ends whose other side is an endpoint | crates/kernel/implementation/linux/ends.rs, syscall/pipe | kernel |
| The Docker comparisons of descriptors 0 to 2 | unicorn harness tests/streams.rs | kernel |
| The program inside a container that holds a pseudo-terminal and reads the window size from its stream | none yet | not assigned |
| The runtime's endpoints (`LocalSystem`) and seeded-test-policy | crates/runtime/implementation/single-kernel, crates/system/implementation/seeded-test-policy | runtime |
| The native executors' `LinkHost` | crates/executor/implementation/hardware-virtualization-inside | native |

The kernel's terminal on main (the `Standard::Terminal` start, `terminal_resized` driven by the legacy
`Event::TerminalResized` at crates/abi/host.rs:51, the harness's `Step::Resize` and tests/terminal.rs) predates the
ruling; with the legacy events it is the kernel track's to remove or leave off by default (its KP2).

Each implementation runs `stream_tests!` once it builds (the brief's ST4). The suite needs two endpoints of one
stream, what is sent at the first arriving at the second; an implementation whose other side is the outside gives
that pair in its test only: seeded-test-policy and `LinkHost` forward to a loop of services on the other side, and
the unicorn `TestHost` gets a client that gives back on descriptor 0 what it was sent on descriptor 1, as `cat`
would. The patches are in pyro-parked/wcontainers-scratch/streams-restart/patches/.

## Linux behavior to match, and the Docker command that shows it

The kernel track keeps these; they are unchanged by the interface. On ARM64 Linux Docker, `alpine:3.22`:
- Without `-i`: 0 is `/dev/null` (fdinfo flags 0400002), 1 and 2 are pipes (flags 01):
  `docker run --rm alpine:3.22 sh -c 'ls -l /proc/self/fd; cat /proc/self/fdinfo/0'`.
- With `-i`: 0 is a pipe (flags 00, mode `fifo 600`): `echo hi | docker run --rm -i alpine:3.22
  sh -c 'ls -l /proc/self/fd; cat /proc/self/fdinfo/0'`.
- A closed reader: `docker run --rm alpine:3.22 sh -c 'trap "" PIPE; (sleep 1; echo x; echo $? >&2) | true'` prints 1.

What Docker shows with `-t` (0 to 2 one open file of `/dev/pts/0`, devpts, a controlling terminal, stty settings:
`docker run --rm -t alpine:3.22 sh -c 'ls -l /proc/self/fd; stty -a; cat /proc/self/stat'`) is, under the ruling
above, the behavior of the program inside the container that holds the pseudo-terminal, not the kernel's default.

## Open

- For the owner (brief, question 1): `type Endpoint: Ord` (crates/system/interface.rs:191) is used by no caller,
  while the kernel needs `Send + Sync + 'static`, which only its own trait states.
- For the owner (brief, question 2): the linter's `conformance_not_run` asks each suite function to take one value;
  a `StreamTransport` alone gives no endpoint, so the stream checks take the transport and two endpoints.
- Not built, not closed off: endpoints made by connect, accept, export or import.
