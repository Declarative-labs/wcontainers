# Status: host internet sockets

Plan: docs/plan-host-internet-sockets.md. Milestones: I1 route resolver and the two generic suites (done), I2 ICMP
and raw sockets (redone in run 1 after the reorganization and the async ruling of 2026-11-21; runs 2 and 3
answered the independent checks' findings, below; run 4 those of run 3's check). Nothing of this track is in progress.

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
  - `independent-check-i2d.sh`: clean build with clippy, three rounds, the 43 broken copies of runs 1 to 3, cargo
    tree, then the UDP and TCP suites (the shared crate is theirs too).
  - `findings-run2.sh`, `findings-run3.sh`, `findings-run4.sh`: the broken copies of each run's findings alone
    (`TESTS` picks the test targets of a copy).
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
- RouteResolver::resolve_route: all 30 suite checks, each selector its own (see the I1 broken copies above and in
  git history of this file).

## Honoured, and how each is checked
- Header fields, by what leaves a dummy link (packet socket) or what a rule does: IPv4 TTL, DSCP, ECN, options,
  source, waived source check, interface, link scope, broadcast permission, mark and owner, ICMP type and code,
  path MTU modes, too long (`MessageTooLong` with the link MTU). IPv6 hop limit, DSCP, ECN, flow label, source
  preferences (a temporary address against the public one), local fragmentation and fragment MTU, and for raw sockets source, hop-by-hop, routing destination
  and destination options and a segment routing header. Received IPv4 options (an echoed record route).
- Send fields: neighbour confirmation, priority (third band of a prio queue), transmit time (fq holds 300 ms;
  rejected times reported with their keys), transmit reports scheduled and software with key, with and without
  the packet.
- Received fields: interface, specific destination, software timestamps, drops since previous, mark and priority
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
  for a forwarded one and given without its outer headers (see I2 run 4).
- Largest fragment and link-layer length: filled by the shared code as for UDP, whose suite checks them.
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
