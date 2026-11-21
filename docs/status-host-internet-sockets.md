# Status: host internet sockets

Plan: docs/plan-host-internet-sockets.md. Milestones: I1 route resolver and the two generic suites (done),
I2 ICMP sockets, I3 raw sockets.

## Done (I1, 2026-11-21)
- `crates/route-resolver/implementation/linux-netlink` (package `wcontainers-route-resolver-linux-netlink`,
  type `LinuxNetlinkRouteResolver`): `RTM_GETROUTE` and `RTM_GETLINK` on a netlink socket of the namespace
  the value is opened in. Linux only (`#![cfg(target_os = "linux")]`, so it builds empty on the Mac).
  Public beyond the trait: `open()` and `counters()` (one `CounterReading` per refusal reason); each
  refusal also logs a `log::warn!`.
- Suite `crates/route-resolver/conformance`: 23 checks, each in a new network namespace made on the test's
  thread before the value is made (root needed), with links, routes and rules built by `ip -batch`.
  Checks that a host may be unable to ask for (link scope, waived source, source preferences, routing
  header, confirmed neighbour, a protocol the routing socket refuses) accept `NotSupported` or the right
  answer, never a wrong one.
- Suites `crates/configurable/conformance` (setting twice answers as once) and
  `crates/negative-route-advice/conformance` (advice taken, and taken again), taking `(implementor, handle,
  setting)` and `(implementor, handle)`. Each has a `tests!` macro and private self-tests in `#[cfg(test)]`
  with a correct and a broken implementor (the broken one must make the check panic).

## How to run
- On the ARM machine (needs root and iproute2 7.x for the dscp and flowlabel rules; Ubuntu's 6.1 lacks
  them), from the orchestrator's cloud folder:
  `./arm-run <checkout> --name host-internet-sockets -- 'IP=$(nix build --no-link --print-out-paths "nixpkgs#iproute2^out") || exit 1; export PATH=$IP/bin:$PATH; export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER="sudo env PATH=$PATH"; cargo test -j $ARM_JOBS -p wcontainers-route-resolver-linux-netlink -p wcontainers-route-resolver-conformance -p wcontainers-configurable-conformance -p wcontainers-negative-route-advice-conformance -- --test-threads 12'`
- The independent check (clean build, then five broken copies of the implementation built under
  target/mutant) is pyro-parked/wcontainers-scratch/host-internet-sockets/independent-check.sh, passed as
  the arm-run command.
- On the Mac the two generic suites run as they are; the route resolver crates build empty.

## Results (ARM64 machine, Linux 7.0.0-1018-aws, load 2 to 15 of 96 cores)
- 23 of 23 route resolver checks pass, 2 + 3 generic self-tests pass; clippy with warnings denied is clean
  on Linux and the Mac; the literal check has no finding in my files.
- Clean build (fresh target folder): all pass. Broken copies, each failing exactly one check: mark not sent
  (mark_selects_the_route), ICMP type and code bytes swapped (icmp_type_and_code_select_the_route), route MTU
  ignored (route_metrics_give_path_mtu_and_hop_limit), owner not sent (owner_selects_the_route), broadcast
  permission unchecked (broadcast_needs_permission).
- Which check fails if a part breaks: destination and source address (every check; source:
  source_selects_the_route), interface (interface_selects_the_route), DSCP (dscp_selects_the_route), flow
  label (flow_label_selects_the_route), ports and protocol (protocol_and_ports_select_the_route), kind
  (loopback, broadcast, multicast checks), device MTU and hop limit (the two route_gives checks), kernel
  errors (failing_routes_give_their_errors), source checks (required_/waived_source_check), IPv4 options
  (loose_source_route_goes_to_its_first_hop).
- cargo tree: no interface or conformance crate depends on an implementation. No reviewed file changed.

## Refused by the implementation (NotSupported, counter, warning)
IPv4 link scope and strict source routes; IPv4 waived source check with an address the host lacks; IPv6
source preferences other than the default with no source; IPv6 routing headers; a confirmed neighbour;
protocols the routing socket refuses (it takes TCP, UDP, ICMP for IPv4, ICMPv6 for IPv6); route kinds other
than unicast, local, broadcast and multicast. Not modelled: the kernel caps a locked IPv4 MTU metric at 576
through a gateway.

## Next
- I2: ICMP sockets. Use the shared `crates/internet-protocol/implementation/linux-socket-options` crate
  (created by the TCP track; read docs/status-host-tcp-sockets.md and docs/status-host-udp-sockets.md).
- Possible later: the refusals above could be honoured by connect-and-read (a UDP socket with
  `SO_DONTROUTE`, `IP_TRANSPARENT` or `IPV6_ADDR_PREFERENCES`, then getsockname and `IP_MTU`) for source and
  MTU, but it cannot give the interface or kind.

## Departures from the plan
- The generic suites test themselves with private implementors under `#[cfg(test)]` in the conformance
  crate: no implementation of Configurable or NegativeRouteAdvice exists yet, and their implementation
  folders are not this track's.

## For the owner
- RouteResolver has no way to build one. Proposed: `pub trait StandaloneRouteResolver: RouteResolver + Sized
  { fn open() -> Result<Self, HostError>; }` (a resolver answers for the network it is opened in). Meanwhile
  `LinuxNetlinkRouteResolver::open()`.
- The contract does not say which error a failed source check gives: Linux's IPv4 lookup gives `ENETUNREACH`
  (for example source 10.1.0.99 that no link has), bind gives `EADDRNOTAVAIL`, and send with `IP_PKTINFO`
  gives `EINVAL`. The implementation passes on the host's answer (IPv4 `ENETUNREACH`, IPv6 `EADDRNOTAVAIL`).
- `NeighbourConfirmation::Confirmed` in a lookup that sends nothing has no host counterpart (Linux confirms
  a neighbour only with a packet, `MSG_CONFIRM`); it is refused.
- There is no way through the interfaces to read the counters of refusals; `counters()` is inherent.

## Dependencies added (licences checked)
netlink-packet-core 0.9.0, netlink-packet-route 0.34.0, netlink-sys 0.9.0 (MIT; their bytes, bitflags,
zerocopy, log, libc are MIT or Apache-2.0 or BSD-2); socket2 0.6.5, log 0.4.35, libc 0.2.191 (MIT OR
Apache-2.0); rustix 1.1.6 in the suite (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT). iproute2 (GPL)
is a test tool run by the suite, never a dependency.
