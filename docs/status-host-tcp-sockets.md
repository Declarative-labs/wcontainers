# Status: host-tcp-sockets

Plan: docs/plan-host-tcp-sockets.md (what each method and query maps to on Linux, what cannot be honoured, the
suite's design and its own numbers). Milestones: T1 (connections, listeners, settings, the suite) and T2 (state,
authentication, repair, multipath) are both done; what is left waits on the owner (For the owner, at the end).
Runs 4 to 15 wrote a long history here; it is in this file's git log (`git log -p docs/status-host-tcp-sockets.md`).

## Run 17 (2026-11-22): C22 run 1, the owner's TCP interface changes of 2026-11-22 (merge 158b6c62)
- Bind on its own: `type BoundSocket = HostTcpBoundSocket` (descriptor, family, protocol, and whether its port was
  bound by number: a requested or kernel-chosen port other than 0, which Linux cannot report later). `bind` is
  `socket`, the `BindingSetting`s, `bind`, `getsockname`; `release` closes it.
- connect and listen take `TcpLocalEnd` by value and fail with `TcpLocalEndError`: an `Unbound` input always fails
  `Unbound`; a `Bound` input that failed before connecting or listening comes back `Bound`, except a connect's
  `EADDRNOTAVAIL` on a port bound as 0 (closed, `Unbound`), as Linux frees that port. listen now listens once, with
  the last `Backlog` among its settings, so nothing can fail after `listen` but `getsockname`, which disconnects first.
- `disconnect`: the watch is dropped (`Watched::unwatch`, new in the shared crate), `connect(AF_UNSPEC)` (rustix
  `connect_unspec`), then `SO_ERROR`; the socket comes back only for a port bound by number, never for an accepted
  connection. portable.rs closes the socket with an abort, counted (no `AF_UNSPEC` there).
- `TakePendingError` on connections and listeners: `SO_ERROR` through socket2's `take_error`, named by the shared
  crate's new `errno::errno_of` (every name of the kernel interface's `Errno`; a number it does not name is a counted
  refusal).
- `TcpListenerReading`: `tcpi_unacked` and `tcpi_sacked` are a listener's accept queue and backlog and a connection's
  `unacknowledged` and `selectively_acknowledged`.
- `EcnMode`: Linux 7.0's last `TCP_INFO` word is bit fields (`tcpi_ecn_mode:2`, `tcpi_accecn_opt_seen:2`,
  `tcpi_accecn_fail_mode:4`), 6.18 and 6.19 have two `u16`s there, same length, so the layout comes from the kernel's
  release (`uname`, read once). This also fixed a misreading on 7.0: the fail mode and option-seen fields were read
  in the 6.18 layout. A `TCP_INFO` without the Accurate ECN fields (before 6.18) gives Classic or Disabled from
  `TCPI_OPT_ECN`. On 6.18 and 6.19 there is no guess: Disabled when the flag is clear, else the reading is refused
  with the counted reason NO_ECN_MODE (For the owner). No such kernel was available to run.
- `TcpKeyVrf` for MD5 and TCP-AO keys: Any is no flag, Default the flag with index 0, Device the flag and its index.
  `TcpListenerSetting::RemoveAuthenticationKeyAsynchronously` is `TCP_AO_DEL_KEY` with `del_async`; a connection's
  removal always sends `del_async` 0. `TcpRepair::OffWithoutWindowProbe` is `TCP_REPAIR_OFF_NO_WP`.
- The two removed `TcpState`s are gone from the state table: Linux's 12 and 13 are now an unknown state, refused.
- `LastReceivedHeader { interface, header }`: the interface from the packet information record, None when missing.
  A connection now keeps `IP_PKTINFO`/`IPV6_RECVPKTINFO` on while it records the last header or reports ICMP errors
  (read back from `IP_RECVTTL` and `IP_RECVERR`), since an ICMP error names its arrival interface only there and its
  `ReceivedPacketFields.interface` can no longer be 0; a listener keeps it on always, as before. An accepted SYN with no
  interface found is refused, counted. Wireless status reports have no interface.
- `IpPacketSetting`: `UnicastInterface` is gone and `TrafficClass` writes the DSCP alone (shared crate).
- Corrected: a listener can be in repair mode: one that listened with `Repair(On)` among its settings stays in it and
  takes `TCP_TIMESTAMP`. timestamp_clock_set_in_repair_mode_stamps_segments now demands that.
- Shared crate (commit 04600f02; read by the UDP, ICMP and raw tracks, see their section below): non-zero
  `InterfaceIndex`, `InterfaceSelector`, `Slice1` include filters, no `UnicastInterface`, no ECN bits in
  `TrafficClass`, optional report interfaces, `Watched::unwatch`, `errno::errno_of`; `set_packet_setting` lost its
  unused `family` argument (only this track calls it).
