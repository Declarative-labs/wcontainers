//! The policy's TCP: refused, as every other request to the outside.

use core::convert::Infallible;

use core::net::SocketAddr;

use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{ByteCount, LocalBinding, LocalPort};
use wcontainers_kernel::UserId;
use wcontainers_negative_route_advisable::NegativeRouteAdvisable;
use wcontainers_readiness::{Readiness, ReadyFor};
use wcontainers_tcp_sockets::authentication::TcpAuthenticationCounters;
use wcontainers_tcp_sockets::repair::TcpTimestampClock;
use wcontainers_tcp_sockets::settings::{TcpListenerSetting, TcpQuickAcknowledgement, TcpSetting};
use wcontainers_tcp_sockets::state::{TcpErrorQueueEntry, TcpListenerQuery, TcpQuery, TcpReading};
use wcontainers_tcp_sockets::{
    ListenBacklog, TcpAccepted, TcpClose, TcpConnected, TcpConnections, TcpFastOpen, TcpListeners,
    TcpListening, TcpProtocol, TcpPushOverride, TcpReceiveRequest, TcpReceived, TcpSendRequest,
    TcpShutdown,
};

use crate::TestPolicy;
use wcontainers_abi::Host;

impl<H: Host> TcpConnections for TestPolicy<H> {
    type Connection = Infallible;

    fn connect(
        &self,
        _protocol: TcpProtocol,
        _owner: UserId,
        _local: LocalBinding<'_>,
        peer: SocketAddr,
        _settings: &[TcpSetting<'_>],
        _fast_open: TcpFastOpen<'_>,
    ) -> Result<TcpConnected<Self::Connection>, HostError> {
        self.refuse(u64::from(peer.port()))?;
        Err(HostError::NotSupported)
    }

    fn send(
        &self,
        connection: &Self::Connection,
        _request: TcpSendRequest,
        _bytes: &[u8],
    ) -> Result<ByteCount, HostError> {
        match *connection {}
    }

    fn receive(
        &self,
        connection: &Self::Connection,
        _request: TcpReceiveRequest,
        _into: &mut [u8],
    ) -> Result<TcpReceived, HostError> {
        match *connection {}
    }

    fn take_error_queue_entry(
        &self,
        connection: &Self::Connection,
        _into: &mut [u8],
    ) -> Result<TcpErrorQueueEntry, HostError> {
        match *connection {}
    }

    fn read_state<'a>(
        &self,
        connection: &Self::Connection,
        _query: TcpQuery<'a>,
    ) -> Result<TcpReading<'a>, HostError> {
        match *connection {}
    }

    fn shut_down(
        &self,
        connection: &Self::Connection,
        _directions: TcpShutdown,
    ) -> Result<(), HostError> {
        match *connection {}
    }

    fn close(&self, connection: Self::Connection, _manner: TcpClose) {
        match connection {}
    }

    fn push_queued_data(
        &self,
        connection: &Self::Connection,
        _ignoring: TcpPushOverride,
    ) -> Result<(), HostError> {
        match *connection {}
    }

    fn send_delayed_acknowledgement(
        &self,
        connection: &Self::Connection,
        _afterwards: TcpQuickAcknowledgement,
    ) -> Result<(), HostError> {
        match *connection {}
    }

    fn rearm_keep_alive_timer(&self, connection: &Self::Connection) -> Result<(), HostError> {
        match *connection {}
    }

    fn discard_error_queue_entries(&self, connection: &Self::Connection) -> Result<(), HostError> {
        match *connection {}
    }

    fn send_window_probe(&self, connection: &Self::Connection) -> Result<(), HostError> {
        match *connection {}
    }

    fn set_timestamp_clock(
        &self,
        connection: &Self::Connection,
        _reading: TcpTimestampClock,
    ) -> Result<(), HostError> {
        match *connection {}
    }

    fn set_authentication_counters(
        &self,
        connection: &Self::Connection,
        _values: TcpAuthenticationCounters,
    ) -> Result<(), HostError> {
        match *connection {}
    }
}

impl<H: Host> TcpListeners for TestPolicy<H> {
    type Listener = Infallible;

    fn listen(
        &self,
        _protocol: TcpProtocol,
        _owner: UserId,
        local: LocalBinding<'_>,
        _backlog: ListenBacklog,
        _settings: &[TcpListenerSetting<'_>],
    ) -> Result<TcpListening<Self::Listener>, HostError> {
        let port = match local.port {
            LocalPort::Requested(port) | LocalPort::ChosenByKernel(port) => port.0,
            LocalPort::ChosenByExitNode(_) => 0,
        };
        self.refuse(u64::from(port))?;
        Err(HostError::NotSupported)
    }

    fn accept<'a>(
        &self,
        listener: &Self::Listener,
        _synchronize_header: &'a mut [u8],
    ) -> Result<TcpAccepted<'a, Self::Connection>, HostError> {
        match *listener {}
    }

    fn read_listener_state<'a>(
        &self,
        listener: &Self::Listener,
        _query: TcpListenerQuery<'a>,
    ) -> Result<TcpReading<'a>, HostError> {
        match *listener {}
    }

    fn set_listener_authentication_counters(
        &self,
        listener: &Self::Listener,
        _values: TcpAuthenticationCounters,
    ) -> Result<(), HostError> {
        match *listener {}
    }

    fn set_listener_timestamp_clock(
        &self,
        listener: &Self::Listener,
        _reading: TcpTimestampClock,
    ) -> Result<(), HostError> {
        match *listener {}
    }

    fn stop_listening(&self, listener: Self::Listener) {
        match listener {}
    }
}

impl<H: Host> Readiness<Infallible> for TestPolicy<H> {
    async fn wait_until_ready(
        &self,
        connection: &Infallible,
        _interest: ReadyFor,
    ) -> Result<ReadyFor, HostError> {
        match *connection {}
    }
}

impl<'a, H: Host> Configurable<Infallible, TcpSetting<'a>> for TestPolicy<H> {
    fn configure(
        &self,
        connection: &Infallible,
        _setting: TcpSetting<'a>,
    ) -> Result<(), HostError> {
        match *connection {}
    }
}

impl<'a, H: Host> Configurable<Infallible, TcpListenerSetting<'a>> for TestPolicy<H> {
    fn configure(
        &self,
        listener: &Infallible,
        _setting: TcpListenerSetting<'a>,
    ) -> Result<(), HostError> {
        match *listener {}
    }
}

impl<H: Host> NegativeRouteAdvisable<Infallible> for TestPolicy<H> {
    fn give_negative_route_advice(&self, connection: &Infallible) -> Result<(), HostError> {
        match *connection {}
    }
}
