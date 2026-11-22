//! Internet sockets (AF_INET and AF_INET6) as `net/ipv4` and `net/ipv6` show them to programs.
//! TCP and UDP between the container's own addresses never leave the kernel: a TCP connection is
//! two sockets joined as local streams are, and a UDP datagram goes straight to the socket bound
//! to its destination.

use alloc::collections::VecDeque;
use alloc::rc::{Rc, Weak};
use alloc::vec::Vec;
use core::cell::RefCell;
use wcontainers_abi::NANOSECONDS_PER_SECOND;

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
/// (`tcp_sndbuf_expand` for its MSS), and the MSS before connecting (all measured).
const TCP_SNDBUF: u64 = 16384;
const TCP_RCVBUF: u64 = 131072;
/// `tcp_rmem`'s largest (measured).
const TCP_RMEM_MAX: u64 = 33_554_432;
const TCP_CONNECTED_SNDBUF: u64 = 2_626_560;
const TCP_MSS_UNCONNECTED: i32 = 536;
/// The MSS while a SYN waits for its answer: 536 less the timestamp option (measured).
const TCP_MSS_SYN_SENT: u32 = 524;
/// What `IP_MTU` gives on a connection inside the container (measured): the loopback's, at most
/// IP_MAX_MTU.
const ROUTE_MTU: i32 = 65535;
/// The same for IPv6, whose cap (IP6_MAX_MTU) is past the loopback's 65536 (measured).
const ROUTE_MTU6: i32 = 65536;
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
    /// For a listener with TCP_DEFER_ACCEPT: its connections held back until data comes
    /// (requests acknowledged but not yet established, `inet_rsk(req)->acked`), oldest first.
    pub deferred: Vec<SocketRef>,
    /// For a connection held back: its listener, and when it was made and when its deferral ends,
    /// on the monotonic clock.
    pub deferral: Option<(Weak<RefCell<Socket>>, u64, u64)>,
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
    /// Errors queued under IP_RECVERR or IPV6_RECVERR, first first (`sk_error_queue`).
    pub errors: VecDeque<QueuedError>,
    /// A UDP datagram being corked.
    pub cork: Option<Corked>,
    /// TCP bytes sent with MSG_MORE or under TCP_CORK and held back, short of a whole segment,
    /// until a send that pushes them, the cork's removal, TCP_NODELAY, the probe timer or the
    /// connection's end (`TCP_NAGLE_CORK`).
    corked: Option<Packet>,
    /// The probe timer that sends the held bytes if nothing pushes them first (`ICSK_TIME_PROBE0`).
    probe: Option<u64>,
    /// What TCP_INFO tells of a connection's traffic.
    pub traffic: Traffic,
    /// The IPv4 multicast groups it joined, each on an interface (`mc_list`).
    pub groups: Vec<([u8; 4], u32)>,
}

/// A UDP datagram's port unreachable error, as `ip_icmp_error` and `ipv6_icmp_error` queue it.
pub(crate) struct QueuedError {
    /// The datagram's destination, which the read names and whose host sent the error.
    to: Endpoint,
    /// The datagram's source, where the error came.
    from: [u8; 16],
    /// What the error message carried of the datagram.
    data: Vec<u8>,
    truesize: u64,
    time: Option<crate::fs::Time>,
    /// For a send timestamp (`__skb_tstamp_tx`) instead of an ICMP error: which (SCM_TSTAMP_SND
    /// or SCM_TSTAMP_SCHED) and its key.
    stamp: Option<(u32, u32)>,
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
            deferred: Vec::new(),
            deferral: None,
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
            errors: VecDeque::new(),
            cork: None,
            corked: None,
            probe: None,
            traffic: Traffic::default(),
            groups: Vec::new(),
        }
    }
}

/// A TCP connection's traffic as TCP_INFO tells it, in this TCP's model: every segment arrives
/// at once, a send goes in segments of the sender's MSS, and each data segment and FIN draws one
/// acknowledgement at once. The handshake's numbers are Linux's on loopback (measured).
#[derive(Clone, Copy, Default)]
pub(crate) struct Traffic {
    /// It was connected, and whether it made the connection (sent the SYN) or was accepted.
    made: bool,
    connector: bool,
    /// A segment came after the handshake, which shows a connector the other end's full window.
    heard: bool,
    sent_fin: bool,
    /// When it last sent data, last received data, and last had a segment acknowledged, on the
    /// monotonic clock.
    sent_at: u64,
    received_at: u64,
    acked_at: u64,
    bytes_sent: u64,
    bytes_acked: u64,
    bytes_received: u64,
    segs_out: u32,
    segs_in: u32,
    data_segs_out: u32,
    data_segs_in: u32,
    delivered: u32,
    /// The largest data segment received.
    largest: u32,
}

/// The MSS a connection advertises (`tcp_mss_clamp` of the route's MTU less the headers and
/// timestamps), the window the other end offers past the handshake, and the route's MTU, on
/// loopback (measured).
const TCP_ADVMSS4: u32 = 65483;
const TCP_ADVMSS6: u32 = 65464;
const TCP_WINDOW: u32 = 65536;
const PMTU4: u32 = 65535;
const PMTU6: u32 = 65536;
/// The model's smoothed round trip and its variation, in microseconds.
const MODEL_RTT: u32 = 10;
const MODEL_RTTVAR: u32 = 5;
/// The size of Linux 7.0's `struct tcp_info` (measured), whose accurate ECN counters past
/// linux-raw-sys's layout stay zero: ECN is never used here.
const TCP_INFO_SIZE: usize = 280;
const _: () = assert!(core::mem::size_of::<tcp_info>() == 248);

impl Traffic {
    /// A connection just made: the connector sent the SYN and the ACK and had the SYN-ACK
    /// acknowledged; the accepted end had the SYN and the ACK.
    fn made(connector: bool, now: u64) -> Self {
        Self {
            made: true,
            connector,
            sent_at: now,
            received_at: now,
            acked_at: now,
            bytes_acked: u64::from(connector),
            segs_out: if connector { 2 } else { 0 },
            segs_in: if connector { 1 } else { 2 },
            delivered: 1,
            ..Self::default()
        }
    }
}

fn advmss(family: u32) -> u32 {
    if family == AF_INET6 {
        TCP_ADVMSS6
    } else {
        TCP_ADVMSS4
    }
}

/// The MSS a connection sends with (`mss_cache`): half the largest window the other end has
/// offered (`tcp_bound_to_half_wnd`), which for a connector is that end's MSS until a segment
/// after the handshake offers the full window.
fn send_mss(family: u32, traffic: &Traffic) -> u32 {
    if traffic.connector && !traffic.heard {
        advmss(family) / 2
    } else {
        TCP_WINDOW / 2
    }
}

/// Data (`size` bytes) or a FIN going from one end of a connection to the other, either of
/// which may be outside the container (`None`).
fn carry(
    from: Option<&mut Traffic>,
    to: Option<&mut Traffic>,
    mss: u32,
    size: u64,
    fin: bool,
    now: u64,
) {
    let segments = if fin {
        1
    } else {
        size.div_ceil(u64::from(mss)) as u32
    };
    let data = u32::from(!fin) * segments;
    if let Some(from) = from {
        from.bytes_sent += size;
        from.bytes_acked += size + u64::from(fin);
        from.segs_out += segments;
        from.data_segs_out += data;
        from.segs_in += segments;
        from.delivered += segments;
        from.heard = true;
        from.acked_at = now;
        if fin {
            from.sent_fin = true;
        } else {
            from.sent_at = now;
        }
    }
    if let Some(to) = to {
        to.bytes_received += size + u64::from(fin);
        to.segs_in += segments;
        to.data_segs_in += data;
        to.segs_out += segments;
        to.heard = true;
        to.acked_at = now;
        if !fin {
            to.received_at = now;
            to.largest = to.largest.max(size.min(u64::from(mss)) as u32);
        }
    }
}

/// Data or a FIN between two of the container's own connected sockets.
fn carry_between(kernel: &mut Kernel, from: &SocketRef, to: &SocketRef, size: u64, fin: bool) {
    if let Some(remote) = inet_ref(from).remote {
        neighbour(kernel, &remote.address, true, true);
    }
    let family = from.borrow().family;
    let mss = send_mss(family, &inet_ref(from).traffic);
    let now = kernel.entry_now;
    // A socket connected to itself is both ends.
    if Rc::ptr_eq(from, to) {
        let traffic = &mut inet(from).traffic;
        carry(Some(traffic), None, mss, size, fin, now);
        carry(None, Some(traffic), mss, size, fin, now);
        return;
    }
    let mut sender = inet(from);
    let mut receiver = to.borrow_mut();
    let receiver = receiver.inet.as_deref_mut().map(|inet| &mut inet.traffic);
    carry(Some(&mut sender.traffic), receiver, mss, size, fin, now);
}

/// Data or a FIN between a connection's socket and the outside, `sent` by the socket or
/// received.
pub(super) fn carry_outside(kernel: &Kernel, socket: &SocketRef, size: u64, fin: bool, sent: bool) {
    let family = socket.borrow().family;
    let mut own = inet(socket);
    let (from, to, mss) = if sent {
        let mss = send_mss(family, &own.traffic);
        (Some(&mut own.traffic), None, mss)
    } else {
        (None, Some(&mut own.traffic), TCP_WINDOW / 2)
    };
    carry(from, to, mss, size, fin, kernel.entry_now);
}

/// A connection to or from the outside was made.
pub(super) fn made_outside(kernel: &mut Kernel, socket: &SocketRef, connector: bool) {
    kernel.sockets.inet.neighbours.ethernet = true;
    inet(socket).traffic = Traffic::made(connector, kernel.entry_now);
    clamp_window(socket);
}

/// A connection's end was established: its window clamp is at most the window its receive
/// buffer gives (`tcp_init_buffer_space`, half the buffer).
fn clamp_window(socket: &SocketRef) {
    let mut own = socket.borrow_mut();
    let full = (own.rcvbuf / 2) as i32;
    let key = (SOL_TCP, TCP_WINDOW_CLAMP);
    let clamp = own.options.get(&key).map_or(0, |bytes| {
        i32::from_le_bytes(bytes[..4].try_into().expect("4 bytes"))
    });
    if clamp == 0 || clamp >= full {
        own.options.insert(key, full.to_le_bytes().to_vec());
    }
}

/// `tcp_set_rcvlowat`: the mark is at most half the receive buffer (half of `tcp_rmem`'s largest
/// while the buffer is not set), and an unset buffer grows to give a window that large, with
/// the window clamp at the mark (`tcp_space_from_win` at the starting scaling ratio of one
/// half). Readers that now have enough hear of it.
pub(super) fn set_receive_low_water<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    value: i32,
) {
    let kept = {
        let mut own = socket.borrow_mut();
        let locked = own.inet.as_deref().unwrap().rcvbuf_set;
        let cap = if locked {
            own.rcvbuf / 2
        } else {
            TCP_RMEM_MAX / 2
        };
        let value = (value as u64).min(cap) as i32;
        if !locked {
            let space = value as u64 * 2;
            if space > own.rcvbuf {
                own.rcvbuf = space;
                own.options
                    .insert((SOL_TCP, TCP_WINDOW_CLAMP), value.to_le_bytes().to_vec());
            }
        }
        value.max(1)
    };
    // `tcp_data_ready` with the new mark.
    socket
        .borrow_mut()
        .options
        .insert((SOL_SOCKET, SO_RCVLOWAT), kept.to_le_bytes().to_vec());
    if inet_ref(socket).done || poll(socket) & EPOLLIN as u16 != 0 {
        super::data_ready(kernel, cx, socket, EPOLLIN | EPOLLRDNORM);
    }
}

/// The TCP state Linux shows for a socket (`sk_state`).
pub(super) fn tcp_state(own: &Socket) -> u8 {
    use crate::linux_extra::{TCP_CLOSE_WAIT, TCP_FIN_WAIT2};
    let inet = own.inet.as_deref().unwrap();
    match own.state {
        State::Listening => crate::linux_extra::TCP_LISTEN,
        _ if inet.connecting.is_some() => crate::linux_extra::TCP_SYN_SENT,
        State::Connected => {
            let sent_fin = own.shutdown & SEND_SHUTDOWN != 0;
            match (sent_fin, inet.done && own.shutdown & RCV_SHUTDOWN != 0) {
                (true, false) => TCP_FIN_WAIT2,
                (false, true) => TCP_CLOSE_WAIT,
                _ => crate::linux_extra::TCP_ESTABLISHED,
            }
        }
        State::Closed => crate::linux_extra::TCP_CLOSE,
    }
}

/// Whether a socket is in Linux's TCP_ESTABLISHED state: a TCP connection neither end has
/// closed, or a connected UDP socket.
fn established(socket: &SocketRef) -> bool {
    let own = socket.borrow();
    match own.inet.as_deref().unwrap().protocol {
        Protocol::Tcp => tcp_state(&own) == crate::linux_extra::TCP_ESTABLISHED,
        Protocol::Udp => own.state == State::Connected,
    }
}

/// A loopback connection's retransmission timeout (`tcpi_rto`, measured), after which its probe
/// timer sends what it holds back (`tcp_probe0_base`; measured 204 to 260 ms).
const LOOPBACK_RTO_MICROSECONDS: u32 = 201_000;

/// Sends what a connection holds back now, and stops its probe timer.
fn push_held<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>, socket: &SocketRef) {
    if let Some(probe) = inet(socket).probe.take() {
        crate::syscall::timer::stop(kernel, probe);
    }
    let held = inet(socket).corked.take();
    let peer = socket.borrow().peer.clone();
    if let (Some(packet), Some(peer)) = (held, peer) {
        deliver(kernel, cx, socket, &peer, packet, false);
    }
}

/// Holds back bytes on a connection, starting its probe timer if it is not running.
fn hold(kernel: &mut Kernel, socket: &SocketRef, packet: Packet) {
    inet(socket).corked = Some(packet);
    if inet_ref(socket).probe.is_none() {
        let until = kernel.entry_now
            + core::time::Duration::from_micros(LOOPBACK_RTO_MICROSECONDS.into()).as_nanos() as u64;
        let probe = crate::syscall::timer::start_held_bytes(kernel, Rc::downgrade(socket), until);
        inet(socket).probe = Some(probe);
    }
}

/// A connection's probe timer fired: what it holds back goes (`tcp_write_wakeup`).
pub(super) fn held_bytes_due<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
) {
    inet(socket).probe = None;
    push_held(kernel, cx, socket);
}

/// Whether TCP_CORK is set on a socket.
fn corked_by_option(socket: &SocketRef) -> bool {
    socket
        .borrow()
        .options
        .get(&(SOL_TCP, TCP_CORK))
        .is_some_and(|value| value.iter().any(|byte| *byte != 0))
}

