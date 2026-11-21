//! The tests every implementation of wcontainers-udp-sockets runs, written against its traits alone.
//!
//! Each check talks over the loopback addresses only, with ports the kernel chooses. A network a check needs
//! beyond loopback (none so far) is the caller's to provide.

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use core::num::NonZeroU16;
use std::time::{Duration, Instant};

use etherparse::{IpDscp, IpEcn, IpNumber, IpSlice, Ipv4Options, Ipv6FlowLabel, UdpHeaderSlice};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    AddressSharing, AnycastAddress, BindAddress, BroadcastPermission, ByteCount, FilterInstruction,
    FilterJump, FilterOperand, FilterOperation, InterfaceIndex, IpHeaderFields, IpSendFields,
    IpSocketSetting, IpVersion, Ipv4HeaderFields, Ipv4RouteSelectors, Ipv6Families,
    Ipv6HeaderFields, Ipv6RouteSelectors, LocalBinding, LocalFragmentation, LocalPort,
    MaximumTransmissionUnit, Membership, MobileAddressPreference, MulticastGroup,
    MulticastMembership, NeighbourConfirmation, NonlocalAddresses, PacketMark, PacketPriority,
    PathMtuDiscovery, Port, PortChoice, PortRange, PortSetting, PortSharing, ProcessorIndex,
    Report, RouteScope, RouteSelectors, SendError, SocketFilter, SourceAddressCheck, SourceFilter,
    SourcePreferences, TemporaryAddressPreference, TimestampSource, Timestamping, TransmitEvent,
    TransmitReportKey, TransmitReportRequest, TransmitStage, TransportPorts, UpperLayer,
    UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_udp_sockets::{
    ChecksumCoverage, ReceiveCoalescing, SegmentSize, UdpChecksum, UdpDatagram, UdpLocalEnd,
    UdpProtocol, UdpReceived, UdpReceivedPacket, UdpSocketSetting, UdpSockets, ZeroChecksumReceipt,
};

/// The owner every check opens its sockets for: the superuser, whom any host lets own a socket.
const OWNER: UserId = UserId(u32::MIN);

/// How long a check waits for a datagram sent over loopback.
const PATIENCE: Duration = Duration::from_secs(1);

/// ff0e, the first group of an IPv6 multicast address of global scope.
const GLOBAL_MULTICAST: u16 = 0xff0e;

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

/// Sets one setting twice, as setting it once must equal.
fn set<S: UdpSockets>(sockets: &S, socket: &S::Socket, setting: UdpSocketSetting<'_>) {
    sockets.configure(socket, setting).expect("set a setting");
    sockets
        .configure(socket, setting)
        .expect("set the same setting again");
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

/// A received UDP or UDP-Lite packet's IP header fields, ports and payload.
struct Seen<'a> {
    source: SocketAddr,
    destination: SocketAddr,
    hop_limit: u8,
    dscp: IpDscp,
    ecn: IpEcn,
    flow_label: Option<Ipv6FlowLabel>,
    protocol: IpNumber,
    options: &'a [u8],
    payload: &'a [u8],
}

fn seen(packet: &[u8]) -> Seen<'_> {
    let ip = IpSlice::from_slice(packet).expect("a whole IP packet");
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
    Seen {
        source: SocketAddr::new(ip.source_addr(), udp.source_port()),
        destination: SocketAddr::new(ip.destination_addr(), udp.destination_port()),
        hop_limit,
        dscp,
        ecn,
        flow_label,
        protocol: ip.payload_ip_number(),
        options,
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
    let next = Port(free + 1);
    let reserved = [PortRange {
        first: next,
        last: next,
    }];
    let socket = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, OWNER)
        .unwrap();
    let local = LocalBinding {
        address: BindAddress::V4(Ipv4Addr::LOCALHOST),
        port: LocalPort::ChosenByExitNode(PortChoice {
            range: PortRange {
                first: Port(free),
                last: next,
            },
            reserved: &reserved,
        }),
    };
    assert_eq!(
        sockets.bind(&socket, local).map(|address| address.port()),
        Ok(free)
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
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    send(&sockets, &sender, UdpProtocol::Udp, from, to, PAYLOAD);
    let mut whole = buffer();
    let length = receive_packet(&sockets, &receiver, &mut whole)
        .fields
        .length;
    let mut short = vec![u8::MIN; length / 2];
    assert_eq!(
        receive_packet(&sockets, &receiver, &mut short)
            .fields
            .length,
        length
    );
    assert_eq!(short[..], whole[..length / 2]);
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

/// A UDP-Lite datagram whose checksum covers less than the receiver's minimum is dropped; one that covers enough
/// arrives.
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
    for (coverage, payload) in [(short, TOO_LITTLE_COVERED), (minimum, PAYLOAD)] {
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
    }
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).payload, PAYLOAD);
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

