#![cfg(target_os = "linux")]

use core::cell::RefCell;
use core::cmp::Ordering;
use core::mem::MaybeUninit;
use std::net::{IpAddr, SocketAddr};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};

use etherparse::{IpNumber, UdpHeader};
use libc::c_int;
use linux_raw_sys::net as uapi;
use rustix::net::SocketType;
use socket2::SockAddr;
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BindAddress, ByteCount, IpHeaderFields, IpSendFields, IpSocketSetting, IpVersion, LocalBinding,
    LocalPort, NonlocalAddresses, PortSetting, ProcessorIndex, Received, Report, RouteSelectors,
    SendError, UpperLayer, UpperLayerSelector,
};
use wcontainers_internet_protocol_linux_socket_options as shared;
use wcontainers_kernel::UserId;
use wcontainers_kernel::signal_information::PollEvents;
use wcontainers_negative_route_advice::NegativeRouteAdvice;
use wcontainers_udp_sockets::{
    ReceiveCoalescing, UdpChecksum, UdpDatagram, UdpLocalEnd, UdpProtocol, UdpReceived,
    UdpReceivedPacket, UdpSocketSetting, UdpSockets, ZeroChecksumReceipt,
};

use shared::datagram::addresses::{for_family, local_address, unmapped};
use shared::datagram::exchange::{ErrorQueue, send_message};
use shared::datagram::memberships::{AnycastMemberships, set_multicast_group};
use shared::datagram::receive::{
    self as datagram_receive, Ancillary, ControlBuffer, Headers, Message,
};
use shared::datagram::send::{self as datagram_send, Control, Refusal, SendState};
use shared::datagram::sticky::{Slot, UDP_LITE_RECEIVE_COVERAGE};
use shared::icmp_error::write_icmp_error;
use shared::readiness::{NO_EVENTS, Readiness, Watched};
use shared::{
    Shortfalls, host_error as from_errno, io_host_error, last_host_error, set_flag, set_int, sock,
};

/// UDP and UDP-Lite sockets of the Linux machine this runs on, with a count of each refusal by its reason.
pub struct HostUdpSockets {
    shortfalls: Shortfalls,
    readiness: Readiness,
}

pub struct HostUdpSocket {
    watched: Watched,
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
    errors: ErrorQueue,
    /// Whether the error queue may hold reports a send asked for or entries a send read, which come first.
    errors_first: bool,
    /// Whether the socket's readiness holds an error for the entries kept off the host's error queue.
    held_errors: bool,
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

pub const SOURCE_PORT_NOT_THE_SOCKETS: &str =
    "a datagram's source port is not the port its socket is bound to";
pub const SOURCE_NOT_THE_BOUND_ADDRESS: &str =
    "a connect's source is not the address its socket is bound to";
pub use shared::datagram::exchange::{ERROR_OF_NO_REPORT, TRANSMIT_TIME_REJECTION_WITHOUT_KEY};
pub const SEND_BUFFER_CUT_BY_THE_HOST: &str =
    "a send buffer size above the host's net.core.wmem_max, which the host cuts to it";
pub const RECEIVE_BUFFER_CUT_BY_THE_HOST: &str =
    "a receive buffer size above the host's net.core.rmem_max, which the host cuts to it";

/// The largest payload of one receive: an IP packet's largest length.
const LARGEST_PAYLOAD: usize = u16::MAX as usize;

/// The receive control messages every socket turns on that socket2 does not, by level.
const IPV4_RECEIVE_OPTIONS: [u32; 6] = [
    uapi::IP_PKTINFO,
    uapi::IP_RECVORIGDSTADDR,
    uapi::IP_RECVTTL,
    uapi::IP_RECVOPTS,
    uapi::IP_RECVFRAGSIZE,
    uapi::IP_RECVERR,
];
const IPV6_RECEIVE_OPTIONS: [u32; 8] = [
    uapi::IPV6_RECVPKTINFO,
    uapi::IPV6_RECVORIGDSTADDR,
    uapi::IPV6_RECVHOPOPTS,
    uapi::IPV6_RECVDSTOPTS,
    uapi::IPV6_RECVRTHDR,
    uapi::IPV6_FLOWINFO,
    uapi::IPV6_RECVFRAGSIZE,
    uapi::IPV6_RECVERR,
];
const SOCKET_RECEIVE_OPTIONS: [u32; 3] =
    [uapi::SO_RXQ_OVFL, uapi::SO_RCVMARK, uapi::SO_RCVPRIORITY];

impl HostUdpSockets {
    pub fn new() -> Result<Self, HostError> {
        Ok(Self {
            shortfalls: Shortfalls::default(),
            readiness: Readiness::new()?,
        })
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

impl HostUdpSocket {
    fn fd(&self) -> BorrowedFd<'_> {
        self.watched.fd()
    }

    /// Holds an error in the socket's readiness while entries taken off the host's error queue wait for receives.
    fn note_kept_entries(&self, state: &mut SocketState) {
        let holds = state.errors.holds_entries();
        if holds != state.held_errors {
            state.held_errors = holds;
            self.watched.hold(PollEvents {
                error_condition: holds,
                ..NO_EVENTS
            });
        }
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
        sockets: &HostUdpSockets,
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
            shared::bind_to(self.fd(), address).map_err(from_errno)?;
            state.local = Some(local_address(self.fd())?);
        } else if port != u16::MIN && port != local.port() {
            return Err(sockets.refuse(SOURCE_PORT_NOT_THE_SOCKETS));
        }
        Ok(())
    }

