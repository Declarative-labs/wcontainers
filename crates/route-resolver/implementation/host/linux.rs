use core::cell::RefCell;
use core::mem::offset_of;
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::io;
use std::os::fd::AsFd;

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv4Options, Ipv6RawExtHeaderSlice};
use linux_raw_sys::net::{
    IPV6_SRCRT_TYPE_2, IPV6_SRCRT_TYPE_4, MSG_PROBE, in6_addr, ipv6_rt_hdr, rt0_hdr,
};
use linux_raw_sys::netlink::RTAX_MTU;
use netlink_packet_core::{
    ErrorMessage, NLM_F_DUMP, NLM_F_REQUEST, NLMSG_ALIGNTO, NetlinkHeader, NetlinkMessage,
    NetlinkPayload,
};
use netlink_packet_route::link::{AfSpecInet6, AfSpecUnspec, LinkAttribute, LinkMessage};
use netlink_packet_route::route::{
    RouteAddress, RouteAttribute, RouteFlags, RouteMessage, RouteMetric, RouteType, RouteVia,
};
use netlink_packet_route::rule::{RuleAttribute, RuleMessage};
use netlink_packet_route::{AddressFamily, RouteNetlinkMessage};
use netlink_sys::protocols::NETLINK_ROUTE;
use netlink_sys::{Socket, SocketAddr as NetlinkAddress};
use rustix::net::{SocketType, ipproto};
use socket2::{Domain, SockAddr, Socket as DatagramSocket, Type};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    BroadcastPermission, HopLimit, InterfaceIndex, IpPacketSetting, IpVersion, Ipv4RouteSelectors,
    Ipv6RouteSelectors, MaximumTransmissionUnit, MobileAddressPreference, NeighbourConfirmation,
    PacketMark, Route, RouteKind, RouteScope, RouteSelectors, SourceAddressCheck,
    SourcePreferences, TemporaryAddressPreference, UpperLayer, UpperLayerSelector,
};
use wcontainers_internet_protocol_linux_socket_options::{
    Shortfalls, io_host_error, open_socket, set_packet_setting,
};
use wcontainers_kernel::UserId;

const LINK_SCOPE: &str = "route lookup: a link-scope IPv4 lookup the routing socket cannot ask for";
const WAIVED_SOURCE_CHECK: &str = "route lookup: a waived source check for an IPv4 address the host lacks, which the routing socket always checks";
const UPPER_LAYER_PROTOCOL: &str = "route lookup: an upper-layer protocol the routing socket does not take (it takes TCP, UDP, ICMP and ICMPv6)";
const RULE_ON_PROTOCOL: &str = "route lookup: an upper-layer protocol the routing socket does not take, which a rule of the namespace selects on";
const HASHED_PROTOCOL: &str = "route lookup: an upper-layer protocol the routing socket does not take, on a routing entry whose choice among next hops may hash the protocol";
const MIXED_SOURCE: &str = "route lookup: an IPv4-mapped destination from an IPv6 source that maps no IPv4 address, which a dual-stack socket cannot send from";
const ROUTE_KIND: &str = "route lookup: a route kind the contract has no name for";
const PROBE: &str = "route lookup: a datagram socket connected out of the route's interface took another routing entry or none, so what it is told is not this route's";

const DEFAULT_SOURCE_PREFERENCES: SourcePreferences = SourcePreferences {
    temporary: TemporaryAddressPreference::SystemDefault,
    mobile: MobileAddressPreference::Home,
};

/// The port number that asks `bind` for any port.
const ANY_PORT: u16 = u16::MIN;

/// The destination port of a neighbour confirmation or a probe socket, which send nothing but need a port.
const PROBE_PORT: u16 = u16::MAX;

/// The most an IPv4 path MTU can be, the largest total length its header holds; Linux caps every IPv4 route's MTU at it.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/net/ip.h
const IP_MAX_MTU: u32 = u16::MAX as u32;

/// The send flags that confirm the neighbour and send nothing.
const CONFIRMING_PROBE: libc::c_int = libc::MSG_CONFIRM | MSG_PROBE as libc::c_int;

/// The flags of a plain send or receive: none.
///
/// https://man7.org/linux/man-pages/man2/send.2.html
const NO_FLAGS: libc::c_int = 0;

/// The netlink port number and multicast groups of the kernel.
///
/// https://man7.org/linux/man-pages/man7/netlink.7.html
const KERNEL: (u32, u32) = (0, 0);

/// The size every IPv4 host reassembles, the path MTU of a gateway route whose MTU is locked with no value.
///
/// https://www.rfc-editor.org/rfc/rfc791#section-3.1
const MINIMUM_REASSEMBLY: u32 = 576;

/// Where a type 2 routing header's address and a type 4 one's segments start, after the same eight bytes.
///
/// https://www.rfc-editor.org/rfc/rfc6275#section-6.4 and https://www.rfc-editor.org/rfc/rfc8754#section-2
const ROUTING_ADDRESSES: usize = offset_of!(rt0_hdr, addr);