- New checks (11), each with the Linux call it covers and measured Linux behaviour from the probes:
  a_bound_socket_holds_its_port_until_released (`bind` alone holds the port: a second bind EADDRINUSE, a connect to
  it ECONNREFUSED, free after `release`), connect_and_listen_take_a_bound_socket_and_its_port,
  a_failed_connect_or_listen_gives_back_its_bound_socket (`connect` ENETUNREACH and `listen` EADDRINUSE keep the
  port), a_clashing_connect_gives_back_only_a_port_bound_by_number (`connect` EADDRNOTAVAIL),
  disconnect_gives_back_only_a_port_bound_by_number (`connect(AF_UNSPEC)`: ECONNRESET after an established
  connection, ECONNREFUSED after a refused one, the port held only when bound by number), pending_error_is_taken_once
  (`SO_ERROR` once, then none; a fresh listener none), listener_reads_its_accept_queue_and_backlog (backlog 7 with 2
  waiting; 100 000 held to `somaxconn`), leaving_repair_without_a_window_probe_sends_none (`TCP_REPAIR_OFF_NO_WP`
  sends nothing, `TCP_REPAIR_OFF` a probe), a_listener_removes_a_key_asynchronously (`del_async` 1),
  keys_are_for_the_vrf_named (`TCP_AO_KEYF_IFINDEX`/`TCP_MD5SIG_FLAG_IFINDEX`: a default-VRF key survives removal as
  any VRF and goes as the default one; loopback, no VRF, EINVAL for both kinds), ecn_mode_is_what_both_ends_negotiated
  (`tcpi_ecn_mode`: `tcp_ecn` 0, 1 and 3 at both ends give Disabled, Classic, Accurate, each on a thread in a network
  namespace of its own, `unshare(CLONE_NEWNET)`, loopback up by `SIOCSIFFLAGS`, so no other check sees the sysctl).
  Changed: last_received_header_records_where_the_last_packet_came reads `Some(loopback)` or None and adds a
  connection made by connect (IPv4 None, IPv6 loopback); the DCTCP check wants Classic at both ends; the information
  check reads the two segment counts from the connection reading; the setting lists lost `UnicastInterface` and `ecn`.
- mutants.sh: 189 copies; five whose text had moved follow the code, and nine new ones break the new calls
  (disconnect-keeps-every-port, port-zero-bound-by-number, clash-keeps-every-port, pending-error-never,
  listener-queue-and-backlog-swapped, repair-off-probes-always, vrf-default-as-any, ecn-accurate-as-classic,
  last-header-interface-none). All 15 run failed the suite (ARM, 19:40 to 19:43, load 6 to 11; log
  c22/mutants.log in scratch); which check each failed is in Which check fails.
- Not checked by behaviour: that the listener's removal is asynchronous (a synchronous one on a listener looks the same
  from outside), and a listener's pending error other than none (Linux sets one only when the socket is destroyed from
  outside, `ss -K`).

## Run 16 (2026-11-22): T2 run 1, the independent check of run 15 answered
- mutants.sh's three setting copies used the names removed at a00ab267: now `Binding(BindingSetting::...)`; each
  fails its check (AddressSharing: address_sharing_lets_connections_leave_from_one_port and
  window_probe_is_answered_and_changes_no_setting; PortSharing: port_sharing_lets_listeners_share_a_port;
  NonlocalAddresses: nonlocal_addresses_are_bound_only_when_bindable).
- A copy that does not build is now told apart: run-conformance.sh keeps cargo's rendered errors on standard error
  (`--message-format=json-render-diagnostics`) and exits 2 when the test binaries do not build; mutants.sh prints
  "the copy did not build" for that exit and "the suite did not fail" only when the binaries ran and passed.
  Tried: a copy given a `compile_error!` printed its error and exited 2.
- New check send_reports_the_bytes_it_took: one plain send of 32 MiB (`ONE_SEND_OFFERED`, more than loopback's
  buffers take at once) to a peer that reads nothing, then a write shutdown; exactly the bytes the send reported
  must arrive. It holds for any correct implementation (one that takes all the bytes passes too). The checker's copy
  send-claims-all (in mutants.sh) fails it and the repair check.
- The plan's connect bullet now says what the code does: when the exit node chooses, `bind` at once under
  `IP_LOCAL_PORT_RANGE` in the part of each range inside the host's range, then each port of the rest, counted; no
  `IP_BIND_ADDRESS_NO_PORT`.
- portable.rs is built and linted on the Mac (macOS, `cargo clippy --no-deps --tests` of the host and the suite with
  warnings denied, `nice -n 19`, 11 s): clean. The owner's linter reads that build.
- The readiness suite edit (61bb2b6f, crates/readiness/conformance/conformance.rs): kept, since the 2026-11-22 notice
  ordered the readiness suite to follow `ReadinessReport`; it only follows the type and removes no check (the checker
  read it). No further edit there.
- This file was cut from 911 lines to what is current.
- Two flaky checks found in 76 full runs, both fixed. timestamp_reads_the_clock_in_its_unit failed twice in 50
  (50 057 µs counted over 50 054 984 ns measured, and 50 058 over 50 055 863): the kernel rounds the time the clock
  is set and the time the segment leaves down to the unit, which with the unit bit lets the count exceed the time
  measured around it, rounded down, by two ticks; the counts went 16 and 137 ns past even that, the host's clock
  against the suite's. `TIMESTAMP_SLACK` is 3 ticks (was 2), reason in the plan; the units still differ a
  thousandfold, so a clock in the wrong unit fails as before. information_reads_what_the_connection_did failed once:
  the receiver had one segment out of order with no retransmission (loopback queues each packet on the sending
  processor, so a sending thread that moves can reorder; the cause was not measured), and the sender had seen that reordering (`reordering_seen` 1); the check now demands that a
  segment received out of order without retransmission was seen as reordered by the sender, in place of none out of
  order. After both: 60 full runs, no failure.
