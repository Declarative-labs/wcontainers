# Status: host-tcp-sockets

Plan: docs/plan-host-tcp-sockets.md. Milestones there: T1 (connections and listeners, the suite), T2 (state
readings, repair, authentication, multipath state, the privileged settings).

## For the UDP, ICMP and raw tracks
- Run 4 (2026-11-21, after the async ruling): TCP uses the UDP track's shared `readiness` for both waits. Two
  notes on it, for its owner: (1) every socket is registered edge-triggered for every event from the moment it
  is watched, so the watcher thread wakes, looks the socket up and takes its lock on every edge of every socket,
  even with nothing waiting (each segment a busy connection receives); arming a oneshot registration with the
  waiters' interest at a wait that has to wait would cost nothing on sockets nobody waits on. (2) TCP's sockets
  are still made by the shared `open_socket` on rustix: making it socket2's `Socket::new` belongs in the shared
  crate (its owner switch around `socket` is shared), so this track leaves it to whoever moves the shared crate to
  socket2 (a `sock` helper was being added there during run 4).
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
  and drop your copy (`icmp_error` and `rest_of_header` in crates/udp-sockets/implementation/host).
- UDP track: `datagram/receive.rs` does not build for `aarch64-unknown-linux-musl` (the Mac's Linux lint
  target): `msg_controllen` and `cmsg_len` are `u32` there, `usize` on gnu. This track now lints on the ARM
  machine (gnu).
- UDP track: your `datagram/receive.rs` defines its own `TIMESTAMPING_TIMES`; the shared one is now `pub`, so
  import it (the literal check reports both as a redefinition until one goes). Your `pub mod datagram;` line in
  `implementation.rs` was left out of this track's commits, since its files were not committed yet: commit it
  with them.
- Still refused there with a counted reason: multicast and anycast memberships, a fixed IPv6 flow label. The
  UDP track needs them more: add them there.

## Done in run 4 (2026-11-21, after the async ruling and the reorganization)
- Renamed to `host`: folder `crates/tcp-sockets/implementation/host`, package `wcontainers-tcp-sockets-host`, types
  `HostTcpSockets`, `HostTcpConnection`, `HostTcpListener` (the UDP track's lock commit f0463350 took the staged
  folder rename; the contents moved in 4f2a4794).
- `wait_for_events` and `wait_for_listener_events` on the shared `readiness` (polling 3.11.0 over epoll; polling
  rather than mio because it registers a socket of any kind on every system, raw ones on Windows included, which
  the raw and ICMP tracks need too). A connection or listener holds its socket as a `Watched`, whose drop
  deletes the registration before the descriptor closes. `HostTcpSockets::new()` now answers a `Result`: it
  starts the readiness thread.
- Suite: eight wait checks (finishes when data arrives and not before; answers at once when ready; dropping a wait
  cancels it; 16 connections with two waits each, half fed, each fed wait woken exactly once and the others
  never; room to send comes back when the peer reads; a reset ends a wait for an event that never comes, with
  the always-reported error or hang-up; a listener's wait finishes when a connection comes; dropping a listener's
  wait cancels it). Plus, in the implementation's own tests, a counting allocator: 100 waits that each have to
  wait allocate nothing on the waiting thread once the connection has waited once
  (a_wait_allocates_nothing_once_warm). And the stopped agent's check reports_name_their_sends_among_plain_ones.
