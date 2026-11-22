# Plan: the host implementation of crates/tcp-sockets

## What it is
- Crate `crates/tcp-sockets/implementation/host`, package `wcontainers-tcp-sockets-host`, type
  `HostTcpSockets`. It implements `TcpSockets` (and so `TcpConnections`, `TcpListeners`,
  `Configurable` for both setting types and `NegativeRouteAdvice`) with real Linux TCP and MPTCP sockets of the
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
- connect: `socket(family, SOCK_STREAM, IPPROTO_TCP or IPPROTO_MPTCP)`; the owner by the thread's
  filesystem user around `socket` (EPERM when the host may not act as that user); the settings by `configure`;
  `IPV6_V6ONLY` from the bind address; `bind` for a requested or kernel-chosen port, `IP_BIND_ADDRESS_NO_PORT`
  and `IP_LOCAL_PORT_RANGE` (once per range between reserved ports) when the exit node chooses; then `connect`,
  `sendto(MSG_FASTOPEN)` for fast open with data, or `TCP_FASTOPEN_CONNECT` and `connect` for until-first-send.
  A fixed IPv6 flow label among the settings is taken from the flow label manager (`IPV6_FLOWLABEL_MGR`, shared
  with the process), with `IPV6_FLOWINFO_SEND` on, and carried in the connect address's `sin6_flowinfo`; Linux keeps
  it for good, so a later different label is refused with a counted reason.
  `EINPROGRESS` is `SynchronizeSent`, an immediate success under `TCP_REPAIR` is `EstablishedForRepair`, an
  immediate success under `TCP_FASTOPEN_CONNECT` is `DeferredUntilFirstSend`.
- listen: the same socket and bind, `TCP_SAVE_SYN` always (accept owes the SYN's IP header), the settings,
  `listen(backlog)`. The `Backlog` setting is `listen` again.
- accept: `accept4` non-blocking; the SYN's header from `TCP_SAVED_SYN`, cut to the buffer, its IP header found
  after the link header when the listener saves that too (`TCP_SAVE_SYN` 2); its fields from that header and what
  the accepted socket reports, the arrival interface from `IP_PKTOPTIONS` or `IPV6_2292PKTOPTIONS` (the listener
  keeps `IP_PKTINFO` and `IPV6_RECVPKTINFO` on), and from the host's addresses for multipath.
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
- wait_until_ready (both `Readiness` impls): `poll` with no timeout, then the shared readiness (epoll).
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
- set_timestamp_clock and the listener's: `TCP_TIMESTAMP` (repair mode). The unit is the one the route gave the
  socket (`tcp_usec_ts`, read from `TCP_INFO`'s `TCPI_OPT_USEC_TS`): Linux cannot change it, so the other unit is
  refused with a counted reason. set_authentication_counters and the
  listener's: `TCP_AO_INFO` with its counter flag (T2).
- read_state and read_listener_state: `TCP_INFO`, `TCP_CC_INFO`, `TCP_CONGESTION`, `SIOCINQ`/`SIOCOUTQ`/
  `SIOCOUTQNSD`, `TCP_SAVED_SYN`, `TCP_TIMESTAMP`, `TCP_REPAIR_WINDOW` and the sequence options, `TCP_AO_INFO`,
  `TCP_AO_GET_KEYS`, `MPTCP_INFO`, `MPTCP_FULL_INFO`, `IP_PKTOPTIONS`, `SO_INCOMING_CPU`.
- Settings: each `TcpSetting` and `TcpListenerSetting` variant is the one socket option its doc names
  (`SO_BUF_LOCK`, `SO_RCVLOWAT`, `SO_ATTACH_FILTER`/`SO_DETACH_FILTER`, `SO_TXREHASH`, `IP_MINTTL`/
  `IPV6_MINHOPCOUNT`, `IP_PKTINFO`-family records, `IP_RECVERR`/`IPV6_RECVERR`, the `TCP_*` options of tcp(7),
  `TCP_MD5SIG_EXT` (an IPv4 prefix of an IPv6 socket mapped), `TCP_AO_ADD_KEY` and `TCP_AO_DEL_KEY` (a key added
  again or removed when absent leaves the socket as asked), `TCP_AO_INFO` (read first, since Linux writes the
  requirement and ICMP choice at every call), `TCP_REPAIR*`, `TCP_FASTOPEN`, `TCP_FASTOPEN_KEY`, `TCP_SAVE_SYN`, the flow label
  manager's reflection), and an internet-protocol setting goes through the shared crate, multicast groups through
  its `set_multicast_group` (`MCAST_JOIN_GROUP`, which Linux takes on TCP sockets) and anycast through its
  `AnycastMemberships`.

## What cannot be honoured (expected; each is confirmed on the machine before it is recorded)
- An owner other than the host's user without CAP_SETUID: EPERM.
- Settings that need CAP_NET_ADMIN or CAP_NET_RAW (mark, repair, transparent addresses, priorities above six)
  answer Linux's own EPERM when the host lacks it.
- A fixed IPv6 flow label on a listener, an accepted connection or after connect (Linux fixes it only at connect).
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
  privileged settings, run as root in a network namespace.

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
- ICMP errors reach a TCP socket only in SYN-SENT: a check fills a listener of backlog 0, connects again, captures
  that SYN with a raw socket and answers it with an ICMP error from a raw socket (root), all on loopback.
- Each setting is set once and then twice on fresh pairs, and the observable outcome (what the peer sees, what
  later calls answer) is equal; settings an implementation may refuse are in these lists too, and must answer the
  same twice.
- Each setting's effect is checked where the traits or the wire can show it: a capture thread reads the host's
  packet socket on loopback (each packet seen twice, once leaving and once arriving, and counted once) and the
  checks look at headers, sequence numbers, windows, timestamps and timing; a peer made to drop everything (a
  minimum hop limit of 255) leaves segments unanswered for the timer checks; how much a connection takes or a peer
  sends while nothing is read shows the buffer settings.
- Memberships and owners are checked by the host's own tables (`/proc/net/igmp`, `igmp6`, `anycast6`, `tcp`, `tcp6`);
  authentication keys on the wire (option kind 29: its key identifiers and MAC length) and by what a keyed listener
  admits.
- The suite needs root, `fq` on loopback, `net.ipv4.tcp_fastopen=3`, `net.ipv6.flowlabel_consistency=0`,
  `net.ipv4.tcp_fwmark_accept=1`, `net.ipv6.conf.lo.forwarding=1` (anycast joins on loopback), `dctcp` and a local route for 127.0.0.7 with the `tcp_usec_ts` feature (its
  connections' timestamps count microseconds); run-conformance.sh makes that namespace, with the iproute2 of the
  checkout's pinned nixpkgs (the machine's 6.1 does not know the feature).
- `tests!` runs each function as its own test on a value made anew, then the suites of the traits the interface
  requires, as the layout writes them: `Configurable` on a connection and on a listener (`configured_connection`,
  `configured_listener`), `NegativeRouteAdvice` (`advised_connection`) and `Readiness` (its bundles; see the status
  file for why they are not yet the layout's one line). The implementation runs it from its own conformance.rs.
- The suite builds on every unix system and runs on Linux: only Linux has the packet socket the wire checks read.
