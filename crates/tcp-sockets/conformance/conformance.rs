//! The tests every implementation of wcontainers-tcp-sockets runs, written against its traits alone.
//!
//! Each check makes its own listener on a loopback address with a port the kernel chooses, so the checks run side by
//! side. Nothing in the contract waits, so an answer of `WouldBlock` is tried again until [`DEADLINE`].
//!
//! The checks watch the wire through the host's packet and raw sockets, so they run as root on a Linux host, in a
//! network namespace of their own whose loopback is up with the `fq` queueing discipline (which honours a transmit
//! delay), with `net.ipv4.tcp_fastopen` at 3 (fast open for clients and listeners), `net.ipv6.flowlabel_consistency`
//! at 0 (which flow label reflection needs) and the `dctcp` congestion control available.
#![cfg(target_os = "linux")]

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use core::num::NonZeroU32;
use core::time::Duration;
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use etherparse::{
    Icmpv4Header, Icmpv4Slice, Icmpv4Type, Icmpv6Header, Icmpv6Slice, Icmpv6Type, IpDscp, IpEcn,
    IpNumber, Ipv4HeaderSlice, Ipv6FlowLabel, Ipv6Header, Ipv6HeaderSlice, TcpHeaderSlice,
    TcpOptionElement, icmpv4, icmpv6,
};
use rustix::net::{AddressFamily, RecvFlags, SendFlags, SocketType, ipproto, sockopt};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    AddressSharing, BindAddress, ByteCount, BytesPerSecond, FilterInstruction, FilterJump,
    FilterOperand, FilterOperation, FlowLabelChoice, HopLimit, IpPacketSetting, IpSocketSetting,
    IpVersion, Ipv6Families, LocalBinding, LocalPort, NonlocalAddresses, PacketPriority,
    PathMtuDiscovery, Port, PortChoice, PortRange, PortSetting, PortSharing, ReceivedPacketFields,
    Report, RouteScope, SocketFilter, TimestampSource, Timestamping, TransmitEvent,
    TransmitReportKey, TransmitReportRequest, TransmitStage,
};
use wcontainers_kernel::UserId;
use wcontainers_kernel::errno::Errno;
use wcontainers_tcp_sockets::repair::{
    TcpRepair, TcpTimestamp, TcpTimestampClock, TcpTimestampUnit,
};
use wcontainers_tcp_sockets::settings::{
    BufferLocks, BufferTuning, FAST_OPEN_KEY_LENGTH, FastOpenKey, FastOpenKeys, MaximumSegmentSize,
    ProbeCount, RetryCount, TcpFinWait2, TcpListenerSetting, TcpQuickAcknowledgement,
    TcpSavedSynchronize, TcpSetting, TransmitRehash, WindowSize,
};
use wcontainers_tcp_sockets::state::TcpErrorQueueEntry;
use wcontainers_tcp_sockets::{
    ListenBacklog, TcpClose, TcpConnectStart, TcpContinuation, TcpFastOpen, TcpListening,
    TcpProtocol, TcpPushOverride, TcpReceiveRequest, TcpReceived, TcpRecordBoundary,
    TcpSendRequest, TcpShutdown, TcpSockets, TcpUrgency,
};

/// How long a check waits for something the network does.
pub const DEADLINE: Duration = Duration::from_secs(10);

/// How long data held back must stay unseen by the peer, well inside the 200 ms ceiling of a cork.
const HELD_FOR: Duration = Duration::from_millis(30);

/// How soon pushed data must arrive, under the cork's 200 ms ceiling so that the push and not the ceiling sent it.
const PUSHED_WITHIN: Duration = Duration::from_millis(150);

/// The owner of a container's first process, root.
const OWNER: UserId = UserId(0);

const BACKLOG: ListenBacklog = ListenBacklog(16);

/// The bytes most checks send.
const MESSAGE: &[u8] = b"conformance";

/// The bytes sent back the other way.
const REPLY: &[u8] = b"reply";

/// The urgent byte of the urgent data checks.
const URGENT: &[u8] = b"!";

/// How far into the unread bytes a peek at an offset starts.
const PEEK_OFFSET: usize = 2;

/// How many bytes a read takes when the rest must stay unread.
const READ_PART: usize = 4;

/// Room for the IP and TCP headers of a SYN.
const HEADER_ROOM: usize = 256;

/// The room a SYN's header is cut to.
const CUT_HEADER: usize = 3;

/// Room for the packet of an error queue entry.
const ENTRY_ROOM: usize = 64;

/// Room for an ICMP error whole from its IP header, or a transmit report's packet from its link header.
const PACKET_ROOM: usize = 512;

/// How long loopback takes at most to deliver what was sent.
const SETTLE: Duration = Duration::from_millis(50);

/// A loopback address that connections to it do not leave from, so that their two ends differ.
const OTHER_LOOPBACK: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 2);

/// The hop limit of the IPv6 header an ICMPv6 error quotes.
const QUOTED_HOP_LIMIT: u8 = 64;

/// The bytes the order check sends one way.
const TRANSFER: usize = 1 << 20;

const KEY: TransmitReportKey = TransmitReportKey(7);

const SEGMENT: MaximumSegmentSize = MaximumSegmentSize(1000);

/// The range the exit node chooses ports from, and the reserved part at its start.
const CHOICE_FIRST: Port = Port(41000);
const RESERVED_LAST: Port = Port(41010);
const CHOICE_LAST: Port = Port(41020);

/// The same above Linux's default local port range, which ends at 60999.
const HIGH_CHOICE_FIRST: Port = Port(61000);
const HIGH_RESERVED_LAST: Port = Port(61010);
const HIGH_CHOICE_LAST: Port = Port(61020);

const FAST_OPEN_QUEUE: Option<NonZeroU32> = NonZeroU32::new(8);

/// A time, a count and a size every setting of their kind accepts.
const SETTING_TIME: Duration = Duration::from_secs(10);
const SHORT_SETTING_TIME: Duration = Duration::from_millis(40);
const SETTING_COUNT: u8 = 4;
const SETTING_BYTES: usize = 1 << 16;

/// A congestion control algorithm every Linux has.
const CONGESTION_CONTROL: &str = "reno";

/// Classic BPF's return of its operand, which ends a filter.
const RETURN_OPERAND: FilterOperation = FilterOperation((libc::BPF_RET | libc::BPF_K) as u16);

/// A filter that keeps every packet whole.
static ACCEPT_ALL: [FilterInstruction; 1] = [FilterInstruction {
    operation: RETURN_OPERAND,
    jump_if_true: FilterJump(0),
    jump_if_false: FilterJump(0),
    operand: FilterOperand(u32::MAX),
}];

/// A filter that drops every packet.
static DROP_ALL: [FilterInstruction; 1] = [FilterInstruction {
    operation: RETURN_OPERAND,
    jump_if_true: FilterJump(0),
    jump_if_false: FilterJump(0),
    operand: FilterOperand(0),
}];

/// How long a fill waits for a send to take anything before it counts the connection full.
const FILLED_AFTER: Duration = Duration::from_millis(100);

/// How many times less a setting that bounds a buffer lets through than the default buffers.
const BOUND_RATIO: usize = 4;

/// A receive low water mark that widens the receive buffer to hold it.
const WIDE_LOW_WATER: usize = 1 << 20;

/// How far from its time a timer of a second or so may fire on a loaded machine.
const TIMER_SLACK: Duration = Duration::from_millis(400);

const KEEP_ALIVE_IDLE: Duration = Duration::from_secs(2);
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(1);

/// The longest a quick acknowledgement takes on loopback, below the shortest delay of a delayed one.
const QUICK_WITHIN: Duration = Duration::from_millis(20);

const DELAYED_ACKNOWLEDGEMENT_MAXIMUM: Duration = Duration::from_millis(5);

/// The longest retransmission timeout of the check of the maximum, and how long the checks watch retransmissions.
const RETRANSMISSION_CEILING: Duration = Duration::from_secs(1);
const RETRANSMISSIONS_WATCHED: Duration = Duration::from_millis(3200);

/// How many times a segment leaves within the given time when its timeouts do not grow, more than when they double.
const LINEAR_TRANSMISSIONS: usize = 6;
const LINEAR_TRANSMISSIONS_WITHIN: Duration = Duration::from_millis(1200);

const USER_TIMEOUT: Duration = Duration::from_millis(500);

const FIN_WAIT_2_LASTS: Duration = Duration::from_secs(1);

/// A congestion control that marks what it sends as capable of explicit congestion notification.
const NOTIFYING_CONGESTION_CONTROL: &str = "dctcp";

/// A pacing rate and a transfer it spreads over more than the given time.
const PACING_RATE: BytesPerSecond = BytesPerSecond(1 << 20);
const PACED_BYTES: usize = 1 << 19;
const PACED_AT_LEAST: Duration = Duration::from_millis(100);

const TRANSMIT_DELAY: Duration = Duration::from_millis(50);

/// An address of the documentation range, which no interface holds.
const NONLOCAL: Ipv4Addr = Ipv4Addr::new(192, 0, 2, 1);

/// Loopback addresses of the fast open checks, one each.
const FAST_OPEN_WITH_DATA: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 3);
const FAST_OPEN_UNTIL_FIRST_SEND: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 4);
const FAST_OPEN_WITHOUT_COOKIE: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 5);
const FAST_OPEN_KEYS: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 6);

/// A TCP timestamp clock reading, in milliseconds.
const CLOCK_READING: u32 = 1 << 30;

fn errno(errno: Errno) -> HostError {
    HostError::Errno(errno as i32)
}

/// Tries the operation again while it answers `WouldBlock`, until the deadline.
fn retry<T>(mut operation: impl FnMut() -> Result<T, HostError>) -> Result<T, HostError> {
    let start = Instant::now();
    loop {
        match operation() {
            Err(HostError::WouldBlock) if start.elapsed() < DEADLINE => {
                std::thread::sleep(Duration::from_millis(1))
            }
            outcome => return outcome,
        }
    }
}

fn bind_any_port(address: BindAddress) -> LocalBinding<'static> {
    LocalBinding {
        address,
        port: LocalPort::Requested(Port(0)),
    }
}

fn ipv4_loopback() -> BindAddress {
    BindAddress::V4(Ipv4Addr::LOCALHOST)
}

fn ipv4_any() -> BindAddress {
    BindAddress::V4(Ipv4Addr::UNSPECIFIED)
}

fn no_reports() -> TransmitReportRequest {
    TransmitReportRequest {
        key: TransmitReportKey(0),
        scheduled: Report::NotRequested,
        software: Report::NotRequested,
        hardware: Report::NotRequested,
        acknowledgement: Report::NotRequested,
        completion: Report::NotRequested,
        wireless_status: Report::NotRequested,
        packet: Report::NotRequested,
    }
}

fn plain() -> TcpSendRequest {
    TcpSendRequest {
        urgency: TcpUrgency::Normal,
        continuation: TcpContinuation::Push,
        record: TcpRecordBoundary::Open,
        transmit_reports: no_reports(),
        transmit_statistics: Report::NotRequested,
    }
}

/// A listener, a connection to it and the connection it accepted.
struct Pair<S: TcpSockets> {
    listener: S::Listener,
    local: SocketAddr,
    client: S::Connection,
    server: S::Connection,
}

