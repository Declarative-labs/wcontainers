//! UdpSockets on another system's UDP sockets: what socket2 covers, and a counted refusal for what only Linux gives.

use core::cmp::Ordering;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd};

use etherparse::{IpDscp, IpEcn, IpNumber, Ipv6FlowLabel};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::AddressSharing;
use wcontainers_internet_protocol::{
    BindAddress, BroadcastPermission, ByteCount, IpHeaderFields, IpSendFields, IpSocketSetting,
    IpVersion, Ipv4RouteSelectors, Ipv6Families, Ipv6RouteSelectors, LocalBinding, LocalPort,
    MobileAddressPreference, NonlocalAddresses, PacketMark, PortSetting, PortSharing,
    ProcessorIndex, Report, RouteScope, RouteSelectors, SendError, SourcePreferences,
    TemporaryAddressPreference, Timestamping, UpperLayer, UpperLayerSelector,
};
use wcontainers_internet_protocol_linux_socket_options as shared;
use wcontainers_kernel::UserId;
use wcontainers_kernel::errno::Errno;
use wcontainers_kernel::signal_information::PollEvents;
use wcontainers_negative_route_advice::NegativeRouteAdvice;
use wcontainers_udp_sockets::{
    ReceiveCoalescing, UdpDatagram, UdpLocalEnd, UdpProtocol, UdpReceived, UdpSocketSetting,
    UdpSockets, ZeroChecksumReceipt,
};

use shared::readiness::{Readiness, Watched};
use shared::{Shortfalls, io_host_error, sock};

/// UDP sockets of the system this runs on, with a count of each refusal by its reason.
pub struct HostUdpSockets {
    shortfalls: Shortfalls,
    readiness: Readiness,
}

pub struct HostUdpSocket {
    watched: Watched,
    ipv6: bool,
}

pub const UDP_LITE: &str = "a UDP-Lite socket, which only Linux has";
pub const OWNER_NOT_THE_PROCESS: &str =
    "a socket opened as a user other than the process's, which only Linux's filesystem user allows";
pub const PORT_CHOSEN_BY_EXIT_NODE: &str =
    "a port chosen by the exit node from its ranges, which only Linux's IP_LOCAL_PORT_RANGE binds";
pub const ROUTE_SELECTORS: &str =
    "a connect's route selectors beyond its destination and ports, which only the Linux build sets";
pub const SOURCE_PORT_NOT_THE_SOCKETS: &str =
    "a connect's source port is not the port its socket is bound to";
pub const DISCONNECT: &str = "a disconnect, the connect to AF_UNSPEC that socket2 does not make";
pub const SEND_HEADER: &str =
    "a send's IP header and send fields, which only the Linux build sets on a host socket";
pub const RECEIVED_HEADER: &str = "a received packet's IP header, which only the Linux build reads from the host's control messages";
pub const UNSENT_BYTES: &str = "the unsent byte count, which socket2 does not read";
pub const INCOMING_PROCESSOR: &str = "a socket's incoming processor, which only Linux keeps";
pub const NEGATIVE_ROUTE_ADVICE: &str =
    "negative route advice, which only the Linux build gives by connecting again";
pub const SETTING: &str = "a setting that only the Linux build makes";

impl HostUdpSockets {
    pub fn new() -> Result<Self, HostError> {
        Ok(Self {
            shortfalls: Shortfalls::default(),
            readiness: Readiness::new()?,
        })
    }

    /// How many times each named reason has made this value refuse something.
    pub fn counters(&self) -> Vec<CounterReading> {
        self.shortfalls.readings()
    }

    fn refuse(&self, reason: &'static str) -> HostError {
        self.shortfalls.refuse(reason)
    }
}

impl HostUdpSocket {
    fn fd(&self) -> BorrowedFd<'_> {
        self.watched.fd()
    }

    /// An address of either version as this socket's family holds it.
    fn for_family(&self, address: IpAddr) -> Result<IpAddr, HostError> {
        match (address, self.ipv6) {
            (IpAddr::V4(address), true) => Ok(IpAddr::V6(address.to_ipv6_mapped())),
            (IpAddr::V6(_), false) => Err(HostError::Errno(Errno::EAFNOSUPPORT as i32)),
            (address, _) => Ok(address),
        }
    }

    fn local(&self) -> Result<SocketAddr, HostError> {
        sock(&self.fd())
            .local_addr()
            .map_err(|error| io_host_error(&error))?
            .as_socket()
            .map(unmapped)
            .ok_or(HostError::Errno(Errno::EAFNOSUPPORT as i32))
    }
}

impl PartialEq for HostUdpSocket {
    fn eq(&self, other: &Self) -> bool {
        self.fd().as_raw_fd() == other.fd().as_raw_fd()
    }
}

impl Eq for HostUdpSocket {}