/// A member of a multicast group takes its datagrams, through the loopback of its sender, until it leaves.
pub fn multicast_membership_takes_the_groups_datagrams<S: UdpSockets>(sockets: S) {
    let interface = loopback_interface(&sockets);
    // The last group of the local network control block, which no interface joins by itself.
    let group = Ipv4Addr::from_bits(linux_raw_sys::net::INADDR_MAX_LOCAL_GROUP);
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
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Ip(IpSocketSetting::MulticastAllGroups {
            version: IpVersion::V4,
            enabled: false,
        }),
    );
    let member = |membership| MulticastGroup {
        group,
        interface: Some(interface),
        membership,
    };
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Ip(IpSocketSetting::MulticastV4(member(
            MulticastMembership::Joined(SourceFilter::Exclude(&[])),
        ))),
    );
    let (sender, from) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    set(
        &sockets,
        &sender,
        UdpSocketSetting::Ip(IpSocketSetting::MulticastLoopback(true)),
    );
    let to = SocketAddr::new(IpAddr::V4(group), port);
    let mut route = ipv4_route(UdpProtocol::Udp, from, to);
    route.interface = Some(interface);
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
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Ip(IpSocketSetting::MulticastV4(member(
            MulticastMembership::Left,
        ))),
    );
    sockets
        .send_to(
            &sender,
            ipv4_header(route),
            plain_fields(),
            datagram(PAYLOAD),
        )
        .unwrap();
    std::thread::sleep(PATIENCE / 2);
    assert_eq!(
        sockets.receive(&receiver, &mut into),
        Err(HostError::WouldBlock)
    );
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

/// A socket bound to an interface still sends over it; one bound to no interface that exists cannot be.
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

/// The incoming processor set is the one read back, and none once unset.
pub fn the_incoming_processor_is_read_back<S: UdpSockets>(sockets: S) {
    let (socket, _) = bound(&sockets, IpVersion::V4, UdpProtocol::Udp);
    let first = ProcessorIndex(u32::MIN);
    set(
        &sockets,
        &socket,
        UdpSocketSetting::Port(PortSetting::IncomingProcessor(Some(first))),
    );
    assert_eq!(sockets.read_incoming_processor(&socket), Ok(Some(first)));
    set(
        &sockets,
        &socket,
        UdpSocketSetting::Port(PortSetting::IncomingProcessor(None)),
    );
    assert_eq!(sockets.read_incoming_processor(&socket), Ok(None));
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

/// Every mode of path MTU discovery sends, over IPv4 and IPv6.
pub fn every_path_mtu_discovery_mode_sends<S: UdpSockets>(sockets: S) {
    for version in [IpVersion::V4, IpVersion::V6] {
        let (receiver, to) = bound(&sockets, version, UdpProtocol::Udp);
        let (sender, from) = bound(&sockets, version, UdpProtocol::Udp);
        let mut into = buffer();
        for mode in [
            PathMtuDiscovery::Dont,
            PathMtuDiscovery::Want,
            PathMtuDiscovery::Do,
            PathMtuDiscovery::Probe,
            PathMtuDiscovery::Interface,
            PathMtuDiscovery::Omit,
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
            sockets
                .send_to(&sender, header, plain_fields(), datagram(PAYLOAD))
                .unwrap();
            let packet = receive_packet(&sockets, &receiver, &mut into);
            assert_eq!(seen(&into[..packet.fields.length]).payload, PAYLOAD);
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
}

/// An IPv6 multicast member takes the group's datagrams with all-groups off, and an anycast address joined on
/// the loopback interface is reachable until left.
pub fn ipv6_memberships_take_their_datagrams<S: UdpSockets>(sockets: S) {
    let interface = loopback_interface(&sockets);
    let group = Ipv6Addr::new(
        GLOBAL_MULTICAST,
        u16::MIN,
        u16::MIN,
        u16::MIN,
        u16::MIN,
        u16::MIN,
        u16::MIN,
        u16::MAX,
    );
    let receiver = sockets
        .open(IpVersion::V6, UdpProtocol::Udp, OWNER)
        .unwrap();
    let port = sockets
        .bind(
            &receiver,
            binding(IpAddr::V6(Ipv6Addr::UNSPECIFIED), u16::MIN),
        )
        .unwrap()
        .port();
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Ip(IpSocketSetting::MulticastAllGroups {
            version: IpVersion::V6,
            enabled: false,
        }),
    );
    let joined = MulticastGroup {
        group,
        interface: Some(interface),
        membership: MulticastMembership::Joined(SourceFilter::Exclude(&[])),
    };
    set(
        &sockets,
        &receiver,
        UdpSocketSetting::Ip(IpSocketSetting::MulticastV6(joined)),
    );
    let (sender, from) = bound(&sockets, IpVersion::V6, UdpProtocol::Udp);
    set(
        &sockets,
        &sender,
        UdpSocketSetting::Ip(IpSocketSetting::MulticastLoopback(true)),
    );
    let to = SocketAddr::new(IpAddr::V6(group), port);
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
    let mut into = buffer();
    let packet = receive_packet(&sockets, &receiver, &mut into);
    assert_eq!(seen(&into[..packet.fields.length]).destination, to);
    // fd00::1, a unique local address of no network the host has.
    let unique_local = u128::from(u8::MAX - 2) << (u128::BITS - u8::BITS);
    let anycast = Ipv6Addr::from_bits(unique_local | Ipv6Addr::LOCALHOST.to_bits());
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

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
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
            udp_lite_coverage_below_the_minimum_is_dropped
            a_segment_size_makes_one_datagram_per_segment
            coalescing_by_segment_size_gives_one_packet
            a_zero_checksum_arrives_only_where_accepted
            a_closed_port_answers_with_an_icmp_error
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
            every_path_mtu_discovery_mode_sends
            a_waived_source_check_sends_from_an_absent_address
            a_group_filter_chooses_the_socket
            ipv6_memberships_take_their_datagrams
            close_frees_the_port
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
