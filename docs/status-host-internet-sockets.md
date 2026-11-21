# Status: host internet sockets

Plan: docs/plan-host-internet-sockets.md. Milestones: I1 route resolver and the two generic suites (done, three
runs, then run 1 after the reorganization and the async ruling, below), I2 ICMP and raw sockets (done in run 1
of I2 before the async ruling; to redo for it, see "Next").

## I1 after the reorganization (2026-11-21, run 1): done
- The resolver is `crates/route-resolver/implementation/host`, package `wcontainers-route-resolver-host`, type
  `HostRouteResolver` (folder moved with `git mv`). Its datagram sockets (the IPv6 source check's bind, the
  neighbour confirmation's `sendto` with `MSG_CONFIRM | MSG_PROBE`, the default TTL read) are socket2's; the
  unicast interface and `IPV6_RTHDR` stay the shared crate's `set_packet_setting` on the same descriptor,
  socket2 having neither; the routing socket stays netlink-sys's. The RouteResolver interface did not change
  in the async ruling, and needs no wait: Linux answers `RTM_GETROUTE` within the request's send (rtnetlink
  runs in the sender's context), so the read that follows has its answer queued (known from Linux's design,
  not measured).
- Independent check, `independent-check-i1.sh` in scratch (ARM64 machine, 20:35 to 20:36 UTC, load 0.5 to
  5.5): a fresh target folder, clippy with warnings denied clean on the four packages, then three rounds:
  resolver 28 of 28 (0.02 s each, 12 threads), Configurable self-tests 2 of 2, NegativeRouteAdvice 3 of 3; 23 s
  in all with the build. Twelve broken copies of the resolver, each caught by exactly one check: mark not sent
  (mark_selects_the_route), owner not sent (owner_selects_the_route), interface not sent
  (interface_selects_the_route), loose source route ignored (loose_source_route_goes_to_its_first_hop), route
  MTU ignored (route_metrics_give_path_mtu_and_hop_limit), locked MTU not capped
  (locked_mtu_without_value_uses_the_device_mtu), broadcast permission ignored (broadcast_needs_permission),
  flow label not sent (flow_label_selects_the_route), confirmation skipped (confirmed_neighbour_is_confirmed),
  IPv6 source check skipped (required_source_check_refuses_a_foreign_address), default TTL fixed at 64
  (ipv4_route_gives_link_source_and_sizes), type 4 segment ignored (routing_header_goes_to_its_first_hop).
- cargo tree (normal, build and dev edges) of all 15 interface crates and the three conformance crates against
  the workspace's 17 implementation crates: none named (control: the resolver's own tree names itself and the
  shared crate). The 46 reviewed files (every interface.rs and the files beside it) have no change against HEAD
  and in the working tree.
- The generic suites: `setting_twice_equals_setting_once((implementor, handle, setting))`, `advice_is_taken`
  and `advice_is_taken_again((implementor, handle))`, each with its `tests!`; the UDP, TCP, ICMP and raw suites
  make the tuples. Their self-tests catch a setting taken only once and advice taken only once.
- Literal check: no finding in this track's files (18 findings elsewhere, all other tracks' work in progress).

## Next: I2 for the async ruling
- The stopped agent's uncommitted I2 work (counters for the received and ICMP error fields Linux does not give,
  an ICMP check that a reply matches the wire, a raw router alert check that needs the hop-by-hop header, and
  more raw checks: 840 lines of diff) is saved as `i2-stopped-work.patch` in scratch and the files were put
  back to HEAD; `git apply` takes it on HEAD 072dfbea. It is untested: the ICMP and raw crates no longer build,
  because their interfaces gained `wait_for_events`.
- To do: `wait_for_events` on both (mio or polling), the basic calls on socket2, the folders renamed to `host`
  (`linux-ping` and `linux-host`), then the suites on the ARM machine. Another track is adding
  `readiness.rs` to the shared crate (uncommitted at 20:40): read it before writing a private one.

