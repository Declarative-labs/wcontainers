# Status of the host-udp-sockets track

Plan: docs/plan-host-udp-sockets.md. Branch: main, checkout /Users/thomaseaton/Documents/GitHub/wcontainers.

## For the other socket tracks (TCP, ICMP and raw)
- This track added the datagram half to the shared crate `crates/internet-protocol/implementation/linux-socket-options`,
  as `pub mod datagram` (one line in `implementation.rs`, the rest in `datagram.rs` and `datagram/`):
  - `datagram::send`: `SendState` (per socket) writes a route's selectors (`apply_route`), a send's header
    (`apply_header`) and its `IpSendFields` (`apply_send_fields`) as socket options, each written only when it
    differs from what the socket last had (`datagram::sticky`), and as `sendmsg` control messages (`Control`:
    source address, TTL or hop limit, transmit time, timestamping record flags and `SCM_TS_OPT_ID`). It keeps
    `SO_TIMESTAMPING` itself (`set_receive_timestamps`), with `OPT_ID`, `OPT_CMSG`, `OPT_TX_SWHW`,
    `OPT_RX_FILTER`, `OPT_PKTINFO` always on and `OPT_TSONLY` following the send's `packet` request.
  - `datagram::receive`: `receive` (one `recvmsg` with the sender's address and room for every control message
    this needs, IPv6 extension headers included), `Ancillary::parse` (every control message into one struct),
    `packet_fields`, `transmit_report`, and `Headers`, `ipv4_header`, `push_ipv6_headers`, `udp_checksum` to
    write a packet whole from its IP header in front of a payload that arrived at the buffer's start.
  - `datagram::memberships`: `set_multicast_group` (join, then the whole source filter with `MCAST_MSFILTER`;
    leave; both idempotent) and `AnycastMemberships` (Linux counts anycast joins, so the socket keeps them).
  - `datagram::addresses`: socket address conversions.
- The ICMP track can use all of these for `send_to` and `receive`. The TCP track's `set_ip_socket_setting` still
  refuses multicast and anycast; it could call `datagram::memberships` instead.

## Milestone U1: done
- Crate `crates/udp-sockets/implementation/linux-host` (package `wcontainers-udp-sockets-linux-host`, type
  `LinuxHostUdpSockets`, socket `LinuxHostUdpSocket`), body `cfg(target_os = "linux")`. Methods as the plan maps
  them. Refusals counted per reason (`counters()`), warned once on standard error.
- Suite `crates/udp-sockets/conformance/conformance.rs`: 30 checks against the traits alone (see the file), every
  setting set twice before its effect is checked.
- Last full run (see Measurements): 30 of 30 pass on the ARM64 machine.

## How to run
- On the ARM64 machine (root, each test in a network namespace of its own thread with only loopback, up):
  `arm-run /Users/thomaseaton/Documents/GitHub/wcontainers --name host-udp-sockets -- 'crates/udp-sockets/implementation/linux-host/conformance.sh 12'`
  (the argument is the test threads; further arguments filter tests by name). It builds as the user and runs
  the test binary with `sudo -n`.
- Lint: `cargo clippy -p wcontainers-internet-protocol-linux-socket-options -p wcontainers-udp-sockets-linux-host -p wcontainers-udp-sockets-conformance --tests`
  there. On the Mac the crate is empty (Linux only); `cargo check` of the three packages passes.
- No random tests: every check is a fixed scenario.

## Measurements
- 2026-11-21, ARM64 machine (96 cores), job host-udp-sockets: the 30 checks take 0.51 s wall at 12 test
  threads; the machine's load was 20 to 30 with other tracks' jobs.

## Honoured, and how each is checked
- open (both versions, both protocols), bind (kernel port, a taken port, exit node's choice around a reserved
  port), connect (filtering, local address, an unbound socket), disconnect (explicit port kept, a kernel-chosen
  port given up, as Linux does, unbound), send_to, receive (IPv4 and IPv6 headers whole, a short buffer keeping
  the whole length, would block), count_unsent_bytes (zero after a delivered send), read_incoming_processor,
  negative route advice (unconnected and connected, then sending), close (port free again).
- Header fields checked by what arrives: TTL, hop limit, DSCP, ECN, flow label, IPv4 options, IPv6 hop-by-hop
  and destination options, source address, mark, link scope, interface (multicast); fragment MTU with
  fragmentation forbidden (`MessageTooLong` with that MTU). Send fields: priority (received), neighbour
  confirmation, transmit time, transmit reports scheduled and software with key and packet or not.
- Datagram fields: checksum computed, omitted (IPv4 arrives; IPv6 dropped unless accepted), UDP-Lite partial
  coverage against the receiver's minimum; segment size.
