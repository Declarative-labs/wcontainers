//! The tests every implementation of wcontainers-raw-sockets runs, written against its traits alone.
//!
//! Each runs as root in a network namespace of its own, with loopback up, on links, routes, rules, queues and
//! firewall tables it builds there with iproute2 and nftables. A packet socket of the namespace sees what
//! leaves a link.
#![cfg(target_os = "linux")]

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::io::Write;
use std::os::fd::AsFd;
use std::pin::pin;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use etherparse::{
    IpDscp, IpEcn, IpNumber, Ipv4Header, Ipv4Options, Ipv6FlowLabel, Ipv6Header,
    Ipv6RawExtHeaderSlice, icmpv4, icmpv6,
};
use wcontainers_abi::HostError;
use wcontainers_icmp_sockets_conformance::{
    BROADCAST, Capture, DUMMY, FAIR_QUEUE, FAR, FLOW_LABEL, HOP_LIMIT, LOCAL, LOCAL_V6, MARK,
    MARK_PROHIBITED, NETWORK, ON_LINK, ON_LINK_V6, OTHER_CODE, OTHER_USER, OWNER_PROHIBITED,
    PATIENCE, PAYLOAD, QUIET, READABLE, REPORT_KEY, ROOT, SELF_LINK, SHOW_GROUPS, TIME_TO_LIVE,
    TWO_SEGMENTS, WAITED_SOCKETS, WAITS_ON_ONE, WRITABLE, Wakes, buffer, finished, finished_soon,
    index, ip, ip6, ipv4, ipv6, no_reports, only_asked_for, output, plain_fields, poll_with, seen,
    tc, version_of, write_setting,
};
use wcontainers_internet_protocol::{
    AnycastAddress, BroadcastPermission, ByteCount, BytesPerSecond, IcmpType, IpHeaderFields,
    IpSendFields, IpSocketSetting, IpVersion, Ipv4HeaderFields, Ipv4RouteSelectors,
    Ipv6HeaderFields, Ipv6RouteSelectors, LocalFragmentation, MaximumTransmissionUnit, Membership,
    MobileAddressPreference, MobilityHeaderType, MulticastGroup, MulticastMembership,
    NeighbourConfirmation, PacketMark, PacketPriority, PathMtuDiscovery, Port, Received,
    ReceivedPacketFields, Report, RouteScope, RouterAlertValue, SendError, SourceAddressCheck,
    SourceFilter, SourcePreferences, TemporaryAddressPreference, TimestampSource, Timestamping,
    TransmitEvent, TransmitReportRequest, TransmitStage, TransportPorts, UpperLayer,
    UpperLayerSelector,
};
use wcontainers_raw_sockets::{
    ChecksumOffset, HeaderIncludedPacket, MulticastRouting, OutgoingReassembly, RawPacket,
    RawSocketSetting, RawSockets, RouterAlert, RouterAlertIsolation,
};
use wcontainers_route_resolver_conformance::seconds_since_confirmed;

#[doc(hidden)]
pub use wcontainers_configurable_conformance as configurable_suite;
/// The ICMP suite, whose wait and allocation tools serve raw sockets too.
pub use wcontainers_icmp_sockets_conformance as icmp_suite;

/// Moves the calling thread into a new network namespace, with loopback up, which needs `CAP_SYS_ADMIN`.
pub fn enter_network_namespace() {
    wcontainers_icmp_sockets_conformance::enter_network_namespace();
}

/// `IPPROTO_RAW`: a socket of it writes its own headers, and only it takes IPv6 router alerts.
pub const RAW_PROTOCOL: IpNumber = IpNumber(linux_raw_sys::net::IPPROTO_RAW as u8);
/// The type and length of the IPv6 router alert option of a hop-by-hop header.
const ROUTER_ALERT_HOP_OPTION: [u8; 2] = [5, 2];
/// A payload longer than the link of the check that sends one too long.
const OVERSIZED_PAYLOAD: usize = 2040;
/// A payload that a pacing queue holds in the send buffer for a while.
const HELD_PAYLOAD: usize = 1020;
const RAW_PACING: BytesPerSecond = BytesPerSecond(16_000);
/// How many packets a check sends at a socket whose receive buffer is the smallest.
const FLOODING_SENDS: usize = 63;
/// iproute2's brief listing of a link: its name, state and link address.
const BRIEF_LINK: [&str; 3] = ["-br", "link", "show"];
/// The network namespace of the calling thread.
const THREAD_NAMESPACE: &str = "/proc/thread-self/ns/net";
/// A protocol number for experiments, which nothing else of the namespace sends or takes.
pub const EXPERIMENT: IpNumber = IpNumber::EXPERIMENTAL_AND_TESTING_0;
/// The other protocol number for experiments.
const OTHER_EXPERIMENT: IpNumber = IpNumber::EXPERIMENTAL_AND_TESTING_1;
/// Type 253 only, as a rule takes it.
const EXPERIMENT_PROHIBITED: &str = "rule add ipproto 253 prohibit";
const NFTABLES: &str = "nft";
const RULESET_FROM_INPUT: [&str; 2] = ["-f", "-"];
/// A firewall table that tracks connections of locally made packets, which makes Linux reassemble their fragments.
const TRACKED_OUTPUT: &str = "table ip tracked { chain output { type filter hook output priority 0; ct state new counter; }; }";
/// A pair of linked virtual links: what leaves the first arrives at the second.
const LINKED_PAIR: &str = "link add ra type veth peer name rb
link set ra up
link set rb up
address add 10.6.0.1/24 dev ra
address add fd06::1/64 dev ra nodad
route add 10.7.0.0/24 dev ra
route add fd07::/64 dev ra";
const FIRST_OF_PAIR: &str = "ra";
const SECOND_OF_PAIR: &str = "rb";
/// The second link forwards what it takes and checks no source.
const FORWARDING: [(&str, &str); 4] = [
    ("/proc/sys/net/ipv4/ip_forward", "1"),
    ("/proc/sys/net/ipv4/conf/all/rp_filter", "0"),
    ("/proc/sys/net/ipv4/conf/rb/rp_filter", "0"),
    ("/proc/sys/net/ipv6/conf/all/forwarding", "1"),
];
/// A destination beyond the pair, which the second link forwards back out of the first.
const BEYOND: Ipv4Addr = Ipv4Addr::new(10, 7, 0, 1);
const BEYOND_V6: Ipv6Addr = Ipv6Addr::new(0xfd07, 0, 0, 0, 0, 0, 0, 1);
/// A source the namespace does not have, which a forwarded packet comes from.
const ELSEWHERE: Ipv4Addr = Ipv4Addr::new(10, 5, 0, 9);
const ELSEWHERE_V6: Ipv6Addr = Ipv6Addr::new(0xfd05, 0, 0, 0, 0, 0, 0, 9);
const ROUTER_ALERT_VALUE: RouterAlertValue = RouterAlertValue(0x2a);
const MULTICAST_FORWARDING: &str = "/proc/sys/net/ipv4/conf/all/mc_forwarding";
const MULTICAST_FORWARDING_V6: &str = "/proc/sys/net/ipv6/conf/all/mc_forwarding";
/// The IPv4 option of a router alert, then the end of the options.
const ROUTER_ALERT_OPTION: [u8; 4] = [148, 4, 0, 0];
/// An offset into a payload where a checksum is written.
const CHECKSUM_OFFSET: ChecksumOffset = ChecksumOffset(2);
const IDENTIFICATION: u16 = 0x1d1d;
static NO_OPTIONS: std::sync::LazyLock<Ipv4Options> = std::sync::LazyLock::new(Ipv4Options::new);

fn under(protocol: IpNumber) -> UpperLayer {
    UpperLayer {
        protocol,
        selector: None,
    }
}

fn v4_route(destination: Ipv4Addr, upper_layer: UpperLayer) -> Ipv4RouteSelectors<'static> {
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
        upper_layer,
    }
}

fn v6_route(destination: Ipv6Addr, upper_layer: UpperLayer) -> Ipv6RouteSelectors<'static> {
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
        upper_layer,
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

/// The plain header of a payload of the protocol to the address.
fn header_to(destination: IpAddr, protocol: IpNumber) -> IpHeaderFields<'static> {
    match destination {
        IpAddr::V4(address) => IpHeaderFields::V4(v4_header(v4_route(address, under(protocol)))),
        IpAddr::V6(address) => IpHeaderFields::V6(v6_header(v6_route(address, under(protocol)))),
    }
}

pub fn payload_to<'a>(destination: IpAddr, protocol: IpNumber, payload: &'a [u8]) -> RawPacket<'a> {
    RawPacket::Payload {
        header: header_to(destination, protocol),
        payload,
    }
}

fn opened<S: RawSockets>(sockets: &S, version: IpVersion, protocol: IpNumber) -> S::Socket {
    sockets
        .open(version, protocol, ROOT)
        .expect("a raw socket opens")
}

fn set<S: RawSockets>(sockets: &S, socket: &S::Socket, setting: RawSocketSetting<'_>) {
    sockets
        .configure(socket, setting)
        .unwrap_or_else(|error| panic!("{setting:?}: {error:?}"));
}

fn send<S: RawSockets>(
    sockets: &S,
    socket: &S::Socket,
    packet: RawPacket<'_>,
) -> Result<(), SendError> {
    sockets.send_to(socket, packet, plain_fields())
}

