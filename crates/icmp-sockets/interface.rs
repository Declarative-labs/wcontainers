#![no_std]

use core::net::{Ipv4Addr, Ipv6Addr};
use core::num::NonZeroU16;

use vstd::prelude::*;
use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BufferSizes, ByteCount, InterfaceIndex, IpHeaderFields, IpSendFields, IpSocketSetting,
    IpVersion, Received, SendError,
};
use wcontainers_kernel::UserId;
use wcontainers_readiness::Readiness;
use wcontainers_volatile_bytes::VolatileBytes;

verus! {

#[allow(
    reviewable::explicit_shared_error::HostError,
    reason = "each method is one call on the host's socket and fails only the ways that call does"
)]
/// Sockets for [ICMP](https://man7.org/linux/man-pages/man7/icmp.7.html) echo requests and replies.
pub trait IcmpSockets:
    for<'a> Configurable<Self::Socket, IpSocketSetting<'a>> + Readiness<Self::Socket>
{
    type Socket: Ord;

    fn open(&self, version: IpVersion, owner: UserId) -> Result<Self::Socket, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Binds the socket to a local address and an echo identifier and returns the identifier bound.
    fn bind(&self, socket: &Self::Socket, local: EchoBinding) -> Result<EchoIdentifier, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn send_to(
        &self,
        socket: &Self::Socket,
        header: IpHeaderFields<'_>,
        fields: IpSendFields,
        message: &impl VolatileBytes,
    ) -> Result<(), SendError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn receive(
        &self,
        socket: &Self::Socket,
        into: &impl VolatileBytes,
    ) -> Result<Received, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Returns the bytes sent on the socket that have not yet left its send buffer.
    fn count_unsent_bytes(&self, socket: &Self::Socket) -> ByteCount
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn read_buffer_sizes(&self, socket: &Self::Socket) -> BufferSizes
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
pub struct EchoIdentifier(pub NonZeroU16);

/// The sharing of an echo identifier with other sockets that [`SO_REUSEADDR`](https://man7.org/linux/man-pages/man7/socket.7.html#:~:text=SO_REUSEADDR) allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentifierSharing {
    Exclusive,
    Shared,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EchoBinding {
    pub address: EchoBindAddress,
    pub identifier: LocalIdentifier,
    pub sharing: IdentifierSharing,
}

/// An address an echo socket is bound to, the unspecified one for any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EchoBindAddress {
    V4(Ipv4Addr),
    V6 {
        address: Ipv6Addr,
        /// The interface a link-local address [belongs to](https://man7.org/linux/man-pages/man7/ipv6.7.html#:~:text=Linux%20supports%20it%20only%20for%20link%2Dlocal%20addresses), which Linux binds the socket to; for any other address Linux only checks that the interface exists.
        scope: Option<InterfaceIndex>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalIdentifier {
    Requested(EchoIdentifier),
    ChosenByExitNode,
}

} // verus!
