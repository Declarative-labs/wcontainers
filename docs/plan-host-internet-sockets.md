# Plan: host internet sockets

Host implementations, on Linux, of RouteResolver, IcmpSockets and RawSockets, and the conformance suites of
those three and of the generic Configurable and NegativeRouteAdvice. Milestones: I1 the route resolver and the
two generic suites; I2 ICMP sockets; I3 raw sockets.

## I1: the route resolver

`crates/route-resolver/implementation/linux-netlink`, package `wcontainers-route-resolver-linux-netlink`.
It asks the kernel's routing socket (`RTM_GETROUTE`, the request behind `ip route get`) through
netlink-packet-route, netlink-packet-core and netlink-sys (all MIT), with rustix and linux-raw-sys for the few
socket calls, and the shared crate `crates/internet-protocol/implementation/linux-socket-options` for
`Shortfalls` (a counter per refusal reason and a warning on stderr at its first count), `host_error` and the
option calls. Each lookup is one route request and one
link request on a netlink socket the value holds, so a value answers for the network namespace it was made in.

Chosen over connect-and-read (connect a UDP socket, then getsockname and `IP_MTU`) because connect-and-read
cannot tell the outgoing interface or the route kind, cannot take another user than the caller (`RTA_UID`),
and needs `CAP_NET_RAW` or `CAP_NET_ADMIN` for a mark (`SO_MARK`). The routing socket cannot take a link scope or a waived source check for
IPv4, nor IPv6 source preferences, which connect-and-read can; those few are refused (below).

What each selector maps to:
- destination `RTA_DST` (for IPv4, a loose source route's first hop instead; for IPv6, a routing header's first
  hop, as the kernel's `fl6_update_dst`: a type 2 header's address, a type 4 header's segment at its segments
  left. The header is first set as `IPV6_RTHDR` on a new socket, so the host's own check refuses what it
  refuses, type 0 with `EINVAL`), source `RTA_SRC`, interface
  `RTA_OIF`, mark `RTA_MARK`, owner `RTA_UID`, DSCP and ECN the header's `rtm_tos` (the kernel ignores ECN),
  flow label `RTA_FLOWLABEL`, upper-layer protocol `RTA_IP_PROTO` (always given: the kernel takes UDP
  when it is absent), ports `RTA_SPORT`/`RTA_DPORT`, an ICMP type and code `RTA_DPORT` in the bytes the
  kernel's flow keeps them in (the flow's ICMP fields share the destination port's place).
- source check: IPv4's is the kernel's own (`ENETUNREACH` for an address the host lacks); IPv6's lookup has
  none, so a required check binds a UDP socket to the address first (`EADDRNOTAVAIL`). A waived IPv4 check
  with an address that the host's own lookup of it does not find local is refused (below), whatever
  `ip_nonlocal_bind` says.
- neighbour confirmation: a confirmed unicast lookup then sends a probe of no bytes with
  `MSG_CONFIRM | MSG_PROBE` to the route's next hop (gateway, else destination) on its interface
  (`IP_UNICAST_IF`/`IPV6_UNICAST_IF`), which confirms the neighbour entry as a send with `MSG_CONFIRM` does
  and sends nothing.
- broadcast permission: a broadcast route without it is `EACCES`, as the kernel's connect and send give.
- result: kind from `rtm_type`; source from `RTA_PREFSRC` (IPv6: the given source when there is one, as
  the kernel sends from it); interface `RTA_OIF`; device MTU `IFLA_MTU`; path MTU the route's `RTAX_MTU`,
  else the device MTU (IPv4; capped at 576 when the MTU is locked with no value and the route has a
  gateway, as `ip_dst_mtu_maybe_forward`) or the device's IPv6 MTU (`mtu6`); hop limit `RTAX_HOPLIMIT`, else
  `ip_default_ttl` (read as an unset socket's `IP_TTL`) or the device's IPv6 `hop_limit`.

Refused with `HostError::NotSupported`, a counter and a warning, each with a named reason:
- IPv4 link scope (`SO_DONTROUTE`, and so a strict source route): the request's `rtm_scope` is ignored by the
  kernel on `RTM_GETROUTE` (Universe, Link and Host give the same route through a gateway).
- IPv4 waived source check with an address the host lacks: the routing socket always checks.
- IPv6 source preferences other than the system's default, with no source given.
- An upper-layer protocol the routing socket refuses (it takes TCP, UDP, and ICMP or ICMPv6).
- A route kind the contract has no name for (for example IPv6 anycast).
- A route with a lightweight-tunnel encapsulation: the kernel takes the encapsulation's headroom off the path
  MTU, and the routing socket does not give it.

Connect-and-read could honour link scope, a waived IPv4 source (`IP_TRANSPARENT`) and source preferences for
the source and path MTU, and a raw socket's connect could take any protocol, but none of them tells the
interface, which a route must give; so those stay refused.

## The suites

The route resolver's suite runs each check in a network namespace of its own: the `tests!` macro enters a
new one on the test's thread (root, `unshare(CLONE_NEWNET)`) before the value is made, and the check
builds its links, addresses, routes, rules and neighbours with `ip` (iproute2, a test tool, not a dependency). Each
check knows the answer from what it built: loopback is local; a dummy device's route gives its interface,
source, kind, MTUs and hop limit; a rule per selector (mark, owner, DSCP, source, interface, protocol, destination port, source port,
ICMP type and code, flow label) sends the lookup to a second device only when the selector is honoured; type 2
and type 4 routing headers and a loose source route through the second device's gateway go there; a
confirmed lookup makes the gateway's neighbour entry confirmed 0 seconds ago (`ip -s neigh`); a locked MTU with
no value gives 576 through a gateway; an encapsulating route's path MTU is what a connected socket is told; a broadcast
address without permission is `EACCES`; a source the namespace lacks is refused when the check is required.

Configurable's suite takes `(implementor, handle, setting)` and checks that setting the same value twice
gives the same answer as once. NegativeRouteAdvice's takes `(implementor, handle)` and checks that advice
is taken, and taken again. The socket suites make these tuples and check the effects they can see.
