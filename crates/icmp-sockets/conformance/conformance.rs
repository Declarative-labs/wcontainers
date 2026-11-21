//! The tests every implementation of wcontainers-icmp-sockets runs, written against its traits alone.
//!
//! Each runs as root in a network namespace of its own, with loopback up and every group allowed ICMP echo
//! sockets, on links, routes, rules and queueing disciplines it builds there with iproute2's `ip` and `tc`. A
//! packet socket of the namespace sees what leaves a link.
#![cfg(target_os = "linux")]

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use core::num::NonZeroU16;
use std::future::Future;
use std::io::Write;
use std::mem::{size_of, zeroed};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::pin::{Pin, pin};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use etherparse::{
    IpDscp, IpEcn, IpNumber, Ipv4HeaderSlice, Ipv4Options, Ipv6FlowLabel, Ipv6HeaderSlice,
    Ipv6RawExtHeaderSlice, icmpv4, icmpv6,
};
use wcontainers_abi::HostError;
use wcontainers_icmp_sockets::{EchoIdentifier, IcmpSockets, IdentifierSharing};
use wcontainers_internet_protocol::{
    AnycastAddress, BroadcastPermission, ByteCount, BytesPerSecond, HopLimit, IcmpCode, IcmpType,
    InterfaceIndex, IpHeaderFields, IpSendFields, IpSocketSetting, IpVersion, Ipv4HeaderFields,
    Ipv4RouteSelectors, Ipv6HeaderFields, Ipv6RouteSelectors, LocalFragmentation,
    MaximumTransmissionUnit, Membership, MobileAddressPreference, MulticastGroup,
    MulticastMembership, NeighbourConfirmation, PacketMark, PacketPriority, PathMtuDiscovery,
    Received, ReceivedPacketFields, Report, RouteScope, SendError, SourceAddressCheck,
    SourceFilter, SourcePreferences, TemporaryAddressPreference, TimestampSource, Timestamping,
    TransmitClock, TransmitEvent, TransmitReportKey, TransmitReportRequest, TransmitStage,
    TransmitTime, TransmitTimeError, TransmitTimeMode, UpperLayer, UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_kernel::signal_information::PollEvents;
use wcontainers_route_resolver_conformance::{
    BATCH, IP, IPV6, PACKETS, number_after, seconds_since_confirmed,
};

#[doc(hidden)]
pub use wcontainers_configurable_conformance as configurable_suite;

/// Moves the calling thread into a new network namespace, which needs `CAP_SYS_ADMIN`, with loopback up and
/// every group in the range Linux lets open ICMP echo sockets.
pub fn enter_network_namespace() {
    // SAFETY: a new network namespace shares no memory or file with the rest of the process.
    unsafe { rustix::thread::unshare_unsafe(rustix::thread::UnshareFlags::NEWNET) }
        .expect("a new network namespace needs root");
    ip(LOOPBACK_UP);
    // This thread's namespace is the one /proc/sys/net shows it; the highest group id Linux takes is one below all ones.
    std::fs::write(PING_GROUP_RANGE, format!("{} {}", u32::MIN, u32::MAX - 1))
        .expect("the ping group range is written");
}

const LOOPBACK_UP: &str = "link set lo up";
pub const PING_GROUP_RANGE: &str = "/proc/sys/net/ipv4/ping_group_range";
pub const ECHO_IGNORE_BROADCASTS: &str = "/proc/sys/net/ipv4/icmp_echo_ignore_broadcasts";
/// The anycast addresses of this thread's network namespace.
pub const ANYCAST_LIST: &str = "/proc/thread-self/net/anycast6";
pub const TRAFFIC_CONTROL: &str = "tc";
const STATISTICS: &str = "-s";
const JSON: &str = "-j";
pub const SHOW_CLASSES: [&str; 5] = [STATISTICS, JSON, "class", "show", "dev"];
pub const SHOW_GROUPS: [&str; 3] = ["maddr", "show", "dev"];
/// The third band of a prio queue, which its default map gives `TC_PRIO_FILLER` alone.
pub const THIRD_BAND: &str = "\"handle\":\"1:3\"";
/// The gateways of the dummy link, as neighbours last confirmed long ago; the link resolves neighbours so that IPv6 confirms them.
pub const STALE_GATEWAYS: &str = "link set ia arp on
neigh add 10.3.0.2 lladdr 02:00:00:00:00:02 dev ia nud stale
neigh add fd03::2 lladdr 02:00:00:00:00:02 dev ia nud stale";
/// `TC_PRIO_FILLER`, which the prio queue's default map puts in its third band, 1:3, where nothing else goes.
pub const FILLER: u32 = 1;
pub const PRIO_QUEUE: &str = "qdisc add dev ia root handle 1: prio";
/// A queue that paces each flow and keeps departure times.
pub const FAIR_QUEUE: &str = "qdisc add dev lo root fq";
pub const SELF_LINK: &str = "lo";
pub const MARK: PacketMark = PacketMark(7);
pub const MARK_PROHIBITED: &str = "rule add fwmark 7 prohibit";
pub const OWNER_PROHIBITED: &str = "rule add uidrange 1000-1000 prohibit";
/// Type 8 code 0, where the flow keeps a destination port.
const ECHO_PROHIBITED: &str = "rule add ipproto icmp dport 2048 prohibit";
/// Type 128 code 0, where the flow keeps a destination port.
const ECHO_PROHIBITED_V6: &str = "rule add ipproto ipv6-icmp dport 32768 prohibit";
/// A code an echo request does not carry.
pub const OTHER_CODE: IcmpCode = IcmpCode(1);
pub const TIME_TO_LIVE: HopLimit = HopLimit(7);
pub const HOP_LIMIT: HopLimit = HopLimit(9);
pub const FLOW_LABEL: u32 = 0x5a5a5;
pub const REPORT_KEY: TransmitReportKey = TransmitReportKey(0x5eed);
/// An echo message longer than the links of the checks that fragment.
const LONG_MESSAGE: usize = 1400;
/// The length of each paced message, and the rate a send buffer fills at.
const PACED_MESSAGE: usize = 1100;
const SLOW_RATE: BytesPerSecond = BytesPerSecond(1000);
/// How many sends a check tries before it expects the send buffer to be full.
const ATTEMPTS: usize = 64;
/// Where an echo message keeps its identifier, after its type, code and checksum.
const IDENTIFIER_BYTES: core::ops::Range<usize> = 4..6;
/// Where an ICMP message keeps its checksum.
pub const CHECKSUM_BYTES: core::ops::Range<usize> = 2..4;
pub const ROOT: UserId = UserId(0);
pub const OTHER_USER: UserId = UserId(1000);
pub const DUMMY: &str = "ia";
/// A dummy link with an IPv4 and an IPv6 prefix, a gateway on each and routes through it to far networks.
pub const NETWORK: &str = "link add ia type dummy
link set ia multicast on
link set ia up
address add 10.3.0.1/24 broadcast 10.3.0.255 dev ia
address add fd03::1/64 dev ia nodad
route add 10.9.0.0/16 via 10.3.0.2
route add fd09::/64 via fd03::2";
/// A second dummy link and routes to [`NOWHERE`] and [`NOWHERE_V6`] on both links, the first preferred.
pub const TWO_ROUTES: &str = "link add ib type dummy
link set ib up
route add 10.8.0.0/16 dev ia metric 1
route add 10.8.0.0/16 dev ib metric 2
route add fd08::/64 dev ia metric 1
route add fd08::/64 dev ib metric 2";
pub const SECOND: &str = "ib";
pub const ON_LINK: Ipv4Addr = Ipv4Addr::new(10, 3, 0, 7);
pub const ON_LINK_V6: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 7);
pub const LOCAL: Ipv4Addr = Ipv4Addr::new(10, 3, 0, 1);
pub const LOCAL_V6: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 1);
pub const GATEWAY: Ipv4Addr = Ipv4Addr::new(10, 3, 0, 2);
pub const GATEWAY_V6: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 2);
pub const BROADCAST: Ipv4Addr = Ipv4Addr::new(10, 3, 0, 255);
pub const FAR: Ipv4Addr = Ipv4Addr::new(10, 9, 1, 1);
pub const FAR_V6: Ipv6Addr = Ipv6Addr::new(0xfd09, 0, 0, 0, 0, 0, 0, 1);
pub const NOWHERE: Ipv4Addr = Ipv4Addr::new(10, 8, 0, 1);
pub const NOWHERE_V6: Ipv6Addr = Ipv6Addr::new(0xfd08, 0, 0, 0, 0, 0, 0, 1);
pub const PAYLOAD: &[u8] = b"wcontainers echo";
const IDENTIFIER: EchoIdentifier = EchoIdentifier(NonZeroU16::new(0x4a21).unwrap());
/// Another identifier, for a socket of the other version: Linux keeps one table of identifiers for both, and a
/// closed socket's identifier stays taken while a child that another test thread forks still holds its descriptor.
const IDENTIFIER_V6: EchoIdentifier = EchoIdentifier(NonZeroU16::new(0x4a22).unwrap());

fn identifier(version: IpVersion) -> EchoIdentifier {
    match version {
        IpVersion::V4 => IDENTIFIER,
        IpVersion::V6 => IDENTIFIER_V6,
    }
}
const SEQUENCE: u16 = 0x0102;
/// How long a check waits for a packet that should come.
pub const PATIENCE: Duration = Duration::from_secs(2);
/// How long a check waits to be sure that a packet does not come.
pub const QUIET: Duration = Duration::from_millis(200);
/// The bytes of an ICMP header, as Linux reads one from an echo socket's message.
const ICMP_HEADER: usize = 8;
static NO_OPTIONS: std::sync::LazyLock<Ipv4Options> = std::sync::LazyLock::new(Ipv4Options::new);

pub fn ip(commands: &str) {
    batch(IP, &[], commands);
}

pub fn ip6(commands: &str) {
    batch(IP, IPV6, commands);
}

pub fn tc(commands: &str) {
    batch(TRAFFIC_CONTROL, &[], commands);
}

/// Runs each line of `commands` as one command of the iproute2 tool, with these options.
fn batch(tool: &str, options: &[&str], commands: &str) {
    let mut child = Command::new(tool)
        .args(options)
        .args(BATCH)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("{tool} runs: {error}"));
    child
        .stdin
        .take()
        .expect("the tool's input")
        .write_all(commands.as_bytes())
        .expect("the tool reads its commands");
    assert!(
        child.wait().expect("the tool ends").success(),
        "{tool}: {commands}"
    );
}

pub fn output(tool: &str, arguments: &[&str]) -> String {
    let output = Command::new(tool)
        .args(arguments)
        .output()
        .expect("the tool runs");
    String::from_utf8(output.stdout).expect("the tool writes text")
}

pub fn write_setting(path: &str, value: &str) {
    std::fs::write(path, value).unwrap_or_else(|error| panic!("{path}: {error}"));
}

pub fn index(name: &str) -> InterfaceIndex {
    let socket = rustix::net::socket(
        rustix::net::AddressFamily::INET,
        rustix::net::SocketType::DGRAM,
        None,
    )
    .expect("a socket opens");
    InterfaceIndex(rustix::net::netdevice::name_to_index(&socket, name).expect(name))
}