/// `tcp_get_info` for a TCP socket, from the model in [`Traffic`].
fn tcp_info<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
) -> Vec<u8> {
    let own = socket.borrow();
    let inet = own.inet.as_deref().unwrap();
    let traffic = inet.traffic;
    let state = tcp_state(&own);
    let family = own.family;
    let now = kernel.entry_now;
    // `tcp_jiffies32` in milliseconds (HZ is 1000 on the reference, measured), which starts
    // five minutes before zero (`INITIAL_JIFFIES`).
    let jiffies = (now / 1_000_000).wrapping_sub(300_000) as u32;
    let since = |at: u64| (now.saturating_sub(at) / 1_000_000) as u32;
    let mut out = Vec::with_capacity(TCP_INFO_SIZE);
    let u32s = |out: &mut Vec<u8>, values: &[u32]| {
        for value in values {
            out.extend_from_slice(&value.to_le_bytes());
        }
    };
    let u64s = |out: &mut Vec<u8>, values: &[u64]| {
        for value in values {
            out.extend_from_slice(&value.to_le_bytes());
        }
    };
    if state == crate::linux_extra::TCP_LISTEN {
        // A listener tells only its pacing, window and backlog (`tcpi_unacked` and `tcpi_sacked`
        // are its queue and its limit).
        out.extend_from_slice(&[state, 0, 0, 0, 0, 0, 0, 0]);
        u32s(&mut out, &[0; 4]);
        u32s(&mut out, &[own.backlog.len() as u32, own.max_backlog]);
        u32s(&mut out, &[0; 12]);
        u32s(&mut out, &[10, 0, 3, 0, 0, 0]);
        u64s(&mut out, &[u64::MAX, u64::MAX]);
        out.resize(TCP_INFO_SIZE, 0);
        return out;
    }
    if !traffic.made {
        // Never connected (`tcp_init_sock`'s values); a connect waiting for a backlog's room has
        // sent its SYN.
        let syn_sent = state == crate::linux_extra::TCP_SYN_SENT;
        out.extend_from_slice(&[state, 0, 0, 0, 0, 0, 0, 1]);
        u32s(&mut out, &[1_000_000, 0]);
        if syn_sent {
            u32s(&mut out, &[TCP_MSS_SYN_SENT, 88]);
        } else {
            u32s(&mut out, &[TCP_MSS_UNCONNECTED as u32, 0]);
        }
        // The SYN is the one segment out.
        u32s(&mut out, &[u32::from(syn_sent), 0, 0, 0, 0]);
        u32s(&mut out, &[jiffies, 0, jiffies, jiffies]);
        let (pmtu, window) = if syn_sent {
            (PMTU4, advmss(family) + 12)
        } else {
            (0, 0)
        };
        u32s(
            &mut out,
            &[
                pmtu,
                window,
                0,
                250_000,
                0x7fff_ffff,
                10,
                window,
                3,
                0,
                0,
                0,
            ],
        );
        u64s(&mut out, &[u64::MAX, u64::MAX, 0, 0]);
        u32s(&mut out, &[u32::from(syn_sent), 0, 0, u32::MAX, 0, 0]);
        u64s(&mut out, &[0; 4]);
        u32s(&mut out, &[0; 2]);
        u64s(&mut out, &[0; 2]);
        u32s(&mut out, &[0, 0, 0, 0, window, 0]);
        out.resize(TCP_INFO_SIZE, 0);
        return out;
    }
    drop(own);
    kernel
        .counters
        .count(cx.host, crate::reason::TCP_INFO_MODELLED, 0);
    let mss = send_mss(family, &traffic);
    let advmss = advmss(family);
    // A connector's first window is what its SYN offered: the MTU less the IP and TCP headers.
    let first = if traffic.connector {
        advmss + 12
    } else {
        advmss
    };
    let app_limited = !traffic.sent_fin && traffic.data_segs_out <= 1;
    let options = (TCPI_OPT_TIMESTAMPS | TCPI_OPT_SACK | TCPI_OPT_WSCALE) as u8;
    // Window scale 10 both ways, as Linux's default buffers give (measured).
    out.extend_from_slice(&[state, 0, 0, 0, 0, options, 0xaa, u8::from(app_limited)]);
    let ato = if traffic.data_segs_in > 0 { 40_000 } else { 0 };
    let rcv_mss = traffic.largest.max(TCP_MSS_UNCONNECTED as u32);
    u32s(
        &mut out,
        &[LOOPBACK_RTO_MICROSECONDS, ato, mss, rcv_mss, 0, 0, 0, 0, 0],
    );
    u32s(
        &mut out,
        &[
            since(traffic.sent_at),
            0,
            since(traffic.received_at),
            since(traffic.acked_at),
        ],
    );
    let pmtu = if family == AF_INET6 { PMTU6 } else { PMTU4 };
    u32s(
        &mut out,
        &[
            pmtu,
            first,
            MODEL_RTT,
            MODEL_RTTVAR,
            0x7fff_ffff,
            10,
            advmss,
            3,
            0,
            first,
            0,
        ],
    );
    // `tcp_update_pacing_rate`: twice the window per round trip while in slow start.
    let pacing = u64::from(mss) * 10 * 2_000_000 / u64::from(MODEL_RTT);
    u64s(
        &mut out,
        &[
            pacing,
            u64::MAX,
            traffic.bytes_acked,
            traffic.bytes_received,
        ],
    );
    u32s(
        &mut out,
        &[
            traffic.segs_out,
            traffic.segs_in,
            0,
            MODEL_RTT,
            traffic.data_segs_in,
            traffic.data_segs_out,
        ],
    );
    let delivery = if traffic.data_segs_out > 0 {
        u64::from(mss) * 1_000_000 / u64::from(MODEL_RTT)
    } else {
        0
    };
    u64s(&mut out, &[delivery, 0, 0, 0]);
    u32s(&mut out, &[traffic.delivered, 0]);
    u64s(&mut out, &[traffic.bytes_sent, 0]);
    let snd_wnd = if traffic.connector && !traffic.heard {
        advmss
    } else {
        TCP_WINDOW
    };
    let rcv_wnd = if traffic.connector || traffic.data_segs_in > 0 {
        TCP_WINDOW
    } else {
        advmss
    };
    u32s(&mut out, &[0, 0, 0, snd_wnd, rcv_wnd, 0]);
    out.resize(TCP_INFO_SIZE, 0);
    out
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
    /// The neighbour table's entries.
    pub neighbours: Neighbours,
}

/// The neighbour table (`arp_tbl`, `nd_tbl`) as the container's own traffic makes it: each
/// family's loopback entry, made by its first packet through lo (IPv4's keyed 0.0.0.0, as ARP
/// keys every loopback neighbour; IPv6's ::1), and whether a packet left by eth0, whose entries
/// (the gateway's, a group's, a broadcast's) are not built.
#[derive(Default)]
pub(crate) struct Neighbours {
    /// When the IPv4 and the IPv6 loopback entries were made, and last confirmed by a TCP
    /// segment, on the monotonic clock.
    pub loopback: [Option<(u64, Option<u64>)>; 2],
    pub ethernet: bool,
}

/// A packet to `to` left: through lo it makes or confirms (for TCP) the loopback entry of its
/// family, else it went by eth0.
fn neighbour(kernel: &mut Kernel, to: &[u8; 16], through_lo: bool, tcp: bool) {
    let now = kernel.entry_now;
    let table = &mut kernel.sockets.inet.neighbours;
    if !through_lo {
        table.ethernet = true;
        return;
    }
    let entry = &mut table.loopback[usize::from(v4_of(to).is_none())];
    let (_, confirmed) = entry.get_or_insert((now, None));
    if tcp {
        *confirmed = Some(now);
    }
}

fn inet(socket: &SocketRef) -> core::cell::RefMut<'_, Inet> {
    core::cell::RefMut::map(socket.borrow_mut(), |own| {
        own.inet.as_deref_mut().expect("an internet socket")
    })
}

impl Inet {
    /// The bytes MSG_MORE holds back.
    pub(super) fn held(&self) -> u64 {
        self.corked
            .as_ref()
            .map_or(0, |packet| packet.data.len() as u64)
    }
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
pub(super) fn create<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    family: u32,
    kind: u32,
    protocol: u32,
) -> Result<Socket, Errno> {
    if protocol >= IPPROTO_MAX as u32 {
        return Err(Errno::EINVAL);
    }
    let refuse = |kernel: &mut Kernel, cx: &mut Context<'_, S>, answer: Errno| {
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
        // The raw kind matches any protocol but IPPROTO_IP itself.
        SOCK_RAW if protocol == 0 => return Err(Errno::EPROTONOSUPPORT),
        // Raw packets, which Docker's CAP_NET_RAW allows (plan section 15).
        SOCK_RAW => return refuse(kernel, cx, Errno::EPERM),
        // SCTP, which the reference has.
        SOCK_SEQPACKET => return refuse(kernel, cx, Errno::ESOCKTNOSUPPORT),
        _ => return Err(Errno::ESOCKTNOSUPPORT),
    };
    let mut socket = Socket::new(kernel.next_object(), family, kind, number);
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
fn take_port<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
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
fn autobind<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
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

pub(super) fn bind<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
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

pub(super) fn listen<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
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

pub(super) fn connect<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
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
    let given = u16::from_le_bytes([bytes[0], bytes[1]]) as u32;
    let named = if given == AF_UNSPEC {
        Named::Unspecified
    } else if protocol == Protocol::Udp && family == AF_INET6 && given == AF_INET {
        // `__ip6_datagram_connect` hands an IPv4 address to IPv4's connect.
        if inet_ref(socket).v6only {
            return Err(Errno::EAFNOSUPPORT);
        }
        read_endpoint(AF_INET, bytes)?
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
                disconnect_tcp(kernel, cx, socket);
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
fn disconnect_tcp<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
) {
    let state = socket.borrow().state;
    match state {
        State::Listening => {
            super::outside::stop_listening(kernel, socket);
            let mut backlog = core::mem::take(&mut socket.borrow_mut().backlog);
            backlog.extend(core::mem::take(&mut inet(socket).deferred));
            for embryo in backlog {
                super::released(kernel, cx, &embryo);
            }
            refuse_waiting(kernel, cx, socket);
        }
        State::Connected => reset_peer(kernel, cx, socket),
        State::Closed => {}
    }
    let (packets, port) = {
        let mut own = socket.borrow_mut();
        own.state = State::Closed;
        own.peer = None;
        own.shutdown = 0;
        own.error = 0;
        let family = own.family;
        let inet = own.inet.as_deref_mut().unwrap();
        inet.remote = None;
        inet.connecting = None;
        inet.done = false;
        // Closing lets go of what the program did not choose (`inet_reset_saddr`, and
        // `tcp_set_state`'s `inet_put_port`, which leaves the port shown); a socket already
        // closed keeps it.
        let mut port = None;
        if state != State::Closed {
            if !inet.address_chosen {
                inet.local.address = if family == AF_INET { ANY4 } else { ANY6 };
            }
            if !inet.port_chosen && inet.held {
                inet.held = false;
                port = Some(inet.local.port);
            }
        }
        (core::mem::take(&mut own.queue), port)
    };
    if let Some(port) = port {
        kernel.sockets.inet.let_go(Protocol::Tcp, port, socket);
    }
    for packet in packets {
        super::drop_packet(kernel, cx, packet);
    }
    kernel.wake(channel(socket));
}

/// Sends a reset to a connection's other end (`tcp_send_active_reset`): it sees ECONNRESET, or
/// EPIPE if its own FIN had come, and is closed (`tcp_reset`, `tcp_done`).
fn reset_peer<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>, socket: &SocketRef) {
    let Some(peer) = socket.borrow().peer.clone() else {
        return;
    };
    let back = peer
        .borrow()
        .peer
        .as_ref()
        .is_some_and(|back| Rc::ptr_eq(back, socket));
    if back {
        reset(kernel, cx, &peer);
    }
}

/// `tcp_reset`: an RST came. A connection held back by TCP_DEFER_ACCEPT is only a request,
/// which goes (`tcp_check_req`'s reset path), never to be accepted.
fn reset<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>, socket: &SocketRef) {
    let deferral = inet(socket).deferral.take();
    if let Some((listener, ..)) = deferral {
        if let Some(listener) = listener.upgrade() {
            inet(&listener)
                .deferred
                .retain(|held| !Rc::ptr_eq(held, socket));
        }
        socket.borrow_mut().state = State::Closed;
        super::released(kernel, cx, socket);
        return;
    }
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
    super::wake_async(kernel, cx, socket, super::Wake::Io, POLL_ERR);
}

fn tcp_connect<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
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
    neighbour(kernel, &to.address, is_own(&to.address), true);
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
    // A SYN to its own address and port meets itself: a simultaneous open.
    if inet_ref(socket).local == to && find_listener(kernel, &to).is_none() {
        join_self(kernel, cx, socket);
        if nonblocking {
            inet(socket).in_progress = true;
            return Err(Errno::EINPROGRESS);
        }
        return Ok(0);
    }
    let Some(listener) = find_listener(kernel, &to) else {
        // The listener's reset comes at once (`tcp_reset` in SYN_SENT).
        refused(kernel, cx, socket);
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
    join(kernel, cx, socket, &listener);
    if nonblocking {
        inet(socket).in_progress = true;
        return Err(Errno::EINPROGRESS);
    }
    Ok(0)
}

/// A connect to the outside: the host's connection starts, and its answer finishes it.
fn connect_outside<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    to: Endpoint,
    nonblocking: bool,
) -> Result<u64, Errno> {
    let id = match super::outside::open(kernel, cx, socket, to) {
        Ok(id) => id,
        Err(error) => {
            failed(kernel, cx, socket, error);
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
fn refused<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>, socket: &SocketRef) {
    failed(kernel, cx, socket, Errno::ECONNREFUSED);
}

/// A connect failed with `error`, as [`refused`].
pub(super) fn failed<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    error: Errno,
) {
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
    super::wake_async(kernel, cx, socket, super::Wake::Io, POLL_ERR);
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
fn join<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    listener: &SocketRef,
) {
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
    inet(socket).traffic = Traffic::made(true, kernel.entry_now);
    inet(&embryo).traffic = Traffic::made(false, kernel.entry_now);
    clamp_window(socket);
    clamp_window(&embryo);
    // TCP_DEFER_ACCEPT holds the connection back until data comes, while the listener's requests
    // are not over its backlog (past it Linux answers with a SYN cookie, which is not deferred).
    let defer = deferral_seconds(&listener.borrow());
    let requests = inet_ref(listener).deferred.len();
    if defer > 0 && requests <= listener.borrow().max_backlog as usize {
        let now = kernel.entry_now;
        let until = now + defer * NANOSECONDS_PER_SECOND;
        inet(&embryo).deferral = Some((Rc::downgrade(listener), now, until));
        inet(listener).deferred.push(embryo);
        crate::syscall::timer::start_deferral(kernel, Rc::downgrade(listener), until);
    } else {
        listener.borrow_mut().backlog.push_back(embryo);
        super::data_ready(kernel, cx, listener, EPOLLIN | EPOLLRDNORM);
    }
    kernel.wake_poll(channel(socket), EPOLLOUT | EPOLLWRNORM);
    super::wake_async(kernel, cx, socket, super::Wake::Io, POLL_OUT);
}

/// A socket connected to itself (`tcp_rcv_synsent_state_process`'s simultaneous open): what it
/// sends it reads.
fn join_self<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>, socket: &SocketRef) {
    {
        let mut own = socket.borrow_mut();
        own.state = State::Connected;
        own.peer = Some(socket.clone());
        if !own.inet.as_deref().unwrap().sndbuf_set {
            own.sndbuf = TCP_CONNECTED_SNDBUF;
        }
        own.inet.as_deref_mut().unwrap().connecting = None;
    }
    inet(socket).traffic = Traffic::made(true, kernel.entry_now);
    clamp_window(socket);
    kernel.wake_poll(channel(socket), EPOLLOUT | EPOLLWRNORM);
    super::wake_async(kernel, cx, socket, super::Wake::Io, POLL_OUT);
}

