//! What only this implementation answers on Linux's raw sockets: its refusals and counts.
#![cfg(target_os = "linux")]

use std::net::{Ipv4Addr, Ipv6Addr};

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv4Options, Ipv6FlowLabel, icmpv4};
use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BroadcastPermission, IcmpCode, IcmpType, IpHeaderFields, IpSendFields, IpVersion,
    Ipv4HeaderFields, Ipv4RouteSelectors, Ipv6HeaderFields, Ipv6RouteSelectors, LocalFragmentation,
    MobileAddressPreference, MobilityHeaderType, NeighbourConfirmation, PacketMark, PacketPriority,
    PathMtuDiscovery, Received, Report, RouteScope, RouterAlertValue, SendError,
    SourceAddressCheck, SourcePreferences, TemporaryAddressPreference, TransmitReportKey,
    TransmitReportRequest, UpperLayer, UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_raw_sockets::{
    HeaderIncludedPacket, OutgoingReassembly, RawPacket, RawSocketSetting, RawSockets, RouterAlert,
    RouterAlertIsolation,
};
use wcontainers_raw_sockets_conformance::icmp_suite::{PAD_N, PADDED, PATIENCE};
use wcontainers_raw_sockets_conformance::{EXPERIMENT, LentBuffers, RAW_PROTOCOL};
use wcontainers_raw_sockets_host::HostRawSockets;

const PAYLOAD: &[u8] = b"payload";

/// Returns the plain route to the IPv4 loopback address, under the protocol given.
fn v4_route(options: &Ipv4Options, protocol: IpNumber) -> Ipv4RouteSelectors<'_> {
    Ipv4RouteSelectors {
        destination: Ipv4Addr::LOCALHOST,
        source: None,
        source_address_check: SourceAddressCheck::Required,
        interface: None,
        dscp: IpDscp::ZERO,
        ecn: IpEcn::NotEct,
        mark: PacketMark(u32::MIN),
        scope: RouteScope::Universe,
        broadcast: BroadcastPermission::Refused,
        options,
        upper_layer: UpperLayer {
            protocol,
            selector: None,
        },
    }
}

fn plain_fields() -> IpSendFields {
    IpSendFields {
        neighbour: NeighbourConfirmation::Unconfirmed,
        priority: PacketPriority(u32::MIN),
        transmit_time: None,
        transmit_reports: TransmitReportRequest {
            key: TransmitReportKey(u32::MIN),
            scheduled: Report::NotRequested,
            software: Report::NotRequested,
            hardware: Report::NotRequested,
            acknowledgement: Report::NotRequested,
            completion: Report::NotRequested,
            wireless_status: Report::NotRequested,
            packet: Report::NotRequested,
        },
    }
}

/// A binding update with one more eight bytes than the smallest header, whose type is 5 and length byte 1.
///
/// https://www.rfc-editor.org/rfc/rfc6275#section-6.1.7
const BINDING_UPDATE: [u8; 16] = [
    IpNumber::IPV6_NO_NEXT_HEADER.0,
    1,
    5,
    0,
    0,
    0,
    0,
    1,
    0,
    0,
    0,
    1,
    1,
    2,
    0,
    0,
];
/// Where a mobility header's length byte is, just before its type.
///
/// https://www.rfc-editor.org/rfc/rfc6275#section-6.1
const MOBILITY_HEADER_LENGTH: usize = 1;

/// Returns the one reason a value counted once, failing if it counted anything else.
fn the_one_reason(sockets: &HostRawSockets) -> &'static str {
    match sockets.counters().as_slice() {
        [reading] if reading.count == 1 => reading.reason,
        other => panic!("{other:?}"),
    }
}

/// An ICMP type and code, which no payload of the experimental protocol is routed by.
const ICMP_SELECTOR: UpperLayerSelector = UpperLayerSelector::Icmp {
    message_type: IcmpType(icmpv4::TYPE_ECHO_REQUEST),
    code: IcmpCode(u8::MIN),
};

