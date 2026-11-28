#![no_std]

pub mod authentication;
pub mod multipath;
pub mod repair;
pub mod settings;
pub mod state;

use core::net::SocketAddr;

use vstd::prelude::*;
use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BindingSetting, ByteCount, LocalBinding, ReceiveTimestamps, ReceivedPacketFields, Report,
    TransmitReportRequest,
};
use wcontainers_kernel::UserId;
use wcontainers_kernel::errno::Errno;
use wcontainers_negative_route_advisable::NegativeRouteAdvisable;
use wcontainers_readiness::Readiness;
use wcontainers_volatile_bytes::VolatileBytes;

use crate::authentication::TcpAuthenticationCounters;
use crate::repair::TcpTimestampClock;
use crate::settings::{TcpListenerSetting, TcpQuickAcknowledgement, TcpSetting};
use crate::state::{
    TcpErrorQueueEntry, TcpListenerQuery, TcpListenerReading, TcpQuery, TcpReading,
};

verus! {

/// Sockets for [TCP](https://man7.org/linux/man-pages/man7/tcp.7.html) connections.
pub trait TcpSockets: TcpConnections + TcpListeners {}

#[allow(
    reviewable::explicit_shared_error::HostError,
    reason = "each method is one call on the host's socket and fails only the ways that call does"
)]
pub trait TcpConnections:
    for<'a> Configurable<Self::Connection, TcpSetting<'a>>
    + NegativeRouteAdvisable<Self::Connection>
    + Readiness<Self::Connection>
{
    type Connection: Ord;
    type BoundSocket: Ord;

    /// Opens a socket and binds it, claiming its local address and port now, so that a clash [fails the bind](https://man7.org/linux/man-pages/man2/bind.2.html#:~:text=The%20given%20address%20is%20already%20in%20use) itself.
    fn bind(
        &self,
        protocol: TcpProtocol,
        owner: UserId,
        local: LocalBinding<'_>,
        settings: &[BindingSetting<'_>],
    ) -> Result<TcpBound<Self::BoundSocket>, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn connect(
        &self,
        local: TcpLocalEnd<'_, Self::BoundSocket>,
        peer: SocketAddr,
        settings: &[TcpSetting<'_>],
        fast_open: TcpFastOpen<'_>,
    ) -> Result<TcpConnected<Self::Connection>, TcpLocalEndError<Self::BoundSocket>>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn send<Bytes: VolatileBytes>(
        &self,
        connection: &Self::Connection,
        request: TcpSendRequest,
        payload: &Bytes,
    ) -> Result<ByteCount, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn receive<Bytes: VolatileBytes>(
        &self,
        connection: &Self::Connection,
        request: TcpReceiveRequest,
        into: &Bytes,
    ) -> Result<TcpReceived, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Takes the oldest entry of the connection's error queue, writing any ICMP packet it holds into the buffer.
    fn take_error_queue_entry(
        &self,
        connection: &Self::Connection,
        into: &mut [u8],
    ) -> Result<TcpErrorQueueEntry, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn read_state<'a>(
        &self,
        connection: &Self::Connection,
        query: TcpQuery<'a>,
    ) -> Result<TcpReading<'a>, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn shut_down(
        &self,
        connection: &Self::Connection,
        directions: TcpShutdown,
    ) -> Result<(), HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn close(&self, connection: Self::Connection, manner: TcpClose)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Ends the connection as a [connect to `AF_UNSPEC`](https://man7.org/linux/man-pages/man2/connect.2.html#:~:text=AF_UNSPEC%3B%20thereafter) does, giving back its socket only if that keeps its port, as a port bound by number does.
    fn disconnect(&self, connection: Self::Connection) -> TcpDisconnected<Self::BoundSocket>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Closes the socket, freeing its address and port at once.
    fn release(&self, bound_socket: Self::BoundSocket)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Sends now what of the connection's queued data its windows, its [cork](https://man7.org/linux/man-pages/man7/tcp.7.html#:~:text=TCP_CORK%20%28since) and [Nagle's algorithm](https://www.rfc-editor.org/rfc/rfc9293.html#section-3.7.4) allow.
    ///
    /// The cork and Nagle's algorithm do not count when the push ignores them and stay ignored for that data until more is queued.
    fn push_queued_data(
        &self,
        connection: &Self::Connection,
        ignoring: TcpPushOverride,
    ) -> Result<(), HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Sends an acknowledgement that the connection, established or in CLOSE-WAIT, has scheduled and is delaying.
    ///
    /// It goes at once if the connection's receive buffer is empty and otherwise when a read empties it while the mode is on, and, only if one was scheduled, the call then puts the connection in the given mode.
    fn send_delayed_acknowledgement(
        &self,
        connection: &Self::Connection,
        afterwards: TcpQuickAcknowledgement,
    ) -> Result<(), HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Times the connection's next keep-alive probe at its keep-alive idle time after the later of the last data and the last acknowledgement it received.
    ///
    /// It times the probe at once if that time is past, and does all of this only when keep-alive is on.
    fn rearm_keep_alive_timer(&self, connection: &Self::Connection) -> Result<(), HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn discard_error_queue_entries(&self, connection: &Self::Connection) -> Result<(), HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Sends the established connection's peer a segment and takes the peer's window from the answer.
    fn send_window_probe(&self, connection: &Self::Connection) -> Result<(), HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Makes the TCP timestamp clock of the connection, which must be in repair mode, read the given value now and count in the given unit.
    fn set_timestamp_clock(
        &self,
        connection: &Self::Connection,
        reading: TcpTimestampClock,
    ) -> Result<(), HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn set_authentication_counters(
        &self,
        connection: &Self::Connection,
        values: TcpAuthenticationCounters,
    ) -> Result<(), HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

#[allow(
    reviewable::explicit_shared_error::HostError,
    reason = "each method is one call on the host's socket and fails only the ways that call does"
)]
pub trait TcpListeners:
    TcpConnections
    + for<'a> Configurable<Self::Listener, TcpListenerSetting<'a>>
    + Readiness<Self::Listener>
{
    type Listener: Ord;

    fn listen(
        &self,
        local: TcpLocalEnd<'_, Self::BoundSocket>,
        backlog: ListenBacklog,
        settings: &[TcpListenerSetting<'_>],
    ) -> Result<TcpListening<Self::Listener>, TcpLocalEndError<Self::BoundSocket>>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn accept<'a>(
        &self,
        listener: &Self::Listener,
        synchronize_header: &'a mut [u8],
    ) -> Result<TcpAccepted<'a, Self::Connection>, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn read_listener_state<'a>(
        &self,
        listener: &Self::Listener,
        query: TcpListenerQuery<'a>,
    ) -> Result<TcpListenerReading<'a>, HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn set_listener_authentication_counters(
        &self,
        listener: &Self::Listener,
        values: TcpAuthenticationCounters,
    ) -> Result<(), HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    /// Makes the TCP timestamp clock of the listener, which must be in repair mode, read the given value now and count in the given unit.
    fn set_listener_timestamp_clock(
        &self,
        listener: &Self::Listener,
        reading: TcpTimestampClock,
    ) -> Result<(), HostError>
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
    fn stop_listening(&self, listener: Self::Listener)
        // TODO: replace this clause with what the method promises.
        ensures
            true,
    ;
}

