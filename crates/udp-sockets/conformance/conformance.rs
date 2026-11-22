//! The tests every implementation of wcontainers-udp-sockets runs, written against its traits alone.
//!
//! Each check talks over the loopback addresses, with ports the kernel chooses. The rest of the network the
//! checks need is the caller's to provide, on a network of its own:
//! - one interface other than loopback with an address in 192.0.2.0/24 and the routes of IPv4 and IPv6
//!   multicast (multicast_membership_takes_the_groups_datagrams, ipv6_memberships_take_their_datagrams);
//! - user 1000's datagrams to 192.0.2.0/24 prohibited (an_owner_is_routed_as_its_user);
//! - loopback taking segment routing headers (a_routing_header_arrives_in_the_header);
//! - loopback pacing each socket's datagrams to its maximum pacing rate, one at a time at the rate
//!   pacing_holds_datagrams_in_the_send_buffer sets (Linux's fq qdisc);
//! - no hardware clock on loopback (loopback_has_no_hardware_clock);
//! - 198.51.100.0/24 routed through a gateway (link_scope_keeps_a_datagram_on_its_link);
//! - loopback holding each datagram until its transmit time (a_transmit_time_holds_a_datagram_until_then);
//! - the local addresses 127.0.0.2 and 2001:db8::2 routed with an MTU of 1500 bytes
//!   (path_mtu_discovery_modes_take_the_routes_mtu, forbidden_fragmentation_refuses_what_would_be_fragmented,
//!   an_ipv6_send_takes_its_source);
//! - an interface with 203.0.113.1/24 and 2001:db8:1::1/64, this prefix's one public address, with a temporary
//!   address made from it and temporary addresses preferred (connect_takes_its_route_selectors), whose queue
//!   keeps a schedule of transmit times on the international atomic clock in strict mode and misses every time
//!   it holds (Linux's etf qdisc with a delta of 0; rejected_transmit_times_are_reported_with_their_keys).
//!
//! The checks are of Linux's behaviour on a Linux network, so the suite builds only on Linux.
#![cfg(target_os = "linux")]

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use core::num::NonZeroU16;
use std::pin::{Pin, pin};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

use etherparse::checksum::Sum16BitWords;
use etherparse::{
    IpDscp, IpEcn, IpNumber, IpSlice, Ipv4Options, Ipv6FlowLabel, UdpHeader, UdpHeaderSlice,
};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    AddressSharing, AnycastAddress, BindAddress, BroadcastPermission, ByteCount, BytesPerSecond,
    FilterInstruction, FilterJump, FilterOperand, FilterOperation, FragmentSize, InterfaceIndex,
    IpHeaderFields, IpSendFields, IpSocketSetting, IpVersion, Ipv4HeaderFields, Ipv4RouteSelectors,
    Ipv6Families, Ipv6HeaderFields, Ipv6RouteSelectors, LocalBinding, LocalFragmentation,
    LocalPort, MaximumTransmissionUnit, Membership, MobileAddressPreference, MulticastGroup,
    MulticastMembership, NeighbourConfirmation, NonlocalAddresses, PacketMark, PacketPriority,
    PathMtuDiscovery, Port, PortChoice, PortRange, PortSetting, PortSharing, ProcessorIndex,
    PtpHardwareClockIndex, Report, RouteScope, RouteSelectors, SendError, SocketFilter,
    SourceAddressCheck, SourceFilter, SourcePreferences, TemporaryAddressPreference,
    TimestampSource, Timestamping, TransmitClock, TransmitEvent, TransmitReportKey,
    TransmitReportRequest, TransmitStage, TransmitTime, TransmitTimeError, TransmitTimeMode,
    TransportPorts, UpperLayer, UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_readiness::ReadyFor;
use wcontainers_readiness_conformance::Waiting;

pub use wcontainers_readiness_conformance as readiness_conformance;
use wcontainers_udp_sockets::{
    ChecksumCoverage, ReceiveCoalescing, SegmentSize, UdpChecksum, UdpDatagram, UdpLocalEnd,
    UdpProtocol, UdpReceived, UdpReceivedPacket, UdpSocketSetting, UdpSockets, ZeroChecksumReceipt,
};

/// The owner every check opens its sockets for: the superuser, whom any host lets own a socket.
const OWNER: UserId = UserId(u32::MIN);

/// How long a check waits for a datagram sent over loopback.
const PATIENCE: Duration = Duration::from_secs(1);

/// How long a check waits to see that no datagram arrives.
const SILENCE: Duration = Duration::from_millis(200);

/// How long a transmit time holds a datagram back.
const HELD: Duration = Duration::from_millis(200);

/// How far apart the transmit times of one check are, so that each rejection names one send.
const APART: Duration = Duration::from_millis(10);

/// 198.51.100.1, an address of TEST-NET-2 ([RFC 5737](https://www.rfc-editor.org/rfc/rfc5737)), which the
/// caller's network routes through a gateway.
const BEYOND_A_GATEWAY: Ipv4Addr = Ipv4Addr::new(198, 51, 100, 1);

/// Local addresses the caller's network routes with an MTU of ROUTE_MTU; the IPv6 one is of the documentation prefix.
const NARROW_V4: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 2);
const NARROW_V6: Ipv6Addr = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2);
const ROUTE_MTU: u16 = linux_raw_sys::if_ether::ETH_DATA_LEN as u16;

/// The addresses of the interface with a schedule of transmit times, of TEST-NET-3 and the documentation prefix,
/// an address its IPv4 route reaches and one its IPv6 prefix does, and the multicast interface's address.
const SCHEDULED_V4: Ipv4Addr = Ipv4Addr::new(203, 0, 113, 1);
const SCHEDULED_V4_PEER: Ipv4Addr = Ipv4Addr::new(203, 0, 113, 2);
const PUBLIC_V6: Ipv6Addr = Ipv6Addr::new(0x2001, 0xdb8, 1, 0, 0, 0, 0, 1);
const PREFIX_V6_PEER: Ipv6Addr = Ipv6Addr::new(0x2001, 0xdb8, 1, 0, 0, 0, 0, 0x99);
const MULTICAST_INTERFACE_V4: Ipv4Addr = Ipv4Addr::new(192, 0, 2, 3);

/// The segments of an IPv6 address that hold a /64 prefix.
const PREFIX_SEGMENTS: usize = 4;

/// The unit of a fragment's data and offset, [RFC 8200](https://www.rfc-editor.org/rfc/rfc8200#section-4.5).
const FRAGMENT_UNIT: u16 = 8;

/// How many ports above a free one a reserved range covers: a choice that ignores it is caught every time.
const RESERVED_WIDTH: u16 = 20;

/// Ports below the range Linux chooses from by default (32768 to 60999), and how many at their start are reserved.
const LOW_PORTS: PortRange = PortRange {
    first: Port(2000),
    last: Port(2010),
};
const LOW_PORTS_RESERVED: u16 = 5;

/// How many senders a check uses so that a port group's default choice reaches every socket of the group.
const SENDERS: usize = 32;

/// ff0e, the first group of an IPv6 multicast address of global scope.
const GLOBAL_MULTICAST: u16 = 0xff0e;

/// A user other than the superuser, whose datagrams the caller's network prohibits beyond this host.
const OTHER_OWNER: UserId = UserId(1000);

/// 192.0.2.1, an address of TEST-NET-1 ([RFC 5737](https://www.rfc-editor.org/rfc/rfc5737)), routed nowhere.
const ROUTED_NOWHERE: Ipv4Addr = Ipv4Addr::new(192, 0, 2, 1);

/// The pacing rate of pacing_holds_datagrams_in_the_send_buffer, below fq's 550 kbit/s at which it paces
/// datagrams one at a time.
const PACING_RATE: BytesPerSecond = BytesPerSecond(50_000);

/// How many datagrams of PACED_LENGTH a paced check sends.
const PACED_DATAGRAMS: usize = 5;
pub const PACED_LENGTH: usize = 1000;

/// The routing type of a segment routing header, [RFC 8754](https://www.rfc-editor.org/rfc/rfc8754#section-2).
const SEGMENT_ROUTING: u8 = 4;

/// The bytes of a segment routing header after its next header and length, up to its segments: type,
/// segments left, last entry, flags and tag.
const SEGMENT_ROUTING_FIXED: usize = 6;

/// fe80, the first group of an IPv6 link-local address.
const LINK_LOCAL: u16 = 0xfe80;

const PAYLOAD: &[u8] = b"a datagram over loopback";
const FROM_A_STRANGER: &[u8] = b"from a stranger";
const TOO_LITTLE_COVERED: &[u8] = b"too little covered";
const WITHOUT_A_CHECKSUM: &[u8] = b"without a checksum";
/// The payloads of one send split into segments of their common length.
const SEGMENTS: &[&[u8]] = &[b"one", b"two", b"six"];

fn loopback(version: IpVersion) -> IpAddr {
    match version {
        IpVersion::V4 => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpVersion::V6 => IpAddr::V6(Ipv6Addr::LOCALHOST),
    }
}

fn binding(address: IpAddr, port: u16) -> LocalBinding<'static> {
    LocalBinding {
        address: match address {
            IpAddr::V4(address) => BindAddress::V4(address),
            IpAddr::V6(address) => BindAddress::V6 {
                address,
                families: Ipv6Families::V6Only,
            },
        },
        port: LocalPort::Requested(Port(port)),
    }
}

fn protocol_number(protocol: UdpProtocol) -> IpNumber {
    match protocol {
        UdpProtocol::Udp => IpNumber::UDP,
        UdpProtocol::Lite => IpNumber::UDP_LITE,
    }
}

fn upper_layer(protocol: UdpProtocol, from: u16, to: u16) -> UpperLayer {
    UpperLayer {
        protocol: protocol_number(protocol),
        selector: Some(UpperLayerSelector::Ports(TransportPorts {
            source: Port(from),
            destination: Port(to),
        })),
    }
}

static NO_OPTIONS: std::sync::LazyLock<Ipv4Options> = std::sync::LazyLock::new(Ipv4Options::new);

fn ipv4_route(
    protocol: UdpProtocol,
    from: SocketAddr,
    to: SocketAddr,
) -> Ipv4RouteSelectors<'static> {
    let IpAddr::V4(destination) = to.ip() else {
        panic!("an IPv4 destination")
    };
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
        upper_layer: upper_layer(protocol, from.port(), to.port()),
    }
}

fn ipv6_route(
    protocol: UdpProtocol,
    from: SocketAddr,
    to: SocketAddr,
) -> Ipv6RouteSelectors<'static> {
    let IpAddr::V6(destination) = to.ip() else {
        panic!("an IPv6 destination")
    };
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
        upper_layer: upper_layer(protocol, from.port(), to.port()),
    }
}

fn route(protocol: UdpProtocol, from: SocketAddr, to: SocketAddr) -> RouteSelectors<'static> {
    match to {
        SocketAddr::V4(_) => RouteSelectors::V4(ipv4_route(protocol, from, to)),
        SocketAddr::V6(_) => RouteSelectors::V6(ipv6_route(protocol, from, to)),
    }
}

fn ipv4_header(route: Ipv4RouteSelectors<'_>) -> IpHeaderFields<'_> {
    IpHeaderFields::V4(Ipv4HeaderFields {
        route,
        time_to_live: None,
        path_mtu_discovery: PathMtuDiscovery::Want,
    })
}

fn ipv6_header(route: Ipv6RouteSelectors<'_>) -> Ipv6HeaderFields<'_> {
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

fn header(protocol: UdpProtocol, from: SocketAddr, to: SocketAddr) -> IpHeaderFields<'static> {
    match to {
        SocketAddr::V4(_) => ipv4_header(ipv4_route(protocol, from, to)),
        SocketAddr::V6(_) => IpHeaderFields::V6(ipv6_header(ipv6_route(protocol, from, to))),
    }
}

fn no_reports() -> TransmitReportRequest {
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

fn plain_fields() -> IpSendFields {
    IpSendFields {
        neighbour: NeighbourConfirmation::Unconfirmed,
        priority: PacketPriority(u32::MIN),
        transmit_time: None,
        transmit_reports: no_reports(),
    }
}

fn datagram(payload: &[u8]) -> UdpDatagram<'_> {
    UdpDatagram {
        checksum: UdpChecksum::Computed,
        segment_size: None,
        payload,
    }
}

/// A socket bound to the loopback address of its version on a port the kernel chooses.
fn bound<S: UdpSockets>(
    sockets: &S,
    version: IpVersion,
    protocol: UdpProtocol,
) -> (S::Socket, SocketAddr) {
    let socket = sockets
        .open(version, protocol, OWNER)
        .expect("open a socket");
    let address = sockets
        .bind(&socket, binding(loopback(version), u16::MIN))
        .expect("bind to loopback");
    (socket, address)
}

/// Sets one setting twice, which must answer as setting it once did.
fn set<S: UdpSockets>(sockets: &S, socket: &S::Socket, setting: UdpSocketSetting<'_>) {
    let once = sockets.configure(socket, setting);
    assert_eq!(
        sockets.configure(socket, setting),
        once,
        "setting {setting:?} again"
    );
    once.expect("set a setting");
}

/// Whether a datagram arrives for a socket before the silence of a check ends, taking it.
fn arrives<S: UdpSockets>(sockets: &S, socket: &S::Socket) -> bool {
    let start = Instant::now();
    let mut into = buffer();
    loop {
        match sockets.receive(socket, &mut into) {
            Ok(_) => return true,
            Err(HostError::WouldBlock) if start.elapsed() < SILENCE => std::thread::yield_now(),
            Err(HostError::WouldBlock) => return false,
            Err(error) => panic!("receive: {error:?}"),
        }
    }
}

fn unspecified(version: IpVersion) -> IpAddr {
    match version {
        IpVersion::V4 => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        IpVersion::V6 => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    }
}

fn version_of(address: IpAddr) -> IpVersion {
    match address {
        IpAddr::V4(_) => IpVersion::V4,
        IpAddr::V6(_) => IpVersion::V6,
    }
}

/// An IPv6 multicast address of global scope.
fn global_group(last: u16) -> Ipv6Addr {
    Ipv6Addr::new(
        GLOBAL_MULTICAST,
        u16::MIN,
        u16::MIN,
        u16::MIN,
        u16::MIN,
        u16::MIN,
        u16::MIN,
        last,
    )
}

/// Joining a group with no source filtered out, or leaving it.
fn membership(
    group: IpAddr,
    interface: Option<InterfaceIndex>,
    joined: bool,
) -> UdpSocketSetting<'static> {
    fn of<A>(joined: bool) -> MulticastMembership<'static, A> {
        if joined {
            MulticastMembership::Joined(SourceFilter::Exclude(&[]))
        } else {
            MulticastMembership::Left
        }
    }
    UdpSocketSetting::Ip(match group {
        IpAddr::V4(group) => IpSocketSetting::MulticastV4(MulticastGroup {
            group,
            interface,
            membership: of(joined),
        }),
        IpAddr::V6(group) => IpSocketSetting::MulticastV6(MulticastGroup {
            group,
            interface,
            membership: of(joined),
        }),
    })
}

