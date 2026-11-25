//! Netlink sockets (`net/netlink/af_netlink.c`) for NETLINK_ROUTE (`net/core/rtnetlink.c`): the
//! questions programs ask about the container's interfaces, addresses and routes, answered from
//! the made-up network in [`crate::network`]. Nothing in that network changes, so no notices are
//! ever sent to the groups a socket joins.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use super::{Packet, Received, Socket, SocketRef, gather, scatter, wait_room};
use crate::linux::general::*;
use crate::linux::if_arp::{ARPHRD_ETHER, ARPHRD_LOOPBACK};
use crate::linux::net::net_device_flags::{
    IFF_BROADCAST, IFF_LOOPBACK, IFF_LOWER_UP, IFF_MULTICAST, IFF_RUNNING, IFF_UP,
};
use crate::linux::net::{
    AF_INET, AF_INET6, AF_NETLINK, AF_UNSPEC, IF_OPER_UNKNOWN, IF_OPER_UP, MSG_OOB, MSG_PEEK,
    MSG_TRUNC, SOCK_DGRAM, SOCK_RAW, SOL_NETLINK,
};
use crate::linux::netlink::rtattr_type_t::*;
use crate::linux::netlink::*;
use crate::network;
use crate::process::Tid;
use crate::{Context, Errno, Kernel};

/// An IPv6 device that is up (`IF_READY`, `net/if_inet6.h`, not in uapi).
const IF_READY: u32 = 0x8000_0000;
/// `sizeof(struct sockaddr_nl)`.
const ADDRESS_SIZE: usize = 12;
/// `sizeof(struct nlmsghdr)`.
const HEADER: usize = 16;

/// A netlink socket's own state.
pub(crate) struct Netlink {
    /// Its port number (`portid`), 0 until bound.
    pub portid: u32,
    /// The groups it joined.
    pub groups: u32,
    /// Options at SOL_NETLINK, by number.
    pub flags: u32,
    /// The parts of a running dump not yet queued (`cb_running`): each comes once a read leaves
    /// room (`netlink_dump`).
    pub dump: VecDeque<Vec<u8>>,
}

/// `netlink_create`: NETLINK_ROUTE only.
pub(super) fn create<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    kind: u32,
    protocol: u32,
) -> Result<Socket, Errno> {
    if kind != SOCK_RAW && kind != SOCK_DGRAM {
        return Err(Errno::ESOCKTNOSUPPORT);
    }
    if protocol >= MAX_LINKS {
        return Err(Errno::EPROTONOSUPPORT);
    }
    if protocol != NETLINK_ROUTE {
        let detail = crate::counters::socket_detail(AF_NETLINK, kind, protocol);
        return Err(kernel.counters.refuse(
            cx.host,
            crate::reason::PROTOCOL_NOT_BUILT,
            detail,
            Errno::EPROTONOSUPPORT,
        ));
    }
    let mut socket = Socket::new(kernel.next_object(), AF_NETLINK, kind, protocol);
    socket.netlink = Some(alloc::boxed::Box::new(Netlink {
        portid: 0,
        groups: 0,
        flags: 0,
        dump: VecDeque::new(),
    }));
    Ok(socket)
}

fn part(socket: &SocketRef) -> atomic_refcell::AtomicRefMut<'_, Netlink> {
    atomic_refcell::AtomicRefMut::map(socket.borrow_mut(), |own| {
        own.netlink.as_deref_mut().expect("a netlink socket")
    })
}

/// `netlink_autobind`: the process's number if no socket holds it, else a negative one.
fn autobind(kernel: &mut Kernel, tid: Tid, socket: &SocketRef) {
    if part(socket).portid != 0 {
        return;
    }
    let pid = kernel.threads[&tid].process as u32;
    let mut candidate = pid;
    let mut rover = -4096i32;
    while kernel.sockets.portid_taken(candidate) {
        rover -= 1;
        candidate = rover as u32;
    }
    kernel.sockets.hold_portid(candidate, socket);
    part(socket).portid = candidate;
}

/// A `sockaddr_nl`.
fn address(portid: u32, groups: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(ADDRESS_SIZE);
    bytes.extend_from_slice(&(AF_NETLINK as u16).to_le_bytes());
    bytes.extend_from_slice(&[0; 2]);
    bytes.extend_from_slice(&portid.to_le_bytes());
    bytes.extend_from_slice(&groups.to_le_bytes());
    bytes
}

fn read_address(bytes: &[u8]) -> Result<(u32, u32), Errno> {
    if bytes.len() < ADDRESS_SIZE || u16::from_le_bytes([bytes[0], bytes[1]]) as u32 != AF_NETLINK {
        return Err(Errno::EINVAL);
    }
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
    Ok((word(4), word(8)))
}

pub(super) fn bind(
    kernel: &mut Kernel,
    tid: Tid,
    socket: &SocketRef,
    bytes: &[u8],
) -> Result<u64, Errno> {
    let (portid, groups) = read_address(bytes)?;
    let bound = part(socket).portid;
    if bound != 0 && portid != 0 && portid != bound {
        return Err(Errno::EINVAL);
    }
    if bound == 0 {
        if portid == 0 {
            autobind(kernel, tid, socket);
        } else {
            if kernel.sockets.portid_taken(portid) {
                return Err(Errno::EADDRINUSE);
            }
            kernel.sockets.hold_portid(portid, socket);
            part(socket).portid = portid;
        }
    }
    part(socket).groups = groups;
    Ok(0)
}

pub(super) fn connect<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    bytes: &[u8],
) -> Result<u64, Errno> {
    if bytes.len() >= 2 && u16::from_le_bytes([bytes[0], bytes[1]]) as u32 == AF_UNSPEC {
        return Ok(0);
    }
    let (portid, _) = read_address(bytes)?;
    if portid != 0 {
        return Err(kernel.counters.refuse(
            cx.host,
            crate::reason::PROTOCOL_NOT_BUILT,
            u64::from(portid),
            Errno::ECONNREFUSED,
        ));
    }
    autobind(kernel, tid, socket);
    Ok(0)
}

pub(super) fn name(socket: &SocketRef, peer: bool) -> Vec<u8> {
    let own = socket.borrow();
    let netlink = own.netlink.as_deref().expect("a netlink socket");
    if peer {
        address(0, 0)
    } else {
        address(netlink.portid, netlink.groups)
    }
}

