//! Internet sockets (AF_INET and AF_INET6) as `net/ipv4` and `net/ipv6` show them to programs.
//! TCP and UDP between the container's own addresses never leave the kernel: a TCP connection is
//! two sockets joined as local streams are, and a UDP datagram goes straight to the socket bound
//! to its destination.

use alloc::collections::VecDeque;
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::RefCell;

use super::{
    Outgoing, Packet, RCV_SHUTDOWN, Received, SEND_SHUTDOWN, SHUTDOWN_MASK, Socket, SocketRef,
    State, channel, gather, scatter, wait_room,
};
use crate::file::FileRef;
use crate::linux::general::*;
use crate::linux::net::*;
use crate::network;
use crate::process::Tid;
use crate::{Context, Errno, Kernel};

/// The protocol numbers, as plain values.
const TCP: u32 = IPPROTO_TCP as u32;
const UDP: u32 = IPPROTO_UDP as u32;
const ICMP: u32 = IPPROTO_ICMP as u32;
const ICMPV6: u32 = IPPROTO_ICMPV6;
const MPTCP: u32 = IPPROTO_MPTCP as u32;
const SCTP: u32 = IPPROTO_SCTP as u32;
const UDPLITE: u32 = IPPROTO_UDPLITE as u32;

/// An address and port, IPv4 addresses held as IPv4-mapped IPv6 ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Endpoint {
    pub address: [u8; 16],
    pub port: u16,
}

/// The IPv4-mapped form of an IPv4 address.
pub(crate) const fn mapped(v4: [u8; 4]) -> [u8; 16] {
    [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, v4[0], v4[1], v4[2], v4[3],
    ]
}

/// The IPv4 address an IPv4-mapped one holds.
pub(crate) fn v4_of(address: &[u8; 16]) -> Option<[u8; 4]> {
    (address[..12] == mapped([0; 4])[..12])
        .then(|| [address[12], address[13], address[14], address[15]])
}

const ANY4: [u8; 16] = mapped([0; 4]);
const ANY6: [u8; 16] = [0; 16];
const LOOPBACK4: [u8; 16] = mapped([127, 0, 0, 1]);
const LOOPBACK6: [u8; 16] = {
    let mut address = [0; 16];
    address[15] = 1;
    address
};

fn is_any(address: &[u8; 16]) -> bool {
    *address == ANY4 || *address == ANY6
}

/// Whether an address is one of the container's own (`inet_addr_type` gives RTN_LOCAL).
fn is_own(address: &[u8; 16]) -> bool {
    match v4_of(address) {
        Some(v4) => v4[0] == 127 || v4 == network::ADDRESS,
        None => *address == LOOPBACK6,
    }
}

/// Whether a bind may name an address: the container's own, a broadcast or multicast one, or any.
fn bindable(address: &[u8; 16]) -> bool {
    if is_any(address) || is_own(address) {
        return true;
    }
    match v4_of(address) {
        Some(v4) => v4 == network::BROADCAST || v4 == [255; 4] || (v4[0] & 0xf0) == 0xe0,
        None => address[0] == 0xff,
    }
}

/// The source address a connection from here to `to` takes (`ip_route_connect`).
fn source_for(to: &[u8; 16]) -> [u8; 16] {
    match v4_of(to) {
        Some(v4) if v4[0] == 127 => LOOPBACK4,
        Some(_) => mapped(network::ADDRESS),
        None => LOOPBACK6,
    }
}

/// The ports used by TCP and UDP: `ip_local_port_range` (measured).
pub(super) const EPHEMERAL: core::ops::RangeInclusive<u16> = 32768..=60999;
/// How long a closed connection holds its ports (`TCP_TIMEWAIT_LEN`).
const TIME_WAIT: u64 = 60_000_000_000;

/// `tcp_wmem` and `tcp_rmem`'s defaults, a connection's send buffer on loopback
/// (`tcp_sndbuf_expand` for its MSS), and the MSS before and after connecting (all measured).
const TCP_SNDBUF: u64 = 16384;
const TCP_RCVBUF: u64 = 131072;
const TCP_CONNECTED_SNDBUF: u64 = 2_626_560;
const TCP_MSS_UNCONNECTED: i32 = 536;
const TCP_MSS_LOOPBACK: i32 = 32741;
/// What `IP_MTU` gives on a connection inside the container (measured).
const ROUTE_MTU: i32 = 65535;
/// The largest UDP payloads (measured).
const UDP_MAX4: u64 = 65507;
const UDP_MAX6: u64 = 65527;
/// The most bytes one buffer of a TCP send holds here.
const TCP_PIECE: u64 = 65536;

/// The protocols with ports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Protocol {
    Tcp,
    Udp,
}

/// An internet socket's own state beside the common [`Socket`]'s.
pub(crate) struct Inet {
    pub protocol: Protocol,
    /// The bound address (any when none was chosen) and port (0 when not bound).
    pub local: Endpoint,
    /// Whether the program chose the address or the port (`SOCK_BINDADDR_LOCK`,
    /// `SOCK_BINDPORT_LOCK`).
    pub address_chosen: bool,
    pub port_chosen: bool,
    /// Whether it holds its port in [`Ports`] (`inet_num`); a refused connect lets go of the
    /// port it was given and still shows it.
    pub held: bool,
    /// A connect returned before it finished (`SS_CONNECTING`): the next connect reports how it
    /// went.
    pub in_progress: bool,
    /// The other end of a connection, or a UDP socket's default destination.
    pub remote: Option<Endpoint>,
    pub v6only: bool,
    /// A TCP connect waiting for room in a listener's backlog (`TCP_SYN_SENT`), and the listener.
    pub connecting: Option<Weak<RefCell<Socket>>>,
    /// For a listener: the connects waiting for room, first first.
    pub waiting: VecDeque<Weak<RefCell<Socket>>>,
    /// The other end's FIN came (`SOCK_DONE`).
    pub done: bool,
    /// It sent its FIN before the other end (it closes actively and holds the ports a while).
    pub closed_first: bool,
    /// Whether the program set the buffer sizes (`SOCK_SNDBUF_LOCK`, `SOCK_RCVBUF_LOCK`).
    pub sndbuf_set: bool,
    pub rcvbuf_set: bool,
    /// The host connection of a TCP connection to the outside.
    pub outside: Option<u32>,
    /// A UDP socket's host connections, by outside destination.
    pub flows: alloc::collections::BTreeMap<Endpoint, u32>,
    /// Whether the host takes more bytes of an outside connection now.
    pub room: bool,
    /// Bytes have come on the connection, and it has sent after they came: what
    /// `/proc/net/tcp` shows of its delayed acknowledgements (a model: Linux's depend on timing).
    pub received: bool,
    pub pingpong: bool,
    /// The program has read the end its FIN marks.
    pub fin_read: bool,
}

impl Inet {
    fn new(protocol: Protocol) -> Self {
        Self {
            protocol,
            local: Endpoint::default(),
            address_chosen: false,
            port_chosen: false,
            held: false,
            in_progress: false,
            remote: None,
            v6only: false,
            connecting: None,
            waiting: VecDeque::new(),
            done: false,
            closed_first: false,
            sndbuf_set: false,
            rcvbuf_set: false,
            outside: None,
            flows: alloc::collections::BTreeMap::new(),
            room: false,
            received: false,
            pingpong: false,
            fin_read: false,
        }
    }
}

/// A closed TCP connection holding its port a while.
pub(super) struct TimeWait {
    pub family: u32,
    pub local: Endpoint,
    pub remote: Endpoint,
    /// Whether it had SO_REUSEADDR.
    pub reuse: bool,
    /// Until when, on the monotonic clock.
    pub until: u64,
}

/// The ports in use: every socket holding one, and closed connections still holding theirs.
#[derive(Default)]
pub(crate) struct Ports {
    pub(super) bound: alloc::collections::BTreeMap<(Protocol, u16), Vec<Weak<RefCell<Socket>>>>,
    /// Closed TCP connections in TIME_WAIT.
    pub(super) time_wait: Vec<TimeWait>,
    /// Connections to the outside and name lookups under way.
    pub outside: super::outside::Outside,
}

fn inet(socket: &SocketRef) -> core::cell::RefMut<'_, Inet> {
    core::cell::RefMut::map(socket.borrow_mut(), |own| {
        own.inet.as_deref_mut().expect("an internet socket")
    })
}

fn inet_ref(socket: &SocketRef) -> core::cell::Ref<'_, Inet> {
    core::cell::Ref::map(socket.borrow(), |own| {
        own.inet.as_deref().expect("an internet socket")
    })
}

fn flag(socket: &Socket, level: u32, name: u32) -> bool {
    socket
        .options
        .get(&(level, name))
        .is_some_and(|value| value.iter().any(|&byte| byte != 0))
}

