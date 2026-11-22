# Plan: the host implementation of crates/tcp-sockets

## What it is
- Crate `crates/tcp-sockets/implementation/host`, package `wcontainers-tcp-sockets-host`, type
  `HostTcpSockets`. It implements `TcpSockets` (and so `TcpConnections`, `TcpListeners`,
  `Configurable` for both setting types and `NegativeRouteAdvisable`) with real Linux TCP and MPTCP sockets of the
  machine it runs on, the exit node. It builds on every unix system: implementation.rs holds the three exported
  types (their Linux-only fields under `cfg`), linux.rs every call, request and setting on Linux, and portable.rs,
  elsewhere, what socket2 covers (connect, listen, accept, plain send and receive, shutdown, close, buffers,
  keep-alive, no-delay, urgent inline, address and port sharing, the waits) with a counted refusal for the rest.
  The owner's linter reads the Mac's build, so the types and their trait impls must exist there.
- A connection and a listener are its own types holding the socket as the shared crate's `readiness::Watched` (the
  descriptor and its registration with the host's readiness) and its family, ordered by descriptor. Every socket is
  non-blocking: every call but the two waits answers at once, `HostError::WouldBlock` when it would wait.
- The waits, `Readiness<HostTcpConnection>` and `Readiness<HostTcpListener>`, are the shared crate's `Watched::wait_until_ready`:
  one host thread per `HostTcpSockets` waits on an epoll set through `polling` and wakes the waiting calls; a wait
  first asks `poll` with no timeout, so one that need not wait finishes at its first poll; nothing is allocated per
  wait once a socket's waiter list has grown; dropping a wait withdraws it.
