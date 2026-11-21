//! The conformance suite on Linux's raw sockets, and what only this implementation answers.
#![cfg(target_os = "linux")]

use std::net::Ipv4Addr;

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv4Options};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    BroadcastPermission, IpHeaderFields, IpSendFields, IpVersion, Ipv4HeaderFields,
    Ipv4RouteSelectors, NeighbourConfirmation, PacketMark, PacketPriority, PathMtuDiscovery,
    Report, RouteScope, SendError, SourceAddressCheck, TransmitReportKey, TransmitReportRequest,
    UpperLayer,
};
use wcontainers_kernel::UserId;
use wcontainers_raw_sockets::{RawPacket, RawSockets};
use wcontainers_raw_sockets_host::{HostRawSockets, OTHER_PROTOCOL};

wcontainers_raw_sockets_conformance::tests!(
    wcontainers_raw_sockets_host::HostRawSockets::new().expect("the host's readiness")
);

/// A payload under another protocol than its IPv4 socket's is refused and counted under its reason.
#[test]
fn another_protocol_is_refused_and_counted() {
    wcontainers_raw_sockets_conformance::enter_network_namespace();
    let sockets = HostRawSockets::new().unwrap();
    let socket = sockets
        .open(
            IpVersion::V4,
            IpNumber::EXPERIMENTAL_AND_TESTING_0,
            UserId(u32::MIN),
        )
        .unwrap();
    let options = Ipv4Options::new();
    let header = IpHeaderFields::V4(Ipv4HeaderFields {
        route: Ipv4RouteSelectors {
            destination: Ipv4Addr::LOCALHOST,
            source: None,
            source_address_check: SourceAddressCheck::Required,
            interface: None,
            dscp: IpDscp::ZERO,
            ecn: IpEcn::NotEct,
            mark: PacketMark(u32::MIN),
            scope: RouteScope::Universe,
            broadcast: BroadcastPermission::Refused,
            options: &options,
            upper_layer: UpperLayer {
                protocol: IpNumber::EXPERIMENTAL_AND_TESTING_1,
                selector: None,
            },
        },
        time_to_live: None,
        path_mtu_discovery: PathMtuDiscovery::Want,
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
    let payload = RawPacket::Payload {
        header,
        payload: b"payload",
    };
    assert_eq!(
        sockets.send_to(&socket, payload, fields),
        Err(SendError::Host(HostError::NotSupported))
    );
    let counted: Vec<_> = sockets
        .counters()
        .into_iter()
        .map(|reading| (reading.reason, reading.count))
        .collect();
    assert_eq!(counted, [(OTHER_PROTOCOL, 1)]);
}