/// A listener's TCP_DEFER_ACCEPT in seconds, as getsockopt gives it back.
pub(super) fn deferral_seconds(listener: &Socket) -> u64 {
    listener
        .options
        .get(&(SOL_TCP, TCP_DEFER_ACCEPT))
        .map_or(0, |bytes| {
            u64::from(u32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")))
        })
}

/// `secs_to_retrans` and `retrans_to_secs` with Linux's first SYN-ACK timeout (1 s) and largest
/// (120 s): the seconds of whole retransmits that cover `seconds`, which is how long a deferred
/// connection waits for data (its last retransmit is answered, and then it is accepted).
fn deferral_rounded(seconds: i32) -> i32 {
    if seconds <= 0 {
        return 0;
    }
    let (mut timeout, mut period, mut retransmits) = (1, 1, 1);
    while seconds > period && retransmits < 255 {
        retransmits += 1;
        timeout = (timeout * 2).min(120);
        period += timeout;
    }
    period
}

/// A deferred connection's data or FIN came, or its deferral ended: it is established and joins
/// the accept queue (`tcp_check_req` creating the child). If that queue is full Linux drops the
/// segment and the request stays until the next try: the client's retransmit of its data or FIN
/// (after TCP_RTO_MIN, 200 ms, on loopback), or the next SYN-ACK timer.
fn arrived<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>, embryo: &SocketRef) {
    let listener = inet_ref(embryo)
        .deferral
        .as_ref()
        .and_then(|(listener, ..)| listener.upgrade());
    if let Some(listener) = listener
        && backlog_full(&listener.borrow())
    {
        let now = kernel.entry_now;
        let sent = !embryo.borrow().queue.is_empty() || inet_ref(embryo).done;
        let next = if sent {
            now + 200_000_000
        } else {
            now + deferred_request(&embryo.borrow(), now).map_or(0, |(left, _)| left)
        };
        if let Some((_, _, until)) = inet(embryo).deferral.as_mut() {
            *until = next;
        }
        crate::syscall::timer::start_deferral(kernel, Rc::downgrade(&listener), next);
        return;
    }
    let Some(deferral) = inet(embryo).deferral.take() else {
        return;
    };
    if let Some(listener) = deferral.0.upgrade() {
        inet(&listener)
            .deferred
            .retain(|held| !Rc::ptr_eq(held, embryo));
        listener.borrow_mut().backlog.push_back(embryo.clone());
        super::data_ready(kernel, cx, &listener, EPOLLIN | EPOLLRDNORM);
    }
}

/// A listener's deferral timer fired: the connections whose deferral ended are accepted, oldest
/// first (the SYN-ACK retransmitted at its end is answered at once on loopback). On a client
/// whose held request went with its listener, the retransmit of its dropped data or FIN draws
/// a reset.
pub(super) fn deferrals_due<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    listener: &SocketRef,
) {
    if listener.borrow().state != State::Listening {
        let gone = listener
            .borrow()
            .peer
            .as_ref()
            .is_some_and(|peer| peer.borrow().dead);
        if gone && listener.borrow().state == State::Connected {
            reset(kernel, cx, listener);
        }
        return;
    }
    let now = kernel.entry_now;
    let due: Vec<SocketRef> = inet_ref(listener)
        .deferred
        .iter()
        .filter(|embryo| {
            inet_ref(embryo)
                .deferral
                .as_ref()
                .is_some_and(|(_, _, until)| *until <= now)
        })
        .cloned()
        .collect();
    for embryo in due {
        arrived(kernel, cx, &embryo);
    }
}

/// A connection held back by TCP_DEFER_ACCEPT, as `/proc/net/tcp` shows it (`get_openreq4`):
/// SYN_RECV, the time to its next SYN-ACK timer and the timers fired so far.
pub(super) fn deferred_request(socket: &Socket, now: u64) -> Option<(u64, u64)> {
    let (_, made, _) = socket.inet.as_deref()?.deferral.as_ref()?;
    let elapsed = now.saturating_sub(*made);
    let (mut fires, mut at, mut timeout) = (0u64, NANOSECONDS_PER_SECOND, NANOSECONDS_PER_SECOND);
    while at <= elapsed {
        fires += 1;
        timeout = (timeout * 2).min(120_000_000_000);
        at += timeout;
    }
    Some((at - elapsed, fires))
}

/// The listener's end of a new connection to `address` on its port from `remote`, holding the
/// port and taking the listener's options (`tcp_create_openreq_child`).
pub(super) fn embryo(
    kernel: &mut Kernel,
    listener: &SocketRef,
    address: [u8; 16],
    remote: Endpoint,
) -> SocketRef {
    let id = kernel.next_object();
    let embryo = {
        let them = listener.borrow();
        let listening = them.inet.as_deref().unwrap();
        let mut new = Socket::new(id, them.family, SOCK_STREAM, TCP);
        new.state = State::Connected;
        new.options = them.options.clone();
        // The accept queue's settings are not the new socket's (`inet_csk_clone_lock`).
        new.options.remove(&(SOL_TCP, TCP_DEFER_ACCEPT));
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
        // The listener's chosen address goes with it, its chosen port not (`inet_csk_clone_lock`
        // clears SOCK_BINDPORT_LOCK).
        inet.address_chosen = listening.address_chosen;
        inet.port_chosen = false;
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
pub(super) fn accepted<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    listener: &SocketRef,
) {
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
        join(kernel, cx, &socket, listener);
    }
}

/// The connects still waiting on a listener that stops listening are refused.
fn refuse_waiting<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    listener: &SocketRef,
) {
    let waiting = core::mem::take(&mut inet(listener).waiting);
    for socket in waiting.iter().filter_map(Weak::upgrade) {
        if inet_ref(&socket).connecting.is_some() {
            refused(kernel, cx, &socket);
        }
    }
}