/// `datagram_poll`: always writable here.
pub(super) fn poll(socket: &SocketRef) -> u16 {
    let own = socket.borrow();
    let mut mask = EPOLLOUT | EPOLLWRNORM | EPOLLWRBAND;
    if !own.queue.is_empty() {
        mask |= EPOLLIN | EPOLLRDNORM;
    }
    if own.error != 0 {
        mask |= EPOLLERR;
    }
    mask as u16
}

pub(super) fn released(kernel: &mut Kernel, socket: &SocketRef) {
    let portid = part(socket).portid;
    if portid != 0 {
        kernel.sockets.let_go_portid(portid, socket);
    }
    let mut own = socket.borrow_mut();
    own.dead = true;
    own.queue.clear();
}

/// `netlink_sendmsg` to the kernel: every request in the bytes is answered into the socket's own
/// queue, as `netlink_rcv_skb` and `rtnetlink_rcv_msg` answer.
pub(super) fn send<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    parts: super::Data<'_>,
    to: Option<&[u8]>,
    total: u64,
) -> Result<u64, Errno> {
    if let Some(bytes) = to.filter(|bytes| !bytes.is_empty()) {
        let (portid, groups) = read_address(bytes)?;
        if portid != 0 || groups != 0 {
            return Err(kernel.counters.refuse(
                cx.host,
                crate::reason::PROTOCOL_NOT_BUILT,
                u64::from(portid),
                Errno::ECONNREFUSED,
            ));
        }
    }
    if total > socket.borrow().sndbuf.saturating_sub(32) {
        return Err(Errno::EMSGSIZE);
    }
    autobind(kernel, tid, socket);
    let data = gather(kernel, cx, tid, parts, 0, total)?;
    let (portid, flags) = {
        let netlink = part(socket);
        (netlink.portid, netlink.flags)
    };
    let mut at = 0;
    let mut replies: Vec<Vec<u8>> = Vec::new();
    // The last message's alignment may reach past the end (`netlink_rcv_skb` pulls what is left).
    while data.len().saturating_sub(at) >= HEADER {
        let request = &data[at..];
        let length = u32::from_le_bytes(request[..4].try_into().expect("4 bytes")) as usize;
        if length < HEADER || length > request.len() {
            break;
        }
        let request = &request[..length];
        let kind = u16::from_le_bytes([request[4], request[5]]);
        let request_flags = u16::from_le_bytes([request[6], request[7]]) as u32;
        let mut error = 0;
        if request_flags & NLM_F_REQUEST != 0 && u32::from(kind) >= NLMSG_MIN_TYPE {
            let uid = kernel.credentials(tid).uid;
            match answer(kernel, cx, request, portid, flags, uid) {
                // `__netlink_dump_start`: one dump at a time; its first part comes at once.
                Ok(Answer::Dump(_)) if !part(socket).dump.is_empty() => {
                    error = -(Errno::EBUSY.0 as i32)
                }
                Ok(Answer::Dump(messages)) => {
                    let mut messages = VecDeque::from(messages);
                    replies.extend(messages.pop_front());
                    part(socket).dump = messages;
                    at += length.next_multiple_of(NLMSG_ALIGNTO as usize);
                    continue;
                }
                Ok(Answer::One(message)) => replies.push(message),
                Ok(Answer::Nothing) => {}
                Err(errno) => error = -(errno.0 as i32),
            }
        }
        if request_flags & NLM_F_ACK != 0 || error != 0 {
            replies.push(acknowledgement(request, error, portid, flags));
        }
        at += length.next_multiple_of(NLMSG_ALIGNTO as usize);
    }
    for reply in replies {
        queue(kernel, cx, tid, socket, reply);
    }
    Ok(total)
}

/// `netlink_sendskb`: a message for the socket's reader, who hears of it.
fn queue<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    reply: Vec<u8>,
) {
    let packet = Packet {
        truesize: super::truesize(reply.len() as u64, 0),
        data: reply,
        consumed: 0,
        files: Vec::new(),
        cred: super::own_cred(kernel, tid),
        sender: alloc::sync::Weak::new(),
        source: None,
        arrival: None,
        time: None,
        urgent: false,
        owner: 0,
    };
    socket.borrow_mut().queue.push_back(packet);
    super::data_ready(kernel, cx, socket, EPOLLIN | EPOLLRDNORM);
}

/// `netlink_sendskb` of a message the kernel made for no request (a message queue's
/// SIGEV_THREAD cookie): from port 0 with empty credentials.
pub(crate) fn send_from_kernel<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    data: Vec<u8>,
) {
    let packet = Packet {
        truesize: super::truesize(data.len() as u64, 0),
        data,
        consumed: 0,
        files: Vec::new(),
        cred: super::Cred {
            pid: 0,
            uid: 0,
            gid: 0,
        },
        sender: alloc::sync::Weak::new(),
        source: None,
        arrival: None,
        time: None,
        urgent: false,
        owner: 0,
    };
    socket.borrow_mut().queue.push_back(packet);
    super::data_ready(kernel, cx, socket, EPOLLIN | EPOLLRDNORM);
}

/// What a request gets: a dump (its parts, then NLMSG_DONE, each one read), one message, or
/// nothing but an acknowledgement if asked for.
enum Answer {
    Dump(Vec<Vec<u8>>),
    One(Vec<u8>),
    Nothing,
}

/// `netlink_ack`: NLMSG_ERROR with the error (0 for an acknowledgement) and the request's header,
/// and for an error its whole body unless NETLINK_CAP_ACK is set.
fn acknowledgement(request: &[u8], error: i32, portid: u32, flags: u32) -> Vec<u8> {
    let capped = error == 0 || flags & (1 << NETLINK_CAP_ACK) != 0;
    let body = if capped { &request[..HEADER] } else { request };
    let mut message = Message::new(
        NLMSG_ERROR as u16,
        if capped { NLM_F_CAPPED as u16 } else { 0 },
        request,
        portid,
    );
    message.push(&error.to_le_bytes());
    message.push(body);
    message.finish()
}

/// One netlink message being built.
struct Message(Vec<u8>);