/// Checks that a payload under another protocol, or an upper layer Linux does not route a send by, is refused and counted.
///
/// The mobility header's case asks for a type other than its message's, whose length byte is that type.
#[test]
fn each_send_refusal_is_counted_under_its_reason() {
    wcontainers_raw_sockets_conformance::enter_network_namespace();
    let options = Ipv4Options::new();
    let selected = |protocol| UpperLayer {
        protocol,
        selector: Some(ICMP_SELECTOR),
    };
    let other_protocol = v4_route(&options, IpNumber::EXPERIMENTAL_AND_TESTING_1);
    let selected_payload = Ipv4RouteSelectors {
        upper_layer: selected(EXPERIMENT),
        ..v4_route(&options, EXPERIMENT)
    };
    let header = |route| {
        IpHeaderFields::V4(Ipv4HeaderFields {
            route,
            time_to_live: None,
            path_mtu_discovery: PathMtuDiscovery::Want,
        })
    };
    let v6_route = Ipv6RouteSelectors {
        destination: Ipv6Addr::LOCALHOST,
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
        upper_layer: selected(EXPERIMENT),
    };
    let mobility_route = Ipv6RouteSelectors {
        upper_layer: UpperLayer {
            protocol: IpNumber::MOBILITY_HEADER,
            selector: Some(UpperLayerSelector::MobilityHeader(MobilityHeaderType(
                BINDING_UPDATE[MOBILITY_HEADER_LENGTH],
            ))),
        },
        ..v6_route.clone()
    };
    let cases = [
        (
            "other protocol",
            IpVersion::V4,
            EXPERIMENT,
            RawPacket::Payload {
                header: header(other_protocol),
                payload: PAYLOAD,
            },
        ),
        (
            "upper layer",
            IpVersion::V4,
            EXPERIMENT,
            RawPacket::Payload {
                header: header(selected_payload),
                payload: PAYLOAD,
            },
        ),
        (
            "upper layer",
            IpVersion::V6,
            IpNumber::MOBILITY_HEADER,
            RawPacket::Payload {
                header: IpHeaderFields::V6(Ipv6HeaderFields {
                    route: mobility_route,
                    hop_limit: None,
                    path_mtu_discovery: PathMtuDiscovery::Want,
                    local_fragmentation: LocalFragmentation::Allowed,
                    fragment_mtu: None,
                    hop_by_hop_options: None,
                    routing_destination_options: None,
                    destination_options: None,
                }),
                payload: &BINDING_UPDATE,
            },
        ),
        (
            "upper layer",
            IpVersion::V4,
            EXPERIMENT,
            RawPacket::HeaderIncluded(HeaderIncludedPacket::V4 {
                route: selected_payload,
                reassembly: OutgoingReassembly::Allowed,
                packet: PAYLOAD,
            }),
        ),
        (
            "upper layer",
            IpVersion::V6,
            EXPERIMENT,
            RawPacket::HeaderIncluded(HeaderIncludedPacket::V6 {
                route: v6_route,
                packet: PAYLOAD,
            }),
        ),
    ];
    let mut reasons: Vec<(&str, &str)> = Vec::new();
    for (kind, version, protocol, packet) in cases {
        let sockets = HostRawSockets::new().unwrap();
        let socket = sockets.open(version, protocol, UserId(u32::MIN)).unwrap();
        assert_eq!(
            sockets.send_to(&socket, packet, plain_fields()),
            Err(SendError::Host(HostError::NotSupported)),
            "{kind}"
        );
        reasons.push((kind, the_one_reason(&sockets)));
    }
    // One reason for each kind of refusal, and a different one for each other kind.
    for (kind, reason) in &reasons {
        for (other_kind, other_reason) in &reasons {
            assert_eq!(
                kind == other_kind,
                reason == other_reason,
                "{kind}: {reason}; {other_kind}: {other_reason}"
            );
        }
    }
}

/// Checks that a packet an IPv6 router alert socket takes as forwarded by its bytes is given whole and counted.
///
/// The packet is local: under protocol 255 its payload repeats its own IPv6 header and hop-by-hop header, which is
/// how a forwarded packet looks to such a socket.
#[test]
fn a_packet_taken_as_forwarded_is_counted() {
    wcontainers_raw_sockets_conformance::enter_network_namespace();
    let sockets = HostRawSockets::new().unwrap();
    let alerted = sockets
        .open(IpVersion::V6, RAW_PROTOCOL, UserId(u32::MIN))
        .unwrap();
    sockets
        .configure(
            &alerted,
            RawSocketSetting::RouterAlert(RouterAlert::Ipv6 {
                value: RouterAlertValue(u16::MIN),
                isolation: RouterAlertIsolation::OwnNamespace,
            }),
        )
        .unwrap();
    // A hop-by-hop header of padding alone before protocol 255: next header, length, and PadN over the rest.
    let hop_by_hop = [
        RAW_PROTOCOL.0,
        u8::MIN,
        PAD_N,
        PADDED,
        u8::MIN,
        u8::MIN,
        u8::MIN,
        u8::MIN,
    ];
    let header = |payload_length: usize| {
        let mut header = etherparse::Ipv6Header {
            next_header: IpNumber::IPV6_HEADER_HOP_BY_HOP,
            hop_limit: u8::MAX,
            source: Ipv6Addr::LOCALHOST.octets(),
            destination: Ipv6Addr::LOCALHOST.octets(),
            ..Default::default()
        };
        header.set_payload_length(payload_length).unwrap();
        header.to_bytes().to_vec()
    };
    let inner = [header(hop_by_hop.len()), hop_by_hop.to_vec()].concat();
    let packet = [
        header(hop_by_hop.len() + inner.len()),
        hop_by_hop.to_vec(),
        inner.clone(),
    ]
    .concat();
    // An IPv6 socket of protocol 255 sends a packet with its own header.
    let sender = rustix::net::socket(
        rustix::net::AddressFamily::INET6,
        rustix::net::SocketType::RAW,
        Some(rustix::net::Protocol::from_raw(
            core::num::NonZeroU32::new(u32::from(RAW_PROTOCOL.0)).unwrap(),
        )),
    )
    .unwrap();
    rustix::net::sendto(
        &sender,
        &packet,
        rustix::net::SendFlags::empty(),
        &std::net::SocketAddr::new(Ipv6Addr::LOCALHOST.into(), u16::MIN),
    )
    .unwrap();
    let start = std::time::Instant::now();
    let mut into = vec![u8::MIN; packet.len()];
    let received = loop {
        match sockets.receive_buffer(&alerted, &mut into) {
            Err(HostError::WouldBlock) => {
                assert!(start.elapsed() < PATIENCE, "nothing arrives");
                std::thread::yield_now();
            }
            other => break other.unwrap(),
        }
    };
    let Received::Packet(fields) = received else {
        panic!("{received:?}");
    };
    // Given whole as forwarded packets are: the payload, which is the inner packet, with no headers written in front.
    assert_eq!(&into[..fields.length], &inner[..]);
    let reason = the_one_reason(&sockets);
    assert!(reason.contains("forwards"), "{reason}");
}