/// `inet_create` and `inet6_create`: the protocol a kind names, or what Linux answers.
pub(super) fn create(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    family: u32,
    kind: u32,
    protocol: u32,
) -> Result<Socket, Errno> {
    if protocol >= IPPROTO_MAX as u32 {
        return Err(Errno::EINVAL);
    }
    let refuse = |kernel: &mut Kernel, cx: &mut Context<'_>, answer: Errno| {
        let detail = (u64::from(family) << 32) | (u64::from(kind) << 16) | u64::from(protocol);
        Err(kernel
            .counters
            .refuse(cx.host, crate::reason::PROTOCOL_NOT_BUILT, detail, answer))
    };
    let icmp = if family == AF_INET { ICMP } else { ICMPV6 };
    let (protocol, number) = match kind {
        SOCK_STREAM => match protocol {
            0 | TCP => (Protocol::Tcp, TCP),
            MPTCP | SCTP => {
                return refuse(kernel, cx, Errno::EPROTONOSUPPORT);
            }
            _ => return Err(Errno::EPROTONOSUPPORT),
        },
        SOCK_DGRAM => match protocol {
            0 | UDP => (Protocol::Udp, UDP),
            // Ping sockets, which Docker's `ping_group_range` lets every group make.
            _ if protocol == icmp => return refuse(kernel, cx, Errno::EACCES),
            UDPLITE => return refuse(kernel, cx, Errno::EPROTONOSUPPORT),
            _ => return Err(Errno::EPROTONOSUPPORT),
        },
        // Raw packets, which Docker's CAP_NET_RAW allows (plan section 15).
        SOCK_RAW => return refuse(kernel, cx, Errno::EPERM),
        // SCTP, which the reference has.
        SOCK_SEQPACKET => return refuse(kernel, cx, Errno::ESOCKTNOSUPPORT),
        _ => return Err(Errno::ESOCKTNOSUPPORT),
    };
    let mut socket = Socket::new(family, kind, number);
    if protocol == Protocol::Tcp {
        socket.sndbuf = TCP_SNDBUF;
        socket.rcvbuf = TCP_RCVBUF;
    }
    socket.inet = Some(alloc::boxed::Box::new(Inet::new(protocol)));
    Ok(socket)
}

/// A `sockaddr_in` or `sockaddr_in6` for an endpoint, as the socket's family shows it.
pub(super) fn address_bytes(family: u32, endpoint: &Endpoint) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(28);
    let port = endpoint.port.to_be_bytes();
    if family == AF_INET {
        bytes.extend_from_slice(&(AF_INET as u16).to_le_bytes());
        bytes.extend_from_slice(&port);
        bytes.extend_from_slice(&v4_of(&endpoint.address).unwrap_or([0; 4]));
        bytes.extend_from_slice(&[0; 8]);
    } else {
        bytes.extend_from_slice(&(AF_INET6 as u16).to_le_bytes());
        bytes.extend_from_slice(&port);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&endpoint.address);
        bytes.extend_from_slice(&[0; 4]);
    }
    bytes
}

/// What a program named: an endpoint, or AF_UNSPEC.
enum Named {
    Endpoint(Endpoint),
    Unspecified,
}

/// Reads an address for a socket of `family` as `inet_bind`, `tcp_v4_connect` and their IPv6
/// forms check it: too short is EINVAL, another family EAFNOSUPPORT.
fn read_endpoint(family: u32, bytes: &[u8]) -> Result<Named, Errno> {
    if bytes.len() < 2 {
        return Err(Errno::EINVAL);
    }
    let named = u16::from_le_bytes([bytes[0], bytes[1]]) as u32;
    if family == AF_INET {
        if bytes.len() < 16 {
            return Err(Errno::EINVAL);
        }
        let port = u16::from_be_bytes([bytes[2], bytes[3]]);
        let v4 = [bytes[4], bytes[5], bytes[6], bytes[7]];
        let endpoint = Endpoint {
            address: mapped(v4),
            port,
        };
        return match named {
            AF_INET => Ok(Named::Endpoint(endpoint)),
            AF_UNSPEC => Ok(Named::Unspecified),
            _ => Err(Errno::EAFNOSUPPORT),
        };
    }
    if named == AF_UNSPEC {
        return Ok(Named::Unspecified);
    }
    if bytes.len() < 24 {
        return Err(Errno::EINVAL);
    }
    if named != AF_INET6 {
        return Err(Errno::EAFNOSUPPORT);
    }
    let port = u16::from_be_bytes([bytes[2], bytes[3]]);
    let mut address = [0; 16];
    address.copy_from_slice(&bytes[8..24]);
    Ok(Named::Endpoint(Endpoint { address, port }))
}

/// Whether a socket of `family` can be at `address` (IPv4 only for IPv4 sockets; IPv4-mapped
/// ones only for IPv6 sockets that take IPv4).
fn family_takes(family: u32, v6only: bool, address: &[u8; 16]) -> bool {
    match (family, v4_of(address).is_some()) {
        (AF_INET, mapped) => mapped,
        (_, true) => !v6only,
        (_, false) => true,
    }
}

impl Ports {
    fn holders(&self, protocol: Protocol, port: u16) -> Vec<SocketRef> {
        self.bound
            .get(&(protocol, port))
            .into_iter()
            .flatten()
            .filter_map(Weak::upgrade)
            .filter(|socket| !socket.borrow().dead)
            .collect()
    }

    fn hold(&mut self, protocol: Protocol, port: u16, socket: &SocketRef) {
        let holders = self.bound.entry((protocol, port)).or_default();
        holders.retain(|other| other.strong_count() > 0);
        holders.push(Rc::downgrade(socket));
    }

    fn let_go(&mut self, protocol: Protocol, port: u16, socket: &SocketRef) {
        if let Some(holders) = self.bound.get_mut(&(protocol, port)) {
            holders.retain(|other| {
                other.strong_count() > 0 && !core::ptr::eq(other.as_ptr(), Rc::as_ptr(socket))
            });
            if holders.is_empty() {
                self.bound.remove(&(protocol, port));
            }
        }
    }
}

/// What a socket on a port takes: IPv4 addresses (all, or one) and IPv6 ones.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cover {
    None,
    All,
    One([u8; 16]),
}

fn cover(family: u32, v6only: bool, address: &[u8; 16]) -> (Cover, Cover) {
    if family == AF_INET || v4_of(address).is_some() {
        let v4 = if *address == ANY4 {
            Cover::All
        } else {
            Cover::One(*address)
        };
        return (v4, Cover::None);
    }
    if *address == ANY6 {
        let v4 = if v6only { Cover::None } else { Cover::All };
        return (v4, Cover::All);
    }
    (Cover::None, Cover::One(*address))
}

fn meet(a: Cover, b: Cover) -> bool {
    match (a, b) {
        (Cover::None, _) | (_, Cover::None) => false,
        (Cover::All, _) | (_, Cover::All) => true,
        (Cover::One(x), Cover::One(y)) => x == y,
    }
}

/// Whether two sockets on one port take an address in common, `a` at `address`.
fn overlaps(a: &Socket, b: &Socket, address: &[u8; 16]) -> bool {
    let (a_inet, b_inet) = (a.inet.as_deref().unwrap(), b.inet.as_deref().unwrap());
    let (a4, a6) = cover(a.family, a_inet.v6only, address);
    let (b4, b6) = cover(b.family, b_inet.v6only, &b_inet.local.address);
    meet(a4, b4) || meet(a6, b6)
}

/// `inet_csk_bind_conflict` and `udp_lib_lport_inuse`: whether `socket` may not hold `port` at
/// `address` beside those holding it.
fn conflicts(
    kernel: &mut Kernel,
    socket: &SocketRef,
    protocol: Protocol,
    address: &[u8; 16],
    port: u16,
    uid: u32,
) -> bool {
    let own = socket.borrow();
    let reuse = flag(&own, SOL_SOCKET, SO_REUSEADDR);
    let reuse_port = flag(&own, SOL_SOCKET, SO_REUSEPORT);
    for other in kernel.sockets.inet.holders(protocol, port) {
        if Rc::ptr_eq(&other, socket) {
            continue;
        }
        let them = other.borrow();
        if !overlaps(&own, &them, address) {
            continue;
        }
        let both_reuse = reuse && flag(&them, SOL_SOCKET, SO_REUSEADDR);
        let ok = match protocol {
            Protocol::Tcp => both_reuse && them.state != State::Listening,
            Protocol::Udp => both_reuse,
        };
        let same_owner = them.owner_uid == uid;
        if ok || (reuse_port && flag(&them, SOL_SOCKET, SO_REUSEPORT) && same_owner) {
            continue;
        }
        return true;
    }
    if protocol == Protocol::Tcp {
        let now = kernel.entry_now;
        let ports = &mut kernel.sockets.inet;
        ports.time_wait.retain(|held| held.until > now);
        let family = own.family;
        let v6only = own.inet.as_deref().unwrap().v6only;
        let (own4, own6) = cover(family, v6only, address);
        return ports.time_wait.iter().any(|held| {
            let end = &held.local;
            let (end4, end6) = cover(AF_INET6, false, &end.address);
            !(reuse && held.reuse) && end.port == port && (meet(own4, end4) || meet(own6, end6))
        });
    }
    false
}

/// Takes a port for `socket` at its local address: the one asked for, or a free one from the
/// ephemeral range starting at a random place (`inet_csk_find_open_port` tries odd ports first,
/// `__inet_hash_connect` even ones).
fn take_port(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    port: u16,
    connecting: bool,
) -> Result<u16, Errno> {
    let protocol = inet_ref(socket).protocol;
    let address = inet_ref(socket).local.address;
    let uid = kernel.credentials(tid).euid;
    socket.borrow_mut().owner_uid = uid;
    if port != 0 {
        if conflicts(kernel, socket, protocol, &address, port, uid) {
            return Err(Errno::EADDRINUSE);
        }
        kernel.sockets.inet.hold(protocol, port, socket);
        return Ok(port);
    }
    let mut seed = [0u8; 4];
    cx.host.random(&mut seed);
    let (low, high) = (*EPHEMERAL.start() as u32, *EPHEMERAL.end() as u32);
    let span = high - low + 1;
    let start = u32::from_le_bytes(seed) % span;
    let parities: [u32; 2] = match (protocol, connecting) {
        (Protocol::Udp, _) => [0, 1],
        (Protocol::Tcp, true) => [0, 1],
        (Protocol::Tcp, false) => [1, 0],
    };
    let choose_parity = protocol == Protocol::Tcp;
    for parity in parities {
        for step in 0..span {
            let candidate = low + (start + step) % span;
            if choose_parity && candidate % 2 != parity {
                continue;
            }
            let candidate = candidate as u16;
            if !conflicts(kernel, socket, protocol, &address, candidate, uid)
                && kernel.sockets.inet.holders(protocol, candidate).is_empty()
            {
                kernel.sockets.inet.hold(protocol, candidate, socket);
                return Ok(candidate);
            }
        }
        if !choose_parity {
            break;
        }
    }
    Err(if connecting {
        Errno::EADDRNOTAVAIL
    } else {
        Errno::EADDRINUSE
    })
}