impl Message {
    /// A message of a kind answering `request`, with its sequence number, to `portid`.
    fn new(kind: u16, flags: u16, request: &[u8], portid: u32) -> Self {
        let mut bytes = Vec::with_capacity(256);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&flags.to_le_bytes());
        bytes.extend_from_slice(&request[8..12]);
        bytes.extend_from_slice(&portid.to_le_bytes());
        Self(bytes)
    }

    fn push(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
        self.0
            .resize(self.0.len().next_multiple_of(NLMSG_ALIGNTO as usize), 0);
    }

    /// An attribute (`struct rtattr`).
    fn attr(&mut self, kind: u32, value: &[u8]) {
        let length = 4 + value.len();
        self.0.extend_from_slice(&(length as u16).to_le_bytes());
        self.0.extend_from_slice(&(kind as u16).to_le_bytes());
        self.push(value);
    }

    fn u32(&mut self, kind: u32, value: u32) {
        self.attr(kind, &value.to_le_bytes());
    }

    fn u8(&mut self, kind: u32, value: u8) {
        self.attr(kind, &[value]);
    }

    fn text(&mut self, kind: u32, value: &str) {
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0);
        self.attr(kind, &bytes);
    }

    fn finish(mut self) -> Vec<u8> {
        let length = self.0.len() as u32;
        self.0[..4].copy_from_slice(&length.to_le_bytes());
        self.0
    }
}

/// `rtnetlink_rcv_msg`: GET requests are answered; anything that would change the network needs
/// CAP_NET_ADMIN, which Docker does not give.
fn answer<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    request: &[u8],
    portid: u32,
    flags: u32,
    uid: u32,
) -> Result<Answer, Errno> {
    let kind = u32::from(u16::from_le_bytes([request[4], request[5]]));
    let request_flags = u32::from(u16::from_le_bytes([request[6], request[7]]));
    if kind >= __RTM_MAX as u32 {
        return Err(Errno::EOPNOTSUPP);
    }
    let body = &request[HEADER..];
    if body.is_empty() {
        return Ok(Answer::Nothing);
    }
    let family = u32::from(body[0]);
    let get = (kind - RTM_BASE as u32) & 3 == 2;
    if !get {
        return Err(Errno::EPERM);
    }
    let strict = flags & (1 << NETLINK_GET_STRICT_CHK) != 0;
    let dump = request_flags & NLM_F_DUMP == NLM_F_DUMP;
    let messages = match kind {
        _ if kind == RTM_GETLINK as u32 => {
            if !dump {
                return one_link(request, portid, body);
            }
            if family == AF_INET6 {
                // IPv6's own answer (`inet6_dump_ifinfo`).
                INTERFACES
                    .iter()
                    .map(|interface| link6(interface, request, portid))
                    .collect::<Vec<_>>()
            } else {
                kernel
                    .counters
                    .count(cx.host, crate::reason::INTERFACE_COUNTERS_NOT_KEPT, 0);
                INTERFACES
                    .iter()
                    .map(|interface| link(interface, request, portid, NLM_F_MULTI as u16))
                    .collect::<Vec<_>>()
            }
        }
        _ if kind == RTM_GETADDR as u32 && dump => {
            let index = (strict && body.len() >= 8)
                .then(|| u32::from_le_bytes(body[4..8].try_into().expect("4 bytes")))
                .filter(|&index| index != 0);
            addresses(family, index, request, portid)
        }
        _ if kind == RTM_GETROUTE as u32 && dump => route_table()
            .iter()
            .filter(|route| family == AF_UNSPEC || family == route.family)
            .map(|route| route_message(route, request, portid, NLM_F_MULTI as u16))
            .collect(),
        _ if kind == RTM_GETROUTE as u32 => {
            return route_get(kernel, family, body, request, portid, uid, strict);
        }
        _ if kind == RTM_GETRULE as u32 && dump => rules(family, request, portid),
        // Rules are only dumped (`fib_nl_dumprule` has no single form).
        _ if kind == RTM_GETRULE as u32 => return Err(Errno::EOPNOTSUPP),
        _ if kind == RTM_GETNEIGH as u32 && dump => {
            neighbours(kernel, cx, family, strict, body, request, portid)
        }
        _ => {
            return Err(kernel.counters.refuse(
                cx.host,
                crate::reason::NETLINK_REQUEST_NOT_BUILT,
                u64::from(kind) | (u64::from(dump) << u16::BITS),
                Errno::EOPNOTSUPP,
            ));
        }
    };
    // The answers fill one read. NLMSG_DONE comes in the next for links, addresses and routes
    // (`RTNL_FLAG_DUMP_SPLIT_NLM_DONE`), in the same one for neighbours and rules
    // (`netlink_dump`).
    let mut joined = Vec::new();
    for message in messages {
        joined.extend_from_slice(&message);
    }
    let mut done = Message::new(NLMSG_DONE as u16, NLM_F_MULTI as u16, request, portid);
    done.push(&0i32.to_le_bytes());
    let mut parts = Vec::new();
    if kind == RTM_GETNEIGH as u32 || kind == RTM_GETRULE as u32 {
        joined.extend_from_slice(&done.finish());
        parts.push(joined);
        return Ok(Answer::Dump(parts));
    }
    if !joined.is_empty() {
        parts.push(joined);
    }
    parts.push(done.finish());
    Ok(Answer::Dump(parts))
}

/// An interface of the made-up network.
pub(super) struct Interface {
    pub index: u32,
    pub name: &'static str,
    pub kind: u32,
    pub flags: u32,
    pub mtu: u32,
    min_mtu: u32,
    max_mtu: u32,
    pub queue_length: u32,
    operstate: u8,
    pub hardware: [u8; 6],
    broadcast: [u8; 6],
    carrier_changes: u32,
    /// Its IPv4 address, prefix length and broadcast address.
    pub v4: ([u8; 4], u8, [u8; 4]),
}

pub(super) const INTERFACES: [Interface; 2] = [
    Interface {
        index: network::LOOPBACK_INDEX,
        name: network::LOOPBACK_NAME,
        kind: ARPHRD_LOOPBACK,
        flags: IFF_UP as u32 | IFF_LOOPBACK as u32 | IFF_RUNNING as u32 | IFF_LOWER_UP as u32,
        mtu: network::LOOPBACK_MTU,
        min_mtu: 0,
        max_mtu: 0,
        queue_length: 1000,
        operstate: IF_OPER_UNKNOWN as u8,
        hardware: [0; 6],
        broadcast: [0; 6],
        carrier_changes: 0,
        v4: ([127, 0, 0, 1], 8, [0; 4]),
    },
    Interface {
        index: network::ETHERNET_INDEX,
        name: network::ETHERNET_NAME,
        kind: ARPHRD_ETHER,
        flags: IFF_UP as u32
            | IFF_BROADCAST as u32
            | IFF_RUNNING as u32
            | IFF_MULTICAST as u32
            | IFF_LOWER_UP as u32,
        mtu: network::MTU,
        min_mtu: 68,
        max_mtu: 65535,
        queue_length: 0,
        operstate: IF_OPER_UP as u8,
        hardware: network::HARDWARE,
        broadcast: [0xff; 6],
        carrier_changes: 2,
        v4: (network::ADDRESS, network::PREFIX, network::BROADCAST),
    },
];