/// Waits for what the socket receives next.
fn receive<S: RawSockets>(sockets: &S, socket: &S::Socket, into: &mut [u8]) -> Received {
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

fn receive_packet<S: RawSockets>(
    sockets: &S,
    socket: &S::Socket,
    into: &mut [u8],
) -> ReceivedPacketFields {
    match receive(sockets, socket, into) {
        Received::Packet(fields) => fields,
        other => panic!("a packet, not {other:?}"),
    }
}

fn nothing_arrives<S: RawSockets>(sockets: &S, socket: &S::Socket) -> bool {
    std::thread::sleep(QUIET);
    sockets.receive(socket, &mut buffer()) == Err(HostError::WouldBlock)
}

/// Accepts an honest refusal of what a host may be unable to send, and checks anything else.
fn unless_refused(result: Result<(), SendError>, check: impl FnOnce(Result<(), SendError>)) {
    if result != Err(SendError::Host(HostError::NotSupported)) {
        check(result);
    }
}

/// An IPv4 packet of the protocol with its own header, written whole here.
fn ipv4_packet(
    source: Ipv4Addr,
    destination: Ipv4Addr,
    protocol: IpNumber,
    options: &[u8],
    more_fragments: bool,
    payload: &[u8],
) -> Vec<u8> {
    let mut header = Ipv4Header::new(
        u16::MIN,
        TIME_TO_LIVE.0,
        protocol,
        source.octets(),
        destination.octets(),
    )
    .expect("an IPv4 header");
    header.identification = IDENTIFICATION;
    header.more_fragments = more_fragments;
    header.options = Ipv4Options::try_from(options).expect("IPv4 options");
    header
        .set_payload_len(payload.len())
        .expect("a payload length");
    header.header_checksum = header.calc_header_checksum();
    let mut packet = header.to_bytes().to_vec();
    packet.extend_from_slice(payload);
    packet
}

/// An IPv6 packet with its own header, the given extension header bytes and then the payload under the protocol.
fn ipv6_packet(
    source: Ipv6Addr,
    destination: Ipv6Addr,
    next_header: IpNumber,
    extensions: &[u8],
    payload: &[u8],
) -> Vec<u8> {
    let header = Ipv6Header {
        traffic_class: u8::MIN,
        flow_label: Ipv6FlowLabel::ZERO,
        payload_length: (extensions.len() + payload.len()) as u16,
        next_header,
        hop_limit: HOP_LIMIT.0,
        source: source.octets(),
        destination: destination.octets(),
    };
    let mut packet = header.to_bytes().to_vec();
    packet.extend_from_slice(extensions);
    packet.extend_from_slice(payload);
    packet
}

fn header_included_v4<'a>(route: Ipv4RouteSelectors<'a>, packet: &'a [u8]) -> RawPacket<'a> {
    RawPacket::HeaderIncluded(HeaderIncludedPacket::V4 {
        route,
        reassembly: OutgoingReassembly::Allowed,
        packet,
    })
}

fn header_included_v6<'a>(route: Ipv6RouteSelectors<'a>, packet: &'a [u8]) -> RawPacket<'a> {
    RawPacket::HeaderIncluded(HeaderIncludedPacket::V6 { route, packet })
}

/// An ICMP echo request with its checksum, as a raw IPv4 socket must send it.
fn echo_request(payload: &[u8]) -> Vec<u8> {
    let header = etherparse::Icmpv4Header::with_checksum(
        etherparse::Icmpv4Type::EchoRequest(etherparse::IcmpEchoHeader {
            id: IDENTIFICATION,
            seq: u16::MIN,
        }),
        payload,
    );
    let mut message = header.to_bytes().to_vec();
    message.extend_from_slice(payload);
    message
}

/// An ICMPv6 echo request without its checksum, which Linux writes for a raw ICMPv6 socket.
fn echo_request_v6(payload: &[u8]) -> Vec<u8> {
    let mut message = vec![icmpv6::TYPE_ECHO_REQUEST, u8::MIN];
    message.resize(etherparse::Icmpv6Header::MIN_LEN, u8::MIN);
    message.extend_from_slice(payload);
    message
}

fn icmp_selector(message: &[u8]) -> Option<UpperLayerSelector> {
    Some(UpperLayerSelector::Icmp {
        message_type: wcontainers_internet_protocol::IcmpType(message[0]),
        code: wcontainers_internet_protocol::IcmpCode(message[1]),
    })
}

/// Raw sockets open for each version and protocol, and not for protocol zero.
pub fn open_takes_each_version_and_protocol<S: RawSockets>(sockets: S) {
    for (version, protocol) in [
        (IpVersion::V4, IpNumber::ICMP),
        (IpVersion::V4, IpNumber::UDP),
        (IpVersion::V4, EXPERIMENT),
        (IpVersion::V4, RAW_PROTOCOL),
        (IpVersion::V6, IpNumber::IPV6_ICMP),
        (IpVersion::V6, EXPERIMENT),
        (IpVersion::V6, RAW_PROTOCOL),
    ] {
        let socket = opened(&sockets, version, protocol);
        sockets.close(socket);
    }
    assert!(
        sockets
            .open(IpVersion::V4, IpNumber(u8::MIN), ROOT)
            .is_err()
    );
}

/// A payload leaves under the socket's protocol with the TTL, DSCP, ECN, options and source of its IPv4 header.
pub fn a_payload_goes_out_with_its_ipv4_header<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    let socket = opened(&sockets, IpVersion::V4, EXPERIMENT);
    let options = Ipv4Options::from(ROUTER_ALERT_OPTION);
    let route = Ipv4RouteSelectors {
        source: Some(LOCAL),
        dscp: IpDscp::try_new(IpDscp::MAX_U8).unwrap(),
        ecn: IpEcn::Ect0,
        options: &options,
        ..v4_route(ON_LINK, under(EXPERIMENT))
    };
    let header = IpHeaderFields::V4(Ipv4HeaderFields {
        time_to_live: Some(TIME_TO_LIVE),
        ..v4_header(route)
    });
    let capture = Capture::on(DUMMY);
    send(
        &sockets,
        &socket,
        RawPacket::Payload {
            header,
            payload: PAYLOAD,
        },
    )
    .expect("the payload is sent");
    let packet = capture.next(|packet| seen(packet).protocol == EXPERIMENT);
    let header = ipv4(&packet);
    assert_eq!(
        (header.ttl(), header.dcp(), header.ecn()),
        (TIME_TO_LIVE.0, route.dscp, route.ecn)
    );
    assert_eq!(header.options(), ROUTER_ALERT_OPTION);
    assert_eq!(
        (header.source_addr(), header.destination_addr()),
        (LOCAL, ON_LINK)
    );
    assert_eq!(seen(&packet).payload, PAYLOAD);
}

/// A payload leaves with the hop limit, traffic class, flow label, source, extension headers and routing header of its IPv6 header.
pub fn a_payload_goes_out_with_its_ipv6_header<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    /// An extension header of eight bytes: next header, length, then padding (PadN of four).
    const PADDING: [u8; 8] = [0, 0, 1, 4, 0, 0, 0, 0];
    let padding = Ipv6RawExtHeaderSlice::from_slice(&PADDING).unwrap();
    // A segment routing header of two segments, the next of them on the link.
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
    let routing = Ipv6RawExtHeaderSlice::from_slice(&routing).unwrap();
    let socket = opened(&sockets, IpVersion::V6, EXPERIMENT);
    let route = Ipv6RouteSelectors {
        source: Some(LOCAL_V6),
        dscp: IpDscp::try_new(IpDscp::MAX_U8).unwrap(),
        ecn: IpEcn::Ect1,
        flow_label: Ipv6FlowLabel::try_new(FLOW_LABEL).unwrap(),
        routing_header: Some(routing),
        ..v6_route(FAR_V6_SEGMENT, under(EXPERIMENT))
    };
    let header = IpHeaderFields::V6(Ipv6HeaderFields {
        hop_limit: Some(HOP_LIMIT),
        hop_by_hop_options: Some(padding.clone()),
        routing_destination_options: Some(padding.clone()),
        destination_options: Some(padding),
        ..v6_header(route.clone())
    });
    let capture = Capture::on(DUMMY);
    send(
        &sockets,
        &socket,
        RawPacket::Payload {
            header,
            payload: PAYLOAD,
        },
    )
    .expect("the payload is sent");
    let packet = capture.next(|packet| seen(packet).protocol == EXPERIMENT);
    let header = ipv6(&packet);
    assert_eq!(header.hop_limit(), HOP_LIMIT.0);
    assert_eq!((header.dscp(), header.ecn()), (route.dscp, route.ecn));
    assert_eq!(header.flow_label(), route.flow_label);
    assert_eq!(
        (header.source_addr(), header.destination_addr()),
        (LOCAL_V6, ON_LINK_V6)
    );
    let seen = seen(&packet);
    let kinds: Vec<IpNumber> = seen.extensions.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(
        kinds,
        [
            IpNumber::IPV6_HEADER_HOP_BY_HOP,
            IpNumber::IPV6_DESTINATION_OPTIONS,
            IpNumber::IPV6_ROUTE_HEADER,
            IpNumber::IPV6_DESTINATION_OPTIONS
        ]
    );
    assert_eq!(seen.payload, PAYLOAD);
}

/// The last segment of the routing header the IPv6 check sends, beyond the link.
const FAR_V6_SEGMENT: Ipv6Addr = Ipv6Addr::new(0xfd09, 0, 0, 0, 0, 0, 0, 9);

/// An IPv6 socket of `IPPROTO_RAW` sends a payload under the protocol its header asks for.
pub fn an_ipv6_raw_protocol_socket_sends_under_the_asked_protocol<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    let socket = opened(&sockets, IpVersion::V6, RAW_PROTOCOL);
    let capture = Capture::on(DUMMY);
    send(
        &sockets,
        &socket,
        payload_to(IpAddr::V6(ON_LINK_V6), EXPERIMENT, PAYLOAD),
    )
    .expect("the payload is sent");
    let packet = capture.next(|packet| seen(packet).protocol == EXPERIMENT);
    assert_eq!(seen(&packet).payload, PAYLOAD);
}

/// A payload under another protocol than the socket's, or with an upper layer other than its message's, is not sent as asked.
pub fn a_payload_under_another_upper_layer_is_not_sent_as_asked<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    let capture = Capture::on(DUMMY);
    let socket = opened(&sockets, IpVersion::V4, EXPERIMENT);
    unless_refused(
        send(
            &sockets,
            &socket,
            payload_to(IpAddr::V4(ON_LINK), OTHER_EXPERIMENT, PAYLOAD),
        ),
        |sent| {
            sent.expect("the payload is sent");
            let packet = capture.next(|packet| ipv4(packet).destination_addr() == ON_LINK);
            assert_eq!(seen(&packet).protocol, OTHER_EXPERIMENT);
        },
    );
    /// The port the second rule below prohibits.
    const PROHIBITED_PORT: Port = Port(9);
    // Rules prohibit the selectors asked for below, which the payloads themselves do not carry.
    const OTHER_CODE_PROHIBITED: &str = "rule add ipproto icmp dport 2049 prohibit
rule add ipproto 253 dport 9 prohibit";
    ip(OTHER_CODE_PROHIBITED);
    let prohibited = Err(SendError::Host(HostError::Errno(libc::EACCES)));
    let routed_as_asked =
        |sent: Result<(), SendError>| assert_eq!(sent, prohibited, "routed by its own fields");
    let socket = opened(&sockets, IpVersion::V4, IpNumber::ICMP);
    let message = echo_request(PAYLOAD);
    let mut route = v4_route(ON_LINK, under(IpNumber::ICMP));
    route.upper_layer.selector = Some(UpperLayerSelector::Icmp {
        message_type: IcmpType(icmpv4::TYPE_ECHO_REQUEST),
        code: OTHER_CODE,
    });
    let routed = |route: Ipv4RouteSelectors<'_>, socket: &S::Socket, payload: &[u8]| {
        send(
            &sockets,
            socket,
            RawPacket::Payload {
                header: IpHeaderFields::V4(v4_header(route)),
                payload,
            },
        )
    };
    unless_refused(routed(route, &socket, &message), routed_as_asked);
    route.upper_layer.selector = icmp_selector(&message);
    routed(route, &socket, &message).expect("an echo request routed by its own type is sent");
    let socket = opened(&sockets, IpVersion::V4, EXPERIMENT);
    let mut route = v4_route(ON_LINK, under(EXPERIMENT));
    route.upper_layer.selector = Some(UpperLayerSelector::Ports(TransportPorts {
        source: PROHIBITED_PORT,
        destination: PROHIBITED_PORT,
    }));
    unless_refused(routed(route, &socket, PAYLOAD), routed_as_asked);
}