/// An echo request of the version with the sequence number and payload; Linux writes the identifier and checksum.
pub fn echo(version: IpVersion, payload: &[u8]) -> Vec<u8> {
    let kind = match version {
        IpVersion::V4 => icmpv4::TYPE_ECHO_REQUEST,
        IpVersion::V6 => icmpv6::TYPE_ECHO_REQUEST,
    };
    let mut message = vec![kind, u8::MIN, u8::MIN, u8::MIN, u8::MIN, u8::MIN];
    message.extend_from_slice(&SEQUENCE.to_be_bytes());
    message.extend_from_slice(payload);
    message
}

fn protocol(version: IpVersion) -> IpNumber {
    match version {
        IpVersion::V4 => IpNumber::ICMP,
        IpVersion::V6 => IpNumber::IPV6_ICMP,
    }
}

/// The upper layer a message is routed by: its protocol, type and code.
fn upper(version: IpVersion, message: &[u8]) -> UpperLayer {
    UpperLayer {
        protocol: protocol(version),
        selector: Some(UpperLayerSelector::Icmp {
            message_type: IcmpType(message[0]),
            code: IcmpCode(message[1]),
        }),
    }
}

fn v4_route(destination: Ipv4Addr, message: &[u8]) -> Ipv4RouteSelectors<'static> {
    Ipv4RouteSelectors {
        destination,
        source: None,
        source_address_check: SourceAddressCheck::Required,
        interface: None,
        dscp: IpDscp::ZERO,
        ecn: IpEcn::NotEct,
        mark: PacketMark(u32::MIN),
        scope: RouteScope::Universe,
        broadcast: BroadcastPermission::Refused,
        options: &NO_OPTIONS,
        upper_layer: upper(IpVersion::V4, message),
    }
}

fn v6_route(destination: Ipv6Addr, message: &[u8]) -> Ipv6RouteSelectors<'static> {
    Ipv6RouteSelectors {
        destination,
        source: None,
        source_address_check: SourceAddressCheck::Required,
        interface: None,
        dscp: IpDscp::ZERO,
        ecn: IpEcn::NotEct,
        flow_label: Ipv6FlowLabel::ZERO,
        mark: PacketMark(u32::MIN),
        source_preferences: SourcePreferences {
            temporary: TemporaryAddressPreference::SystemDefault,
            mobile: MobileAddressPreference::Home,
        },
        routing_header: None,
        upper_layer: upper(IpVersion::V6, message),
    }
}

fn v4_header(route: Ipv4RouteSelectors<'_>) -> Ipv4HeaderFields<'_> {
    Ipv4HeaderFields {
        route,
        time_to_live: None,
        path_mtu_discovery: PathMtuDiscovery::Want,
    }
}

fn v6_header(route: Ipv6RouteSelectors<'_>) -> Ipv6HeaderFields<'_> {
    Ipv6HeaderFields {
        route,
        hop_limit: None,
        path_mtu_discovery: PathMtuDiscovery::Want,
        local_fragmentation: LocalFragmentation::Allowed,
        fragment_mtu: None,
        hop_by_hop_options: None,
        routing_destination_options: None,
        destination_options: None,
    }
}

/// The plain header of a message to the address.
pub fn header_to(destination: IpAddr, message: &[u8]) -> IpHeaderFields<'static> {
    match destination {
        IpAddr::V4(address) => IpHeaderFields::V4(v4_header(v4_route(address, message))),
        IpAddr::V6(address) => IpHeaderFields::V6(v6_header(v6_route(address, message))),
    }
}

pub fn no_reports() -> TransmitReportRequest {
    TransmitReportRequest {
        key: TransmitReportKey(u32::MIN),
        scheduled: Report::NotRequested,
        software: Report::NotRequested,
        hardware: Report::NotRequested,
        acknowledgement: Report::NotRequested,
        completion: Report::NotRequested,
        wireless_status: Report::NotRequested,
        packet: Report::NotRequested,
    }
}

pub fn plain_fields() -> IpSendFields {
    IpSendFields {
        neighbour: NeighbourConfirmation::Unconfirmed,
        priority: PacketPriority(u32::MIN),
        transmit_time: None,
        transmit_reports: no_reports(),
    }
}

pub fn version_of(address: IpAddr) -> IpVersion {
    match address {
        IpAddr::V4(_) => IpVersion::V4,
        IpAddr::V6(_) => IpVersion::V6,
    }
}

fn opened<S: IcmpSockets>(sockets: &S, version: IpVersion) -> S::Socket {
    sockets.open(version, ROOT).expect("an echo socket opens")
}

fn set<S: IcmpSockets>(sockets: &S, socket: &S::Socket, setting: IpSocketSetting<'_>) {
    sockets
        .configure(socket, setting)
        .unwrap_or_else(|error| panic!("{setting:?}: {error:?}"));
}

/// Sends an echo request with the payload to the address, plainly.
fn ping<S: IcmpSockets>(
    sockets: &S,
    socket: &S::Socket,
    to: IpAddr,
    payload: &[u8],
) -> Result<(), SendError> {
    let message = echo(version_of(to), payload);
    sockets.send_to(socket, header_to(to, &message), plain_fields(), &message)
}

pub fn buffer() -> Vec<u8> {
    vec![u8::MIN; usize::from(u16::MAX)]
}

/// Waits for what the socket receives next.
fn receive<S: IcmpSockets>(sockets: &S, socket: &S::Socket, into: &mut [u8]) -> Received {
    let start = Instant::now();
    loop {
        match sockets.receive(socket, into) {
            Err(HostError::WouldBlock) if start.elapsed() < PATIENCE => {
                std::thread::sleep(Duration::from_millis(1));
            }
            other => return other.expect("something arrives"),
        }
    }
}

fn receive_packet<S: IcmpSockets>(
    sockets: &S,
    socket: &S::Socket,
    into: &mut [u8],
) -> ReceivedPacketFields {
    match receive(sockets, socket, into) {
        Received::Packet(fields) => fields,
        other => panic!("a packet, not {other:?}"),
    }
}

/// Answers whether nothing arrives at the socket for a while.
fn nothing_arrives<S: IcmpSockets>(sockets: &S, socket: &S::Socket) -> bool {
    std::thread::sleep(QUIET);
    sockets.receive(socket, &mut buffer()) == Err(HostError::WouldBlock)
}

/// An IP packet's version, source, destination, upper-layer protocol and the bytes after its headers.
pub struct Seen<'a> {
    pub source: IpAddr,
    pub destination: IpAddr,
    pub protocol: IpNumber,
    pub payload: &'a [u8],
    pub extensions: Vec<(IpNumber, &'a [u8])>,
}

pub fn seen(packet: &[u8]) -> Seen<'_> {
    if let Ok(header) = Ipv4HeaderSlice::from_slice(packet) {
        return Seen {
            source: IpAddr::V4(header.source_addr()),
            destination: IpAddr::V4(header.destination_addr()),
            protocol: header.protocol(),
            payload: &packet[header.slice().len()..],
            extensions: Vec::new(),
        };
    }
    let header = Ipv6HeaderSlice::from_slice(packet).expect("an IPv4 or IPv6 packet");
    let mut next = header.next_header();
    let mut rest = &packet[header.slice().len()..];
    let mut extensions = Vec::new();
    while matches!(
        next,
        IpNumber::IPV6_HEADER_HOP_BY_HOP
            | IpNumber::IPV6_ROUTE_HEADER
            | IpNumber::IPV6_DESTINATION_OPTIONS
            | IpNumber::IPV6_FRAGMENTATION_HEADER
    ) {
        let length = if next == IpNumber::IPV6_FRAGMENTATION_HEADER {
            etherparse::Ipv6FragmentHeader::LEN
        } else {
            Ipv6RawExtHeaderSlice::from_slice(rest)
                .expect("an extension header")
                .slice()
                .len()
        };
        extensions.push((next, &rest[..length]));
        next = IpNumber(rest[0]);
        rest = &rest[length..];
    }
    Seen {
        source: IpAddr::V6(header.source_addr()),
        destination: IpAddr::V6(header.destination_addr()),
        protocol: next,
        payload: rest,
        extensions,
    }
}

/// A packet socket that sees every packet leaving one link of the namespace, from the moment it is made.
pub struct Capture(OwnedFd);

impl Capture {
    pub fn on(link: &str) -> Self {
        let all = (libc::ETH_P_ALL as u16).to_be();
        // SAFETY: socket makes a new descriptor, owned below.
        let fd = unsafe {
            libc::socket(
                libc::AF_PACKET,
                libc::SOCK_DGRAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                i32::from(all),
            )
        };
        assert!(!fd.is_negative(), "a packet socket opens");
        // SAFETY: the descriptor was just made and nothing else owns it.
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        // SAFETY: sockaddr_ll is plain data.
        let mut address: libc::sockaddr_ll = unsafe { zeroed() };
        address.sll_family = libc::AF_PACKET as u16;
        address.sll_protocol = all;
        address.sll_ifindex = index(link).0 as i32;
        // SAFETY: the address is a sockaddr_ll of its own size.
        let bound = unsafe {
            libc::bind(
                fd.as_raw_fd(),
                (&address as *const libc::sockaddr_ll).cast(),
                size_of::<libc::sockaddr_ll>() as u32,
            )
        };
        assert!(!bound.is_negative(), "the packet socket binds to {link}");
        Self(fd)
    }

    /// The next packet leaving the link that `wanted` takes, if one leaves within the time given.
    pub fn within(&self, time: Duration, wanted: impl Fn(&[u8]) -> bool) -> Option<Vec<u8>> {
        let start = Instant::now();
        let mut packet = buffer();
        while start.elapsed() < time {
            // SAFETY: sockaddr_ll is plain data.
            let mut address: libc::sockaddr_ll = unsafe { zeroed() };
            let mut length = size_of::<libc::sockaddr_ll>() as u32;
            // SAFETY: the buffers are writable for the lengths given.
            let read = unsafe {
                libc::recvfrom(
                    self.0.as_raw_fd(),
                    packet.as_mut_ptr().cast(),
                    packet.len(),
                    libc::MSG_DONTWAIT,
                    (&mut address as *mut libc::sockaddr_ll).cast(),
                    &mut length,
                )
            };
            if read.is_negative() {
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            let bytes = &packet[..read as usize];
            if address.sll_pkttype == libc::PACKET_OUTGOING && wanted(bytes) {
                return Some(bytes.to_vec());
            }
        }
        None
    }

    pub fn next(&self, wanted: impl Fn(&[u8]) -> bool) -> Vec<u8> {
        self.within(PATIENCE, wanted)
            .expect("the packet leaves the link")
    }

    pub fn echo_request(&self) -> Vec<u8> {
        self.next(is_echo_request)
    }
}

fn is_echo_request(packet: &[u8]) -> bool {
    let seen = seen(packet);
    matches!(
        (seen.protocol, seen.payload.first()),
        (IpNumber::ICMP, Some(&icmpv4::TYPE_ECHO_REQUEST))
            | (IpNumber::IPV6_ICMP, Some(&icmpv6::TYPE_ECHO_REQUEST))
    )
}

pub fn ipv4(packet: &[u8]) -> Ipv4HeaderSlice<'_> {
    Ipv4HeaderSlice::from_slice(packet).expect("an IPv4 packet")
}