/// `rtnl_fill_ifinfo`, with the attributes programs read. The traffic counters are not kept and
/// read zero.
fn link(interface: &Interface, request: &[u8], portid: u32, flags: u16) -> Vec<u8> {
    let mut message = Message::new(RTM_NEWLINK as u16, flags, request, portid);
    let mut head = Vec::with_capacity(16);
    head.extend_from_slice(&[0, 0]);
    head.extend_from_slice(&(interface.kind as u16).to_le_bytes());
    head.extend_from_slice(&interface.index.to_le_bytes());
    head.extend_from_slice(&interface.flags.to_le_bytes());
    head.extend_from_slice(&0u32.to_le_bytes());
    message.push(&head);
    message.text(IFLA_IFNAME as u32, interface.name);
    message.u32(IFLA_TXQLEN as u32, interface.queue_length);
    message.u8(IFLA_OPERSTATE as u32, interface.operstate);
    message.u8(IFLA_LINKMODE as u32, 0);
    message.u32(IFLA_MTU as u32, interface.mtu);
    message.u32(IFLA_MIN_MTU as u32, interface.min_mtu);
    message.u32(IFLA_MAX_MTU as u32, interface.max_mtu);
    message.u32(IFLA_GROUP as u32, 0);
    message.u32(IFLA_PROMISCUITY as u32, 0);
    message.u32(IFLA_ALLMULTI as u32, 0);
    message.u32(IFLA_NUM_TX_QUEUES as u32, 1);
    message.u32(IFLA_GSO_MAX_SEGS as u32, 65535);
    message.u32(IFLA_GSO_MAX_SIZE as u32, 65536);
    message.u32(IFLA_NUM_RX_QUEUES as u32, 1);
    message.u8(IFLA_CARRIER as u32, 1);
    message.u32(IFLA_CARRIER_CHANGES as u32, interface.carrier_changes);
    message.u8(IFLA_PROTO_DOWN as u32, 0);
    message.attr(IFLA_ADDRESS as u32, &interface.hardware);
    message.attr(IFLA_BROADCAST as u32, &interface.broadcast);
    message.attr(IFLA_STATS64 as u32, &[0; 200]);
    message.attr(IFLA_STATS as u32, &[0; 96]);
    message.text(IFLA_QDISC as u32, "noqueue");
    message.attr(IFLA_MAP as u32, &[0; 32]);
    message.finish()
}

/// `inet6_fill_ifinfo`: an interface as IPv6 sees it, with its IPv6 flags and times.
fn link6(interface: &Interface, request: &[u8], portid: u32) -> Vec<u8> {
    let mut message = Message::new(RTM_NEWLINK as u16, NLM_F_MULTI as u16, request, portid);
    let mut head = Vec::with_capacity(16);
    head.extend_from_slice(&[AF_INET6 as u8, 0]);
    head.extend_from_slice(&(interface.kind as u16).to_le_bytes());
    head.extend_from_slice(&interface.index.to_le_bytes());
    head.extend_from_slice(&interface.flags.to_le_bytes());
    head.extend_from_slice(&0u32.to_le_bytes());
    message.push(&head);
    message.text(IFLA_IFNAME as u32, interface.name);
    message.attr(IFLA_ADDRESS as u32, &interface.hardware);
    message.u32(IFLA_MTU as u32, interface.mtu);
    message.u8(IFLA_OPERSTATE as u32, interface.operstate);
    // IFLA_PROTINFO with the device's flags (IF_READY) and its reassembly size and times.
    let mut protinfo = Message(Vec::new());
    protinfo.u32(IFLA_INET6_FLAGS as u32, IF_READY);
    let mut cache = Vec::with_capacity(16);
    for value in [65535u32, 0, 30000, 1000] {
        cache.extend_from_slice(&value.to_le_bytes());
    }
    protinfo.attr(IFLA_INET6_CACHEINFO as u32, &cache);
    message.attr(IFLA_PROTINFO as u32, &protinfo.0);
    message.finish()
}

/// RTM_GETLINK for one interface, by number or by IFLA_IFNAME (`rtnl_getlink`).
fn one_link(request: &[u8], portid: u32, body: &[u8]) -> Result<Answer, Errno> {
    if body.len() < 16 {
        return Err(Errno::EINVAL);
    }
    let index = i32::from_le_bytes(body[4..8].try_into().expect("4 bytes"));
    let mut name: Option<&[u8]> = None;
    let mut at = 16;
    while at + 4 <= body.len() {
        let length = u16::from_le_bytes([body[at], body[at + 1]]) as usize;
        let kind = u16::from_le_bytes([body[at + 2], body[at + 3]]);
        if length < 4 || at + length > body.len() {
            break;
        }
        if u32::from(kind) == IFLA_IFNAME as u32 {
            let value = &body[at + 4..at + length];
            name = Some(&value[..value.iter().position(|&b| b == 0).unwrap_or(value.len())]);
        }
        at += length.next_multiple_of(NLMSG_ALIGNTO as usize);
    }
    let found = INTERFACES.iter().find(|interface| {
        if index > 0 {
            interface.index == index as u32
        } else {
            name.is_some_and(|name| name == interface.name.as_bytes())
        }
    });
    if index <= 0 && name.is_none() {
        return Err(Errno::EINVAL);
    }
    let interface = found.ok_or(Errno::ENODEV)?;
    Ok(Answer::One(link(interface, request, portid, 0)))
}

