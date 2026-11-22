#![no_std]

use etherparse::IpNumber;
use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    ByteCount, IpHeaderFields, IpSendFields, IpSocketSetting, IpVersion, Ipv4RouteSelectors,
    Ipv6RouteSelectors, Received, RouterAlertValue, SendError,
};
use wcontainers_kernel::UserId;
use wcontainers_readiness::Readiness;

/// Sockets for [raw](https://man7.org/linux/man-pages/man7/raw.7.html) IP packets.
pub trait RawSockets:
    for<'a> Configurable<Self::Socket, RawSocketSetting<'a>> + Readiness<Self::Socket>
{
    type Socket: Ord;

    fn open(
        &self,
        version: IpVersion,
        protocol: IpNumber,
        owner: UserId,
    ) -> Result<Self::Socket, HostError>;
    fn send_to(
        &self,
        socket: &Self::Socket,
        packet: RawPacket<'_>,
        fields: IpSendFields,
    ) -> Result<(), SendError>;
    fn receive(&self, socket: &Self::Socket, into: &mut [u8]) -> Result<Received, HostError>;
    /// Returns the bytes sent on the socket that have not yet left its send buffer.
    fn count_unsent_bytes(&self, socket: &Self::Socket) -> Result<ByteCount, HostError>;
    fn close(&self, socket: Self::Socket);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RawSocketSetting<'a> {
    Ip(IpSocketSetting<'a>),
    Checksum(Option<ChecksumOffset>),
    RouterAlert(RouterAlert),
    MulticastRouting(MulticastRouting),
}

/// Where in a packet's data its checksum sits, for the stack to fill in on send and check on receipt, as [`IPV6_CHECKSUM`](https://www.rfc-editor.org/rfc/rfc3542.html#section-3.1) sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChecksumOffset(pub u32);

/// The packets carrying a [router alert](https://man7.org/linux/man-pages/man2/IP_ROUTER_ALERT.2const.html) that the socket takes instead of their being forwarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouterAlert {
    Off,
    Ipv4,
    Ipv6 {
        value: RouterAlertValue,
        isolation: RouterAlertIsolation,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouterAlertIsolation {
    AllNamespaces,
    OwnNamespace,
}

/// The multicast routing role of a socket, which hears every multicast packet and membership report on the link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MulticastRouting {
    Disabled,
    Enabled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RawPacket<'a> {
    Payload {
        header: IpHeaderFields<'a>,
        payload: &'a [u8],
    },
    HeaderIncluded(HeaderIncludedPacket<'a>),
}

/// A whole IP packet whose header the program wrote, as [`IP_HDRINCL`](https://man7.org/linux/man-pages/man2/IP_HDRINCL.2const.html) sends it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeaderIncludedPacket<'a> {
    V4 {
        route: Ipv4RouteSelectors<'a>,
        reassembly: OutgoingReassembly,
        packet: &'a [u8],
    },
    V6 {
        route: Ipv6RouteSelectors<'a>,
        packet: &'a [u8],
    },
}

/// The firewall's permission to reassemble outgoing fragments, which [`IP_NODEFRAG`](https://man7.org/linux/man-pages/man2/IP_NODEFRAG.2const.html) withdraws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutgoingReassembly {
    Allowed,
    Disabled,
}