pub fn ipv6(packet: &[u8]) -> Ipv6HeaderSlice<'_> {
    Ipv6HeaderSlice::from_slice(packet).expect("an IPv6 packet")
}

/// The echo identifier of an ICMP echo message.
fn identifier_of(message: &[u8]) -> u16 {
    u16::from_be_bytes(
        message[IDENTIFIER_BYTES]
            .try_into()
            .expect("an echo message's identifier"),
    )
}

/// Accepts an honest refusal of what a host may be unable to send, and checks anything else.
fn unless_refused<T>(result: Result<T, SendError>, check: impl FnOnce(Result<T, SendError>)) {
    if !matches!(result, Err(SendError::Host(HostError::NotSupported))) {
        check(result);
    }
}

pub fn checks_out(message: &[u8], seen: &Seen<'_>) -> bool {
    match (seen.source, seen.destination) {
        (IpAddr::V6(source), IpAddr::V6(destination)) => {
            etherparse::checksum::Sum16BitWords::new()
                .add_16bytes(source.octets())
                .add_16bytes(destination.octets())
                .add_4bytes((message.len() as u32).to_be_bytes())
                .add_2bytes([u8::MIN, IpNumber::IPV6_ICMP.0])
                .add_slice(message)
                .ones_complement()
                == u16::MIN
        }
        _ => {
            etherparse::checksum::Sum16BitWords::new()
                .add_slice(message)
                .ones_complement()
                == u16::MIN
        }
    }
}

/// An echo request to a local address gets its reply, written whole from its IP header as it was on the wire, with the identifier bound.
fn reply_arrives_whole<S: IcmpSockets>(sockets: S, to: IpAddr) {
    let version = version_of(to);
    let socket = opened(&sockets, version);
    sockets
        .bind_identifier(&socket, IDENTIFIER, IdentifierSharing::Exclusive)
        .expect("the identifier binds");
    let wire = Capture::on(SELF_LINK);
    ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
    let mut into = buffer();
    let fields = receive_packet(&sockets, &socket, &mut into);
    let packet = &into[..fields.length];
    let seen = seen(packet);
    assert_eq!(
        (seen.source, seen.destination, seen.protocol),
        (to, to, protocol(version))
    );
    let reply = match version {
        IpVersion::V4 => icmpv4::TYPE_ECHO_REPLY,
        IpVersion::V6 => icmpv6::TYPE_ECHO_REPLY,
    };
    let on_the_wire = wire.next(|packet| {
        let seen = crate::seen(packet);
        seen.protocol == protocol(version) && seen.payload.first() == Some(&reply)
    });
    assert_eq!(packet.len(), on_the_wire.len());
    let headers = packet.len() - seen.payload.len();
    match version {
        IpVersion::V4 => {
            // Every field but the identification, the fragment flags and offset, and the checksum over them.
            const NOT_COMPARED: [core::ops::Range<usize>; 2] = [4..8, 10..12];
            let compared = |header: &[u8]| {
                let mut header = header[..headers].to_vec();
                for range in NOT_COMPARED {
                    header[range].fill(u8::MIN);
                }
                header
            };
            assert_eq!(compared(packet), compared(&on_the_wire));
            let given = ipv4(packet);
            assert_eq!(
                given.header_checksum(),
                given.to_header().calc_header_checksum()
            );
        }
        IpVersion::V6 => assert_eq!(&packet[..headers], &on_the_wire[..headers]),
    }
    let message = seen.payload;
    assert_eq!((message[0], message[1]), (reply, u8::MIN));
    assert_eq!(identifier_of(message), IDENTIFIER.0.get());
    assert_eq!(
        &message[ICMP_HEADER - 2..ICMP_HEADER],
        SEQUENCE.to_be_bytes()
    );
    assert_eq!(&message[ICMP_HEADER..], PAYLOAD);
    assert!(checks_out(message, &seen), "{message:?}");
    let length = match version {
        IpVersion::V4 => usize::from(ipv4(packet).total_len()),
        IpVersion::V6 => usize::from(ipv6(packet).payload_length()) + etherparse::Ipv6Header::LEN,
    };
    assert_eq!(length, fields.length);
    // The preferred source of a reply to the packet, which only IPv4 has, if it is told at all.
    let preferred = match to {
        IpAddr::V4(address) => Some(address),
        IpAddr::V6(_) => None,
    };
    assert!(
        fields.specific_destination.is_none() || fields.specific_destination == preferred,
        "{:?}",
        fields.specific_destination
    );
    assert_eq!(fields.interface, index(SELF_LINK));
}

/// An IPv4 echo request to loopback gets its reply, whole from its IPv4 header.
pub fn echo_reply_arrives_whole_over_ipv4<S: IcmpSockets>(sockets: S) {
    reply_arrives_whole(sockets, IpAddr::V4(Ipv4Addr::LOCALHOST));
}

/// An IPv6 echo request to loopback gets its reply, whole from its IPv6 header.
pub fn echo_reply_arrives_whole_over_ipv6<S: IcmpSockets>(sockets: S) {
    reply_arrives_whole(sockets, IpAddr::V6(Ipv6Addr::LOCALHOST));
}

/// The request that leaves carries the bound identifier and the message's sequence number and payload.
pub fn the_request_carries_the_bound_identifier<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let socket = opened(&sockets, version_of(to));
        let identifier = identifier(version_of(to));
        sockets
            .bind_identifier(&socket, identifier, IdentifierSharing::Exclusive)
            .expect("the identifier binds");
        let capture = Capture::on(DUMMY);
        ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
        let request = capture.echo_request();
        let seen = seen(&request);
        assert_eq!(seen.destination, to);
        assert_eq!(identifier_of(seen.payload), identifier.0.get());
        assert_eq!(&seen.payload[ICMP_HEADER..], PAYLOAD);
        assert!(checks_out(seen.payload, &seen));
    }
}

/// Two sockets take one identifier only when both share it.
pub fn identifiers_are_shared_only_when_both_ask<S: IcmpSockets>(sockets: S) {
    for version in [IpVersion::V4, IpVersion::V6] {
        let first = opened(&sockets, version);
        let second = opened(&sockets, version);
        let identifier = identifier(version);
        sockets
            .bind_identifier(&first, identifier, IdentifierSharing::Exclusive)
            .expect("the identifier binds");
        assert_eq!(
            sockets.bind_identifier(&second, identifier, IdentifierSharing::Shared),
            Err(HostError::Errno(libc::EADDRINUSE))
        );
        let third = opened(&sockets, version);
        let fourth = opened(&sockets, version);
        let shared = EchoIdentifier(identifier.0.saturating_add(u16::from(u8::MAX)));
        for socket in [&third, &fourth] {
            sockets
                .bind_identifier(socket, shared, IdentifierSharing::Shared)
                .expect("a shared identifier binds twice");
        }
    }
}

/// Closing a socket frees its identifier, at once or once no child forked meanwhile holds its descriptor.
pub fn close_frees_the_identifier<S: IcmpSockets>(sockets: S) {
    for version in [IpVersion::V4, IpVersion::V6] {
        let first = opened(&sockets, version);
        let identifier = identifier(version);
        sockets
            .bind_identifier(&first, identifier, IdentifierSharing::Exclusive)
            .expect("the identifier binds");
        let second = opened(&sockets, version);
        assert_eq!(
            sockets.bind_identifier(&second, identifier, IdentifierSharing::Exclusive),
            Err(HostError::Errno(libc::EADDRINUSE))
        );
        sockets.close(first);
        let start = Instant::now();
        let rebound = loop {
            match sockets.bind_identifier(&second, identifier, IdentifierSharing::Exclusive) {
                Err(HostError::Errno(libc::EADDRINUSE)) if start.elapsed() < PATIENCE => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                other => break other,
            }
        };
        assert_eq!(rebound, Ok(()), "{version:?}");
    }
}

pub fn receive_with_nothing_queued_would_block<S: IcmpSockets>(sockets: S) {
    for version in [IpVersion::V4, IpVersion::V6] {
        let socket = opened(&sockets, version);
        assert_eq!(
            sockets.receive(&socket, &mut buffer()),
            Err(HostError::WouldBlock)
        );
    }
}

/// Where an IPv4 header keeps its total length.
const IPV4_TOTAL_LENGTH: core::ops::Range<usize> = 2..4;
/// Where an IPv6 header keeps its payload length.
const IPV6_PAYLOAD_LENGTH: core::ops::Range<usize> = 4..6;

/// The whole length of a packet that the start of its header tells.
fn told_length(version: IpVersion, start: &[u8]) -> usize {
    let field = |range: core::ops::Range<usize>| {
        usize::from(u16::from_be_bytes(
            start[range].try_into().expect("a length of two bytes"),
        ))
    };
    match version {
        IpVersion::V4 => field(IPV4_TOTAL_LENGTH),
        IpVersion::V6 => field(IPV6_PAYLOAD_LENGTH) + etherparse::Ipv6Header::LEN,
    }
}

/// A buffer that cuts the reply in its IP header or in its ICMP message takes its start, whose header and the
/// received length both tell the whole length.
pub fn a_short_buffer_keeps_the_whole_length<S: IcmpSockets>(sockets: S) {
    for (to, header, reply) in [
        (
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            etherparse::Ipv4Header::MIN_LEN,
            icmpv4::TYPE_ECHO_REPLY,
        ),
        (
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            etherparse::Ipv6Header::LEN,
            icmpv6::TYPE_ECHO_REPLY,
        ),
    ] {
        let socket = opened(&sockets, version_of(to));
        // Longer than the header, so that the message is cut by a buffer that holds the header and more.
        let payload = vec![u8::MAX; header + PAYLOAD.len()];
        let whole = header + ICMP_HEADER + payload.len();
        for short in [header / 2, header + ICMP_HEADER] {
            ping(&sockets, &socket, to, &payload).unwrap();
            let mut into = vec![u8::MIN; short];
            let fields = receive_packet(&sockets, &socket, &mut into);
            assert_eq!(fields.length, whole, "{to} into {short}");
            assert_eq!(
                told_length(version_of(to), &into),
                whole,
                "{to} into {short}"
            );
            if short > header {
                assert_eq!(into[header], reply, "{to}");
            }
        }
    }
}