/// Gives a socket that has none a port of its own (`inet_autobind`).
fn autobind(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    connecting: bool,
) -> Result<(), Errno> {
    if inet_ref(socket).held {
        return Ok(());
    }
    if inet_ref(socket).local.address == Endpoint::default().address {
        let any = if socket.borrow().family == AF_INET {
            ANY4
        } else {
            ANY6
        };
        inet(socket).local.address = any;
    }
    let port = take_port(kernel, cx, tid, socket, 0, connecting)?;
    let mut own = inet(socket);
    own.local.port = port;
    own.held = true;
    Ok(())
}

pub(super) fn bind(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    bytes: &[u8],
) -> Result<u64, Errno> {
    let family = socket.borrow().family;
    let endpoint = match read_endpoint(family, bytes)? {
        Named::Endpoint(endpoint) => endpoint,
        // `inet_bind_sk` takes AF_UNSPEC with the any address, for old programs.
        Named::Unspecified if family == AF_INET && bytes[4..8] == [0; 4] => Endpoint {
            address: ANY4,
            port: u16::from_be_bytes([bytes[2], bytes[3]]),
        },
        Named::Unspecified => return Err(Errno::EAFNOSUPPORT),
    };
    let v6only = inet_ref(socket).v6only;
    if family == AF_INET6 && v4_of(&endpoint.address).is_some() && v6only {
        return Err(Errno::EINVAL);
    }
    let freebind = {
        let own = socket.borrow();
        flag(&own, SOL_IP, IP_FREEBIND) || flag(&own, SOL_IP, IP_TRANSPARENT)
    };
    if !bindable(&endpoint.address) && !freebind {
        return Err(Errno::EADDRNOTAVAIL);
    }
    {
        let own = inet_ref(socket);
        if own.held || socket.borrow().state != State::Closed {
            return Err(Errno::EINVAL);
        }
    }
    inet(socket).local.address = endpoint.address;
    let no_port = flag(&socket.borrow(), SOL_IP, IP_BIND_ADDRESS_NO_PORT);
    if endpoint.port == 0 && no_port {
        inet(socket).address_chosen = !is_any(&endpoint.address);
        return Ok(0);
    }
    match take_port(kernel, cx, tid, socket, endpoint.port, false) {
        Ok(port) => {
            let mut own = inet(socket);
            own.local.port = port;
            own.held = true;
            own.address_chosen = !is_any(&endpoint.address);
            own.port_chosen = endpoint.port != 0;
            Ok(0)
        }
        Err(error) => {
            inet(socket).local.address = [0; 16];
            Err(error)
        }
    }
}

pub(super) fn listen(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    backlog: i32,
) -> Result<u64, Errno> {
    if inet_ref(socket).protocol != Protocol::Tcp {
        return Err(Errno::EOPNOTSUPP);
    }
    let state = socket.borrow().state;
    if state == State::Connected || inet_ref(socket).connecting.is_some() {
        return Err(Errno::EINVAL);
    }
    if state != State::Listening {
        // A listener must not share its port with another listener that cannot share it.
        let (protocol, address, port) = {
            let own = inet_ref(socket);
            (
                own.protocol,
                own.local.address,
                own.held.then_some(own.local.port),
            )
        };
        let uid = kernel.credentials(tid).euid;
        if let Some(port) = port {
            socket.borrow_mut().state = State::Listening;
            if conflicts(kernel, socket, protocol, &address, port, uid) {
                socket.borrow_mut().state = State::Closed;
                return Err(Errno::EADDRINUSE);
            }
        } else {
            autobind(kernel, cx, tid, socket, false)?;
        }
    }
    let outside = {
        let mut own = socket.borrow_mut();
        own.max_backlog = backlog as u32;
        own.state = State::Listening;
        state != State::Listening && reached_from_outside(&own)
    };
    if outside {
        super::outside::listen(kernel, cx, socket);
    }
    Ok(0)
}

/// The address a connect names, as Linux sends to it: the any address is this host's loopback.
fn destination(family: u32, endpoint: Endpoint) -> Endpoint {
    let address = if endpoint.address == ANY4 {
        LOOPBACK4
    } else if endpoint.address == ANY6 && family == AF_INET6 {
        LOOPBACK6
    } else {
        endpoint.address
    };
    Endpoint {
        address,
        ..endpoint
    }
}

/// Whether a destination is unicast (`RTCF_MULTICAST`, `RTCF_BROADCAST` and IPv6 multicast are
/// not for TCP).
fn unicast(address: &[u8; 16]) -> bool {
    match v4_of(address) {
        Some(v4) => !(v4 == network::BROADCAST || v4 == [255; 4] || (v4[0] & 0xf0) == 0xe0),
        None => address[0] != 0xff,
    }
}

/// Whether the network reaches a destination: IPv4 through the default route, IPv6 only on
/// loopback (the made-up network has no IPv6 route out, as Docker's default bridge).
fn reachable(address: &[u8; 16]) -> bool {
    v4_of(address).is_some() || is_own(address)
}

pub(super) fn connect(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    socket: &SocketRef,
    bytes: &[u8],
) -> Result<u64, Errno> {
    if bytes.len() < 2 {
        return Err(Errno::EINVAL);
    }
    let protocol = inet_ref(socket).protocol;
    let family = socket.borrow().family;
    let named = if u16::from_le_bytes([bytes[0], bytes[1]]) as u32 == AF_UNSPEC {
        Named::Unspecified
    } else {
        read_endpoint(family, bytes)?
    };
    match protocol {
        Protocol::Udp => match named {
            Named::Unspecified => {
                disconnect_udp(kernel, socket);
                Ok(0)
            }
            Named::Endpoint(endpoint) => udp_connect(kernel, cx, tid, socket, endpoint),
        },
        Protocol::Tcp => match named {
            Named::Unspecified => {
                disconnect_tcp(kernel, socket);
                Ok(0)
            }
            Named::Endpoint(endpoint) => tcp_connect(kernel, cx, tid, file, socket, endpoint),
        },
    }
}

/// `__udp_disconnect`: no default destination, and the address and port it was given at
/// connect let go unless the program chose them.
fn disconnect_udp(kernel: &mut Kernel, socket: &SocketRef) {
    let mut own = socket.borrow_mut();
    own.state = State::Closed;
    let family = own.family;
    let inet = own.inet.as_deref_mut().unwrap();
    inet.remote = None;
    if !inet.address_chosen {
        inet.local.address = if family == AF_INET { ANY4 } else { ANY6 };
    }
    if !inet.port_chosen && inet.held {
        inet.held = false;
        let port = core::mem::take(&mut inet.local.port);
        drop(own);
        kernel.sockets.inet.let_go(Protocol::Udp, port, socket);
    }
}

/// `tcp_disconnect`: a connection is reset, a listener stops listening.
fn disconnect_tcp(kernel: &mut Kernel, socket: &SocketRef) {
    let state = socket.borrow().state;
    match state {
        State::Listening => {
            super::outside::stop_listening(kernel, socket);
            let backlog = core::mem::take(&mut socket.borrow_mut().backlog);
            for embryo in backlog {
                super::released(kernel, &embryo);
            }
            refuse_waiting(kernel, socket);
        }
        State::Connected => reset_peer(kernel, socket),
        State::Closed => {}
    }
    let packets = {
        let mut own = socket.borrow_mut();
        own.state = State::Closed;
        own.peer = None;
        own.shutdown = 0;
        own.error = 0;
        let inet = own.inet.as_deref_mut().unwrap();
        inet.remote = None;
        inet.connecting = None;
        inet.done = false;
        core::mem::take(&mut own.queue)
    };
    for packet in packets {
        super::drop_packet(kernel, packet);
    }
    kernel.wake(channel(socket));
}

/// Sends a reset to a connection's other end (`tcp_send_active_reset`): it sees ECONNRESET, or
/// EPIPE if its own FIN had come, and is closed (`tcp_reset`, `tcp_done`).
fn reset_peer(kernel: &mut Kernel, socket: &SocketRef) {
    let Some(peer) = socket.borrow().peer.clone() else {
        return;
    };
    let back = peer
        .borrow()
        .peer
        .as_ref()
        .is_some_and(|back| Rc::ptr_eq(back, socket));
    if back {
        reset(kernel, &peer);
    }
}

/// `tcp_reset`: an RST came.
fn reset(kernel: &mut Kernel, socket: &SocketRef) {
    {
        let mut own = socket.borrow_mut();
        let close_wait = own.shutdown & RCV_SHUTDOWN != 0 && own.state == State::Connected;
        own.error = if close_wait {
            Errno::EPIPE.0 as i32
        } else {
            Errno::ECONNRESET.0 as i32
        };
        own.state = State::Closed;
        own.shutdown = SHUTDOWN_MASK;
    }
    kernel.wake(channel(socket));
}

