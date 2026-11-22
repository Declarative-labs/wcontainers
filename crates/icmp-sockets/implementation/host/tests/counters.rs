//! What only this implementation answers on Linux's ping sockets: its refusals and counts.
#![cfg(target_os = "linux")]

use etherparse::IpNumber;
use wcontainers_abi::HostError;
use wcontainers_icmp_sockets::IcmpSockets;
use wcontainers_icmp_sockets_host::HostIcmpSockets;
use wcontainers_internet_protocol::{
    IcmpCode, IcmpType, IpHeaderFields, IpSendFields, IpVersion, Ipv6HeaderFields,
    Ipv6RouteSelectors, NeighbourConfirmation, Report, SendError, TransmitReportRequest,
    UpperLayer, UpperLayerSelector,
};
use wcontainers_kernel::UserId;

/// Checks that Linux makes an echo socket only for a caller whose group is in the namespace's ping group range.
#[test]
fn an_empty_ping_group_range_refuses_the_socket() {
    wcontainers_icmp_sockets_conformance::enter_network_namespace();
    // A first group above the last is the empty range, Linux's default.
    std::fs::write(
        wcontainers_icmp_sockets_conformance::PING_GROUP_RANGE,
        "1 0",
    )
    .unwrap();
    let sockets = HostIcmpSockets::new().unwrap();
    for version in [IpVersion::V4, IpVersion::V6] {
        assert_eq!(
            sockets.open(version, UserId(u32::MIN)).err(),
            Some(HostError::Errno(libc::EACCES))
        );
    }
}

/// The option that pads an IPv6 extension header by its length.
///
/// https://www.rfc-editor.org/rfc/rfc8200#section-4.2
const PAD_N: u8 = 1;
/// How many bytes PadN pads after its type and length in the smallest extension header.
const PADDED: u8 = 4;

/// Returns the one reason a value counted once, failing if it counted anything else.
fn the_one_reason(sockets: &HostIcmpSockets) -> &'static str {
    match sockets.counters().as_slice() {
        [reading] if reading.count == 1 => reading.reason,
        other => panic!("{other:?}"),
    }
}