/// The TTL, DSCP, ECN, options and source of an IPv4 send are in the header that leaves.
pub fn ipv4_header_fields_go_out<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    /// A router alert option, then the end of the options.
    const ROUTER_ALERT: [u8; 4] = [148, 4, 0, 0];
    let options = Ipv4Options::from(ROUTER_ALERT);
    let socket = opened(&sockets, IpVersion::V4);
    let message = echo(IpVersion::V4, PAYLOAD);
    let route = Ipv4RouteSelectors {
        source: Some(LOCAL),
        dscp: IpDscp::try_new(IpDscp::MAX_U8).unwrap(),
        ecn: IpEcn::Ect0,
        options: &options,
        ..v4_route(ON_LINK, &message)
    };
    let header = Ipv4HeaderFields {
        time_to_live: Some(TIME_TO_LIVE),
        ..v4_header(route)
    };
    let capture = Capture::on(DUMMY);
    sockets
        .send_to(
            &socket,
            IpHeaderFields::V4(header),
            plain_fields(),
            &message,
        )
        .expect("the request is sent");
    let request = capture.echo_request();
    let header = ipv4(&request);
    assert_eq!(header.ttl(), TIME_TO_LIVE.0);
    assert_eq!((header.dcp(), header.ecn()), (route.dscp, route.ecn));
    assert_eq!(header.options(), ROUTER_ALERT);
    assert_eq!(header.source_addr(), LOCAL);
    let socket = opened(&sockets, IpVersion::V4);
    ping(&sockets, &socket, IpAddr::V4(ON_LINK), PAYLOAD).unwrap();
    let plain = capture.echo_request();
    assert!(ipv4(&plain).options().is_empty());
    assert_eq!(ipv4(&plain).ttl(), default_ttl());
}

/// The namespace's default TTL, as a socket that never set its own reads it.
fn default_ttl() -> u8 {
    let socket = rustix::net::socket(
        rustix::net::AddressFamily::INET,
        rustix::net::SocketType::DGRAM,
        None,
    )
    .expect("a socket opens");
    rustix::net::sockopt::ip_ttl(&socket).expect("the default TTL") as u8
}

/// The hop limit, DSCP, ECN and flow label of an IPv6 send are in the header that leaves.
pub fn ipv6_header_fields_go_out<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    let socket = opened(&sockets, IpVersion::V6);
    let message = echo(IpVersion::V6, PAYLOAD);
    let route = Ipv6RouteSelectors {
        dscp: IpDscp::try_new(IpDscp::MAX_U8).unwrap(),
        ecn: IpEcn::Ect1,
        flow_label: Ipv6FlowLabel::try_new(FLOW_LABEL).unwrap(),
        ..v6_route(ON_LINK_V6, &message)
    };
    let header = Ipv6HeaderFields {
        hop_limit: Some(HOP_LIMIT),
        ..v6_header(route.clone())
    };
    let capture = Capture::on(DUMMY);
    sockets
        .send_to(
            &socket,
            IpHeaderFields::V6(header),
            plain_fields(),
            &message,
        )
        .expect("the request is sent");
    let request = capture.echo_request();
    let header = ipv6(&request);
    assert_eq!(header.hop_limit(), HOP_LIMIT.0);
    assert_eq!((header.dscp(), header.ecn()), (route.dscp, route.ecn));
    assert_eq!(header.flow_label(), route.flow_label);
}

/// The last segment of a segment routing header the checks send, beyond the link.
pub const FAR_V6_SEGMENT: Ipv6Addr = Ipv6Addr::new(0xfd09, 0, 0, 0, 0, 0, 0, 9);
/// An IPv6 extension header of eight bytes: next header, length, then padding (PadN of four).
pub const PADDING_HEADER: [u8; 8] = [0, 0, 1, 4, 0, 0, 0, 0];

/// A routing header's length in eight-byte units after its first eight, for two segments.
pub const TWO_SEGMENTS: u8 = 4;

/// A segment routing header of two segments, [`FAR_V6_SEGMENT`] last and [`ON_LINK_V6`] next.
pub fn segment_routing_header() -> Vec<u8> {
    let mut routing = vec![
        u8::MIN,
        TWO_SEGMENTS,
        linux_raw_sys::net::IPV6_SRCRT_TYPE_4 as u8,
        1,
        1,
        u8::MIN,
        u8::MIN,
        u8::MIN,
    ];
    routing.extend_from_slice(&FAR_V6_SEGMENT.octets());
    routing.extend_from_slice(&ON_LINK_V6.octets());
    routing
}

/// The kinds of the extension headers a packet carries, in order.
pub fn extension_kinds(packet: &[u8]) -> Vec<IpNumber> {
    seen(packet)
        .extensions
        .iter()
        .map(|(kind, _)| *kind)
        .collect()
}

/// IPv6 extension headers and a routing header go out with the request, or are refused.
pub fn ipv6_extension_headers_go_out<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    let padding = Ipv6RawExtHeaderSlice::from_slice(&PADDING_HEADER).unwrap();
    let routing = segment_routing_header();
    let routing = Ipv6RawExtHeaderSlice::from_slice(&routing).unwrap();
    let message = echo(IpVersion::V6, PAYLOAD);
    let socket = opened(&sockets, IpVersion::V6);
    let capture = Capture::on(DUMMY);
    let header = Ipv6HeaderFields {
        hop_by_hop_options: Some(padding.clone()),
        routing_destination_options: Some(padding.clone()),
        destination_options: Some(padding),
        ..v6_header(Ipv6RouteSelectors {
            routing_header: Some(routing),
            ..v6_route(FAR_V6_SEGMENT, &message)
        })
    };
    let sent = sockets.send_to(
        &socket,
        IpHeaderFields::V6(header),
        plain_fields(),
        &message,
    );
    unless_refused(sent, |sent| {
        sent.expect("the request is sent");
        let request = capture.echo_request();
        assert_eq!(ipv6(&request).destination_addr(), ON_LINK_V6);
        assert_eq!(
            extension_kinds(&request),
            [
                IpNumber::IPV6_HEADER_HOP_BY_HOP,
                IpNumber::IPV6_DESTINATION_OPTIONS,
                IpNumber::IPV6_ROUTE_HEADER,
                IpNumber::IPV6_DESTINATION_OPTIONS
            ]
        );
    });
    let socket = opened(&sockets, IpVersion::V6);
    ping(&sockets, &socket, IpAddr::V6(ON_LINK_V6), PAYLOAD).unwrap();
    assert!(extension_kinds(&capture.echo_request()).is_empty());
}

/// An IPv6 request leaves from the local source address its send gives, or the source is refused; of two
/// addresses on the link, either is taken when asked for, whichever the host would choose by itself.
pub fn an_ipv6_source_goes_out<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    const SECOND_ADDRESS: &str = "address add fd03::5/64 dev ia nodad";
    ip(SECOND_ADDRESS);
    const SOURCE: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 5);
    let message = echo(IpVersion::V6, PAYLOAD);
    let socket = opened(&sockets, IpVersion::V6);
    let capture = Capture::on(DUMMY);
    for source in [SOURCE, LOCAL_V6] {
        let route = Ipv6RouteSelectors {
            source: Some(source),
            ..v6_route(ON_LINK_V6, &message)
        };
        let sent = sockets.send_to(
            &socket,
            IpHeaderFields::V6(v6_header(route)),
            plain_fields(),
            &message,
        );
        unless_refused(sent, |sent| {
            sent.expect("the request is sent");
            assert_eq!(seen(&capture.echo_request()).source, IpAddr::V6(source));
        });
    }
}

/// Each path MTU discovery mode sends, with the don't-fragment bit on IPv4 only for the modes that set it.
pub fn every_path_mtu_discovery_mode_sends<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    let capture = Capture::on(DUMMY);
    for (mode, dont_fragment) in [
        (PathMtuDiscovery::Dont, false),
        (PathMtuDiscovery::Want, true),
        (PathMtuDiscovery::Do, true),
        (PathMtuDiscovery::Probe, true),
        (PathMtuDiscovery::Interface, false),
        (PathMtuDiscovery::Omit, false),
    ] {
        let socket = opened(&sockets, IpVersion::V4);
        let message = echo(IpVersion::V4, PAYLOAD);
        let header = Ipv4HeaderFields {
            path_mtu_discovery: mode,
            ..v4_header(v4_route(ON_LINK, &message))
        };
        sockets
            .send_to(
                &socket,
                IpHeaderFields::V4(header),
                plain_fields(),
                &message,
            )
            .expect("the request is sent");
        assert_eq!(
            ipv4(&capture.echo_request()).dont_fragment(),
            dont_fragment,
            "{mode:?}"
        );
        let socket = opened(&sockets, IpVersion::V6);
        let message = echo(IpVersion::V6, PAYLOAD);
        let header = Ipv6HeaderFields {
            path_mtu_discovery: mode,
            ..v6_header(v6_route(ON_LINK_V6, &message))
        };
        sockets
            .send_to(
                &socket,
                IpHeaderFields::V6(header),
                plain_fields(),
                &message,
            )
            .expect("the request is sent");
        capture.echo_request();
    }
}

/// A fragment MTU below the dummy link's MTU, which a long message does not fit.
const FRAGMENT_MTU: u32 = 1320;

/// A message longer than the link or the fragment MTU is too long where fragmenting is forbidden, and is
/// fragmented to the link where it is allowed.
pub fn a_message_over_the_mtu_is_too_long_or_fragmented<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    /// The link's MTU at the end, which the commands below set.
    const LINK_MTU: u32 = 1360;
    const SMALL_LINK: &str = "link set ia mtu 1360";
    let long = vec![u8::MAX; LONG_MESSAGE];
    let too_long = |mtu| {
        Err(SendError::MessageTooLong {
            maximum_transmission_unit: MaximumTransmissionUnit(mtu),
        })
    };
    let socket = opened(&sockets, IpVersion::V6);
    let message = echo(IpVersion::V6, &long);
    let forbidden = Ipv6HeaderFields {
        local_fragmentation: LocalFragmentation::Forbidden,
        fragment_mtu: Some(MaximumTransmissionUnit(FRAGMENT_MTU)),
        ..v6_header(v6_route(ON_LINK_V6, &message))
    };
    assert_eq!(
        sockets.send_to(
            &socket,
            IpHeaderFields::V6(forbidden.clone()),
            plain_fields(),
            &message
        ),
        too_long(FRAGMENT_MTU)
    );
    ip(SMALL_LINK);
    let forbidden = Ipv6HeaderFields {
        fragment_mtu: None,
        ..forbidden
    };
    assert_eq!(
        sockets.send_to(
            &socket,
            IpHeaderFields::V6(forbidden),
            plain_fields(),
            &message
        ),
        too_long(LINK_MTU)
    );
    let v4_socket = opened(&sockets, IpVersion::V4);
    let v4_message = echo(IpVersion::V4, &long);
    let header = |mode| {
        IpHeaderFields::V4(Ipv4HeaderFields {
            path_mtu_discovery: mode,
            ..v4_header(v4_route(ON_LINK, &v4_message))
        })
    };
    assert_eq!(
        sockets.send_to(
            &v4_socket,
            header(PathMtuDiscovery::Do),
            plain_fields(),
            &v4_message
        ),
        too_long(LINK_MTU)
    );
    let capture = Capture::on(DUMMY);
    sockets
        .send_to(
            &v4_socket,
            header(PathMtuDiscovery::Dont),
            plain_fields(),
            &v4_message,
        )
        .expect("the message is sent in fragments");
    let fragment = capture.next(|packet| {
        Ipv4HeaderSlice::from_slice(packet).is_ok_and(|header| header.more_fragments())
    });
    assert!(fragment.len() <= LINK_MTU as usize, "{}", fragment.len());
    let allowed = v6_header(v6_route(ON_LINK_V6, &message));
    sockets
        .send_to(
            &socket,
            IpHeaderFields::V6(allowed),
            plain_fields(),
            &message,
        )
        .expect("the message is sent in fragments");
    let fragment = capture.next(|packet| {
        seen(packet)
            .extensions
            .iter()
            .any(|(kind, _)| *kind == IpNumber::IPV6_FRAGMENTATION_HEADER)
    });
    assert!(fragment.len() <= LINK_MTU as usize, "{}", fragment.len());
}

