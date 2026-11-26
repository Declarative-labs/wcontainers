//! The `ip` and `tc` commands the checks build their networks with, done through the kernel's routing socket.
//!
//! Each line is read as iproute2 reads it and becomes the one request `ip` or `tc` sends for it, and its
//! acknowledgement is checked; a line of a kind not done here stops the check. The one firewall table a check needs is
//! sent to the netfilter socket as the batch `nft` sends for it.

use std::net::{IpAddr, Ipv6Addr};

use netlink_packet_core::{
    DefaultNla, Emitable, NLA_F_NESTED, NLM_F_ACK, NLM_F_APPEND, NLM_F_CREATE, NLM_F_DUMP,
    NLM_F_EXCL, NLM_F_REPLACE, NLM_F_REQUEST, NetlinkHeader, NetlinkMessage, NetlinkPayload,
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
    RouteNextHop, RouteProtocol, RouteScope, RouteSeg6IpTunnel, RouteType, RouteVia, Seg6Header,
    Seg6Mode,
};
use netlink_packet_route::rule::{
    RuleAction, RuleAttribute, RuleMessage, RulePortRange, RuleUidRange,
};
use netlink_packet_route::tc::{
    TcAction, TcActionAttribute, TcActionGeneric, TcActionOption, TcActionType, TcAttribute,
    TcFilterFlower, TcFilterFlowerOption, TcHandle, TcMessage, TcOption, TcStats2,
};
use netlink_packet_route::{AddressFamily, IpProtocol, RouteNetlinkMessage};