fn listen<S: TcpSockets>(
    sockets: &S,
    address: BindAddress,
    settings: &[TcpListenerSetting<'_>],
) -> (S::Listener, SocketAddr) {
    let listening = sockets
        .listen(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(address),
            BACKLOG,
            settings,
        )
        .expect("listen on loopback");
    (listening.listener, listening.local)
}

fn accept<S: TcpSockets>(sockets: &S, listener: &S::Listener) -> S::Connection {
    let mut header = [0; 0];
    retry(|| {
        sockets
            .accept(listener, &mut header)
            .map(|accepted| accepted.connection)
    })
    .expect("accept the connection")
}

/// What an accept gave, the SYN's header copied out of the buffer it was cut to.
struct Accepted<C> {
    connection: C,
    peer: SocketAddr,
    local: SocketAddr,
    synchronize: ReceivedPacketFields,
    synchronize_header: Vec<u8>,
}

fn accept_with_header<S: TcpSockets>(
    sockets: &S,
    listener: &S::Listener,
    room: usize,
) -> Accepted<S::Connection> {
    let mut header = vec![0; room];
    retry(|| {
        sockets
            .accept(listener, &mut header)
            .map(|accepted| Accepted {
                connection: accepted.connection,
                peer: accepted.peer,
                local: accepted.local,
                synchronize: accepted.synchronize,
                synchronize_header: accepted.synchronize_header.to_vec(),
            })
    })
    .expect("accept")
}

/// Sends the bytes from one connection while reading them at the other, returning what arrived.
fn transfer<S: TcpSockets>(
    sockets: &S,
    from: &S::Connection,
    to: &S::Connection,
    bytes: &[u8],
) -> Vec<u8> {
    let mut sent = 0;
    let mut received = vec![0; bytes.len()];
    let mut length = 0;
    let start = Instant::now();
    while length < bytes.len() && start.elapsed() < DEADLINE {
        if sent < bytes.len() {
            match sockets.send(from, plain(), &bytes[sent..]) {
                Ok(taken) => sent += taken.0,
                Err(HostError::WouldBlock) => {}
                Err(error) => panic!("send: {error:?}"),
            }
        }
        match sockets.receive(to, TcpReceiveRequest::Read, &mut received[length..]) {
            Ok(got) if got.length.0 == 0 => break,
            Ok(got) => length += got.length.0,
            Err(HostError::WouldBlock) => {}
            Err(error) => panic!("receive: {error:?}"),
        }
    }
    received.truncate(length);
    received
}

fn pair_with<S: TcpSockets>(
    sockets: &S,
    client_settings: &[TcpSetting<'_>],
    listener_settings: &[TcpListenerSetting<'_>],
) -> Pair<S> {
    let (listener, local) = listen(sockets, ipv4_loopback(), listener_settings);
    let client = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            client_settings,
            TcpFastOpen::Off,
        )
        .expect("connect to the listener")
        .connection;
    let server = accept(sockets, &listener);
    Pair {
        listener,
        local,
        client,
        server,
    }
}

fn pair<S: TcpSockets>(sockets: &S) -> Pair<S> {
    pair_with(sockets, &[], &[])
}

fn close<S: TcpSockets>(sockets: &S, pair: Pair<S>) {
    sockets.close(pair.client, TcpClose::Orderly);
    sockets.close(pair.server, TcpClose::Orderly);
    sockets.stop_listening(pair.listener);
}

fn send_all<S: TcpSockets>(
    sockets: &S,
    connection: &S::Connection,
    request: TcpSendRequest,
    bytes: &[u8],
) {
    let mut sent = 0;
    while sent < bytes.len() {
        let taken = retry(|| sockets.send(connection, request, &bytes[sent..])).expect("send");
        sent += taken.0;
    }
}

/// Reads until the given number of bytes or the end of the stream.
fn receive_up_to<S: TcpSockets>(sockets: &S, connection: &S::Connection, wanted: usize) -> Vec<u8> {
    let mut received = vec![0; wanted];
    let mut length = 0;
    while length < wanted {
        let got =
            retry(|| sockets.receive(connection, TcpReceiveRequest::Read, &mut received[length..]))
                .expect("receive");
        if got.length.0 == 0 {
            break;
        }
        length += got.length.0;
    }
    received.truncate(length);
    received
}

/// Peeks until the given number of bytes wait to be read.
fn wait_for_unread<S: TcpSockets>(sockets: &S, connection: &S::Connection, wanted: usize) {
    let mut buffer = vec![0; wanted];
    let start = Instant::now();
    loop {
        match sockets.receive(
            connection,
            TcpReceiveRequest::Peek {
                offset: ByteCount(0),
            },
            &mut buffer,
        ) {
            Ok(got) if got.length.0 >= wanted => return,
            Ok(_) | Err(HostError::WouldBlock) if start.elapsed() < DEADLINE => {
                std::thread::sleep(Duration::from_millis(1))
            }
            outcome => panic!("waiting for {wanted} unread bytes: {outcome:?}"),
        }
    }
}

/// Asserts that nothing arrives on the connection for the given time.
fn assert_silent<S: TcpSockets>(sockets: &S, connection: &S::Connection, period: Duration) {
    let start = Instant::now();
    let mut buffer = [0; 1];
    while start.elapsed() < period {
        let outcome = sockets.receive(
            connection,
            TcpReceiveRequest::Peek {
                offset: ByteCount(0),
            },
            &mut buffer,
        );
        assert_eq!(
            outcome.map(|got| got.length),
            Err(HostError::WouldBlock),
            "data arrived while held back"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Asserts that the bytes arrive within the given time.
fn assert_arrives_within<S: TcpSockets>(
    sockets: &S,
    connection: &S::Connection,
    bytes: &[u8],
    within: Duration,
) {
    let start = Instant::now();
    let mut buffer = vec![0; bytes.len()];
    let got = retry(|| sockets.receive(connection, TcpReceiveRequest::Read, &mut buffer))
        .expect("receive");
    assert!(start.elapsed() < within, "took {:?}", start.elapsed());
    assert_eq!(&buffer[..got.length.0], bytes);
}

fn pattern(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| (index % usize::from(u8::MAX)) as u8)
        .collect()
}

/// Sends a byte each way and checks that both arrive.
fn assert_carries_bytes<S: TcpSockets>(sockets: &S, pair: &Pair<S>) {
    send_all(sockets, &pair.client, plain(), MESSAGE);
    assert_eq!(receive_up_to(sockets, &pair.server, MESSAGE.len()), MESSAGE);
    send_all(sockets, &pair.server, plain(), REPLY);
    assert_eq!(receive_up_to(sockets, &pair.client, REPLY.len()), REPLY);
}

/// One TCP segment loopback carried, as the host's packet socket saw it.
#[derive(Clone, Debug)]
struct Segment {
    at: Instant,
    to_listener: bool,
    hop_limit: u8,
    /// The IPv4 header's DSCP and ECN fields.
    dscp: Option<IpDscp>,
    ecn: Option<IpEcn>,
    flow_label: Option<u32>,
    dont_fragment: bool,
    sequence: u32,
    acknowledgement: u32,
    window: u16,
    synchronize: bool,
    finish: bool,
    reset: bool,
    payload: usize,
    timestamp: Option<u32>,
}

impl Segment {
    fn parse(packet: &[u8], listener: SocketAddr, at: Instant) -> Option<Self> {
        let (source, destination, hop_limit, dscp, ecn, flow_label, dont_fragment, transport) =
            if let Ok(header) = Ipv4HeaderSlice::from_slice(packet) {
                let end = usize::from(header.total_len()).min(packet.len());
                (header.protocol() == IpNumber::TCP).then_some((
                    IpAddr::V4(header.source_addr()),
                    IpAddr::V4(header.destination_addr()),
                    header.ttl(),
                    Some(header.dcp()),
                    Some(header.ecn()),
                    None,
                    header.dont_fragment(),
                    &packet[header.slice().len()..end],
                ))?
            } else {
                let header = Ipv6HeaderSlice::from_slice(packet).ok()?;
                let end =
                    (Ipv6Header::LEN + usize::from(header.payload_length())).min(packet.len());
                (header.next_header() == IpNumber::TCP).then_some((
                    IpAddr::V6(header.source_addr()),
                    IpAddr::V6(header.destination_addr()),
                    header.hop_limit(),
                    None,
                    None,
                    Some(header.flow_label().value()),
                    false,
                    &packet[Ipv6Header::LEN..end],
                ))?
            };
        let tcp = TcpHeaderSlice::from_slice(transport).ok()?;
        let to_listener = SocketAddr::new(destination, tcp.destination_port()) == listener;
        if !to_listener && SocketAddr::new(source, tcp.source_port()) != listener {
            return None;
        }
        Some(Segment {
            at,
            to_listener,
            hop_limit,
            dscp,
            ecn,
            flow_label,
            dont_fragment,
            sequence: tcp.sequence_number(),
            acknowledgement: tcp.acknowledgment_number(),
            window: tcp.window_size(),
            synchronize: tcp.syn(),
            finish: tcp.fin(),
            reset: tcp.rst(),
            payload: transport.len() - tcp.slice().len(),
            timestamp: tcp.options_iterator().find_map(|option| match option {
                Ok(TcpOptionElement::Timestamp(value, _)) => Some(value),
                _ => None,
            }),
        })
    }

    /// The sequence number that follows this segment.
    fn end(&self) -> u32 {
        let flags = u32::from(self.synchronize) + u32::from(self.finish);
        self.sequence.wrapping_add(self.payload as u32 + flags)
    }

    fn is_data_to_listener(&self) -> bool {
        self.to_listener && self.payload > 0
    }
}

/// The segments to and from one listener, gathered by a thread reading the host's packet socket, which
/// sees each loopback packet twice: once leaving and once arriving.
struct Capture {
    segments: Arc<Mutex<Vec<Segment>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// How long apart the two copies of one loopback packet may arrive at the packet socket.
const COPY_GAP: Duration = Duration::from_millis(5);

/// The buffer of a capture's packet socket, large enough for every check's traffic at once.
const CAPTURE_BUFFER: usize = 1 << 24;

impl Capture {
    fn start(listener: SocketAddr) -> Self {
        let socket = rustix::net::socket(
            AddressFamily::PACKET,
            SocketType::DGRAM,
            Some(rustix::net::eth::ALL),
        )
        .expect("a packet socket (the checks that watch the wire need root)");
        sockopt::set_socket_recv_buffer_size_force(&socket, CAPTURE_BUFFER)
            .expect("a capture buffer");
        sockopt::set_socket_timeout(&socket, sockopt::Timeout::Recv, Some(COPY_GAP))
            .expect("a receive timeout");
        let segments = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = std::thread::spawn({
            let segments = segments.clone();
            let stop = stop.clone();
            move || {
                let mut buffer = vec![0; usize::from(u16::MAX)];
                let mut recent: Vec<(Instant, Vec<u8>)> = Vec::new();
                while !stop.load(Ordering::Relaxed) {
                    let Ok((length, _)) =
                        rustix::net::recv(&socket, &mut buffer, RecvFlags::empty())
                    else {
                        continue;
                    };
                    let at = Instant::now();
                    let packet = &buffer[..length];
                    let Some(segment) = Segment::parse(packet, listener, at) else {
                        continue;
                    };
                    recent.retain(|(seen, _)| at.duration_since(*seen) < COPY_GAP);
                    if let Some(copy) = recent.iter().position(|(_, bytes)| bytes == packet) {
                        recent.swap_remove(copy);
                        continue;
                    }
                    recent.push((at, packet.to_vec()));
                    segments.lock().expect("the capture").push(segment);
                }
            }
        });
        Capture {
            segments,
            stop,
            thread: Some(thread),
        }
    }

    fn segments(&self) -> Vec<Segment> {
        self.segments.lock().expect("the capture").clone()
    }

    fn since(&self, start: Instant) -> Vec<Segment> {
        let mut segments = self.segments();
        segments.retain(|segment| segment.at >= start);
        segments
    }

    /// The first segment since the start that meets the condition, waiting for it up to the given time.
    fn wait_for(
        &self,
        start: Instant,
        within: Duration,
        condition: impl Fn(&Segment) -> bool,
    ) -> Option<Segment> {
        loop {
            if let Some(found) = self.since(start).into_iter().find(&condition) {
                return Some(found);
            }
            if start.elapsed() > within {
                return None;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// The first segment captured that meets the condition, waiting for it up to the deadline.
    fn first(&self, condition: impl Fn(&Segment) -> bool) -> Segment {
        let start = Instant::now();
        loop {
            if let Some(found) = self.segments().into_iter().find(&condition) {
                return found;
            }
            assert!(start.elapsed() < DEADLINE, "no such segment was captured");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// The sequence number the connection's next segment to the listener takes.
    fn next_to_listener(&self) -> u32 {
        self.first(|segment| segment.to_listener);
        self.segments()
            .iter()
            .filter(|segment| segment.to_listener)
            .map(Segment::end)
            .next_back()
            .expect("a segment to the listener")
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn any_of(address: BindAddress) -> BindAddress {
    match address {
        BindAddress::V4(_) => ipv4_any(),
        BindAddress::V6 { families, .. } => BindAddress::V6 {
            address: Ipv6Addr::UNSPECIFIED,
            families,
        },
    }
}

fn ipv6_loopback() -> BindAddress {
    BindAddress::V6 {
        address: Ipv6Addr::LOCALHOST,
        families: Ipv6Families::V6Only,
    }
}

/// A pair on the given loopback address whose segments are captured from the SYN on.
fn watched_pair<S: TcpSockets>(
    sockets: &S,
    address: BindAddress,
    client_settings: &[TcpSetting<'_>],
    listener_settings: &[TcpListenerSetting<'_>],
) -> (Pair<S>, Capture) {
    let (listener, local) = listen(sockets, address, listener_settings);
    let capture = Capture::start(local);
    let client = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(any_of(address)),
            local,
            client_settings,
            TcpFastOpen::Off,
        )
        .expect("connect to the listener")
        .connection;
    let server = accept(sockets, &listener);
    let pair = Pair {
        listener,
        local,
        client,
        server,
    };
    (pair, capture)
}

/// Makes the connection drop every segment that arrives, by a minimum hop limit no packet meets.
fn drop_arrivals<S: TcpSockets>(sockets: &S, connection: &S::Connection, version: IpVersion) {
    sockets
        .configure(
            connection,
            TcpSetting::MinimumHopLimit {
                version,
                minimum: Some(HopLimit(u8::MAX)),
            },
        )
        .expect("a minimum hop limit");
}

/// Sends until the connection takes nothing more for a while, the peer not reading, giving the bytes taken.
fn fill<S: TcpSockets>(sockets: &S, connection: &S::Connection) -> usize {
    let chunk = pattern(SETTING_BYTES);
    let mut taken = 0;
    let mut last = Instant::now();
    while last.elapsed() < FILLED_AFTER {
        match sockets.send(connection, plain(), &chunk) {
            Ok(count) => {
                taken += count.0;
                last = Instant::now();
            }
            Err(HostError::WouldBlock) => std::thread::sleep(Duration::from_millis(1)),
            Err(error) => panic!("send: {error:?}"),
        }
    }
    taken
}

/// The bytes waiting to be read on the connection.
fn unread<S: TcpSockets>(sockets: &S, connection: &S::Connection) -> usize {
    let mut buffer = vec![0; CAPTURE_BUFFER];
    sockets
        .receive(
            connection,
            TcpReceiveRequest::Peek {
                offset: ByteCount(0),
            },
            &mut buffer,
        )
        .map_or(0, |got| got.length.0)
}

/// How long after the send that started it a connection's first acknowledgement of data came.
fn acknowledgement_delay<S: TcpSockets>(
    sockets: &S,
    pair: &Pair<S>,
    capture: &Capture,
) -> Duration {
    let start = Instant::now();
    send_all(sockets, &pair.client, plain(), MESSAGE);
    let data = capture
        .wait_for(start, DEADLINE, Segment::is_data_to_listener)
        .expect("the data segment");
    let acknowledgement = capture
        .wait_for(start, DEADLINE, |segment| {
            !segment.to_listener && segment.acknowledgement == data.end()
        })
        .expect("its acknowledgement");
    acknowledgement.at - data.at
}

/// Waits until the receive answers something other than `WouldBlock`.
fn receive_outcome<S: TcpSockets>(
    sockets: &S,
    connection: &S::Connection,
    within: Duration,
) -> Result<TcpReceived, HostError> {
    let start = Instant::now();
    let mut buffer = [0; ENTRY_ROOM];
    loop {
        match sockets.receive(connection, TcpReceiveRequest::Read, &mut buffer) {
            Err(HostError::WouldBlock) if start.elapsed() < within => {
                std::thread::sleep(Duration::from_millis(1))
            }
            outcome => return outcome,
        }
    }
}

pub fn connect_and_accept_name_both_ends<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[],
            TcpFastOpen::Off,
        )
        .expect("connect");
    assert_eq!(
        connected.start,
        TcpConnectStart::SynchronizeSent {
            fast_open_data_taken: ByteCount(0)
        }
    );
    let accepted = accept_with_header(&sockets, &listener, HEADER_ROOM);
    assert_eq!(accepted.peer, connected.local);
    assert_eq!(accepted.local, local);
    let ipv4 =
        Ipv4HeaderSlice::from_slice(&accepted.synchronize_header).expect("the SYN's IPv4 header");
    assert_eq!(ipv4.protocol(), IpNumber::TCP);
    assert_eq!(IpAddr::V4(ipv4.source_addr()), connected.local.ip());
    assert_eq!(accepted.synchronize.length, usize::from(ipv4.total_len()));
    let tcp = TcpHeaderSlice::from_slice(&accepted.synchronize_header[ipv4.slice().len()..])
        .expect("the SYN's TCP header");
    assert!(tcp.syn() && !tcp.ack());
    assert_eq!(tcp.source_port(), connected.local.port());
    assert_eq!(tcp.destination_port(), local.port());
    sockets.close(accepted.connection, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

pub fn accept_cuts_the_synchronize_header_to_the_buffer<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[],
            TcpFastOpen::Off,
        )
        .expect("connect");
    let accepted = accept_with_header(&sockets, &listener, CUT_HEADER);
    assert_eq!(accepted.synchronize_header.len(), CUT_HEADER);
    assert!(accepted.synchronize.length > CUT_HEADER);
    sockets.close(accepted.connection, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

pub fn connect_settings_apply_to_the_synchronize<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    let segment = SEGMENT;
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[TcpSetting::MaximumSegmentSize(Some(segment))],
            TcpFastOpen::Off,
        )
        .expect("connect");
    let accepted = accept_with_header(&sockets, &listener, HEADER_ROOM);
    let ipv4 = Ipv4HeaderSlice::from_slice(&accepted.synchronize_header).expect("IPv4 header");
    let tcp = TcpHeaderSlice::from_slice(&accepted.synchronize_header[ipv4.slice().len()..])
        .expect("TCP header");
    let announced = tcp.options_iterator().find_map(|option| match option {
        Ok(TcpOptionElement::MaximumSegmentSize(size)) => Some(size),
        _ => None,
    });
    assert_eq!(announced, Some(segment.0));
    sockets.close(accepted.connection, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

pub fn bytes_arrive_in_order_both_ways<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let bytes = pattern(TRANSFER);
    assert!(transfer(&sockets, &pair.client, &pair.server, &bytes) == bytes);
    send_all(&sockets, &pair.server, plain(), REPLY);
    assert_eq!(receive_up_to(&sockets, &pair.client, REPLY.len()), REPLY);
    close(&sockets, pair);
}

pub fn connects_over_ipv6<S: TcpSockets>(sockets: S) {
    let loopback = BindAddress::V6 {
        address: Ipv6Addr::LOCALHOST,
        families: Ipv6Families::V6Only,
    };
    let (listener, local) = listen(&sockets, loopback, &[]);
    let any = BindAddress::V6 {
        address: Ipv6Addr::UNSPECIFIED,
        families: Ipv6Families::V6Only,
    };
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(any),
            local,
            &[],
            TcpFastOpen::Off,
        )
        .expect("connect");
    let server = accept(&sockets, &listener);
    send_all(&sockets, &connected.connection, plain(), MESSAGE);
    assert_eq!(receive_up_to(&sockets, &server, MESSAGE.len()), MESSAGE);
    assert_eq!(connected.local.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
    sockets.close(server, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

pub fn dual_stack_listener_accepts_ipv4<S: TcpSockets>(sockets: S) {
    let any = BindAddress::V6 {
        address: Ipv6Addr::UNSPECIFIED,
        families: Ipv6Families::DualStack,
    };
    let (listener, local) = listen(&sockets, any, &[]);
    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), local.port());
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            peer,
            &[],
            TcpFastOpen::Off,
        )
        .expect("connect");
    let accepted = accept_with_header(&sockets, &listener, 0);
    assert_eq!(
        accepted.peer.ip(),
        IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped())
    );
    assert_eq!(accepted.peer.port(), connected.local.port());
    sockets.close(accepted.connection, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

pub fn multipath_connections_carry_bytes<S: TcpSockets>(sockets: S) {
    let listening = sockets
        .listen(
            TcpProtocol::Multipath,
            OWNER,
            bind_any_port(ipv4_loopback()),
            BACKLOG,
            &[],
        )
        .expect("listen for multipath connections");
    let connected = sockets
        .connect(
            TcpProtocol::Multipath,
            OWNER,
            bind_any_port(ipv4_any()),
            listening.local,
            &[],
            TcpFastOpen::Off,
        )
        .expect("connect with multipath");
    let server = accept(&sockets, &listening.listener);
    send_all(&sockets, &connected.connection, plain(), MESSAGE);
    assert_eq!(receive_up_to(&sockets, &server, MESSAGE.len()), MESSAGE);
    sockets.close(server, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listening.listener);
}

pub fn requested_port_is_bound<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    sockets.stop_listening(listener);
    let binding = LocalBinding {
        address: ipv4_loopback(),
        port: LocalPort::Requested(Port(local.port())),
    };
    let listening = sockets
        .listen(TcpProtocol::Tcp, OWNER, binding, BACKLOG, &[])
        .expect("listen on the port");
    assert_eq!(listening.local, local);
    sockets.stop_listening(listening.listener);
}

/// The exit node's port comes from its range outside the reserved part, also where the range lies above the host's own.
pub fn exit_node_chooses_a_port_outside_the_reserved<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    for (first, reserved_last, last) in [
        (CHOICE_FIRST, RESERVED_LAST, CHOICE_LAST),
        (HIGH_CHOICE_FIRST, HIGH_RESERVED_LAST, HIGH_CHOICE_LAST),
    ] {
        let reserved = [PortRange {
            first,
            last: reserved_last,
        }];
        let choice = PortChoice {
            range: PortRange { first, last },
            reserved: &reserved,
        };
        let allowed = reserved_last.0 + 1..=last.0;
        let connected = sockets
            .connect(
                TcpProtocol::Tcp,
                OWNER,
                LocalBinding {
                    address: ipv4_any(),
                    port: LocalPort::ChosenByExitNode(choice),
                },
                local,
                &[],
                TcpFastOpen::Off,
            )
            .expect("connect");
        let port = connected.local.port();
        assert!(allowed.contains(&port), "port {port}");
        let server = accept(&sockets, &listener);
        sockets.close(server, TcpClose::Orderly);
        sockets.close(connected.connection, TcpClose::Orderly);
        let listening = sockets
            .listen(
                TcpProtocol::Tcp,
                OWNER,
                LocalBinding {
                    address: ipv4_loopback(),
                    port: LocalPort::ChosenByExitNode(choice),
                },
                BACKLOG,
                &[],
            )
            .expect("listen");
        let port = listening.local.port();
        assert!(allowed.contains(&port), "port {port}");
        sockets.stop_listening(listening.listener);
    }
    sockets.stop_listening(listener);
}

pub fn stopped_listener_refuses_connections<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    sockets.stop_listening(listener);
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[],
            TcpFastOpen::Off,
        )
        .expect("start connecting");
    let mut buffer = [0; 1];
    let outcome =
        retry(|| sockets.receive(&connected.connection, TcpReceiveRequest::Read, &mut buffer));
    assert_eq!(
        outcome.map(|got| got.length),
        Err(errno(Errno::ECONNREFUSED))
    );
    sockets.close(connected.connection, TcpClose::Orderly);
}

pub fn accept_without_a_connection_would_block<S: TcpSockets>(sockets: S) {
    let (listener, _) = listen(&sockets, ipv4_loopback(), &[]);
    let mut header = [0; 0];
    let outcome = sockets
        .accept(&listener, &mut header)
        .map(|accepted| accepted.peer);
    assert_eq!(outcome, Err(HostError::WouldBlock));
    sockets.stop_listening(listener);
}

pub fn write_shutdown_ends_the_peers_stream<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    sockets
        .shut_down(&pair.client, TcpShutdown::Write)
        .expect("shut down");
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len() + 1),
        MESSAGE,
        "then the end"
    );
    send_all(&sockets, &pair.server, plain(), REPLY);
    assert_eq!(receive_up_to(&sockets, &pair.client, REPLY.len()), REPLY);
    assert_eq!(
        sockets.send(&pair.client, plain(), MESSAGE),
        Err(errno(Errno::EPIPE))
    );
    close(&sockets, pair);
}

pub fn read_shutdown_ends_the_own_stream<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .shut_down(&pair.server, TcpShutdown::Read)
        .expect("shut down");
    let mut buffer = [0; 1];
    let got = sockets
        .receive(&pair.server, TcpReceiveRequest::Read, &mut buffer)
        .expect("receive");
    assert_eq!(got.length, ByteCount(0));
    send_all(&sockets, &pair.server, plain(), REPLY);
    assert_eq!(receive_up_to(&sockets, &pair.client, REPLY.len()), REPLY);
    close(&sockets, pair);
}

pub fn shutdown_of_both_directions_ends_both<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .shut_down(&pair.client, TcpShutdown::ReadAndWrite)
        .expect("shut down");
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        Vec::<u8>::new()
    );
    assert_eq!(
        sockets.send(&pair.client, plain(), MESSAGE),
        Err(errno(Errno::EPIPE))
    );
    close(&sockets, pair);
}

pub fn orderly_close_ends_the_peers_stream<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    sockets.close(pair.client, TcpClose::Orderly);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len() + 1),
        MESSAGE
    );
    sockets.close(pair.server, TcpClose::Orderly);
    sockets.stop_listening(pair.listener);
}

