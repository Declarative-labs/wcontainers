# Status: host-tcp-sockets

Plan: docs/plan-host-tcp-sockets.md. Milestones there: T1 (connections and listeners, the suite), T2 (state
readings, repair, authentication, multipath state, the privileged settings, ICMP error entries).

## For the UDP, ICMP and raw tracks
- The shared crate is `crates/internet-protocol/implementation/linux-socket-options` (package
  `wcontainers-internet-protocol-linux-socket-options`), made by this track. Its body is
  `cfg(target_os = "linux")`. It holds: `host_error`, `last_host_error`; the raw option calls `set_option`,
  `set_int`, `set_flag`, `set_bytes`, `get_option`, `get_int`, `get_bytes`; `Shortfalls` (counted refusals,
  warned once, readable as `CounterReading`s); `set_filter`; `receive_timestamping_flags`, `set_timestamping`;
  `set_ip_socket_setting`, `set_port_setting`, `set_packet_setting`; `ControlBuffer`, `receive_message`,
  `timestamps`, `take_error_queue_message`, `transmit_event` and the `TIMESTAMP_STAGE_*` and
  `TIMESTAMPING_TIMES` constants; and (since 2026-11-21 evening) `family_of`, `socket_address`, `open_socket`
  (non-blocking, owner by the thread's filesystem user, EPERM when the host may not act as that user),
  `allowed_ranges`, `set_port_range`, `in_port_ranges`, `bind` (with `choose_now` for listeners and datagram
  sockets) and `peer_for` (an IPv4 peer of an IPv6 socket, mapped).
- UDP track: your `datagram/receive.rs` defines its own `TIMESTAMPING_TIMES`; the shared one is now `pub`, so
  import it (the literal check reports both as a redefinition until one goes). Your `pub mod datagram;` line in
  `implementation.rs` was left out of this track's commits, since its files were not committed yet: commit it
  with them.
- Still refused there with a counted reason: multicast and anycast memberships, a fixed IPv6 flow label. The
  UDP track needs them more: add them there.

## Done (T1, 2026-11-21)
- `crates/tcp-sockets/implementation/linux-host` (package `wcontainers-tcp-sockets-linux-host`, type
  `LinuxHostTcpSockets`, connection `LinuxHostTcpConnection`, listener `LinuxHostTcpListener`, all fields
  private; public beyond the traits: `new()` and `counters()`). Real non-blocking Linux TCP and MPTCP
  sockets. connect (settings first, bind as the binding asks, exit-node port choice outside reserved
  ranges through `IP_LOCAL_PORT_RANGE` per allowed range, fast open with data by `MSG_FASTOPEN`, until first
  send by `TCP_FASTOPEN_CONNECT`), listen (TCP listeners always save SYN network headers for accept), accept
  (SYN header from `TCP_SAVED_SYN`, cut to the buffer), send (urgent, more to come, record end, transmit
  report requests with guest keys mapped from the kernel's byte offsets), receive (read, peek, peek at an
  offset by `SO_PEEK_OFF`, urgent byte; receive timestamps and unread byte counts from control messages),
  shut_down, close (abort by a linger of zero), stop_listening, take_error_queue_entry and
  discard_error_queue_entries for transmit timestamps, every plain setting of settings.rs, and the action
  methods push_queued_data, send_delayed_acknowledgement, rearm_keep_alive_timer, send_window_probe,
  set_timestamp_clock (both), give_negative_route_advice.
- Suite `crates/tcp-sockets/conformance`: 43 checks against the traits alone, each on a value made anew,
  including the Configurable and NegativeRouteAdvice suites run on a connection and a listener.
- Measured on the ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-21 18:27, load 1.4: 43 passed, 0
  failed; the run itself 0.06 s. A clean build of the test binary took 9.9 s wall (load 1.0 to 2.6).
- Independent check run by this track on the same machine (job 20261121-182557, load 1.0 to 2.8): clean
  rebuild and suite passed; four broken copies (`mutants.sh`) each failed it: MSG_MORE ignored fails
  more_to_come_holds_data_until_pushed; abort closing in order fails abort_resets_the_peer; peek offset
  ignored fails peek_at_an_offset_skips_bytes; push doing nothing fails more_to_come_holds_data_until_pushed
  and push_ignoring_neither_keeps_the_cork. `cargo tree` of every interface crate this track uses shows no
  implementation; `git diff` of every interface.rs and data file since 4308b9f7 is empty.
- Again from a clean clone of main at 23c46b24 (job host-tcp-sockets-verify/20261121-183028, load 2.0 to
  4.1): 43 passed, the four broken copies each failed as above. Main's committed Cargo.lock did not parse
  there (a `[[package]]` without its name, from commit 8eaca3b6 of another track), so the clone used the
  checkout's working lock: tell whoever owns that commit; this track did not edit the shared lock.
- Which check fails when a method breaks: connect (every check; connect_settings_apply_to_the_synchronize for
  its settings, exit_node_chooses_a_port_outside_the_reserved for bindings, fast_open_* for fast open), listen
  and accept (every check; connect_and_accept_name_both_ends and accept_cuts_* for their fields), send and
  receive (bytes_arrive_in_order_both_ways and most others; urgent_*, peek_*, more_to_come_*,
  unread_byte_reports_*, receive_software_timestamps_* for the requests), shut_down (the three shutdown
  checks), close (orderly_close_*, abort_resets_the_peer), stop_listening
  (stopped_listener_refuses_connections), take_error_queue_entry (software_transmit_report_*,
  acknowledgement_report_*, empty_error_queue_would_block), discard_error_queue_entries
  (discarding_empties_the_error_queue), push_queued_data (more_to_come_*, push_ignoring_*), configure
  (cork_holds_*, urgent_inline_*, unread_byte_reports_*, receive_software_timestamps_*, the two
  set-twice checks, the Configurable suite), give_negative_route_advice (its two checks), set_timestamp_clock
  and the listener's (timestamp_clock_needs_repair_mode, which only checks the refusal outside repair mode).
  Weak until T2 can read state: send_delayed_acknowledgement, rearm_keep_alive_timer and send_window_probe
  are checked only to succeed and leave the connection working.

## How to run
- Suite on the ARM machine (needs sudo; runs in its own network namespace with only loopback):
  `cloud/arm-run <checkout> --name host-tcp-sockets -- crates/tcp-sockets/implementation/linux-host/run-conformance.sh`
- Broken copies: the same with `crates/tcp-sockets/implementation/linux-host/mutants.sh` (about 15 s).
- Lint from the Mac for Linux: `CARGO_TARGET_DIR=$PWD/target/host-tcp-sockets nix develop -c cargo clippy
  --no-deps --tests -p wcontainers-tcp-sockets-linux-host -p wcontainers-tcp-sockets-conformance -p
  wcontainers-internet-protocol-linux-socket-options --target aarch64-unknown-linux-musl -- -D warnings`
  (and once without `--target`: on the Mac the implementation builds empty).
- The literal check reports nothing in this track's files.

## Next (T2, then what T1 left)
- ICMP error entries of the error queue (now a counted refusal): ICMP errors reach a TCP socket's queue only in
  SYN-SENT and SYN-RECEIVED; check it with a router namespace and an unreachable route; the entry is to be
  written whole from its IP header.
- Transmit reports with the packet, wireless status or statistics (`OPT_TSONLY` off, `SO_WIFI_STATUS`,
  `OPT_STATS` parsed into `TcpTransmitStatistics`): counted refusals now.
- read_state and read_listener_state (every query), authentication (MD5, AO, counters), repair options and
  windows, `RecordsLastReceivedHeader`, `SaveSynchronize(FromLinkHeader)` and what a guest's own saving
  means for `TakeSavedSynchronize`: counted refusals now.
- Then strengthen the weak checks above with state readings, and run the privileged settings as root.

## Departures from the plan
- The plan said `SO_KEEPALIVE` rearms the keep-alive timer; Linux re-times it only on `TCP_KEEPIDLE` (and on
  keep-alive turned on from off), so the idle time is written again. Plan corrected.
- The binding and socket opening helpers moved into the shared crate for the UDP track.
- The ICMP part of the error queue and three kinds of transmit report moved to T2 (see Next).
- Cargo.lock is not committed by this track: it holds other tracks' uncommitted lines, and cargo writes this
  track's entries back by itself (no new external crate was added).

## Dependencies added (all already in Cargo.lock)
- rustix 1.1.6: Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT. linux-raw-sys 0.12.1: the same. libc
  0.2.191: MIT OR Apache-2.0. etherparse 0.21.0: MIT OR Apache-2.0.

## For the owner
- No way to build one: the crate has `LinuxHostTcpSockets::new()`. Suggested trait beside `TcpSockets`:
  `pub trait StandaloneTcpSockets: TcpSockets { fn new() -> Self; }`.
- `owner: UserId` is undefined when the exit node runs as one user: a guest's root (`UserId(0)`) connecting
  through a host process running as uid 1000 can only be answered EPERM (the host cannot make a socket owned
  by another user without CAP_SETUID), so on an unprivileged host every guest connect fails. The suite
  therefore runs as root.
- `TcpAccepted::synchronize` asks for fields Linux never reports for a SYN: for IPv4 the arrival interface
  (example: a SYN to 127.0.0.1 gives no interface anywhere; the implementation takes the interface holding
  the local address and counts it), and the SYN's own mark, priority and timestamps (it gives the accepted
  socket's mark and priority and no timestamps).
- `TransmitReport::interface` is required, but Linux names no interface in a report without the packet:
  the implementation gives index 0, Linux's own "none".
- `TransmitReportKey` cannot be passed to Linux for TCP (`SCM_TS_OPT_ID` is refused on TCP sockets): the
  implementation keeps each send's key by the byte offset the kernel reports instead.
- `RecordsLastReceivedHeader(Report)` does not say which header is recorded for a TCP socket: IPv4's
  `IP_PKTOPTIONS` gives control messages (packet information, TTL, TOS), not the header bytes that
  `TcpReading::LastReceivedHeader` carries.
