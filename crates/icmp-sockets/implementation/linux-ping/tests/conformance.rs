//! The conformance suite on Linux's ping sockets, and what only this implementation answers.
#![cfg(target_os = "linux")]

use etherparse::IpNumber;
use wcontainers_abi::HostError;
use wcontainers_icmp_sockets::IcmpSockets;
use wcontainers_icmp_sockets_linux_ping::{IPV6_SOURCE, LinuxPingSockets};
use wcontainers_internet_protocol::{
    IcmpCode, IcmpType, IpHeaderFields, IpSendFields, IpVersion, Ipv6HeaderFields,
    Ipv6RouteSelectors, LocalFragmentation, MobileAddressPreference, NeighbourConfirmation,
    PacketMark, PacketPriority, PathMtuDiscovery, Report, SendError, SourceAddressCheck,
    SourcePreferences, TemporaryAddressPreference, TransmitReportKey, TransmitReportRequest,
    UpperLayer, UpperLayerSelector,
};
use wcontainers_kernel::UserId;

wcontainers_icmp_sockets_conformance::tests!(
    wcontainers_icmp_sockets_linux_ping::LinuxPingSockets::new()
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
    let sockets = LinuxPingSockets::new();
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
    let sockets = LinuxPingSockets::new();
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
