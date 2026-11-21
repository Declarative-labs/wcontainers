//! The tests every implementation of wcontainers-icmp-sockets runs, written against its traits alone.

use core::net::Ipv4Addr;
use core::num::NonZeroU16;

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv4Options};
use wcontainers_icmp_sockets::{EchoIdentifier, IcmpSockets, IdentifierSharing};
use wcontainers_internet_protocol::{
    BroadcastPermission, ByteCount, IpHeaderFields, IpSendFields, IpSocketSetting, IpVersion,
    Ipv4HeaderFields, Ipv4RouteSelectors, NeighbourConfirmation, PacketMark, PacketPriority,
    PathMtuDiscovery, Report, RouteScope, SourceAddressCheck, TransmitReportKey,
    TransmitReportRequest, UpperLayer,
};
use wcontainers_kernel::UserId;

/// Calls each method of `IcmpSockets` as a placeholder that fails until the real checks are written.
pub fn call_icmp_sockets_methods<S: IcmpSockets>(sockets: S) {
    let options = Ipv4Options::new();
    let route = Ipv4RouteSelectors {
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
            protocol: IpNumber::ICMP,
            selector: None,
        },
    };
    let header = IpHeaderFields::V4(Ipv4HeaderFields {
        route,
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
    if let Ok(socket) = sockets.open(IpVersion::V4, UserId(u32::MIN)) {
        let _ = sockets.bind_identifier(
            &socket,
            EchoIdentifier(NonZeroU16::MIN),
            IdentifierSharing::Exclusive,
        );
        let _ = sockets.configure(&socket, IpSocketSetting::SendBuffer(ByteCount(usize::MIN)));
        let _ = sockets.send_to(&socket, header, fields, &[]);
        let _ = sockets.receive(&socket, &mut []);
        let _ = sockets.count_unsent_bytes(&socket);
        sockets.close(socket);
    }
    todo!("this is a placeholder: the real checks of IcmpSockets are not written yet");
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn call_icmp_sockets_methods() {
            $crate::call_icmp_sockets_methods($value);
        }
    };
}
