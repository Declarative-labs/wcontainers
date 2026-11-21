#![no_std]

pub mod authentication;
pub mod multipath;
pub mod repair;
pub mod settings;
pub mod state;

use core::net::SocketAddr;

use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    ByteCount, LocalBinding, ReceiveTimestamps, ReceivedPacketFields, Report, TransmitReportRequest,
};
use wcontainers_kernel::UserId;
use wcontainers_negative_route_advice::NegativeRouteAdvice;

use crate::authentication::TcpAuthenticationCounters;
use crate::repair::TcpTimestampClock;
use crate::settings::{TcpListenerSetting, TcpQuickAcknowledgement, TcpSetting};
use crate::state::{TcpErrorQueueEntry, TcpListenerQuery, TcpQuery, TcpReading};

/// Sockets for [TCP](https://man7.org/linux/man-pages/man7/tcp.7.html) connections.
pub trait TcpSockets: TcpConnections + TcpListeners {}

pub trait TcpConnections:
    for<'a> Configurable<Self::Connection, TcpSetting<'a>> + NegativeRouteAdvice<Self::Connection>
{
    type Connection: Ord;

    fn connect(
        &self,
        protocol: TcpProtocol,
        owner: UserId,
        local: LocalBinding<'_>,
        peer: SocketAddr,
        settings: &[TcpSetting<'_>],
        fast_open: TcpFastOpen<'_>,
    ) -> Result<TcpConnected<Self::Connection>, HostError>;
    fn send(
        &self,
        connection: &Self::Connection,
        request: TcpSendRequest,
        bytes: &[u8],
    ) -> Result<ByteCount, HostError>;
    fn receive(
        &self,
        connection: &Self::Connection,
        request: TcpReceiveRequest,
        into: &mut [u8],
    ) -> Result<TcpReceived, HostError>;
    /// Takes the oldest entry of the connection's error queue, writing any ICMP packet it holds into the buffer.
    fn take_error_queue_entry(
        &self,
        connection: &Self::Connection,
        into: &mut [u8],
    ) -> Result<TcpErrorQueueEntry, HostError>;
    fn read_state<'a>(
        &self,
        connection: &Self::Connection,
        query: TcpQuery<'a>,
    ) -> Result<TcpReading<'a>, HostError>;
    fn shut_down(
        &self,
        connection: &Self::Connection,
        directions: TcpShutdown,
    ) -> Result<(), HostError>;
    fn close(&self, connection: Self::Connection, manner: TcpClose);
    /// Sends now what of the connection's queued data its windows, its cork and [Nagle's algorithm](https://man7.org/linux/man-pages/man7/tcp.7.html) allow, the cork and Nagle's algorithm not counting when the push ignores them and staying ignored for that data until more is queued.
    fn push_queued_data(
        &self,
        connection: &Self::Connection,
        ignoring: TcpPushOverride,
    ) -> Result<(), HostError>;
    /// Sends an acknowledgement that the connection, established or in CLOSE-WAIT, has scheduled and is delaying, at once if its receive buffer is empty and otherwise when a read empties it while the mode is on, and, only if one was scheduled, then puts the connection in the given mode.
    fn send_delayed_acknowledgement(
        &self,
        connection: &Self::Connection,
        afterwards: TcpQuickAcknowledgement,
    ) -> Result<(), HostError>;
    /// Times the connection's next keep-alive probe at its keep-alive idle time after the later of the last data and the last acknowledgement it received, at once if that time is past, when keep-alive is on.
    fn rearm_keep_alive_timer(&self, connection: &Self::Connection) -> Result<(), HostError>;
    fn discard_error_queue_entries(&self, connection: &Self::Connection) -> Result<(), HostError>;
    /// Sends the established connection's peer a segment and takes the peer's window from the answer.
    fn send_window_probe(&self, connection: &Self::Connection) -> Result<(), HostError>;
    /// Makes the TCP timestamp clock of the connection, which must be in repair mode, read the given value now and count in the given unit.
    fn set_timestamp_clock(
        &self,
        connection: &Self::Connection,
        reading: TcpTimestampClock,
    ) -> Result<(), HostError>;
    fn set_authentication_counters(
        &self,
        connection: &Self::Connection,
        values: TcpAuthenticationCounters,
    ) -> Result<(), HostError>;
}

pub trait TcpListeners:
    TcpConnections + for<'a> Configurable<Self::Listener, TcpListenerSetting<'a>>
{
    type Listener: Ord;

    fn listen(
        &self,
        protocol: TcpProtocol,
        owner: UserId,
        local: LocalBinding<'_>,
        backlog: ListenBacklog,
        settings: &[TcpListenerSetting<'_>],
    ) -> Result<TcpListening<Self::Listener>, HostError>;
    fn accept<'a>(
        &self,
        listener: &Self::Listener,
        synchronize_header: &'a mut [u8],
    ) -> Result<TcpAccepted<'a, Self::Connection>, HostError>;
    fn read_listener_state<'a>(
        &self,
        listener: &Self::Listener,
        query: TcpListenerQuery<'a>,
    ) -> Result<TcpReading<'a>, HostError>;
    fn set_listener_authentication_counters(
        &self,
        listener: &Self::Listener,
        values: TcpAuthenticationCounters,
    ) -> Result<(), HostError>;
    /// Makes the TCP timestamp clock of the listener, which must be in repair mode, read the given value now and count in the given unit.
    fn set_listener_timestamp_clock(
        &self,
        listener: &Self::Listener,
        reading: TcpTimestampClock,
    ) -> Result<(), HostError>;
    fn stop_listening(&self, listener: Self::Listener);
}

/// The protocol of a stream socket, [TCP](https://man7.org/linux/man-pages/man7/tcp.7.html) or [MPTCP](https://docs.kernel.org/networking/mptcp.html).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpProtocol {
    Tcp,
    Multipath,
}

/// The use of [TCP Fast Open](https://man7.org/linux/man-pages/man7/tcp.7.html) by a connect: none, with data for the SYN, or `TCP_FASTOPEN_CONNECT`.
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

/// The limits a push ignores for the data already queued: neither, or both the cork and [Nagle's algorithm](https://man7.org/linux/man-pages/man7/tcp.7.html), as setting `TCP_NODELAY` does.
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