/// A header to a destination through a given interface.
fn header_through(
    from: SocketAddr,
    to: SocketAddr,
    interface: Option<InterfaceIndex>,
) -> IpHeaderFields<'static> {
    match route(UdpProtocol::Udp, from, to) {
        RouteSelectors::V4(mut route) => {
            route.interface = interface;
            ipv4_header(route)
        }
        RouteSelectors::V6(mut route) => {
            route.interface = interface;
            IpHeaderFields::V6(ipv6_header(route))
        }
    }
}

fn send<S: UdpSockets>(
    sockets: &S,
    socket: &S::Socket,
    protocol: UdpProtocol,
    from: SocketAddr,
    to: SocketAddr,
    payload: &[u8],
) {
    sockets
        .send_to(
            socket,
            header(protocol, from, to),
            plain_fields(),
            datagram(payload),
        )
        .expect("send a datagram");
}

/// The next packet or report for a socket, waited for up to the patience of a check.
fn receive<S: UdpSockets>(sockets: &S, socket: &S::Socket, into: &mut [u8]) -> UdpReceived {
    let start = Instant::now();
    loop {
        match sockets.receive(socket, into) {
            Err(HostError::WouldBlock) if start.elapsed() < PATIENCE => std::thread::yield_now(),
            other => return other.expect("receive"),
        }
    }
}

fn receive_packet<S: UdpSockets>(
    sockets: &S,
    socket: &S::Socket,
    into: &mut [u8],
) -> UdpReceivedPacket {
    match receive(sockets, socket, into) {
        UdpReceived::Packet(packet) => packet,
        UdpReceived::TransmitReport(report) => panic!("a packet, not {report:?}"),
    }
}

/// A received UDP or UDP-Lite packet's IP header fields, ports, checksum and payload.
struct Seen<'a> {
    source: SocketAddr,
    destination: SocketAddr,
    hop_limit: u8,
    dscp: IpDscp,
    ecn: IpEcn,
    flow_label: Option<Ipv6FlowLabel>,
    protocol: IpNumber,
    options: &'a [u8],
    checksum: u16,
    payload: &'a [u8],
}

/// Whether a checksum over a transport message and the pseudo-header of its IP header holds.
fn pseudo_header_checksum_holds(ip: &IpSlice<'_>, protocol: IpNumber, message: &[u8]) -> bool {
    let sum = match (ip.source_addr(), ip.destination_addr()) {
        (IpAddr::V4(source), IpAddr::V4(destination)) => Sum16BitWords::new()
            .add_4bytes(source.octets())
            .add_4bytes(destination.octets()),
        (IpAddr::V6(source), IpAddr::V6(destination)) => Sum16BitWords::new()
            .add_16bytes(source.octets())
            .add_16bytes(destination.octets()),
        _ => return false,
    };
    let length = u32::try_from(ip.payload().payload.len()).expect("a packet's length");
    sum.add_2bytes([u8::MIN, protocol.0])
        .add_4bytes(length.to_be_bytes())
        .add_slice(message)
        .ones_complement()
        == u16::MIN
}

/// Checks an IPv4 header's checksum.
fn check_ipv4_header(packet: &[u8]) {
    if let Ok(header) = etherparse::Ipv4HeaderSlice::from_slice(packet) {
        let whole = header.to_header();
        assert_eq!(
            whole.calc_header_checksum(),
            header.header_checksum(),
            "the IPv4 header checksum"
        );
    }
}

/// A packet's header fields, after checking its IPv4 header checksum and its UDP or UDP-Lite checksum, which
/// may be absent (zero) only for UDP.
fn seen(packet: &[u8]) -> Seen<'_> {
    let ip = IpSlice::from_slice(packet).expect("a whole IP packet");
    check_ipv4_header(packet);
    let (hop_limit, dscp, ecn, flow_label, options) = match &ip {
        IpSlice::Ipv4(_) => {
            let header = etherparse::Ipv4HeaderSlice::from_slice(packet).expect("an IPv4 header");
            (
                header.ttl(),
                header.dcp(),
                header.ecn(),
                None,
                header.options(),
            )
        }
        IpSlice::Ipv6(v6) => {
            let header = v6.header();
            (
                header.hop_limit(),
                header.dscp(),
                header.ecn(),
                Some(header.flow_label()),
                &[][..],
            )
        }
    };
    let transport = ip.payload().payload;
    let udp = UdpHeaderSlice::from_slice(transport).expect("a UDP header");
    let protocol = ip.payload_ip_number();
    // A UDP-Lite length field is its checksum's coverage, zero for the whole datagram.
    let covered = match usize::from(udp.length()) {
        0 => transport,
        coverage if protocol == IpNumber::UDP_LITE => &transport[..coverage.min(transport.len())],
        _ => transport,
    };
    assert!(
        (udp.checksum() == u16::MIN && protocol == IpNumber::UDP)
            || pseudo_header_checksum_holds(&ip, protocol, covered),
        "the checksum {:#06x} of a {protocol:?} datagram",
        udp.checksum()
    );
    Seen {
        source: SocketAddr::new(ip.source_addr(), udp.source_port()),
        destination: SocketAddr::new(ip.destination_addr(), udp.destination_port()),
        hop_limit,
        dscp,
        ecn,
        flow_label,
        protocol,
        options,
        checksum: udp.checksum(),
        payload: &transport[udp.slice().len()..],
    }
}

/// The port of a socket that was bound and then closed, so that nothing listens on it.
fn closed_port<S: UdpSockets>(sockets: &S, version: IpVersion) -> SocketAddr {
    let (socket, address) = bound(sockets, version, UdpProtocol::Udp);
    sockets.close(socket);
    address
}

fn buffer() -> Vec<u8> {
    vec![u8::MIN; usize::from(u16::MAX)]
}

/// Opens, binds and closes a socket of each version and protocol.
pub fn open_gives_sockets_of_each_version_and_protocol<S: UdpSockets>(sockets: S) {
    for version in [IpVersion::V4, IpVersion::V6] {
        for protocol in [UdpProtocol::Udp, UdpProtocol::Lite] {
            let (socket, address) = bound(&sockets, version, protocol);
            assert_eq!(address.ip(), loopback(version));
            sockets.close(socket);
        }
    }
}

/// A bind returns the address bound, with the port the kernel chose.
pub fn bind_returns_the_bound_address<S: UdpSockets>(sockets: S) {
    let (socket, address) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    assert_ne!(address.port(), u16::MIN);
    let again = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    assert_eq!(
        sockets.bind(&again, binding(address.ip(), address.port())),
        Err(HostError::Errno(linux_raw_sys::errno::EADDRINUSE as i32)),
    );
    sockets.close(socket);
}

/// Two sockets that share the address may bind one port; a socket that does not may not.
pub fn address_sharing_lets_two_sockets_bind_one_port<S: UdpSockets>(sockets: S) {
    let first = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    set(
        &sockets,
        &first,
        UdpSocketSetting::Port(PortSetting::AddressSharing(AddressSharing::Shared)),
    );
    let address = sockets
        .bind(&first, binding(loopback(IpVersion::V4), u16::MIN))
        .unwrap();
    let second = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    set(
        &sockets,
        &second,
        UdpSocketSetting::Port(PortSetting::AddressSharing(AddressSharing::Shared)),
    );
    assert_eq!(
        sockets.bind(&second, binding(address.ip(), address.port())),
        Ok(address)
    );
    let third = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    set(
        &sockets,
        &third,
        UdpSocketSetting::Port(PortSetting::AddressSharing(AddressSharing::Exclusive)),
    );
    assert!(
        sockets
            .bind(&third, binding(address.ip(), address.port()))
            .is_err()
    );
}

/// Two sockets that share the port may bind it, and datagrams to it reach one of them.
pub fn port_sharing_lets_two_sockets_bind_one_port<S: UdpSockets>(sockets: S) {
    let first = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    set(
        &sockets,
        &first,
        UdpSocketSetting::Port(PortSetting::PortSharing(PortSharing::Shared)),
    );
    let address = sockets
        .bind(&first, binding(loopback(IpVersion::V4), u16::MIN))
        .unwrap();
    let second = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    set(
        &sockets,
        &second,
        UdpSocketSetting::Port(PortSetting::PortSharing(PortSharing::Shared)),
    );
    assert_eq!(
        sockets.bind(&second, binding(address.ip(), address.port())),
        Ok(address)
    );
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    send(&sockets, &sender, UdpProtocol::Udp, from, address, PAYLOAD);
    let start = Instant::now();
    let mut into = buffer();
    let arrived = loop {
        match (
            sockets.receive(&first, &mut into),
            sockets.receive(&second, &mut into),
        ) {
            (Ok(_), _) | (_, Ok(_)) => break true,
            _ if start.elapsed() > PATIENCE => break false,
            _ => std::thread::yield_now(),
        }
    };
    assert!(arrived, "the datagram reaches one socket of the group");
}

/// A port the exit node chooses lies in the range and outside its reserved part.
pub fn bind_chosen_by_exit_node_avoids_reserved_ports<S: UdpSockets>(sockets: S) {
    let free = closed_port(&sockets, IpVersion::V4).port();
    let reserved = [PortRange {
        first: Port(free + 1),
        last: Port(free + RESERVED_WIDTH),
    }];
    let socket = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    let local = LocalBinding {
        address: BindAddress::V4(Ipv4Addr::LOCALHOST),
        port: LocalPort::ChosenByExitNode(PortChoice {
            range: PortRange {
                first: Port(free),
                last: reserved[0].last,
            },
            reserved: &reserved,
        }),
    };
    assert_eq!(
        sockets.bind(&socket, local).map(|address| address.port()),
        Ok(free)
    );
}

/// The exit node chooses from any range, also one a host would not choose from by itself.
pub fn bind_chosen_by_exit_node_takes_any_range<S: UdpSockets>(sockets: S) {
    let reserved = [PortRange {
        first: LOW_PORTS.first,
        last: Port(LOW_PORTS.first.0 + LOW_PORTS_RESERVED - 1),
    }];
    let socket = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    let local = LocalBinding {
        address: BindAddress::V4(Ipv4Addr::LOCALHOST),
        port: LocalPort::ChosenByExitNode(PortChoice {
            range: LOW_PORTS,
            reserved: &reserved,
        }),
    };
    let port = sockets.bind(&socket, local).unwrap().port();
    assert!(
        (reserved[0].last.0 + 1..=LOW_PORTS.last.0).contains(&port),
        "port {port}"
    );
}

/// A datagram arrives whole from its IPv4 header, with the time to live, DSCP and ECN it was sent with.
pub fn a_datagram_arrives_with_its_ipv4_header<S: UdpSockets>(sockets: S) {
    for protocol in [UdpProtocol::Udp, UdpProtocol::Lite] {
        let (receiver, to) = bound(&sockets, IpVersion::V4, protocol);
        let (sender, from) = bound(&sockets, IpVersion::V4, protocol);
        let mut route = ipv4_route(protocol, from, to);
        route.dscp = IpDscp::MAX;
        route.ecn = IpEcn::Ect1;
        let header = IpHeaderFields::V4(Ipv4HeaderFields {
            route,
            time_to_live: Some(wcontainers_internet_protocol::HopLimit(u8::MAX)),
            path_mtu_discovery: PathMtuDiscovery::Want,
        });
        sockets
            .send_to(&sender, header, plain_fields(), datagram(PAYLOAD))
            .unwrap();
        let mut into = buffer();
        let packet = receive_packet(&sockets, &receiver, &mut into);
        let seen = seen(&into[..packet.fields.length]);
        assert_eq!((seen.source, seen.destination), (from, to));
        assert_ne!(seen.checksum, u16::MIN, "the checksum the sender computed");
        assert_eq!(
            (seen.hop_limit, seen.dscp, seen.ecn),
            (u8::MAX, IpDscp::MAX, IpEcn::Ect1)
        );
        assert_eq!(
            (seen.protocol, seen.payload),
            (protocol_number(protocol), PAYLOAD)
        );
        assert_ne!(packet.fields.interface, InterfaceIndex(u32::MIN));
        assert_eq!(packet.segment_size, None);
    }
}

/// A datagram arrives whole from its IPv6 header, with the hop limit, traffic class and flow label it was sent with.
pub fn a_datagram_arrives_with_its_ipv6_header<S: UdpSockets>(sockets: S) {
    for protocol in [UdpProtocol::Udp, UdpProtocol::Lite] {
        let (receiver, to) = bound(&sockets, IpVersion::V6, protocol);
        let (sender, from) = bound(&sockets, IpVersion::V6, protocol);
        let mut fields = ipv6_header(ipv6_route(protocol, from, to));
        fields.route.dscp = IpDscp::MAX;
        fields.route.ecn = IpEcn::Ect0;
        fields.route.flow_label = Ipv6FlowLabel::try_new(u32::from(u16::MAX)).unwrap();
        fields.hop_limit = Some(wcontainers_internet_protocol::HopLimit(u8::MAX));
        sockets
            .send_to(
                &sender,
                IpHeaderFields::V6(fields.clone()),
                plain_fields(),
                datagram(PAYLOAD),
            )
            .unwrap();
        let mut into = buffer();
        let packet = receive_packet(&sockets, &receiver, &mut into);
        let seen = seen(&into[..packet.fields.length]);
        assert_eq!((seen.source, seen.destination), (from, to));
        assert_ne!(seen.checksum, u16::MIN, "the checksum the sender computed");
        assert_eq!(
            (seen.hop_limit, seen.dscp, seen.ecn),
            (u8::MAX, IpDscp::MAX, IpEcn::Ect0)
        );
        assert_eq!(seen.flow_label, Some(fields.route.flow_label));
        assert_eq!(
            (seen.protocol, seen.payload),
            (protocol_number(protocol), PAYLOAD)
        );
    }
}

/// IPv4 options sent with a datagram arrive in its header.
pub fn ipv4_options_arrive_in_the_header<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    // Three no-operation options and the end of the list.
    let options = Ipv4Options::try_from(&[1, 1, 1, 0][..]).unwrap();
    let mut route = ipv4_route(UdpProtocol::Udp, from, to);
    route.options = &options;
    sockets
        .send_to(
            &sender,
            ipv4_header(route),
            plain_fields(),
            datagram(PAYLOAD),
        )
        .unwrap();
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    let seen = seen(&into[..packet.fields.length]);
    assert_eq!((seen.options, seen.payload), (options.as_slice(), PAYLOAD));
}

/// A packet cut to a short buffer still reports its whole length.
pub fn a_short_buffer_keeps_the_whole_length<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    // Longer than the headers, so that a cut inside it leaves the buffer shorter than the payload, and of varied
    // bytes, as words of all ones leave a checksum as it was.
    let payload: Vec<u8> = (0..usize::from(ROUTE_MTU))
        .map(|place| place as u8)
        .collect();
    send(&sockets, &sender, UdpProtocol::Udp, from, to, &payload);
    let mut whole = buffer();
    let length = receive_packet(&sockets, &receiver, &mut whole)
        .fields
        .length;
    let headers = length - payload.len();
    // Cut inside the UDP header, then inside the payload, where the checksum is still the whole datagram's.
    for cut in [
        headers - etherparse::UdpHeader::LEN / 2,
        length - payload.len() / 2,
    ] {
        send(&sockets, &sender, UdpProtocol::Udp, from, to, &payload);
        let mut short = vec![u8::MIN; cut];
        assert_eq!(
            receive_packet(&sockets, &receiver, &mut short)
                .fields
                .length,
            length
        );
        assert_eq!(short[..], whole[..cut], "cut to {cut} bytes");
    }
}

