# Plan: the host implementation of crates/udp-sockets

## What it is
- Crate `crates/udp-sockets/implementation/host`, package `wcontainers-udp-sockets-host`, type
  `HostUdpSockets`, socket `HostUdpSocket`. It implements `UdpSockets` (with `Configurable<_, UdpSocketSetting>` and
  `NegativeRouteAdvice`) with real UDP and UDP-Lite sockets of the Linux machine it runs on (`linux.rs`). On
  every other unix system (`portable.rs`) it opens, binds and connects UDP sockets, waits on them and makes the
  settings socket2 covers there, and answers a counted `NotSupported` for the rest (below).
- A socket is its own type: the non-blocking `OwnedFd`, its family and protocol, and the host state the
  implementation keeps for it (the per-send options last written, the peer, the drop counter last read),
  ordered by descriptor. Nothing waits: what would wait answers `HostError::WouldBlock`.
- Built on socket2 for the calls and settings it covers (create, bind, connect, local address, sendmsg, buffer
  sizes, sharing, multicast loopback and all-groups, TOS, traffic class, mark, priority, broadcast, freebind,
  transparency, IPv6 only), and on rustix, linux-raw-sys and libc on the same descriptor for the rest (MIT or
  Apache-2.0); polling (Apache-2.0 or MIT) for the host's readiness; etherparse (MIT or Apache-2.0, already the
  interface's) writes the headers receive owes.
- Waiting: the value owns a `Readiness` of the shared crate, one poller (epoll; kqueue elsewhere) and one thread
  that waits on it. A socket owns its descriptor through the `Watched` it gets at open, which deletes any
  registration before the descriptor closes. A waiting call lists itself in its socket's waiter list (whose room
  is kept, so a wait allocates nothing once it has grown) and asks the socket with a `poll` of no timeout; only
  if it has to wait does it register the socket, edge-triggered, for the events its unwoken calls ask for
  (changed only when a call asks for more). The thread, on an event, asks the socket the same way, wakes each
  waiter whose events have come (taking its waker, so each poll is woken once), and deletes the registration
  once no call waits unwoken. So a socket nobody waits on costs the thread nothing, and a call waiting for an
  event the poller only approximates (POLLRDBAND, POLLMSG and POLLRDHUP by the socket being readable) is looked
  at again at each change of its socket, not at once forever, as a level-triggered one would be. Dropping a
  call removes it from the list, and the last call waiting unwoken takes the registration with it.
- Work on the shared types of crates/internet-protocol goes into the one shared crate
  `crates/internet-protocol/implementation/linux-socket-options`, which the TCP track created: its errno
  mapping, raw option call, `IpSocketSetting` and `PortSetting` onto options, owner and bind. This track adds
  to it the datagram half the ICMP and raw tracks need too: `IpHeaderFields` and `IpSendFields` onto options
  and control messages, and control messages and the error queue onto `ReceivedPacketFields`,
  `TransmitReport` and ICMP errors written whole from their IP header.
- On other unix systems: open (UDP as the process's own user), bind (requested or kernel-chosen port, IPv6 only or
  dual stack), connect (destination and ports alone), wait_for_events, close, and the settings socket2 covers
  (buffers, address and port sharing, multicast loopback) through socket2; the settings at the value every socket
  there has and cannot leave are taken. Everything else is refused, counted under a named reason: UDP-Lite,
  another owner, the exit node's port choice, other route selectors, disconnect (no socket2 call), send (the
  IPv4 path MTU discovery the header asks for, and most fields, have no socket2 call), receive (the IP header
  needs control messages socket2 does not parse), the unsent bytes, the incoming processor, negative route advice
  and the other settings. No Windows build: its sockets are not descriptors.