fn tcp_connect(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    file: &FileRef,
    socket: &SocketRef,
    endpoint: Endpoint,
) -> Result<u64, Errno> {
    let nonblocking = file.borrow().flags & O_NONBLOCK != 0;
    // A call parked on its own connect comes back here to see how it went, as does a connect after
    // one that returned before it finished.
    let waiting_call = kernel.threads[&tid].call.progress == 1;
    if waiting_call || inet_ref(socket).in_progress {
        if inet_ref(socket).connecting.is_some() {
            if nonblocking {
                return Err(Errno::EALREADY);
            }
            return wait_connect(kernel, tid, socket);
        }
        inet(socket).in_progress = false;
        return finish_connect(socket);
    }
    let state = socket.borrow().state;
    match state {
        State::Connected | State::Listening => return Err(Errno::EISCONN),
        State::Closed => {}
    }
    let family = socket.borrow().family;
    if !family_takes(family, inet_ref(socket).v6only, &endpoint.address) {
        return Err(Errno::ENETUNREACH);
    }
    if !unicast(&endpoint.address) {
        return Err(Errno::ENETUNREACH);
    }
    let to = destination(family, endpoint);
    if !reachable(&to.address) {
        return Err(Errno::ENETUNREACH);
    }
    if !inet_ref(socket).address_chosen {
        inet(socket).local.address = source_for(&to.address);
    }
    autobind(kernel, cx, tid, socket, true)?;
    if !is_own(&to.address) {
        return connect_outside(kernel, cx, tid, socket, to, nonblocking);
    }
    {
        let mut own = socket.borrow_mut();
        own.error = 0;
        own.shutdown = 0;
        let inet = own.inet.as_deref_mut().unwrap();
        inet.remote = Some(to);
        inet.done = false;
        inet.closed_first = false;
    }
    let Some(listener) = find_listener(kernel, &to) else {
        // The listener's reset comes at once (`tcp_reset` in SYN_SENT).
        refused(kernel, socket);
        if nonblocking {
            inet(socket).in_progress = true;
            return Err(Errno::EINPROGRESS);
        }
        // `inet_stream_connect` takes the error and disconnects (`tcp_disconnect`).
        let mut own = socket.borrow_mut();
        own.error = 0;
        own.shutdown = 0;
        return Err(Errno::ECONNREFUSED);
    };
    if backlog_full(&listener.borrow()) {
        kernel
            .counters
            .count(cx.host, crate::reason::TCP_BACKLOG_WAIT, 0);
        inet(socket).connecting = Some(Rc::downgrade(&listener));
        inet(&listener).waiting.push_back(Rc::downgrade(socket));
        if nonblocking {
            inet(socket).in_progress = true;
            return Err(Errno::EINPROGRESS);
        }
        return wait_connect(kernel, tid, socket);
    }
    join(kernel, socket, &listener);
    if nonblocking {
        inet(socket).in_progress = true;
        return Err(Errno::EINPROGRESS);
    }
    Ok(0)
}

/// A connect to the outside: the host's connection starts, and its answer finishes it.
fn connect_outside(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    to: Endpoint,
    nonblocking: bool,
) -> Result<u64, Errno> {
    let id = match super::outside::open(kernel, cx, socket, to, true) {
        Ok(id) => id,
        Err(error) => {
            failed(kernel, socket, error);
            let mut own = socket.borrow_mut();
            own.error = 0;
            own.shutdown = 0;
            return Err(error);
        }
    };
    {
        let mut own = socket.borrow_mut();
        own.error = 0;
        own.shutdown = 0;
        if !own.inet.as_deref().unwrap().sndbuf_set {
            own.sndbuf = TCP_CONNECTED_SNDBUF;
        }
        let inet = own.inet.as_deref_mut().unwrap();
        inet.remote = Some(to);
        inet.done = false;
        inet.closed_first = false;
        inet.outside = Some(id);
        inet.room = false;
        inet.connecting = Some(Weak::new());
    }
    if nonblocking {
        inet(socket).in_progress = true;
        return Err(Errno::EINPROGRESS);
    }
    wait_connect(kernel, tid, socket)
}

/// Waits for a connect to finish, as `inet_wait_for_connect`.
fn wait_connect(kernel: &mut Kernel, tid: Tid, socket: &SocketRef) -> Result<u64, Errno> {
    let timeout = socket.borrow().send_timeout;
    let error = wait_room(kernel, tid, socket, false, timeout);
    match error {
        Errno::PARKED => {
            kernel.thread_mut(tid).call.progress = 1;
            Err(error)
        }
        // A connect that timed out or was interrupted goes on in the background.
        Errno::EAGAIN => {
            inet(socket).in_progress = true;
            Err(Errno::EINPROGRESS)
        }
        _ => {
            inet(socket).in_progress = true;
            Err(error)
        }
    }
}

/// How a connect this call waited on ended.
fn finish_connect(socket: &SocketRef) -> Result<u64, Errno> {
    let mut own = socket.borrow_mut();
    if own.state == State::Connected {
        return Ok(0);
    }
    let error = core::mem::take(&mut own.error);
    Err(if error != 0 {
        Errno(error as u32)
    } else {
        Errno::ECONNABORTED
    })
}

/// A connect was refused: the socket is closed with ECONNREFUSED to report, and lets go of the
/// port it was given (`tcp_set_state(TCP_CLOSE)`).
fn refused(kernel: &mut Kernel, socket: &SocketRef) {
    failed(kernel, socket, Errno::ECONNREFUSED);
}

/// A connect failed with `error`, as [`refused`].
pub(super) fn failed(kernel: &mut Kernel, socket: &SocketRef, error: Errno) {
    let port = {
        let mut own = socket.borrow_mut();
        own.state = State::Closed;
        own.error = error.0 as i32;
        own.shutdown = SHUTDOWN_MASK;
        let inet = own.inet.as_deref_mut().unwrap();
        inet.remote = None;
        inet.connecting = None;
        inet.outside = None;
        let let_go = !inet.port_chosen && inet.held;
        if let_go {
            inet.held = false;
        }
        let_go.then_some(inet.local.port)
    };
    if let Some(port) = port {
        kernel.sockets.inet.let_go(Protocol::Tcp, port, socket);
    }
    if !inet_ref(socket).address_chosen {
        let any = if socket.borrow().family == AF_INET {
            ANY4
        } else {
            ANY6
        };
        inet(socket).local.address = any;
    }
    kernel.wake(channel(socket));
}

/// The listener for a destination: one bound to its address, else one bound to any address its
/// family covers (`compute_score`).
fn find_listener(kernel: &Kernel, to: &Endpoint) -> Option<SocketRef> {
    let mut best: Option<(u8, SocketRef)> = None;
    for socket in kernel.sockets.inet.holders(Protocol::Tcp, to.port) {
        let score = {
            let own = socket.borrow();
            if own.state != State::Listening {
                continue;
            }
            let inet = own.inet.as_deref().unwrap();
            if !family_takes(own.family, inet.v6only, &to.address) {
                continue;
            }
            if inet.local.address == to.address {
                2 + u8::from(own.family == AF_INET)
            } else if is_any(&inet.local.address)
                && (inet.local.address == ANY6 || v4_of(&to.address).is_some())
            {
                u8::from(own.family == AF_INET)
            } else {
                continue;
            }
        };
        if best.as_ref().is_none_or(|(top, _)| score > *top) {
            best = Some((score, socket));
        }
    }
    best.map(|(_, socket)| socket)
}

/// Joins a connecting socket to a new socket in the listener's backlog, the listener's end of
/// the connection, which takes the listener's options (`tcp_create_openreq_child`).
fn join(kernel: &mut Kernel, socket: &SocketRef, listener: &SocketRef) {
    let (local, remote) = {
        let own = inet_ref(socket);
        (own.local, own.remote.expect("a destination"))
    };
    let embryo = embryo(kernel, listener, remote.address, local);
    embryo.borrow_mut().peer = Some(socket.clone());
    {
        let mut client = socket.borrow_mut();
        client.state = State::Connected;
        client.peer = Some(embryo.clone());
        if !client.inet.as_deref().unwrap().sndbuf_set {
            client.sndbuf = TCP_CONNECTED_SNDBUF;
        }
        client.inet.as_deref_mut().unwrap().connecting = None;
    }
    listener.borrow_mut().backlog.push_back(embryo);
    kernel.wake_poll(channel(listener), EPOLLIN | EPOLLRDNORM);
    kernel.wake_poll(channel(socket), EPOLLOUT | EPOLLWRNORM);
}

/// The listener's end of a new connection to `address` on its port from `remote`, holding the
/// port and taking the listener's options (`tcp_create_openreq_child`).
pub(super) fn embryo(
    kernel: &mut Kernel,
    listener: &SocketRef,
    address: [u8; 16],
    remote: Endpoint,
) -> SocketRef {
    let embryo = {
        let them = listener.borrow();
        let listening = them.inet.as_deref().unwrap();
        let mut new = Socket::new(them.family, SOCK_STREAM, TCP);
        new.state = State::Connected;
        new.options = them.options.clone();
        new.owner_uid = them.owner_uid;
        new.receive_timeout = them.receive_timeout;
        new.send_timeout = them.send_timeout;
        new.sndbuf = if listening.sndbuf_set {
            them.sndbuf
        } else {
            TCP_CONNECTED_SNDBUF
        };
        new.rcvbuf = them.rcvbuf;
        let mut inet = Inet::new(Protocol::Tcp);
        inet.local = Endpoint {
            address,
            port: listening.local.port,
        };
        inet.address_chosen = true;
        inet.port_chosen = true;
        inet.held = true;
        inet.remote = Some(remote);
        inet.v6only = listening.v6only;
        inet.sndbuf_set = listening.sndbuf_set;
        inet.rcvbuf_set = listening.rcvbuf_set;
        new.inet = Some(alloc::boxed::Box::new(inet));
        Rc::new(RefCell::new(new))
    };
    let port = inet_ref(listener).local.port;
    kernel.sockets.inet.hold(Protocol::Tcp, port, &embryo);
    embryo
}