fn udp_connect<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
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
pub(super) fn shutdown<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    mode: u8,
) -> Result<u64, Errno> {
    let (state, connecting, protocol) = {
        let own = socket.borrow();
        let inet = own.inet.as_deref().unwrap();
        (own.state, inet.connecting.is_some(), inet.protocol)
    };
    if connecting {
        disconnect_tcp(kernel, cx, socket);
        return Ok(0);
    }
    match state {
        State::Listening => {
            if mode & RCV_SHUTDOWN != 0 {
                disconnect_tcp(kernel, cx, socket);
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
                    Some(id) => {
                        carry_outside(kernel, socket, 0, true, true);
                        super::outside::finish(kernel, cx, id);
                    }
                    None => send_fin(kernel, cx, socket),
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
fn send_fin<S: crate::System>(kernel: &mut Kernel, cx: &mut Context<'_, S>, socket: &SocketRef) {
    let Some(peer) = socket.borrow().peer.clone() else {
        return;
    };
    // The FIN goes after what is held back.
    push_held(kernel, cx, socket);
    let other_done = inet_ref(socket).done;
    if !other_done {
        inet(socket).closed_first = true;
    }
    carry_between(kernel, socket, &peer, 0, true);
    arrived(kernel, cx, &peer);
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
    // `tcp_fin` tells the end, then `tcp_data_ready` the data.
    if !peer.borrow().dead {
        let ended = {
            let other = peer.borrow();
            other.shutdown == SHUTDOWN_MASK || other.state == State::Closed
        };
        let reason = if ended { POLL_HUP } else { POLL_IN };
        super::wake_async(kernel, cx, &peer, super::Wake::Data, reason);
        super::data_ready(kernel, cx, &peer, EPOLLIN | EPOLLRDNORM);
    }
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
        if own.error != 0 || !inet.errors.is_empty() {
            mask |= EPOLLERR;
            if flag(&own, SOL_SOCKET, SO_SELECT_ERR_QUEUE) {
                mask |= EPOLLPRI;
            }
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
        // `tcp_stream_is_readable`: at an urgent byte not inline, a byte past it is wanted.
        let held: usize = own
            .queue
            .iter()
            .map(|packet| (packet.data.len() - packet.consumed).max(usize::from(packet.urgent)))
            .sum();
        let at_mark = !own.urgent_inline()
            && own
                .queue
                .iter()
                .find(|packet| packet.urgent || packet.consumed < packet.data.len())
                .is_some_and(|packet| packet.urgent);
        // `tcp_epollin_ready`: SO_RCVLOWAT's bytes (one more at such a byte), or fewer once
        // the receiver's window is closed, here when its buffer is full.
        let target = super::receive_low_water(&own) + u64::from(at_mark);
        let truesize: u64 = own.queue.iter().map(|packet| packet.truesize).sum();
        if held > 0 && (held as u64 >= target || truesize >= own.rcvbuf) {
            mask |= EPOLLIN | EPOLLRDNORM;
        }
        if own
            .queue
            .iter()
            .any(|packet| packet.urgent && packet.consumed == 0)
        {
            mask |= EPOLLPRI;
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

/// Whether room that came back is told by an I/O signal: `sk_stream_write_space` for TCP (while
/// it may still send), `sock_def_write_space` (`sock_writeable`) for UDP.
pub(super) fn space_to_tell(own: &Socket) -> bool {
    match own.inet.as_deref().unwrap().protocol {
        Protocol::Tcp => writable(own) && own.shutdown & SEND_SHUTDOWN == 0,
        Protocol::Udp => own.written << 1 < own.sndbuf,
    }
}

/// The last descriptor went: a connection is reset if bytes were left unread (or SO_LINGER asks
/// for no wait), else the other end gets a FIN; a listener's waiting connections are reset; the
/// ports are let go.
pub(super) fn released<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
) {
    super::outside::released(kernel, socket);
    let (state, protocol) = {
        let own = socket.borrow();
        (own.state, own.inet.as_deref().unwrap().protocol)
    };
    if protocol == Protocol::Tcp {
        match state {
            State::Listening => {
                // Requests held back go without a word (`inet_csk_listen_stop` resets only the
                // accept queue); their clients' next sends draw resets. A client whose data or
                // FIN was dropped while the queue was full sends it again at its retransmit.
                let deferred = core::mem::take(&mut inet(socket).deferred);
                for embryo in deferred {
                    let until = inet(&embryo).deferral.take().map(|(_, _, until)| until);
                    let sent = !embryo.borrow().queue.is_empty() || inet_ref(&embryo).done;
                    let packets = core::mem::take(&mut embryo.borrow_mut().queue);
                    for packet in packets {
                        super::drop_packet(kernel, cx, packet);
                    }
                    let client = embryo.borrow().peer.clone();
                    if let (true, Some(until), Some(client)) = (sent, until, client) {
                        crate::syscall::timer::start_deferral(
                            kernel,
                            Rc::downgrade(&client),
                            until,
                        );
                    }
                    embryo.borrow_mut().state = State::Closed;
                    super::released(kernel, cx, &embryo);
                }
                let backlog = core::mem::take(&mut socket.borrow_mut().backlog);
                for embryo in backlog {
                    reset_peer(kernel, cx, &embryo);
                    super::released(kernel, cx, &embryo);
                }
                refuse_waiting(kernel, cx, socket);
            }
            State::Connected if inet_ref(socket).outside.is_some() => {}
            State::Connected => {
                let linger_zero = socket
                    .borrow()
                    .options
                    .get(&(SOL_SOCKET, SO_LINGER))
                    .is_some_and(|value| value[..4] != [0; 4] && value[4..8] == [0; 4]);
                if !socket.borrow().queue.is_empty() || linger_zero {
                    reset_peer(kernel, cx, socket);
                } else if socket.borrow().shutdown & SEND_SHUTDOWN == 0 {
                    socket.borrow_mut().shutdown |= SEND_SHUTDOWN;
                    send_fin(kernel, cx, socket);
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
        super::drop_packet(kernel, cx, packet);
    }
    kernel.wake(channel(socket));
}

/// `tcp_sendmsg` and `udp_sendmsg`.
pub(super) fn send<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    message: &Outgoing,
    total: u64,
    nonblocking: bool,
) -> Result<u64, Errno> {
    let protocol = inet_ref(socket).protocol;
    match protocol {
        Protocol::Tcp => {
            // `tcp_sendmsg_locked` reads only the socket-level messages, and any fault is EINVAL,
            // in ESTABLISHED or CLOSE_WAIT: in any other state (not yet connected, or shut for
            // writing) `sk_stream_wait_connect` fails or waits first, and the call runs again
            // when the connection is made.
            let established = {
                let own = socket.borrow();
                own.state == State::Connected
                    && own.shutdown & SEND_SHUTDOWN == 0
                    && inet_ref(socket).connecting.is_none()
            };
            if let Some(control) = message.control.as_ref().filter(|_| established) {
                sent_control(kernel, cx, tid, socket, control, true, false)
                    .map_err(|_| Errno::EINVAL)?;
            }
            // Urgent data to the outside is refused only where it would go: an unconnected
            // socket fails first (`sk_stream_wait_connect`).
            let outside = established && inet_ref(socket).outside.is_some();
            // Held bytes go to the outside at once: the host has no cork.
            let held = [
                (message.flags & MSG_MORE != 0, u64::from(MSG_MORE)),
                (
                    corked_by_option(socket),
                    (u64::from(SOL_TCP) << u32::BITS) | u64::from(TCP_CORK),
                ),
            ];
            for (asked, detail) in held {
                if asked && total > 0 && outside {
                    kernel
                        .counters
                        .count(cx.host, crate::reason::OPTION_NOT_ACTED_ON, detail);
                }
            }
            if message.flags & MSG_OOB != 0 && total > 0 && outside {
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

fn tcp_send<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    message: &Outgoing,
    total: u64,
    nonblocking: bool,
) -> Result<u64, Errno> {
    let mut sent = kernel.threads[&tid].call.progress;
    // Whether this call has sent bytes, whose acknowledgement pushes any held back after them.
    let mut went = false;
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
                socket.borrow_mut().no_space = true;
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
                Ok(count) => {
                    carry_outside(kernel, socket, count as u64, false, true);
                    sent += count as u64;
                }
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
            reset(kernel, cx, socket);
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
            socket.borrow_mut().no_space = true;
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
        // With MSG_OOB the last byte is urgent: the receiver's mark (`tcp_mark_urg`).
        let urgent = message.flags & MSG_OOB != 0 && sent + size == total;
        let (data, last) = if urgent {
            let mut data = data;
            let last = data.pop().expect("a byte");
            (data, Some(last))
        } else {
            (data, None)
        };
        let fresh = data.len() as u64;
        socket.borrow_mut().written += fresh;
        // With MSG_MORE, bytes short of a whole segment are held back (`TCP_NAGLE_CORK`) and
        // later ones join them; a whole segment goes, and its acknowledgement, which on loopback
        // comes at once, pushes the rest (`tcp_data_snd_check`: Nagle's rule with nothing in
        // flight), as it does for a whole segment an earlier round of this call sent. Without
        // MSG_MORE everything held goes with the new bytes. Under TCP_CORK whole segments go and
        // the rest stays held, as the cork outlasts the acknowledgement.
        let cork = corked_by_option(socket);
        let more = (message.flags & MSG_MORE != 0 || cork) && last.is_none();
        let mut packet = inet(socket)
            .corked
            .take()
            .unwrap_or_else(|| empty_packet(kernel, tid, socket, 0));
        packet.data.extend_from_slice(&data);
        let family = socket.borrow().family;
        let mss = send_mss(family, &inet_ref(socket).traffic) as usize;
        if more && packet.data.len() < mss && (!went || cork) {
            hold(kernel, socket, packet);
        } else if cork && last.is_none() {
            let whole = packet.data.len() / mss * mss;
            let rest = packet.data.split_off(whole);
            let owner = packet.owner;
            went = true;
            push_held(kernel, cx, socket);
            deliver(kernel, cx, socket, &peer, packet, false);
            if !rest.is_empty() {
                let mut held = empty_packet(kernel, tid, socket, owner);
                held.data = rest;
                hold(kernel, socket, held);
            }
        } else {
            went |= !packet.data.is_empty();
            push_held(kernel, cx, socket);
            deliver(kernel, cx, socket, &peer, packet, last.is_some());
        }
        sent += fresh;
        if let Some(byte) = last {
            urgent_arrives(kernel, cx, tid, socket, &peer, byte);
            sent += 1;
        }
    }
}

/// A packet of nothing yet from a connection's sender, whose descriptors count against `owner`.
fn empty_packet(kernel: &Kernel, tid: Tid, socket: &SocketRef, owner: u32) -> Packet {
    Packet {
        data: Vec::new(),
        consumed: 0,
        files: Vec::new(),
        cred: super::own_cred(kernel, tid),
        sender: Rc::downgrade(socket),
        truesize: 0,
        source: None,
        arrival: None,
        time: None,
        urgent: false,
        owner,
    }
}

/// Bytes going from one end of a connection to the other's queue (with an urgent byte after them
/// when `urgent`), told to the receiver; nothing goes when there is nothing.
fn deliver<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    peer: &SocketRef,
    packet: Packet,
    urgent: bool,
) {
    let size = packet.data.len() as u64;
    if size == 0 && !urgent {
        return;
    }
    {
        let mut own = inet(socket);
        own.pingpong |= own.received;
    }
    inet(peer).received = true;
    let packet = Packet {
        truesize: size,
        time: super::stamp(kernel, cx, peer),
        ..packet
    };
    // Bytes past the receiver's buffer are still on their way under Linux's window, so only
    // a send that finds room there is told to the receiver's I/O signal.
    let arrives = {
        let other = peer.borrow();
        let held: u64 = other.queue.iter().map(|packet| packet.truesize).sum();
        held < other.rcvbuf
    };
    if size > 0 {
        peer.borrow_mut().queue.push_back(packet);
        if arrives {
            super::data_ready(kernel, cx, peer, EPOLLIN | EPOLLRDNORM);
        } else {
            kernel.wake_poll(channel(peer), EPOLLIN | EPOLLRDNORM);
        }
    }
    carry_between(kernel, socket, peer, size + u64::from(urgent), false);
    arrived(kernel, cx, peer);
}

/// `tcp_check_urg` and `tcp_urg`: a new urgent byte at the end of the receiver's queue. An
/// earlier one becomes an ordinary byte, unless it is the next to read and not inline, when it is
/// passed over. The receiver's owner hears of it (SIGURG), and its readers of data.
fn urgent_arrives<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    peer: &SocketRef,
    byte: u8,
) {
    let dropped = {
        let mut other = peer.borrow_mut();
        let inline = other.urgent_inline();
        let earlier = other.queue.iter().position(|packet| packet.urgent);
        let mut dropped = None;
        if let Some(at) = earlier {
            let next = other
                .queue
                .iter()
                .take(at)
                .all(|packet| packet.consumed == packet.data.len());
            if next && !inline {
                dropped = other.queue.remove(at);
            } else {
                let packet = &mut other.queue[at];
                packet.urgent = false;
                packet.consumed = 0;
            }
        }
        dropped
    };
    if let Some(packet) = dropped {
        super::drop_packet(kernel, cx, packet);
    }
    let packet = Packet {
        data: alloc::vec![byte],
        consumed: 0,
        files: Vec::new(),
        cred: super::own_cred(kernel, tid),
        sender: Rc::downgrade(socket),
        truesize: 1,
        source: None,
        arrival: None,
        time: super::stamp(kernel, cx, peer),
        urgent: true,
        owner: 0,
    };
    socket.borrow_mut().written += 1;
    peer.borrow_mut().queue.push_back(packet);
    super::send_urgent(kernel, cx, peer);
    super::data_ready(
        kernel,
        cx,
        peer,
        EPOLLIN | EPOLLPRI | EPOLLRDNORM | EPOLLRDBAND,
    );
}

/// `tcp_recv_urg`: the urgent byte, out of band, once (a peek leaves it); EINVAL with none, with
/// SO_OOBINLINE or once it was read. Its read leaves it in the queue emptied, the mark.
fn receive_urgent<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    parts: super::Data<'_>,
    flags: u32,
) -> Result<(u64, Received), Errno> {
    let byte = {
        let mut own = socket.borrow_mut();
        let inline = own.urgent_inline();
        let packet = own
            .queue
            .iter_mut()
            .find(|packet| packet.urgent)
            .filter(|packet| !inline && packet.consumed == 0)
            .ok_or(Errno::EINVAL)?;
        let byte = packet.data[0];
        if flags & MSG_PEEK == 0 {
            packet.consumed = 1;
        }
        byte
    };
    let mut received = Received {
        flags: MSG_OOB,
        ..Received::default()
    };
    if parts.size() == 0 {
        received.flags |= MSG_TRUNC;
        return Ok((0, received));
    }
    if flags & MSG_TRUNC == 0 {
        scatter(kernel, cx, tid, parts, &[byte]).map_err(|_| Errno::EFAULT)?;
    }
    Ok((1, received))
}

/// The true size Linux charges a receiver for a datagram of `size` bytes (measured): its buffer
/// with the IP and UDP headers and the link's room (`ip_append_data`), the head from the small
/// head cache when it fits (`kmalloc_reserve`), and past `SKB_MAX_ALLOC` the payload in pages,
/// counted exactly.
pub(super) fn datagram_truesize(size: u64) -> u64 {
    const SMALL_HEAD: u64 = 640;
    let head = size + 28 + 31;
    if head >= super::SKB_MAX_ALLOC {
        return SMALL_HEAD + super::SK_BUFF + size;
    }
    let wanted = head.next_multiple_of(64) + super::SHARED_INFO;
    let allocated = if wanted <= SMALL_HEAD {
        SMALL_HEAD
    } else {
        wanted.next_power_of_two()
    };
    allocated + super::SK_BUFF
}

/// What a send's control messages set for its datagram.
#[derive(Clone, Copy, Default)]
struct SentControl {
    /// IPv4's TTL or IPv6's hop limit, and the type of service or traffic class.
    ttl: Option<u8>,
    tos: Option<u8>,
    /// Its mark and priority (`sockcm_cookie`'s), which receivers with SO_RCVMARK and
    /// SO_RCVPRIORITY are told.
    mark: Option<u32>,
    priority: Option<u32>,
    /// The source address packet information names, if not the any address.
    source: Option<[u8; 16]>,
    /// The interface packet information names, if any.
    interface: Option<u32>,
    /// UDP's segment size (UDP_SEGMENT), from the message or else the socket.
    segment: Option<u16>,
    /// The send timestamps this send asks for in place of the socket's, and its key.
    stamp_flags: Option<u32>,
    stamp_key: Option<u32>,
}

/// The most segments one UDP send may make (`UDP_MAX_SEGMENTS`).
const UDP_MAX_SEGMENTS: u64 = 128;

/// Whether an interface number names one of the container's interfaces.
fn interface_known(index: u32) -> bool {
    index == network::LOOPBACK_INDEX || index == network::ETHERNET_INDEX
}

/// `rt_tos2priority`: the queueing priority a type of service gives.
fn tos_priority(tos: u8) -> u32 {
    const BY_TOS: [u32; 16] = [0, 0, 0, 0, 2, 2, 2, 2, 6, 6, 6, 6, 4, 4, 4, 4];
    BY_TOS[usize::from((tos & 0x1e) >> 1)]
}

/// The control messages of an internet send: `sock_cmsg_send` for TCP, `ip_cmsg_send` for a
/// datagram over IPv4 (`v4`) and `ip6_datagram_send_ctl` over IPv6. Descriptors and credentials
/// are passed over, what Linux does not know is EINVAL, and what it knows that is not built here
/// (send timestamps, IPv4 options, IPv6 extension headers) is refused and counted.
fn sent_control<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    bytes: &[u8],
    socket_level_only: bool,
    v4: bool,
) -> Result<SentControl, Errno> {
    let mut out = SentControl::default();
    let mut at = 0usize;
    let credentials = kernel.credentials(tid);
    let powerful = credentials.has(CAP_NET_RAW) || credentials.has(CAP_NET_ADMIN);
    let mut refuse = |level: u32, kind: u32| {
        Err(kernel.counters.refuse(
            cx.host,
            crate::reason::OPTION_NOT_BUILT,
            (u64::from(level) << 32) | u64::from(kind),
            Errno::EINVAL,
        ))
    };
    let txtime = socket
        .borrow()
        .options
        .contains_key(&(SOL_SOCKET, SO_TXTIME));
    let family = socket.borrow().family;
    // `udp_cmsg_send` reads UDP's own messages first.
    if !socket_level_only {
        while at + 16 <= bytes.len() {
            let length = u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"));
            let level = u32::from_le_bytes(bytes[at + 8..at + 12].try_into().expect("4 bytes"));
            let kind = u32::from_le_bytes(bytes[at + 12..at + 16].try_into().expect("4 bytes"));
            if length < 16 || at as u64 + length > bytes.len() as u64 {
                return Err(Errno::EINVAL);
            }
            if level == SOL_UDP {
                if kind != crate::linux_extra::UDP_SEGMENT || length != 18 {
                    return Err(Errno::EINVAL);
                }
                out.segment = Some(u16::from_le_bytes([bytes[at + 16], bytes[at + 17]]));
            }
            at += length.next_multiple_of(8) as usize;
        }
        at = 0;
    }
    while at + 16 <= bytes.len() {
        let length = u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"));
        let level = u32::from_le_bytes(bytes[at + 8..at + 12].try_into().expect("4 bytes"));
        let kind = u32::from_le_bytes(bytes[at + 12..at + 16].try_into().expect("4 bytes"));
        if length < 16 || at as u64 + length > bytes.len() as u64 {
            return Err(Errno::EINVAL);
        }
        let data = &bytes[at + 16..at + length as usize];
        // The readings of an int that must fill the message.
        let int = || match data.len() {
            4 => Ok(i32::from_le_bytes(data.try_into().expect("4 bytes"))),
            _ => Err(Errno::EINVAL),
        };
        match level {
            // `__sock_cmsg_send`.
            SOL_SOCKET => match kind {
                SCM_RIGHTS | SCM_CREDENTIALS => {}
                SO_MARK => {
                    if !powerful {
                        return Err(Errno::EPERM);
                    }
                    out.mark = Some(int()? as u32);
                }
                SO_PRIORITY => {
                    let priority = int()?;
                    if !(powerful || (0..=6).contains(&priority)) {
                        return Err(Errno::EPERM);
                    }
                    out.priority = Some(priority as u32);
                }
                // The transmit time means nothing here: no queue discipline holds packets.
                SCM_TXTIME => {
                    if !txtime || data.len() != 8 {
                        return Err(Errno::EINVAL);
                    }
                }
                // Send timestamps for one send: only the recording flags; software ones are
                // not built.
                SO_TIMESTAMPING_OLD | SO_TIMESTAMPING_NEW => {
                    let flags = int()? as u32;
                    let hardware = SOF_TIMESTAMPING_TX_HARDWARE as u32;
                    let software = SOF_TIMESTAMPING_TX_SOFTWARE as u32
                        | SOF_TIMESTAMPING_TX_SCHED as u32
                        | SOF_TIMESTAMPING_TX_ACK as u32
                        | SOF_TIMESTAMPING_TX_COMPLETION as u32;
                    if flags & !(hardware | software) != 0 {
                        return Err(Errno::EINVAL);
                    }
                    let own = socket.borrow();
                    let udp = own
                        .inet
                        .as_deref()
                        .is_some_and(|inet| inet.protocol == Protocol::Udp);
                    let tsonly = own.tsflags & SOF_TIMESTAMPING_OPT_TSONLY as u32 != 0;
                    let completion = SOF_TIMESTAMPING_TX_COMPLETION as u32;
                    drop(own);
                    if flags & software != 0 && !(udp && tsonly && flags & completion == 0) {
                        return refuse(level, kind);
                    }
                    out.stamp_flags = Some(flags);
                }
                // The key of this send's timestamps, which only sockets with OPT_ID that are
                // not TCP name.
                SCM_TS_OPT_ID => {
                    let own = socket.borrow();
                    let tcp = own
                        .inet
                        .as_deref()
                        .is_some_and(|inet| inet.protocol == Protocol::Tcp);
                    if tcp || own.tsflags & SOF_TIMESTAMPING_OPT_ID as u32 == 0 || data.len() != 4 {
                        return Err(Errno::EINVAL);
                    }
                    out.stamp_key = Some(u32::from_le_bytes(data.try_into().expect("4 bytes")));
                }
                _ => return Err(Errno::EINVAL),
            },
            _ if socket_level_only => {}
            // An IPv6 socket sending over IPv4 takes IPv6 packet information naming an IPv4
            // address.
            SOL_IPV6 if v4 && family == AF_INET6 && kind == IPV6_PKTINFO => {
                if data.len() < 20 {
                    return Err(Errno::EINVAL);
                }
                let address: [u8; 16] = data[..16].try_into().expect("16 bytes");
                let Some(v4) = v4_of(&address) else {
                    return Err(Errno::EINVAL);
                };
                let index = u32::from_le_bytes(data[16..20].try_into().expect("4 bytes"));
                out.interface = (index != 0).then_some(index).or(out.interface);
                out.source = (v4 != [0; 4]).then_some(address);
            }
            SOL_IP if v4 => match kind {
                IP_TTL => {
                    let ttl = int()?;
                    if !(1..=255).contains(&ttl) {
                        return Err(Errno::EINVAL);
                    }
                    out.ttl = Some(ttl as u8);
                }
                IP_TOS => {
                    let tos = match data.len() {
                        1 => i32::from(data[0]),
                        _ => int()?,
                    };
                    if !(0..=255).contains(&tos) {
                        return Err(Errno::EINVAL);
                    }
                    out.tos = Some(tos as u8);
                    out.priority = Some(tos_priority(tos as u8));
                }
                IP_PKTINFO => {
                    // `struct in_pktinfo`: the interface, the source, and an address unused here.
                    if data.len() != 12 {
                        return Err(Errno::EINVAL);
                    }
                    let index = u32::from_le_bytes(data[..4].try_into().expect("4 bytes"));
                    if index != 0 {
                        out.interface = Some(index);
                    }
                    let source: [u8; 4] = data[4..8].try_into().expect("4 bytes");
                    out.source = (source != [0; 4]).then(|| mapped(source));
                }
                // Only raw sockets use the protocol.
                IP_PROTOCOL => {
                    if !(1..=255).contains(&int()?) {
                        return Err(Errno::EINVAL);
                    }
                }
                IP_RETOPTS => return refuse(level, kind),
                _ => return Err(Errno::EINVAL),
            },
            SOL_IPV6 if !v4 => match kind {
                IPV6_PKTINFO | IPV6_2292PKTINFO => {
                    if data.len() < 20 {
                        return Err(Errno::EINVAL);
                    }
                    let index = u32::from_le_bytes(data[16..20].try_into().expect("4 bytes"));
                    if index != 0 {
                        if out.interface.is_some_and(|before| before != index) {
                            return Err(Errno::EINVAL);
                        }
                        out.interface = Some(index);
                    }
                    if out.interface.is_some_and(|index| !interface_known(index)) {
                        return Err(Errno::ENODEV);
                    }
                    let address: [u8; 16] = data[..16].try_into().expect("16 bytes");
                    if address != ANY6 {
                        // Link-local addresses need an interface; none is the container's.
                        if out.interface.is_none()
                            && address[0] == 0xfe
                            && address[1] & 0xc0 == 0x80
                        {
                            return Err(Errno::EINVAL);
                        }
                        if !is_own(&address) || v4_of(&address).is_some() {
                            return Err(Errno::EINVAL);
                        }
                        out.source = Some(address);
                    }
                }
                // Flow labels are sent only with IPV6_FLOWINFO_SEND, which is not built.
                IPV6_FLOWINFO => {
                    if data.len() < 4 {
                        return Err(Errno::EINVAL);
                    }
                }
                IPV6_HOPLIMIT | IPV6_2292HOPLIMIT => {
                    let hops = int()?;
                    if !(-1..=255).contains(&hops) {
                        return Err(Errno::EINVAL);
                    }
                    out.ttl = (hops >= 0).then_some(hops as u8);
                }
                IPV6_TCLASS => {
                    let class = int()?;
                    if !(-1..=255).contains(&class) {
                        return Err(Errno::EINVAL);
                    }
                    // -1 is kept as it is, so the class sent is 255 (measured).
                    out.tos = Some(class as u8);
                }
                // Nothing here fragments.
                IPV6_DONTFRAG => {
                    if !(0..=1).contains(&int()?) {
                        return Err(Errno::EINVAL);
                    }
                }
                IPV6_HOPOPTS | IPV6_2292HOPOPTS | IPV6_DSTOPTS | IPV6_2292DSTOPTS
                | IPV6_RTHDRDSTOPTS | IPV6_RTHDR | IPV6_2292RTHDR => return refuse(level, kind),
                _ => return Err(Errno::EINVAL),
            },
            _ => {}
        }
        at += length.next_multiple_of(8) as usize;
    }
    Ok(out)
}

fn udp_send<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    message: &Outgoing,
    to: Option<&[u8]>,
    total: u64,
) -> Result<u64, Errno> {
    let family = socket.borrow().family;
    let corking = flag(&socket.borrow(), SOL_UDP, crate::linux_extra::UDP_CORK)
        || message.flags & MSG_MORE != 0;
    // `udp_sendmsg` starts with the length a UDP header can carry, then urgent data, then any
    // corked frames, which take the bytes whatever destination is named.
    if family == AF_INET {
        if total > 0xffff {
            return Err(Errno::EMSGSIZE);
        }
        if message.flags & MSG_OOB != 0 {
            return Err(Errno::EOPNOTSUPP);
        }
        if inet_ref(socket).cork.is_some() {
            return udp_append(kernel, cx, tid, socket, message, total, corking);
        }
    }
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
                // `udpv6_sendmsg` hands an IPv4 destination to `udp_sendmsg`.
                AF_INET => {
                    if inet_ref(socket).v6only {
                        return Err(Errno::ENETUNREACH);
                    }
                    if total > 0xffff {
                        return Err(Errno::EMSGSIZE);
                    }
                    if message.flags & MSG_OOB != 0 {
                        return Err(Errno::EOPNOTSUPP);
                    }
                    if bytes.len() < 16 {
                        return Err(Errno::EINVAL);
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
                AF_UNSPEC => None,
                _ => return Err(Errno::EINVAL),
            }
        }
        None => None,
    };
    // `udpv6_sendmsg` looks for corked frames once the destination is read.
    if inet_ref(socket).cork.is_some() {
        return udp_append(kernel, cx, tid, socket, message, total, corking);
    }
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
    let mut sent = match &message.control {
        Some(control) => {
            let v4 = v4_of(&to.address).is_some();
            sent_control(kernel, cx, tid, socket, control, false, v4)?
        }
        None => SentControl::default(),
    };
    if sent.segment.is_none() {
        sent.segment = socket
            .borrow()
            .options
            .get(&(SOL_UDP, crate::linux_extra::UDP_SEGMENT))
            .map(|value| u16::from_le_bytes([value[0], value[1]]));
    }
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
    let route = route(&to, &sent)?;
    // `udp_sendmsg`: a broadcast route wants SO_BROADCAST.
    let broadcast = v4_of(&to.address).is_some_and(|v4| v4 == [255; 4] || v4 == network::BROADCAST);
    if broadcast && !flag(&socket.borrow(), SOL_SOCKET, SO_BROADCAST) {
        return Err(Errno::EACCES);
    }
    // Multicast leaves by its interface, from that interface's address.
    let group = v4_of(&to.address).is_some_and(multicast);
    let through = sent
        .interface
        .unwrap_or_else(|| multicast_interface(socket));
    if group && !inet_ref(socket).address_chosen && through == network::LOOPBACK_INDEX {
        from.address = LOOPBACK4;
    }
    if let Some(source) = sent.source {
        from.address = source;
    }
    let data = gather(kernel, cx, tid, message.data, 0, total)?;
    if route == Route::Lost {
        return Ok(total);
    }
    if corking {
        inet(socket).cork = Some(Corked {
            to,
            from,
            data,
            sent,
        });
        return Ok(total);
    }
    udp_deliver(kernel, cx, tid, socket, to, from, data, sent)?;
    send_stamps(kernel, cx, socket, &sent);
    Ok(total)
}

/// Whether an IPv4 address is a multicast group.
fn multicast(address: [u8; 4]) -> bool {
    address[0] & 0xf0 == 0xe0
}

/// The interface an IPv4 address belongs to, if it is the container's own (`__ip_dev_find`).
fn interface_of(address: [u8; 4]) -> Option<u32> {
    match address {
        [127, ..] => Some(network::LOOPBACK_INDEX),
        own if own == network::ADDRESS => Some(network::ETHERNET_INDEX),
        _ => None,
    }
}

/// `ip_mc_find_dev`: the interface a membership names by number, else by address, else the one
/// the route to the group leaves by (eth0).
fn membership_interface(index: u32, address: [u8; 4]) -> Option<u32> {
    if index != 0 {
        return interface_known(index).then_some(index);
    }
    if address != [0; 4] {
        return interface_of(address);
    }
    Some(network::ETHERNET_INDEX)
}

/// IP_ADD_MEMBERSHIP and IP_DROP_MEMBERSHIP (`ip_mreq` or `ip_mreqn`), and MCAST_JOIN_GROUP and
/// MCAST_LEAVE_GROUP (`group_req`), as `ip_mc_join_group` and `ip_mc_leave_group`.
fn membership(socket: &SocketRef, name: u32, bytes: &[u8]) -> Result<u64, Errno> {
    let word = |at: usize| -> [u8; 4] { bytes[at..at + 4].try_into().expect("4 bytes") };
    let (group, index, address, join) = match name {
        IP_ADD_MEMBERSHIP | IP_DROP_MEMBERSHIP => {
            // Only these refuse a connection's socket (`IS_ICSK`); the group_req forms take it.
            if inet_ref(socket).protocol == Protocol::Tcp {
                return Err(Errno::EPROTO);
            }
            if bytes.len() < 8 {
                return Err(Errno::EINVAL);
            }
            let index = if bytes.len() >= 12 {
                u32::from_le_bytes(word(8))
            } else {
                0
            };
            (word(0), index, word(4), name == IP_ADD_MEMBERSHIP)
        }
        _ => {
            // `struct group_req`: the interface, then a socket address that must be IPv4's.
            if bytes.len() < 136 {
                return Err(Errno::EINVAL);
            }
            if u16::from_le_bytes([bytes[8], bytes[9]]) as u32 != AF_INET {
                return Err(Errno::EINVAL);
            }
            (
                word(12),
                u32::from_le_bytes(word(0)),
                [0; 4],
                name == MCAST_JOIN_GROUP,
            )
        }
    };
    if !multicast(group) {
        return Err(Errno::EINVAL);
    }
    let interface = membership_interface(index, address);
    let mut own = inet(socket);
    if join {
        let interface = interface.ok_or(Errno::ENODEV)?;
        if own.groups.contains(&(group, interface)) {
            return Err(Errno::EADDRINUSE);
        }
        // `sysctl_igmp_max_memberships` (measured 20).
        if own.groups.len() >= 20 {
            return Err(Errno::ENOBUFS);
        }
        own.groups.push((group, interface));
        return Ok(0);
    }
    if index == 0 && address == [0; 4] && interface.is_none() {
        return Err(Errno::ENODEV);
    }
    let position = own.groups.iter().position(|&(joined, on)| {
        joined == group && (index == 0 && address == [0; 4] || Some(on) == interface)
    });
    let position = position.ok_or(Errno::EADDRNOTAVAIL)?;
    own.groups.remove(position);
    Ok(0)
}

/// Whether a socket joined a group on an interface.
fn joined(socket: &SocketRef, group: [u8; 4], interface: u32) -> bool {
    socket
        .borrow()
        .inet
        .as_deref()
        .is_some_and(|inet| inet.groups.contains(&(group, interface)))
}

/// `ip_check_mc_rcu`: an interface is a member of the all-hosts group from when it is up, and of
/// any other group while some socket joined it there.
pub(super) fn interface_in_group(kernel: &Kernel, group: [u8; 4], interface: u32) -> bool {
    u32::from_be_bytes(group) == INADDR_ALLHOSTS_GROUP
        || kernel
            .sockets
            .inet
            .bound
            .values()
            .flatten()
            .filter_map(Weak::upgrade)
            .any(|other| joined(&other, group, interface))
}

/// A datagram to a multicast group or a broadcast address, given to every socket of the
/// container that takes it (`__udp4_lib_mcast_deliver`): a group's only when it leaves by an
/// interface where some socket joined it and the sender loops multicast back, and to a socket
/// bound to the any address or the destination, connected to none or the sender, that joined
/// the group there or takes every group (IP_MULTICAST_ALL). Its copy to the network outside is
/// not sent: the host carries no multicast or broadcast.
#[allow(clippy::too_many_arguments, reason = "a datagram's parts")]
fn deliver_many<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    to: Endpoint,
    from: Endpoint,
    data: &[u8],
    sent: SentControl,
    interface: u32,
) {
    let group = v4_of(&to.address).filter(|&address| multicast(address));
    if interface == network::ETHERNET_INDEX {
        kernel
            .counters
            .count(cx.host, crate::reason::MULTICAST_NOT_SENT_OUT, 0);
    }
    let looped = !flag_or(&socket.borrow(), SOL_IP, IP_MULTICAST_LOOP, true);
    if group.is_some() && looped {
        return;
    }
    if let Some(group) = group
        && !(joined(socket, group, interface) || interface_in_group(kernel, group, interface))
    {
        return;
    }
    let receivers = kernel.sockets.inet.holders(Protocol::Udp, to.port);
    for receiver in receivers {
        let takes = {
            let own = receiver.borrow();
            let inet = own.inet.as_deref().unwrap();
            let bound = is_any(&inet.local.address) || inet.local.address == to.address;
            let connected = inet
                .remote
                .filter(|_| own.state == State::Connected)
                .is_none_or(|remote| remote == from);
            let allowed = match group {
                Some(group) => {
                    inet.groups.contains(&(group, interface))
                        || flag_or(&own, SOL_IP, IP_MULTICAST_ALL, true)
                }
                None => true,
            };
            family_takes(own.family, inet.v6only, &to.address) && bound && connected && allowed
        };
        if !takes {
            continue;
        }
        if !udp_room(&receiver.borrow(), datagram_truesize(data.len() as u64)) {
            receiver.borrow_mut().drops += 1;
            continue;
        }
        let mut arrival = arrival_from(socket, &to, sent);
        arrival.index = interface;
        if group.is_some() && sent.ttl.is_none() {
            arrival.ttl = option_or(&socket.borrow(), SOL_IP, IP_MULTICAST_TTL, 1) as u8;
        }
        let packet = Packet {
            data: data.to_vec(),
            consumed: 0,
            files: Vec::new(),
            cred: super::own_cred(kernel, tid),
            sender: Weak::new(),
            truesize: datagram_truesize(data.len() as u64),
            source: Some(from),
            arrival: Some(prepared(&receiver, arrival)),
            time: super::stamp(kernel, cx, &receiver),
            urgent: false,
            owner: 0,
        };
        receiver.borrow_mut().queue.push_back(packet);
        super::data_ready(kernel, cx, &receiver, EPOLLIN | EPOLLRDNORM);
    }
}

/// A flag option's value, or its default when never set.
fn flag_or(socket: &Socket, level: u32, name: u32, default: bool) -> bool {
    socket
        .options
        .get(&(level, name))
        .map_or(default, |value| value.iter().any(|&byte| byte != 0))
}

/// An int option's value, or its default when never set.
fn option_or(socket: &Socket, level: u32, name: u32, default: i32) -> i32 {
    socket.options.get(&(level, name)).map_or(default, |value| {
        i32::from_le_bytes(value[..4].try_into().expect("4 bytes"))
    })
}

/// The interface multicast leaves a socket by: IP_MULTICAST_IF's, else eth0.
fn multicast_interface(socket: &SocketRef) -> u32 {
    let own = socket.borrow();
    match own.options.get(&(SOL_IP, IP_MULTICAST_IF)) {
        Some(value) if value.len() >= 8 => {
            u32::from_le_bytes(value[4..8].try_into().expect("4 bytes"))
        }
        _ => network::ETHERNET_INDEX,
    }
}

/// Where a datagram goes by the interface and source its packet information name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Route {
    /// By its destination.
    Usual,
    /// Out of eth0 to a loopback address, where nothing answers: it is lost.
    Lost,
}

/// `ip_route_output_flow` and `ip6_dst_lookup_flow` for a datagram whose packet information
/// names a source or an interface: a source must be the container's own and not multicast or
/// broadcast, the interface must exist, IPv6 has no route out of eth0, and a loopback source
/// cannot leave through eth0.
fn route(to: &Endpoint, sent: &SentControl) -> Result<Route, Errno> {
    let via = sent.interface;
    let Some(destination) = v4_of(&to.address) else {
        if via == Some(network::ETHERNET_INDEX) {
            return Err(Errno::ENETUNREACH);
        }
        return Ok(Route::Usual);
    };
    let source = sent.source.and_then(|source| v4_of(&source));
    if let Some(source) = source {
        if (224..=239).contains(&source[0]) || source == [255; 4] {
            return Err(Errno::EINVAL);
        }
        if !is_own(&mapped(source)) {
            return Err(Errno::ENETUNREACH);
        }
    }
    if via.is_some_and(|index| !interface_known(index)) {
        return Err(Errno::ENODEV);
    }
    // The container's own addresses are reached through lo, except loopback ones named out of
    // eth0.
    let out_of_eth0 = via == Some(network::ETHERNET_INDEX) && destination[0] == 127;
    let leaves = via != Some(network::LOOPBACK_INDEX) && (!is_own(&to.address) || out_of_eth0);
    if source.is_some_and(|source| source[0] == 127) && leaves {
        return Err(Errno::EINVAL);
    }
    if out_of_eth0 {
        return Ok(Route::Lost);
    }
    Ok(Route::Usual)
}

/// `__skb_tstamp_tx` for a UDP datagram that left: a SCHED entry and then a SND entry on the
/// error queue as the send asks, each only the time (OPT_TSONLY), keyed by OPT_ID's count of
/// such sends unless the send names its key.
fn send_stamps<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    sent: &SentControl,
) {
    // A hardware request takes a key too (`SKBTX_ANY_TSTAMP`), though none comes here.
    let record = SOF_TIMESTAMPING_TX_SOFTWARE as u32
        | SOF_TIMESTAMPING_TX_SCHED as u32
        | SOF_TIMESTAMPING_TX_HARDWARE as u32;
    let tsflags = socket.borrow().tsflags;
    let asked = sent.stamp_flags.unwrap_or(tsflags) & record;
    if asked == 0 {
        return;
    }
    let key = if tsflags & SOF_TIMESTAMPING_OPT_ID as u32 == 0 {
        0
    } else if let Some(key) = sent.stamp_key {
        key
    } else {
        let mut own = socket.borrow_mut();
        own.tskey = own.tskey.wrapping_add(1);
        own.tskey.wrapping_sub(1)
    };
    let time = Some(kernel.now(cx));
    let truesize = datagram_truesize(0);
    let infos = [
        (
            SOF_TIMESTAMPING_TX_SCHED as u32,
            crate::linux_extra::SCM_TSTAMP_SCHED,
        ),
        (
            SOF_TIMESTAMPING_TX_SOFTWARE as u32,
            crate::linux_extra::SCM_TSTAMP_SND,
        ),
    ];
    for (flag, info) in infos {
        if asked & flag == 0 {
            continue;
        }
        let mut own = socket.borrow_mut();
        let held: u64 = own
            .inet
            .as_deref()
            .unwrap()
            .errors
            .iter()
            .map(|error| error.truesize)
            .sum();
        // `sock_queue_err_skb`: none past the receive buffer.
        if held + truesize >= own.rcvbuf {
            continue;
        }
        own.inet
            .as_deref_mut()
            .unwrap()
            .errors
            .push_back(QueuedError {
                to: Endpoint::default(),
                from: [0; 16],
                data: Vec::new(),
                truesize,
                time,
                stamp: Some((info, key)),
            });
        drop(own);
        super::error_report(kernel, cx, socket);
    }
}

/// `__udp_enqueue_schedule_skb`: a datagram fits a receiver whose queue has room for its true
/// size, or whose queue is empty (unless its buffer is past half the largest int).
fn udp_room(receiver: &Socket, truesize: u64) -> bool {
    let held: u64 = receiver.queue.iter().map(|packet| packet.truesize).sum();
    let rcvbuf = receiver.rcvbuf;
    held + truesize <= rcvbuf || (rcvbuf <= (i32::MAX >> 1) as u64 && held == 0)
}

/// What a UDP datagram is held as while corked (MSG_MORE or UDP_CORK): its destination, source,
/// bytes so far and the first send's control messages (`up->pending`, `inet->cork`).
pub(crate) struct Corked {
    to: Endpoint,
    from: Endpoint,
    data: Vec<u8>,
    sent: SentControl,
}

/// `udp_sendmsg`'s `do_append_data` with frames corked: the bytes join them, whatever destination
/// was named (a datagram may not outgrow UDP's largest, and a fault drops every corked byte), and
/// go as one datagram unless still corked.
fn udp_append<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    message: &Outgoing,
    total: u64,
    corking: bool,
) -> Result<u64, Errno> {
    let (held, v4) = {
        let inet = inet_ref(socket);
        let cork = inet.cork.as_ref().expect("corked frames");
        (cork.data.len() as u64, v4_of(&cork.to.address).is_some())
    };
    let largest = if v4 { UDP_MAX4 } else { UDP_MAX6 };
    if held + total > largest {
        inet(socket).cork = None;
        return Err(Errno::EMSGSIZE);
    }
    let data = match gather(kernel, cx, tid, message.data, 0, total) {
        Ok(data) => data,
        Err(error) => {
            inet(socket).cork = None;
            return Err(error);
        }
    };
    inet(socket)
        .cork
        .as_mut()
        .expect("corked frames")
        .data
        .extend_from_slice(&data);
    if !corking {
        uncork(kernel, cx, tid, socket)?;
    }
    Ok(total)
}