impl PartialOrd for HostUdpSocket {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HostUdpSocket {
    fn cmp(&self, other: &Self) -> Ordering {
        self.fd().as_raw_fd().cmp(&other.fd().as_raw_fd())
    }
}

/// An IPv4 address an IPv6 socket holds mapped, as the address it is.
fn unmapped(address: SocketAddr) -> SocketAddr {
    match address {
        SocketAddr::V6(v6) => match v6.ip().to_ipv4_mapped() {
            Some(v4) => SocketAddr::new(IpAddr::V4(v4), v6.port()),
            None => address,
        },
        SocketAddr::V4(_) => address,
    }
}

/// The source address preferences a socket has when none are asked for.
const SYSTEM_PREFERENCES: SourcePreferences = SourcePreferences {
    temporary: TemporaryAddressPreference::SystemDefault,
    mobile: MobileAddressPreference::Home,
};

/// Whether IPv4 route selectors ask for no more than their destination and ports.
fn ipv4_only_destination(route: &Ipv4RouteSelectors<'_>) -> bool {
    route.source.is_none()
        && route.interface.is_none()
        && route.dscp == IpDscp::ZERO
        && route.ecn == IpEcn::NotEct
        && route.mark == PacketMark(u32::MIN)
        && route.scope == RouteScope::Universe
        && route.broadcast == BroadcastPermission::Refused
        && route.options.is_empty()
}

/// Whether IPv6 route selectors ask for no more than their destination and ports.
fn ipv6_only_destination(route: &Ipv6RouteSelectors<'_>) -> bool {
    route.source.is_none()
        && route.interface.is_none()
        && route.dscp == IpDscp::ZERO
        && route.ecn == IpEcn::NotEct
        && route.flow_label == Ipv6FlowLabel::ZERO
        && route.mark == PacketMark(u32::MIN)
        && route.source_preferences == SYSTEM_PREFERENCES
        && route.routing_header.is_none()
}

/// The ports of a UDP upper layer.
fn ports(upper_layer: &UpperLayer) -> Result<(u16, u16), HostError> {
    match upper_layer.selector {
        Some(UpperLayerSelector::Ports(ports)) if upper_layer.protocol == IpNumber::UDP => {
            Ok((ports.source.0, ports.destination.0))
        }
        _ => Err(HostError::Errno(Errno::EINVAL as i32)),
    }
}

/// Timestamping that asks for nothing, which every socket starts with.
const NO_TIMESTAMPS: Timestamping = Timestamping {
    receive_software: Report::NotRequested,
    receive_hardware: Report::NotRequested,
    bound_clock: None,
};

impl UdpSockets for HostUdpSockets {
    type Socket = HostUdpSocket;

    fn open(
        &self,
        version: IpVersion,
        protocol: UdpProtocol,
        owner: UserId,
    ) -> Result<Self::Socket, HostError> {
        if protocol == UdpProtocol::Lite {
            return Err(self.refuse(UDP_LITE));
        }
        if owner.0 != rustix::process::getuid().as_raw() {
            return Err(self.refuse(OWNER_NOT_THE_PROCESS));
        }
        let (domain, ipv6) = match version {
            IpVersion::V4 => (Domain::IPV4, false),
            IpVersion::V6 => (Domain::IPV6, true),
        };
        let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))
            .map_err(|error| io_host_error(&error))?;
        socket
            .set_nonblocking(true)
            .map_err(|error| io_host_error(&error))?;
        Ok(HostUdpSocket {
            watched: self.readiness.watch(OwnedFd::from(socket))?,
            ipv6,
        })
    }

    fn bind(
        &self,
        socket: &Self::Socket,
        local: LocalBinding<'_>,
    ) -> Result<SocketAddr, HostError> {
        let fd = socket.fd();
        let host = sock(&fd);
        let address = match local.address {
            BindAddress::V4(address) => socket.for_family(IpAddr::V4(address))?,
            BindAddress::V6 { address, families } => {
                let address = socket.for_family(IpAddr::V6(address))?;
                host.set_only_v6(families == Ipv6Families::V6Only)
                    .map_err(|error| io_host_error(&error))?;
                address
            }
        };
        let port = match local.port {
            LocalPort::Requested(port) | LocalPort::ChosenByKernel(port) => port.0,
            LocalPort::ChosenByExitNode(_) => return Err(self.refuse(PORT_CHOSEN_BY_EXIT_NODE)),
        };
        host.bind(&SockAddr::from(SocketAddr::new(address, port)))
            .map_err(|error| io_host_error(&error))?;
        socket.local()
    }

    fn connect(
        &self,
        socket: &Self::Socket,
        route: RouteSelectors<'_>,
    ) -> Result<SocketAddr, HostError> {
        let (destination, upper_layer, only_destination) = match &route {
            RouteSelectors::V4(route) => (
                IpAddr::V4(route.destination),
                route.upper_layer,
                ipv4_only_destination(route),
            ),
            RouteSelectors::V6(route) => (
                IpAddr::V6(route.destination),
                route.upper_layer,
                ipv6_only_destination(route),
            ),
        };
        let (source_port, destination_port) = ports(&upper_layer)?;
        if !only_destination {
            return Err(self.refuse(ROUTE_SELECTORS));
        }
        let destination = socket.for_family(destination)?;
        let fd = socket.fd();
        let local = socket.local()?;
        if source_port != u16::MIN && source_port != local.port() {
            if local.port() != u16::MIN {
                return Err(self.refuse(SOURCE_PORT_NOT_THE_SOCKETS));
            }
            let any = if socket.ipv6 {
                IpAddr::V6(Ipv6Addr::UNSPECIFIED)
            } else {
                IpAddr::V4(Ipv4Addr::UNSPECIFIED)
            };
            sock(&fd)
                .bind(&SockAddr::from(SocketAddr::new(any, source_port)))
                .map_err(|error| io_host_error(&error))?;
        }
        sock(&fd)
            .connect(&SockAddr::from(SocketAddr::new(
                destination,
                destination_port,
            )))
            .map_err(|error| io_host_error(&error))?;
        socket.local()
    }

    fn disconnect(&self, _socket: &Self::Socket) -> Result<UdpLocalEnd, HostError> {
        Err(self.refuse(DISCONNECT))
    }

    fn send_to(
        &self,
        socket: &Self::Socket,
        header: IpHeaderFields<'_>,
        _fields: IpSendFields,
        _datagram: UdpDatagram<'_>,
    ) -> Result<(), SendError> {
        if matches!(header, IpHeaderFields::V6(_)) && !socket.ipv6 {
            return Err(SendError::Host(HostError::Errno(
                Errno::EAFNOSUPPORT as i32,
            )));
        }
        Err(SendError::Host(self.refuse(SEND_HEADER)))
    }

    fn receive(&self, _socket: &Self::Socket, _into: &mut [u8]) -> Result<UdpReceived, HostError> {
        Err(self.refuse(RECEIVED_HEADER))
    }

    fn count_unsent_bytes(&self, _socket: &Self::Socket) -> Result<ByteCount, HostError> {
        Err(self.refuse(UNSENT_BYTES))
    }

    fn read_incoming_processor(
        &self,
        _socket: &Self::Socket,
    ) -> Result<Option<ProcessorIndex>, HostError> {
        Err(self.refuse(INCOMING_PROCESSOR))
    }

    async fn wait_for_events(
        &self,
        socket: &Self::Socket,
        interest: PollEvents,
    ) -> Result<PollEvents, HostError> {
        socket.watched.wait_for_events(interest).await
    }

    fn close(&self, socket: Self::Socket) {
        drop(socket);
    }
}