/// Whether connections from outside reach a listener: one bound to eth0's address, or to the
/// any address taking IPv4 (outside connections arrive through the gateway over IPv4).
fn reached_from_outside(own: &Socket) -> bool {
    let inet = own.inet.as_deref().unwrap();
    let address = inet.local.address;
    address == mapped(network::ADDRESS)
        || address == ANY4
        || (address == ANY6 && (own.family == AF_INET || !inet.v6only))
}

/// Whether a listener's backlog is full (`sk_acceptq_is_full`).
pub(super) fn backlog_full(listener: &Socket) -> bool {
    listener.backlog.len() > listener.max_backlog as usize
}

/// After an accept made room in a listener's backlog: the first connect waiting for it joins,
/// then connections waiting outside.
pub(super) fn accepted(kernel: &mut Kernel, cx: &mut Context<'_>, listener: &SocketRef) {
    loop {
        if backlog_full(&listener.borrow()) {
            return;
        }
        let Some(waiting) = inet(listener).waiting.pop_front() else {
            super::outside::take(kernel, cx, listener);
            return;
        };
        let Some(socket) = waiting.upgrade() else {
            continue;
        };
        if socket.borrow().dead || inet_ref(&socket).connecting.is_none() {
            continue;
        }
        join(kernel, &socket, listener);
    }
}

/// The connects still waiting on a listener that stops listening are refused.
fn refuse_waiting(kernel: &mut Kernel, listener: &SocketRef) {
    let waiting = core::mem::take(&mut inet(listener).waiting);
    for socket in waiting.iter().filter_map(Weak::upgrade) {
        if inet_ref(&socket).connecting.is_some() {
            refused(kernel, &socket);
        }
    }
}

fn udp_connect(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    endpoint: Endpoint,
) -> Result<u64, Errno> {
    let family = socket.borrow().family;
    if !family_takes(family, inet_ref(socket).v6only, &endpoint.address) {
        return Err(Errno::ENETUNREACH);
    }
    let to = destination(family, endpoint);
    if !reachable(&to.address) {
        return Err(Errno::ENETUNREACH);
    }
    if !inet_ref(socket).address_chosen && unicast(&to.address) {
        inet(socket).local.address = source_for(&to.address);
    }
    autobind(kernel, cx, tid, socket, true)?;
    inet(socket).remote = Some(to);
    let mut own = socket.borrow_mut();
    own.state = State::Connected;
    own.error = 0;
    Ok(0)
}

/// The local and remote addresses getsockname and getpeername give.
pub(super) fn name(socket: &SocketRef, peer: bool) -> Result<Vec<u8>, Errno> {
    let own = socket.borrow();
    let inet = own.inet.as_deref().unwrap();
    if peer {
        let remote = inet.remote.filter(|_| own.state == State::Connected);
        let remote = remote.ok_or(Errno::ENOTCONN)?;
        return Ok(address_bytes(own.family, &remote));
    }
    let mut local = inet.local;
    if local.address == [0; 16] && own.family == AF_INET {
        local.address = ANY4;
    }
    Ok(address_bytes(own.family, &local))
}

/// The address an accepted connection's other end is at.
pub(super) fn peer_bytes(socket: &SocketRef) -> Vec<u8> {
    let own = socket.borrow();
    let remote = own.inet.as_deref().unwrap().remote.unwrap_or_default();
    address_bytes(own.family, &remote)
}

/// `inet_shutdown` with `tcp_shutdown`: a TCP send shut sends a FIN.
pub(super) fn shutdown(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    socket: &SocketRef,
    mode: u8,
) -> Result<u64, Errno> {
    let (state, connecting, protocol) = {
        let own = socket.borrow();
        let inet = own.inet.as_deref().unwrap();
        (own.state, inet.connecting.is_some(), inet.protocol)
    };
    if connecting {
        disconnect_tcp(kernel, socket);
        return Ok(0);
    }
    match state {
        State::Listening => {
            if mode & RCV_SHUTDOWN != 0 {
                disconnect_tcp(kernel, socket);
            }
            Ok(0)
        }
        State::Closed => {
            socket.borrow_mut().shutdown |= mode;
            kernel.wake(channel(socket));
            Err(Errno::ENOTCONN)
        }
        State::Connected => {
            let already = socket.borrow().shutdown & SEND_SHUTDOWN != 0;
            socket.borrow_mut().shutdown |= mode;
            if protocol == Protocol::Tcp && mode & SEND_SHUTDOWN != 0 && !already {
                let outside = inet_ref(socket).outside;
                match outside {
                    Some(id) => super::outside::finish(cx, id),
                    None => send_fin(kernel, socket),
                }
            }
            kernel.wake(channel(socket));
            Ok(0)
        }
    }
}

/// A FIN to the other end of a connection: it reads the end after what was sent. Once both ends
/// have sent theirs the connection is closed, and the end that sent first holds its ports for
/// TIME_WAIT.
fn send_fin(kernel: &mut Kernel, socket: &SocketRef) {
    let Some(peer) = socket.borrow().peer.clone() else {
        return;
    };
    let other_done = inet_ref(socket).done;
    if !other_done {
        inet(socket).closed_first = true;
    }
    if !peer.borrow().dead {
        let mut other = peer.borrow_mut();
        other.shutdown |= RCV_SHUTDOWN;
        other.inet.as_deref_mut().unwrap().done = true;
    }
    if other_done {
        for end in [socket, &peer] {
            let mut own = end.borrow_mut();
            own.state = State::Closed;
            own.shutdown = SHUTDOWN_MASK;
        }
        let first = if inet_ref(socket).closed_first {
            socket
        } else {
            &peer
        };
        hold_time_wait(kernel, first);
    }
    kernel.wake_poll(channel(&peer), EPOLLIN | EPOLLRDNORM | EPOLLRDHUP);
}

/// A connection's end that closed first holds its port for TIME_WAIT.
fn hold_time_wait(kernel: &mut Kernel, socket: &SocketRef) {
    let (local, remote) = {
        let inet = inet_ref(socket);
        (inet.local, inet.remote.unwrap_or_default())
    };
    let held = TimeWait {
        family: socket.borrow().family,
        local,
        remote,
        reuse: flag(&socket.borrow(), SOL_SOCKET, SO_REUSEADDR),
        until: kernel.entry_now + TIME_WAIT,
    };
    kernel.sockets.inet.time_wait.push(held);
}

/// `tcp_poll` and `udp_poll`.
pub(super) fn poll(socket: &SocketRef) -> u16 {
    let own = socket.borrow();
    let inet = own.inet.as_deref().unwrap();
    let mut mask = 0u32;
    if inet.protocol == Protocol::Udp {
        if own.error != 0 {
            mask |= EPOLLERR;
        }
        if own.shutdown & RCV_SHUTDOWN != 0 {
            mask |= EPOLLRDHUP | EPOLLIN | EPOLLRDNORM;
        }
        if own.shutdown == SHUTDOWN_MASK {
            mask |= EPOLLHUP;
        }
        if !own.queue.is_empty() {
            mask |= EPOLLIN | EPOLLRDNORM;
        }
        if own.written << 1 < own.sndbuf {
            mask |= EPOLLOUT | EPOLLWRNORM | EPOLLWRBAND;
        }
        return mask as u16;
    }
    if own.state == State::Listening {
        return if own.backlog.is_empty() {
            0
        } else {
            (EPOLLIN | EPOLLRDNORM) as u16
        };
    }
    let connecting = inet.connecting.is_some();
    if own.shutdown == SHUTDOWN_MASK || (own.state == State::Closed && !connecting) {
        mask |= EPOLLHUP;
    }
    if own.shutdown & RCV_SHUTDOWN != 0 {
        mask |= EPOLLIN | EPOLLRDNORM | EPOLLRDHUP;
    }
    if !connecting {
        if !own.queue.is_empty() {
            mask |= EPOLLIN | EPOLLRDNORM;
        }
        let room = match inet.outside {
            Some(_) => inet.room,
            None => writable(&own),
        };
        if own.shutdown & SEND_SHUTDOWN != 0 || room {
            mask |= EPOLLOUT | EPOLLWRNORM;
        }
    }
    if own.error != 0 {
        mask |= EPOLLERR;
    }
    mask as u16
}

/// `__sk_stream_is_writeable`: the free room is at least half of what is held.
fn writable(own: &Socket) -> bool {
    let held = own.written;
    own.sndbuf.saturating_sub(held) >= held / 2
}