/// The words of the `ip` and `tc` commands done here, as ip-link(8), ip-address(8), ip-route(8), ip-rule(8),
/// ip-neighbour(8), tc(8), tc-prio(8), tc-fq(8), tc-etf(8), tc-flower(8) and tc-actions(8) spell them.
mod word {
    pub(super) const ACTION: &str = "action";
    pub(super) const ADD: &str = "add";
    pub(super) const ADDRESS: &str = "address";
    pub(super) const ADDRGENMODE: &str = "addrgenmode";
    pub(super) const ALL: &str = "all";
    pub(super) const ARP: &str = "arp";
    pub(super) const BLACKHOLE: &str = "blackhole";
    pub(super) const BROADCAST: &str = "broadcast";
    pub(super) const CLOCKID: &str = "clockid";
    pub(super) const CLSACT: &str = "clsact";
    pub(super) const DEL: &str = "del";
    pub(super) const DELTA: &str = "delta";
    pub(super) const DEV: &str = "dev";
    pub(super) const DPORT: &str = "dport";
    pub(super) const DSCP: &str = "dscp";
    pub(super) const DST_MAC: &str = "dst_mac";
    pub(super) const DUMMY: &str = "dummy";
    pub(super) const EGRESS: &str = "egress";
    pub(super) const ENCAP: &str = "encap";
    pub(super) const ETF: &str = "etf";
    pub(super) const FILTER: &str = "filter";
    pub(super) const FLOWER: &str = "flower";
    pub(super) const FLOWLABEL: &str = "flowlabel";
    pub(super) const FQ: &str = "fq";
    pub(super) const FROM: &str = "from";
    pub(super) const FWMARK: &str = "fwmark";
    pub(super) const HANDLE: &str = "handle";
    pub(super) const HOPLIMIT: &str = "hoplimit";
    pub(super) const INET: &str = "inet";
    pub(super) const INET6: &str = "inet6";
    pub(super) const IPPROTO: &str = "ipproto";
    pub(super) const LINK: &str = "link";
    pub(super) const LLADDR: &str = "lladdr";
    pub(super) const LOCAL: &str = "local";
    pub(super) const LOCK: &str = "lock";
    pub(super) const LOOKUP: &str = "lookup";
    pub(super) const MAIN: &str = "main";
    pub(super) const METRIC: &str = "metric";
    pub(super) const MNGTMPADDR: &str = "mngtmpaddr";
    pub(super) const MODE: &str = "mode";
    pub(super) const MTU: &str = "mtu";
    pub(super) const MULTICAST: &str = "multicast";
    pub(super) const NAME: &str = "name";
    pub(super) const NEIGH: &str = "neigh";
    pub(super) const NEXTHOP: &str = "nexthop";
    pub(super) const NODAD: &str = "nodad";
    pub(super) const NONE: &str = "none";
    pub(super) const NUD: &str = "nud";
    pub(super) const OIF: &str = "oif";
    pub(super) const ON: &str = "on";
    pub(super) const PASS: &str = "pass";
    pub(super) const PEER: &str = "peer";
    pub(super) const PRIO: &str = "prio";
    pub(super) const PROHIBIT: &str = "prohibit";
    pub(super) const PROTOCOL: &str = "protocol";
    pub(super) const QDISC: &str = "qdisc";
    pub(super) const REPLACE: &str = "replace";
    pub(super) const ROOT: &str = "root";
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
            word::UP,
            word::TYPE,
            word::DUMMY,
        ] => {
            let (mut message, flags) = new_link(name, InfoKind::Dummy, None);
            if let RouteNetlinkMessage::NewLink(link) = &mut message {
                link.header.flags |= LinkFlags::Up;
                link.header.change_mask |= LinkFlags::Up;
            }
            Some((message, flags))
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
        [word::ROUTE, word::ADD, rest @ ..] => route(rest, false),
        [word::ROUTE, word::DEL, rest @ ..] => route(rest, true),
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

/// Does one line of `tc` commands through the routing socket, or tells that this kind is not done here.
pub(crate) fn traffic_control_done(line: &str) -> bool {
    let words: Vec<&str> = line.split_whitespace().collect();
    let request = match words.as_slice() {
        [word::QDISC, word::ADD, word::DEV, link, rest @ ..] => queueing_discipline(link, rest),
        [word::QDISC, word::DEL, word::DEV, link, word::ROOT] => {
            let mut message = TcMessage::default();
            message.header.index = tc_index(link);
            message.header.parent = TcHandle::ROOT;
            Some((RouteNetlinkMessage::DelQueueDiscipline(message), CHANGE))
        }
        [word::FILTER, word::ADD, word::DEV, link, rest @ ..] => flower_filter(link, rest),
        _ => None,
    };
    let Some((message, flags)) = request else {
        return false;
    };
    ask(message, flags, line);
    true
}

/// The bands of a prio queue made without `bands`, and the band each of the sixteen priorities goes to, as tc-prio(8)
/// gives them; the queue's options are `struct tc_prio_qopt`, an int and the sixteen bytes.
///
/// https://man7.org/linux/man-pages/man8/tc-prio.8.html
const PRIO_BANDS: i32 = 3;
const PRIO_MAP: [u8; 16] = [1, 2, 2, 2, 1, 2, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1];

/// The attribute of etf's options that holds `struct tc_etf_qopt`: the delta, the clock and the flags, each 32 bits.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/pkt_sched.h
const TCA_ETF_PARMS: u16 = 1;

/// The clock names etf's `clockid` takes, and the clocks they name.
///
/// https://man7.org/linux/man-pages/man8/tc-etf.8.html
const CLOCKS: [(&str, i32); 1] = [("CLOCK_TAI", libc::CLOCK_TAI)];

/// What ends a queueing discipline's major number in a handle such as `1:`.
///
/// https://man7.org/linux/man-pages/man8/tc.8.html
const HANDLE_SEPARATOR: char = ':';

/// Gives the request that adds a queueing discipline as `tc qdisc add dev <link> ...` does, if every word is known.
///
/// Like tc, a discipline is the root unless it is `clsact`, which takes the ingress parent and handle `ffff:`, and has
/// the handle the kernel chooses unless one is given.
fn queueing_discipline(link: &str, words: &[&str]) -> Option<(RouteNetlinkMessage, u16)> {
    let mut message = TcMessage::default();
    message.header.index = tc_index(link);
    let mut rest = words.iter();
    while let Some(word) = rest.next() {
        match *word {
            word::ROOT => message.header.parent = TcHandle::ROOT,
            word::HANDLE => {
                let major = rest.next()?.strip_suffix(HANDLE_SEPARATOR)?;
                message.header.handle = TcHandle {
                    major: u16::from_str_radix(major, HEX_RADIX).ok()?,
                    minor: u16::MIN,
                };
            }
            word::CLSACT => {
                message.header.parent = TcHandle::CLSACT;
                message.header.handle = TcHandle {
                    major: TcHandle::CLSACT.major,
                    minor: u16::MIN,
                };
                message
                    .attributes
                    .push(TcAttribute::Kind(word::CLSACT.to_owned()));
            }
            word::PRIO => {
                let mut options = PRIO_BANDS.to_ne_bytes().to_vec();
                options.extend(PRIO_MAP);
                message
                    .attributes
                    .push(TcAttribute::Kind(word::PRIO.to_owned()));
                message.attributes.push(TcAttribute::Other(DefaultNla::new(
                    linux_raw_sys::netlink::TCA_OPTIONS as u16,
                    options,
                )));
            }
            word::FQ => {
                message
                    .attributes
                    .push(TcAttribute::Kind(word::FQ.to_owned()));
                message.attributes.push(TcAttribute::Options(Vec::new()));
            }
            word::ETF => {
                let [word::CLOCKID, clock, word::DELTA, delta] =
                    [rest.next()?, rest.next()?, rest.next()?, rest.next()?].map(|word| *word)
                else {
                    return None;
                };
                let (_, clock) = CLOCKS.iter().find(|(name, _)| *name == clock)?;
                let delta: i32 = delta.parse().ok()?;
                let parameters = [delta, *clock, i32::from(u8::MIN)]
                    .iter()
                    .flat_map(|value| value.to_ne_bytes())
                    .collect();
                message
                    .attributes
                    .push(TcAttribute::Kind(word::ETF.to_owned()));
                message
                    .attributes
                    .push(TcAttribute::Options(vec![TcOption::Other(
                        DefaultNla::new(TCA_ETF_PARMS, parameters),
                    )]));
            }
            _ => return None,
        }
    }
    Some((
        RouteNetlinkMessage::NewQueueDiscipline(message),
        NLM_F_CREATE | NLM_F_EXCL,
    ))
}

/// The generic action's kind, and the attribute of its options that holds `struct tc_gact`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/tc_act/tc_gact.h
const GENERIC_ACTION: &str = "gact";
const TCA_GACT_PARMS: u16 = 2;

/// The bytes of a link-layer address.
///
/// https://www.rfc-editor.org/rfc/rfc7042#section-2.1
const MAC_LENGTH: usize = 6;

/// Gives the request that adds a filter as `tc filter add dev <link> egress protocol all flower dst_mac <mac> action
/// pass` does, the one filter the checks add.
///
/// Like tc, the filter has the priority and handle the kernel chooses, matches every protocol, masks the whole address
/// and passes what it matches through a generic action of the index the kernel chooses.
fn flower_filter(link: &str, words: &[&str]) -> Option<(RouteNetlinkMessage, u16)> {
    let [
        word::EGRESS,
        word::PROTOCOL,
        word::ALL,
        word::FLOWER,
        word::DST_MAC,
        address,
        word::ACTION,
        word::PASS,
    ] = words
    else {
        return None;
    };
    let address: [u8; MAC_LENGTH] = mac(address)?.try_into().ok()?;
    let mut generic = TcActionGeneric::default();
    generic.action = TcActionType::Ok;
    let mut parameters = vec![u8::MIN; generic.buffer_len()];
    generic.emit(&mut parameters);
    let mut action = TcAction::default();
    action.attributes = vec![
        TcActionAttribute::Kind(GENERIC_ACTION.to_owned()),
        TcActionAttribute::Options(vec![TcActionOption::Other(DefaultNla::new(
            TCA_GACT_PARMS,
            parameters,
        ))]),
    ];
    let mut message = TcMessage::default();
    message.header.index = tc_index(link);
    message.header.parent = egress();
    // The priority in the upper half, zero for the kernel's choice, and the protocol in network order in the lower.
    message.header.info = u32::from(u16::try_from(libc::ETH_P_ALL).ok()?.to_be());
    message.attributes = vec![
        TcAttribute::Kind(TcFilterFlower::KIND.to_owned()),
        TcAttribute::Options(vec![
            TcOption::Flower(TcFilterFlowerOption::EthDst(address)),
            TcOption::Flower(TcFilterFlowerOption::EthDstMask([u8::MAX; MAC_LENGTH])),
            TcOption::Flower(TcFilterFlowerOption::Actions(vec![action])),
            TcOption::Flower(TcFilterFlowerOption::Flags(u32::MIN)),
        ]),
    ];
    Some((
        RouteNetlinkMessage::NewTrafficFilter(message),
        NLM_F_CREATE | NLM_F_EXCL,
    ))
}

/// The parent of a link's egress filters under `clsact`.
fn egress() -> TcHandle {
    TcHandle {
        major: TcHandle::CLSACT.major,
        minor: TcHandle::MIN_EGRESS,
    }
}

/// Gives a link-layer address written as iproute2 writes one, bytes in hexadecimal between colons.
fn mac(text: &str) -> Option<Vec<u8>> {
    text.split(MAC_SEPARATOR)
        .map(|byte| u8::from_str_radix(byte, HEX_RADIX))
        .collect::<Result<_, _>>()
        .ok()
}

/// Gives the index of a link as a traffic control message holds it.
fn tc_index(link: &str) -> i32 {
    i32::try_from(link_index(link)).unwrap_or_else(|_| panic!("the index of {link} fits 31 bits"))
}

/// Gives the packets a class of the link's queueing discipline has sent, as `tc -s class show dev <link>` reads them.
pub(crate) fn class_packets(link: &str, class: TcHandle) -> u64 {
    let mut request = TcMessage::default();
    request.header.index = tc_index(link);
    dump(
        RouteNetlinkMessage::GetTrafficClass(request),
        "the traffic classes",
    )
    .into_iter()
    .find_map(|answer| match answer {
        RouteNetlinkMessage::NewTrafficClass(found) if found.header.handle == class => {
            basic_packets(
                found
                    .attributes
                    .iter()
                    .filter_map(|attribute| match attribute {
                        TcAttribute::Stats2(statistics) => Some(statistics.as_slice()),
                        _ => None,
                    }),
            )
        }
        _ => None,
    })
    .unwrap_or_else(|| panic!("no statistics of class {class} on {link}"))
}

/// Gives the packets the action of the link's first egress filter has taken, as `tc -s filter show dev <link> egress`
/// reads them.
pub(crate) fn egress_action_packets(link: &str) -> u64 {
    let mut request = TcMessage::default();
    request.header.index = tc_index(link);
    request.header.parent = egress();
    dump(
        RouteNetlinkMessage::GetTrafficFilter(request),
        "the egress filters",
    )
    .into_iter()
    .find_map(|answer| {
        let RouteNetlinkMessage::NewTrafficFilter(found) = answer else {
            return None;
        };
        found.attributes.iter().find_map(|attribute| {
            let TcAttribute::Options(options) = attribute else {
                return None;
            };
            options.iter().find_map(|option| {
                let TcOption::Flower(TcFilterFlowerOption::Actions(actions)) = option else {
                    return None;
                };
                basic_packets(actions.iter().flat_map(|action| {
                    action
                        .attributes
                        .iter()
                        .filter_map(|attribute| match attribute {
                            TcActionAttribute::Stats(statistics) => Some(statistics.as_slice()),
                            _ => None,
                        })
                }))
            })
        })
    })
    .unwrap_or_else(|| panic!("no egress action statistics on {link}"))
}

/// Gives the packet count of the first basic statistics among these.
fn basic_packets<'a>(mut statistics: impl Iterator<Item = &'a [TcStats2]>) -> Option<u64> {
    statistics.find_map(|list| {
        list.iter().find_map(|statistic| match statistic {
            TcStats2::Basic(basic) => Some(u64::from(basic.packets)),
            _ => None,
        })
    })
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

/// Gives a routing table's number from its name or number, as iproute2 reads one.
fn table_number(text: &str) -> Option<u32> {
    match text {
        word::MAIN => Some(MAIN_TABLE),
        word::LOCAL => Some(LOCAL_TABLE),
        number => number.parse().ok(),
    }
}

/// Gives a destination written as an address and its prefix length, or as one address alone, as iproute2 reads it.
fn destination_prefix(text: &str) -> Option<(IpAddr, u8)> {
    let (address, length) = match text.split_once(PREFIX_SEPARATOR) {
        Some((address, length)) => (address.parse().ok()?, Some(length.parse().ok()?)),
        None => (text.parse().ok()?, None),
    };
    let whole = match address {
        IpAddr::V4(_) => u32::BITS,
        IpAddr::V6(_) => u128::BITS,
    };
    Some((address, length.unwrap_or(u8::try_from(whole).ok()?)))
}

/// Gives the request that adds a route as `ip route add ...` does, or removes one as `ip route del ...` does, if every
/// word is one done here.
///
/// Like iproute2, an added route is the boot protocol's, unicast unless its type is given first, and of the link's
/// scope when it names no gateway; a removal names no protocol, and the scope only of a local route. A local route is
/// of the host's scope and the local table unless one is named; any other is of the main table unless named. Each
/// `nexthop` after the route's own words is one path of a multipath route, of the weight one.
fn route(words: &[&str], deleting: bool) -> Option<(RouteNetlinkMessage, u16)> {
    let mut message = RouteMessage::default();
    if !deleting {
        message.header.protocol = RouteProtocol::Boot;
        message.header.kind = RouteType::Unicast;
    }
    let mut paths = words.split(|word| *word == word::NEXTHOP);
    let words = paths.next()?;
    let next_hops = paths.map(next_hop).collect::<Option<Vec<_>>>()?;
    let mut gateway = !next_hops.is_empty();
    if gateway {
        message
            .attributes
            .push(RouteAttribute::MultiPath(next_hops));
    }
    let mut table = None;
    let mut metrics = Vec::new();
    let mut locked = 0;
    let mut destination = None;
    let mut rest = words.iter();
    while let Some(word) = rest.next() {
        match *word {
            word::PROHIBIT => message.header.kind = RouteType::Prohibit,
            word::BLACKHOLE => message.header.kind = RouteType::BlackHole,
            word::UNREACHABLE => message.header.kind = RouteType::Unreachable,
            word::LOCAL if destination.is_none() => message.header.kind = RouteType::Local,
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
            word::TABLE => table = Some(table_number(rest.next()?)?),
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
                let (address, length) = destination_prefix(prefix)?;
                message.header.destination_prefix_length = length;
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
    let local = message.header.kind == RouteType::Local;
    let table = table.unwrap_or(if local { LOCAL_TABLE } else { MAIN_TABLE });
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
    message.header.scope = if local {
        RouteScope::Host
    } else if deleting {
        RouteScope::NoWhere
    } else if message.header.kind == RouteType::Unicast && !gateway {
        RouteScope::Link
    } else {
        RouteScope::Universe
    };
    if deleting {
        return Some((RouteNetlinkMessage::DelRoute(message), CHANGE));
    }
    Some((
        RouteNetlinkMessage::NewRoute(message),
        NLM_F_CREATE | NLM_F_EXCL,
    ))
}

/// Gives one path of a multipath route written `via <address> dev <link>`, as iproute2 reads a `nexthop`.
fn next_hop(words: &[&str]) -> Option<RouteNextHop> {
    let [word::VIA, address, word::DEV, link] = words else {
        return None;
    };
    let mut hop = RouteNextHop::default();
    hop.interface_index = link_index(link);
    hop.attributes = vec![RouteAttribute::Gateway(route_address(
        address.parse().ok()?,
    ))];
    Some(hop)
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
/// Like iproute2, a rule is of the options' family, and its source must be too; it looks up the main table unless
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
                // iproute2 refuses a source of the other family than the command's.
                let address: IpAddr = rest.next()?.parse().ok()?;
                let (address_family, bits) = match address {
                    IpAddr::V4(_) => (AddressFamily::Inet, u32::BITS),
                    IpAddr::V6(_) => (AddressFamily::Inet6, u128::BITS),
                };
                if address_family != family {
                    return None;
                }
                message.header.src_len = u8::try_from(bits).ok()?;
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
            word::LLADDR => message
                .attributes
                .push(NeighbourAttribute::LinkLayerAddress(mac(words.next()?)?)),
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

/// The numbers of nf_tables(5)'s messages and attributes that libc does not name, as Linux defines them.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/netfilter/nf_tables.h
mod nftables {
    pub(super) const NFTA_TABLE_NAME: u16 = 1;
    pub(super) const NFTA_TABLE_FLAGS: u16 = 2;
    pub(super) const NFTA_CHAIN_TABLE: u16 = 1;
    pub(super) const NFTA_CHAIN_NAME: u16 = 3;
    pub(super) const NFTA_CHAIN_HOOK: u16 = 4;
    pub(super) const NFTA_CHAIN_TYPE: u16 = 7;
    pub(super) const NFTA_HOOK_HOOKNUM: u16 = 1;
    pub(super) const NFTA_HOOK_PRIORITY: u16 = 2;
    pub(super) const NFTA_RULE_TABLE: u16 = 1;
    pub(super) const NFTA_RULE_CHAIN: u16 = 2;
    pub(super) const NFTA_RULE_EXPRESSIONS: u16 = 4;
    pub(super) const NFTA_LIST_ELEM: u16 = 1;
    pub(super) const NFTA_EXPR_NAME: u16 = 1;
    pub(super) const NFTA_EXPR_DATA: u16 = 2;
    pub(super) const NFTA_CT_DREG: u16 = 1;
    pub(super) const NFTA_CT_KEY: u16 = 2;
    pub(super) const NFTA_BITWISE_SREG: u16 = 1;
    pub(super) const NFTA_BITWISE_DREG: u16 = 2;
    pub(super) const NFTA_BITWISE_LEN: u16 = 3;
    pub(super) const NFTA_BITWISE_MASK: u16 = 4;
    pub(super) const NFTA_BITWISE_XOR: u16 = 5;
    pub(super) const NFTA_DATA_VALUE: u16 = 1;
    pub(super) const NFTA_CMP_SREG: u16 = 1;
    pub(super) const NFTA_CMP_OP: u16 = 2;
    pub(super) const NFTA_CMP_DATA: u16 = 3;
    pub(super) const NFTA_COUNTER_BYTES: u16 = 1;
    pub(super) const NFTA_COUNTER_PACKETS: u16 = 2;
    /// The bit of a new connection in conntrack's state, `NF_CT_STATE_BIT(IP_CT_NEW)`.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/netfilter/nf_conntrack_common.h
    pub(super) const CONNECTION_NEW: u32 = 1 << 3;
}

/// The firewall table, chain and rule of the connection tracking check, as `nft` names them.
///
/// https://man7.org/linux/man-pages/man8/nft.8.html
const TRACKED_TABLE: &str = "tracked";
const TRACKED_CHAIN: &str = "output";
const FILTER_CHAIN: &str = "filter";

/// Makes the calling thread's namespace track the connections of the packets it makes, as `nft -f` loads
/// `table ip tracked { chain output { type filter hook output priority 0; ct state new counter; }; }`.
///
/// The batch is the one `nft` sends: the table, the base chain on the output hook, and the rule `[ct load state => reg
/// 1] [bitwise reg 1 = (reg 1 & new) ^ 0] [cmp neq reg 1 0] [counter]`, each acknowledged.
pub(crate) fn track_made_connections() {
    use nftables::*;
    let word = |value: u32| value.to_be_bytes().to_vec();
    let register = || word(libc::NFT_REG_1.cast_unsigned());
    let text = |value: &str| [value.as_bytes(), &[u8::MIN]].concat();
    let value =
        |kind: u16, bytes: Vec<u8>| nested(kind, &[DefaultNla::new(NFTA_DATA_VALUE, bytes)]);
    let expression = |name: &str, data: Vec<DefaultNla>| {
        DefaultNla::new(
            NFTA_LIST_ELEM | NLA_F_NESTED,
            emitted(&[
                DefaultNla::new(NFTA_EXPR_NAME, text(name)),
                nested(NFTA_EXPR_DATA, &data),
            ]),
        )
    };
    let no_state = u32::MIN.to_ne_bytes().to_vec();
    let rule = [
        DefaultNla::new(NFTA_RULE_TABLE, text(TRACKED_TABLE)),
        DefaultNla::new(NFTA_RULE_CHAIN, text(TRACKED_CHAIN)),
        nested(
            NFTA_RULE_EXPRESSIONS,
            &[
                expression(
                    "ct",
                    vec![
                        DefaultNla::new(NFTA_CT_DREG, register()),
                        DefaultNla::new(NFTA_CT_KEY, word(libc::NFT_CT_STATE.cast_unsigned())),
                    ],
                ),
                expression(
                    "bitwise",
                    vec![
                        DefaultNla::new(NFTA_BITWISE_SREG, register()),
                        DefaultNla::new(NFTA_BITWISE_DREG, register()),
                        DefaultNla::new(
                            NFTA_BITWISE_LEN,
                            word(u32::try_from(size_of::<u32>()).unwrap_or(u32::MAX)),
                        ),
                        value(NFTA_BITWISE_MASK, CONNECTION_NEW.to_ne_bytes().to_vec()),
                        value(NFTA_BITWISE_XOR, no_state.clone()),
                    ],
                ),
                expression(
                    "cmp",
                    vec![
                        DefaultNla::new(NFTA_CMP_SREG, register()),
                        DefaultNla::new(NFTA_CMP_OP, word(libc::NFT_CMP_NEQ.cast_unsigned())),
                        value(NFTA_CMP_DATA, no_state),
                    ],
                ),
                expression(
                    "counter",
                    vec![
                        DefaultNla::new(NFTA_COUNTER_BYTES, u64::MIN.to_be_bytes().to_vec()),
                        DefaultNla::new(NFTA_COUNTER_PACKETS, u64::MIN.to_be_bytes().to_vec()),
                    ],
                ),
            ],
        ),
    ];
    let messages = [
        (
            libc::NFT_MSG_NEWTABLE,
            NLM_F_CREATE,
            vec![
                DefaultNla::new(NFTA_TABLE_NAME, text(TRACKED_TABLE)),
                DefaultNla::new(NFTA_TABLE_FLAGS, word(u32::MIN)),
            ],
        ),
        (
            libc::NFT_MSG_NEWCHAIN,
            NLM_F_CREATE,
            vec![
                DefaultNla::new(NFTA_CHAIN_TABLE, text(TRACKED_TABLE)),
                DefaultNla::new(NFTA_CHAIN_NAME, text(TRACKED_CHAIN)),
                nested(
                    NFTA_CHAIN_HOOK,
                    &[
                        DefaultNla::new(
                            NFTA_HOOK_HOOKNUM,
                            word(libc::NF_INET_LOCAL_OUT.cast_unsigned()),
                        ),
                        DefaultNla::new(NFTA_HOOK_PRIORITY, word(u32::MIN)),
                    ],
                ),
                DefaultNla::new(NFTA_CHAIN_TYPE, text(FILTER_CHAIN)),
            ],
        ),
        (
            libc::NFT_MSG_NEWRULE,
            NLM_F_CREATE | NLM_F_APPEND,
            Vec::from(rule),
        ),
    ];
    let acknowledged = messages.len();
    let mut batch = netfilter_message(libc::NFNL_MSG_BATCH_BEGIN, u16::MIN, u32::MIN, &[]);
    for (sequence, (kind, flags, attributes)) in (1..).zip(messages) {
        let kind = (libc::NFNL_SUBSYS_NFTABLES << u8::BITS) | kind;
        batch.extend(netfilter_message(
            kind,
            NLM_F_ACK | flags,
            sequence,
            &attributes,
        ));
    }
    batch.extend(netfilter_message(
        libc::NFNL_MSG_BATCH_END,
        u16::MIN,
        u32::MAX,
        &[],
    ));
    let socket = netlink_sys::Socket::new(netlink_sys::protocols::NETLINK_NETFILTER)
        .expect("a netfilter socket");
    socket
        .connect(&netlink_sys::SocketAddr::new(u32::MIN, u32::MIN))
        .expect("the netfilter socket connects to the kernel");
    socket
        .send(&batch, i32::from(u8::MIN))
        .expect("the firewall batch is sent");
    let mut answered = 0;
    while answered < acknowledged {
        let mut reply = Vec::with_capacity(ANSWER_ROOM);
        socket
            .recv(&mut reply, i32::from(u8::MIN))
            .expect("the firewall batch is answered");
        let mut at = usize::MIN;
        while at < reply.len() {
            let answer = NetlinkMessage::<RouteNetlinkMessage>::deserialize(&reply[at..])
                .expect("an answer the routing message type reads, as every acknowledgement is");
            match answer.payload {
                NetlinkPayload::Error(error) if error.code.is_none() => answered += 1,
                other => panic!("nft: {other:?}"),
            }
            at += usize::try_from(answer.header.length)
                .unwrap_or(usize::MIN)
                .next_multiple_of(NETLINK_ALIGNMENT)
                .max(NETLINK_ALIGNMENT);
        }
    }
}

/// Gives an attribute holding these attributes, marked as nested as libnftnl marks its own.
fn nested(kind: u16, inner: &[DefaultNla]) -> DefaultNla {
    DefaultNla::new(kind | NLA_F_NESTED, emitted(inner))
}

/// Gives the bytes of these attributes, each padded to the netlink boundary.
fn emitted(attributes: &[DefaultNla]) -> Vec<u8> {
    let mut bytes = vec![u8::MIN; attributes.buffer_len()];
    attributes.emit(&mut bytes);
    bytes
}

/// Gives one nfnetlink message: the netlink header, `struct nfgenmsg` naming IPv4 and the nf_tables subsystem, and the
/// attributes.
fn netfilter_message(kind: i32, flags: u16, sequence: u32, attributes: &[DefaultNla]) -> Vec<u8> {
    let mut payload = vec![
        u8::try_from(libc::NFPROTO_IPV4).unwrap_or(u8::MAX),
        u8::try_from(libc::NFNETLINK_V0).unwrap_or(u8::MAX),
    ];
    payload.extend(
        u16::try_from(libc::NFNL_SUBSYS_NFTABLES)
            .unwrap_or(u16::MAX)
            .to_be_bytes(),
    );
    payload.extend(emitted(attributes));
    let mut header = NetlinkHeader::default();
    header.length = u32::try_from(header.buffer_len() + payload.len()).unwrap_or(u32::MAX);
    header.message_type = u16::try_from(kind).unwrap_or(u16::MAX);
    header.flags = NLM_F_REQUEST | flags;
    header.sequence_number = sequence;
    let mut bytes = vec![u8::MIN; header.buffer_len()];
    header.emit(&mut bytes);
    bytes.extend(payload);
    bytes
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