pub fn abort_resets_the_peer<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets.close(pair.client, TcpClose::Abort);
    let mut buffer = [0; 1];
    let outcome = retry(|| sockets.receive(&pair.server, TcpReceiveRequest::Read, &mut buffer));
    assert_eq!(outcome.map(|got| got.length), Err(errno(Errno::ECONNRESET)));
    sockets.close(pair.server, TcpClose::Orderly);
    sockets.stop_listening(pair.listener);
}

pub fn peek_leaves_the_bytes<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    wait_for_unread(&sockets, &pair.server, MESSAGE.len());
    let mut buffer = vec![0; MESSAGE.len()];
    let got = sockets
        .receive(
            &pair.server,
            TcpReceiveRequest::Peek {
                offset: ByteCount(0),
            },
            &mut buffer,
        )
        .expect("peek");
    assert_eq!(&buffer[..got.length.0], MESSAGE);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    close(&sockets, pair);
}

pub fn peek_at_an_offset_skips_bytes<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    wait_for_unread(&sockets, &pair.server, MESSAGE.len());
    let mut buffer = vec![0; MESSAGE.len()];
    let got = sockets
        .receive(
            &pair.server,
            TcpReceiveRequest::Peek {
                offset: ByteCount(PEEK_OFFSET),
            },
            &mut buffer,
        )
        .expect("peek");
    assert_eq!(&buffer[..got.length.0], &MESSAGE[PEEK_OFFSET..]);
    let got = sockets
        .receive(
            &pair.server,
            TcpReceiveRequest::Peek {
                offset: ByteCount(0),
            },
            &mut buffer,
        )
        .expect("peek");
    assert_eq!(&buffer[..got.length.0], MESSAGE);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    close(&sockets, pair);
}

fn urgent() -> TcpSendRequest {
    TcpSendRequest {
        urgency: TcpUrgency::Urgent,
        ..plain()
    }
}

pub fn urgent_byte_is_read_apart<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    send_all(&sockets, &pair.client, urgent(), URGENT);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    let mut byte = vec![0; URGENT.len()];
    let peeked = retry(|| {
        match sockets.receive(&pair.server, TcpReceiveRequest::PeekUrgentByte, &mut byte) {
            // Linux answers EINVAL until the urgent byte has arrived.
            Err(HostError::Errno(code)) if code == Errno::EINVAL as i32 => {
                Err(HostError::WouldBlock)
            }
            outcome => outcome,
        }
    })
    .expect("peek the urgent byte");
    assert_eq!((peeked.length.0, &byte[..]), (URGENT.len(), URGENT));
    let read = sockets
        .receive(&pair.server, TcpReceiveRequest::ReadUrgentByte, &mut byte)
        .expect("read the urgent byte");
    assert_eq!((read.length.0, &byte[..]), (URGENT.len(), URGENT));
    let again = sockets
        .receive(&pair.server, TcpReceiveRequest::ReadUrgentByte, &mut byte)
        .map(|got| got.length);
    assert_eq!(again, Err(errno(Errno::EINVAL)));
    close(&sockets, pair);
}

pub fn urgent_inline_keeps_the_byte_in_the_stream<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .configure(&pair.server, TcpSetting::UrgentInline(true))
        .expect("urgent inline");
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    send_all(&sockets, &pair.client, urgent(), URGENT);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len() + URGENT.len()),
        [MESSAGE, URGENT].concat()
    );
    close(&sockets, pair);
}

pub fn cork_holds_data_until_uncorked<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .configure(&pair.client, TcpSetting::Cork(true))
        .expect("cork");
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    assert_silent(&sockets, &pair.server, HELD_FOR);
    sockets
        .configure(&pair.client, TcpSetting::Cork(false))
        .expect("uncork");
    assert_arrives_within(&sockets, &pair.server, MESSAGE, PUSHED_WITHIN);
    close(&sockets, pair);
}

pub fn more_to_come_holds_data_until_pushed<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let more = TcpSendRequest {
        continuation: TcpContinuation::MoreToCome,
        ..plain()
    };
    send_all(&sockets, &pair.client, more, MESSAGE);
    assert_silent(&sockets, &pair.server, HELD_FOR);
    sockets
        .push_queued_data(&pair.client, TcpPushOverride::CorkAndNagle)
        .expect("push");
    assert_arrives_within(&sockets, &pair.server, MESSAGE, PUSHED_WITHIN);
    close(&sockets, pair);
}

pub fn push_ignoring_neither_keeps_the_cork<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .configure(&pair.client, TcpSetting::Cork(true))
        .expect("cork");
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    sockets
        .push_queued_data(&pair.client, TcpPushOverride::Neither)
        .expect("push");
    assert_silent(&sockets, &pair.server, HELD_FOR);
    sockets
        .push_queued_data(&pair.client, TcpPushOverride::CorkAndNagle)
        .expect("push past the cork");
    assert_arrives_within(&sockets, &pair.server, MESSAGE, PUSHED_WITHIN);
    // The cork stays: later data is held again.
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    assert_silent(&sockets, &pair.server, HELD_FOR);
    close(&sockets, pair);
}

pub fn push_ignoring_neither_sends_data_held_for_more<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let more = TcpSendRequest {
        continuation: TcpContinuation::MoreToCome,
        ..plain()
    };
    send_all(&sockets, &pair.client, more, MESSAGE);
    assert_silent(&sockets, &pair.server, HELD_FOR);
    sockets
        .push_queued_data(&pair.client, TcpPushOverride::Neither)
        .expect("push");
    assert_arrives_within(&sockets, &pair.server, MESSAGE, PUSHED_WITHIN);
    close(&sockets, pair);
}

/// Two sends under a cork, the first ending a record: they leave as two segments, not one.
pub fn record_end_starts_a_new_segment<S: TcpSockets>(sockets: S) {
    let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), &[], &[]);
    sockets
        .configure(&pair.client, TcpSetting::Cork(true))
        .expect("cork");
    let start = Instant::now();
    let ended = TcpSendRequest {
        record: TcpRecordBoundary::Ended,
        ..plain()
    };
    send_all(&sockets, &pair.client, ended, MESSAGE);
    send_all(&sockets, &pair.client, plain(), REPLY);
    sockets
        .configure(&pair.client, TcpSetting::Cork(false))
        .expect("uncork");
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len() + REPLY.len()),
        [MESSAGE, REPLY].concat()
    );
    let lengths: Vec<usize> = capture
        .since(start)
        .iter()
        .filter(|segment| segment.is_data_to_listener())
        .map(|segment| segment.payload)
        .collect();
    assert_eq!(lengths, [MESSAGE.len(), REPLY.len()]);
    close(&sockets, pair);
}

pub fn unread_byte_reports_count_what_is_left<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let mut buffer = [0; READ_PART];
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    wait_for_unread(&sockets, &pair.server, MESSAGE.len());
    let got = sockets
        .receive(&pair.server, TcpReceiveRequest::Read, &mut buffer)
        .expect("receive");
    assert_eq!(got.unread_after, None, "not asked for");
    sockets
        .configure(
            &pair.server,
            TcpSetting::UnreadByteReports(Report::Requested),
        )
        .expect("unread reports");
    let got = sockets
        .receive(&pair.server, TcpReceiveRequest::Read, &mut buffer)
        .expect("receive");
    assert_eq!(
        (got.length, got.unread_after),
        (
            ByteCount(READ_PART),
            Some(ByteCount(MESSAGE.len() - 2 * READ_PART))
        )
    );
    close(&sockets, pair);
}

