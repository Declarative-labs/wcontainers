# Status of the host-udp-sockets track

Plan: docs/plan-host-udp-sockets.md. Branch: main, checkout /Users/thomaseaton/Documents/GitHub/wcontainers.

## For the other socket tracks (TCP, ICMP and raw)
- This track added the datagram half to the shared crate `crates/internet-protocol/implementation/linux-socket-options`,
  as `pub mod datagram`:
  - `datagram::send`: `SendState` (per socket) writes a route's selectors (`apply_route`), a send's header
    (`apply_header`) and its `IpSendFields` (`apply_send_fields`) as socket options, each written only when it
    differs from what the socket last had (`datagram::sticky`), and as `sendmsg` control messages (`Control`).
    It keeps `SO_TIMESTAMPING` itself (`set_receive_timestamps`).
  - `datagram::receive`: `receive` (one `recvmsg`), `Ancillary::parse`, `packet_fields`, `transmit_report`, and
    `Headers`, `ipv4_header`, `push_ipv6_headers`, `udp_checksum` to write a packet whole from its IP header.
    `udp_checksum` returns the checksum as a number, as etherparse's header fields hold it: write it with
    `to_be_bytes()` (this track wrote `to_ne_bytes()` until run 3, so every written checksum was byte-swapped).
  - `datagram::memberships`: `set_multicast_group` (join, then the whole source filter with `MCAST_MSFILTER`;
    leave; a join on no interface is made on the interface Linux would choose) and `AnycastMemberships`.
  - `datagram::addresses`: socket address conversions.
- Run 3 added to `implementation.rs`: `bind_address` (the address of a binding, `IPV6_V6ONLY` set; `bind` uses it),
  `host_port_range`, and `bind_chosen_by_exit_node`, which binds at once to a port of a `PortChoice` from any
  range: the kernel chooses (`IP_LOCAL_PORT_RANGE`) in the part of a range inside the host's
  `net.ipv4.ip_local_port_range`, and the ports outside it are tried one at a time (counted,
  `PORTS_OUTSIDE_THE_HOSTS_RANGE`). TCP track: Linux ignores a socket's `IP_LOCAL_PORT_RANGE` bound that lies
  outside the host's range and takes the host's bound instead, so `in_port_ranges` (and `bind(.., true)`) can
  give a port outside the allowed range for a range the host's range does not hold; a listener could use
  `bind_chosen_by_exit_node`.
- Run 3 made `datagram/receive.rs` and `datagram/send.rs` build for `aarch64-unknown-linux-musl` (the Mac lints
  them clean for musl now), and the UDP crate uses the shared `icmp_error::write_icmp_error` (its own copy is gone).
- ICMP track: your uncommitted `datagram/exchange.rs` repeats `TRANSMIT_TIME_REJECTION_WITHOUT_KEY` and
  `ERROR_OF_NO_REPORT` of the UDP crate (the literal check reports both). Once it is committed, the UDP crate
  will import them from there and drop its copies. Two things the UDP crate does that `exchange` could take:
  receive reads the error queue first only while a send's reports may wait, and otherwise only when the normal
  receive fails (an ICMP error sets the socket error, which fails the next normal receive), so a receive is one
  system call; and see "Transmit time rejections" below for the key map it needs.

## Milestone U1: done (run 3 fixed every finding of the second independent check)
- Crate `crates/udp-sockets/implementation/linux-host` (package `wcontainers-udp-sockets-linux-host`, type
  `LinuxHostUdpSockets`, socket `LinuxHostUdpSocket`), body `cfg(target_os = "linux")`. Refusals, host cuts and
  slow paths counted per reason (`counters()`), warned once on standard error.
- Suite `crates/udp-sockets/conformance/conformance.rs`: 47 checks against the traits alone. `set` sets every
  setting twice and asserts both answers are equal; the Configurable and NegativeRouteAdvice suites run on a
  socket too. Every received UDP and UDP-Lite checksum, IPv4 header checksum and ICMP checksum is verified.
  The network the suite needs beyond loopback is listed at the top of the file; the implementation's test makes it.
- The implementation's own test `a_buffer_the_host_cuts_is_counted` (in `tests/conformance.rs`).
- Last run: 48 of 48 pass on the ARM64 machine; 30 runs in a row at 12 threads, none failing (28 s).

