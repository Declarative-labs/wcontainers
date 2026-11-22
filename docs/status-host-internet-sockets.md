# Status: host internet sockets

Plan: docs/plan-host-internet-sockets.md. Milestones: I1 route resolver and the two generic suites (done; run again
on 2026-11-21 evening for the owner's linter and twice on 2026-11-22 for the checks' findings, below), I2 ICMP and raw sockets (redone in run 1 after the
reorganization and the async ruling of 2026-11-21; runs 2 to 5 answered the independent checks' findings). Nothing of
this track is in progress.

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
- Left as it is: no fault found to fix, and the check's patience (2 s for one local ICMP error) was not raised,
  since raising it would only hide a lost wake. If it fails again, flake-hunt.sh keeps the panic's text, which
  names the step (no capture, no wake, no error event, or the receive).

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
  - `independent-check-i2b.sh`: clean build with clippy, three rounds, 24 broken copies, cargo tree.
  - `i1-linter/independent-check-i1b.sh`: I1's clean build with clippy, three rounds, 8 broken copies of the
    resolver, cargo tree, and the lookup cost (a throwaway crate in /work outside the checkout).
  - `i1-linter/hunt2.sh`: the resolver suite `ROUNDS` times (default 100), then a broken copy that sends.
  - `i1-run3/resolver.sh`: clippy on the four I1 packages, then their tests (`ROUNDS`, `FILTER`).
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
  - `literals.sh`: the literal check in a throwaway git copy (the synced copy has no .git).
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
  wait_for_events (the six wait checks), close (close_frees_the_identifier), configure (each setting's check and
  Configurable's suite).
- RawSockets: open (every check, open_takes_each_version_and_protocol), send_to (payload, header included,
  reassembly, route selector, mobility header, path MTU, send field checks), receive (arrival, mark and
  priority, short buffer, would block, ICMP error, router alert checks), count_unsent_bytes (ip_settings_are_taken), wait_for_events (the six wait checks),
  close (close_gives_up_the_sockets_holdings), configure (checksum, router alert, multicast routing, IP and
  multicast settings, Configurable's suite).
- RouteResolver::resolve_route: 28 suite checks that see answers, each selector its own; 3 see only refusals
  here, and 2 see refusals in their last part (see I1 runs 3 and 4 above).

## Honoured, and how each is checked
- Header fields, by what leaves a dummy link (packet socket) or what a rule does: IPv4 TTL, DSCP, ECN, options,
  source, waived source check, interface, link scope, broadcast permission, mark and owner, ICMP type and code,
  path MTU modes (the don't-fragment bit, and which MTU each takes, fragments to or is too long for), too long (`MessageTooLong` with the link MTU). IPv6 hop limit, DSCP, ECN, flow label, source
  preferences (a temporary address against the public one), local fragmentation and fragment MTU, and for raw sockets source, hop-by-hop, routing destination
  and destination options and a segment routing header. Received IPv4 options (an echoed record route).
- Send fields: neighbour confirmation, priority (third band of a prio queue), transmit time (fq holds 300 ms;
  rejected times reported with their keys), transmit reports scheduled and software with key, with and without
  the packet, and with it the loopback link and the frame's length.
- Received fields: interface, specific destination, largest fragment (none for a whole packet), software timestamps, drops since previous, mark and priority
  (raw), whole IPv4 and IPv6 headers with extension headers (raw IPv6 traffic class checked), the whole length and
  the start when the buffer cuts the packet anywhere; ICMP errors whole, their outer header as on the wire, and
  their start and whole length when cut.
- Raw only: a packet with its own header leaves byte for byte; its route takes the flow's protocol and mark; a
  mobility header is sent by its type;
  withdrawn reassembly; checksum offset; IPv4 and IPv6 router alerts with isolation, turned off and changed; multicast routing.
- Settings: multicast membership, its source filters (as Linux lists them, and IPv4 raw delivery) and loopback
  (both versions), IPv4 all-groups, anycast, pacing rate, send and receive buffer,
  timestamping. Every setting kind also passes Configurable's suite.
- Waits: every event poll gives, through the host's poll.

## Not honoured, with the Linux reason
- ICMP over IPv6 (net/ipv6/ping.c), refused with NotSupported, counted and warned: a source address, extension
  headers and routing header, neighbour confirmation.
- Both kinds: an upper-layer selector other than what Linux routes the send by, and a raw payload under another
  protocol than the socket's (IPv4; IPv6 unless the socket's is `IPPROTO_RAW`).
- Ping sockets give no `SO_RXQ_OVFL`, `SO_RCVMARK` or `SO_RCVPRIORITY`: mark and priority are written as 0 and
  counted (RECEIVED_FIELDS_NOT_GIVEN), drops are read from `SO_MEMINFO` at each receive (totals exact, checked;
  the extra system call counted as DROPS_ASKED).
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
- Wireless status reports: refused as in the UDP track. Acknowledgement reports are TCP's; hardware and
  completion reports need a device with a hardware clock, which the namespace has not.
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
  for `RawSockets` and `RouteResolver`. Meanwhile `new()` (fallible: it starts the readiness thread) and the
  resolver's `open()`, which the linter reports as constructors.
- The route resolver contract does not say whose routes a lookup answers for. The layout's `tests!` makes the value
  before a suite function runs, so the suite can only move the thread into its own network namespace after; it so
  requires a resolver to answer for the calling thread's namespace at each lookup. Example: a resolver that bound
  one routing socket when made answers 127.0.0.1 from the host's table, not the check's.
- The layout gives no home to test tools two suites share (a conformance crate may export only suite functions, and
  may not be a normal dependency of another), so the ICMP and resolver suites each keep their iproute2 wrappers,
  and `constant_defined_twice` reports seven of their constants. Example: `IP = "ip"` in both.
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
