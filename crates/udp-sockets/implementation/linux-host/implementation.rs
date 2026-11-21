#![cfg(target_os = "linux")]

use core::cell::RefCell;
use core::cmp::Ordering;
use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};

use etherparse::{IpNumber, UdpHeader};
use libc::c_int;
use linux_raw_sys::net as uapi;
use rustix::net::{AddressFamily, SocketFlags, SocketType};
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BindAddress, ByteCount, IpHeaderFields, IpSendFields, IpSocketSetting, IpVersion, Ipv6Families,
    LocalBinding, LocalPort, MaximumTransmissionUnit, NonlocalAddresses, PortRange, PortSetting,
    ProcessorIndex, RouteSelectors, SendError, UpperLayer, UpperLayerSelector,
};
use wcontainers_internet_protocol_linux_socket_options as shared;
use wcontainers_kernel::UserId;
use wcontainers_negative_route_advice::NegativeRouteAdvice;
use wcontainers_udp_sockets::{
    ReceiveCoalescing, UdpChecksum, UdpDatagram, UdpLocalEnd, UdpProtocol, UdpReceived,
    UdpReceivedPacket, UdpSocketSetting, UdpSockets, ZeroChecksumReceipt,
};

use shared::datagram::addresses::{for_family, local_address, unmapped};
use shared::datagram::memberships::{AnycastMemberships, set_multicast_group};
use shared::datagram::receive::{
    self as datagram_receive, Ancillary, ControlBuffer, Headers, Message,
};
use shared::datagram::send::{self as datagram_send, Control, Refusal, SendState};
use shared::datagram::sticky::{Slot, UDP_LITE_RECEIVE_COVERAGE};
use shared::{
    Shortfalls, get_option, host_error as from_errno, last_host_error, set_flag, set_int,
    set_option,
};

/// UDP and UDP-Lite sockets of the Linux machine this runs on, with a count of each refusal by its reason.
pub struct LinuxHostUdpSockets {
    shortfalls: Shortfalls,
}

pub struct LinuxHostUdpSocket {
    fd: OwnedFd,
    ipv6: bool,
    protocol: UdpProtocol,
    state: RefCell<SocketState>,
}

struct SocketState {
    send: SendState,
    nonlocal: NonlocalAddresses,
    anycast: AnycastMemberships,
    local: Option<SocketAddr>,
    peer: Option<SocketAddr>,
    drops: u32,
    deferred: VecDeque<Deferred>,
}

/// An error queue entry read while a send looked for its own, kept for the next receive.
struct Deferred {
    data: Vec<u8>,
    control: Box<ControlBuffer>,
    control_length: usize,
    source: Option<SocketAddr>,
}

impl PartialEq for LinuxHostUdpSocket {
    fn eq(&self, other: &Self) -> bool {
        self.fd.as_raw_fd() == other.fd.as_raw_fd()
    }
}

impl Eq for LinuxHostUdpSocket {}

impl PartialOrd for LinuxHostUdpSocket {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LinuxHostUdpSocket {
    fn cmp(&self, other: &Self) -> Ordering {
        self.fd.as_raw_fd().cmp(&other.fd.as_raw_fd())
    }
}

pub const SOURCE_PORT_NOT_THE_SOCKETS: &str =
    "a datagram's source port is not the port its socket is bound to";
pub const SOURCE_NOT_THE_BOUND_ADDRESS: &str =
    "a connect's source is not the address its socket is bound to";
pub const TRANSMIT_TIME_REJECTION_WITHOUT_KEY: &str =
    "linux reports a rejected transmit time with the time, not the key of its send";
pub const ERROR_OF_NO_REPORT: &str =
    "an error queue entry of an origin the contract has no report for";
pub const PORT_RANGE_OUTSIDE_THE_HOSTS: &str =
    "no port of a range the exit node chooses from is in the host's own range";

/// The receive control messages every socket turns on, by level.
const IPV4_RECEIVE_OPTIONS: [u32; 7] = [
    uapi::IP_PKTINFO,
    uapi::IP_RECVORIGDSTADDR,
    uapi::IP_RECVTTL,
    uapi::IP_RECVTOS,
    uapi::IP_RECVOPTS,
    uapi::IP_RECVFRAGSIZE,
    uapi::IP_RECVERR,
];
const IPV6_RECEIVE_OPTIONS: [u32; 10] = [
    uapi::IPV6_RECVPKTINFO,
    uapi::IPV6_RECVORIGDSTADDR,
    uapi::IPV6_RECVHOPLIMIT,
    uapi::IPV6_RECVTCLASS,
    uapi::IPV6_RECVHOPOPTS,
    uapi::IPV6_RECVDSTOPTS,
    uapi::IPV6_RECVRTHDR,
    uapi::IPV6_FLOWINFO,
    uapi::IPV6_RECVFRAGSIZE,
    uapi::IPV6_RECVERR,
];
const SOCKET_RECEIVE_OPTIONS: [u32; 3] =
    [uapi::SO_RXQ_OVFL, uapi::SO_RCVMARK, uapi::SO_RCVPRIORITY];

impl LinuxHostUdpSockets {
    pub fn new() -> Self {
        Self {
            shortfalls: Shortfalls::default(),
        }
    }

