use core::cell::RefCell;
use core::cmp::Ordering;
use core::num::{NonZeroU16, NonZeroU32};
use core::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};

use etherparse::IpNumber;
use linux_raw_sys::net as uapi;
use rustix::net::{SocketType, ipproto};
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_byte_sink::ByteSink;
use wcontainers_byte_source::ByteSource;
use wcontainers_configurable::Configurable;
use wcontainers_icmp_sockets::{
    EchoBindAddress, EchoBinding, EchoIdentifier, IcmpSockets, IdentifierSharing, LocalIdentifier,
};
use wcontainers_internet_protocol::{
    BufferSizes, ByteCount, IcmpCode, IcmpType, InterfaceIndex, InterfaceSelector, IpHeaderFields,
    IpSendFields, IpSocketSetting, IpVersion, Ipv6RouteSelectors, NeighbourConfirmation, Received,
    Report, RouteSelectors, SendError, UpperLayer, UpperLayerSelector,
};
use wcontainers_internet_protocol_linux_socket_options as shared;
use wcontainers_kernel::UserId;
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor, UnconditionalEvents};
use wcontainers_route_resolver::RouteResolver;
use wcontainers_route_resolver_host::HostRouteResolver;
use wcontainers_volatile_bytes::VolatileBytes;

use crate::{HostIcmpSocket, HostIcmpSockets};
use shared::datagram::addresses::local_address;
use shared::datagram::exchange::{
    ErrorQueue, buffer_sizes, dropped_packets, note_packet_wish, receive_in_order,
    reports_requested, send_message, unsent_bytes,
};
use shared::datagram::memberships::{AnycastMemberships, set_multicast_group};
use shared::datagram::receive::{self, Ancillary, ControlBuffer, Headers};
use shared::datagram::send::{Control, Refusal, SendState};
use shared::readiness::{DATAGRAM_FACTS, Watcher};
use shared::{Shortfalls, host_error, io_host_error, set_flag, sock};

/// A count every received packet adds to without a lock, warned of at its first.
pub(crate) struct PacketCount {
    reason: &'static str,
    count: AtomicU64,
}

impl PacketCount {
    const fn new(reason: &'static str) -> Self {
        Self {
            reason,
            count: AtomicU64::new(u64::MIN),
        }
    }

    fn add(&self) {
        if self.count.fetch_add(1, Relaxed) == u64::MIN {
            eprintln!("warning: {}", self.reason);
        }
    }

    fn reading(&self) -> Option<CounterReading> {
        let count = self.count.load(Relaxed);
        (count != u64::MIN).then_some(CounterReading {
            reason: self.reason,
            // One counter for every packet, which no time is measured for.
            detail: u64::MIN,
            count,
            nanoseconds: u64::MIN,
        })
    }
}

pub(crate) struct SocketState {
    send: SendState,
    anycast: AnycastMemberships,
    errors: ErrorQueue,
    /// Whether the socket's readiness holds an error for the entries kept off the host's error queue.
    held_errors: bool,
    drops: u32,
    owner: UserId,
    /// The address an IPv6 socket is bound to, while it is one and no link-local scope binds the socket to its link.
    bound_source: Option<Ipv6Addr>,
    /// The link a link-local address's scope binds an IPv6 socket to.
    bound_link: Option<InterfaceIndex>,
}

const BOUND_ROUTE_ASKED: &str = "an ipv6 ping from a bound address by a bound interface, which linux's lookup with \
     a source only prefers: the routing socket is asked whether it leaves by that interface";
const BOUND_WITH_SOURCE: &str = "an ipv6 ping from a bound address by a bound interface, whose lookup by \
     preference leaves by another interface than a socket bound to it would";
const IPV6_SOURCE: &str =
    "linux's ipv6 ping sockets send from their bound address, not from one a send gives";
const IPV6_EXTENSION_HEADERS: &str =
    "linux's ipv6 ping sockets drop the extension headers and routing header of a send";
const IPV6_NEIGHBOUR_CONFIRMATION: &str =
    "linux's ipv6 ping sockets do not pass on a send's neighbour confirmation";
const UPPER_LAYER: &str =
    "an upper layer other than the type and code of the message, which linux routes a ping by";
const RECEIVED_FIELDS_NOT_GIVEN: &str = "linux gives a ping socket a packet without its mark, priority or preferred \
     source, and an ipv4 one without its identification and fragment flags: each is written as zero or none";
