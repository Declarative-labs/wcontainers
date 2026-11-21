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
  sockets) and `peer_for` (an IPv4 peer of an IPv6 socket, mapped). `receive_message` now also gives the raw
  control bytes (`control_bytes`), for `datagram::receive::Ancillary::parse`; `take_error_queue_message`,
  `ErrorQueueMessage` and `transmit_event` are gone (TCP reads its error queue through `Ancillary` now).
- New, 2026-11-21 evening: `icmp_error::write_icmp_error`, the ICMP error of an error queue entry written whole
  from its IP header, generalised from the UDP track's private `icmp_error`: it takes `quoted`, the part of the
  quoted transport header Linux took off (the 8-byte UDP header for UDP, nothing for TCP, whose payload starts
  at the TCP header), and builds the ICMP header and checksum with etherparse. UDP track: please switch to it
  and drop your copy (`icmp_error` and `rest_of_header` in crates/udp-sockets/implementation/linux-host).
- UDP track: `datagram/receive.rs` does not build for `aarch64-unknown-linux-musl` (the Mac's Linux lint
  target): `msg_controllen` and `cmsg_len` are `u32` there, `usize` on gnu. This track now lints on the ARM
  machine (gnu).
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
- Suite `crates/tcp-sockets/conformance`: 48 checks against the traits alone, each on a value made anew,
  including the Configurable and NegativeRouteAdvice suites run on a connection and a listener.
- Run 2 (2026-11-21 evening) finished the error queue and the send requests T1 had left:
  - ICMP error entries: written whole from their IP header by the shared `write_icmp_error` (outer IP header
    from the offender to the connection's local address, ICMP header with type, code and the MTU or pointer,
    the quoted IP header, then the quoted TCP bytes Linux kept). The connection keeps its two addresses for
    this, since Linux clears them when the error ends the connection. Checked over IPv4 and IPv6
    (`icmp_error_reaches_a_synchronizing_connection_over_ipv4/_ipv6`): a connection to a listener of backlog 0
    whose one place is taken stays in SYN-SENT; the check captures its SYN with a raw TCP socket and answers it
    with a host (address) unreachable error from a raw ICMP socket, then checks every header and the checksum
    of the entry. These two checks need root (raw sockets).
  - Transmit reports with the packet (`OPT_TSONLY` turned off from the first send that asks; the length is
    what Linux gave, counted when cut to the buffer), with statistics (`OPT_STATS`, its `TCP_NLA_*` attributes
    parsed with netlink-packet-core into `TcpTransmitStatistics`), and wireless status (`SO_WIFI_STATUS`,
    entries with `SCM_WIFI_STATUS`). Linux gives a TCP socket's reports either the packet or statistics:
    a send asking for the one after the other was armed is refused (NotSupported, counted).
  - A report's interface is now what Linux reports (with `IP_PKTINFO` on), no longer always index 0.
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
  acknowledgement_report_*, empty_error_queue_would_block, the two icmp_error_* checks,
  transmit_report_carries_the_packet_*, transmit_statistics_come_*, wireless_status_request_still_sends), discard_error_queue_entries
  (discarding_empties_the_error_queue), push_queued_data (more_to_come_*, push_ignoring_*), configure
  (cork_holds_*, urgent_inline_*, unread_byte_reports_*, receive_software_timestamps_*, the two
  set-twice checks, the Configurable suite), give_negative_route_advice (its two checks), set_timestamp_clock
  and the listener's (timestamp_clock_needs_repair_mode, which only checks the refusal outside repair mode).
  Weak until T2 can read state: send_delayed_acknowledgement, rearm_keep_alive_timer and send_window_probe
  are checked only to succeed and leave the connection working.

- Run 2, measured on the ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-21 18:46 to 18:50, load 1.3
  to 2.9: 48 passed, 0 failed, eight times in a row; the run itself 0.06 s. Clippy with warnings denied clean
  on Linux (gnu) and the Mac. Eight broken copies (`mutants.sh`, 17 s) each failed: the four above as before,
  ICMP entries refused fails both icmp_error checks, quoted addresses swapped fails the IPv4 one, report packet
  dropped fails transmit_report_carries_the_packet_when_asked, statistics dropped fails
  transmit_statistics_come_with_the_report_when_asked.
