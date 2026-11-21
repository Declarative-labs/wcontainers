# Status: host internet sockets

Plan: docs/plan-host-internet-sockets.md. Milestones: I1 route resolver and the two generic suites (done,
reworked in run 2 after the independent check), I2 ICMP sockets, I3 raw sockets.

## Done (I1, 2026-11-21)
- `crates/route-resolver/implementation/linux-netlink` (package `wcontainers-route-resolver-linux-netlink`,
  type `LinuxNetlinkRouteResolver`): `RTM_GETROUTE` and `RTM_GETLINK` on a netlink socket of the namespace
  the value is opened in. Linux only (`#![cfg(target_os = "linux")]`, so it builds empty on the Mac).
  Public beyond the trait: `open()` and `counters()` (one `CounterReading` per reason that refused).
  Refusals go through the shared crate's `Shortfalls` (counter per reason, warning on stderr at the first).
- Run 2 added: IPv6 routing headers (type 2 to its address, type 4 to its segment at segments left, the
  header first checked by the host's own `IPV6_RTHDR`, so type 0 is `EINVAL`); neighbour confirmation (a
  `MSG_CONFIRM | MSG_PROBE` probe of no bytes to the next hop on the route's interface); the locked IPv4
  MTU with no value through a gateway (576); refusal of encapsulating routes; a waived IPv4 source check
  decided by the host's lookup of the source (local or not), so `ip_nonlocal_bind` no longer changes it.
- Suite `crates/route-resolver/conformance`: 26 checks, each in a new network namespace made on the test's
  thread before the value is made (root needed), with links, routes, rules and neighbours built by `ip`.
  Run 2 added source_port_selects_the_route, locked_mtu_without_value_uses_the_device_mtu,
  encapsulating_route_leaves_room_for_its_headers (path MTU must equal a connected socket's `IPV6_MTU`
  unless refused); icmp_type_and_code now matches the code (rule dport 2051 = type 8 code 3, with code 0
  and swapped bytes going elsewhere); routing_header_goes_to_its_first_hop (types 2 and 4, type 0 `EINVAL`,
  no refusal accepted); confirmed_neighbour_is_confirmed (the gateway's entry `confirmed` 0 seconds ago
  after a confirmed lookup, not after an unconfirmed one, IPv4 and IPv6); the waived check runs with
  `ip_nonlocal_bind` 0 and 1.
- Suites `crates/configurable/conformance` (setting twice answers as once) and
  `crates/negative-route-advice/conformance` (advice taken, and taken again), taking `(implementor, handle,
  setting)` and `(implementor, handle)`. Each has a `tests!` macro and private self-tests in `#[cfg(test)]`
  with a correct and a broken implementor (the broken one must make the check panic). Unchanged in run 2.

## How to run
- On the ARM machine (needs root and iproute2 7.x for the dscp and flowlabel rules; Ubuntu's 6.1 lacks
  them), from the orchestrator's cloud folder, with the script in this track's scratch folder:
  `./arm-run <checkout> --name host-internet-sockets -- "$(cat <scratch>/suite.sh)"` (clippy with warnings
  denied, then the four packages' tests at 12 threads).
- The independent check (clean build in a fresh target folder, then twelve broken copies of the
  implementation built under target/mutant): `<scratch>/independent-check.sh`, passed the same way.
  `<scratch>` is pyro-parked/wcontainers-scratch/host-internet-sockets.
- On the Mac the two generic suites run as they are; the route resolver crates build empty. The shared
  socket options crate does not build for aarch64-unknown-linux-musl (cmsg types), so Linux lint and
  tests run on the ARM machine.

## Results (ARM64 machine, Linux 7.0.0, load 1.2 to 2.6 of 96 cores, 2026-11-21 18:47 to 18:50 UTC)
- 26 of 26 route resolver checks pass, 2 + 3 generic self-tests pass; clippy with warnings denied clean
  on Linux (ARM machine) and on the Mac; the literal check has no finding in my files.
- Clean build (fresh target folder): all pass. Twelve broken copies, each failing exactly one check:
  mark not sent (mark_selects_the_route); ICMP bytes swapped and ICMP code dropped
  (icmp_type_and_code_select_the_route); route MTU ignored (route_metrics_give_path_mtu_and_hop_limit);
  owner not sent (owner_selects_the_route); broadcast unchecked (broadcast_needs_permission); source port
  dropped (source_port_selects_the_route); segments left ignored and routing header ignored
  (routing_header_goes_to_its_first_hop); confirmation skipped (confirmed_neighbour_is_confirmed); locked
  cap ignored (locked_mtu_without_value_uses_the_device_mtu); waived check skipped
  (waived_source_check_takes_a_foreign_address).
- Which check fails if another part breaks: destination and source address (every check; source:
  source_selects_the_route), interface (interface_selects_the_route), DSCP (dscp_selects_the_route), flow
  label (flow_label_selects_the_route), protocol and destination port (protocol_and_ports_select_the_route),
  kind (loopback, broadcast, multicast checks), device MTU and hop limit (the two route_gives checks),
  kernel errors (failing_routes_give_their_errors), required source check
  (required_source_check_refuses_a_foreign_address), IPv4 options (loose_source_route_goes_to_its_first_hop).
- cargo tree: no interface or conformance crate depends on an implementation. No reviewed file changed.

## Refused by the implementation (NotSupported, counter, warning)
IPv4 link scope and strict source routes; IPv4 waived source check with an address the host lacks; IPv6
source preferences other than the default with no source; protocols the routing socket refuses (it takes
TCP, UDP, ICMP for IPv4, ICMPv6 for IPv6; so GRE, the mobility header and the rest); route kinds other than
unicast, local, broadcast and multicast; routes with a lightweight-tunnel encapsulation (path MTU less a
headroom the routing socket does not give: measured, a seg6 encap route on a 1400 link gives `IPV6_MTU`
1336). Why they stay: the routing socket has no field for them (it carries `rtm_scope` but ignores it on a lookup), and connect-and-read, which could take
link scope, `IP_TRANSPARENT`, `IPV6_ADDR_PREFERENCES` or any protocol (raw socket), cannot tell the
interface a route must give. Their checks accept NotSupported or the right answer, never a wrong one.

## Findings of the independent check, and what was done
- Routing headers refused; the check used type 0: fixed (above).
- No source port check; the ICMP check did not test the code: fixed (above).
- Waived IPv4 source with `ip_nonlocal_bind=1` gave `ENETUNREACH`: fixed, refused either way now.
- IPv6 anycast refused: kept (RouteKind has no Anycast); recorded for the owner below.
- Link-local destination without an interface resolves while `connect` gives `EINVAL`: kept. Measured on
  the ARM machine: a UDP `sendto` to fe80::5 with no scope succeeds and `ip -6 route get fe80::5` gives a
  route, so the lookup does have an answer; `EINVAL` is connect's and bind's rule that a link-local peer
  needs a scope, which the socket implementations apply, not the route lookup.
- The locked MTU note: corrected and modelled (measured: `mtu lock 0` via a gateway gives `IP_MTU` 576, on
  the link 1400).
- `log::warn!` with no logger, private copies of `Shortfalls` and `host_error`: now the shared crate's.
- Confirmed neighbour refused but undetectable: now honoured and checked.

## Next
- I2: ICMP sockets. Use the shared `crates/internet-protocol/implementation/linux-socket-options` crate
  (read docs/status-host-tcp-sockets.md and docs/status-host-udp-sockets.md first).

## For the other socket tracks (shared crate)
- `set_packet_setting` with `UnicastInterface` for IPv6 passes the index in host byte order, but the kernel's
  `IPV6_UNICAST_IF` takes it in network byte order as IPv4's does (`ntohl` in do_ipv6_setsockopt): measured,
  index 2 in host order gives `EADDRNOTAVAIL`. The resolver sets the option itself with `to_be()` for both
  until the shared crate is fixed; then it should call the shared helper.

## Departures from the plan
- The generic suites test themselves with private implementors under `#[cfg(test)]` in the conformance
  crate: no implementation of Configurable or NegativeRouteAdvice exists yet, and their implementation
  folders are not this track's.

## For the owner
- RouteResolver has no way to build one. Proposed: `pub trait StandaloneRouteResolver: RouteResolver + Sized
  { fn open() -> Result<Self, HostError>; }` (a resolver answers for the network it is opened in). Meanwhile
  `LinuxNetlinkRouteResolver::open()`.
- RouteKind has no anycast: a lookup to fd01:: (the subnet-router anycast address of fd01::/64, with
  forwarding on) is `anycast fd01:: dev lo table local` on Linux and delivered locally, and the resolver can
  only refuse it.
- The contract does not say which error a failed source check gives: Linux's IPv4 lookup gives `ENETUNREACH`
  (for example source 10.1.0.99 that no link has), bind gives `EADDRNOTAVAIL`, and send with `IP_PKTINFO`
  gives `EINVAL`. The implementation passes on the host's answer (IPv4 `ENETUNREACH`, IPv6 `EADDRNOTAVAIL`).
- There is no way through the interfaces to read the counters of refusals; `counters()` is inherent.

## Dependencies (licences checked)
netlink-packet-core 0.9.0, netlink-packet-route 0.34.0, netlink-sys 0.9.0 (MIT; their bytes, bitflags,
zerocopy, log, libc are MIT or Apache-2.0 or BSD-2); rustix 1.1.6 (Apache-2.0 WITH LLVM-exception OR
Apache-2.0 OR MIT), linux-raw-sys 0.12.1 (same), libc 0.2.191 (MIT OR Apache-2.0). Run 2 dropped socket2
and log from the resolver. iproute2 (GPL) is a test tool run by the suite, never a dependency.
