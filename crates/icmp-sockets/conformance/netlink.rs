//! The `ip` commands the checks build their networks with, done through the kernel's routing socket.
//!
//! Each line is read as iproute2 reads it. A kind of line done here becomes the one request `ip` sends for it, and its
//! acknowledgement is checked; any other line is left to the iproute2 tool, a group of kinds at a time.

use std::net::{IpAddr, Ipv6Addr};

use netlink_packet_core::{
    DefaultNla, NLM_F_ACK, NLM_F_CREATE, NLM_F_EXCL, NLM_F_REPLACE, NLM_F_REQUEST, NetlinkHeader,
    NetlinkMessage, NetlinkPayload,
};
use netlink_packet_route::address::{
    AddressAttribute, AddressFlags, AddressHeaderFlags, AddressMessage, AddressScope,
};
use netlink_packet_route::link::{
    AfSpecInet6, AfSpecUnspec, In6AddrGenMode, InfoData, InfoKind, InfoVeth, LinkAttribute,
    LinkFlags, LinkInfo, LinkMessage,
};
use netlink_packet_route::neighbour::{
    NeighbourAddress, NeighbourAttribute, NeighbourMessage, NeighbourState,
};
use netlink_packet_route::route::{
    RouteAddress, RouteAttribute, RouteLwEnCapType, RouteLwTunnelEncap, RouteMessage, RouteMetric,
    RouteProtocol, RouteScope, RouteSeg6IpTunnel, RouteType, RouteVia, Seg6Header, Seg6Mode,
};
use netlink_packet_route::rule::{
    RuleAction, RuleAttribute, RuleMessage, RulePortRange, RuleUidRange,
};
use netlink_packet_route::{AddressFamily, IpProtocol, RouteNetlinkMessage};

/// The first byte of every IPv4 loopback address, to which `ip` gives the host's scope.
///
/// https://www.rfc-editor.org/rfc/rfc1122#section-3.2.1.3
const LOOPBACK_NETWORK: u8 = 127;

/// Room for the kernel's answer to one request: an acknowledgement, or an error that quotes the request.
///
/// https://man7.org/linux/man-pages/man7/netlink.7.html
const ANSWER_ROOM: usize = 8192;

/// The flags of a request that changes only what is there.
///
/// https://man7.org/linux/man-pages/man7/netlink.7.html
const CHANGE: u16 = 0;

/// The `ip` option that makes a command without an address IPv6's; without it a rule is IPv4's.
///
/// https://man7.org/linux/man-pages/man8/ip.8.html
const IPV6_OPTION: &str = "-6";

/// Does one line of `ip` commands, given with these options, through the routing socket, or tells that this kind is not done here.
pub fn done(line: &str, options: &[&str]) -> bool {
    let family = if options.contains(&IPV6_OPTION) {
        AddressFamily::Inet6
    } else {
        AddressFamily::Inet
    };
    let words: Vec<&str> = line.split_whitespace().collect();
    let request = match words.as_slice() {
        ["link", "add", name, "type", "dummy"] => Some(new_link(name, InfoKind::Dummy, None)),
        ["link", "add", name, "type", "veth", "peer", "name", peer] => {
            let mut other = LinkMessage::default();
            other
                .attributes
                .push(LinkAttribute::IfName((*peer).to_owned()));
            Some(new_link(
                name,
                InfoKind::Veth,
                Some(InfoData::Veth(InfoVeth::Peer(other))),
            ))
        }
        ["link", "set", name, settings @ ..] => set_link(name, settings),
        ["address", verb @ ("add" | "replace"), prefix, rest @ ..] => address(verb, prefix, rest),
        ["route", "add", rest @ ..] => route(rest),
        ["rule", "add", rest @ ..] => rule(family, rest),
        ["neigh", "add", address, rest @ ..] => neighbour(address, rest),
        _ => None,
    };
    let Some((message, flags)) = request else {
        return false;
    };
    ask(message, flags, line);
    true
}

