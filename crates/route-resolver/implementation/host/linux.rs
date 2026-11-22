use core::cell::RefCell;
use core::mem::offset_of;
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::io;
use std::os::fd::AsFd;

use etherparse::{IpDscp, IpEcn, Ipv4Options, Ipv6RawExtHeaderSlice};
use linux_raw_sys::net::{
    IPV6_SRCRT_TYPE_2, IPV6_SRCRT_TYPE_4, MSG_PROBE, in6_addr, ipv6_rt_hdr, rt0_hdr,
};
use linux_raw_sys::netlink::RTAX_MTU;
use netlink_packet_core::{NLM_F_REQUEST, NetlinkHeader, NetlinkMessage, NetlinkPayload};
use netlink_packet_route::link::{AfSpecInet6, AfSpecUnspec, LinkAttribute, LinkMessage};
use netlink_packet_route::route::{
    RouteAddress, RouteAttribute, RouteMessage, RouteMetric, RouteType, RouteVia,
};
use netlink_packet_route::{AddressFamily, RouteNetlinkMessage};
use netlink_sys::protocols::NETLINK_ROUTE;
use netlink_sys::{Socket, SocketAddr as NetlinkAddress};
use socket2::{Domain, SockAddr, Socket as DatagramSocket, Type};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    BroadcastPermission, HopLimit, InterfaceIndex, IpPacketSetting, IpVersion, Ipv4RouteSelectors,
    Ipv6RouteSelectors, MaximumTransmissionUnit, MobileAddressPreference, NeighbourConfirmation,
    Route, RouteKind, RouteScope, RouteSelectors, SourceAddressCheck, SourcePreferences,
    TemporaryAddressPreference, UpperLayer, UpperLayerSelector,
};
use wcontainers_internet_protocol_linux_socket_options::{
    Shortfalls, io_host_error, set_packet_setting,
};
use wcontainers_kernel::UserId;

const LINK_SCOPE: &str = "route lookup: a link-scope IPv4 lookup the routing socket cannot ask for";
const WAIVED_SOURCE_CHECK: &str = "route lookup: a waived source check for an IPv4 address the host lacks, which the routing socket always checks";
const SOURCE_PREFERENCES: &str = "route lookup: IPv6 source preferences other than the default, which the routing socket cannot take";
const UPPER_LAYER_PROTOCOL: &str = "route lookup: an upper-layer protocol the routing socket does not take (it takes TCP, UDP, ICMP and ICMPv6)";
const ROUTE_KIND: &str = "route lookup: a route kind the contract has no name for";
const ENCAPSULATION: &str = "route lookup: the path MTU of an encapsulating route, less a headroom the routing socket does not give";

const DEFAULT_SOURCE_PREFERENCES: SourcePreferences = SourcePreferences {
    temporary: TemporaryAddressPreference::SystemDefault,
    mobile: MobileAddressPreference::Home,
};

/// The port number that asks `bind` for any port.
const ANY_PORT: u16 = u16::MIN;