    /// How many times each named reason has made this value refuse or drop something.
    pub fn counters(&self) -> Vec<CounterReading> {
        self.shortfalls.readings()
    }

    fn count(&self, reason: &'static str) {
        self.shortfalls.count(reason);
    }

    fn refuse(&self, reason: &'static str) -> HostError {
        self.shortfalls.refuse(reason)
    }
}

impl Default for LinuxHostUdpSockets {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxHostUdpSocket {
    fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    fn version(&self) -> IpVersion {
        if self.ipv6 {
            IpVersion::V6
        } else {
            IpVersion::V4
        }
    }

    fn protocol_number(&self) -> IpNumber {
        match self.protocol {
            UdpProtocol::Udp => IpNumber::UDP,
            UdpProtocol::Lite => IpNumber::UDP_LITE,
        }
    }

    /// The ports of a header's upper layer, which must be this socket's protocol.
    fn ports(&self, upper_layer: &UpperLayer) -> Result<(u16, u16), HostError> {
        match upper_layer.selector {
            Some(UpperLayerSelector::Ports(ports))
                if upper_layer.protocol == self.protocol_number() =>
            {
                Ok((ports.source.0, ports.destination.0))
            }
            _ => Err(HostError::Errno(libc::EINVAL)),
        }
    }

    fn local(&self, state: &mut SocketState) -> Result<SocketAddr, HostError> {
        match state.local {
            Some(local) => Ok(local),
            None => {
                let local = local_address(self.fd())?;
                state.local = Some(local);
                Ok(local)
            }
        }
    }

    /// Binds an unbound socket to a datagram's source port, and refuses one bound to another port.
    fn take_source_port(
        &self,
        sockets: &LinuxHostUdpSockets,
        state: &mut SocketState,
        source: Option<IpAddr>,
        port: u16,
        destination: &SocketAddr,
    ) -> Result<(), HostError> {
        let local = self.local(state)?;
        if local.port() == u16::MIN && (port != u16::MIN || source.is_some()) {
            let address = source
                .unwrap_or_else(|| datagram_send::unspecified_like(unmapped(*destination).ip()));
            let address = for_family(SocketAddr::new(address, port), self.ipv6);
            rustix::net::bind(self.fd(), &address).map_err(from_errno)?;
            state.local = Some(local_address(self.fd())?);
        } else if port != u16::MIN && port != local.port() {
            return Err(sockets.refuse(SOURCE_PORT_NOT_THE_SOCKETS));
        }
        Ok(())
    }

    fn send_error(&self, state: &mut SocketState) -> SendError {
        // A local error of the error queue carries the MTU a datagram was too long for.
        loop {
            let mut control = Box::new(ControlBuffer::new());
            let mut data = vec![u8::MIN; usize::from(u16::MAX)];
            let (length, control_length, source, error) = {
                let Ok(message) =
                    datagram_receive::receive(self.fd(), &mut data, &mut control, true)
                else {
                    return SendError::Host(HostError::Errno(libc::EMSGSIZE));
                };
                let ancillary = Ancillary::parse(message.control);
                (
                    message.length,
                    message.control.len(),
                    message.source,
                    ancillary.error.map(|(error, _)| error),
                )
            };
            match error {
                Some(error)
                    if error.ee_origin == libc::SO_EE_ORIGIN_LOCAL
                        && error.ee_errno == libc::EMSGSIZE as u32 =>
                {
                    return SendError::MessageTooLong {
                        maximum_transmission_unit: MaximumTransmissionUnit(error.ee_info),
                    };
                }
                _ => {
                    data.truncate(length);
                    state.deferred.push_back(Deferred {
                        data,
                        control,
                        control_length,
                        source,
                    });
                }
            }
        }
    }