/// `udp_push_pending_frames`: corked bytes go as one datagram.
pub(super) fn uncork<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
) -> Result<(), Errno> {
    let Some(Corked {
        to,
        from,
        data,
        sent,
    }) = inet(socket).cork.take()
    else {
        return Ok(());
    };
    udp_deliver(kernel, cx, tid, socket, to, from, data, sent)?;
    send_stamps(kernel, cx, socket, &sent);
    Ok(())
}

/// A datagram from `from` to `to`: to the outside, to the container's name server, to the socket
/// that takes it, or answered as a port nothing holds.
#[allow(clippy::too_many_arguments, reason = "a datagram's parts")]
fn udp_deliver<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    to: Endpoint,
    from: Endpoint,
    data: Vec<u8>,
    sent: SentControl,
) -> Result<(), Errno> {
    let total = data.len() as u64;
    // `udp_send_skb`: a datagram longer than its segment size goes as segments of it, at most
    // `UDP_MAX_SEGMENTS`, kept whole to a receiver with UDP_GRO (`udp_unexpected_gso`).
    let segment = sent
        .segment
        .map(u64::from)
        .filter(|&size| size > 0 && total > size);
    if let Some(size) = segment
        && total > size * UDP_MAX_SEGMENTS
    {
        return Err(Errno::EINVAL);
    }
    let pieces = |data: &[u8]| -> Vec<Vec<u8>> {
        match segment {
            Some(size) => data.chunks(size as usize).map(<[u8]>::to_vec).collect(),
            None => alloc::vec![data.to_vec()],
        }
    };
    if let Some(v4) = v4_of(&to.address)
        && !unicast(&to.address)
    {
        let interface = if multicast(v4) {
            sent.interface
                .unwrap_or_else(|| multicast_interface(socket))
        } else {
            network::ETHERNET_INDEX
        };
        neighbour(
            kernel,
            &to.address,
            interface == network::LOOPBACK_INDEX,
            false,
        );
        for piece in pieces(&data) {
            deliver_many(kernel, cx, tid, socket, to, from, &piece, sent, interface);
        }
        return Ok(());
    }
    // Through lo every destination is the container's own.
    let through_lo = is_own(&to.address) || sent.interface == Some(network::LOOPBACK_INDEX);
    neighbour(kernel, &to.address, through_lo, false);
    if !through_lo {
        if !unicast(&to.address) {
            return Ok(());
        }
        for piece in pieces(&data) {
            super::outside::send_datagram(kernel, cx, socket, to, &piece)?;
        }
        return Ok(());
    }
    let receiver = find_receiver(kernel, &to, &from);
    let name_server = Endpoint {
        address: mapped(network::NAME_SERVER),
        port: 53,
    };
    if receiver.is_none() && to == name_server {
        super::outside::query(kernel, cx, socket, name_server, &data);
        return Ok(());
    }
    match receiver {
        Some(receiver) => {
            let whole = flag(&receiver.borrow(), SOL_UDP, crate::linux_extra::UDP_GRO);
            let (pieces, kept) = match segment {
                Some(size) if whole => (alloc::vec![data], size as u16),
                _ => (pieces(&data), 0),
            };
            for data in pieces {
                let truesize = datagram_truesize(data.len() as u64);
                if !udp_room(&receiver.borrow(), truesize) {
                    receiver.borrow_mut().drops += 1;
                    continue;
                }
                let arrival = Arrival {
                    segment: kept,
                    ..arrival_from(socket, &to, sent)
                };
                let packet = Packet {
                    data,
                    consumed: 0,
                    files: Vec::new(),
                    cred: super::own_cred(kernel, tid),
                    sender: Weak::new(),
                    truesize,
                    source: Some(from),
                    arrival: Some(prepared(&receiver, arrival)),
                    time: super::stamp(kernel, cx, &receiver),
                    urgent: false,
                    owner: 0,
                };
                receiver.borrow_mut().queue.push_back(packet);
                super::data_ready(kernel, cx, &receiver, EPOLLIN | EPOLLRDNORM);
            }
        }
        None if unicast(&to.address) => port_unreachable(kernel, cx, socket, to, from, &data),
        None => {}
    }
    Ok(())
}