- Built on socket2 (`SockRef` on the socket's descriptor) for the basic calls and the settings it covers, as the
  owner ruled; the rest through rustix, linux-raw-sys (the uapi constants and structures rustix does not wrap) and
  libc's `setsockopt`/`getsockopt` as the last resort, on the same descriptor. All are MIT or Apache-2.0.
- Errors: the host's errno is the Linux errno, kept as `HostError::Errno`; `EAGAIN` becomes `WouldBlock`.
- The shared work for `crates/internet-protocol` types (its socket, port and packet settings onto socket
  options, the errno mapping, the raw option call) lives in the one implementation-side crate
  `crates/internet-protocol/implementation/linux-socket-options` (package
  `wcontainers-internet-protocol-linux-socket-options`), which the UDP, ICMP and raw tracks use too.
- What the host cannot honour answers `HostError::NotSupported` (or the errno Linux gives), counted per named
  reason in the value and warned once on standard error. Its counters are readable as abi `CounterReading`s.
- The interface has no way to build one: it gets the smallest public `new()`, and the owner is asked for a
  `StandaloneTcpSockets` trait.

## Methods on the host
- bind: `socket` as connect opens it, `setsockopt` for each `BindingSetting` (the shared crate), `bind` as below,
  `getsockname`. The bound socket is the descriptor, its family and protocol, and whether its port was bound by
  number (a requested or kernel-chosen port other than 0), which Linux cannot report afterwards. release: `close`.
- connect and listen take `TcpLocalEnd`: `Unbound` opens and binds as below; `Bound` takes the socket as it is. A
  failure before the socket starts connecting or listening gives a `Bound` input back, except a connect's
  `EADDRNOTAVAIL` on a port bound as 0 (Linux frees that port: the socket is closed, `Unbound`); a failure after it
  started (only `getsockname`) disconnects it first. An `Unbound` input always fails as `Unbound`.
- disconnect: the descriptor leaves the readiness watch, `connect` to `AF_UNSPEC` (rustix's `connect_unspec`), then
  `SO_ERROR` for the pending error; the socket comes back only when its port was bound by number (never for an
  accepted connection), else it is closed. A refused `AF_UNSPEC`, which Linux never answers, closes it, counted.
  A port is bound by number when it is requested or chosen by the program's kernel and not 0; a port the exit node
  chose is bound as 0 (`IP_LOCAL_PORT_RANGE` with port 0). portable.rs has no `AF_UNSPEC`: it aborts the
  connection (`ECONNRESET` pending while it had a peer, as Linux's disconnect sets), binds a new socket to a port
  bound by number again, and counts both (the old socket's settings are lost).
- connect: `socket(family, SOCK_STREAM, IPPROTO_TCP or IPPROTO_MPTCP)`; the owner by the thread's
  filesystem user around `socket` (EPERM when the host may not act as that user); the settings by `configure`;
  `IPV6_V6ONLY` from the bind address; `bind` for a requested or kernel-chosen port; when the exit node chooses,
  `bind` at once, for each allowed range, to a port the kernel picks under `IP_LOCAL_PORT_RANGE` in the part of the
  range inside the host's own range (Linux ignores a socket's range outside it), then to each port of the rest in
  turn, counted (`IP_BIND_ADDRESS_NO_PORT` is not used: the port is fixed before `connect`); then `connect`,
  `sendto(MSG_FASTOPEN)` for fast open with data, or `TCP_FASTOPEN_CONNECT` and `connect` for until-first-send.
  A fixed IPv6 flow label among the settings is taken from the flow label manager (`IPV6_FLOWLABEL_MGR`, shared
  with the process), with `IPV6_FLOWINFO_SEND` on, and carried in the connect address's `sin6_flowinfo`; Linux keeps
  it for good, so a later different label is refused with a counted reason.
  `EINPROGRESS` is `SynchronizeSent`, an immediate success under `TCP_REPAIR` is `EstablishedForRepair`, an
  immediate success under `TCP_FASTOPEN_CONNECT` is `DeferredUntilFirstSend`.
- listen: the same socket and bind, `TCP_SAVE_SYN` always (accept owes the SYN's IP header), the settings,
  `listen` once with the last `Backlog` among them, else the backlog given. The `Backlog` setting on a listener is
  `listen` again. A listener that listened in repair mode stays in it and takes `TCP_TIMESTAMP`.
- accept: `accept4` non-blocking; the SYN's header from `TCP_SAVED_SYN`, cut to the buffer, its IP header found
  after the link header when the listener saves that too (`TCP_SAVE_SYN` 2); its fields from that header and what
  the accepted socket reports, the arrival interface from `IP_PKTOPTIONS` or `IPV6_2292PKTOPTIONS` (the listener
  keeps `IP_PKTINFO` and `IPV6_RECVPKTINFO` on), and from the host's addresses for multipath; no interface at all
  refuses the accept, counted. A connection keeps packet information on only while it records the last header or
  reports ICMP errors (`IP_RECVERR`), since an ICMP error names its arrival interface only in it.
- send: socket2's `send_with_flags`, or its `sendmsg` when the send asks for transmit reports, with `MSG_NOSIGNAL`
  and `MSG_OOB`, `MSG_MORE`, `MSG_EOR` from the request; transmit reports as an `SO_TIMESTAMPING` control message
  (written by the shared crate's `datagram::send::Control`) with the request's record flags, the socket's own option flags (`OPT_ID`,
  `OPT_TSONLY`, `OPT_STATS`) kept on by the implementation; Linux refuses `SCM_TS_OPT_ID` on TCP, so the guest's key
  is kept by the byte offset the kernel reports for the send.
- receive: under the connection's lock (a peek at an offset sets the socket-wide `SO_PEEK_OFF` for its call),
  socket2's `recv_with_flags`, or, when timestamps or unread-byte reports are on, the shared crate's
  `receive_message` (libc's `recvmsg`, since socket2's `RecvFlags` gives no `MSG_CTRUNC`), with `MSG_PEEK` (and
  `SO_PEEK_OFF` for an offset), `MSG_OOB`; timestamps from the control messages, walked in place without allocating;
  `unread_after` from `TCP_CM_INQ`.
- take_error_queue_entry: `recvmsg(MSG_ERRQUEUE)`, `sock_extended_err` and the control messages it comes with;
  an ICMP error written whole from its IP header by the shared crate's `write_icmp_error`; a report's packet
  with `OPT_TSONLY` off, its statistics from `OPT_STATS`, wireless status from `SO_WIFI_STATUS`.
- discard_error_queue_entries: `recvmsg(MSG_ERRQUEUE)` until `EAGAIN`, each entry read as the take reads it (its
  report forgotten) but with no packet counted as cut.
- wait_until_ready (both `Readiness` impls): `poll` with no timeout, then the shared readiness (epoll); `POLLERR`
  and `POLLHUP` are the report's unconditional events and end every wait, the other bits its facts.
- shut_down: `shutdown`. close: `SO_LINGER` of zero for an abort, then the descriptor closes. stop_listening:
  the descriptor closes.
- push_queued_data: `TCP_NODELAY` set to its current value (`CorkAndNagle`) or `TCP_CORK` set to its current
  value when uncorked (`Neither`), the two option writes Linux pushes on.
- send_delayed_acknowledgement: `TCP_QUICKACK`, which sends a scheduled delayed acknowledgement and sets the
  mode.
- rearm_keep_alive_timer: `TCP_KEEPIDLE` written again with its current value, which re-times the probe from the
  last data or acknowledgement received when keep-alive is on.
- send_window_probe: the only window probe Linux offers a program is the one `TCP_REPAIR` set to
  `TCP_REPAIR_OFF` sends: repair on, then off; it needs CAP_NET_ADMIN (EPERM without it). Leaving repair mode clears
  `SO_REUSEADDR` and the repair mode, so both are put back (with the repair queue) as they were.
- give_negative_route_advice: `SO_CNX_ADVICE` set to one, which calls `dst_negative_advice`.
- set_timestamp_clock and the listener's: `TCP_TIMESTAMP` (repair mode), the unit in the value's low bit, which
  Linux 6.7 and later take as the connection's unit (`tp->tcp_usec_ts = val & 1`); an older kernel keeps
  milliseconds, so a microsecond clock that `TCP_INFO`'s `TCPI_OPT_USEC_TS` does not then show is put back and
  refused with a counted reason. set_authentication_counters and the
  listener's: `TCP_AO_INFO` with its counter flag.
- read_state and read_listener_state (linux/state.rs), one source per query: Information `TCP_INFO` (linux-raw-sys's
  structure, and the Accurate ECN fields after `tcpi_total_rto_time` from a structure of our own until it carries
  them; a shorter answer from an older kernel is counted, its missing fields zero), `tcpi_unacked` and `tcpi_sacked`
  as a connection's segment counts and a listener's accept queue and backlog; the ECN mode from `tcpi_ecn_mode` on
  Linux 7.0 and later, whose last word is bit fields, from `TCPI_OPT_ECN` before 6.18 (no Accurate ECN there), and
  on 6.18 and 6.19, whose last word is two `u16`s and which flag Classic and Accurate ECN alike, Disabled when the
  flag is clear and otherwise refused, counted (no guess); the layout by the kernel's release (`uname`), the only
  thing that tells it; TakePendingError `SO_ERROR`, mapped to the kernel interface's error names; CongestionControl
  `TCP_CONGESTION` (cut to the buffer as Linux cuts it) and `TCP_CC_INFO` (DCTCP's and BBR's forms by name, Vegas's
  by its length, none when empty); Queues `SIOCINQ`, `SIOCOUTQ`, `SIOCOUTQNSD`, `SIOCATMARK`; Tuned `SO_SNDBUF`,
  `SO_RCVBUF`, `SO_RCVLOWAT`, `TCP_MAXSEG`, `TCP_WINDOW_CLAMP`, `TCP_QUICKACK` and `tcpi_pmtu`; TakeSavedSynchronize
  the saved SYN accept kept; Timestamp `TCP_TIMESTAMP`, its unit from `TCPI_OPT_USEC_TS`; Repair `TCP_REPAIR_WINDOW`,
  `TCP_QUEUE_SEQ` under each repair queue (the queue chosen put back) and `TCP_AO_REPAIR` (none when Linux says
  ENOENT); Authentication `TCP_AO_INFO`; AuthenticationKey `TCP_AO_GET_KEYS` for the one key; Multipath `MPTCP_INFO`
  and MultipathSubflow `MPTCP_FULL_INFO` (structures of our own: neither libc nor linux-raw-sys has them), read once
  with arrays on the stack for the first subflow and the eight more the in-kernel path manager allows, and again with
  arrays on the heap as long as the count only for a subflow past those (counted); LastReceivedHeader `IP_PKTOPTIONS`
  or `IPV6_2292PKTOPTIONS` by the peer's family into room for the packet information, three integer records and four
  extension headers of the largest size (RFC 8200's hop-by-hop, destination, routing, destination), the IPv6
  extension headers recorded given in order as the header bytes (a room filled to its end counted, since Linux stops
  writing at the first record that does not fit and says nothing); Linux records no IPv4 options for TCP, so an IPv4
  reading's header bytes are always none (counted); IncomingProcessor `SO_INCOMING_CPU`. Linux's own refusals pass through (repair reads out of repair mode
  EPERM or EINVAL, `TCP_AO_INFO` without authentication ENOENT, `MPTCP_INFO` on TCP or a fallen-back connection
  EOPNOTSUPP, `TCP_TIMESTAMP` on MPTCP EOPNOTSUPP).
- Repair settings: `RepairOptions` `TCP_REPAIR_OPTIONS` (a few `tcp_repair_opt` at a time on the stack, Linux taking
  each on its own; option kinds from etherparse), `RepairWindow` `TCP_REPAIR_WINDOW`, `AuthenticationRepair`
  `TCP_AO_REPAIR` (initial sequence numbers in network order, as Linux keeps them). All need repair mode
  (CAP_NET_ADMIN).
- Settings: each `TcpSetting` and `TcpListenerSetting` variant is the one socket option its doc names
  (`SO_BUF_LOCK`, `SO_RCVLOWAT`, `SO_ATTACH_FILTER`/`SO_DETACH_FILTER`, `SO_TXREHASH`, `IP_MINTTL`/
  `IPV6_MINHOPCOUNT`, `IP_PKTINFO`-family records, `IP_RECVERR`/`IPV6_RECVERR`, the `TCP_*` options of tcp(7),
  `TCP_MD5SIG_EXT` (an IPv4 prefix of an IPv6 socket mapped), `TCP_AO_ADD_KEY` and `TCP_AO_DEL_KEY` (a key added
  again or removed when absent leaves the socket as asked; `del_async` only for the listener's asynchronous
  removal; a key's VRF as `TCP_AO_KEYF_IFINDEX` or `TCP_MD5SIG_FLAG_IFINDEX` with the index, 0 for the default VRF,
  no flag for any), `TCP_REPAIR` with `TCP_REPAIR_OFF_NO_WP` for leaving without a window probe, `TCP_AO_INFO` (read first, since Linux writes the
  requirement and ICMP choice at every call), `TCP_REPAIR*`, `TCP_FASTOPEN`, `TCP_FASTOPEN_KEY`, `TCP_SAVE_SYN`, the flow label
  manager's reflection), and an internet-protocol setting goes through the shared crate, multicast groups through
  its `set_multicast_group` (`MCAST_JOIN_GROUP`, which Linux takes on TCP sockets) and anycast through its
  `AnycastMemberships`.

## What cannot be honoured (expected; each is confirmed on the machine before it is recorded)
- An owner other than the host's user without CAP_SETUID: EPERM.
- Settings that need CAP_NET_ADMIN or CAP_NET_RAW (mark, repair, transparent addresses, priorities above six)
  answer Linux's own EPERM when the host lacks it.
- A fixed IPv6 flow label on a listener, an accepted connection or after connect (Linux fixes it only at connect).
- What Linux does not record of the last packet a TCP socket received: its fixed IP header
  (`LastReceivedHeader` gives the IPv6 extension headers recorded), and the IPv4 options of
  that packet (`IP_PKTOPTIONS` writes only `IP_PKTINFO`, `IP_TTL` and `IP_TOS`: do_ip_getsockopt in
  net/ipv4/ip_sockglue.c), so an IPv4 reading's header bytes are none, counted.
- A multipath connection that fell back to TCP has no `MPTCP_INFO` (EOPNOTSUPP), so `fell_back_to_tcp` is never
  true in a reading on Linux 7.0.
- The ECN mode of a connection using ECN on Linux 6.18 and 6.19, which report none (refused, counted).
- Fields of `ReceivedPacketFields` that Linux never reports for a SYN (its IPv4 arrival interface, its
  timestamps, and everything of a multipath SYN): taken from what the accepted socket reports, each counted under
  a named reason.

## Milestones
- T1: connect (settings, fast open), listen, accept, send, receive with their requests, shut_down, close,
  stop_listening, the error queue, the settings that need no privilege (among them `RecordsLastReceivedHeader`, the
  `IP_PKTINFO`-family and `IPV6_RECV*` records, `SaveSynchronize` from the link header, MD5 keys, the TCP
  Authentication Option's keys, options and initial counters, memberships and a fixed flow label at connect), the
  action methods; the suite for them, passing on the Linux machine over loopback.
- T2: read_state and read_listener_state with every query; repair options and windows, the authentication repair
  values and `set_authentication_counters` (whose effect only read_state shows), timestamps, multipath state; the
  privileged settings, run as root in a network namespace. Host needs, measured on Linux 7.0 (Ubuntu's
  7.0.0-1018-aws): `CONFIG_TCP_AO` (6.7 and later) for the authentication readings and repair values,
  `CONFIG_MPTCP` with `MPTCP_FULL_INFO` (6.5 and later) for the subflows, later kernels for the newest
  `mptcp_info` fields (`subflows_total`, `endp_laminar_max`, the `last_*` times; zero and counted before), 6.18 or later for the Accurate ECN fields
  of `TCP_INFO` (zero and counted before), `tcp_vegas` and `tcp_bbr` modules for their congestion information,
  CAP_NET_ADMIN for repair mode.

## The suite (crates/tcp-sockets/conformance/conformance.rs)
- Written against the traits alone: each public function takes one `TcpSockets` value, makes its own listener
  on 127.0.0.1 (or ::1) port chosen by the kernel, connects to it, accepts, and checks one behaviour by what the
  other end sees: bytes arriving in order, the end of stream after a shutdown, a reset after an abort, urgent
  data, peeking, more-to-come and cork holding data until a push, a refused connect, an error queue entry for a
  requested software transmit timestamp, and so on. Calls other than the waits answer at once, so `WouldBlock` is
  tried again until a deadline; the waits are polled by hand with a waker that counts its wakes.
- Multipath is checked on the wire: the SYN and SYN-ACK carry the MPTCP option (kind 30) exactly when both ends
  asked for multipath.
- Methods that are T2 here (authentication counters, listener state) are checked so that an answer, if one is
  given, is right (the counters set read back, the listener reads as listening with its congestion control), and
  otherwise is the not-supported refusal; a connection without authentication may not accept counters.
- ICMP errors reach a TCP socket's error queue only in SYN-SENT: a check fills a listener of backlog 0, connects
  again, captures that SYN with a raw socket and answers it with an ICMP error from a raw socket (root), all on
  loopback. An established connection takes one as its pending error (Linux's soft error) when the segment quoted is
  the next it would send: the check quotes its captured SYN one sequence number on.
- Each setting is set once and then twice on fresh pairs, and the observable outcome (what the peer sees, what
  later calls answer) is equal; settings an implementation may refuse are in these lists too, and must answer the
  same twice.
- Each setting's effect is checked where the traits or the wire can show it: a capture thread reads the host's
  packet socket on loopback (each packet seen twice, once leaving and once arriving, and counted once) and the
  checks look at headers, sequence numbers, windows, timestamps and timing; a peer made to drop everything (a
  minimum hop limit of 255) leaves segments unanswered for the timer checks; how much a connection takes or a peer
  sends while nothing is read shows the buffer settings.
- The state readings are checked by what the traits and the wire show: each end's information agrees with the
  other's, counts follow a transfer, states follow a shutdown; queues follow a cork, a dropping peer and urgent data;
  tuned values follow the settings; a clock advances as time does in its unit; a connection read in repair mode and
  written back into a new socket (sequence numbers, options, window, clock) goes on with the old peer; the
  authentication readings follow the keys and counters set, its repair values the captured SYN and SYN-ACK; the
  multipath readings agree across ends and name the subflow's addresses; the last header names loopback; settings
  given once and twice read the same tuned values, authentication state and last header.
- Memberships and owners are checked by the host's own tables (`/proc/net/igmp`, `igmp6`, `anycast6`, `tcp`, `tcp6`);
  authentication keys on the wire (option kind 29: its key identifiers and MAC length) and by what a keyed listener
  admits.
- The suite needs root, `fq` on loopback, `net.ipv4.tcp_fastopen=3`, `net.ipv6.flowlabel_consistency=0`,
  `net.ipv4.tcp_fwmark_accept=1`, `net.ipv6.conf.lo.forwarding=1` (anycast joins on loopback), `dctcp` and a local route for 127.0.0.7 with the `tcp_usec_ts` feature (its
  connections' timestamps count microseconds) and one VRF device, the only master interface (the key checks name it
  and an MD5 key of it must not hold for loopback); run-conformance.sh makes that namespace, with the iproute2 of the
  checkout's pinned nixpkgs (the machine's 6.1 does not know the feature).
- `tests!` runs each function as its own test on a value made anew, then the suites of the traits the interface
  requires, as the layout writes them: `Configurable` on a connection and on a listener (`configured_connection`,
  `configured_listener`), `NegativeRouteAdvisable` (`advised_connection`) and `Readiness` (its bundles; see the status
  file for why they are not yet the layout's one line). The implementation runs it from its own conformance.rs.
- The suite builds on every unix system and runs on Linux: only Linux has the packet socket the wire checks read.

## The suite's own choices

Numbers no standard fixes, which the suite chose; each constant of conformance.rs that has one links here (the
owner's linter wants a link for every named number). They are kept as small and as short as loopback on a loaded
machine allows, and each stays clear of the value it is checked against.
- Times. DEADLINE 10 s, the longest any wait for the network may take. SETTLE 50 ms, the most loopback takes to
  deliver. RETRY_INTERVAL 1 ms between tries of a call that would block. FILLED_AFTER 100 ms with nothing taken
  before a connection counts as full. HELD_FOR 30 ms and PUSHED_WITHIN 150 ms, both under the cork's 200 ms ceiling
  (tcp(7)), so that a push and not the ceiling sends. TIMER_SLACK 400 ms, how late a timer of about a second may fire
  under load. QUICK_WITHIN 20 ms, below the shortest delayed acknowledgement. COPY_GAP 5 ms between loopback's two
  copies of a packet. CLOCK_RUNS 50 ms of timestamp clock between setting and the segment that shows it.
  TIMESTAMP_SLACK 3 ticks: one the unit bit takes, one of rounding (the kernel rounds both the time the clock is
  set and the time a segment leaves down to the unit, so a segment's count can exceed the time measured around it,
  rounded down, by two ticks with the bit), and one for the host's clock and the suite's, which measured up to
  137 ns apart on the ARM machine (counts of 50 057 µs after 50 054 984 ns and 50 058 µs after 50 055 863 ns).
- Settings. SETTING_TIME 10 s, SHORT_SETTING_TIME 40 ms, SETTING_COUNT 4, SETTING_BYTES 64 KiB: values every
  setting of their kind takes. KEEP_ALIVE_IDLE 2 s and KEEP_ALIVE_INTERVAL 1 s, KEEP_ALIVE_PROBE_COUNTS 1 and 2 (the
  least, and one that shows each probe counted). DELAYED_ACKNOWLEDGEMENT_MAXIMUM 5 ms. RETRANSMISSION_CEILING 1 s,
  watched for RETRANSMISSIONS_WATCHED 3.2 s; LINEAR_TRANSMISSIONS 6 within 1.2 s (more than doubling timeouts send).
  USER_TIMEOUT 500 ms. FIN_WAIT_2_LASTS 1 s. ONE_SYNCHRONIZE_RETRY, the fewest SYN retries that retry at all.
  PACING_RATE 1 MiB/s over PACED_BYTES 1 MiB, at least PACED_AT_LEAST 400 ms (measured: 704 ms paced, 0.65 ms not).
  TRANSMIT_DELAY 50 ms. FAST_OPEN_QUEUE 8. BACKLOG 16. SEGMENT 1000 bytes, below loopback's segment size.
  BOUND_RATIO 4, how much less a bounded buffer lets through than the default one. WIDE_LOW_WATER 1 MiB.
  LARGER_BUFFER 4, how many times SETTING_BYTES the tuned check asks of the receive buffer: more than the send
  buffer doubled, so that each buffer reads its own. SMALL_RECEIVE_BUFFER 8 KiB, the repair check's client receive
  buffer, well below the window a connection offers by default, so that the two ends' windows differ.
  SET_PROCESSOR 3, a processor a listener is told to receive on that is not the first (Linux takes any number).
  SMALL_BACKLOG 7 with LEFT_WAITING 2 connections left unaccepted (fewer than it holds, and unlike each other);
  LARGE_BACKLOG 100 000, above any `somaxconn` (4096 by default), so that the clamp shows.
- Sizes. TRANSFER 1 MiB for the order check. ONE_SEND_OFFERED 32 MiB for one send to a peer that reads nothing,
  more than loopback's largest buffers take at once (the send buffer's and the peer's receive buffer's ceilings of
  tcp_wmem and tcp_rmem, 4 and 6 MiB by default), so that the send takes part. HEADER_ROOM 256 bytes for a SYN's headers, CUT_HEADER 3 to cut them.
  ENTRY_ROOM 64 and PACKET_ROOM 512 bytes for error queue entries and their packets. PEEK_OFFSET 2 and READ_PART 4
  bytes into MESSAGE. CAPTURE_BUFFER 16 MiB, enough for every check's traffic at once. STATISTICS_ROUNDS 10 and
  STATISTICS_HOP_LIMIT 33 (unlike any default) for the statistics check. WATCHED_CONNECTIONS 16.
- Names and values that only need to differ. Ports 41000 to 41020 (reserved to 41010) and 61000 to 61020 (above
  Linux's default local range, which ends at 60999) for the exit node's port choice. Fast open key bytes 1, 2 and 3;
  authentication key identifiers 1, 2 and 3, AUTHENTICATION_MAC_LENGTH 16 (Linux's default is 12, so the length
  given shows on the wire) and AUTHENTICATION_COUNTERS 1 to 5 and LATER_AUTHENTICATION_COUNTERS 11 to 15, each unlike
  every other. SEND_EXTENSION 1 and RECEIVE_EXTENSION 2, unlike a new connection's zero. TransmitReportKey 7. CLOCK_READING
  2^30 ms. MICROSECOND_CLOCK 127.0.0.7, the loopback address run-conformance.sh gives the `tcp_usec_ts` route. Flow labels
  0x2468A and 0x13579, below 0x80000, which Linux keeps for its own, and 0xABCDE for the setting checks. Mark 7.
  Owners 1001 and 1002 beside root. Peers: SHARING_PEERS 3 (two share, one does not), BACKLOG_OVERFLOW 3 (more than
  the smallest backlog holds). ONE_MEMBER: one socket joined, however often, is one user in the host's tables;
  ONE_WAKE: a waker woken once.

## The shared crate's own choices

Numbers of crates/internet-protocol/implementation/linux-socket-options that no standard fixes, linked from there.
- REASON_SLOTS 64: how many reasons one `Shortfalls` counts apart, more than any host implementation names; the
  rest are counted together in the last slot and warned of at every count.
- CONTROL_ROOM 1024 bytes: room for one receive's control messages (timestamps, an extended error with its
  offender, packet information and a transmit report's TCP statistics); a cut is seen by `MSG_CTRUNC` and counted by the caller.
