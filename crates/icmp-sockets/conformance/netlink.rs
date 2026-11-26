//! The `ip` commands the checks build their networks with, done through the kernel's routing socket.
//!
//! Each line is read as iproute2 reads it. A kind of line done here becomes the one request `ip` sends for it, and its
//! acknowledgement is checked; any other line is left to the iproute2 tool, a group of kinds at a time.

use std::net::{IpAddr, Ipv6Addr};

use netlink_packet_core::{
    DefaultNla, NLM_F_ACK, NLM_F_CREATE, NLM_F_DUMP, NLM_F_EXCL, NLM_F_REPLACE, NLM_F_REQUEST,
    NetlinkHeader, NetlinkMessage, NetlinkPayload,
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

/// The words of the `ip` commands done here, as ip-link(8), ip-address(8), ip-route(8), ip-rule(8) and ip-neighbour(8)
/// spell them.
mod word {
    pub(super) const ADD: &str = "add";
    pub(super) const ADDRESS: &str = "address";
    pub(super) const ADDRGENMODE: &str = "addrgenmode";
    pub(super) const ARP: &str = "arp";
    pub(super) const BLACKHOLE: &str = "blackhole";
    pub(super) const BROADCAST: &str = "broadcast";
    pub(super) const DEV: &str = "dev";
    pub(super) const DPORT: &str = "dport";
    pub(super) const DSCP: &str = "dscp";
    pub(super) const DUMMY: &str = "dummy";
    pub(super) const ENCAP: &str = "encap";
    pub(super) const FLOWLABEL: &str = "flowlabel";
    pub(super) const FROM: &str = "from";
    pub(super) const FWMARK: &str = "fwmark";
    pub(super) const HOPLIMIT: &str = "hoplimit";
    pub(super) const INET: &str = "inet";
    pub(super) const INET6: &str = "inet6";
    pub(super) const IPPROTO: &str = "ipproto";
    pub(super) const LINK: &str = "link";
    pub(super) const LLADDR: &str = "lladdr";
    pub(super) const LOCK: &str = "lock";
    pub(super) const LOOKUP: &str = "lookup";
    pub(super) const METRIC: &str = "metric";
    pub(super) const MNGTMPADDR: &str = "mngtmpaddr";
    pub(super) const MODE: &str = "mode";
    pub(super) const MTU: &str = "mtu";
    pub(super) const MULTICAST: &str = "multicast";
    pub(super) const NAME: &str = "name";
    pub(super) const NEIGH: &str = "neigh";
    pub(super) const NODAD: &str = "nodad";
    pub(super) const NONE: &str = "none";
    pub(super) const NUD: &str = "nud";
    pub(super) const OIF: &str = "oif";
    pub(super) const ON: &str = "on";
    pub(super) const PEER: &str = "peer";
    pub(super) const PROHIBIT: &str = "prohibit";
    pub(super) const REPLACE: &str = "replace";
    pub(super) const ROUTE: &str = "route";
    pub(super) const RULE: &str = "rule";
    pub(super) const SEG6: &str = "seg6";
    pub(super) const SEGS: &str = "segs";
    pub(super) const SET: &str = "set";
    pub(super) const SPORT: &str = "sport";
    pub(super) const STALE: &str = "stale";
    pub(super) const TABLE: &str = "table";
    pub(super) const TYPE: &str = "type";
    pub(super) const UIDRANGE: &str = "uidrange";
    pub(super) const UNREACHABLE: &str = "unreachable";
    pub(super) const UP: &str = "up";
    pub(super) const VETH: &str = "veth";
    pub(super) const VIA: &str = "via";
}

/// What iproute2 writes between an address and its prefix length, between segments, between a range's ends, between a
/// link address's bytes, and before a hexadecimal number.
///
/// https://man7.org/linux/man-pages/man8/ip.8.html
const PREFIX_SEPARATOR: char = '/';
const SEGMENT_SEPARATOR: char = ',';
const RANGE_SEPARATOR: char = '-';
const MAC_SEPARATOR: char = ':';
const HEX_PREFIX: &str = "0x";
/// The base of the hexadecimal numbers iproute2 reads.
///
/// https://man7.org/linux/man-pages/man8/ip.8.html
const HEX_RADIX: u32 = 16;

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
pub(crate) fn done(line: &str, options: &[&str]) -> bool {
    let family = if options.contains(&IPV6_OPTION) {
        AddressFamily::Inet6
    } else {
        AddressFamily::Inet
    };
    let words: Vec<&str> = line.split_whitespace().collect();
    let request = match words.as_slice() {
        [word::LINK, word::ADD, name, word::TYPE, word::DUMMY] => {
            Some(new_link(name, InfoKind::Dummy, None))
        }
        [
            word::LINK,
            word::ADD,
            name,
            word::TYPE,
            word::VETH,
            word::PEER,
            word::NAME,
            peer,
        ] => {
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
        [word::LINK, word::SET, name, settings @ ..] => set_link(name, settings),
        [
            word::ADDRESS,
            verb @ (word::ADD | word::REPLACE),
            prefix,
            rest @ ..,
        ] => address(verb, prefix, rest),
        [word::ROUTE, word::ADD, rest @ ..] => route(rest),
        [word::RULE, word::ADD, rest @ ..] => rule(family, rest),
        [word::NEIGH, word::ADD, address, rest @ ..] => neighbour(address, rest),
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
            word::UP => {
                link.header.flags |= LinkFlags::Up;
                link.header.change_mask |= LinkFlags::Up;
            }
            word::MTU => link
                .attributes
                .push(LinkAttribute::Mtu(rest.next()?.parse().ok()?)),
            word::ARP if rest.next() == Some(&word::ON) => {
                link.header.change_mask |= LinkFlags::Noarp
            }
            word::MULTICAST if rest.next() == Some(&word::ON) => {
                link.header.flags |= LinkFlags::Multicast;
                link.header.change_mask |= LinkFlags::Multicast;
            }
            word::ADDRGENMODE if rest.next() == Some(&word::NONE) => {
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
    let (address, length) = prefix.split_once(PREFIX_SEPARATOR)?;
    let address: IpAddr = address.parse().ok()?;
    let mut message = AddressMessage::default();
    message.header.prefix_len = length.parse().ok()?;
    message.header.family = match address {
        IpAddr::V4(_) => AddressFamily::Inet,
        IpAddr::V6(_) => AddressFamily::Inet6,
    };
    // iproute2 gives only an IPv4 loopback address a scope of its own; Linux chooses each IPv6 address's.
    message.header.scope = match address {
        IpAddr::V4(v4) if v4.is_loopback() => AddressScope::Host,
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
            word::DEV => message.header.index = link_index(words.next()?),
            word::BROADCAST => message
                .attributes
                .push(AddressAttribute::Broadcast(words.next()?.parse().ok()?)),
            word::NODAD => flags |= AddressFlags::Nodad,
            word::MNGTMPADDR => flags |= AddressFlags::Managetempaddr,
            _ => return None,
        }
    }
    // iproute2 puts flags that fit the header's byte there, and any others in IFA_FLAGS alone.
    match u8::try_from(flags.bits()) {
        Ok(byte) => message.header.flags = AddressHeaderFlags::from_bits_retain(byte),
        Err(_) => message.attributes.push(AddressAttribute::Flags(flags)),
    }
    let creation = if verb == word::REPLACE {
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
            word::PROHIBIT => message.header.kind = RouteType::Prohibit,
            word::BLACKHOLE => message.header.kind = RouteType::BlackHole,
            word::UNREACHABLE => message.header.kind = RouteType::Unreachable,
            word::VIA => {
                gateway = true;
                let mut next = *rest.next()?;
                if next == word::INET6 {
                    let address: Ipv6Addr = rest.next()?.parse().ok()?;
                    message
                        .attributes
                        .push(RouteAttribute::Via(RouteVia::Inet6(address)));
                    continue;
                }
                if next == word::INET {
                    next = rest.next()?;
                }
                let address: IpAddr = next.parse().ok()?;
                message
                    .attributes
                    .push(RouteAttribute::Gateway(route_address(address)));
            }
            word::DEV => message
                .attributes
                .push(RouteAttribute::Oif(link_index(rest.next()?))),
            word::TABLE => table = rest.next()?.parse().ok()?,
            word::METRIC => message
                .attributes
                .push(RouteAttribute::Priority(rest.next()?.parse().ok()?)),
            word::MTU => {
                let mut value = *rest.next()?;
                if value == word::LOCK {
                    locked |= 1 << linux_raw_sys::netlink::RTAX_MTU as u32;
                    value = rest.next()?;
                }
                metrics.push(RouteMetric::Mtu(value.parse().ok()?));
            }
            word::HOPLIMIT => metrics.push(RouteMetric::Hoplimit(rest.next()?.parse().ok()?)),
            word::ENCAP => {
                // `encap seg6 mode encap segs <address>[,<address>...]`, the one encapsulation the checks use.
                let [word::SEG6, word::MODE, word::ENCAP, word::SEGS, segments] = [
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
                    .split(SEGMENT_SEPARATOR)
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
                let (address, length) = prefix.split_once(PREFIX_SEPARATOR)?;
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
    if locked != u32::MIN {
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
            word::FWMARK => message
                .attributes
                .push(RuleAttribute::FwMark(number(rest.next()?)?)),
            word::UIDRANGE => {
                let (start, end) = rest.next()?.split_once(RANGE_SEPARATOR)?;
                message
                    .attributes
                    .push(RuleAttribute::UidRange(RuleUidRange {
                        start: start.parse().ok()?,
                        end: end.parse().ok()?,
                    }));
            }
            word::IPPROTO => {
                let name = *rest.next()?;
                let protocol = match PROTOCOLS.iter().find(|(known, _)| *known == name) {
                    Some((_, value)) => u8::try_from(*value).ok()?,
                    None => name.parse().ok()?,
                };
                message
                    .attributes
                    .push(RuleAttribute::IpProtocol(IpProtocol::from(protocol)));
            }
            port @ (word::DPORT | word::SPORT) => {
                let value: u16 = rest.next()?.parse().ok()?;
                let range = RulePortRange {
                    start: value,
                    end: value,
                };
                message.attributes.push(if port == word::DPORT {
                    RuleAttribute::DestinationPortRange(range)
                } else {
                    RuleAttribute::SourcePortRange(range)
                });
            }
            word::FROM => {
                let address: IpAddr = rest.next()?.parse().ok()?;
                (message.header.family, message.header.src_len) = match address {
                    IpAddr::V4(_) => (AddressFamily::Inet, u8::try_from(u32::BITS).ok()?),
                    IpAddr::V6(_) => (AddressFamily::Inet6, u8::try_from(u128::BITS).ok()?),
                };
                message.attributes.push(RuleAttribute::Source(address));
            }
            word::OIF => message
                .attributes
                .push(RuleAttribute::Oifname((*rest.next()?).to_owned())),
            word::DSCP => {
                let value: u8 = rest.next()?.parse().ok()?;
                message
                    .attributes
                    .push(RuleAttribute::Other(DefaultNla::new(
                        linux_raw_sys::netlink::FRA_DSCP as u16,
                        vec![value],
                    )));
            }
            word::FLOWLABEL => {
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
            word::LOOKUP | word::TABLE => table = Some(rest.next()?.parse().ok()?),
            word::PROHIBIT => {
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
    match text.strip_prefix(HEX_PREFIX) {
        Some(hexadecimal) => u32::from_str_radix(hexadecimal, HEX_RADIX).ok(),
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
            word::LLADDR => {
                let bytes: Vec<u8> = words
                    .next()?
                    .split(MAC_SEPARATOR)
                    .map(|byte| u8::from_str_radix(byte, HEX_RADIX))
                    .collect::<Result<_, _>>()
                    .ok()?;
                message
                    .attributes
                    .push(NeighbourAttribute::LinkLayerAddress(bytes));
            }
            word::DEV => message.header.ifindex = link_index(words.next()?),
            word::NUD if words.next() == Some(&word::STALE) => {
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
    let socket = sent(message, NLM_F_ACK | flags, line);
    let mut reply = Vec::with_capacity(ANSWER_ROOM);
    socket
        .recv(&mut reply, i32::from(u8::MIN))
        .unwrap_or_else(|error| panic!("{line}: {error}"));
    match NetlinkMessage::<RouteNetlinkMessage>::deserialize(&reply).map(|answer| answer.payload) {
        Ok(NetlinkPayload::Error(error)) if error.code.is_none() => {}
        other => panic!("ip {line}: {other:?}"),
    }
}

/// Sends one request with these flags to a new routing socket of the calling thread's namespace, and gives the socket.
fn sent(message: RouteNetlinkMessage, flags: u16, what: &str) -> netlink_sys::Socket {
    let mut header = NetlinkHeader::default();
    header.flags = NLM_F_REQUEST | flags;
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
        .unwrap_or_else(|error| panic!("{what}: {error}"));
    socket
}

/// Room for one part of a dump, which the kernel fills with whole messages.
///
/// https://man7.org/linux/man-pages/man7/netlink.7.html
const DUMP_ROOM: usize = 1 << 16;

/// The boundary every netlink message starts on.
///
/// https://man7.org/linux/man-pages/man7/netlink.7.html
const NETLINK_ALIGNMENT: usize = 4;

/// Gives every message the kernel answers a dump request with, as `ip ... show` reads them.
fn dump(message: RouteNetlinkMessage, what: &str) -> Vec<RouteNetlinkMessage> {
    let socket = sent(message, NLM_F_DUMP, what);
    let mut answers = Vec::new();
    loop {
        let mut reply = Vec::with_capacity(DUMP_ROOM);
        socket
            .recv(&mut reply, i32::from(u8::MIN))
            .unwrap_or_else(|error| panic!("{what}: {error}"));
        let mut at = usize::MIN;
        while at < reply.len() {
            let answer = NetlinkMessage::<RouteNetlinkMessage>::deserialize(&reply[at..])
                .unwrap_or_else(|error| panic!("{what}: {error}"));
            let length = usize::try_from(answer.header.length).unwrap_or(usize::MIN);
            match answer.payload {
                NetlinkPayload::Done(_) => return answers,
                NetlinkPayload::InnerMessage(inner) => answers.push(inner),
                NetlinkPayload::Error(error) => panic!("{what}: {error:?}"),
                _ => {}
            }
            assert!(length > usize::MIN, "{what}: a message of no length");
            at += length.next_multiple_of(NETLINK_ALIGNMENT);
        }
    }
}

/// The local routing table, where Linux keeps a route to each address of the namespace's own.
///
/// https://man7.org/linux/man-pages/man8/ip-route.8.html
const LOCAL_TABLE: u32 = 255;

/// Tells whether the local table routes this one address, as `ip -6 route show table local <address>` lists it.
pub(crate) fn locally_routed(address: Ipv6Addr) -> bool {
    let mut request = RouteMessage::default();
    request.header.address_family = AddressFamily::Inet6;
    dump(RouteNetlinkMessage::GetRoute(request), "the IPv6 routes")
        .into_iter()
        .any(|answer| {
            let RouteNetlinkMessage::NewRoute(route) = answer else {
                return false;
            };
            let table = route
                .attributes
                .iter()
                .find_map(|attribute| match attribute {
                    RouteAttribute::Table(table) => Some(*table),
                    _ => None,
                })
                .unwrap_or_else(|| u32::from(route.header.table));
            table == LOCAL_TABLE
                && u32::from(route.header.destination_prefix_length) == u128::BITS
                && route
                    .attributes
                    .contains(&RouteAttribute::Destination(RouteAddress::Inet6(address)))
        })
}

/// Gives how many clock ticks ago the link's neighbour entry for the address was confirmed, as `ip -s neigh show` reads it.
pub(crate) fn confirmed_ticks(address: IpAddr, link: &str) -> Option<u32> {
    let index = link_index(link);
    let destination = match address {
        IpAddr::V4(v4) => NeighbourAddress::Inet(v4),
        IpAddr::V6(v6) => NeighbourAddress::Inet6(v6),
    };
    dump(
        RouteNetlinkMessage::GetNeighbour(NeighbourMessage::default()),
        "the neighbours",
    )
    .into_iter()
    .find_map(|answer| {
        let RouteNetlinkMessage::NewNeighbour(entry) = answer else {
            return None;
        };
        if entry.header.ifindex != index
            || !entry
                .attributes
                .contains(&NeighbourAttribute::Destination(destination.clone()))
        {
            return None;
        }
        entry
            .attributes
            .iter()
            .find_map(|attribute| match attribute {
                NeighbourAttribute::CacheInfo(information) => Some(information.confirmed),
                _ => None,
            })
    })
}

/// Gives the link's temporary IPv6 address once it is no longer tentative, as `ip -6 address show temporary -tentative` does.
pub(crate) fn temporary_address(link: &str) -> Option<Ipv6Addr> {
    let index = link_index(link);
    let mut request = AddressMessage::default();
    request.header.family = AddressFamily::Inet6;
    dump(
        RouteNetlinkMessage::GetAddress(request),
        "the IPv6 addresses",
    )
    .into_iter()
    .find_map(|answer| {
        let RouteNetlinkMessage::NewAddress(address) = answer else {
            return None;
        };
        // Linux names a temporary address by IFA_F_SECONDARY, and gives all flags in IFA_FLAGS when it sends them.
        let flags = address
            .attributes
            .iter()
            .find_map(|attribute| match attribute {
                AddressAttribute::Flags(flags) => Some(*flags),
                _ => None,
            })
            .unwrap_or_else(|| {
                AddressFlags::from_bits_retain(u32::from(address.header.flags.bits()))
            });
        if address.header.index != index
            || !flags.contains(AddressFlags::Secondary)
            || flags.contains(AddressFlags::Tentative)
        {
            return None;
        }
        address
            .attributes
            .iter()
            .find_map(|attribute| match attribute {
                AddressAttribute::Address(IpAddr::V6(v6)) => Some(*v6),
                _ => None,
            })
    })
}

/// Gives a link's link-layer address, as `ip -br link show <link>` lists it.
pub(crate) fn link_address(link: &str) -> Vec<u8> {
    let index = link_index(link);
    dump(
        RouteNetlinkMessage::GetLink(LinkMessage::default()),
        "the links",
    )
    .into_iter()
    .find_map(|answer| {
        let RouteNetlinkMessage::NewLink(found) = answer else {
            return None;
        };
        if found.header.index != index {
            return None;
        }
        found
            .attributes
            .into_iter()
            .find_map(|attribute| match attribute {
                LinkAttribute::Address(bytes) => Some(bytes),
                _ => None,
            })
    })
    .unwrap_or_else(|| panic!("no link address on {link}"))
}