/// Receiving with nothing queued would block.
pub fn receive_with_nothing_queued_would_block<S: UdpSockets>(sockets: S) {
    let (socket, _) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    assert_eq!(
        sockets.receive(&socket, &mut buffer()),
        Err(HostError::WouldBlock)
    );
}

/// A connected socket returns its local address and takes datagrams from its peer only.
pub fn a_connected_socket_takes_only_its_peers_datagrams<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (peer, peer_address) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (stranger, stranger_address) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    assert_eq!(
        sockets.connect(&receiver, route(UdpProtocol::Udp, to, peer_address)),
        Ok(to)
    );
    send(
        &sockets,
        &stranger,
        UdpProtocol::Udp,
        stranger_address,
        to,
        FROM_A_STRANGER,
    );
    send(&sockets, &peer, UdpProtocol::Udp, peer_address, to, PAYLOAD);
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).payload, PAYLOAD);
    let unbound = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    let local = sockets
        .connect(
            &unbound,
            route(
                UdpProtocol::Udp,
                SocketAddr::new(to.ip(), u16::MIN),
                peer_address,
            ),
        )
        .unwrap();
    assert_ne!(local.port(), u16::MIN);
}

/// A connected socket sends to its peer, and disconnecting keeps the address and port it was bound to and takes
/// anyone's datagrams again; a port the kernel chose at bind is given up.
pub fn disconnect_keeps_the_binding<S: UdpSockets>(sockets: S) {
    let local = closed_port(&sockets, IpVersion::V4);
    let socket = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    sockets
        .bind(&socket, binding(local.ip(), local.port()))
        .unwrap();
    let (peer, peer_address) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (stranger, stranger_address) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    sockets
        .connect(&socket, route(UdpProtocol::Udp, local, peer_address))
        .unwrap();
    send(
        &sockets,
        &socket,
        UdpProtocol::Udp,
        local,
        peer_address,
        PAYLOAD,
    );
    let mut into = buffer();
    let packet = receive_packet(&sockets, &peer, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).source, local);
    assert_eq!(sockets.disconnect(&socket), Ok(UdpLocalEnd::Bound(local)));
    send(
        &sockets,
        &stranger,
        UdpProtocol::Udp,
        stranger_address,
        local,
        PAYLOAD,
    );
    let packet = receive_packet(&sockets, &socket, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).source, stranger_address);
    let (chosen, address) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    sockets
        .connect(&chosen, route(UdpProtocol::Udp, address, peer_address))
        .unwrap();
    assert_eq!(
        sockets.disconnect(&chosen),
        Ok(UdpLocalEnd::Bound(SocketAddr::new(address.ip(), u16::MIN)))
    );
    let unbound = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    assert_eq!(sockets.disconnect(&unbound), Ok(UdpLocalEnd::Unbound));
}

/// After a disconnect a send to the former peer arrives, so it names the address, which Linux needs on a socket with no peer.
pub fn a_send_after_a_disconnect_carries_the_former_peers_address<S: UdpSockets>(sockets: S) {
    for version in [IpVersion::V4, IpVersion::V6] {
        let (socket, local) = bound(&sockets, version, UdpProtocol::Udp);
        let (peer, peer_address) = bound(&sockets, version, UdpProtocol::Udp);
        sockets
            .connect(&socket, route(UdpProtocol::Udp, local, peer_address))
            .unwrap();
        sockets.disconnect(&socket).unwrap();
        send(
            &sockets,
            &socket,
            UdpProtocol::Udp,
            local,
            peer_address,
            PAYLOAD,
        );
        let mut into = buffer();
        let packet = receive_packet(&sockets, &peer, &mut into);
        assert_eq!(
            seen(&into[..packet.fields.length]).payload,
            PAYLOAD,
            "{version:?}"
        );
    }
}

/// A UDP-Lite datagram whose checksum covers less than the receiver's minimum is dropped; one that covers enough
/// arrives, and with no minimum any coverage does.
pub fn udp_lite_coverage_below_the_minimum_is_dropped<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Lite);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Lite);
    let header_length = UdpHeaderSlice::from_slice(&[u8::MIN; etherparse::UdpHeader::LEN])
        .unwrap()
        .slice()
        .len() as u16;
    let minimum = NonZeroU16::new(header_length + 2).unwrap();
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::MinimumChecksumCoverage(Some(ChecksumCoverage(minimum))),
    );
    let short = NonZeroU16::new(header_length).unwrap();
    let send_covered = |coverage, payload| {
        let lite = UdpDatagram {
            checksum: UdpChecksum::Partial(ChecksumCoverage(coverage)),
            segment_size: None,
            payload,
        };
        sockets
            .send_to(
                &sender,
                header(UdpProtocol::Lite, from, to),
                plain_fields(),
                lite,
            )
            .unwrap();
    };
    send_covered(short, TOO_LITTLE_COVERED);
    send_covered(minimum, PAYLOAD);
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).payload, PAYLOAD);
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::MinimumChecksumCoverage(None),
    );
    send_covered(short, TOO_LITTLE_COVERED);
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(
        seen(&into[..packet.fields.length]).payload,
        TOO_LITTLE_COVERED
    );
}

/// The three segments of one send arrive as three datagrams of the segment size.
pub fn a_segment_size_makes_one_datagram_per_segment<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let size = SegmentSize(NonZeroU16::new(SEGMENTS[0].len() as u16).unwrap());
    let payload = SEGMENTS.concat();
    let send = UdpDatagram {
        checksum: UdpChecksum::Computed,
        segment_size: Some(size),
        payload: &payload,
    };
    sockets
        .send_to(
            &sender,
            header(UdpProtocol::Udp, from, to),
            plain_fields(),
            send,
        )
        .unwrap();
    let mut into = buffer();
    for &segment in SEGMENTS {
        let packet = receive_packet(&sockets, &receiver, &mut into);
        assert_eq!(seen(&into[..packet.fields.length]).payload, segment);
    }
}

/// A receiver that coalesces by segment size takes the segments of one send as one packet with that size.
pub fn coalescing_by_segment_size_gives_one_packet<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::ReceiveCoalescing(ReceiveCoalescing::BySegmentSize),
    );
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let size = SegmentSize(NonZeroU16::new(SEGMENTS[0].len() as u16).unwrap());
    let payload = SEGMENTS.concat();
    let send = UdpDatagram {
        checksum: UdpChecksum::Computed,
        segment_size: Some(size),
        payload: &payload,
    };
    sockets
        .send_to(
            &sender,
            header(UdpProtocol::Udp, from, to),
            plain_fields(),
            send,
        )
        .unwrap();
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(packet.segment_size, Some(size));
    assert_eq!(seen(&into[..packet.fields.length]).payload, payload);
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::ReceiveCoalescing(ReceiveCoalescing::Separate),
    );
    sockets
        .send_to(
            &sender,
            header(UdpProtocol::Udp, from, to),
            plain_fields(),
            send,
        )
        .unwrap();
    assert_eq!(
        receive_packet(&sockets, &receiver, &mut into).segment_size,
        None
    );
}

/// An IPv6 datagram without a checksum is dropped unless the receiver accepts such; an IPv4 one arrives.
pub fn a_zero_checksum_arrives_only_where_accepted<S: UdpSockets>(sockets: S) {
    let omitted = UdpDatagram {
        checksum: UdpChecksum::Omitted,
        segment_size: None,
        payload: WITHOUT_A_CHECKSUM,
    };
    let (receiver, to) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::ZeroChecksumReceipt(ZeroChecksumReceipt::Dropped),
    );
    sockets
        .send_to(
            &sender,
            header(UdpProtocol::Udp, from, to),
            plain_fields(),
            omitted,
        )
        .unwrap();
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).payload, PAYLOAD);
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::ZeroChecksumReceipt(ZeroChecksumReceipt::Accepted),
    );
    sockets
        .send_to(
            &sender,
            header(UdpProtocol::Udp, from, to),
            plain_fields(),
            omitted,
        )
        .unwrap();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).payload, omitted.payload);
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    sockets
        .send_to(
            &sender,
            header(UdpProtocol::Udp, from, to),
            plain_fields(),
            omitted,
        )
        .unwrap();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).payload, omitted.payload);
}

/// A datagram to a closed port comes back as an ICMP port unreachable message that quotes it.
pub fn a_closed_port_answers_with_an_icmp_error<S: UdpSockets>(sockets: S) {
    for version in [IpVersion::V4, IpVersion::V6] {
        let closed = closed_port(&sockets, version);
        let (sender, from) = bound(&sockets, version, UdpProtocol::Udp);
        send(&sockets, &sender, UdpProtocol::Udp, from, closed, PAYLOAD);
        let mut into = buffer();
        let packet = receive_packet(&sockets, &sender, &mut into);
        let ip = IpSlice::from_slice(&into[..packet.fields.length]).expect("a whole ICMP packet");
        let message = ip.payload().payload;
        let unreachable = match version {
            IpVersion::V4 => (
                IpNumber::ICMP,
                [
                    etherparse::icmpv4::TYPE_DEST_UNREACH,
                    etherparse::icmpv4::CODE_DST_UNREACH_PORT,
                ],
            ),
            IpVersion::V6 => (
                IpNumber::IPV6_ICMP,
                [
                    etherparse::icmpv6::TYPE_DST_UNREACH,
                    etherparse::icmpv6::CODE_DST_UNREACH_PORT,
                ],
            ),
        };
        assert_eq!(
            (ip.payload_ip_number(), [message[0], message[1]]),
            unreachable
        );
        check_ipv4_header(&into[..packet.fields.length]);
        let checksum_holds = match version {
            IpVersion::V4 => Sum16BitWords::new().add_slice(message).ones_complement() == u16::MIN,
            IpVersion::V6 => pseudo_header_checksum_holds(&ip, IpNumber::IPV6_ICMP, message),
        };
        assert!(checksum_holds, "the ICMP checksum");
        let quoted = seen(&message[etherparse::Icmpv6Header::MIN_LEN..]);
        assert_eq!((quoted.source, quoted.destination), (from, closed));
        assert_eq!(quoted.payload, PAYLOAD);
    }
}

/// An IPv6 datagram over the fragment MTU that may not be fragmented is too long for that MTU.
pub fn a_datagram_over_the_mtu_is_too_long<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    let mtu = MaximumTransmissionUnit(u32::from(u16::MAX / 2));
    let mut fields = ipv6_header(ipv6_route(UdpProtocol::Udp, from, to));
    fields.fragment_mtu = Some(mtu);
    fields.local_fragmentation = LocalFragmentation::Forbidden;
    fields.path_mtu_discovery = PathMtuDiscovery::Do;
    let payload = vec![u8::MIN; mtu.0 as usize];
    assert_eq!(
        sockets.send_to(
            &sender,
            IpHeaderFields::V6(fields.clone()),
            plain_fields(),
            datagram(&payload)
        ),
        Err(SendError::MessageTooLong {
            maximum_transmission_unit: mtu
        }),
    );
    sockets
        .send_to(
            &sender,
            IpHeaderFields::V6(fields),
            plain_fields(),
            datagram(PAYLOAD),
        )
        .unwrap();
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).payload, PAYLOAD);
}

/// A send that asks for transmit reports gets one per stage asked for, with its key and, if asked, its packet.
pub fn transmit_reports_carry_the_key_of_their_send<S: UdpSockets>(sockets: S) {
    let (_receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    for (key, packet) in [
        (TransmitReportKey(u32::MAX), Report::Requested),
        (TransmitReportKey(u32::MIN), Report::NotRequested),
    ] {
        let mut fields = plain_fields();
        fields.transmit_reports.key = key;
        fields.transmit_reports.scheduled = Report::Requested;
        fields.transmit_reports.software = Report::Requested;
        fields.transmit_reports.packet = packet;
        sockets
            .send_to(
                &sender,
                header(UdpProtocol::Udp, from, to),
                fields,
                datagram(PAYLOAD),
            )
            .unwrap();
        let mut into = buffer();
        for stage in [TransmitStage::Scheduled, TransmitStage::Sent] {
            let UdpReceived::TransmitReport(report) = receive(&sockets, &sender, &mut into) else {
                panic!("a transmit report");
            };
            assert_eq!(report.key, key);
            let TransmitEvent::Timestamp {
                stage: reported,
                source,
                ..
            } = report.event
            else {
                panic!("a timestamp")
            };
            assert_eq!((reported, source), (stage, TimestampSource::Software));
            assert_eq!(report.packet_length.is_some(), packet == Report::Requested);
        }
    }
}

/// Receive timestamps come with each packet while they are set, and not otherwise.
pub fn receive_timestamps_come_only_when_set<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let mut into = buffer();
    for software in [Report::Requested, Report::NotRequested] {
        let timestamping = Timestamping {
            receive_software: software,
            receive_hardware: Report::NotRequested,
            bound_clock: None,
        };
        set(
            &sockets,
            &receiver,
            UdpSocketSetting::Ip(IpSocketSetting::Timestamping(timestamping)),
        );
        send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
        let packet = receive_packet(&sockets, &receiver, &mut into);
        assert_eq!(
            packet.fields.timestamps.software.is_some(),
            software == Report::Requested
        );
        assert_eq!(packet.fields.timestamps.hardware, None);
    }
}

/// A receive buffer too small for what was sent drops datagrams, and the packets after the drops count them.
pub fn a_small_receive_buffer_counts_its_drops<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Ip(IpSocketSetting::ReceiveBuffer(ByteCount(usize::MIN))),
    );
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let payload = vec![u8::MIN; usize::from(u8::MAX)];
    let sent = usize::from(u8::MAX);
    for _ in 0..sent {
        send(&sockets, &sender, UdpProtocol::Udp, from, to, &payload);
    }
    let mut received = usize::MIN;
    let mut into = buffer();
    while let Ok(UdpReceived::Packet(_)) = sockets.receive(&receiver, &mut into) {
        received += 1;
    }
    assert!(received < sent, "{received} of {sent} received");
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    let dropped = receive_packet(&sockets, &receiver, &mut into)
        .fields
        .drops_since_previous
        .0 as usize;
    assert_eq!(dropped, sent - received);
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    assert_eq!(
        receive_packet(&sockets, &receiver, &mut into)
            .fields
            .drops_since_previous
            .0,
        u32::MIN
    );
}

/// The send buffer, pacing rate and IPv6 multicast settings are taken, and sending goes on.
pub fn sending_settings_are_taken<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    set(
        &sockets,
        &sender,
        UdpSocketSetting::Ip(IpSocketSetting::SendBuffer(ByteCount(usize::from(
            u16::MAX,
        )))),
    );
    set(
        &sockets,
        &sender,
        UdpSocketSetting::Ip(IpSocketSetting::MaximumPacingRate(Some(
            wcontainers_internet_protocol::BytesPerSecond(u64::from(u32::MAX)),
        ))),
    );
    set(
        &sockets,
        &sender,
        UdpSocketSetting::Ip(IpSocketSetting::MaximumPacingRate(None)),
    );
    set(
        &sockets,
        &sender,
        UdpSocketSetting::Ip(IpSocketSetting::MulticastAllGroups {
            version: IpVersion::V4,
            enabled: false,
        }),
    );
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).payload, PAYLOAD);
    assert_eq!(
        sockets.count_unsent_bytes(&sender),
        Ok(ByteCount(usize::MIN))
    );
}