/// A mark chooses the route a rule gives it.
pub fn mark_selects_the_route<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    ip(MARK_PROHIBITED);
    ip6(MARK_PROHIBITED);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let socket = opened(&sockets, version_of(to));
        let message = echo(version_of(to), PAYLOAD);
        let mut header = header_to(to, &message);
        match &mut header {
            IpHeaderFields::V4(header) => header.route.mark = MARK,
            IpHeaderFields::V6(header) => header.route.mark = MARK,
        }
        assert_eq!(
            sockets.send_to(&socket, header, plain_fields(), &message),
            Err(SendError::Host(HostError::Errno(libc::EACCES))),
            "{to}"
        );
        ping(&sockets, &socket, to, PAYLOAD).expect("an unmarked request is sent");
    }
}

/// A socket's owner chooses the route a rule on its user gives it.
pub fn owner_selects_the_route<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    ip(OWNER_PROHIBITED);
    ip6(OWNER_PROHIBITED);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let other = sockets
            .open(version_of(to), OTHER_USER)
            .expect("an echo socket of another user opens");
        assert_eq!(
            ping(&sockets, &other, to, PAYLOAD),
            Err(SendError::Host(HostError::Errno(libc::EACCES))),
            "{to}"
        );
        let root = opened(&sockets, version_of(to));
        ping(&sockets, &root, to, PAYLOAD).expect("the superuser's request is sent");
    }
}

/// A message's ICMP type and code choose the route a rule on them gives it; a selector other than the message's is refused or routed by.
pub fn icmp_type_and_code_select_the_route<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    ip(ECHO_PROHIBITED);
    ip6(ECHO_PROHIBITED_V6);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let socket = opened(&sockets, version_of(to));
        assert_eq!(
            ping(&sockets, &socket, to, PAYLOAD),
            Err(SendError::Host(HostError::Errno(libc::EACCES))),
            "{to}"
        );
        let message = echo(version_of(to), PAYLOAD);
        let mut header = header_to(to, &message);
        let other = Some(UpperLayerSelector::Icmp {
            message_type: IcmpType(message[0]),
            code: OTHER_CODE,
        });
        match &mut header {
            IpHeaderFields::V4(header) => header.route.upper_layer.selector = other,
            IpHeaderFields::V6(header) => header.route.upper_layer.selector = other,
        }
        unless_refused(
            sockets.send_to(&socket, header, plain_fields(), &message),
            |sent| sent.expect("a request routed by another code is sent"),
        );
    }
}

/// An interface chooses among routes to a destination the one out of that link.
pub fn interface_selects_the_link<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    ip(TWO_ROUTES);
    let second = index(SECOND);
    let first_capture = Capture::on(DUMMY);
    let second_capture = Capture::on(SECOND);
    for to in [IpAddr::V4(NOWHERE), IpAddr::V6(NOWHERE_V6)] {
        let socket = opened(&sockets, version_of(to));
        ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
        assert_eq!(seen(&first_capture.echo_request()).destination, to);
        let message = echo(version_of(to), PAYLOAD);
        let mut header = header_to(to, &message);
        match &mut header {
            IpHeaderFields::V4(header) => header.route.interface = Some(second),
            IpHeaderFields::V6(header) => header.route.interface = Some(second),
        }
        sockets
            .send_to(&socket, header, plain_fields(), &message)
            .expect("the request is sent on the second link");
        assert_eq!(seen(&second_capture.echo_request()).destination, to);
    }
}

/// A link-scope send takes a destination on the link and refuses one through a gateway.
pub fn link_scope_takes_only_the_link<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    let socket = opened(&sockets, IpVersion::V4);
    for (to, on_link) in [(ON_LINK, true), (FAR, false)] {
        let message = echo(IpVersion::V4, PAYLOAD);
        let route = Ipv4RouteSelectors {
            scope: RouteScope::Link,
            ..v4_route(to, &message)
        };
        let sent = sockets.send_to(
            &socket,
            IpHeaderFields::V4(v4_header(route)),
            plain_fields(),
            &message,
        );
        assert_eq!(sent.is_ok(), on_link, "{to}: {sent:?}");
        ping(&sockets, &socket, IpAddr::V4(to), PAYLOAD).expect("a universe-scope request is sent");
    }
}

/// A broadcast destination is refused without the permission and sent with it.
pub fn broadcast_needs_its_permission<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    let socket = opened(&sockets, IpVersion::V4);
    let capture = Capture::on(DUMMY);
    assert_eq!(
        ping(&sockets, &socket, IpAddr::V4(BROADCAST), PAYLOAD),
        Err(SendError::Host(HostError::Errno(libc::EACCES)))
    );
    let message = echo(IpVersion::V4, PAYLOAD);
    let route = Ipv4RouteSelectors {
        broadcast: BroadcastPermission::Allowed,
        ..v4_route(BROADCAST, &message)
    };
    sockets
        .send_to(
            &socket,
            IpHeaderFields::V4(v4_header(route)),
            plain_fields(),
            &message,
        )
        .expect("the broadcast is sent");
    assert_eq!(
        seen(&capture.echo_request()).destination,
        IpAddr::V4(BROADCAST)
    );
}

/// A source the namespace lacks is refused when its check is required and sent from when it is waived.
pub fn a_waived_source_check_sends_from_an_absent_address<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    const ABSENT: Ipv4Addr = Ipv4Addr::new(10, 99, 0, 1);
    let socket = opened(&sockets, IpVersion::V4);
    let capture = Capture::on(DUMMY);
    let message = echo(IpVersion::V4, PAYLOAD);
    let required = Ipv4RouteSelectors {
        source: Some(ABSENT),
        ..v4_route(ON_LINK, &message)
    };
    assert!(
        sockets
            .send_to(
                &socket,
                IpHeaderFields::V4(v4_header(required)),
                plain_fields(),
                &message
            )
            .is_err()
    );
    let waived = Ipv4RouteSelectors {
        source_address_check: SourceAddressCheck::Waived,
        ..required
    };
    sockets
        .send_to(
            &socket,
            IpHeaderFields::V4(v4_header(waived)),
            plain_fields(),
            &message,
        )
        .expect("the request is sent from the absent address");
    assert_eq!(seen(&capture.echo_request()).source, IpAddr::V4(ABSENT));
}

/// Makes the dummy link's address the source of temporary addresses, of which Linux makes one at once.
const TEMPORARY_ADDRESSES: &str = "/proc/sys/net/ipv6/conf/ia/use_tempaddr";
const MANAGED_TEMPORARY: &str = "address replace fd03::1/64 dev ia nodad mngtmpaddr";

/// Gives the dummy link a temporary address beside [`LOCAL_V6`] and answers which source each preference takes.
pub fn sources_by_preference() -> [(TemporaryAddressPreference, Ipv6Addr); 2] {
    write_setting(TEMPORARY_ADDRESSES, "1");
    ip6(MANAGED_TEMPORARY);
    let temporary = wcontainers_route_resolver_conformance::temporary_address(DUMMY);
    [
        (TemporaryAddressPreference::Temporary, temporary),
        (TemporaryAddressPreference::Public, LOCAL_V6),
    ]
}

/// IPv6 source preferences choose between the link's public address and a temporary one.
pub fn source_preferences_are_taken<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    let sources = sources_by_preference();
    let socket = opened(&sockets, IpVersion::V6);
    let capture = Capture::on(DUMMY);
    let message = echo(IpVersion::V6, PAYLOAD);
    for (temporary, source) in sources {
        let route = Ipv6RouteSelectors {
            source_preferences: SourcePreferences {
                temporary,
                mobile: MobileAddressPreference::Home,
            },
            ..v6_route(ON_LINK_V6, &message)
        };
        sockets
            .send_to(
                &socket,
                IpHeaderFields::V6(v6_header(route)),
                plain_fields(),
                &message,
            )
            .expect("the request is sent");
        assert_eq!(
            seen(&capture.echo_request()).source,
            IpAddr::V6(source),
            "{temporary:?}"
        );
    }
}

/// A confirmed send confirms its next hop's neighbour entry, or is refused.
pub fn a_confirmed_send_confirms_the_neighbour<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    ip(STALE_GATEWAYS);
    for (to, gateway) in [
        (IpAddr::V4(FAR), IpAddr::V4(GATEWAY)),
        (IpAddr::V6(FAR_V6), IpAddr::V6(GATEWAY_V6)),
    ] {
        let socket = opened(&sockets, version_of(to));
        let message = echo(version_of(to), PAYLOAD);
        let fields = IpSendFields {
            neighbour: NeighbourConfirmation::Confirmed,
            ..plain_fields()
        };
        assert!(seconds_since_confirmed(gateway, DUMMY) > 0);
        unless_refused(
            sockets.send_to(&socket, header_to(to, &message), fields, &message),
            |sent| {
                sent.expect("the confirmed request is sent");
                assert_eq!(seconds_since_confirmed(gateway, DUMMY), 0, "{gateway}");
            },
        );
    }
}

/// A send's priority chooses its band of a priority queue.
pub fn priority_chooses_the_band<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    tc(PRIO_QUEUE);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let socket = opened(&sockets, version_of(to));
        let message = echo(version_of(to), PAYLOAD);
        let third_band = || {
            number_after(
                &output(TRAFFIC_CONTROL, &[&SHOW_CLASSES[..], &[DUMMY]].concat()),
                THIRD_BAND,
                PACKETS,
            )
        };
        let before = third_band();
        ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
        assert_eq!(third_band(), before, "{to}: priority 0 is not filler");
        let fields = IpSendFields {
            priority: PacketPriority(FILLER),
            ..plain_fields()
        };
        sockets
            .send_to(&socket, header_to(to, &message), fields, &message)
            .expect("the request is sent");
        assert_eq!(third_band(), before + 1, "{to}");
    }
}

/// A send with a transmit time leaves no earlier than that time, on a queue that keeps departure times.
pub fn a_transmit_time_holds_the_send<S: IcmpSockets>(sockets: S) {
    tc(FAIR_QUEUE);
    const HELD: Duration = Duration::from_millis(300);
    for to in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        let socket = opened(&sockets, version_of(to));
        let message = echo(version_of(to), PAYLOAD);
        let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
        let now = Duration::new(now.tv_sec as u64, now.tv_nsec as u32);
        let fields = IpSendFields {
            transmit_time: Some(TransmitTime {
                clock: TransmitClock::Monotonic,
                at: now + HELD,
                mode: TransmitTimeMode::Strict,
                errors: Report::NotRequested,
            }),
            ..plain_fields()
        };
        let start = Instant::now();
        sockets
            .send_to(&socket, header_to(to, &message), fields, &message)
            .expect("the request is sent");
        receive_packet(&sockets, &socket, &mut buffer());
        assert!(start.elapsed() >= HELD, "{to}: {:?}", start.elapsed());
    }
}