    /// A packet or report of the error queue, or `None` for an entry that has none.
    fn error_entry(
        &self,
        sockets: &LinuxHostUdpSockets,
        state: &mut SocketState,
        into: &mut [u8],
        message: &Message<'_>,
    ) -> Result<Option<UdpReceived>, HostError> {
        let ancillary = Ancillary::parse(message.control);
        let Some((error, offender)) = ancillary.error else {
            sockets.count(ERROR_OF_NO_REPORT);
            return Ok(None);
        };
        let copied = message.length.min(into.len());
        match error.ee_origin {
            libc::SO_EE_ORIGIN_ICMP | libc::SO_EE_ORIGIN_ICMP6 => {
                let local = self.local(state)?;
                let packet = icmp_error(
                    into,
                    copied,
                    &ancillary,
                    &error,
                    offender,
                    message.source,
                    local,
                    self.protocol_number(),
                );
                Ok(packet.map(|length| {
                    UdpReceived::Packet(UdpReceivedPacket {
                        fields: ancillary.packet_fields(length, u32::MIN),
                        segment_size: None,
                    })
                }))
            }
            libc::SO_EE_ORIGIN_TIMESTAMPING => {
                let packet = (message.length > usize::MIN).then_some(message.length);
                match ancillary.transmit_report(&error, packet) {
                    Some(report) => Ok(Some(UdpReceived::TransmitReport(report))),
                    None => {
                        sockets.count(ERROR_OF_NO_REPORT);
                        Ok(None)
                    }
                }
            }
            // The send that made a local error answered it.
            libc::SO_EE_ORIGIN_LOCAL => Ok(None),
            datagram_receive::SO_EE_ORIGIN_TXTIME => {
                sockets.count(TRANSMIT_TIME_REJECTION_WITHOUT_KEY);
                Ok(None)
            }
            _ => {
                sockets.count(ERROR_OF_NO_REPORT);
                Ok(None)
            }
        }
    }

    fn datagram(
        &self,
        state: &mut SocketState,
        into: &mut [u8],
        message: &Message<'_>,
    ) -> Result<UdpReceived, HostError> {
        let ancillary = Ancillary::parse(message.control);
        let source = message.source.ok_or(HostError::Errno(libc::EINVAL))?;
        let local = self.local(state)?;
        let destination = ancillary
            .original_destination
            .map(unmapped)
            .unwrap_or(unmapped(local));
        let protocol = self.protocol_number();
        let payload = message.length;
        let udp_length = UdpHeader::LEN + payload;
        let mut headers = Headers::new();
        match (source.ip(), destination.ip()) {
            (IpAddr::V4(from), IpAddr::V4(to)) => {
                let header =
                    datagram_receive::ipv4_header(from, to, protocol, udp_length, &ancillary)
                        .ok_or(HostError::Errno(libc::EMSGSIZE))?;
                headers.push(&header.to_bytes());
            }
            (IpAddr::V6(from), IpAddr::V6(to)) => {
                datagram_receive::push_ipv6_headers(
                    &mut headers,
                    from,
                    to,
                    protocol,
                    udp_length,
                    &ancillary,
                )
                .ok_or(HostError::Errno(libc::EMSGSIZE))?;
            }
            _ => return Err(HostError::Errno(libc::EAFNOSUPPORT)),
        }
        let udp_start = headers.len();
        let length_field = match self.protocol {
            UdpProtocol::Udp => {
                u16::try_from(udp_length).map_err(|_| HostError::Errno(libc::EMSGSIZE))?
            }
            // The coverage a UDP-Lite sender chose is not given to the receiver: zero is the whole datagram.
            UdpProtocol::Lite => u16::MIN,
        };
        let udp = UdpHeader {
            source_port: source.port(),
            destination_port: destination.port(),
            length: length_field,
            checksum: u16::MIN,
        };
        headers.push(&udp.to_bytes());
        let copied = payload.min(into.len());
        let placed = headers.place(into, copied);
        if placed >= headers.len() {
            let checksum = datagram_receive::udp_checksum(
                source.ip(),
                destination.ip(),
                protocol,
                udp_length,
                &into[udp_start..headers.len()],
                &into[headers.len()..placed],
            );
            let at = udp_start + UdpHeader::LEN - size_of_checksum();
            into[at..at + size_of_checksum()].copy_from_slice(&checksum.to_ne_bytes());
        }
        let drops = match ancillary.drops {
            Some(total) => {
                let since = total.wrapping_sub(state.drops);
                state.drops = total;
                since
            }
            None => u32::MIN,
        };
        Ok(UdpReceived::Packet(UdpReceivedPacket {
            fields: ancillary.packet_fields(headers.len() + payload, drops),
            segment_size: ancillary
                .segment_size
                .and_then(core::num::NonZeroU16::new)
                .map(wcontainers_udp_sockets::SegmentSize),
        }))
    }