/// A packet with its own header leaves as it was written.
pub fn header_included_packets_go_out_as_written<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    let capture = Capture::on(DUMMY);
    let packet = ipv4_packet(LOCAL, ON_LINK, EXPERIMENT, &[], false, PAYLOAD);
    let socket = opened(&sockets, IpVersion::V4, EXPERIMENT);
    send(
        &sockets,
        &socket,
        header_included_v4(v4_route(ON_LINK, under(EXPERIMENT)), &packet),
    )
    .expect("the packet is sent");
    assert_eq!(
        capture.next(|packet| seen(packet).protocol == EXPERIMENT),
        packet
    );
    let packet = ipv6_packet(LOCAL_V6, ON_LINK_V6, EXPERIMENT, &[], PAYLOAD);
    let socket = opened(&sockets, IpVersion::V6, EXPERIMENT);
    send(
        &sockets,
        &socket,
        header_included_v6(v6_route(ON_LINK_V6, under(EXPERIMENT)), &packet),
    )
    .expect("the packet is sent");
    assert_eq!(
        capture.next(|packet| seen(packet).protocol == EXPERIMENT),
        packet
    );
    // The same socket sends a payload again once its packets wrote their own headers.
    send(
        &sockets,
        &socket,
        payload_to(IpAddr::V6(ON_LINK_V6), EXPERIMENT, PAYLOAD),
    )
    .expect("the payload is sent");
    let sent = capture.next(|packet| seen(packet).protocol == EXPERIMENT);
    assert_eq!(seen(&sent).payload, PAYLOAD);
}

/// A packet with its own header is routed by its route's protocol and mark, which rules choose on.
pub fn header_included_packets_take_their_route<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    ip(EXPERIMENT_PROHIBITED);
    ip6(EXPERIMENT_PROHIBITED);
    ip(MARK_PROHIBITED);
    let prohibited = Err(SendError::Host(HostError::Errno(libc::EACCES)));
    let socket = opened(&sockets, IpVersion::V4, RAW_PROTOCOL);
    let packet = ipv4_packet(LOCAL, ON_LINK, EXPERIMENT, &[], false, PAYLOAD);
    for (route, answer) in [
        (v4_route(ON_LINK, under(EXPERIMENT)), prohibited),
        (v4_route(ON_LINK, under(OTHER_EXPERIMENT)), Ok(())),
        (
            Ipv4RouteSelectors {
                mark: MARK,
                ..v4_route(ON_LINK, under(OTHER_EXPERIMENT))
            },
            prohibited,
        ),
    ] {
        assert_eq!(
            send(&sockets, &socket, header_included_v4(route, &packet)),
            answer,
            "{route:?}"
        );
    }
    let socket = opened(&sockets, IpVersion::V6, RAW_PROTOCOL);
    let packet = ipv6_packet(LOCAL_V6, ON_LINK_V6, EXPERIMENT, &[], PAYLOAD);
    for (protocol, answer) in [(EXPERIMENT, prohibited), (OTHER_EXPERIMENT, Ok(()))] {
        assert_eq!(
            send(
                &sockets,
                &socket,
                header_included_v6(v6_route(ON_LINK_V6, under(protocol)), &packet)
            ),
            answer,
            "{protocol:?}"
        );
    }
}

/// A fragment with its own header waits for the rest of its packet where connections are tracked, unless reassembly is withdrawn.
pub fn withdrawn_reassembly_lets_a_fragment_leave<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    let mut nft = Command::new(NFTABLES)
        .args(RULESET_FROM_INPUT)
        .stdin(Stdio::piped())
        .spawn()
        .expect("nftables runs");
    nft.stdin
        .take()
        .expect("nftables' input")
        .write_all(TRACKED_OUTPUT.as_bytes())
        .expect("nftables reads its ruleset");
    assert!(nft.wait().expect("nftables ends").success());
    let socket = opened(&sockets, IpVersion::V4, EXPERIMENT);
    let fragment = ipv4_packet(LOCAL, ON_LINK, EXPERIMENT, &[], true, PAYLOAD);
    let capture = Capture::on(DUMMY);
    for (reassembly, leaves) in [
        (OutgoingReassembly::Allowed, false),
        (OutgoingReassembly::Disabled, true),
    ] {
        let packet = RawPacket::HeaderIncluded(HeaderIncludedPacket::V4 {
            route: v4_route(ON_LINK, under(EXPERIMENT)),
            reassembly,
            packet: &fragment,
        });
        send(&sockets, &socket, packet).expect("the fragment is sent");
        assert_eq!(
            capture
                .within(QUIET, |packet| seen(packet).protocol == EXPERIMENT)
                .is_some(),
            leaves,
            "{reassembly:?}"
        );
    }
}

/// An IPv4 raw socket receives the packets of its protocol whole, header included.
pub fn ipv4_packets_arrive_whole<S: RawSockets>(sockets: S) {
    let socket = opened(&sockets, IpVersion::V4, IpNumber::ICMP);
    let message = echo_request(PAYLOAD);
    let mut route = v4_route(Ipv4Addr::LOCALHOST, under(IpNumber::ICMP));
    route.upper_layer.selector = icmp_selector(&message);
    let header = Ipv4HeaderFields {
        time_to_live: Some(TIME_TO_LIVE),
        ..v4_header(route)
    };
    send(
        &sockets,
        &socket,
        RawPacket::Payload {
            header: IpHeaderFields::V4(header),
            payload: &message,
        },
    )
    .expect("the echo request is sent");
    let mut into = buffer();
    let fields = receive_packet(&sockets, &socket, &mut into);
    let packet = &into[..fields.length];
    let header = ipv4(packet);
    assert_eq!(usize::from(header.total_len()), fields.length);
    assert_eq!(header.ttl(), TIME_TO_LIVE.0);
    assert_eq!(header.protocol(), IpNumber::ICMP);
    assert_eq!(seen(packet).payload, message);
    assert_eq!(fields.interface, index(SELF_LINK));
    assert_eq!(fields.specific_destination, Some(Ipv4Addr::LOCALHOST));
}

/// Assured forwarding class 4, low drop, which no default sets.
const ARRIVING_DSCP: IpDscp = IpDscp::AF41;

/// An IPv6 raw socket receives the packets of its protocol with their header and extension headers written in front.
pub fn ipv6_packets_arrive_with_their_headers<S: RawSockets>(sockets: S) {
    /// A hop-by-hop header of eight bytes: next header, length, then padding (PadN of four).
    const PADDING: [u8; 8] = [0, 0, 1, 4, 0, 0, 0, 0];
    let padding = Ipv6RawExtHeaderSlice::from_slice(&PADDING).unwrap();
    let socket = opened(&sockets, IpVersion::V6, IpNumber::IPV6_ICMP);
    let message = echo_request_v6(PAYLOAD);
    let mut route = v6_route(Ipv6Addr::LOCALHOST, under(IpNumber::IPV6_ICMP));
    route.upper_layer.selector = icmp_selector(&message);
    route.flow_label = Ipv6FlowLabel::try_new(FLOW_LABEL).unwrap();
    route.dscp = ARRIVING_DSCP;
    route.ecn = IpEcn::Ect0;
    let header = Ipv6HeaderFields {
        hop_limit: Some(HOP_LIMIT),
        hop_by_hop_options: Some(padding),
        ..v6_header(route.clone())
    };
    send(
        &sockets,
        &socket,
        RawPacket::Payload {
            header: IpHeaderFields::V6(header),
            payload: &message,
        },
    )
    .expect("the echo request is sent");
    let mut into = buffer();
    let fields = receive_packet(&sockets, &socket, &mut into);
    let packet = &into[..fields.length];
    let header = ipv6(packet);
    assert_eq!(
        usize::from(header.payload_length()) + Ipv6Header::LEN,
        fields.length
    );
    assert_eq!(header.hop_limit(), HOP_LIMIT.0);
    assert_eq!(header.flow_label().value(), FLOW_LABEL);
    assert_eq!((header.dscp(), header.ecn()), (route.dscp, route.ecn));
    let seen = seen(packet);
    assert_eq!(
        (seen.source, seen.destination, seen.protocol),
        (
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpNumber::IPV6_ICMP
        )
    );
    // The header arrives chained to the payload's protocol.
    let mut chained = PADDING;
    chained[0] = IpNumber::IPV6_ICMP.0;
    assert_eq!(
        seen.extensions,
        [(IpNumber::IPV6_HEADER_HOP_BY_HOP, &chained[..])]
    );
    assert_eq!(seen.payload[0], icmpv6::TYPE_ECHO_REQUEST);
    assert!(wcontainers_icmp_sockets_conformance::checks_out(
        seen.payload,
        &seen
    ));
}

pub fn receive_with_nothing_queued_would_block<S: RawSockets>(sockets: S) {
    for (version, protocol) in [(IpVersion::V4, EXPERIMENT), (IpVersion::V6, EXPERIMENT)] {
        let socket = opened(&sockets, version, protocol);
        assert_eq!(
            sockets.receive(&socket, &mut buffer()),
            Err(HostError::WouldBlock)
        );
    }
}

/// A buffer that cuts a packet in its IP header, after it or in its payload takes its start and tells its whole
/// length.
pub fn a_short_buffer_keeps_the_whole_length<S: RawSockets>(sockets: S) {
    for (to, header) in [
        (IpAddr::V4(Ipv4Addr::LOCALHOST), Ipv4Header::MIN_LEN),
        (IpAddr::V6(Ipv6Addr::LOCALHOST), Ipv6Header::LEN),
    ] {
        let socket = opened(&sockets, version_of(to), EXPERIMENT);
        let sent = || {
            send(&sockets, &socket, payload_to(to, EXPERIMENT, PAYLOAD))
                .expect("the payload is sent")
        };
        // The first is shorter than the payload Linux gives an IPv6 socket.
        let cuts = [
            PAYLOAD.len() / 2,
            header / 2,
            header,
            header + PAYLOAD.len() / 2,
        ];
        let whole = short_buffers_take_the_start(&sockets, &socket, sent, &cuts);
        assert_eq!(whole.len(), header + PAYLOAD.len(), "{to}");
        let seen = seen(&whole);
        assert_eq!((seen.protocol, seen.payload), (EXPERIMENT, PAYLOAD), "{to}");
    }
}