/// A schedule of transmit times on the dummy link, on the international atomic clock in strict mode, that misses
/// every time it holds (Linux's etf with a delta of 0).
pub const MISSING_SCHEDULE: &str = "qdisc add dev ia root etf clockid CLOCK_TAI delta 0";
/// How far a scheduled send is put ahead of now, and how much earlier a past one is.
pub const SCHEDULED_AHEAD: Duration = Duration::from_millis(100);

/// The sends a rejected transmit times check makes: each one's time, whether it asks for errors, and what is
/// reported; its key is its place.
pub fn rejected_sends() -> Vec<(TransmitTime, Option<TransmitTimeError>)> {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::Tai);
    let now = Duration::new(
        u64::try_from(now.tv_sec).expect("a time after the epoch"),
        u32::try_from(now.tv_nsec).expect("nanoseconds"),
    );
    let time = |at, mode, errors| TransmitTime {
        clock: TransmitClock::InternationalAtomicTime,
        at,
        mode,
        errors,
    };
    vec![
        (
            time(
                now + SCHEDULED_AHEAD,
                TransmitTimeMode::Strict,
                Report::NotRequested,
            ),
            None,
        ),
        (
            time(
                now - SCHEDULED_AHEAD,
                TransmitTimeMode::Strict,
                Report::Requested,
            ),
            Some(TransmitTimeError::InvalidParameter),
        ),
        (
            time(
                now + SCHEDULED_AHEAD,
                TransmitTimeMode::Deadline,
                Report::Requested,
            ),
            Some(TransmitTimeError::InvalidParameter),
        ),
        (
            time(
                now + SCHEDULED_AHEAD,
                TransmitTimeMode::Strict,
                Report::Requested,
            ),
            Some(TransmitTimeError::Missed),
        ),
    ]
}

/// The send fields of a rejected transmit times check's send at its place: a missed time asks for the packet.
pub fn rejected_send_fields(
    place: usize,
    time: TransmitTime,
    reason: Option<TransmitTimeError>,
) -> IpSendFields {
    let mut fields = IpSendFields {
        transmit_time: Some(time),
        ..plain_fields()
    };
    fields.transmit_reports.key = TransmitReportKey(u32::try_from(place).expect("a small place"));
    if reason == Some(TransmitTimeError::Missed) {
        fields.transmit_reports.packet = Report::Requested;
    }
    fields
}

/// Checks the rejections a socket reported, in the order received, against the sends made.
pub fn check_rejections(
    received: impl FnMut(&mut [u8]) -> Result<Received, HostError>,
    sends: &[(TransmitTime, Option<TransmitTimeError>)],
) {
    let mut received = received;
    let mut reported = Vec::new();
    let mut into = buffer();
    let start = Instant::now();
    while start.elapsed() < SCHEDULED_AHEAD + PATIENCE {
        match received(&mut into) {
            Ok(Received::TransmitReport(report)) => {
                let TransmitEvent::TransmitTimeRejected(reason) = report.event else {
                    panic!("a rejection, not {report:?}")
                };
                assert_eq!(
                    report.packet_length.is_some(),
                    reason == TransmitTimeError::Missed
                );
                reported.push((report.key, Some(reason)));
            }
            Ok(Received::Packet(packet)) => panic!("a report, not {packet:?}"),
            Err(HostError::WouldBlock) => std::thread::sleep(Duration::from_millis(1)),
            Err(error) => panic!("receive: {error:?}"),
        }
        if reported.len() == sends.iter().filter(|send| send.1.is_some()).count() {
            break;
        }
    }
    reported.sort_by_key(|(key, _)| key.0);
    let expected: Vec<_> = sends
        .iter()
        .enumerate()
        .filter(|(_, send)| send.1.is_some())
        .map(|(place, send)| {
            (
                TransmitReportKey(u32::try_from(place).expect("a small place")),
                send.1,
            )
        })
        .collect();
    assert_eq!(reported, expected);
}

/// A transmit time the link's schedule cannot keep is reported with the key of its send and why: a time already
/// past or a mode other than the schedule's is invalid, and a time the schedule misses is missed, with the packet
/// as asked; a send that did not ask for errors gets no report.
pub fn rejected_transmit_times_are_reported_with_their_keys<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    tc(MISSING_SCHEDULE);
    let to = IpAddr::V4(ON_LINK);
    let socket = opened(&sockets, IpVersion::V4);
    let message = echo(IpVersion::V4, PAYLOAD);
    let sends = rejected_sends();
    for (place, &(time, reason)) in sends.iter().enumerate() {
        let fields = rejected_send_fields(place, time, reason);
        match sockets.send_to(&socket, header_to(to, &message), fields, &message) {
            Ok(()) | Err(SendError::Host(_)) => {}
            Err(error) => panic!("send {place}: {error:?}"),
        }
    }
    check_rejections(|into| sockets.receive(&socket, into), &sends);
}

/// Transmit reports come on the socket with the key of their send, with the packet or without it as asked.
pub fn transmit_reports_carry_the_key_of_their_send<S: IcmpSockets>(sockets: S) {
    for to in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        for packet in [Report::Requested, Report::NotRequested] {
            let socket = opened(&sockets, version_of(to));
            let message = echo(version_of(to), PAYLOAD);
            let key = REPORT_KEY;
            let fields = IpSendFields {
                transmit_reports: TransmitReportRequest {
                    key,
                    scheduled: Report::Requested,
                    software: Report::Requested,
                    packet,
                    ..no_reports()
                },
                ..plain_fields()
            };
            sockets
                .send_to(&socket, header_to(to, &message), fields, &message)
                .expect("the request is sent");
            let mut stages = Vec::new();
            let mut into = buffer();
            while stages.len() < 2 {
                match receive(&sockets, &socket, &mut into) {
                    Received::TransmitReport(report) => {
                        assert_eq!(report.key, key);
                        assert_eq!(report.packet_length.is_some(), packet == Report::Requested);
                        let TransmitEvent::Timestamp { stage, source, .. } = report.event else {
                            panic!("a timestamp, not {:?}", report.event);
                        };
                        assert_eq!(source, TimestampSource::Software);
                        stages.push(stage);
                    }
                    Received::Packet(_) => {}
                }
            }
            stages.sort_by_key(|stage| *stage as u8);
            assert_eq!(
                stages,
                [TransmitStage::Scheduled, TransmitStage::Sent],
                "{to}"
            );
        }
    }
}

/// Receive timestamps come only when the setting asks for them; loopback has no hardware clock to bind to.
pub fn receive_timestamps_come_only_when_set<S: IcmpSockets>(sockets: S) {
    for to in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        let socket = opened(&sockets, version_of(to));
        ping(&sockets, &socket, to, PAYLOAD).unwrap();
        let fields = receive_packet(&sockets, &socket, &mut buffer());
        assert_eq!(fields.timestamps.software, None);
        let stamping = Timestamping {
            receive_software: Report::Requested,
            receive_hardware: Report::Requested,
            bound_clock: None,
        };
        set(&sockets, &socket, IpSocketSetting::Timestamping(stamping));
        ping(&sockets, &socket, to, PAYLOAD).unwrap();
        let fields = receive_packet(&sockets, &socket, &mut buffer());
        assert!(fields.timestamps.software.is_some(), "{to}");
        assert_eq!(fields.timestamps.hardware, None);
        let bound = Timestamping {
            bound_clock: Some(wcontainers_internet_protocol::PtpHardwareClockIndex(
                u32::MIN,
            )),
            ..stamping
        };
        assert!(
            sockets
                .configure(&socket, IpSocketSetting::Timestamping(bound))
                .is_err()
        );
    }
}

/// An ICMP error about a request arrives on its socket, written whole from its IP header and quoting the request.
fn icmp_error_arrives_whole<S: IcmpSockets>(sockets: S, to: IpAddr) {
    ip(NETWORK);
    {
        let version = version_of(to);
        let socket = opened(&sockets, version);
        sockets
            .bind_identifier(&socket, identifier(version), IdentifierSharing::Exclusive)
            .unwrap();
        let request = provoke_icmp_error(&sockets, &socket, to);
        let source = seen(&request).source;
        let mut into = buffer();
        let fields = receive_packet(&sockets, &socket, &mut into);
        let packet = seen(&into[..fields.length]);
        assert_eq!(packet.protocol, protocol(version));
        assert_eq!(packet.destination, source);
        assert_eq!(&packet.payload[..2], unreachable(version));
        assert!(checks_out(packet.payload, &packet));
        let quoted = seen(&packet.payload[ICMP_HEADER..]);
        assert_eq!((quoted.source, quoted.destination), (source, to));
        assert_eq!(identifier_of(quoted.payload), identifier(version).0.get());
    }
}

pub fn an_icmp_error_arrives_whole_over_ipv4<S: IcmpSockets>(sockets: S) {
    icmp_error_arrives_whole(sockets, IpAddr::V4(ON_LINK));
}

pub fn an_icmp_error_arrives_whole_over_ipv6<S: IcmpSockets>(sockets: S) {
    icmp_error_arrives_whole(sockets, IpAddr::V6(ON_LINK_V6));
}

/// Sends an ICMP message from a raw socket of the namespace to a local address; the checksum is written here for IPv4.
pub fn send_raw_icmp(version: IpVersion, to: IpAddr, message: &mut [u8]) {
    let (family, protocol) = match version {
        IpVersion::V4 => (rustix::net::AddressFamily::INET, rustix::net::ipproto::ICMP),
        IpVersion::V6 => (
            rustix::net::AddressFamily::INET6,
            rustix::net::ipproto::ICMPV6,
        ),
    };
    if version == IpVersion::V4 {
        let checksum = etherparse::checksum::Sum16BitWords::new()
            .add_slice(message)
            .ones_complement()
            .to_ne_bytes();
        message[CHECKSUM_BYTES].copy_from_slice(&checksum);
    }
    let raw = rustix::net::socket(family, rustix::net::SocketType::RAW, Some(protocol))
        .expect("a raw socket opens");
    rustix::net::sendto(
        &raw,
        message,
        rustix::net::SendFlags::empty(),
        &std::net::SocketAddr::new(to, u16::MIN),
    )
    .expect("the ICMP message is sent");
}