/// The last descriptor went: a connection is reset if bytes were left unread (or SO_LINGER asks
/// for no wait), else the other end gets a FIN; a listener's waiting connections are reset; the
/// ports are let go.
pub(super) fn released(kernel: &mut Kernel, socket: &SocketRef) {
    super::outside::released(kernel, socket);
    let (state, protocol) = {
        let own = socket.borrow();
        (own.state, own.inet.as_deref().unwrap().protocol)
    };
    if protocol == Protocol::Tcp {
        match state {
            State::Listening => {
                let backlog = core::mem::take(&mut socket.borrow_mut().backlog);
                for embryo in backlog {
                    reset_peer(kernel, &embryo);
                    super::released(kernel, &embryo);
                }
                refuse_waiting(kernel, socket);
            }
            State::Connected if inet_ref(socket).outside.is_some() => {}
            State::Connected => {
                let linger_zero = socket
                    .borrow()
                    .options
                    .get(&(SOL_SOCKET, SO_LINGER))
                    .is_some_and(|value| value[..4] != [0; 4] && value[4..8] == [0; 4]);
                if !socket.borrow().queue.is_empty() || linger_zero {
                    reset_peer(kernel, socket);
                } else if socket.borrow().shutdown & SEND_SHUTDOWN == 0 {
                    socket.borrow_mut().shutdown |= SEND_SHUTDOWN;
                    send_fin(kernel, socket);
                } else if !inet_ref(socket).done && inet_ref(socket).closed_first {
                    // Its FIN went and the other end's has not come: it holds its ports.
                    hold_time_wait(kernel, socket);
                }
            }
            State::Closed => {}
        }
    }
    let (port, peer, queue) = {
        let mut own = socket.borrow_mut();
        own.dead = true;
        own.state = State::Closed;
        own.shutdown = SHUTDOWN_MASK;
        let queue = core::mem::take(&mut own.queue);
        let peer = own.peer.take();
        let inet = own.inet.as_deref_mut().unwrap();
        inet.connecting = None;
        (inet.held.then_some(inet.local.port), peer, queue)
    };
    if let Some(port) = port {
        kernel.sockets.inet.let_go(protocol, port, socket);
    }
    if let Some(peer) = peer {
        kernel.wake(channel(&peer));
    }
    for packet in queue {
        super::drop_packet(kernel, packet);
    }
    kernel.wake(channel(socket));
}

/// `tcp_sendmsg` and `udp_sendmsg`.
pub(super) fn send(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    message: &Outgoing,
    total: u64,
    nonblocking: bool,
) -> Result<u64, Errno> {
    if !message.files.is_empty() {
        return Err(Errno::EINVAL);
    }
    let protocol = inet_ref(socket).protocol;
    match protocol {
        Protocol::Tcp => {
            if message.flags & MSG_OOB != 0 {
                return Err(kernel.counters.refuse_flag(
                    cx.host,
                    __NR_sendmsg,
                    MSG_OOB.into(),
                    Errno::EOPNOTSUPP,
                ));
            }
            tcp_send(kernel, cx, tid, socket, message, total, nonblocking)
        }
        Protocol::Udp => {
            let to = message.address.as_deref().filter(|bytes| !bytes.is_empty());
            udp_send(kernel, cx, tid, socket, message, to, total)
        }
    }
}

fn tcp_send(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    message: &Outgoing,
    total: u64,
    nonblocking: bool,
) -> Result<u64, Errno> {
    let mut sent = kernel.threads[&tid].call.progress;
    loop {
        {
            let mut own = socket.borrow_mut();
            if own.error != 0 && sent == 0 {
                let error = core::mem::take(&mut own.error);
                return Err(Errno(error as u32));
            }
            if own.shutdown & SEND_SHUTDOWN != 0 {
                return if sent > 0 {
                    Ok(sent)
                } else {
                    Err(Errno::EPIPE)
                };
            }
        }
        if inet_ref(socket).connecting.is_some() {
            if nonblocking {
                return Err(Errno::EAGAIN);
            }
            let timeout = socket.borrow().send_timeout;
            return Err(wait_room(kernel, tid, socket, false, timeout));
        }
        let outside = inet_ref(socket).outside;
        if let Some(id) = outside {
            if sent >= total {
                return Ok(sent);
            }
            if socket.borrow().state != State::Connected {
                return Err(Errno::EPIPE);
            }
            if !inet_ref(socket).room {
                if sent > 0 && (nonblocking || kernel.signal_pending(tid)) {
                    return Ok(sent);
                }
                let timeout = socket.borrow().send_timeout;
                let error = wait_room(kernel, tid, socket, nonblocking, timeout);
                if error == Errno::PARKED {
                    kernel.thread_mut(tid).call.progress = sent;
                }
                return Err(error);
            }
            let size = (total - sent).min(TCP_PIECE);
            let data = gather(kernel, cx, tid, message.data, sent, size)?;
            match super::outside::send(kernel, cx, socket, id, &data) {
                Ok(count) => sent += count as u64,
                Err(_) if sent > 0 => return Ok(sent),
                Err(error) => return Err(error),
            }
            continue;
        }
        let Some(peer) = socket.borrow().peer.clone() else {
            return Err(Errno::EPIPE);
        };
        if socket.borrow().state != State::Connected {
            return Err(Errno::EPIPE);
        }
        if peer.borrow().dead {
            // The other end is gone: what is sent draws its reset, which ends the connection.
            reset(kernel, socket);
            let rest = total - sent;
            return Ok(sent + rest);
        }
        if sent >= total {
            return Ok(sent);
        }
        let (sndbuf, written, timeout) = {
            let own = socket.borrow();
            (own.sndbuf, own.written, own.send_timeout)
        };
        if written >= sndbuf {
            kernel
                .counters
                .count(cx.host, crate::reason::TCP_ROOM_MODELLED, 0);
            if sent > 0 && (nonblocking || kernel.signal_pending(tid)) {
                return Ok(sent);
            }
            let error = wait_room(kernel, tid, socket, nonblocking, timeout);
            if error == Errno::PARKED {
                kernel.thread_mut(tid).call.progress = sent;
            }
            return Err(error);
        }
        let size = (total - sent).min(sndbuf - written).min(TCP_PIECE);
        let data = gather(kernel, cx, tid, message.data, sent, size)?;
        let packet = Packet {
            data,
            consumed: 0,
            files: Vec::new(),
            cred: super::own_cred(kernel, tid),
            sender: Rc::downgrade(socket),
            truesize: size,
            source: None,
        };
        socket.borrow_mut().written += size;
        {
            let mut own = inet(socket);
            own.pingpong |= own.received;
        }
        inet(&peer).received = true;
        peer.borrow_mut().queue.push_back(packet);
        kernel.wake_poll(channel(&peer), EPOLLIN | EPOLLRDNORM);
        sent += size;
    }
}

/// The true size Linux charges a receiver for a datagram of `size` bytes (measured): its buffer
/// with the IP and UDP headers and the link's room (`ip_append_data`), the head from the small
/// head cache when it fits (`kmalloc_reserve`), and past `SKB_MAX_ALLOC` the payload in pages,
/// counted exactly.
pub(super) fn datagram_truesize(size: u64) -> u64 {
    const SHARED_INFO: u64 = 320;
    const SK_BUFF: u64 = 256;
    const SMALL_HEAD: u64 = 640;
    const MAX_ALLOC: u64 = (4096 << 2) - SHARED_INFO;
    let head = size + 28 + 31;
    if head >= MAX_ALLOC {
        return SMALL_HEAD + SK_BUFF + size;
    }
    let wanted = head.next_multiple_of(64) + SHARED_INFO;
    let allocated = if wanted <= SMALL_HEAD {
        SMALL_HEAD
    } else {
        wanted.next_power_of_two()
    };
    allocated + SK_BUFF
}

