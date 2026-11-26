//! The local system's TCP on the host's sockets, made and waited on by the network.

use core::convert::Infallible;
use core::num::NonZeroU32;
use std::net::{Ipv4Addr, Shutdown};

use core::net::SocketAddr;
use wcontainers_internet_protocol::{
    DropCount, InterfaceIndex, PacketMark, PacketPriority, ReceiveTimestamps, ReceivedPacketFields,
};
use wcontainers_tcp_sockets::TcpConnectStart;

use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{BindingSetting, ByteCount, LocalBinding, LocalPort};
use wcontainers_kernel::UserId;
use wcontainers_kernel::errno::Errno;
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

use wcontainers_volatile_bytes::VolatileBytes;

use crate::LocalSystem;
use crate::host_call::{receive_into, send_from};

/// A TCP connection or listener, or a UDP socket, of the local system, by the network's number of it.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NetworkHandle(pub(crate) u32);

/// Gives the unspecified address of a peer's family, for a local address the host chooses.
fn unspecified(peer: SocketAddr) -> SocketAddr {
    match peer {
        SocketAddr::V4(_) => SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), u16::MIN),
        SocketAddr::V6(_) => SocketAddr::new(std::net::Ipv6Addr::UNSPECIFIED.into(), u16::MIN),
    }
}

impl<E> TcpConnections for LocalSystem<E> {
    type Connection = NetworkHandle;
    type BoundSocket = Infallible;

    /// Refuses: the network binds a socket only as it connects or listens.
    fn bind(
        &self,
        _protocol: TcpProtocol,
        _owner: UserId,
        _local: LocalBinding<'_>,
        _settings: &[BindingSetting<'_>],
    ) -> Result<TcpBound<Self::BoundSocket>, HostError> {
        Err(HostError::NotSupported)
    }

    fn connect(
        &self,
        local: TcpLocalEnd<'_, Self::BoundSocket>,
        peer: SocketAddr,
        _settings: &[TcpSetting<'_>],
        _fast_open: TcpFastOpen<'_>,
    ) -> Result<TcpConnected<Self::Connection>, TcpLocalEndError<Self::BoundSocket>> {
        if let TcpLocalEnd::Bound(bound_socket) = local {
            match bound_socket {}
        }
        let connection = self
            .state()
            .network()
            .connect_tcp(peer)
            .map_err(TcpLocalEndError::Unbound)?;
        Ok(TcpConnected {
            connection: NetworkHandle(connection),
            local: unspecified(peer),
            start: TcpConnectStart::SynchronizeSent {
                fast_open_data_taken: ByteCount(usize::MIN),
            },
        })
    }

    fn send(
        &self,
        connection: &Self::Connection,
        _request: TcpSendRequest,
        payload: &impl VolatileBytes,
    ) -> Result<ByteCount, HostError> {
        send_from(payload, |bytes| {
            self.state().network().send_tcp(connection.0, bytes)
        })
        .map(ByteCount)
    }

    fn receive(
        &self,
        connection: &Self::Connection,
        request: TcpReceiveRequest,
        into: &impl VolatileBytes,
    ) -> Result<TcpReceived, HostError> {
        let peek = match request {
            TcpReceiveRequest::Read => false,
            TcpReceiveRequest::Peek {
                offset: ByteCount(0),
            } => true,
            _ => return Err(HostError::NotSupported),
        };
        let length = receive_into(into, |bytes| {
            self.state()
                .network()
                .receive_tcp(connection.0, bytes, peek)
        })?;
        Ok(TcpReceived {
            length: ByteCount(length),
            timestamps: ReceiveTimestamps {
                software: None,
                hardware: None,
            },
            unread_after: None,
        })
    }

    fn take_error_queue_entry(
        &self,
        _connection: &Self::Connection,
        _into: &mut [u8],
    ) -> Result<TcpErrorQueueEntry, HostError> {
        Err(HostError::NotSupported)
    }

    fn read_state<'a>(
        &self,
        _connection: &Self::Connection,
        _query: TcpQuery<'a>,
    ) -> Result<TcpReading<'a>, HostError> {
        Err(HostError::NotSupported)
    }

    fn shut_down(
        &self,
        connection: &Self::Connection,
        directions: TcpShutdown,
    ) -> Result<(), HostError> {
        let how = match directions {
            TcpShutdown::Read => Shutdown::Read,
            TcpShutdown::Write => Shutdown::Write,
            TcpShutdown::ReadAndWrite => Shutdown::Both,
        };
        self.state().network().shut_down_tcp(connection.0, how)
    }

