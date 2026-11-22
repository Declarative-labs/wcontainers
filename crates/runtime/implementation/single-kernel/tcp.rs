//! The local system's TCP: connections and listeners are the host's sockets, made and waited on
//! by the network; what Linux's sockets offer beyond moving bytes is not supported.

use std::net::{Ipv4Addr, Shutdown};

use core::net::SocketAddr;
use wcontainers_internet_protocol::{
    DropCount, InterfaceIndex, PacketMark, PacketPriority, ReceiveTimestamps, ReceivedPacketFields,
};
use wcontainers_tcp_sockets::TcpConnectStart;

use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{ByteCount, LocalBinding, LocalPort};
use wcontainers_kernel::UserId;
use wcontainers_negative_route_advice::NegativeRouteAdvice;
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

use crate::LocalSystem;

/// A TCP connection or listener of the local system, by the network's number of it.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NetworkHandle(u32);

/// The unspecified address of a peer's family, for a local address the host chooses.
fn unspecified(peer: SocketAddr) -> SocketAddr {
    match peer {
        SocketAddr::V4(_) => SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0),
        SocketAddr::V6(_) => SocketAddr::new(std::net::Ipv6Addr::UNSPECIFIED.into(), 0),
    }
}

impl TcpConnections for LocalSystem {
    type Connection = NetworkHandle;

    fn connect(
        &self,
        _protocol: TcpProtocol,
        _owner: UserId,
        _local: LocalBinding<'_>,
        peer: SocketAddr,
        _settings: &[TcpSetting<'_>],
        _fast_open: TcpFastOpen<'_>,
    ) -> Result<TcpConnected<Self::Connection>, HostError> {
        let connection = self.state().network().tcp_connect(peer)?;
        Ok(TcpConnected {
            connection: NetworkHandle(connection),
            local: unspecified(peer),
            start: TcpConnectStart::SynchronizeSent {
                fast_open_data_taken: ByteCount(0),
            },
        })
    }

    fn send(
        &self,
        connection: &Self::Connection,
        _request: TcpSendRequest,
        bytes: &[u8],
    ) -> Result<ByteCount, HostError> {
        self.state()
            .network()
            .tcp_send(connection.0, bytes)
            .map(ByteCount)
    }

    fn receive(
        &self,
        connection: &Self::Connection,
        request: TcpReceiveRequest,
        into: &mut [u8],
    ) -> Result<TcpReceived, HostError> {
        let peek = match request {
            TcpReceiveRequest::Read => false,
            TcpReceiveRequest::Peek {
                offset: ByteCount(0),
            } => true,
            _ => return Err(HostError::NotSupported),
        };
        let length = self
            .state()
            .network()
            .tcp_receive(connection.0, into, peek)?;
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
        self.state().network().tcp_shut_down(connection.0, how)
    }

    fn close(&self, connection: Self::Connection, _manner: TcpClose) {
        self.state().network().tcp_close(connection.0);
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

impl TcpListeners for LocalSystem {
    type Listener = NetworkHandle;

    fn listen(
        &self,
        _protocol: TcpProtocol,
        _owner: UserId,
        local: LocalBinding<'_>,
        _backlog: ListenBacklog,
        _settings: &[TcpListenerSetting<'_>],
    ) -> Result<TcpListening<Self::Listener>, HostError> {
        let LocalPort::Requested(port) = local.port else {
            return Err(HostError::NotSupported);
        };
        let listener = self.state().network().tcp_listen(port.0)?;
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
        let connection = self.state().network().tcp_accept(listener.0)?;
        let unknown = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        Ok(TcpAccepted {
            connection: NetworkHandle(connection),
            peer: unknown,
            local: unknown,
            synchronize: ReceivedPacketFields {
                length: 0,
                interface: InterfaceIndex(0),
                specific_destination: None,
                largest_fragment: None,
                link_layer_length: None,
                timestamps: ReceiveTimestamps {
                    software: None,
                    hardware: None,
                },
                mark: PacketMark(0),
                priority: PacketPriority(0),
                drops_since_previous: DropCount(0),
            },
            synchronize_header: &synchronize_header[..0],
        })
    }

    fn read_listener_state<'a>(
        &self,
        _listener: &Self::Listener,
        _query: TcpListenerQuery<'a>,
    ) -> Result<TcpReading<'a>, HostError> {
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
        self.state().network().tcp_close(listener.0);
    }
}

impl Readiness<NetworkHandle> for LocalSystem {
    async fn wait_until_ready(
        &self,
        connection: &NetworkHandle,
        interest: ReadyFor,
    ) -> Result<ReadyFor, HostError> {
        std::future::poll_fn(|context| {
            self.state()
                .network()
                .poll_ready(connection.0, interest, context.waker())
        })
        .await
    }
}

impl<'a> Configurable<NetworkHandle, TcpSetting<'a>> for LocalSystem {
    fn configure(
        &self,
        _connection: &NetworkHandle,
        _setting: TcpSetting<'a>,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}

impl<'a> Configurable<NetworkHandle, TcpListenerSetting<'a>> for LocalSystem {
    fn configure(
        &self,
        _listener: &NetworkHandle,
        _setting: TcpListenerSetting<'a>,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}

impl NegativeRouteAdvice<NetworkHandle> for LocalSystem {
    fn give_negative_route_advice(&self, _connection: &NetworkHandle) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}