/// A member of a multicast group takes its datagrams through the loopback of its sender while that is on, from
/// the sources its filter lets through, until it leaves.
///
/// Runs over the interface the multicast route chooses: Linux's loopback interface brings back every datagram
/// sent over it, looped or not.
pub fn multicast_membership_takes_the_groups_datagrams<S: UdpSockets>(sockets: S) {
    // The last group of the local network control block, which no interface joins by itself.
    let group = Ipv4Addr::from_bits(linux_raw_sys::net::INADDR_MAX_LOCAL_GROUP);
    let receiver = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    let port = sockets
        .bind(&receiver, binding(unspecified(IpVersion::V4), u16::MIN))
        .unwrap()
        .port();
    let join = |filter| {
        set(
            &sockets,
            &receiver,
            UdpSocketSetting::Ip(IpSocketSetting::MulticastV4(MulticastGroup {
                group,
                interface: None,
                membership: MulticastMembership::Joined(filter),
            })),
        );
    };
    join(SourceFilter::Exclude(&[]));
    let sender = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    let from = sockets
        .bind(&sender, binding(unspecified(IpVersion::V4), u16::MIN))
        .unwrap();
    let to = SocketAddr::new(IpAddr::V4(group), port);
    let send_one = || {
        sockets
            .send_to(
                &sender,
                header_through(from, to, None),
                plain_fields(),
                datagram(PAYLOAD),
            )
            .unwrap();
    };
    let looped = |on| {
        set(
            &sockets,
            &sender,
            UdpSocketSetting::Ip(IpSocketSetting::MulticastLoopback(on)),
        );
    };
    looped(true);
    send_one();
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    let arrived = seen(&into[..packet.fields.length]);
    assert_eq!(arrived.destination, to);
    let IpAddr::V4(source) = arrived.source.ip() else {
        unreachable!("an IPv4 datagram's source")
    };
    looped(false);
    send_one();
    assert!(!arrives(&sockets, &receiver), "a datagram not looped back");
    looped(true);
    let only_source = [source];
    let only_another = [Ipv4Addr::from_bits(source.to_bits() + 1)];
    for (filter, takes) in [
        (SourceFilter::Include(&only_source[..]), true),
        (SourceFilter::Include(&only_another[..]), false),
        (SourceFilter::Exclude(&only_source[..]), false),
        (SourceFilter::Exclude(&only_another[..]), true),
    ] {
        join(filter);
        send_one();
        assert_eq!(arrives(&sockets, &receiver), takes, "{filter:?}");
    }
    set(
        &sockets,
        &receiver,
        membership(IpAddr::V4(group), None, false),
    );
    send_one();
    assert!(!arrives(&sockets, &receiver), "a datagram after leaving");
}

/// A socket on a group's port takes the datagrams of a group only another socket joined while all-groups is on,
/// over IPv4 and IPv6.
pub fn all_groups_decides_whether_other_sockets_memberships_count<S: UdpSockets>(sockets: S) {
    let ipv4 = (
        IpAddr::V4(Ipv4Addr::from_bits(
            linux_raw_sys::net::INADDR_MAX_LOCAL_GROUP,
        )),
        Some(loopback_interface(&sockets)),
        loopback(IpVersion::V4),
    );
    // IPv6 multicast is not looped back from the loopback address (see ipv6_memberships_take_their_datagrams).
    let ipv6 = (
        IpAddr::V6(global_group(u16::MAX)),
        None,
        unspecified(IpVersion::V6),
    );
    for (group, interface, sender_address) in [ipv4, ipv6] {
        let version = version_of(group);
        let sharing = UdpSocketSetting::Port(PortSetting::AddressSharing(AddressSharing::Shared));
        let member = sockets.open(version, UdpProtocol::Udp, OWNER).unwrap();
        set(&sockets, &member, sharing);
        let port = sockets
            .bind(&member, binding(unspecified(version), u16::MIN))
            .unwrap()
            .port();
        set(&sockets, &member, membership(group, interface, true));
        let bystander = sockets.open(version, UdpProtocol::Udp, OWNER).unwrap();
        set(&sockets, &bystander, sharing);
        sockets
            .bind(&bystander, binding(unspecified(version), port))
            .unwrap();
        let sender = sockets.open(version, UdpProtocol::Udp, OWNER).unwrap();
        let from = sockets
            .bind(&sender, binding(sender_address, u16::MIN))
            .unwrap();
        let to = SocketAddr::new(group, port);
        for enabled in [true, false] {
            set(
                &sockets,
                &bystander,
                UdpSocketSetting::Ip(IpSocketSetting::MulticastAllGroups { version, enabled }),
            );
            sockets
                .send_to(
                    &sender,
                    header_through(from, to, interface),
                    plain_fields(),
                    datagram(PAYLOAD),
                )
                .unwrap();
            assert!(arrives(&sockets, &member), "{version:?} to the member");
            assert_eq!(
                arrives(&sockets, &bystander),
                enabled,
                "{version:?} to the bystander with all-groups {enabled}"
            );
        }
    }
}

/// The interface a datagram over the IPv4 loopback arrives on.
fn loopback_interface<S: UdpSockets>(sockets: &S) -> InterfaceIndex {
    let (receiver, to) = bound(sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(sockets, IpVersion::V4, UdpProtocol::Udp);
    send(sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    receive_packet(sockets, &receiver, &mut buffer())
        .fields
        .interface
}

/// The interface the IPv4 multicast route chooses, from a looped datagram to a group joined on no interface given.
fn multicast_interface<S: UdpSockets>(sockets: &S) -> InterfaceIndex {
    let group = IpAddr::V4(Ipv4Addr::from_bits(
        linux_raw_sys::net::INADDR_MAX_LOCAL_GROUP,
    ));
    let receiver = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    let port = sockets
        .bind(&receiver, binding(unspecified(IpVersion::V4), u16::MIN))
        .unwrap()
        .port();
    set(sockets, &receiver, membership(group, None, true));
    let sender = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    let from = sockets
        .bind(&sender, binding(unspecified(IpVersion::V4), u16::MIN))
        .unwrap();
    set(
        sockets,
        &sender,
        UdpSocketSetting::Ip(IpSocketSetting::MulticastLoopback(true)),
    );
    let to = SocketAddr::new(group, port);
    send(sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    receive_packet(sockets, &receiver, &mut buffer())
        .fields
        .interface
}

/// A socket bound to an interface still sends over it and takes only what arrives on it; one bound to no
/// interface that exists cannot be.
pub fn a_bound_interface_is_taken<S: UdpSockets>(sockets: S) {
    let interface = loopback_interface(&sockets);
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    set(
        &sockets,
        &sender,
        UdpSocketSetting::Port(PortSetting::BoundInterface(Some(interface))),
    );
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    assert_eq!(
        receive_packet(&sockets, &receiver, &mut buffer())
            .fields
            .interface,
        interface
    );
    set(
        &sockets,
        &sender,
        UdpSocketSetting::Port(PortSetting::BoundInterface(None)),
    );
    let elsewhere = multicast_interface(&sockets);
    assert_ne!(elsewhere, interface);
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Port(PortSetting::BoundInterface(Some(elsewhere))),
    );
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    assert!(
        !arrives(&sockets, &receiver),
        "a socket bound to another interface took a loopback datagram"
    );
    // No socket took it, so the sender is told the port is closed.
    receive_packet(&sockets, &sender, &mut buffer());
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Port(PortSetting::BoundInterface(None)),
    );
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    assert!(arrives(&sockets, &receiver));
    assert!(
        sockets
            .configure(
                &sender,
                UdpSocketSetting::Port(PortSetting::BoundInterface(Some(InterfaceIndex(u32::MAX))))
            )
            .is_err()
    );
}

/// An address the host does not have can be bound only while nonlocal addresses are bindable.
pub fn nonlocal_addresses_can_be_bound_when_allowed<S: UdpSockets>(sockets: S) {
    // A unicast address outside the loopback network, every bit of the loopback address flipped.
    let absent = IpAddr::V4(!Ipv4Addr::LOCALHOST);
    let refused = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    set(
        &sockets,
        &refused,
        UdpSocketSetting::Port(PortSetting::NonlocalAddresses(NonlocalAddresses::Refused)),
    );
    assert!(sockets.bind(&refused, binding(absent, u16::MIN)).is_err());
    let allowed = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    set(
        &sockets,
        &allowed,
        UdpSocketSetting::Port(PortSetting::NonlocalAddresses(NonlocalAddresses::Bindable)),
    );
    assert_eq!(
        sockets
            .bind(&allowed, binding(absent, u16::MIN))
            .map(|address| address.ip()),
        Ok(absent)
    );
}

/// Up to two processors this thread may run on, from which the checks of the incoming processor take theirs.
fn two_processors() -> Vec<usize> {
    let allowed = rustix::thread::sched_getaffinity(None).expect("this thread's processors");
    (0..rustix::thread::CpuSet::MAX_CPU)
        .filter(|&processor| allowed.is_set(processor))
        .take(2)
        .collect()
}

/// Runs `work` on this thread held to one processor, then lets the thread run where it could before.
fn on_processor(processor: usize, work: impl FnOnce()) {
    let allowed = rustix::thread::sched_getaffinity(None).expect("this thread's processors");
    let mut only = rustix::thread::CpuSet::new();
    only.set(processor);
    rustix::thread::sched_setaffinity(None, &only).expect("hold this thread to one processor");
    work();
    rustix::thread::sched_setaffinity(None, &allowed).expect("free this thread again");
}

fn processor_index(processor: usize) -> ProcessorIndex {
    ProcessorIndex(u32::try_from(processor).expect("a processor's number"))
}

/// The incoming processor set is the one read back, and none once unset; a connected socket's is then the
/// processor that last took in a packet for it, over loopback the one its sender ran on.
pub fn the_incoming_processor_is_read_back<S: UdpSockets>(sockets: S) {
    let (socket, local) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let processors = two_processors();
    for &processor in &processors {
        let processor = processor_index(processor);
        set(
            &sockets,
            &socket,
            UdpSocketSetting::Port(PortSetting::IncomingProcessor(Some(processor))),
        );
        assert_eq!(
            sockets.read_incoming_processor(&socket),
            Ok(Some(processor))
        );
    }
    set(
        &sockets,
        &socket,
        UdpSocketSetting::Port(PortSetting::IncomingProcessor(None)),
    );
    assert_eq!(sockets.read_incoming_processor(&socket), Ok(None));
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    sockets
        .connect(&socket, route(UdpProtocol::Udp, local, from))
        .expect("connect");
    let mut into = buffer();
    for &processor in processors.iter().rev() {
        on_processor(processor, || {
            send(&sockets, &sender, UdpProtocol::Udp, from, local, PAYLOAD);
        });
        receive_packet(&sockets, &socket, &mut into);
        assert_eq!(
            sockets.read_incoming_processor(&socket),
            Ok(Some(processor_index(processor)))
        );
    }
}

/// A connected socket given negative route advice keeps sending to its peer.
pub fn negative_route_advice_keeps_a_socket_sending<S: UdpSockets>(sockets: S) {
    let (socket, local) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    let (peer, peer_address) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    assert_eq!(sockets.give_negative_route_advice(&socket), Ok(()));
    sockets
        .connect(&socket, route(UdpProtocol::Udp, local, peer_address))
        .unwrap();
    assert_eq!(sockets.give_negative_route_advice(&socket), Ok(()));
    send(
        &sockets,
        &socket,
        UdpProtocol::Udp,
        local,
        peer_address,
        PAYLOAD,
    );
    let mut into = buffer();
    let packet = receive_packet(&sockets, &peer, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).payload, PAYLOAD);
}

/// A send's source, mark, priority, neighbour confirmation, link scope and transmit time are taken, and the
/// receiver sees the mark, priority and the address the datagram was sent to.
pub fn send_fields_reach_the_receiver<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let sender = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    let from = sockets
        .bind(
            &sender,
            binding(IpAddr::V4(Ipv4Addr::UNSPECIFIED), u16::MIN),
        )
        .unwrap();
    let mut route = ipv4_route(UdpProtocol::Udp, from, to);
    let IpAddr::V4(source) = to.ip() else {
        panic!("an IPv4 source")
    };
    route.source = Some(source);
    route.mark = PacketMark(u32::MAX);
    route.scope = RouteScope::Link;
    let mut fields = plain_fields();
    fields.priority = PacketPriority(u32::from(u8::MAX));
    fields.neighbour = NeighbourConfirmation::Confirmed;
    fields.transmit_time = Some(wcontainers_internet_protocol::TransmitTime {
        clock: wcontainers_internet_protocol::TransmitClock::Monotonic,
        at: Duration::ZERO,
        mode: wcontainers_internet_protocol::TransmitTimeMode::Strict,
        errors: Report::NotRequested,
    });
    sockets
        .send_to(&sender, ipv4_header(route), fields, datagram(PAYLOAD))
        .unwrap();
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(
        seen(&into[..packet.fields.length]).source.ip(),
        IpAddr::V4(source)
    );
    assert_eq!(
        (packet.fields.mark, packet.fields.priority),
        (route.mark, fields.priority)
    );
    assert_eq!(packet.fields.specific_destination, Some(source));
}

/// IPv6 hop-by-hop and destination options sent with a datagram arrive in its header chain.
pub fn ipv6_options_arrive_in_the_header<S: UdpSockets>(sockets: S) {
    // Options headers of Pad1 options only, the shortest an options header is.
    let padding = [u8::MIN; etherparse::Ipv6RawExtHeader::MIN_LEN];
    let options = etherparse::Ipv6RawExtHeaderSlice::from_slice(&padding).unwrap();
    for hop_by_hop in [false, true] {
        let (receiver, to) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
        let (sender, from) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
        let mut fields = ipv6_header(ipv6_route(UdpProtocol::Udp, from, to));
        let first = if hop_by_hop {
            fields.hop_by_hop_options = Some(options.clone());
            IpNumber::IPV6_HEADER_HOP_BY_HOP
        } else {
            fields.destination_options = Some(options.clone());
            IpNumber::IPV6_DESTINATION_OPTIONS
        };
        sockets
            .send_to(
                &sender,
                IpHeaderFields::V6(fields),
                plain_fields(),
                datagram(PAYLOAD),
            )
            .unwrap();
        let mut into = buffer();
        let packet = receive_packet(&sockets, &receiver, &mut into);
        let header =
            etherparse::Ipv6HeaderSlice::from_slice(&into[..packet.fields.length]).unwrap();
        assert_eq!(header.next_header(), first);
        assert_eq!(seen(&into[..packet.fields.length]).payload, PAYLOAD);
    }
}