    /// The next packet or report of the error queue, skipping entries that have none.
    fn take_error_entry(
        &self,
        sockets: &HostUdpSockets,
        state: &mut SocketState,
        into: &mut [u8],
    ) -> Result<UdpReceived, HostError> {
        let local = self.local(state)?;
        let protocol = self.protocol_number();
        loop {
            let entry = state.errors.next(self.fd(), &sockets.shortfalls)?;
            let write =
                |into: &mut [u8], copied, ancillary: &Ancillary<'_>, error: &_, offender| {
                    // Linux gives the quoted datagram from its payload on, after the destination it was sent to.
                    let destination = entry.source.map(unmapped)?;
                    let quoted = UdpHeader {
                        source_port: local.port(),
                        destination_port: destination.port(),
                        length: u16::try_from(UdpHeader::LEN + copied).ok()?,
                        checksum: u16::MIN,
                    };
                    write_icmp_error(
                        into,
                        copied,
                        ancillary,
                        error,
                        offender,
                        unmapped(local).ip(),
                        destination.ip(),
                        protocol,
                        &quoted.to_bytes(),
                    )
                };
            match state
                .errors
                .received(&sockets.shortfalls, &entry, into, write)
            {
                Some(Received::Packet(fields)) => {
                    return Ok(UdpReceived::Packet(UdpReceivedPacket {
                        fields,
                        segment_size: None,
                    }));
                }
                Some(Received::TransmitReport(report)) => {
                    return Ok(UdpReceived::TransmitReport(report));
                }
                None => {}
            }
        }
    }