- Received fields checked: interface, specific destination, mark, priority, software timestamps, drops since
  previous (exactly the number dropped), segment size.
- Settings checked by their effect: address and port sharing, nonlocal addresses (refused and bindable), bound
  interface (and a nonexistent one refused), incoming processor, IPv4 multicast join and leave with multicast
  loopback and all-groups off, receive buffer, timestamping, receive coalescing, zero checksum receipt, minimum
  checksum coverage. Taken twice without effect checked: send buffer, maximum pacing rate.
- ICMP errors on the error queue as whole ICMP packets (port unreachable, IPv4 and IPv6, quoting the datagram).

## Implemented but not yet checked by the suite (next run)
- IPv6 multicast, anycast, the group filter, transparent nonlocal addresses, multicast all-groups for IPv6,
  routing header, routing destination options, source address check waived, broadcast permission, source
  preferences, path MTU discovery modes other than do, hardware timestamps and a bound clock (loopback has no
  hardware clock: Linux answers what it answers), acknowledgement and completion reports (Linux makes none for
  UDP), the owner as another user (`setfsuid` around `socket`; checkable through /proc/net/udp's uid), pacing
  (needs an fq qdisc the suite cannot set up).

## Not honoured, with the Linux reason
- Wireless status reports: Linux reports them (`SO_WIFI_STATUS`) with no key of the send; a send asking for one
  answers `NotSupported`, counted.
- Transmit time rejections (`SO_EE_ORIGIN_TXTIME`): Linux gives the packet's time, not the send's key; such an
  entry is dropped, counted and warned.
- Header fields Linux never gives a UDP socket: IPv4 identification and flags (written as zero and none), the
  received UDP checksum (recomputed), the sender's UDP-Lite coverage (written as zero, the whole datagram). In an
  ICMP error the quoted IPv4 TTL and the quoted UDP checksum are zero.
- An error queue entry longer than the buffer reports only the part read: Linux's error queue gives no length.
- A transmit report without its packet (`packet` not requested, `OPT_TSONLY`) has interface 0: Linux gives the
  interface only with the packet. Linux reads `OPT_TSONLY` when it makes the report, so a send whose request
  differs from the one before it while reports are outstanding can get the other's choice.
- A send whose source port is not the socket's, and a connect whose source is not the bound address: Linux
  sends from the bound port and address only; `NotSupported`, counted.
- Exit node port choice: done once per allowed range with `IP_LOCAL_PORT_RANGE`; Linux falls back to the host's
  range when a range and it do not meet, so ranges outside `net.ipv4.ip_local_port_range` answer
  `NotSupported`, counted.
- Negative route advice has no system call: a connected socket is connected again to its peer, which drops its
  cached route and takes a new flow hash even when the route was still good; an expired IPv6 exception route is
  not removed from the table.
- Send and receive buffer sizes above the host's `wmem_max`/`rmem_max` are cut by the host (the shared crate's
  plain `SO_SNDBUF`/`SO_RCVBUF`), not counted yet.

## Dependencies and licences
- rustix 1.1.6 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT), linux-raw-sys 0.12.1 (same), libc 0.2.191
  (MIT OR Apache-2.0), etherparse 0.21.0 (MIT OR Apache-2.0); all already in the workspace.

## Departures from the plan
- The plan's crate name stays; the shared crate's existing calls (`set_ip_socket_setting`, `set_port_setting`,
  `Shortfalls`, the raw option calls, `receive_timestamping_flags`) are used instead of this track's own, and
  the datagram modules moved there, as the plan said they would.

## For the owner
- The interface has no constructor: proposed `trait StandaloneUdpSockets: UdpSockets { fn new() -> Self; }`;
  meanwhile `LinuxHostUdpSockets::new()`.
- `UDPLITE_SEND_CSCOV`, `UDPLITE_RECV_CSCOV` and `SO_EE_ORIGIN_TXTIME` are in no established crate (libc and
  linux-raw-sys lack linux/udp.h and linux/errqueue.h). The kernel implementation keeps private copies in
  `crates/kernel/implementation/linux/linux_extra.rs`; the shared crate now has `UDP_LITE_SEND_COVERAGE`,
  `UDP_LITE_RECEIVE_COVERAGE` and `SO_EE_ORIGIN_TXTIME` (named as the TCP track named the stage constants).
  One fact in two places: one of the two should import the other.
- `TransmitReport.interface` cannot be filled for a report without its packet (see above): an
  `Option<InterfaceIndex>` would say so; this implementation writes 0.
- `TransmitTimeRejected` carries a key, but Linux reports the rejected packet's transmit time.