pub fn receive_software_timestamps_when_asked<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let mut buffer = [0; 1];
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    let got = retry(|| sockets.receive(&pair.server, TcpReceiveRequest::Read, &mut buffer))
        .expect("receive");
    assert_eq!(got.timestamps.software, None, "not asked for");
    let timestamping = Timestamping {
        receive_software: Report::Requested,
        receive_hardware: Report::NotRequested,
        bound_clock: None,
    };
    sockets
        .configure(
            &pair.server,
            TcpSetting::Ip(IpSocketSetting::Timestamping(timestamping)),
        )
        .expect("timestamping");
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    let got = retry(|| sockets.receive(&pair.server, TcpReceiveRequest::Read, &mut buffer))
        .expect("receive");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is past the epoch");
    let stamp = got.timestamps.software.expect("a software timestamp");
    assert!(
        stamp <= now && now - stamp < DEADLINE,
        "stamp {stamp:?} now {now:?}"
    );
    assert_eq!(got.timestamps.hardware, None);
    close(&sockets, pair);
}

fn take_entry<S: TcpSockets>(sockets: &S, connection: &S::Connection) -> TcpErrorQueueEntry {
    let mut buffer = [0; ENTRY_ROOM];
    retry(|| sockets.take_error_queue_entry(connection, &mut buffer)).expect("an error queue entry")
}

pub fn software_transmit_report_names_the_send<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let key = KEY;
    let reported = TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key,
            software: Report::Requested,
            ..no_reports()
        },
        ..plain()
    };
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is past the epoch");
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    send_all(&sockets, &pair.client, reported, MESSAGE);
    let TcpErrorQueueEntry::TransmitReport { report, .. } = take_entry(&sockets, &pair.client)
    else {
        panic!("not a transmit report");
    };
    assert_eq!(report.key, key);
    let TransmitEvent::Timestamp {
        stage,
        source,
        time,
    } = report.event
    else {
        panic!("not a timestamp: {:?}", report.event);
    };
    assert_eq!(
        (stage, source),
        (TransmitStage::Sent, TimestampSource::Software)
    );
    assert!(time >= before && time - before < DEADLINE);
    let mut buffer = [0; ENTRY_ROOM];
    assert_eq!(
        sockets.take_error_queue_entry(&pair.client, &mut buffer),
        Err(HostError::WouldBlock)
    );
    close(&sockets, pair);
}

pub fn acknowledgement_report_follows_the_peers_acknowledgement<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let key = KEY;
    let reported = TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key,
            acknowledgement: Report::Requested,
            ..no_reports()
        },
        ..plain()
    };
    send_all(&sockets, &pair.client, reported, MESSAGE);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    let TcpErrorQueueEntry::TransmitReport { report, .. } = take_entry(&sockets, &pair.client)
    else {
        panic!("not a transmit report");
    };
    assert_eq!(report.key, key);
    assert!(matches!(
        report.event,
        TransmitEvent::Timestamp {
            stage: TransmitStage::Acknowledged,
            ..
        }
    ));
    close(&sockets, pair);
}

pub fn discarding_empties_the_error_queue<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let reported = TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key: KEY,
            software: Report::Requested,
            ..no_reports()
        },
        ..plain()
    };
    send_all(&sockets, &pair.client, reported, MESSAGE);
    send_all(&sockets, &pair.client, reported, MESSAGE);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, 2 * MESSAGE.len()),
        [MESSAGE, MESSAGE].concat()
    );
    sockets
        .discard_error_queue_entries(&pair.client)
        .expect("discard");
    let mut buffer = [0; ENTRY_ROOM];
    assert_eq!(
        sockets.take_error_queue_entry(&pair.client, &mut buffer),
        Err(HostError::WouldBlock)
    );
    sockets
        .discard_error_queue_entries(&pair.client)
        .expect("discard an empty queue");
    close(&sockets, pair);
}

pub fn empty_error_queue_would_block<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let mut buffer = [0; ENTRY_ROOM];
    assert_eq!(
        sockets.take_error_queue_entry(&pair.client, &mut buffer),
        Err(HostError::WouldBlock)
    );
    close(&sockets, pair);
}

fn take_entry_into<S: TcpSockets>(
    sockets: &S,
    connection: &S::Connection,
    buffer: &mut [u8],
) -> TcpErrorQueueEntry {
    retry(|| sockets.take_error_queue_entry(connection, buffer)).expect("an error queue entry")
}

/// A raw socket of the host, which the ICMP checks use as the network between the connection and its peer.
fn raw_socket(version: IpVersion, protocol: rustix::net::Protocol) -> OwnedFd {
    let family = match version {
        IpVersion::V4 => AddressFamily::INET,
        IpVersion::V6 => AddressFamily::INET6,
    };
    let socket = rustix::net::socket(family, SocketType::RAW, Some(protocol))
        .expect("a raw socket (the ICMP checks need root)");
    sockopt::set_socket_timeout(&socket, sockopt::Timeout::Recv, Some(SETTLE))
        .expect("a receive timeout");
    socket
}

/// Waits on a raw TCP socket for the SYN from `local` to `peer`, giving what Linux delivered of it.
fn capture_synchronize(
    capture: &OwnedFd,
    version: IpVersion,
    local: SocketAddr,
    peer: SocketAddr,
) -> Vec<u8> {
    let start = Instant::now();
    let mut buffer = [0; PACKET_ROOM];
    while start.elapsed() < DEADLINE {
        let Ok((length, _)) = rustix::net::recv(capture, &mut buffer, RecvFlags::empty()) else {
            continue;
        };
        let packet = &buffer[..length];
        let segment = match version {
            IpVersion::V4 => Ipv4HeaderSlice::from_slice(packet)
                .ok()
                .map(|header| &packet[header.slice().len()..]),
            IpVersion::V6 => Some(packet),
        };
        if let Some(tcp) = segment.and_then(|segment| TcpHeaderSlice::from_slice(segment).ok())
            && tcp.syn()
            && !tcp.ack()
            && tcp.source_port() == local.port()
            && tcp.destination_port() == peer.port()
        {
            return packet.to_vec();
        }
    }
    panic!("no SYN from {local} to {peer} arrived");
}

/// Connects to a listener whose accept queue is full, so that the connection stays in SYN-SENT, and has the
/// network answer its SYN with a destination unreachable error; checks the error queue entry written whole.
fn icmp_error_reaches_a_synchronizing_connection<S: TcpSockets>(sockets: S, version: IpVersion) {
    let (address, listener_address, loopback) = match version {
        IpVersion::V4 => (
            ipv4_loopback(),
            BindAddress::V4(OTHER_LOOPBACK),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
        ),
        IpVersion::V6 => {
            let address = BindAddress::V6 {
                address: Ipv6Addr::LOCALHOST,
                families: Ipv6Families::V6Only,
            };
            (address, address, IpAddr::V6(Ipv6Addr::LOCALHOST))
        }
    };
    let listening = sockets
        .listen(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(listener_address),
            ListenBacklog(0),
            &[],
        )
        .expect("listen on loopback");
    let peer = listening.local;
    let connect = |settings: &[TcpSetting<'_>]| {
        sockets
            .connect(
                TcpProtocol::Tcp,
                OWNER,
                bind_any_port(address),
                peer,
                settings,
                TcpFastOpen::Off,
            )
            .expect("connect to the listener")
    };
    let queued = connect(&[]).connection;
    retry(|| sockets.send(&queued, plain(), URGENT)).expect("the first connection is established");
    std::thread::sleep(SETTLE);
    let capture = raw_socket(version, ipproto::TCP);
    let waiting = connect(&[TcpSetting::IcmpErrorReports {
        version,
        reports: Report::Requested,
    }]);
    let local = waiting.local;
    assert!(matches!(
        waiting.start,
        TcpConnectStart::SynchronizeSent { .. }
    ));
    let synchronize = capture_synchronize(&capture, version, local, peer);
    let (quoted, icmp) = match version {
        IpVersion::V4 => {
            let header = Ipv4HeaderSlice::from_slice(&synchronize).expect("an IPv4 header");
            let tcp = TcpHeaderSlice::from_slice(&synchronize[header.slice().len()..])
                .expect("a TCP header");
            let quoted = [header.slice(), tcp.slice()].concat();
            let icmp = Icmpv4Header::with_checksum(
                Icmpv4Type::DestinationUnreachable(icmpv4::DestUnreachableHeader::Host),
                &quoted,
            );
            (quoted, icmp.to_bytes().to_vec())
        }
        IpVersion::V6 => {
            let tcp = TcpHeaderSlice::from_slice(&synchronize).expect("a TCP header");
            let header = Ipv6Header {
                traffic_class: 0,
                flow_label: Ipv6FlowLabel::ZERO,
                payload_length: u16::try_from(synchronize.len()).expect("a short SYN"),
                next_header: IpNumber::TCP,
                hop_limit: QUOTED_HOP_LIMIT,
                source: Ipv6Addr::LOCALHOST.octets(),
                destination: Ipv6Addr::LOCALHOST.octets(),
            };
            let quoted = [&header.to_bytes()[..], tcp.slice()].concat();
            let icmp = Icmpv6Header::with_checksum(
                Icmpv6Type::DestinationUnreachable(icmpv6::DestUnreachableCode::Address),
                Ipv6Addr::LOCALHOST.octets(),
                Ipv6Addr::LOCALHOST.octets(),
                &quoted,
            )
            .expect("a short ICMPv6 error");
            (quoted, icmp.to_bytes().to_vec())
        }
    };
    let sender = raw_socket(
        version,
        match version {
            IpVersion::V4 => ipproto::ICMP,
            IpVersion::V6 => ipproto::ICMPV6,
        },
    );
    rustix::net::sendto(
        &sender,
        &[icmp, quoted].concat(),
        SendFlags::empty(),
        &SocketAddr::new(local.ip(), 0),
    )
    .expect("send the ICMP error");
    let mut buffer = [0; PACKET_ROOM];
    let TcpErrorQueueEntry::IcmpError(fields) =
        take_entry_into(&sockets, &waiting.connection, &mut buffer)
    else {
        panic!("not an ICMP error");
    };
    let packet = &buffer[..fields.length];
    let (icmp, inner) = match version {
        IpVersion::V4 => {
            let outer = Ipv4HeaderSlice::from_slice(packet).expect("an IPv4 header");
            assert_eq!(usize::from(outer.total_len()), fields.length);
            assert_eq!(outer.protocol(), IpNumber::ICMP);
            assert_eq!(IpAddr::V4(outer.source_addr()), loopback);
            assert_eq!(IpAddr::V4(outer.destination_addr()), local.ip());
            let icmp = Icmpv4Slice::from_slice(&packet[outer.slice().len()..])
                .expect("the ICMP error's header");
            assert_eq!(
                (icmp.type_u8(), icmp.code_u8()),
                (icmpv4::TYPE_DEST_UNREACH, icmpv4::CODE_DST_UNREACH_HOST)
            );
            assert_eq!(
                icmp.icmp_type().calc_checksum(icmp.payload()),
                icmp.checksum()
            );
            let inner = Ipv4HeaderSlice::from_slice(icmp.payload()).expect("the quoted header");
            assert_eq!(inner.protocol(), IpNumber::TCP);
            assert_eq!(IpAddr::V4(inner.source_addr()), local.ip());
            assert_eq!(IpAddr::V4(inner.destination_addr()), peer.ip());
            (icmp.payload(), inner.slice().len())
        }
        IpVersion::V6 => {
            let outer = Ipv6HeaderSlice::from_slice(packet).expect("an IPv6 header");
            assert_eq!(
                usize::from(outer.payload_length()) + Ipv6Header::LEN,
                fields.length
            );
            assert_eq!(outer.next_header(), IpNumber::IPV6_ICMP);
            assert_eq!(IpAddr::V6(outer.source_addr()), loopback);
            assert_eq!(IpAddr::V6(outer.destination_addr()), local.ip());
            let icmp = Icmpv6Slice::from_slice(&packet[Ipv6Header::LEN..])
                .expect("the ICMPv6 error's header");
            assert_eq!(
                (icmp.type_u8(), icmp.code_u8()),
                (icmpv6::TYPE_DST_UNREACH, icmpv6::CODE_DST_UNREACH_ADDR)
            );
            assert_eq!(
                icmp.icmp_type()
                    .calc_checksum(
                        outer.source_addr().octets(),
                        outer.destination_addr().octets(),
                        icmp.payload()
                    )
                    .expect("a checksum"),
                icmp.checksum()
            );
            let inner = Ipv6HeaderSlice::from_slice(icmp.payload()).expect("the quoted header");
            assert_eq!(inner.next_header(), IpNumber::TCP);
            assert_eq!(IpAddr::V6(inner.source_addr()), local.ip());
            assert_eq!(IpAddr::V6(inner.destination_addr()), peer.ip());
            (icmp.payload(), Ipv6Header::LEN)
        }
    };
    let tcp = TcpHeaderSlice::from_slice(&icmp[inner..]).expect("the quoted TCP header");
    let sent = TcpHeaderSlice::from_slice(match version {
        IpVersion::V4 => {
            &synchronize[Ipv4HeaderSlice::from_slice(&synchronize)
                .expect("an IPv4 header")
                .slice()
                .len()..]
        }
        IpVersion::V6 => &synchronize,
    })
    .expect("a TCP header");
    assert_eq!(
        (
            tcp.source_port(),
            tcp.destination_port(),
            tcp.sequence_number()
        ),
        (local.port(), peer.port(), sent.sequence_number())
    );
    sockets.close(waiting.connection, TcpClose::Abort);
    sockets.close(queued, TcpClose::Abort);
    sockets.stop_listening(listening.listener);
}

pub fn icmp_error_reaches_a_synchronizing_connection_over_ipv4<S: TcpSockets>(sockets: S) {
    icmp_error_reaches_a_synchronizing_connection(sockets, IpVersion::V4);
}

pub fn icmp_error_reaches_a_synchronizing_connection_over_ipv6<S: TcpSockets>(sockets: S) {
    icmp_error_reaches_a_synchronizing_connection(sockets, IpVersion::V6);
}

pub fn transmit_report_carries_the_packet_when_asked<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let reported = TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key: KEY,
            software: Report::Requested,
            packet: Report::Requested,
            ..no_reports()
        },
        ..plain()
    };
    send_all(&sockets, &pair.client, reported, MESSAGE);
    let mut buffer = [0; PACKET_ROOM];
    let TcpErrorQueueEntry::TransmitReport { report, statistics } =
        take_entry_into(&sockets, &pair.client, &mut buffer)
    else {
        panic!("not a transmit report");
    };
    assert_eq!(report.key, KEY);
    assert_eq!(statistics, None);
    let length = report.packet_length.expect("the packet").0;
    assert!(length >= MESSAGE.len(), "a packet of {length} bytes");
    assert!(buffer[..length].ends_with(MESSAGE));
    close(&sockets, pair);
}