const DROPS_ASKED: &str = "linux gives a ping socket's packets no drop count, so each receive asks the socket \
     for its count, one more system call, and tells every drop so far with the packet, even those after it came";
const ICMP_ERROR_FIELDS_NOT_GIVEN: &str = "linux gives a ping socket an icmp error without the identification and \
     fragment flags of an ipv4 one or the quoted header's fields but its addresses: each is written as zero";

/// The largest ICMP message an IP packet carries, the limit of its payload length.
const LARGEST_MESSAGE: usize = u16::MAX as usize;

/// The receive control messages every socket turns on beside the ones socket2 sets, by level.
const IPV4_RECEIVE_OPTIONS: &[u32] = &[
    uapi::IP_PKTINFO,
    uapi::IP_RECVTTL,
    uapi::IP_RECVOPTS,
    uapi::IP_RECVFRAGSIZE,
    uapi::IP_RECVERR,
];
const IPV6_RECEIVE_OPTIONS: &[u32] = &[
    uapi::IPV6_RECVPKTINFO,
    uapi::IPV6_RECVHOPOPTS,
    uapi::IPV6_RECVDSTOPTS,
    uapi::IPV6_RECVRTHDR,
    uapi::IPV6_FLOWINFO,
    uapi::IPV6_RECVFRAGSIZE,
    uapi::IPV6_RECVERR,
];

impl HostIcmpSockets {
    pub fn new() -> Result<Self, HostError> {
        Ok(Self {
            shortfalls: Shortfalls::default(),
            fields_not_given: PacketCount::new(RECEIVED_FIELDS_NOT_GIVEN),
            drops_asked: PacketCount::new(DROPS_ASKED),
            watcher: Watcher::new()?,
            routes: HostRouteResolver::open(),
        })
    }

    /// Returns how many times each named reason has made this value refuse, drop, leave out or slow down something.
    pub fn counters(&self) -> Vec<CounterReading> {
        let mut readings = self.shortfalls.readings();
        readings.extend(
            [&self.fields_not_given, &self.drops_asked]
                .into_iter()
                .filter_map(PacketCount::reading),
        );
        readings.sort_by_key(|reading| reading.reason);
        readings
    }

    fn refuse(&self, reason: &'static str) -> SendError {
        SendError::Host(self.shortfalls.refuse(reason))
    }

    /// Holds an IPv6 ping from `source` to the bound interface, which Linux's lookup with a source only prefers.
    ///
    /// The ping is sent where that lookup leaves by the interface anyway; else the bound lookup's error answers, or a refusal.
    fn hold_to_bound(
        &self,
        route: &Ipv6RouteSelectors<'_>,
        source: Ipv6Addr,
        bound: InterfaceIndex,
        owner: UserId,
        neighbour: NeighbourConfirmation,
    ) -> Result<(), SendError> {
        self.shortfalls.count(BOUND_ROUTE_ASKED);
        let route = Ipv6RouteSelectors {
            source: Some(source),
            ..route.clone()
        };
        match self
            .routes
            .resolve_route(RouteSelectors::V6(route), owner, neighbour)
        {
            Ok(found) if found.interface == bound => Ok(()),
            Ok(_) | Err(HostError::NotSupported) => Err(self.refuse(BOUND_WITH_SOURCE)),
            Err(error) => Err(SendError::Host(error)),
        }
    }
}

impl PartialEq for HostIcmpSocket {
    fn eq(&self, other: &Self) -> bool {
        self.fd().as_raw_fd() == other.fd().as_raw_fd()
    }
}

impl Eq for HostIcmpSocket {}

impl PartialOrd for HostIcmpSocket {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HostIcmpSocket {
    fn cmp(&self, other: &Self) -> Ordering {
        self.fd().as_raw_fd().cmp(&other.fd().as_raw_fd())
    }
}

impl HostIcmpSocket {
    fn fd(&self) -> BorrowedFd<'_> {
        self.watched.fd()
    }