/// `__udp4_lib_err` and `__udp6_lib_err` for a datagram to `to` no socket took: with the error
/// option of the way it went its sender queues the error (`ip_icmp_error`, if its receive buffer
/// has room) and hears of it; without, only a sender connected to `to` hears of it.
fn port_unreachable<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    to: Endpoint,
    from: Endpoint,
    data: &[u8],
) {
    let v4 = v4_of(&to.address).is_some();
    let recverr = {
        let own = socket.borrow();
        if v4 {
            flag(&own, SOL_IP, IP_RECVERR)
        } else {
            flag(&own, SOL_IPV6, IPV6_RECVERR)
        }
    };
    if !recverr {
        let connected_to = {
            let own = socket.borrow();
            (own.state == State::Connected)
                .then(|| own.inet.as_deref().unwrap().remote)
                .flatten()
        };
        if connected_to == Some(to) {
            socket.borrow_mut().error = Errno::ECONNREFUSED.0 as i32;
            super::error_report(kernel, cx, socket);
        }
        return;
    }
    // ICMP quotes the packet up to 576 bytes in all, ICMPv6 up to IPv6's minimum MTU (measured:
    // 520 and 1184 bytes of the datagram).
    let (headers, room) = if v4 { (56, 576 - 56) } else { (96, 1280 - 96) };
    let quoted = &data[..data.len().min(room)];
    let truesize = datagram_truesize(quoted.len() as u64 + headers);
    let time = super::stamp(kernel, cx, socket);
    {
        let mut own = socket.borrow_mut();
        let held: u64 = own.queue.iter().map(|packet| packet.truesize).sum::<u64>()
            + own
                .inet
                .as_deref()
                .unwrap()
                .errors
                .iter()
                .map(|error| error.truesize)
                .sum::<u64>();
        if held + truesize < own.rcvbuf {
            own.inet
                .as_deref_mut()
                .unwrap()
                .errors
                .push_back(QueuedError {
                    to,
                    from: from.address,
                    data: quoted.to_vec(),
                    truesize,
                    time,
                    stamp: None,
                });
        }
        own.error = Errno::ECONNREFUSED.0 as i32;
    }
    super::error_report(kernel, cx, socket);
}

/// `ip_recv_error` and `ipv6_recv_error`: the first queued error, taken even by a peek, named by
/// its datagram's destination, with the error message's own control messages and then the
/// error's; the socket's error becomes the next one's, or none (`sock_dequeue_err_skb`).
fn read_error<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    parts: super::Data<'_>,
) -> Result<(u64, Received), Errno> {
    let Some(error) = inet(socket).errors.pop_front() else {
        return Err(Errno::EAGAIN);
    };
    // `sock_dequeue_err_skb`: the socket's error is the next ICMP error's, or none after the
    // last ICMP error; timestamps leave it.
    let next = inet_ref(socket)
        .errors
        .front()
        .map(|next| next.stamp.is_none());
    match next {
        Some(true) => socket.borrow_mut().error = Errno::ECONNREFUSED.0 as i32,
        _ if error.stamp.is_none() => socket.borrow_mut().error = 0,
        _ => {}
    }
    if next.is_some() {
        super::error_report(kernel, cx, socket);
    }
    let family = socket.borrow().family;
    let (level, kind) = if family == AF_INET {
        (SOL_IP, IP_RECVERR)
    } else {
        (SOL_IPV6, IPV6_RECVERR)
    };
    if let Some((info, key)) = error.stamp {
        // ENOMSG from SO_EE_ORIGIN_TIMESTAMPING, with no offender and no bytes (OPT_TSONLY).
        let mut extended = Errno::ENOMSG.0.to_le_bytes().to_vec();
        extended.extend_from_slice(&[crate::linux_extra::SO_EE_ORIGIN_TIMESTAMPING, 0, 0, 0]);
        extended.extend_from_slice(&info.to_le_bytes());
        extended.extend_from_slice(&key.to_le_bytes());
        let offender = if family == AF_INET { 16 } else { 28 };
        extended.resize(extended.len() + offender, 0);
        let received = Received {
            flags: MSG_ERRQUEUE,
            error: Some((level, kind, extended)),
            time: error.time,
            ..Received::default()
        };
        return Ok((0, received));
    }
    let size = parts.size();
    let copied = (error.data.len() as u64).min(size);
    scatter(kernel, cx, tid, parts, &error.data[..copied as usize])?;
    let v4 = v4_of(&error.to.address).is_some();
    // `struct sock_extended_err` and the offender, the host that sent the error.
    let mut extended = Errno::ECONNREFUSED.0.to_le_bytes().to_vec();
    use crate::linux_extra::{
        ICMP_DEST_UNREACH, ICMP_PORT_UNREACH, ICMPV6_DEST_UNREACH, ICMPV6_PORT_UNREACH,
        SO_EE_ORIGIN_ICMP, SO_EE_ORIGIN_ICMP6,
    };
    if v4 {
        extended.extend_from_slice(&[SO_EE_ORIGIN_ICMP, ICMP_DEST_UNREACH, ICMP_PORT_UNREACH, 0]);
    } else {
        extended.extend_from_slice(&[
            SO_EE_ORIGIN_ICMP6,
            ICMPV6_DEST_UNREACH,
            ICMPV6_PORT_UNREACH,
            0,
        ]);
    }
    extended.extend_from_slice(&[0; 8]);
    let offender = Endpoint {
        address: error.to.address,
        port: 0,
    };
    extended.extend_from_slice(&address_bytes(family, &offender));
    let index = if v4_of(&error.from) == Some(network::ADDRESS) {
        network::ETHERNET_INDEX
    } else {
        network::LOOPBACK_INDEX
    };
    let mut received = Received {
        address: Some(address_bytes(family, &error.to)),
        flags: MSG_ERRQUEUE,
        // The error message's header: ICMP's precedence is internetwork control.
        arrival: Some(Arrival {
            destination: error.from,
            index,
            ttl: 64,
            tos: if v4 { 0xc0 } else { 0 },
            known: true,
            error: true,
            prepared: true,
            mark: 0,
            priority: 0,
            segment: 0,
            drops: 0,
        }),
        error: Some((level, kind, extended)),
        time: error.time,
        ..Received::default()
    };
    if copied < error.data.len() as u64 {
        received.flags |= MSG_TRUNC;
    }
    Ok((copied, received))
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
pub(super) fn receive<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    parts: super::Data<'_>,
    flags: u32,
    nonblocking: bool,
) -> Result<(u64, Received), Errno> {
    let size = parts.size();
    if flags & MSG_ERRQUEUE != 0 {
        return read_error(kernel, cx, tid, socket, parts);
    }
    if inet_ref(socket).protocol == Protocol::Udp {
        return udp_receive(kernel, cx, tid, socket, parts, size, flags, nonblocking);
    }
    {
        let own = socket.borrow();
        if own.state == State::Listening {
            return Err(Errno::ENOTCONN);
        }
        if flags & MSG_OOB != 0 {
            drop(own);
            return receive_urgent(kernel, cx, tid, socket, parts, flags);
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
        super::outside::pull_and_wait(kernel, cx, socket, id);
    }
    // `sk_wait_data` keeps a waiting reader's data from sending its I/O signal.
    socket.borrow_mut().waiting_data = false;
    let result = super::receive_stream(kernel, cx, tid, socket, parts, size, flags, nonblocking);
    if result.as_ref().err() == Some(&Errno::PARKED) {
        socket.borrow_mut().waiting_data = true;
    }
    if let Some(id) = outside {
        super::outside::pull_and_wait(kernel, cx, socket, id);
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
fn udp_receive<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    parts: super::Data<'_>,
    size: u64,
    flags: u32,
    nonblocking: bool,
) -> Result<(u64, Received), Errno> {
    let peeking = flags & MSG_PEEK != 0;
    // `sk_peek_offset`: where a peek starts with SO_PEEK_OFF, past whole datagrams
    // (`__skb_try_recv_from_queue`).
    let mut skip = if peeking {
        socket.borrow().peek_offset.unwrap_or(0)
    } else {
        0
    };
    let found = {
        let mut own = socket.borrow_mut();
        if own.error != 0 {
            let error = core::mem::take(&mut own.error);
            return Err(Errno(error as u32));
        }
        let mut found = None;
        for (index, packet) in own.queue.iter().enumerate() {
            let length = packet.data.len() as u64;
            if skip >= length && (length > 0 || skip > 0) {
                skip -= length;
                continue;
            }
            found = Some(index);
            break;
        }
        if found.is_none() {
            // Shut for reading, an empty queue ends a wait; a nonblocking read finds nothing
            // (`__skb_wait_for_more_packets` is never reached).
            if own.queue.is_empty() && own.shutdown & RCV_SHUTDOWN != 0 && !nonblocking {
                return Ok((0, Received::default()));
            }
            let timeout = own.receive_timeout;
            drop(own);
            return Err(wait_room(kernel, tid, socket, nonblocking, timeout));
        }
        found.expect("a datagram")
    };
    let (data, source, arrival, time) = {
        let own = socket.borrow();
        let packet = &own.queue[found];
        (
            packet.data.clone(),
            packet.source,
            packet.arrival,
            packet.time,
        )
    };
    let left = &data[skip as usize..];
    let copied = (left.len() as u64).min(size);
    if let Err(error) = scatter(kernel, cx, tid, parts, &left[..copied as usize]) {
        // `udp_recvmsg` drops a datagram it could not copy out, unless peeking.
        if !peeking {
            socket.borrow_mut().queue.remove(found);
        }
        return Err(error);
    }
    let family = socket.borrow().family;
    let mut received = Received {
        address: source.map(|from| address_bytes(family, &from)),
        arrival,
        time,
        ..Received::default()
    };
    if left.len() as u64 > size {
        received.flags |= MSG_TRUNC;
    }
    if flags & MSG_TRUNC != 0 {
        received.reported = Some(data.len() as u64);
    }
    // `skb_consume_udp`: a peek moves the peek offset on by what it gave back, a read back.
    let given = received.reported.unwrap_or(copied);
    let mut own = socket.borrow_mut();
    if let Some(offset) = &mut own.peek_offset {
        *offset = if peeking {
            *offset + given
        } else {
            offset.saturating_sub(given)
        };
    }
    if !peeking {
        own.queue.remove(found);
    }
    drop(own);
    Ok((copied, received))
}

/// What a datagram from `sender` to `to` inside the container carries in its header and route:
/// the sender's TTL or hop limit and type of service or class, and the interface of the address
/// it went to (`inet_iif`, eth0's for eth0's address, else loopback).
fn arrival_from(sender: &SocketRef, to: &Endpoint, sent: SentControl) -> Arrival {
    let own = sender.borrow();
    let option = |level: u32, name: u32, default: i32| {
        own.options
            .get(&(level, name))
            .map(|value| i32::from_le_bytes(value[..4].try_into().expect("4 bytes")))
            .filter(|value| *value >= 0)
            .unwrap_or(default)
    };
    // Over IPv4 an IPv6 socket's own IPv4 options apply too (`udp_sendmsg`).
    let (ttl, tos) = if v4_of(&to.address).is_some() {
        (
            sent.ttl
                .map_or_else(|| option(SOL_IP, IP_TTL, 64), i32::from),
            sent.tos
                .map_or_else(|| option(SOL_IP, IP_TOS, 0), i32::from),
        )
    } else {
        (
            sent.ttl
                .map_or_else(|| option(SOL_IPV6, IPV6_UNICAST_HOPS, 64), i32::from),
            sent.tos
                .map_or_else(|| option(SOL_IPV6, IPV6_TCLASS, 0), i32::from),
        )
    };
    let through_lo = sent.interface == Some(network::LOOPBACK_INDEX);
    let index = if v4_of(&to.address) == Some(network::ADDRESS) && !through_lo {
        network::ETHERNET_INDEX
    } else {
        network::LOOPBACK_INDEX
    };
    let word = |name: u32| {
        own.options.get(&(SOL_SOCKET, name)).map_or(0, |value| {
            u32::from_le_bytes(value[..4].try_into().expect("4 bytes"))
        })
    };
    let mark = sent.mark.unwrap_or_else(|| word(SO_MARK));
    let priority = sent.priority.unwrap_or_else(|| word(SO_PRIORITY));
    Arrival {
        destination: to.address,
        index,
        ttl: ttl as u8,
        tos: tos as u8,
        known: true,
        error: false,
        prepared: true,
        mark,
        priority,
        segment: 0,
        drops: 0,
    }
}

/// What came from outside to eth0's address: the host does not say its TTL or type of service.
pub(super) fn arrival_outside() -> Arrival {
    Arrival {
        destination: mapped(network::ADDRESS),
        index: network::ETHERNET_INDEX,
        ttl: 64,
        tos: 0,
        known: false,
        error: false,
        prepared: true,
        mark: 0,
        priority: 0,
        segment: 0,
        drops: 0,
    }
}

/// A received datagram's header and route, for the control messages its receiver asks for.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Arrival {
    pub destination: [u8; 16],
    pub index: u32,
    pub ttl: u8,
    pub tos: u8,
    /// Whether the TTL and type of service are the sender's (else 64 and 0, made up).
    pub known: bool,
    /// An error message's, whose IPv4 packet information names no local address.
    pub error: bool,
    /// Whether its receiver asked for packet information when it came (`ipv4_pktinfo_prepare`):
    /// if not, an IPv4 datagram's names no interface or local address.
    pub prepared: bool,
    /// The sender's mark and priority, for SO_RCVMARK and SO_RCVPRIORITY.
    pub mark: u32,
    pub priority: u32,
    /// The segment size of a datagram kept whole for a receiver with UDP_GRO (0 for none).
    pub segment: u16,
    /// The receiver's drops as it was queued, if it asked for them then (SO_RXQ_OVFL).
    pub drops: u32,
}

/// What `ipv4_pktinfo_prepare` keeps of a datagram as it is queued for `receiver`.
pub(super) fn prepared(receiver: &SocketRef, arrival: Arrival) -> Arrival {
    let own = receiver.borrow();
    let asked = flag(&own, SOL_IP, IP_PKTINFO)
        || (own.family == AF_INET6 && flag(&own, SOL_IPV6, IPV6_RECVPKTINFO));
    // `sock_skb_set_dropcount`.
    let drops = if flag(&own, SOL_SOCKET, SO_RXQ_OVFL) {
        own.drops
    } else {
        0
    };
    Arrival {
        prepared: asked,
        drops,
        ..arrival
    }
}

/// The control messages of IP_PKTINFO, IP_RECVTTL, IP_RECVTOS and their IPv6 forms for a
/// datagram, in Linux's order: an IPv6 socket's own (`ip6_datagram_recv_common_ctl`, and for an
/// IPv6 datagram `ip6_datagram_recv_specific_ctl`), then the IPv4 ones for an IPv4 datagram
/// (`ip_cmsg_recv_offset`). Each is (level, type, data).
pub(super) fn arrival_messages<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    arrival: &Arrival,
) -> Vec<(u32, u32, Vec<u8>)> {
    let own = socket.borrow();
    let on = |level: u32, name: u32| flag(&own, level, name);
    let mut out = Vec::new();
    let v4 = v4_of(&arrival.destination);
    let index = if arrival.prepared || v4.is_none() {
        arrival.index
    } else {
        0
    };
    let mut header_used = false;
    if own.family == AF_INET6 {
        if on(SOL_IPV6, IPV6_RECVPKTINFO) {
            let mut data = arrival.destination.to_vec();
            data.extend_from_slice(&index.to_le_bytes());
            out.push((SOL_IPV6, IPV6_PKTINFO, data));
        }
        if v4.is_none() {
            if on(SOL_IPV6, IPV6_RECVHOPLIMIT) {
                header_used = true;
                out.push((
                    SOL_IPV6,
                    IPV6_HOPLIMIT,
                    i32::from(arrival.ttl).to_le_bytes().to_vec(),
                ));
            }
            if on(SOL_IPV6, IPV6_RECVTCLASS) {
                header_used = true;
                out.push((
                    SOL_IPV6,
                    IPV6_TCLASS,
                    i32::from(arrival.tos).to_le_bytes().to_vec(),
                ));
            }
        }
    }
    if let Some(destination) = v4 {
        if on(SOL_IP, IP_PKTINFO) {
            let mut data = index.to_le_bytes().to_vec();
            let named = arrival.prepared && !arrival.error;
            data.extend_from_slice(if named { &destination } else { &[0; 4] });
            data.extend_from_slice(&destination);
            out.push((SOL_IP, IP_PKTINFO, data));
        }
        if on(SOL_IP, IP_RECVTTL) {
            header_used = true;
            out.push((
                SOL_IP,
                IP_TTL,
                i32::from(arrival.ttl).to_le_bytes().to_vec(),
            ));
        }
        if on(SOL_IP, IP_RECVTOS) {
            header_used = true;
            out.push((SOL_IP, IP_TOS, alloc::vec![arrival.tos]));
        }
    }
    drop(own);
    if header_used && !arrival.known {
        kernel
            .counters
            .count(cx.host, crate::reason::OUTSIDE_HEADER_MADE_UP, 0);
    }
    out
}

