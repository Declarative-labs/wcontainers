use wcontainers_abi::{CounterReading, HostError};
use wcontainers_configurable::Configurable;
use wcontainers_icmp_sockets::{EchoBinding, EchoIdentifier, IcmpSockets};
use wcontainers_internet_protocol::{
    BufferSizes, ByteCount, IpHeaderFields, IpSendFields, IpSocketSetting, IpVersion, Received,
    SendError,
};
use wcontainers_internet_protocol_linux_socket_options::Shortfalls;
use wcontainers_kernel::UserId;
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor};

use crate::{HostIcmpSocket, HostIcmpSockets};

/// Why a socket is refused on a system without Linux's ping sockets.
const NO_PING_SOCKETS: &str = "icmp sockets: this system has no Linux ping sockets to open";

impl HostIcmpSockets {
    /// Makes the sockets, which on this system refuse every socket.
    pub fn new() -> Result<Self, HostError> {
        Ok(Self {
            shortfalls: Shortfalls::default(),
        })
    }

    /// Returns how many times each named reason has made this value refuse something.
    pub fn counters(&self) -> Vec<CounterReading> {
        self.shortfalls.readings()
    }
}

impl IcmpSockets for HostIcmpSockets {
    type Socket = HostIcmpSocket;

    fn open(&self, _version: IpVersion, _owner: UserId) -> Result<Self::Socket, HostError> {
        Err(self.shortfalls.refuse(NO_PING_SOCKETS))
    }

    fn bind(
        &self,
        socket: &Self::Socket,
        _local: EchoBinding,
    ) -> Result<EchoIdentifier, HostError> {
        match socket.never {}
    }

    fn send_to(
        &self,
        socket: &Self::Socket,
        _header: IpHeaderFields<'_>,
        _fields: IpSendFields,
        _message: &[u8],
    ) -> Result<(), SendError> {
        match socket.never {}
    }

    fn receive(&self, socket: &Self::Socket, _into: &mut [u8]) -> Result<Received, HostError> {
        match socket.never {}
    }

    fn count_unsent_bytes(&self, socket: &Self::Socket) -> Result<ByteCount, HostError> {
        match socket.never {}
    }

    fn read_buffer_sizes(&self, socket: &Self::Socket) -> Result<BufferSizes, HostError> {
        match socket.never {}
    }

    fn close(&self, socket: Self::Socket) {
        match socket.never {}
    }
}

impl Readiness<HostIcmpSocket> for HostIcmpSockets {
    async fn wait_until_ready(
        &self,
        socket: &HostIcmpSocket,
        _interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        match socket.never {}
    }
}

impl<'a> Configurable<HostIcmpSocket, IpSocketSetting<'a>> for HostIcmpSockets {
    fn configure(
        &self,
        socket: &HostIcmpSocket,
        _setting: IpSocketSetting<'a>,
    ) -> Result<(), HostError> {
        match socket.never {}
    }
}
