# Plan: host internet sockets

Host implementations, on Linux, of RouteResolver, IcmpSockets and RawSockets, and the conformance suites of
those three and of the generic Configurable and NegativeRouteAdvice. Milestones: I1 the route resolver and the
two generic suites; I2 ICMP and raw sockets.

## I1: the route resolver

`crates/route-resolver/implementation/host`, package `wcontainers-route-resolver-host`, type
`HostRouteResolver`. It asks the kernel's routing socket (`RTM_GETROUTE`, the request behind `ip route get`)
through netlink-packet-route, netlink-packet-core and netlink-sys (all MIT), with socket2 (MIT OR Apache-2.0)
for the datagram sockets of the IPv6 source check, the neighbour confirmation and the default TTL, and the
shared crate `crates/internet-protocol/implementation/linux-socket-options` for
`Shortfalls` (a counter per refusal reason and a warning on stderr at its first count), `host_error` and the
option calls. Each lookup is one route request and one
link request on a netlink socket the value holds, so a value answers for the network namespace it was made in;
the request and reply buffers are kept in the value, so a lookup allocates no buffer once they have grown.

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
  kernel's flow keeps them in (the flow's ICMP fields share the destination port's place). A link-local
  destination with no interface is looked up as any other, as Linux's UDP and raw sends take it (only a
  connect, and a ping socket's send, need the interface: measured, EINVAL).
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
  `ip_default_ttl` (read as an unset socket's `IP_TTL`, socket2's `ttl_v4`) or the device's IPv6 `hop_limit`.
- socket2 covers the datagram calls (socket, bind, `sendto` with flags, `IP_TTL`); the unicast interface
  (`IP_UNICAST_IF`/`IPV6_UNICAST_IF`, which socket2 lacks) and `IPV6_RTHDR` are the shared crate's
  `set_packet_setting` on the same descriptor. The routing socket is netlink-sys's, the netlink crates' own.

Refused with `HostError::NotSupported`, a counter and a warning, each with a named reason:
- IPv4 link scope (`SO_DONTROUTE`, and so a strict source route): the request's `rtm_scope` is ignored by the
  kernel on `RTM_GETROUTE` (Universe, Link and Host give the same route through a gateway).
- IPv4 waived source check with an address the host lacks: the routing socket always checks.
- IPv6 source preferences other than the system's default, with no source given.
- An upper-layer protocol the routing socket refuses (it takes TCP, UDP, and ICMP or ICMPv6), and so a
  mobility header selector, refused before asking.
- A route kind the contract has no name for (for example the subnet-router anycast address of a link that
  forwards).
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

## I2: ICMP and raw sockets

Both are built on the shared crate's datagram half (`SendState` for header and send fields, `Ancillary` and
`Headers` for what arrives, memberships), plus `datagram::exchange` (one `sendmsg`, the error queue read as
packets and reports), which this milestone adds there for all three datagram tracks.

Both wait on the shared crate's `readiness` (polling 3.11.0 over epoll, which the UDP track added: one epoll
set and one thread per value, a socket registered edge-triggered as long as it lives and deleted before its
descriptor closes, a waiting call that allocates nothing once its socket's waiter list has grown and withdraws
itself when dropped). Error queue entries a send reads off the host while it looks for its own are held as an
error condition in that readiness until a receive takes them. The basic calls and the options socket2 covers
(socket, bind, `SO_REUSEADDR`, `IP_RECVTOS`, `IPV6_RECVTCLASS`, `IPV6_RECVHOPLIMIT`, `IP_HDRINCL`,
`IPV6_HDRINCL`) go through socket2 on the socket's descriptor; the rest through the shared crate's option calls.
A send that asks for transmit time errors is noted in the shared `ErrorQueue`, so that a rejection comes with its
send's key.