/// A datagram to the loopback network's broadcast address is refused without permission and arrives with it.
pub fn broadcast_needs_its_permission<S: UdpSockets>(sockets: S) {
    let receiver = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    let port = sockets
        .bind(
            &receiver,
            binding(IpAddr::V4(Ipv4Addr::UNSPECIFIED), u16::MIN),
        )
        .unwrap()
        .port();
    // 127.255.255.255, the broadcast address of the loopback network 127.0.0.0/8.
    let broadcast = Ipv4Addr::from_bits(Ipv4Addr::LOCALHOST.to_bits() | (u32::MAX >> u8::BITS));
    let to = SocketAddr::new(IpAddr::V4(broadcast), port);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let refused = sockets.send_to(
        &sender,
        header(UdpProtocol::Udp, from, to),
        plain_fields(),
        datagram(PAYLOAD),
    );
    assert_eq!(
        refused,
        Err(SendError::Host(HostError::Errno(
            linux_raw_sys::errno::EACCES as i32
        )))
    );
    let mut route = ipv4_route(UdpProtocol::Udp, from, to);
    route.broadcast = BroadcastPermission::Allowed;
    sockets
        .send_to(
            &sender,
            ipv4_header(route),
            plain_fields(),
            datagram(PAYLOAD),
        )
        .unwrap();
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).destination, to);
}

/// What sending a datagram longer than a route's MTU came to.
#[derive(Debug, PartialEq, Eq)]
enum OverTheMtu {
    TooLong(MaximumTransmissionUnit),
    Fragmented,
    Whole,
}

/// Each mode of path MTU discovery treats a datagram longer than its route's MTU as Linux does: fragmented
/// (Want, Dont), too long (Do), or sent whole by the interface's larger MTU (Probe, Interface, Omit), over IPv4
/// and IPv6.
pub fn path_mtu_discovery_modes_take_the_routes_mtu<S: UdpSockets>(sockets: S) {
    let mtu = MaximumTransmissionUnit(u32::from(ROUTE_MTU));
    let payload = vec![u8::MIN; usize::from(ROUTE_MTU) * 2];
    for narrow in [IpAddr::V4(NARROW_V4), IpAddr::V6(NARROW_V6)] {
        let version = version_of(narrow);
        let receiver = sockets.open(version, UdpProtocol::Udp, OWNER).unwrap();
        let to = sockets.bind(&receiver, binding(narrow, u16::MIN)).unwrap();
        let (sender, from) = bound(&sockets, version, UdpProtocol::Udp);
        let mut into = buffer();
        for (mode, expected) in [
            (PathMtuDiscovery::Want, OverTheMtu::Fragmented),
            (PathMtuDiscovery::Dont, OverTheMtu::Fragmented),
            (PathMtuDiscovery::Do, OverTheMtu::TooLong(mtu)),
            (PathMtuDiscovery::Probe, OverTheMtu::Whole),
            (PathMtuDiscovery::Interface, OverTheMtu::Whole),
            (PathMtuDiscovery::Omit, OverTheMtu::Whole),
        ] {
            let header = match header(UdpProtocol::Udp, from, to) {
                IpHeaderFields::V4(mut header) => {
                    header.path_mtu_discovery = mode;
                    IpHeaderFields::V4(header)
                }
                IpHeaderFields::V6(mut header) => {
                    header.path_mtu_discovery = mode;
                    IpHeaderFields::V6(header)
                }
            };
            let outcome = match sockets.send_to(&sender, header, plain_fields(), datagram(&payload))
            {
                Err(SendError::MessageTooLong {
                    maximum_transmission_unit,
                }) => OverTheMtu::TooLong(maximum_transmission_unit),
                Err(error) => panic!("{mode:?}: {error:?}"),
                Ok(()) => {
                    let packet = receive_packet(&sockets, &receiver, &mut into);
                    assert_eq!(seen(&into[..packet.fields.length]).payload, payload);
                    match packet.fields.largest_fragment {
                        // The largest fragment is the MTU less what a fragment's data cannot fill: it holds
                        // a multiple of 8 bytes.
                        Some(FragmentSize(size)) => {
                            assert!(
                                (ROUTE_MTU - FRAGMENT_UNIT..=ROUTE_MTU).contains(&size),
                                "{version:?} {mode:?}: largest fragment {size}"
                            );
                            OverTheMtu::Fragmented
                        }
                        None => OverTheMtu::Whole,
                    }
                }
            };
            assert_eq!(outcome, expected, "{version:?} {mode:?}");
        }
    }
}

/// A source the host lacks may be sent from only with the source check waived on a transparent socket, and
/// IPv6 source preferences are taken.
pub fn a_waived_source_check_sends_from_an_absent_address<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let sender = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    set(
        &sockets,
        &sender,
        UdpSocketSetting::Port(PortSetting::NonlocalAddresses(
            NonlocalAddresses::Transparent,
        )),
    );
    let absent = !Ipv4Addr::LOCALHOST;
    let from = sockets
        .bind(&sender, binding(IpAddr::V4(absent), u16::MIN))
        .unwrap();
    let mut route = ipv4_route(UdpProtocol::Udp, from, to);
    route.source = Some(absent);
    route.source_address_check = SourceAddressCheck::Waived;
    sockets
        .send_to(
            &sender,
            ipv4_header(route),
            plain_fields(),
            datagram(PAYLOAD),
        )
        .unwrap();
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).source, from);
    let (receiver, to) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    let mut route = ipv6_route(UdpProtocol::Udp, from, to);
    route.source_preferences = SourcePreferences {
        temporary: TemporaryAddressPreference::Public,
        mobile: MobileAddressPreference::CareOf,
    };
    sockets
        .send_to(
            &sender,
            IpHeaderFields::V6(ipv6_header(route)),
            plain_fields(),
            datagram(PAYLOAD),
        )
        .unwrap();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).source, from);
}

/// A port group's filter chooses the socket each datagram reaches: here always the second.
pub fn a_group_filter_chooses_the_socket<S: UdpSockets>(sockets: S) {
    let program = [FilterInstruction {
        operation: FilterOperation(
            (linux_raw_sys::if_tun::BPF_RET | linux_raw_sys::if_tun::BPF_K) as u16,
        ),
        jump_if_true: FilterJump(u8::MIN),
        jump_if_false: FilterJump(u8::MIN),
        operand: FilterOperand(1),
    }];
    let first = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    set(
        &sockets,
        &first,
        UdpSocketSetting::Port(PortSetting::PortSharing(PortSharing::Shared)),
    );
    let address = sockets
        .bind(&first, binding(loopback(IpVersion::V4), u16::MIN))
        .unwrap();
    let second = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    set(
        &sockets,
        &second,
        UdpSocketSetting::Port(PortSetting::PortSharing(PortSharing::Shared)),
    );
    sockets
        .bind(&second, binding(address.ip(), address.port()))
        .unwrap();
    set(
        &sockets,
        &first,
        UdpSocketSetting::Port(PortSetting::GroupFilter(Some(SocketFilter(&program)))),
    );
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let mut into = buffer();
    for _ in SEGMENTS {
        send(&sockets, &sender, UdpProtocol::Udp, from, address, PAYLOAD);
        receive_packet(&sockets, &second, &mut into);
    }
    assert_eq!(
        sockets.receive(&first, &mut into),
        Err(HostError::WouldBlock)
    );
    set(
        &sockets,
        &first,
        UdpSocketSetting::Port(PortSetting::GroupFilter(None)),
    );
    // Detached, the group chooses by each sender's addresses again, so some of many senders reach the first.
    for _ in 0..SENDERS {
        let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
        send(&sockets, &sender, UdpProtocol::Udp, from, address, PAYLOAD);
    }
    assert!(
        arrives(&sockets, &first),
        "a datagram reaches the first socket"
    );
}

/// An IPv6 multicast member takes the group's datagrams with all-groups off, joined on the interface the route
/// chooses or on one given, and a link-local anycast address joined on that interface is reachable.
///
/// Needs an interface with IPv6 multicast besides loopback: Linux never takes an IPv6 multicast datagram over
/// its loopback interface, drops a looped one whose source is the loopback address, and lets a host join only
/// an anycast address in a prefix of the interface.
pub fn ipv6_memberships_take_their_datagrams<S: UdpSockets>(sockets: S) {
    let group = global_group;
    let unspecified = binding(IpAddr::V6(Ipv6Addr::UNSPECIFIED), u16::MIN);
    let receiver = sockets
        .open(IpVersion::V6, UdpProtocol::Udp, OWNER)
        .unwrap();
    let port = sockets.bind(&receiver, unspecified).unwrap().port();
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Ip(IpSocketSetting::MulticastAllGroups {
            version: IpVersion::V6,
            enabled: false,
        }),
    );
    let sender = sockets
        .open(IpVersion::V6, UdpProtocol::Udp, OWNER)
        .unwrap();
    let from = sockets.bind(&sender, unspecified).unwrap();
    set(
        &sockets,
        &sender,
        UdpSocketSetting::Ip(IpSocketSetting::MulticastLoopback(true)),
    );
    let mut into = buffer();
    let mut take = |group, interface| {
        let joined = MulticastGroup {
            group,
            interface,
            membership: MulticastMembership::Joined(SourceFilter::Exclude(&[])),
        };
        set(
            &sockets,
            &receiver,
            UdpSocketSetting::Ip(IpSocketSetting::MulticastV6(joined)),
        );
        let to = SocketAddr::new(IpAddr::V6(group), port);
        let mut route = ipv6_route(UdpProtocol::Udp, from, to);
        route.interface = interface;
        sockets
            .send_to(
                &sender,
                IpHeaderFields::V6(ipv6_header(route)),
                plain_fields(),
                datagram(PAYLOAD),
            )
            .unwrap();
        let packet = receive_packet(&sockets, &receiver, &mut into);
        let arrived = seen(&into[..packet.fields.length]);
        assert_eq!((arrived.destination, arrived.payload), (to, PAYLOAD));
        packet.fields.interface
    };
    let interface = take(group(u16::MAX), None);
    assert_eq!(take(group(u16::MAX - 1), Some(interface)), interface);
    let send_to_group = |group| {
        let to = SocketAddr::new(IpAddr::V6(group), port);
        sockets
            .send_to(
                &sender,
                header_through(from, to, Some(interface)),
                plain_fields(),
                datagram(PAYLOAD),
            )
            .unwrap();
    };
    set(
        &sockets,
        &receiver,
        membership(IpAddr::V6(group(u16::MAX)), None, false),
    );
    send_to_group(group(u16::MAX));
    assert!(!arrives(&sockets, &receiver), "a datagram after leaving");
    let looped = |on| {
        set(
            &sockets,
            &sender,
            UdpSocketSetting::Ip(IpSocketSetting::MulticastLoopback(on)),
        );
    };
    looped(false);
    send_to_group(group(u16::MAX - 1));
    assert!(!arrives(&sockets, &receiver), "a datagram not looped back");
    looped(true);
    send_to_group(group(u16::MAX - 1));
    assert!(arrives(&sockets, &receiver), "a datagram looped back again");
    // fe80::ffff, a link-local address, in the prefix of every interface with IPv6.
    let anycast = Ipv6Addr::new(
        LINK_LOCAL,
        u16::MIN,
        u16::MIN,
        u16::MIN,
        u16::MIN,
        u16::MIN,
        u16::MIN,
        u16::MAX,
    );
    let member = |membership| AnycastAddress {
        address: anycast,
        interface: Some(interface),
        membership,
    };
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Ip(IpSocketSetting::Anycast(member(Membership::Joined))),
    );
    let to = SocketAddr::new(IpAddr::V6(anycast), port);
    let mut route = ipv6_route(UdpProtocol::Udp, from, to);
    route.interface = Some(interface);
    sockets
        .send_to(
            &sender,
            IpHeaderFields::V6(ipv6_header(route.clone())),
            plain_fields(),
            datagram(PAYLOAD),
        )
        .unwrap();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).destination, to);
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Ip(IpSocketSetting::Anycast(member(Membership::Left))),
    );
    sockets
        .send_to(
            &sender,
            IpHeaderFields::V6(ipv6_header(route)),
            plain_fields(),
            datagram(PAYLOAD),
        )
        .unwrap();
    assert!(
        !arrives(&sockets, &receiver),
        "a datagram to an anycast address left"
    );
}

/// A socket's owner is the user its datagrams are routed for: the caller's network prohibits user 1000's
/// datagrams to an address it routes for the superuser.
pub fn an_owner_is_routed_as_its_user<S: UdpSockets>(sockets: S) {
    let to = SocketAddr::new(IpAddr::V4(ROUTED_NOWHERE), u16::MAX);
    let answers = [OWNER, OTHER_OWNER].map(|owner| {
        let socket = sockets
            .open(IpVersion::V4, UdpProtocol::Udp, owner)
            .unwrap();
        let from = sockets
            .bind(
                &socket,
                binding(IpAddr::V4(Ipv4Addr::UNSPECIFIED), u16::MIN),
            )
            .unwrap();
        sockets.send_to(
            &socket,
            header(UdpProtocol::Udp, from, to),
            plain_fields(),
            datagram(PAYLOAD),
        )
    });
    assert_eq!(
        answers,
        [
            Ok(()),
            Err(SendError::Host(HostError::Errno(
                linux_raw_sys::errno::EACCES as i32
            )))
        ]
    );
}

/// A segment routing header arrives in the header as sent, after the routing destination options when they
/// are sent too.
pub fn a_routing_header_arrives_in_the_header<S: UdpSockets>(sockets: S) {
    let padding = [u8::MIN; etherparse::Ipv6RawExtHeader::MIN_LEN];
    let options = etherparse::Ipv6RawExtHeaderSlice::from_slice(&padding).unwrap();
    for with_options in [false, true] {
        let (receiver, to) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
        let (sender, from) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
        let IpAddr::V6(destination) = to.ip() else {
            unreachable!("an IPv6 socket's address")
        };
        // No segment left: the only segment is the destination.
        let mut segments = vec![SEGMENT_ROUTING];
        segments.resize(SEGMENT_ROUTING_FIXED, u8::MIN);
        segments.extend(destination.octets());
        let routing = etherparse::Ipv6RawExtHeader::new_raw(IpNumber::UDP, &segments)
            .unwrap()
            .to_bytes();
        let mut route = ipv6_route(UdpProtocol::Udp, from, to);
        route.routing_header =
            Some(etherparse::Ipv6RawExtHeaderSlice::from_slice(&routing).unwrap());
        let mut fields = ipv6_header(route);
        if with_options {
            fields.routing_destination_options = Some(options.clone());
        }
        sockets
            .send_to(
                &sender,
                IpHeaderFields::V6(fields),
                plain_fields(),
                datagram(PAYLOAD),
            )
            .unwrap();
        let mut into = buffer();
        let packet = receive_packet(&sockets, &receiver, &mut into);
        let whole = &into[..packet.fields.length];
        let header = etherparse::Ipv6HeaderSlice::from_slice(whole).unwrap();
        let mut next = header.next_header();
        let mut rest = &whole[header.slice().len()..];
        if with_options {
            assert_eq!(next, IpNumber::IPV6_DESTINATION_OPTIONS);
            let options = etherparse::Ipv6RawExtHeaderSlice::from_slice(rest).unwrap();
            next = options.next_header();
            rest = &rest[options.slice().len()..];
        }
        assert_eq!(next, IpNumber::IPV6_ROUTE_HEADER);
        let arrived = etherparse::Ipv6RawExtHeaderSlice::from_slice(rest).unwrap();
        assert_eq!(
            (arrived.next_header(), arrived.payload()),
            (IpNumber::UDP, &segments[..])
        );
        assert_eq!(seen(whole).payload, PAYLOAD);
    }
}

