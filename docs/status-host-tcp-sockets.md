# Status: host-tcp-sockets

Plan: docs/plan-host-tcp-sockets.md. Milestones there: T1 (connections and listeners, the suite), T2 (state
readings, repair, authentication, multipath state, the privileged settings).

## For the UDP, ICMP and raw tracks
- New, run 6: `receive_message` (TCP's only) no longer allocates: `ReceivedMessage` has `length`, `flags` and
  `control` (the bytes), and `messages()` walks them in place as level, type and data (`ControlMessages`, which
  takes any control bytes the kernel wrote). UDP track: `Ancillary::parse` could take its messages from
  `ControlMessages::new` so the crate has one CMSG walk. TCP now sends its transmit report requests through
  socket2's `sendmsg` with the UDP track's `datagram::send::Control`.
- TCP uses the shared `readiness` for both waits; since 12b98f7f (UDP track) it arms a one-shot registration only
  while a call waits, and `open_socket` makes TCP's sockets with socket2 (9ad7b201).
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
- New, run 8: `Shortfalls` counts without a lock (a fixed array of 64 slots, each a reason taken once and an atomic
  count; reasons past 64 are counted under one named overflow reason); `readings()` is still sorted by reason.
  TCP now takes multicast groups through `datagram::memberships::set_multicast_group` and anycast through
  `AnycastMemberships` (Linux takes `MCAST_JOIN_GROUP` and `MCAST_MSFILTER` on a TCP socket, measured; only the
  old `IP_ADD_MEMBERSHIP` is EPROTO there). `set_ip_socket_setting` and `set_packet_setting` still refuse
  memberships and a fixed flow label for whoever calls them without handling those first.

## Done in run 9 (2026-11-22): the independent check of run 8 answered
- Memberships by their effect: multicast_groups_are_joined_and_left reads `/proc/net/igmp` and `/proc/net/igmp6`
  (groups 224.0.0.123 and ff02::123, which no other check joins): none before, one user after a join and after a
  second, none after a leave, a second leave and the close. New anycast_addresses_are_joined_and_left does the same
  with `/proc/net/anycast6` (fd00::123): the namespace now has `net.ipv6.conf.lo.forwarding=1`, since Linux lets a
  host (not a router) join only anycast addresses its interface's prefixes hold.
- The owner: new connect_and_listen_make_sockets_of_their_owner connects as uid 1001 and listens as uid 1002 over
  IPv4 and IPv6 and finds each in `/proc/net/tcp` or `tcp6` (by its ports) with that uid.
- The TCP Authentication Option's settings are honoured (they need no privilege, as the checker measured):
  `AddAuthenticationKey` is `TCP_AO_ADD_KEY` (peer prefix, an IPv4 one of an IPv6 socket mapped, interface, both
  identifiers, algorithm, MAC length, options excluded or not, the key's use as current or requested next); Linux
  answers EEXIST for a key whose identifiers a held key has, so when `TCP_AO_GET_KEYS` shows the held key is the one
  asked for, that is a success (its use applied through `TCP_AO_INFO`). `RemoveAuthenticationKey` is
  `TCP_AO_DEL_KEY` (with the keys to use next and removal when unused); an absent key's removal is a success, the
  keys to use next still applied. `AuthenticationOptions` and `InitialAuthenticationCounters` are `TCP_AO_INFO`,
  read first, since Linux writes the requirement and the ICMP choice at every call. `AuthenticationRepair` and
  `set_authentication_counters` stay T2 (counted refusals).
  New authentication_keys_admit_only_connections_signed_with_them (11 cases on the wire, option kind 29: its key
  identifiers and its length, 4 + the MAC length 16 the check gives, unlike Linux's default 12): matching keys
  admit, every segment both ways signed; a key added twice; the key asked to be current signs the SYN; a key
  removed with another made current; another secret, no key, and options covered on one end only are refused; a
  listener's key removed twice admits plain clients; a listener requiring authentication refuses plain clients
  until it no longer requires it. New authentication_options_choose_the_keys_in_use: options set twice on an
  established connection switch the key its data is signed with. The listener settings list gains the four
  settings (for a peer other than the clients), all required to succeed; the refusable list a key and counters on
  established connections (Linux: EISCONN both times).
- A listener's `UnreadByteReports` reaches the connections it accepts (accept copies the flag, as Linux copies
  `TCP_INQ`); checked by new listener_unread_byte_reports_reach_the_connections_it_accepts.
- keep_alive_probes_end_an_unanswered_connection runs with 1 and with 2 probes: as many probes as asked, an
  interval apart, then the reset an interval after the last.
- accept_names_where_the_synchronize_arrived also accepts multipath connections (their arrival interface comes from
  the host's addresses), to 127.0.0.2, which loopback holds only by its prefix: the exact-address-only copy now
  fails. Linux gives an accepted multipath socket mark 0 (measured), so the mark is checked for TCP only.
- Shared `Shortfalls`: a reason counted in the overflow slot is warned of at every count, since that slot cannot
  tell its reasons apart.
- Note: commit d60b9609 ("refactored everything to fit the new reviewable rust format", not this track's) took
  part of this run's uncommitted edits of implementation.rs, conformance.rs and the shared host.rs with it; the
  rest is in 198547b0.

## Done in run 8 (2026-11-21): the independent check of run 7 answered
- The two flaky checks. listener_backlog_bounds_the_accept_queue: two of 60 full runs still failed after a first
  change (accepting at once), both with two connections accepted: a SYN that came while the queue was momentarily
  empty, or two handshakes finishing side by side, got in. The check now connects one client, waits (through the
  listener's wait) until it is queued, then connects the others (a full queue drops their SYNs), settles, accepts
  one, and watches for 30 ms with a last try after the period however late the thread ran.
  pacing_rate_spreads_what_is_sent: 1 MiB at 1 MiB/s with a bar of 400 ms (measured, ARM machine, load 2: paced
  704 ms, lifted 0.65 ms; the old 512 KiB took 0.2 s against a bar of 100 ms).
- A fixed IPv6 flow label is honoured at connect: the socket takes the label from the flow label manager
  (`IPV6_FLOWLABEL_MGR`, shared with this process's sockets), `IPV6_FLOWINFO_SEND` on, and connects to an address
  whose `sin6_flowinfo` carries it (std keeps that field as given, so it is put in network order). Measured on the
  ARM machine: the manager needs CAP_NET_ADMIN (uid 1000 in a root network namespace: EPERM; root: taken), so
  Linux's EPERM is what an unprivileged host answers. Linux keeps a connection's label for good, so the same label
  again is a success and another, or automatic or none after it, or any fixed label on a listener or accepted
  connection, is the counted refusal `FLOW_LABEL_FIXED_AT_CONNECT`. Checked by fixed_flow_label_marks_every_segment
  (every segment the client sends, the SYN first, carries it; the same label again changes nothing; another is
  refused with the label kept, or carried).
- Multicast and anycast memberships go to Linux (see above); multicast_groups_are_joined_and_left joins and leaves
  an IPv4 and an IPv6 group on the loopback interface twice each, all taken, and the connection still carries bytes.
- MD5 signature keys (`Md5Key`) are honoured through `TCP_MD5SIG_EXT` (prefix and interface; an IPv4 prefix of an
  IPv6 socket mapped; removing an absent key is a success, Linux's ENOENT otherwise). Checked by
  md5_keys_admit_only_connections_signed_with_them (a keyed listener takes clients signed with its key, a key set
  again replacing the last, and every segment both ways carries kind 19; it answers no SYN with another key, no key,
  or a key removed; removed twice, the listener takes plain clients). The TCP Authentication Option stays in T2:
  see Departures.
- The window probe's check now chooses the receive repair queue first and, after the probe, sends (which in repair
  mode fills the queue chosen) and peeks the bytes back: refused if the queue was lost.
- no_delay_sends_small_segments_while_others_are_unacknowledged has a fourth case: a push past cork and Nagle first,
  then Nagle must still hold the second small segment.
- `Shortfalls` counts without a lock (see above), so accepts on many threads no longer meet at one mutex.
- `NotSentLowWater(None)` is written as `u32::MAX` (no bar); 0 would mean the host's sysctl, which is `u32::MAX` in
  the suite's namespace too, so no check tells them apart: under For the owner.
- `RecordsLastReceivedHeader` kept by accepted connections stays unobservable before T2's read_state.

## Done in run 7 (2026-11-21): the independent check of run 6 answered
- `RecordsLastReceivedHeader` is honoured: Requested turns on `IP_PKTINFO`, `IP_RECVTTL`, `IP_RECVTOS` (whose records
  `IP_PKTOPTIONS` gives a TCP socket) and, on IPv6 sockets, `IPV6_RECVPKTINFO`, `IPV6_RECVHOPLIMIT`,
  `IPV6_RECVTCLASS`, `IPV6_RECVHOPOPTS`, `IPV6_RECVDSTOPTS`, `IPV6_RECVRTHDR` (whose records `IPV6_2292PKTOPTIONS`
  gives from the last packet that came in order); NotRequested turns them off. Measured as uid 1000 on the ARM
  machine: each is taken without privilege. A listener with it keeps it for the connections it accepts (accept
  no longer turns off their packet information then). The suite takes both values at connect and on established
  IPv4 and IPv6 connections, and on IPv4 and IPv6 listeners (listener_settings_set_twice_equal_set_once demands
  success). Its effect shows only through `TcpQuery::LastReceivedHeader`, which is read_state (T2).
- `SaveSynchronize(FromLinkHeader)` is honoured: the listener's `TCP_SAVE_SYN` is 2 (measured as uid 1000: taken
  without privilege; on loopback the saved headers start with a 14-byte Ethernet header). Accept finds where the
  IP header starts (the first place an IP header from the peer to the local address holds a TCP header between
  their ports that ends the headers, by etherparse's lax slices; counted when none is found) and gives the IP
  header from there whichever way the SYN was saved; `TakeSavedSynchronize` then gives the link header too.
  saved_synchronize_is_taken_once covers link, link then network, link then off; it also checks that accept's
  header is the SYN's IPv4 header from 127.0.0.1 to 127.0.0.1.
- Reading a saved SYN again into 64 KiB when it is longer than the first room is a counted slow path
  (`LONG_SAVED_SYNCHRONIZE`); the first room now holds an Ethernet header too.
- Peeks at an offset no longer disturb other receives: every receive holds the connection's `receiving` lock, since
  `SO_PEEK_OFF` is the socket's for the call (an uncontended lock beside a system call; Linux takes the socket's
  own lock for each receive anyway). Checked by the implementation's own peeks_from_two_threads_each_start_where_asked
  (two threads, 20 000 peeks each, one at offset 5 and one at 0, every one exact); it fails without the lock.
- New wait checks: wait_for_urgent_data_finishes_when_it_arrives (exceptional_condition, `POLLPRI`) and
  wait_for_the_peers_shutdown_finishes_when_it_comes (peer_shut_down_writing, `POLLRDHUP`, with no hang-up); each
  fails when the shared readiness does not register its bit (the checker's broken copy and one for `RDHUP`).
- An IPv4 SYN's arrival interface is now the one Linux keeps for the connection, read from `IP_PKTOPTIONS` (the
  listener has `IP_PKTINFO` on; measured: an accepted socket reports interface 1 for 127.0.0.1, and a dual-stack
  listener's IPv4-mapped connection too): Linux records there the interface of the segment that completed the
  handshake (`tcp_v4_syn_recv_sock`: `mc_index = inet_iif(skb)`), the SYN's own only for fast open, and that is
  counted (`ARRIVAL_INTERFACE_OF_THE_HANDSHAKE`). The host's addresses (`getifaddrs`, cached until a netlink notice
  of an address change) now serve only multipath connections, which have no packet options. No check on one host
  tells the two apart: a SYN to an address a dummy interface holds is reported on that interface by both
  (measured, interface 2).
- Broken copies: 106 (mutants.sh); the new ones and the three mended ones are under Measurements.

## Done in run 6 (2026-11-21): the independent check of run 5 answered
- Multipath is checked on the wire (multipath_is_negotiated_when_both_ends_ask, which replaces
  multipath_connections_carry_bytes): the capture marks segments with the MPTCP option (kind 30, which etherparse
  reports as an unknown option); a multipath client and listener both offer it in the SYN and SYN-ACK, a plain
  client to a multipath listener neither, a multipath client to a plain listener only in the SYN; bytes cross each.
- shutdown_of_both_directions_ends_both now also reads the client's own end of stream (0 at once, as Linux gives).
- The reporting send goes through socket2's `sendmsg` (`MsgHdr::with_buffers`, `with_control`), its control message
  written by the shared `datagram::send::Control`. The receive with control messages stays libc's `recvmsg` in the
  shared `receive_message`: socket2 0.6.5's `RecvFlags` offers only `is_truncated`, `is_end_of_record`,
  `is_out_of_band`, `is_confirm` and `is_dontroute` (src/lib.rs:387, src/sys/unix.rs:623-659), no `MSG_CTRUNC`, and
  both the error queue and now the timestamped receive count a cut (`CUT_CONTROL`). The control messages are walked
  in place (no `Vec`); checked by a_receive_with_control_messages_allocates_nothing (100 timestamped receives with
  unread-byte reports, none allocating).
- set_timestamp_clock: Linux cannot change a connection's clock unit, which the route gives (`tcp_usec_ts`;
  measured: a connection over a route with the feature advances 50 056 in 50 ms, one without 52, and writing
  `TCP_TIMESTAMP` with either low bit changes neither). The implementation reads the unit from `TCP_INFO`'s
  `TCPI_OPT_USEC_TS` and refuses the other with a counted reason. The suite's namespace adds a local route for
  127.0.0.7 with the feature (iproute2 7.1 from the checkout's pinned nixpkgs; the machine's 6.1 lacks it):
  timestamp_clock_set_in_repair_mode_stamps_segments checks each unit on its own route (at least 50 ms or 50 000 µs
  past the reading after 50 ms, at most the time it ran), and
  timestamp_clock_in_the_other_unit_counts_in_it_or_is_refused that the other unit counts in it or is refused.
- saved_synchronize_is_taken_once covers multipath listeners: a save setting there must either be refused (Linux:
  ENOPROTOOPT, errno 92, for on and off) or leave the SYN to be taken.
- Discarding error queue entries reads each as the take does but counts no cut packet (`next_error_queue_entry`
  gives whether one was cut; only the take counts it); checked by discarding_cuts_no_packet.
- New checks for the methods that had none: authentication_counters_are_set_only_where_authentication_is (a
  connection without authentication may not take counters: Linux says EISCONN; a listener's, if set, read back,
  as Linux does, else NotSupported) and listener_state_reads_a_listener (Information reads Listen and
  CongestionControl the one given, else NotSupported). Both T2 methods still answer the counted refusal.
- Broken copies: 99 now (mutants.sh); the new ones and their results are under Measurements.

## Done in run 5 (2026-11-21): T1 complete
- The suite finds the loopback interface by its `IFF_LOOPBACK` flag (nix's `getifaddrs`) instead of naming it
  "lo": the literal check finds nothing in this track's files now.
- Socket creation is socket2's (the shared `open_socket`, moved by the UDP track in 9ad7b201). The one rustix
  `socket_with` left in the implementation opens the netlink socket that hears of address changes, not a TCP socket.
- Independent check, see Measurements.

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
- Suite `crates/tcp-sockets/conformance`: 102 checks against the traits alone since run 9 (and four of the
  implementation's own: the wait and receive allocation checks, discarding_cuts_no_packet and the two-thread peeks;
  106 tests in one binary), each on a value made anew. The body
  is `cfg(target_os = "linux")`: the checks watch the wire. A capture thread reads the host's packet socket
  (`AF_PACKET`, every loopback packet seen twice, counted once) filtered to one listener's port, and the checks
  read headers, sequence numbers, windows, TCP timestamps and times from it; since run 9 the host's tables in /proc too. Timer checks make the peer drop
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
  - The host's interface addresses are read with nix's `getifaddrs` instead of unsafe libc, cached until a netlink
    notice of an address change; since run 7 only multipath accepts use them (see run 7).
- wait_for_events: wait_for_events_finishes_when_data_arrives, wait_for_events_answers_at_once_when_ready,
  dropping_a_wait_cancels_it, many_waits_each_wake_once, wait_for_room_finishes_when_the_peer_reads,
  wait_for_urgent_data_finishes_when_it_arrives, wait_for_the_peers_shutdown_finishes_when_it_comes,
  wait_ends_with_the_errors_always_reported. wait_for_listener_events: listener_wait_finishes_when_a_connection_comes,
  dropping_a_listener_wait_cancels_it.
- Which check fails when a method or setting breaks, each one run as a broken copy by mutants.sh (results under
  Measurements): connect, listen, accept: every check; their owner: connect_and_listen_make_sockets_of_their_owner;
  their protocol: multipath_is_negotiated_when_both_ends_ask. send and receive: bytes_arrive_in_order_both_ways and the
  request checks (urgent_*, peek_*, more_to_come_*, record_end_starts_a_new_segment, unread_byte_reports_*,
  receive_software_timestamps_*, the transmit report checks). shut_down: the three shutdown checks (ReadAndWrite as Write fails
  shutdown_of_both_directions_ends_both). close:
  orderly_close_*, abort_resets_the_peer. stop_listening: stopped_listener_refuses_connections.
  take_error_queue_entry and discard_error_queue_entries: the report, ICMP and discarding checks.
  push_queued_data: more_to_come_holds_data_until_pushed, push_ignoring_neither_*. send_delayed_acknowledgement:
  delayed_acknowledgement_is_sent_on_demand. rearm_keep_alive_timer: rearming_the_keep_alive_timer_sends_a_due_probe.
  send_window_probe: window_probe_is_answered_and_changes_no_setting. set_timestamp_clock:
  timestamp_clock_set_in_repair_mode_stamps_segments and timestamp_clock_in_the_other_unit_counts_in_it_or_is_refused
  (the listener's: only its refusal, see For the owner). set_authentication_counters and the listener's:
  authentication_counters_are_set_only_where_authentication_is. read_listener_state: listener_state_reads_a_listener.
  read_state: saved_synchronize_is_taken_once (TakeSavedSynchronize only; the rest is T2). receive's lock: the
  implementation's own peeks_from_two_threads_each_start_where_asked.
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
  multicast loopback and all-groups (no effect on TCP traffic), packet priority (not on the wire), unicast
  interface, route scope, source preferences, bound interface, incoming processor (one interface, no read-back
  before T2), IPv4 options and IPv6 extension headers, the IPv6 variants of traffic class and path MTU discovery,
  `RecordsLastReceivedHeader` (shows only in `TcpQuery::LastReceivedHeader`, T2), `InitialAuthenticationCounters`
  (shows only in read_state, T2) and the authentication options' ICMP choice (would need an ICMP error to an
  authenticated connection; not attempted). The mark is checked by accept_names_where_the_synchronize_arrived.
  `SaveSynchronize` with all three values is checked through `TakeSavedSynchronize` (saved_synchronize_is_taken_once).
  T2 settings (the authentication repair values, repair options and windows) are not in T1.

## Measurements
- Run 8, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-21 23:17 to 23:21, load 2.8 to 5.3: clippy with
  warnings denied clean (tcp host, suite, shared crate); 60 full runs in a row at `RUST_TEST_THREADS=12`, each 101
  passed, 0 failed, 3.44 to 3.51 s (log repeat-run8b.log in scratch). Before the backlog check's second change, 60
  runs (23:13 to 23:17, load 1.6 to 3.7) failed 2 times, both in it with two connections accepted (repeat-run8.log).
  After the literal fix (23:24, load 1.8): three runs, each 101 passed; the literal check finds nothing in this
  track's files (it found `PrefixLength(32)` and a flow label equal to the route-resolver suite's `LABEL`, both
  changed).
- Run 8 broken copies (mutants.sh, 23:21 to 23:22, load 4 to 5), each failed in the check named:
  window-probe-repair-queue-lost (window_probe_is_answered_and_changes_no_setting), push-leaves-no-delay-on
  (no_delay_sends_small_segments_while_others_are_unacknowledged), md5-key-ignored and md5-absent-removal-refused
  (md5_keys_admit_only_connections_signed_with_them), fixed-flow-label-not-held and fixed-flow-label-forgotten
  (fixed_flow_label_marks_every_segment), multicast-refused (multicast_groups_are_joined_and_left). 113 copies now.
- Run 8 independent check from a clean clone of main at 18d3ef97 (job host-tcp-sockets-clean, 23:22 to 23:23, load
  2.6 to 4.7; log clean-check-run8.log in scratch): build folder emptied, clippy with warnings denied clean (10 s),
  101 passed in 3.47 s; `cargo tree` of tcp-sockets, internet-protocol, configurable, negative-route-advice, kernel,
  abi and the suite names only interface and conformance crates (abi, configurable, executor, internet-protocol,
  kernel, negative-route-advice, tcp-sockets and the two conformance crates); `git diff ae9c53ca HEAD` of the .rs
  files beside the tcp-sockets, internet-protocol, kernel, configurable and negative-route-advice interfaces is
  empty (abi's and system's changed only in other tracks' commits 564626c0, fd65d351, 4ee5def7). Again from a clean
  clone at e040df6a (23:25, load 2.3 to 4.5): clippy clean (9.2 s), 101 passed in 3.49 s.
- Probe of Linux's answers on a TCP socket (scratch tools/probe_memberships.py, ARM machine 23:05, load 2): as root
  and as uid 1000, `MCAST_JOIN_GROUP` and `MCAST_LEAVE_GROUP` on the loopback interface taken for IPv4 and IPv6, on
  no interface ENODEV; anycast join EADDRNOTAVAIL as root, EPERM as uid 1000; the flow label manager taken as root
  (an exclusive label a second time EPERM, a process-shared one taken again), EPERM as uid 1000; on an IPv4 socket
  ENOPROTOOPT.
- Run 7, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-21 22:33, load 1.9 to 2.7: clippy with warnings
  denied clean (tcp host, suite); 98 passed, 0 failed, three runs, each 3.4 s (94 suite checks and four of the
  implementation's own).
- Run 7 independent check from a clean clone of main at e7bf5cbd (job host-tcp-sockets-clean, 22:34 to 22:35, load
  1.8 to 4.5; log clean-check-run7.log in scratch): build folder emptied, clippy with warnings denied clean (9.5 s),
  98 passed in 3.46 s; `cargo tree` of tcp-sockets, internet-protocol, configurable, negative-route-advice, kernel,
  abi and the suite shows no implementation crate; `git diff ae9c53ca e7bf5cbd` of the .rs files beside the
  tcp-sockets, internet-protocol, kernel, configurable and negative-route-advice interfaces is empty (abi's and
  system's changed only in other tracks' commits 564626c0, fd65d351, 4ee5def7). Literal check: no finding in this
  track's files.
- Run 7 broken copies (mutants.sh, 22:28 to 22:30, load 2 to 5; log mutants-run-7.log in scratch), each failed:
  multipath-save-syn-refusal-skipped, link-header-kept-as-network, link-header-not-saved,
  link-header-given-by-accept, saved-synchronize-not-kept, saved-synchronize-off-ignored,
  saved-synchronize-taken-twice (saved_synchronize_is_taken_once); records-refused (both connection setting checks
  and the listener's); receive-unlocked (peeks_from_two_threads_each_start_where_asked);
  urgent-wait-unregistered (wait_for_urgent_data_finishes_when_it_arrives); peer-shutdown-unregistered
  (wait_for_the_peers_shutdown_finishes_when_it_comes); quoted-addresses-swapped
  (icmp_error_reaches_a_synchronizing_connection_over_ipv4; its text follows another track's uncommitted edit of
  icmp_error.rs). Not caught, as said above: `RecordsLastReceivedHeader` doing nothing (no read before T2) and the
  IPv4 arrival interface taken from the host's addresses instead.
- Run 6, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-21 21:59 to 22:04, load 4 to 16: clippy with
  warnings denied clean (tcp host, suite, shared crate); 95 passed, 0 failed, five runs, each 3.4 s (92 suite checks
  and three of the implementation's own). Literal check (22:03): no finding in this track's files.
- Run 6 broken copies (mutants.sh, 22:00 to 22:02, load 5 to 9; log mutants-run-6.log in scratch), each failed:
  clock-always-refused (both clock checks, the window probe check), clock-unit-ignored
  (timestamp_clock_in_the_other_unit_counts_in_it_or_is_refused), clock-microseconds-as-milliseconds
  (timestamp_clock_set_in_repair_mode_stamps_segments), multipath-is-tcp (multipath_is_negotiated_when_both_ends_ask,
  saved_synchronize_is_taken_once), tcp-is-multipath (every test: a multipath listener refuses `TCP_SAVE_SYN`),
  shutdown-both-is-write (shutdown_of_both_directions_ends_both), multipath-save-syn-refusal-skipped
  (saved_synchronize_is_taken_once), discard-counts-cuts (discarding_cuts_no_packet), authentication-counters-fake
  and listener-authentication-counters-fake (authentication_counters_are_set_only_where_authentication_is),
  listener-state-fake (listener_state_reads_a_listener), report-packet-dropped and statistics-dropped (their report
  checks, after the error queue change).
- Run 6 independent check from a clean clone of main at ff302f08 (job host-tcp-sockets-clean, 22:01 to 22:02, load 5
  to 6; log clean-check-run6.log in scratch): build folder emptied, clippy clean (9.3 s), 95 passed in 3.44 s;
  `cargo tree` of tcp-sockets, internet-protocol, configurable, negative-route-advice, kernel, abi and the suite
  shows no implementation crate; `git diff ae9c53ca HEAD` of every .rs file beside the tcp-sockets,
  internet-protocol, kernel, configurable, negative-route-advice and abi interfaces is empty; system's interface.rs
  differs by two lines from other tracks' commits.
- Run 5, ARM machine (Graviton4, Linux 7.0.0-1018-aws), 2026-11-21 21:28, load 14 to 15: clippy with warnings
  denied clean, 90 passed in 3.43 s. Literal check (21:29): no finding in this track's files.
- Run 5 independent check from a clean clone of main at 02f9c69c (job host-tcp-sockets-clean, 21:30 to 21:32, load
  11 to 31; log clean-check-run5.log in scratch): build folder emptied, clippy clean (9.6 s), 90 passed in 3.42 s;
  `cargo tree` of tcp-sockets, internet-protocol, configurable, negative-route-advice, kernel, abi and the suite
  shows no implementation crate; four broken copies each failed: abort-closes-in-order (abort_resets_the_peer,
  wait_ends_with_the_errors_always_reported), push-does-nothing (more_to_come_holds_data_until_pushed,
  push_ignoring_neither_keeps_the_cork), wait-never-finishes (seven wait checks and the allocation check),
  readiness-wakes-nothing (four wait checks). `git diff ae9c53ca HEAD` of every interface.rs and data file this
  track uses (tcp-sockets, internet-protocol, kernel, configurable, negative-route-advice) is empty; abi's and
  system's interface.rs changed only in other tracks' commits 564626c0, fd65d351, 4ee5def7.
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
  copy, 130 copies; names pick some). Whether every copy's text is still there once: scratch
  tools/check_mutants.sh <checkout> (on the Mac, no build).
- Lint for Linux on the ARM machine: `cloud/arm-run <checkout> --name host-tcp-sockets -- 'cargo clippy -j
  $ARM_JOBS --no-deps --tests -p wcontainers-tcp-sockets-host -p wcontainers-tcp-sockets-conformance --
  -D warnings'`, and the same on the Mac without arm-run (with `CARGO_TARGET_DIR=$PWD/target/host-tcp-sockets
  nix develop -c`; there both crates build empty).
- Files copied from the Mac can carry times older than the ARM machine's last build, so cargo there may skip
  them: `touch` the changed files in the command when a result looks stale.
- The literal check (run 4, on the ARM machine in a copy given its own `git init`, since arm-run copies no .git)
  found eight findings in this track's files that earlier runs had not seen; seven are fixed (named constants,
  a different test flow label, alignment by `cmsghdr`'s type, the fast open sysctl as its two named bits); the
  eighth, the name "lo", went in run 5. Command: `rm -rf target/lit && mkdir -p target/lit && tar
  --exclude=./target -cf - . | tar -xf - -C target/lit && cd target/lit && git init -q && git add -A && cargo run
  -q -p wcontainers-literals -- check`, then look for tcp-sockets in its output.
- Cargo.lock in this shared checkout: commit this track's lines through a private index (`GIT_INDEX_FILE` set to
  a scratch file, `git read-tree HEAD`, `git apply --cached` of a patch of those lines, `git add` of own files,
  `git commit`), then `git add` own files in the real index; other tracks' staged and unstaged changes stay.
- The shared crate linux-socket-options was being reorganized by another track during run 5 (its
  implementation.rs staged as linux.rs): if this crate stops building on it, wait and try again.

## Next (T2)
- read_state and read_listener_state (every query), the authentication repair values and
  set_authentication_counters, repair options and windows: counted refusals now.
- With read_state, give the settings listed above as unobservable a check of their effect, first
  `RecordsLastReceivedHeader` through `TcpQuery::LastReceivedHeader` and `InitialAuthenticationCounters` through the
  authentication status.
- authentication_counters_are_set_only_where_authentication_is and listener_state_reads_a_listener still pass on
  any NotSupported answer (conformance.rs, `assert_eq!(error, HostError::NotSupported)`): once T2 answers them, make
  both demand the real answer on Linux.

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

- Run 8 and 9: the plan put all authentication in T2, while the milestone asks for every setting of settings.rs
  that needs no privilege. MD5 keys (run 8) and the TCP Authentication Option's keys, options and initial counters
  (run 9) moved to T1, plan corrected; the authentication repair values (repair mode needs CAP_NET_ADMIN) and the
  method `set_authentication_counters` stay T2.

## Dependencies added
- Run 5: nix 0.31.4 in the suite too (Linux only, `net` feature: the loopback interface by its flag): MIT.
- Run 4: socket2 0.6.5 with feature `all` (the basic calls): MIT OR Apache-2.0. polling 3.11.0 comes through the
  shared crate (the UDP track added it): Apache-2.0 OR MIT.
- rustix 1.1.6 (also in the suite, for its raw and packet sockets): Apache-2.0 WITH LLVM-exception OR Apache-2.0
  OR MIT. netlink-packet-core 0.9.0 (the statistics' attributes): MIT. linux-raw-sys 0.12.1: the same. libc
  0.2.191 (also in the suite since run 3, for BPF codes): MIT OR Apache-2.0. etherparse 0.21.0: MIT OR
  Apache-2.0. Run 3: nix 0.31.4 with only its `net` feature (interface addresses): MIT; it brings memoffset
  0.9.1: MIT.

## For the owner
- `TcpSetting::NotSentLowWater(None)` does not say whether None is no bar or the host's default: Linux reads 0 as
  its sysctl `net.ipv4.tcp_notsent_lowat` and `u32::MAX` as no bar. Example: with the sysctl at 16384, None as 0
  bars a connection at 16 KiB unsent, as `u32::MAX` not at all. The implementation writes `u32::MAX`.
- A fixed IPv6 flow label (`FlowLabelChoice::Label`) can be asked of a listener or an established connection, but
  Linux fixes a TCP socket's label only as it connects (`np->flow_label` from the connect address). Example: a
  connection made without a label and then given a fixed label is answered NotSupported.
- `set_timestamp_clock` asks for a unit, but Linux fixes a connection's clock unit from its route (the
  `tcp_usec_ts` route feature) and `TCP_TIMESTAMP` only sets the value. Example: Microseconds on a connection over
  127.0.0.1 is answered NotSupported. And "its low bit given as the unit" is not what Linux 7.0 reports: on a
  microsecond connection, writing 0x40000000 reads back 0x40000000 (low bit 0), on a millisecond one writing
  0x40000001 reads 0x40000003.
- Building one now starts a host thread and an epoll set, so it can fail: the suggested trait is
  `pub trait StandaloneTcpSockets: TcpSockets + Sized { fn new() -> Result<Self, HostError>; }` (the crate has
  `HostTcpSockets::new() -> Result<Self, HostError>`).
- `PollEvents` has `message_available` (`POLLMSG`), which Linux never reports for a socket: a wait asking only
  for it finishes only on an error, hang-up or closed descriptor, as `poll` does on Linux. Not a fault, but the
  contract does not say it.
- `owner: UserId` is undefined when the exit node runs as one user: a guest's root (`UserId(0)`) connecting
  through a host process running as uid 1000 can only be answered EPERM (the host cannot make a socket owned
  by another user without CAP_SETUID), so on an unprivileged host every guest connect fails. The suite
  therefore runs as root.
- `TcpAccepted::synchronize` asks for fields Linux never reports for a SYN: for IPv4 the arrival interface (Linux
  keeps that of the segment that completed the handshake, given in its place and counted), and the SYN's own mark,
  priority and timestamps (it gives the accepted socket's mark and priority and no timestamps).
- Linux gives an accepted multipath socket mark 0 whatever its SYN's mark (`tcp_fwmark_accept` marks only the
  subflow), so `TcpAccepted::synchronize.mark` of a multipath accept is the socket's 0 (counted with the other
  fields of the socket). Example: a multipath client with mark 7 to 127.0.0.2 is accepted with mark 0.
- A multipath accept has no SYN to give: Linux has no `TCP_SAVED_SYN` (nor packet options) on MPTCP sockets, so
  `synchronize_header` is empty and `synchronize.length` is 0, counted (`NO_SAVED_SYNCHRONIZE`), with the interface
  taken from the host's addresses (counted). Example: a multipath client to a multipath listener on 127.0.0.1 is
  accepted with length 0. The contract has no way to say the SYN is unknown (an `Option` would).
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