    fn datagram(
        &self,
        state: &mut SocketState,
        into: &mut [u8],
        tail: &[u8],
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
        let length_field = match self.protocol {
            UdpProtocol::Udp => {
                u16::try_from(udp_length).map_err(|_| HostError::Errno(libc::EMSGSIZE))?
            }
            // The coverage a UDP-Lite sender chose is not given to the receiver: zero is the whole datagram.
            UdpProtocol::Lite => u16::MIN,
        };
        let mut udp = UdpHeader {
            source_port: source.port(),
            destination_port: destination.port(),
            length: length_field,
            checksum: u16::MIN,
        };
        let copied = payload.min(into.len());
        udp.checksum = datagram_receive::udp_checksum(
            source.ip(),
            destination.ip(),
            protocol,
            udp_length,
            &udp.to_bytes(),
            &[&into[..copied], tail],
        );
        headers.push(&udp.to_bytes());
        headers.place(into, copied);
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
}

fn connect(fd: BorrowedFd<'_>, peer: SocketAddr) -> Result<(), HostError> {
    sock(&fd)
        .connect(&SockAddr::from(peer))
        .map_err(|error| io_host_error(&error))
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

impl UdpSockets for HostUdpSockets {
    type Socket = HostUdpSocket;

    fn open(
        &self,
        version: IpVersion,
        protocol: UdpProtocol,
        owner: UserId,
    ) -> Result<Self::Socket, HostError> {
        let ipv6 = version == IpVersion::V6;
        let number = match protocol {
            UdpProtocol::Udp => rustix::net::ipproto::UDP,
            UdpProtocol::Lite => rustix::net::ipproto::UDPLITE,
        };
        let fd = shared::open_socket(version, SocketType::DGRAM, number, owner)?;
        let borrowed = fd.as_fd();
        let host = sock(&borrowed);
        host.set_recv_tos_v4(true)
            .map_err(|error| io_host_error(&error))?;
        for option in IPV4_RECEIVE_OPTIONS {
            set_flag(borrowed, uapi::SOL_IP, option, true)?;
        }
        if ipv6 {
            host.set_recv_hoplimit_v6(true)
                .and_then(|()| host.set_recv_tclass_v6(true))
                .map_err(|error| io_host_error(&error))?;
            for option in IPV6_RECEIVE_OPTIONS {
                set_flag(borrowed, uapi::SOL_IPV6, option, true)?;
            }
        }
        for option in SOCKET_RECEIVE_OPTIONS {
            set_flag(borrowed, uapi::SOL_SOCKET, option, true)?;
        }
        let send = SendState::open(borrowed, ipv6)?;
        Ok(HostUdpSocket {
            watched: self.readiness.watch(fd)?,
            ipv6,
            protocol,
            state: RefCell::new(SocketState {
                send,
                nonlocal: NonlocalAddresses::Refused,
                anycast: AnycastMemberships::default(),
                local: None,
                peer: None,
                drops: u32::MIN,
                errors: ErrorQueue::default(),
                errors_first: false,
                held_errors: false,
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
        if !socket.ipv6 && matches!(local.address, BindAddress::V6 { .. }) {
            return Err(HostError::Errno(libc::EAFNOSUPPORT));
        }
        let address = shared::bind_address(fd, &local.address)?;
        let address = for_family(SocketAddr::new(address, u16::MIN), socket.ipv6).ip();
        match local.port {
            LocalPort::ChosenByExitNode(choice) => {
                shared::bind_chosen_by_exit_node(fd, address, &choice, &self.shortfalls)?;
            }
            LocalPort::Requested(port) | LocalPort::ChosenByKernel(port) => {
                shared::bind_to(fd, SocketAddr::new(address, port.0)).map_err(from_errno)?;
            }
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
        let bound = unmapped(before).ip();
        if source.is_some_and(|source| !bound.is_unspecified() && source != bound) {
            return Err(self.refuse(SOURCE_NOT_THE_BOUND_ADDRESS));
        }
        connect(fd, destination)?;
        let local = local_address(fd)?;
        if let Some(source) = source
            && unmapped(local).ip() != source
        {
            // Back to the peer it had, or to none; a port the kernel chose and Linux gave up is taken again.
            match state.peer {
                Some(peer) => connect(fd, peer)?,
                None => rustix::net::connect_unspec(fd).map_err(from_errno)?,
            }
            if local_address(fd)?.port() != before.port() {
                shared::bind_to(fd, before).map_err(from_errno)?;
            }
            state.local = Some(local_address(fd)?);
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
        let mut state = socket.state.borrow_mut();
        let sent = self.send_now(socket, &mut state, header, fields, datagram);
        socket.note_kept_entries(&mut state);
        sent
    }

    fn receive(&self, socket: &Self::Socket, into: &mut [u8]) -> Result<UdpReceived, HostError> {
        let mut state = socket.state.borrow_mut();
        let received = self.receive_now(socket, &mut state, into);
        socket.note_kept_entries(&mut state);
        received
    }

    fn count_unsent_bytes(&self, socket: &Self::Socket) -> Result<ByteCount, HostError> {
        let mut unsent: c_int = c_int::default();
        // SAFETY: SIOCOUTQ (TIOCOUTQ) writes one int through the pointer.
        let result = unsafe { libc::ioctl(socket.fd().as_raw_fd(), libc::TIOCOUTQ, &mut unsent) };
        if result < 0 {
            return Err(last_host_error());
        }
        Ok(ByteCount(unsent as usize))
    }

    fn read_incoming_processor(
        &self,
        socket: &Self::Socket,
    ) -> Result<Option<ProcessorIndex>, HostError> {
        // Linux's -1, no processor, is beyond every processor's number.
        let processor = sock(&socket.fd())
            .cpu_affinity()
            .map_err(|error| io_host_error(&error))?;
        Ok(u32::try_from(processor).ok().map(ProcessorIndex))
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

impl HostUdpSockets {
    fn send_now(
        &self,
        socket: &HostUdpSocket,
        state: &mut SocketState,
        header: IpHeaderFields<'_>,
        fields: IpSendFields,
        datagram: UdpDatagram<'_>,
    ) -> Result<(), SendError> {
        let fd = socket.fd();
        let (upper_layer, ipv4) = match &header {
            IpHeaderFields::V4(header) => (header.route.upper_layer, true),
            // Linux's answer to an IPv4 socket's send to an address of another family.
            IpHeaderFields::V6(_) if !socket.ipv6 => {
                return Err(SendError::Host(HostError::Errno(libc::EAFNOSUPPORT)));
            }
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
        let sent = send_message(fd, name, datagram.payload, control.bytes(), flags);
        // A transmit time Linux rejects at once fails the send with ENOBUFS and queues its rejection.
        if let Some(time) = fields.transmit_time
            && time.errors == Report::Requested
            && matches!(sent, Ok(()) | Err(HostError::Errno(libc::ENOBUFS)))
        {
            let reports = &fields.transmit_reports;
            state.errors.expect_transmit_time_errors(
                u64::try_from(time.at.as_nanos()).unwrap_or(u64::MAX),
                reports.key,
                reports.packet == Report::Requested,
            );
            // Forgetting the sends that can no longer be rejected is tried again at the next receive.
            let _ = state.errors.settle(fd, &self.shortfalls);
            state.errors_first = true;
        }
        match sent {
            Ok(()) => {
                if state.local.is_some_and(|local| local.port() == u16::MIN) {
                    state.local = None;
                }
                let reports = fields.transmit_reports;
                state.errors_first |= [
                    reports.scheduled,
                    reports.software,
                    reports.hardware,
                    reports.acknowledgement,
                    reports.completion,
                ]
                .contains(&Report::Requested);
                Ok(())
            }
            Err(HostError::Errno(libc::EMSGSIZE)) => {
                let error = state.errors.message_too_long(fd, &self.shortfalls);
                state.errors_first |= state.errors.holds_entries();
                Err(error)
            }
            Err(error) => Err(SendError::Host(error)),
        }
    }

    fn receive_now(
        &self,
        socket: &HostUdpSocket,
        state: &mut SocketState,
        into: &mut [u8],
    ) -> Result<UdpReceived, HostError> {
        let fd = socket.fd();
        // Reports a send asked for come before the datagrams that arrive after them.
        if state.errors_first {
            let settled = state.errors.settle(fd, &self.shortfalls)?;
            match socket.take_error_entry(self, state, into) {
                Err(HostError::WouldBlock) => state.errors_first = !settled,
                taken => return taken,
            }
        }
        // What does not fit `into` lands here, so that the checksum covers the whole payload.
        let mut tail = [const { MaybeUninit::<u8>::uninit() }; LARGEST_PAYLOAD];
        let mut control = ControlBuffer::new();
        match datagram_receive::receive_with_tail(fd, into, &mut tail, &mut control, false) {
            Ok(message) => {
                if message.control_truncated {
                    self.count(datagram_receive::CONTROL_TRUNCATED);
                }
                let beyond = message.length.saturating_sub(into.len()).min(tail.len());
                // SAFETY: the kernel wrote the first `beyond` bytes of the tail.
                let tail =
                    unsafe { core::slice::from_raw_parts(tail.as_ptr().cast::<u8>(), beyond) };
                socket.datagram(state, into, tail, &message)
            }
            // A pending socket error signals an ICMP error; a send may have taken that signal first.
            Err(error) => match socket.take_error_entry(self, state, into) {
                Err(HostError::WouldBlock) => Err(error),
                taken => taken,
            },
        }
    }
}

impl<'a> Configurable<HostUdpSocket, UdpSocketSetting<'a>> for HostUdpSockets {
    fn configure(
        &self,
        socket: &HostUdpSocket,
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
            UdpSocketSetting::Ip(
                setting @ (IpSocketSetting::SendBuffer(bytes)
                | IpSocketSetting::ReceiveBuffer(bytes)),
            ) => {
                shared::set_ip_socket_setting(
                    fd,
                    socket.version(),
                    &setting,
                    u32::MIN,
                    &self.shortfalls,
                )?;
                let (kept, reason) = match setting {
                    IpSocketSetting::SendBuffer(_) => {
                        (sock(&fd).send_buffer_size(), SEND_BUFFER_CUT_BY_THE_HOST)
                    }
                    _ => (sock(&fd).recv_buffer_size(), RECEIVE_BUFFER_CUT_BY_THE_HOST),
                };
                // Linux keeps twice the size asked for, or twice its own limit when that is less.
                if kept.map_err(|error| io_host_error(&error))? < bytes.0.saturating_add(bytes.0) {
                    self.count(reason);
                }
                Ok(())
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
                // A coverage of the UDP header alone takes every datagram Linux finds valid.
                coverage.map_or(UdpHeader::LEN as c_int, |coverage| {
                    c_int::from(coverage.0.get())
                }),
            ),
        }
    }
}

impl NegativeRouteAdvice<HostUdpSocket> for HostUdpSockets {
    fn give_negative_route_advice(&self, socket: &HostUdpSocket) -> Result<(), HostError> {
        // Linux has no call for it: connecting again drops the cached route and takes a new flow hash.
        let peer = socket.state.borrow().peer;
        match peer {
            Some(peer) => connect(socket.fd(), peer),
            None => Ok(()),
        }
    }
}