pub fn transmit_statistics_come_with_the_report_when_asked<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let reported = |statistics| TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key: KEY,
            software: Report::Requested,
            ..no_reports()
        },
        transmit_statistics: statistics,
        ..plain()
    };
    send_all(
        &sockets,
        &pair.client,
        reported(Report::NotRequested),
        MESSAGE,
    );
    let TcpErrorQueueEntry::TransmitReport { statistics, .. } = take_entry(&sockets, &pair.client)
    else {
        panic!("not a transmit report");
    };
    assert_eq!(statistics, None);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    send_all(&sockets, &pair.client, reported(Report::Requested), MESSAGE);
    let TcpErrorQueueEntry::TransmitReport { report, statistics } =
        take_entry(&sockets, &pair.client)
    else {
        panic!("not a transmit report");
    };
    assert_eq!(report.key, KEY);
    let statistics = statistics.expect("the statistics asked for");
    assert!(statistics.bytes_sent.0 >= MESSAGE.len() as u64);
    assert!(statistics.data_segments_out.0 >= 1);
    assert!(statistics.send_congestion_window.0 >= 1);
    close(&sockets, pair);
}

pub fn wireless_status_request_still_sends<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let reported = TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key: KEY,
            wireless_status: Report::Requested,
            ..no_reports()
        },
        ..plain()
    };
    send_all(&sockets, &pair.client, reported, MESSAGE);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    // Loopback is no wireless link, so no status comes.
    let mut buffer = [0; ENTRY_ROOM];
    assert_eq!(
        sockets.take_error_queue_entry(&pair.client, &mut buffer),
        Err(HostError::WouldBlock)
    );
    close(&sockets, pair);
}

/// What one connect with fast open showed on the wire.
struct FastOpenRound {
    start: TcpConnectStart,
    synchronize_data: usize,
    data_acknowledged: bool,
}

/// Connects with fast open, sends the message, and checks that the listener's connection receives it whole.
fn fast_open_round<S: TcpSockets>(
    sockets: &S,
    listener: &S::Listener,
    local: SocketAddr,
    capture: &Capture,
    settings: &[TcpSetting<'_>],
    fast_open: TcpFastOpen<'_>,
) -> FastOpenRound {
    let start = Instant::now();
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            settings,
            fast_open,
        )
        .expect("connect with fast open");
    let taken = match connected.start {
        TcpConnectStart::SynchronizeSent {
            fast_open_data_taken,
        } => fast_open_data_taken.0,
        _ => 0,
    };
    if connected.start == TcpConnectStart::DeferredUntilFirstSend {
        let early = capture.wait_for(start, SETTLE, |segment| {
            segment.to_listener && segment.synchronize
        });
        assert!(early.is_none(), "a deferred connect sent its SYN early");
    }
    send_all(sockets, &connected.connection, plain(), &MESSAGE[taken..]);
    let server = accept(sockets, listener);
    assert_eq!(receive_up_to(sockets, &server, MESSAGE.len()), MESSAGE);
    let synchronize = capture
        .wait_for(start, DEADLINE, |segment| {
            segment.to_listener && segment.synchronize
        })
        .expect("the SYN");
    let answer = capture
        .wait_for(start, DEADLINE, |segment| {
            !segment.to_listener && segment.synchronize
        })
        .expect("the SYN-ACK");
    sockets.close(server, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    FastOpenRound {
        start: connected.start,
        synchronize_data: synchronize.payload,
        data_acknowledged: synchronize.payload > 0 && answer.acknowledgement == synchronize.end(),
    }
}

/// A fast open listener on its own loopback address, since a client keeps the cookies it is given per address.
fn fast_open_listener<S: TcpSockets>(
    sockets: &S,
    address: Ipv4Addr,
    settings: &[TcpListenerSetting<'_>],
) -> (S::Listener, SocketAddr, Capture) {
    let mut all = vec![TcpListenerSetting::FastOpenQueue(FAST_OPEN_QUEUE)];
    all.extend_from_slice(settings);
    let (listener, local) = listen(sockets, BindAddress::V4(address), &all);
    let capture = Capture::start(local);
    (listener, local, capture)
}

/// The first connect fetches a cookie; the second puts all its data in the SYN, which the listener takes.
pub fn fast_open_with_data_puts_it_in_the_synchronize<S: TcpSockets>(sockets: S) {
    let (listener, local, capture) = fast_open_listener(&sockets, FAST_OPEN_WITH_DATA, &[]);
    let fast_open = TcpFastOpen::WithData(MESSAGE);
    fast_open_round(&sockets, &listener, local, &capture, &[], fast_open);
    let round = fast_open_round(&sockets, &listener, local, &capture, &[], fast_open);
    assert_eq!(
        round.start,
        TcpConnectStart::SynchronizeSent {
            fast_open_data_taken: ByteCount(MESSAGE.len())
        }
    );
    assert_eq!(round.synchronize_data, MESSAGE.len());
    assert!(round.data_acknowledged);
    sockets.stop_listening(listener);
}

/// Once a cookie is held, a connect until the first send sends nothing until that send, whose data goes in the SYN.
pub fn fast_open_until_first_send_defers_the_synchronize<S: TcpSockets>(sockets: S) {
    let (listener, local, capture) = fast_open_listener(&sockets, FAST_OPEN_UNTIL_FIRST_SEND, &[]);
    let fast_open = TcpFastOpen::UntilFirstSend;
    fast_open_round(&sockets, &listener, local, &capture, &[], fast_open);
    let round = fast_open_round(&sockets, &listener, local, &capture, &[], fast_open);
    assert_eq!(round.start, TcpConnectStart::DeferredUntilFirstSend);
    assert_eq!(round.synchronize_data, MESSAGE.len());
    assert!(round.data_acknowledged);
    sockets.stop_listening(listener);
}

/// Both ends doing without cookies, the very first connect's data goes in the SYN.
pub fn fast_open_without_cookie_needs_no_first_connect<S: TcpSockets>(sockets: S) {
    let without = TcpSetting::FastOpenWithoutCookie(true);
    let (listener, local, capture) = fast_open_listener(
        &sockets,
        FAST_OPEN_WITHOUT_COOKIE,
        &[TcpListenerSetting::Socket(without.clone())],
    );
    let round = fast_open_round(
        &sockets,
        &listener,
        local,
        &capture,
        &[without],
        TcpFastOpen::WithData(MESSAGE),
    );
    assert_eq!(round.synchronize_data, MESSAGE.len());
    assert!(round.data_acknowledged);
    sockets.stop_listening(listener);
}

/// A listener takes SYN data only with a cookie of its primary or backup key, and only while its queue is on.
pub fn fast_open_keys_and_queue_decide_what_the_listener_takes<S: TcpSockets>(sockets: S) {
    let keys = |primary: u8, backup: Option<u8>| {
        TcpListenerSetting::FastOpenKeys(FastOpenKeys {
            primary: FastOpenKey([primary; FAST_OPEN_KEY_LENGTH]),
            backup: backup.map(|key| FastOpenKey([key; FAST_OPEN_KEY_LENGTH])),
        })
    };
    let (listener, local, capture) = fast_open_listener(&sockets, FAST_OPEN_KEYS, &[keys(1, None)]);
    let fast_open = TcpFastOpen::WithData(MESSAGE);
    let round = || fast_open_round(&sockets, &listener, local, &capture, &[], fast_open);
    round();
    assert!(round().data_acknowledged, "a cookie of the key");
    sockets
        .configure(&listener, keys(2, None))
        .expect("a new key");
    let stale = round();
    assert_eq!(stale.synchronize_data, MESSAGE.len());
    assert!(!stale.data_acknowledged, "a cookie of a key no longer held");
    sockets
        .configure(&listener, keys(3, Some(2)))
        .expect("a new key, the last one kept as backup");
    assert!(round().data_acknowledged, "a cookie of the backup key");
    sockets
        .configure(&listener, TcpListenerSetting::FastOpenQueue(None))
        .expect("fast open off");
    let off = round();
    assert_eq!(off.synchronize_data, MESSAGE.len());
    assert!(!off.data_acknowledged, "fast open off");
    sockets.stop_listening(listener);
}

/// A keep-alive probe: no data, one sequence number below what the connection sends next.
fn is_probe(segment: &Segment, next: u32) -> bool {
    segment.to_listener
        && segment.payload == 0
        && !segment.synchronize
        && !segment.reset
        && !segment.finish
        && segment.sequence == next.wrapping_sub(1)
}

fn assert_near(what: &str, measured: Duration, expected: Duration) {
    assert!(
        measured + TIMER_SLACK > expected && measured < expected + TIMER_SLACK,
        "{what}: {measured:?}, not {expected:?}"
    );
}

/// The peer answering nothing, the connection probes after the idle time and resets an interval after its last probe.
pub fn keep_alive_probes_end_an_unanswered_connection<S: TcpSockets>(sockets: S) {
    let (pair, capture) = watched_pair(
        &sockets,
        ipv4_loopback(),
        &[
            TcpSetting::KeepAlive(true),
            TcpSetting::KeepAliveIdle(KEEP_ALIVE_IDLE),
            TcpSetting::KeepAliveInterval(KEEP_ALIVE_INTERVAL),
            TcpSetting::KeepAliveProbes(ProbeCount(1)),
        ],
        &[],
    );
    let start = Instant::now();
    drop_arrivals(&sockets, &pair.server, IpVersion::V4);
    let next = capture.next_to_listener();
    let ending = KEEP_ALIVE_IDLE + KEEP_ALIVE_INTERVAL + TIMER_SLACK;
    let probe = capture
        .wait_for(start, ending, |segment| is_probe(segment, next))
        .expect("a keep-alive probe");
    assert_near("the probe", probe.at - start, KEEP_ALIVE_IDLE);
    let reset = capture
        .wait_for(start, ending, |segment| {
            segment.to_listener && segment.reset
        })
        .expect("a reset after the last probe");
    assert_near("the reset", reset.at - probe.at, KEEP_ALIVE_INTERVAL);
    let probes = capture
        .segments()
        .iter()
        .filter(|segment| is_probe(segment, next))
        .count();
    assert_eq!(probes, 1);
    assert_eq!(
        receive_outcome(&sockets, &pair.client, DEADLINE).map(|got| got.length),
        Err(errno(Errno::ETIMEDOUT))
    );
    close(&sockets, pair);
}

/// A probe answered by nothing is followed by the next only an interval later, unless the timer is rearmed: then the
/// next is due at once, the idle time since the last arrival having passed.
pub fn rearming_the_keep_alive_timer_sends_a_due_probe<S: TcpSockets>(sockets: S) {
    let (pair, capture) = watched_pair(
        &sockets,
        ipv4_loopback(),
        &[
            TcpSetting::KeepAlive(true),
            TcpSetting::KeepAliveIdle(KEEP_ALIVE_INTERVAL),
            TcpSetting::KeepAliveInterval(SETTING_TIME),
        ],
        &[],
    );
    let start = Instant::now();
    drop_arrivals(&sockets, &pair.server, IpVersion::V4);
    let next = capture.next_to_listener();
    let first = capture
        .wait_for(start, KEEP_ALIVE_INTERVAL + TIMER_SLACK, |segment| {
            is_probe(segment, next)
        })
        .expect("a keep-alive probe");
    std::thread::sleep(TIMER_SLACK);
    let rearmed = Instant::now();
    sockets.rearm_keep_alive_timer(&pair.client).expect("rearm");
    let second = capture.wait_for(rearmed, TIMER_SLACK, |segment| {
        segment.at > first.at && is_probe(segment, next)
    });
    assert!(second.is_some(), "no probe after the rearm");
    close(&sockets, pair);
}

/// The peer answering nothing, a second small send waits for the first to be acknowledged, unless without delay.
pub fn no_delay_sends_small_segments_while_others_are_unacknowledged<S: TcpSockets>(sockets: S) {
    for (settings, segments) in [
        (vec![], 1),
        (vec![TcpSetting::NoDelay(true)], 2),
        (
            vec![TcpSetting::NoDelay(true), TcpSetting::NoDelay(false)],
            1,
        ),
    ] {
        let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), &settings, &[]);
        drop_arrivals(&sockets, &pair.server, IpVersion::V4);
        let start = Instant::now();
        send_all(&sockets, &pair.client, plain(), MESSAGE);
        send_all(&sockets, &pair.client, plain(), REPLY);
        std::thread::sleep(HELD_FOR);
        let mut sequences: Vec<u32> = capture
            .since(start)
            .iter()
            .filter(|segment| segment.is_data_to_listener())
            .map(|segment| segment.sequence)
            .collect();
        sequences.dedup();
        assert_eq!(sequences.len(), segments, "{settings:?}");
        close(&sockets, pair);
    }
}

/// Delaying acknowledgements, the listener's connection acknowledges data late; quick again, at once; and with a
/// maximum delay, within it.
pub fn quick_acknowledgement_settings_time_the_acknowledgement<S: TcpSockets>(sockets: S) {
    let delaying = TcpSetting::QuickAcknowledgement(TcpQuickAcknowledgement::Off);
    for (settings, delayed) in [
        (vec![delaying.clone()], true),
        (
            vec![
                delaying.clone(),
                TcpSetting::QuickAcknowledgement(TcpQuickAcknowledgement::On),
            ],
            false,
        ),
        (
            vec![
                delaying.clone(),
                TcpSetting::DelayedAcknowledgementMaximum(DELAYED_ACKNOWLEDGEMENT_MAXIMUM),
            ],
            false,
        ),
    ] {
        let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), &[], &[]);
        for setting in &settings {
            sockets
                .configure(&pair.server, setting.clone())
                .expect("an acknowledgement setting");
        }
        let delay = acknowledgement_delay(&sockets, &pair, &capture);
        assert_eq!(delay > QUICK_WITHIN, delayed, "{settings:?}: {delay:?}");
        close(&sockets, pair);
    }
}

/// A delayed acknowledgement of data read is sent on demand, and the mode asked for holds for the next data.
pub fn delayed_acknowledgement_is_sent_on_demand<S: TcpSockets>(sockets: S) {
    for afterwards in [TcpQuickAcknowledgement::On, TcpQuickAcknowledgement::Off] {
        let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), &[], &[]);
        sockets
            .configure(
                &pair.server,
                TcpSetting::QuickAcknowledgement(TcpQuickAcknowledgement::Off),
            )
            .expect("delay acknowledgements");
        let start = Instant::now();
        send_all(&sockets, &pair.client, plain(), MESSAGE);
        assert_eq!(
            receive_up_to(&sockets, &pair.server, MESSAGE.len()),
            MESSAGE
        );
        sockets
            .send_delayed_acknowledgement(&pair.server, afterwards)
            .expect("send it");
        let data = capture
            .wait_for(start, DEADLINE, Segment::is_data_to_listener)
            .expect("the data");
        let acknowledgement = capture
            .wait_for(start, DEADLINE, |segment| {
                !segment.to_listener && segment.acknowledgement == data.end()
            })
            .expect("its acknowledgement");
        let delay = acknowledgement.at - data.at;
        assert!(delay < QUICK_WITHIN, "{afterwards:?}: {delay:?}");
        let next = acknowledgement_delay(&sockets, &pair, &capture);
        assert_eq!(
            next > QUICK_WITHIN,
            afterwards == TcpQuickAcknowledgement::Off,
            "{afterwards:?}: {next:?}"
        );
        close(&sockets, pair);
    }
}

