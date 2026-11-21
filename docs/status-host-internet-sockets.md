# Status: host internet sockets

Plan: docs/plan-host-internet-sockets.md. Milestones: I1 route resolver and the two generic suites (done), I2 ICMP
and raw sockets (done, redone in run 1 after the reorganization and the async ruling of 2026-11-21, below).
Nothing of this track is in progress.

## I2 after the async ruling (2026-11-21, run 1): done
- `crates/icmp-sockets/implementation/host` (package `wcontainers-icmp-sockets-host`, type `HostIcmpSockets`,
  socket `HostIcmpSocket`) and `crates/raw-sockets/implementation/host` (package `wcontainers-raw-sockets-host`,
  type `HostRawSockets`, socket `HostRawSocket`), folders moved with `git mv` from `linux-ping` and `linux-host`.
- `wait_for_events` on the shared crate's `readiness` (polling 3.11.0 over epoll, which the UDP track added and
  TCP uses too). polling rather than mio because it registers a socket of any kind, raw ones included, on every
  system. A socket is a `Watched` whose drop deletes the registration before its descriptor closes; a waiting
  call allocates nothing once the socket's waiter list has grown and withdraws itself when dropped; the one
  place a wake is fired is `readiness::wake`. Error queue entries a too long send read off the host while it
  looked for its own are held as an error condition in that readiness until a receive takes them.
- socket2 makes, binds and sends (the shared crate's `open_socket`, `bind_to`, `send_message`) and sets
  `SO_REUSEADDR`, `IP_RECVTOS`, `IPV6_RECVTCLASS`, `IPV6_RECVHOPLIMIT`, `IP_HDRINCL` and `IPV6_HDRINCL`; the rest,
  which socket2 lacks, through the shared crate's option calls on the same descriptor.
- A send that asks for transmit time errors is noted in the shared `ErrorQueue`, so a rejected time comes back
  as a transmit report with its send's key (before, it was counted and dropped).
- The stopped agent's uncommitted work (counters for received and ICMP error fields Linux does not give, more
  raw and ICMP checks) is applied and finished. One of its checks was wrong for Linux: an IPv6 raw socket bound
  to no address takes every multicast packet of its protocol whatever its all-groups setting (raw_v6_match
  delivers to an unbound socket before any membership test), so the all-groups effect is checked for IPv4 only.
- New checks in both suites, polled by hand with a waker that counts its wakes (no executor): a wait for data is
  pending until the reply or packet arrives and then woken once; a wait for writing finishes at its first poll;
  a dropped wait gives up its waker and is not woken; 35 waits on 32 sockets each wake once; an ICMP error ends
  a wait for data with the error condition; an error a too long send took off the host still ends a wait at once,
  and no error is held after a receive took it; rejected transmit times (etf with a delta of 0 on the dummy link)
  come with their keys. The raw crate's own test checks that a refusal is counted under its reason.
- Shared readiness fix (56f2302d): a wait polled again after its events came used to give its waker back before
  it looked, so a later edge could wake it a second time; caught as a flake of
  a_wait_for_data_finishes_when_the_reply_arrives (count 2, in 1 of 3 rounds and 3 of 9 broken ICMP copies). Now it
  looks first and finishes without a waker. 20 rounds of the 12 wait checks after it, no failure; the UDP suite
  (59 of 59) and the TCP suite (90 of 90) pass with it.

