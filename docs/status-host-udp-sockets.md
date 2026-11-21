# Status of the host-udp-sockets track

Plan: docs/plan-host-udp-sockets.md. Branch: main, checkout /Users/thomaseaton/Documents/GitHub/wcontainers.

## For the other socket tracks (TCP, ICMP and raw)
- This track added the datagram half to the shared crate `crates/internet-protocol/implementation/linux-socket-options`,
  as `pub mod datagram`:
  - `datagram::send`: `SendState` (per socket) writes a route's selectors (`apply_route`), a send's header
    (`apply_header`) and its `IpSendFields` (`apply_send_fields`) as socket options, each written only when it
    differs from what the socket last had (`datagram::sticky`), and as `sendmsg` control messages (`Control`).
    It keeps `SO_TIMESTAMPING` itself (`set_receive_timestamps`).
  - `datagram::receive`: `receive` (one `recvmsg`), `receive_with_tail` (the same, with what does not fit the
    buffer going on into a second one), `Ancillary::parse`, `packet_fields`, `transmit_report`, and `Headers`,
    `ipv4_header`, `push_ipv6_headers`, `udp_checksum` to write a packet whole from its IP header.
    `udp_checksum` takes the payload in parts and returns the checksum as etherparse's header fields hold it.
  - `datagram::memberships`: `set_multicast_group` (join, then the whole source filter with `MCAST_MSFILTER`;
    leave; a join on no interface is made on the interface Linux would choose) and `AnycastMemberships`.
  - `datagram::addresses`: socket address conversions.
- `implementation.rs`: `bind_address`, `host_port_range`, and `bind_chosen_by_exit_node`, which binds at once to
  a port of a `PortChoice` from any range (ports outside the host's `net.ipv4.ip_local_port_range` are tried
  one at a time, counted, `PORTS_OUTSIDE_THE_HOSTS_RANGE`).
- Run 4 added to the ICMP track's `datagram::exchange` (the UDP crate now uses `send_message` and `ErrorQueue`,
  its own copies are gone): `ErrorQueue::expect_transmit_time_errors` notes a send that asked for transmit time
  errors; `ErrorQueue::received` is `error_entry` with a transmit time rejection given its send's key and
  reason (`TransmitTimeRejected`), and the packet if the send asked; `ErrorQueue::settle` reads the error queue
  into the kept entries once the host holds none of the socket's sends (`SO_MEMINFO`), and forgets the noted
  sends with no rejection there; `holds_entries`. ICMP and raw tracks: switching from `error_entry` to
  `ErrorQueue::received` gives your sockets the same.

## Milestone U1: done (run 4 fixed every finding of the third independent check)
- Crate `crates/udp-sockets/implementation/linux-host` (package `wcontainers-udp-sockets-linux-host`, type
  `LinuxHostUdpSockets`, socket `LinuxHostUdpSocket`), body `cfg(target_os = "linux")`. Refusals, host cuts and
  slow paths counted per reason (`counters()`), warned once on standard error.
- Suite `crates/udp-sockets/conformance/conformance.rs`: 52 checks against the traits alone. `set` sets every
  setting twice and asserts both answers are equal; the Configurable and NegativeRouteAdvice suites run on a
  socket too. Every received UDP and UDP-Lite checksum, IPv4 header checksum and ICMP checksum is verified.
  The network the suite needs beyond loopback is listed at the top of the file; the implementation's test makes it.
- The implementation's own test `a_buffer_the_host_cuts_is_counted` (in `tests/conformance.rs`).
- Last run: 53 of 53 pass on the ARM64 machine, and 30 runs in a row.

## How to run
- On the ARM64 machine (root):
  `arm-run /Users/thomaseaton/Documents/GitHub/wcontainers --name host-udp-sockets -- 'crates/udp-sockets/implementation/linux-host/conformance.sh 12'`
  (the argument is the test threads; further arguments filter tests by name). It builds as the user and runs the
  test binary with `sudo -n`. The test makes 64 network namespaces at the first test's start, while every other
  test waits before opening a socket (a child process holds its parent's descriptors until it starts its
  program, which kept closed sockets alive), with `ip -batch` and `tc` (test tools, never dependencies); each
  test then enters one of its own.