    fn close(&self, connection: Self::Connection, _manner: TcpClose) {
        self.state().network().close_tcp(connection.0);
    }

    /// Closes the connection, whose port no bind of its own holds, leaving the reset Linux leaves pending.
    fn disconnect(&self, connection: Self::Connection) -> TcpDisconnected<Self::BoundSocket> {
        self.state().network().close_tcp(connection.0);
        TcpDisconnected {
            bound_socket: None,
            pending_error: Some(Errno::ECONNRESET),
        }
    }

    fn release(&self, bound_socket: Self::BoundSocket) {
        match bound_socket {}
    }

    fn push_queued_data(
        &self,
        _connection: &Self::Connection,
        _ignoring: TcpPushOverride,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }

    fn send_delayed_acknowledgement(
        &self,
        _connection: &Self::Connection,
        _afterwards: TcpQuickAcknowledgement,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }

    fn rearm_keep_alive_timer(&self, _connection: &Self::Connection) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }

    fn discard_error_queue_entries(&self, _connection: &Self::Connection) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }

    fn send_window_probe(&self, _connection: &Self::Connection) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }

    fn set_timestamp_clock(
        &self,
        _connection: &Self::Connection,
        _reading: TcpTimestampClock,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }

    fn set_authentication_counters(
        &self,
        _connection: &Self::Connection,
        _values: TcpAuthenticationCounters,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}

impl<E> TcpListeners for LocalSystem<E> {
    type Listener = NetworkHandle;

    fn listen(
        &self,
        local: TcpLocalEnd<'_, Self::BoundSocket>,
        _backlog: ListenBacklog,
        _settings: &[TcpListenerSetting<'_>],
    ) -> Result<TcpListening<Self::Listener>, TcpLocalEndError<Self::BoundSocket>> {
        let local = match local {
            TcpLocalEnd::Bound(bound_socket) => match bound_socket {},
            TcpLocalEnd::Unbound { local, .. } => local,
        };
        let LocalPort::Requested(port) = local.port else {
            return Err(TcpLocalEndError::Unbound(HostError::NotSupported));
        };
        let listener = self
            .state()
            .network()
            .listen_tcp(port.0)
            .map_err(TcpLocalEndError::Unbound)?;
        Ok(TcpListening {
            listener: NetworkHandle(listener),
            local: SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port.0),
        })
    }

    fn accept<'a>(
        &self,
        listener: &Self::Listener,
        synchronize_header: &'a mut [u8],
    ) -> Result<TcpAccepted<'a, Self::Connection>, HostError> {
        let connection = self.state().network().accept_tcp(listener.0)?;
        let unknown = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), u16::MIN);
        Ok(TcpAccepted {
            connection: NetworkHandle(connection),
            peer: unknown,
            local: unknown,
            synchronize: ReceivedPacketFields {
                length: usize::MIN,
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
            synchronize_header: &synchronize_header[..0],
        })
    }

    fn read_listener_state<'a>(
        &self,
        _listener: &Self::Listener,
        _query: TcpListenerQuery<'a>,
    ) -> Result<TcpListenerReading<'a>, HostError> {
        Err(HostError::NotSupported)
    }

    fn set_listener_authentication_counters(
        &self,
        _listener: &Self::Listener,
        _values: TcpAuthenticationCounters,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }

    fn set_listener_timestamp_clock(
        &self,
        _listener: &Self::Listener,
        _reading: TcpTimestampClock,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }

    fn stop_listening(&self, listener: Self::Listener) {
        self.state().network().close_tcp(listener.0);
    }
}

impl<E> Readiness<NetworkHandle> for LocalSystem<E> {
    async fn wait_until_ready(
        &self,
        connection: &NetworkHandle,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        std::future::poll_fn(|context| {
            self.state()
                .network()
                .poll_ready(connection.0, interest, context.waker())
        })
        .await
    }
}

impl<'a, E> Configurable<NetworkHandle, TcpSetting<'a>> for LocalSystem<E> {
    fn configure(
        &self,
        _connection: &NetworkHandle,
        _setting: TcpSetting<'a>,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}

impl<'a, E> Configurable<NetworkHandle, TcpListenerSetting<'a>> for LocalSystem<E> {
    fn configure(
        &self,
        _listener: &NetworkHandle,
        _setting: TcpListenerSetting<'a>,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}

impl<E> NegativeRouteAdvisable<NetworkHandle> for LocalSystem<E> {
    fn give_negative_route_advice(&self, _connection: &NetworkHandle) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}