/// Datagrams paced to a socket's maximum pacing rate arrive that far apart, waiting in its send buffer
/// meanwhile, so that a small send buffer fills and a send would block.
pub fn pacing_holds_datagrams_in_the_send_buffer<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Ip(IpSocketSetting::Timestamping(Timestamping {
            receive_software: Report::Requested,
            receive_hardware: Report::NotRequested,
            bound_clock: None,
        })),
    );
    let datagrams = PACED_DATAGRAMS * PACED_LENGTH;
    set(
        &sockets,
        &sender,
        UdpSocketSetting::Ip(IpSocketSetting::SendBuffer(ByteCount(
            datagrams * PACED_DATAGRAMS,
        ))),
    );
    set(
        &sockets,
        &sender,
        UdpSocketSetting::Ip(IpSocketSetting::MaximumPacingRate(Some(PACING_RATE))),
    );
    let payload = vec![u8::MAX; PACED_LENGTH];
    for _ in 0..PACED_DATAGRAMS {
        send(&sockets, &sender, UdpProtocol::Udp, from, to, &payload);
    }
    assert_ne!(
        sockets.count_unsent_bytes(&sender),
        Ok(ByteCount(usize::MIN))
    );
    let mut into = buffer();
    let times: Vec<Duration> = (0..PACED_DATAGRAMS)
        .map(|_| {
            receive_packet(&sockets, &receiver, &mut into)
                .fields
                .timestamps
                .software
                .expect("a receive time")
        })
        .collect();
    // Each datagram after the first waits at least its payload's time at the rate.
    let gaps = (PACED_DATAGRAMS - 1) * PACED_LENGTH;
    let least = Duration::from_secs_f64(gaps as f64 / PACING_RATE.0 as f64);
    assert!(times[PACED_DATAGRAMS - 1] - times[0] >= least, "{times:?}");
    set(
        &sockets,
        &sender,
        UdpSocketSetting::Ip(IpSocketSetting::SendBuffer(ByteCount(usize::MIN))),
    );
    let blocked = (0..PACED_DATAGRAMS).any(|_| {
        sockets.send_to(
            &sender,
            header(UdpProtocol::Udp, from, to),
            plain_fields(),
            datagram(&payload),
        ) == Err(SendError::Host(HostError::WouldBlock))
    });
    assert!(
        blocked,
        "a paced send into the smallest send buffer would block"
    );
}

/// Loopback has no hardware clock: hardware receive times are taken as a setting but none arrives, and no
/// hardware clock can be bound, on a socket bound to loopback or to no interface.
pub fn loopback_has_no_hardware_clock<S: UdpSockets>(sockets: S) {
    let interface = loopback_interface(&sockets);
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let hardware = Timestamping {
        receive_software: Report::NotRequested,
        receive_hardware: Report::Requested,
        bound_clock: None,
    };
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Ip(IpSocketSetting::Timestamping(hardware)),
    );
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    let packet = receive_packet(&sockets, &receiver, &mut buffer());
    assert_eq!(packet.fields.timestamps.hardware, None);
    let bound_clock = Timestamping {
        bound_clock: Some(PtpHardwareClockIndex(u32::MIN)),
        ..hardware
    };
    for bound_interface in [None, Some(interface)] {
        set(
            &sockets,
            &receiver,
            UdpSocketSetting::Port(PortSetting::BoundInterface(bound_interface)),
        );
        assert!(
            sockets
                .configure(
                    &receiver,
                    UdpSocketSetting::Ip(IpSocketSetting::Timestamping(bound_clock))
                )
                .is_err()
        );
    }
}

/// Closing a socket frees its port for another.
pub fn close_frees_the_port<S: UdpSockets>(sockets: S) {
    let address = closed_port(&sockets, IpVersion::V6);
    let socket = sockets
        .open(IpVersion::V6, UdpProtocol::Udp, OWNER)
        .unwrap();
    assert_eq!(
        sockets.bind(&socket, binding(address.ip(), address.port())),
        Ok(address)
    );
}

/// The facts no UDP socket has, for it has no urgent data and no peer that finishes sending: a wait for one waits on.
const NEVER_ON_UDP: [ReadyFor; 2] = [ReadyFor::URGENT_DATA, ReadyFor::PEER_FINISHED_SENDING];

/// The wakes a waiting call has given, counted, for a check that polls the call by hand.
#[derive(Default)]
struct Wakes {
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
    fn count(&self) -> usize {
        *self.count.lock().expect("the wakes")
    }

    /// Whether the call has been woken `count` times before the patience of a check ends.
    fn reach(&self, count: usize) -> bool {
        self.reach_within(count, PATIENCE)
    }

    fn reach_within(&self, count: usize, patience: Duration) -> bool {
        let wakes = self.count.lock().expect("the wakes");
        let (wakes, _) = self
            .changed
            .wait_timeout_while(wakes, patience, |wakes| *wakes < count)
            .expect("the wakes");
        *wakes >= count
    }
}

/// Polls a waiting call once with a waker that counts into `wakes`.
fn poll_with<F: Future>(call: Pin<&mut F>, wakes: &Arc<Wakes>) -> Poll<F::Output> {
    let waker = Waker::from(Arc::clone(wakes));
    call.poll(&mut Context::from_waker(&waker))
}

/// The facts a finished wait gave.
fn finished(poll: Poll<Result<ReadyFor, HostError>>) -> ReadyFor {
    match poll {
        Poll::Ready(facts) => facts.expect("the wait succeeds"),
        Poll::Pending => panic!("the wait did not finish"),
    }
}

/// The facts of a wait polled until it finishes, within the patience of a check.
fn finished_soon<F: Future<Output = Result<ReadyFor, HostError>>>(
    mut call: Pin<&mut F>,
) -> ReadyFor {
    let wakes = Arc::new(Wakes::default());
    match poll_with(call.as_mut(), &wakes) {
        Poll::Pending => {
            assert!(wakes.reach(1), "the wait finished");
            finished(poll_with(call, &wakes))
        }
        ready => finished(ready),
    }
}

/// A wait on each of many sockets at once is woken once when its datagram arrives, however many more do.
pub fn waits_on_many_sockets_each_wake_once<S: UdpSockets>(sockets: S) {
    let receivers: Vec<_> = (0..readiness_conformance::MANY_WAITS)
        .map(|_| bound(&sockets, IpVersion::V4, UdpProtocol::Udp))
        .collect();
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let mut waits: Vec<_> = receivers
        .iter()
        .map(|(receiver, _)| {
            (
                Box::pin(sockets.wait_until_ready(receiver, ReadyFor::READABLE)),
                Arc::new(Wakes::default()),
            )
        })
        .collect();
    for (wait, wakes) in &mut waits {
        assert!(poll_with(wait.as_mut(), wakes).is_pending());
    }
    for (_, to) in &receivers {
        send(&sockets, &sender, UdpProtocol::Udp, from, *to, PAYLOAD);
    }
    for (_, wakes) in &waits {
        assert!(wakes.reach(1), "a wait woken when its datagram arrives");
    }
    // More datagrams before the waits are polled again wake none of them again.
    for (_, to) in &receivers {
        send(&sockets, &sender, UdpProtocol::Udp, from, *to, PAYLOAD);
    }
    std::thread::sleep(SILENCE);
    for (wait, wakes) in &mut waits {
        assert_eq!(wakes.count(), 1, "a wait woken more than once");
        assert!(finished(poll_with(wait.as_mut(), wakes)).contains(ReadyFor::READABLE));
    }
}

/// An ICMP error that a send read off the host while looking for its own error still ends a wait for data at once.
pub fn a_wait_sees_an_error_a_send_read_first<S: UdpSockets>(sockets: S) {
    let closed = closed_port(&sockets, IpVersion::V6);
    let (sender, from) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    let wakes = Arc::new(Wakes::default());
    send(&sockets, &sender, UdpProtocol::Udp, from, closed, PAYLOAD);
    {
        let mut wait = pin!(sockets.wait_until_ready(&sender, ReadyFor::READABLE));
        let facts = match poll_with(wait.as_mut(), &wakes) {
            Poll::Pending => {
                assert!(wakes.reach(1), "woken by the ICMP error");
                finished(poll_with(wait, &wakes))
            }
            ready => finished(ready),
        };
        assert!(facts.contains(ReadyFor::ERROR), "{facts:?}");
    }
    let mtu = MaximumTransmissionUnit(u32::from(u16::MAX / 2));
    let mut fields = ipv6_header(ipv6_route(UdpProtocol::Udp, from, closed));
    fields.fragment_mtu = Some(mtu);
    fields.local_fragmentation = LocalFragmentation::Forbidden;
    fields.path_mtu_discovery = PathMtuDiscovery::Do;
    let payload = vec![u8::MIN; mtu.0 as usize];
    assert_eq!(
        sockets.send_to(
            &sender,
            IpHeaderFields::V6(fields),
            plain_fields(),
            datagram(&payload)
        ),
        Err(SendError::MessageTooLong {
            maximum_transmission_unit: mtu
        }),
    );
    let facts = finished(poll_with(
        pin!(sockets.wait_until_ready(&sender, ReadyFor::READABLE)),
        &wakes,
    ));
    assert!(facts.contains(ReadyFor::ERROR), "{facts:?}");
    let mut into = buffer();
    let packet = receive_packet(&sockets, &sender, &mut into);
    let ip = IpSlice::from_slice(&into[..packet.fields.length]).expect("a whole ICMP packet");
    assert_eq!(ip.payload_ip_number(), IpNumber::IPV6_ICMP);
}

/// A wait polled again with another waker wakes that one, the newest it was given.
pub fn a_wait_polled_again_wakes_its_newest_waker<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let first = Arc::new(Wakes::default());
    let newest = Arc::new(Wakes::default());
    let mut wait = pin!(sockets.wait_until_ready(&receiver, ReadyFor::READABLE));
    assert!(poll_with(wait.as_mut(), &first).is_pending());
    assert!(poll_with(wait.as_mut(), &newest).is_pending());
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    assert!(newest.reach(1), "the newest waker woken");
    assert!(finished(poll_with(wait.as_mut(), &newest)).contains(ReadyFor::READABLE));
}

/// A wait woken whose fact is gone by the time it is polled again waits on, and is woken by the next one.
pub fn a_wait_whose_fact_went_waits_for_the_next<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    let wakes = Arc::new(Wakes::default());
    let mut wait = pin!(sockets.wait_until_ready(&receiver, ReadyFor::READABLE));
    assert!(poll_with(wait.as_mut(), &wakes).is_pending());
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    assert!(wakes.reach(1), "woken by the first datagram");
    let mut into = buffer();
    receive_packet(&sockets, &receiver, &mut into);
    assert!(
        poll_with(wait.as_mut(), &wakes).is_pending(),
        "nothing left to read"
    );
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    assert!(wakes.reach(2), "woken again by the second datagram");
    assert!(finished(poll_with(wait.as_mut(), &wakes)).contains(ReadyFor::READABLE));
}

/// Two waits on one socket finish each by its own facts: the wait for writing at once, the wait for data when a
/// datagram arrives and not before.
pub fn two_waits_on_one_socket_finish_by_their_own_facts<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Lite);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Lite);
    let reading = Arc::new(Wakes::default());
    let writing = Arc::new(Wakes::default());
    let mut read = pin!(sockets.wait_until_ready(&receiver, ReadyFor::READABLE));
    assert!(poll_with(read.as_mut(), &reading).is_pending());
    let facts = finished(poll_with(
        pin!(sockets.wait_until_ready(&receiver, ReadyFor::WRITABLE)),
        &writing,
    ));
    assert!(
        facts.contains(ReadyFor::WRITABLE) && !facts.contains(ReadyFor::READABLE),
        "{facts:?}"
    );
    std::thread::sleep(SILENCE);
    assert_eq!(reading.count(), 0, "the wait for data woken with no data");
    send(&sockets, &sender, UdpProtocol::Lite, from, to, PAYLOAD);
    assert!(reading.reach(1), "the wait for data woken by its datagram");
    assert!(finished(poll_with(read.as_mut(), &reading)).contains(ReadyFor::READABLE));
}

/// A wait for every fact gives exactly the facts the socket has: room to send, then data to read, then an error.
pub fn a_wait_gives_the_facts_the_socket_has_and_no_others<S: UdpSockets>(sockets: S) {
    for version in [IpVersion::V4, IpVersion::V6] {
        let (receiver, to) = bound(&sockets, version, UdpProtocol::Udp);
        let (sender, from) = bound(&sockets, version, UdpProtocol::Udp);
        let every = |socket: &S::Socket| {
            finished_soon(pin!(sockets.wait_until_ready(socket, ReadyFor::all())))
        };
        assert_eq!(
            every(&receiver),
            ReadyFor::WRITABLE,
            "{version:?}: nothing to read"
        );
        send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
        finished_soon(pin!(
            sockets.wait_until_ready(&receiver, ReadyFor::READABLE)
        ));
        assert_eq!(
            every(&receiver),
            ReadyFor::READABLE | ReadyFor::WRITABLE,
            "{version:?}: a datagram queued"
        );
        let closed = closed_port(&sockets, version);
        send(&sockets, &sender, UdpProtocol::Udp, from, closed, PAYLOAD);
        finished_soon(pin!(sockets.wait_until_ready(&sender, ReadyFor::empty())));
        assert_eq!(
            every(&sender),
            ReadyFor::ERROR | ReadyFor::WRITABLE,
            "{version:?}: an ICMP error queued"
        );
    }
}

/// A wait for a fact no UDP socket has waits on while the socket is readable and writable, until an error ends it.
pub fn a_wait_for_a_fact_udp_never_has_waits_on<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    finished_soon(pin!(
        sockets.wait_until_ready(&receiver, ReadyFor::READABLE)
    ));
    let mut waits: Vec<_> = NEVER_ON_UDP
        .iter()
        .map(|interest| {
            (
                Box::pin(sockets.wait_until_ready(&receiver, *interest)),
                Arc::new(Wakes::default()),
            )
        })
        .collect();
    for (wait, wakes) in &mut waits {
        assert!(poll_with(wait.as_mut(), wakes).is_pending());
    }
    std::thread::sleep(SILENCE);
    for ((wait, wakes), interest) in waits.iter_mut().zip(NEVER_ON_UDP) {
        assert_eq!(wakes.count(), 0, "a wait for {interest:?} woken");
        assert!(poll_with(wait.as_mut(), wakes).is_pending());
    }
    let closed = closed_port(&sockets, IpVersion::V4);
    send(&sockets, &receiver, UdpProtocol::Udp, to, closed, PAYLOAD);
    for ((wait, wakes), interest) in waits.iter_mut().zip(NEVER_ON_UDP) {
        assert!(
            wakes.reach(1),
            "a wait for {interest:?} woken by the ICMP error"
        );
        assert_eq!(
            finished(poll_with(wait.as_mut(), wakes)),
            ReadyFor::READABLE | ReadyFor::WRITABLE | ReadyFor::ERROR,
            "{interest:?}"
        );
    }
    drop(waits);
    assert!(arrives(&sockets, &receiver), "the datagram stays queued");
}

/// The send buffer size the checks of a full send buffer give, of which Linux keeps twice.
pub const SEND_BUFFER: usize = 16384;

/// A pacing rate that holds each datagram after a socket's first for a quarter of a second.
pub const SLOW_PACING_RATE: BytesPerSecond = BytesPerSecond(4000);