- socket2 0.6.5 (`SockRef` on the socket's descriptor) for connect, fast-open `sendto`, listen, accept
  (`accept4`, which also gives the peer), plain send and receive, shutdown, the abort's linger, the local address,
  and the settings it covers whole: keep-alive on or off, no-delay, cork, urgent inline, quick acknowledgement,
  segment size, thin linear timeouts, congestion control, not-sent low water; and the reads and writes of
  no-delay, cork, quick acknowledgement and address sharing inside the action methods. Kept on rustix and libc,
  because socket2 lacks them or does not compose: the keep-alive idle, interval and probes (socket2's
  `set_tcp_keepalive` also turns keep-alive on, so setting the idle time alone would change another setting), the
  user timeout (socket2 cuts a time that is not whole milliseconds where the contract's answer is EINVAL), sends
  and receives with control messages (socket2 passes raw control bytes only; the shared crate parses them), and
  every Linux-only option. Creating the socket stays in the shared `open_socket` (see above).

## Done (T1, 2026-11-21)
- `crates/tcp-sockets/implementation/host` (package `wcontainers-tcp-sockets-host`, type
  `HostTcpSockets`, connection `HostTcpConnection`, listener `HostTcpListener`, all fields
  private; public beyond the traits: `new()` and `counters()`). Real non-blocking Linux TCP and MPTCP
  sockets: connect (settings, bindings, exit-node port choice, fast open with data by `MSG_FASTOPEN`, until first
  send by `TCP_FASTOPEN_CONNECT`), listen (backlogs among its settings applied after `listen`), accept (SYN
  header from `TCP_SAVED_SYN`), send and receive with every request, shut_down, close, stop_listening, the error
  queue (transmit reports with packet, statistics or wireless status; ICMP errors written whole), every plain
  setting of settings.rs, and the action methods. read_state, read_listener_state, authentication and the repair
  queues are T2 (counted refusals now).
- Suite `crates/tcp-sockets/conformance`: 72 checks against the traits alone, each on a value made anew. The body
  is `cfg(target_os = "linux")`: the checks watch the wire. A capture thread reads the host's packet socket
  (`AF_PACKET`, every loopback packet seen twice, counted once) filtered to one listener's port, and the checks
  read headers, sequence numbers, windows, TCP timestamps and times from it. Timer checks make the peer drop
  everything with a minimum hop limit of 255. Buffer checks measure what a connection takes, or what a peer
  sends, while nothing is read. The namespace the suite needs (root, `fq` on loopback, `net.ipv4.tcp_fastopen=3`,
  `net.ipv6.flowlabel_consistency=0`, `dctcp`) is written at the top of the suite and made by run-conformance.sh.
- Run 3 (2026-11-21 evening) answered the independent check of run 2:
  - Every plain setting now has a check of its effect, not only of being set twice: see the list below.
  - Fast open is exercised for real: the namespace turns the listener side on, each fast open check has a loopback
    address of its own (a client keeps cookies per address), and the checks read the SYN's payload and whether the
    SYN-ACK acknowledges it. They cover data in the SYN once a cookie is held, a deferred connect sending nothing
    until its first send, doing without cookies, the listener's keys (a stale key's cookie refused, the backup
    key's taken) and its queue turned off.
  - The action methods: push ignoring neither sends data held by more-to-come; a record end makes the next send a
    new segment; the delayed acknowledgement is sent at once on demand and the mode asked for holds for the next
    data; rearming sends a due keep-alive probe at once; the window probe is captured one sequence number below the
    next and answered; the timestamp clock set in repair mode is what the next segment's TSval reads; negative
    advice gives an IPv6 connection a new flow label (not with transmit rehash off).
  - send_window_probe no longer clears `SO_REUSEADDR` or repair mode (both put back, with the repair queue);
    checked by window_probe_is_answered_and_changes_no_setting.
  - `TCP_QUICKACK`'s value 2 is the named `QUICK_ACKNOWLEDGEMENT_THEN_DELAYING` (Linux names none); the suite's
    BPF return comes from libc's `BPF_RET | BPF_K`.
  - A port the exit node chooses is now bound at once, for connections too, through the shared
    `bind_chosen_by_exit_node`: Linux ignores a socket's `IP_LOCAL_PORT_RANGE` outside the host's own range (the
    UDP track's finding), so a range above 60999 gave a port outside it. exit_node_chooses_a_port_outside_the_reserved
    now also takes a range of 61000 to 61020.
  - The host's interface addresses are read with nix's `getifaddrs` instead of unsafe libc. It still runs once per
    IPv4 accept (counted, warned): Linux gives no arrival interface for a TCP SYN.
- wait_for_events: wait_for_events_finishes_when_data_arrives, wait_for_events_answers_at_once_when_ready,
  dropping_a_wait_cancels_it, many_waits_each_wake_once, wait_for_room_finishes_when_the_peer_reads,
  wait_ends_with_the_errors_always_reported. wait_for_listener_events: listener_wait_finishes_when_a_connection_comes,
  dropping_a_listener_wait_cancels_it.
- Which check fails when a method or setting breaks, each one run as a broken copy by mutants.sh (results under
  Measurements): connect, listen, accept: every check. send and receive: bytes_arrive_in_order_both_ways and the
  request checks (urgent_*, peek_*, more_to_come_*, record_end_starts_a_new_segment, unread_byte_reports_*,
  receive_software_timestamps_*, the transmit report checks). shut_down: the three shutdown checks. close:
  orderly_close_*, abort_resets_the_peer. stop_listening: stopped_listener_refuses_connections.
  take_error_queue_entry and discard_error_queue_entries: the report, ICMP and discarding checks.
  push_queued_data: more_to_come_holds_data_until_pushed, push_ignoring_neither_*. send_delayed_acknowledgement:
  delayed_acknowledgement_is_sent_on_demand. rearm_keep_alive_timer: rearming_the_keep_alive_timer_sends_a_due_probe.
  send_window_probe: window_probe_is_answered_and_changes_no_setting. set_timestamp_clock:
  timestamp_clock_set_in_repair_mode_stamps_segments (the listener's: only its refusal, see For the owner).
  give_negative_route_advice: negative_route_advice_picks_a_new_flow. Settings: send and receive buffers, buffer
  locks, low waters (send_settings_*, receive_settings_*); window clamp (window_clamp_bounds_the_synchronize_window);
  pacing rate, transmit delay (their checks); hop limit, traffic class, path MTU discovery, flow label
  (header_settings_mark_each_segment); filter, minimum hop limit (arrival_settings_*); keep-alive and its idle,
  interval and probes (keep_alive_probes_end_an_unanswered_connection); no-delay, quick acknowledgement, delayed
  acknowledgement maximum, retransmission minimum and maximum, thin linear timeouts, user timeout, SYN retries,
  defer accept, FIN-WAIT-2, congestion control, address and port sharing, nonlocal addresses, transmit rehash,
  repair (their checks); cork, urgent inline, unread byte reports, maximum segment size, ICMP error reports (as
  before); listener backlog, fast open queue and keys, flow label reflection (their checks).
- Settings still checked only for being set twice the same, since loopback or the traits cannot show them yet:
  multicast loopback and all-groups (no effect on TCP traffic), packet priority and mark (not on the wire),
  unicast interface, route scope, source preferences, bound interface, incoming processor (one interface, no
  read-back before T2), IPv4 options and IPv6 extension headers, a fixed flow label, the IPv6 variants of traffic
  class and path MTU discovery, `SaveSynchronize` Off (shows only in `TakeSavedSynchronize`, T2). Refused with a
  counted reason and checked to answer the same twice: `RecordsLastReceivedHeader`,
  `SaveSynchronize(FromLinkHeader)`. T2 settings (authentication, repair queues, options and windows) are not in
  T1.

## Measurements
- Run 4, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-21 21:20, load 1.7 to 3.2: clippy with warnings
  denied clean; 90 passed, 0 failed, three runs in a row, each 3.4 s (89 suite checks and the allocation check).
- Run 4 broken copies (mutants.sh, now 90 copies), same machine, 20:49 to 21:19, load 2 to 21: every copy failed the
  suite (logs mutants-run-5.log and mutants-run-5b.log in scratch). New copies and what fails: the connection's
  wait finishing at once (six wait checks and the allocation check), never finishing (seven), the listener's
  wait finishing at once (both listener wait checks), never finishing (listener_wait_finishes_when_a_connection_comes),
  the shared readiness never withdrawing a dropped wait (both dropping checks and the allocation check), the
  readiness firing no wake (four, after the data and listener checks were made to demand a wake: they had passed
  on a poll alone), and the stopped agent's check of plain sends between reported ones. The first run stalled on
  never-finishing because the allocation check parked without a deadline; it now has one.
- ARM machine (Graviton4, 96 cores, Linux 7.0.0-1018-aws), 2026-11-21 19:34, load 3.4 to 5.8: 72 passed, 0
  failed, five runs in a row, each 3.3 s (the keep-alive, SYN retry and retransmission checks wait about 3 s).
- Broken copies (mutants.sh), same machine: 63 copies, 2026-11-21 19:50 to 19:57 (load 4 to 9.5, 6 min 53 s),
  each failed the suite; the 64th (exit-node port by the kernel only, as before the binding change) at 19:58 failed
  exit_node_chooses_a_port_outside_the_reserved. Each copy breaks one method, request or setting (every setting
  of the list above doing nothing, one at a time); the checks each one failed are in the scratch log
  mutants-run-3.log and mutants-run-4.log. Two copies of the first run passed and were mended: the timestamping
  copy broke code connections never reach (it now breaks the arm they use), and the retransmission maximum was
  missed because Linux can send a tail loss probe first (the check now looks at the longest gap over 3.2 s).
- Independent check from a clean clone of main at 239247f8 (job host-tcp-sockets-clean, 2026-11-21 19:49, load
  10 to 11): main's own Cargo.lock now parses; build directory emptied, clippy with warnings denied clean (9.3 s),
  72 passed; `cargo tree` of tcp-sockets, internet-protocol, configurable, negative-route-advice, kernel, abi and
  the suite shows no implementation crate; `git diff 4308b9f7 HEAD` of every interface.rs and data file this track
  uses is empty (only other tracks' conformance suites of configurable and negative-route-advice changed).
- After the binding change, 72 passed three times (19:52, load 9.3 to 9.6), clippy clean on Linux and the Mac.
- Calibration with Python on the same kernel (scratch tools/probe*.py), load 6 to 12: a connection whose peer
  reads nothing takes 2 720 256 bytes; with a 64 KiB send buffer 256 000, a locked send buffer 175 589, a 16 KiB
  not-sent low water 160 768. The peer then holds 113 152 unread; with a 1 MiB receive low water 2 069 998.
  `TCP_TX_DELAY` of 50 ms delays nothing without `fq` and 50.2 ms with it. 512 KiB at a pacing rate of 1 MB/s took
  0.20 s without `fq` and 0.23 s with it.
- Earlier runs: 43 and then 48 checks passed on the same machine (2026-11-21 18:27 and 18:50).

## How to run
- Every build and test runs on the ARM machine; the Mac only edits, formats (`nix develop -c rustfmt --edition
  2024 <file>`) and commits.
- Suite on the ARM machine (needs sudo; makes its own network namespace):
  `cloud/arm-run <checkout> --name host-tcp-sockets -- crates/tcp-sockets/implementation/host/run-conformance.sh`
  (`RUST_TEST_THREADS=12` before it keeps to the machine's share).
- Broken copies: the same with `crates/tcp-sockets/implementation/host/mutants.sh [names]` (about 20 s a
  copy, 90 copies; names pick some).
- Lint for Linux on the ARM machine: `cloud/arm-run <checkout> --name host-tcp-sockets -- 'cargo clippy -j
  $ARM_JOBS --no-deps --tests -p wcontainers-tcp-sockets-host -p wcontainers-tcp-sockets-conformance --
  -D warnings'`, and the same on the Mac without arm-run (with `CARGO_TARGET_DIR=$PWD/target/host-tcp-sockets
  nix develop -c`; there both crates build empty).
- Files copied from the Mac can carry times older than the ARM machine's last build, so cargo there may skip
  them: `touch` the changed files in the command when a result looks stale.
- The literal check (run 4, on the ARM machine in a copy given its own `git init`, since arm-run copies no .git)
  found eight findings in this track's files that earlier runs had not seen; seven are fixed (named constants,
  a different test flow label, alignment by `cmsghdr`'s type, the fast open sysctl as its two named bits). One
  stays: `LOOPBACK_INTERFACE = "lo"` in the suite restates the kernel implementation's `LOOPBACK_NAME`, which the
  suite may not import (an interface crate must not lean on an implementation); removing it needs the name in a
  crate both can use, or the suite finding loopback by its `IFF_LOOPBACK` flag (next run).

## Next (finishing T1 after run 4)
- Socket creation on socket2: move the shared `open_socket` (rustix `socket_with` inside the owner switch) to
  `socket2::Socket::new` with `Type::STREAM.nonblocking().cloexec()`, agreed with the UDP track, who is moving the
  shared crate to socket2.
- The one literal finding left (`"lo"` in the suite, see How to run).

## Next (T2)
- read_state and read_listener_state (every query), authentication (MD5, AO, counters), repair options, queues
  and windows, `RecordsLastReceivedHeader`, `SaveSynchronize(FromLinkHeader)` and what a guest's own saving
  means for `TakeSavedSynchronize`: counted refusals now.
- With read_state, give the settings listed above as unobservable a check of their effect.

## Departures from the plan
- Run 4: the plan's "the contract waits for nothing" is gone with the async ruling: the two waits are the shared
  readiness's (plan corrected), and the basic calls moved to socket2 (plan corrected).
- The plan said `SO_KEEPALIVE` rearms the keep-alive timer; Linux re-times it only on `TCP_KEEPIDLE` (and on
  keep-alive turned on from off), so the idle time is written again. Plan corrected.
- The binding and socket opening helpers moved into the shared crate for the UDP track.
- Run 1 moved the ICMP part of the error queue and three kinds of transmit report to T2; run 2 did them in T1.
- The plan's router namespace for ICMP errors became raw sockets on loopback: the check captures the SYN and
  sends the error itself, which needs no namespace setup beyond the suite's own.
- wireless_status_request_still_sends only checks that asking works and that loopback brings no status: no
  wireless link exists on the test machine.
- Cargo.lock: this track commits only its own lines of it (run 3: nix and memoffset, and its two packages'
  entries), applied to the index from a patch of those hunks; other tracks' uncommitted lines stay.
- Run 3: the plan said `SCM_TS_OPT_ID` carries a send's report key; Linux refuses it on TCP, so the key is kept by
  the kernel's byte offset (plan corrected). The plan's suite gained the wire checks above and the refused settings
  in the set-twice lists (plan corrected).
- Run 3: connections no longer choose their exit-node port at connect (which let one port serve several peers);
  they bind it at once like listeners, one path that keeps to ranges outside the host's own.
- Run 3: the suite's body is Linux only (it was empty of Linux-only calls before): the wire checks need the host's
  packet socket.

## Dependencies added
- Run 4: socket2 0.6.5 with feature `all` (the basic calls): MIT OR Apache-2.0. polling 3.11.0 comes through the
  shared crate (the UDP track added it): Apache-2.0 OR MIT.
- rustix 1.1.6 (also in the suite, for its raw and packet sockets): Apache-2.0 WITH LLVM-exception OR Apache-2.0
  OR MIT. netlink-packet-core 0.9.0 (the statistics' attributes): MIT. linux-raw-sys 0.12.1: the same. libc
  0.2.191 (also in the suite since run 3, for BPF codes): MIT OR Apache-2.0. etherparse 0.21.0: MIT OR
  Apache-2.0. Run 3: nix 0.31.4 with only its `net` feature (interface addresses): MIT; it brings memoffset
  0.9.1: MIT.

## For the owner
- Building one now starts a host thread and an epoll set, so it can fail: the suggested trait is
  `pub trait StandaloneTcpSockets: TcpSockets + Sized { fn new() -> Result<Self, HostError>; }` (the crate has
  `HostTcpSockets::new() -> Result<Self, HostError>`).
- `PollEvents` has `message_available` (`POLLMSG`), which Linux never reports for a socket: a wait asking only
  for it finishes only on an error, hang-up or closed descriptor, as `poll` does on Linux. Not a fault, but the
  contract does not say it.
- No way to build one: the crate has `HostTcpSockets::new()`. Suggested trait beside `TcpSockets`:
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
- Linux 7.0 does not keep a `TCP_WINDOW_CLAMP` set before connect: measured with Python, a clamp of 8192 bounds the
  SYN's and the handshake's windows (8000 with a segment size of 1000), then the windows grow (15 808, 23 712, ...)
  and the clamp reads 104 960. Set after the handshake it never shrinks the window already offered. So
  `WindowClamp` is honoured as Linux honours it, and the suite checks the SYN's window only.
- `set_listener_timestamp_clock` asks for a listener "in repair mode", but Linux never lets a listener into repair
  mode (`tcp_can_repair_sock` refuses `TCP_LISTEN`): the call is always EPERM on Linux, and the suite checks only
  that refusal.
- `FlowLabelReflection` on Linux reflects the SYN's flow label in the SYN-ACK only (measured: the accepted
  connection's data then carries a label of its own); the contract does not say which segments reflect it.