/// Gives the request that adds a link of the kind, as `ip link add <name> type <kind>` sends it.
fn new_link(name: &str, kind: InfoKind, data: Option<InfoData>) -> (RouteNetlinkMessage, u16) {
    let mut link = LinkMessage::default();
    let mut information = vec![LinkInfo::Kind(kind)];
    information.extend(data.map(LinkInfo::Data));
    link.attributes = vec![
        LinkAttribute::IfName(name.to_owned()),
        LinkAttribute::LinkInfo(information),
    ];
    (
        RouteNetlinkMessage::NewLink(link),
        NLM_F_CREATE | NLM_F_EXCL,
    )
}

/// Gives the request that changes a link as `ip link set <name> ...` does, if every setting is one done here.
fn set_link(name: &str, settings: &[&str]) -> Option<(RouteNetlinkMessage, u16)> {
    let mut link = LinkMessage::default();
    link.header.index = link_index(name);
    let mut rest = settings.iter();
    while let Some(setting) = rest.next() {
        match *setting {
            "up" => {
                link.header.flags |= LinkFlags::Up;
                link.header.change_mask |= LinkFlags::Up;
            }
            "mtu" => link
                .attributes
                .push(LinkAttribute::Mtu(rest.next()?.parse().ok()?)),
            "arp" if rest.next() == Some(&"on") => link.header.change_mask |= LinkFlags::Noarp,
            "multicast" if rest.next() == Some(&"on") => {
                link.header.flags |= LinkFlags::Multicast;
                link.header.change_mask |= LinkFlags::Multicast;
            }
            "addrgenmode" if rest.next() == Some(&"none") => {
                link.attributes
                    .push(LinkAttribute::AfSpecUnspec(vec![AfSpecUnspec::Inet6(
                        vec![AfSpecInet6::AddrGenMode(In6AddrGenMode::None)],
                    )]));
            }
            _ => return None,
        }
    }
    Some((RouteNetlinkMessage::SetLink(link), CHANGE))
}

/// Gives the request that gives a link an address as `ip address add|replace <prefix> ...` does, if every word is known.
fn address(verb: &str, prefix: &str, rest: &[&str]) -> Option<(RouteNetlinkMessage, u16)> {
    let (address, length) = prefix.split_once('/')?;
    let address: IpAddr = address.parse().ok()?;
    let mut message = AddressMessage::default();
    message.header.prefix_len = length.parse().ok()?;
    message.header.family = match address {
        IpAddr::V4(_) => AddressFamily::Inet,
        IpAddr::V6(_) => AddressFamily::Inet6,
    };
    // iproute2 gives only an IPv4 loopback address a scope of its own; Linux chooses each IPv6 address's.
    message.header.scope = match address {
        IpAddr::V4(v4) if v4.octets()[0] == LOOPBACK_NETWORK => AddressScope::Host,
        _ => AddressScope::Universe,
    };
    message.attributes = vec![
        AddressAttribute::Local(address),
        AddressAttribute::Address(address),
    ];
    let mut flags = AddressFlags::empty();
    let mut words = rest.iter();
    while let Some(word) = words.next() {
        match *word {
            "dev" => message.header.index = link_index(words.next()?),
            "broadcast" => message
                .attributes
                .push(AddressAttribute::Broadcast(words.next()?.parse().ok()?)),
            "nodad" => flags |= AddressFlags::Nodad,
            "mngtmpaddr" => flags |= AddressFlags::Managetempaddr,
            _ => return None,
        }
    }
    // iproute2 puts flags that fit the header's byte there, and any others in IFA_FLAGS alone.
    match u8::try_from(flags.bits()) {
        Ok(byte) => message.header.flags = AddressHeaderFlags::from_bits_retain(byte),
        Err(_) => message.attributes.push(AddressAttribute::Flags(flags)),
    }
    let creation = if verb == "replace" {
        NLM_F_CREATE | NLM_F_REPLACE
    } else {
        NLM_F_CREATE | NLM_F_EXCL
    };
    Some((RouteNetlinkMessage::NewAddress(message), creation))
}

/// The routing table `ip route add` uses when it is given none.
///
/// https://man7.org/linux/man-pages/man8/ip-route.8.html
const MAIN_TABLE: u32 = 254;