- Lint: `cargo clippy -p wcontainers-internet-protocol-linux-socket-options -p wcontainers-udp-sockets-linux-host -p wcontainers-udp-sockets-conformance --tests`
  there and, for musl, on the Mac with `--target aarch64-unknown-linux-musl`. Both clean for these crates
  (`-D warnings` stops at two warnings of the reviewed `crates/kernel/interface.rs`).
- Independent check: pyro-parked/wcontainers-scratch/host-udp-sockets/independent-check.sh, passed as the arm-run
  command (`-- "$(cat .../independent-check.sh)"`): a clean build and run, 32 broken copies of the implementation
  and of the shared crate's send and exchange files, and cargo tree of the interface crates. Log of run 4:
  independent-check-run4.log beside it.
- No random tests: every check is a fixed scenario.

## Measurements
- 2026-11-21, ARM64 machine (96 cores): the 53 tests take 0.88 s at 12 threads, load 1.1 (making the 64
  networks is part of it). Run 3: 48 tests in 0.86 to 0.87 s at load 3.4 to 10.5; 30 runs in 28 s.
- Pacing on the machine's loopback with fq, 5 datagrams of 1000 bytes at 50 000 bytes per second: arrivals at
  0.1, 20.9, 41.7, 62.6 and 83.4 ms (Python, run 2); the check asks for at least 80 ms, and less than 80 ms
  once the rate is unset.
- IPv6 path MTU modes over a local route of MTU 1500 (Python, run 3): Dont and Want fragment (largest fragment
  1496), Do answers EMSGSIZE, Probe, Interface and Omit send 3000 bytes whole.
- etf on a dummy interface, clock TAI (Python, run 4, load 3.6 to 8.7): with a delta of 0 every future time is
  missed (ECANCELED, code 2, after its time) and the send succeeds; a past time, another clock (realtime,
  monotonic) or a socket in deadline mode is invalid at once (EINVAL, code 1) and the send answers ENOBUFS;
  with a delta of 100 000 ns nothing is reported. The entry gives the time as data high and information low.
- Source preferences (Python, run 4): with `use_tempaddr` 2 and 2001:db8:1::1/64 mngtmpaddr, connect to
  2001:db8:1::99 takes the temporary address by default and with temporary, the public one with public; home
  and care-of change nothing, even with an address marked home.

## Independent check (run 4, 2026-11-21, ARM64 machine, load 3.4 to 7.5, 79 s)
- Clean build (fresh target folder, 12 s): 53 of 53 pass. 30 runs in a row at 12 threads: none failing (31 s,
  load up to 24.2).
- 32 broken copies of the implementation and of the shared crate's send and exchange files. 30 are each caught
  by a named check (run 3's 19, still caught, and this run's: a cut datagram checksummed over the cut part,
  source preferences, local fragmentation, a bound interface, transmit time errors, deadline mode, the realtime
  clock given as the atomic one, a rejection without its key, the IPv6 source, connect ignoring its selectors,
  pacing unset, minimum coverage unset). The other two pass, as expected: negative route advice doing nothing
  and IPv4 checksum omission ignored, whose effects cannot be seen (below).
- cargo tree (normal, build and dev edges) of wcontainers-udp-sockets, -internet-protocol, -configurable,
  -negative-route-advice, -kernel and -udp-sockets-conformance: no implementation crate.
- No reviewed file changed: none of this track's commits touches an `interface.rs` or a data file beside one.

## Which check fails when a method breaks
- open: every check (owner: an_owner_is_routed_as_its_user). bind: bind_returns_the_bound_address, the sharing
  checks, both exit node checks, a_dual_stack_socket_takes_ipv4. connect: a_connected_socket_takes_only_its_peers_datagrams,
  and connect_takes_its_route_selectors for its selectors. disconnect: disconnect_keeps_the_binding. send_to:
  every check that sends; each header and send field by its own check. receive: every check that receives;
  a_short_buffer_keeps_the_whole_length (cut in the UDP header and in the payload), receive_with_nothing_queued_would_block,
  a_closed_port_answers_with_an_icmp_error, transmit_reports_carry_the_key_of_their_send,
  rejected_transmit_times_are_reported_with_their_keys. count_unsent_bytes: pacing_holds_datagrams_in_the_send_buffer,
  sending_settings_are_taken. read_incoming_processor: the_incoming_processor_is_read_back. close:
  close_frees_the_port. configure: each setting's check, and the two Configurable contract checks.
- give_negative_route_advice: negative_route_advice_keeps_a_socket_sending and negative_route_advice_contract_holds
  catch only a refusal. One that does nothing and answers Ok passes the suite: its effect cannot be seen
  through the traits (see below).