## I2: done (2026-11-21, before the async ruling)
- `crates/icmp-sockets/implementation/linux-ping` (package `wcontainers-icmp-sockets-linux-ping`, type
  `LinuxPingSockets`, socket `LinuxPingSocket`): Linux ping sockets. Needs the caller's group inside
  `net.ipv4.ping_group_range` (the suite's namespace sets it to every group; the crate's own test checks that an
  empty range gives `EACCES`). Public beyond the traits: `new()`, `counters()` and the refusal reasons.
- `crates/raw-sockets/implementation/linux-host` (package `wcontainers-raw-sockets-linux-host`, type
  `LinuxHostRawSockets`, socket `LinuxHostRawSocket`): `SOCK_RAW` sockets (root or `CAP_NET_RAW`), every
  method, both packet kinds, every setting. Public beyond the traits: `new()`, `counters()`, the reasons.
- Shared crate `crates/internet-protocol/implementation/linux-socket-options`, added by this track:
  `datagram::exchange` (`send_message`, `ErrorEntry`, `ErrorQueue` with `message_too_long`, `error_entry` that
  turns an error queue entry into `Received`, `unsent_bytes` and `dropped_packets` from `SO_MEMINFO`, and the
  reasons `ERROR_OF_NO_REPORT`, `TRANSMIT_TIME_REJECTION_WITHOUT_KEY`), and
  `icmp_error::write_icmp_error_quoting_header` (an ICMP error whose quoted packet Linux gave from its IP header
  on, as it does to a raw socket that writes its headers). `set_packet_setting` now gives `IPV6_UNICAST_IF` in
  network byte order, and the route resolver calls it instead of its own copy.