fn udp_send(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    message: &Outgoing,
    to: Option<&[u8]>,
    total: u64,
) -> Result<u64, Errno> {
    let family = socket.borrow().family;
    let named = match to {
        Some(bytes) if family == AF_INET => {
            if bytes.len() < 16 {
                return Err(Errno::EINVAL);
            }
            match u16::from_le_bytes([bytes[0], bytes[1]]) as u32 {
                AF_INET | AF_UNSPEC => {}
                _ => return Err(Errno::EAFNOSUPPORT),
            }
            let endpoint = Endpoint {
                address: mapped([bytes[4], bytes[5], bytes[6], bytes[7]]),
                port: u16::from_be_bytes([bytes[2], bytes[3]]),
            };
            if endpoint.port == 0 {
                return Err(Errno::EINVAL);
            }
            Some(endpoint)
        }
        Some(bytes) => {
            if bytes.len() < 2 {
                return Err(Errno::EINVAL);
            }
            match u16::from_le_bytes([bytes[0], bytes[1]]) as u32 {
                AF_INET6 => match read_endpoint(AF_INET6, bytes)? {
                    Named::Endpoint(endpoint) => {
                        if endpoint.port == 0 {
                            return Err(Errno::EINVAL);
                        }
                        if v4_of(&endpoint.address).is_some() && inet_ref(socket).v6only {
                            return Err(Errno::ENETUNREACH);
                        }
                        Some(endpoint)
                    }
                    Named::Unspecified => None,
                },
                AF_UNSPEC => None,
                _ => return Err(Errno::EINVAL),
            }
        }
        None => None,
    };
    let to = match named {
        Some(endpoint) => destination(family, endpoint),
        None => {
            let own = socket.borrow();
            if own.state != State::Connected {
                return Err(Errno::EDESTADDRREQ);
            }
            own.inet.as_deref().unwrap().remote.expect("a destination")
        }
    };
    {
        let mut own = socket.borrow_mut();
        if own.error != 0 {
            let error = core::mem::take(&mut own.error);
            return Err(Errno(error as u32));
        }
        if own.shutdown & SEND_SHUTDOWN != 0 {
            return Err(Errno::EPIPE);
        }
    }
    let largest = if v4_of(&to.address).is_some() {
        UDP_MAX4
    } else {
        UDP_MAX6
    };
    if total > largest {
        return Err(Errno::EMSGSIZE);
    }
    if !reachable(&to.address) {
        return Err(Errno::ENETUNREACH);
    }
    autobind(kernel, cx, tid, socket, false)?;
    let mut from = inet_ref(socket).local;
    if is_any(&from.address) {
        from.address = source_for(&to.address);
    }
    if family == AF_INET6 && v4_of(&to.address).is_none() && v4_of(&from.address).is_some() {
        from.address = LOOPBACK6;
    }
    let data = gather(kernel, cx, tid, message.data, 0, total)?;
    if !is_own(&to.address) {
        if !unicast(&to.address) {
            return Ok(total);
        }
        super::outside::send_datagram(kernel, cx, socket, to, &data)?;
        return Ok(total);
    }
    let receiver = find_receiver(kernel, &to, &from);
    let name_server = Endpoint {
        address: mapped(network::NAME_SERVER),
        port: 53,
    };
    if receiver.is_none() && to == name_server {
        super::outside::query(kernel, cx, socket, name_server, &data);
        return Ok(total);
    }
    match receiver {
        Some(receiver) => {
            let truesize = datagram_truesize(total);
            let held: u64 = receiver
                .borrow()
                .queue
                .iter()
                .map(|packet| packet.truesize)
                .sum();
            if held <= receiver.borrow().rcvbuf {
                let packet = Packet {
                    data,
                    consumed: 0,
                    files: Vec::new(),
                    cred: super::own_cred(kernel, tid),
                    sender: Weak::new(),
                    truesize,
                    source: Some(from),
                };
                receiver.borrow_mut().queue.push_back(packet);
                kernel.wake_poll(channel(&receiver), EPOLLIN | EPOLLRDNORM);
            }
        }
        None if unicast(&to.address) => {
            // Port unreachable: a connected sender hears of it (`__udp4_lib_err`).
            let connected_to = {
                let own = socket.borrow();
                (own.state == State::Connected)
                    .then(|| own.inet.as_deref().unwrap().remote)
                    .flatten()
            };
            if connected_to == Some(to) {
                socket.borrow_mut().error = Errno::ECONNREFUSED.0 as i32;
                kernel.wake_poll(channel(socket), EPOLLERR);
            }
        }
        None => {}
    }
    Ok(total)
}

/// The UDP socket a datagram to `to` from `from` goes to: the most specific match, a connected
/// socket only from its destination (`compute_score`).
fn find_receiver(kernel: &Kernel, to: &Endpoint, from: &Endpoint) -> Option<SocketRef> {
    let mut best: Option<(u8, SocketRef)> = None;
    for socket in kernel.sockets.inet.holders(Protocol::Udp, to.port) {
        let score = {
            let own = socket.borrow();
            let inet = own.inet.as_deref().unwrap();
            if !family_takes(own.family, inet.v6only, &to.address) {
                continue;
            }
            let mut score = u8::from(own.family == AF_INET);
            if inet.local.address == to.address {
                score += 4;
            } else if !(is_any(&inet.local.address)
                && (inet.local.address == ANY6 || v4_of(&to.address).is_some()))
            {
                continue;
            }
            if let Some(remote) = inet.remote.filter(|_| own.state == State::Connected) {
                if remote != *from {
                    continue;
                }
                score += 4;
            }
            score
        };
        if best.as_ref().is_none_or(|(top, _)| score > *top) {
            best = Some((score, socket));
        }
    }
    best.map(|(_, socket)| socket)
}

/// `tcp_recvmsg` and `udp_recvmsg`.
pub(super) fn receive(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    parts: super::Data<'_>,
    flags: u32,
    nonblocking: bool,
) -> Result<(u64, Received), Errno> {
    let size = parts.size();
    if inet_ref(socket).protocol == Protocol::Udp {
        return udp_receive(kernel, cx, tid, socket, parts, size, flags, nonblocking);
    }
    if flags & MSG_OOB != 0 {
        return Err(kernel.counters.refuse_flag(
            cx.host,
            __NR_recvmsg,
            MSG_OOB.into(),
            Errno::EOPNOTSUPP,
        ));
    }
    {
        let own = socket.borrow();
        if own.state == State::Listening {
            return Err(Errno::ENOTCONN);
        }
        let connecting = own.inet.as_deref().unwrap().connecting.is_some();
        if own.state == State::Closed && !connecting && own.queue.is_empty() {
            if own.error != 0 {
                drop(own);
                let error = core::mem::take(&mut socket.borrow_mut().error);
                return Err(Errno(error as u32));
            }
            if own.shutdown & RCV_SHUTDOWN != 0 {
                return Ok((0, Received::default()));
            }
            return Err(Errno::ENOTCONN);
        }
    }
    let outside = inet_ref(socket).outside;
    if let Some(id) = outside {
        super::outside::pull(kernel, cx, socket, id);
    }
    let result = super::receive_stream(kernel, cx, tid, socket, parts, size, flags, nonblocking);
    if let Some(id) = outside {
        super::outside::pull(kernel, cx, socket, id);
    }
    let (copied, mut received) = result?;
    // A read of the end moves past the FIN (`tcp_recvmsg`'s `++*seq`).
    if copied == 0 && size > 0 && flags & MSG_PEEK == 0 && inet_ref(socket).done {
        inet(socket).fin_read = true;
    }
    received.name = None;
    received.cred = None;
    Ok((copied, received))
}

#[allow(
    clippy::too_many_arguments,
    reason = "a read names where its bytes go and how"
)]
fn udp_receive(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    tid: Tid,
    socket: &SocketRef,
    parts: super::Data<'_>,
    size: u64,
    flags: u32,
    nonblocking: bool,
) -> Result<(u64, Received), Errno> {
    if flags & MSG_OOB != 0 {
        return Err(Errno::EOPNOTSUPP);
    }
    {
        let mut own = socket.borrow_mut();
        if own.error != 0 {
            let error = core::mem::take(&mut own.error);
            return Err(Errno(error as u32));
        }
        if own.queue.is_empty() {
            if own.shutdown & RCV_SHUTDOWN != 0 {
                return Ok((0, Received::default()));
            }
            let timeout = own.receive_timeout;
            drop(own);
            return Err(wait_room(kernel, tid, socket, nonblocking, timeout));
        }
    }
    let (data, source) = {
        let own = socket.borrow();
        let packet = own.queue.front().expect("a datagram");
        (packet.data.clone(), packet.source)
    };
    let copied = (data.len() as u64).min(size);
    scatter(kernel, cx, tid, parts, &data[..copied as usize])?;
    let family = socket.borrow().family;
    let mut received = Received {
        address: source.map(|from| address_bytes(family, &from)),
        ..Received::default()
    };
    if data.len() as u64 > size {
        received.flags |= MSG_TRUNC;
    }
    if flags & MSG_TRUNC != 0 {
        received.reported = Some(data.len() as u64);
    }
    if flags & MSG_PEEK == 0 {
        socket.borrow_mut().queue.pop_front();
    }
    Ok((copied, received))
}

