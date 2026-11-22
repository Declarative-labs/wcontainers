//! Implements the local system's UDP on the host's sockets, each datagram given as a whole packet.

use core::net::{IpAddr, SocketAddr};
use core::num::NonZeroU32;

use etherparse::{IpNumber, Ipv4Header, Ipv6Header, UdpHeader};
use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BufferSizes, ByteCount, DropCount, InterfaceIndex, IpHeaderFields, IpSendFields, IpVersion,
    LocalBinding, PacketMark, PacketPriority, ProcessorIndex, ReceiveTimestamps,
    ReceivedPacketFields, RouteSelectors, SendError, UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_kernel::errno::Errno;
use wcontainers_udp_sockets::{
    UdpDatagram, UdpLocalEnd, UdpProtocol, UdpReceived, UdpReceivedPacket, UdpSocketSetting,
    UdpSockets,
};

use crate::LocalSystem;
use crate::tcp::NetworkHandle;

/// Gives the length of the IP and UDP headers of a datagram of `peer`'s address family.
fn headers_length(peer: SocketAddr) -> usize {
    UdpHeader::LEN
        + match peer {
            SocketAddr::V4(_) => Ipv4Header::MIN_LEN,
            SocketAddr::V6(_) => Ipv6Header::LEN,
        }
}

/// Gives an address in `family`'s address family, mapped, or unspecified where it has no form.
fn in_family(address: SocketAddr, family: SocketAddr) -> IpAddr {
    match (address.ip(), family) {
        (IpAddr::V4(v4), SocketAddr::V6(_)) => IpAddr::V6(v4.to_ipv6_mapped()),
        (IpAddr::V6(v6), SocketAddr::V4(_)) => v6
            .to_ipv4_mapped()
            .map_or(IpAddr::V4(core::net::Ipv4Addr::UNSPECIFIED), IpAddr::V4),
        (address, _) => address,
    }
}

/// Receives a datagram from `peer` to `local` through `payload` into `into` as a whole packet.
///
/// `payload` fills the buffer it is given and returns how many bytes it wrote; this returns the
/// packet's length from its IP header.
pub fn receive_packet(
    peer: SocketAddr,
    local: SocketAddr,
    into: &mut [u8],
    payload: impl FnOnce(&mut [u8]) -> Result<usize, HostError>,
) -> Result<usize, HostError> {
    let headers = headers_length(peer);
    let room = into
        .get_mut(headers..)
        .ok_or(HostError::Errno(Errno::EMSGSIZE as i32))?;
    let length = payload(room)?;
    let too_long = HostError::Errno(Errno::EMSGSIZE as i32);
    let udp = UdpHeader::without_ipv4_checksum(peer.port(), local.port(), length)
        .map_err(|_| too_long)?;
    let udp_length = UdpHeader::LEN + length;
    let ip_length = match (peer.ip(), in_family(local, peer)) {
        (IpAddr::V4(source), IpAddr::V4(destination)) => {
            let ip = Ipv4Header::new(
                u16::try_from(udp_length).map_err(|_| too_long)?,
                Ipv4Header::default().time_to_live,
                IpNumber::UDP,
                source.octets(),
                destination.octets(),
            )
            .map_err(|_| too_long)?;
            into[..Ipv4Header::MIN_LEN].copy_from_slice(&ip.to_bytes());
            Ipv4Header::MIN_LEN
        }
        (IpAddr::V6(source), IpAddr::V6(destination)) => {
            let ip = Ipv6Header {
                payload_length: u16::try_from(udp_length).map_err(|_| too_long)?,
                next_header: IpNumber::UDP,
                source: source.octets(),
                destination: destination.octets(),
                ..Ipv6Header::default()
            };
            into[..Ipv6Header::LEN].copy_from_slice(&ip.to_bytes());
            Ipv6Header::LEN
        }
        _ => return Err(HostError::Errno(Errno::EAFNOSUPPORT as i32)),
    };
    into[ip_length..headers].copy_from_slice(&udp.to_bytes());
    Ok(headers + length)
}