/// Each setting bounds what a connection takes while its peer reads nothing, far below what it takes without.
pub fn send_settings_bound_what_a_connection_takes<S: TcpSockets>(sockets: S) {
    let unbounded = {
        let pair = pair(&sockets);
        let taken = fill(&sockets, &pair.client);
        close(&sockets, pair);
        taken
    };
    let low_water = TcpSetting::NotSentLowWater(Some(ByteCount(SETTING_BYTES / BOUND_RATIO)));
    for setting in [
        TcpSetting::Ip(IpSocketSetting::SendBuffer(ByteCount(SETTING_BYTES))),
        TcpSetting::BufferLocks(BufferLocks {
            send: BufferTuning::Locked,
            receive: BufferTuning::Automatic,
        }),
        low_water.clone(),
    ] {
        let pair = pair_with(&sockets, std::slice::from_ref(&setting), &[]);
        let taken = fill(&sockets, &pair.client);
        assert!(
            taken * BOUND_RATIO < unbounded,
            "{setting:?}: {taken} of {unbounded}"
        );
        close(&sockets, pair);
    }
    let pair = pair_with(&sockets, &[low_water], &[]);
    let bounded = fill(&sockets, &pair.client);
    sockets
        .configure(&pair.client, TcpSetting::NotSentLowWater(None))
        .expect("no low water");
    let more = fill(&sockets, &pair.client);
    assert!(
        (bounded + more) * BOUND_RATIO > unbounded,
        "{bounded} and {more} of {unbounded}"
    );
    close(&sockets, pair);
}

/// Each setting of the receiving connection bounds what its peer sends while it reads nothing, or widens it; a locked
/// receive buffer keeps a low water mark from widening it.
pub fn receive_settings_bound_what_the_peer_sends<S: TcpSockets>(sockets: S) {
    let unread_with = |settings: &[TcpSetting<'_>]| {
        let pair = pair_with(&sockets, settings, &[]);
        fill(&sockets, &pair.server);
        let waiting = unread(&sockets, &pair.client);
        close(&sockets, pair);
        waiting
    };
    let unbounded = unread_with(&[]);
    let wide = TcpSetting::ReceiveLowWater(ByteCount(WIDE_LOW_WATER));
    let locked = TcpSetting::BufferLocks(BufferLocks {
        send: BufferTuning::Automatic,
        receive: BufferTuning::Locked,
    });
    let narrower = |waiting: usize| waiting * 2 < unbounded;
    let wider = |waiting: usize| waiting > unbounded * 2;
    let small = ByteCount(SETTING_BYTES / BOUND_RATIO);
    let narrowed = unread_with(&[TcpSetting::Ip(IpSocketSetting::ReceiveBuffer(small))]);
    assert!(narrower(narrowed), "{narrowed} of {unbounded}");
    let widened = unread_with(std::slice::from_ref(&wide));
    assert!(wider(widened), "{widened} of {unbounded}");
    let kept = unread_with(&[locked, wide]);
    assert!(!wider(kept), "{kept} of {unbounded}");
}

/// A window clamp given to connect bounds the window its SYN announces.
pub fn window_clamp_bounds_the_synchronize_window<S: TcpSockets>(sockets: S) {
    let clamp = WindowSize((SETTING_BYTES / BOUND_RATIO) as u32);
    let announced = |settings: &[TcpSetting<'_>]| {
        let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), settings, &[]);
        let window = capture
            .first(|segment| segment.to_listener && segment.synchronize)
            .window;
        close(&sockets, pair);
        u32::from(window)
    };
    let (unclamped, clamped) = (
        announced(&[]),
        announced(&[TcpSetting::WindowClamp(Some(clamp))]),
    );
    assert!(
        clamped <= clamp.0 && clamped < unclamped,
        "{clamped} of {unclamped}"
    );
}

/// When a segment the peer never acknowledges left again.
fn transmissions(capture: &Capture, start: Instant) -> Vec<Duration> {
    capture
        .since(start)
        .iter()
        .filter(|segment| segment.is_data_to_listener())
        .map(|segment| segment.at - start)
        .collect()
}

/// The peer answering nothing, a short minimum makes the first retransmission early, a maximum caps how far the
/// timeout grows, and thin linear timeouts keep it from growing.
pub fn retransmission_settings_time_the_retransmissions<S: TcpSockets>(sockets: S) {
    let watched: Vec<_> = [
        TcpSetting::RetransmissionTimeoutMinimum(SHORT_SETTING_TIME),
        TcpSetting::RetransmissionTimeoutMaximum(RETRANSMISSION_CEILING),
        TcpSetting::ThinLinearTimeouts(true),
    ]
    .into_iter()
    .map(|setting| {
        let (pair, capture) = watched_pair(
            &sockets,
            ipv4_loopback(),
            std::slice::from_ref(&setting),
            &[],
        );
        drop_arrivals(&sockets, &pair.server, IpVersion::V4);
        (setting, pair, capture, Instant::now())
    })
    .collect();
    for (_, pair, _, _) in &watched {
        send_all(&sockets, &pair.client, plain(), MESSAGE);
    }
    std::thread::sleep(RETRANSMISSIONS_WATCHED);
    for (setting, pair, capture, start) in watched {
        let times = transmissions(&capture, start);
        let longest = times
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .max()
            .unwrap_or_default();
        match setting {
            TcpSetting::RetransmissionTimeoutMinimum(_) => assert!(
                times.len() > 1 && times[1] - times[0] < SHORT_SETTING_TIME + QUICK_WITHIN * 2,
                "{times:?}"
            ),
            TcpSetting::RetransmissionTimeoutMaximum(_) => assert!(
                times.last() > Some(&(RETRANSMISSION_CEILING * 2))
                    && longest < RETRANSMISSION_CEILING + TIMER_SLACK,
                "{times:?}"
            ),
            _ => assert!(
                times
                    .iter()
                    .filter(|time| **time < LINEAR_TRANSMISSIONS_WITHIN)
                    .count()
                    >= LINEAR_TRANSMISSIONS,
                "{times:?}"
            ),
        }
        close(&sockets, pair);
    }
}

/// The peer answering nothing, a user timeout ends the connection; lifted, the connection waits on.
pub fn user_timeout_ends_an_unacknowledged_connection<S: TcpSockets>(sockets: S) {
    let timeout = TcpSetting::UserTimeout(Some(USER_TIMEOUT));
    let timed = pair_with(&sockets, std::slice::from_ref(&timeout), &[]);
    let lifted = pair_with(&sockets, &[timeout, TcpSetting::UserTimeout(None)], &[]);
    let start = Instant::now();
    for pair in [&timed, &lifted] {
        drop_arrivals(&sockets, &pair.server, IpVersion::V4);
        send_all(&sockets, &pair.client, plain(), MESSAGE);
    }
    assert_eq!(
        receive_outcome(&sockets, &timed.client, DEADLINE).map(|got| got.length),
        Err(errno(Errno::ETIMEDOUT))
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed >= USER_TIMEOUT && elapsed < USER_TIMEOUT * BOUND_RATIO as u32,
        "{elapsed:?}"
    );
    let mut buffer = [0; 1];
    assert_eq!(
        sockets
            .receive(&lifted.client, TcpReceiveRequest::Read, &mut buffer)
            .map(|got| got.length),
        Err(HostError::WouldBlock)
    );
    close(&sockets, timed);
    close(&sockets, lifted);
}

/// A listener dropping every SYN, a connect retries once and then fails.
pub fn synchronize_retries_end_an_unanswered_connect<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(
        &sockets,
        ipv4_loopback(),
        &[TcpListenerSetting::Socket(TcpSetting::MinimumHopLimit {
            version: IpVersion::V4,
            minimum: Some(HopLimit(u8::MAX)),
        })],
    );
    let capture = Capture::start(local);
    let client = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[TcpSetting::SynchronizeRetries(RetryCount(1))],
            TcpFastOpen::Off,
        )
        .expect("connect")
        .connection;
    // With the default retries the connect would last minutes.
    assert_eq!(
        receive_outcome(&sockets, &client, DEADLINE).map(|got| got.length),
        Err(errno(Errno::ETIMEDOUT))
    );
    let synchronizes = capture
        .segments()
        .iter()
        .filter(|segment| segment.to_listener && segment.synchronize)
        .count();
    assert!(synchronizes > 1, "{synchronizes} SYNs");
    sockets.close(client, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

/// A listener deferring accepts gives a connection only once data has come on it; with no deferral, at once.
pub fn defer_accept_holds_a_connection_until_data<S: TcpSockets>(sockets: S) {
    let deferring = TcpListenerSetting::Socket(TcpSetting::DeferAccept(Some(SETTING_TIME)));
    for (settings, deferred) in [
        (vec![deferring.clone()], true),
        (
            vec![
                deferring,
                TcpListenerSetting::Socket(TcpSetting::DeferAccept(None)),
            ],
            false,
        ),
    ] {
        let (listener, local) = listen(&sockets, ipv4_loopback(), &settings);
        let client = sockets
            .connect(
                TcpProtocol::Tcp,
                OWNER,
                bind_any_port(ipv4_any()),
                local,
                &[],
                TcpFastOpen::Off,
            )
            .expect("connect")
            .connection;
        if deferred {
            std::thread::sleep(SETTLE);
            let mut header = [0; 0];
            assert_eq!(
                sockets.accept(&listener, &mut header).map(|_| ()),
                Err(HostError::WouldBlock)
            );
            send_all(&sockets, &client, plain(), MESSAGE);
        }
        let server = accept(&sockets, &listener);
        if deferred {
            assert_eq!(receive_up_to(&sockets, &server, MESSAGE.len()), MESSAGE);
        }
        sockets.close(client, TcpClose::Orderly);
        sockets.close(server, TcpClose::Orderly);
        sockets.stop_listening(listener);
    }
}

/// Closed in order, a connection whose FIN-WAIT-2 resets ends with a reset; one whose FIN-WAIT-2 lasts a second is
/// gone after it, so the peer's FIN is answered by a reset; one with the system's default still answers it in order.
pub fn fin_wait_2_settings_decide_how_a_closed_connection_ends<S: TcpSockets>(sockets: S) {
    let resetting = TcpSetting::FinWait2(TcpFinWait2::Reset);
    let ends = [
        vec![resetting.clone()],
        vec![TcpSetting::FinWait2(TcpFinWait2::Lasts(FIN_WAIT_2_LASTS))],
        vec![resetting, TcpSetting::FinWait2(TcpFinWait2::SystemDefault)],
    ]
    .map(|settings| {
        let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), &settings, &[]);
        let start = Instant::now();
        sockets.close(pair.client, TcpClose::Orderly);
        assert_eq!(
            receive_outcome(&sockets, &pair.server, DEADLINE).map(|got| got.length),
            Ok(ByteCount(0))
        );
        (pair.listener, pair.server, capture, start)
    });
    let reset_after = |capture: &Capture, start: Instant, within: Duration| {
        capture
            .wait_for(start, within, |segment| {
                segment.to_listener && segment.reset
            })
            .is_some()
    };
    let [reset, lasting, default] = ends;
    assert!(reset_after(&reset.2, reset.3, PUSHED_WITHIN), "no reset");
    std::thread::sleep(FIN_WAIT_2_LASTS + TIMER_SLACK);
    let finished = Instant::now();
    for (_, server, _, _) in [&lasting, &default] {
        sockets
            .shut_down(server, TcpShutdown::Write)
            .expect("shut down");
    }
    assert!(
        reset_after(&lasting.2, finished, PUSHED_WITHIN),
        "no reset after FIN-WAIT-2 ended"
    );
    std::thread::sleep(PUSHED_WITHIN);
    assert!(
        !reset_after(&default.2, finished, Duration::ZERO),
        "a reset in the default FIN-WAIT-2"
    );
    for (listener, server, _, _) in [reset, lasting, default] {
        sockets.close(server, TcpClose::Orderly);
        sockets.stop_listening(listener);
    }
}

/// Each setting that drops arrivals holds the peer's data back until it is lifted.
pub fn arrival_settings_drop_what_they_refuse<S: TcpSockets>(sockets: S) {
    for (address, refusing, lifting) in [
        (
            ipv4_loopback(),
            TcpSetting::Filter(Some(SocketFilter(&DROP_ALL))),
            TcpSetting::Filter(None),
        ),
        (
            ipv4_loopback(),
            TcpSetting::MinimumHopLimit {
                version: IpVersion::V4,
                minimum: Some(HopLimit(u8::MAX)),
            },
            TcpSetting::MinimumHopLimit {
                version: IpVersion::V4,
                minimum: None,
            },
        ),
        (
            ipv6_loopback(),
            TcpSetting::MinimumHopLimit {
                version: IpVersion::V6,
                minimum: Some(HopLimit(u8::MAX)),
            },
            TcpSetting::MinimumHopLimit {
                version: IpVersion::V6,
                minimum: None,
            },
        ),
    ] {
        let (pair, _capture) = watched_pair(&sockets, address, &[], &[]);
        sockets
            .configure(&pair.server, refusing.clone())
            .expect("refuse arrivals");
        send_all(&sockets, &pair.client, plain(), MESSAGE);
        assert_silent(&sockets, &pair.server, HELD_FOR);
        sockets
            .configure(&pair.server, lifting)
            .expect("take arrivals");
        assert_eq!(
            receive_up_to(&sockets, &pair.server, MESSAGE.len()),
            MESSAGE,
            "{refusing:?}"
        );
        close(&sockets, pair);
    }
    let pair = pair_with(&sockets, &[], &[]);
    sockets
        .configure(
            &pair.server,
            TcpSetting::Filter(Some(SocketFilter(&ACCEPT_ALL))),
        )
        .expect("a filter that keeps everything");
    assert_carries_bytes(&sockets, &pair);
    close(&sockets, pair);
}