    /// Binds to a port of the first range with a free one, through the kernel's choice within that range.
    fn bind_exit_node_port(
        &self,
        sockets: &LinuxHostUdpSockets,
        address: SocketAddr,
        ranges: &[PortRange],
    ) -> Result<(), HostError> {
        let host = host_port_range();
        let mut inside = false;
        for range in ranges {
            let first = range.first.0.max(host.first.0);
            let last = range.last.0.min(host.last.0);
            if first > last {
                continue;
            }
            inside = true;
            let packed = u32::from(first) | (u32::from(last) << u16::BITS);
            set_option(self.fd(), uapi::SOL_IP, uapi::IP_LOCAL_PORT_RANGE, &packed)?;
            match rustix::net::bind(self.fd(), &address) {
                Ok(()) => return Ok(()),
                Err(rustix::io::Errno::ADDRINUSE) => continue,
                Err(error) => return Err(from_errno(error)),
            }
        }
        if inside {
            Err(HostError::Errno(libc::EADDRINUSE))
        } else {
            Err(sockets.refuse(PORT_RANGE_OUTSIDE_THE_HOSTS))
        }
    }
}

/// Writes the options of the socket's permission to use addresses it does not have, which sends may have changed.
fn apply_nonlocal(
    fd: BorrowedFd<'_>,
    ipv6: bool,
    send: &mut SendState,
    nonlocal: NonlocalAddresses,
) -> Result<(), HostError> {
    let (freebind, transparent) = if ipv6 {
        (Slot::Ipv6Freebind, Slot::Ipv6Transparent)
    } else {
        (Slot::Ipv4Freebind, Slot::Ipv4Transparent)
    };
    send.sticky
        .ensure_flag(fd, freebind, nonlocal == NonlocalAddresses::Bindable)?;
    send.sticky
        .ensure_flag(fd, transparent, nonlocal == NonlocalAddresses::Transparent)
}

const fn size_of_checksum() -> usize {
    core::mem::size_of::<u16>()
}

/// Where the host's own range of ports to choose from is, `net.ipv4.ip_local_port_range`.
const HOST_PORT_RANGE: &str = "/proc/sys/net/ipv4/ip_local_port_range";

/// The host's own range of ports to choose from.
fn host_port_range() -> PortRange {
    let text = std::fs::read_to_string(HOST_PORT_RANGE).unwrap_or_default();
    let mut ports = text
        .split_whitespace()
        .filter_map(|word| word.parse::<u16>().ok());
    match (ports.next(), ports.next()) {
        (Some(first), Some(last)) => PortRange {
            first: wcontainers_internet_protocol::Port(first),
            last: wcontainers_internet_protocol::Port(last),
        },
        _ => PortRange {
            first: wcontainers_internet_protocol::Port(u16::MIN),
            last: wcontainers_internet_protocol::Port(u16::MAX),
        },
    }
}

/// The ranges of a choice less its reserved ones, in order.
fn allowed_ranges(range: PortRange, reserved: &[PortRange]) -> Vec<PortRange> {
    let mut allowed = vec![range];
    for hole in reserved {
        allowed = allowed
            .into_iter()
            .flat_map(|part| {
                let mut pieces = Vec::new();
                if hole.last.0 < part.first.0 || hole.first.0 > part.last.0 {
                    pieces.push(part);
                    return pieces;
                }
                if hole.first.0 > part.first.0 {
                    pieces.push(PortRange {
                        first: part.first,
                        last: wcontainers_internet_protocol::Port(hole.first.0 - 1),
                    });
                }
                if hole.last.0 < part.last.0 {
                    pieces.push(PortRange {
                        first: wcontainers_internet_protocol::Port(hole.last.0 + 1),
                        last: part.last,
                    });
                }
                pieces
            })
            .collect();
    }
    allowed
}

/// Writes an ICMP error about a datagram this socket sent, from its IP header, and returns its length.
#[allow(clippy::too_many_arguments)]
fn icmp_error(
    into: &mut [u8],
    copied: usize,
    ancillary: &Ancillary<'_>,
    error: &libc::sock_extended_err,
    offender: Option<SocketAddr>,
    original_destination: Option<SocketAddr>,
    local: SocketAddr,
    protocol: IpNumber,
) -> Option<usize> {
    let offender = unmapped(offender?);
    let destination = unmapped(original_destination?);
    let us = ancillary
        .header_destination
        .map(|address| unmapped(SocketAddr::new(address, u16::MIN)).ip())
        .unwrap_or(unmapped(local).ip());
    let quoted = UdpHeader {
        source_port: local.port(),
        destination_port: destination.port(),
        length: u16::try_from(UdpHeader::LEN + copied).ok()?,
        checksum: u16::MIN,
    };
    let quiet = Ancillary::default();
    let mut headers = Headers::new();
    let rest = rest_of_header(error);
    let icmp_start;
    let icmp_length;
    match (offender.ip(), us, destination.ip()) {
        (IpAddr::V4(offender), IpAddr::V4(us), IpAddr::V4(destination)) => {
            let inner = datagram_receive::ipv4_header(
                us,
                destination,
                protocol,
                UdpHeader::LEN + copied,
                &quiet,
            )?;
            icmp_length =
                etherparse::Icmpv4Header::MIN_LEN + inner.header_len() + UdpHeader::LEN + copied;
            let outer = datagram_receive::ipv4_header(
                offender,
                us,
                IpNumber::ICMP,
                icmp_length,
                ancillary,
            )?;
            headers.push(&outer.to_bytes());
            icmp_start = headers.len();
            headers.push(&[error.ee_type, error.ee_code]);
            headers.push(&[u8::MIN; 2]);
            headers.push(&rest);
            headers.push(&inner.to_bytes());
        }
        (IpAddr::V6(offender), IpAddr::V6(us), IpAddr::V6(destination)) => {
            icmp_length = etherparse::Icmpv6Header::MIN_LEN
                + etherparse::Ipv6Header::LEN
                + UdpHeader::LEN
                + copied;
            datagram_receive::push_ipv6_headers(
                &mut headers,
                offender,
                us,
                IpNumber::IPV6_ICMP,
                icmp_length,
                ancillary,
            )?;
            icmp_start = headers.len();
            headers.push(&[error.ee_type, error.ee_code]);
            headers.push(&[u8::MIN; 2]);
            headers.push(&rest);
            datagram_receive::push_ipv6_headers(
                &mut headers,
                us,
                destination,
                protocol,
                UdpHeader::LEN + copied,
                &quiet,
            )?;
        }
        _ => return None,
    }
    headers.push(&quoted.to_bytes());
    let placed = headers.place(into, copied);
    if placed >= headers.len() {
        let message = &into[icmp_start..placed];
        let checksum = match (offender.ip(), us) {
            (IpAddr::V6(_), IpAddr::V6(_)) => datagram_receive::udp_checksum(
                offender.ip(),
                us,
                IpNumber::IPV6_ICMP,
                icmp_length,
                message,
                &[],
            ),
            _ => etherparse::checksum::Sum16BitWords::new()
                .add_slice(message)
                .ones_complement()
                .to_be(),
        };
        let at = icmp_start + size_of_checksum();
        into[at..at + size_of_checksum()].copy_from_slice(&checksum.to_ne_bytes());
    }
    Some(icmp_start + icmp_length)
}

/// The four bytes after an ICMP message's checksum: the MTU of a packet too big, the pointer of a parameter
/// problem, else unused.
fn rest_of_header(error: &libc::sock_extended_err) -> [u8; size_of::<u32>()] {
    let v4 = error.ee_origin == libc::SO_EE_ORIGIN_ICMP;
    let mtu = if v4 {
        error.ee_type == etherparse::icmpv4::TYPE_DEST_UNREACH
            && error.ee_code == etherparse::icmpv4::CODE_DST_UNREACH_NEED_FRAG
    } else {
        error.ee_type == etherparse::icmpv6::TYPE_PACKET_TOO_BIG
    };
    let pointer = if v4 {
        error.ee_type == etherparse::icmpv4::TYPE_PARAMETER_PROBLEM
    } else {
        error.ee_type == etherparse::icmpv6::TYPE_PARAMETER_PROBLEM
    };
    if mtu {
        error.ee_info.to_be_bytes()
    } else if pointer && v4 {
        [error.ee_info as u8, u8::MIN, u8::MIN, u8::MIN]
    } else if pointer {
        error.ee_info.to_be_bytes()
    } else {
        [u8::MIN; size_of::<u32>()]
    }
}

impl UdpSockets for LinuxHostUdpSockets {
    type Socket = LinuxHostUdpSocket;