- congestion_control_state_names_the_algorithm failed once (see Known flake); not seen again in 135 runs.

## What exists
- `crates/tcp-sockets/implementation/host` (package `wcontainers-tcp-sockets-host`; types `HostTcpSockets`,
  `HostTcpConnection`, `HostTcpListener`, fields private; public beyond the traits: `new()` and `counters()`).
  linux.rs and linux/state.rs: every call, request, setting, query and method of the contract on Linux's TCP and
  MPTCP sockets (socket2 for what it covers, rustix, linux-raw-sys and libc for the rest; the mapping is the plan's
  "Methods on the host"). portable.rs (macOS and other unix): the basic calls on socket2, every other call a
  counted refusal; built and linted on the Mac, never run. implementation.rs holds the exported types.
- `Readiness<HostTcpConnection>` and `Readiness<HostTcpListener>` on the shared crate's `readiness::Watched`
  (polling over epoll, one host thread per `HostTcpSockets`, no allocation per wait once warm, dropping a wait
  withdraws it); `POLLERR` and `POLLHUP` are the report's `unconditional` events.
- Shared crate `crates/internet-protocol/implementation/linux-socket-options` (made by this track, used by the UDP,
  ICMP and raw hosts): errno mapping, raw option calls, `Shortfalls` (counted refusals, warned once), the internet
  protocol settings, binding and port ranges, `open_socket`, control messages, `write_icmp_error`, `readiness`.