/// Gives the request that adds a route as `ip route add ...` does, if every word is one done here.
///
/// Like iproute2, the route is the boot protocol's, of the main table unless named, unicast unless its type is given
/// first, and of the link's scope when it names no gateway.
fn route(words: &[&str]) -> Option<(RouteNetlinkMessage, u16)> {
    let mut message = RouteMessage::default();
    message.header.protocol = RouteProtocol::Boot;
    message.header.kind = RouteType::Unicast;
    let mut table = MAIN_TABLE;
    let mut gateway = false;
    let mut metrics = Vec::new();
    let mut locked = 0;
    let mut destination = None;
    let mut rest = words.iter();
    while let Some(word) = rest.next() {
        match *word {
            "prohibit" => message.header.kind = RouteType::Prohibit,
            "blackhole" => message.header.kind = RouteType::BlackHole,
            "unreachable" => message.header.kind = RouteType::Unreachable,
            "via" => {
                gateway = true;
                let mut next = *rest.next()?;
                if next == "inet6" {
                    let address: Ipv6Addr = rest.next()?.parse().ok()?;
                    message
                        .attributes
                        .push(RouteAttribute::Via(RouteVia::Inet6(address)));
                    continue;
                }
                if next == "inet" {
                    next = rest.next()?;
                }
                let address: IpAddr = next.parse().ok()?;
                message
                    .attributes
                    .push(RouteAttribute::Gateway(route_address(address)));
            }
            "dev" => message
                .attributes
                .push(RouteAttribute::Oif(link_index(rest.next()?))),
            "table" => table = rest.next()?.parse().ok()?,
            "metric" => message
                .attributes
                .push(RouteAttribute::Priority(rest.next()?.parse().ok()?)),
            "mtu" => {
                let mut value = *rest.next()?;
                if value == "lock" {
                    locked |= 1 << linux_raw_sys::netlink::RTAX_MTU as u32;
                    value = rest.next()?;
                }
                metrics.push(RouteMetric::Mtu(value.parse().ok()?));
            }
            "hoplimit" => metrics.push(RouteMetric::Hoplimit(rest.next()?.parse().ok()?)),
            "encap" => {
                // `encap seg6 mode encap segs <address>[,<address>...]`, the one encapsulation the checks use.
                let ["seg6", "mode", "encap", "segs", segments] = [
                    rest.next()?,
                    rest.next()?,
                    rest.next()?,
                    rest.next()?,
                    rest.next()?,
                ]
                .map(|word| *word) else {
                    return None;
                };
                let segments: Vec<Ipv6Addr> = segments
                    .split(',')
                    .map(str::parse)
                    .collect::<Result<_, _>>()
                    .ok()?;
                let mut header = Seg6Header::default();
                header.mode = Seg6Mode::Encap;
                header.segments = segments;
                message
                    .attributes
                    .push(RouteAttribute::EncapType(RouteLwEnCapType::Seg6));
                message
                    .attributes
                    .push(RouteAttribute::Encap(vec![RouteLwTunnelEncap::Seg6(
                        RouteSeg6IpTunnel::Seg6(header),
                    )]));
            }
            prefix if destination.is_none() => {
                let (address, length) = prefix.split_once('/')?;
                let address: IpAddr = address.parse().ok()?;
                message.header.destination_prefix_length = length.parse().ok()?;
                destination = Some(address);
            }
            _ => return None,
        }
    }
    let destination = destination?;
    message.header.address_family = match destination {
        IpAddr::V4(_) => AddressFamily::Inet,
        IpAddr::V6(_) => AddressFamily::Inet6,
    };
    message
        .attributes
        .insert(0, RouteAttribute::Destination(route_address(destination)));
    // A table past the header's byte goes in RTA_TABLE, the header naming none.
    match u8::try_from(table) {
        Ok(byte) => message.header.table = byte,
        Err(_) => message.attributes.push(RouteAttribute::Table(table)),
    }
    if locked != 0 {
        metrics.push(RouteMetric::Lock(locked));
    }
    if !metrics.is_empty() {
        message.attributes.push(RouteAttribute::Metrics(metrics));
    }
    message.header.scope = if message.header.kind == RouteType::Unicast && !gateway {
        RouteScope::Link
    } else {
        RouteScope::Universe
    };
    Some((
        RouteNetlinkMessage::NewRoute(message),
        NLM_F_CREATE | NLM_F_EXCL,
    ))
}