`crates/icmp-sockets/implementation/host`, package `wcontainers-icmp-sockets-host`: Linux ping
sockets (`SOCK_DGRAM` with `IPPROTO_ICMP` or `IPPROTO_ICMPV6`), which need the caller's group inside
`net.ipv4.ping_group_range`. open is the socket, owned by the given user (filesystem user while it is made);
bind_identifier is `SO_REUSEADDR` as asked and a bind to the identifier as port; send_to writes the header and
send fields as the UDP track does, then one `sendmsg` of the message (Linux puts the identifier in it and
computes the checksum); receive gives an error queue entry first (an ICMP error whole from its IP header, or a
transmit report), else an echo reply with its IP header written in front (from the sender, `IP_PKTINFO`, the
TTL, TOS and options messages; IPv6 with its extension headers); count_unsent_bytes is `SO_MEMINFO`'s
`SK_MEMINFO_WMEM_ALLOC`, which is what `SIOCOUTQ` gives a UDP socket (ping sockets take no ioctl); close drops
the descriptor.

Refused with a counter and a warning, by what Linux's ping sockets do (net/ipv6/ping.c): for IPv6 a source
address (the socket's bound one is used), extension headers and a routing header (dropped), and neighbour
confirmation (the flags are not passed on). For both, an upper-layer selector other than the type and code
of the message, which Linux routes by.

`crates/raw-sockets/implementation/host`, package `wcontainers-raw-sockets-host`: `SOCK_RAW`
sockets of the given protocol (root or `CAP_NET_RAW`). A payload is sent with `IP_HDRINCL`/`IPV6_HDRINCL` off
and the header fields as for ICMP; a packet with its own header with it on, its route selectors as options
and `IP_PKTINFO`, the flow's protocol as `IP_PROTOCOL` (IPv4) or the address's port (IPv6), and reassembly as
`IP_NODEFRAG`. Both are kept per socket and written only when a send wants the other value. receive gives the
error queue first (ICMP errors whole; a socket that wrote its headers gets the quoted packet from its IP header
on), else the packet: IPv4 as Linux gives it, IPv6 with its header and extension headers written in front.
Settings: the IP ones as for ICMP; checksum `IPV6_CHECKSUM` (offset, or -1 for none); router alert
`IP_ROUTER_ALERT`, or `IPV6_ROUTER_ALERT_ISOLATE` and `IPV6_ROUTER_ALERT`; multicast routing `MRT_INIT` and
`MRT_DONE` (`MRT6_*` for IPv6). Linux takes the last two only once per socket, so the socket keeps what it
has and changes it only to another value. count_unsent_bytes as for ICMP.

Refused: an upper-layer selector other than what Linux routes the send by (the message's ICMP type and code,
the mobility header type for IPv6, none for other protocols or a packet with its own header), and a payload
under another protocol than the socket's (IPv4, or IPv6 unless the socket's is `IPPROTO_RAW`).

Each suite runs every check as root in a network namespace of its own, which the `tests!` macro enters on the
test's thread before the value is made, with loopback up and (ICMP) every group in the ping range. Checks see
what leaves a dummy link on a packet socket of the namespace, and its statistics. Header fields (TTL or hop limit, DSCP and ECN, flow label, options, extension
headers, path MTU modes, local fragmentation and fragment MTU, source, interface, mark, scope, broadcast) are
checked by what arrives; send fields by what arrives (priority), the neighbour entry (confirmation), a qdisc
that holds the packet (transmit time) and the error queue (transmit reports with their key, with and without
the packet), and an etf schedule that misses every time (rejected transmit times with their keys). ICMP errors
come from a raw ICMP socket of the namespace, quoting what left the dummy link. Settings by their effect;
Configurable's own suite runs on a setting of each socket kind. Waits are polled by hand with a waker that
counts its wakes: a wait for data is pending until the reply or packet arrives and is then woken once; a wait for
writing finishes at its first poll; a dropped wait gives up its waker and is not woken; 35 waits on 32 sockets
each wake once; an ICMP error ends a wait for data with the error condition, also when a too long send took it
off the host first.