- Suite `crates/tcp-sockets/conformance`: 121 checks that each take one value, against the traits alone, plus 7
  functions that build values for the required suites (Configurable on a connection and a listener,
  NegativeRouteAdvisable, four Readiness bundles); `tests!` runs 137 tests, and the implementation's own 4 run beside
  them (allocation checks, discarding_cuts_no_packet, the two-thread peeks). It runs on Linux only (a capture thread
  reads the host's packet socket on loopback) as root in a namespace run-conformance.sh makes.
- mutants.sh: 189 broken copies, each breaking one method, request, query, setting or field; every one has failed
  the suite in some run (the latest full run of all is the checker's of run 15; the 15 changed or new of run 17 ran
  then).

## Which check fails when a method breaks
- Run 17's copies: listen-backlog-argument-ignored listener_reads_its_accept_queue_and_backlog,
  listener_backlog_bounds_the_accept_queue and the two ICMP checks of a synchronizing connection; listener-state-refused
  12 listener checks; disconnect-keeps-every-port disconnect_gives_back_only_a_port_bound_by_number;
  port-zero-bound-by-number that and a_clashing_connect_gives_back_only_a_port_bound_by_number; clash-keeps-every-port
  the clash check; pending-error-never a_bound_socket_holds_its_port_until_released and pending_error_is_taken_once;
  listener-queue-and-backlog-swapped listener_reads_its_accept_queue_and_backlog; repair-off-probes-always
  leaving_repair_without_a_window_probe_sends_none; vrf-default-as-any keys_are_for_the_vrf_named;
  ecn-accurate-as-classic ecn_mode_is_what_both_ends_negotiated; last-header-interface-none
  last_received_header_records_where_the_last_packet_came; fixed-flow-label-not-held, fast-open-connect-ignored,
  accept-interface-none and subflow-information-zeroed their checks as before.
- bind and release: a_bound_socket_holds_its_port_until_released and every check below that binds first. connect and
  listen from a bound socket: connect_and_listen_take_a_bound_socket_and_its_port and the failure, clash and
  disconnect checks. disconnect: disconnect_gives_back_only_a_port_bound_by_number. TakePendingError (both):
  pending_error_is_taken_once. Listener Information: listener_reads_its_accept_queue_and_backlog. Repair
  OffWithoutWindowProbe: leaving_repair_without_a_window_probe_sends_none. RemoveAuthenticationKeyAsynchronously:
  a_listener_removes_a_key_asynchronously. Key VRF: keys_are_for_the_vrf_named. ECN mode:
  ecn_mode_is_what_both_ends_negotiated and the DCTCP case of congestion_control_state_names_the_algorithm.
- connect, listen, accept: every check. listen's backlog argument: listener_backlog_bounds_the_accept_queue and the
  two ICMP checks of a synchronizing connection. The owner: connect_and_listen_make_sockets_of_their_owner. The
  protocol: multipath_is_negotiated_when_both_ends_ask. Accept's frame length: connect_and_accept_name_both_ends.
- send: bytes_arrive_in_order_both_ways, send_reports_the_bytes_it_took (its count), and the request checks
  (urgent_*, more_to_come_*, record_end_starts_a_new_segment, the transmit report checks). receive: the same plus
  peek_*, unread_byte_reports_*, receive_software_timestamps_when_asked.
- shut_down: the three shutdown checks. close: orderly_close_ends_the_peers_stream, abort_resets_the_peer.
  stop_listening: stopped_listener_refuses_connections.
- take_error_queue_entry, discard_error_queue_entries: the report, ICMP and discarding checks.
- push_queued_data: more_to_come_holds_data_until_pushed, push_ignoring_neither_*. send_delayed_acknowledgement:
  delayed_acknowledgement_is_sent_on_demand. rearm_keep_alive_timer: rearming_the_keep_alive_timer_sends_a_due_probe.
  send_window_probe: window_probe_is_answered_and_changes_no_setting. give_negative_route_advice:
  negative_route_advice_picks_a_new_flow and the module negative_route_advice.
- set_timestamp_clock: timestamp_clock_set_in_repair_mode_stamps_segments, timestamp_clock_in_the_other_unit_counts_in_it,
  timestamp_reads_the_clock_in_its_unit, the repair check. set_listener_timestamp_clock: the first of these (Linux
  refuses: see For the owner).
- set_authentication_counters and the listener's: authentication_counters_are_set_only_where_authentication_is,
  authentication_state_reads_the_keys_and_counters.
- read_state by query: Information information_reads_what_the_connection_did; CongestionControl
  congestion_control_state_names_the_algorithm; Queues queues_count_unread_unacknowledged_and_unsent; Tuned
  tuned_values_follow_the_settings; TakeSavedSynchronize saved_synchronize_is_taken_once; Timestamp
  timestamp_reads_the_clock_in_its_unit; Repair repaired_connection_continues_where_the_original_left_off and
  authentication_repair_values_are_read_and_written; Authentication and AuthenticationKey
  authentication_state_reads_the_keys_and_counters; Multipath and MultipathSubflow
  multipath_state_describes_the_connection; LastReceivedHeader last_received_header_records_where_the_last_packet_came;
  IncomingProcessor incoming_processor_is_read_as_received_or_set; every query on a multipath connection and
  listener multipath_connection_answers_every_query; the set-twice checks compare tuned values, authentication state
  and the last header. read_listener_state: listener_state_reads_a_listener and the Tuned, Timestamp,
  authentication and incoming processor checks above.
- Repair settings: QueueSequence, RepairOptions, RepairWindow the repair check; AuthenticationRepair the
  authentication repair check.
- wait_until_ready on a connection: the modules readiness_tcp_connections and readiness_tcp_connections_hanging_up,
  waits_on_many_connections_each_wake_once, wait_for_room_finishes_when_the_peer_reads,
  wait_for_urgent_data_finishes_when_it_arrives, wait_for_the_peers_shutdown_finishes_when_it_comes,
  wait_for_a_hang_up_finishes_when_both_ways_are_shut and wait_for_an_error_finishes_when_the_peer_resets (these two
  ask for nothing, so only an unconditional event ends them), a_wait_gives_the_facts_the_connection_has_and_no_others.
  On a listener: the module readiness_tcp_listeners and a_wait_gives_the_facts_the_listener_has_and_no_others.
- configure (both traits): the modules configurable_tcp_connections and configurable_tcp_listeners (set twice equals
  set once), and each setting's own check (buffers and low waters: send_settings_* and receive_settings_*; window
  clamp; pacing; transmit delay; header_settings_mark_each_segment for hop limit, traffic class, path MTU discovery
  and flow label; arrival_settings_* for filter and minimum hop limit; keep-alive with idle, interval and probes;
  no-delay, quick acknowledgement, delayed acknowledgement maximum, retransmission bounds, thin linear timeouts, user
  timeout, SYN retries, defer accept, FIN-WAIT-2, congestion control, address and port sharing, nonlocal addresses,
  transmit rehash, repair, memberships, MD5 and authentication keys and options, cork, urgent inline, unread byte
  reports, segment size, ICMP error reports, records of the last header, save SYN, the mark; listener backlog, fast
  open queue and keys, flow label reflection). Settings only the set-twice checks see, since loopback shows nothing
  more: multicast loopback and all-groups, packet priority, unicast interface, route scope, source preferences,
  bound interface, IPv4 options and IPv6 extension headers on established connections, the IPv6 variants of traffic
  class and path MTU discovery.
