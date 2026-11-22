//! The conformance suite on Linux's ping sockets, and what only this implementation answers.
#![cfg(target_os = "linux")]

use etherparse::IpNumber;
use wcontainers_abi::HostError;
use wcontainers_icmp_sockets::IcmpSockets;
use wcontainers_icmp_sockets_host::{
    DROPS_ASKED, HostIcmpSockets, IPV6_SOURCE, RECEIVED_FIELDS_NOT_GIVEN,
};
use wcontainers_internet_protocol::{
    IcmpCode, IcmpType, IpHeaderFields, IpSendFields, IpVersion, Ipv6HeaderFields,
    Ipv6RouteSelectors, LocalFragmentation, MobileAddressPreference, NeighbourConfirmation,
    PacketMark, PacketPriority, PathMtuDiscovery, Report, SendError, SourceAddressCheck,
    SourcePreferences, TemporaryAddressPreference, TransmitReportKey, TransmitReportRequest,
    UpperLayer, UpperLayerSelector,
};
use wcontainers_kernel::UserId;

wcontainers_icmp_sockets_conformance::tests!(
    wcontainers_icmp_sockets_host::HostIcmpSockets::new().expect("the host's readiness")
);

/// Linux makes an echo socket only for a caller whose group is in the namespace's ping group range.
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

/// An IPv6 source Linux's ping socket would not send from is refused and counted under its reason.
#[test]
fn an_ipv6_source_is_refused_and_counted() {
    wcontainers_icmp_sockets_conformance::enter_network_namespace();
    let sockets = HostIcmpSockets::new().unwrap();
    let socket = sockets.open(IpVersion::V6, UserId(u32::MIN)).unwrap();
    let message = [IcmpType(128).0, u8::MIN, 0, 0, 0, 0, 0, 0];
    let route = Ipv6RouteSelectors {
        destination: std::net::Ipv6Addr::LOCALHOST,
        source: Some(std::net::Ipv6Addr::LOCALHOST),
        source_address_check: SourceAddressCheck::Required,
        interface: None,
        dscp: etherparse::IpDscp::ZERO,
        ecn: etherparse::IpEcn::NotEct,
        flow_label: etherparse::Ipv6FlowLabel::ZERO,
        mark: PacketMark(u32::MIN),
        source_preferences: SourcePreferences {
            temporary: TemporaryAddressPreference::SystemDefault,
            mobile: MobileAddressPreference::Home,
        },
        routing_header: None,
        upper_layer: UpperLayer {
            protocol: IpNumber::IPV6_ICMP,
            selector: Some(UpperLayerSelector::Icmp {
                message_type: IcmpType(message[0]),
                code: IcmpCode(message[1]),
            }),
        },
    };
    let header = IpHeaderFields::V6(Ipv6HeaderFields {
        route,
        hop_limit: None,
        path_mtu_discovery: PathMtuDiscovery::Want,
        local_fragmentation: LocalFragmentation::Allowed,
        fragment_mtu: None,
        hop_by_hop_options: None,
        routing_destination_options: None,
        destination_options: None,
    });
    let fields = IpSendFields {
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
    };
    assert_eq!(
        sockets.send_to(&socket, header, fields, &message),
        Err(SendError::Host(HostError::NotSupported))
    );
    let counted: Vec<_> = sockets
        .counters()
        .into_iter()
        .map(|reading| (reading.reason, reading.count))
        .collect();
    assert_eq!(counted, [(IPV6_SOURCE, 1)]);
}

/// Each received packet counts once the fields Linux does not give and the drop count it asks the socket for.
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
    let mut expected = [(DROPS_ASKED, 1), (RECEIVED_FIELDS_NOT_GIVEN, 1)];
    expected.sort();
    assert_eq!(counted, expected);
}

/// A transmit report without its packet, which Linux gives with no interface, counts that once.
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

/// A send that changes whether reports hold their packet while the host still holds earlier sends counts that.
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