/// One integer option's value as a call gave it: at least four bytes (or one for the IP level's
/// small values).
fn int_value(bytes: &[u8], level: u32) -> Result<i32, Errno> {
    if bytes.len() >= 4 {
        return Ok(i32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")));
    }
    // `do_ip_setsockopt` reads an int or a byte, and nothing leaves the value 0 (measured).
    if level == SOL_IP {
        return Ok(bytes.first().map_or(0, |byte| i32::from(*byte)));
    }
    Err(Errno::EINVAL)
}

/// setsockopt for an internet socket's own levels, and the SOL_SOCKET options whose meaning is
/// its own. Returns `None` for an option the common code answers.
pub(super) fn set_option<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    tid: Tid,
    socket: &SocketRef,
    level: u32,
    name: u32,
    bytes: &[u8],
) -> Option<Result<u64, Errno>> {
    let family = socket.borrow().family;
    let protocol = inet_ref(socket).protocol;
    let not_built = |kernel: &mut Kernel, cx: &mut Context<'_, S>| {
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
            _ => None,
        },
        SOL_TCP if protocol == Protocol::Tcp => {
            if bytes.len() < 4 && name != TCP_CONGESTION {
                return Some(Err(Errno::EINVAL));
            }
            let value = int_value(bytes, level).unwrap_or(0);
            match name {
                // Quick acknowledgements are pingpong mode's opposite (`__tcp_sock_set_quickack`).
                TCP_QUICKACK => {
                    inet(socket).pingpong = value == 0;
                    Some(Ok(0))
                }
                // `do_tcp_setsockopt`: a fast open connect is 0 or 1, chosen while closed.
                TCP_FASTOPEN_CONNECT
                    if !(0..=1).contains(&value)
                        || tcp_state(&socket.borrow()) != crate::linux_extra::TCP_CLOSE =>
                {
                    Some(Err(Errno::EINVAL))
                }
                // Taking the cork off sends what it held (`__tcp_sock_set_cork`).
                TCP_CORK => {
                    let stored = store(socket, i32::from(value != 0));
                    if value == 0 {
                        push_held(kernel, cx, socket);
                    }
                    stored
                }
                // Setting it sends what is held, even under the cork (`__tcp_sock_set_nodelay`).
                TCP_NODELAY => {
                    let stored = store(socket, i32::from(value != 0));
                    if value != 0 {
                        push_held(kernel, cx, socket);
                    }
                    stored
                }
                TCP_FASTOPEN_CONNECT => {
                    if value != 0 {
                        kernel.counters.count(
                            cx.host,
                            crate::reason::OPTION_NOT_ACTED_ON,
                            (u64::from(level) << 32) | u64::from(name),
                        );
                    }
                    store(socket, i32::from(value != 0))
                }
                TCP_THIN_LINEAR_TIMEOUTS | TCP_FASTOPEN_NO_COOKIE | TCP_INQ => {
                    store(socket, i32::from(value != 0))
                }
                TCP_THIN_DUPACK => Some(if (0..=1).contains(&value) {
                    Ok(0)
                } else {
                    Err(Errno::EINVAL)
                }),
                // Repair mode wants CAP_NET_ADMIN (`tcp_can_repair_sock`); without it nothing
                // of it may be set.
                TCP_REPAIR | TCP_REPAIR_QUEUE | TCP_TIMESTAMP | TCP_AO_REPAIR => {
                    Some(Err(if kernel.credentials(tid).has(CAP_NET_ADMIN) {
                        return not_built(kernel, cx);
                    } else {
                        Errno::EPERM
                    }))
                }
                TCP_QUEUE_SEQ => Some(Err(
                    if socket.borrow().state == State::Closed
                        && inet_ref(socket).connecting.is_none()
                    {
                        Errno::EINVAL
                    } else {
                        Errno::EPERM
                    },
                )),
                TCP_REPAIR_WINDOW => Some(Err(if bytes.len() != 20 {
                    Errno::EINVAL
                } else {
                    Errno::EPERM
                })),
                // A saved SYN and a delay before each send change what the peer sees; kept,
                // and counted.
                TCP_SAVE_SYN | TCP_TX_DELAY => {
                    if name == TCP_SAVE_SYN && !(0..=2).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    if value != 0 {
                        kernel.counters.count(
                            cx.host,
                            crate::reason::OPTION_NOT_ACTED_ON,
                            (u64::from(level) << 32) | u64::from(name),
                        );
                    }
                    store(socket, value)
                }
                // Retransmission's bounds: nothing is lost between the container's own sockets.
                TCP_RTO_MAX_MS => {
                    if !(1000..=120_000).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, value)
                }
                // Whole jiffies (HZ 1000) of at least two and at most TCP_RTO_MIN.
                TCP_RTO_MIN_US => {
                    let jiffies = (i64::from(value) + 999).div_euclid(1000);
                    if !(2..=200).contains(&jiffies) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, (jiffies * 1000) as i32)
                }
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
                TCP_DEFER_ACCEPT => store(socket, deferral_rounded(value)),
                // `tcp_set_window_clamp`: 0 (no clamp) only while closed.
                TCP_WINDOW_CLAMP => {
                    let closed = socket.borrow().state == State::Closed
                        && inet_ref(socket).connecting.is_none();
                    if value == 0 && !closed {
                        return Some(Err(Errno::EINVAL));
                    }
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
        // `udp_lib_setsockopt`: checksums mean nothing without a wire, so IPv6's are only kept;
        // encapsulation (IPsec and L2TP) is not built.
        SOL_UDP if protocol == Protocol::Udp => {
            use crate::linux_extra::{
                UDP_CORK, UDP_GRO, UDP_NO_CHECK6_RX, UDP_NO_CHECK6_TX, UDP_SEGMENT,
                UDPLITE_RECV_CSCOV, UDPLITE_SEND_CSCOV,
            };
            let value = match int_value(bytes, level) {
                Ok(value) => value,
                Err(error) => return Some(Err(error)),
            };
            match name {
                UDP_CORK => {
                    let stored = store(socket, i32::from(value != 0));
                    if value == 0
                        && let Err(error) = uncork(kernel, cx, tid, socket)
                    {
                        return Some(Err(error));
                    }
                    stored
                }
                UDP_NO_CHECK6_TX | UDP_NO_CHECK6_RX | UDP_GRO => {
                    store(socket, i32::from(value != 0))
                }
                UDP_SEGMENT => {
                    if !(0..=0xffff).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, value)
                }
                // Coverage is UDP-Lite's alone.
                UDPLITE_SEND_CSCOV | UDPLITE_RECV_CSCOV => Some(Err(Errno::ENOPROTOOPT)),
                _ => not_built(kernel, cx),
            }
        }
        SOL_IP => {
            if name == IP_HDRINCL {
                return Some(Err(Errno::ENOPROTOOPT));
            }
            if matches!(
                name,
                IP_ADD_MEMBERSHIP | IP_DROP_MEMBERSHIP | MCAST_JOIN_GROUP | MCAST_LEAVE_GROUP
            ) {
                return Some(membership(socket, name, bytes));
            }
            // `ip_mreqn`, `ip_mreq` or an address: the interface multicast leaves by.
            if name == IP_MULTICAST_IF {
                if protocol == Protocol::Tcp || bytes.len() < 4 {
                    return Some(Err(Errno::EINVAL));
                }
                let word =
                    |at: usize| -> [u8; 4] { bytes[at..at + 4].try_into().expect("4 bytes") };
                let address = if bytes.len() >= 8 { word(4) } else { word(0) };
                let index = if bytes.len() >= 12 {
                    u32::from_le_bytes(word(8))
                } else {
                    0
                };
                if index == 0 && address == [0; 4] {
                    socket.borrow_mut().options.remove(&(level, name));
                    return Some(Ok(0));
                }
                let index = if index == 0 {
                    interface_of(address)
                } else {
                    interface_known(index).then_some(index)
                };
                let Some(index) = index else {
                    return Some(Err(Errno::EADDRNOTAVAIL));
                };
                let mut kept = address.to_vec();
                kept.extend_from_slice(&index.to_le_bytes());
                socket.borrow_mut().options.insert((level, name), kept);
                return Some(Ok(0));
            }
            let value = match int_value(bytes, level) {
                Ok(value) => value,
                Err(error) => return Some(Err(error)),
            };
            match name {
                IP_TTL => {
                    if bytes.is_empty() {
                        return Some(Err(Errno::EINVAL));
                    }
                    if value != -1 && !(1..=255).contains(&value) {
                        return Some(Err(Errno::EINVAL));
                    }
                    store(socket, value)
                }
                // `__ip_sock_set_tos`: a new type of service sets the priority it gives.
                IP_TOS => {
                    let value = if protocol == Protocol::Tcp {
                        value & !3
                    } else {
                        value
                    } & 0xff;
                    let old = socket
                        .borrow()
                        .options
                        .get(&(level, name))
                        .map_or(0, |bytes| bytes[0]);
                    if old != value as u8 {
                        let priority = tos_priority(value as u8);
                        socket
                            .borrow_mut()
                            .options
                            .insert((SOL_SOCKET, SO_PRIORITY), priority.to_le_bytes().to_vec());
                    }
                    store(socket, value)
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
                // Turning the error option off empties the error queue.
                IP_RECVERR => {
                    if value == 0 {
                        inet(socket).errors.clear();
                    }
                    store(socket, i32::from(value != 0))
                }
                IP_FREEBIND
                | IP_TRANSPARENT
                | IP_BIND_ADDRESS_NO_PORT
                | IP_MULTICAST_LOOP
                | IP_MULTICAST_ALL
                | IP_NODEFRAG
                | IP_PKTINFO
                | IP_RECVTTL
                | IP_RECVTOS => store(socket, i32::from(value != 0)),
                // No IP options are ever sent, no security context is given and no route here
                // fragments, so what these would add to a receive never comes.
                IP_RECVOPTS | IP_RETOPTS | IP_PASSSEC | IP_RECVFRAGSIZE => {
                    store(socket, i32::from(value != 0))
                }
                // `ip_ra_control`: raw sockets only.
                IP_ROUTER_ALERT => Some(Err(Errno::EINVAL)),
                // The iptables tables want CAP_NET_ADMIN.
                64 | 65 => Some(Err(Errno::EPERM)),
                _ => not_built(kernel, cx),
            }
        }
        SOL_IPV6 if family == AF_INET6 => {
            let value = match int_value(bytes, level) {
                Ok(value) => value,
                Err(error) => return Some(Err(error)),
            };
            match name {
                // A connection to an IPv4-mapped address becomes an IPv4 socket.
                IPV6_ADDRFORM => {
                    if value != AF_INET as i32 {
                        return Some(Err(Errno::EINVAL));
                    }
                    if protocol == Protocol::Udp && inet_ref(socket).cork.is_some() {
                        return Some(Err(Errno::EBUSY));
                    }
                    if !established(socket) {
                        return Some(Err(Errno::ENOTCONN));
                    }
                    let mapped = inet_ref(socket)
                        .remote
                        .is_some_and(|remote| v4_of(&remote.address).is_some());
                    if inet_ref(socket).v6only || !mapped {
                        return Some(Err(Errno::EADDRNOTAVAIL));
                    }
                    socket.borrow_mut().family = AF_INET;
                    Some(Ok(0))
                }
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
                IPV6_RECVERR => {
                    if value == 0 {
                        inet(socket).errors.clear();
                    }
                    store(socket, i32::from(value != 0))
                }
                IPV6_MULTICAST_LOOP | IPV6_FREEBIND | IPV6_TRANSPARENT | IPV6_DONTFRAG
                | IPV6_RECVPKTINFO | IPV6_RECVHOPLIMIT | IPV6_RECVTCLASS => {
                    store(socket, i32::from(value != 0))
                }
                // No extension headers are ever sent, no route here fragments or changes its
                // path MTU, and flow labels are never read: what these would add never comes.
                IPV6_RECVHOPOPTS
                | IPV6_RECVRTHDR
                | IPV6_RECVDSTOPTS
                | IPV6_RECVPATHMTU
                | IPV6_RECVFRAGSIZE
                | IPV6_AUTOFLOWLABEL
                | IPV6_ROUTER_ALERT_ISOLATE => store(socket, i32::from(value != 0)),
                // `ip6_ra_control`: raw sockets only.
                IPV6_ROUTER_ALERT => Some(Err(Errno::ENOPROTOOPT)),
                // The ip6tables tables want CAP_NET_ADMIN.
                64 | 65 => Some(Err(Errno::EPERM)),
                _ => not_built(kernel, cx),
            }
        }
        _ => Some(Err(Errno::ENOPROTOOPT)),
    }
}