- Tried by breaking the implementation this run (ARM machine, 06:23 to 06:25): send-claims-all (send's count),
  setting-AddressSharing-noop, setting-PortSharing-noop, setting-NonlocalAddresses-noop: each failed the checks named
  above.

## What Linux cannot honour (each answered with the contract's error or Linux's own, counted with a named reason)
- Kernel and configuration needed: `CONFIG_TCP_AO` (Linux 6.7+) for the authentication keys, options, readings and
  repair values; `CONFIG_MPTCP` (5.6+) for multipath, `MPTCP_FULL_INFO` (6.5+) for subflows, later kernels for the
  newest `mptcp_info` fields and 6.18+ for `TCP_INFO`'s Accurate ECN fields (zero and counted before: SHORT_INFORMATION,
  SHORT_MULTIPATH_INFORMATION); 6.7+ for a microsecond timestamp clock (NO_MICROSECOND_CLOCK before); CAP_NET_ADMIN for
  repair mode, the window probe, the timestamp clock, the mark, transparent addresses and the flow label manager
  (Linux's EPERM without it); CAP_SETUID to own a socket as another user (EPERM); the `tcp_dctcp`, `tcp_vegas` and
  `tcp_bbr` modules for those congestion controls. The test machine (Linux 7.0.0-1018-aws) has all.
- Linux's own refusals pass through: repair reads out of repair mode (EPERM, EINVAL), `TCP_AO_INFO` without
  authentication (ENOENT), `MPTCP_INFO` on TCP or a fallen-back connection (EOPNOTSUPP), `TCP_TIMESTAMP` on MPTCP
  (EOPNOTSUPP), the timestamp clock of a listener that did not listen in repair mode (EPERM), a fixed flow label after
  connect (FLOW_LABEL_FIXED_AT_CONNECT).
- What Linux does not record, given as the nearest truth and counted: a SYN's priority, timestamps, own mark and
  fragment size, its IPv4 arrival interface, and everything of a multipath SYN (SYNCHRONIZE_FIELDS_OF_THE_SOCKET,
  NO_SYNCHRONIZE_FRAGMENT_SIZE, ARRIVAL_INTERFACE_OF_THE_HANDSHAKE, NO_SAVED_SYNCHRONIZE, ARRIVAL_INTERFACE_BY_ADDRESS);
  the last packet's length, fixed header and IPv4 options (NO_RECORDED_LENGTH, NO_RECORDED_IPV4_OPTIONS);
  `tcpi_last_ack_sent` and `tcpi_fackets` (UNFILLED_INFORMATION); a multipath socket's urgent mark, window clamp and
  quick acknowledgement mode (NO_MULTIPATH_URGENT_MARK, NO_MULTIPATH_CLAMP); a transmit report with both packet and
  statistics (PACKET_OR_STATISTICS).

## Measurements
- Run 17, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-22: clippy with warnings denied (`--no-deps
  --tests`) clean on the TCP host, its suite and the shared crate after `cargo clean` (35 crates built; job
  host-tcp-sockets-clean, 19:45, load 5 to 8); then 10 full runs at `RUST_TEST_THREADS=12`, each 137 + 4 passed in
  about 7.3 s (19:45 to 19:47, load 7 to 9; log c22/clean2.log in scratch). Before that 3 more full runs, all passed
  (19:44, load 6). Mac (macOS, `nice -n 19`, 19:42, load 6.8): clippy of the host (portable.rs) and suite clean.
- Interface files: no file of the tcp-sockets, internet-protocol, kernel, readiness, configurable,
  negative-route-advisable or abi interfaces changed since merge 158b6c62, and all but abi's equal the main it merged
  (3731f877); abi's exit.rs, host.rs and interface.rs differed from that main before this run (not this track's).
- Run 16, ARM machine (Graviton4, 96 cores, Linux 7.0.0-1018-aws), 2026-11-22: clippy with warnings denied
  (`--no-deps --tests`) clean on the TCP host and suite (06:23, load 14). Full suite at `RUST_TEST_THREADS=12`:
  06:23 (load 13 to 14) 125 passed, 1 failed (congestion_control_state_names_the_algorithm), 4 passed; then 30 lone
  runs of that check, 0 failed (06:24, load 7 to 8); 25 full runs (06:25 to 06:28, load 6 to 11), each 126 + 4
  passed in 7.3 s (logs t2r1/congestion.log and repeat20.log in scratch); 50 full runs (06:29 to 06:36, load 6 to
  11), 47 passed, 3 failed (the two flakes above; log repeat50.log). After their fix: 60 full runs (06:37 to 06:45,
  load 6 to 10), each 126 + 4 passed (repeat60.log).
- Run 16 clean check (job host-tcp-sockets-clean, build folder deleted first, 06:46, load 4 to 9; log
  t2r1/clean-check.log in scratch): clippy with warnings denied clean on the TCP host, its suite and the shared crate
  (9.6 s); 126 + 4 pass twice; `cargo tree -e normal,build,dev` of tcp-sockets, internet-protocol, configurable,
  negative-route-advisable, readiness, the TCP suite and the readiness suite names only abi, configurable(-conformance),
  executor, internet-protocol, kernel, negative-route-advisable(-conformance), readiness(-conformance) and
  tcp-sockets(-conformance): no implementation. `git log a00ab267..HEAD` and `git status` of every .rs file beside
  the tcp-sockets, internet-protocol, readiness, negative-route-advisable, configurable, kernel and abi interfaces
  (interface.rs and the data files): none.
- Mac (macOS, 06:24): clippy with warnings denied of the host (portable.rs) and the suite: clean, 11 s.
- Run 16 broken copies (mutants.sh, 06:23 to 06:25, load 11 to 14; log t2r1/fixes.log in scratch): the four above,
  each failed. After the two flake fixes (06:47 to 06:49, load 2 to 8; log t2r1/mutants-clock.log): the seven clock
  copies (clock-always-refused, clock-unit-ignored, both route refusals, clock-microseconds-as-milliseconds,
  clock-set-outside-repair, timestamp-unit-milliseconds) and four of the information reading
  (information-state-shifted, information-acknowledged-as-received, information-rtt-is-variance,
  ic-segments-in-is-out) each failed, the clock ones in the timestamp checks and the information ones in
  information_reads_what_the_connection_did: the wider slack lets no wrong unit through.
