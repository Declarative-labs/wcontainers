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
    leave; both idempotent; a join on no interface is made on the interface Linux would choose, found with a
    probe socket, because `MCAST_MSFILTER` with interface 0 answers `EINVAL`) and `AnycastMemberships` (Linux
    counts anycast joins, so the socket keeps them).
  - `datagram::addresses`: socket address conversions.
- The ICMP track can use all of these for `send_to` and `receive`. The TCP track's `set_ip_socket_setting` still
  refuses multicast and anycast; it could call `datagram::memberships` instead.
- Bug for the TCP track (measured here): Linux reads `IPV6_UNICAST_IF` in network byte order, as it reads
  `IP_UNICAST_IF`. `set_ip_packet_setting` in the shared `implementation.rs` passes the IPv6 index in host order
  (`IpVersion::V6 => set_option(..., IPV6_UNICAST_IF, &index)`): index 2 becomes 33554432 and Linux answers
  `EADDRNOTAVAIL`. `datagram::send` now sends `to_be()`; I left `implementation.rs` alone (other tracks edit it).

## Milestone U1: done (independent check below)
- Crate `crates/udp-sockets/implementation/linux-host` (package `wcontainers-udp-sockets-linux-host`, type
  `LinuxHostUdpSockets`, socket `LinuxHostUdpSocket`), body `cfg(target_os = "linux")`. Methods as the plan maps
  them. Refusals and host cuts counted per reason (`counters()`), warned once on standard error.
- Suite `crates/udp-sockets/conformance/conformance.rs`: 39 checks against the traits alone (see the file), every
  setting set twice before its effect is checked. The network the suite needs beyond loopback is listed at the
  top of the file; the implementation's test makes it.
- The implementation's own test `a_buffer_the_host_cuts_is_counted` (in `tests/conformance.rs`).
- Last run: 40 of 40 pass on the ARM64 machine.

## How to run
- On the ARM64 machine (root; each test in a network namespace of its own thread, made by `isolated()` in
  `tests/conformance.rs` with iproute2's `ip` and `tc`, test tools only):
  `arm-run /Users/thomaseaton/Documents/GitHub/wcontainers --name host-udp-sockets -- 'crates/udp-sockets/implementation/linux-host/conformance.sh 12'`
  (the argument is the test threads; further arguments filter tests by name). It builds as the user and runs
  the test binary with `sudo -n`.
- Lint: `cargo clippy -p wcontainers-internet-protocol-linux-socket-options -p wcontainers-udp-sockets-linux-host -p wcontainers-udp-sockets-conformance --tests`
  there (my crates are clean; `-D warnings` stops at two warnings of the reviewed `crates/kernel/interface.rs`).
  On the Mac the crate is empty (Linux only); clippy of the three packages is clean.
- Independent check: pyro-parked/wcontainers-scratch/host-udp-sockets/independent-check.sh, passed as the
  arm-run command (`-- "$(cat .../independent-check.sh)"`): a clean build and run, six broken copies of the
  implementation, and cargo tree of the interface crates.
- No random tests: every check is a fixed scenario.

## Measurements
- 2026-11-21, ARM64 machine (96 cores), job host-udp-sockets: the 35 checks took 0.51 s wall at 12 test
  threads at a load of 6.1 to 6.8; the 40 tests took 0.52 s at a load of 11; the clean build and run took 12 s
  at a load of 6.9 to 9.8.
- Pacing on the machine's loopback with fq, 5 datagrams of 1000 bytes at 50 000 bytes per second: arrivals at
  0.1, 20.9, 41.7, 62.6 and 83.4 ms (Python, same namespace set-up); the check asks for at least 80 ms.

## Independent check (2026-11-21, ARM64 machine)
- Clean build (fresh target folder): 40 of 40 pass.
- Six broken copies, each caught (load 6.9 to 17, 34 s in all):
  - count_unsent_bytes always zero: pacing_holds_datagrams_in_the_send_buffer fails.
  - close keeping the descriptor open: close_frees_the_port, disconnect_keeps_the_binding,
    bind_chosen_by_exit_node_avoids_reserved_ports and a_closed_port_answers_with_an_icmp_error fail.
  - disconnect without `connect(AF_UNSPEC)`: disconnect_keeps_the_binding fails.
  - read_incoming_processor always none: the_incoming_processor_is_read_back fails.
  - negative route advice refusing a connected socket: negative_route_advice_keeps_a_socket_sending fails.
  - connect without `connect`: a_connected_socket_takes_only_its_peers_datagrams, disconnect_keeps_the_binding
    and a_closed_port_answers_with_an_icmp_error fail.
- cargo tree (normal, build and dev edges) of wcontainers-udp-sockets, -internet-protocol, -configurable,
  -negative-route-advice, -kernel and -udp-sockets-conformance: no implementation crate.
- No reviewed file changed: none of this track's commits (741ce8c3, 8713c3c3, 68bc6e36, 77a0fcec, a73a0bc4,
  1206cea5) touches an `interface.rs` or a data file beside one, and the working tree has no change to them.
