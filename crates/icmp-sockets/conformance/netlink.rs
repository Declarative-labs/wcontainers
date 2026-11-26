//! The `ip` commands the checks build their networks with, done through the kernel's routing socket.
//!
//! Each line is read as iproute2 reads it. A kind of line done here becomes the one request `ip` sends for it, and its
//! acknowledgement is checked; any other line is left to the iproute2 tool, a group of kinds at a time.

use std::net::IpAddr;

use netlink_packet_core::{
    NLM_F_ACK, NLM_F_CREATE, NLM_F_EXCL, NLM_F_REPLACE, NLM_F_REQUEST, NetlinkHeader,
    NetlinkMessage, NetlinkPayload,
};
use netlink_packet_route::address::{
    AddressAttribute, AddressFlags, AddressHeaderFlags, AddressMessage, AddressScope,
};
use netlink_packet_route::link::{
    AfSpecInet6, AfSpecUnspec, In6AddrGenMode, InfoData, InfoKind, InfoVeth, LinkAttribute,
    LinkFlags, LinkInfo, LinkMessage,
};
use netlink_packet_route::{AddressFamily, RouteNetlinkMessage};

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

/// Does one line of `ip` commands through the routing socket, or tells that this kind is not done here.
pub fn done(line: &str) -> bool {
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