    fn open(
        &self,
        version: IpVersion,
        protocol: UdpProtocol,
        owner: UserId,
    ) -> Result<Self::Socket, HostError> {
        let ipv6 = version == IpVersion::V6;
        let family = if ipv6 {
            AddressFamily::INET6
        } else {
            AddressFamily::INET
        };
        let number = match protocol {
            UdpProtocol::Udp => rustix::net::ipproto::UDP,
            UdpProtocol::Lite => rustix::net::ipproto::UDPLITE,
        };
        let fd = as_owner(owner, || {
            rustix::net::socket_with(
                family,
                SocketType::DGRAM,
                SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
                Some(number),
            )
            .map_err(from_errno)
        })?;
        let borrowed = fd.as_fd();
        for option in IPV4_RECEIVE_OPTIONS {
            set_flag(borrowed, uapi::SOL_IP, option, true)?;
        }
        if ipv6 {
            for option in IPV6_RECEIVE_OPTIONS {
                set_flag(borrowed, uapi::SOL_IPV6, option, true)?;
            }
        }
        for option in SOCKET_RECEIVE_OPTIONS {
            set_flag(borrowed, uapi::SOL_SOCKET, option, true)?;
        }
        let send = SendState::open(borrowed, ipv6)?;
        Ok(LinuxHostUdpSocket {
            fd,
            ipv6,
            protocol,
            state: RefCell::new(SocketState {
                send,
                nonlocal: NonlocalAddresses::Refused,
                anycast: AnycastMemberships::default(),
                local: None,
                peer: None,
                drops: u32::MIN,
                deferred: VecDeque::new(),
            }),
        })
    }