/// The destination port of a neighbour confirmation, which sends nothing but needs a port.
const CONFIRMATION_PORT: u16 = u16::MAX;

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
        RouteSelectors::V6(route) => lookup.resolve_v6(route, owner)?,
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
        self.sequence = self.sequence.wrapping_add(1);
        let mut header = NetlinkHeader::default();
        header.flags = NLM_F_REQUEST;
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
        loop {
            // A peek tells the reply's whole length, so that the buffer grows to take it before it is read.
            buffers.reply.clear();
            let length = self
                .socket
                .recv(&mut buffers.reply, libc::MSG_PEEK | libc::MSG_TRUNC)
                .map_err(io_error)?;
            buffers.reply.clear();
            buffers.reply.reserve(length);
            self.socket
                .recv(&mut buffers.reply, NO_FLAGS)
                .map_err(io_error)?;
            let reply = NetlinkMessage::<RouteNetlinkMessage>::deserialize(&buffers.reply)
                .map_err(|_| HostError::Errno(libc::EPROTO))?;
            if reply.header.sequence_number != self.sequence {
                continue;
            }
            return match reply.payload {
                NetlinkPayload::InnerMessage(message) => Ok(message),
                NetlinkPayload::Error(error) => Err(HostError::Errno(
                    error
                        .code
                        .map_or(libc::EPROTO, |code| code.get().saturating_neg()),
                )),
                _ => Err(HostError::Errno(libc::EPROTO)),
            };
        }
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
        self.selectors(
            &mut request,
            route.interface,
            route.mark.0,
            owner,
            route.upper_layer,
        )?;
        let found = self.found(&self.exchange_route(request)?, IpAddr::V4(destination))?;
        if found.kind == RouteKind::Broadcast && route.broadcast == BroadcastPermission::Refused {
            return Err(HostError::Errno(libc::EACCES));
        }
        let link = self.read_link(found.interface)?;
        let default_hop_limit = match found.hop_limit {
            Some(limit) => limit,
            // A socket whose `IP_TTL` was never set gives the namespace's default.
            None => datagram_socket(IpVersion::V4)?.ttl_v4().map_err(io_error)?,
        };
        let path_mtu = match found.mtu {
            Some(mtu) => mtu,
            None if found.mtu_locked && found.through_gateway => link.mtu.min(MINIMUM_REASSEMBLY),
            None => link.mtu,
        };
        let resolved = Route {
            kind: found.kind,
            source: match route.source {
                Some(source) => IpAddr::V4(source),
                None => found.source.ok_or(HostError::Errno(libc::EPROTO))?,
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
        if route.source.is_none() && route.source_preferences != DEFAULT_SOURCE_PREFERENCES {
            return Err(self.shortfalls.refuse(SOURCE_PREFERENCES));
        }
        let destination = match &route.routing_header {
            None => route.destination,
            Some(header) => self.routing_header_first_hop(header, route.destination)?,
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
        self.selectors(
            &mut request,
            route.interface,
            route.mark.0,
            owner,
            route.upper_layer,
        )?;
        let found = self.found(&self.exchange_route(request)?, IpAddr::V6(destination))?;
        let link = self.read_link(found.interface)?;
        let source = match route.source {
            Some(source) => IpAddr::V6(source),
            None => found.source.ok_or(HostError::Errno(libc::EPROTO))?,
        };
        let conf = link.ipv6.ok_or(HostError::Errno(libc::EPROTO))?;
        let resolved = Route {
            kind: found.kind,
            source,
            interface: found.interface,
            path_mtu: MaximumTransmissionUnit(match found.mtu {
                Some(mtu) => mtu,
                None => u32::try_from(conf.mtu).map_err(|_| HostError::Errno(libc::EPROTO))?,
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
        let (version, peer) = match next_hop {
            IpAddr::V4(address) => (
                IpVersion::V4,
                SocketAddr::V4(SocketAddrV4::new(address, CONFIRMATION_PORT)),
            ),
            IpAddr::V6(address) => (
                IpVersion::V6,
                SocketAddr::V6(SocketAddrV6::new(
                    address,
                    CONFIRMATION_PORT,
                    u32::MIN,
                    scope_of(address, Some(interface)),
                )),
            ),
        };
        let socket = datagram_socket(version)?;
        set_packet_setting(
            socket.as_fd(),
            version,
            &IpPacketSetting::UnicastInterface(Some(interface)),
            self.shortfalls,
        )?;
        socket
            .send_to_with_flags(&[], &SockAddr::from(peer), CONFIRMING_PROBE)
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
                RouteAttribute::Encap(_) | RouteAttribute::EncapType(_) => {
                    return Err(self.shortfalls.refuse(ENCAPSULATION));
                }
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
        })
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
        interface: Option<InterfaceIndex>,
        mark: u32,
        owner: UserId,
        upper_layer: UpperLayer,
    ) -> Result<(), HostError> {
        let attributes = &mut request.attributes;
        if let Some(interface) = interface {
            attributes.push(RouteAttribute::Oif(interface.0));
        }
        attributes.push(RouteAttribute::Mark(mark));
        attributes.push(RouteAttribute::Uid(owner.0));
        attributes.push(RouteAttribute::IpProto(upper_layer.protocol.0));
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
            // The routing socket takes no protocol with a mobility header type, which it would read from the same bytes.
            Some(UpperLayerSelector::MobilityHeader(_)) => {
                return Err(self.shortfalls.refuse(UPPER_LAYER_PROTOCOL));
            }
        }
        Ok(())
    }
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

fn io_error(error: io::Error) -> HostError {
    io_host_error(&error)
}