/// How long a check waits for paced datagrams to leave room in a send buffer.
const ROOM_PATIENCE: Duration = Duration::from_secs(5);

/// The most a datagram of `PACED_LENGTH` bytes takes of a send buffer: its payload, its headers and the memory the
/// host keeps it in.
pub const PACED_DATAGRAM_ROOM: usize = 4 * PACED_LENGTH;

/// Sends paced datagrams until a send would block, and answers how many were sent.
fn fill_send_buffer<S: UdpSockets>(
    sockets: &S,
    sender: &S::Socket,
    from: SocketAddr,
    to: SocketAddr,
    payload: &[u8],
) -> usize {
    let mut sent = 0;
    loop {
        match sockets.send_to(
            sender,
            header(UdpProtocol::Udp, from, to),
            plain_fields(),
            datagram(payload),
        ) {
            Ok(()) => sent += 1,
            Err(SendError::Host(HostError::WouldBlock)) => return sent,
            Err(error) => panic!("send: {error:?}"),
        }
        assert!(sent <= SEND_BUFFER, "a send buffer that never fills");
    }
}

/// A socket on loopback whose datagrams are paced at the slow rate, and an address to send them to.
fn slowly_paced<S: UdpSockets>(
    sockets: &S,
    version: IpVersion,
    send_buffer: usize,
) -> (S::Socket, SocketAddr, S::Socket, SocketAddr) {
    let (receiver, to) = bound(sockets, version, UdpProtocol::Udp);
    let (sender, from) = bound(sockets, version, UdpProtocol::Udp);
    set(
        sockets,
        &sender,
        UdpSocketSetting::Ip(IpSocketSetting::SendBuffer(ByteCount(send_buffer))),
    );
    set(
        sockets,
        &sender,
        UdpSocketSetting::Ip(IpSocketSetting::MaximumPacingRate(Some(SLOW_PACING_RATE))),
    );
    (sender, from, receiver, to)
}

/// A send buffer filled until a send would block counts unsent at least the payload of the datagrams it holds,
/// every one but the first two, which may have left, and less than the most it can hold: twice the size given,
/// as Linux keeps, and one datagram more.
pub fn a_full_send_buffer_counts_its_unsent_bytes<S: UdpSockets>(sockets: S) {
    for version in [IpVersion::V4, IpVersion::V6] {
        let (sender, from, _receiver, to) = slowly_paced(&sockets, version, SEND_BUFFER);
        let payload = vec![u8::MAX; PACED_LENGTH];
        let sent = fill_send_buffer(&sockets, &sender, from, to, &payload);
        let unsent = sockets
            .count_unsent_bytes(&sender)
            .expect("count the unsent bytes")
            .0;
        let held = sent.saturating_sub(2) * PACED_LENGTH;
        assert!(
            (held..SEND_BUFFER * 2 + PACED_DATAGRAM_ROOM).contains(&unsent),
            "{version:?}: {unsent} bytes unsent after {sent} datagrams"
        );
    }
}

/// A wait for writing on a full send buffer does not finish until its paced datagrams leave room, then does,
/// also after a wait for data on the same socket was woken first.
pub fn a_wait_for_writing_finishes_when_room_is_made<S: UdpSockets>(sockets: S) {
    let (sender, from, receiver, to) = slowly_paced(&sockets, IpVersion::V4, usize::MIN);
    let payload = vec![u8::MAX; PACED_LENGTH];
    fill_send_buffer(&sockets, &sender, from, to, &payload);
    let reading = Arc::new(Wakes::default());
    let mut read = pin!(sockets.wait_until_ready(&sender, ReadyFor::READABLE));
    assert!(poll_with(read.as_mut(), &reading).is_pending());
    let wakes = Arc::new(Wakes::default());
    let mut wait = pin!(sockets.wait_until_ready(&sender, ReadyFor::WRITABLE));
    assert!(
        poll_with(wait.as_mut(), &wakes).is_pending(),
        "writable with a full send buffer"
    );
    send(&sockets, &receiver, UdpProtocol::Udp, to, from, PAYLOAD);
    assert!(reading.reach(1), "the wait for data woken by its datagram");
    assert!(finished(poll_with(read.as_mut(), &reading)).contains(ReadyFor::READABLE));
    assert_eq!(wakes.count(), 0, "the wait for writing woken with no room");
    assert!(
        wakes.reach_within(1, ROOM_PATIENCE),
        "woken when room is made"
    );
    assert!(finished(poll_with(wait.as_mut(), &wakes)).contains(ReadyFor::WRITABLE));
    assert_eq!(
        sockets.send_to(
            &sender,
            header(UdpProtocol::Udp, from, to),
            plain_fields(),
            datagram(&payload)
        ),
        Ok(())
    );
}

/// A wait for data or writing on a full send buffer, made while a wait for data alone waits, finishes when room
/// is made, and the wait for data waits on.
pub fn a_wait_for_data_or_writing_finishes_when_room_is_made<S: UdpSockets>(sockets: S) {
    let (sender, from, _receiver, to) = slowly_paced(&sockets, IpVersion::V6, usize::MIN);
    let payload = vec![u8::MAX; PACED_LENGTH];
    fill_send_buffer(&sockets, &sender, from, to, &payload);
    let reading = Arc::new(Wakes::default());
    let mut read = pin!(sockets.wait_until_ready(&sender, ReadyFor::READABLE));
    assert!(poll_with(read.as_mut(), &reading).is_pending());
    let wakes = Arc::new(Wakes::default());
    let either = ReadyFor::READABLE | ReadyFor::WRITABLE;
    let mut wait = pin!(sockets.wait_until_ready(&sender, either));
    assert!(
        poll_with(wait.as_mut(), &wakes).is_pending(),
        "writable with a full send buffer"
    );
    assert!(
        wakes.reach_within(1, ROOM_PATIENCE),
        "woken when room is made"
    );
    assert_eq!(finished(poll_with(wait, &wakes)), ReadyFor::WRITABLE);
    assert_eq!(reading.count(), 0, "the wait for data woken with no data");
    assert!(poll_with(read, &reading).is_pending());
}

/// A send whose header's upper layer is another protocol than the socket's is refused, and sends nothing.
pub fn a_header_of_another_protocol_is_refused<S: UdpSockets>(sockets: S) {
    for (protocol, other) in [
        (UdpProtocol::Udp, UdpProtocol::Lite),
        (UdpProtocol::Lite, UdpProtocol::Udp),
    ] {
        let (socket, from) = bound(&sockets, IpVersion::V4, protocol);
        let (receiver, to) = bound(&sockets, IpVersion::V4, protocol);
        let sent = sockets.send_to(
            &socket,
            header(other, from, to),
            plain_fields(),
            datagram(PAYLOAD),
        );
        assert!(sent.is_err(), "{protocol:?} sent as {other:?}");
        assert!(
            !arrives(&sockets, &receiver),
            "{protocol:?} sent as {other:?}"
        );
    }
}

/// A checksum only the other protocol has, UDP-Lite's partial coverage on UDP or no checksum on UDP-Lite, is
/// refused, and sends nothing.
pub fn a_checksum_of_the_other_protocol_is_refused<S: UdpSockets>(sockets: S) {
    let header_alone = NonZeroU16::new(UdpHeader::LEN_U16).expect("a coverage");
    let coverage = ChecksumCoverage(header_alone);
    for (protocol, checksum) in [
        (UdpProtocol::Udp, UdpChecksum::Partial(coverage)),
        (UdpProtocol::Lite, UdpChecksum::Omitted),
    ] {
        let (socket, from) = bound(&sockets, IpVersion::V6, protocol);
        let (receiver, to) = bound(&sockets, IpVersion::V6, protocol);
        let sent = sockets.send_to(
            &socket,
            header(protocol, from, to),
            plain_fields(),
            UdpDatagram {
                checksum,
                ..datagram(PAYLOAD)
            },
        );
        assert!(sent.is_err(), "{protocol:?} with {checksum:?}");
        assert!(
            !arrives(&sockets, &receiver),
            "{protocol:?} with {checksum:?}"
        );
    }
}

/// An ICMP error about a datagram sent waits for a receive however the sends after it answer, as Linux fails the
/// next send with the error it reports.
pub fn an_icmp_error_waits_for_a_receive_after_later_sends<S: UdpSockets>(sockets: S) {
    for version in [IpVersion::V4, IpVersion::V6] {
        let closed = closed_port(&sockets, version);
        let (sender, from) = bound(&sockets, version, UdpProtocol::Udp);
        send(&sockets, &sender, UdpProtocol::Udp, from, closed, PAYLOAD);
        let facts = finished_soon(pin!(sockets.wait_until_ready(&sender, ReadyFor::empty())));
        assert!(facts.contains(ReadyFor::ERROR), "{version:?}: {facts:?}");
        let later = sockets.send_to(
            &sender,
            header(UdpProtocol::Udp, from, closed),
            plain_fields(),
            datagram(PAYLOAD),
        );
        let mut into = buffer();
        let packet = receive_packet(&sockets, &sender, &mut into);
        let ip = IpSlice::from_slice(&into[..packet.fields.length]).expect("a whole ICMP packet");
        let icmp = match version {
            IpVersion::V4 => IpNumber::ICMP,
            IpVersion::V6 => IpNumber::IPV6_ICMP,
        };
        assert_eq!(ip.payload_ip_number(), icmp, "{version:?}: after {later:?}");
    }
}

/// A send whose header is IPv6 on an IPv4 socket is refused as Linux refuses a destination of another family.
pub fn a_header_of_another_version_is_refused<S: UdpSockets>(sockets: S) {
    let (socket, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (receiver, to) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    assert_eq!(
        sockets.send_to(
            &socket,
            header(UdpProtocol::Udp, from, to),
            plain_fields(),
            datagram(PAYLOAD)
        ),
        Err(SendError::Host(HostError::Errno(
            linux_raw_sys::errno::EAFNOSUPPORT as i32
        )))
    );
    assert!(!arrives(&sockets, &receiver));
}

/// A datagram whose route must stay on its link is not sent through a gateway; one of universe scope is.
pub fn link_scope_keeps_a_datagram_on_its_link<S: UdpSockets>(sockets: S) {
    let to = SocketAddr::new(IpAddr::V4(BEYOND_A_GATEWAY), u16::MAX);
    let sender = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    let from = sockets
        .bind(&sender, binding(unspecified(IpVersion::V4), u16::MIN))
        .unwrap();
    let answers = [RouteScope::Universe, RouteScope::Link].map(|scope| {
        let mut route = ipv4_route(UdpProtocol::Udp, from, to);
        route.scope = scope;
        sockets.send_to(
            &sender,
            ipv4_header(route),
            plain_fields(),
            datagram(PAYLOAD),
        )
    });
    assert_eq!(
        answers,
        [
            Ok(()),
            Err(SendError::Host(HostError::Errno(
                linux_raw_sys::errno::ENETUNREACH as i32
            )))
        ]
    );
}

/// A datagram with a transmit time on the monotonic clock arrives no earlier than that time.
pub fn a_transmit_time_holds_a_datagram_until_then<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    let now = Duration::new(
        u64::try_from(now.tv_sec).unwrap(),
        u32::try_from(now.tv_nsec).unwrap(),
    );
    let mut fields = plain_fields();
    fields.transmit_time = Some(wcontainers_internet_protocol::TransmitTime {
        clock: wcontainers_internet_protocol::TransmitClock::Monotonic,
        at: now + HELD,
        mode: wcontainers_internet_protocol::TransmitTimeMode::Strict,
        errors: Report::NotRequested,
    });
    let start = Instant::now();
    sockets
        .send_to(
            &sender,
            header(UdpProtocol::Udp, from, to),
            fields,
            datagram(PAYLOAD),
        )
        .unwrap();
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert!(
        start.elapsed() >= HELD,
        "arrived after {:?}",
        start.elapsed()
    );
    assert_eq!(seen(&into[..packet.fields.length]).payload, PAYLOAD);
}