/// The request being sent and the reply being read, kept so that a lookup allocates no buffer once they have grown.
#[derive(Default)]
pub(crate) struct Buffers {
    request: Vec<u8>,
    reply: Vec<u8>,
}

/// Resolves one route on a routing socket opened in the calling thread's network namespace.
pub(crate) fn resolve(
    buffers: &mut Buffers,
    shortfalls: &Shortfalls,
    route: RouteSelectors<'_>,
    owner: UserId,
    neighbour: NeighbourConfirmation,
) -> Result<Route, HostError> {
    let lookup = Lookup {
        routing: RefCell::new(RoutingSocket::open(buffers)?),
        shortfalls,
    };
    let (found, resolved) = match &route {
        RouteSelectors::V4(route) => lookup.resolve_v4(route, owner)?,
        RouteSelectors::V6(route) => match route.destination.to_ipv4_mapped() {
            Some(destination) if route.routing_header.is_none() => {
                lookup.resolve_mapped(route, destination, owner)?
            }
            _ => lookup.resolve_v6(route, owner)?,
        },
    };
    if neighbour == NeighbourConfirmation::Confirmed && found.kind == RouteKind::Unicast {
        lookup.confirm_neighbour(found.next_hop, found.interface)?;
    }
    Ok(resolved)
}

struct Lookup<'a> {
    routing: RefCell<RoutingSocket<'a>>,
    shortfalls: &'a Shortfalls,
}

struct RoutingSocket<'a> {
    socket: Socket,
    sequence: u32,
    buffers: &'a mut Buffers,
}

impl<'a> RoutingSocket<'a> {
    /// Opens a routing socket, which answers for the namespace the calling thread is in now.
    fn open(buffers: &'a mut Buffers) -> Result<Self, HostError> {
        let socket = Socket::new(NETLINK_ROUTE).map_err(io_error)?;
        socket
            .connect(&NetlinkAddress::new(KERNEL.0, KERNEL.1))
            .map_err(io_error)?;
        Ok(Self {
            socket,
            sequence: u32::MIN,
            buffers,
        })
    }

    fn exchange(&mut self, message: RouteNetlinkMessage) -> Result<RouteNetlinkMessage, HostError> {
        self.send(message, NLM_F_REQUEST)?;
        loop {
            self.receive()?;
            let reply = NetlinkMessage::<RouteNetlinkMessage>::deserialize(&self.buffers.reply)
                .map_err(|_| HostError::Errno(libc::EPROTO))?;
            if reply.header.sequence_number != self.sequence {
                continue;
            }
            return match reply.payload {
                NetlinkPayload::InnerMessage(message) => Ok(message),
                NetlinkPayload::Error(error) => Err(netlink_error(&error)),
                _ => Err(HostError::Errno(libc::EPROTO)),
            };
        }
    }

    /// Asks for a dump and tells whether any message of it matches, reading it to its end.
    fn dump(
        &mut self,
        message: RouteNetlinkMessage,
        matches: impl Fn(&RouteNetlinkMessage) -> bool,
    ) -> Result<bool, HostError> {
        self.send(message, NLM_F_REQUEST | NLM_F_DUMP)?;
        let mut matched = false;
        loop {
            self.receive()?;
            let mut rest = self.buffers.reply.as_slice();
            while !rest.is_empty() {
                let reply = NetlinkMessage::<RouteNetlinkMessage>::deserialize(rest)
                    .map_err(|_| HostError::Errno(libc::EPROTO))?;
                let length = usize::try_from(reply.header.length)
                    .map_err(|_| HostError::Errno(libc::EPROTO))?
                    .next_multiple_of(usize::from(NLMSG_ALIGNTO));
                if reply.header.sequence_number == self.sequence {
                    match reply.payload {
                        NetlinkPayload::Done(_) => return Ok(matched),
                        NetlinkPayload::InnerMessage(message) => matched |= matches(&message),
                        NetlinkPayload::Error(error) => return Err(netlink_error(&error)),
                        _ => {}
                    }
                }
                rest = rest.get(length..).unwrap_or_default();
            }
        }
    }

    fn send(&mut self, message: RouteNetlinkMessage, flags: u16) -> Result<(), HostError> {
        self.sequence = self.sequence.wrapping_add(1);
        let mut header = NetlinkHeader::default();
        header.flags = flags;
        header.sequence_number = self.sequence;
        let mut packet = NetlinkMessage::new(header, NetlinkPayload::from(message));
        packet.finalize();
        let buffers = &mut *self.buffers;
        buffers.request.clear();
        buffers.request.resize(packet.buffer_len(), u8::MIN);
        packet.serialize(&mut buffers.request);
        self.socket
            .send(&buffers.request, NO_FLAGS)
            .map_err(io_error)?;
        Ok(())
    }

