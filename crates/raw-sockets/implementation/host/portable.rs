use etherparse::IpNumber;
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BufferSizes, ByteCount, IpSendFields, IpVersion, Received, SendError,
};
use wcontainers_internet_protocol_linux_socket_options::Shortfalls;
use wcontainers_kernel::UserId;
use wcontainers_raw_sockets::{RawPacket, RawSocketSetting, RawSockets};
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor};

use crate::{HostRawSocket, HostRawSockets};

/// Why a socket is refused on a system without Linux's raw sockets.
const NO_RAW_SOCKETS: &str = "raw sockets: this system has no Linux raw IP sockets to open";

impl HostRawSockets {
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

impl RawSockets for HostRawSockets {
    type Socket = HostRawSocket;

    fn open(
        &self,
        _version: IpVersion,
        _protocol: IpNumber,
        _owner: UserId,
    ) -> Result<Self::Socket, HostError> {
        Err(self.shortfalls.refuse(NO_RAW_SOCKETS))
    }

    fn send_to(
        &self,
        socket: &Self::Socket,
        _packet: RawPacket<'_>,
        _fields: IpSendFields,
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

impl Readiness<HostRawSocket> for HostRawSockets {
    async fn wait_until_ready(
        &self,
        socket: &HostRawSocket,
        _interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        match socket.never {}
    }
}

impl<'a> Configurable<HostRawSocket, RawSocketSetting<'a>> for HostRawSockets {
    fn configure(
        &self,
        socket: &HostRawSocket,
        _setting: RawSocketSetting<'a>,
    ) -> Result<(), HostError> {
        match socket.never {}
    }
}