/// One address message (`inet_fill_ifaddr`, `inet6_fill_ifaddr`).
#[allow(clippy::too_many_arguments, reason = "an address has many parts")]
fn address_message(
    request: &[u8],
    portid: u32,
    family: u32,
    prefix: u8,
    scope: u8,
    index: u32,
    address: &[u8],
    broadcast: Option<[u8; 4]>,
    label: Option<&str>,
) -> Vec<u8> {
    let mut message = Message::new(RTM_NEWADDR as u16, NLM_F_MULTI as u16, request, portid);
    let mut head = Vec::with_capacity(8);
    head.push(family as u8);
    head.push(prefix);
    head.push(IFA_F_PERMANENT as u8);
    head.push(scope);
    head.extend_from_slice(&index.to_le_bytes());
    message.push(&head);
    let mut cache = Vec::with_capacity(16);
    cache.extend_from_slice(&u32::MAX.to_le_bytes());
    cache.extend_from_slice(&u32::MAX.to_le_bytes());
    cache.extend_from_slice(&[0; 8]);
    if family == AF_INET {
        message.attr(IFA_ADDRESS as u32, address);
        message.attr(IFA_LOCAL as u32, address);
        if let Some(broadcast) = broadcast {
            message.attr(IFA_BROADCAST as u32, &broadcast);
        }
        message.text(IFA_LABEL as u32, label.unwrap_or(""));
        message.u32(IFA_FLAGS as u32, IFA_F_PERMANENT);
        message.attr(IFA_CACHEINFO as u32, &cache);
    } else {
        message.attr(IFA_ADDRESS as u32, address);
        message.attr(IFA_CACHEINFO as u32, &cache);
        message.u32(IFA_FLAGS as u32, IFA_F_PERMANENT);
        message.u8(IFA_PROTO as u32, IFAPROT_KERNEL_LO as u8);
    }
    message.finish()
}

fn addresses(family: u32, index: Option<u32>, request: &[u8], portid: u32) -> Vec<Vec<u8>> {
    let host = rt_scope_t::RT_SCOPE_HOST as u8;
    let universe = rt_scope_t::RT_SCOPE_UNIVERSE as u8;
    let lo = network::LOOPBACK_INDEX;
    let eth = network::ETHERNET_INDEX;
    let mut out = Vec::new();
    let wanted = |kind: u32, at: u32| {
        (family == AF_UNSPEC || family == kind) && index.is_none_or(|index| index == at)
    };
    if wanted(AF_INET, lo) {
        out.push(address_message(
            request,
            portid,
            AF_INET,
            8,
            host,
            lo,
            &[127, 0, 0, 1],
            None,
            Some(network::LOOPBACK_NAME),
        ));
    }
    if wanted(AF_INET, eth) {
        out.push(address_message(
            request,
            portid,
            AF_INET,
            network::PREFIX,
            universe,
            eth,
            &network::ADDRESS,
            Some(network::BROADCAST),
            Some(network::ETHERNET_NAME),
        ));
    }
    if wanted(AF_INET6, lo) {
        let mut loopback = [0u8; 16];
        loopback[15] = 1;
        out.push(address_message(
            request, portid, AF_INET6, 128, host, lo, &loopback, None, None,
        ));
    }
    out
}

/// The neighbour table's dump (`neigh_dump_info`): IPv4's entries, then IPv6's. A request for
/// proxies (NTF_PROXY) has none; under strict checking an interface named filters.
fn neighbours<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    family: u32,
    strict: bool,
    body: &[u8],
    request: &[u8],
    portid: u32,
) -> Vec<Vec<u8>> {
    let field = |at: usize, size: usize| body.get(at..at + size);
    let index = field(4, 4).filter(|_| strict).map_or(0, |bytes| {
        u32::from_le_bytes(bytes.try_into().expect("4 bytes"))
    });
    let proxies = field(10, 1).is_some_and(|flags| u32::from(flags[0]) & NTF_PROXY != 0);
    if kernel.sockets.inet.neighbours.ethernet && !proxies && family != AF_INET6 {
        kernel
            .counters
            .count(cx.host, crate::reason::NEIGHBOURS_NOT_BUILT, 0);
    }
    let table = &kernel.sockets.inet.neighbours;
    if proxies || (index != 0 && index != network::LOOPBACK_INDEX) {
        return Vec::new();
    }
    let now = kernel.entry_now;
    // Times in clock ticks of 100 a second (`jiffies_to_clock_t`).
    let ticks =
        |since: u64| (now.saturating_sub(since) / crate::linux_extra::NANOSECONDS_PER_TICK) as u32;
    let mut out = Vec::new();
    for (slot, (kind, kind_of)) in [(AF_INET, RTN_BROADCAST), (AF_INET6, RTN_LOCAL)]
        .into_iter()
        .enumerate()
    {
        let Some((made, confirmed)) = table.loopback[slot] else {
            continue;
        };
        if family != AF_UNSPEC && family != kind {
            continue;
        }
        let mut message = Message::new(RTM_NEWNEIGH as u16, NLM_F_MULTI as u16, request, portid);
        // `struct ndmsg`: family, padding, interface, state, flags, type.
        let mut header = alloc::vec![kind as u8, 0, 0, 0];
        header.extend_from_slice(&network::LOOPBACK_INDEX.to_le_bytes());
        header.extend_from_slice(&(NUD_NOARP as u16).to_le_bytes());
        header.extend_from_slice(&[0, kind_of as u8]);
        message.push(&header);
        let mut address = alloc::vec![0u8; if kind == AF_INET { 4 } else { 16 }];
        if kind == AF_INET6 {
            address[15] = 1;
        }
        message.attr(NDA_DST as u32, &address);
        message.attr(NDA_LLADDR as u32, &[0; 6]);
        message.u32(NDA_PROBES as u32, 0);
        // `struct nda_cacheinfo`: since confirmed (a new entry counts as confirmed twice the
        // base reachable time, 30 s, before it was made), used and updated, and its references
        // past the table's own.
        let confirmed = confirmed.map_or(ticks(made) + 6000, ticks);
        let mut times = Vec::new();
        for value in [confirmed, ticks(made), ticks(made), 0] {
            times.extend_from_slice(&value.to_le_bytes());
        }
        message.attr(NDA_CACHEINFO as u32, &times);
        out.push(message.finish());
    }
    out
}

/// One route (`fib_dump_info`, `rt6_fill_node`).
struct Route {
    family: u32,
    destination: [u8; 16],
    prefix: u8,
    table: u8,
    protocol: u8,
    scope: u8,
    kind: u8,
    gateway: Option<[u8; 4]>,
    source: Option<[u8; 4]>,
    interface: u32,
}

