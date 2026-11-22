# Status: host internet sockets

Plan: docs/plan-host-internet-sockets.md. Milestones: I1 route resolver and the two generic suites (done; run again
on 2026-11-21 evening for the owner's linter and twice on 2026-11-22 for the checks' findings, below), I2 ICMP and raw sockets (redone in run 1 after the
reorganization and the async ruling of 2026-11-21; runs 2 to 5 answered the independent checks' findings; a new run 1
on 2026-11-22 put the ICMP and raw crates in the linter's layout; run 2 cleared the suites' comment and literal
findings and checked itself; run 3 answered the I2 check's four findings, below). I1 run 5 (2026-11-22, after the
restart) conformed the ICMP and raw crates to the owner's interfaces of a00ab267 and checked I1 again. I2 run 4
(2026-11-22) answered that check's five findings; I2 run 5 answered the next check's four; I2 run 6 found and fixed
the ICMP wait flake's cause and answered that check's four minor findings; I2 run 7 answered the next check's three
and cleared 17 literal findings the check script had hidden. Next: the independent check of I2; the linter
findings left all wait on a ruling (below), and the interface selector's meaning on the owner's (For the owner).
C22 run 1 (2026-11-22, below) conformed the three hosts and their suites to the owner's interfaces merged at
158b6c62. Next: the independent check of C22. Nothing of this track is in progress.

## 2026-11-22: C22 run 1 (the owner's ping, raw and shared IP interface changes, merged at 158b6c62)
- Ping `bind(EchoBinding) -> EchoIdentifier` replaces bind_identifier: `SO_REUSEADDR` from the sharing choice
  (socket2), a bind to `sockaddr_in`, or `sockaddr_in6` with the scope as `sin6_scope_id` (0 for none), port the
  requested identifier or 0, then `getsockname` (shared `local_address`) for the identifier bound. An address of the
  other version goes to Linux as its own family, so the answer is Linux's. New ICMP checks, from the probe of
  networking-fixes/ping-raw-udp: the_exit_node_chooses_an_identifier_when_asked (chosen identifier returned,
  carried by the request, and taken: a second exclusive bind of it gets EADDRINUSE);
  a_bound_address_is_the_requests_source (10.3.0.5 and fd03::5); a_link_local_address_binds_the_socket_to_its_link
  (fe80::5 with ib's scope sends to fd08::1 by ib though ia is preferred; a global address and :: with ib's scope
  send by ia); binding_refuses_what_linux_refuses (not local and 224.0.0.1 EADDRNOTAVAIL, an absent scope ENODEV,
  link-local without scope EINVAL and with another link EADDRNOTAVAIL, IPv4-mapped EINVAL, an IPv6 address on an
  IPv4 socket EAFNOSUPPORT and an IPv4 one on an IPv6 socket EINVAL, a second bind EINVAL: all measured here).
  The old checks now want the identifier returned.