    fn bind(
        &self,
        socket: &Self::Socket,
        local: LocalBinding<'_>,
    ) -> Result<SocketAddr, HostError> {
        let fd = socket.fd();
        let mut state = socket.state.borrow_mut();
        let state = &mut *state;
        apply_nonlocal(fd, socket.ipv6, &mut state.send, state.nonlocal)?;
        let address = match local.address {
            BindAddress::V4(address) => IpAddr::V4(address),
            BindAddress::V6 { address, families } => {
                if socket.ipv6 {
                    set_flag(
                        fd,
                        uapi::SOL_IPV6,
                        uapi::IPV6_V6ONLY,
                        families == Ipv6Families::V6Only,
                    )?;
                }
                IpAddr::V6(address)
            }
        };
        let port = match local.port {
            LocalPort::Requested(port) | LocalPort::ChosenByKernel(port) => port.0,
            LocalPort::ChosenByExitNode(_) => u16::MIN,
        };
        let address = for_family(SocketAddr::new(address, port), socket.ipv6);
        match local.port {
            LocalPort::ChosenByExitNode(choice) => {
                socket.bind_exit_node_port(
                    self,
                    address,
                    &allowed_ranges(choice.range, choice.reserved),
                )?;
            }
            _ => rustix::net::bind(fd, &address).map_err(from_errno)?,
        }
        let bound = local_address(fd)?;
        state.local = Some(bound);
        Ok(unmapped(bound))
    }

    fn connect(
        &self,
        socket: &Self::Socket,
        route: RouteSelectors<'_>,
    ) -> Result<SocketAddr, HostError> {
        let fd = socket.fd();
        let mut state = socket.state.borrow_mut();
        let state = &mut *state;
        let (upper_layer, source) = match &route {
            RouteSelectors::V4(route) => (route.upper_layer, route.source.map(IpAddr::V4)),
            RouteSelectors::V6(route) => (route.upper_layer, route.source.map(IpAddr::V6)),
        };
        let (source_port, destination_port) = socket.ports(&upper_layer)?;
        let destination = state
            .send
            .apply_route(fd, &route, destination_port, socket.ipv6)?;
        socket.take_source_port(self, state, source, source_port, &destination)?;
        let before = socket.local(state)?;
        rustix::net::connect(fd, &destination).map_err(from_errno)?;
        let local = local_address(fd)?;
        if let Some(source) = source
            && unmapped(local).ip() != source
        {
            rustix::net::connect_unspec(fd).map_err(from_errno)?;
            state.local = Some(before);
            state.peer = None;
            return Err(self.refuse(SOURCE_NOT_THE_BOUND_ADDRESS));
        }
        state.local = Some(local);
        state.peer = Some(destination);
        Ok(unmapped(local))
    }

    fn disconnect(&self, socket: &Self::Socket) -> Result<UdpLocalEnd, HostError> {
        let fd = socket.fd();
        rustix::net::connect_unspec(fd).map_err(from_errno)?;
        let mut state = socket.state.borrow_mut();
        state.peer = None;
        let local = local_address(fd)?;
        state.local = Some(local);
        let unbound = local.port() == u16::MIN && local.ip().is_unspecified();
        Ok(if unbound {
            UdpLocalEnd::Unbound
        } else {
            UdpLocalEnd::Bound(unmapped(local))
        })
    }

    fn send_to(
        &self,
        socket: &Self::Socket,
        header: IpHeaderFields<'_>,
        fields: IpSendFields,
        datagram: UdpDatagram<'_>,
    ) -> Result<(), SendError> {
        let fd = socket.fd();
        let mut state = socket.state.borrow_mut();
        let state = &mut *state;
        let (upper_layer, ipv4) = match &header {
            IpHeaderFields::V4(header) => (header.route.upper_layer, true),
            IpHeaderFields::V6(header) => (header.route.upper_layer, false),
        };
        let (source_port, destination_port) =
            socket.ports(&upper_layer).map_err(SendError::Host)?;
        let mut control = Control::new();
        let destination = state
            .send
            .apply_header(fd, &mut control, &header, destination_port, socket.ipv6)
            .map_err(SendError::Host)?;
        socket
            .take_source_port(self, state, None, source_port, &destination)
            .map_err(SendError::Host)?;
        let flags = match state.send.apply_send_fields(fd, &mut control, &fields) {
            Ok(flags) => flags,
            Err(Refusal::Host(error)) => return Err(SendError::Host(error)),
            Err(Refusal::Unsupported(reason)) => return Err(SendError::Host(self.refuse(reason))),
        };
        let sticky = &mut state.send.sticky;
        let invalid = SendError::Host(HostError::Errno(libc::EINVAL));
        let checksum = match (socket.protocol, datagram.checksum) {
            (UdpProtocol::Udp, UdpChecksum::Computed) => (false, false, u16::MIN),
            (UdpProtocol::Udp, UdpChecksum::Omitted) => (ipv4, !ipv4, u16::MIN),
            (UdpProtocol::Lite, UdpChecksum::Computed) => (false, false, u16::MIN),
            (UdpProtocol::Lite, UdpChecksum::Partial(coverage)) => (false, false, coverage.0.get()),
            _ => return Err(invalid),
        };
        sticky
            .ensure_flag(fd, Slot::NoChecksum, checksum.0)
            .map_err(SendError::Host)?;
        if socket.ipv6 {
            sticky
                .ensure_flag(fd, Slot::NoChecksumIpv6, checksum.1)
                .map_err(SendError::Host)?;
        }
        if socket.protocol == UdpProtocol::Lite {
            sticky
                .ensure_int(fd, Slot::ChecksumCoverage, c_int::from(checksum.2))
                .map_err(SendError::Host)?;
        }
        if let Some(size) = datagram.segment_size {
            control.push(
                libc::SOL_UDP as u32,
                libc::UDP_SEGMENT as u32,
                &size.0.get(),
            );
        }
        let name = (state.peer != Some(destination)).then_some(destination);
        match send_message(fd, name, datagram.payload, control.bytes(), flags) {
            Ok(()) => {
                if state.local.is_some_and(|local| local.port() == u16::MIN) {
                    state.local = None;
                }
                Ok(())
            }
            Err(HostError::Errno(libc::EMSGSIZE)) => Err(socket.send_error(state)),
            Err(error) => Err(SendError::Host(error)),
        }
    }