/// The routing tables of the made-up network, main and local.
fn route_table() -> Vec<Route> {
    let main = rt_class_t::RT_TABLE_MAIN as u8;
    let local = rt_class_t::RT_TABLE_LOCAL as u8;
    let kernel_protocol = RTPROT_KERNEL as u8;
    let boot = RTPROT_BOOT as u8;
    let (host, link, universe) = (
        rt_scope_t::RT_SCOPE_HOST as u8,
        rt_scope_t::RT_SCOPE_LINK as u8,
        rt_scope_t::RT_SCOPE_UNIVERSE as u8,
    );
    let (unicast, local_kind, broadcast) =
        (RTN_UNICAST as u8, RTN_LOCAL as u8, RTN_BROADCAST as u8);
    let v4 = |address: [u8; 4]| {
        let mut out = [0u8; 16];
        out[..4].copy_from_slice(&address);
        out
    };
    let a = network::ADDRESS;
    let subnet = [a[0], a[1], 0, 0];
    let (lo, eth) = (network::LOOPBACK_INDEX, network::ETHERNET_INDEX);
    let loopback = [127, 0, 0, 1];
    let route =
        |destination: [u8; 4], prefix, table, protocol, scope, kind, gateway, source, interface| {
            Route {
                family: AF_INET,
                destination: v4(destination),
                prefix,
                table,
                protocol,
                scope,
                kind,
                gateway,
                source,
                interface,
            }
        };
    let mut table = alloc::vec![
        route(
            [0; 4],
            0,
            main,
            boot,
            universe,
            unicast,
            Some(network::GATEWAY),
            None,
            eth
        ),
        route(
            subnet,
            network::PREFIX,
            main,
            kernel_protocol,
            link,
            unicast,
            None,
            Some(a),
            eth
        ),
        route(
            [127, 0, 0, 0],
            8,
            local,
            kernel_protocol,
            host,
            local_kind,
            None,
            Some(loopback),
            lo
        ),
        route(
            loopback,
            32,
            local,
            kernel_protocol,
            host,
            local_kind,
            None,
            Some(loopback),
            lo
        ),
        route(
            [127, 255, 255, 255],
            32,
            local,
            kernel_protocol,
            link,
            broadcast,
            None,
            Some(loopback),
            lo
        ),
        route(
            a,
            32,
            local,
            kernel_protocol,
            host,
            local_kind,
            None,
            Some(a),
            eth
        ),
        route(
            network::BROADCAST,
            32,
            local,
            kernel_protocol,
            link,
            broadcast,
            None,
            Some(a),
            eth
        ),
    ];
    let mut six = [0u8; 16];
    six[15] = 1;
    table.push(Route {
        family: AF_INET6,
        destination: six,
        prefix: 128,
        table: local,
        protocol: kernel_protocol,
        scope: universe,
        kind: local_kind,
        gateway: None,
        source: None,
        interface: lo,
    });
    table
}

/// `inet_rtm_getroute` and `inet6_rtm_getroute`: the route a packet to one address takes, or
/// with RTM_F_FIB_MATCH the table entry that chose it. IPv4's local and main tables are one
/// table here, as on a Linux with no rules added, so both say the main table.
fn route_get(
    kernel: &Kernel,
    family: u32,
    body: &[u8],
    request: &[u8],
    portid: u32,
    uid: u32,
    strict: bool,
) -> Result<Answer, Errno> {
    const RTMSG: usize = 12;
    if body.len() < RTMSG {
        return Err(Errno::EINVAL);
    }
    let rtm_flags = u32::from_le_bytes(body[8..12].try_into().expect("4 bytes"));
    let (mut to, mut oif) = (None, None);
    let mut kinds = Vec::new();
    let mut at = RTMSG;
    while body.len() - at >= 4 {
        let length = usize::from(u16::from_le_bytes([body[at], body[at + 1]]));
        let kind = u16::from_le_bytes([body[at + 2], body[at + 3]]);
        if length < 4 || at + length > body.len() {
            break;
        }
        let data = &body[at + 4..at + length];
        kinds.push(u32::from(kind));
        if kind == RTA_DST as u16 {
            to = Some(data.to_vec());
        } else if kind == RTA_OIF as u16 && data.len() >= 4 {
            oif = Some(u32::from_le_bytes(data[..4].try_into().expect("4 bytes")));
        }
        at += length.next_multiple_of(NLMSG_ALIGNTO as usize);
    }
    if family != AF_INET && family != AF_INET6 {
        return Err(Errno::EOPNOTSUPP);
    }
    if strict {
        valid_route_question(family, body, rtm_flags, &kinds)?;
    }
    if family == AF_INET6 {
        let mut loopback = [0u8; 16];
        loopback[15] = 1;
        if to.as_deref() != Some(&loopback[..]) {
            return Err(Errno::ENETUNREACH);
        }
        let mut message = Message::new(RTM_NEWROUTE as u16, 0, request, portid);
        let local = rt_class_t::RT_TABLE_LOCAL as u8;
        message.push(&[
            AF_INET6 as u8,
            128,
            128,
            0,
            local,
            RTPROT_KERNEL as u8,
            0,
            RTN_LOCAL as u8,
            0,
            0,
            0,
            0,
        ]);
        message.u32(RTA_TABLE as u32, u32::from(local));
        message.attr(RTA_DST as u32, &loopback);
        message.attr(RTA_SRC as u32, &[0; 16]);
        message.attr(RTA_PREFSRC as u32, &loopback);
        message.u32(RTA_PRIORITY as u32, 0);
        message.u32(RTA_OIF as u32, network::LOOPBACK_INDEX);
        message.attr(RTA_CACHEINFO as u32, &cache_information());
        message.u8(RTA_PREF as u32, 0);
        return Ok(Answer::One(message.finish()));
    }
    let to: [u8; 4] = match to {
        None => [0; 4],
        Some(bytes) => bytes
            .get(..4)
            .ok_or(Errno::EINVAL)?
            .try_into()
            .expect("4 bytes"),
    };
    let main = rt_class_t::RT_TABLE_MAIN as u8;
    // `fib_lookup`: the longest prefix that holds the address.
    let address = u32::from_be_bytes(to);
    let entry = route_table()
        .into_iter()
        .filter(|route| route.family == AF_INET)
        .filter(|route| {
            let prefix = u32::from(route.prefix);
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            let destination = u32::from_be_bytes(route.destination[..4].try_into().expect("4"));
            address & mask == destination & mask
        })
        .max_by_key(|route| route.prefix)
        .ok_or(Errno::ENETUNREACH)?;
    if rtm_flags & RTM_F_FIB_MATCH != 0 {
        let entry = Route {
            table: main,
            ..entry
        };
        return Ok(Answer::One(route_message(&entry, request, portid, 0)));
    }
    use crate::linux_extra::{RTCF_BROADCAST, RTCF_LOCAL, RTCF_MULTICAST};
    let a = network::ADDRESS;
    let loopback = [127, 0, 0, 1];
    let limited_broadcast = to == [255; 4];
    let multicast = to[0] & 0xf0 == 0xe0;
    // `ip_route_output_key_hash_rcu`: no destination is this host; a local route leaves by the
    // loopback; else `__mkroute_output` makes broadcasts and groups of the address alone, with
    // no gateway, and a group is local only while the interface is a member.
    let local = RTN_LOCAL as u8;
    let (kind, mut flags, mut interface, source, mut gateway) = if to == [0; 4] {
        (local, RTCF_LOCAL, network::LOOPBACK_INDEX, loopback, None)
    } else if entry.kind == local {
        let source = entry.source.unwrap_or(to);
        (local, RTCF_LOCAL, network::LOOPBACK_INDEX, source, None)
    } else {
        let source = entry.source.unwrap_or(a);
        let kind = if limited_broadcast {
            RTN_BROADCAST as u8
        } else if multicast {
            RTN_MULTICAST as u8
        } else {
            entry.kind
        };
        let mut flags = if entry.interface == network::LOOPBACK_INDEX {
            RTCF_LOCAL
        } else {
            0
        };
        let mut gateway = entry.gateway;
        if kind == RTN_BROADCAST as u8 {
            flags |= RTCF_BROADCAST | RTCF_LOCAL;
            gateway = None;
        } else if kind == RTN_MULTICAST as u8 {
            flags |= RTCF_MULTICAST | RTCF_LOCAL;
            if !super::inet::interface_in_group(kernel, to, entry.interface) {
                flags &= !RTCF_LOCAL;
            }
            gateway = None;
        }
        (kind, flags, entry.interface, source, gateway)
    };
    // Out of an interface named: the loopback takes anything as for this host.
    match oif {
        None | Some(0) => {}
        Some(index) if index == network::LOOPBACK_INDEX => {
            interface = index;
            flags |= RTCF_LOCAL;
            gateway = None;
        }
        Some(index) if index == network::ETHERNET_INDEX => {}
        Some(_) => return Err(Errno::ENODEV),
    }
    flags |= RTM_F_CLONED;
    let mut message = Message::new(RTM_NEWROUTE as u16, 0, request, portid);
    let mut head = [AF_INET as u8, 32, 0, 0, main, 0, 0, kind, 0, 0, 0, 0];
    head[8..].copy_from_slice(&flags.to_le_bytes());
    message.push(&head);
    message.u32(RTA_TABLE as u32, u32::from(main));
    message.attr(RTA_DST as u32, &to);
    message.u32(RTA_OIF as u32, interface);
    message.attr(RTA_PREFSRC as u32, &source);
    if let Some(gateway) = gateway {
        message.attr(RTA_GATEWAY as u32, &gateway);
    }
    message.u32(RTA_UID as u32, uid);
    message.attr(RTA_CACHEINFO as u32, &cache_information());
    Ok(Answer::One(message.finish()))
}