- Run 15 (05:15 to 05:24, load 7 to 21): clean build, clippy clean, readiness suite 13, shared crate 1, TCP 125 + 4
  pass twice; 27 broken copies failed.
- Run 14 (04:26 to 04:34, load 7 to 24): the seven changed checks alone 30 times and the whole suite 10 times at 12
  threads: no failure.
- Run 13, probes as root in a namespace (scratch tools/probe_t2.py, load 2 to 4): repair reads out of repair mode
  EPERM (window) and EINVAL (sequence); `TCP_AO_INFO` without authentication ENOENT; `MPTCP_INFO` 96 bytes on a
  multipath socket and listener, EOPNOTSUPP on TCP and a fallen-back socket; `SO_INCOMING_CPU` -1 on a fresh
  listener; `IP_MTU` ENOTCONN on a listener; `TCP_CC_INFO` 0 bytes for cubic, 16 for vegas and dctcp, 20 for bbr;
  `TCP_TIMESTAMP` EOPNOTSUPP on MPTCP; a connection read in repair mode and written into a new socket carries bytes
  both ways with the old peer.
- Run 8 (load 2.8 to 5.3): 60 full runs in a row at 12 threads, no failure. Run 8 probe: as root and uid 1000,
  `MCAST_JOIN_GROUP` taken on a TCP socket; anycast join EADDRNOTAVAIL as root, EPERM as uid 1000; the flow label
  manager EPERM as uid 1000.
- Calibration with Python on the same kernel (run 3, load 6 to 12): a connection whose peer reads nothing takes
  2 720 256 bytes; with a 64 KiB send buffer 256 000, a locked send buffer 175 589, a 16 KiB not-sent low water
  160 768; the peer then holds 113 152 unread, 2 069 998 with a 1 MiB receive low water. `TCP_TX_DELAY` of 50 ms
  delays nothing without `fq` and 50.2 ms with it. Pacing: 1 MiB at 1 MiB/s took 704 ms paced, 0.65 ms not (run 8).

## Known flake
- congestion_control_state_names_the_algorithm failed once in 136 full runs of run 16 (06:23, load 13 to 14, the
  first run after a build); the panic message was not kept (that run printed test names only). It passed 30 times
  alone and in every other full run. Its load-sensitive bounds are Vegas's round trip below 50 ms and BBR's
  bandwidth at most every segment sent delivered within the smallest round trip. If it fails again, the message
  (`--nocapture` or the panic line) says which; the check is not weakened.

## How to run
- Every build and test runs on the ARM machine (cloud/arm-run); the Mac only edits, formats (`nix develop -c
  rustfmt --edition 2024 <file>`) and commits, except the macOS build of portable.rs, which only the Mac can make:
  `CARGO_TARGET_DIR=$PWD/target/host-tcp-sockets nice -n 19 nix develop -c cargo clippy -j 4 --no-deps --tests -p
  wcontainers-tcp-sockets-host -p wcontainers-tcp-sockets-conformance -- -D warnings` (delete the folder after).
- Suite (needs sudo; makes its own namespace): `cloud/arm-run <checkout> --name host-tcp-sockets --
  'RUST_TEST_THREADS=12 crates/tcp-sockets/implementation/host/run-conformance.sh'`. Exit 1: a test failed; 2: the
  test binaries did not build.
- Broken copies: the same with `crates/tcp-sockets/implementation/host/mutants.sh [names]` (about 90 s a copy, 180
  copies; names pick some). Whether each copy's text is there once: scratch tools/check_mutants.sh <checkout>.
- Lint for Linux: `cloud/arm-run <checkout> --name host-tcp-sockets -- 'cargo clippy -j $ARM_JOBS --no-deps --tests
  -p wcontainers-tcp-sockets-host -p wcontainers-tcp-sockets-conformance -- -D warnings'`.
- Files copied from the Mac can carry times older than the ARM machine's last build: `touch` the changed files in
  the command when a result looks stale.
- The literal check: in a copy given its own `git init` on the ARM machine (`rm -rf target/lit && mkdir -p
  target/lit && tar --exclude=./target -cf - . | tar -xf - -C target/lit && cd target/lit && git init -q && git add
  -A && cargo run -q -p wcontainers-literals -- check`).
- The owner's linter (`cargo reviewable check`) is not to be run until the orchestrator says so. Its last run here
  (run 14) left in this track's files only the structural findings under For the owner (7 `conformance_not_run`, 7
  `public_outside_interface`, 5 `implementation_folder`) and 2 `grammar_unclear`.
- Cargo.lock in this shared checkout: commit only this track's lines (a private index: `GIT_INDEX_FILE` set to a
  scratch file, `git read-tree HEAD`, `git apply --cached` of a patch of those lines, `git commit`).

## Next
- Nothing of T1 or T2 is open but the owner's rulings below. Run the owner's linter once when it may be used again.

