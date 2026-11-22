#![no_std]
#![allow(async_fn_in_trait)]

use core::num::NonZeroU16;

use wcontainers_abi::HostError;
use wcontainers_abi::readiness::Readiness;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    ByteCount, IpHeaderFields, IpSendFields, IpSocketSetting, IpVersion, Received, SendError,
};
use wcontainers_kernel::UserId;

/// Sockets for [ICMP](https://man7.org/linux/man-pages/man7/icmp.7.html) echo requests and replies.
pub trait IcmpSockets: for<'a> Configurable<Self::Socket, IpSocketSetting<'a>> {
    type Socket: Ord;

    fn open(&self, version: IpVersion, owner: UserId) -> Result<Self::Socket, HostError>;
    fn bind_identifier(
        &self,
        socket: &Self::Socket,
        identifier: EchoIdentifier,
        sharing: IdentifierSharing,
    ) -> Result<(), HostError>;
    fn send_to(
        &self,
        socket: &Self::Socket,
        header: IpHeaderFields<'_>,
        fields: IpSendFields,
        message: &[u8],
    ) -> Result<(), SendError>;
    fn receive(&self, socket: &Self::Socket, into: &mut [u8]) -> Result<Received, HostError>;
    /// Returns the bytes sent on the socket that have not yet left its send buffer.
    fn count_unsent_bytes(&self, socket: &Self::Socket) -> Result<ByteCount, HostError>;
    /// Waits until the socket has one of the events asked for, and returns the events it has.
    async fn wait_for_events(
        &self,
        socket: &Self::Socket,
        interest: Readiness,
    ) -> Result<Readiness, HostError>;
    fn close(&self, socket: Self::Socket);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EchoIdentifier(pub NonZeroU16);

/// The sharing of an echo identifier with other sockets that [`SO_REUSEADDR`](https://man7.org/linux/man-pages/man7/socket.7.html) allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentifierSharing {
    Exclusive,
    Shared,
}