/// The hop limit, traffic class, fragmenting and flow label settings show in the headers of what the connection sends.
pub fn header_settings_mark_each_segment<S: TcpSockets>(sockets: S) {
    let data_after = |pair: &Pair<S>, capture: &Capture| {
        let start = Instant::now();
        send_all(&sockets, &pair.client, plain(), MESSAGE);
        capture
            .wait_for(start, DEADLINE, Segment::is_data_to_listener)
            .expect("a data segment")
    };
    let limit = HopLimit(SETTING_COUNT);
    let (pair, capture) = watched_pair(
        &sockets,
        ipv4_loopback(),
        &[
            TcpSetting::Packet(IpPacketSetting::HopLimit {
                version: IpVersion::V4,
                limit: Some(limit),
            }),
            TcpSetting::Packet(IpPacketSetting::TrafficClass {
                version: IpVersion::V4,
                dscp: IpDscp::AF41,
                ecn: IpEcn::ZERO,
            }),
            TcpSetting::Packet(IpPacketSetting::PathMtuDiscovery {
                version: IpVersion::V4,
                mode: PathMtuDiscovery::Dont,
            }),
        ],
        &[],
    );
    let marked = data_after(&pair, &capture);
    assert_eq!(
        (marked.hop_limit, marked.dscp, marked.dont_fragment),
        (limit.0, Some(IpDscp::AF41), false)
    );
    for setting in [
        IpPacketSetting::HopLimit {
            version: IpVersion::V4,
            limit: None,
        },
        IpPacketSetting::PathMtuDiscovery {
            version: IpVersion::V4,
            mode: PathMtuDiscovery::Do,
        },
    ] {
        sockets
            .configure(&pair.client, TcpSetting::Packet(setting))
            .expect("a header setting");
    }
    let plain_segment = data_after(&pair, &capture);
    let listener_side = capture.first(|segment| !segment.to_listener);
    assert_eq!(
        (plain_segment.hop_limit, plain_segment.dont_fragment),
        (listener_side.hop_limit, true)
    );
    close(&sockets, pair);
    let (pair, capture) = watched_pair(
        &sockets,
        ipv6_loopback(),
        &[
            TcpSetting::Packet(IpPacketSetting::HopLimit {
                version: IpVersion::V6,
                limit: Some(limit),
            }),
            TcpSetting::Packet(IpPacketSetting::FlowLabel(FlowLabelChoice::Unlabelled)),
        ],
        &[],
    );
    let marked = data_after(&pair, &capture);
    assert_eq!((marked.hop_limit, marked.flow_label), (limit.0, Some(0)));
    sockets
        .configure(
            &pair.client,
            TcpSetting::Packet(IpPacketSetting::FlowLabel(FlowLabelChoice::Automatic)),
        )
        .expect("automatic flow labels");
    assert_ne!(data_after(&pair, &capture).flow_label, Some(0));
    close(&sockets, pair);
}

/// A congestion control that needs explicit congestion notification marks the data it sends as capable of it.
pub fn congestion_control_is_the_one_named<S: TcpSockets>(sockets: S) {
    let control = TcpSetting::CongestionControl(NOTIFYING_CONGESTION_CONTROL);
    let (pair, capture) = watched_pair(
        &sockets,
        ipv4_loopback(),
        std::slice::from_ref(&control),
        &[TcpListenerSetting::Socket(control.clone())],
    );
    let start = Instant::now();
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    let data = capture
        .wait_for(start, DEADLINE, Segment::is_data_to_listener)
        .expect("a data segment");
    assert_eq!(data.ecn, Some(IpEcn::TWO));
    close(&sockets, pair);
}

/// A pacing rate spreads a transfer out over time; lifted, the transfer is quick.
pub fn pacing_rate_spreads_what_is_sent<S: TcpSockets>(sockets: S) {
    let pacing = |rate| TcpSetting::Ip(IpSocketSetting::MaximumPacingRate(rate));
    let bytes = pattern(PACED_BYTES);
    for (settings, paced) in [
        (vec![pacing(Some(PACING_RATE))], true),
        (vec![pacing(Some(PACING_RATE)), pacing(None)], false),
    ] {
        let pair = pair_with(&sockets, &settings, &[]);
        let start = Instant::now();
        assert_eq!(
            transfer(&sockets, &pair.client, &pair.server, &bytes),
            bytes
        );
        let elapsed = start.elapsed();
        assert_eq!(elapsed > PACED_AT_LEAST, paced, "{settings:?}: {elapsed:?}");
        close(&sockets, pair);
    }
}

/// A transmit delay holds each segment back by that long; set to nothing, segments leave at once.
pub fn transmit_delay_holds_each_segment<S: TcpSockets>(sockets: S) {
    let pair = pair_with(&sockets, &[TcpSetting::TransmitDelay(TRANSMIT_DELAY)], &[]);
    let timed = |pair: &Pair<S>| {
        let start = Instant::now();
        send_all(&sockets, &pair.client, plain(), MESSAGE);
        assert_eq!(
            receive_up_to(&sockets, &pair.server, MESSAGE.len()),
            MESSAGE
        );
        start.elapsed()
    };
    let delayed = timed(&pair);
    assert!(delayed > TRANSMIT_DELAY - QUICK_WITHIN, "{delayed:?}");
    sockets
        .configure(&pair.client, TcpSetting::TransmitDelay(Duration::ZERO))
        .expect("no delay");
    let quick = timed(&pair);
    assert!(quick < TRANSMIT_DELAY - QUICK_WITHIN, "{quick:?}");
    close(&sockets, pair);
}

/// Connections sharing their address leave from one port to different peers; one not sharing cannot take it.
pub fn address_sharing_lets_connections_leave_from_one_port<S: TcpSockets>(sockets: S) {
    let shared = [TcpSetting::Port(PortSetting::AddressSharing(
        AddressSharing::Shared,
    ))];
    let listeners = [(); 3].map(|_| listen(&sockets, ipv4_loopback(), &[]));
    let connect = |port: Port, peer: SocketAddr, settings: &[TcpSetting<'_>]| {
        sockets.connect(
            TcpProtocol::Tcp,
            OWNER,
            LocalBinding {
                address: ipv4_loopback(),
                port: LocalPort::Requested(port),
            },
            peer,
            settings,
            TcpFastOpen::Off,
        )
    };
    let first = connect(Port(0), listeners[0].1, &shared).expect("the first connection");
    let port = Port(first.local.port());
    let second = connect(port, listeners[1].1, &shared).expect("a second from the port");
    assert_eq!(second.local, first.local);
    assert_eq!(
        connect(port, listeners[2].1, &[]).map(|connected| connected.local),
        Err(errno(Errno::EADDRINUSE))
    );
    sockets.close(first.connection, TcpClose::Orderly);
    sockets.close(second.connection, TcpClose::Orderly);
    for (listener, _) in listeners {
        sockets.stop_listening(listener);
    }
}

fn listen_at<S: TcpSockets>(
    sockets: &S,
    address: BindAddress,
    port: Port,
    settings: &[TcpListenerSetting<'_>],
) -> Result<TcpListening<S::Listener>, HostError> {
    sockets.listen(
        TcpProtocol::Tcp,
        OWNER,
        LocalBinding {
            address,
            port: LocalPort::Requested(port),
        },
        BACKLOG,
        settings,
    )
}

/// Listeners sharing their port listen on one; one not sharing cannot.
pub fn port_sharing_lets_listeners_share_a_port<S: TcpSockets>(sockets: S) {
    let shared = [TcpListenerSetting::Socket(TcpSetting::Port(
        PortSetting::PortSharing(PortSharing::Shared),
    ))];
    let first = listen_at(&sockets, ipv4_loopback(), Port(0), &shared).expect("listen");
    let port = Port(first.local.port());
    let second = listen_at(&sockets, ipv4_loopback(), port, &shared).expect("listen on the port");
    assert_eq!(
        listen_at(&sockets, ipv4_loopback(), port, &[]).map(|listening| listening.local),
        Err(errno(Errno::EADDRINUSE))
    );
    sockets.stop_listening(first.listener);
    sockets.stop_listening(second.listener);
}

/// An address no interface holds can be bound only while nonlocal addresses are bindable.
pub fn nonlocal_addresses_are_bound_only_when_bindable<S: TcpSockets>(sockets: S) {
    let address = BindAddress::V4(NONLOCAL);
    let setting = |addresses| {
        TcpListenerSetting::Socket(TcpSetting::Port(PortSetting::NonlocalAddresses(addresses)))
    };
    let bound = listen_at(
        &sockets,
        address,
        Port(0),
        &[setting(NonlocalAddresses::Bindable)],
    )
    .expect("listen on a nonlocal address");
    sockets.stop_listening(bound.listener);
    for settings in [
        vec![],
        vec![
            setting(NonlocalAddresses::Bindable),
            setting(NonlocalAddresses::Refused),
        ],
    ] {
        assert_eq!(
            listen_at(&sockets, address, Port(0), &settings).map(|listening| listening.local),
            Err(errno(Errno::EADDRNOTAVAIL))
        );
    }
}

/// A listener of backlog 0, from the listen or set later, holds one connection to accept, not the three waiting.
pub fn listener_backlog_bounds_the_accept_queue<S: TcpSockets>(sockets: S) {
    let smallest = TcpListenerSetting::Backlog(ListenBacklog(0));
    for later in [false, true] {
        let settings = if later {
            vec![]
        } else {
            vec![smallest.clone()]
        };
        let (listener, local) = listen(&sockets, ipv4_loopback(), &settings);
        if later {
            sockets
                .configure(&listener, smallest.clone())
                .expect("a smaller backlog");
        }
        let clients = [(); 3].map(|_| {
            sockets
                .connect(
                    TcpProtocol::Tcp,
                    OWNER,
                    bind_any_port(ipv4_any()),
                    local,
                    &[],
                    TcpFastOpen::Off,
                )
                .expect("connect")
                .connection
        });
        std::thread::sleep(SETTLE);
        let mut header = [0; 0];
        let mut accepted = Vec::new();
        let start = Instant::now();
        while start.elapsed() < HELD_FOR {
            if let Ok(taken) = sockets.accept(&listener, &mut header) {
                accepted.push(taken.connection);
            }
        }
        assert_eq!(accepted.len(), 1, "later: {later}");
        for connection in accepted.into_iter().chain(clients) {
            sockets.close(connection, TcpClose::Orderly);
        }
        sockets.stop_listening(listener);
    }
}

/// The flow label of the next data segment, which IPv6 connections carry.
fn data_flow_label<S: TcpSockets>(sockets: &S, pair: &Pair<S>, capture: &Capture) -> u32 {
    let start = Instant::now();
    send_all(sockets, &pair.client, plain(), MESSAGE);
    capture
        .wait_for(start, DEADLINE, Segment::is_data_to_listener)
        .and_then(|segment| segment.flow_label)
        .expect("an IPv6 data segment")
}

/// Negative advice makes a connection pick a new flow, which shows in its flow label, unless rehashing is off.
pub fn negative_route_advice_picks_a_new_flow<S: TcpSockets>(sockets: S) {
    let rehash = TcpSetting::TransmitRehash;
    for (settings, rehashed) in [
        (vec![], true),
        (vec![rehash(TransmitRehash::Disabled)], false),
        (
            vec![
                rehash(TransmitRehash::Disabled),
                rehash(TransmitRehash::Enabled),
            ],
            true,
        ),
        (
            vec![
                rehash(TransmitRehash::Disabled),
                rehash(TransmitRehash::SystemDefault),
            ],
            true,
        ),
    ] {
        let (pair, capture) = watched_pair(&sockets, ipv6_loopback(), &settings, &[]);
        let before = data_flow_label(&sockets, &pair, &capture);
        sockets
            .give_negative_route_advice(&pair.client)
            .expect("advice");
        let after = data_flow_label(&sockets, &pair, &capture);
        assert_eq!(before != after, rehashed, "{settings:?}");
        close(&sockets, pair);
    }
}

/// A listener reflecting flow labels answers a SYN with the SYN's own flow label.
pub fn flow_label_reflection_answers_with_the_peers_label<S: TcpSockets>(sockets: S) {
    let reflecting = TcpListenerSetting::FlowLabelReflection(true);
    for (settings, reflected) in [
        (vec![reflecting.clone()], true),
        (
            vec![reflecting, TcpListenerSetting::FlowLabelReflection(false)],
            false,
        ),
    ] {
        let (pair, capture) = watched_pair(&sockets, ipv6_loopback(), &[], &settings);
        let synchronizing = |to_listener: bool| {
            capture.first(|segment| segment.to_listener == to_listener && segment.synchronize)
        };
        let (synchronize, answer) = (synchronizing(true), synchronizing(false));
        assert_eq!(
            answer.flow_label == synchronize.flow_label,
            reflected,
            "{settings:?}"
        );
        close(&sockets, pair);
    }
}

/// A window probe goes out one sequence number below the next and is answered; the connection keeps sharing its
/// address and, when in repair mode, stays in it.
pub fn window_probe_is_answered_and_changes_no_setting<S: TcpSockets>(sockets: S) {
    let shared = [TcpSetting::Port(PortSetting::AddressSharing(
        AddressSharing::Shared,
    ))];
    let from_port = |port: u16| LocalBinding {
        address: ipv4_loopback(),
        port: LocalPort::Requested(Port(port)),
    };
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    let capture = Capture::start(local);
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            from_port(0),
            local,
            &shared,
            TcpFastOpen::Off,
        )
        .expect("connect");
    let server = accept(&sockets, &listener);
    let pair = Pair {
        listener,
        local,
        client: connected.connection,
        server,
    };
    assert_carries_bytes(&sockets, &pair);
    std::thread::sleep(SETTLE);
    let next = capture.next_to_listener();
    let start = Instant::now();
    sockets
        .send_window_probe(&pair.client)
        .expect("window probe");
    let probe = capture
        .wait_for(start, PUSHED_WITHIN, |segment| is_probe(segment, next))
        .expect("the probe");
    capture
        .wait_for(start, PUSHED_WITHIN, |segment| {
            !segment.to_listener && segment.at >= probe.at && segment.acknowledgement == next
        })
        .expect("its answer");
    let (other, peer) = listen(&sockets, ipv4_loopback(), &[]);
    let second = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            from_port(connected.local.port()),
            peer,
            &shared,
            TcpFastOpen::Off,
        )
        .expect("a second connection from the port the probing one shares");
    sockets.close(second.connection, TcpClose::Abort);
    sockets.stop_listening(other);
    let clock = TcpTimestampClock {
        value: TcpTimestamp(CLOCK_READING),
        unit: TcpTimestampUnit::Milliseconds,
    };
    sockets
        .configure(&pair.client, TcpSetting::Repair(TcpRepair::On))
        .expect("repair mode");
    sockets
        .send_window_probe(&pair.client)
        .expect("window probe in repair mode");
    assert_eq!(sockets.set_timestamp_clock(&pair.client, clock), Ok(()));
    sockets
        .configure(&pair.client, TcpSetting::Repair(TcpRepair::Off))
        .expect("out of repair mode");
    close(&sockets, pair);
}

