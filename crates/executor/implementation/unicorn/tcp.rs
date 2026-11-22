//! The test host's TCP on this machine's sockets, each call and readiness kept in the record.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::task::{Poll, Waker};

use core::net::SocketAddr;
use wcontainers_internet_protocol::{
    DropCount, InterfaceIndex, PacketMark, PacketPriority, ReceiveTimestamps, ReceivedPacketFields,
};
use wcontainers_tcp_sockets::TcpConnectStart;

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

use super::{TestHost, lock};
use crate::replay::Entry;

/// A TCP connection or listener of the test host, by its number.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TcpHandle(u32);

/// The readiness each TCP connection or listener reported and the waker of each wait, by number.
#[derive(Default)]
pub(super) struct Readied {
    pub(super) arrived: BTreeMap<u32, ReadyFor>,
    pub(super) waiting: BTreeMap<u32, Waker>,
}

impl Readied {
    /// What `number` became ready for, ending its wait.
    pub(super) fn arrive(readied: &Mutex<Readied>, number: u32, ready: ReadyFor) {
        let waiting = {
            let mut readied = lock(readied);
            *readied.arrived.entry(number).or_insert(ready) |= ready;
            readied.waiting.remove(&number)
        };
        if let Some(waker) = waiting {
            waker.wake();
        }
    }

    /// Waits until the network says what `number` is ready for, forgetting it if let go.
    pub(super) async fn wait(readied: &Mutex<Readied>, number: u32) -> ReadyFor {
        /// Forgets the readiness waited for, when the wait is let go.
        struct Forget<'a>(&'a Mutex<Readied>, u32);
        impl Drop for Forget<'_> {
            fn drop(&mut self) {
                let mut readied = lock(self.0);
                readied.arrived.remove(&self.1);
                readied.waiting.remove(&self.1);
            }
        }
        let _forget = Forget(readied, number);
        std::future::poll_fn(|context| {
            let mut readied = lock(readied);
            match readied.arrived.remove(&number) {
                Some(ready) => Poll::Ready(ready),
                None => {
                    readied.waiting.insert(number, context.waker().clone());
                    Poll::Pending
                }
            }
        })
        .await
    }
}

/// Gives the unspecified address of a peer's family, for a local address the machine chooses.
fn unspecified(peer: SocketAddr) -> SocketAddr {
    match peer {
        SocketAddr::V4(_) => SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0),
        SocketAddr::V6(_) => SocketAddr::new(std::net::Ipv6Addr::UNSPECIFIED.into(), 0),
    }
}

impl<H: Send> TcpConnections for TestHost<H> {
    type Connection = TcpHandle;

    fn connect(
        &self,
        _protocol: TcpProtocol,
        _owner: UserId,
        _local: LocalBinding<'_>,
        peer: SocketAddr,
        _settings: &[TcpSetting<'_>],
        _fast_open: TcpFastOpen<'_>,
    ) -> Result<TcpConnected<Self::Connection>, HostError> {
        let number = self.last_socket.fetch_add(1, Ordering::Relaxed) + 1;
        let number = self.answer(
            Entry::SocketMade,
            |entry| match entry {
                Entry::SocketMade(result) => Ok(result),
                other => Err(other),
            },
            || {
                lock(&self.network).tcp_connect(number, peer);
                Ok(number)
            },
        )?;
        Ok(TcpConnected {
            connection: TcpHandle(number),
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
        self.answer(
            Entry::Sent,
            |entry| match entry {
                Entry::Sent(result) => Ok(result),
                other => Err(other),
            },
            || lock(&self.network).tcp_send(connection.0, bytes),
        )
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
        let (result, bytes) = self.answer(
            |(result, bytes)| Entry::Received(result, bytes),
            |entry| match entry {
                Entry::Received(result, bytes) if bytes.len() <= into.len() => Ok((result, bytes)),
                other => Err(other),
            },
            || {
                let mut bytes = vec![0; into.len()];
                let result = lock(&self.network).tcp_receive(connection.0, &mut bytes, peek);
                bytes.truncate(*result.as_ref().unwrap_or(&0));
                (result, bytes)
            },
        );
        into[..bytes.len()].copy_from_slice(&bytes);
        Ok(TcpReceived {
            length: ByteCount(result?),
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
        if directions != TcpShutdown::Read && !self.replaying() {
            lock(&self.network).tcp_finish(connection.0);
        }
        Ok(())
    }

    fn close(&self, connection: Self::Connection, _manner: TcpClose) {
        if !self.replaying() {
            lock(&self.network).tcp_close(connection.0);
        }
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

impl<H: Send> TcpListeners for TestHost<H> {
    type Listener = TcpHandle;

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
        let number = self.last_socket.fetch_add(1, Ordering::Relaxed) + 1;
        let number = self.answer(
            Entry::SocketMade,
            |entry| match entry {
                Entry::SocketMade(result) => Ok(result),
                other => Err(other),
            },
            || lock(&self.network).listen(number, port.0).map(|()| number),
        )?;
        Ok(TcpListening {
            listener: TcpHandle(number),
            local: SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port.0),
        })
    }

    fn accept<'a>(
        &self,
        listener: &Self::Listener,
        synchronize_header: &'a mut [u8],
    ) -> Result<TcpAccepted<'a, Self::Connection>, HostError> {
        let number = self.last_socket.fetch_add(1, Ordering::Relaxed) + 1;
        let number = self.answer(
            Entry::SocketMade,
            |entry| match entry {
                Entry::SocketMade(result) => Ok(result),
                other => Err(other),
            },
            || {
                lock(&self.network)
                    .accept(listener.0, number)
                    .map(|()| number)
            },
        )?;
        let unknown = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
        Ok(TcpAccepted {
            connection: TcpHandle(number),
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
        if !self.replaying() {
            lock(&self.network).tcp_close(listener.0);
        }
    }
}

impl<H: Send> Readiness<TcpHandle> for TestHost<H> {
    async fn wait_until_ready(
        &self,
        connection: &TcpHandle,
        interest: ReadyFor,
    ) -> Result<ReadyFor, HostError> {
        if !self.replaying() {
            lock(&self.network).arm(connection.0, interest);
        }
        Ok(Readied::wait(&self.ready, connection.0).await)
    }
}

impl<'a, H: Send> Configurable<TcpHandle, TcpSetting<'a>> for TestHost<H> {
    fn configure(
        &self,
        _connection: &TcpHandle,
        _setting: TcpSetting<'a>,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}

impl<'a, H: Send> Configurable<TcpHandle, TcpListenerSetting<'a>> for TestHost<H> {
    fn configure(
        &self,
        _listener: &TcpHandle,
        _setting: TcpListenerSetting<'a>,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}

impl<H: Send> NegativeRouteAdvisable<TcpHandle> for TestHost<H> {
    fn give_negative_route_advice(&self, _connection: &TcpHandle) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}
