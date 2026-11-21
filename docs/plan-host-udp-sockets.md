# Plan: the host implementation of crates/udp-sockets

## What it is
- Crate `crates/udp-sockets/implementation/linux-host`, package `wcontainers-udp-sockets-linux-host`, type
  `LinuxHostUdpSockets`. It implements `UdpSockets` (with `Configurable<_, UdpSocketSetting>` and
  `NegativeRouteAdvice`) with real UDP and UDP-Lite sockets of the Linux machine it runs on. Its body is
  `cfg(target_os = "linux")`, so the workspace still builds on the Mac.
- A socket is its own type: the non-blocking `OwnedFd`, its family and protocol, and the host state the
  implementation keeps for it (the per-send options last written, the peer, the drop counter last read),
  ordered by descriptor. Nothing waits: what would wait answers `HostError::WouldBlock`.
- Built on rustix, linux-raw-sys and libc (MIT or Apache-2.0), as the TCP track's plan and the shared crate's
  manifest chose; etherparse (MIT or Apache-2.0, already the interface's) writes the headers receive owes.
- Work on the shared types of crates/internet-protocol goes into the one shared crate
  `crates/internet-protocol/implementation/linux-socket-options`, which the TCP track created: its errno
  mapping, raw option call, `IpSocketSetting` and `PortSetting` onto options, owner and bind. This track adds
  to it the datagram half the ICMP and raw tracks need too: `IpHeaderFields` and `IpSendFields` onto options
  and control messages, and control messages and the error queue onto `ReceivedPacketFields`,
  `TransmitReport` and ICMP errors written whole from their IP header.
- What the host cannot honour answers `HostError::NotSupported` (or Linux's own errno), counted per named
  reason, readable as abi `CounterReading`s, and warned once on standard error.
- The interface has no constructor: the crate has the smallest public `new()`, and the owner is asked for a
  `StandaloneUdpSockets` trait.

## Methods on the host
- open: `socket(AF_INET or AF_INET6, SOCK_DGRAM | SOCK_NONBLOCK | SOCK_CLOEXEC, IPPROTO_UDP or
  IPPROTO_UDPLITE)` under the owner as the thread's filesystem user; then every receive control message on
  (packet information, original destination, TTL or hop limit, TOS or traffic class, IP options, IPv6
  extension headers and flow information, fragment size, mark, priority, drop counter), `IP_RECVERR` or
  `IPV6_RECVERR`, `SO_TIMESTAMPING` with `OPT_ID`, `OPT_CMSG`, `OPT_TX_SWHW` and `OPT_RX_FILTER`, and IPv6
  automatic flow labels off (a route's label is the label sent).
- bind: `IPV6_V6ONLY` from the address, `bind`; the exit node's choice is `IP_LOCAL_PORT_RANGE` once per
  range between reserved ports, checked to have landed inside it; returns `getsockname`.
- connect: the route's selectors onto the socket's options (TOS or traffic class, mark, unicast interface,
  `SO_DONTROUTE` for link scope, `SO_BROADCAST`, `IP_OPTIONS`, transparency, flow label lease and
  `IPV6_FLOWINFO_SEND`, source preferences, `IPV6_RTHDR`), a bind to the source first when the socket has
  no address, then `connect` to the destination and the upper layer's destination port.
- disconnect: `connect(AF_UNSPEC)`, then `getsockname`.
- send_to: `sendmsg` with the per-send fields that Linux takes as control messages (source and interface
  `IP_PKTINFO`/`IPV6_PKTINFO`, TTL, hop limit, TOS, traffic class, IP options, IPv6 extension headers,
  `IPV6_DONTFRAG`, mark, priority, `SCM_TXTIME`, the timestamping record flags and `SCM_TS_OPT_ID`,
  `UDP_SEGMENT`) and flags (`MSG_CONFIRM`, `MSG_DONTROUTE`); the rest (path MTU discovery, IPv6 fragment MTU,
  broadcast, transparency, flow label, source preferences, checksum omission and UDP-Lite coverage, transmit
  time mode and errors) written as socket options only when they differ from what the socket last had.
  `EMSGSIZE` with a local error queued is `MessageTooLong` with that error's MTU.
- receive: `recvmsg(MSG_ERRQUEUE)` first (an ICMP error becomes the ICMP packet, IP header first, quoting the
  datagram's headers; a timestamp becomes a `TransmitReport`; a local error was already the send's answer),
  else `recvmsg(MSG_TRUNC)`; the payload is moved up and the IP and UDP headers written before it from the
  control messages; `UDP_GRO` gives the segment size.
- count_unsent_bytes: `ioctl(SIOCOUTQ)`. read_incoming_processor: `SO_INCOMING_CPU`, -1 is none.
- give_negative_route_advice: Linux has no call for it; a connected socket is connected again to its peer,
  which drops its cached route and takes a new flow hash; an unconnected socket caches no route.
- close: the descriptor closes.
- Settings: `Ip` and `Port` through the shared crate; `ReceiveCoalescing` is `UDP_GRO`,
  `ZeroChecksumReceipt` is `UDP_NO_CHECK6_RX`, `MinimumChecksumCoverage` is `UDPLITE_RECV_CSCOV`.

## What cannot be honoured (expected; each confirmed on the machine before it is recorded)
- Header fields Linux never gives a UDP socket: IPv4 identification, flags and the received UDP checksum and
  UDP-Lite coverage; they are written as zero, no flags, a computed checksum and full coverage.
- Wireless status reports carry no key on Linux; asking for one answers `NotSupported`.
- A transmit time rejection carries the packet's time, not its key, so it cannot become a `TransmitReport`.

## The suite (crates/udp-sockets/conformance/conformance.rs)
- Written against the traits alone. Each function takes one `UdpSockets` value, opens sockets on 127.0.0.1
  or ::1 with kernel-chosen ports, and checks one behaviour by what the other end receives: the payload and
  every header field (addresses, ports, TTL or hop limit, DSCP and ECN, flow label), connect filtering and
  disconnect, UDP-Lite coverage, segmentation and coalescing, checksum omission and its receipt, ICMP port
  unreachable on the error queue, `MessageTooLong`, transmit reports with their keys, receive timestamps,
  drops after a small receive buffer, address and port sharing, nonlocal binding, multicast and anycast
  membership, the incoming processor, the unsent byte count, negative route advice and close.
- Each setting is also set twice and must answer and behave as set once.
- The implementation's own test runs `tests!` with each test in a network namespace of its own thread
  (unshare, loopback up), as root on the ARM64 machine: one command, `scripts/conformance.sh` in the crate.