    /// Holds an error in the socket's readiness while entries taken off the host's error queue wait for receives.
    fn note_kept_entries(&self, state: &mut SocketState) {
        let holds = state.errors.holds_entries();
        if holds != state.held_errors {
            state.held_errors = holds;
            self.watched.hold(ReadinessReport {
                ready_for: ReadyFor::empty(),
                unconditional: if holds {
                    UnconditionalEvents::ERROR
                } else {
                    UnconditionalEvents::empty()
                },
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

    fn protocol(&self) -> IpNumber {
        if self.ipv6 {
            IpNumber::IPV6_ICMP
        } else {
            IpNumber::ICMP
        }
    }

    /// Writes the ICMP error of an error queue entry quoting `data` whole from its IP header, and returns its length.
    fn icmp_error(
        &self,
        into: &mut [u8],
        data: &[u8],
        ancillary: &Ancillary<'_>,
        error: &libc::sock_extended_err,
        offender: Option<SocketAddr>,
        destination: Option<SocketAddr>,
    ) -> Option<usize> {
        let local = local_address(self.fd()).ok()?.ip();
        // The quoted ICMP header is the start of what Linux gives, so no part of it was taken off.
        shared::icmp_error::write_whole_icmp_error(
            into,
            data,
            ancillary,
            error,
            offender,
            local,
            destination?.ip(),
            self.protocol(),
            &[],
        )
    }
}

/// Answers whether the upper layer of a send's route is the message's ICMP type and code under the socket's protocol.
fn routes_by_the_message(upper_layer: &UpperLayer, protocol: IpNumber, message: &[u8]) -> bool {
    let selector = match message {
        [message_type, code, ..] => Some(UpperLayerSelector::Icmp {
            message_type: IcmpType(*message_type),
            code: IcmpCode(*code),
        }),
        // Linux refuses a message shorter than an ICMP header itself.
        _ => upper_layer.selector,
    };
    upper_layer.protocol == protocol && upper_layer.selector == selector
}

/// One send of the bytes a message writes to it, which notes that it was made.
struct Sending<'a> {
    sockets: &'a HostIcmpSockets,
    socket: &'a HostIcmpSocket,
    header: IpHeaderFields<'a>,
    fields: IpSendFields,
    sent: bool,
}

impl ByteSink for Sending<'_> {
    type Error = SendError;

    fn write(&mut self, bytes: &[u8]) -> Result<usize, SendError> {
        self.sent = true;
        self.sockets
            .send_bytes(self.socket, self.header.clone(), self.fields, bytes)
            .map(|()| bytes.len())
    }
}

/// One receive into the bytes it is given to read into, which keeps what came with them.
struct Receiving<'a> {
    sockets: &'a HostIcmpSockets,
    socket: &'a HostIcmpSocket,
    received: Option<Received>,
}

impl ByteSource for Receiving<'_> {
    type Error = HostError;

    fn read(&mut self, into: &mut [u8]) -> Result<usize, HostError> {
        let received = self.sockets.receive_bytes(self.socket, into)?;
        self.received = Some(received);
        Ok(written_length(&received, into.len()))
    }
}

/// Returns the number of bytes a receive wrote into a buffer of `room` bytes, the start of what it tells the length of.
fn written_length(received: &Received, room: usize) -> usize {
    let whole = match received {
        Received::Packet(fields) => fields.length,
        Received::TransmitReport(report) => {
            report.packet_length.map_or(usize::MIN, |length| length.0)
        }
    };
    whole.min(room)
}

impl IcmpSockets for HostIcmpSockets {
    type Socket = HostIcmpSocket;

    fn open(&self, version: IpVersion, owner: UserId) -> Result<Self::Socket, HostError> {
        let ipv6 = version == IpVersion::V6;
        let protocol = if ipv6 { ipproto::ICMPV6 } else { ipproto::ICMP };
        let fd = shared::open_socket(version, SocketType::DGRAM, protocol, owner)?;
        let borrowed = fd.as_fd();
        let socket2 = sock(&borrowed);
        if ipv6 {
            socket2
                .set_recv_hoplimit_v6(true)
                .and_then(|()| socket2.set_recv_tclass_v6(true))
                .map_err(|error| io_host_error(&error))?;
            for &option in IPV6_RECEIVE_OPTIONS {
                set_flag(borrowed, uapi::SOL_IPV6, option, true)?;
            }
        } else {
            socket2
                .set_recv_tos_v4(true)
                .map_err(|error| io_host_error(&error))?;
            for &option in IPV4_RECEIVE_OPTIONS {
                set_flag(borrowed, uapi::SOL_IP, option, true)?;
            }
        }
        let send = SendState::open(borrowed, ipv6)?;
        Ok(HostIcmpSocket {
            watched: self.watcher.watch(fd, DATAGRAM_FACTS)?,
            ipv6,
            state: RefCell::new(SocketState {
                send,
                anycast: AnycastMemberships::default(),
                errors: ErrorQueue::default(),
                held_errors: false,
                drops: u32::MIN,
                owner,
                bound_source: None,
                bound_link: None,
            }),
        })
    }

