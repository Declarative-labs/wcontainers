//! The policy's UDP: refused, as every other request to the outside.

use core::convert::Infallible;

use core::net::SocketAddr;

use wcontainers_abi::{Host, HostError};
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BufferSizes, ByteCount, IpHeaderFields, IpSendFields, IpVersion, LocalBinding, ProcessorIndex,
    RouteSelectors, SendError,
};
use wcontainers_kernel::UserId;
use wcontainers_udp_sockets::{
    UdpDatagram, UdpLocalEnd, UdpProtocol, UdpReceived, UdpSocketSetting, UdpSockets,
};

use crate::TestPolicy;

impl<H: Host> UdpSockets for TestPolicy<H> {
    type Socket = Infallible;

    /// Refuses the socket, with no number concerned, as it is opened only to reach outside.
    fn open(
        &self,
        _version: IpVersion,
        _protocol: UdpProtocol,
        _owner: UserId,
    ) -> Result<Self::Socket, HostError> {
        self.refuse(u64::MIN)?;
        Err(HostError::NotSupported)
    }

    fn bind(
        &self,
        socket: &Self::Socket,
        _local: LocalBinding<'_>,
    ) -> Result<SocketAddr, HostError> {
        match *socket {}
    }

    fn connect(
        &self,
        socket: &Self::Socket,
        _route: RouteSelectors<'_>,
    ) -> Result<SocketAddr, HostError> {
        match *socket {}
    }

    fn disconnect(&self, socket: &Self::Socket) -> Result<UdpLocalEnd, HostError> {
        match *socket {}
    }

    fn send_to(
        &self,
        socket: &Self::Socket,
        _header: IpHeaderFields<'_>,
        _fields: IpSendFields,
        _datagram: UdpDatagram<'_>,
    ) -> Result<(), SendError> {
        match *socket {}
    }

    fn receive(&self, socket: &Self::Socket, _into: &mut [u8]) -> Result<UdpReceived, HostError> {
        match *socket {}
    }

    fn count_unsent_bytes(&self, socket: &Self::Socket) -> Result<ByteCount, HostError> {
        match *socket {}
    }

    fn read_buffer_sizes(&self, socket: &Self::Socket) -> Result<BufferSizes, HostError> {
        match *socket {}
    }

    fn read_incoming_processor(
        &self,
        socket: &Self::Socket,
    ) -> Result<Option<ProcessorIndex>, HostError> {
        match *socket {}
    }

    fn close(&self, socket: Self::Socket) {
        match socket {}
    }
}

impl<'a, H: Host> Configurable<Infallible, UdpSocketSetting<'a>> for TestPolicy<H> {
    fn configure(
        &self,
        socket: &Infallible,
        _setting: UdpSocketSetting<'a>,
    ) -> Result<(), HostError> {
        match *socket {}
    }
}
