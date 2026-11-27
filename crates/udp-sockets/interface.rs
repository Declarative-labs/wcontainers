#![no_std]

use core::net::SocketAddr;
use core::num::NonZeroU16;

use vstd::prelude::*;
use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BindingSetting, BufferSizes, ByteCount, IpHeaderFields, IpSendFields, IpSocketSetting,
    IpVersion, LocalBinding, ProcessorIndex, ReceivedPacketFields, RouteSelectors, SendError,
    TransmitReport,
};
use wcontainers_kernel::UserId;
use wcontainers_negative_route_advisable::NegativeRouteAdvisable;
use wcontainers_readiness::Readiness;
use wcontainers_volatile_bytes::VolatileBytes;

verus! {

#[allow(
    reviewable::explicit_shared_error::HostError,
    reason = "each method is one call on the host's socket and fails only the ways that call does"
)]
/// Sockets for [UDP](https://man7.org/linux/man-pages/man7/udp.7.html) and [UDP-Lite](https://man7.org/linux/man-pages/man7/udplite.7.html) datagrams.
pub trait UdpSockets:
    for<'a> Configurable<Self::Socket, UdpSocketSetting<'a>>
    + NegativeRouteAdvisable<Self::Socket>
    + Readiness<Self::Socket>
{
    type Socket: Ord;

    fn open(
        &self,
        version: IpVersion,
        protocol: UdpProtocol,
        owner: UserId,
    ) -> Result<Self::Socket, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Binds the socket to a local address and returns the address bound.
    fn bind(&self, socket: &Self::Socket, local: LocalBinding<'_>)
    -> Result<SocketAddr, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Sets the socket's peer to the route's destination and ports and returns the socket's local address.
    fn connect(
        &self,
        socket: &Self::Socket,
        route: RouteSelectors<'_>,
    ) -> Result<SocketAddr, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Ends the socket's association with its peer, as a [`connect`](https://man7.org/linux/man-pages/man2/connect.2.html) to `AF_UNSPEC` does, and returns what stays bound.
    fn disconnect(&self, socket: &Self::Socket) -> Result<UdpLocalEnd, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn send_to(
        &self,
        socket: &Self::Socket,
        header: IpHeaderFields<'_>,
        fields: IpSendFields,
        datagram: UdpDatagram<'_>,
    ) -> Result<(), SendError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn receive(
        &self,
        socket: &Self::Socket,
        into: &impl VolatileBytes,
    ) -> Result<UdpReceived, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Returns the bytes sent on the socket that have not yet left its send buffer.
    fn count_unsent_bytes(&self, socket: &Self::Socket) -> Result<ByteCount, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn read_buffer_sizes(&self, socket: &Self::Socket) -> Result<BufferSizes, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Returns the socket's incoming processor, the one set or, once the socket is connected, the one that last took in a packet for it.
    fn read_incoming_processor(
        &self,
        socket: &Self::Socket,
    ) -> Result<Option<ProcessorIndex>, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn close(&self, socket: Self::Socket)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UdpSocketSetting<'a> {
    Ip(IpSocketSetting<'a>),
    Binding(BindingSetting<'a>),
    ReceiveCoalescing(ReceiveCoalescing),
    ZeroChecksumReceipt(ZeroChecksumReceipt),
    MinimumChecksumCoverage(Option<ChecksumCoverage>),
}

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

/// The bytes from the start of a [UDP-Lite](https://man7.org/linux/man-pages/man7/udplite.7.html) datagram that its checksum covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChecksumCoverage(pub NonZeroU16);

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

/// One datagram, or one for each segment of the payload, to the ports of its header's upper layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UdpDatagram<'a> {
    pub checksum: UdpChecksum,
    pub segment_size: Option<SegmentSize>,
    pub payload: &'a [u8],
}

/// The checksum of a datagram sent: whole, left out as `SO_NO_CHECK` and `UDP_NO_CHECK6_TX` ask, or over a UDP-Lite coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UdpChecksum {
    Computed,
    Omitted,
    Partial(ChecksumCoverage),
}

/// The payload length of each datagram that one send makes or one receive gives, [`UDP_SEGMENT`](https://man7.org/linux/man-pages/man7/udp.7.html#:~:text=UDP_SEGMENT%20%28since) and `UDP_GRO`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentSize(pub NonZeroU16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UdpReceived {
    Packet(UdpReceivedPacket),
    TransmitReport(TransmitReport),
}

/// A packet for the socket, whole from its IP header in the buffer: a datagram, several of one segment size, or an ICMP error about one it sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UdpReceivedPacket {
    pub fields: ReceivedPacketFields,
    pub segment_size: Option<SegmentSize>,
}

} // verus!