/// Messages held by a pacing rate wait in the send buffer and are counted unsent; a delivered send leaves none.
pub fn pacing_holds_messages_in_the_send_buffer<S: IcmpSockets>(sockets: S) {
    tc(FAIR_QUEUE);
    const RATE: BytesPerSecond = BytesPerSecond(20_000);
    const MESSAGES: usize = 4;
    let long = vec![u8::MAX; PACED_MESSAGE];
    for to in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        let socket = opened(&sockets, version_of(to));
        ping(&sockets, &socket, to, PAYLOAD).unwrap();
        receive_packet(&sockets, &socket, &mut buffer());
        assert_eq!(
            sockets.count_unsent_bytes(&socket),
            Ok(ByteCount(usize::MIN))
        );
        set(
            &sockets,
            &socket,
            IpSocketSetting::MaximumPacingRate(Some(RATE)),
        );
        let start = Instant::now();
        for _ in 0..MESSAGES {
            ping(&sockets, &socket, to, &long).expect("a paced request is sent");
        }
        let unsent = sockets.count_unsent_bytes(&socket).unwrap();
        assert!(unsent.0 >= long.len(), "{to}: {unsent:?}");
        for _ in 0..MESSAGES {
            receive_packet(&sockets, &socket, &mut buffer());
        }
        // Each request of about a thousand bytes waits its time at the rate behind the one before.
        let least =
            Duration::from_secs_f64(((MESSAGES - 1) * PACED_MESSAGE) as f64 / RATE.0 as f64 / 2.0);
        assert!(start.elapsed() >= least, "{to}: {:?}", start.elapsed());
    }
}

/// The smallest send buffer makes a send that would wait behind paced ones would block.
pub fn a_full_send_buffer_would_block<S: IcmpSockets>(sockets: S) {
    tc(FAIR_QUEUE);
    let long = vec![u8::MAX; PACED_MESSAGE];
    let to = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let socket = opened(&sockets, IpVersion::V4);
    set(
        &sockets,
        &socket,
        IpSocketSetting::SendBuffer(ByteCount(usize::MIN)),
    );
    set(
        &sockets,
        &socket,
        IpSocketSetting::MaximumPacingRate(Some(SLOW_RATE)),
    );
    let blocked = (0..ATTEMPTS)
        .any(|_| ping(&sockets, &socket, to, &long) == Err(SendError::Host(HostError::WouldBlock)));
    assert!(blocked);
}

/// A receive buffer too small for every reply drops some, and the next packet tells how many.
pub fn a_small_receive_buffer_counts_its_drops<S: IcmpSockets>(sockets: S) {
    const REQUESTS: usize = 64;
    let to = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let socket = opened(&sockets, IpVersion::V4);
    set(
        &sockets,
        &socket,
        IpSocketSetting::ReceiveBuffer(ByteCount(usize::MIN)),
    );
    let long = vec![u8::MAX; PACED_MESSAGE];
    for _ in 0..REQUESTS {
        ping(&sockets, &socket, to, &long).unwrap();
    }
    std::thread::sleep(QUIET);
    let mut received = 0;
    let mut dropped = 0;
    let mut into = buffer();
    while let Ok(Received::Packet(fields)) = sockets.receive(&socket, &mut into) {
        received += 1;
        dropped += fields.drops_since_previous.0 as usize;
    }
    assert!(received < REQUESTS, "{received} of {REQUESTS} arrived");
    set(
        &sockets,
        &socket,
        IpSocketSetting::ReceiveBuffer(ByteCount(usize::from(u16::MAX))),
    );
    ping(&sockets, &socket, to, PAYLOAD).unwrap();
    let last = receive_packet(&sockets, &socket, &mut into);
    dropped += last.drops_since_previous.0 as usize;
    assert_eq!(received + dropped, REQUESTS);
}

/// Joined groups and anycast addresses are the host's while joined, and multicast loopback brings a reply.
pub fn memberships_are_taken<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    write_setting(ECHO_IGNORE_BROADCASTS, &u8::MIN.to_string());
    const GROUP: Ipv4Addr = Ipv4Addr::new(239, 1, 2, 3);
    const GROUP_V6: Ipv6Addr = Ipv6Addr::new(0xff15, 0, 0, 0, 0, 0, 0, 0x123);
    const ANYCAST: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 0x99);
    let link = Some(index(DUMMY));
    let socket = opened(&sockets, IpVersion::V4);
    let joined = |group: IpAddr| {
        output(IP, &[&SHOW_GROUPS[..], &[DUMMY]].concat()).contains(&group.to_string())
    };
    let group = |membership| MulticastGroup {
        group: GROUP,
        interface: link,
        membership,
    };
    set(
        &sockets,
        &socket,
        IpSocketSetting::MulticastV4(group(MulticastMembership::Joined(SourceFilter::Exclude(
            &[],
        )))),
    );
    assert!(joined(IpAddr::V4(GROUP)));
    for (looped, replied) in [(false, false), (true, true)] {
        set(
            &sockets,
            &socket,
            IpSocketSetting::MulticastLoopback(looped),
        );
        let message = echo(IpVersion::V4, PAYLOAD);
        let route = Ipv4RouteSelectors {
            interface: link,
            ..v4_route(GROUP, &message)
        };
        sockets
            .send_to(
                &socket,
                IpHeaderFields::V4(v4_header(route)),
                plain_fields(),
                &message,
            )
            .expect("the multicast request is sent");
        assert_eq!(
            !nothing_arrives(&sockets, &socket),
            replied,
            "loopback {looped}"
        );
    }
    set(
        &sockets,
        &socket,
        IpSocketSetting::MulticastV4(group(MulticastMembership::Left)),
    );
    assert!(!joined(IpAddr::V4(GROUP)));
    for enabled in [false, true] {
        set(
            &sockets,
            &socket,
            IpSocketSetting::MulticastAllGroups {
                version: IpVersion::V4,
                enabled,
            },
        );
    }
    let socket = opened(&sockets, IpVersion::V6);
    let group_v6 = |membership| MulticastGroup {
        group: GROUP_V6,
        interface: link,
        membership,
    };
    set(
        &sockets,
        &socket,
        IpSocketSetting::MulticastV6(group_v6(MulticastMembership::Joined(
            SourceFilter::Exclude(&[]),
        ))),
    );
    assert!(joined(IpAddr::V6(GROUP_V6)));
    set(
        &sockets,
        &socket,
        IpSocketSetting::MulticastV6(group_v6(MulticastMembership::Left)),
    );
    assert!(!joined(IpAddr::V6(GROUP_V6)));
    let anycast = |membership| AnycastAddress {
        address: ANYCAST,
        interface: link,
        membership,
    };
    let listed = || {
        std::fs::read_to_string(ANYCAST_LIST)
            .expect("the anycast list")
            .contains(&format!("{:032x}", u128::from(ANYCAST)))
    };
    set(
        &sockets,
        &socket,
        IpSocketSetting::Anycast(anycast(Membership::Joined)),
    );
    assert!(listed());
    set(
        &sockets,
        &socket,
        IpSocketSetting::Anycast(anycast(Membership::Left)),
    );
    assert!(!listed());
}

/// No poll event at all, from which a check asks for the ones it waits for.
pub const NO_EVENTS: PollEvents = PollEvents {
    data_to_read: false,
    exceptional_condition: false,
    writing_possible: false,
    error_condition: false,
    hang_up: false,
    descriptor_not_open: false,
    normal_data_to_read: false,
    priority_band_data_to_read: false,
    normal_data_writable: false,
    priority_data_writable: false,
    message_available: false,
    peer_shut_down_writing: false,
};

pub const READABLE: PollEvents = PollEvents {
    data_to_read: true,
    ..NO_EVENTS
};

pub const WRITABLE: PollEvents = PollEvents {
    writing_possible: true,
    ..NO_EVENTS
};

/// How many sockets a check waits on at once, and how many more waits it makes on the first of them.
pub const WAITED_SOCKETS: usize = 32;
pub const WAITS_ON_ONE: usize = 3;

/// The wakes a waiting call has given, counted, for a check that polls the call by hand.
#[derive(Default)]
pub struct Wakes {
    count: Mutex<usize>,
    changed: Condvar,
}

impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        *self.count.lock().expect("the wakes") += 1;
        self.changed.notify_all();
    }
}

impl Wakes {
    pub fn count(&self) -> usize {
        *self.count.lock().expect("the wakes")
    }

    /// Whether the call has been woken `count` times before the patience of a check ends.
    pub fn reach(&self, count: usize) -> bool {
        let wakes = self.count.lock().expect("the wakes");
        let (wakes, _) = self
            .changed
            .wait_timeout_while(wakes, PATIENCE, |wakes| *wakes < count)
            .expect("the wakes");
        *wakes >= count
    }
}

/// Polls a waiting call once with a waker that counts into `wakes`.
pub fn poll_with<F: Future>(call: Pin<&mut F>, wakes: &Arc<Wakes>) -> Poll<F::Output> {
    let waker = Waker::from(Arc::clone(wakes));
    call.poll(&mut Context::from_waker(&waker))
}

/// The events a finished wait gave.
pub fn finished(poll: Poll<Result<PollEvents, HostError>>) -> PollEvents {
    match poll {
        Poll::Ready(events) => events.expect("wait for events"),
        Poll::Pending => panic!("the wait did not finish"),
    }
}

/// The events a wait gives at once or when it is next woken.
pub fn finished_soon<F: Future<Output = Result<PollEvents, HostError>>>(
    mut call: Pin<&mut F>,
    wakes: &Arc<Wakes>,
) -> PollEvents {
    match poll_with(call.as_mut(), wakes) {
        Poll::Pending => {
            assert!(wakes.reach(1), "the wait is woken");
            finished(poll_with(call, wakes))
        }
        ready => finished(ready),
    }
}

/// Whether events hold none that was not asked for, but for those poll always reports.
pub fn only_asked_for(events: PollEvents, interest: PollEvents) -> bool {
    let asked = |got: bool, wanted: bool| !got || wanted;
    asked(events.data_to_read, interest.data_to_read)
        && asked(events.exceptional_condition, interest.exceptional_condition)
        && asked(events.writing_possible, interest.writing_possible)
        && asked(events.normal_data_to_read, interest.normal_data_to_read)
        && asked(
            events.priority_band_data_to_read,
            interest.priority_band_data_to_read,
        )
        && asked(events.normal_data_writable, interest.normal_data_writable)
        && asked(
            events.priority_data_writable,
            interest.priority_data_writable,
        )
        && asked(events.message_available, interest.message_available)
        && asked(
            events.peer_shut_down_writing,
            interest.peer_shut_down_writing,
        )
}

/// A wait for data does not finish before the echo reply arrives, and finishes, woken once, when it does.
pub fn a_wait_for_data_finishes_when_the_reply_arrives<S: IcmpSockets>(sockets: S) {
    for to in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        let socket = opened(&sockets, version_of(to));
        let wakes = Arc::new(Wakes::default());
        let mut wait = pin!(sockets.wait_for_events(&socket, READABLE));
        assert!(poll_with(wait.as_mut(), &wakes).is_pending());
        std::thread::sleep(QUIET);
        assert_eq!(wakes.count(), 0, "woken with nothing to read");
        assert!(poll_with(wait.as_mut(), &wakes).is_pending());
        ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
        assert!(wakes.reach(1), "woken when the reply arrives");
        let events = finished(poll_with(wait.as_mut(), &wakes));
        assert!(events.data_to_read, "{events:?}");
        assert!(only_asked_for(events, READABLE), "{events:?}");
        assert_eq!(wakes.count(), 1);
        assert!(matches!(
            sockets.receive(&socket, &mut buffer()),
            Ok(Received::Packet(_))
        ));
    }
}