/// Checks that each send Linux's ping sockets would not honour is refused and counted once under a reason of its own.
#[test]
fn each_send_refusal_is_counted_under_its_reason() {
    use wcontainers_icmp_sockets_conformance as suite;
    suite::enter_network_namespace();
    let message = suite::echo(IpVersion::V6, suite::PAYLOAD);
    let to = std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST);
    let IpHeaderFields::V6(plain) = suite::header_to(to, &message) else {
        unreachable!("an IPv6 destination has an IPv6 header");
    };
    // A header of padding alone: next header, length in eight-byte units after the first, and PadN over the rest.
    let padding = [
        u8::MIN,
        u8::MIN,
        PAD_N,
        PADDED,
        u8::MIN,
        u8::MIN,
        u8::MIN,
        u8::MIN,
    ];
    let padding = etherparse::Ipv6RawExtHeaderSlice::from_slice(&padding).unwrap();
    let with_route = |route| Ipv6HeaderFields {
        route,
        ..plain.clone()
    };
    let source = with_route(Ipv6RouteSelectors {
        source: Some(std::net::Ipv6Addr::LOCALHOST),
        ..plain.route.clone()
    });
    let routing_header = with_route(Ipv6RouteSelectors {
        routing_header: Some(padding.clone()),
        ..plain.route.clone()
    });
    let other_code = with_route(Ipv6RouteSelectors {
        upper_layer: UpperLayer {
            protocol: IpNumber::IPV6_ICMP,
            selector: Some(UpperLayerSelector::Icmp {
                message_type: IcmpType(message[0]),
                code: IcmpCode(message[1] + 1),
            }),
        },
        ..plain.route.clone()
    });
    let confirmed = IpSendFields {
        neighbour: NeighbourConfirmation::Confirmed,
        ..suite::plain_fields()
    };
    let cases = [
        ("source", source, suite::plain_fields()),
        ("extension", routing_header, suite::plain_fields()),
        (
            "extension",
            Ipv6HeaderFields {
                hop_by_hop_options: Some(padding.clone()),
                ..plain.clone()
            },
            suite::plain_fields(),
        ),
        (
            "extension",
            Ipv6HeaderFields {
                routing_destination_options: Some(padding.clone()),
                ..plain.clone()
            },
            suite::plain_fields(),
        ),
        (
            "extension",
            Ipv6HeaderFields {
                destination_options: Some(padding.clone()),
                ..plain.clone()
            },
            suite::plain_fields(),
        ),
        ("neighbour", plain.clone(), confirmed),
        ("upper layer", other_code, suite::plain_fields()),
    ];
    let mut reasons: Vec<(&str, &str)> = Vec::new();
    for (kind, header, fields) in cases {
        let sockets = HostIcmpSockets::new().unwrap();
        let socket = sockets.open(IpVersion::V6, UserId(u32::MIN)).unwrap();
        assert_eq!(
            sockets.send_to(&socket, IpHeaderFields::V6(header), fields, &message),
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
    // The upper layer of an IPv4 ping too, under the same reason as over IPv6.
    let sockets = HostIcmpSockets::new().unwrap();
    let socket = sockets.open(IpVersion::V4, UserId(u32::MIN)).unwrap();
    let message = suite::echo(IpVersion::V4, suite::PAYLOAD);
    let to = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
    let IpHeaderFields::V4(mut header) = suite::header_to(to, &message) else {
        unreachable!("an IPv4 destination has an IPv4 header");
    };
    header.route.upper_layer.selector = None;
    assert_eq!(
        sockets.send_to(
            &socket,
            IpHeaderFields::V4(header),
            suite::plain_fields(),
            &message
        ),
        Err(SendError::Host(HostError::NotSupported))
    );
    assert_eq!(
        Some(the_one_reason(&sockets)),
        reasons.last().map(|(_, reason)| *reason)
    );
}

/// Checks that each received packet counts once the fields Linux does not give and the drop count it asks the socket for.
#[test]
fn a_received_packet_counts_what_linux_does_not_give() {
    use wcontainers_icmp_sockets_conformance as suite;
    suite::enter_network_namespace();
    let sockets = HostIcmpSockets::new().unwrap();
    let socket = sockets.open(IpVersion::V4, UserId(u32::MIN)).unwrap();
    let to = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
    let message = suite::echo(IpVersion::V4, suite::PAYLOAD);
    sockets
        .send_to(
            &socket,
            suite::header_to(to, &message),
            suite::plain_fields(),
            &message,
        )
        .unwrap();
    let start = std::time::Instant::now();
    let mut into = suite::buffer();
    while sockets.receive(&socket, &mut into) == Err(HostError::WouldBlock) {
        assert!(start.elapsed() < suite::PATIENCE, "no reply");
        std::thread::yield_now();
    }
    let counted: Vec<_> = sockets
        .counters()
        .into_iter()
        .map(|reading| (reading.reason, reading.count))
        .collect();
    // The fields Linux does not give and the drop count asked for, each under a reason of its own.
    assert!(
        matches!(counted.as_slice(), [(first, 1), (second, 1)] if first != second),
        "{counted:?}"
    );
}

/// Checks that a transmit report without its packet, which Linux gives with no interface, counts that once.
#[test]
fn a_report_without_its_packet_counts_its_missing_interface() {
    use wcontainers_icmp_sockets_conformance as suite;
    use wcontainers_internet_protocol::Received;
    use wcontainers_internet_protocol_linux_socket_options::datagram::exchange::REPORT_INTERFACE_NOT_GIVEN;
    suite::enter_network_namespace();
    let sockets = HostIcmpSockets::new().unwrap();
    let to = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
    for packet in [Report::Requested, Report::NotRequested] {
        let socket = sockets.open(IpVersion::V4, UserId(u32::MIN)).unwrap();
        let message = suite::echo(IpVersion::V4, suite::PAYLOAD);
        let fields = IpSendFields {
            transmit_reports: TransmitReportRequest {
                key: suite::REPORT_KEY,
                software: Report::Requested,
                packet,
                ..suite::no_reports()
            },
            ..suite::plain_fields()
        };
        sockets
            .send_to(&socket, suite::header_to(to, &message), fields, &message)
            .unwrap();
        let start = std::time::Instant::now();
        let mut into = suite::buffer();
        while !matches!(
            sockets.receive(&socket, &mut into),
            Ok(Received::TransmitReport(_))
        ) {
            assert!(start.elapsed() < suite::PATIENCE, "no report");
            std::thread::yield_now();
        }
    }
    let counted: Vec<_> = sockets
        .counters()
        .into_iter()
        .filter(|reading| reading.reason == REPORT_INTERFACE_NOT_GIVEN)
        .map(|reading| reading.count)
        .collect();
    assert_eq!(counted, [1]);
}

/// Checks that a send that changes whether reports hold their packet while the host still holds earlier sends counts that.
#[test]
fn a_packet_wish_changed_while_sends_are_held_is_counted() {
    use wcontainers_configurable::Configurable;
    use wcontainers_icmp_sockets_conformance as suite;
    use wcontainers_internet_protocol::{BytesPerSecond, IpSocketSetting};
    use wcontainers_internet_protocol_linux_socket_options::datagram::exchange::PACKET_WISH_CHANGED;
    suite::enter_network_namespace();
    suite::tc(suite::FAIR_QUEUE);
    let sockets = HostIcmpSockets::new().unwrap();
    let socket = sockets.open(IpVersion::V4, UserId(u32::MIN)).unwrap();
    // Slow enough that the second and third sends wait in the fair queue.
    let rate = BytesPerSecond(1000);
    sockets
        .configure(&socket, IpSocketSetting::MaximumPacingRate(Some(rate)))
        .unwrap();
    let to = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
    let message = suite::echo(IpVersion::V4, &[u8::MAX; 1000]);
    for packet in [Report::Requested, Report::Requested, Report::NotRequested] {
        let fields = IpSendFields {
            transmit_reports: TransmitReportRequest {
                key: suite::REPORT_KEY,
                software: Report::Requested,
                packet,
                ..suite::no_reports()
            },
            ..suite::plain_fields()
        };
        sockets
            .send_to(&socket, suite::header_to(to, &message), fields, &message)
            .unwrap();
    }
    let counted: Vec<_> = sockets
        .counters()
        .into_iter()
        .filter(|reading| reading.reason == PACKET_WISH_CHANGED)
        .map(|reading| reading.count)
        .collect();
    assert_eq!(counted, [1]);
}