- mutants.sh shared one build folder between copies and so once ran a copy with the previous copy's broken
  shared crate; it now touches every file any copy breaks, in each copy.

## How to run
- Suite on the ARM machine (needs sudo; runs in its own network namespace with only loopback):
  `cloud/arm-run <checkout> --name host-tcp-sockets -- crates/tcp-sockets/implementation/linux-host/run-conformance.sh`
- Broken copies: the same with `crates/tcp-sockets/implementation/linux-host/mutants.sh` (about 15 s).
- Lint for Linux on the ARM machine: `cloud/arm-run <checkout> --name host-tcp-sockets -- 'cargo clippy -j
  $ARM_JOBS --no-deps --tests -p wcontainers-tcp-sockets-linux-host -p wcontainers-tcp-sockets-conformance -p
  wcontainers-internet-protocol-linux-socket-options -- -D warnings'`, and the same on the Mac without arm-run
  (with `CARGO_TARGET_DIR=$PWD/target/host-tcp-sockets nix develop -c`; there the implementation builds empty).
  The Mac's musl target no longer builds the shared crate (see the UDP note above).
- Files copied from the Mac can carry times older than the ARM machine's last build, so cargo there may skip
  them: `touch` the changed files in the command when a result looks stale.
- The literal check reports nothing in this track's files.

## Next (T2)
- read_state and read_listener_state (every query), authentication (MD5, AO, counters), repair options and
  windows, `RecordsLastReceivedHeader`, `SaveSynchronize(FromLinkHeader)` and what a guest's own saving
  means for `TakeSavedSynchronize`: counted refusals now.
- Then strengthen the weak checks above with state readings, and run the privileged settings as root.

## Departures from the plan
- The plan said `SO_KEEPALIVE` rearms the keep-alive timer; Linux re-times it only on `TCP_KEEPIDLE` (and on
  keep-alive turned on from off), so the idle time is written again. Plan corrected.
- The binding and socket opening helpers moved into the shared crate for the UDP track.
- Run 1 moved the ICMP part of the error queue and three kinds of transmit report to T2; run 2 did them in T1.
- The plan's router namespace for ICMP errors became raw sockets on loopback: the check captures the SYN and
  sends the error itself, which needs no namespace setup beyond the suite's own.
- wireless_status_request_still_sends only checks that asking works and that loopback brings no status: no
  wireless link exists on the test machine.
- Cargo.lock is not committed by this track: it holds other tracks' uncommitted lines, and cargo writes this
  track's entries back by itself (no new external crate was added).

## Dependencies added (all already in Cargo.lock)
- rustix 1.1.6 (also in the suite, for its raw sockets): Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT.
  netlink-packet-core 0.9.0 (the statistics' attributes): MIT. linux-raw-sys 0.12.1: the same. libc
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
- `TransmitReport::interface` is required, but Linux names an interface in a report only when the socket has
  `IP_PKTINFO` on: otherwise the implementation gives index 0, Linux's own "none".
- `TcpTransmitStatistics::received_hop_limit` is required, but Linux gives `TCP_NLA_TTL` only in a report an
  acknowledgement brought (example: a software send report has none): the implementation gives 0 then.
- A wireless status report names no send on Linux (`ee_data` is 0): the implementation gives key 0, so the
  request's `key` cannot be honoured for it.
- Linux gives a TCP socket's transmit reports the packet or statistics, never both, and the choice is the
  socket's: example, a send asking for statistics after one asked for the packet is answered NotSupported.
- `TransmitReportKey` cannot be passed to Linux for TCP (`SCM_TS_OPT_ID` is refused on TCP sockets): the
  implementation keeps each send's key by the byte offset the kernel reports instead.
- `RecordsLastReceivedHeader(Report)` does not say which header is recorded for a TCP socket: IPv4's
  `IP_PKTOPTIONS` gives control messages (packet information, TTL, TOS), not the header bytes that
  `TcpReading::LastReceivedHeader` carries.