/// A wait for what the socket already has finishes at its first poll, with only the events asked for.
pub fn a_wait_for_writing_finishes_at_once<S: IcmpSockets>(sockets: S) {
    let socket = opened(&sockets, IpVersion::V4);
    let wakes = Arc::new(Wakes::default());
    let events = finished(poll_with(
        pin!(sockets.wait_for_events(&socket, WRITABLE)),
        &wakes,
    ));
    assert!(events.writing_possible, "{events:?}");
    assert!(only_asked_for(events, WRITABLE), "{events:?}");
    assert_eq!(wakes.count(), 0);
}

/// A dropped wait gives up its waker and is not woken by the event it waited for; a new wait then sees it.
pub fn a_dropped_wait_is_withdrawn<S: IcmpSockets>(sockets: S) {
    let to = IpAddr::V6(Ipv6Addr::LOCALHOST);
    let socket = opened(&sockets, IpVersion::V6);
    let wakes = Arc::new(Wakes::default());
    {
        let wait = pin!(sockets.wait_for_events(&socket, READABLE));
        assert!(poll_with(wait, &wakes).is_pending());
    }
    assert_eq!(
        Arc::strong_count(&wakes),
        1,
        "the dropped wait kept its waker"
    );
    ping(&sockets, &socket, to, PAYLOAD).expect("the request is sent");
    std::thread::sleep(QUIET);
    assert_eq!(wakes.count(), 0, "a dropped wait was woken");
    let events = finished(poll_with(
        pin!(sockets.wait_for_events(&socket, READABLE)),
        &wakes,
    ));
    assert!(events.data_to_read, "{events:?}");
}

/// Many waits at once, on many sockets and several on one, each finish woken once when their reply arrives.
pub fn many_waits_each_wake_once<S: IcmpSockets>(sockets: S) {
    let to = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let watched: Vec<_> = (0..WAITED_SOCKETS)
        .map(|_| opened(&sockets, IpVersion::V4))
        .collect();
    let mut waits: Vec<_> = watched
        .iter()
        .chain(core::iter::repeat_n(&watched[0], WAITS_ON_ONE))
        .map(|socket| {
            (
                Box::pin(sockets.wait_for_events(socket, READABLE)),
                Arc::new(Wakes::default()),
            )
        })
        .collect();
    for (wait, wakes) in &mut waits {
        assert!(poll_with(wait.as_mut(), wakes).is_pending());
    }
    for socket in &watched {
        ping(&sockets, socket, to, PAYLOAD).expect("the request is sent");
    }
    for (wait, wakes) in &mut waits {
        assert!(wakes.reach(1), "a wait woken when its reply arrives");
        assert!(finished(poll_with(wait.as_mut(), wakes)).data_to_read);
    }
    std::thread::sleep(QUIET);
    for (_, wakes) in &waits {
        assert_eq!(wakes.count(), 1, "a wait woken more than once");
    }
}

/// The type and code of the ICMP error the checks send: the destination host is unreachable.
fn unreachable(version: IpVersion) -> [u8; 2] {
    match version {
        IpVersion::V4 => [icmpv4::TYPE_DEST_UNREACH, icmpv4::CODE_DST_UNREACH_HOST],
        IpVersion::V6 => [icmpv6::TYPE_DST_UNREACH, icmpv6::CODE_DST_UNREACH_ADDR],
    }
}

/// Sends an echo request to an address on the dummy link and an ICMP error about it to its source, giving the request.
fn provoke_icmp_error<S: IcmpSockets>(sockets: &S, socket: &S::Socket, to: IpAddr) -> Vec<u8> {
    let capture = Capture::on(DUMMY);
    ping(sockets, socket, to, PAYLOAD).expect("the request is sent");
    let request = capture.echo_request();
    let version = version_of(to);
    let quoted_length = match version {
        IpVersion::V4 => ipv4(&request).slice().len() + ICMP_HEADER,
        IpVersion::V6 => request.len(),
    };
    let mut message = unreachable(version).to_vec();
    message.resize(ICMP_HEADER, u8::MIN);
    message.extend_from_slice(&request[..quoted_length]);
    send_raw_icmp(version, seen(&request).source, &mut message);
    request
}

/// An ICMP error about a request ends a wait for data with the error condition, which poll always reports.
pub fn an_icmp_error_ends_a_wait_for_data<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let socket = opened(&sockets, version_of(to));
        let wakes = Arc::new(Wakes::default());
        let mut wait = pin!(sockets.wait_for_events(&socket, READABLE));
        assert!(poll_with(wait.as_mut(), &wakes).is_pending());
        provoke_icmp_error(&sockets, &socket, to);
        assert!(wakes.reach(1), "woken by the ICMP error");
        let events = finished(poll_with(wait.as_mut(), &wakes));
        assert!(events.error_condition, "{events:?}");
        assert!(matches!(
            sockets.receive(&socket, &mut buffer()),
            Ok(Received::Packet(_))
        ));
    }
}

/// An ICMP error that a too long send read off the host while looking for its own error still ends a wait at once.
pub fn a_wait_sees_an_error_a_send_read_first<S: IcmpSockets>(sockets: S) {
    ip(NETWORK);
    let to = IpAddr::V6(ON_LINK_V6);
    let socket = opened(&sockets, IpVersion::V6);
    let wakes = Arc::new(Wakes::default());
    provoke_icmp_error(&sockets, &socket, to);
    let events = finished_soon(pin!(sockets.wait_for_events(&socket, READABLE)), &wakes);
    assert!(events.error_condition, "{events:?}");
    let message = echo(IpVersion::V6, &vec![u8::MAX; LONG_MESSAGE]);
    let forbidden = Ipv6HeaderFields {
        local_fragmentation: LocalFragmentation::Forbidden,
        fragment_mtu: Some(MaximumTransmissionUnit(FRAGMENT_MTU)),
        ..v6_header(v6_route(ON_LINK_V6, &message))
    };
    assert_eq!(
        sockets.send_to(
            &socket,
            IpHeaderFields::V6(forbidden),
            plain_fields(),
            &message
        ),
        Err(SendError::MessageTooLong {
            maximum_transmission_unit: MaximumTransmissionUnit(FRAGMENT_MTU)
        })
    );
    let events = finished(poll_with(
        pin!(sockets.wait_for_events(&socket, READABLE)),
        &wakes,
    ));
    assert!(events.error_condition, "{events:?}");
    let mut into = buffer();
    let fields = receive_packet(&sockets, &socket, &mut into);
    assert_eq!(seen(&into[..fields.length]).protocol, IpNumber::IPV6_ICMP);
    assert!(nothing_arrives(&sockets, &socket));
    let wait = pin!(sockets.wait_for_events(&socket, READABLE));
    assert!(
        poll_with(wait, &wakes).is_pending(),
        "an error held after its entry was received"
    );
}

/// Each setting set twice answers as it did once, by Configurable's own suite.
pub fn configurable_value<S: IcmpSockets>(
    sockets: S,
    setting: IpSocketSetting<'static>,
) -> (S, S::Socket, IpSocketSetting<'static>) {
    ip(NETWORK);
    let socket = opened(&sockets, IpVersion::V6);
    (sockets, socket, setting)
}

/// A membership of an IPv6 group, which Linux would refuse to join a second time.
pub fn multicast_setting() -> IpSocketSetting<'static> {
    const GROUP_V6: Ipv6Addr = Ipv6Addr::new(0xff15, 0, 0, 0, 0, 0, 0, 0x124);
    IpSocketSetting::MulticastV6(MulticastGroup {
        group: GROUP_V6,
        interface: None,
        membership: MulticastMembership::Joined(SourceFilter::Exclude(&[])),
    })
}

/// An anycast address, which Linux would count twice.
pub fn anycast_setting() -> IpSocketSetting<'static> {
    const ANYCAST: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 0x98);
    IpSocketSetting::Anycast(AnycastAddress {
        address: ANYCAST,
        interface: None,
        membership: Membership::Joined,
    })
}

pub fn send_buffer_setting() -> IpSocketSetting<'static> {
    IpSocketSetting::SendBuffer(ByteCount(usize::from(u16::MAX)))
}

pub fn timestamping_setting() -> IpSocketSetting<'static> {
    IpSocketSetting::Timestamping(Timestamping {
        receive_software: Report::Requested,
        receive_hardware: Report::NotRequested,
        bound_clock: None,
    })
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test in a network namespace of its own.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        $crate::each_check!(
            $value;
            echo_reply_arrives_whole_over_ipv4,
            echo_reply_arrives_whole_over_ipv6,
            the_request_carries_the_bound_identifier,
            identifiers_are_shared_only_when_both_ask,
            close_frees_the_identifier,
            receive_with_nothing_queued_would_block,
            a_short_buffer_keeps_the_whole_length,
            ipv4_header_fields_go_out,
            ipv6_header_fields_go_out,
            ipv6_extension_headers_go_out,
            an_ipv6_source_goes_out,
            every_path_mtu_discovery_mode_sends,
            a_message_over_the_mtu_is_too_long_or_fragmented,
            mark_selects_the_route,
            owner_selects_the_route,
            icmp_type_and_code_select_the_route,
            interface_selects_the_link,
            link_scope_takes_only_the_link,
            broadcast_needs_its_permission,
            a_waived_source_check_sends_from_an_absent_address,
            source_preferences_are_taken,
            a_confirmed_send_confirms_the_neighbour,
            priority_chooses_the_band,
            a_transmit_time_holds_the_send,
            transmit_reports_carry_the_key_of_their_send,
            receive_timestamps_come_only_when_set,
            an_icmp_error_arrives_whole_over_ipv4,
            an_icmp_error_arrives_whole_over_ipv6,
            pacing_holds_messages_in_the_send_buffer,
            a_full_send_buffer_would_block,
            a_small_receive_buffer_counts_its_drops,
            memberships_are_taken,
            a_wait_for_data_finishes_when_the_reply_arrives,
            a_wait_for_writing_finishes_at_once,
            a_dropped_wait_is_withdrawn,
            many_waits_each_wake_once,
            an_icmp_error_ends_a_wait_for_data,
            a_wait_sees_an_error_a_send_read_first,
            rejected_transmit_times_are_reported_with_their_keys,
        );
        mod configurable_multicast {
            $crate::configurable_suite::tests!({
                $crate::enter_network_namespace();
                $crate::configurable_value($value, $crate::multicast_setting())
            });
        }
        mod configurable_anycast {
            $crate::configurable_suite::tests!({
                $crate::enter_network_namespace();
                $crate::configurable_value($value, $crate::anycast_setting())
            });
        }
        mod configurable_send_buffer {
            $crate::configurable_suite::tests!({
                $crate::enter_network_namespace();
                $crate::configurable_value($value, $crate::send_buffer_setting())
            });
        }
        mod configurable_timestamping {
            $crate::configurable_suite::tests!({
                $crate::enter_network_namespace();
                $crate::configurable_value($value, $crate::timestamping_setting())
            });
        }
    };
}

/// One test per named check, each entering its own network namespace before the value is made.
#[doc(hidden)]
#[macro_export]
macro_rules! each_check {
    ($value:expr; $($check:ident),* $(,)?) => {
        $(
            #[test]
            fn $check() {
                $crate::enter_network_namespace();
                $crate::$check($value);
            }
        )*
    };
}