/// Gives an address as a route attribute holds it.
fn route_address(address: IpAddr) -> RouteAddress {
    match address {
        IpAddr::V4(v4) => RouteAddress::Inet(v4),
        IpAddr::V6(v6) => RouteAddress::Inet6(v6),
    }
}

/// The protocol names a rule's `ipproto` may give, and their numbers.
///
/// https://www.iana.org/assignments/protocol-numbers/protocol-numbers.xhtml
const PROTOCOLS: [(&str, i32); 4] = [
    ("icmp", libc::IPPROTO_ICMP),
    ("ipv6-icmp", libc::IPPROTO_ICMPV6),
    ("udp", libc::IPPROTO_UDP),
    ("gre", libc::IPPROTO_GRE),
];

/// The mask iproute2 gives a rule's flow label when none is written, the label's twenty bits.
///
/// https://www.rfc-editor.org/rfc/rfc6437
const FLOW_LABEL_BITS: u32 = 0xf_ffff;

/// Gives the request that adds a rule as `ip rule add ...` does, if every word is one done here.
///
/// Like iproute2, a rule is of the options' family unless its source names another, looks up the main table unless
/// it names one or is a type such as `prohibit`, and takes a port or user range of one when given one number.
fn rule(family: AddressFamily, words: &[&str]) -> Option<(RouteNetlinkMessage, u16)> {
    let mut message = RuleMessage::default();
    message.header.family = family;
    message.header.action = RuleAction::ToTable;
    let mut table = Some(MAIN_TABLE);
    let mut rest = words.iter();
    while let Some(word) = rest.next() {
        match *word {
            "fwmark" => message
                .attributes
                .push(RuleAttribute::FwMark(number(rest.next()?)?)),
            "uidrange" => {
                let (start, end) = rest.next()?.split_once('-')?;
                message
                    .attributes
                    .push(RuleAttribute::UidRange(RuleUidRange {
                        start: start.parse().ok()?,
                        end: end.parse().ok()?,
                    }));
            }
            "ipproto" => {
                let name = *rest.next()?;
                let protocol = match PROTOCOLS.iter().find(|(known, _)| *known == name) {
                    Some((_, value)) => u8::try_from(*value).ok()?,
                    None => name.parse().ok()?,
                };
                message
                    .attributes
                    .push(RuleAttribute::IpProtocol(IpProtocol::from(protocol)));
            }
            port @ ("dport" | "sport") => {
                let value: u16 = rest.next()?.parse().ok()?;
                let range = RulePortRange {
                    start: value,
                    end: value,
                };
                message.attributes.push(if port == "dport" {
                    RuleAttribute::DestinationPortRange(range)
                } else {
                    RuleAttribute::SourcePortRange(range)
                });
            }
            "from" => {
                let address: IpAddr = rest.next()?.parse().ok()?;
                (message.header.family, message.header.src_len) = match address {
                    IpAddr::V4(_) => (AddressFamily::Inet, u8::try_from(u32::BITS).ok()?),
                    IpAddr::V6(_) => (AddressFamily::Inet6, u8::try_from(u128::BITS).ok()?),
                };
                message.attributes.push(RuleAttribute::Source(address));
            }
            "oif" => message
                .attributes
                .push(RuleAttribute::Oifname((*rest.next()?).to_owned())),
            "dscp" => {
                let value: u8 = rest.next()?.parse().ok()?;
                message
                    .attributes
                    .push(RuleAttribute::Other(DefaultNla::new(
                        linux_raw_sys::netlink::FRA_DSCP as u16,
                        vec![value],
                    )));
            }
            "flowlabel" => {
                let label = number(rest.next()?)?;
                for (kind, value) in [
                    (linux_raw_sys::netlink::FRA_FLOWLABEL, label),
                    (linux_raw_sys::netlink::FRA_FLOWLABEL_MASK, FLOW_LABEL_BITS),
                ] {
                    message
                        .attributes
                        .push(RuleAttribute::Other(DefaultNla::new(
                            kind as u16,
                            value.to_be_bytes().to_vec(),
                        )));
                }
            }
            "lookup" | "table" => table = Some(rest.next()?.parse().ok()?),
            "prohibit" => {
                message.header.action = RuleAction::Prohibit;
                table = None;
            }
            _ => return None,
        }
    }
    // A table past the header's byte goes in FRA_TABLE, the header naming none; a type names no table.
    if let Some(table) = table {
        match u8::try_from(table) {
            Ok(byte) => message.header.table = byte,
            Err(_) => message.attributes.push(RuleAttribute::Table(table)),
        }
    }
    Some((
        RouteNetlinkMessage::NewRule(message),
        NLM_F_CREATE | NLM_F_EXCL,
    ))
}