/// Receives what `sent` makes arrive, whole and then into buffers of each length of `cuts`, which must take its
/// start and tell its whole length. Gives the whole.
fn short_buffers_take_the_start<S: RawSockets>(
    sockets: &S,
    socket: &S::Socket,
    sent: impl Fn(),
    cuts: &[usize],
) -> Vec<u8> {
    sent();
    let mut whole = buffer();
    let length = receive_packet(sockets, socket, &mut whole).length;
    whole.truncate(length);
    for &short in cuts {
        sent();
        let mut into = vec![u8::MIN; short];
        let fields = receive_packet(sockets, socket, &mut into);
        assert_eq!(fields.length, whole.len(), "into {short}");
        let version = version_of(seen(&whole).source);
        assert_eq!(
            unnumbered(version, into),
            unnumbered(version, whole[..short].to_vec()),
            "into {short}"
        );
    }
    whole
}

/// The start of a packet without what differs between two sends of it: an IPv4 header's identification and the
/// checksum over it.
fn unnumbered(version: IpVersion, mut start: Vec<u8>) -> Vec<u8> {
    const IPV4_NUMBERED: [core::ops::Range<usize>; 2] = [4..6, 10..12];
    if version == IpVersion::V4 {
        for range in IPV4_NUMBERED {
            let range = range.start.min(start.len())..range.end.min(start.len());
            start[range].fill(u8::MIN);
        }
    }
    start
}

/// A packet sent with a mark and a priority arrives at another raw socket of its protocol with them.
pub fn received_packets_carry_their_mark_and_priority<S: RawSockets>(sockets: S) {
    const PRIORITY: PacketPriority = PacketPriority(5);
    for to in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        let receiver = opened(&sockets, version_of(to), EXPERIMENT);
        let sender = opened(&sockets, version_of(to), EXPERIMENT);
        let mut packet = payload_to(to, EXPERIMENT, PAYLOAD);
        if let RawPacket::Payload { header, .. } = &mut packet {
            match header {
                IpHeaderFields::V4(header) => header.route.mark = MARK,
                IpHeaderFields::V6(header) => header.route.mark = MARK,
            }
        }
        let fields = IpSendFields {
            priority: PRIORITY,
            ..plain_fields()
        };
        sockets
            .send_to(&sender, packet, fields)
            .expect("the payload is sent");
        let fields = receive_packet(&sockets, &receiver, &mut buffer());
        assert_eq!((fields.mark, fields.priority), (MARK, PRIORITY), "{to}");
        send(&sockets, &sender, payload_to(to, EXPERIMENT, PAYLOAD)).expect("the payload is sent");
        let fields = receive_packet(&sockets, &receiver, &mut buffer());
        assert_eq!(
            (fields.mark, fields.priority),
            (PacketMark(u32::MIN), PacketPriority(u32::MIN)),
            "{to}: a plain send"
        );
    }
}

/// A checksum offset makes Linux write the payload's checksum there, and none leaves it as written.
pub fn a_checksum_offset_is_written<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    let socket = opened(&sockets, IpVersion::V6, EXPERIMENT);
    let capture = Capture::on(DUMMY);
    let zeros = [u8::MIN; etherparse::Icmpv6Header::MIN_LEN];
    for (offset, written) in [(Some(CHECKSUM_OFFSET), true), (None, false)] {
        set(&sockets, &socket, RawSocketSetting::Checksum(offset));
        send(
            &sockets,
            &socket,
            payload_to(IpAddr::V6(ON_LINK_V6), EXPERIMENT, &zeros),
        )
        .unwrap();
        let packet = capture.next(|packet| seen(packet).protocol == EXPERIMENT);
        let seen = seen(&packet);
        assert_eq!(seen.payload != zeros, written, "{offset:?}");
        if written {
            let sum = etherparse::checksum::Sum16BitWords::new()
                .add_16bytes(LOCAL_V6.octets())
                .add_16bytes(ON_LINK_V6.octets())
                .add_4bytes((zeros.len() as u32).to_be_bytes())
                .add_2bytes([u8::MIN, EXPERIMENT.0])
                .add_slice(seen.payload)
                .ones_complement();
            assert_eq!(sum, u16::MIN, "{packet:?}");
        }
    }
    let v4 = opened(&sockets, IpVersion::V4, EXPERIMENT);
    assert!(
        sockets
            .configure(&v4, RawSocketSetting::Checksum(Some(CHECKSUM_OFFSET)))
            .is_err()
    );
}

/// Builds the pair of linked links that forward packets beyond them back out of the first, and their neighbours.
fn forwarding_pair() {
    ip(LINKED_PAIR);
    for (path, value) in FORWARDING {
        write_setting(path, value);
    }
    // The link address of the second link, from iproute2's brief listing: name, state, address.
    let text = output(
        wcontainers_route_resolver_conformance::IP,
        &[&BRIEF_LINK[..], &[SECOND_OF_PAIR]].concat(),
    );
    let address = text.split_whitespace().nth(2).expect("a link address");
    ip(&format!(
        "neigh add {BEYOND} lladdr {address} dev {FIRST_OF_PAIR}\nneigh add {BEYOND_V6} lladdr {address} dev {FIRST_OF_PAIR}"
    ));
}

/// A router alert socket of the namespace takes the forwarded IPv4 packets of its protocol that carry the alert.
pub fn an_ipv4_router_alert_takes_forwarded_packets<S: RawSockets>(sockets: S) {
    forwarding_pair();
    let alerted = opened(&sockets, IpVersion::V4, EXPERIMENT);
    set(
        &sockets,
        &alerted,
        RawSocketSetting::RouterAlert(RouterAlert::Ipv4),
    );
    let sender = opened(&sockets, IpVersion::V4, EXPERIMENT);
    let packet = ipv4_packet(
        ELSEWHERE,
        BEYOND,
        EXPERIMENT,
        &ROUTER_ALERT_OPTION,
        false,
        PAYLOAD,
    );
    let forward = |sockets: &S| {
        send(
            sockets,
            &sender,
            header_included_v4(v4_route(BEYOND, under(EXPERIMENT)), &packet),
        )
        .expect("the packet is sent")
    };
    forward(&sockets);
    let mut into = buffer();
    let fields = receive_packet(&sockets, &alerted, &mut into);
    assert_eq!(seen(&into[..fields.length]).payload, PAYLOAD);
    assert_eq!(fields.interface, index(SECOND_OF_PAIR));
    set(
        &sockets,
        &alerted,
        RawSocketSetting::RouterAlert(RouterAlert::Off),
    );
    forward(&sockets);
    assert!(nothing_arrives(&sockets, &alerted));
}