/// The protocol of a stream socket, [TCP](https://man7.org/linux/man-pages/man7/tcp.7.html) or [MPTCP](https://docs.kernel.org/networking/mptcp.html).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpProtocol {
    Tcp,
    Multipath,
}

/// The use of [TCP Fast Open](https://www.rfc-editor.org/rfc/rfc7413.html) by a connect: none, with data for the SYN, or `TCP_FASTOPEN_CONNECT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpFastOpen<'a> {
    Off,
    WithData(&'a [u8]),
    UntilFirstSend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpConnected<C> {
    pub connection: C,
    pub local: SocketAddr,
    pub start: TcpConnectStart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpConnectStart {
    SynchronizeSent { fast_open_data_taken: ByteCount },
    DeferredUntilFirstSend,
    EstablishedForRepair,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListenBacklog(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpListening<L> {
    pub listener: L,
    pub local: SocketAddr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpBound<B> {
    pub bound_socket: B,
    pub local: SocketAddr,
}

/// The socket a connect or listen starts from: one bound before, which the call takes, or a new one the call binds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpLocalEnd<'a, B> {
    Bound(B),
    Unbound {
        protocol: TcpProtocol,
        owner: UserId,
        local: LocalBinding<'a>,
    },
}

/// The error of a connect or listen, giving back the socket it was given if that still holds its port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpLocalEndError<B> {
    Bound { bound_socket: B, error: HostError },
    Unbound(HostError),
}

/// A connection's socket after a disconnect, with the error left pending on it, such as the `ECONNRESET` of a disconnect that reset the connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpDisconnected<B> {
    pub bound_socket: Option<B>,
    pub pending_error: Option<Errno>,
}

/// A connection taken from a listener, with the IP header of its SYN cut to the buffer given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpAccepted<'a, C> {
    pub connection: C,
    pub peer: SocketAddr,
    pub local: SocketAddr,
    pub synchronize: ReceivedPacketFields,
    pub synchronize_header: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpUrgency {
    Normal,
    Urgent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpContinuation {
    Push,
    MoreToCome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpRecordBoundary {
    Open,
    Ended,
}

/// The flags of one [send](https://man7.org/linux/man-pages/man2/send.2.html) on a connection: `MSG_OOB`, `MSG_MORE`, `MSG_EOR` and its timestamping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpSendRequest {
    pub urgency: TcpUrgency,
    pub continuation: TcpContinuation,
    pub record: TcpRecordBoundary,
    pub transmit_reports: TransmitReportRequest,
    pub transmit_statistics: Report,
}

/// The limits a push ignores for the data already queued: neither, or both the [cork](https://man7.org/linux/man-pages/man7/tcp.7.html#:~:text=TCP_CORK%20%28since) and [Nagle's algorithm](https://www.rfc-editor.org/rfc/rfc9293.html#section-3.7.4), as setting `TCP_NODELAY` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpPushOverride {
    Neither,
    CorkAndNagle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpReceiveRequest {
    Read,
    Peek { offset: ByteCount },
    ReadUrgentByte,
    PeekUrgentByte,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpReceived {
    pub length: ByteCount,
    pub timestamps: ReceiveTimestamps,
    pub unread_after: Option<ByteCount>,
}

/// The direction or directions of a connection that one [shutdown](https://man7.org/linux/man-pages/man2/shutdown.2.html) ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpShutdown {
    Read,
    Write,
    ReadAndWrite,
}

/// A way to close a connection: in order, or by [RFC 9293](https://www.rfc-editor.org/rfc/rfc9293#section-3.10.5)'s ABORT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpClose {
    Orderly,
    Abort,
}

} // verus!
