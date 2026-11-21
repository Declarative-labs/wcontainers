//! The tests every implementation of wcontainers-tcp-sockets runs, written against its traits alone.
//!
//! Each check makes its own listener on a loopback address with a port the kernel chooses, so the checks run side by
//! side. Nothing in the contract waits, so an answer of `WouldBlock` is tried again until [`DEADLINE`].

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use core::num::NonZeroU32;
use core::time::Duration;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use std::os::fd::OwnedFd;

use etherparse::{
    Icmpv4Header, Icmpv4Slice, Icmpv4Type, Icmpv6Header, Icmpv6Slice, Icmpv6Type, IpNumber,
    Ipv4HeaderSlice, Ipv6FlowLabel, Ipv6Header, Ipv6HeaderSlice, TcpHeaderSlice, TcpOptionElement,
    icmpv4, icmpv6,
};
use rustix::net::{AddressFamily, RecvFlags, SendFlags, SocketType, ipproto, sockopt};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    AddressSharing, BindAddress, ByteCount, BytesPerSecond, FilterInstruction, FilterJump,
    FilterOperand, FilterOperation, HopLimit, IpPacketSetting, IpSocketSetting, IpVersion,
    Ipv6Families, LocalBinding, LocalPort, NonlocalAddresses, PacketPriority, PathMtuDiscovery,
    Port, PortChoice, PortRange, PortSetting, PortSharing, ReceivedPacketFields, Report,
    RouteScope, SocketFilter, TimestampSource, Timestamping, TransmitEvent, TransmitReportKey,
    TransmitReportRequest, TransmitStage,
};
use wcontainers_kernel::UserId;
use wcontainers_kernel::errno::Errno;
use wcontainers_tcp_sockets::repair::{TcpTimestamp, TcpTimestampClock, TcpTimestampUnit};
use wcontainers_tcp_sockets::settings::{
    BufferLocks, BufferTuning, FAST_OPEN_KEY_LENGTH, FastOpenKey, FastOpenKeys, MaximumSegmentSize,
    ProbeCount, RetryCount, TcpFinWait2, TcpListenerSetting, TcpQuickAcknowledgement,
    TcpSavedSynchronize, TcpSetting, TransmitRehash, WindowSize,
};
use wcontainers_tcp_sockets::state::TcpErrorQueueEntry;
use wcontainers_tcp_sockets::{
    ListenBacklog, TcpClose, TcpConnectStart, TcpContinuation, TcpFastOpen, TcpProtocol,
    TcpPushOverride, TcpReceiveRequest, TcpRecordBoundary, TcpSendRequest, TcpShutdown, TcpSockets,
    TcpUrgency,
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

const FAST_OPEN_QUEUE: Option<NonZeroU32> = NonZeroU32::new(8);

/// A time, a count and a size every setting of their kind accepts.
const SETTING_TIME: Duration = Duration::from_secs(10);
const SHORT_SETTING_TIME: Duration = Duration::from_millis(40);
const SETTING_COUNT: u8 = 4;
const SETTING_BYTES: usize = 1 << 16;

/// A congestion control algorithm every Linux has.
const CONGESTION_CONTROL: &str = "reno";

/// Classic BPF's `BPF_RET | BPF_K`, which ends a filter returning its operand.
const RETURN_OPERAND: FilterOperation = FilterOperation(0x06);

/// A filter that keeps every packet whole.
static ACCEPT_ALL: [FilterInstruction; 1] = [FilterInstruction {
    operation: RETURN_OPERAND,
    jump_if_true: FilterJump(0),
    jump_if_false: FilterJump(0),
    operand: FilterOperand(u32::MAX),
}];

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

pub fn exit_node_chooses_a_port_outside_the_reserved<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    let first = CHOICE_FIRST;
    let last = CHOICE_LAST;
    let reserved = [PortRange {
        first,
        last: RESERVED_LAST,
    }];
    let choice = PortChoice {
        range: PortRange { first, last },
        reserved: &reserved,
    };
    let binding = LocalBinding {
        address: ipv4_any(),
        port: LocalPort::ChosenByExitNode(choice),
    };
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            binding,
            local,
            &[],
            TcpFastOpen::Off,
        )
        .expect("connect");
    let port = connected.local.port();
    assert!(
        (RESERVED_LAST.0 + 1..=CHOICE_LAST.0).contains(&port),
        "port {port}"
    );
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
    assert!(
        (RESERVED_LAST.0 + 1..=CHOICE_LAST.0).contains(&port),
        "port {port}"
    );
    sockets.stop_listening(listening.listener);
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

pub fn push_ignoring_neither_sends_uncorked_data<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    sockets
        .push_queued_data(&pair.client, TcpPushOverride::Neither)
        .expect("push");
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    close(&sockets, pair);
}