/// IPv6 router alert sockets take forwarded packets with their alert's value, from other namespaces only unless isolated.
pub fn an_ipv6_router_alert_takes_forwarded_packets<S: RawSockets>(sockets: S) {
    forwarding_pair();
    let alert = |value, isolation| RouterAlert::Ipv6 { value, isolation };
    let own = opened(&sockets, IpVersion::V6, RAW_PROTOCOL);
    set(
        &sockets,
        &own,
        RawSocketSetting::RouterAlert(alert(
            ROUTER_ALERT_VALUE,
            RouterAlertIsolation::OwnNamespace,
        )),
    );
    let other_value = opened(&sockets, IpVersion::V6, RAW_PROTOCOL);
    set(
        &sockets,
        &other_value,
        RawSocketSetting::RouterAlert(alert(
            RouterAlertValue(ROUTER_ALERT_VALUE.0 + 1),
            RouterAlertIsolation::AllNamespaces,
        )),
    );
    let home = rustix::fs::open(
        THREAD_NAMESPACE,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .expect("this namespace opens");
    enter_network_namespace();
    let everywhere = opened(&sockets, IpVersion::V6, RAW_PROTOCOL);
    set(
        &sockets,
        &everywhere,
        RawSocketSetting::RouterAlert(alert(
            ROUTER_ALERT_VALUE,
            RouterAlertIsolation::AllNamespaces,
        )),
    );
    let isolated = opened(&sockets, IpVersion::V6, RAW_PROTOCOL);
    set(
        &sockets,
        &isolated,
        RawSocketSetting::RouterAlert(alert(
            ROUTER_ALERT_VALUE,
            RouterAlertIsolation::OwnNamespace,
        )),
    );
    rustix::thread::move_into_link_name_space(
        home.as_fd(),
        Some(rustix::thread::LinkNameSpaceType::Network),
    )
    .expect("the check's namespace is entered again");
    // A hop-by-hop header with the router alert option and then padding (PadN of none).
    let [high, low] = ROUTER_ALERT_VALUE.0.to_be_bytes();
    let [option, option_length] = ROUTER_ALERT_HOP_OPTION;
    let hop_by_hop = [
        EXPERIMENT.0,
        u8::MIN,
        option,
        option_length,
        high,
        low,
        1,
        u8::MIN,
    ];
    let packet = ipv6_packet(
        ELSEWHERE_V6,
        BEYOND_V6,
        IpNumber::IPV6_HEADER_HOP_BY_HOP,
        &hop_by_hop,
        PAYLOAD,
    );
    let sender = opened(&sockets, IpVersion::V6, RAW_PROTOCOL);
    send(
        &sockets,
        &sender,
        header_included_v6(
            v6_route(BEYOND_V6, under(IpNumber::IPV6_HEADER_HOP_BY_HOP)),
            &packet,
        ),
    )
    .expect("the packet is sent");
    let mut into = buffer();
    let fields = receive_packet(&sockets, &own, &mut into);
    let alerted = seen(&into[..fields.length]);
    assert_eq!(
        (alerted.source, alerted.destination),
        (IpAddr::V6(ELSEWHERE_V6), IpAddr::V6(BEYOND_V6))
    );
    assert_eq!(alerted.payload, PAYLOAD);
    for (socket, takes) in [
        (&everywhere, true),
        (&other_value, false),
        (&isolated, false),
    ] {
        assert_eq!(!nothing_arrives(&sockets, socket), takes);
    }
}

/// Multicast routing takes the namespace's one role and turns multicast forwarding on while it holds it.
pub fn multicast_routing_takes_the_namespace_role<S: RawSockets>(sockets: S) {
    for (version, protocol, forwarding) in [
        (IpVersion::V4, IpNumber::IGMP, MULTICAST_FORWARDING),
        (IpVersion::V6, IpNumber::IPV6_ICMP, MULTICAST_FORWARDING_V6),
    ] {
        let read = || {
            std::fs::read_to_string(forwarding)
                .expect("the forwarding setting")
                .trim()
                .parse::<u8>()
                .expect("a number")
        };
        let first = opened(&sockets, version, protocol);
        let second = opened(&sockets, version, protocol);
        let routing = |enabled| RawSocketSetting::MulticastRouting(enabled);
        set(&sockets, &first, routing(MulticastRouting::Enabled));
        assert_ne!(read(), u8::MIN, "{version:?}");
        assert!(
            sockets
                .configure(&second, routing(MulticastRouting::Enabled))
                .is_err()
        );
        set(&sockets, &first, routing(MulticastRouting::Disabled));
        assert_eq!(read(), u8::MIN, "{version:?}");
        set(&sockets, &second, routing(MulticastRouting::Enabled));
    }
}

/// Closing a socket gives up what it held: the namespace's multicast routing role, at once or once no child forked
/// meanwhile holds its descriptor.
pub fn close_gives_up_the_sockets_holdings<S: RawSockets>(sockets: S) {
    let routing = RawSocketSetting::MulticastRouting(MulticastRouting::Enabled);
    let first = opened(&sockets, IpVersion::V4, IpNumber::IGMP);
    set(&sockets, &first, routing);
    let second = opened(&sockets, IpVersion::V4, IpNumber::IGMP);
    assert!(sockets.configure(&second, routing).is_err());
    sockets.close(first);
    let start = Instant::now();
    let taken = loop {
        match sockets.configure(&second, routing) {
            Err(_) if start.elapsed() < PATIENCE => std::thread::sleep(Duration::from_millis(1)),
            other => break other,
        }
    };
    assert_eq!(taken, Ok(()));
}

/// The IP header fields that choose a payload's route are taken: interface, link scope, broadcast, a waived source, source preferences, owner, mark.
pub fn payload_routes_take_their_selectors<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    ip(OWNER_PROHIBITED);
    ip(MARK_PROHIBITED);
    let prohibited = Err(SendError::Host(HostError::Errno(libc::EACCES)));
    let capture = Capture::on(DUMMY);
    let socket = opened(&sockets, IpVersion::V4, EXPERIMENT);
    let sent = |route: Ipv4RouteSelectors<'_>| {
        send(
            &sockets,
            &socket,
            RawPacket::Payload {
                header: IpHeaderFields::V4(v4_header(route)),
                payload: PAYLOAD,
            },
        )
    };
    let route = v4_route(ON_LINK, under(EXPERIMENT));
    assert_eq!(
        sent(Ipv4RouteSelectors {
            mark: MARK,
            ..route
        }),
        prohibited
    );
    let elsewhere = Ipv4RouteSelectors {
        interface: Some(index(DUMMY)),
        ..v4_route(
            wcontainers_icmp_sockets_conformance::NOWHERE,
            under(EXPERIMENT),
        )
    };
    sent(elsewhere).expect("the payload is sent on the link");
    assert!(
        sent(Ipv4RouteSelectors {
            scope: RouteScope::Link,
            ..v4_route(FAR, under(EXPERIMENT))
        })
        .is_err()
    );
    assert_eq!(sent(v4_route(BROADCAST, under(EXPERIMENT))), prohibited);
    sent(Ipv4RouteSelectors {
        broadcast: BroadcastPermission::Allowed,
        ..v4_route(BROADCAST, under(EXPERIMENT))
    })
    .expect("the broadcast is sent");
    const ABSENT: Ipv4Addr = Ipv4Addr::new(10, 99, 0, 1);
    assert!(
        sent(Ipv4RouteSelectors {
            source: Some(ABSENT),
            ..route
        })
        .is_err()
    );
    sent(Ipv4RouteSelectors {
        source: Some(ABSENT),
        source_address_check: SourceAddressCheck::Waived,
        ..route
    })
    .expect("the payload is sent from the absent address");
    let from_absent = capture.next(|packet| {
        seen(packet).protocol == EXPERIMENT && seen(packet).source == IpAddr::V4(ABSENT)
    });
    assert_eq!(seen(&from_absent).payload, PAYLOAD);
    let other = sockets
        .open(IpVersion::V4, EXPERIMENT, OTHER_USER)
        .expect("a raw socket of another user opens");
    assert_eq!(
        sockets.send_to(
            &other,
            payload_to(IpAddr::V4(ON_LINK), EXPERIMENT, PAYLOAD),
            plain_fields()
        ),
        prohibited
    );
    let socket = opened(&sockets, IpVersion::V6, EXPERIMENT);
    ip6(OWNER_PROHIBITED);
    ip6(MARK_PROHIBITED);
    ip(wcontainers_icmp_sockets_conformance::TWO_ROUTES);
    let second_capture = Capture::on(wcontainers_icmp_sockets_conformance::SECOND);
    let sent = |route: Ipv6RouteSelectors<'_>| {
        send(
            &sockets,
            &socket,
            RawPacket::Payload {
                header: IpHeaderFields::V6(v6_header(route)),
                payload: PAYLOAD,
            },
        )
    };
    let route = v6_route(ON_LINK_V6, under(EXPERIMENT));
    assert_eq!(
        sent(Ipv6RouteSelectors {
            mark: MARK,
            ..route.clone()
        }),
        prohibited
    );
    let nowhere = v6_route(
        wcontainers_icmp_sockets_conformance::NOWHERE_V6,
        under(EXPERIMENT),
    );
    for (interface, capture) in [
        (None, &capture),
        (
            Some(wcontainers_icmp_sockets_conformance::SECOND),
            &second_capture,
        ),
    ] {
        sent(Ipv6RouteSelectors {
            interface: interface.map(index),
            ..nowhere.clone()
        })
        .expect("the payload is sent");
        capture.next(|packet| seen(packet).destination == IpAddr::V6(nowhere.destination));
    }
    const ABSENT_V6: Ipv6Addr = Ipv6Addr::new(0xfd99, 0, 0, 0, 0, 0, 0, 1);
    assert!(
        sent(Ipv6RouteSelectors {
            source: Some(ABSENT_V6),
            ..route.clone()
        })
        .is_err()
    );
    sent(Ipv6RouteSelectors {
        source: Some(ABSENT_V6),
        source_address_check: SourceAddressCheck::Waived,
        ..route
    })
    .expect("the payload is sent from the absent address");
    capture.next(|packet| {
        seen(packet).protocol == EXPERIMENT && seen(packet).source == IpAddr::V6(ABSENT_V6)
    });
    let other = sockets
        .open(IpVersion::V6, EXPERIMENT, OTHER_USER)
        .expect("a raw socket of another user opens");
    assert_eq!(
        sockets.send_to(
            &other,
            payload_to(IpAddr::V6(ON_LINK_V6), EXPERIMENT, PAYLOAD),
            plain_fields()
        ),
        prohibited
    );
}

/// A mobility header sent by its type as the route's selector leaves as given, with the checksum Linux writes.
pub fn a_mobility_header_is_sent_by_its_type<S: RawSockets>(sockets: S) {
    /// A binding refresh request: no next header, no length past eight bytes, type 0, then checksum and reserved.
    const BINDING_REFRESH_REQUEST: [u8; 8] = [59, 0, 0, 0, 0, 0, 0, 0];
    /// Where a mobility header keeps its type, after its next header and length.
    const MOBILITY_HEADER_TYPE: usize = 2;
    const CHECKSUM: core::ops::Range<usize> = 4..6;
    ip(NETWORK);
    let socket = opened(&sockets, IpVersion::V6, IpNumber::MOBILITY_HEADER);
    let capture = Capture::on(DUMMY);
    let route = v6_route(
        ON_LINK_V6,
        UpperLayer {
            protocol: IpNumber::MOBILITY_HEADER,
            selector: Some(UpperLayerSelector::MobilityHeader(MobilityHeaderType(
                BINDING_REFRESH_REQUEST[MOBILITY_HEADER_TYPE],
            ))),
        },
    );
    send(
        &sockets,
        &socket,
        RawPacket::Payload {
            header: IpHeaderFields::V6(v6_header(route)),
            payload: &BINDING_REFRESH_REQUEST,
        },
    )
    .expect("the mobility header is sent");
    let sent = capture.next(|packet| seen(packet).protocol == IpNumber::MOBILITY_HEADER);
    let seen = seen(&sent);
    let mut without_checksum = seen.payload.to_vec();
    without_checksum[CHECKSUM].fill(u8::MIN);
    assert_eq!(without_checksum, BINDING_REFRESH_REQUEST);
    let (IpAddr::V6(source), IpAddr::V6(destination)) = (seen.source, seen.destination) else {
        panic!("an IPv6 packet");
    };
    let sum = etherparse::checksum::Sum16BitWords::new()
        .add_16bytes(source.octets())
        .add_16bytes(destination.octets())
        .add_4bytes((seen.payload.len() as u32).to_be_bytes())
        .add_2bytes([u8::MIN, IpNumber::MOBILITY_HEADER.0])
        .add_slice(seen.payload);
    assert_eq!(sum.ones_complement(), u16::MIN, "the checksum Linux writes");
}

/// IPv6 source preferences choose between the link's public address and a temporary one.
pub fn ipv6_source_preferences_choose_the_source<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    let sources = wcontainers_icmp_sockets_conformance::sources_by_preference();
    let capture = Capture::on(DUMMY);
    let socket = opened(&sockets, IpVersion::V6, EXPERIMENT);
    for (temporary, source) in sources {
        let route = Ipv6RouteSelectors {
            source_preferences: SourcePreferences {
                temporary,
                mobile: MobileAddressPreference::Home,
            },
            ..v6_route(ON_LINK_V6, under(EXPERIMENT))
        };
        send(
            &sockets,
            &socket,
            RawPacket::Payload {
                header: IpHeaderFields::V6(v6_header(route)),
                payload: PAYLOAD,
            },
        )
        .expect("the payload is sent");
        let packet = capture.next(|packet| {
            seen(packet).protocol == EXPERIMENT
                && seen(packet).destination == IpAddr::V6(ON_LINK_V6)
        });
        assert_eq!(seen(&packet).source, IpAddr::V6(source), "{temporary:?}");
    }
}