/// Outside repair mode the timestamp clock cannot be set; in it, the value set is what the next segments carry.
pub fn timestamp_clock_set_in_repair_mode_stamps_segments<S: TcpSockets>(sockets: S) {
    let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), &[], &[]);
    let clock = TcpTimestampClock {
        value: TcpTimestamp(CLOCK_READING),
        unit: TcpTimestampUnit::Milliseconds,
    };
    assert_eq!(
        sockets.set_timestamp_clock(&pair.client, clock),
        Err(errno(Errno::EPERM))
    );
    assert_eq!(
        sockets.set_listener_timestamp_clock(&pair.listener, clock),
        Err(errno(Errno::EPERM))
    );
    sockets
        .configure(&pair.client, TcpSetting::Repair(TcpRepair::On))
        .expect("repair mode");
    let set = Instant::now();
    sockets
        .set_timestamp_clock(&pair.client, clock)
        .expect("set the clock");
    sockets
        .configure(&pair.client, TcpSetting::Repair(TcpRepair::Off))
        .expect("out of repair mode");
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    let data = capture
        .wait_for(set, DEADLINE, Segment::is_data_to_listener)
        .expect("a data segment");
    let since = data
        .timestamp
        .expect("a timestamp")
        .wrapping_sub(CLOCK_READING);
    assert!(
        u128::from(since) <= set.elapsed().as_millis() + 1,
        "{since} ms after the reading"
    );
    close(&sockets, pair);
}

/// The settings of a connection checked for being set idempotently, each taking no privilege.
fn connection_settings() -> Vec<TcpSetting<'static>> {
    vec![
        TcpSetting::Ip(IpSocketSetting::SendBuffer(ByteCount(SETTING_BYTES))),
        TcpSetting::Ip(IpSocketSetting::ReceiveBuffer(ByteCount(SETTING_BYTES))),
        TcpSetting::Ip(IpSocketSetting::MaximumPacingRate(Some(BytesPerSecond(
            SETTING_BYTES as u64,
        )))),
        TcpSetting::Ip(IpSocketSetting::MaximumPacingRate(None)),
        TcpSetting::Ip(IpSocketSetting::MulticastLoopback(true)),
        TcpSetting::Ip(IpSocketSetting::MulticastAllGroups {
            version: IpVersion::V4,
            enabled: false,
        }),
        TcpSetting::Ip(IpSocketSetting::Timestamping(Timestamping {
            receive_software: Report::Requested,
            receive_hardware: Report::NotRequested,
            bound_clock: None,
        })),
        TcpSetting::Packet(IpPacketSetting::HopLimit {
            version: IpVersion::V4,
            limit: Some(HopLimit(SETTING_COUNT)),
        }),
        TcpSetting::Packet(IpPacketSetting::HopLimit {
            version: IpVersion::V4,
            limit: None,
        }),
        TcpSetting::Packet(IpPacketSetting::PathMtuDiscovery {
            version: IpVersion::V4,
            mode: PathMtuDiscovery::Do,
        }),
        TcpSetting::Packet(IpPacketSetting::Priority(PacketPriority(u32::from(
            SETTING_COUNT,
        )))),
        TcpSetting::Packet(IpPacketSetting::RouteScope(RouteScope::Universe)),
        TcpSetting::Port(PortSetting::AddressSharing(AddressSharing::Shared)),
        TcpSetting::Port(PortSetting::PortSharing(PortSharing::Shared)),
        TcpSetting::Port(PortSetting::NonlocalAddresses(NonlocalAddresses::Bindable)),
        TcpSetting::Port(PortSetting::NonlocalAddresses(NonlocalAddresses::Refused)),
        TcpSetting::Port(PortSetting::BoundInterface(None)),
        TcpSetting::Port(PortSetting::IncomingProcessor(None)),
        TcpSetting::BufferLocks(BufferLocks {
            send: BufferTuning::Locked,
            receive: BufferTuning::Automatic,
        }),
        TcpSetting::ReceiveLowWater(ByteCount(1)),
        TcpSetting::Filter(Some(SocketFilter(&ACCEPT_ALL))),
        TcpSetting::Filter(None),
        TcpSetting::TransmitRehash(TransmitRehash::Enabled),
        TcpSetting::TransmitRehash(TransmitRehash::SystemDefault),
        TcpSetting::MinimumHopLimit {
            version: IpVersion::V4,
            minimum: Some(HopLimit(1)),
        },
        TcpSetting::MinimumHopLimit {
            version: IpVersion::V4,
            minimum: None,
        },
        TcpSetting::IcmpErrorReports {
            version: IpVersion::V4,
            reports: Report::Requested,
        },
        TcpSetting::KeepAlive(true),
        TcpSetting::KeepAliveIdle(SETTING_TIME),
        TcpSetting::KeepAliveInterval(SETTING_TIME),
        TcpSetting::KeepAliveProbes(ProbeCount(SETTING_COUNT)),
        TcpSetting::NoDelay(true),
        TcpSetting::Cork(false),
        TcpSetting::UrgentInline(true),
        TcpSetting::QuickAcknowledgement(TcpQuickAcknowledgement::On),
        TcpSetting::MaximumSegmentSize(Some(SEGMENT)),
        TcpSetting::WindowClamp(Some(WindowSize(SETTING_BYTES as u32))),
        TcpSetting::SynchronizeRetries(RetryCount(SETTING_COUNT)),
        TcpSetting::UserTimeout(Some(SETTING_TIME)),
        TcpSetting::UserTimeout(None),
        TcpSetting::FinWait2(TcpFinWait2::Lasts(SETTING_TIME)),
        TcpSetting::FinWait2(TcpFinWait2::Reset),
        TcpSetting::FinWait2(TcpFinWait2::SystemDefault),
        TcpSetting::RetransmissionTimeoutMinimum(SHORT_SETTING_TIME),
        TcpSetting::RetransmissionTimeoutMaximum(SETTING_TIME),
        TcpSetting::DelayedAcknowledgementMaximum(SHORT_SETTING_TIME),
        TcpSetting::TransmitDelay(SHORT_SETTING_TIME),
        TcpSetting::ThinLinearTimeouts(true),
        TcpSetting::CongestionControl(CONGESTION_CONTROL),
        TcpSetting::NotSentLowWater(Some(ByteCount(SETTING_BYTES))),
        TcpSetting::NotSentLowWater(None),
        TcpSetting::FastOpenWithoutCookie(true),
        TcpSetting::UnreadByteReports(Report::Requested),
    ]
}

/// Each setting given to connect once and twice, then set once and twice on the established connections: the outcomes
/// agree and the connections carry bytes.
pub fn connection_settings_set_twice_equal_set_once<S: TcpSockets>(sockets: S) {
    for setting in connection_settings() {
        let once = pair_with(&sockets, std::slice::from_ref(&setting), &[]);
        let twice = pair_with(&sockets, &[setting.clone(), setting.clone()], &[]);
        assert_carries_bytes(&sockets, &once);
        assert_carries_bytes(&sockets, &twice);
        let first = sockets.configure(&once.client, setting.clone());
        let outcomes = [
            first,
            sockets.configure(&twice.client, setting.clone()),
            sockets.configure(&twice.client, setting.clone()),
        ];
        assert!(
            outcomes.iter().all(|outcome| *outcome == first),
            "{setting:?}: {outcomes:?}"
        );
        assert_carries_bytes(&sockets, &once);
        assert_carries_bytes(&sockets, &twice);
        close(&sockets, once);
        close(&sockets, twice);
    }
}

/// Each listener setting set once and twice on fresh listeners: the outcomes agree, every setting an implementation
/// must have succeeds, and the listeners still accept connections that carry bytes.
pub fn listener_settings_set_twice_equal_set_once<S: TcpSockets>(sockets: S) {
    let key = FastOpenKey([u8::MAX; FAST_OPEN_KEY_LENGTH]);
    let settings = [
        (
            TcpListenerSetting::Backlog(ListenBacklog(u32::from(SETTING_COUNT))),
            true,
        ),
        (TcpListenerSetting::FastOpenQueue(FAST_OPEN_QUEUE), true),
        (TcpListenerSetting::FastOpenQueue(None), true),
        (
            TcpListenerSetting::FastOpenKeys(FastOpenKeys {
                primary: key,
                backup: Some(key),
            }),
            true,
        ),
        (
            TcpListenerSetting::SaveSynchronize(TcpSavedSynchronize::FromNetworkHeader),
            true,
        ),
        (
            TcpListenerSetting::SaveSynchronize(TcpSavedSynchronize::Off),
            true,
        ),
        (
            TcpListenerSetting::SaveSynchronize(TcpSavedSynchronize::FromLinkHeader),
            false,
        ),
        (
            TcpListenerSetting::Socket(TcpSetting::DeferAccept(None)),
            true,
        ),
        (TcpListenerSetting::Socket(TcpSetting::NoDelay(true)), true),
    ];
    for (setting, required) in settings {
        let once = pair(&sockets);
        let first = sockets.configure(&once.listener, setting.clone());
        let twice = pair(&sockets);
        let outcomes = [
            first,
            sockets.configure(&twice.listener, setting.clone()),
            sockets.configure(&twice.listener, setting.clone()),
        ];
        assert!(
            outcomes.iter().all(|outcome| *outcome == first),
            "{setting:?}: {outcomes:?}"
        );
        if required {
            assert_eq!(first, Ok(()), "{setting:?}");
        }
        for pair in [once, twice] {
            let client = sockets
                .connect(
                    TcpProtocol::Tcp,
                    OWNER,
                    bind_any_port(ipv4_any()),
                    pair.local,
                    &[],
                    TcpFastOpen::Off,
                )
                .expect("connect after the setting")
                .connection;
            let server = accept(&sockets, &pair.listener);
            send_all(&sockets, &client, plain(), MESSAGE);
            assert_eq!(receive_up_to(&sockets, &server, MESSAGE.len()), MESSAGE);
            sockets.close(client, TcpClose::Orderly);
            sockets.close(server, TcpClose::Orderly);
            close(&sockets, pair);
        }
    }
}

/// Connection settings an implementation may refuse, set once and twice on established connections: the outcomes agree.
pub fn refusable_connection_settings_set_twice_equal_set_once<S: TcpSockets>(sockets: S) {
    {
        let setting = TcpSetting::RecordsLastReceivedHeader(Report::Requested);
        let once = pair(&sockets);
        let twice = pair(&sockets);
        let first = sockets.configure(&once.client, setting.clone());
        let outcomes = [
            first,
            sockets.configure(&twice.client, setting.clone()),
            sockets.configure(&twice.client, setting.clone()),
        ];
        assert!(
            outcomes.iter().all(|outcome| *outcome == first),
            "{setting:?}: {outcomes:?}"
        );
        assert_carries_bytes(&sockets, &once);
        assert_carries_bytes(&sockets, &twice);
        close(&sockets, once);
        close(&sockets, twice);
    }
}

/// The contract of `Configurable` on a connection, by its own suite.
pub fn connection_configurable_contract_holds<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    wcontainers_configurable_conformance::setting_twice_equals_setting_once((
        sockets,
        pair.client,
        TcpSetting::NoDelay(true),
    ));
}

/// The contract of `Configurable` on a listener, by its own suite.
pub fn listener_configurable_contract_holds<S: TcpSockets>(sockets: S) {
    let (listener, _) = listen(&sockets, ipv4_loopback(), &[]);
    wcontainers_configurable_conformance::setting_twice_equals_setting_once((
        sockets,
        listener,
        TcpListenerSetting::Backlog(BACKLOG),
    ));
}

/// The contract of `NegativeRouteAdvice` on a connection, by its own suite.
pub fn negative_route_advice_contract_holds<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    wcontainers_negative_route_advice_conformance::advice_is_taken_again((sockets, pair.client));
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        $crate::tests!(@each $value;
            connect_and_accept_name_both_ends,
            accept_cuts_the_synchronize_header_to_the_buffer,
            connect_settings_apply_to_the_synchronize,
            bytes_arrive_in_order_both_ways,
            connects_over_ipv6,
            dual_stack_listener_accepts_ipv4,
            multipath_connections_carry_bytes,
            requested_port_is_bound,
            exit_node_chooses_a_port_outside_the_reserved,
            stopped_listener_refuses_connections,
            accept_without_a_connection_would_block,
            write_shutdown_ends_the_peers_stream,
            read_shutdown_ends_the_own_stream,
            shutdown_of_both_directions_ends_both,
            orderly_close_ends_the_peers_stream,
            abort_resets_the_peer,
            peek_leaves_the_bytes,
            peek_at_an_offset_skips_bytes,
            urgent_byte_is_read_apart,
            urgent_inline_keeps_the_byte_in_the_stream,
            cork_holds_data_until_uncorked,
            more_to_come_holds_data_until_pushed,
            push_ignoring_neither_keeps_the_cork,
            push_ignoring_neither_sends_data_held_for_more,
            record_end_starts_a_new_segment,
            unread_byte_reports_count_what_is_left,
            receive_software_timestamps_when_asked,
            software_transmit_report_names_the_send,
            acknowledgement_report_follows_the_peers_acknowledgement,
            discarding_empties_the_error_queue,
            empty_error_queue_would_block,
            icmp_error_reaches_a_synchronizing_connection_over_ipv4,
            icmp_error_reaches_a_synchronizing_connection_over_ipv6,
            transmit_report_carries_the_packet_when_asked,
            transmit_statistics_come_with_the_report_when_asked,
            wireless_status_request_still_sends,
            fast_open_with_data_puts_it_in_the_synchronize,
            fast_open_until_first_send_defers_the_synchronize,
            fast_open_without_cookie_needs_no_first_connect,
            fast_open_keys_and_queue_decide_what_the_listener_takes,
            keep_alive_probes_end_an_unanswered_connection,
            rearming_the_keep_alive_timer_sends_a_due_probe,
            no_delay_sends_small_segments_while_others_are_unacknowledged,
            quick_acknowledgement_settings_time_the_acknowledgement,
            delayed_acknowledgement_is_sent_on_demand,
            send_settings_bound_what_a_connection_takes,
            receive_settings_bound_what_the_peer_sends,
            window_clamp_bounds_the_synchronize_window,
            retransmission_settings_time_the_retransmissions,
            user_timeout_ends_an_unacknowledged_connection,
            synchronize_retries_end_an_unanswered_connect,
            defer_accept_holds_a_connection_until_data,
            fin_wait_2_settings_decide_how_a_closed_connection_ends,
            arrival_settings_drop_what_they_refuse,
            header_settings_mark_each_segment,
            congestion_control_is_the_one_named,
            pacing_rate_spreads_what_is_sent,
            transmit_delay_holds_each_segment,
            address_sharing_lets_connections_leave_from_one_port,
            port_sharing_lets_listeners_share_a_port,
            nonlocal_addresses_are_bound_only_when_bindable,
            listener_backlog_bounds_the_accept_queue,
            negative_route_advice_picks_a_new_flow,
            flow_label_reflection_answers_with_the_peers_label,
            window_probe_is_answered_and_changes_no_setting,
            timestamp_clock_set_in_repair_mode_stamps_segments,
            connection_settings_set_twice_equal_set_once,
            refusable_connection_settings_set_twice_equal_set_once,
            listener_settings_set_twice_equal_set_once,
            connection_configurable_contract_holds,
            listener_configurable_contract_holds,
            negative_route_advice_contract_holds,
        );
    };
    (@each $value:expr; $($check:ident),* $(,)?) => {
        $(
            #[test]
            fn $check() {
                $crate::$check($value);
            }
        )*
    };
}