    fn receive(&self, socket: &Self::Socket, into: &mut [u8]) -> Result<UdpReceived, HostError> {
        let fd = socket.fd();
        let mut state = socket.state.borrow_mut();
        let state = &mut *state;
        loop {
            let mut control = ControlBuffer::new();
            let received = match state.deferred.pop_front() {
                Some(deferred) => {
                    let copied = deferred.data.len().min(into.len());
                    into[..copied].copy_from_slice(&deferred.data[..copied]);
                    let message = Message {
                        length: deferred.data.len(),
                        source: deferred.source,
                        control: &deferred.control.bytes()[..deferred.control_length],
                        control_truncated: false,
                    };
                    socket.error_entry(self, state, into, &message)?
                }
                None => match datagram_receive::receive(fd, into, &mut control, true) {
                    Ok(message) => {
                        if message.control_truncated {
                            self.count(datagram_receive::CONTROL_TRUNCATED);
                        }
                        socket.error_entry(self, state, into, &message)?
                    }
                    Err(HostError::WouldBlock) => break,
                    Err(error) => return Err(error),
                },
            };
            if let Some(received) = received {
                return Ok(received);
            }
        }
        let mut control = ControlBuffer::new();
        let message = datagram_receive::receive(fd, into, &mut control, false)?;
        if message.control_truncated {
            self.count(datagram_receive::CONTROL_TRUNCATED);
        }
        socket.datagram(state, into, &message)
    }

    fn count_unsent_bytes(&self, socket: &Self::Socket) -> Result<ByteCount, HostError> {
        let mut unsent: c_int = c_int::default();
        // SAFETY: SIOCOUTQ (TIOCOUTQ) writes one int through the pointer.
        let result = unsafe { libc::ioctl(socket.fd.as_raw_fd(), libc::TIOCOUTQ, &mut unsent) };
        if result < 0 {
            return Err(last_host_error());
        }
        Ok(ByteCount(unsent as usize))
    }

    fn read_incoming_processor(
        &self,
        socket: &Self::Socket,
    ) -> Result<Option<ProcessorIndex>, HostError> {
        let processor = get_option::<c_int>(socket.fd(), uapi::SOL_SOCKET, uapi::SO_INCOMING_CPU)?;
        Ok(u32::try_from(processor).ok().map(ProcessorIndex))
    }