- What the host cannot honour answers `HostError::NotSupported` (or Linux's own errno), counted per named
  reason, readable as abi `CounterReading`s, and warned once on standard error.
- The interface has no constructor: the crate has the smallest public `new()`, and the owner is asked for a
  `StandaloneUdpSockets` trait.

## Methods on the host
- open: socket2's `Socket::new(AF_INET or AF_INET6, SOCK_DGRAM | SOCK_NONBLOCK, IPPROTO_UDP or
  IPPROTO_UDPLITE)` (socket2 adds `SOCK_CLOEXEC`) under the owner as the thread's filesystem user; then every receive control message on
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
- receive (libc's `recvmsg`, as socket2's gives no `MSG_CTRUNC`): the entries kept, and `recvmsg(MSG_ERRQUEUE)`
  while a send's reports may wait, first; else after a normal receive that fails with an error other than
  would-block, since Linux sets the socket error with each ICMP error it queues and answers that error first (a
  send that fails may have taken it, so it moves the host's entries into the kept ones). An error queue entry
  is read on the stack, so only its own bytes are allocated (an ICMP error becomes the ICMP packet, IP header first, quoting the
  datagram's headers; a timestamp becomes a `TransmitReport`; a local error was already the send's answer),
  else `recvmsg(MSG_TRUNC)`; the payload is moved up and the IP and UDP headers written before it from the
  control messages; `UDP_GRO` gives the segment size.
- count_unsent_bytes: `ioctl(SIOCOUTQ)`. read_incoming_processor: socket2's `cpu_affinity` (`SO_INCOMING_CPU`),
  -1 is none.
- give_negative_route_advice: Linux has no call for it; a connected socket is connected again to its peer,
  which drops its cached route and takes a new flow hash; an unconnected socket caches no route.
- wait_for_events: `Watched::wait_for_events`, the events of `poll` (asked for and always reported), with an
  error held while entries a send read off the error queue wait for receives.
- close: the registration is deleted, then the descriptor closes.
- Settings: `Ip` and `Port` through the shared crate; `ReceiveCoalescing` is `UDP_GRO`,
  `ZeroChecksumReceipt` is `UDP_NO_CHECK6_RX`, `MinimumChecksumCoverage` is `UDPLITE_RECV_CSCOV`
  (none is 8, the UDP header, which takes every valid coverage).

## What cannot be honoured (expected; each confirmed on the machine before it is recorded)
- Header fields Linux never gives a UDP socket: IPv4 identification, flags and the received UDP checksum and
  UDP-Lite coverage; they are written as zero, no flags, a computed checksum and full coverage.
- Wireless status reports carry no key on Linux; asking for one answers `NotSupported`.
- A transmit time rejection carries the packet's time, not its key: each send that asks for its errors is
  noted with its time, so that the rejection of that time gives its key.

## The suite (crates/udp-sockets/conformance/conformance.rs)
- Written against the traits alone. Each function takes one `UdpSockets` value, opens sockets on 127.0.0.1
  or ::1 with kernel-chosen ports, and checks one behaviour by what the other end receives: the payload and
  every header field (addresses, ports, TTL or hop limit, DSCP and ECN, flow label), connect filtering and
  disconnect, UDP-Lite coverage, segmentation and coalescing, checksum omission and its receipt, ICMP port
  unreachable on the error queue, `MessageTooLong`, transmit reports with their keys, receive timestamps,
  drops after a small receive buffer, address and port sharing, nonlocal binding, multicast and anycast
  membership, the incoming processor, the unsent byte count, negative route advice and close.
- Each setting is also set twice and must answer and behave as set once.
- Waits are polled by hand with a waker that counts its wakes (no executor): a wait for data is pending and not
  woken until a datagram arrives, then woken once and finished; a wait for writing finishes at its first poll;
  a dropped wait gives back its waker and is not woken; 35 waits at once on 32 sockets each wake once; an ICMP
  error ends a wait for data with the error condition, also when a too-long send read it off the host first; a
  wait polled again wakes its newest waker; a wait whose event went is woken by the next; two waits on one
  socket finish each by its own events; a wait for no event ends on an error; a wait for writing on a full send
  buffer finishes when its paced datagrams leave room, also after a wait for data on it was woken first, and a
  wait for data or writing made while a wait for data waits; a wait for every event gives exactly what Linux's
  poll gives a UDP socket (room to send as POLLOUT, POLLWRNORM and POLLWRBAND, then data as POLLIN and
  POLLRDNORM, then an error); a wait for POLLRDNORM, POLLWRNORM or POLLWRBAND alone gives it alone; waits for
  the events a UDP socket never has (POLLPRI, POLLHUP, POLLRDBAND, POLLMSG, POLLRDHUP) wait on.
- A send whose header's upper layer or whose checksum is the other protocol's is refused and sends nothing.
- The unsent bytes of a send buffer filled until a send would block are at least the payload it holds and less
  than twice the size given (what Linux keeps) and one datagram more; a connected socket's incoming processor
  is the processor its sender ran on.
- The implementation's own tests: the watcher thread does not wake for datagrams nobody waits for, nor keeps
  running for a wait for an event the socket lacks; a wait allocates nothing once warm, nor a receive that would
  block; a send's report is read before a datagram that arrived after it (an order the contract leaves open);
  the unsent bytes of a full send buffer are Linux's memory count (the owner has not ruled on it).
- The implementation's own test runs `tests!` with each test in a network namespace of its own, all made before
  any test opens a socket, as root on the ARM64 machine: one command, `conformance.sh` in the crate.
