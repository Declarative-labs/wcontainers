//! What one send and one receive of a small datagram over loopback take: run with `--ignored --nocapture`.
#![cfg(target_os = "linux")]

use core::cell::RefCell;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv4Options};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    BindAddress, BroadcastPermission, IpHeaderFields, IpSendFields, IpVersion, Ipv4HeaderFields,
    Ipv4RouteSelectors, LocalBinding, LocalPort, NeighbourConfirmation, PacketMark, PacketPriority,
    PathMtuDiscovery, Port, Report, RouteScope, SourceAddressCheck, TransmitReportKey,
    TransmitReportRequest, TransportPorts, UpperLayer, UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_udp_sockets::{UdpChecksum, UdpDatagram, UdpProtocol, UdpSockets};
use wcontainers_udp_sockets_host::{HostUdpSocket, HostUdpSockets};

/// Datagrams sent before they are all received, fewer than a loopback socket's default receive buffer holds.
const BATCH: usize = 32;
/// Batches measured; the median batch is reported.
const BATCHES: usize = 4000;
const PAYLOAD: [u8; 64] = [0x5a; 64];

fn bound(sockets: &HostUdpSockets) -> (HostUdpSocket, SocketAddr) {
    let socket = sockets
        .open(IpVersion::V4, UdpProtocol::Udp, UserId(u32::MIN))
        .expect("open");
    let address = sockets
        .bind(
            &socket,
            LocalBinding {
                address: BindAddress::V4(Ipv4Addr::LOCALHOST),
                port: LocalPort::Requested(Port(u16::MIN)),
            },
        )
        .expect("bind");
    (socket, address)
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2] / u32::try_from(BATCH).expect("a small batch")
}

#[test]
#[ignore = "a measurement, run on its own"]
fn a_send_and_a_receive_take() {
    let sockets = HostUdpSockets::new().expect("make the host's UDP sockets");
    let (sender, from) = bound(&sockets);
    let (receiver, to) = bound(&sockets);
    let IpAddr::V4(destination) = to.ip() else {
        panic!("an IPv4 address")
    };
    let options = Ipv4Options::new();
    let header = IpHeaderFields::V4(Ipv4HeaderFields {
        route: Ipv4RouteSelectors {
            destination,
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
                    source: Port(from.port()),
                    destination: Port(to.port()),
                })),
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
    let datagram = UdpDatagram {
        checksum: UdpChecksum::Computed,
        segment_size: None,
        payload: &PAYLOAD,
    };
    let mut into = [u8::MIN; 2048];
    let (mut sends, mut receives) = (Vec::new(), Vec::new());
    for _ in 0..BATCHES {
        let start = Instant::now();
        for _ in 0..BATCH {
            sockets
                .send_to(&sender, header.clone(), fields, datagram)
                .expect("send");
        }
        sends.push(start.elapsed());
        let start = Instant::now();
        let mut received = 0;
        while received < BATCH {
            match sockets.receive(&receiver, &RefCell::new(&mut into[..])) {
                Ok(_) => received += 1,
                Err(HostError::WouldBlock) => {}
                Err(error) => panic!("receive: {error:?}"),
            }
        }
        receives.push(start.elapsed());
    }
    println!(
        "udp loopback, 64-byte datagrams: send {:?}, receive {:?} (median of {BATCHES} batches of {BATCH})",
        median(sends),
        median(receives)
    );
}