- Which check fails when a method breaks: open (every check; owner: an_owner_is_routed_as_its_user), bind
  (bind_returns_the_bound_address, the sharing checks, bind_chosen_by_exit_node_avoids_reserved_ports),
  connect (a_connected_socket_takes_only_its_peers_datagrams), disconnect (disconnect_keeps_the_binding),
  send_to (every check that sends; each header and send field by its own check), receive (every check that
  receives; a_short_buffer_keeps_the_whole_length, receive_with_nothing_queued_would_block,
  a_closed_port_answers_with_an_icmp_error, transmit_reports_carry_the_key_of_their_send), count_unsent_bytes
  (pacing_holds_datagrams_in_the_send_buffer, sending_settings_are_taken), read_incoming_processor
  (the_incoming_processor_is_read_back), negative route advice (negative_route_advice_keeps_a_socket_sending),
  close (close_frees_the_port), configure (each setting's check).

## Honoured, and how each is checked
- open (both versions, both protocols, the owner by routing: user 1000 prohibited, the superuser routed), bind
  (kernel port, a taken port, exit node's choice around a reserved port), connect (filtering, local address, an
  unbound socket), disconnect (explicit port kept, a kernel-chosen port given up, as Linux does, unbound),
  send_to, receive (IPv4 and IPv6 headers whole, a short buffer keeping the whole length, would block),
  count_unsent_bytes (zero after a delivered send, not zero while paced datagrams wait),
  read_incoming_processor, negative route advice (unconnected and connected, then sending), close (port free
  again).
- Header fields checked by what arrives: TTL, hop limit, DSCP, ECN, flow label, IPv4 options, IPv6 hop-by-hop,
  destination and routing destination options, routing header (segment routing), source address, mark, link
  scope, interface (multicast); fragment MTU with fragmentation forbidden (`MessageTooLong` with that MTU).
  Send fields: priority (received), neighbour confirmation, transmit time, transmit reports scheduled and
  software with key and packet or not.
- Datagram fields: checksum computed, omitted (IPv4 arrives; IPv6 dropped unless accepted), UDP-Lite partial
  coverage against the receiver's minimum; segment size.
- Received fields checked: interface, specific destination, mark, priority, software timestamps, no hardware
  timestamp on loopback, drops since previous (exactly the number dropped), segment size.
- Also checked: broadcast refused without permission (`EACCES`) and delivered with it; every path MTU discovery
  mode sends (IPv4 and IPv6); a waived source check on a transparent socket sends from an address the host
  lacks; IPv6 source preferences; the port group's filter choosing the second socket, and detached.
- Settings checked by their effect: address and port sharing, nonlocal addresses (refused and bindable), bound
  interface (and a nonexistent one refused), incoming processor, IPv4 multicast join and leave with multicast
  loopback and all-groups off, IPv6 multicast joined on the interface Linux chooses and on one given with
  all-groups off, IPv6 anycast (link-local), receive buffer, send buffer (a paced send into the smallest one
  would block), maximum pacing rate (datagrams arrive at least the rate apart), timestamping (software; hardware
  taken, none on loopback; a bound clock loopback lacks refused), receive coalescing, zero checksum receipt,
  minimum checksum coverage.
- ICMP errors on the error queue as whole ICMP packets (port unreachable, IPv4 and IPv6, quoting the datagram).

## Not honoured or not checkable, with the Linux reason
- Wireless status reports: Linux reports them (`SO_WIFI_STATUS`) with no key of the send; a send asking for one
  answers `NotSupported`, counted.
- Acknowledgement reports: Linux makes them only for TCP (`SOF_TIMESTAMPING_TX_ACK`); a UDP send asking for one
  gets none, as on Linux. Completion reports (`SOF_TIMESTAMPING_TX_COMPLETION`) and hardware transmit reports
  come only from a driver with a hardware clock; loopback has none, so neither is checked.
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
- Send and receive buffer sizes above the host's `wmem_max`/`rmem_max` are cut by the host; taken, counted
  (`SEND_BUFFER_CUT_BY_THE_HOST`, `RECEIVE_BUFFER_CUT_BY_THE_HOST`) and warned.
- Routing headers: Linux takes only segment routing (type 4) and, with mobile IPv6 built in, type 2; others
  answer `EINVAL` from Linux.

## Dependencies and licences
- rustix 1.1.6 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT), linux-raw-sys 0.12.1 (same), libc 0.2.191
  (MIT OR Apache-2.0), etherparse 0.21.0 (MIT OR Apache-2.0); all already in the workspace. iproute2 (GPL) is a
  test tool the implementation's test runs, never a dependency.

## Departures from the plan
- The plan's crate name stays; the shared crate's existing calls (`set_ip_socket_setting`, `set_port_setting`,
  `Shortfalls`, the raw option calls, `receive_timestamping_flags`) are used instead of this track's own, and
  the datagram modules moved there, as the plan said they would.
- The plan's test namespace had loopback only. Linux takes no IPv6 multicast over loopback, drops a looped one
  whose source is ::1 and lets a host join only an anycast address in a prefix of the interface, so the test
  namespace also has a dummy interface; it has the route, rule, qdisc and sysctl the owner, pacing and routing
  header checks need too (listed at the top of the suite).

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
- There is no way through the interfaces to read the counters of refusals; `counters()` is inherent.