    fn bind(&self, socket: &Self::Socket, local: EchoBinding) -> Result<EchoIdentifier, HostError> {
        let fd = socket.fd();
        sock(&fd)
            .set_reuse_address(local.sharing == IdentifierSharing::Shared)
            .map_err(|error| io_host_error(&error))?;
        // A ping socket's port is its identifier, and Linux chooses one for port zero.
        let port = match local.identifier {
            LocalIdentifier::Requested(identifier) => identifier.0.get(),
            LocalIdentifier::ChosenByExitNode => u16::MIN,
        };
        // Linux answers an address of the other version by its family and length, as given.
        let address = match local.address {
            EchoBindAddress::V4(address) => SocketAddr::V4(SocketAddrV4::new(address, port)),
            EchoBindAddress::V6 { address, scope } => SocketAddr::V6(SocketAddrV6::new(
                address,
                port,
                u32::MIN,
                scope.map_or(u32::MIN, |index| index.0.get()),
            )),
        };
        shared::bind_to(fd, address).map_err(host_error)?;
        let bound = local_address(fd)?;
        if let SocketAddr::V6(bound) = bound {
            // Linux gives a scope only to a link-local address, whose link the bind holds the socket to.
            let link = NonZeroU32::new(bound.scope_id()).map(InterfaceIndex);
            let mut state = socket.state.borrow_mut();
            state.bound_link = link;
            state.bound_source =
                (!bound.ip().is_unspecified() && link.is_none()).then_some(*bound.ip());
        }
        NonZeroU16::new(bound.port())
            .map(EchoIdentifier)
            .ok_or(HostError::Errno(libc::EPROTO))
    }

    fn send_to(
        &self,
        socket: &Self::Socket,
        header: IpHeaderFields<'_>,
        fields: IpSendFields,
        message: &impl VolatileBytes,
    ) -> Result<(), SendError> {
        let mut sending = Sending {
            sockets: self,
            socket,
            header,
            fields,
            sent: false,
        };
        message.read_to(&mut sending)?;
        if sending.sent {
            Ok(())
        } else {
            // Bytes with nothing to reach may never write to the sink, and the empty message is sent here instead.
            self.send_bytes(socket, sending.header, fields, &[])
        }
    }

    fn receive(
        &self,
        socket: &Self::Socket,
        into: &impl VolatileBytes,
    ) -> Result<Received, HostError> {
        let mut receiving = Receiving {
            sockets: self,
            socket,
            received: None,
        };
        into.write_from(&mut receiving)?;
        // Bytes with nothing to reach may never read from the source, and the packet is taken here instead.
        receiving
            .received
            .map_or_else(|| self.receive_bytes(socket, &mut []), Ok)
    }

    fn count_unsent_bytes(&self, socket: &Self::Socket) -> Result<ByteCount, HostError> {
        unsent_bytes(socket.fd())
    }

    fn read_buffer_sizes(&self, socket: &Self::Socket) -> Result<BufferSizes, HostError> {
        buffer_sizes(socket.fd())
    }

    fn close(&self, socket: Self::Socket) {
        drop(socket);
    }
}

impl Readiness<HostIcmpSocket> for HostIcmpSockets {
    async fn wait_until_ready(
        &self,
        socket: &HostIcmpSocket,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        socket.watched.wait_until_ready(interest).await
    }
}