/// `inet_rtm_valid_getroute_req` and `inet6_rtm_valid_getroute_req` under
/// NETLINK_GET_STRICT_CHK: whole-address prefixes, IPv4's other header fields unset, only the
/// flags and attributes a question may carry.
fn valid_route_question(family: u32, body: &[u8], flags: u32, kinds: &[u32]) -> Result<(), Errno> {
    let full = if family == AF_INET6 { 128 } else { 32 };
    let (dst_len, src_len) = (body[1], body[2]);
    if (dst_len != 0 && dst_len != full) || (src_len != 0 && src_len != full) {
        return Err(Errno::EINVAL);
    }
    let allowed_flags = if family == AF_INET6 {
        RTM_F_FIB_MATCH
    } else {
        if body[4..8].iter().any(|&field| field != 0) {
            return Err(Errno::EINVAL);
        }
        RTM_F_NOTIFY | RTM_F_LOOKUP_TABLE | RTM_F_FIB_MATCH
    };
    if flags & !allowed_flags != 0 {
        return Err(Errno::EINVAL);
    }
    let has = |kind: u32| kinds.contains(&kind);
    if (has(RTA_SRC as u32) && src_len == 0) || (has(RTA_DST as u32) && dst_len == 0) {
        return Err(Errno::EINVAL);
    }
    let allowed = [
        RTA_IIF,
        RTA_OIF,
        RTA_SRC,
        RTA_DST,
        RTA_IP_PROTO,
        RTA_SPORT,
        RTA_DPORT,
        RTA_MARK,
        RTA_UID,
    ];
    if kinds
        .iter()
        .any(|kind| !allowed.iter().any(|&allowed| allowed as u32 == *kind))
    {
        return Err(Errno::EINVAL);
    }
    Ok(())
}

/// `rta_cacheinfo` of a route answer: its one reference (the answer's own; Linux's count also
/// has the route cache's, which varies), and nothing else kept.
fn cache_information() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[0] = 1;
    bytes
}

/// `fib_nl_dumprule`: the rules a network namespace starts with, IPv4's, its multicast
/// routing's (`RTNL_FAMILY_IPMR`) and IPv6's, each with no priority for the first (0).
fn rules(family: u32, request: &[u8], portid: u32) -> Vec<Vec<u8>> {
    use crate::linux::netlink::{
        FR_ACT_TO_TBL, FRA_PRIORITY, FRA_PROTOCOL, FRA_SUPPRESS_PREFIXLEN, FRA_TABLE,
    };
    let (local, main, default) = (
        rt_class_t::RT_TABLE_LOCAL as u8,
        rt_class_t::RT_TABLE_MAIN as u8,
        rt_class_t::RT_TABLE_DEFAULT as u8,
    );
    let rules: [(u32, u8, u32); 6] = [
        (AF_INET, local, 0),
        (AF_INET, main, 32766),
        (AF_INET, default, 32767),
        (RTNL_FAMILY_IPMR, default, 32767),
        (AF_INET6, local, 0),
        (AF_INET6, main, 32766),
    ];
    rules
        .iter()
        .filter(|(of, ..)| family == AF_UNSPEC || family == *of)
        .map(|&(of, table, priority)| {
            let mut message = Message::new(RTM_NEWRULE as u16, NLM_F_MULTI as u16, request, portid);
            message.push(&[
                of as u8,
                0,
                0,
                0,
                table,
                0,
                0,
                FR_ACT_TO_TBL as u8,
                0,
                0,
                0,
                0,
            ]);
            message.u32(FRA_TABLE as u32, u32::from(table));
            message.u32(FRA_SUPPRESS_PREFIXLEN as u32, u32::MAX);
            message.u8(FRA_PROTOCOL as u32, RTPROT_KERNEL as u8);
            if priority != 0 {
                message.u32(FRA_PRIORITY as u32, priority);
            }
            message.finish()
        })
        .collect()
}