- Suites: `crates/icmp-sockets/conformance` (31 checks plus Configurable's suite on 4 settings; with the crate's own 2 tests, 37) and
  `crates/raw-sockets/conformance` (24 checks plus Configurable's suite on 4 settings, 28). Each check runs as root in
  a new network namespace entered by the `tests!` macro. The route resolver suite now exports its namespace tools
  (`IP`, `BATCH`, `IPV6`, `PACKETS`, `number_after`, `packets_sent`, `seconds_since_confirmed`), the ICMP suite
  its packet tools (`Capture`, a packet socket that sees what leaves a link; `seen`; the network and addresses),
  and the raw suite uses both: one definition of each fact, as the literal check asks.
- The I1 findings of the last independent check are fixed: the resolver suite checks that a lookup sends
  nothing (da's transmit counter, multicast off on da), strict source routes (refused or link-scope only) and a
  mobility header selector; the plan's two wording points are corrected; the shared unicast interface helper is
  fixed and used. Three broken copies, each caught: probe flag dropped (confirmed_neighbour_is_confirmed),
  strict as loose (strict_source_route_takes_only_a_first_hop_on_the_link), IPv6 unicast interface in host order
  (confirmed_neighbour_is_confirmed). Resolver suite: 28 checks.

## How to run
- Scripts in pyro-parked/wcontainers-scratch/host-internet-sockets, passed to arm-run from the orchestrator's
  cloud folder: `./arm-run <checkout> --name host-internet-sockets -- "$(cat <scratch>/rounds.sh)"` (raw, ICMP and
  resolver suites three times), `independent-check-i1.sh` (the resolver and generic suites from a clean build,
  three rounds, twelve broken copies, cargo tree of every interface crate), `independent-check-i2.sh` (clean
  build with clippy, the nine packages' tests, thirteen broken copies, cargo tree). Test tools put on the path there: iproute2 7.x and nftables from nixpkgs.
- On the Mac: `CARGO_TARGET_DIR=target/host-internet-sockets cargo clippy --target aarch64-unknown-linux-gnu
  --no-deps --all-targets -p <package> -- -D warnings` checks and lints the Linux code without the ARM machine
  (the gnu target's standard library was added with rustup; musl does not build the shared crate).
- Literal check: `cargo run -q -p wcontainers-literals -- check`; no finding in this track's files.
- Committing Cargo.lock in the shared checkout: `git commit -- Cargo.lock` takes the whole working file with
  other tracks' entries. Build the wanted file (HEAD's plus this track's hunks, `lock-hunks.py` in scratch), then
  `git update-index --cacheinfo 100644,$(git hash-object -w <file>),Cargo.lock` and commit the index. Other
  tracks stage renames in the shared index, so commit from a private one: `GIT_INDEX_FILE=<scratch>/idx git
  read-tree HEAD`, add this track's paths and the lock there, then `git commit` with the same variable.

## Results (ARM64 machine, Linux 7.0.0-1018-aws, 96 cores)
- 20:01 UTC, load 8.4 to 17 (final code): three rounds, each ICMP 37 of 37 (0.61 s), raw 28 of 28 (0.62 s),
  resolver 28 of 28 (0.02 s), at 12 test threads.
- 19:55 UTC, load 5.7 to 9.7: three rounds, each ICMP 37 of 37 (0.61 s), raw 27 of 27 (0.62 s; 28 of 28 with the close check added after), resolver 28 of
  28 (0.02 s), at 12 test threads. Generic suites 2 + 3 self-tests pass.
- Clippy with warnings denied clean for all nine packages (Mac, gnu target, and ARM machine).

## Which check fails when a method breaks
- IcmpSockets::open: every check (owner: owner_selects_the_route). bind_identifier:
  identifiers_are_shared_only_when_both_ask, the_request_carries_the_bound_identifier, echo replies (identifier).
  send_to: every check that sends; each header field and send field has its own (below). receive:
  echo_reply_arrives_whole_over_ipv4/ipv6, a_short_buffer_keeps_the_whole_length,
  receive_with_nothing_queued_would_block, an_icmp_error_arrives_whole_over_ipv4/ipv6,
  transmit_reports_carry_the_key_of_their_send, a_small_receive_buffer_counts_its_drops. count_unsent_bytes:
  pacing_holds_messages_in_the_send_buffer. close: close_frees_the_identifier. configure: each setting's check
  and Configurable's suite.
- RawSockets::open: every check, open_takes_each_version_and_protocol. send_to: payload checks (IPv4, IPv6,
  IPPROTO_RAW IPv6), header_included_packets_go_out_as_written, header_included_packets_take_their_route,
  withdrawn_reassembly_lets_a_fragment_leave, payload_routes_take_their_selectors, path_mtu_modes_and_too_long_
  payloads, send_fields_are_taken. receive: ipv4_packets_arrive_whole, ipv6_packets_arrive_with_their_headers,
  a_short_buffer_keeps_the_whole_length, receive_with_nothing_queued_would_block, icmp_errors_arrive_whole,
  the router alert checks. count_unsent_bytes: ip_settings_are_taken. close:
  close_gives_up_the_sockets_holdings (the multicast routing role is free again). configure:
  a_checksum_offset_is_written, the two router alert checks, multicast_routing_takes_the_namespace_role,
  ip_settings_are_taken, multicast_settings_are_taken, Configurable's suite.
- Broken copies tried: see the independent check below.

## Honoured, and how each is checked
- Header fields, by what leaves a dummy link (packet socket) or what a rule does: IPv4 TTL, DSCP, ECN, options,
  source, waived source check (sends from an absent address), interface (chooses between two routes), link
  scope, broadcast permission, mark and owner (rules that prohibit), ICMP type and code (a rule on them), path
  MTU modes (don't-fragment bit per mode), too long (`MessageTooLong` with the link MTU). IPv6 hop limit, DSCP,
  ECN, flow label, source preferences, local fragmentation and fragment MTU (`MessageTooLong` with either), and
  for raw sockets source, hop-by-hop, routing destination and destination options and a segment routing header.
- Send fields: neighbour confirmation (the gateway's entry confirmed 0 s ago), priority (the third band of a prio
  queue), transmit time (fq holds the packet 300 ms), transmit reports scheduled and software with key, with and
  without the packet.
- Received fields: interface, specific destination (raw IPv4: the address; never another), software timestamps (none unless set; no hardware one on loopback), drops since
  previous (received plus dropped equals sent), whole IPv4 and IPv6 headers with extension headers; ICMP errors
  whole (outer header, ICMP header and checksum, quoted header and identifier or payload).
- Raw only: a packet with its own header leaves byte for byte; its route takes the flow's protocol
  (`IP_PROTOCOL`; IPv6 through the address's port) and mark; withdrawn reassembly lets a fragment leave where
  nftables tracks connections, which otherwise holds it; checksum offset (written, and left alone with none;
  refused on IPv4); IPv4 router alert (a forwarded packet over a veth pair goes to the socket, not after Off);
  IPv6 router alert by value and isolation (a socket in another namespace takes it only when not isolated);
  multicast routing (one socket per namespace, `mc_forwarding` on while held, both versions).
- Settings: multicast membership (in `ip maddr` while joined; for ICMP also a looped multicast echo is answered
  only with multicast loopback on, for raw the socket gets its own multicast send only then), anycast (in
  `/proc/thread-self/net/anycast6`), pacing rate, send buffer (a paced socket fills it), receive buffer (drops),
  timestamping (a bound clock loopback lacks is refused). Every setting kind also passes Configurable's suite.

## Not honoured, with the Linux reason
- ICMP over IPv6 (net/ipv6/ping.c), refused with NotSupported, counted and warned: a source address (Linux sends
  from the bound one), extension headers and routing header (Linux drops them), neighbour confirmation (Linux
  does not pass the flags on). The suite accepts the refusal or the right effect.
- Both kinds: an upper-layer selector other than what Linux routes the send by (the message's ICMP type and code,
  the mobility header type for IPv6 raw, none for other protocols or a packet with its own header), and a raw
  payload under another protocol than the socket's (IPv4; IPv6 unless the socket's is `IPPROTO_RAW`).
- Ping sockets give no `SO_RXQ_OVFL`, `SO_RCVMARK` or `SO_RCVPRIORITY` (only timestamps): received mark and
  priority are written as 0, the specific destination is none (Linux gives 0.0.0.0, not preparing it), and drops are read from `SO_MEMINFO` at each receive, so drops after a packet was
  queued are told with it rather than with the next packet. The totals are exact (checked).
- Received largest fragment and link-layer length: filled by the shared code from Linux's messages as for UDP,
  whose suite checks them; no check of this track makes a fragmented arrival or a hardware report.
- Multicast all-groups on a ping socket is taken but has no effect to see: a ping socket takes only echo replies.
- Wireless status reports: refused as in the UDP track (no key). Acknowledgement reports are TCP's only;
  hardware and completion reports need a device with a hardware clock, which the namespace has not.
- Linux behaviour the suites follow, measured: a raw send into a full send buffer answers `ENOBUFS`, not
  would-block; `IPV6_DONTFRAG` holds for a raw IPv6 payload of another protocol than UDP or ICMPv6 only in mode
  `Do`; `IPV6_MTU` (fragment MTU) fragments nothing smaller than the path MTU (a 1448-byte packet left whole with
  it at 1300); IPv6 router alerts reach only `IPPROTO_RAW` sockets (`ENOPROTOOPT` otherwise) and come from the
  packet's IPv6 header on, which the implementation recognises by the header's source and length and does not
  write again.
- Error queue entries of a raw socket that switched between payloads and packets with their own headers while
  errors were in flight are read by the socket's current mode (Linux decides it when the error arrives).

## Independent check of I2 (run here, ARM64 machine, 19:57 to 19:59 UTC, load 4.5 to 7.6)
- Clean build in a fresh target folder (`independent-check-i2.sh`): clippy with warnings denied clean on the nine
  packages; ICMP 37 of 37, raw 27 of 27, resolver 28 of 28, generic self-tests 2 + 3; 24 s in all. The raw
  close check was added after it and ran three times (28 of 28 each).
- Thirteen broken copies, each caught: ICMP identifier never shared (identifiers_are_shared_only_when_both_ask),
  unsent count always zero (pacing_holds_messages_in_the_send_buffer), reply without its header (both echo
  reply checks and a_short_buffer_keeps_the_whole_length), close leaking the descriptor
  (close_frees_the_identifier), drops not counted (a_small_receive_buffer_counts_its_drops); raw header never
  included (five checks), isolation ignored (an_ipv6_router_alert_takes_forwarded_packets), reassembly ignored
  (withdrawn_reassembly_lets_a_fragment_leave), no checksum left on (a_checksum_offset_is_written), multicast
  routing never given up (multicast_routing_takes_the_namespace_role), IPv6 header not written
  (ipv6_packets_arrive_with_their_headers), route protocol not sent (header_included_packets_take_their_route),
  close leaking the descriptor (close_gives_up_the_sockets_holdings).
- cargo tree (normal, build and dev edges) of the six interface crates and five conformance crates: no
  implementation crate (control: the raw implementation's tree does name the shared crate).
- Reviewed files: this run's commits touch no interface.rs and no data file beside one; the working tree has no
  change to them.

## For the other socket tracks (shared crate)
- New: `datagram::exchange` (above). The UDP crate now re-exports the two reasons from it (02e1d659, two lines in
  its implementation.rs, done here because the literal check counted the copy). UDP and TCP could also use
  `send_message`, `ErrorQueue` and `error_entry` in place of their own.
- `set_packet_setting` gives `IPV6_UNICAST_IF` in network byte order (1991e134).

## Departures from the plan
- The plan named I2 ICMP and I3 raw; the task made both one milestone, done together.
- The suites share test tools across suite crates (the raw suite depends on the ICMP and route resolver suites,
  the ICMP suite on the resolver suite) so that each fact has one definition.
- The generic suites still test themselves with private implementors under `#[cfg(test)]`: a named test double
  would go under crates/configurable/implementation/ and crates/negative-route-advice/implementation/, which
  this track does not own, and no other suite needs one.

## For the owner
- No interface can build its implementation. Proposed, as `StandaloneRuntime` does:
  `pub trait StandaloneIcmpSockets: IcmpSockets + Sized { fn open_host() -> Result<Self, HostError>; }`, the same
  for `RawSockets` and `RouteResolver` (`fn open() -> Result<Self, HostError>`). Meanwhile `new()`/`open()`.
- `ReceivedPacketFields.mark` and `.priority` are plain values, but Linux gives a ping socket neither: an
  `Option` would say so; this implementation writes 0. Example: a reply that arrived with mark 7 reads mark 0.
- `MRT_INIT`/`MRT_DONE` (200, 201, linux/mroute.h and mroute6.h) are in no established crate (libc and
  linux-raw-sys lack them); the raw implementation names them once.
- RouteKind has no anycast (I1): fd01:: is `anycast` on Linux and can only be refused.
- The contract does not say which error a failed source check gives (I1): Linux's IPv4 lookup gives
  `ENETUNREACH`, bind `EADDRNOTAVAIL`, a send with `IP_PKTINFO` `EINVAL`.
- No way through the interfaces to read the refusal counters; `counters()` is inherent.

## Dependencies (licences checked)
- socket2 0.6.5 (MIT OR Apache-2.0), added to the resolver in I1's run after the reorganization.
- netlink-packet-core 0.9.0, netlink-packet-route 0.34.0, netlink-sys 0.9.0 (MIT); rustix 1.1.6 (Apache-2.0 WITH
  LLVM-exception OR Apache-2.0 OR MIT), linux-raw-sys 0.12.1 (same), libc 0.2.191 (MIT OR Apache-2.0), etherparse
  0.21.0 (MIT OR Apache-2.0). No new crate in I2. iproute2 and nftables (GPL) are test tools the suites run,
  never dependencies.