pub fn record_end_is_taken<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let ended = TcpSendRequest {
        record: TcpRecordBoundary::Ended,
        ..plain()
    };
    send_all(&sockets, &pair.client, ended, MESSAGE);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
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

pub fn fast_open_with_data_delivers_it<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(
        &sockets,
        ipv4_loopback(),
        &[TcpListenerSetting::FastOpenQueue(FAST_OPEN_QUEUE)],
    );
    let data = MESSAGE;
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[],
            TcpFastOpen::WithData(data),
        )
        .expect("connect with data");
    let TcpConnectStart::SynchronizeSent {
        fast_open_data_taken,
    } = connected.start
    else {
        panic!("started as {:?}", connected.start);
    };
    let server = accept(&sockets, &listener);
    send_all(
        &sockets,
        &connected.connection,
        plain(),
        &data[fast_open_data_taken.0..],
    );
    assert_eq!(receive_up_to(&sockets, &server, data.len()), data);
    sockets.close(server, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

pub fn fast_open_until_first_send_connects_on_sending<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(
        &sockets,
        ipv4_loopback(),
        &[TcpListenerSetting::FastOpenQueue(FAST_OPEN_QUEUE)],
    );
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[],
            TcpFastOpen::UntilFirstSend,
        )
        .expect("connect");
    assert!(matches!(
        connected.start,
        TcpConnectStart::DeferredUntilFirstSend
            | TcpConnectStart::SynchronizeSent {
                fast_open_data_taken: ByteCount(0)
            }
    ));
    send_all(&sockets, &connected.connection, plain(), MESSAGE);
    let server = accept(&sockets, &listener);
    assert_eq!(receive_up_to(&sockets, &server, MESSAGE.len()), MESSAGE);
    sockets.close(server, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

pub fn delayed_acknowledgement_can_be_sent_in_either_mode<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .configure(
            &pair.server,
            TcpSetting::QuickAcknowledgement(TcpQuickAcknowledgement::Off),
        )
        .expect("delay acknowledgements");
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    sockets
        .send_delayed_acknowledgement(&pair.server, TcpQuickAcknowledgement::On)
        .expect("send it, quick after");
    sockets
        .send_delayed_acknowledgement(&pair.server, TcpQuickAcknowledgement::Off)
        .expect("send it, delaying after");
    assert_carries_bytes(&sockets, &pair);
    close(&sockets, pair);
}

pub fn keep_alive_timer_rearms_when_on<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .configure(&pair.client, TcpSetting::KeepAlive(true))
        .expect("keep alive");
    sockets
        .configure(&pair.client, TcpSetting::KeepAliveIdle(SETTING_TIME))
        .expect("idle time");
    sockets.rearm_keep_alive_timer(&pair.client).expect("rearm");
    assert_carries_bytes(&sockets, &pair);
    close(&sockets, pair);
}

pub fn negative_route_advice_keeps_the_connection<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .give_negative_route_advice(&pair.client)
        .expect("advice");
    assert_carries_bytes(&sockets, &pair);
    close(&sockets, pair);
}

pub fn window_probe_keeps_the_connection<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .send_window_probe(&pair.client)
        .expect("window probe");
    assert_carries_bytes(&sockets, &pair);
    close(&sockets, pair);
}

pub fn timestamp_clock_needs_repair_mode<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let clock = TcpTimestampClock {
        value: TcpTimestamp(u32::from(SETTING_COUNT)),
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

pub fn listener_settings_set_twice_equal_set_once<S: TcpSockets>(sockets: S) {
    let key = FastOpenKey([u8::MAX; FAST_OPEN_KEY_LENGTH]);
    let settings = [
        TcpListenerSetting::Backlog(ListenBacklog(u32::from(SETTING_COUNT))),
        TcpListenerSetting::FastOpenQueue(FAST_OPEN_QUEUE),
        TcpListenerSetting::FastOpenQueue(None),
        TcpListenerSetting::FastOpenKeys(FastOpenKeys {
            primary: key,
            backup: None,
        }),
        TcpListenerSetting::SaveSynchronize(TcpSavedSynchronize::FromNetworkHeader),
        TcpListenerSetting::SaveSynchronize(TcpSavedSynchronize::Off),
        TcpListenerSetting::Socket(TcpSetting::DeferAccept(None)),
        TcpListenerSetting::Socket(TcpSetting::NoDelay(true)),
    ];
    for setting in settings {
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
        assert_eq!(first, Ok(()), "{setting:?}");
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
            push_ignoring_neither_sends_uncorked_data,
            record_end_is_taken,
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
            fast_open_with_data_delivers_it,
            fast_open_until_first_send_connects_on_sending,
            delayed_acknowledgement_can_be_sent_in_either_mode,
            keep_alive_timer_rearms_when_on,
            negative_route_advice_keeps_the_connection,
            window_probe_keeps_the_connection,
            timestamp_clock_needs_repair_mode,
            connection_settings_set_twice_equal_set_once,
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