    /// Reads one datagram of the kernel's into the reply buffer.
    fn receive(&mut self) -> Result<(), HostError> {
        let reply = &mut self.buffers.reply;
        // A peek tells the datagram's whole length, so that the buffer grows to take it before it is read.
        reply.clear();
        let length = self
            .socket
            .recv(reply, libc::MSG_PEEK | libc::MSG_TRUNC)
            .map_err(io_error)?;
        reply.clear();
        reply.reserve(length);
        self.socket.recv(reply, NO_FLAGS).map_err(io_error)?;
        Ok(())
    }
}

impl Lookup<'_> {
    fn resolve_v4(
        &self,
        route: &Ipv4RouteSelectors<'_>,
        owner: UserId,
    ) -> Result<(FoundRoute, Route), HostError> {
        if route.scope == RouteScope::Link {
            return Err(self.shortfalls.refuse(LINK_SCOPE));
        }
        if !route.options.as_slice().is_empty() {
            // The options are taken as a send takes them, or refused with the error `IP_OPTIONS` gives.
            set_packet_setting(
                datagram_socket(IpVersion::V4)?.as_fd(),
                IpVersion::V4,
                &IpPacketSetting::Ipv4Options(route.options),
                self.shortfalls,
            )?;
        }
        let destination = match source_route_first_hop(route.options)? {
            None => route.destination,
            Some(SourceRoute::Loose(first_hop)) => first_hop,
            Some(SourceRoute::Strict) => return Err(self.shortfalls.refuse(LINK_SCOPE)),
        };
        if let Some(source) = route.source
            && route.source_address_check == SourceAddressCheck::Waived
            && !self.is_local_v4(source)?
        {
            return Err(self.shortfalls.refuse(WAIVED_SOURCE_CHECK));
        }
        let mut request = RouteMessage::default();
        request.header.address_family = AddressFamily::Inet;
        request.header.destination_prefix_length = full_prefix(Ipv4Addr::BITS);
        request.header.tos = traffic_class(route.dscp, route.ecn);
        request
            .attributes
            .push(RouteAttribute::Destination(RouteAddress::Inet(destination)));
        if let Some(source) = route.source {
            request.header.source_prefix_length = full_prefix(Ipv4Addr::BITS);
            request
                .attributes
                .push(RouteAttribute::Source(RouteAddress::Inet(source)));
        }
        let protocol = self.selectors(
            &mut request,
            IpVersion::V4,
            route.interface,
            route.mark.0,
            owner,
            route.upper_layer,
        )?;
        let found = self.found(
            &self.exchange_route(request.clone())?,
            IpAddr::V4(destination),
        )?;
        if protocol == Protocol::Withheld {
            self.protocol_not_hashed(&request)?;
        }
        if found.kind == RouteKind::Broadcast && route.broadcast == BroadcastPermission::Refused {
            return Err(HostError::Errno(libc::EACCES));
        }
        let link = self.read_link(found.interface)?;
        let default_hop_limit = match found.hop_limit {
            Some(limit) => limit,
            // A socket whose `IP_TTL` was never set gives the namespace's default.
            None => datagram_socket(IpVersion::V4)?.ttl_v4().map_err(io_error)?,
        };
        let path_mtu = if found.encapsulated {
            self.probe(
                &request,
                &found,
                &ProbeSelectors {
                    destination: IpAddr::V4(destination),
                    source: route.source.map(IpAddr::V4),
                    owner,
                    mark: route.mark,
                    dscp: route.dscp,
                    ecn: route.ecn,
                    preferences: None,
                },
            )?
            .path_mtu
        } else {
            match found.mtu {
                Some(mtu) => mtu,
                None if found.mtu_locked && found.through_gateway => {
                    link.mtu.min(MINIMUM_REASSEMBLY)
                }
                None => link.mtu,
            }
            .min(IP_MAX_MTU)
        };
        let resolved = Route {
            kind: found.kind,
            // A link with no IPv4 address gives no preferred source; a socket connected along it gets the unspecified one.
            source: match (route.source, found.source) {
                (Some(source), _) => IpAddr::V4(source),
                (None, Some(source)) => source,
                (None, None) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            },
            interface: found.interface,
            path_mtu: MaximumTransmissionUnit(path_mtu),
            device_mtu: MaximumTransmissionUnit(link.mtu),
            default_hop_limit: hop_limit(default_hop_limit)?,
        };
        Ok((found, resolved))
    }

    fn resolve_v6(
        &self,
        route: &Ipv6RouteSelectors<'_>,
        owner: UserId,
    ) -> Result<(FoundRoute, Route), HostError> {
        // Every IPv6 send of Linux takes the unspecified destination for loopback's address.
        let destination = if route.destination.is_unspecified() {
            Ipv6Addr::LOCALHOST
        } else {
            route.destination
        };
        let destination = match &route.routing_header {
            None => destination,
            Some(header) => self.routing_header_first_hop(header, destination)?,
        };
        let mut request = RouteMessage::default();
        request.header.address_family = AddressFamily::Inet6;
        request.header.destination_prefix_length = full_prefix(Ipv6Addr::BITS);
        request.header.tos = traffic_class(route.dscp, route.ecn);
        request
            .attributes
            .push(RouteAttribute::Destination(RouteAddress::Inet6(
                destination,
            )));
        if let Some(source) = route.source {
            if route.source_address_check == SourceAddressCheck::Required {
                has_address_v6(source, route.interface)?;
            }
            request.header.source_prefix_length = full_prefix(Ipv6Addr::BITS);
            request
                .attributes
                .push(RouteAttribute::Source(RouteAddress::Inet6(source)));
        }
        request
            .attributes
            .push(RouteAttribute::Flowlabel(route.flow_label.value()));
        let protocol = self.selectors(
            &mut request,
            IpVersion::V6,
            route.interface,
            route.mark.0,
            owner,
            route.upper_layer,
        )?;
        let found = self.found(
            &self.exchange_route(request.clone())?,
            IpAddr::V6(destination),
        )?;
        if protocol == Protocol::Withheld {
            self.protocol_not_hashed(&request)?;
        }
        let link = self.read_link(found.interface)?;
        // A source an address of the host's own routes from itself whatever the preferences, as the first rule of choosing a source says.
        let preferences = (route.source.is_none()
            && found.kind != RouteKind::Local
            && route.source_preferences != DEFAULT_SOURCE_PREFERENCES)
            .then_some(route.source_preferences);
        let probe = if found.encapsulated || preferences.is_some() {
            Some(self.probe(
                &request,
                &found,
                &ProbeSelectors {
                    destination: IpAddr::V6(destination),
                    source: route.source.map(IpAddr::V6),
                    owner,
                    mark: route.mark,
                    dscp: route.dscp,
                    ecn: route.ecn,
                    preferences,
                },
            )?)
        } else {
            None
        };
        let source = match (route.source, &probe) {
            (Some(source), _) => IpAddr::V6(source),
            (None, Some(probe)) if preferences.is_some() => probe.source,
            (None, _) => found.source.ok_or(HostError::Errno(libc::EPROTO))?,
        };
        let conf = link.ipv6.ok_or(HostError::Errno(libc::EPROTO))?;
        let resolved = Route {
            kind: found.kind,
            source,
            interface: found.interface,
            path_mtu: MaximumTransmissionUnit(match (&probe, found.mtu) {
                (Some(probe), _) if found.encapsulated => probe.path_mtu,
                (_, Some(mtu)) => mtu,
                (_, None) => u32::try_from(conf.mtu).map_err(|_| HostError::Errno(libc::EPROTO))?,
            }),
            device_mtu: MaximumTransmissionUnit(link.mtu),
            default_hop_limit: hop_limit(match found.hop_limit {
                Some(limit) => limit,
                None => {
                    u32::try_from(conf.hop_limit).map_err(|_| HostError::Errno(libc::EPROTO))?
                }
            })?,
        };
        Ok((found, resolved))
    }

    /// Resolves an IPv4-mapped destination over IPv4, as a dual-stack socket sends to one, with the source mapped back.
    fn resolve_mapped(
        &self,
        route: &Ipv6RouteSelectors<'_>,
        destination: Ipv4Addr,
        owner: UserId,
    ) -> Result<(FoundRoute, Route), HostError> {
        let source = match route.source {
            None => None,
            Some(source) => Some(
                source
                    .to_ipv4_mapped()
                    .ok_or_else(|| self.shortfalls.refuse(MIXED_SOURCE))?,
            ),
        };
        let options = Ipv4Options::new();
        let (found, mut resolved) = self.resolve_v4(
            &Ipv4RouteSelectors {
                destination,
                source,
                source_address_check: route.source_address_check,
                interface: route.interface,
                dscp: route.dscp,
                ecn: route.ecn,
                mark: route.mark,
                scope: RouteScope::Universe,
                broadcast: BroadcastPermission::Refused,
                options: &options,
                upper_layer: route.upper_layer,
            },
            owner,
        )?;
        if let IpAddr::V4(source) = resolved.source {
            resolved.source = IpAddr::V6(source.to_ipv6_mapped());
        }
        Ok((found, resolved))
    }

    /// Returns the address a routing header sends the packet to first, once the host accepted it as `IPV6_RTHDR`.
    fn routing_header_first_hop(
        &self,
        header: &Ipv6RawExtHeaderSlice<'_>,
        destination: Ipv6Addr,
    ) -> Result<Ipv6Addr, HostError> {
        let check = datagram_socket(IpVersion::V6)?;
        set_packet_setting(
            check.as_fd(),
            IpVersion::V6,
            &IpPacketSetting::RoutingHeader(Some(header.clone())),
            self.shortfalls,
        )?;
        let bytes = header.slice();
        let byte = |offset| {
            bytes
                .get(offset)
                .copied()
                .ok_or(HostError::Errno(libc::EINVAL))
        };
        let index = match u32::from(byte(offset_of!(ipv6_rt_hdr, type_))?) {
            // A type 2 header holds one address.
            IPV6_SRCRT_TYPE_2 => usize::MIN,
            IPV6_SRCRT_TYPE_4 => usize::from(byte(offset_of!(ipv6_rt_hdr, segments_left))?),
            _ => return Ok(destination),
        };
        let start = ROUTING_ADDRESSES + index * size_of::<in6_addr>();
        bytes
            .get(start..)
            .and_then(|addresses| addresses.first_chunk::<{ size_of::<in6_addr>() }>())
            .map(|&octets| Ipv6Addr::from(octets))
            .ok_or(HostError::Errno(libc::EINVAL))
    }

    /// Tells whether the host has the address, as the kernel's IPv4 source check asks: its own lookup of it is local.
    fn is_local_v4(&self, address: Ipv4Addr) -> Result<bool, HostError> {
        let mut request = RouteMessage::default();
        request.header.address_family = AddressFamily::Inet;
        request.header.destination_prefix_length = full_prefix(Ipv4Addr::BITS);
        request
            .attributes
            .push(RouteAttribute::Destination(RouteAddress::Inet(address)));
        match self.exchange(RouteNetlinkMessage::GetRoute(request)) {
            Ok(RouteNetlinkMessage::NewRoute(reply)) => Ok(reply.header.kind == RouteType::Local),
            Ok(_) => Err(HostError::Errno(libc::EPROTO)),
            Err(HostError::Errno(_)) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Confirms the neighbour as a send with `MSG_CONFIRM` does, by a probe on the route's interface that sends nothing.
    fn confirm_neighbour(
        &self,
        next_hop: IpAddr,
        interface: InterfaceIndex,
    ) -> Result<(), HostError> {
        let version = match next_hop {
            IpAddr::V4(_) => IpVersion::V4,
            IpAddr::V6(_) => IpVersion::V6,
        };
        let socket = datagram_socket(version)?;
        set_packet_setting(
            socket.as_fd(),
            version,
            &IpPacketSetting::UnicastInterface(Some(interface)),
            self.shortfalls,
        )?;
        socket
            .send_to_with_flags(
                &[],
                &socket_address(next_hop, PROBE_PORT, interface),
                CONFIRMING_PROBE,
            )
            .map_err(io_error)?;
        Ok(())
    }

    fn exchange(&self, message: RouteNetlinkMessage) -> Result<RouteNetlinkMessage, HostError> {
        self.routing.borrow_mut().exchange(message)
    }

    fn exchange_route(&self, request: RouteMessage) -> Result<RouteMessage, HostError> {
        match self.exchange(RouteNetlinkMessage::GetRoute(request)) {
            Ok(RouteNetlinkMessage::NewRoute(reply)) => Ok(reply),
            Ok(_) => Err(HostError::Errno(libc::EPROTO)),
            Err(HostError::Errno(libc::EOPNOTSUPP)) => {
                Err(self.shortfalls.refuse(UPPER_LAYER_PROTOCOL))
            }
            Err(error) => Err(error),
        }
    }

    fn found(&self, reply: &RouteMessage, destination: IpAddr) -> Result<FoundRoute, HostError> {
        let kind = match reply.header.kind {
            RouteType::Unicast => RouteKind::Unicast,
            RouteType::Local => RouteKind::Local,
            RouteType::Broadcast => RouteKind::Broadcast,
            RouteType::Multicast => RouteKind::Multicast,
            _ => return Err(self.shortfalls.refuse(ROUTE_KIND)),
        };
        let mut source = None;
        let mut interface = None;
        let mut next_hop = destination;
        let mut through_gateway = false;
        let mut mtu = None;
        let mut mtu_locked = false;
        let mut hop_limit = None;
        let mut encapsulated = false;
        for attribute in &reply.attributes {
            match attribute {
                RouteAttribute::PrefSource(address) => source = address_of(address),
                RouteAttribute::Oif(index) => interface = Some(InterfaceIndex(*index)),
                RouteAttribute::Gateway(address) => {
                    next_hop = address_of(address).ok_or(HostError::Errno(libc::EPROTO))?;
                    through_gateway = true;
                }
                RouteAttribute::Via(RouteVia::Inet(address)) => {
                    next_hop = IpAddr::V4(*address);
                    through_gateway = true;
                }
                RouteAttribute::Via(RouteVia::Inet6(address)) => {
                    next_hop = IpAddr::V6(*address);
                    through_gateway = true;
                }
                RouteAttribute::Encap(_) | RouteAttribute::EncapType(_) => encapsulated = true,
                RouteAttribute::Metrics(metrics) => {
                    for metric in metrics {
                        match metric {
                            RouteMetric::Mtu(value) => mtu = Some(*value),
                            RouteMetric::Hoplimit(value) => hop_limit = Some(*value),
                            RouteMetric::Lock(locked) => {
                                mtu_locked = locked & (1 << RTAX_MTU as u32) != 0;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(FoundRoute {
            kind,
            source,
            interface: interface.ok_or(HostError::Errno(libc::EPROTO))?,
            next_hop,
            through_gateway,
            mtu,
            mtu_locked,
            hop_limit,
            encapsulated,
        })
    }

    /// Reads what a datagram socket connected along the lookup's routing entry is told, once the routing socket confirms its connect took that entry.
    fn probe(
        &self,
        request: &RouteMessage,
        found: &FoundRoute,
        selectors: &ProbeSelectors,
    ) -> Result<Probe, HostError> {
        let version = match selectors.destination {
            IpAddr::V4(_) => IpVersion::V4,
            IpAddr::V6(_) => IpVersion::V6,
        };
        // A socket's routing owner is the filesystem user that made it, which takes `CAP_SETUID` to be another.
        let socket = DatagramSocket::from(
            open_socket(version, SocketType::DGRAM, ipproto::UDP, selectors.owner)
                .map_err(|_| self.shortfalls.refuse(PROBE))?,
        );
        let set = |setting: &IpPacketSetting<'_>| {
            set_packet_setting(socket.as_fd(), version, setting, self.shortfalls)
        };
        set(&IpPacketSetting::UnicastInterface(Some(found.interface)))?;
        set(&IpPacketSetting::TrafficClass {
            version,
            dscp: selectors.dscp,
            ecn: selectors.ecn,
        })?;
        if selectors.mark != PacketMark(u32::MIN) {
            // Setting a mark takes `CAP_NET_ADMIN`, which the routing socket does not need.
            set(&IpPacketSetting::Mark(selectors.mark))
                .map_err(|_| self.shortfalls.refuse(PROBE))?;
        }
        if let Some(preferences) = selectors.preferences {
            set(&IpPacketSetting::SourcePreferences(preferences))?;
        }
        if found.kind == RouteKind::Broadcast {
            socket.set_broadcast(true).map_err(io_error)?;
        }
        if let IpAddr::V6(destination) = selectors.destination
            && destination.is_multicast()
        {
            socket
                .set_multicast_if_v6(found.interface.0)
                .map_err(io_error)?;
        }
        if let Some(source) = selectors.source {
            socket
                .bind(&socket_address(source, ANY_PORT, found.interface))
                .map_err(|_| self.shortfalls.refuse(PROBE))?;
        }
        socket
            .connect(&socket_address(
                selectors.destination,
                PROBE_PORT,
                found.interface,
            ))
            .map_err(|_| self.shortfalls.refuse(PROBE))?;
        let local = socket
            .local_addr()
            .map_err(io_error)?
            .as_socket()
            .ok_or(HostError::Errno(libc::EPROTO))?;
        let path_mtu = match version {
            IpVersion::V4 => rustix::net::sockopt::ip_mtu(&socket),
            IpVersion::V6 => rustix::net::sockopt::ipv6_mtu(&socket),
        }
        .map_err(errno_error)?;
        let mut taken = RouteMessage::default();
        taken.header.address_family = request.header.address_family;
        taken.header.destination_prefix_length = request.header.destination_prefix_length;
        taken.header.tos = traffic_class(selectors.dscp, selectors.ecn);
        let address = |address| match address {
            IpAddr::V4(address) => RouteAddress::Inet(address),
            IpAddr::V6(address) => RouteAddress::Inet6(address),
        };
        let attributes = &mut taken.attributes;
        attributes.push(RouteAttribute::Destination(address(selectors.destination)));
        if let Some(source) = selectors.source {
            taken.header.source_prefix_length = request.header.source_prefix_length;
            attributes.push(RouteAttribute::Source(address(source)));
        }
        attributes.push(RouteAttribute::Oif(found.interface.0));
        attributes.push(RouteAttribute::Mark(selectors.mark.0));
        attributes.push(RouteAttribute::Uid(selectors.owner.0));
        attributes.push(RouteAttribute::IpProto(IpNumber::UDP.0));
        attributes.push(RouteAttribute::Sport(local.port()));
        attributes.push(RouteAttribute::Dport(PROBE_PORT));
        if self.entry(request.clone())? != self.entry(taken)? {
            return Err(self.shortfalls.refuse(PROBE));
        }
        Ok(Probe {
            source: local.ip(),
            path_mtu,
        })
    }

    /// Returns the routing entry a lookup matches, without what changes from one asking to the next.
    fn entry(&self, mut request: RouteMessage) -> Result<RouteMessage, HostError> {
        request.header.flags |= RouteFlags::FibMatch;
        let mut entry = self.exchange_route(request)?;
        entry
            .attributes
            .retain(|attribute| !matches!(attribute, RouteAttribute::CacheInfo(_)));
        Ok(entry)
    }

    fn read_link(&self, interface: InterfaceIndex) -> Result<Link, HostError> {
        let mut request = LinkMessage::default();
        request.header.index = interface.0;
        let RouteNetlinkMessage::NewLink(reply) =
            self.exchange(RouteNetlinkMessage::GetLink(request))?
        else {
            return Err(HostError::Errno(libc::EPROTO));
        };
        let mut mtu = None;
        let mut ipv6 = None;
        for attribute in &reply.attributes {
            match attribute {
                LinkAttribute::Mtu(value) => mtu = Some(*value),
                LinkAttribute::AfSpecUnspec(families) => {
                    for family in families {
                        if let AfSpecUnspec::Inet6(settings) = family {
                            for setting in settings {
                                if let AfSpecInet6::DevConf(conf) = setting {
                                    ipv6 = Some(Ipv6DeviceSettings {
                                        mtu: conf.mtu6,
                                        hop_limit: conf.hoplimit,
                                    });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(Link {
            mtu: mtu.ok_or(HostError::Errno(libc::EPROTO))?,
            ipv6,
        })
    }

    /// Adds the selectors both versions share: interface, mark, owner and the upper layer.
    fn selectors(
        &self,
        request: &mut RouteMessage,
        version: IpVersion,
        interface: Option<InterfaceIndex>,
        mark: u32,
        owner: UserId,
        upper_layer: UpperLayer,
    ) -> Result<Protocol, HostError> {
        let protocol = if routing_socket_takes(version, upper_layer.protocol) {
            Protocol::Given
        } else {
            self.no_rule_selects(version, upper_layer.protocol)?;
            Protocol::Withheld
        };
        let attributes = &mut request.attributes;
        if let Some(interface) = interface {
            attributes.push(RouteAttribute::Oif(interface.0));
        }
        attributes.push(RouteAttribute::Mark(mark));
        attributes.push(RouteAttribute::Uid(owner.0));
        if protocol == Protocol::Given {
            attributes.push(RouteAttribute::IpProto(upper_layer.protocol.0));
        }
        match upper_layer.selector {
            None => {}
            Some(UpperLayerSelector::Ports(ports)) => {
                attributes.push(RouteAttribute::Sport(ports.source.0));
                attributes.push(RouteAttribute::Dport(ports.destination.0));
            }
            // The kernel's flow keeps an ICMP type and code where the destination port's bytes are.
            Some(UpperLayerSelector::Icmp { message_type, code }) => {
                attributes.push(RouteAttribute::Dport(u16::from_be_bytes([
                    message_type.0,
                    code.0,
                ])));
            }
            // The kernel's flow keeps a mobility header type in the destination port's first byte.
            Some(UpperLayerSelector::MobilityHeader(message_type)) => {
                attributes.push(RouteAttribute::Dport(u16::from_be_bytes([
                    message_type.0,
                    u8::MIN,
                ])));
            }
        }
        Ok(protocol)
    }

    /// Refuses a protocol the routing socket does not take when one of the version's rules selects on it or on what a lookup without one is asked as.
    ///
    /// A lookup without the protocol differs from Linux's own only there and in a hash among next hops.
    fn no_rule_selects(&self, version: IpVersion, protocol: IpNumber) -> Result<(), HostError> {
        let mut request = RuleMessage::default();
        // The IPv4 routing socket asks a lookup without a protocol as UDP, the IPv6 one as protocol 0, which no rule selects.
        let (family, asked_as) = match version {
            IpVersion::V4 => (AddressFamily::Inet, Some(IpNumber::UDP)),
            IpVersion::V6 => (AddressFamily::Inet6, None),
        };
        request.header.family = family;
        let selected = |number: u8| number == protocol.0 || Some(IpNumber(number)) == asked_as;
        let selects = self.routing.borrow_mut().dump(
            RouteNetlinkMessage::GetRule(request),
            |message| {
                matches!(message, RouteNetlinkMessage::NewRule(rule) if rule.attributes.iter().any(
                    |attribute| matches!(attribute, RuleAttribute::IpProtocol(number) if selected(u8::from(*number)))
                ))
            },
        )?;
        if selects {
            return Err(self.shortfalls.refuse(RULE_ON_PROTOCOL));
        }
        Ok(())
    }

    /// Refuses a lookup without its protocol whose routing entry has several next hops, among which Linux may choose by a hash of the protocol.
    fn protocol_not_hashed(&self, request: &RouteMessage) -> Result<(), HostError> {
        let several = self
            .entry(request.clone())?
            .attributes
            .iter()
            .any(|attribute| {
                matches!(
                    attribute,
                    RouteAttribute::MultiPath(_) | RouteAttribute::NhId(_)
                )
            });
        if several {
            return Err(self.shortfalls.refuse(HASHED_PROTOCOL));
        }
        Ok(())
    }
}

/// Whether the routing socket was given the lookup's protocol.
#[derive(PartialEq, Eq)]
enum Protocol {
    Given,
    Withheld,
}

/// Tells whether the routing socket takes the protocol: TCP, UDP and the version's ICMP.
fn routing_socket_takes(version: IpVersion, protocol: IpNumber) -> bool {
    let icmp = match version {
        IpVersion::V4 => IpNumber::ICMP,
        IpVersion::V6 => IpNumber::IPV6_ICMP,
    };
    [IpNumber::TCP, IpNumber::UDP, icmp].contains(&protocol)
}

struct FoundRoute {
    kind: RouteKind,
    source: Option<IpAddr>,
    interface: InterfaceIndex,
    /// The gateway, or the destination of a route on the link.
    next_hop: IpAddr,
    through_gateway: bool,
    mtu: Option<u32>,
    mtu_locked: bool,
    hop_limit: Option<u32>,
    encapsulated: bool,
}

/// The selectors of a lookup a probe socket takes, besides the route's interface.
struct ProbeSelectors {
    destination: IpAddr,
    source: Option<IpAddr>,
    owner: UserId,
    mark: PacketMark,
    dscp: IpDscp,
    ecn: IpEcn,
    preferences: Option<SourcePreferences>,
}

/// What a datagram socket connected along a routing entry is told: the source Linux chose and the path MTU.
struct Probe {
    source: IpAddr,
    path_mtu: u32,
}

struct Link {
    mtu: u32,
    ipv6: Option<Ipv6DeviceSettings>,
}

struct Ipv6DeviceSettings {
    mtu: i32,
    hop_limit: i32,
}

enum SourceRoute {
    Loose(Ipv4Addr),
    Strict,
}

/// Returns the first hop of a source route among IPv4 options in the form `IP_OPTIONS` takes.
fn source_route_first_hop(options: &Ipv4Options) -> Result<Option<SourceRoute>, HostError> {
    let malformed = HostError::Errno(libc::EINVAL);
    let mut rest = options.as_slice();
    while let Some(&kind) = rest.first() {
        match kind {
            libc::IPOPT_END => break,
            libc::IPOPT_NOOP => rest = &rest[1..],
            _ => {
                let length =
                    usize::from(*rest.get(usize::from(libc::IPOPT_OLEN)).ok_or(malformed)?);
                if length <= usize::from(libc::IPOPT_OLEN) {
                    return Err(malformed);
                }
                let option = rest.get(..length).ok_or(malformed)?;
                if kind == libc::IPOPT_LSRR || kind == libc::IPOPT_SSRR {
                    let first_hop = option
                        .get(usize::from(libc::IPOPT_OFFSET) + 1..)
                        .and_then(|addresses| addresses.first_chunk())
                        .map(|&octets| Ipv4Addr::from(octets))
                        .ok_or(malformed)?;
                    return Ok(Some(if kind == libc::IPOPT_LSRR {
                        SourceRoute::Loose(first_hop)
                    } else {
                        SourceRoute::Strict
                    }));
                }
                rest = &rest[length..];
            }
        }
    }
    Ok(None)
}

/// Fails as `bind` does when the host has no such address, the check the IPv6 routing socket leaves out.
fn has_address_v6(address: Ipv6Addr, interface: Option<InterfaceIndex>) -> Result<(), HostError> {
    let socket = datagram_socket(IpVersion::V6)?;
    let address = SocketAddrV6::new(address, ANY_PORT, u32::MIN, scope_of(address, interface));
    socket.bind(&SockAddr::from(address)).map_err(io_error)
}

/// Returns the socket address of a port at the address, with the interface as its scope when it is link-local.
fn socket_address(address: IpAddr, port: u16, interface: InterfaceIndex) -> SockAddr {
    SockAddr::from(match address {
        IpAddr::V4(address) => SocketAddr::V4(SocketAddrV4::new(address, port)),
        IpAddr::V6(address) => SocketAddr::V6(SocketAddrV6::new(
            address,
            port,
            u32::MIN,
            scope_of(address, Some(interface)),
        )),
    })
}

/// Returns the scope a link-local address needs, the interface's; none for any other.
fn scope_of(address: Ipv6Addr, interface: Option<InterfaceIndex>) -> u32 {
    match interface {
        Some(interface) if address.is_unicast_link_local() => interface.0,
        _ => u32::MIN,
    }
}

fn datagram_socket(version: IpVersion) -> Result<DatagramSocket, HostError> {
    let domain = match version {
        IpVersion::V4 => Domain::IPV4,
        IpVersion::V6 => Domain::IPV6,
    };
    DatagramSocket::new(domain, Type::DGRAM, None).map_err(io_error)
}

fn address_of(address: &RouteAddress) -> Option<IpAddr> {
    match address {
        RouteAddress::Inet(address) => Some(IpAddr::V4(*address)),
        RouteAddress::Inet6(address) => Some(IpAddr::V6(*address)),
        _ => None,
    }
}

fn traffic_class(dscp: IpDscp, ecn: IpEcn) -> u8 {
    (dscp.value() << IpEcn::MAX_U8.count_ones()) | ecn.value()
}

fn full_prefix(bits: u32) -> u8 {
    u8::try_from(bits).unwrap_or(u8::MAX)
}

fn hop_limit(limit: u32) -> Result<HopLimit, HostError> {
    u8::try_from(limit)
        .map(HopLimit)
        .map_err(|_| HostError::Errno(libc::EPROTO))
}

fn netlink_error(error: &ErrorMessage) -> HostError {
    HostError::Errno(
        error
            .code
            .map_or(libc::EPROTO, |code| code.get().saturating_neg()),
    )
}

fn io_error(error: io::Error) -> HostError {
    io_host_error(&error)
}

fn errno_error(error: rustix::io::Errno) -> HostError {
    io_error(error.into())
}
