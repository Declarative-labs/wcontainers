#![cfg(target_os = "linux")]

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};
use std::io;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use etherparse::{IpDscp, IpEcn, Ipv4Options};
use netlink_packet_core::{NLM_F_REQUEST, NetlinkHeader, NetlinkMessage, NetlinkPayload};
use netlink_packet_route::link::{AfSpecInet6, AfSpecUnspec, LinkAttribute, LinkMessage};
use netlink_packet_route::route::{
    RouteAddress, RouteAttribute, RouteMessage, RouteMetric, RouteType,
};
use netlink_packet_route::{AddressFamily, RouteNetlinkMessage};
use netlink_sys::protocols::NETLINK_ROUTE;
use netlink_sys::{Socket, SocketAddr};
use socket2::{Domain, SockAddr, Type};
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_internet_protocol::{
    BroadcastPermission, HopLimit, InterfaceIndex, Ipv4RouteSelectors, Ipv6RouteSelectors,
    MaximumTransmissionUnit, MobileAddressPreference, NeighbourConfirmation, Route, RouteKind,
    RouteScope, RouteSelectors, SourceAddressCheck, SourcePreferences, TemporaryAddressPreference,
    UpperLayer, UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_route_resolver::RouteResolver;

/// A route resolver for the network namespace it was opened in, through the kernel's routing socket.
pub struct LinuxNetlinkRouteResolver {
    routing: Mutex<RoutingSocket>,
    /// A socket whose `IP_TTL` was never set, so that reading it gives the namespace's default.
    unset_hop_limit: socket2::Socket,
    refusals: [AtomicU64; REFUSALS.len()],
}

struct RoutingSocket {
    socket: Socket,
    sequence: u32,
}

#[derive(Clone, Copy)]
enum Refusal {
    LinkScope,
    WaivedSourceCheck,
    SourcePreferences,
    RoutingHeader,
    NeighbourConfirmation,
    UpperLayerProtocol,
    RouteKind,
}

const REFUSALS: [Refusal; 7] = [
    Refusal::LinkScope,
    Refusal::WaivedSourceCheck,
    Refusal::SourcePreferences,
    Refusal::RoutingHeader,
    Refusal::NeighbourConfirmation,
    Refusal::UpperLayerProtocol,
    Refusal::RouteKind,
];

/// The reason of each refusal, in the order of `REFUSALS`.
const REASONS: [&str; REFUSALS.len()] = [
    "link-scope route lookup the routing socket cannot ask for",
    "waived source check for an IPv4 address the host lacks",
    "IPv6 source preferences other than the system's default",
    "IPv6 routing header",
    "neighbour confirmation in a lookup that sends nothing",
    "upper-layer protocol the routing socket does not take",
    "route kind the contract has no name for",
];

impl Refusal {
    fn reason(self) -> &'static str {
        REASONS[self as usize]
    }
}

const DEFAULT_SOURCE_PREFERENCES: SourcePreferences = SourcePreferences {
    temporary: TemporaryAddressPreference::SystemDefault,
    mobile: MobileAddressPreference::Home,
};

/// The port number that asks `bind` for any port.
const ANY_PORT: u16 = 0;

/// The netlink port number and multicast groups of the kernel.
const KERNEL: (u32, u32) = (0, 0);

impl LinuxNetlinkRouteResolver {
    /// Opens a resolver for the calling thread's network namespace.
    pub fn open() -> Result<Self, HostError> {
        let mut socket = Socket::new(NETLINK_ROUTE).map_err(host_error)?;
        socket.bind_auto().map_err(host_error)?;
        socket
            .connect(&SocketAddr::new(KERNEL.0, KERNEL.1))
            .map_err(host_error)?;
        let unset_hop_limit =
            socket2::Socket::new(Domain::IPV4, Type::DGRAM, None).map_err(host_error)?;
        Ok(Self {
            routing: Mutex::new(RoutingSocket {
                socket,
                sequence: u32::MIN,
            }),
            unset_hop_limit,
            refusals: [const { AtomicU64::new(0) }; REFUSALS.len()],
        })
    }