/// getsockopt for an internet socket's own levels and its own SOL_SOCKET options; `None` for an
/// option the common code answers.
/// Whether an internet socket's protocol reads options of `level` at all; if not, what it
/// answers before reading the length (`ip_getsockopt`'s EOPNOTSUPP, `ipv6_getsockopt`'s
/// ENOPROTOOPT).
pub(super) fn level_known(socket: &SocketRef, level: u32) -> Result<(), Errno> {
    let family = socket.borrow().family;
    let protocol = inet_ref(socket).protocol;
    let known = level == SOL_SOCKET
        || level == SOL_IP
        || (level == SOL_IPV6 && family == AF_INET6)
        || (level == SOL_TCP && protocol == Protocol::Tcp)
        || (level == SOL_UDP && protocol == Protocol::Udp);
    match known {
        true => Ok(()),
        false if family == AF_INET6 => Err(Errno::ENOPROTOOPT),
        false => Err(Errno::EOPNOTSUPP),
    }
}

pub(super) fn get_option<S: crate::System>(
    kernel: &mut Kernel,
    cx: &mut Context<'_, S>,
    socket: &SocketRef,
    level: u32,
    name: u32,
    room: i32,
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
    let not_built = |kernel: &mut Kernel, cx: &mut Context<'_, S>| {
        Some(Err(kernel.counters.refuse(
            cx.host,
            crate::reason::OPTION_NOT_BUILT,
            (u64::from(level) << 32) | u64::from(name),
            Errno::ENOPROTOOPT,
        )))
    };
    match level {
        SOL_SOCKET => match name {
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
            TCP_QUICKACK => int(i32::from(!inet_ref(socket).pingpong)),
            TCP_KEEPIDLE => kept(7200),
            TCP_KEEPINTVL => kept(75),
            TCP_KEEPCNT => kept(9),
            TCP_SYNCNT => kept(6),
            // 0 reads as the system's FIN timeout (`tcp_fin_timeout`, 60 s).
            TCP_LINGER2 => kept(0).map(|bytes| {
                bytes.map(|bytes| {
                    match i32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")) {
                        0 => 60i32.to_le_bytes().to_vec(),
                        _ => bytes,
                    }
                })
            }),
            TCP_MAXSEG => {
                let set = socket
                    .borrow()
                    .options
                    .get(&(level, name))
                    .map(|bytes| i32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")))
                    .filter(|&value| value != 0);
                let mss = send_mss(family, &inet_ref(socket).traffic) as i32;
                let syn_sent = inet_ref(socket).connecting.is_some();
                int(match (connected, set) {
                    (true, Some(value)) => value.min(mss),
                    (true, None) => mss,
                    (false, None) if syn_sent => TCP_MSS_SYN_SENT as i32,
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
            TCP_INFO => Some(Ok(tcp_info(kernel, cx, socket))),
            // Deprecated and always off; no repair mode (it wants CAP_NET_ADMIN), so its queue,
            // sequence and window cannot be read; never MPTCP.
            TCP_THIN_DUPACK | TCP_REPAIR | TCP_IS_MPTCP => int(0),
            TCP_REPAIR_QUEUE | TCP_QUEUE_SEQ => Some(Err(Errno::EINVAL)),
            TCP_REPAIR_WINDOW if room != 20 => Some(Err(Errno::EINVAL)),
            TCP_REPAIR_WINDOW | TCP_AO_REPAIR => Some(Err(Errno::EPERM)),
            // `tcp_clock_ms`: the monotonic clock in milliseconds, with no offset here.
            TCP_TIMESTAMP => int((kernel.entry_now / 1_000_000) as u32 as i32),
            // Cubic keeps no information to give, and no SYN is saved, no upper layer protocol
            // set and no fast open key made: nothing.
            TCP_CC_INFO | TCP_SAVED_SYN | TCP_ULP | TCP_FASTOPEN_KEY => Some(Ok(Vec::new())),
            TCP_SAVE_SYN | TCP_TX_DELAY => kept(0),
            TCP_ZEROCOPY_RECEIVE if room < 16 => Some(Err(Errno::EINVAL)),
            TCP_RTO_MAX_MS => kept(120_000),
            TCP_RTO_MIN_US => kept(200_000),
            _ => not_built(kernel, cx),
        },
        SOL_UDP if protocol == Protocol::Udp => {
            use crate::linux_extra::{
                UDP_CORK, UDP_ENCAP, UDP_GRO, UDP_NO_CHECK6_RX, UDP_NO_CHECK6_TX, UDP_SEGMENT,
                UDPLITE_RECV_CSCOV, UDPLITE_SEND_CSCOV,
            };
            match name {
                UDP_CORK | UDP_NO_CHECK6_TX | UDP_NO_CHECK6_RX | UDP_GRO | UDP_SEGMENT => kept(0),
                // `udp_lib_getsockopt`: plain UDP's coverage is always whole, and no
                // encapsulation can be set here (UDP_ENCAP is not built), so each reads 0.
                UDPLITE_SEND_CSCOV | UDPLITE_RECV_CSCOV | UDP_ENCAP => int(0),
                _ => not_built(kernel, cx),
            }
        }
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
            | IP_NODEFRAG
            | IP_PKTINFO
            | IP_RECVTTL
            | IP_RECVTOS => kept(0),
            IP_MULTICAST_TTL | IP_MULTICAST_LOOP | IP_MULTICAST_ALL => kept(1),
            IP_MTU_DISCOVER => kept(IP_PMTUDISC_WANT as i32),
            IP_MTU => {
                if !connected {
                    return Some(Err(Errno::ENOTCONN));
                }
                int(ROUTE_MTU)
            }
            IP_OPTIONS => Some(Ok(Vec::new())),
            IP_MULTICAST_IF => {
                let own = socket.borrow();
                let kept = own.options.get(&(level, name));
                Some(Ok(kept.map_or_else(
                    || alloc::vec![0; 4],
                    |value| value[..4].to_vec(),
                )))
            }
            IP_HDRINCL => kept(0),
            IP_RECVOPTS | IP_RETOPTS | IP_PASSSEC | IP_RECVFRAGSIZE => kept(0),
            // Off, and never set here (setting them is counted as not built).
            IP_ROUTER_ALERT | IP_RECVORIGDSTADDR | IP_MINTTL | IP_CHECKSUM | IP_RECVERR_RFC4884
            | IP_UNICAST_IF | IP_LOCAL_PORT_RANGE => int(0),
            // TCP's saved packet information: none asked for gives nothing.
            IP_PKTOPTIONS if protocol != Protocol::Tcp => Some(Err(Errno::ENOPROTOOPT)),
            IP_PKTOPTIONS
                if !flag(&socket.borrow(), SOL_IP, IP_PKTINFO)
                    && !flag(&socket.borrow(), SOL_IP, IP_RECVTTL) =>
            {
                Some(Ok(Vec::new()))
            }
            // `inet_num`: the local port, for these protocols.
            IP_PROTOCOL => int(i32::from(inet_ref(socket).local.port)),
            // The iptables tables want CAP_NET_ADMIN.
            64..=67 => Some(Err(Errno::EPERM)),
            // `getorigdst`: no connection here went through address translation.
            SO_ORIGINAL_DST if protocol != Protocol::Tcp => Some(Err(Errno::ENOPROTOOPT)),
            SO_ORIGINAL_DST if room < 16 => Some(Err(Errno::EINVAL)),
            SO_ORIGINAL_DST => Some(Err(Errno::ENOENT)),
            _ => not_built(kernel, cx),
        },
        SOL_IPV6 if family == AF_INET6 => match name {
            IPV6_ADDRFORM if !established(socket) => Some(Err(Errno::ENOTCONN)),
            IPV6_ADDRFORM => int(AF_INET6 as i32),
            IPV6_V6ONLY => int(i32::from(inet_ref(socket).v6only)),
            IPV6_UNICAST_HOPS => {
                let hops = kept(-1)?.ok()?;
                let hops = i32::from_le_bytes(hops[..4].try_into().expect("4 bytes"));
                int(if hops == -1 { 64 } else { hops })
            }
            IPV6_MULTICAST_HOPS | IPV6_MULTICAST_LOOP => kept(1),
            IPV6_TCLASS | IPV6_RECVERR | IPV6_FREEBIND | IPV6_TRANSPARENT | IPV6_DONTFRAG
            | IPV6_RECVPKTINFO | IPV6_RECVHOPLIMIT | IPV6_RECVTCLASS => kept(0),
            IPV6_MTU_DISCOVER => kept(IPV6_PMTUDISC_WANT as i32),
            IPV6_MTU => {
                if !connected {
                    return Some(Err(Errno::ENOTCONN));
                }
                int(ROUTE_MTU6)
            }
            IPV6_RECVHOPOPTS
            | IPV6_RECVRTHDR
            | IPV6_RECVDSTOPTS
            | IPV6_RECVPATHMTU
            | IPV6_RECVFRAGSIZE
            | IPV6_ROUTER_ALERT_ISOLATE => kept(0),
            IPV6_AUTOFLOWLABEL | IPV6_MULTICAST_ALL => kept(1),
            // Off, and never set here (setting them is counted as not built).
            IPV6_2292PKTINFO | IPV6_2292HOPOPTS | IPV6_2292DSTOPTS | IPV6_2292RTHDR
            | IPV6_2292HOPLIMIT | IPV6_FLOWINFO | IPV6_MULTICAST_IF | IPV6_ROUTER_ALERT
            | IPV6_RECVERR_RFC4884 | IPV6_FLOWINFO_SEND | IPV6_MINHOPCOUNT
            | IPV6_RECVORIGDSTADDR | IPV6_UNICAST_IF => int(0),
            // `IPV6_PREFER_SRC_PUBTMP_DEFAULT | IPV6_PREFER_SRC_HOME`, as `ipv6_getsockopt`.
            IPV6_ADDR_PREFERENCES => int(0x500),
            // No sticky extension headers: nothing.
            IPV6_HOPOPTS | IPV6_RTHDRDSTOPTS | IPV6_RTHDR | IPV6_DSTOPTS => Some(Ok(Vec::new())),
            // TCP's saved packet information: none asked for gives nothing.
            IPV6_2292PKTOPTIONS if protocol != Protocol::Tcp => Some(Err(Errno::ENOPROTOOPT)),
            IPV6_2292PKTOPTIONS
                if !flag(&socket.borrow(), SOL_IPV6, IPV6_RECVPKTINFO)
                    && !flag(&socket.borrow(), SOL_IPV6, IPV6_RECVHOPLIMIT)
                    && !flag(&socket.borrow(), SOL_IPV6, IPV6_RECVTCLASS) =>
            {
                Some(Ok(Vec::new()))
            }
            // `struct ip6_mtuinfo`: no address, and the route's MTU.
            IPV6_PATHMTU => {
                if !connected {
                    return Some(Err(Errno::ENOTCONN));
                }
                let mut bytes = alloc::vec![0u8; 28];
                bytes.extend_from_slice(&(ROUTE_MTU6 as u32).to_le_bytes());
                Some(Ok(bytes))
            }
            // The ip6tables tables want CAP_NET_ADMIN.
            64 | 65 | 68 | 69 => Some(Err(Errno::EPERM)),
            // `ipv6_getorigdst`: no connection here went through address translation.
            IP6T_SO_ORIGINAL_DST if protocol != Protocol::Tcp => Some(Err(Errno::ENOPROTOOPT)),
            IP6T_SO_ORIGINAL_DST if room < 28 => Some(Err(Errno::EINVAL)),
            IP6T_SO_ORIGINAL_DST => Some(Err(Errno::ENOENT)),
            _ => not_built(kernel, cx),
        },
        _ => Some(Err(Errno::EOPNOTSUPP)),
    }
}