## The last independent check's I1 findings, each
- Link-local destination with no interface: measured in a fresh namespace with two dummy links (ARM machine,
  probe4/linklocal.py in scratch): UDP and raw `sendto` to fe80::1 and ff02::1 with no scope succeed (the packet
  takes the routing's choice), `connect` answers EINVAL, and a ping socket's send answers EINVAL. So a route is
  what a send gets; the refusal belongs to connect, which the socket implementations answer. Kept, plan says so,
  and the suite checks it (a_link_local_destination_without_an_interface_routes).
- No check of the route kind refusal: added an_anycast_route_is_not_given_as_another_kind (forwarding on, the
  subnet-router anycast fd01::; an error, or Local as Linux delivers it, passes) and the crate's
  refusals_are_counted_under_their_reasons.
- Source preferences covered nothing: the check now makes a temporary address (`mngtmpaddr` with use_tempaddr 1,
  measured: Linux makes it at once and a connect with IPV6_PREFER_SRC_TMP takes it) and expects it for Temporary
  and the public one for Public. The mobile half stays Home: on this kernel a care-of preference changes nothing
  (CONFIG_IPV6_MIP6 is a module, so the home address rule is not built), measured by the verifier and again here.
- Refusals never shown counted: the resolver, ICMP and raw crates each have a test that reads `counters()`.
- Mobility header arm with no effect: removed; a mobility header selector is refused before asking
  (UPPER_LAYER_PROTOCOL), one netlink round trip less.
- Required source check accepted any error: now it must be a host errno, not a refusal.
- Status wording on anycast: corrected (below, "For the owner").
- Allocations per lookup: the request and reply buffers are kept in the resolver (no allocation once grown); the
  netlink crates' attribute vectors and the second (link) request remain, and confirmations, IPv6 source checks
  and routing headers still open a socket each. Not measured; matters only if a hot path resolves routes.

## How to run
- Every build and test runs on the ARM machine through the orchestrator's `arm-run`; the Mac only edits, formats
  (`nix develop -c rustfmt --edition 2024 <file>`) and commits. Scripts are in
  pyro-parked/wcontainers-scratch/host-internet-sockets, run as
  `./arm-run <checkout> --name host-internet-sockets -- "$(cat <scratch>/<script>)"` from the orchestrator's cloud
  folder:
  - `lint-and-suites.sh`: clippy with warnings denied on the nine packages, then the ICMP, raw and resolver
    suites and the generic self-tests (`ROUNDS=3` before it for three rounds, `FILTER=<name>` to filter).
  - `independent-check-i2b.sh`: clean build with clippy, three rounds, 24 broken copies, cargo tree.
  - `readiness-fix.sh`: the wait checks twenty times, then the UDP and TCP suites.
  - `literals.sh`: the literal check in a throwaway git copy (the synced copy has no .git).
- Test tools put on the path there: iproute2 and nftables from nixpkgs (GPL tools, never dependencies).
- Cargo.lock: other tracks' entries are in the working file, so this track commits a lock built from HEAD's with
  `lock-edit.py` (renames and added dependencies, as cargo orders them) through a private index:
  `GIT_INDEX_FILE=<scratch>/idx git read-tree HEAD`, `git add` this track's paths there, `git update-index
  --cacheinfo 100644,$(git hash-object -w <lock>),Cargo.lock`, commit with the same variable, then
  `git reset -q -- <those paths> Cargo.lock` in the shared index.

## Results (ARM64 machine, Linux 7.0.0-1018-aws, 96 cores, 12 test threads)
- 21:19 to 21:20 UTC, load 1.9, final code: three rounds, each ICMP 45 of 45 (0.61 s), raw 37 of 37 (2.87 s;
  the wait checks' quiet periods), resolver 31 of 31 (0.02 s), Configurable 2 of 2, NegativeRouteAdvice 3 of 3.
- 21:15 UTC, load 4.9 to 5.2: the 6 ICMP and 6 raw wait checks twenty times each, all pass (0.40 s a run).
- Before the readiness fix, 21:12 UTC, load 4.7 to 6.7: the clean build and three rounds took 34 s; round 2 had
  the double wake above.

## Independent check of I2 (this run, 21:11 to 21:14 UTC, load 3.1 to 6.7)
- Clean build in a fresh target folder, clippy with warnings denied clean on the nine packages.
- 24 broken copies, each caught: ICMP open ignoring the owner (owner_selects_the_route), identifier never shared
  (identifiers_are_shared_only_when_both_ask), transmit time errors not noted
  (rejected_transmit_times_are_reported_with_their_keys), reply without its header (both echo reply checks,
  a_short_buffer_keeps_the_whole_length), unsent count zero (pacing_holds_messages_in_the_send_buffer), a wait
  answering at once (five wait checks), kept errors not held (a_wait_sees_an_error_a_send_read_first), close leaking
  (close_frees_the_identifier), timestamps ignored (receive_timestamps_come_only_when_set); raw open writing no
  header for IPPROTO_RAW (an_ipv6_raw_protocol_socket_sends_under_the_asked_protocol), header never included (five
  checks), route protocol not sent (header_included_packets_take_their_route), IPv6 header not written
  (ipv6_packets_arrive_with_their_headers), unsent count zero (ip_settings_are_taken), a wait answering at once (five),
  kept errors not held, transmit time errors not noted, close leaking (close_gives_up_the_sockets_holdings),
  isolation ignored (an_ipv6_router_alert_takes_forwarded_packets), no checksum left on
  (a_checksum_offset_is_written); resolver anycast as unicast (the anycast check and the counted refusals),
  preferences ignored (source_preferences_choose_among_the_link_addresses), source check refused
  (required_source_check_refuses_a_foreign_address, source_selects_the_route), link-local refused
  (a_link_local_destination_without_an_interface_routes). The double wake also showed in three ICMP copies.
- cargo tree (normal, build and dev edges) of all 15 interface crates and 13 conformance crates against the
  workspace's 17 implementation crates: none named (control: the raw implementation's tree names itself and
  the shared crate).
- Reviewed files: the 31 files beside every interface (interface.rs and the data files) have no change in this
  run's commits, against HEAD in the working tree, or since 072dfbea.
- Literal check: no finding in this track's files (20 in all, elsewhere).

## Which check fails when a method breaks
- IcmpSockets: open (every check; owner: owner_selects_the_route), bind_identifier
  (identifiers_are_shared_only_when_both_ask, the_request_carries_the_bound_identifier), send_to (every sending
  check; each header and send field its own, below), receive (echo reply, ICMP error, short buffer, would block,
  transmit report and rejection, drops checks), count_unsent_bytes (pacing_holds_messages_in_the_send_buffer),
  wait_for_events (the six wait checks), close (close_frees_the_identifier), configure (each setting's check and
  Configurable's suite).
- RawSockets: open (every check, open_takes_each_version_and_protocol), send_to (payload, header included,
  reassembly, route selector, path MTU, send field checks), receive (arrival, short buffer, would block, ICMP
  error, router alert checks), count_unsent_bytes (ip_settings_are_taken), wait_for_events (the six wait checks),
  close (close_gives_up_the_sockets_holdings), configure (checksum, router alert, multicast routing, IP and
  multicast settings, Configurable's suite).
- RouteResolver::resolve_route: all 30 suite checks, each selector its own (see the I1 broken copies above and in
  git history of this file).

## Honoured, and how each is checked
- Header fields, by what leaves a dummy link (packet socket) or what a rule does: IPv4 TTL, DSCP, ECN, options,
  source, waived source check, interface, link scope, broadcast permission, mark and owner, ICMP type and code,
  path MTU modes, too long (`MessageTooLong` with the link MTU). IPv6 hop limit, DSCP, ECN, flow label, source
  preferences, local fragmentation and fragment MTU, and for raw sockets source, hop-by-hop, routing destination
  and destination options and a segment routing header.
- Send fields: neighbour confirmation, priority (third band of a prio queue), transmit time (fq holds 300 ms;
  rejected times reported with their keys), transmit reports scheduled and software with key, with and without
  the packet.
- Received fields: interface, specific destination, software timestamps, drops since previous, whole IPv4 and
  IPv6 headers with extension headers; ICMP errors whole.
- Raw only: a packet with its own header leaves byte for byte; its route takes the flow's protocol and mark;
  withdrawn reassembly; checksum offset; IPv4 and IPv6 router alerts with isolation; multicast routing.
- Settings: multicast membership and loopback, IPv4 all-groups, anycast, pacing rate, send and receive buffer,
  timestamping. Every setting kind also passes Configurable's suite.
- Waits: every event poll gives, through the host's poll.

## Not honoured, with the Linux reason
- ICMP over IPv6 (net/ipv6/ping.c), refused with NotSupported, counted and warned: a source address, extension
  headers and routing header, neighbour confirmation.
- Both kinds: an upper-layer selector other than what Linux routes the send by, and a raw payload under another
  protocol than the socket's (IPv4; IPv6 unless the socket's is `IPPROTO_RAW`).
- Ping sockets give no `SO_RXQ_OVFL`, `SO_RCVMARK` or `SO_RCVPRIORITY`: mark and priority are written as 0 and
  counted (RECEIVED_FIELDS_NOT_GIVEN), drops are read from `SO_MEMINFO` at each receive (totals exact, checked).
- ICMP errors: Linux gives no identification or fragment flags of an IPv4 one, nor (under a payload) the quoted
  header's fields but its addresses: written as zero and counted (ICMP_ERROR_FIELDS_NOT_GIVEN).
- Multicast all-groups: no effect to see on a ping socket (it takes only echo replies) or an IPv6 raw socket
  bound to no address (it takes every multicast packet of its protocol).
- Largest fragment and link-layer length: filled by the shared code as for UDP, whose suite checks them.
- Wireless status reports: refused as in the UDP track. Acknowledgement reports are TCP's; hardware and
  completion reports need a device with a hardware clock, which the namespace has not.
- Measured Linux behaviour the suites follow: a raw send into a full send buffer answers `ENOBUFS`;
  `IPV6_DONTFRAG` holds for an IPv6 raw payload of another protocol than UDP or ICMPv6 only in mode `Do`;
  `IPV6_MTU` fragments nothing smaller than the path MTU; IPv6 router alerts reach only `IPPROTO_RAW` sockets.

## For the other socket tracks
- UDP and TCP: the readiness fix above (56f2302d) is in the shared crate; your suites pass with it.
- UDP: the literal check now finds your WAITED_SOCKETS and WAITS_ON_ONE (32 and 3) redefine the ICMP suite's
  public ones; import them from wcontainers-icmp-sockets-conformance, or say which crate should own them.
- TCP: your FLOW_LABEL 0x12345 restates the resolver suite's LABEL, and "lo" its SELF_LINK; say if you want
  LABEL public at the top of the resolver suite.
- The ICMP suite exports its wait tools (`Wakes`, `poll_with`, `finished`, `finished_soon`, `only_asked_for`,
  `READABLE`, `WRITABLE`, `NO_EVENTS`) and the rejected transmit time tools for any suite to use.

## Departures from the plan
- The plan named I2 ICMP and I3 raw; the task made both one milestone.
- The suites share test tools across suite crates (raw on ICMP and resolver, ICMP on resolver) so each fact
  has one definition.
- The generic suites test themselves with private implementors under `#[cfg(test)]`: a named test double would
  go under crates/configurable/implementation/ and crates/negative-route-advice/implementation/, which this
  track does not own, and no other suite needs one.
- The shared readiness (the UDP track's file) was changed here, in a commit of its own, for the double wake.

## For the owner
- No interface can build its implementation. Proposed, as `StandaloneRuntime` does:
  `pub trait StandaloneIcmpSockets: IcmpSockets + Sized { fn open_host() -> Result<Self, HostError>; }`, the same
  for `RawSockets` and `RouteResolver`. Meanwhile `new()` (fallible now: it starts the readiness thread) and
  `open()`.
- The async ruling's `wait_for_events` fits as it stands; nothing found that does not work.
- `ReceivedPacketFields.mark` and `.priority` are plain values, but Linux gives a ping socket neither: an `Option`
  would say so. Example: a reply that arrived with mark 7 reads mark 0.
- `MRT_INIT`/`MRT_DONE` (200, 201, linux/mroute.h and mroute6.h) are in no established crate (libc and
  linux-raw-sys lack them); the raw implementation names them once.
- RouteKind has no anycast: the subnet-router anycast address of a link that forwards (fd01:: with
  net.ipv6.conf.da.forwarding=1) is `anycast ... dev lo table local` and can only be refused (or called Local).
  Without forwarding fd01:: is an ordinary unicast route and resolves as one.
- The contract does not say which error a failed source check gives: Linux's IPv4 lookup gives `ENETUNREACH`,
  bind `EADDRNOTAVAIL`, a send with `IP_PKTINFO` `EINVAL`.
- No way through the interfaces to read the refusal counters; `counters()` is inherent.

## Dependencies (licences checked)
- socket2 0.6.5 (MIT OR Apache-2.0), now named by the ICMP and raw crates too; polling 3.11.0 (Apache-2.0 OR
  MIT) through the shared crate.
- netlink-packet-core 0.9.0, netlink-packet-route 0.34.0, netlink-sys 0.9.0 (MIT); rustix 1.1.6 (Apache-2.0 WITH
  LLVM-exception OR Apache-2.0 OR MIT), linux-raw-sys 0.12.1 (same), libc 0.2.191 (MIT OR Apache-2.0), etherparse
  0.21.0 (MIT OR Apache-2.0). iproute2 and nftables (GPL) are test tools the suites run, never dependencies.