    /// Returns how many lookups were refused for each reason.
    pub fn counters(&self) -> impl Iterator<Item = CounterReading> + '_ {
        REFUSALS.iter().map(|&refusal| CounterReading {
            reason: refusal.reason(),
            detail: 0,
            count: self.refusals[refusal as usize].load(Ordering::Relaxed),
            nanoseconds: 0,
        })
    }

    fn refuse(&self, refusal: Refusal) -> HostError {
        let count = self.refusals[refusal as usize].fetch_add(1, Ordering::Relaxed) + 1;
        log::warn!(
            "route lookup refused: {} ({count} so far)",
            refusal.reason()
        );
        HostError::NotSupported
    }

    fn resolve_v4(
        &self,
        route: &Ipv4RouteSelectors<'_>,
        owner: UserId,
    ) -> Result<Route, HostError> {
        if route.scope == RouteScope::Link {
            return Err(self.refuse(Refusal::LinkScope));
        }
        let destination = match source_route_first_hop(route.options)? {
            None => route.destination,
            Some(SourceRoute::Loose(first_hop)) => first_hop,
            Some(SourceRoute::Strict) => return Err(self.refuse(Refusal::LinkScope)),
        };
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
        selectors(
            &mut request,
            route.interface,
            route.mark.0,
            owner,
            route.upper_layer,
        );
        let reply = match self.exchange_route(request) {
            Ok(reply) => reply,
            Err(error) => {
                return Err(match route.source {
                    Some(source)
                        if route.source_address_check == SourceAddressCheck::Waived
                            && has_address(IpAddr::V4(source), route.interface).is_err() =>
                    {
                        self.refuse(Refusal::WaivedSourceCheck)
                    }
                    _ => error,
                });
            }
        };
        let found = self.found(&reply)?;
        if found.kind == RouteKind::Broadcast && route.broadcast == BroadcastPermission::Refused {
            return Err(HostError::Errno(libc::EACCES));
        }
        let link = self.link(found.interface)?;
        let default_hop_limit = match found.hop_limit {
            Some(limit) => limit,
            None => self.unset_hop_limit.ttl_v4().map_err(host_error)?,
        };
        Ok(Route {
            kind: found.kind,
            source: match route.source {
                Some(source) => IpAddr::V4(source),
                None => found.source.ok_or(HostError::Errno(libc::EPROTO))?,
            },
            interface: found.interface,
            path_mtu: MaximumTransmissionUnit(found.mtu.unwrap_or(link.mtu)),
            device_mtu: MaximumTransmissionUnit(link.mtu),
            default_hop_limit: hop_limit(default_hop_limit)?,
        })
    }

    fn resolve_v6(
        &self,
        route: &Ipv6RouteSelectors<'_>,
        owner: UserId,
    ) -> Result<Route, HostError> {
        if route.routing_header.is_some() {
            return Err(self.refuse(Refusal::RoutingHeader));
        }
        if route.source.is_none() && route.source_preferences != DEFAULT_SOURCE_PREFERENCES {
            return Err(self.refuse(Refusal::SourcePreferences));
        }
        let mut request = RouteMessage::default();
        request.header.address_family = AddressFamily::Inet6;
        request.header.destination_prefix_length = full_prefix(Ipv6Addr::BITS);
        request.header.tos = traffic_class(route.dscp, route.ecn);
        request
            .attributes
            .push(RouteAttribute::Destination(RouteAddress::Inet6(
                route.destination,
            )));
        if let Some(source) = route.source {
            if route.source_address_check == SourceAddressCheck::Required {
                has_address(IpAddr::V6(source), route.interface)?;
            }
            request.header.source_prefix_length = full_prefix(Ipv6Addr::BITS);
            request
                .attributes
                .push(RouteAttribute::Source(RouteAddress::Inet6(source)));
        }
        request
            .attributes
            .push(RouteAttribute::Flowlabel(route.flow_label.value()));
        selectors(
            &mut request,
            route.interface,
            route.mark.0,
            owner,
            route.upper_layer,
        );
        let reply = self.exchange_route(request)?;
        let found = self.found(&reply)?;
        let link = self.link(found.interface)?;
        let source = match route.source {
            Some(source) => IpAddr::V6(source),
            None => found.source.ok_or(HostError::Errno(libc::EPROTO))?,
        };
        let conf = link.ipv6.ok_or(HostError::Errno(libc::EPROTO))?;
        Ok(Route {
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
        })
    }

    fn exchange_route(&self, request: RouteMessage) -> Result<RouteMessage, HostError> {
        match self.exchange(RouteNetlinkMessage::GetRoute(request)) {
            Ok(RouteNetlinkMessage::NewRoute(reply)) => Ok(reply),
            Ok(_) => Err(HostError::Errno(libc::EPROTO)),
            Err(HostError::Errno(libc::EOPNOTSUPP)) => {
                Err(self.refuse(Refusal::UpperLayerProtocol))
            }
            Err(error) => Err(error),
        }
    }

    fn found(&self, reply: &RouteMessage) -> Result<FoundRoute, HostError> {
        let kind = match reply.header.kind {
            RouteType::Unicast => RouteKind::Unicast,
            RouteType::Local => RouteKind::Local,
            RouteType::Broadcast => RouteKind::Broadcast,
            RouteType::Multicast => RouteKind::Multicast,
            _ => return Err(self.refuse(Refusal::RouteKind)),
        };
        let mut source = None;
        let mut interface = None;
        let mut mtu = None;
        let mut hop_limit = None;
        for attribute in &reply.attributes {
            match attribute {
                RouteAttribute::PrefSource(RouteAddress::Inet(address)) => {
                    source = Some(IpAddr::V4(*address));
                }
                RouteAttribute::PrefSource(RouteAddress::Inet6(address)) => {
                    source = Some(IpAddr::V6(*address));
                }
                RouteAttribute::Oif(index) => interface = Some(InterfaceIndex(*index)),
                RouteAttribute::Metrics(metrics) => {
                    for metric in metrics {
                        match metric {
                            RouteMetric::Mtu(value) => mtu = Some(*value),
                            RouteMetric::Hoplimit(value) => hop_limit = Some(*value),
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
            mtu,
            hop_limit,
        })
    }

    fn link(&self, interface: InterfaceIndex) -> Result<Link, HostError> {
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

    fn exchange(&self, message: RouteNetlinkMessage) -> Result<RouteNetlinkMessage, HostError> {
        let mut routing = self
            .routing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        routing.sequence = routing.sequence.wrapping_add(1);
        let sequence = routing.sequence;
        let mut header = NetlinkHeader::default();
        header.flags = NLM_F_REQUEST;
        header.sequence_number = sequence;
        let mut packet = NetlinkMessage::new(header, NetlinkPayload::from(message));
        packet.finalize();
        let mut bytes = vec![0; packet.buffer_len()];
        packet.serialize(&mut bytes);
        routing.socket.send(&bytes, 0).map_err(host_error)?;
        loop {
            let (bytes, _) = routing.socket.recv_from_full().map_err(host_error)?;
            let reply = NetlinkMessage::<RouteNetlinkMessage>::deserialize(&bytes)
                .map_err(|_| HostError::Errno(libc::EPROTO))?;
            if reply.header.sequence_number != sequence {
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

impl RouteResolver for LinuxNetlinkRouteResolver {
    fn resolve_route(
        &self,
        route: RouteSelectors<'_>,
        owner: UserId,
        neighbour: NeighbourConfirmation,
    ) -> Result<Route, HostError> {
        if neighbour == NeighbourConfirmation::Confirmed {
            return Err(self.refuse(Refusal::NeighbourConfirmation));
        }
        match &route {
            RouteSelectors::V4(route) => self.resolve_v4(route, owner),
            RouteSelectors::V6(route) => self.resolve_v6(route, owner),
        }
    }
}

struct FoundRoute {
    kind: RouteKind,
    source: Option<IpAddr>,
    interface: InterfaceIndex,
    mtu: Option<u32>,
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

/// Adds the selectors both versions share: interface, mark, owner and the upper layer.
fn selectors(
    request: &mut RouteMessage,
    interface: Option<InterfaceIndex>,
    mark: u32,
    owner: UserId,
    upper_layer: UpperLayer,
) {
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
        // The kernel's flow keeps an ICMP type and code, and a mobility header type, where the destination port's bytes are.
        Some(UpperLayerSelector::Icmp { message_type, code }) => {
            attributes.push(RouteAttribute::Dport(u16::from_be_bytes([
                message_type.0,
                code.0,
            ])));
        }
        Some(UpperLayerSelector::MobilityHeader(kind)) => {
            attributes.push(RouteAttribute::Dport(u16::from_be_bytes([kind.0, u8::MIN])));
        }
    }
}

/// The first hop of a source route among IPv4 options in the form `IP_OPTIONS` takes.
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
fn has_address(address: IpAddr, interface: Option<InterfaceIndex>) -> Result<(), HostError> {
    let address = match address {
        IpAddr::V4(address) => SockAddr::from(SocketAddrV4::new(address, ANY_PORT)),
        IpAddr::V6(address) => {
            let scope = match interface {
                Some(interface) if address.is_unicast_link_local() => interface.0,
                _ => u32::MIN,
            };
            SockAddr::from(SocketAddrV6::new(address, ANY_PORT, u32::MIN, scope))
        }
    };
    let socket = socket2::Socket::new(address.domain(), Type::DGRAM, None).map_err(host_error)?;
    socket.bind(&address).map_err(host_error)
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

fn host_error(error: io::Error) -> HostError {
    HostError::Errno(error.raw_os_error().unwrap_or(libc::EIO))
}