## Departures from the plan
- Run 17: listen listens once with the last `Backlog` among its settings (was: listen, then listen again), so that
  a failure after listening cannot leave a bound socket listening. A connection's packet information follows its
  records and ICMP error reports (was: only its records), for the ICMP error's arrival interface. The plan says both.
- Run 16: `TIMESTAMP_SLACK` 3 ticks and the out-of-order relation of the information check (above), each a
  measured tolerance of the host, not of the implementation.
- Tuned's path MTU is `tcpi_pmtu` for connections and listeners alike (`IP_MTU` is ENOTCONN on a listener); the
  congestion information's form is chosen by name for DCTCP and by length for BBR and Vegas (`TCP_CC_INFO` does not
  say which it wrote).
- The suite's own numbers link to the plan's section "The suite's own choices" (DECISIONS.md 11 of the linter).
- The implementation builds on every unix system (portable.rs), since the owner's linter reads the Mac's build.
- `TCP_KEEPIDLE` written again rearms the keep-alive timer (`SO_KEEPALIVE` does not re-time it).
- ICMP errors for the suite come from raw sockets on loopback (the check captures the SYN and answers it), not a
  router namespace. wireless_status_request_still_sends checks only that asking works: no wireless link exists there.
- `SCM_TS_OPT_ID` is refused on TCP, so a send's report key is kept by the kernel's byte offset.
- Connections bind an exit-node-chosen port at once, as listeners do (run 3).
- MD5 keys and the TCP Authentication Option's keys, options and initial counters moved to T1 (no privilege
  needed); the authentication repair values and `set_authentication_counters` stayed T2.