/// Path MTU modes set the don't-fragment bit as Linux does; a payload over the MTU is too long where it may not be fragmented.
pub fn path_mtu_modes_and_too_long_payloads<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    const LINK_MTU: u32 = 1340;
    const SMALL_LINK: &str = "link set ia mtu 1340";
    let long = vec![u8::MAX; OVERSIZED_PAYLOAD];
    let capture = Capture::on(DUMMY);
    let socket = opened(&sockets, IpVersion::V4, EXPERIMENT);
    for (mode, dont_fragment) in [
        (PathMtuDiscovery::Dont, false),
        (PathMtuDiscovery::Want, true),
        (PathMtuDiscovery::Do, true),
        (PathMtuDiscovery::Probe, true),
        (PathMtuDiscovery::Interface, false),
        (PathMtuDiscovery::Omit, false),
    ] {
        let header = IpHeaderFields::V4(Ipv4HeaderFields {
            path_mtu_discovery: mode,
            ..v4_header(v4_route(ON_LINK, under(EXPERIMENT)))
        });
        send(
            &sockets,
            &socket,
            RawPacket::Payload {
                header,
                payload: PAYLOAD,
            },
        )
        .unwrap();
        let packet = capture.next(|packet| seen(packet).protocol == EXPERIMENT);
        assert_eq!(ipv4(&packet).dont_fragment(), dont_fragment, "{mode:?}");
    }
    ip(SMALL_LINK);
    let too_long = Err(SendError::MessageTooLong {
        maximum_transmission_unit: MaximumTransmissionUnit(LINK_MTU),
    });
    let header = IpHeaderFields::V4(Ipv4HeaderFields {
        path_mtu_discovery: PathMtuDiscovery::Do,
        ..v4_header(v4_route(ON_LINK, under(EXPERIMENT)))
    });
    assert_eq!(
        send(
            &sockets,
            &socket,
            RawPacket::Payload {
                header,
                payload: &long
            }
        ),
        too_long
    );
    let socket = opened(&sockets, IpVersion::V6, EXPERIMENT);
    // Linux forbids fragmenting a raw IPv6 payload of another protocol than UDP and ICMPv6 only with this mode.
    let header = IpHeaderFields::V6(Ipv6HeaderFields {
        local_fragmentation: LocalFragmentation::Forbidden,
        path_mtu_discovery: PathMtuDiscovery::Do,
        ..v6_header(v6_route(ON_LINK_V6, under(EXPERIMENT)))
    });
    assert_eq!(
        send(
            &sockets,
            &socket,
            RawPacket::Payload {
                header,
                payload: &long
            }
        ),
        too_long
    );
    send(
        &sockets,
        &socket,
        payload_to(IpAddr::V6(ON_LINK_V6), EXPERIMENT, &long),
    )
    .expect("the payload is sent in fragments");
    capture.next(|packet| {
        seen(packet)
            .extensions
            .iter()
            .any(|(kind, _)| *kind == IpNumber::IPV6_FRAGMENTATION_HEADER)
    });
}

/// An ICMP error about a sent packet arrives whole, its outer header as on the wire, quoting the packet's header as
/// Linux kept it or as it was written; a buffer that cuts it in its outer header, its ICMP header or the quoted
/// packet takes its start.
pub fn icmp_errors_arrive_whole<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    let capture = Capture::on(DUMMY);
    let v4 = opened(&sockets, IpVersion::V4, EXPERIMENT);
    let v6 = opened(&sockets, IpVersion::V6, EXPERIMENT);
    let written = ipv4_packet(LOCAL, ON_LINK, EXPERIMENT, &[], false, PAYLOAD);
    let written_v6 = ipv6_packet(LOCAL_V6, ON_LINK_V6, EXPERIMENT, &[], PAYLOAD);
    let sends = [
        (&v4, payload_to(IpAddr::V4(ON_LINK), EXPERIMENT, PAYLOAD)),
        (
            &v4,
            header_included_v4(v4_route(ON_LINK, under(EXPERIMENT)), &written),
        ),
        (&v6, payload_to(IpAddr::V6(ON_LINK_V6), EXPERIMENT, PAYLOAD)),
        (
            &v6,
            header_included_v6(v6_route(ON_LINK_V6, under(EXPERIMENT)), &written_v6),
        ),
    ];
    for (socket, packet) in sends {
        let wire = Capture::on(SELF_LINK);
        let provoke = || {
            send(&sockets, socket, packet.clone()).expect("the packet is sent");
            send_icmp_error_about(&capture.next(|packet| seen(packet).protocol == EXPERIMENT))
        };
        let error = provoke();
        let sent = seen(&error[etherparse::Icmpv4Header::MIN_LEN..]);
        let (local, to) = (sent.source, sent.destination);
        let icmp = match version_of(local) {
            IpVersion::V4 => IpNumber::ICMP,
            IpVersion::V6 => IpNumber::IPV6_ICMP,
        };
        let mut into = buffer();
        let fields = loop {
            let fields = receive_packet(&sockets, socket, &mut into);
            if seen(&into[..fields.length]).protocol == icmp {
                break fields;
            }
        };
        let given = &into[..fields.length];
        let outer = seen(given);
        assert_eq!(outer.destination, local);
        assert_eq!(&outer.payload[..2], &error[..2]);
        assert!(wcontainers_icmp_sockets_conformance::checks_out(
            outer.payload,
            &outer
        ));
        let inner = seen(&outer.payload[etherparse::Icmpv4Header::MIN_LEN..]);
        assert_eq!(
            (inner.source, inner.destination, inner.protocol),
            (local, to, EXPERIMENT)
        );
        assert_eq!(inner.payload, &sent.payload[..inner.payload.len()]);
        let on_the_wire = wire.next(|packet| {
            let seen = seen(packet);
            seen.protocol == icmp && seen.payload.get(..2) == Some(&error[..2])
        });
        assert_eq!(given.len(), on_the_wire.len(), "{to}");
        wcontainers_icmp_sockets_conformance::headers_are_as_on_the_wire(given, &on_the_wire);
        let header = given.len() - outer.payload.len();
        let inner_header =
            outer.payload.len() - etherparse::Icmpv4Header::MIN_LEN - inner.payload.len();
        let in_the_quoted_payload =
            header + etherparse::Icmpv4Header::MIN_LEN + inner_header + QUOTED_PAYLOAD / 2;
        // The first is shorter than what Linux gives of the quoted packet, which the rest of the error comes from.
        for short in [
            QUOTED_PAYLOAD / 2,
            header / 2,
            header + etherparse::Icmpv4Header::MIN_LEN / 2,
            in_the_quoted_payload,
        ] {
            provoke();
            let mut cut = vec![u8::MIN; short];
            let fields = receive_packet(&sockets, socket, &mut cut);
            assert_eq!(fields.length, given.len(), "{to} into {short}");
            assert_eq!(cut, given[..short], "{to} into {short}");
        }
    }
}

/// The quoted header and the first eight bytes after it, which an IPv4 ICMP error quotes.
const QUOTED_PAYLOAD: usize = 8;

/// Sends the ICMP error that a host is unreachable about a packet that left, to its source, giving the error.
fn send_icmp_error_about(sent: &[u8]) -> Vec<u8> {
    let packet = seen(sent);
    let version = version_of(packet.source);
    let (mut error, quoted) = match version {
        IpVersion::V4 => (
            vec![icmpv4::TYPE_DEST_UNREACH, icmpv4::CODE_DST_UNREACH_HOST],
            &sent[..ipv4(sent).slice().len() + QUOTED_PAYLOAD],
        ),
        IpVersion::V6 => (
            vec![icmpv6::TYPE_DST_UNREACH, icmpv6::CODE_DST_UNREACH_ADDR],
            sent,
        ),
    };
    error.resize(etherparse::Icmpv4Header::MIN_LEN, u8::MIN);
    error.extend_from_slice(quoted);
    wcontainers_icmp_sockets_conformance::send_raw_icmp(version, packet.source, &mut error);
    error
}