impl HostIcmpSockets {
    /// Sends the message's bytes as `send_to` sends the bytes it is given.
    fn send_bytes(
        &self,
        socket: &HostIcmpSocket,
        mut header: IpHeaderFields<'_>,
        fields: IpSendFields,
        message: &[u8],
    ) -> Result<(), SendError> {
        // Linux answers an address of the other version by the address alone, which it is given as its family takes it.
        let other_version = match &header {
            IpHeaderFields::V4(header) if socket.ipv6 => {
                Some(IpAddr::V6(header.route.destination.to_ipv6_mapped()))
            }
            IpHeaderFields::V6(header) if !socket.ipv6 => {
                Some(IpAddr::V6(header.route.destination))
            }
            _ => None,
        };
        if let Some(destination) = other_version {
            let flags = libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL;
            let destination = SocketAddr::new(destination, u16::MIN);
            return send_message(socket.fd(), Some(destination), message, &[], flags)
                .map_err(SendError::Host);
        }
        let upper_layer = match &header {
            IpHeaderFields::V4(header) => header.route.upper_layer,
            IpHeaderFields::V6(header) => {
                if header.route.source.is_some() {
                    return Err(self.refuse(IPV6_SOURCE));
                }
                if header.route.routing_header.is_some()
                    || header.hop_by_hop_options.is_some()
                    || header.routing_destination_options.is_some()
                    || header.destination_options.is_some()
                {
                    return Err(self.refuse(IPV6_EXTENSION_HEADERS));
                }
                if fields.neighbour == NeighbourConfirmation::Confirmed {
                    return Err(self.refuse(IPV6_NEIGHBOUR_CONFIRMATION));
                }
                if let Some(InterfaceSelector::Bound(bound)) = header.route.interface {
                    let (source, owner) = {
                        let state = socket.state.borrow();
                        (state.bound_source, state.owner)
                    };
                    if let Some(source) = source {
                        self.hold_to_bound(&header.route, source, bound, owner, fields.neighbour)?;
                    }
                }
                header.route.upper_layer
            }
        };
        // Linux refuses IPV6_UNICAST_IF on a socket bound to a link, which holds every send to that link anyway.
        if let IpHeaderFields::V6(header) = &mut header
            && let Some(InterfaceSelector::Bound(index) | InterfaceSelector::Requested(index)) =
                header.route.interface
            && socket.state.borrow().bound_link == Some(index)
        {
            header.route.interface = None;
        }
        if !routes_by_the_message(&upper_layer, socket.protocol(), message) {
            return Err(self.refuse(UPPER_LAYER));
        }
        let fd = socket.fd();
        let mut state = socket.state.borrow_mut();
        let state = &mut *state;
        let mut control = Control::new();
        // A ping takes no port: its identifier is the socket's.
        let destination = state
            .send
            .apply_header(fd, &mut control, &header, u16::MIN, socket.ipv6)
            .map_err(SendError::Host)?;
        note_packet_wish(
            fd,
            &self.shortfalls,
            state.send.reports_hold_the_packet(),
            &fields.transmit_reports,
        );
        let flags = match state.send.apply_send_fields(fd, &mut control, &fields) {
            Ok(flags) => flags,
            Err(Refusal::Host(error)) => return Err(SendError::Host(error)),
            Err(Refusal::Unsupported(reason)) => return Err(self.refuse(reason)),
        };
        let sent = send_message(fd, Some(destination), message, control.bytes(), flags);
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
            // Forgetting the sends that can no longer be rejected is tried again at the next such send.
            let _ = state.errors.settle(fd, &self.shortfalls);
            socket.note_kept_entries(state);
            state.errors.read_first();
        }
        match sent {
            Ok(()) => {
                if reports_requested(&fields.transmit_reports) {
                    state.errors.read_first();
                }
                Ok(())
            }
            Err(HostError::Errno(libc::EMSGSIZE)) => {
                let error = state.errors.message_too_long(fd, &self.shortfalls);
                socket.note_kept_entries(state);
                Err(error)
            }
            Err(HostError::WouldBlock) => Err(SendError::Host(HostError::WouldBlock)),
            // Linux may fail a send with the socket error an ICMP error set, which a receive looks for.
            Err(error) => {
                state.errors.keep_host_entries(fd, &self.shortfalls);
                socket.note_kept_entries(state);
                Err(SendError::Host(error))
            }
        }
    }

    /// Receives into the buffer as `receive` receives into the bytes it is given.
    fn receive_bytes(
        &self,
        socket: &HostIcmpSocket,
        into: &mut [u8],
    ) -> Result<Received, HostError> {
        let mut state = socket.state.borrow_mut();
        receive_in_order(
            socket.fd(),
            &mut *state,
            &self.shortfalls,
            into,
            |state| &mut state.errors,
            |state, into| self.take_error_entry(socket, state, into),
            |state, into| self.receive_packet(socket, state, into),
        )
    }

    /// Takes the next ICMP error or transmit report of the error queue, kept entries first, past entries that give none.
    fn take_error_entry(
        &self,
        socket: &HostIcmpSocket,
        state: &mut SocketState,
        into: &mut [u8],
    ) -> Result<Received, HostError> {
        let fd = socket.fd();
        loop {
            let next = state.errors.next(fd, &self.shortfalls);
            socket.note_kept_entries(state);
            let entry = next?;
            let received = state.errors.received(
                &self.shortfalls,
                &entry,
                into,
                |into, _, ancillary, error, offender| {
                    let written = socket.icmp_error(
                        into,
                        &entry.data,
                        ancillary,
                        error,
                        offender,
                        entry.source,
                    );
                    if written.is_some() {
                        self.shortfalls.count(ICMP_ERROR_FIELDS_NOT_GIVEN);
                    }
                    written
                },
            );
            if let Some(received) = received {
                return Ok(received);
            }
        }
    }

    /// Receives the next echo reply, written whole from its IP header.
    fn receive_packet(
        &self,
        socket: &HostIcmpSocket,
        state: &mut SocketState,
        into: &mut [u8],
    ) -> Result<Received, HostError> {
        let fd = socket.fd();
        let mut control = ControlBuffer::new();
        // Linux cuts a ping socket's packet to the buffers given and tells only what it copied.
        let message = receive::with_tail(|tail| {
            let room = LARGEST_MESSAGE.saturating_sub(into.len()).min(tail.len());
            receive::receive_with_tail(fd, into, &mut tail[..room], &mut control, false)
        })?;
        if message.control_truncated {
            self.shortfalls.count(receive::CONTROL_TRUNCATED);
        }
        let ancillary = Ancillary::parse(message.control);
        let invalid = HostError::Errno(libc::EINVAL);
        let source = message.source.ok_or(invalid)?.ip();
        let destination = ancillary.header_destination.ok_or(invalid)?;
        let mut headers = Headers::new();
        match (source, destination) {
            (IpAddr::V4(from), IpAddr::V4(to)) => {
                let header =
                    receive::ipv4_header(from, to, IpNumber::ICMP, message.length, &ancillary)
                        .ok_or(HostError::Errno(libc::EMSGSIZE))?;
                headers.push(&header.to_bytes());
            }
            (IpAddr::V6(from), IpAddr::V6(to)) => {
                receive::push_ipv6_headers(
                    &mut headers,
                    from,
                    to,
                    IpNumber::IPV6_ICMP,
                    message.length,
                    &ancillary,
                )
                .ok_or(HostError::Errno(libc::EMSGSIZE))?;
            }
            _ => return Err(HostError::Errno(libc::EAFNOSUPPORT)),
        }
        headers.place(into, message.length.min(into.len()));
        self.drops_asked.add();
        let total = dropped_packets(fd)?;
        let drops = total.wrapping_sub(state.drops);
        state.drops = total;
        let mut fields = ancillary.packet_fields(headers.len() + message.length, drops)?;
        // Linux prepares no preferred source for a ping socket and gives the unspecified address in its place.
        if fields.specific_destination == Some(Ipv4Addr::UNSPECIFIED) {
            fields.specific_destination = None;
        }
        self.fields_not_given.add();
        Ok(Received::Packet(fields))
    }
}

impl<'a> Configurable<HostIcmpSocket, IpSocketSetting<'a>> for HostIcmpSockets {
    fn configure(
        &self,
        socket: &HostIcmpSocket,
        setting: IpSocketSetting<'a>,
    ) -> Result<(), HostError> {
        let fd = socket.fd();
        let mut state = socket.state.borrow_mut();
        let state = &mut *state;
        match setting {
            IpSocketSetting::MulticastV4(group) => set_multicast_group(
                fd,
                uapi::SOL_IP,
                IpAddr::V4(group.group),
                &group,
                |source| IpAddr::V4(*source),
            ),
            IpSocketSetting::MulticastV6(group) => set_multicast_group(
                fd,
                uapi::SOL_IPV6,
                IpAddr::V6(group.group),
                &group,
                |source| IpAddr::V6(*source),
            ),
            IpSocketSetting::Anycast(anycast) => state.anycast.set(fd, &anycast),
            IpSocketSetting::Timestamping(timestamping) => {
                state.send.set_receive_timestamps(fd, &timestamping)
            }
            setting => shared::set_ip_socket_setting(
                fd,
                socket.version(),
                &setting,
                u32::MIN,
                &self.shortfalls,
            ),
        }
    }
}
