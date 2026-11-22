#![no_std]
#![allow(async_fn_in_trait)]

use core::net::SocketAddr;
use core::num::NonZeroU16;

use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    ByteCount, IpHeaderFields, IpSendFields, IpSocketSetting, IpVersion, LocalBinding, PortSetting,
    ProcessorIndex, ReceivedPacketFields, RouteSelectors, SendError, TransmitReport,
};
use wcontainers_kernel::UserId;
use wcontainers_kernel::signal_information::PollEvents;
use wcontainers_negative_route_advice::NegativeRouteAdvice;

/// Sockets for [UDP](https://man7.org/linux/man-pages/man7/udp.7.html) and [UDP-Lite](https://man7.org/linux/man-pages/man7/udplite.7.html) datagrams.
pub trait UdpSockets:
    for<'a> Configurable<Self::Socket, UdpSocketSetting<'a>> + NegativeRouteAdvice<Self::Socket>
{
    type Socket: Ord;

    fn open(
        &self,
        version: IpVersion,
        protocol: UdpProtocol,
        owner: UserId,
    ) -> Result<Self::Socket, HostError>;
    /// Binds the socket to a local address and returns the address bound.
    fn bind(&self, socket: &Self::Socket, local: LocalBinding<'_>)
    -> Result<SocketAddr, HostError>;
    /// Sets the socket's peer to the route's destination and ports and returns the socket's local address.
    fn connect(
        &self,
        socket: &Self::Socket,
        route: RouteSelectors<'_>,
    ) -> Result<SocketAddr, HostError>;
    /// Ends the socket's association with its peer, as a [`connect`](https://man7.org/linux/man-pages/man2/connect.2.html) to `AF_UNSPEC` does, and returns what stays bound.
    fn disconnect(&self, socket: &Self::Socket) -> Result<UdpLocalEnd, HostError>;
    fn send_to(
        &self,
        socket: &Self::Socket,
        header: IpHeaderFields<'_>,
        fields: IpSendFields,
        datagram: UdpDatagram<'_>,
    ) -> Result<(), SendError>;
    fn receive(&self, socket: &Self::Socket, into: &mut [u8]) -> Result<UdpReceived, HostError>;
    /// Returns the bytes sent on the socket that have not yet left its send buffer.
    fn count_unsent_bytes(&self, socket: &Self::Socket) -> Result<ByteCount, HostError>;
    /// Returns the socket's incoming processor, the one set or, once the socket is connected, the one that last took in a packet for it.
    fn read_incoming_processor(
        &self,
        socket: &Self::Socket,
    ) -> Result<Option<ProcessorIndex>, HostError>;
    /// Waits until the socket has one of the events asked for, and returns the events it has.
    async fn wait_for_events(
        &self,
        socket: &Self::Socket,
        interest: PollEvents,
    ) -> Result<PollEvents, HostError>;
    fn close(&self, socket: Self::Socket);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UdpProtocol {
    Udp,
    Lite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UdpLocalEnd {
    Bound(SocketAddr),
    Unbound,
}

/// The bytes from the start of a [UDP-Lite](https://man7.org/linux/man-pages/man7/udplite.7.html) datagram that its checksum covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChecksumCoverage(pub NonZeroU16);

/// The payload length of each datagram that one send makes or one receive gives, [`UDP_SEGMENT`](https://man7.org/linux/man-pages/man7/udp.7.html) and `UDP_GRO`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentSize(pub NonZeroU16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiveCoalescing {
    Separate,
    BySegmentSize,
}

/// The taking of IPv6 datagrams without a checksum, `UDP_NO_CHECK6_RX`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZeroChecksumReceipt {
    Dropped,
    Accepted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UdpSocketSetting<'a> {
    Ip(IpSocketSetting<'a>),
    Port(PortSetting<'a>),
    ReceiveCoalescing(ReceiveCoalescing),
    ZeroChecksumReceipt(ZeroChecksumReceipt),
    MinimumChecksumCoverage(Option<ChecksumCoverage>),
}

/// The checksum of a datagram sent: whole, left out as `SO_NO_CHECK` and `UDP_NO_CHECK6_TX` ask, or over a UDP-Lite coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UdpChecksum {
    Computed,
    Omitted,
    Partial(ChecksumCoverage),
}

/// One datagram, or one for each segment of the payload, to the ports of its header's upper layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UdpDatagram<'a> {
    pub checksum: UdpChecksum,
    pub segment_size: Option<SegmentSize>,
    pub payload: &'a [u8],
}

/// A packet for the socket, whole from its IP header in the buffer: a datagram, several of one segment size, or an ICMP error about one it sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UdpReceivedPacket {
    pub fields: ReceivedPacketFields,
    pub segment_size: Option<SegmentSize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UdpReceived {
    Packet(UdpReceivedPacket),
    TransmitReport(TransmitReport),
}