/// A send's priority, transmit time and neighbour confirmation are taken, and its transmit reports carry its key.
pub fn send_fields_are_taken<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    ip(wcontainers_icmp_sockets_conformance::STALE_GATEWAYS);
    let far = [
        (
            IpAddr::V4(FAR),
            IpAddr::V4(wcontainers_icmp_sockets_conformance::GATEWAY),
        ),
        (
            IpAddr::V6(wcontainers_icmp_sockets_conformance::FAR_V6),
            IpAddr::V6(wcontainers_icmp_sockets_conformance::GATEWAY_V6),
        ),
    ];
    for (to, gateway) in far {
        let socket = opened(&sockets, version_of(to), EXPERIMENT);
        assert!(seconds_since_confirmed(gateway, DUMMY) > 0);
        let confirmed = IpSendFields {
            neighbour: NeighbourConfirmation::Confirmed,
            ..plain_fields()
        };
        sockets
            .send_to(&socket, payload_to(to, EXPERIMENT, PAYLOAD), confirmed)
            .unwrap();
        assert_eq!(seconds_since_confirmed(gateway, DUMMY), 0, "{gateway}");
    }
    tc(wcontainers_icmp_sockets_conformance::PRIO_QUEUE);
    let third_band = || {
        wcontainers_route_resolver_conformance::number_after(
            &output(
                wcontainers_icmp_sockets_conformance::TRAFFIC_CONTROL,
                &[
                    &wcontainers_icmp_sockets_conformance::SHOW_CLASSES[..],
                    &[DUMMY],
                ]
                .concat(),
            ),
            wcontainers_icmp_sockets_conformance::THIRD_BAND,
            wcontainers_route_resolver_conformance::PACKETS,
        )
    };
    let filler = IpSendFields {
        priority: PacketPriority(wcontainers_icmp_sockets_conformance::FILLER),
        ..plain_fields()
    };
    for (to, _) in far {
        let socket = opened(&sockets, version_of(to), EXPERIMENT);
        let before = third_band();
        // Through the gateway, whose neighbour entry lets the packet leave at once.
        sockets
            .send_to(&socket, payload_to(to, EXPERIMENT, PAYLOAD), filler)
            .unwrap();
        assert_eq!(third_band(), before + 1, "{to}");
    }
    for to in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        for packet in [Report::Requested, Report::NotRequested] {
            let socket = opened(&sockets, version_of(to), EXPERIMENT);
            let fields = IpSendFields {
                transmit_reports: TransmitReportRequest {
                    key: REPORT_KEY,
                    scheduled: Report::Requested,
                    software: Report::Requested,
                    packet,
                    ..no_reports()
                },
                ..plain_fields()
            };
            sockets
                .send_to(&socket, payload_to(to, EXPERIMENT, PAYLOAD), fields)
                .unwrap();
            let mut stages = Vec::new();
            let mut into = buffer();
            while stages.len() < 2 {
                if let Received::TransmitReport(report) = receive(&sockets, &socket, &mut into) {
                    assert_eq!(report.key, REPORT_KEY);
                    assert_eq!(report.packet_length.is_some(), packet == Report::Requested);
                    let TransmitEvent::Timestamp { stage, source, .. } = report.event else {
                        panic!("a timestamp, not {:?}", report.event);
                    };
                    assert_eq!(source, TimestampSource::Software);
                    stages.push(stage);
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

/// A transmit time holds a send until then on a queue that keeps departure times.
pub fn a_transmit_time_holds_the_send<S: RawSockets>(sockets: S) {
    tc(FAIR_QUEUE);
    const HELD: Duration = Duration::from_millis(300);
    for to in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        let socket = opened(&sockets, version_of(to), EXPERIMENT);
        let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
        let now = Duration::new(now.tv_sec as u64, now.tv_nsec as u32);
        let fields = IpSendFields {
            transmit_time: Some(wcontainers_internet_protocol::TransmitTime {
                clock: wcontainers_internet_protocol::TransmitClock::Monotonic,
                at: now + HELD,
                mode: wcontainers_internet_protocol::TransmitTimeMode::Strict,
                errors: Report::NotRequested,
            }),
            ..plain_fields()
        };
        let start = Instant::now();
        sockets
            .send_to(&socket, payload_to(to, EXPERIMENT, PAYLOAD), fields)
            .unwrap();
        receive_packet(&sockets, &socket, &mut buffer());
        assert!(start.elapsed() >= HELD, "{to}: {:?}", start.elapsed());
    }
}

/// The socket settings of the IP layer are taken: pacing and the unsent count, the send and receive buffers, receive
/// timestamps.
pub fn ip_settings_are_taken<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    tc(FAIR_QUEUE);
    let to = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let socket = opened(&sockets, IpVersion::V4, EXPERIMENT);
    send(&sockets, &socket, payload_to(to, EXPERIMENT, PAYLOAD)).unwrap();
    let fields = receive_packet(&sockets, &socket, &mut buffer());
    assert_eq!(fields.timestamps.software, None);
    assert_eq!(
        sockets.count_unsent_bytes(&socket),
        Ok(ByteCount(usize::MIN))
    );
    set(
        &sockets,
        &socket,
        RawSocketSetting::Ip(IpSocketSetting::Timestamping(Timestamping {
            receive_software: Report::Requested,
            receive_hardware: Report::NotRequested,
            bound_clock: None,
        })),
    );
    send(&sockets, &socket, payload_to(to, EXPERIMENT, PAYLOAD)).unwrap();
    let fields = receive_packet(&sockets, &socket, &mut buffer());
    assert!(fields.timestamps.software.is_some());
    let long = vec![u8::MAX; HELD_PAYLOAD];
    set(
        &sockets,
        &socket,
        RawSocketSetting::Ip(IpSocketSetting::MaximumPacingRate(Some(RAW_PACING))),
    );
    set(
        &sockets,
        &socket,
        RawSocketSetting::Ip(IpSocketSetting::SendBuffer(ByteCount(usize::MIN))),
    );
    let mut blocked = false;
    for _ in 0..u8::MAX {
        match send(&sockets, &socket, payload_to(to, EXPERIMENT, &long)) {
            Ok(()) => {}
            // Linux answers that it has no buffer for a raw send that its full send buffer leaves no room for.
            Err(SendError::Host(HostError::WouldBlock | HostError::Errno(libc::ENOBUFS))) => {
                blocked = true;
                break;
            }
            Err(error) => panic!("{error:?}"),
        }
    }
    assert!(blocked);
    assert!(sockets.count_unsent_bytes(&socket).unwrap().0 >= long.len());
    let receiver = opened(&sockets, IpVersion::V4, OTHER_EXPERIMENT);
    set(
        &sockets,
        &receiver,
        RawSocketSetting::Ip(IpSocketSetting::ReceiveBuffer(ByteCount(usize::MIN))),
    );
    let sender = opened(&sockets, IpVersion::V4, OTHER_EXPERIMENT);
    let requests = FLOODING_SENDS;
    for _ in 0..requests {
        send(&sockets, &sender, payload_to(to, OTHER_EXPERIMENT, &long)).unwrap();
    }
    std::thread::sleep(QUIET);
    let mut received = 0;
    let mut dropped = 0;
    let mut into = buffer();
    while let Ok(Received::Packet(fields)) = sockets.receive(&receiver, &mut into) {
        received += 1;
        dropped += fields.drops_since_previous.0 as usize;
    }
    // Both raw sockets of the protocol take each packet; the receiver's drops are told with its next packet.
    set(
        &sockets,
        &receiver,
        RawSocketSetting::Ip(IpSocketSetting::ReceiveBuffer(ByteCount(usize::from(
            u16::MAX,
        )))),
    );
    send(&sockets, &sender, payload_to(to, OTHER_EXPERIMENT, PAYLOAD)).unwrap();
    let last = receive_packet(&sockets, &receiver, &mut into);
    dropped += last.drops_since_previous.0 as usize;
    assert!(received < requests);
    assert_eq!(received + dropped, requests);
}

/// Joining a group, or leaving it, on the dummy link.
fn membership(group: IpAddr, joined: bool) -> RawSocketSetting<'static> {
    let interface = Some(index(DUMMY));
    RawSocketSetting::Ip(match group {
        IpAddr::V4(group) => IpSocketSetting::MulticastV4(MulticastGroup {
            group,
            interface,
            membership: if joined {
                MulticastMembership::Joined(SourceFilter::Exclude(&[]))
            } else {
                MulticastMembership::Left
            },
        }),
        IpAddr::V6(group) => IpSocketSetting::MulticastV6(MulticastGroup {
            group,
            interface,
            membership: if joined {
                MulticastMembership::Joined(SourceFilter::Exclude(&[]))
            } else {
                MulticastMembership::Left
            },
        }),
    })
}

/// A joined group is the host's while joined; multicast loopback brings the socket its own multicast send, and an
/// IPv4 socket of no membership takes it only while it takes every group's.
pub fn multicast_settings_are_taken<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    const GROUP: Ipv4Addr = Ipv4Addr::new(239, 1, 2, 4);
    const GROUP_V6: Ipv6Addr = Ipv6Addr::new(0xff15, 0, 0, 0, 0, 0, 0, 0x125);
    let link = Some(index(DUMMY));
    let joined = |group: IpAddr| {
        output(
            wcontainers_route_resolver_conformance::IP,
            &[&SHOW_GROUPS[..], &[DUMMY]].concat(),
        )
        .contains(&group.to_string())
    };
    for group in [IpAddr::V4(GROUP), IpAddr::V6(GROUP_V6)] {
        let version = version_of(group);
        let member = opened(&sockets, version, EXPERIMENT);
        let other = opened(&sockets, version, EXPERIMENT);
        set(&sockets, &member, membership(group, true));
        assert!(joined(group), "{group}");
        let send_to_group = || {
            let header = match group {
                IpAddr::V4(group) => IpHeaderFields::V4(v4_header(Ipv4RouteSelectors {
                    interface: link,
                    ..v4_route(group, under(EXPERIMENT))
                })),
                IpAddr::V6(group) => IpHeaderFields::V6(v6_header(Ipv6RouteSelectors {
                    interface: link,
                    ..v6_route(group, under(EXPERIMENT))
                })),
            };
            send(
                &sockets,
                &member,
                RawPacket::Payload {
                    header,
                    payload: PAYLOAD,
                },
            )
            .expect("the multicast payload is sent");
        };
        for (looped, arrives) in [(false, false), (true, true)] {
            set(
                &sockets,
                &member,
                RawSocketSetting::Ip(IpSocketSetting::MulticastLoopback(looped)),
            );
            send_to_group();
            assert_eq!(
                !nothing_arrives(&sockets, &member),
                arrives,
                "{group} loopback {looped}"
            );
            if looped {
                assert!(!nothing_arrives(&sockets, &other), "{group}");
            }
        }
        for enabled in [false, true] {
            set(
                &sockets,
                &other,
                RawSocketSetting::Ip(IpSocketSetting::MulticastAllGroups { version, enabled }),
            );
            send_to_group();
            assert!(!nothing_arrives(&sockets, &member), "{group}");
            // Linux gives an IPv6 raw socket bound to no address every multicast packet of its protocol.
            assert_eq!(
                !nothing_arrives(&sockets, &other),
                enabled || version == IpVersion::V6,
                "{group} all groups {enabled}"
            );
        }
        set(&sockets, &member, membership(group, false));
        assert!(!joined(group), "{group}");
    }
}

/// An anycast address is the host's while the socket holds it.
pub fn anycast_is_taken<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    const ANYCAST: Ipv6Addr = Ipv6Addr::new(0xfd03, 0, 0, 0, 0, 0, 0, 0x97);
    let socket = opened(&sockets, IpVersion::V6, EXPERIMENT);
    let listed = || {
        std::fs::read_to_string(wcontainers_icmp_sockets_conformance::ANYCAST_LIST)
            .expect("the anycast list")
            .contains(&format!("{:032x}", u128::from(ANYCAST)))
    };
    for (membership, held) in [(Membership::Joined, true), (Membership::Left, false)] {
        set(
            &sockets,
            &socket,
            RawSocketSetting::Ip(IpSocketSetting::Anycast(AnycastAddress {
                address: ANYCAST,
                interface: Some(index(DUMMY)),
                membership,
            })),
        );
        assert_eq!(listed(), held);
    }
}

/// A wait for data does not finish before a packet of the socket's protocol arrives, and finishes, woken once, when one does.
pub fn a_wait_for_data_finishes_when_a_packet_arrives<S: RawSockets>(sockets: S) {
    for to in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        let version = version_of(to);
        let receiver = opened(&sockets, version, EXPERIMENT);
        let sender = opened(&sockets, version, EXPERIMENT);
        let wakes = Arc::new(Wakes::default());
        let mut wait = pin!(sockets.wait_for_events(&receiver, READABLE));
        assert!(poll_with(wait.as_mut(), &wakes).is_pending());
        std::thread::sleep(QUIET);
        assert_eq!(wakes.count(), 0, "woken with nothing to read");
        assert!(poll_with(wait.as_mut(), &wakes).is_pending());
        send(&sockets, &sender, payload_to(to, EXPERIMENT, PAYLOAD)).expect("the payload is sent");
        assert!(wakes.reach(1), "woken when the packet arrives");
        let events = finished(poll_with(wait.as_mut(), &wakes));
        assert!(events.data_to_read, "{events:?}");
        assert!(only_asked_for(events, READABLE), "{events:?}");
        assert_eq!(wakes.count(), 1);
        assert!(matches!(
            sockets.receive(&receiver, &mut buffer()),
            Ok(Received::Packet(_))
        ));
    }
}