## How to run
- On the ARM64 machine (root):
  `arm-run /Users/thomaseaton/Documents/GitHub/wcontainers --name host-udp-sockets -- 'crates/udp-sockets/implementation/linux-host/conformance.sh 12'`
  (the argument is the test threads; further arguments filter tests by name). It builds as the user and runs the
  test binary with `sudo -n`. The test makes 64 network namespaces at the first test's start, while every other
  test waits before opening a socket (a child process holds its parent's descriptors until it starts its
  program, which kept closed sockets alive: the flaky failures of run 2), with `ip -batch` and `tc` (test tools,
  never dependencies); each test then enters one of its own.
- Lint: `cargo clippy -p wcontainers-internet-protocol-linux-socket-options -p wcontainers-udp-sockets-linux-host -p wcontainers-udp-sockets-conformance --tests`
  there and, for musl, on the Mac with `--target aarch64-unknown-linux-musl`. Both clean for these crates
  (`-D warnings` stops at two warnings of the reviewed `crates/kernel/interface.rs`).
- Independent check: pyro-parked/wcontainers-scratch/host-udp-sockets/independent-check.sh, passed as the arm-run
  command (`-- "$(cat .../independent-check.sh)"`): a clean build and run, 19 broken copies of the implementation,
  and cargo tree of the interface crates. Log of run 3: independent-check-run3.log beside it.
- No random tests: every check is a fixed scenario.

## Measurements
- 2026-11-21, ARM64 machine (96 cores): the 48 tests take 0.86 to 0.87 s at 12 threads, load 3.4 to 10.5
  (making the 64 networks is part of it); a clean build and run took 11 s at a load of 10.0 to 10.5; 30 runs
  took 28 s at a load of 2.8 to 8.1.
- Pacing on the machine's loopback with fq, 5 datagrams of 1000 bytes at 50 000 bytes per second: arrivals at
  0.1, 20.9, 41.7, 62.6 and 83.4 ms (Python, run 2); the check asks for at least 80 ms.
- IPv6 path MTU modes over a local route of MTU 1500 (Python, run 3): Dont and Want fragment (largest fragment
  1496: 1448 bytes of data, a multiple of 8, and 48 of headers), Do answers EMSGSIZE, Probe, Interface and Omit
  send 3000 bytes whole.

## Independent check (run 3, 2026-11-21, ARM64 machine, load 10.0 to 12.8, 54 s)
- Clean build (fresh target folder): 48 of 48 pass.
- 19 broken copies, each caught by the check named:
  - count_unsent_bytes always zero: pacing_holds_datagrams_in_the_send_buffer.
  - close keeping the descriptor open: close_frees_the_port and three more.
  - disconnect without `connect(AF_UNSPEC)`: disconnect_keeps_the_binding.
  - read_incoming_processor always none: the_incoming_processor_is_read_back.
  - negative route advice refusing a connected socket: negative_route_advice_keeps_a_socket_sending,
    negative_route_advice_contract_holds.
  - connect without `connect`: a_connected_socket_takes_only_its_peers_datagrams, disconnect_keeps_the_binding.
  - checksums in host byte order: 22 checks.
  - receive not reading the error queue after a failed receive: a_closed_port_answers_with_an_icmp_error.
  - multicast loopback ignored: both multicast membership checks. All-groups ignored:
    all_groups_decides_whether_other_sockets_memberships_count. Group filter detach ignored:
    a_group_filter_chooses_the_socket. Anycast leave ignored: ipv6_memberships_take_their_datagrams. Source
    filters ignored: multicast_membership_takes_the_groups_datagrams.
  - link scope ignored: link_scope_keeps_a_datagram_on_its_link. Every path MTU mode as Want:
    path_mtu_discovery_modes_take_the_routes_mtu. Transmit time not sent:
    a_transmit_time_holds_a_datagram_until_then. Dual stack forced to IPv6 only: a_dual_stack_socket_takes_ipv4.
    Reserved ports ignored: both exit node checks (the reserved range is 20 ports wide now).
