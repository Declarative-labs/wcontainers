//! The policy's TCP: refused, as every other request to the outside.

use core::convert::Infallible;

use core::net::SocketAddr;

use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{BindingSetting, ByteCount, LocalBinding, LocalPort};
use wcontainers_kernel::UserId;
use wcontainers_negative_route_advisable::NegativeRouteAdvisable;
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor};
use wcontainers_tcp_sockets::authentication::TcpAuthenticationCounters;
use wcontainers_tcp_sockets::repair::TcpTimestampClock;
use wcontainers_tcp_sockets::settings::{TcpListenerSetting, TcpQuickAcknowledgement, TcpSetting};
use wcontainers_tcp_sockets::state::{
    TcpErrorQueueEntry, TcpListenerQuery, TcpListenerReading, TcpQuery, TcpReading,
};
use wcontainers_tcp_sockets::{
    ListenBacklog, TcpAccepted, TcpBound, TcpClose, TcpConnected, TcpConnections, TcpDisconnected,
    TcpFastOpen, TcpListeners, TcpListening, TcpLocalEnd, TcpLocalEndError, TcpProtocol,
    TcpPushOverride, TcpReceiveRequest, TcpReceived, TcpSendRequest, TcpShutdown,
};

use crate::TestPolicy;
use wcontainers_abi::Host;
use wcontainers_volatile_bytes::VolatileBytes;

/// Returns the port a binding asks for, 0 where the exit node chooses it.
fn binding_port(local: &LocalBinding<'_>) -> u16 {
    match local.port {
        LocalPort::Requested(port) | LocalPort::ChosenByKernel(port) => port.0,
        LocalPort::ChosenByExitNode(_) => u16::MIN,
    }
}

/// Refuses a socket made from `local`, which can only be unbound since no bind succeeds.
fn refused<H: Host>(
    policy: &TestPolicy<H>,
    local: TcpLocalEnd<'_, Infallible>,
    detail: impl FnOnce(&LocalBinding<'_>) -> u16,
) -> TcpLocalEndError<Infallible> {
    match local {
        TcpLocalEnd::Bound(bound_socket) => match bound_socket {},
        TcpLocalEnd::Unbound { local, .. } => TcpLocalEndError::Unbound(
            policy
                .refuse(u64::from(detail(&local)))
                .err()
                .unwrap_or(HostError::NotSupported),
        ),
    }
}

impl<H: Host> TcpConnections for TestPolicy<H> {
    type Connection = Infallible;
    type BoundSocket = Infallible;

    fn bind(
        &self,
        _protocol: TcpProtocol,
        _owner: UserId,
        local: LocalBinding<'_>,
        _settings: &[BindingSetting<'_>],
    ) -> Result<TcpBound<Self::BoundSocket>, HostError> {
        self.refuse(u64::from(binding_port(&local)))?;
        Err(HostError::NotSupported)
    }

    fn connect(
        &self,
        local: TcpLocalEnd<'_, Self::BoundSocket>,
        peer: SocketAddr,
        _settings: &[TcpSetting<'_>],
        _fast_open: TcpFastOpen<'_>,
    ) -> Result<TcpConnected<Self::Connection>, TcpLocalEndError<Self::BoundSocket>> {
        Err(refused(self, local, |_| peer.port()))
    }

    fn send(
        &self,
        connection: &Self::Connection,
        _request: TcpSendRequest,
        _payload: &impl VolatileBytes,
    ) -> Result<ByteCount, HostError> {
        match *connection {}
    }

    fn receive(
        &self,
        connection: &Self::Connection,
        _request: TcpReceiveRequest,
        _into: &impl VolatileBytes,
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

    fn disconnect(&self, connection: Self::Connection) -> TcpDisconnected<Self::BoundSocket> {
        match connection {}
    }

    fn release(&self, bound_socket: Self::BoundSocket) {
        match bound_socket {}
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
        local: TcpLocalEnd<'_, Self::BoundSocket>,
        _backlog: ListenBacklog,
        _settings: &[TcpListenerSetting<'_>],
    ) -> Result<TcpListening<Self::Listener>, TcpLocalEndError<Self::BoundSocket>> {
        Err(refused(self, local, binding_port))
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
    ) -> Result<TcpListenerReading<'a>, HostError> {
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
    ) -> Result<ReadinessReport, HostError> {
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