## Honoured, and how each is checked
- open (both versions, both protocols, the owner by routing), bind (kernel port, a taken port, the exit node's
  choice around a wide reserved range and from a range below the host's own, IPv6 only and dual stack),
  connect (with its selectors), disconnect (explicit port kept, a kernel-chosen port given up, as Linux does),
  send_to, receive, count_unsent_bytes, read_incoming_processor, negative route advice, close.
- Header fields checked by what arrives or what is refused: TTL, hop limit, DSCP, ECN, flow label, IPv4 options,
  IPv6 hop-by-hop, destination and routing destination options, segment routing header, IPv4 and IPv6 source
  address, mark, interface; link scope (`ENETUNREACH` to an address behind a gateway, universe sends); every
  path MTU discovery mode over a route of MTU 1500 (Want and Dont fragment, Do is too long with that MTU, Probe,
  Interface and Omit send whole), IPv4 and IPv6; local fragmentation forbidden makes a Want datagram over the
  route's MTU too long (MTU 1500), allowed fragments it; fragment MTU with Do.
- Route selectors through connect, by the local address it returns: IPv6 source preferences (public gives the
  public address, temporary the temporary one, the system default the host's choice, temporary), and an IPv4
  interface given (that interface's address, not the route's).
- Send fields: priority (received), transmit time (held at least 200 ms by fq on the monotonic clock),
  transmit time errors (etf on the atomic clock: a past time, the realtime clock and deadline mode are
  reported invalid, a missed time missed with its packet, each with its send's key; a send not asking gets no
  report), transmit reports scheduled and software with key and packet or not.
- Datagram fields: checksum computed (valid and not zero on arrival), omitted over IPv6, UDP-Lite partial
  coverage against the receiver's minimum; segment size.
- Received fields: interface, specific destination, largest fragment, mark, priority, software timestamps, no
  hardware timestamp on loopback, drops since previous, segment size; a packet cut by a short buffer keeps the
  whole length and the bytes of the whole packet, checksum included.
- Settings checked by their effect: address and port sharing, nonlocal addresses, bound interface (sends over
  it; a socket bound to another interface takes no loopback datagram until unbound), incoming processor, IPv4
  multicast (join, loopback off and on, source filters Include and Exclude with sources, leave), IPv6 multicast
  (join on the chosen interface and on one given, leave, loopback off and on), all-groups on and off for both
  versions, IPv6 anycast join and leave, the port group's filter (choosing and detached), receive buffer, send
  buffer, maximum pacing rate (set and unset), timestamping, receive coalescing, zero checksum receipt, minimum
  checksum coverage (set, and none: any coverage arrives).
- ICMP errors on the error queue as whole ICMP packets with valid checksums (port unreachable, IPv4 and IPv6).

## Not honoured or not checkable, with the Linux reason
- Mobile IPv6 source preferences (home, care-of): written (`IPV6_PREFER_SRC_HOME`, `_COA`), but Linux applies
  them only in source address rule 4, which it builds only with mobile IPv6 built in; the machine's kernel has
  `CONFIG_IPV6_MIP6=m`, and an address marked home is chosen the same with either (measured).
- IPv4 checksum omission (`SO_NO_CHECK`): written, but not checkable through a host receiver: Linux gives a UDP
  socket no received checksum, so a receiver writes the computed one. IPv6 omission is checked, because Linux
  drops a zero checksum unless the receiver accepts it.
- Wireless status reports: Linux reports them (`SO_WIFI_STATUS`) with no key of the send; a send asking for one
  answers `NotSupported`, counted.
- Acknowledgement reports: Linux makes them only for TCP. Completion and hardware transmit reports, and
  `link_layer_length` (Linux gives the frame length only in `SCM_TIMESTAMPING_PKTINFO`, which comes only with a
  hardware receive timestamp), need a driver with a hardware clock; loopback has none, so none is checked.
- Neighbour confirmation (`MSG_CONFIRM`): taken, but its only effect is the neighbour entry's confirmed time,
  which no socket can read; loopback and the dummy interfaces have no neighbour entries.
- Negative route advice has no system call: a connected socket is connected again to its peer, which drops its
  cached route and takes a new transmit hash. Its effect cannot be seen through the traits: Linux uses that hash
  for the automatic IPv6 flow label, which the contract replaces by the label each send gives (measured: label 0
  before and after), and UDP's multipath hash is of addresses and ports.
- Transmit time rejections: a rejection Linux makes at once also fails the send with ENOBUFS, as Linux does for
  a socket with `IP_RECVERR`; the report follows on the error queue. A rejection of a time no noted send gave
  (a send that did not ask, on a socket whose last send did) is dropped, counted and warned.
- Header fields Linux never gives a UDP socket: IPv4 identification and flags (written as zero and none), the
  received UDP checksum (recomputed), the sender's UDP-Lite coverage (written as zero, the whole datagram).
- In an ICMP error: the quoted IPv4 TTL and UDP checksum are zero, and the quoted IP total length and UDP length
  are those of the part quoted (Linux gives only the quoted payload, after the headers it strips).
- An error queue entry longer than the buffer reports only the part read: Linux's error queue gives no length.
- A transmit report without its packet has interface 0: Linux gives the interface only with the packet.
- A send whose source port is not the socket's, and a connect whose source is not the bound address: Linux sends
  from the bound port and address only; `NotSupported`, counted.
- Send and receive buffer sizes above the host's `wmem_max`/`rmem_max` are cut by the host; taken, counted and
  warned.
- Routing headers: Linux takes only segment routing (type 4) and, with mobile IPv6 built in, type 2.

## Dependencies and licences
- rustix 1.1.6 (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT; the suite uses its clocks, with its
  `linux_4_11` feature for the atomic clock), linux-raw-sys 0.12.1 (same), libc 0.2.191 (MIT OR Apache-2.0),
  etherparse 0.21.0 (MIT OR Apache-2.0); all already in the workspace. The suite also uses
  wcontainers-configurable-conformance and wcontainers-negative-route-advice-conformance. iproute2 (GPL) is a
  test tool the implementation's test runs, never a dependency.

## Departures from the plan
- The plan's crate name stays; the shared crate's calls are used instead of this track's own (opening as the
  owner, binding, ICMP errors, sending, the error queue), and the datagram modules moved there.
- The plan's test namespace had loopback only. It now also has two dummy interfaces: multicast0 (IPv4 and IPv6
  multicast, 192.0.2.3/24, a gateway route) and scheduled0 (203.0.113.1/24, 2001:db8:1::1/64 with a temporary
  address, an etf qdisc of delta 0), routes of MTU 1500 to 127.0.0.2 and 2001:db8::2, a user rule, an fq qdisc on
  loopback and a segment-routing sysctl, all listed at the top of the suite. Reasons: Linux takes no IPv6
  multicast over loopback and its loopback interface brings back every multicast datagram; path MTU modes
  differ only where a route's MTU is below the interface's; transmit time errors need a qdisc that keeps a
  schedule; source preferences need a public and a temporary address.