/// Gives a number written in decimal, or in hexadecimal after `0x`, as iproute2 reads one.
fn number(text: &str) -> Option<u32> {
    match text.strip_prefix("0x") {
        Some(hexadecimal) => u32::from_str_radix(hexadecimal, 16).ok(),
        None => text.parse().ok(),
    }
}

/// Gives the request that adds a neighbour entry as `ip neigh add <address> lladdr <mac> dev <link> [nud <state>]` does.
///
/// Like iproute2, the entry is permanent unless a state is given.
fn neighbour(address: &str, rest: &[&str]) -> Option<(RouteNetlinkMessage, u16)> {
    let address: IpAddr = address.parse().ok()?;
    let mut message = NeighbourMessage::default();
    message.header.state = NeighbourState::Permanent;
    (message.header.family, message.attributes) = match address {
        IpAddr::V4(v4) => (
            AddressFamily::Inet,
            vec![NeighbourAttribute::Destination(NeighbourAddress::Inet(v4))],
        ),
        IpAddr::V6(v6) => (
            AddressFamily::Inet6,
            vec![NeighbourAttribute::Destination(NeighbourAddress::Inet6(v6))],
        ),
    };
    let mut words = rest.iter();
    while let Some(word) = words.next() {
        match *word {
            "lladdr" => {
                let bytes: Vec<u8> = words
                    .next()?
                    .split(':')
                    .map(|byte| u8::from_str_radix(byte, 16))
                    .collect::<Result<_, _>>()
                    .ok()?;
                message
                    .attributes
                    .push(NeighbourAttribute::LinkLayerAddress(bytes));
            }
            "dev" => message.header.ifindex = link_index(words.next()?),
            "nud" if words.next() == Some(&"stale") => {
                message.header.state = NeighbourState::Stale;
            }
            _ => return None,
        }
    }
    Some((
        RouteNetlinkMessage::NewNeighbour(message),
        NLM_F_CREATE | NLM_F_EXCL,
    ))
}

/// Gives the index of a link of the calling thread's namespace, or stops the check when there is none.
fn link_index(name: &str) -> u32 {
    nix::net::if_::if_nametoindex(name).unwrap_or_else(|error| panic!("no link {name}: {error}"))
}

/// Sends one request to the routing socket of the calling thread's namespace and stops the check unless it is done.
fn ask(message: RouteNetlinkMessage, flags: u16, line: &str) {
    let mut header = NetlinkHeader::default();
    header.flags = NLM_F_REQUEST | NLM_F_ACK | flags;
    let mut request = NetlinkMessage::new(header, NetlinkPayload::from(message));
    request.finalize();
    let mut bytes = vec![u8::MIN; request.buffer_len()];
    request.serialize(&mut bytes);
    let socket =
        netlink_sys::Socket::new(netlink_sys::protocols::NETLINK_ROUTE).expect("a routing socket");
    // The kernel's port number and no multicast groups (netlink(7)).
    socket
        .connect(&netlink_sys::SocketAddr::new(u32::MIN, u32::MIN))
        .expect("the routing socket connects to the kernel");
    socket
        .send(&bytes, i32::from(u8::MIN))
        .unwrap_or_else(|error| panic!("{line}: {error}"));
    let mut reply = Vec::with_capacity(ANSWER_ROOM);
    socket
        .recv(&mut reply, i32::from(u8::MIN))
        .unwrap_or_else(|error| panic!("{line}: {error}"));
    match NetlinkMessage::<RouteNetlinkMessage>::deserialize(&reply).map(|answer| answer.payload) {
        Ok(NetlinkPayload::Error(error)) if error.code.is_none() => {}
        other => panic!("ip {line}: {other:?}"),
    }
}