- `read_buffer_sizes` (ping and raw) is socket2's `SO_SNDBUF` and `SO_RCVBUF`. New check
  buffer_sizes_are_read_back (ICMP suite, used by the raw suite): the namespace's wmem_default and rmem_default,
  then different sizes asked for each, read back doubled, then twice the limits, read back as twice wmem_max and
  rmem_max. The six lines are in both hosts: the shared crate (the TCP track's) has no read helper; one there would
  serve UDP too.
- Raw multicast routing removed: the setting, its `MRT_INIT`/`MRT_DONE` code and constants, its check and its
  Configurable case. close_gives_up_the_sockets_holdings now joins a group of each version and wants the dummy
  link to leave it once the socket is closed.
- `InterfaceIndex(NonZeroU32)`: the suites build indices with `NonZeroU32::new`; the resolver answers an `RTA_OIF`
  of 0 or none with EPROTO. `TransmitReport.interface` is an Option: check_loopback_reports wants the loopback
  with the packet and None without it, check_rejections wants None for a rejected transmit time. The ICMP counter
  test of the missing report interface is gone with the shared crate's count.
- `SourceFilter::Include(&Slice1)`: the ICMP suite's filtered membership makes it with `Slice1::try_from_slice`,
  an empty include list becoming `Left` (mitsein added to the suite, MIT, the owner's choice; lock commit 183cbcaa).
- `InterfaceSelector` (it arrived in the same merge, f4a31828): the ICMP and raw suites send with both a requested
  and a bound interface (interface_selects_the_link wants both by the second link, then a plain send by the first
  again; the raw route check sends by both). The resolver gives either as `RTA_OIF`, which holds the lookup as a
  bound socket's is held except over IPv6 with a source: there, when the routing socket's lookup fails or leaves by
  another interface, a UDP socket bound to the device (`SO_BINDTOIFINDEX`, socket2 now with its "all" feature) and
  the source, with the owner, mark and traffic class, connects; its error is the answer, a route it finds is
  refused under a new reason (also any such lookup under a rule on the upper layer or one netlink-packet-route
  cannot read). New check a_bound_interface_routes_as_a_bound_socket compares the seven interface cases with a
  socket bound to the device; interface_selects_the_route runs with both selectors. The resolver counter test has
  the new reason (seven reasons).
- `IpPacketSetting::UnicastInterface` and `TrafficClass.ecn` gone: the resolver's helper sockets set
  `IP_UNICAST_IF`/`IPV6_UNICAST_IF` (network byte order) through the shared `set_option`, and the traffic class
  with the ECN bits through socket2's `set_tos_v4`/`set_tclass_v6`. The shared `set_packet_setting` lost its
  version argument and `packet_fields` now answers a Result (the shared crate's commit 04600f02); followed.
- Broken copies (`c22/check.sh`, logs check2.log and final.log in scratch, ARM): the ping scope ignored fails
  a_link_local_address_binds_the_socket_to_its_link and binding_refuses_what_linux_refuses; the chosen identifier
  not read back fails the_exit_node_chooses_an_identifier_when_asked; send and receive buffer sizes swapped fail
  buffer_sizes_are_read_as_linux_gives_them in the ICMP and in the raw suite; the resolver's bound branch skipped
  fails a_bound_interface_routes_as_a_bound_socket and the counter test. A diagnostic copy showed one suite case is
  refused (below).
- Final (`c22/final.sh`, ARM, 19:31 UTC, load 19 to 12): clean build folder, clippy with warnings denied clean on
  the nine packages, the broken copies above, three rounds each ICMP 64 + 4 counter + 1 wait, raw 53 + 2 + 1,
  resolver 38 + 1, Configurable 2, NegativeRouteAdvisable 7, all passing (55 s with the build). After a last rename
  (19:33 UTC, load 6): clippy clean and one round, the same counts. No interface.rs or data file differs from main
  (`git diff main --` of every interface file of this track's crates, internet-protocol, readiness, udp, tcp, kernel).
- Not honoured, new: an IPv6 lookup from a source by a bound interface whose bound socket finds a route the routing
  socket would not give, for example fd0a::5 (main routes it by da at metric 1 and db at metric 2) from fd02::1
  bound to db: a bound socket takes db's route, the routing socket (which holds a lookup with a source to an
  interface only as a preference) takes da's, so the resolver refuses it, counted. Answering it from the probe
  would need the route's kind, hop limit and gateway (for neighbour confirmation) from a connected socket.

## 2026-11-22: I2 run 7 (the check's three findings, and the literal check)
- Mobility header type never really checked (blocking): true. a_mobility_header_is_sent_by_its_type now sends a
  binding update (type 5, length byte 1, 16 bytes) with selector MobilityHeader(5) and wants it out whole with the
  checksum Linux writes; then adds `ip -6 rule add ipproto 135 dport 1280 prohibit` (Linux matches a rule's
  destination port against the type in its first byte, as it does an ICMP type) and wants that send to get EACCES
  (routed by its type), a binding refresh request (type 0) with selector 5 refused or prohibited, and the binding
  update with selector 0 refused or sent. The raw counter test now also sends the binding update with selector 1
  (its length byte) and wants it counted under the upper layer reason. Broken copies (`i2-run7/check.sh`, log
  check.log, ARM, 07:06 UTC, load 2.0 to 2.7): the verifier's offset 1 fails the check and the counter test; a
  mismatched mobility selector passed to Linux fails both too.
- A refused router alert change dropped the old alert (minor): fixed. set_router_alert writes the new alert before
  clearing the old one, except from one IPv6 alert to another, where Linux needs the old gone first and the old is
  written back if the new is refused. an_ipv4_router_alert_takes_forwarded_packets now asks the IPv4 alert socket
  for an IPv6 alert and, when refused, wants the next forwarded packet still taken. Broken copy: the old order
  (clear, then write) fails it.
- Multicast routing off on a socket that never had it (minor): the check only confirmed the reproduction of the
  departure already recorded (Departures); unchanged.
- Literal check: the scripts' "0 findings" of runs since I2 run 2 were false. The check stopped on missing
  linux-raw-sys sources (it needs `cargo fetch` first there) and the script counted its one error line as none.
  With the fetch it found 17 in this track's files: the resolver suite's copies of the ICMP suite's IP, ADDRESS,
  PACKETS, TRAFFIC_CONTROL, DEVICE, the JSON fields and quote, B_MTU 1300 (the ICMP suite's ROUTE_MTU), the resolver
  counter test's "ip" and flow label, both settle_addresses' '/', and the ICMP and raw counter tests' PAD_N and
  PADDED. Fixed: the resolver suite depends on the ICMP suite (manifest and Cargo.lock, commit 47930ab8) and uses its
  batch, number_after, seconds_since_confirmed and temporary_address, re-exporting it as icmp_suite for the
  resolver counter test; PAD_N and PADDED live once in the ICMP suite; LABEL is public in the resolver suite; '/' is
  PREFIX_LENGTH; B_MTU is 1310 (a free value under da's 1400 and over IPv6's 1280). After: none in this track's files.
- Final (`i2-run7/final.sh`, log final.log, ARM, 07:12 UTC, load 6.3 to 7.1): clean build with clippy denied
  warnings clean on the nine packages; three rounds, each ICMP 59, 5 and 1, raw 53, 2 and 1, resolver 37 and 1,
  Configurable 2, NegativeRouteAdvisable 7 (35 s with the build); cargo tree of the 30 interface and conformance
  crates reaches none of the 17 implementations (control 2). No interface.rs or data file changed (`git status`).
  Code commit d0174c58.

## 2026-11-22: I2 run 6 (the ICMP flake found, and the check's four minor findings)
- ICMP suite flake (blocking): found and fixed. The cause is in the checks' setup, not in the host implementation:
  after `ip address add fd03::1/64 dev ia nodad` returns, Linux has the address but routes it locally only once its
  address work (behind the routing lock) has run, so for a while a packet to fd03::1 leaves by the dummy link's
  prefix route. A check that sent its ICMP error to the request's source (fd03::1) in that window sent it out of
  the dummy link, and the ping socket never got it. Seen with a diagnostic copy of the suite (`i2-run6/diag-hunt.sh`,
  log diag-hunt3.log, ARM, 06:26 to 06:34 UTC, load 6 to 10, raw and ICMP suites in turn at 12 threads): 3 failures
  in about 125 rounds (both of the check's failing checks, each time in the IPv6 case), and each time the
  namespace's counters showed the error sent (Icmp6OutDestUnreachs 1) with nothing received (Ip6InReceives 0),
  3 packets out of ia (request, router solicitation, the error) and none through lo, while 2 s later the local
  route was there. Measured on its own (`i2-run6/dad-window2.sh`, 06:31 UTC, load 11 to 14): `route get fd03::1` in
  the same iproute2 batch as the address answers a non-local route in 4 of 200 fresh namespaces alone and 191 of
  200 beside 24 loops of namespace churn. Fix: the ICMP suite's `ip` (which the raw suite uses too) and the
  resolver suite's wait, after a batch that adds or replaces an IPv6 address, until `ip -6 route show table local`
  lists it (at most `SETTLING`, 10 s, in the ICMP suite; 200 tries 10 ms apart in the resolver's). The resolver
  suite never failed of it, but its lookups of its own addresses race the same way.
- Ping socket drop count told early (minor): true, and kept, now said. Linux gives a ping socket no `SO_RXQ_OVFL`
  (ping_recvmsg calls only sock_recv_timestamp, not sock_recv_cmsgs), so no per-packet count exists to read;
  `SO_MEMINFO` at each receive tells every drop up to then, and nothing tells which of them came after the packet
  was queued. Totals are exact (checked). The DROPS_ASKED warning now says drops after the packet are told with
  it; listed under Not honoured.
- Multicast routing off on a socket that never had it (minor): recorded (Departures). It answers Ok without asking
  Linux, whose `MRT_DONE` answers EACCES from a socket that is not the router. Configurable's rule (twice is once)
  needs a second Disabled after one that ended routing to answer as the first did, and Linux would answer it
  EACCES; a new socket starts Disabled, so Disabled is answered from the socket's state.
- Counter tests (minor): the ICMP crate's each_send_refusal_is_counted_under_its_reason sends every refused IPv6
  send (a source, each of the four extension headers, a confirmed neighbour, a wrong ICMP code) and an IPv4 one
  with no selector, each on a new value, and wants one count each, one reason per kind and a different one for
  each other kind. The raw crate's test of the same name does so for another protocol and the upper layer of a
  payload, an IPv4 and an IPv6 packet with its own header; a_packet_taken_as_forwarded_is_counted sends a local
  protocol 255 packet whose payload repeats its own headers to an IPv6 router alert socket of protocol 255 and wants
  the payload whole and one count under the forwarded reason. Broken copies (`i2-run6/check.sh`, log check.log):
  the neighbour refusal skipped (the verifier's), destination options not refused, extension headers counted under
  the source's reason, the IPv6 header-included selector not refused, the forwarded count dropped, the payload
  selector refused under the other protocol's reason: each fails its test.
- Care-of preference unchecked (minor): sources_by_preference now gives each temporary preference with Home (its
  known address, which the host's own UDP socket with the same `IPV6_ADDR_PREFERENCES` must also choose) and with
  CareOf (whatever that host socket chooses), and the ICMP and raw source checks send all four. Linux prefers a
  care-of address only over a Mobile IPv6 home address (CONFIG_IPV6_MIP6, a module), which no link here has, so
  CareOf chooses as Home does here: the check shows it is taken and sent as the host sends it, not that it changes
  the source (listed under Not honoured).
- With the fix (`i2-run6/check.sh`, ARM, 06:36 to 06:45 UTC, load 5.9 to 9.8): 150 rounds of the raw and ICMP suite
  executables in turn at 12 threads, 0 failing runs of 300 (before it, 3 failures in about 125 rounds of the same
  turns). Final (`lint-and-suites.sh`, `ROUNDS=3`, 06:46 UTC, load 3.8 to 4.6): clippy with warnings denied clean on
  the nine packages; three rounds, each ICMP 59 and its 5 counter and 1 wait tests, raw 53 and 2 and 1, resolver 37
  and 1, Configurable 2, NegativeRouteAdvisable 7. This run's one code commit (3e59c08d) touches the three suites,
  the two counter tests and one warning's text; no interface.rs, data file or manifest.
## 2026-11-22: I2 run 5 (the check's four findings)
- Checksum offset value never checked (blocking): true. a_checksum_offset_is_written now wants the all-zero payload
  to carry its checksum at the offset alone: every other byte still zero, the two bytes there not both zero, and the
  whole summing to zero with the pseudo-header, which leaves one value for them (`is_checksummed_at`). New
  a_checksum_offset_is_checked_on_receipt: over loopback, a sender sends an all-zero payload without the offset
  (a wrong checksum) and then with it; a socket without the offset takes the wrong one first, a socket with it takes
  the right one first and nothing after. Broken copies (`i2-run5/broken.sh`, log beside it, ARM, 05:56 UTC, load 9.8
  to 11.8): the verifier's offset moved by two fails both checks; the offset never set (left off) fails both.
- Raw send of the other version (minor): fixed. A payload or a packet with its own header of the other version
  than the socket's now goes to the host as the address alone (IPv4-mapped on an IPv6 socket, no options set, the
  socket's sticky state untouched), as the ICMP host does, and the host's error is the answer, uncounted; the
  hard-coded EINVAL is gone. New a_header_of_the_other_version_gets_the_hosts_error (raw) compares the four cases
  with the host's own raw socket of protocol 253 sent the same bytes; it passes, so the answers are the verifier's
  measured host errors: EAFNOSUPPORT on an IPv4 socket for both kinds, and on an IPv6 socket the error the
  IPv4-mapped address gets (ENETUNREACH in the verifier's run). A packet with its own IPv4 header on an IPv6 socket
  answered EINVAL before (Linux's answer for a `sockaddr_in`); it now takes the mapped address as the other three
  do, for one rule. Broken copy: the early answer skipped (back through the options) fails the new check.
- Hardware, acknowledgement and completion transmit reports (minor): the record was wrong to list them beside the
  refused wireless status. They are accepted and passed to Linux as `SO_TIMESTAMPING` flags, and Linux reports none
  on these sockets here: acknowledgement reports are TCP's, and hardware and completion reports need a device that
  makes them, which the namespace has not. That is Linux's own answer for these sockets, not a fake, so no counter
  (corrected under Honoured and Not honoured below). The flags are set by the shared crate, which the UDP track
  uses too; counting them would be a change there for both tracks.
- ARM disk full (minor, not this track's code): at this run's start (05:55 UTC) / had 187 GB free of 581 GB, and
  /work is on /dev/root, not the separate stripe arm-machine.md names. This run deleted this track's stale build
  folders there (clean-i2r2, check, verify, mutant, bench); its check scripts remove their clean and broken-copy
  folders when they end. The machine's layout is the orchestrator's.
- Check (`i2-run5/check.sh`, log beside it, ARM, 05:57 UTC, load 9.6 to 18.8): clean build of the nine packages,
  clippy with warnings denied clean; three rounds, each ICMP 59 and 5 crate tests, raw 53 and 1, both allocation
  tests, resolver 37 and its counter test, Configurable 2, NegativeRouteAdvisable 7 (37 s with the build). cargo
  tree (normal, build, dev) of the 30 interface and conformance crates: none reaches any of the 17 implementations
  (control 2). This run's one code commit touches only the raw suite and the raw host, no interface.rs or data file.

## 2026-11-22: I2 run 4 (the check's five findings)
- IPv6 path MTU not capped (minor): fixed. resolve_v6 caps every IPv6 path MTU at `IP6_MAX_MTU` (65535 plus the
  40-byte header, `etherparse::Ipv6Header::LEN`), as `ip6_mtu`. New resolver check
  a_path_mtu_is_capped_at_the_largest_packet_of_its_version: a dummy link dx of MTU 100000 with 10.3.0.1/24 and
  fd03::1/64; a neighbour on it must resolve with device MTU 100000 and the path MTU a connected socket is told
  (below 100000) on both versions. Broken copies: the IPv6 cap removed fails it; the IPv4 cap removed fails it,
  loopback_route_is_local and unspecified_destination_is_loopback.
- Interface selector with a source (minor): the resolver answers as a send that names the interface per packet
  does (`IP_PKTINFO`/`IPV6_PKTINFO`, the way the ICMP, raw and UDP hosts send `route.interface`, and as
  `IP_UNICAST_IF`/`IPV6_UNICAST_IF` do), not as a socket bound to the device. Read in the kernel: over IPv6
  `ip6_route_output_flags` makes the lookup strict on the interface only for a bound socket or when no source is
  given, for a per-packet interface and for the routing socket alike; over IPv4 the interface holds the lookup in
  every case. New check an_interface_routes_as_a_send_that_names_it compares (source, path MTU) or the error with
  a UDP socket that names the interface with `IP(V6)_UNICAST_IF` and binds the source, in seven cases: IPv4 FAR by
  db without a source and from db's and da's address (on-link out of db, Linux's fallback); IPv6 fd0a::5 (main
  routes it by da at metric 1 and db at metric 2) by db without a source (db) and from fd02::1 (da), FAR_V6 by db
  from fd02::1 (da; a bound socket gets ENETUNREACH), an unreachable route by da (EHOSTUNREACH; bound gets
  ENETUNREACH). Broken copies: the interface not asked (fails it and interface_selects_the_route), the interface
  held as a bound device (fails it). The verifier's `IP_UNICAST_IF` failure was the byte order: both versions take
  the index in network byte order (my first version gave IPv6 the host's and failed the same way). Destination
  0.0.0.0 is left out: a connect looks up twice (the second time to the source address it chose), a send once,
  so no socket is a reference for a send there. The meaning is raised under For the owner.
- Advice suite failed an honest refusal (minor): fixed. advice_is_taken accepts `Ok(())` or
  `Err(HostError::NotSupported)` and nothing else; advice_is_taken_again wants the second answer equal to the
  first. Self-tests: a tally and an always-refusing implementor pass both; taking once then refusing, and an
  answer of `Closed`, are each caught (7 tests). Names kept, since the UDP suite calls advice_is_taken_again by name.
- UDP claim (minor): qualified. At this run the UDP host and suite still do not build against a00ab267 (55 error
  lines from `cargo check -p wcontainers-udp-sockets-host --tests`, ARM, 05:36 UTC), so NegativeRouteAdvisable's
  suite runs on real sockets only in the TCP suite now (3 of 3 with the new suite, 05:36 UTC). The UDP code is the
  UDP track's.
- Cost of a lookup (minor): measured, kept. ARM, 05:37 UTC, load 12.8 to 17.6, release, 20000 each, twice: an IPv4
  loopback lookup 18.2 and 18.3 us; a routing socket opened, connected and closed 2.9 and 2.8 us; a datagram
  socket opened, `IP_TTL` read and closed (the namespace's default TTL, every IPv4 answer without a route hop
  limit) 2.4 and 2.4 us. Both are per lookup because the calling thread's namespace and its sysctl may change
  between lookups; reading `ip_default_ttl` from /proc would cost an open, a read and a close as well. Only a hot
  path that resolves routes would feel it.
- Check (`i2-run4/check.sh`, then `check2.sh` after the byte-order fix; logs beside them; ARM, 05:35 to 05:38 UTC,
  load 11.8 to 35): clean build, clippy with warnings denied clean on the nine packages; three rounds each ICMP 59
  and 5 crate tests, raw 51 and 1, both allocation tests, the shared readiness test (first script); resolver 37
  of 37 and its counter test, Configurable 2, NegativeRouteAdvisable 7 (second script, three rounds). Three broken
  ICMP and raw copies, each caught: ICMP identifier sharing never set (identifiers_are_shared_only_when_both_ask),
  ICMP wait answering at once (14 checks, ten of them the readiness suite's), raw checksum left off
  (a_checksum_offset_is_written). cargo tree (normal, build, dev) of the 30 interface and conformance crates: none
  reaches any of the 17 implementations (control: the raw host's tree names itself and the shared crate). No file
  of this run is an interface.rs or a data file beside one (`git status` and `git diff` on the Mac).

## 2026-11-22: I1 run 5 (after the restart and the owner's interfaces of a00ab267)
- Conformed first: `wait_until_ready` returns `ReadinessReport`; ERROR and HANG_UP are `UnconditionalEvents`. The
  ICMP and raw hosts hold a taken-off ICMP error as `ReadinessReport { unconditional: ERROR, .. }` through the
  shared `Watched::hold`; both suites read `ready_for` for facts and `unconditional` for the error, and a wait for
  writing must report exactly WRITABLE with no unconditional event (47b28be5). The shared readiness and the readiness
  suite were conformed by another track in the same minutes (61bb2b6f); this track left them alone. The negative
  route advisable rename was already applied by the merge; no `PortSetting` is used in this track's crates.
- Check (`i1-run5/check.sh`, log `i1-run5/check.log`, ARM, 05:16 to 05:17 UTC, load 7.3 to 12.9): clean build of
  the four I1 packages, clippy with warnings denied clean; three rounds, each resolver 35 of 35 and its counter test,
  Configurable 2 of 2, NegativeRouteAdvisable 3 of 3 (24 s with the build).
- Broken copies, each caught: mark always 0 (mark_selects_the_route, source_preferences_choose_among_the_link_addresses),
  IPv4 path MTU not capped (loopback_route_is_local, unspecified_destination_is_loopback), rules on a protocol ignored
  (mobility_header_type_selects_the_route, other_protocols_route, the counter test), a second multicast join refused
  in the shared crate (the ICMP suite's configurable::setting_twice_equals_setting_once).
- Which check fails when a method breaks: RouteResolver::resolve_route, as above and in I1 runs 2 to 4;
  Configurable::configure, setting_twice_equals_setting_once (on the ICMP and raw hosts, the copy above); its
  self-test catches a setting taken once; NegativeRouteAdvisable::give_negative_route_advice, advice_is_taken and
  advice_is_taken_again (self-test likewise; run on real sockets by the TCP suite; the UDP suite also calls it but
  did not build against a00ab267 at the time, see I2 run 4). Neither generic trait
  reads back, so these see only answers (plan, "The suites").
- cargo tree (normal, build, dev) of the 16 interface and 14 conformance crates: none reaches any implementation
  crate (control: the resolver's own tree names itself and the shared crate).
- Reviewed files: no commit of this run touches an interface.rs or a data file; HEAD's 32 such files differ from
  origin/main only in crates/abi (exit.rs, host.rs, interface.rs), by the kernel track's commits (ca411e59 and
  earlier), not this track's.
- Suites with the conformed readiness (`lint-and-suites.sh`, ARM, 05:15 UTC, load 7.6): clippy clean on the nine
  packages; ICMP 59 and 5, raw 51 and 1, both allocation tests, resolver 35 and 1, Configurable 2,
  NegativeRouteAdvisable 3.

## 2026-11-22: I2 run 3 (the I2 check's four findings)
- Waits for writing never checked (blocking): true; the readiness switch had dropped the only one. Each suite now
  runs the readiness suite a second time, `readiness_for_writing`: ready is `waiting_to_write` (the smallest send
  buffer filled, until WouldBlock, or ENOBUFS for raw, with sends that loopback's fq holds 5 s by their Monotonic
  transmit time; made writable by `tc qdisc del dev lo root`, which frees them); failing is
  `waiting_to_write_until_an_icmp_error` (the same on the dummy link's fq after one plain send that leaves at once,
  then an ICMP error about it). New a_wait_for_writing_finishes_at_once in both: a new socket of each version, a wait
  for WRITABLE and for READABLE | WRITABLE each finish at the first poll with exactly WRITABLE. The plan's text and
  its "35 waits on 32 sockets" are corrected. ICMP now 59 checks, raw 51.
- Received traffic class unchecked (minor): echo requests in reply_arrives_whole now carry DSCP EF with ECN ECT(0)
  (IPv4) and ECT(1) (IPv6), and the reply's received class must be that; every ICMP error the suites send from a
  raw socket carries EF with ECT(0) (`ERROR_DSCP`, `ERROR_ECN`, set with IP_TOS / IPV6_TCLASS), and
  `has_the_error_traffic_class` checks it in both ICMP error checks and the raw icmp_errors_arrive_whole.
- Realtime never sent (minor): the shared rejected-times list now has a Realtime send, which etf (CLOCK_TAI only)
  drops as invalid; its time is TAI's now plus 100 ms, so a Realtime mapped to the atomic clock is only missed
  (caught). Linux tells a rejection by its time alone, so it goes before the missed send of the same time (the first
  attempt put it after and the check failed on the right code). Realtime against Monotonic cannot be told apart
  (Not honoured, below).
- Header of the other version (minor): an ICMP send whose header is of the other version than the socket's now goes
  to the host as the address alone (IPv4-mapped on an IPv6 socket, no options set) and the host's error is the
  answer, uncounted. New a_header_of_the_other_version_gets_the_hosts_error compares with the host's own ping socket
  sent the same message: EINVAL both ways on the ARM machine (the check's note expected EAFNOSUPPORT for an IPv6
  address on an IPv4 socket; measured EINVAL). The first version set the header's IPv6 options on the IPv4 socket
  and answered ENOPROTOOPT; caught by the new check.
- Broken copies (`i2-run3/broken.sh`, `broken2.sh`, logs `broken.log`, `broken3.log`, ARM, 04:50 to 04:56 UTC, load
  2.3 to 5.8), each failing: DATAGRAM_FACTS without WRITABLE (5 ICMP, 5 raw checks), WRITABLE polled as IN (5 and
  5), IP_RECVTOS off (ICMP 3: both IPv4 reply checks and the IPv4 error; raw icmp_errors_arrive_whole), IPV6_RECVTCLASS
  off (ICMP 2; raw 2), Realtime as CLOCK_TAI (rejected_transmit_times in both), the other-version header refused as
  before (a_header_of_the_other_version_gets_the_hosts_error). Realtime as CLOCK_MONOTONIC passes, as expected.
  The first broken.sh lacked the verifier's `touch` of every source, so three later copies kept an earlier copy's
  shared crate; their results were not used, and broken2.sh runs them again with it.
- The ICMP a_wait_sees_an_error_a_send_read_first failed in 3 of the first 9 rounds (04:44 to 04:45 UTC, load 8.5
  to 26), "the wait is woken", then passed in 10 rounds alone and 45 rounds of the whole suite after (04:46 to 04:56,
  load 2.3 to 26). The same unexplained wake flake as "The wait flake" below. finished_soon now polls again when not
  woken and prints what the wait gives, telling a lost wake from a lost fact next time.
- Final (`lint-and-suites.sh`, ARM, 04:56 UTC, load 2.8): clippy with warnings denied clean on the nine packages;
  ICMP 59 and 5, raw 51 and 1, both allocation tests, resolver 35 and 1, Configurable 2, NegativeRouteAdvice 3.
  Neither commit of this run touches an interface.rs or a data file.
- Linter (Mac, 04:52 UTC): new findings of the same kinds already raised: public_outside_interface on
  `has_the_error_traffic_class` and `held_fields` (shared with the raw suite), conformance_not_run on the two
  `waiting_to_write*` bundles. One magic literal it found in finished_soon is gone.

## 2026-11-22: I2 run 2 (the suites' comment and literal findings, then a self-check)
- Linter (Mac, `cargo reviewable check`, besides the model's unanswered names and summaries): 170 findings in this
  track's files at the start (04:05 UTC), 93 at the end (04:22 UTC). Gone: every summary over one line or 150
  characters or of two sentences (the detail now follows a blank `///` line), the five literals that restated
  STATISTICS, JSON and DEVICE, and the 25 magic literals: `LOOK_AGAIN_SOON` names the 1 ms pause; the routing header
  is built from `AT_THE_FIRST_OF_TWO` and `FLAGS_AND_TAG` (the raw suite's copy of it is gone, it calls the ICMP
  suite's `segment_routing_header`, and its own `FAR_V6_SEGMENT` copy too); `PAD_N` names the hop-by-hop padding;
  `TYPE_AND_CODE` the ICMP error's first two bytes; `Wakes::reach(1)` became `first_came()` (every caller asked for
  one); the source filter pair is a tuple and the preference sources a `Vec`; zero counters start at `usize::MIN`;
  and `counting_allocator!` counts the bytes asked for instead of calls (a number in a macro body is always a
  finding; zero bytes is zero allocations, and the shrinking broken copy still fails both allocation tests).
- Left, each waiting on a ruling already raised (For the owner): 61 public_outside_interface (the shared test tools,
  the inherent `new` and `counters`, and the new one below on `never`), 21 conformance_not_run (the readiness line,
  whose required form the readiness suite's `tests!` does not take; the `waiting_for_*` bundles; `counting_allocator!`
  as a second macro; the crate tests naming the suite for its tools; `#![cfg(target_os = "linux")]` in conformance.rs),
  4 implementation_folder (`impl Readiness` and `impl Configurable`), 7 constant_defined_twice with the resolver suite.
- Self-check (`i2-run2/check.sh`, then `check2.sh` and `check3.sh`, ARM, 04:15 to 04:23 UTC, load 3.7 to 20):
  - Clean build in a fresh folder, clippy with warnings denied clean on the nine packages, three rounds each ICMP 51
    of 51 and 5 crate tests, raw 44 of 44 and 1, both allocation tests, resolver 35 of 35 and its counter test,
    Configurable 2 of 2, NegativeRouteAdvice 3 of 3, the shared readiness test (35 s with the build).
  - 64 broken copies of runs 1 to 5, rewritten for the code now in linux.rs: every one is caught but one. The eight
    whose text had moved (the waits answering at once and errors not held, now on `wait_until_ready` and
    `ReadyFor::ERROR`, the checksum offset, the IPv6 alert never cleared, the resolver's preferences, the readiness
    hold) were written again and each fails; the counts are caught by tests/counters.rs, which the old script did not
    run. The one not caught is known since run 5: an ICMP send that answers only EINVAL as a refusal passes, because
    an absent source gives a ping socket another error; the copy that refuses any other errno fails
    a_waived_source_check_sends_from_an_absent_address.
  - A broken wait fails eight checks, five of them the readiness suite's (a_wait_does_not_finish_before_the_handle_is_ready
    and the rest), in both suites.
  - cargo tree (normal, build, dev) of the 16 interface crates and 14 conformance crates against the 17 implementation
    crates: none named (control: the raw host's tree names itself and the shared crate). The literal check: 0
    findings. No commit since 46e143ea touches an interface.rs or a data file beside one, nor does the working tree.
- Final run (ARM, 04:22 UTC, load 20, `lint-and-suites.sh`, `ROUNDS=3`): clippy clean; each round ICMP 51, raw 44,
  resolver 35, the crate tests, Configurable 2, NegativeRouteAdvice 3.

## 2026-11-22: I2 run 1 after the reorganization (the I1 check's findings, then the linter's layout)
- The I1 check's eight findings, each (broken copies in `i2-run1/findings.sh`, log `i2-run1/findings.log`, ARM,
  03:44 UTC, load 0.3 to 1.1; each copy caught by the check named):
  - Routing header passed a first hop taken from the last entry: the type 4 header now has three segments and
    segments left 1, so its next segment is neither its first nor its last (srh-first-hop-from-last-entry fails
    routing_header_goes_to_its_first_hop).
  - IPv4 route via an IPv6 gateway: locked_mtu_without_value_uses_the_device_mtu adds `10.21.0.0/16 via inet6
    fd01::2 dev da mtu lock 0`, path MTU 576 (ipv6-gateway-of-ipv4-route-ignored fails it).
  - Probe's traffic class unchecked: new dscp_rule_takes_an_encapsulating_route, IPv4 and IPv6: a rule on DSCP 8
    sends lookups to a seg6 route on db in table 100 (main has a plain one on da); the answer must be db with the
    path MTU a socket of that traffic class connected there is told (below db's), and without the DSCP da at 1400.
    Linux's IPv6 connect takes the socket's traffic class (ip6_datagram_flow_key_init). probe-without-traffic-class
    fails it.
  - IPv4 options not validated: a lookup with options now sets them as `IP_OPTIONS` on a datagram socket first and
    answers its error. New malformed_ipv4_options_are_refused_as_a_send_refuses_them: an LSRR with its pointer
    past its end, one with a length of no whole address, and two LSRRs must give the error `IP_OPTIONS` gives (the
    check asks the host for it and requires one); the well-formed LSRR resolves (ipv4-options-not-validated fails).
  - No IPv4 address in the namespace gave EPROTO: the source is now 0.0.0.0, as a connected socket gets. New
    a_namespace_without_an_ipv4_address_routes_from_the_unspecified_address (a fresh namespace, lo and a dummy dc
    with addrgenmode none, MTU 1450): equal to the connected socket's (0.0.0.0, 1450). With other links holding
    addresses Linux lends one (measured: 10.1.0.1), which the resolver already gave. no-source-is-eproto fails it.
  - IPv4-mapped destination: now resolved over IPv4, as a dual-stack socket sends (source mapped back; a source
    that maps no IPv4 address is a counted refusal, MIXED_SOURCE, in the counter test's six reasons). New
    an_ipv4_mapped_destination_routes_over_ipv4 compares with a dual-stack connected socket for FAR and db's
    gateway. mapped-looked-up-in-ipv6 and mapped-source-not-mapped-back fail it.
  - One lock for the whole lookup: the kept buffers are taken with `try_lock`; a lookup that finds them busy uses
    buffers of its own, so lookups no longer wait on each other. Not measured.
  - Generic suites see only answers: unchanged, as recorded in I1 runs 2 to 4 and the plan; neither trait reads
    back, and the effects are checked by the requiring suites (ICMP and raw each setting's, now also
    other_settings_set_twice_answer_as_once).
- Linter layout, ICMP and raw (the start of run: 401 findings in this track's files besides the model's unanswered
  names and summaries; the end: 187):
  - Each host crate now builds on every unix system: implementation.rs holds the types (fields under
    `cfg(target_os = "linux")`), linux.rs Linux's calls (moved with `git mv`), portable.rs a counted refusal of
    every socket (NO_PING_SOCKETS, NO_RAW_SOCKETS; the socket type holds `Infallible` there). Checked with `cargo
    check` on the Mac. The reason constants are private; the crate tests count distinct reasons instead.
  - conformance.rs at the crate root runs the suite (`[[test]] conformance`); the crate's own tests are
    tests/counters.rs.
  - Each suite's `tests!` lists every check in the standard shape; each check enters its namespace first (after the
    value is made), as the resolver suite does. One `mod configurable` line runs Configurable's suite on one
    setting (ICMP: a multicast membership; raw: an IPv4 router alert); the other setting kinds are the new check
    other_settings_set_twice_answer_as_once. The helper macros `each_check` and `configurable_checks` are gone; the
    suites re-export the required suites under their own names.
  - Function summaries in the third person (helpers "Returns ...", checks "Checks that ..."), links on every named
    number (array lengths that were the only number made slices), the router alert and checksum -1 named.
  - Results (ARM, 04:02 UTC, load 1.9 to 2.5, `lint-and-suites.sh`, `ROUNDS=3`): clippy with warnings denied
    clean on the nine packages; each round ICMP 51 and 5 crate tests, raw 44 and 1, both allocation tests,
    resolver 35 and its counter test, Configurable 2, NegativeRouteAdvice 3, the shared readiness test.
  - cargo tree (normal, build, dev) of the six interface crates and three suites of this track: no implementation
    (control: the ICMP host crate's tree names the shared crate). No commit of this run touches an interface.rs or a
    data file.
- Left in this track's files (end of run, Mac, 04:01 UTC), each to do or raised:
  - 75 comment and literal findings in the two suites (summaries over two lines or 150 characters, not one
    sentence, magic literals in checks): to do, rewording each.
  - public_outside_interface (61) and conformance_not_run on crate tests (8): the suites' shared test tools (the
    raw suite and the crate tests use the ICMP suite's namespace, capture and send helpers), and the inherent
    `new` and `counters`: raised below.
  - The readiness line: the readiness suite's own `tests!` takes a module, ready and failing values, not the
    layout's one value, so `mod readiness { ...::tests!(f($value)); }` cannot be written here (also in UDP and
    TCP), and `waiting_for_*` are taken for suite functions: for the readiness track.
  - conformance.rs keeps `#![cfg(target_os = "linux")]`, which the rule counts as more than one invocation: the
    ICMP and raw suites are Linux-only files, so the invocation does not compile elsewhere.
  - implementation_folder on `impl Readiness` and `impl Configurable` in linux.rs: traits their own interface
    requires (the same in UDP and TCP); raised below.
  - The seven constant_defined_twice between the resolver and ICMP suites, as before.

## 2026-11-22: I1 run 4, the independent check's findings, each
- IPv4 never saw an answer for another protocol (blocking): true. other_protocols_route added its rule on UDP
  before the first IPv4 lookup, so every IPv4 GRE and SCTP lookup was refused (RULE_ON_PROTOCOL) and accepted;
  run 3's claim that the check sees answers in its first part held for IPv6 only. Now the first part puts only the
  port rule on IPv4 (UDP and port rules on IPv6) and DEMANDS answers on both versions: GRE by da, SCTP to 2905 by
  db, to 2906 by da. Then a rule on UDP over IPv4 (refusal accepted, the same answers checked if given), then a
  rule on GRE on both (GRE by db, refusal accepted), and IPv6 SCTP to 2906 must still be answered by da. The
  mobility check's first part (a port rule naming no protocol) now demands answers too; its second part (a rule on
  protocol 135) accepts a refusal.
  Broken copies (`i1-run4/check.sh`, log `i1-run4/check.log`), each caught: the verifier's
  ipv4-other-protocols-always-refused (other_protocols_route fails; before: 31 of 31 passed), the same for IPv6
  and for both (other protocols, mobility, counter test), rules ignored, mobility type dropped, other protocol as
  UDP, IPv4's UDP forgotten. Vacuity copy (refusals fail the suite): 26 pass, the same five fail.
- Generic suites see only answers (minor): unchanged, kept as recorded below and in the plan ("The suites").
- Check (`i1-run4/check.sh`, ARM, 03:15 to 03:16 UTC, load 1.1 to 3.4): clean build and clippy with warnings
  denied clean; three rounds, each resolver 31 of 31 and its counter test, Configurable 2 of 2,
  NegativeRouteAdvice 3 of 3 (23 s with the build).
- Linter (Mac, 03:17 UTC): in the I1 files the ten findings already raised, and one summary_too_long (the new
  check's summary, 151 characters), fixed. crates/system/conformance's conformance_not_run for RouteResolver is
  not this track's file.

## 2026-11-22: I1 run 3, the independent check's findings, each
- Encapsulation check still accepted a refusal (blocking): it did; this run's status claim above was wrong. Now
  encapsulating_route_leaves_room_for_its_headers has no refusal branch and checks IPv4 too (a seg6 route of each
  version on da; the told MTU must be below da's, and the route's path MTU equal to it). Broken copy (MTU from the
  route) fails it.
- Unspecified IPv6 destination (minor): resolve_v6 now looks `::` up as `::1`, as every IPv6 send of Linux does
  (udpv6_sendmsg, rawv6_sendmsg, the connects); IPv4 0.0.0.0 the kernel already makes loopback. New check
  unspecified_destination_is_loopback: both versions equal loopback's route and the source and path MTU a socket
  connected to the unspecified address gets. Broken copy (no mapping) fails it. The verifier's seeded comparison
  (its verify.rs, run unchanged by `i1-run3/verify-run.sh`), 20000 lookups for each seed 0x5eed1234, 987654321
  and 42: no mismatch (before: 50 of 2000, 530 and 458 of 20000, all `::`).
- Generic suites see only answers (minor): unchanged and kept in view. Neither trait reads back, the one-tuple
  form is the brief's, and the requiring suites check the effects (plan, "The suites").
- other_protocols_route checked nothing when answered, and other protocols were always refused (minor): both
  changed. A protocol the routing socket does not take (it takes TCP, UDP and the version's ICMP) is now looked
  up without one. Read in the kernel: the IPv4 routing socket then asks as UDP (`ip_proto = IPPROTO_UDP`,
  inet_rtm_getroute), the IPv6 one as protocol 0 (found when the first version of the new check failed). That is
  Linux's own answer except where a rule selects on the protocol (or on UDP, over IPv4), or where the entry hashes
  among next hops (IPv6's hash always takes the protocol). So the version's rules are dumped (`RTM_GETRULE`) and
  the entry read (`RTM_F_FIB_MATCH`, `RTA_MULTIPATH` or `RTA_NH_ID`), each a new counted refusal
  (RULE_ON_PROTOCOL, HASHED_PROTOCOL). Ports are given as before, so port rules see them. A mobility header type
  is now `RTA_DPORT`'s first byte, as the kernel's flow keeps it, so mobility headers are answered too. Not done:
  IPv4 under hash policy 0 ignores the protocol, so its multipath entries could be answered; refused alike.
  The check now: rules on UDP and on destination port 2905: GRE goes by main (da), SCTP to 2905 by table 100 (db),
  to 2906 by main; then a rule on GRE: GRE by db. Each part accepts NotSupported. The mobility check: a port rule
  without protocol (type 5 to db, 6 and 7 to da), then one on protocol 135 (5 and 6 to db, 7 to da). Measured with
  `ip` first: a port rule without a protocol is accepted, and `ip route get ... ipproto gre` is "Unsupported ip proto".
  Broken copies, each caught: rules not looked at (both checks and the counter test), the IPv4 UDP rule
  forgotten, another protocol asked as UDP over IPv6, the mobility type dropped; the hash check skipped (counter
  test, which now has five reasons: a rule on protocol 135 and a GRE lookup over a two-way IPv6 route are new).
- Flow label lease (minor): the reason is now in the plan. A lease outlives its socket by at least six seconds
  (FL_MIN_LINGER, check_linger in net/ipv6/ip6_flowlabel.c) and meanwhile another user's lease of that label fails
  with EPERM, so a lookup that takes one would change the namespace for others. Such lookups stay refused (PROBE).
- Vacuity copy (unless_refused panics on NotSupported) now: 26 checks pass. Five fail: the three that only ever
  see a refusal here (link scope, strict source route, waived source) and mobility and other protocols, which see
  answers in their first part (the broken copies above fail them) and refusals once a rule names their protocol.
- Independent check (`i1-run3/independent-check-i1d.sh`, ARM, 02:59 to 03:01 UTC, load 2.3 to 3.6): clean build
  and clippy with warnings denied clean; three rounds, each resolver 31 of 31 and its counter test, Configurable 2
  of 2, NegativeRouteAdvice 3 of 3 (22 s with the build). 19 broken copies, each caught: the 13 of run 2 (routing
  header rerun alone with this run's text, `i1-run3/rh.sh`), and unspecified not loopback, rules ignored, hash
  check skipped, mobility type dropped, other protocol as UDP, IPv4's UDP forgotten. cargo tree of the 50 interface
  and conformance crates: none reaches any of the 17 implementations. No commit of this run touches an
  interface.rs or data file.
- Linter (Mac, 03:00 UTC, mid-run): in the I1 files only the ten findings already raised (three
  public_outside_interface, seven constant_defined_twice) and one new magic_literal (an array length in the
  mobility check), fixed; at the end (after the commits) only the ten raised ones. Model-answer findings are the
  orchestrator's.

## 2026-11-22: I1 run 2, the independent check's findings, each
- Source preferences refused though the host can answer (blocking): honoured now. Linux keeps the route of the
  lookup without a source and picks the source from that routing entry and its device (ip6_dst_lookup_tail,
  ip6_route_get_saddr). So the resolver keeps its lookup and reads the source from a probe: a UDP socket made as
  the lookup's owner (the shared `open_socket`), `IPV6_UNICAST_IF` the route's interface, the lookup's traffic
  class, mark and preferences, connected to the destination; `getsockname` is the source. Both lookups are asked
  again with `RTM_F_FIB_MATCH` (the routing entry itself) and must be equal, else a counted refusal (PROBE). A
  local destination skips the probe (it is its own source). The suite check now has no refusal branch and also
  wants db's temporary address under a mark and db's public one under an owner. Broken copies, each caught by
  source_preferences_choose_among_the_link_addresses: probe source ignored, probe not made as the owner, probe
  without the mark (and the old mark and owner breaks now fail it too). The plan's reason for refusing is gone.
- Encapsulating route refused though a socket is told its MTU (minor): the same probe gives its path MTU
  (`IP_MTU`/`IPV6_MTU`), both versions. Broken copy (MTU from the route) fails
  encapsulating_route_leaves_room_for_its_headers. Wrong when written: the check still accepted a refusal
  until run 3 below.
- The entry comparison has its own case in tests/counters.rs (a flow label selects db's /64 in table 100, and a
  probe, which takes no label, gets main's wider fd09::/16 via db): refused and counted. The broken copy that skips
  the comparison fails that test; no suite check can reach it (it needs an implementation that probes).
- IPv4 path MTU not capped at IP_MAX_MTU (minor): every IPv4 path MTU is capped at 65535, as ipv4_mtu. The suite no
  longer sets lo's MTU (a new namespace's lo is 65536; the old comment was wrong) and loopback_route_is_local wants
  the path MTU a connected socket is told, IPv4 (65535) and IPv6 (65536). Broken copy (no cap) fails it.
- Anycast check passed any error (minor): only NotSupported or a Local route pass now.
- Generic suites see only answers (minor): true and now said in the plan. Neither trait can read back what it
  changed, and the brief fixes the one-tuple form, so a configure that toggles state passes
  setting_twice_equals_setting_once and advice that does nothing passes both advice checks; the self-tests catch
  only an error on the second call. The effects are the requiring suites' (ICMP and raw check each setting's; UDP's
  negative_route_advice_keeps_a_socket_sending, TCP's negative_route_advice_picks_a_new_flow).
- Vacuous checks (minor): the earlier "all 30, each selector its own" overstated it. Vacuity copy (unless_refused
  panics on NotSupported), now: 25 pass, 5 only ever see the counted refusal on this implementation:
  link_scope_takes_only_routes_on_the_link, strict_source_route_takes_only_a_first_hop_on_the_link,
  waived_source_check_takes_a_foreign_address, mobility_header_type_selects_the_route, other_protocols_route.
  For those selectors the suite checks only that a refusal is NotSupported.
- Linter count (minor): the earlier "15" was wrong; the list had 11. This run's end, Mac: the I1 files have the
  three public_outside_interface (impl, `open`, `counters`) and the seven constant_defined_twice with the ICMP
  suite (IP, PACKETS, TRAFFIC_CONTROL, DEVICE, FIELD, LOCAL, JSON_QUOTE), all raised with the owner; the four new
  ones this run made (LOOPBACK_MTU twice with the kernel's private one, TEMPORARY_ADDRESSES' array length, two
  summaries too long or over two lines) were fixed after that run and not re-checked.
- Independent check (`i1-run2/independent-check-i1c.sh`, ARM, 02:35 UTC, load 8.1 to 9.8): clean build, clippy
  with warnings denied clean; three rounds each resolver 30 of 30 and its counter test, Configurable 2 of 2,
  NegativeRouteAdvice 3 of 3 (23 s with the build). 13 broken copies, each caught: the 7 earlier ones still apt
  (mark, owner, confirmation, locked MTU, flow label, anycast, routing header) and the 6 above. cargo tree of the 50
  interface and conformance crates: none reaches any of the 17 implementations. No commit since da182336 touches
  an interface.rs or data file; the working tree's crates/abi/interface.rs and host.rs edits are another track's.
- Final run (ARM, 02:37 UTC, load 5.1 to 6.4, `lint-and-suites.sh`, three rounds): clippy clean on the nine
  packages; each round ICMP 58 and 1, raw 47 and 1, resolver 30 and 1, Configurable 2, NegativeRouteAdvice 3.
- Which check fails when a method breaks: RouteResolver::resolve_route, 25 checks that see answers (above and the
  broken copies); Configurable::configure and NegativeRouteAdvice::give_negative_route_advice, only an error.
- New dependency of the resolver: rustix 1.1.6 (already in the tree, licence below) for `IP_MTU` and `IPV6_MTU`.

## 2026-11-21 evening: I1 in the linter's layout
- Linter (`cargo reviewable check`, Mac) at the start: 149 findings in the I1 files besides the model's unanswered
  names and summaries; at the end 15, each below. ICMP and raw files: 393 left, for an I2 run.
- The resolver builds on every system (implementation.rs holds the type; linux.rs the lookup; elsewhere a counted
  refusal, NO_ROUTING_SOCKET), so the linter sees `RouteResolver` implemented. Its helpers are methods of a private
  `Lookup`, not of the exported type; the refusal reasons are private (the counter test checks three distinct
  reasons, each counted once). `open()` no longer fails.
- Each lookup opens its routing socket (connect autobinds it) in the calling thread's namespace: the layout's
  `tests!` only names the value, so the suite enters its namespace after the value is made. Measured, ARM machine,
  02:04 UTC, load 8.5 to 11.2, release, 20000 each, twice: a loopback lookup 18.0 and 17.8 us; a routing socket
  opened, connected and closed 2.7 and 2.8 us, the cost this adds.
- conformance.rs is the one invocation and a `[[test]] conformance`; the counter test is tests/counters.rs and names
  no suite. The suite's `tests!` has exactly the layout's shape, in the file's order; its helpers are private; every
  number constant has its link; summaries are third person, one line.
- The ICMP suite keeps its own copies of the iproute2 tools it took from the resolver suite (the raw suite takes them
  from it), so neither depends on the resolver suite.
- Flake found and fixed: confirmed_neighbour_is_confirmed counted every frame da sent, and da's own multicast
  reports failed it once in 40 rounds (and once in the broken copies). It now counts frames to the gateways' link
  address with a tc flower filter on da's egress: 100 rounds, no failure; a broken copy that sends a real datagram
  to the next hop fails it.
- Independent check (`i1-linter/independent-check-i1b.sh`, 02:03 UTC, load 5.8 to 11.5): clean build with clippy
  denied warnings clean, three rounds each resolver 30 of 30 and its counter test, Configurable 2 of 2,
  NegativeRouteAdvice 3 of 3 (23 s with the build). Broken copies, each caught by its check: mark ignored
  (mark_selects_the_route), owner ignored (owner_selects_the_route), confirmation skipped
  (confirmed_neighbour_is_confirmed), locked MTU ignored (locked_mtu_without_value_uses_the_device_mtu), flow label
  ignored (flow_label_selects_the_route), anycast as unicast (an_anycast_route_is_not_given_as_another_kind),
  preferences ignored (source_preferences_choose_among_the_link_addresses), routing header ignored
  (routing_header_goes_to_its_first_hop). cargo tree (normal, build, dev) of the 50 interface and conformance
  crates against the 17 implementations: none reached (control: the resolver's own tree names it). No commit of
  this run touches an interface.rs or a data file; the working tree's crates/abi/host.rs edit is another track's.
- Which check fails when a method breaks: RouteResolver::resolve_route, all 30 checks, each selector its own (above);
  Configurable::configure, setting_twice_equals_setting_once (its self-test catches a setting taken once only);
  NegativeRouteAdvice::give_negative_route_advice, advice_is_taken and advice_is_taken_again (self-test likewise).
- Final run (ARM, 02:08 UTC, load 0.8): clippy clean on the nine packages; ICMP 58 and 1, raw 47 and 1, resolver 30
  and 1, Configurable 2, NegativeRouteAdvice 3; three rounds of the same at 02:07 UTC.
- Linter findings left in the I1 files: the resolver's impl block, `open` and `counters` (no interface trait builds
  or counts, see For the owner); seven `constant_defined_twice` between the resolver and ICMP suites (IP, PACKETS,
  TRAFFIC_CONTROL, DEVICE, FIELD, LOCAL, JSON_QUOTE), since the layout gives two suites no shared home for their
  test tools; `grammar_unclear` on `link` was answered by renaming it `read_link` (not yet re-checked).

## 2026-11-21: waiting is the Readiness trait
- Repair after the independent check: three long comments of the ICMP suite and one of the raw suite made one
  sentence each. Run on the ARM machine (job readiness-repair): ICMP 58 + 1, raw 47 + 1, route resolver 31 pass.
  The literal check still finds ICMP suite `ROUTE_MTU = 1300` (eeddf0af5, before this change).
- The interfaces lost `wait_for_events`; IcmpSockets and RawSockets now require `Readiness<Self::Socket>`
  (crates/readiness), whose facts are the six of `ReadyFor`. Both host implementations write
  `impl Readiness<HostXSocket> for HostXSockets` on the shared `Watcher`, watching with `DATAGRAM_FACTS`; a held
  ICMP error is `hold(ReadyFor::ERROR)`.
- Both suites run the whole readiness suite (module `readiness_contract`): ready by an echo reply (ICMP) or a packet
  of the socket's protocol (raw), failing by an ICMP error about a request. Each re-exports it as
  `readiness_conformance`.
- Deleted as now in the readiness suite: a_wait_for_data_finishes_when_the_reply_arrives (raw: ..._a_packet_arrives),
  a_wait_for_writing_finishes_at_once, a_dropped_wait_is_withdrawn, an_icmp_error_ends_a_wait_for_data. Deleted as
  only the twelve poll bits' (normal and band bits): `only_asked_for`, `NO_EVENTS`, `READABLE`, `WRITABLE`.
- many_waits_each_wake_once is now waits_on_many_sockets_each_wake_once: the several waits on one socket are the
  readiness suite's; its count is the readiness suite's `MANY_WAITS`. `WAITED_SOCKETS` and `WAITS_ON_ONE` are gone
  (the UDP suite deleted its copies too), which ends the literal check's report of them.
- New in each suite: a wait for a fact it never has (URGENT_DATA, PEER_FINISHED_SENDING, the ICMP suite's
  `NEVER_ON_DATAGRAMS`) waits on with a reply to read, until an ICMP error ends it, never giving either fact.
- Run on the ARM machine (`--name readiness-icmp-raw`), five rounds: ICMP 58 and 1, raw 47 and 1, resolver 31,
  every round; clippy on the six packages without a warning.
- Still reported by the literal check, from run 5 and not of this change: the ICMP suite's `ROUTE_MTU = 1300`
  restates the resolver suite's private `B_MTU`; a different fact (a narrow route, not link db), so left for the owner.
- For the owner: the trait's one-sentence comment does not say that ERROR and HANG_UP end a wait whether or not they
  were asked for; the implementations do and the readiness suite checks it.

## I2 run 5: the independent check's findings, each
- Largest fragment checked by nothing (blocking): new checks a_fragmented_reply_tells_its_largest_fragment (ICMP)
  and a_fragmented_packet_tells_its_largest_fragment (raw): loopback's MTU lowered to 1280, a 3000-byte echo or
  payload to 127.0.0.1 and ::1, and the field must equal the longest fragment of that packet on loopback's wire
  (matched by the fragments' identification). A reply or packet in one piece must tell none (reply_arrives_whole,
  ipv4_packets_arrive_whole). The verifier's broken copy (`let _ = integer();`) fails both. The status no longer
  calls it not honoured; link-layer length now has its Linux reason (below).
- Transmit report interface unchecked and made up (blocking): both suites now check, through the ICMP suite's new
  `check_loopback_reports`, that a report holding its packet names loopback and tells the frame's length (the IP
  packet on the wire plus loopback's Ethernet header: 58 and 78 bytes for the 24-byte echo). A report without its
  packet (`SOF_TIMESTAMPING_OPT_TSONLY`) has no `IP_PKTINFO` (ipv4_datagram_support_cmsg: "Not possible for packets
  ... without payload"), so its interface 0 is now counted and warned of (REPORT_INTERFACE_NOT_GIVEN, also for a
  rejected transmit time), listed below and raised with the owner. Broken copies: interface always 0, length
  without the link header, the count dropped: each fails.
- Path MTU modes Interface and Omit not told from Dont (minor): new check each_path_mtu_discovery_mode_takes_its_mtu
  in both suites, on the shared `path_mtu_modes_take_their_mtu`: a route of MTU 1300 out of a link of 1500
  (`route ... mtu 1300`, the route's own MTU, which Linux's path MTU takes as an ICMP-learnt one would be; simpler
  than a crafted "fragmentation needed"), packets of 1400 and 1600 bytes, IPv4 and IPv6. Dont and Want fragment to
  1300; Do is too long at 1300; Probe and Interface leave 1400 whole (DF on IPv4 only for Probe) and are too long
  at 1500; Omit leaves 1400 whole and fragments 1600 to the link's 1500. Read in the kernel (ip_sk_use_pmtu,
  ip_sk_ignore_df, ip6_setup_cork, __ip6_append_data) and passing on Linux 7.0. Broken copies Interface and Omit as
  Dont, Omit as Interface, Probe as Do: each fails both. Probe and Interface stay alike over IPv6 (they differ only
  in taking learnt path MTUs).
- Required source accepted any error (minor): both suites now want `HostError::Errno`, not a refusal. Broken copies
  that answer the host's error as NotSupported fail a_waived_source_check_sends_from_an_absent_address and
  payload_routes_take_their_selectors.
- Wait flake (minor): still not explained. A hunt of the check alone (`flake-wait.sh`: the raw and ICMP
  an_icmp_error_ends_a_wait_for_data, 12 loops of 100 process runs each, 00:07 UTC, load 2.2 to 4.3): 2400 passes,
  no failure. Both checks now say, if not woken, whether the wait has its events when polled again (a lost wake) or
  not (an error Linux never queued), so the next failure tells which.
- Forwarded guess uncounted (minor): each packet taken as forwarded is now counted and warned of
  (FORWARDED_BY_ITS_BYTES). No crate test reaches it: only the suite's forwarding pair makes such a packet, and a
  suite function consumes its value before its counters could be read.
- Found on the way: Linux reads `SOF_TIMESTAMPING_OPT_TSONLY` from the socket when it takes a timestamp
  (__skb_tstamp_tx), not per send, so a send that changes whether its reports hold the packet while earlier sends
  wait in the host changes theirs too. Such a send is now counted and warned of (PACKET_WISH_CHANGED, when the host
  still holds bytes of the socket's sends); crate test a_packet_wish_changed_while_sends_are_held_is_counted (fq
  pacing at 1000 bytes a second, three sends); listed below.

## I2 run 4: the independent check's findings, each
- IPv6 router alert Off checked by nothing (blocking): an_ipv6_router_alert_takes_forwarded_packets now turns the
  first socket's alert Off and changes another socket's value to the forwarded packet's, sends again, and expects the
  packet at the changed socket only (and still at the other namespace's). The verifier's broken copy (no -1 written)
  fails it.
- Received IPv4 options checked by nothing (blocking): new ICMP check an_echo_reply_arrives_with_its_ipv4_options
  sends an echo with a record route option of two slots; Linux echoes it, filled, and the reply must carry options
  of that length and match the reply on loopback's wire byte for byte (but the identification and fragment fields).
  The broken copy without `IP_RECVOPTS` fails it.
- Source filters checked by nothing (blocking): the ICMP suite's `source_filters_are_written` joins a group with
  Include of one source, of two, Exclude of one and Exclude of none, and reads the namespace's lists
  (/proc/thread-self/net/mcfilter and mcfilter6: per source, how many sockets include and exclude it) after each.
  It runs for IPv4 and IPv6 in the new ICMP check multicast_source_filters_are_written and the new raw check
  multicast_source_filters_choose_the_senders, which also checks delivery on an IPv4 raw socket: a looped multicast
  send from the dummy link's address arrives under Include of that address and Exclude of another, not under
  Include of another or Exclude of it. IPv6 raw delivery cannot show a filter (an unbound IPv6 raw socket takes every
  multicast packet of its protocol, and the interface has no bind); listed under Not honoured.
- Forwarded-whole guess fooled by a crafted local packet (minor): not changed, recorded under Not honoured. Linux
  gives nothing that tells a forwarded router alert packet (given from its IPv6 header) from a local one (given after
  its headers): the destination, hop-by-hop header, length and source in the ancillary data are the same for both,
  and IPV6_RECVORIGDSTADDR's port only moves the ambiguity. The exact answer is whether the destination is local
  in the socket's namespace, a route lookup per such packet in a namespace a value may not be in (the suite opens
  sockets of one value in two namespaces). The guess runs only for an IPPROTO_RAW socket with an IPv6 alert set, and
  only a local packet of protocol 255 whose payload repeats its own IPv6 and hop-by-hop headers fools it.
- Raw drops "since previous" unchecked (minor): ip_settings_are_taken now floods twice; each flood's received plus
  told drops must be its own 63. The broken copy that tells the running total fails it. Found on the way: the
  check's sender takes every packet too, and once both sockets were full Linux sent itself a protocol unreachable
  error, which a receive answers as ENOPROTOOPT; the sender is now emptied after each flood. The flood part runs
  before the fair queue is put on loopback.
- Hold wakes untested (minor): new test tests/readiness.rs in the shared crate polls a wait until pending, holds an
  error, and expects one wake and then the error; with nothing held a new wait is pending. The broken copy whose hold
  wakes nothing fails it. Through the traits this can only be reached by a race, so it is tested on the readiness.
- An error queue read before every receive (minor): gone. ICMP and raw now receive in the order of the shared new
  `datagram::exchange::receive_in_order`, as the UDP track's receive does: the error queue first only while reports
  a send asked for may wait there (`ErrorQueue::read_first`, until settled) or entries a send took are kept; else the
  packet queue, and the error queue when Linux answers the receive with a socket error. Read in the kernel: every
  ICMP error a ping or raw socket with IP_RECVERR queues sets the socket error (ping_err, raw_err, rawv6_err), a
  receive answers it before any packet, and reading an entry off the queue sets it again while the next entry is an
  ICMP error (sock_dequeue_err_skb). So a packet costs one recvmsg (ICMP also its SO_MEMINFO, counted). A send that
  fails with an error other than would-block or too long keeps the host's entries, since Linux may have answered it
  with that socket error: measured, a ping send after an unread ICMP error fails with EHOSTUNREACH. New checks in
  both suites: icmp_errors_queued_together_each_arrive (three errors before any receive, each received, then
  nothing), and in the ICMP suite an_icmp_error_a_send_met_still_arrives.
- Status count of reviewed files (minor): 31, corrected below.
- ICMP multicast loopback over IPv6 (minor): memberships_are_taken now sends an IPv6 echo to the joined group with
  loopback off (no reply) and on (a reply), as for IPv4. IPv4 all-groups on a ping socket stays listed as without a
  visible effect.

## Independent check, run 4 (23:11 to 23:18 UTC, load 1.4 to 4.5, `independent-check-i2d.sh` then `findings-run4.sh`)
- Clean build in a fresh target folder; clippy with warnings denied clean on the nine packages; three rounds, each
  ICMP 51 of 51, raw 42 of 42, resolver 31 of 31, Configurable 2 of 2, NegativeRouteAdvice 3 of 3 and the four crate
  tests (34 s with the build); the shared crate's readiness test 1 of 1.
- 57 broken copies, each caught (none failed to apply): the 43 of runs 1 to 3 and the 14 of this run (13 copies, two
  of them run on both suites): IPv6 alert never taken off (an_ipv6_router_alert_takes_forwarded_packets),
  IP_RECVOPTS off (an_echo_reply_arrives_with_its_ipv4_options), Include written as Exclude of none and Exclude's
  sources dropped (multicast_source_filters_are_written, multicast_source_filters_choose_the_senders), raw drops told
  as the total (ip_settings_are_taken), IPv6 multicast loop always on (memberships_are_taken), no error queue read
  after a receive error (the ICMP error checks of both suites and icmp_errors_queued_together_each_arrive), a failed
  ping send keeping no entries (an_icmp_error_a_send_met_still_arrives), reports not read first (both suites' transmit
  report and rejected time checks), a hold that wakes nothing (a_held_error_wakes_a_waiting_call).
- cargo tree (normal, build and dev edges) of the 15 interface crates and 13 conformance crates against the 17
  implementation crates: none named (control: the raw implementation's tree names itself and the shared crate).
- The UDP suite (75 and 5) passes with this run's shared crate change. The TCP suite failed once (99 of 100,
  fixed_flow_label_marks_every_segment, its connect at line 789) and passed three times right after (101 of 101,
  23:18 UTC, load 8 to 9; the suite had grown meanwhile): taken as the TCP track's, which the change here (new
  functions and one field in `exchange.rs`) does not reach.
- 40 rounds of both whole suites side by side (`flake-hunt.sh`, 23:15 to 23:18 UTC, load 1.4 to 4.5): no failure.
- Reviewed files: no commit since 0033b9a1 changes any of the 31 files; in the working tree crates/abi/host.rs has
  another track's uncommitted edit (10 lines removed), not this track's.
- Literal check: no finding in this track's files (19 in all; three name this track's constants: the UDP suite's
  copies of WAITED_SOCKETS and WAITS_ON_ONE, the TCP suite's restated LABEL).

## I2 run 3: the independent check's findings, each
- Raw received mark and priority checked by nothing (blocking): new raw check
  received_packets_carry_their_mark_and_priority (IPv4 and IPv6 loopback: mark 7 and priority 5 arrive, a plain
  send then 0 and 0). The verifier's broken copy (`SO_RCVMARK`, `SO_RCVPRIORITY` not turned on) fails it.
- ICMP neighbour confirmation accepted a refusal over IPv4 (blocking): only IPv6 may refuse now (Linux's IPv6 ping
  sockets drop the flag; IPv4's honour it). The verifier's broken copy fails a_confirmed_send_confirms_the_neighbour.
- No mobility header send (blocking): new raw check a_mobility_header_is_sent_by_its_type (an IPv6 raw socket of
  protocol 135 sends a binding refresh request with its type as selector; it leaves as given with the checksum
  Linux writes at offset 4). The broken copy that drops the mobility selector fails it.
- ICMP error cut inside its ICMP header got zeros (minor): the shared `icmp_error` now writes the ICMP header and
  its checksum into the headers before placing them, so any cut holds the right start, for every socket kind
  (UDP and TCP keep their calls). The ICMP and raw sockets also write the error from all of the entry's data
  (new `write_whole_icmp_error` and `write_whole_icmp_error_quoting_header`): with only what fit in the buffer, a
  buffer shorter than the quoted data told a length too short and a wrong checksum. New ICMP check
  a_short_buffer_keeps_the_start_of_an_icmp_error (an error quoting the whole request, cut in the outer header,
  the ICMP header and the quoted message, IPv4 and IPv6, each cut equal to the start of the whole error); the raw
  icmp_errors_arrive_whole cuts the same ways, and once shorter than the quoted data.
- IPv4 error's outer header fields unchecked (minor): both suites now compare the error's outer IP header with the
  one on loopback's wire (all but the IPv4 identification and fragment fields, which Linux does not give, and the
  checksum over them, which must hold). The broken copies with the IPv4 TTL or the raw IPv6 hop limit flipped fail.
- Raw suite had no IPv6 short buffer or ICMP error (minor): a_short_buffer_keeps_the_whole_length now cuts IPv4 and
  IPv6 packets (shorter than the payload, in the header, at its end, in the payload; each cut equal to the start of
  the whole but for the IPv4 identification and checksum, which differ per send); icmp_errors_arrive_whole sends
  IPv6 payloads and IPv6 packets with their own header too. Broken copies that tell or give an IPv6 length cut to
  the buffer fail it.
- Three vectors per IPv6 send (minor): gone; the shared `send.rs` hands the sticky options the header slices.
- The shared readiness's rewrite (edge-triggered, d2c4f3ad, the UDP track's) is committed: the wait checks and the
  allocation tests were run again on it (see Results).

## Independent check, run 3 (22:34 to 22:38 UTC, load 2.1 to 4.5, `independent-check-i2d.sh`)
- Clean build in a fresh target folder; clippy with warnings denied clean on the nine packages; three rounds, each
  ICMP 47 of 47, raw 40 of 40, resolver 31 of 31, Configurable 2 of 2, NegativeRouteAdvice 3 of 3 and the four crate
  tests (34 s with the build).
- 43 broken copies, each caught: the 31 of runs 1 and 2 (same checks as before) and the 12 of run 3: received
  mark and priority not turned on (received_packets_carry_their_mark_and_priority), IPv4 confirmation refused
  (a_confirmed_send_confirms_the_neighbour), mobility selector dropped (a_mobility_header_is_sent_by_its_type), the
  shared error writing zeros into a buffer that cuts its headers (ICMP a_short_buffer_keeps_the_start_of_an_icmp_error,
  raw icmp_errors_arrive_whole), an ICMP, a raw and a raw header-quoting error written from the cut data alone
  (the same checks), the ICMP IPv4 error's TTL flipped (an_icmp_error_arrives_whole_over_ipv4), the raw IPv6 error's
  hop limit flipped (icmp_errors_arrive_whole), a raw IPv6 length told or given cut to the buffer (both
  a_short_buffer_keeps_the_whole_length).
- cargo tree (normal, build and dev edges) of the 15 interface crates and 13 conformance crates against the 17
  implementation crates: none named (control: the raw implementation's tree names itself and the shared crate).
- The UDP suite (75 and 5) and the TCP suite (98) pass with this run's shared crate changes.
- Reviewed files: none of the 31 files (every interface.rs and the data files beside them) differs since run 2's
  0033b9a1 or in the working tree.
- Literal check (after `cargo fetch` there): no finding in this track's files (8 in all; two are the UDP suite's
  copies of WAITED_SOCKETS and WAITS_ON_ONE).

## I2 run 2: the independent check's findings, each
- IPv6 source preferences checked by nothing (blocking): both suites now give the dummy link a temporary
  address beside fd03::1 (use_tempaddr 1, `mngtmpaddr`), and expect it for Temporary and fd03::1 for Public
  (ICMP source_preferences_are_taken; raw ipv6_source_preferences_choose_the_source, moved out of
  payload_routes_take_their_selectors). The verifier's broken copy (preferences ignored in the shared send.rs)
  now fails both. The temporary address is taken only once it is no longer tentative (`-tentative`): Linux runs
  its detection later, and one round in forty failed before that (the resolver suite's check had the same gap).
- ICMP receive into a short buffer (blocking): Linux's ping_recvmsg cuts to the buffers given and returns what it
  copied, MSG_TRUNC or not. The receive now gives a tail buffer behind the caller's (64 KiB on the stack,
  uninitialised, filled only past the caller's buffer), so the length and the rebuilt header's total length are
  whole. The check now cuts the reply inside the IP header and inside the ICMP message, IPv4 and IPv6, and reads
  the whole length from the given header bytes too; the broken copy without the tail fails it.
- The ICMP-error wait flake (blocking): not reproduced. See "The wait flake" below for what was run and found.
- IPv6 traffic class of a received raw packet (minor): ipv6_packets_arrive_with_their_headers now sends DSCP AF41
  and ECN Ect0 and checks them in the rebuilt header; the broken copy (`set_recv_tclass_v6(false)`) fails it.
- Per-packet cost of an ICMP receive (minor): the drop count is one `SO_MEMINFO` call per packet, now counted
  under its own reason (DROPS_ASKED); that count and RECEIVED_FIELDS_NOT_GIVEN are atomics in the value, so a
  receive takes no lock for them (the shared `Shortfalls` locks a map). The crate's
  a_received_packet_counts_what_linux_does_not_give checks both counts.
- Receive on libc (minor): `recvmsg` stays libc in the shared crate because socket2 0.6.5's `RecvFlags` gives no
  `MSG_CTRUNC` (read again in its source: only is_truncated, is_end_of_record, is_out_of_band, is_confirm,
  is_dontroute); the UDP track says the same.
- Allocation per wait (minor): each implementation crate has tests/waiting.rs, which counts the waiting thread's
  allocations over 100 waits that have to wait (poll, event, poll to the end, drop), after 100 that warm the
  socket: none. A broken copy that gives the waiter list's room back (`shrink_to_fit` after a withdraw) fails
  both. The counting allocator and the measuring loop are in the ICMP suite (`counting_allocator!`,
  `wait_allocations`).
- Broken copies of earlier runs were partly stale: each was built in one shared folder from a copy whose
  restored files kept their old times, which cargo takes as unchanged, so a broken copy could still carry the
  one before it (it showed here: the source preference break lingered in three later copies). The scripts now
  touch every source file of the copy. This may explain the verifier's ICMP wait failure under the "drops
  always zero" copy, which passed from a clean build.

## The wait flake
- What the check saw: the raw an_icmp_error_ends_a_wait_for_data failed in 2 of 20 rounds at load 12 to 27, text
  not kept. Its code (`ip`, a capture, one send, one ICMP error from a raw ICMP socket of the namespace, a wake
  awaited for 2 s) has no step that should take seconds; the readiness's one-shot arming was read for a lost or
  late wake and none found (each report re-reads the socket and re-arms for any waiter left).
- The shared readiness changed two minutes before that check ran (12b98f7f, registration only while a call
  waits, by the UDP track), so the failing rounds may have run on it while the earlier 20 clean rounds did not.
- Runs here (ARM machine, both suites side by side at 6 test threads each, every failure's text kept,
  `flake-hunt.sh`): 40 rounds at load 5 to 16, no wait check failed (one failure, the temporary address one
  above). Then 150 rounds of each suite, 21:52 to 22:00 UTC, load 2.4 to 26.2 (a peak from other tracks at
  21:58): no failure of any check. So 190 rounds here, none of the wait checks failed.
- Run 3: 60 more rounds of both suites on the edge-triggered readiness, no failure (250 rounds in all).
- Run 4: seen once more, the raw an_icmp_error_ends_a_wait_for_data in round 2 of 3 at load 1.9 to 2.2 (23:05 UTC),
  "woken by the ICMP error" (which address was not told then; the message now names it). The 40 rounds after, and
  the 3 rounds of each later run, passed. Still no fault found; the shared readiness is the UDP track's.
- Found in I2 run 6 (above): a lost fact, not a lost wake. An ICMP error sent to a `nodad` IPv6 address before
  Linux had routed it locally left by the dummy link. The suites now wait for the local route after adding an
  IPv6 address; the patience was not raised.

## Independent check, run 2 (21:54 to 21:57 UTC, load 3.3 to 8.1)
- Clean build in a fresh target folder; clippy with warnings denied clean on the nine packages; three rounds, each
  ICMP 46 of 46, its counter test and allocation test, raw 38 of 38, its counter test and allocation test, resolver
  31 of 31, Configurable 2 of 2, NegativeRouteAdvice 3 of 3 (34 s with the build).
- 31 broken copies (`independent-check-i2c.sh`, each copy's files made new so no earlier break lingers), each
  caught: the 24 of run 1 (same checks as listed below, but for raw kept errors not held, now caught by
  a_wait_sees_an_error_a_send_read_first alone), and source preferences ignored in the shared send.rs (ICMP
  source_preferences_are_taken, raw ipv6_source_preferences_choose_the_source), ICMP receive without the tail
  (a_short_buffer_keeps_the_whole_length), drop count not counted (a_received_packet_counts_what_linux_does_not_give),
  raw IPv6 traffic class not received (ipv6_packets_arrive_with_their_headers), waiter room given back after each
  withdraw (both a_wait_allocates_nothing_once_warm).
- cargo tree (normal, build and dev edges) of the 15 interface crates and 13 conformance crates against the 17
  implementation crates: none named (control: the raw implementation's tree names itself and the shared crate).
- Reviewed files: none of the 31 files beside the interfaces (interface.rs and their data files) is touched by this
  run's commits or differs in the working tree; since f3b36cd4 only crates/abi/interface.rs and host.rs changed,
  in the kernel track's 4ee5def7.
- Literal check: no finding in this track's files (12 in all, elsewhere; two are the UDP suite's copies of
  WAITED_SOCKETS and WAITS_ON_ONE).

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
- Source preferences covered nothing (only answered in 2026-11-22's run: until then a refusal passed it): the check now makes a temporary address (`mngtmpaddr` with use_tempaddr 1,
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
  - `i2-run6/check.sh`: broken copies for run 6's counter tests (`NO_MUTANTS=1` skips them), then the raw and ICMP
    suite executables in turn `ROUNDS` times (default 150) at 12 threads, each failure's text kept.
    `i2-run6/diag-hunt.sh`: the same turns with a patched ICMP suite that prints the namespace's counters, routes,
    links and what left lo when an ICMP error is missing. `i2-run6/dad-window2.sh`: how often a `nodad` address is
    not yet routed locally right after it is added.
  - `i2-run4/check.sh`: clean build of the nine packages with clippy, `ROUNDS` (default 3) of every suite, broken
    copies of the run's fixes and three of the ICMP and raw hosts, the TCP suite's advice tests, the UDP build
    state, the lookup cost, cargo tree (`NO_MUTANTS=1` stops after the rounds). `i2-run4/check2.sh`: the same
    for the resolver and the generic suites only, and the lookup cost (the throwaway crate needs
    `RUSTC_BOOTSTRAP=1`, which the workspace's .cargo/config.toml sets for itself).
  - `i1-run5/check.sh`: I1's clean build with clippy, three rounds, four broken copies, cargo tree of every interface
    and conformance crate.
  - `independent-check-i2b.sh`: clean build with clippy, three rounds, 24 broken copies, cargo tree.
  - `i1-linter/independent-check-i1b.sh`: I1's clean build with clippy, three rounds, 8 broken copies of the
    resolver, cargo tree, and the lookup cost (a throwaway crate in /work outside the checkout).
  - `i1-linter/hunt2.sh`: the resolver suite `ROUNDS` times (default 100), then a broken copy that sends.
  - `i1-run3/resolver.sh`: clippy on the four I1 packages, then their tests (`ROUNDS`, `FILTER`).
  - `i2-run2/check.sh`: clean build with clippy, three rounds, 64 broken copies of runs 1 to 5, cargo tree, the
    literal check; `i2-run2/check2.sh` the copies rewritten for linux.rs and the counter tests, `check3.sh` the
    allocation copies. `i2-run2/summaries.py <files>` lists summaries over one line or 150 characters.
  - `i2-run1/findings.sh`: the I1 packages' clippy and three rounds, then the seven broken resolver copies of the
    2026-11-22 findings (`NO_MUTANTS=1` skips them). Linter tools of that run: `i2-lint/*.py`.
  - `i1-run3/independent-check-i1d.sh`: I1's clean build, three rounds, 19 broken copies, vacuity, cargo tree.
  - `i1-run3/verify-run.sh`: the verifier's seeded comparison with connected sockets, three seeds of 20000.
  - `independent-check-i2d.sh`: clean build with clippy, three rounds, the 43 broken copies of runs 1 to 3, cargo
    tree, then the UDP and TCP suites (the shared crate is theirs too).
  - `findings-run2.sh` to `findings-run5.sh` (and `findings-run5b.sh`, `findings-run5c.sh`): the broken copies
    of each run's findings alone (`TESTS` picks the test targets of a copy).
  - `final-run5.sh`: lint-and-suites.sh, the shared readiness test, then the UDP and TCP suites.
  - `flake-wait.sh`: one check (`CHECK`, default the ICMP-error wait) of the raw and ICMP suites alone, `ROUNDS`
    process runs in each of `LOOPS` parallel loops, failures' text kept.
  - `run4-suites.sh`: lint-and-suites.sh and the shared crate's readiness test.
  - `flake-hunt.sh`: both suites side by side, `ROUNDS` times (default 20), `FILTER` to pick checks, each
    failure's text printed.
  - `readiness-fix.sh`: the wait checks twenty times, then the UDP and TCP suites.
  - `literals.sh`: the literal check in a throwaway git copy (the synced copy has no .git). Run `cargo fetch -q`
    first there: without linux-raw-sys's sources the check stops at one error line, which the script counts as 0.
  - `i2-run7/check.sh`: clippy and the raw tests, then broken copies of run 7's two fixes (`NO_MUTANTS=1` skips
    them). `i2-run7/final.sh`: clean build with clippy, `ROUNDS` (default 3) of every suite, cargo tree.
- Test tools put on the path there: iproute2 and nftables from nixpkgs (GPL tools, never dependencies).
- Cargo.lock: other tracks' entries are in the working file, so this track commits a lock built from HEAD's with
  `lock-edit.py` (renames and added dependencies, as cargo orders them) through a private index:
  `GIT_INDEX_FILE=<scratch>/idx git read-tree HEAD`, `git add` this track's paths there, `git update-index
  --cacheinfo 100644,$(git hash-object -w <lock>),Cargo.lock`, commit with the same variable, then
  `git reset -q -- <those paths> Cargo.lock` in the shared index.

## Results (ARM64 machine, Linux 7.0.0-1018-aws, 96 cores, 12 test threads)
- Run 5, 00:08 to 00:09 UTC, load 2.1 to 2.9, final code (`final-run5.sh`, `ROUNDS=3`): clippy with warnings denied
  clean; three rounds, each ICMP 55 of 55 (0.95 s), raw 44 of 44 (2.9 s), resolver 31 of 31, Configurable 2 of 2,
  NegativeRouteAdvice 3 of 3 and the allocation tests; the shared readiness test; UDP 75 and 5, TCP 106 of 106.
  Broken copies of this run (00:02 to 00:05 UTC, load 1.2 to 1.8): fragment size dropped, report interface 0,
  report length without the link header, Interface and Omit as Dont, Omit as Interface, Probe as Do, the ICMP and
  raw source checks answered as refusals, the interface and packet wish counts dropped: each caught. The literal
  check: 0 findings in all. No commit of this run touches a reviewed file or a manifest.
- Run 4, 23:20 UTC, load 6.1 to 10.9, final code: three rounds, each ICMP 51 of 51 (0.95 s), raw 42 of 42 (2.9 s),
  resolver 31 of 31, Configurable 2 of 2, NegativeRouteAdvice 3 of 3, the four crate tests and the shared readiness
  test; clippy with warnings denied clean.
- Run 3, 22:38 UTC, load 2.7 to 3.2, final code: three rounds, each ICMP 47 of 47 (0.61 s), raw 40 of 40 (2.87 s),
  resolver 31 of 31, Configurable 2 of 2, NegativeRouteAdvice 3 of 3 and the four crate tests; clippy clean.
- Run 3, on the edge-triggered readiness (d2c4f3ad), 22:31 to 22:35 UTC, load 1.5 to 3.3: 60 rounds of both whole
  suites side by side at 6 test threads each (`flake-hunt.sh`), no failure; the two allocation tests 20 times each,
  all pass (no allocation over 100 waits).
- Run 2, 22:02 UTC, load 5.5 to 5.8, final code: three rounds, each ICMP 46 of 46 (0.61 s), raw 38 of 38
  (2.87 s), resolver 31 of 31, Configurable 2 of 2, NegativeRouteAdvice 3 of 3, and the four crate tests
  (two counter tests, two allocation tests) pass; clippy with warnings denied clean.
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
- I2 run 5 adds: raw configure (the checksum's offset by a_checksum_offset_is_written and
  a_checksum_offset_is_checked_on_receipt), raw send_to and receive (the same receipt check, and
  a_header_of_the_other_version_gets_the_hosts_error).
- Run 5 adds: receive (both largest fragment checks, the report interface and length in check_loopback_reports),
  send_to (each_path_mtu_discovery_mode_takes_its_mtu in both suites, the absent source's host error).
- Run 4 adds: receive (an_echo_reply_arrives_with_its_ipv4_options, icmp_errors_queued_together_each_arrive,
  an_icmp_error_a_send_met_still_arrives; raw drops over two floods), configure (both source filter checks, the
  IPv6 alert turned off and changed, ICMP IPv6 multicast loopback).
- IcmpSockets: open (every check; owner: owner_selects_the_route), bind_identifier
  (identifiers_are_shared_only_when_both_ask, the_request_carries_the_bound_identifier), send_to (every sending
  check; each header and send field its own, below), receive (echo reply, ICMP error, short buffer of a reply or
  an error, would block,
  transmit report and rejection, drops checks), count_unsent_bytes (pacing_holds_messages_in_the_send_buffer),
  Readiness::wait_until_ready (the readiness suite's six for data and six for writing, a_wait_for_writing_finishes_at_once, waits_on_many_sockets_each_wake_once, a_wait_sees_an_error_a_send_read_first, the never-had fact; run 2's broken wait fails eight), close (close_frees_the_identifier), configure (each setting's check and
  Configurable's suite).
- RawSockets: open (every check, open_takes_each_version_and_protocol), send_to (payload, header included,
  reassembly, route selector, mobility header, path MTU, send field checks), receive (arrival, mark and
  priority, short buffer, would block, ICMP error, router alert checks), count_unsent_bytes (ip_settings_are_taken), Readiness::wait_until_ready (the readiness suite's six for data and six for writing, a_wait_for_writing_finishes_at_once, waits_on_many_sockets_each_wake_once, a_wait_sees_an_error_a_send_read_first, the never-had fact; run 2's broken wait fails eight),
  close (close_gives_up_the_sockets_holdings), configure (checksum, router alert, multicast routing, IP and
  multicast settings, Configurable's suite).
- RouteResolver::resolve_route: 28 suite checks that see answers, each selector its own; 3 see only refusals
  here, and 2 see refusals in their last part (see I1 runs 3 and 4 above). I2 run 4 adds the path MTU cap and the
  per-packet interface checks.
- NegativeRouteAdvisable::give_negative_route_advice: advice_is_taken (an answer other than Ok or NotSupported)
  and advice_is_taken_again (a second answer unlike the first).

## Honoured, and how each is checked
- Header fields, by what leaves a dummy link (packet socket) or what a rule does: IPv4 TTL, DSCP, ECN, options,
  source, waived source check, interface, link scope, broadcast permission, mark and owner, ICMP type and code,
  path MTU modes (the don't-fragment bit, and which MTU each takes, fragments to or is too long for), too long (`MessageTooLong` with the link MTU). IPv6 hop limit, DSCP, ECN, flow label, source
  preferences (a temporary address against the public one; care-of as the host's own socket sends it), local fragmentation and fragment MTU, and for raw sockets source, hop-by-hop, routing destination
  and destination options and a segment routing header. Received IPv4 options (an echoed record route).
- Send fields: neighbour confirmation, priority (third band of a prio queue), transmit time (fq holds 300 ms;
  rejected times reported with their keys, among them a Realtime send etf drops as of another clock), transmit
  reports scheduled and software with key, with and without the packet, and with it the loopback link and the
  frame's length. Hardware, acknowledgement and completion reports are passed to Linux, which makes none here (below).
- Received fields: interface, specific destination, largest fragment (none for a whole packet), software timestamps, drops since previous, mark and priority
  (raw), whole IPv4 and IPv6 headers with extension headers (raw IPv6 traffic class checked), the whole length and
  the start when the buffer cuts the packet anywhere; ICMP errors whole, their outer header as on the wire, and
  their start and whole length when cut. The received traffic class of an echo reply (both versions) and of an
  ICMP error's outer header (both suites) is checked with a request or error sent with DSCP EF and ECN.
- Raw only: a packet with its own header leaves byte for byte; its route takes the flow's protocol and mark; a
  mobility header is sent by its type, and its type is what a rule sees (a mismatched selector refused);
  withdrawn reassembly; checksum offset (where it is written, and a wrong checksum dropped on receipt); IPv4 and IPv6 router alerts with isolation, turned off and changed, and a refused change keeps the old alert; multicast routing.
- Settings: multicast membership, its source filters (as Linux lists them, and IPv4 raw delivery) and loopback
  (both versions), IPv4 all-groups, anycast, pacing rate, send and receive buffer,
  timestamping. Every setting kind also passes Configurable's suite.
- Waits, through the host's poll: for data and for writing, each through the readiness suite (ready and failing
  bundles), a wait for writing on a new socket at once; ERROR and HANG_UP end waits that did not ask; URGENT_DATA
  and PEER_FINISHED_SENDING never come on these sockets and a wait for them waits on (checked).
- A header of the other version than an ICMP or raw socket's goes to the host as its address alone (IPv4-mapped on
  an IPv6 socket), and the host's error is the answer: ICMP EINVAL both ways (measured on the ARM machine's 7.0
  kernel); raw as the host's own raw socket answers (checked).

## Not honoured, with the Linux reason
- ICMP over IPv6 (net/ipv6/ping.c), refused with NotSupported, counted and warned: a source address, extension
  headers and routing header, neighbour confirmation.
- Both kinds: an upper-layer selector other than what Linux routes the send by, and a raw payload under another
  protocol than the socket's (IPv4; IPv6 unless the socket's is `IPPROTO_RAW`).
- Ping sockets give no `SO_RXQ_OVFL`, `SO_RCVMARK` or `SO_RCVPRIORITY`: mark and priority are written as 0 and
  counted (RECEIVED_FIELDS_NOT_GIVEN), drops are read from `SO_MEMINFO` at each receive (totals exact, checked).
  So a packet is told every drop up to its receive, also those after it was queued, which Linux's own per-packet
  count would give the next packet; the extra system call and this are counted and warned as DROPS_ASKED.
- A care-of source preference is passed to Linux (`IPV6_PREFER_SRC_COA`) but chooses as Home here: Linux prefers a
  care-of address only over a Mobile IPv6 home address (CONFIG_IPV6_MIP6), which no link has. Checked only against
  the host's own socket with the same preference.
- ICMP errors: Linux gives no identification or fragment flags of an IPv4 one, nor (under a payload) the quoted
  header's fields but its addresses: written as zero and counted (ICMP_ERROR_FIELDS_NOT_GIVEN).
- Multicast all-groups: no effect to see on a ping socket (it takes only echo replies) or an IPv6 raw socket
  bound to no address (it takes every multicast packet of its protocol). The same holds for source filters' effect
  on delivery: what Linux holds of them is checked from its lists, and their effect on IPv4 raw sockets.
- A packet an IPv6 router alert socket of `IPPROTO_RAW` takes is told forwarded (given whole) by its bytes: Linux
  gives no sign. A local packet of protocol 255 whose payload repeats its own IPv6 and hop-by-hop headers is taken
  for a forwarded one and given without its outer headers (see I2 run 4). Each packet taken as forwarded is counted
  and warned (FORWARDED_BY_ITS_BYTES).
- Link-layer length: Linux gives `SCM_TIMESTAMPING_PKTINFO` only with a hardware receive timestamp
  (__sock_recv_timestamp), which needs a device with a hardware clock; the namespace has none, so it stays None.
- A transmit report without its packet: Linux gives no `IP_PKTINFO` with it (`SOF_TIMESTAMPING_OPT_TSONLY`), so its
  interface is told as 0, counted and warned (REPORT_INTERFACE_NOT_GIVEN).
- Whether reports hold the packet is a socket flag Linux reads when it takes each timestamp: a send that changes
  it while earlier sends wait in the host changes their reports too; counted and warned (PACKET_WISH_CHANGED).
- IPv6 path MTU modes Probe and Interface send alike here; they differ only in taking learnt path MTUs.
- Transmit clock Realtime against Monotonic: honoured (`CLOCK_REALTIME`) but not told apart by any check. Linux's
  fq reads every transmit time as monotonic whatever the socket's clock, and etf takes only `CLOCK_TAI` and drops
  any other socket clock alike; only `getsockopt(SO_TXTIME)` shows it, which the traits do not give. A Realtime
  mapped to the atomic clock is caught.
- Wireless status reports: refused as in the UDP track, counted and warned.
- Hardware, acknowledgement and completion reports: accepted, not refused. They are passed to Linux and the send
  answers as Linux does, but no report arrives: acknowledgement reports are TCP's, and hardware and completion
  reports need a device that makes them, which the namespace has not. Not counted: it is Linux's own answer.
- Measured Linux behaviour the suites follow: a raw send into a full send buffer answers `ENOBUFS`;
  `IPV6_DONTFRAG` holds for an IPv6 raw payload of another protocol than UDP or ICMPv6 only in mode `Do`;
  `IPV6_MTU` fragments nothing smaller than the path MTU; IPv6 router alerts reach only `IPPROTO_RAW` sockets.

## For the other socket tracks
- UDP, new in I2 run 4: your receive order holds for several ICMP errors too, which I had doubted: reading an entry
  off the error queue sets the socket error again while the next entry is an ICMP error (sock_dequeue_err_skb), so
  each receive is answered with an error while one waits. ICMP and raw now use the same order through the shared
  `datagram::exchange::receive_in_order` (with `ErrorQueue::read_first` and `reports_requested`); your code is
  unchanged and could use it. The ICMP and raw suites' icmp_errors_queued_together_each_arrive checks it.
- UDP and TCP: the readiness fix above (56f2302d) is in the shared crate; your suites pass with it.
- UDP: the literal check now finds your WAITED_SOCKETS and WAITS_ON_ONE (32 and 3) redefine the ICMP suite's
  public ones; import them from wcontainers-icmp-sockets-conformance, or say which crate should own them.
- TCP: your FLOW_LABEL 0x12345 restates the resolver suite's LABEL, and "lo" its SELF_LINK; say if you want
  LABEL public at the top of the resolver suite.
- UDP and TCP, new in I2 run 3: `icmp_error::write_icmp_error` now writes the ICMP header (with its checksum)
  before placing the headers, so a buffer that cuts an error inside its ICMP header holds the right bytes; your
  calls are unchanged. When the buffer is shorter than the data Linux gave, an error written from the `copied`
  bytes alone has a short length and a wrong checksum (and UDP's quoted UDP length is `copied` too):
  `write_whole_icmp_error(into, &entry.data, ...)` writes it from all of the entry. ICMP and raw use it.
- The ICMP suite's `counting_allocator!` and `wait_allocations` count a waiting call's allocations for any
  socket kind.
- The ICMP suite exports its wait tools (`Wakes`, `poll_with`, `finished`, `finished_soon`, `woken`, each on
  `ReadinessReport`) and the rejected transmit time tools for any suite to use.

## Departures from the plan
- The plan named I2 ICMP and I3 raw; the task made both one milestone.
- The suites share test tools across suite crates (raw on ICMP and resolver, ICMP on resolver) so each fact
  has one definition.
- The generic suites test themselves with private implementors under `#[cfg(test)]`: a named test double would
  go under crates/configurable/implementation/ and crates/negative-route-advice/implementation/, which this
  track does not own, and no other suite needs one.
- The shared readiness (the UDP track's file) was changed here, in a commit of its own, for the double wake.
- Raw multicast routing Disabled on a socket that is not the router answers Ok, from the socket's state, where
  Linux's `MRT_DONE` answers EACCES: Configurable's rule (setting a value twice equals setting it once) needs a
  second Disabled after the one that ended routing to answer Ok, and a new socket starts Disabled.

## For the owner
- The route selectors' `interface` does not say whether it names the interface per packet (as a send's
  `IP_PKTINFO`/`IPV6_PKTINFO` and `IP_UNICAST_IF` do) or binds to the device (`SO_BINDTODEVICE`). Over IPv6 with a
  source they differ. Example: in the resolver suite's network, fd09::5 from fd02::1 naming db is routed by da
  (path MTU 1400) per packet, and fails with ENETUNREACH for a socket bound to db. The host resolver and the
  suite take the per-packet meaning, since the same selectors are a send's header route and the socket hosts send
  them per packet.
- The linter's public_outside_interface wants every field of a type that fills an associated type pub ("plain
  data"), which the brief's private fields forbid, and on Linux the socket holds a registered descriptor and its
  state, which pub fields would let anyone build or change. Example: crates/icmp-sockets/implementation/host/
  implementation.rs, `HostIcmpSocket.never` (the portable build's `Infallible`; the Mac is where the linter runs).
- The linter asks for `mod readiness { ...::tests!($crate::<function>($value)); }` with a function giving
  `(T, T::Socket)`, but the readiness suite's `tests!` takes a module name, a ready value and a failing value. Example:
  crates/icmp-sockets/conformance/conformance.rs, `waiting_for_a_reply` and `waiting_for_data_until_an_icmp_error`.
- The linter's implementation_folder reports `impl Readiness<HostIcmpSocket> for HostIcmpSockets` (and Configurable)
  in the ICMP and raw host crates, though IcmpSockets requires both as supertraits, so the implementation cannot
  live anywhere else. Example: crates/icmp-sockets/implementation/host/linux.rs, `impl Readiness`.
- The route resolver contract does not say how an IPv4-mapped IPv6 destination routes; the host implementation
  resolves it over IPv4 as a dual-stack socket sends (::ffff:10.9.1.1 by da from ::ffff:10.1.0.1).
- No interface can build its implementation. Proposed, as `StandaloneRuntime` does:
  `pub trait StandaloneIcmpSockets: IcmpSockets + Sized { fn open_host() -> Result<Self, HostError>; }`, the same
  for `RawSockets` and `RouteResolver`. Meanwhile `new()` (fallible: it starts the readiness thread) and the
  resolver's `open()`, which the linter reports as constructors.
- The route resolver contract does not say whose routes a lookup answers for. The layout's `tests!` makes the value
  before a suite function runs, so the suite can only move the thread into its own network namespace after; it so
  requires a resolver to answer for the calling thread's namespace at each lookup. Example: a resolver that bound
  one routing socket when made answers 127.0.0.1 from the host's table, not the check's.
- The layout gives no home to test tools two suites share (a conformance crate may export only suite functions, and
  may not be a normal dependency of another), so the ICMP and resolver suites each keep their iproute2 wrappers,
  and `constant_defined_twice` reports seven of their constants. Example: `IP = "ip"` in both.
- `Readiness::wait_until_ready` with `ReadinessReport` (a00ab267) fits as it stands; nothing found that does not work.
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
- `TransmitReport.interface` is a plain index, but Linux gives none with a report that does not hold its packet:
  an `Option` would say so. Example: a ping to 127.0.0.1 with a software report and `packet: NotRequested` reads
  interface 0 (with the packet, 1).
- The contract does not say what `TransmitReport.packet_length` counts: Linux's is the frame, link-layer header
  included (58 bytes for a 24-byte echo over IPv4 on loopback, whose IP packet is 44); the suites expect that.

## Dependencies (licences checked)
- socket2 0.6.5 (MIT OR Apache-2.0), now named by the ICMP and raw crates too; polling 3.11.0 (Apache-2.0 OR
  MIT) through the shared crate.
- netlink-packet-core 0.9.0, netlink-packet-route 0.34.0, netlink-sys 0.9.0 (MIT); rustix 1.1.6 (Apache-2.0 WITH
  LLVM-exception OR Apache-2.0 OR MIT), linux-raw-sys 0.12.1 (same), libc 0.2.191 (MIT OR Apache-2.0), etherparse
  0.21.0 (MIT OR Apache-2.0). iproute2 and nftables (GPL) are test tools the suites run, never dependencies.