- cargo tree (normal, build and dev edges) of wcontainers-udp-sockets, -internet-protocol, -configurable,
  -negative-route-advice, -kernel and -udp-sockets-conformance: no implementation crate.
- No reviewed file changed: none of this track's commits touches an `interface.rs` or a data file beside one.
- Which check fails when a method breaks: open (every check; owner: an_owner_is_routed_as_its_user), bind
  (bind_returns_the_bound_address, the sharing checks, both exit node checks, a_dual_stack_socket_takes_ipv4),
  connect (a_connected_socket_takes_only_its_peers_datagrams), disconnect (disconnect_keeps_the_binding),
  send_to (every check that sends; each header and send field by its own check), receive (every check that
  receives; a_short_buffer_keeps_the_whole_length, receive_with_nothing_queued_would_block,
  a_closed_port_answers_with_an_icmp_error, transmit_reports_carry_the_key_of_their_send), count_unsent_bytes
  (pacing_holds_datagrams_in_the_send_buffer, sending_settings_are_taken), read_incoming_processor
  (the_incoming_processor_is_read_back), negative route advice (negative_route_advice_keeps_a_socket_sending,
  negative_route_advice_contract_holds), close (close_frees_the_port), configure (each setting's check, and the
  two Configurable contract checks).

## Honoured, and how each is checked
- open (both versions, both protocols, the owner by routing), bind (kernel port, a taken port, the exit node's
  choice around a wide reserved range and from a range below the host's own, IPv6 only and dual stack),
  connect, disconnect (explicit port kept, a kernel-chosen port given up, as Linux does), send_to, receive,
  count_unsent_bytes, read_incoming_processor, negative route advice, close.
- Header fields checked by what arrives or what is refused: TTL, hop limit, DSCP, ECN, flow label, IPv4 options,
  IPv6 hop-by-hop, destination and routing destination options, segment routing header, source address, mark,
  interface; link scope (`ENETUNREACH` to an address behind a gateway, universe sends); every path MTU discovery
  mode over a route of MTU 1500 (Want and Dont fragment, Do is too long with that MTU, Probe, Interface and Omit
  send whole by the interface's MTU), IPv4 and IPv6; fragment MTU with fragmentation forbidden.
  Send fields: priority (received), transmit time (held at least 200 ms by fq), transmit reports scheduled and
  software with key and packet or not.
- Datagram fields: checksum computed (valid and not zero on arrival), omitted, UDP-Lite partial coverage against
  the receiver's minimum; segment size.
- Received fields: interface, specific destination, largest fragment (within 8 bytes of the route MTU when
  fragmented, none when whole), mark, priority, software timestamps, no hardware timestamp on loopback, drops
  since previous, segment size.
- Settings checked by their effect: address and port sharing, nonlocal addresses, bound interface, incoming
  processor, IPv4 multicast (join, loopback off and on, source filters Include and Exclude with sources, leave)
  over the interface the multicast route chooses, IPv6 multicast (join on the chosen interface and on one given,
  leave, loopback off and on), all-groups on and off for both versions (a socket on the port that did not join
  takes the group's datagrams only while it is on), IPv6 anycast join and leave, the port group's filter
  (choosing the second socket, and detached: some of 32 senders reach the first), receive buffer, send buffer,
  maximum pacing rate, timestamping, receive coalescing, zero checksum receipt, minimum checksum coverage.
- ICMP errors on the error queue as whole ICMP packets with valid checksums (port unreachable, IPv4 and IPv6).

## Not honoured or not checkable, with the Linux reason
- Wireless status reports: Linux reports them (`SO_WIFI_STATUS`) with no key of the send; a send asking for one
  answers `NotSupported`, counted.
- Acknowledgement reports: Linux makes them only for TCP. Completion and hardware transmit reports, and
  `link_layer_length` (Linux gives the frame length only in `SCM_TIMESTAMPING_PKTINFO`, which comes only with a
  hardware receive timestamp), need a driver with a hardware clock; loopback has none, so none is checked.
- Neighbour confirmation (`MSG_CONFIRM`): taken, but its only effect is the neighbour entry's confirmed time,
  which no socket can read; loopback and the dummy interface have no neighbour entries.
- Negative route advice has no system call: a connected socket is connected again to its peer, which drops its
  cached route and takes a new transmit hash. Its effect cannot be seen through the traits: Linux uses that hash
  for the automatic IPv6 flow label, which the contract replaces by the label each send gives (measured: label 0
  before and after), and UDP's multipath hash is of addresses and ports. An expired IPv6 exception route is not
  removed from the table.
- Transmit time rejections (`SO_EE_ORIGIN_TXTIME`): Linux gives the rejected packet's time, not the send's key;
  such an entry is dropped, counted and warned. Not yet done: a per-socket map from each send's transmit time to
  its key, filled when the send asks for errors, would give the key. Left because the ICMP track is moving the
  error queue handling into the shared `datagram::exchange` right now (uncommitted), where the map belongs, and
  it needs a rule for dropping times never rejected and an ETF qdisc to check it.
- Header fields Linux never gives a UDP socket: IPv4 identification and flags (written as zero and none), the
  received UDP checksum (recomputed), the sender's UDP-Lite coverage (written as zero, the whole datagram).
- In an ICMP error: the quoted IPv4 TTL and UDP checksum are zero, and the quoted IP total length and UDP length
  are those of the part quoted, not of the datagram sent (Linux gives only the quoted payload, after the headers
  it strips, so a datagram longer than the quote shows shorter lengths there).
- An error queue entry longer than the buffer reports only the part read: Linux's error queue gives no length.
- A transmit report without its packet has interface 0: Linux gives the interface only with the packet.
- A send whose source port is not the socket's, and a connect whose source is not the bound address: Linux sends
  from the bound port and address only; `NotSupported`, counted. A socket bound to a specific address is refused
  before connecting; one bound to the unspecified address is connected, and if Linux chose another source it is
  put back as it was (its old peer, or none, with its port).
- Send and receive buffer sizes above the host's `wmem_max`/`rmem_max` are cut by the host; taken, counted and
  warned.
- Routing headers: Linux takes only segment routing (type 4) and, with mobile IPv6 built in, type 2.

## Dependencies and licences
- rustix 1.1.6 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT; the suite uses its clock), linux-raw-sys
  0.12.1 (same), libc 0.2.191 (MIT OR Apache-2.0), etherparse 0.21.0 (MIT OR Apache-2.0); all already in the
  workspace. The suite also uses wcontainers-configurable-conformance and
  wcontainers-negative-route-advice-conformance. iproute2 (GPL) is a test tool the implementation's test runs,
  never a dependency.

## Departures from the plan
- The plan's crate name stays; the shared crate's calls are used instead of this track's own (opening as the
  owner, binding, ICMP errors), and the datagram modules moved there, as the plan said they would.
- The plan's test namespace had loopback only. The test network now also has a dummy interface (IPv4 and IPv6
  multicast, 192.0.2.3/24, a gateway route), routes of MTU 1500 to 127.0.0.2 and 2001:db8::2, a user rule, an
  fq qdisc and a segment-routing sysctl, all listed at the top of the suite. Reasons: Linux takes no IPv6
  multicast over loopback; its loopback interface brings back every multicast datagram, so IPv4 multicast
  loopback can only be seen on another interface; and the path MTU modes differ only where a route's MTU is
  below the interface's.
- A receive no longer reads the error queue every time (see the note for the ICMP track): run speed, one system
  call per datagram.

## For the owner
- The interface has no constructor: proposed `trait StandaloneUdpSockets: UdpSockets { fn new() -> Self; }`;
  meanwhile `LinuxHostUdpSockets::new()`.
- `UDPLITE_SEND_CSCOV`, `UDPLITE_RECV_CSCOV` and `SO_EE_ORIGIN_TXTIME` are in no established crate. The kernel
  implementation keeps private copies in `crates/kernel/implementation/linux/linux_extra.rs`; the shared crate
  has `UDP_LITE_SEND_COVERAGE`, `UDP_LITE_RECEIVE_COVERAGE` and `SO_EE_ORIGIN_TXTIME`. One of the two should
  import the other.
- `TransmitReport.interface` cannot be filled for a report without its packet: an `Option<InterfaceIndex>` would
  say so; this implementation writes 0.
- There is no way through the interfaces to read the counters of refusals; `counters()` is inherent.