/// A wait for what the socket already has finishes at its first poll, with only the events asked for.
pub fn a_wait_for_writing_finishes_at_once<S: RawSockets>(sockets: S) {
    let socket = opened(&sockets, IpVersion::V6, RAW_PROTOCOL);
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
pub fn a_dropped_wait_is_withdrawn<S: RawSockets>(sockets: S) {
    let to = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let receiver = opened(&sockets, IpVersion::V4, EXPERIMENT);
    let wakes = Arc::new(Wakes::default());
    {
        let wait = pin!(sockets.wait_for_events(&receiver, READABLE));
        assert!(poll_with(wait, &wakes).is_pending());
    }
    assert_eq!(
        Arc::strong_count(&wakes),
        1,
        "the dropped wait kept its waker"
    );
    let sender = opened(&sockets, IpVersion::V4, EXPERIMENT);
    send(&sockets, &sender, payload_to(to, EXPERIMENT, PAYLOAD)).expect("the payload is sent");
    std::thread::sleep(QUIET);
    assert_eq!(wakes.count(), 0, "a dropped wait was woken");
    let events = finished(poll_with(
        pin!(sockets.wait_for_events(&receiver, READABLE)),
        &wakes,
    ));
    assert!(events.data_to_read, "{events:?}");
}

/// Many waits at once, on many sockets of one protocol and several on one, each finish woken once by the packet
/// they all take.
pub fn many_waits_each_wake_once<S: RawSockets>(sockets: S) {
    let to = IpAddr::V6(Ipv6Addr::LOCALHOST);
    let watched: Vec<_> = (0..WAITED_SOCKETS)
        .map(|_| opened(&sockets, IpVersion::V6, EXPERIMENT))
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
    let sender = opened(&sockets, IpVersion::V6, EXPERIMENT);
    send(&sockets, &sender, payload_to(to, EXPERIMENT, PAYLOAD)).expect("the payload is sent");
    for (wait, wakes) in &mut waits {
        assert!(wakes.reach(1), "a wait woken when the packet arrives");
        assert!(finished(poll_with(wait.as_mut(), wakes)).data_to_read);
    }
    std::thread::sleep(QUIET);
    for (_, wakes) in &waits {
        assert_eq!(wakes.count(), 1, "a wait woken more than once");
    }
}

/// An ICMP error about a payload sent ends a wait for data with the error condition, which poll always reports.
pub fn an_icmp_error_ends_a_wait_for_data<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    let capture = Capture::on(DUMMY);
    for to in [IpAddr::V4(ON_LINK), IpAddr::V6(ON_LINK_V6)] {
        let socket = opened(&sockets, version_of(to), EXPERIMENT);
        let wakes = Arc::new(Wakes::default());
        let mut wait = pin!(sockets.wait_for_events(&socket, READABLE));
        assert!(poll_with(wait.as_mut(), &wakes).is_pending());
        send(&sockets, &socket, payload_to(to, EXPERIMENT, PAYLOAD)).expect("the payload is sent");
        let sent = capture
            .next(|packet| seen(packet).protocol == EXPERIMENT && seen(packet).destination == to);
        let error = send_icmp_error_about(&sent);
        assert!(wakes.reach(1), "woken by the ICMP error");
        let events = finished(poll_with(wait.as_mut(), &wakes));
        assert!(events.error_condition, "{events:?}");
        let mut into = buffer();
        let fields = receive_packet(&sockets, &socket, &mut into);
        assert_eq!(seen(&into[..fields.length]).payload[..2], error[..2]);
    }
}

/// An ICMP error that a too long send read off the host while looking for its own error still ends a wait at once.
pub fn a_wait_sees_an_error_a_send_read_first<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    let capture = Capture::on(DUMMY);
    let socket = opened(&sockets, IpVersion::V4, EXPERIMENT);
    let wakes = Arc::new(Wakes::default());
    send(
        &sockets,
        &socket,
        payload_to(IpAddr::V4(ON_LINK), EXPERIMENT, PAYLOAD),
    )
    .expect("the payload is sent");
    let error = send_icmp_error_about(&capture.next(|packet| seen(packet).protocol == EXPERIMENT));
    let events = finished_soon(pin!(sockets.wait_for_events(&socket, READABLE)), &wakes);
    assert!(events.error_condition, "{events:?}");
    let long = vec![u8::MAX; OVERSIZED_PAYLOAD];
    let mtu = u32::try_from(OVERSIZED_PAYLOAD / 2).expect("a small MTU");
    ip(&format!("link set {DUMMY} mtu {mtu}"));
    let header = IpHeaderFields::V4(Ipv4HeaderFields {
        path_mtu_discovery: PathMtuDiscovery::Do,
        ..v4_header(v4_route(ON_LINK, under(EXPERIMENT)))
    });
    assert_eq!(
        send(
            &sockets,
            &socket,
            RawPacket::Payload {
                header,
                payload: &long
            }
        ),
        Err(SendError::MessageTooLong {
            maximum_transmission_unit: MaximumTransmissionUnit(mtu)
        })
    );
    let events = finished(poll_with(
        pin!(sockets.wait_for_events(&socket, READABLE)),
        &wakes,
    ));
    assert!(events.error_condition, "{events:?}");
    let mut into = buffer();
    let fields = receive_packet(&sockets, &socket, &mut into);
    let packet = seen(&into[..fields.length]);
    assert_eq!(packet.protocol, IpNumber::ICMP);
    assert_eq!(&packet.payload[..2], &error[..2]);
    assert!(nothing_arrives(&sockets, &socket));
    let wait = pin!(sockets.wait_for_events(&socket, READABLE));
    assert!(
        poll_with(wait, &wakes).is_pending(),
        "an error held after its entry was received"
    );
}

/// A transmit time the link's schedule cannot keep is reported with the key of its send and why, as the ICMP suite
/// checks it.
pub fn rejected_transmit_times_are_reported_with_their_keys<S: RawSockets>(sockets: S) {
    ip(NETWORK);
    tc(wcontainers_icmp_sockets_conformance::MISSING_SCHEDULE);
    let socket = opened(&sockets, IpVersion::V4, EXPERIMENT);
    let sends = wcontainers_icmp_sockets_conformance::rejected_sends();
    for (place, &(time, reason)) in sends.iter().enumerate() {
        let fields =
            wcontainers_icmp_sockets_conformance::rejected_send_fields(place, time, reason);
        match sockets.send_to(
            &socket,
            payload_to(IpAddr::V4(ON_LINK), EXPERIMENT, PAYLOAD),
            fields,
        ) {
            Ok(()) | Err(SendError::Host(_)) => {}
            Err(error) => panic!("send {place}: {error:?}"),
        }
    }
    wcontainers_icmp_sockets_conformance::check_rejections(
        |into| sockets.receive(&socket, into),
        &sends,
    );
}

/// A setting set twice answers as it did once, by Configurable's own suite.
pub fn configurable_value<S: RawSockets>(
    sockets: S,
    version: IpVersion,
    protocol: IpNumber,
    setting: RawSocketSetting<'static>,
) -> (S, S::Socket, RawSocketSetting<'static>) {
    let socket = opened(&sockets, version, protocol);
    (sockets, socket, setting)
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test in a network namespace of its own.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        $crate::each_check!(
            $value;
            open_takes_each_version_and_protocol,
            a_payload_goes_out_with_its_ipv4_header,
            a_payload_goes_out_with_its_ipv6_header,
            an_ipv6_raw_protocol_socket_sends_under_the_asked_protocol,
            a_payload_under_another_upper_layer_is_not_sent_as_asked,
            header_included_packets_go_out_as_written,
            header_included_packets_take_their_route,
            withdrawn_reassembly_lets_a_fragment_leave,
            ipv4_packets_arrive_whole,
            ipv6_packets_arrive_with_their_headers,
            receive_with_nothing_queued_would_block,
            a_short_buffer_keeps_the_whole_length,
            received_packets_carry_their_mark_and_priority,
            a_checksum_offset_is_written,
            an_ipv4_router_alert_takes_forwarded_packets,
            an_ipv6_router_alert_takes_forwarded_packets,
            multicast_routing_takes_the_namespace_role,
            close_gives_up_the_sockets_holdings,
            payload_routes_take_their_selectors,
            ipv6_source_preferences_choose_the_source,
            a_mobility_header_is_sent_by_its_type,
            path_mtu_modes_and_too_long_payloads,
            icmp_errors_arrive_whole,
            send_fields_are_taken,
            a_transmit_time_holds_the_send,
            ip_settings_are_taken,
            multicast_settings_are_taken,
            anycast_is_taken,
            a_wait_for_data_finishes_when_a_packet_arrives,
            a_wait_for_writing_finishes_at_once,
            a_dropped_wait_is_withdrawn,
            many_waits_each_wake_once,
            an_icmp_error_ends_a_wait_for_data,
            a_wait_sees_an_error_a_send_read_first,
            rejected_transmit_times_are_reported_with_their_keys,
        );
        $crate::configurable_checks!(
            $value;
            configurable_router_alert: (
                wcontainers_internet_protocol::IpVersion::V4,
                etherparse::IpNumber::EXPERIMENTAL_AND_TESTING_0,
                $crate::router_alert_setting()
            ),
            configurable_ipv6_router_alert: (
                wcontainers_internet_protocol::IpVersion::V6,
                $crate::RAW_PROTOCOL,
                $crate::ipv6_router_alert_setting()
            ),
            configurable_multicast_routing: (
                wcontainers_internet_protocol::IpVersion::V4,
                etherparse::IpNumber::IGMP,
                $crate::multicast_routing_setting()
            ),
            configurable_checksum: (
                wcontainers_internet_protocol::IpVersion::V6,
                etherparse::IpNumber::EXPERIMENTAL_AND_TESTING_0,
                $crate::checksum_setting()
            ),
        );
    };
}

pub fn router_alert_setting() -> RawSocketSetting<'static> {
    RawSocketSetting::RouterAlert(RouterAlert::Ipv4)
}

pub fn ipv6_router_alert_setting() -> RawSocketSetting<'static> {
    RawSocketSetting::RouterAlert(RouterAlert::Ipv6 {
        value: ROUTER_ALERT_VALUE,
        isolation: RouterAlertIsolation::OwnNamespace,
    })
}

pub fn multicast_routing_setting() -> RawSocketSetting<'static> {
    RawSocketSetting::MulticastRouting(MulticastRouting::Enabled)
}

pub fn checksum_setting() -> RawSocketSetting<'static> {
    RawSocketSetting::Checksum(Some(CHECKSUM_OFFSET))
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

/// One module of Configurable's suite per named setting, each on a socket of the version and protocol given.
#[doc(hidden)]
#[macro_export]
macro_rules! configurable_checks {
    ($value:expr; $($name:ident: ($version:expr, $protocol:expr, $setting:expr)),* $(,)?) => {
        $(
            mod $name {
                $crate::configurable_suite::tests!({
                    $crate::enter_network_namespace();
                    $crate::configurable_value($value, $version, $protocol, $setting)
                });
            }
        )*
    };
}