## Dependencies (all MIT or Apache-2.0)
- socket2 0.6.5 (feature `all`): MIT OR Apache-2.0. rustix 1.1.6 (also the suite's): Apache-2.0 WITH LLVM-exception
  OR Apache-2.0 OR MIT. linux-raw-sys 0.12.1 (features net, if_tun, ioctl; also the suite's): the same. libc 0.2.191:
  MIT OR Apache-2.0. etherparse 0.21.0: MIT OR Apache-2.0. netlink-packet-core 0.9.0: MIT. nix 0.31.4 (feature
  `net`; also the suite's) with memoffset 0.9.1: MIT. polling 3.11.0 through the shared crate: Apache-2.0 OR MIT
  (chosen over mio because it registers a socket of any kind on every system, which the raw and ICMP tracks need).
- Neither libc nor linux-raw-sys has `mptcp_info`, `mptcp_full_info`, `mptcp_subflow_info`, the `tcp_cc_info` forms
  or `TCP_INFO`'s Accurate ECN fields, so linux/state.rs defines them `repr(C)` from Linux's uapi headers (linked).

## For the UDP, ICMP and raw tracks
- The shared readiness answers `ReadinessReport`; `Watched::hold` takes one; `readiness::NOTHING` is a report of no
  fact; `DATAGRAM_FACTS` is READABLE | WRITABLE. The shared `set_port_setting` is `set_binding_setting`.
- 2026-11-22 (04600f02), for the interface changes: `Ancillary::packet_fields` returns `Result` and refuses a packet
  without packet information or with index 0 (`EPROTO`; Every host turns it on at open, so this should not happen):
  add `?`. `Ancillary::interface()` gives the optional index. `transmit_report` gives `interface: None` when Linux
  names none; a rejected transmit time always None; `REPORT_INTERFACE_NOT_GIVEN` is gone (its check in the ICMP
  host's tests/counters.rs must go too), `NO_ARRIVAL_INTERFACE` counts an ICMP error without packet information.
  Datagram sends name a route's interface per packet alike for `InterfaceSelector::Bound` and `Requested` (a socket
  bound to the device is strict already through its `BoundInterface` setting): yours to refine if your probes say
  otherwise. `SourceFilter::Include` takes `Slice1`. `Watched::unwatch` gives the descriptor back unwatched;
  `errno::errno_of` names a Linux error number.
- `receive_message` allocates nothing: `ReceivedMessage` has `length`, `flags` and `control`, and `messages()` walks
  them in place (`ControlMessages`). `icmp_error::write_icmp_error` writes an error queue entry's ICMP error whole.
- `Shortfalls` counts without a lock (64 slots; reasons past them counted under one overflow reason, warned at
  every count).

## For the owner
- The shared helper crate crates/internet-protocol/implementation/linux-socket-options implements no interface,
  so the layout reports everything it exports (`public_outside_interface`, 20) and wants a conformance target it
  cannot have. Example: `Shortfalls::refuse`, used by all four socket implementations. It needs a place in the
  layout (an interface of its own, or a kind of crate the linter allows to export).
- The brief says an implementation's fields are private; the linter's `public_outside_interface` says a type that
  fills an associated type is plain data with every field `pub`. Example: `HostTcpConnection` holds its socket's
  descriptor and registration; made public, any caller could take the descriptor out. Kept private.
- The linter reads the Mac's build, so a Linux-only file is read as tokens: its impls of the required traits
  (`impl Readiness<HostTcpConnection> for HostTcpSockets` in linux.rs) are reported as `implementation_folder`,
  while the same impls in portable.rs pass.
- The readiness suite cannot be run as the layout writes a required suite (`tests!($crate::f($value))` with `f`
  giving `(T, T::Connection)`): its checks need a way to make the handle ready, so it takes a `Waiting` bundle with
  a closure. Example: `a_wait_finishes_when_the_handle_becomes_ready` must send data to the connection while a wait
  is pending.
- Building one starts a host thread and an epoll set, so it can fail: the suggested trait is
  `pub trait StandaloneTcpSockets: TcpSockets + Sized { fn new() -> Result<Self, HostError>; }` (the crate has
  `HostTcpSockets::new() -> Result<Self, HostError>`).
- `owner: UserId` is undefined when the exit node runs as one user: a guest's root connecting through a host
  process running as uid 1000 can only be answered EPERM, so on an unprivileged host every guest connect fails. The
  suite runs as root.
- `TcpSetting::NotSentLowWater(None)` does not say whether None is no bar or the host's default: Linux reads 0 as
  its sysctl and `u32::MAX` as no bar. The implementation writes `u32::MAX`.
- A fixed IPv6 flow label can be asked of a listener or an established connection, but Linux fixes a TCP socket's
  label only as it connects. Example: a connection made without a label and then given one is answered NotSupported.
- `TcpAccepted::synchronize` asks for fields Linux never reports for a SYN: for IPv4 the arrival interface (Linux
  keeps that of the segment that completed the handshake), and the SYN's own mark, priority and timestamps. Its
  `largest_fragment` is always None: `ReceivedPacketFields` does not say whether None means "not reassembled" or
  "not known". Example: a SYN that came in two fragments is reported exactly like one that did not.
- Linux gives an accepted multipath socket mark 0 whatever its SYN's mark. Example: a multipath client with mark 7
  to 127.0.0.2 is accepted with mark 0.
- A multipath accept has no SYN to give (no `TCP_SAVED_SYN` on MPTCP): `synchronize_header` is empty and its length
  0, counted. The contract has no way to say the SYN is unknown (an `Option` would).
- `TcpTransmitStatistics::received_hop_limit` is required, but Linux gives
  `TCP_NLA_TTL` only in a report an acknowledgement brought: 0 otherwise. A wireless status report names no send on
  Linux (`ee_data` 0): key 0. Linux gives a TCP socket's transmit reports the packet or statistics, never both.
  `TransmitReportKey` cannot be passed to Linux for TCP (`SCM_TS_OPT_ID` refused): kept by byte offset.
- `TcpReading::LastReceivedHeader` asks for the header of the last packet, but Linux keeps for a TCP socket only
  records: no fixed header, and for IPv4 no options. The implementation gives the IPv6 extension headers recorded as
  the header bytes, none for IPv4, counted. Example: an IPv6 segment with an 8-byte hop-by-hop header reads `[6, 0, 0, 0, 0, 0, 0, 0]`.
- Linux 7.0 does not keep a `TCP_WINDOW_CLAMP` set before connect beyond the handshake (measured: a clamp of 8192
  bounds the SYN's window, then the windows grow and the clamp reads 104 960); the suite checks the SYN's window.
- The Vegas form of `CongestionControlInformation` names Linux's fields the other way round: `tcpv_rtt` is Vegas's
  base RTT, `tcpv_minrtt` the lowest of the current round (0x7fffffff µs before its first sample). Given as Linux
  gives them.
- `MultipathInformation::fell_back_to_tcp` is never true on Linux 7.0: `MPTCP_INFO` on a fallen-back connection
  answers EOPNOTSUPP. Example: a multipath client to a plain listener.
- `TcpQuery::CongestionControl` with a buffer shorter than the name: Linux cuts it ("reno" into 3 bytes reads
  "ren"); the contract does not say.
- `TcpInformation::bytes_acknowledged` counts the SYN on Linux: 1 048 577 after 1 MiB acknowledged.
- Neither clock call names its error outside repair mode, so the suite takes any refusal. (A listener enters repair
  mode only by listening in it: `tcp_can_repair_sock` refuses `TCP_LISTEN`.)
- `EcnMode` on Linux 6.18 and 6.19: `TCP_INFO` has no `tcpi_ecn_mode` and `TCPI_OPT_ECN` is set for Classic and
  Accurate ECN alike, so the host refuses the whole Information reading of a connection using ECN there (counted,
  NO_ECN_MODE) rather than guess. Example: two 6.18 ends with `tcp_ecn` 3 cannot read their `TCP_INFO`. Your ruling
  is wanted: refuse those kernels as exit nodes, or keep failing the reading.
- `TcpLocalEnd::Unbound` with `LocalPort::Requested(Port(0))` is a bind to port 0, which Linux treats as bound as 0
  (freed by a disconnect or a clash), so the host counts only a requested or kernel-chosen port other than 0 as bound
  by number. The contract's wording ("a port bound by number") does not say which side port 0 falls on.
- `RemoveAuthenticationKeyAsynchronously` cannot be told apart from a plain removal on a listener from outside: both
  leave the key gone. The suite checks the key goes and no other does.
- `FlowLabelReflection` on Linux reflects the SYN's flow label in the SYN-ACK only; the contract does not say which
  segments reflect it.