/// Describes a whole packet of `length` bytes received, of which nothing else is known.
pub fn received(length: usize) -> UdpReceived {
    UdpReceived::Packet(UdpReceivedPacket {
        fields: ReceivedPacketFields {
            length,
            interface: InterfaceIndex(NonZeroU32::MIN),
            specific_destination: None,
            largest_fragment: None,
            link_layer_length: None,
            timestamps: ReceiveTimestamps {
                software: None,
                hardware: None,
            },
            mark: PacketMark(u32::MIN),
            priority: PacketPriority(u32::MIN),
            drops_since_previous: DropCount(u32::MIN),
        },
        segment_size: None,
    })
}

/// Gives the destination address and port a route selects.
fn destination(route: &RouteSelectors<'_>) -> Result<SocketAddr, HostError> {
    let (address, upper_layer) = match route {
        RouteSelectors::V4(route) => (IpAddr::V4(route.destination), route.upper_layer),
        RouteSelectors::V6(route) => (IpAddr::V6(route.destination), route.upper_layer),
    };
    match upper_layer.selector {
        Some(UpperLayerSelector::Ports(ports)) => Ok(SocketAddr::new(address, ports.destination.0)),
        _ => Err(HostError::NotSupported),
    }
}

impl UdpSockets for LocalSystem {
    type Socket = NetworkHandle;

    fn open(
        &self,
        version: IpVersion,
        protocol: UdpProtocol,
        _owner: UserId,
    ) -> Result<Self::Socket, HostError> {
        if protocol != UdpProtocol::Udp {
            return Err(HostError::NotSupported);
        }
        self.state()
            .network()
            .udp_open(version == IpVersion::V6)
            .map(NetworkHandle)
    }

    fn bind(
        &self,
        _socket: &Self::Socket,
        _local: LocalBinding<'_>,
    ) -> Result<SocketAddr, HostError> {
        Err(HostError::NotSupported)
    }

    fn connect(
        &self,
        socket: &Self::Socket,
        route: RouteSelectors<'_>,
    ) -> Result<SocketAddr, HostError> {
        let to = destination(&route)?;
        self.state().network().udp_connect(socket.0, to)
    }

    fn disconnect(&self, _socket: &Self::Socket) -> Result<UdpLocalEnd, HostError> {
        Err(HostError::NotSupported)
    }

    fn send_to(
        &self,
        socket: &Self::Socket,
        _header: IpHeaderFields<'_>,
        _fields: IpSendFields,
        datagram: UdpDatagram<'_>,
    ) -> Result<(), SendError> {
        self.state()
            .network()
            .udp_send(socket.0, datagram.payload)
            .map(|_| ())
            .map_err(SendError::Host)
    }

    fn receive(&self, socket: &Self::Socket, into: &mut [u8]) -> Result<UdpReceived, HostError> {
        self.state()
            .network()
            .udp_receive(socket.0, into)
            .map(received)
    }

    fn count_unsent_bytes(&self, _socket: &Self::Socket) -> Result<ByteCount, HostError> {
        Err(HostError::NotSupported)
    }

    fn read_buffer_sizes(&self, _socket: &Self::Socket) -> Result<BufferSizes, HostError> {
        Err(HostError::NotSupported)
    }

    fn read_incoming_processor(
        &self,
        _socket: &Self::Socket,
    ) -> Result<Option<ProcessorIndex>, HostError> {
        Err(HostError::NotSupported)
    }

    fn close(&self, socket: Self::Socket) {
        self.state().network().udp_close(socket.0);
    }
}

impl<'a> Configurable<NetworkHandle, UdpSocketSetting<'a>> for LocalSystem {
    fn configure(
        &self,
        _socket: &NetworkHandle,
        _setting: UdpSocketSetting<'a>,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}