impl<'a> Configurable<HostUdpSocket, UdpSocketSetting<'a>> for HostUdpSockets {
    fn configure(
        &self,
        socket: &HostUdpSocket,
        setting: UdpSocketSetting<'a>,
    ) -> Result<(), HostError> {
        let fd = socket.fd();
        let host = sock(&fd);
        let set = match setting {
            UdpSocketSetting::Ip(IpSocketSetting::SendBuffer(bytes)) => {
                host.set_send_buffer_size(bytes.0)
            }
            UdpSocketSetting::Ip(IpSocketSetting::ReceiveBuffer(bytes)) => {
                host.set_recv_buffer_size(bytes.0)
            }
            UdpSocketSetting::Ip(IpSocketSetting::MulticastLoopback(on)) if socket.ipv6 => {
                host.set_multicast_loop_v6(on)
            }
            UdpSocketSetting::Ip(IpSocketSetting::MulticastLoopback(on)) => {
                host.set_multicast_loop_v4(on)
            }
            UdpSocketSetting::Port(PortSetting::AddressSharing(sharing)) => {
                host.set_reuse_address(sharing == AddressSharing::Shared)
            }
            UdpSocketSetting::Port(PortSetting::PortSharing(sharing)) => {
                host.set_reuse_port(sharing == PortSharing::Shared)
            }
            // What every socket here has and cannot be changed from.
            UdpSocketSetting::Ip(IpSocketSetting::MaximumPacingRate(None))
            | UdpSocketSetting::Port(
                PortSetting::NonlocalAddresses(NonlocalAddresses::Refused)
                | PortSetting::BoundInterface(None)
                | PortSetting::GroupFilter(None)
                | PortSetting::IncomingProcessor(None),
            )
            | UdpSocketSetting::ReceiveCoalescing(ReceiveCoalescing::Separate)
            | UdpSocketSetting::ZeroChecksumReceipt(ZeroChecksumReceipt::Dropped) => Ok(()),
            UdpSocketSetting::Ip(IpSocketSetting::Timestamping(timestamping))
                if timestamping == NO_TIMESTAMPS =>
            {
                Ok(())
            }
            // Linux's answer to a UDP-Lite option on a UDP socket, the only kind opened here.
            UdpSocketSetting::MinimumChecksumCoverage(_) => {
                return Err(HostError::Errno(Errno::ENOPROTOOPT as i32));
            }
            _ => return Err(self.refuse(SETTING)),
        };
        set.map_err(|error| io_host_error(&error))
    }
}

impl NegativeRouteAdvice<HostUdpSocket> for HostUdpSockets {
    fn give_negative_route_advice(&self, _socket: &HostUdpSocket) -> Result<(), HostError> {
        Err(self.refuse(NEGATIVE_ROUTE_ADVICE))
    }
}