fn route_message(route: &Route, request: &[u8], portid: u32, flags: u16) -> Vec<u8> {
    let mut message = Message::new(RTM_NEWROUTE as u16, flags, request, portid);
    let head = [
        route.family as u8,
        route.prefix,
        0,
        0,
        route.table,
        route.protocol,
        route.scope,
        route.kind,
        0,
        0,
        0,
        0,
    ];
    message.push(&head);
    message.u32(RTA_TABLE as u32, u32::from(route.table));
    if route.family == AF_INET {
        if route.prefix > 0 {
            message.attr(RTA_DST as u32, &route.destination[..4]);
        }
        if let Some(source) = route.source {
            message.attr(RTA_PREFSRC as u32, &source);
        }
        if let Some(gateway) = route.gateway {
            message.attr(RTA_GATEWAY as u32, &gateway);
        }
        message.u32(RTA_OIF as u32, route.interface);
    } else {
        message.attr(RTA_DST as u32, &route.destination);
        message.u32(RTA_PRIORITY as u32, 0);
        message.u32(RTA_OIF as u32, route.interface);
        message.attr(RTA_CACHEINFO as u32, &[0; 32]);
        message.u8(RTA_PREF as u32, 0);
    }
    message.finish()
}

/// `netlink_recvmsg`: one message buffer, the kernel as its sender.
pub(super) fn receive<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    parts: super::Data<'_>,
    flags: u32,
    nonblocking: bool,
) -> Result<(u64, Received), Errno> {
    // `netlink_recvmsg` has no urgent data.
    if flags & MSG_OOB != 0 {
        return Err(Errno::EOPNOTSUPP);
    }
    let size = parts.size();
    if socket.borrow().queue.is_empty() {
        let timeout = socket.borrow().receive_timeout;
        return Err(wait_room(kernel, tid, socket, nonblocking, timeout));
    }
    let data = socket
        .borrow()
        .queue
        .front()
        .expect("a message")
        .data
        .clone();
    let copied = (data.len() as u64).min(size);
    // `netlink_recvmsg` lets the message go even when copying it out faults, and goes on with a
    // running dump once the queue holds at most half its room, a peek too.
    let copy = scatter(kernel, cx, tid, parts, &data[..copied as usize]);
    if flags & MSG_PEEK == 0 {
        socket.borrow_mut().queue.pop_front();
    }
    let next = {
        let own = socket.borrow();
        let held: u64 = own.queue.iter().map(|packet| packet.truesize).sum();
        held <= own.rcvbuf / 2 && !own.netlink.as_deref().unwrap().dump.is_empty()
    };
    if next {
        let reply = part(socket).dump.pop_front().expect("a part");
        queue(kernel, cx, tid, socket, reply);
    }
    copy?;
    let mut received = Received {
        address: Some(address(0, 0)),
        ..Received::default()
    };
    if data.len() as u64 > size {
        received.flags |= MSG_TRUNC;
    }
    if flags & MSG_TRUNC != 0 {
        received.reported = Some(data.len() as u64);
    }
    Ok((copied, received))
}

/// setsockopt at SOL_NETLINK (`netlink_setsockopt`): the flag options are kept; joining and
/// leaving groups changes which groups it is in.
pub(super) fn set_option(
    socket: &SocketRef,
    level: u32,
    name: u32,
    bytes: &[u8],
) -> Option<Result<u64, Errno>> {
    if level != SOL_NETLINK {
        return None;
    }
    if bytes.len() < 4 {
        return Some(Err(Errno::EINVAL));
    }
    let value = u32::from_le_bytes(bytes[..4].try_into().expect("4 bytes"));
    let mut netlink = part(socket);
    match name {
        NETLINK_ADD_MEMBERSHIP | NETLINK_DROP_MEMBERSHIP => {
            if value == 0 || value > 32 {
                return Some(Err(Errno::EINVAL));
            }
            let bit = 1 << (value - 1);
            if name == NETLINK_ADD_MEMBERSHIP {
                netlink.groups |= bit;
            } else {
                netlink.groups &= !bit;
            }
        }
        NETLINK_PKTINFO
        | NETLINK_BROADCAST_ERROR
        | NETLINK_NO_ENOBUFS
        | NETLINK_LISTEN_ALL_NSID
        | NETLINK_CAP_ACK
        | NETLINK_EXT_ACK
        | NETLINK_GET_STRICT_CHK => {
            if value != 0 {
                netlink.flags |= 1 << name;
            } else {
                netlink.flags &= !(1 << name);
            }
        }
        _ => return Some(Err(Errno::ENOPROTOOPT)),
    }
    Some(Ok(0))
}

pub(super) fn get_option(
    socket: &SocketRef,
    level: u32,
    name: u32,
) -> Option<Result<Vec<u8>, Errno>> {
    if level != SOL_NETLINK {
        return None;
    }
    let netlink = part(socket);
    Some(match name {
        NETLINK_PKTINFO
        | NETLINK_BROADCAST_ERROR
        | NETLINK_NO_ENOBUFS
        | NETLINK_LISTEN_ALL_NSID
        | NETLINK_CAP_ACK
        | NETLINK_EXT_ACK
        | NETLINK_GET_STRICT_CHK => Ok(u32::from(netlink.flags & (1 << name) != 0)
            .to_le_bytes()
            .to_vec()),
        NETLINK_LIST_MEMBERSHIPS => Ok(netlink.groups.to_le_bytes().to_vec()),
        _ => Err(Errno::ENOPROTOOPT),
    })
}