- A receive reads the error queue first only while a send's reports or entries may wait, and otherwise only
  after a normal receive fails: run speed, one system call per datagram. While reports may wait, it also reads
  `SO_MEMINFO` to learn when none can still come.
- A receive takes what does not fit its buffer into a 64 KiB tail on the stack (uninitialised, a second
  `iovec`), so that a cut datagram's checksum is the whole datagram's.

## For the owner
- The interface has no constructor: proposed `trait StandaloneUdpSockets: UdpSockets { fn new() -> Self; }`;
  meanwhile `LinuxHostUdpSockets::new()`.
- `UdpSocketSetting::MinimumChecksumCoverage(None)` is not defined. Example: Linux's `UDPLITE_RECV_CSCOV` of 0
  means full coverage required (a datagram of coverage 8 is then dropped), while a socket never set takes any
  coverage. This implementation reads None as no minimum (it writes 8, the UDP header, which takes every valid
  coverage), so a guest's "full coverage required" cannot be said through the contract.
- `UDPLITE_SEND_CSCOV`, `UDPLITE_RECV_CSCOV`, `SO_EE_ORIGIN_TXTIME` and the two `SO_EE_CODE_TXTIME_*` are in no
  established crate. The kernel implementation keeps private copies in
  `crates/kernel/implementation/linux/linux_extra.rs`; the shared crate has its own. One should import the other.
- `TransmitReport.interface` cannot be filled for a report without its packet: an `Option<InterfaceIndex>` would
  say so; this implementation writes 0.
- There is no way through the interfaces to read the counters of refusals; `counters()` is inherent.
