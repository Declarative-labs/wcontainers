//! The tests every implementation of wcontainers-udp-sockets runs, written against its traits alone.

use core::net::Ipv4Addr;

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv4Options};
use wcontainers_internet_protocol::{
    BindAddress, BroadcastPermission, ByteCount, IpHeaderFields, IpSendFields, IpSocketSetting,
    IpVersion, Ipv4HeaderFields, Ipv4RouteSelectors, LocalBinding, LocalPort,
    NeighbourConfirmation, PacketMark, PacketPriority, PathMtuDiscovery, Port, Report, RouteScope,
    RouteSelectors, SourceAddressCheck, TransmitReportKey, TransmitReportRequest, TransportPorts,
    UpperLayer, UpperLayerSelector,
};
use wcontainers_kernel_interface::UserId;
use wcontainers_udp_sockets::{
    UdpChecksum, UdpDatagram, UdpProtocol, UdpSocketSetting, UdpSockets,
};

/// Calls each method of `UdpSockets` as a placeholder that fails until the real checks are written.
pub fn call_udp_sockets_methods<S: UdpSockets>(sockets: S) {
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
            protocol: IpNumber::UDP,
            selector: Some(UpperLayerSelector::Ports(TransportPorts {
                source: Port(u16::MIN),
                destination: Port(u16::MIN),
            })),
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
    let datagram = UdpDatagram {
        checksum: UdpChecksum::Computed,
        segment_size: None,
        payload: &[],
    };
    let local = LocalBinding {
        address: BindAddress::V4(Ipv4Addr::UNSPECIFIED),
        port: LocalPort::Requested(Port(u16::MIN)),
    };
    if let Ok(socket) = sockets.open(IpVersion::V4, UdpProtocol::Udp, UserId(u32::MIN)) {
        let _ = sockets.bind(&socket, local);
        let _ = sockets.configure(
            &socket,
            UdpSocketSetting::Ip(IpSocketSetting::SendBuffer(ByteCount(usize::MIN))),
        );
        let _ = sockets.connect(&socket, RouteSelectors::V4(route));
        let _ = sockets.disconnect(&socket);
        let _ = sockets.send_to(&socket, header, fields, datagram);
        let _ = sockets.receive(&socket, &mut []);
        let _ = sockets.count_unsent_bytes(&socket);
        let _ = sockets.read_incoming_processor(&socket);
        let _ = sockets.give_negative_route_advice(&socket);
        sockets.close(socket);
    }
    todo!("this is a placeholder: the real checks of UdpSockets are not written yet");
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn call_udp_sockets_methods() {
            $crate::call_udp_sockets_methods($value);
        }
    };
}