    fn close(&self, socket: Self::Socket) {
        drop(socket);
    }
}

/// One `sendmsg` of a payload to a name, or to the connected peer when there is none.
fn send_message(
    fd: BorrowedFd<'_>,
    name: Option<SocketAddr>,
    payload: &[u8],
    control: &[u8],
    flags: c_int,
) -> Result<(), HostError> {
    let send = |name: *const libc::c_void, length: libc::socklen_t| {
        let mut vector = libc::iovec {
            iov_base: payload.as_ptr() as *mut _,
            iov_len: payload.len(),
        };
        let mut header = shared::datagram::addresses::zeroed::<libc::msghdr>();
        header.msg_name = name as *mut _;
        header.msg_namelen = length;
        header.msg_iov = &mut vector;
        header.msg_iovlen = 1;
        if !control.is_empty() {
            header.msg_control = control.as_ptr() as *mut _;
            header.msg_controllen = control.len();
        }
        // SAFETY: every pointer in the header describes a buffer that outlives the call.
        unsafe { libc::sendmsg(fd.as_raw_fd(), &header, flags) }
    };
    let sent = match name {
        // SAFETY: rustix gives a pointer to `length` bytes of the socket address for the duration of the call.
        Some(name) => unsafe {
            rustix::net::addr::SocketAddrArg::with_sockaddr(&name, |pointer, length| {
                send(pointer.cast(), length)
            })
        },
        None => send(core::ptr::null(), libc::socklen_t::MIN),
    };
    if sent < 0 {
        Err(last_host_error())
    } else {
        Ok(())
    }
}

/// Runs `make` with the thread's filesystem user set to `owner`, which a new socket takes as its own.
fn as_owner<T>(owner: UserId, make: impl FnOnce() -> Result<T, HostError>) -> Result<T, HostError> {
    let query = libc::uid_t::MAX;
    // SAFETY: setfsuid with an invalid id changes nothing and returns the current filesystem user.
    let current = unsafe { libc::setfsuid(query) } as libc::uid_t;
    if current == owner.0 {
        return make();
    }
    // SAFETY: setfsuid only changes this thread's filesystem user; it is set back below.
    let changed = unsafe {
        libc::setfsuid(owner.0);
        libc::setfsuid(query) as libc::uid_t == owner.0
    };
    let result = if changed {
        make()
    } else {
        Err(HostError::Errno(libc::EPERM))
    };
    // SAFETY: as above.
    unsafe { libc::setfsuid(current) };
    result
}

impl<'a> Configurable<LinuxHostUdpSocket, UdpSocketSetting<'a>> for LinuxHostUdpSockets {
    fn configure(
        &self,
        socket: &LinuxHostUdpSocket,
        setting: UdpSocketSetting<'a>,
    ) -> Result<(), HostError> {
        let fd = socket.fd();
        let mut state = socket.state.borrow_mut();
        let state = &mut *state;
        match setting {
            UdpSocketSetting::Ip(IpSocketSetting::MulticastV4(group)) => set_multicast_group(
                fd,
                uapi::SOL_IP,
                IpAddr::V4(group.group),
                &group,
                |source| IpAddr::V4(*source),
            ),
            UdpSocketSetting::Ip(IpSocketSetting::MulticastV6(group)) => set_multicast_group(
                fd,
                uapi::SOL_IPV6,
                IpAddr::V6(group.group),
                &group,
                |source| IpAddr::V6(*source),
            ),
            UdpSocketSetting::Ip(IpSocketSetting::Anycast(anycast)) => {
                state.anycast.set(fd, &anycast)
            }
            UdpSocketSetting::Ip(IpSocketSetting::Timestamping(timestamping)) => {
                state.send.set_receive_timestamps(fd, &timestamping)
            }
            UdpSocketSetting::Ip(setting) => shared::set_ip_socket_setting(
                fd,
                socket.version(),
                &setting,
                u32::MIN,
                &self.shortfalls,
            ),
            UdpSocketSetting::Port(setting) => {
                shared::set_port_setting(fd, socket.version(), &setting)?;
                if let PortSetting::NonlocalAddresses(nonlocal) = setting {
                    state.nonlocal = nonlocal;
                    for slot in [
                        Slot::Ipv4Freebind,
                        Slot::Ipv4Transparent,
                        Slot::Ipv6Freebind,
                        Slot::Ipv6Transparent,
                    ] {
                        state.send.sticky.forget(slot);
                    }
                }
                Ok(())
            }
            UdpSocketSetting::ReceiveCoalescing(coalescing) => set_flag(
                fd,
                libc::SOL_UDP as u32,
                libc::UDP_GRO as u32,
                coalescing == ReceiveCoalescing::BySegmentSize,
            ),
            UdpSocketSetting::ZeroChecksumReceipt(receipt) => set_flag(
                fd,
                libc::SOL_UDP as u32,
                libc::UDP_NO_CHECK6_RX as u32,
                receipt == ZeroChecksumReceipt::Accepted,
            ),
            UdpSocketSetting::MinimumChecksumCoverage(coverage) => set_int(
                fd,
                uapi::IPPROTO_UDPLITE as u32,
                UDP_LITE_RECEIVE_COVERAGE,
                c_int::from(coverage.map_or(u16::MIN, |coverage| coverage.0.get())),
            ),
        }
    }
}

impl NegativeRouteAdvice<LinuxHostUdpSocket> for LinuxHostUdpSockets {
    fn give_negative_route_advice(&self, socket: &LinuxHostUdpSocket) -> Result<(), HostError> {
        // Linux has no call for it: connecting again drops the cached route and takes a new flow hash.
        let peer = socket.state.borrow().peer;
        match peer {
            Some(peer) => rustix::net::connect(socket.fd(), &peer).map_err(from_errno),
            None => Ok(()),
        }
    }
}