/// A dual-stack IPv6 socket holds its port for IPv4 too, takes IPv4 datagrams with their IPv4 header and sends
/// IPv4 ones with the IPv4 header fields given; an IPv6-only one leaves the port's IPv4 side to others.
pub fn a_dual_stack_socket_takes_ipv4<S: UdpSockets>(sockets: S) {
    let dual = sockets
        .open(IpVersion::V6, UdpProtocol::Udp, OWNER)
        .unwrap();
    let local = sockets
        .bind(
            &dual,
            LocalBinding {
                address: BindAddress::V6 {
                    address: Ipv6Addr::UNSPECIFIED,
                    families: Ipv6Families::DualStack,
                },
                port: LocalPort::Requested(Port(u16::MIN)),
            },
        )
        .unwrap();
    let ipv4_side = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    assert_eq!(
        sockets.bind(
            &ipv4_side,
            binding(unspecified(IpVersion::V4), local.port())
        ),
        Err(HostError::Errno(linux_raw_sys::errno::EADDRINUSE as i32))
    );
    let (ipv4, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let to = SocketAddr::new(loopback(IpVersion::V4), local.port());
    let with_time_to_live = |from, to, ttl| {
        let IpHeaderFields::V4(mut header) = header(UdpProtocol::Udp, from, to) else {
            unreachable!("an IPv4 destination")
        };
        header.time_to_live = Some(wcontainers_internet_protocol::HopLimit(ttl));
        IpHeaderFields::V4(header)
    };
    let mut into = buffer();
    for (sender, receiver, from, to, ttl) in [
        (&ipv4, &dual, from, to, u8::MAX),
        (&dual, &ipv4, to, from, u8::MAX - 1),
    ] {
        sockets
            .send_to(
                sender,
                with_time_to_live(from, to, ttl),
                plain_fields(),
                datagram(PAYLOAD),
            )
            .unwrap();
        let packet = receive_packet(&sockets, receiver, &mut into);
        let seen = seen(&into[..packet.fields.length]);
        assert_eq!(
            (seen.source, seen.destination, seen.hop_limit, seen.payload),
            (from, to, ttl, PAYLOAD)
        );
    }
    let only_any = sockets
        .open(IpVersion::V6, UdpProtocol::Udp, OWNER)
        .unwrap();
    let port = sockets
        .bind(&only_any, binding(unspecified(IpVersion::V6), u16::MIN))
        .unwrap()
        .port();
    assert_eq!(
        sockets
            .bind(&ipv4_side, binding(unspecified(IpVersion::V4), port))
            .map(|address| address.port()),
        Ok(port)
    );
}

/// The contract of `Configurable` on a socket, by its own suite, for a setting taken.
pub fn configurable_contract_holds_for_a_setting_taken<S: UdpSockets>(sockets: S) {
    let (socket, _) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    wcontainers_configurable_conformance::setting_twice_equals_setting_once((
        sockets,
        socket,
        UdpSocketSetting::Port(PortSetting::AddressSharing(AddressSharing::Shared)),
    ));
}

/// The contract of `Configurable` on a socket, by its own suite, for a setting refused.
pub fn configurable_contract_holds_for_a_setting_refused<S: UdpSockets>(sockets: S) {
    let (socket, _) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let no_interface =
        UdpSocketSetting::Port(PortSetting::BoundInterface(Some(InterfaceIndex(u32::MAX))));
    assert!(sockets.configure(&socket, no_interface).is_err());
    wcontainers_configurable_conformance::setting_twice_equals_setting_once((
        sockets,
        socket,
        no_interface,
    ));
}

/// A socket waited on for data, made ready by a datagram from another socket, for the readiness suite.
pub fn waiting_for_a_datagram<S: UdpSockets>(
    sockets: S,
) -> Waiting<S, S::Socket, impl FnOnce(&S, &S::Socket)> {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    Waiting {
        readiness: sockets,
        handle: receiver,
        interest: ReadyFor::READABLE,
        make_ready: move |sockets: &S, _: &S::Socket| {
            send(sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
        },
    }
}

/// A socket waited on for data, which an ICMP error about a datagram it sends fails, for the readiness suite.
pub fn waiting_for_data_until_an_icmp_error<S: UdpSockets>(
    sockets: S,
) -> Waiting<S, S::Socket, impl FnOnce(&S, &S::Socket)> {
    let closed = closed_port(&sockets, IpVersion::V6);
    let (socket, from) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    Waiting {
        readiness: sockets,
        handle: socket,
        interest: ReadyFor::READABLE,
        make_ready: move |sockets: &S, socket: &S::Socket| {
            send(sockets, socket, UdpProtocol::Udp, from, closed, PAYLOAD);
        },
    }
}

/// The contract of `NegativeRouteAdvice` on a connected socket, by its own suite.
pub fn negative_route_advice_contract_holds<S: UdpSockets>(sockets: S) {
    let (socket, local) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    let (_peer, peer_address) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    sockets
        .connect(&socket, route(UdpProtocol::Udp, local, peer_address))
        .unwrap();
    wcontainers_negative_route_advice_conformance::advice_is_taken_again((sockets, socket));
}

/// The time now on a clock, since its epoch.
fn clock_now(clock: rustix::time::ClockId) -> Duration {
    let now = rustix::time::clock_gettime(clock);
    Duration::new(
        u64::try_from(now.tv_sec).unwrap(),
        u32::try_from(now.tv_nsec).unwrap(),
    )
}

/// Connect takes its route's selectors: the IPv6 source preferences choose between the public and the temporary
/// address, the host's choice being temporary, and an IPv4 interface given gives that interface's address.
pub fn connect_takes_its_route_selectors<S: UdpSockets>(sockets: S) {
    let connected = |version, route| {
        let socket = sockets.open(version, UdpProtocol::Udp, OWNER).unwrap();
        sockets.connect(&socket, route).expect("connect").ip()
    };
    let port = closed_port(&sockets, IpVersion::V6).port();
    let from = SocketAddr::new(unspecified(IpVersion::V6), u16::MIN);
    let peer = SocketAddr::new(IpAddr::V6(PREFIX_V6_PEER), port);
    let preferring = |temporary| {
        let mut route = ipv6_route(UdpProtocol::Udp, from, peer);
        route.source_preferences.temporary = temporary;
        connected(IpVersion::V6, RouteSelectors::V6(route))
    };
    assert_eq!(
        preferring(TemporaryAddressPreference::Public),
        IpAddr::V6(PUBLIC_V6)
    );
    let IpAddr::V6(temporary) = preferring(TemporaryAddressPreference::Temporary) else {
        panic!("an IPv6 source")
    };
    assert_ne!(temporary, PUBLIC_V6);
    assert_eq!(
        temporary.segments()[..PREFIX_SEGMENTS],
        PUBLIC_V6.segments()[..PREFIX_SEGMENTS],
        "{temporary} is of the prefix"
    );
    assert_eq!(
        preferring(TemporaryAddressPreference::SystemDefault),
        IpAddr::V6(temporary)
    );
    let from = SocketAddr::new(unspecified(IpVersion::V4), u16::MIN);
    let peer = SocketAddr::new(IpAddr::V4(SCHEDULED_V4_PEER), port);
    assert_eq!(
        connected(IpVersion::V4, route(UdpProtocol::Udp, from, peer)),
        IpAddr::V4(SCHEDULED_V4)
    );
    let mut through = ipv4_route(UdpProtocol::Udp, from, peer);
    through.interface = Some(multicast_interface(&sockets));
    assert_eq!(
        connected(IpVersion::V4, RouteSelectors::V4(through)),
        IpAddr::V4(MULTICAST_INTERFACE_V4)
    );
}

/// Local fragmentation forbidden refuses a datagram over the route's MTU that path MTU discovery would let be
/// fragmented, and allowed fragments it.
pub fn forbidden_fragmentation_refuses_what_would_be_fragmented<S: UdpSockets>(sockets: S) {
    let receiver = sockets
        .open(IpVersion::V6, UdpProtocol::Udp, OWNER)
        .unwrap();
    let to = sockets
        .bind(&receiver, binding(IpAddr::V6(NARROW_V6), u16::MIN))
        .unwrap();
    let (sender, from) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    let payload = vec![u8::MIN; usize::from(ROUTE_MTU) * 2];
    let mut header = ipv6_header(ipv6_route(UdpProtocol::Udp, from, to));
    header.path_mtu_discovery = PathMtuDiscovery::Want;
    header.local_fragmentation = LocalFragmentation::Forbidden;
    assert_eq!(
        sockets.send_to(
            &sender,
            IpHeaderFields::V6(header.clone()),
            plain_fields(),
            datagram(&payload)
        ),
        Err(SendError::MessageTooLong {
            maximum_transmission_unit: MaximumTransmissionUnit(u32::from(ROUTE_MTU))
        })
    );
    header.local_fragmentation = LocalFragmentation::Allowed;
    sockets
        .send_to(
            &sender,
            IpHeaderFields::V6(header),
            plain_fields(),
            datagram(&payload),
        )
        .unwrap();
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert!(packet.fields.largest_fragment.is_some());
    assert_eq!(seen(&into[..packet.fields.length]).payload, payload);
}

/// An IPv6 send from a socket bound to no address leaves from the source its header gives.
pub fn an_ipv6_send_takes_its_source<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    let sender = sockets
        .open(IpVersion::V6, UdpProtocol::Udp, OWNER)
        .unwrap();
    let from = sockets
        .bind(&sender, binding(unspecified(IpVersion::V6), u16::MIN))
        .unwrap();
    let mut route = ipv6_route(UdpProtocol::Udp, from, to);
    route.source = Some(NARROW_V6);
    sockets
        .send_to(
            &sender,
            IpHeaderFields::V6(ipv6_header(route)),
            plain_fields(),
            datagram(PAYLOAD),
        )
        .unwrap();
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(
        seen(&into[..packet.fields.length]).source,
        SocketAddr::new(IpAddr::V6(NARROW_V6), from.port())
    );
}

/// With its maximum pacing rate unset again, a socket's datagrams leave at once.
pub fn an_unset_pacing_rate_holds_nothing<S: UdpSockets>(sockets: S) {
    let (receiver, to) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Ip(IpSocketSetting::Timestamping(Timestamping {
            receive_software: Report::Requested,
            receive_hardware: Report::NotRequested,
            bound_clock: None,
        })),
    );
    for rate in [Some(PACING_RATE), None] {
        set(
            &sockets,
            &sender,
            UdpSocketSetting::Ip(IpSocketSetting::MaximumPacingRate(rate)),
        );
    }
    let payload = vec![u8::MAX; PACED_LENGTH];
    for _ in 0..PACED_DATAGRAMS {
        send(&sockets, &sender, UdpProtocol::Udp, from, to, &payload);
    }
    let mut into = buffer();
    let times: Vec<Duration> = (0..PACED_DATAGRAMS)
        .map(|_| {
            receive_packet(&sockets, &receiver, &mut into)
                .fields
                .timestamps
                .software
                .expect("a receive time")
        })
        .collect();
    let gaps = (PACED_DATAGRAMS - 1) * PACED_LENGTH;
    let paced = Duration::from_secs_f64(gaps as f64 / PACING_RATE.0 as f64);
    assert!(times[PACED_DATAGRAMS - 1] - times[0] < paced, "{times:?}");
}

/// A transmit time the interface's schedule cannot keep is reported with the key of its send and why: a time
/// already past, or a clock or mode other than the schedule's, is invalid, and a time the schedule misses is
/// missed, with the packet if asked; a send that did not ask for errors gets no report.
pub fn rejected_transmit_times_are_reported_with_their_keys<S: UdpSockets>(sockets: S) {
    let sender = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    let from = sockets
        .bind(&sender, binding(unspecified(IpVersion::V4), u16::MIN))
        .unwrap();
    let to = SocketAddr::new(IpAddr::V4(SCHEDULED_V4_PEER), from.port());
    let atomic = clock_now(rustix::time::ClockId::Tai);
    // Each send's clock, time, mode, whether it asks for errors, and what is reported; its key is its place.
    let sends = [
        (
            TransmitClock::InternationalAtomicTime,
            atomic + HELD,
            TransmitTimeMode::Strict,
            Report::NotRequested,
            None,
        ),
        (
            TransmitClock::InternationalAtomicTime,
            HELD,
            TransmitTimeMode::Strict,
            Report::Requested,
            Some(TransmitTimeError::InvalidParameter),
        ),
        (
            TransmitClock::Realtime,
            atomic + HELD + APART,
            TransmitTimeMode::Strict,
            Report::Requested,
            Some(TransmitTimeError::InvalidParameter),
        ),
        (
            TransmitClock::InternationalAtomicTime,
            atomic + HELD + APART + APART,
            TransmitTimeMode::Deadline,
            Report::Requested,
            Some(TransmitTimeError::InvalidParameter),
        ),
        (
            TransmitClock::InternationalAtomicTime,
            atomic + HELD - APART,
            TransmitTimeMode::Strict,
            Report::Requested,
            Some(TransmitTimeError::Missed),
        ),
    ];
    let key = |place: usize| TransmitReportKey(u32::try_from(place).unwrap());
    // A missed time's report comes with the packet, which its send asks for.
    let with_packet = |reason| reason == Some(TransmitTimeError::Missed);
    for (place, &(clock, at, mode, errors, reason)) in sends.iter().enumerate() {
        let mut fields = plain_fields();
        fields.transmit_time = Some(TransmitTime {
            clock,
            at,
            mode,
            errors,
        });
        fields.transmit_reports.key = key(place);
        if with_packet(reason) {
            fields.transmit_reports.packet = Report::Requested;
        }
        match sockets.send_to(
            &sender,
            header(UdpProtocol::Udp, from, to),
            fields,
            datagram(PAYLOAD),
        ) {
            Ok(()) | Err(SendError::Host(_)) => {}
            Err(error) => panic!("send {place}: {error:?}"),
        }
    }
    let mut reported = Vec::new();
    let mut into = buffer();
    let start = Instant::now();
    while start.elapsed() < HELD + SILENCE {
        match sockets.receive(&sender, &mut into) {
            Ok(UdpReceived::TransmitReport(report)) => {
                let TransmitEvent::TransmitTimeRejected(reason) = report.event else {
                    panic!("a rejection, not {report:?}")
                };
                assert_eq!(report.packet_length.is_some(), with_packet(Some(reason)));
                reported.push((report.key, Some(reason)));
            }
            Ok(UdpReceived::Packet(packet)) => panic!("a report, not {packet:?}"),
            Err(HostError::WouldBlock) => std::thread::yield_now(),
            Err(error) => panic!("receive: {error:?}"),
        }
    }
    reported.sort_by_key(|(key, _)| key.0);
    let expected: Vec<_> = sends
        .iter()
        .enumerate()
        .filter(|(_, send)| send.4.is_some())
        .map(|(place, send)| (key(place), send.4))
        .collect();
    assert_eq!(reported, expected);
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        $crate::readiness_conformance::tests!(
            readiness_contract,
            ready: $crate::waiting_for_a_datagram($value),
            failing: $crate::waiting_for_data_until_an_icmp_error($value)
        );
        $crate::tests!(@each $value;
            open_gives_sockets_of_each_version_and_protocol
            bind_returns_the_bound_address
            address_sharing_lets_two_sockets_bind_one_port
            port_sharing_lets_two_sockets_bind_one_port
            bind_chosen_by_exit_node_avoids_reserved_ports
            a_datagram_arrives_with_its_ipv4_header
            a_datagram_arrives_with_its_ipv6_header
            ipv4_options_arrive_in_the_header
            a_short_buffer_keeps_the_whole_length
            receive_with_nothing_queued_would_block
            a_connected_socket_takes_only_its_peers_datagrams
            disconnect_keeps_the_binding
            a_send_after_a_disconnect_carries_the_former_peers_address
            udp_lite_coverage_below_the_minimum_is_dropped
            a_segment_size_makes_one_datagram_per_segment
            coalescing_by_segment_size_gives_one_packet
            a_zero_checksum_arrives_only_where_accepted
            a_closed_port_answers_with_an_icmp_error
            connect_takes_its_route_selectors
            forbidden_fragmentation_refuses_what_would_be_fragmented
            an_ipv6_send_takes_its_source
            an_unset_pacing_rate_holds_nothing
            rejected_transmit_times_are_reported_with_their_keys
            a_datagram_over_the_mtu_is_too_long
            transmit_reports_carry_the_key_of_their_send
            receive_timestamps_come_only_when_set
            a_small_receive_buffer_counts_its_drops
            sending_settings_are_taken
            multicast_membership_takes_the_groups_datagrams
            a_bound_interface_is_taken
            nonlocal_addresses_can_be_bound_when_allowed
            the_incoming_processor_is_read_back
            negative_route_advice_keeps_a_socket_sending
            send_fields_reach_the_receiver
            ipv6_options_arrive_in_the_header
            broadcast_needs_its_permission
            path_mtu_discovery_modes_take_the_routes_mtu
            a_waived_source_check_sends_from_an_absent_address
            a_group_filter_chooses_the_socket
            ipv6_memberships_take_their_datagrams
            an_owner_is_routed_as_its_user
            a_routing_header_arrives_in_the_header
            pacing_holds_datagrams_in_the_send_buffer
            loopback_has_no_hardware_clock
            close_frees_the_port
            bind_chosen_by_exit_node_takes_any_range
            all_groups_decides_whether_other_sockets_memberships_count
            link_scope_keeps_a_datagram_on_its_link
            a_transmit_time_holds_a_datagram_until_then
            a_dual_stack_socket_takes_ipv4
            configurable_contract_holds_for_a_setting_taken
            configurable_contract_holds_for_a_setting_refused
            negative_route_advice_contract_holds
            waits_on_many_sockets_each_wake_once
            a_wait_sees_an_error_a_send_read_first
            a_wait_polled_again_wakes_its_newest_waker
            a_wait_whose_fact_went_waits_for_the_next
            two_waits_on_one_socket_finish_by_their_own_facts
            a_full_send_buffer_counts_its_unsent_bytes
            a_wait_for_writing_finishes_when_room_is_made
            a_header_of_another_version_is_refused
            a_wait_gives_the_facts_the_socket_has_and_no_others
            a_wait_for_a_fact_udp_never_has_waits_on
            a_wait_for_data_or_writing_finishes_when_room_is_made
            a_header_of_another_protocol_is_refused
            a_checksum_of_the_other_protocol_is_refused
            an_icmp_error_waits_for_a_receive_after_later_sends
        );
    };
    (@each $value:expr; $($name:ident)*) => {
        $(
            #[test]
            fn $name() {
                $crate::$name($value);
            }
        )*
    };
}
