//! Implements UDP for the test host on this machine's sockets, each call kept in the record.

use std::sync::atomic::Ordering;

use core::net::SocketAddr;
use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BufferSizes, ByteCount, IpHeaderFields, IpSendFields, IpVersion, LocalBinding, ProcessorIndex,
    RouteSelectors, SendError, UpperLayerSelector,
};
use wcontainers_kernel::UserId;
use wcontainers_negative_route_advisable::NegativeRouteAdvisable;
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor};
use wcontainers_runtime_single_kernel::udp::received;
use wcontainers_udp_sockets::{
    UdpDatagram, UdpLocalEnd, UdpProtocol, UdpReceived, UdpSocketSetting, UdpSockets,
};

use super::tcp::Readied;
use super::{TestHost, lock};
use crate::replay::Entry;

/// A UDP socket of the test host, by its number.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UdpHandle(u32);

/// Gives the destination address and port a route selects.
fn destination(route: &RouteSelectors<'_>) -> Result<SocketAddr, HostError> {
    let (address, upper_layer) = match route {
        RouteSelectors::V4(route) => (route.destination.into(), route.upper_layer),
        RouteSelectors::V6(route) => (route.destination.into(), route.upper_layer),
    };
    match upper_layer.selector {
        Some(UpperLayerSelector::Ports(ports)) => Ok(SocketAddr::new(address, ports.destination.0)),
        _ => Err(HostError::NotSupported),
    }
}

impl<H: Send> UdpSockets for TestHost<H> {
    type Socket = UdpHandle;

    fn open(
        &self,
        _version: IpVersion,
        protocol: UdpProtocol,
        _owner: UserId,
    ) -> Result<Self::Socket, HostError> {
        if protocol != UdpProtocol::Udp {
            return Err(HostError::NotSupported);
        }
        Ok(UdpHandle(
            self.last_socket.fetch_add(1, Ordering::Relaxed) + 1,
        ))
    }

    fn bind(
        &self,
        _socket: &Self::Socket,
        _local: LocalBinding<'_>,
    ) -> Result<SocketAddr, HostError> {
        Err(HostError::NotSupported)
    }

    fn connect(
        &self,
        socket: &Self::Socket,
        route: RouteSelectors<'_>,
    ) -> Result<SocketAddr, HostError> {
        let to = destination(&route)?;
        self.answer(
            Entry::Connected,
            |entry| match entry {
                Entry::Connected(result) => Ok(result),
                other => Err(other),
            },
            || lock(&self.network).udp_connect(socket.0, to),
        )?;
        Ok(SocketAddr::new(to.ip(), u16::MIN))
    }

    fn disconnect(&self, _socket: &Self::Socket) -> Result<UdpLocalEnd, HostError> {
        Err(HostError::NotSupported)
    }

    fn send_to(
        &self,
        socket: &Self::Socket,
        _header: IpHeaderFields<'_>,
        _fields: IpSendFields,
        datagram: UdpDatagram<'_>,
    ) -> Result<(), SendError> {
        self.answer(
            Entry::Sent,
            |entry| match entry {
                Entry::Sent(result) => Ok(result),
                other => Err(other),
            },
            || lock(&self.network).udp_send(socket.0, datagram.payload),
        )
        .map(|_| ())
        .map_err(SendError::Host)
    }

    fn receive(&self, socket: &Self::Socket, into: &mut [u8]) -> Result<UdpReceived, HostError> {
        let (result, bytes) = self.answer(
            |(result, bytes)| Entry::Received(result, bytes),
            |entry| match entry {
                Entry::Received(result, bytes) if bytes.len() <= into.len() => Ok((result, bytes)),
                other => Err(other),
            },
            || {
                let mut bytes = vec![u8::MIN; into.len()];
                let result = lock(&self.network).udp_receive(socket.0, &mut bytes);
                bytes.truncate(*result.as_ref().unwrap_or(&usize::MIN));
                (result, bytes)
            },
        );
        into[..bytes.len()].copy_from_slice(&bytes);
        result.map(received)
    }

    fn count_unsent_bytes(&self, _socket: &Self::Socket) -> Result<ByteCount, HostError> {
        Err(HostError::NotSupported)
    }

    fn read_buffer_sizes(&self, _socket: &Self::Socket) -> Result<BufferSizes, HostError> {
        Err(HostError::NotSupported)
    }

    fn read_incoming_processor(
        &self,
        _socket: &Self::Socket,
    ) -> Result<Option<ProcessorIndex>, HostError> {
        Err(HostError::NotSupported)
    }

    fn close(&self, socket: Self::Socket) {
        if !self.replaying() {
            lock(&self.network).udp_close(socket.0);
        }
    }
}

impl<H: Send> Readiness<UdpHandle> for TestHost<H> {
    async fn wait_until_ready(
        &self,
        socket: &UdpHandle,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        if !self.replaying() {
            lock(&self.network).arm(socket.0, interest);
        }
        Ok(Readied::wait(&self.ready, socket.0).await)
    }
}

impl<'a, H: Send> Configurable<UdpHandle, UdpSocketSetting<'a>> for TestHost<H> {
    fn configure(
        &self,
        _socket: &UdpHandle,
        _setting: UdpSocketSetting<'a>,
    ) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}

impl<H: Send> NegativeRouteAdvisable<UdpHandle> for TestHost<H> {
    fn give_negative_route_advice(&self, _socket: &UdpHandle) -> Result<(), HostError> {
        Err(HostError::NotSupported)
    }
}
