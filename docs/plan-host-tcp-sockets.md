# Plan: the host implementation of crates/tcp-sockets

## What it is
- Crate `crates/tcp-sockets/implementation/linux-host`, package `wcontainers-tcp-sockets-linux-host`, type
  `LinuxHostTcpSockets`. It implements `TcpSockets` (and so `TcpConnections`, `TcpListeners`,
  `Configurable` for both setting types and `NegativeRouteAdvice`) with real Linux TCP and MPTCP sockets of the
  machine it runs on, the exit node. Its whole body is `cfg(target_os = "linux")`, so the workspace still builds
  on the Mac.
- A connection and a listener are its own types holding the socket's `OwnedFd` and its family, ordered by
  descriptor. Every socket is non-blocking: the contract waits for nothing, so an operation that would wait
  answers `HostError::WouldBlock`, as the abi's host interface does.
- Built on rustix (socket calls and the socket options it has), linux-raw-sys (the constants and structures of
  the Linux uapi headers that rustix does not wrap) and libc's `setsockopt`/`getsockopt` as the last resort for
  options that rustix does not wrap. All three are MIT or Apache-2.0.
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
  `EINPROGRESS` is `SynchronizeSent`, an immediate success under `TCP_REPAIR` is `EstablishedForRepair`, an
  immediate success under `TCP_FASTOPEN_CONNECT` is `DeferredUntilFirstSend`.
- listen: the same socket and bind, `TCP_SAVE_SYN` always (accept owes the SYN's IP header), the settings,
  `listen(backlog)`. The `Backlog` setting is `listen` again.
- accept: `accept4` non-blocking; the SYN's header from `TCP_SAVED_SYN`, cut to the buffer; its fields from
  that header and what the accepted socket reports.
- send: `sendmsg` with `MSG_NOSIGNAL` and `MSG_OOB`, `MSG_MORE`, `MSG_EOR` from the request; transmit reports as
  an `SO_TIMESTAMPING` control message with the request's record flags and `SCM_TS_OPT_ID` for its key, the
  socket's own option flags (`OPT_ID`, `OPT_TSONLY`, `OPT_STATS`) kept on by the implementation.
- receive: `recvmsg` with `MSG_PEEK` (and `SO_PEEK_OFF` for an offset), `MSG_OOB`; timestamps from the
  control messages; `unread_after` from `TCP_CM_INQ` when unread-byte reports are on.
- take_error_queue_entry: `recvmsg(MSG_ERRQUEUE)`, `sock_extended_err` and the control messages it comes with;
  an ICMP error written whole from its IP header by the shared crate's `write_icmp_error`; a report's packet
  with `OPT_TSONLY` off, its statistics from `OPT_STATS`, wireless status from `SO_WIFI_STATUS`.
- discard_error_queue_entries: `recvmsg(MSG_ERRQUEUE)` until `EAGAIN`.
- shut_down: `shutdown`. close: `SO_LINGER` of zero for an abort, then the descriptor closes. stop_listening:
  the descriptor closes.
- push_queued_data: `TCP_NODELAY` set to its current value (`CorkAndNagle`) or `TCP_CORK` set to its current
  value when uncorked (`Neither`), the two option writes Linux pushes on.
- send_delayed_acknowledgement: `TCP_QUICKACK`, which sends a scheduled delayed acknowledgement and sets the
  mode.
- rearm_keep_alive_timer: `TCP_KEEPIDLE` written again with its current value, which re-times the probe from the
  last data or acknowledgement received when keep-alive is on.
- send_window_probe: the only window probe Linux offers a program is the one `TCP_REPAIR` set to
  `TCP_REPAIR_OFF` sends: repair on, then off; it needs CAP_NET_ADMIN (EPERM without it).
- give_negative_route_advice: `SO_CNX_ADVICE` set to one, which calls `dst_negative_advice`.
- set_timestamp_clock and the listener's: `TCP_TIMESTAMP` (repair mode). set_authentication_counters and the
  listener's: `TCP_AO_INFO` with its counter flag.
- read_state and read_listener_state: `TCP_INFO`, `TCP_CC_INFO`, `TCP_CONGESTION`, `SIOCINQ`/`SIOCOUTQ`/
  `SIOCOUTQNSD`, `TCP_SAVED_SYN`, `TCP_TIMESTAMP`, `TCP_REPAIR_WINDOW` and the sequence options, `TCP_AO_INFO`,
  `TCP_AO_GET_KEYS`, `MPTCP_INFO`, `MPTCP_FULL_INFO`, `IP_PKTOPTIONS`, `SO_INCOMING_CPU`.
- Settings: each `TcpSetting` and `TcpListenerSetting` variant is the one socket option its doc names
  (`SO_BUF_LOCK`, `SO_RCVLOWAT`, `SO_ATTACH_FILTER`/`SO_DETACH_FILTER`, `SO_TXREHASH`, `IP_MINTTL`/
  `IPV6_MINHOPCOUNT`, `IP_PKTINFO`-family records, `IP_RECVERR`/`IPV6_RECVERR`, the `TCP_*` options of tcp(7),
  `TCP_MD5SIG_EXT`, `TCP_AO_*`, `TCP_REPAIR*`, `TCP_FASTOPEN`, `TCP_FASTOPEN_KEY`, `TCP_SAVE_SYN`, the flow label
  manager's reflection), and an internet-protocol setting goes through the shared crate.

## What cannot be honoured (expected; each is confirmed on the machine before it is recorded)
- An owner other than the host's user without CAP_SETUID: EPERM.
- Settings that need CAP_NET_ADMIN or CAP_NET_RAW (mark, repair, transparent addresses, priorities above six)
  answer Linux's own EPERM when the host lacks it.
- Fields of `ReceivedPacketFields` that Linux never reports for a SYN (its IPv4 arrival interface, its
  timestamps): taken from what the accepted socket reports, each counted under a named reason.

## Milestones
- T1: connect (settings, fast open), listen, accept, send, receive with their requests, shut_down, close,
  stop_listening, the error queue, the settings that need no privilege, the action methods; the suite for
  them, passing on the Linux machine over loopback.
- T2: read_state and read_listener_state with every query; repair, timestamps, authentication (MD5 and AO),
  multipath; the privileged settings, run as root in a network namespace.

## The suite (crates/tcp-sockets/conformance/conformance.rs)
- Written against the traits alone: each public function takes one `TcpSockets` value, makes its own listener
  on 127.0.0.1 (or ::1) port chosen by the kernel, connects to it, accepts, and checks one behaviour by what the
  other end sees: bytes arriving in order, the end of stream after a shutdown, a reset after an abort, urgent
  data, peeking, more-to-come and cork holding data until a push, a refused connect, an error queue entry for a
  requested software transmit timestamp, and so on. Operations that answer `WouldBlock` are retried until a
  deadline, since nothing in the contract waits.
- ICMP errors reach a TCP socket only in SYN-SENT: a check fills a listener of backlog 0, connects again, captures
  that SYN with a raw socket and answers it with an ICMP error from a raw socket (root), all on loopback.
- Each setting is set once and then twice on fresh pairs, and the observable outcome (what the peer sees, what
  later calls answer) is equal. A setting the implementation answers `NotSupported` must answer it twice.
- `tests!` runs each function as its own test on a value made anew.