/// One integer option's value as a call gave it: at least four bytes (or one for the IP level's
/// small values).
fn int_value(bytes: &[u8], level: u32) -> Result<i32, Errno> {
    if bytes.len() >= 4 {
        return Ok(i32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")));
    }
    if level == SOL_IP && !bytes.is_empty() {
        return Ok(i32::from(bytes[0]));
    }
    Err(Errno::EINVAL)
}

/// setsockopt for an internet socket's own levels, and the SOL_SOCKET options whose meaning is
/// its own. Returns `None` for an option the common code answers.
pub(super) fn set_option(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    socket: &SocketRef,
    level: u32,
    name: u32,
    bytes: &[u8],
) -> Option<Result<u64, Errno>> {
    let family = socket.borrow().family;
    let protocol = inet_ref(socket).protocol;
    let not_built = |kernel: &mut Kernel, cx: &mut Context<'_>| {
        Some(Err(kernel.counters.refuse(
            cx.host,
            crate::reason::OPTION_NOT_BUILT,
            (u64::from(level) << 32) | u64::from(name),
            Errno::ENOPROTOOPT,
        )))
    };
    let store = |socket: &SocketRef, value: i32| {
        socket
            .borrow_mut()
            .options
            .insert((level, name), value.to_le_bytes().to_vec());
        Some(Ok(0))
    };
    match level {
        SOL_SOCKET => match name {
            SO_SNDBUF | SO_RCVBUF => {
                let value = match int_value(bytes, level) {
                    Ok(value) => value,
                    Err(error) => return Some(Err(error)),
                };
                let wanted = (value as u32 as u64).min(super::MAX_BUFFER) * 2;
                let mut own = socket.borrow_mut();
                if name == SO_SNDBUF {
                    own.sndbuf = wanted.max(super::MIN_SNDBUF);
                    own.inet.as_deref_mut().unwrap().sndbuf_set = true;
                } else {
                    own.rcvbuf = wanted.max(super::MIN_RCVBUF);
                    own.inet.as_deref_mut().unwrap().rcvbuf_set = true;
                }
                drop(own);
                kernel.wake_poll(channel(socket), EPOLLOUT | EPOLLWRNORM | EPOLLWRBAND);
                Some(Ok(0))
            }
            SO_BINDTODEVICE => {
                let name_bytes =
                    &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())];
                if !name_bytes.is_empty()
                    && name_bytes != network::LOOPBACK_NAME.as_bytes()
                    && name_bytes != network::ETHERNET_NAME.as_bytes()
                {
                    return Some(Err(Errno::ENODEV));
                }
                socket
                    .borrow_mut()
                    .options
                    .insert((level, name), name_bytes.to_vec());
                Some(Ok(0))
            }
            _ => None,
        },
        SOL_TCP if protocol == Protocol::Tcp => {
            if bytes.len() < 4 && name != TCP_CONGESTION {
                return Some(Err(Errno::EINVAL));
            }
            let value = int_value(bytes, level).unwrap_or(0);
            match name {
                TCP_NODELAY
                | TCP_CORK
                | TCP_QUICKACK
                | TCP_THIN_LINEAR_TIMEOUTS
                | TCP_FASTOPEN_CONNECT
                | TCP_FASTOPEN_NO_COOKIE
                | TCP_INQ => store(socket, i32::from(value != 0)),
                TCP_KEEPIDLE | TCP_KEEPINTVL => {
                    if !(1..=32767).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, value)
                }
                TCP_KEEPCNT | TCP_SYNCNT => {
                    if !(1..=127).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, value)
                }
                TCP_MAXSEG => {
                    if value != 0 && !(88..=32767).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, value)
                }
                TCP_USER_TIMEOUT | TCP_NOTSENT_LOWAT | TCP_FASTOPEN => {
                    if value < 0 {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, value)
                }
                TCP_LINGER2 => store(socket, value.clamp(-1, 120)),
                TCP_WINDOW_CLAMP => {
                    if value != 0 && value < (super::MIN_RCVBUF / 2) as i32 {
                        return store(socket, (super::MIN_RCVBUF / 2) as i32);
                    }
                    store(socket, value)
                }
                TCP_CONGESTION => {
                    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                    if &bytes[..end] == b"cubic" || &bytes[..end] == b"reno" {
                        socket
                            .borrow_mut()
                            .options
                            .insert((level, name), bytes[..end].to_vec());
                        Some(Ok(0))
                    } else {
                        Some(Err(Errno::ENOENT))
                    }
                }
                _ => not_built(kernel, cx),
            }
        }
        SOL_IP => {
            if name == IP_HDRINCL {
                return Some(Err(Errno::ENOPROTOOPT));
            }
            let value = match int_value(bytes, level) {
                Ok(value) => value,
                Err(error) => return Some(Err(error)),
            };
            match name {
                IP_TTL => {
                    if value != -1 && !(1..=255).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, value)
                }
                IP_TOS => {
                    let value = if protocol == Protocol::Tcp {
                        value & !3
                    } else {
                        value
                    };
                    store(socket, value & 0xff)
                }
                IP_MULTICAST_TTL => {
                    let value = if value == -1 { 1 } else { value };
                    if !(0..=255).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, value)
                }
                IP_MTU_DISCOVER => {
                    if !(0..=5).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, value)
                }
                IP_RECVERR
                | IP_FREEBIND
                | IP_TRANSPARENT
                | IP_BIND_ADDRESS_NO_PORT
                | IP_MULTICAST_LOOP
                | IP_MULTICAST_ALL
                | IP_NODEFRAG => store(socket, i32::from(value != 0)),
                _ => not_built(kernel, cx),
            }
        }
        SOL_IPV6 if family == AF_INET6 => {
            let value = match int_value(bytes, level) {
                Ok(value) => value,
                Err(error) => return Some(Err(error)),
            };
            match name {
                IPV6_V6ONLY => {
                    if inet_ref(socket).held {
                        return Some(Err(Errno::EINVAL));
                    }
                    inet(socket).v6only = value != 0;
                    Some(Ok(0))
                }
                IPV6_UNICAST_HOPS => {
                    if !(-1..=255).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, value)
                }
                IPV6_MULTICAST_HOPS => {
                    if !(-1..=255).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, if value == -1 { 1 } else { value })
                }
                IPV6_TCLASS => {
                    if !(-1..=255).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, value.max(0))
                }
                IPV6_MTU_DISCOVER => {
                    if !(0..=5).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, value)
                }
                IPV6_RECVERR | IPV6_MULTICAST_LOOP | IPV6_FREEBIND | IPV6_TRANSPARENT
                | IPV6_DONTFRAG => store(socket, i32::from(value != 0)),
                _ => not_built(kernel, cx),
            }
        }
        _ => Some(Err(Errno::ENOPROTOOPT)),
    }
}

/// getsockopt for an internet socket's own levels and its own SOL_SOCKET options; `None` for an
/// option the common code answers.
pub(super) fn get_option(
    kernel: &mut Kernel,
    cx: &mut Context<'_>,
    socket: &SocketRef,
    level: u32,
    name: u32,
) -> Option<Result<Vec<u8>, Errno>> {
    let family = socket.borrow().family;
    let protocol = inet_ref(socket).protocol;
    let connected = socket.borrow().state == State::Connected;
    let int = |value: i32| Some(Ok(value.to_le_bytes().to_vec()));
    let kept = |default: i32| {
        let own = socket.borrow();
        Some(Ok(own
            .options
            .get(&(level, name))
            .cloned()
            .unwrap_or_else(|| default.to_le_bytes().to_vec())))
    };
    let not_built = |kernel: &mut Kernel, cx: &mut Context<'_>| {
        Some(Err(kernel.counters.refuse(
            cx.host,
            crate::reason::OPTION_NOT_BUILT,
            (u64::from(level) << 32) | u64::from(name),
            Errno::ENOPROTOOPT,
        )))
    };
    match level {
        SOL_SOCKET => match name {
            SO_BINDTODEVICE => kept(0).map(|bytes| bytes.map(|_| Vec::new())),
            SO_PEERNAME => {
                let own = socket.borrow();
                let remote = own.inet.as_deref().unwrap().remote;
                Some(
                    remote
                        .filter(|_| own.state == State::Connected)
                        .map(|remote| address_bytes(family, &remote))
                        .ok_or(Errno::ENOTCONN),
                )
            }
            _ => None,
        },
        SOL_TCP if protocol == Protocol::Tcp => match name {
            TCP_NODELAY
            | TCP_CORK
            | TCP_THIN_LINEAR_TIMEOUTS
            | TCP_FASTOPEN_CONNECT
            | TCP_FASTOPEN_NO_COOKIE
            | TCP_INQ
            | TCP_USER_TIMEOUT
            | TCP_NOTSENT_LOWAT
            | TCP_FASTOPEN
            | TCP_WINDOW_CLAMP
            | TCP_DEFER_ACCEPT => kept(0),
            TCP_QUICKACK => kept(1),
            TCP_KEEPIDLE => kept(7200),
            TCP_KEEPINTVL => kept(75),
            TCP_KEEPCNT => kept(9),
            TCP_SYNCNT => kept(6),
            TCP_LINGER2 => kept(60),
            TCP_MAXSEG => {
                let set = socket
                    .borrow()
                    .options
                    .get(&(level, name))
                    .map(|bytes| i32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")))
                    .filter(|&value| value != 0);
                int(match (connected, set) {
                    (true, Some(value)) => value.min(TCP_MSS_LOOPBACK),
                    (true, None) => TCP_MSS_LOOPBACK,
                    (false, Some(value)) => value,
                    (false, None) => TCP_MSS_UNCONNECTED,
                })
            }
            TCP_CONGESTION => {
                let own = socket.borrow();
                let mut name = own
                    .options
                    .get(&(level, name))
                    .cloned()
                    .unwrap_or_else(|| b"cubic".to_vec());
                name.resize(16, 0);
                Some(Ok(name))
            }
            _ => not_built(kernel, cx),
        },
        SOL_IP => match name {
            IP_TTL => {
                let ttl = kept(-1)?.ok()?;
                let ttl = i32::from_le_bytes(ttl[..4].try_into().expect("4 bytes"));
                int(if ttl == -1 { 64 } else { ttl })
            }
            IP_TOS
            | IP_RECVERR
            | IP_FREEBIND
            | IP_TRANSPARENT
            | IP_BIND_ADDRESS_NO_PORT
            | IP_NODEFRAG => kept(0),
            IP_MULTICAST_TTL | IP_MULTICAST_LOOP | IP_MULTICAST_ALL => kept(1),
            IP_MTU_DISCOVER => kept(IP_PMTUDISC_WANT as i32),
            IP_MTU => {
                if !connected {
                    return Some(Err(Errno::ENOTCONN));
                }
                int(ROUTE_MTU)
            }
            IP_OPTIONS => Some(Ok(Vec::new())),
            IP_HDRINCL => kept(0),
            _ => not_built(kernel, cx),
        },
        SOL_IPV6 if family == AF_INET6 => match name {
            IPV6_V6ONLY => int(i32::from(inet_ref(socket).v6only)),
            IPV6_UNICAST_HOPS => {
                let hops = kept(-1)?.ok()?;
                let hops = i32::from_le_bytes(hops[..4].try_into().expect("4 bytes"));
                int(if hops == -1 { 64 } else { hops })
            }
            IPV6_MULTICAST_HOPS | IPV6_MULTICAST_LOOP => kept(1),
            IPV6_TCLASS | IPV6_RECVERR | IPV6_FREEBIND | IPV6_TRANSPARENT | IPV6_DONTFRAG => {
                kept(0)
            }
            IPV6_MTU_DISCOVER => kept(IPV6_PMTUDISC_WANT as i32),
            IPV6_MTU => {
                if !connected {
                    return Some(Err(Errno::ENOTCONN));
                }
                int(ROUTE_MTU)
            }
            _ => not_built(kernel, cx),
        },
        _ => Some(Err(Errno::EOPNOTSUPP)),
    }
}
