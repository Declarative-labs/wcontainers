#![cfg(target_os = "linux")]

use core::cell::RefCell;
use core::cmp::Ordering;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};

use etherparse::IpNumber;
use linux_raw_sys::net as uapi;
use rustix::net::{SocketType, ipproto};
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_configurable::Configurable;
use wcontainers_icmp_sockets::{EchoIdentifier, IcmpSockets, IdentifierSharing};
use wcontainers_internet_protocol::{
    ByteCount, IcmpCode, IcmpType, IpHeaderFields, IpSendFields, IpSocketSetting, IpVersion,
    NeighbourConfirmation, Received, Report, SendError, UpperLayer, UpperLayerSelector,
};
use wcontainers_internet_protocol_linux_socket_options as shared;
use wcontainers_kernel::UserId;
use wcontainers_kernel::signal_information::PollEvents;

use shared::datagram::addresses::local_address;
use shared::datagram::exchange::{ErrorQueue, dropped_packets, send_message, unsent_bytes};
use shared::datagram::memberships::{AnycastMemberships, set_multicast_group};
use shared::datagram::receive::{self, Ancillary, ControlBuffer, Headers};
use shared::datagram::send::{Control, Refusal, SendState};
use shared::readiness::{NO_EVENTS, Readiness, Watched};
use shared::{Shortfalls, host_error, io_host_error, set_flag, sock};

/// ICMP echo sockets of the Linux machine this runs on, its ping sockets, with a count of each refusal by its reason.
pub struct HostIcmpSockets {
    shortfalls: Shortfalls,
    fields_not_given: PacketCount,
    drops_asked: PacketCount,
    readiness: Readiness,
}

/// A count every received packet adds to without a lock, warned of at its first.
struct PacketCount {
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
            detail: 0,
            count,
            nanoseconds: 0,
        })
    }
}

pub struct HostIcmpSocket {
    watched: Watched,
    ipv6: bool,
    state: RefCell<SocketState>,
}

struct SocketState {
    send: SendState,
    anycast: AnycastMemberships,
    errors: ErrorQueue,
    /// Whether the socket's readiness holds an error for the entries kept off the host's error queue.
    held_errors: bool,
    drops: u32,
}

pub const IPV6_SOURCE: &str =
    "linux's ipv6 ping sockets send from their bound address, not from one a send gives";
pub const IPV6_EXTENSION_HEADERS: &str =
    "linux's ipv6 ping sockets drop the extension headers and routing header of a send";
pub const IPV6_NEIGHBOUR_CONFIRMATION: &str =
    "linux's ipv6 ping sockets do not pass on a send's neighbour confirmation";
pub const UPPER_LAYER: &str =
    "an upper layer other than the type and code of the message, which linux routes a ping by";
pub const RECEIVED_FIELDS_NOT_GIVEN: &str = "linux gives a ping socket a packet without its mark, priority or preferred \
     source, and an ipv4 one without its identification and fragment flags: each is written as zero or none";
pub const DROPS_ASKED: &str = "linux gives a ping socket's packets no drop count, so each receive asks the socket \
     for its count, one more system call";
pub const ICMP_ERROR_FIELDS_NOT_GIVEN: &str = "linux gives a ping socket an icmp error without the identification and \
     fragment flags of an ipv4 one or the quoted header's fields but its addresses: each is written as zero";

/// The largest ICMP message an IP packet carries, the limit of its payload length.
const LARGEST_MESSAGE: usize = u16::MAX as usize;

/// The receive control messages every socket turns on beside the ones socket2 sets, by level.
const IPV4_RECEIVE_OPTIONS: [u32; 5] = [
    uapi::IP_PKTINFO,
    uapi::IP_RECVTTL,
    uapi::IP_RECVOPTS,
    uapi::IP_RECVFRAGSIZE,
    uapi::IP_RECVERR,
];
const IPV6_RECEIVE_OPTIONS: [u32; 7] = [
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
            readiness: Readiness::new()?,
        })
    }

    /// How many times each named reason has made this value refuse, drop, leave out or slow down something.
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

    fn protocol(&self) -> IpNumber {
        if self.ipv6 {
            IpNumber::IPV6_ICMP
        } else {
            IpNumber::ICMP
        }
    }

    /// The ICMP error of an error queue entry, written whole from its IP header.
    fn icmp_error(
        &self,
        into: &mut [u8],
        copied: usize,
        ancillary: &Ancillary<'_>,
        error: &libc::sock_extended_err,
        offender: Option<SocketAddr>,
        destination: Option<SocketAddr>,
    ) -> Option<usize> {
        let local = local_address(self.fd()).ok()?.ip();
        // The quoted ICMP header is the start of what Linux gives, so no part of it was taken off.
        shared::icmp_error::write_icmp_error(
            into,
            copied,
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
            for option in IPV6_RECEIVE_OPTIONS {
                set_flag(borrowed, uapi::SOL_IPV6, option, true)?;
            }
        } else {
            socket2
                .set_recv_tos_v4(true)
                .map_err(|error| io_host_error(&error))?;
            for option in IPV4_RECEIVE_OPTIONS {
                set_flag(borrowed, uapi::SOL_IP, option, true)?;
            }
        }
        let send = SendState::open(borrowed, ipv6)?;
        Ok(HostIcmpSocket {
            watched: self.readiness.watch(fd)?,
            ipv6,
            state: RefCell::new(SocketState {
                send,
                anycast: AnycastMemberships::default(),
                errors: ErrorQueue::default(),
                held_errors: false,
                drops: u32::MIN,
            }),
        })
    }

    fn bind_identifier(
        &self,
        socket: &Self::Socket,
        identifier: EchoIdentifier,
        sharing: IdentifierSharing,
    ) -> Result<(), HostError> {
        let fd = socket.fd();
        sock(&fd)
            .set_reuse_address(sharing == IdentifierSharing::Shared)
            .map_err(|error| io_host_error(&error))?;
        let any = if socket.ipv6 {
            IpAddr::V6(Ipv6Addr::UNSPECIFIED)
        } else {
            IpAddr::V4(Ipv4Addr::UNSPECIFIED)
        };
        shared::bind_to(fd, SocketAddr::new(any, identifier.0.get())).map_err(host_error)
    }

    fn send_to(
        &self,
        socket: &Self::Socket,
        header: IpHeaderFields<'_>,
        fields: IpSendFields,
        message: &[u8],
    ) -> Result<(), SendError> {
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
                header.route.upper_layer
            }
        };
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
        }
        match sent {
            Ok(()) => Ok(()),
            Err(HostError::Errno(libc::EMSGSIZE)) => {
                let error = state.errors.message_too_long(fd, &self.shortfalls);
                socket.note_kept_entries(state);
                Err(error)
            }
            Err(error) => Err(SendError::Host(error)),
        }
    }

    fn receive(&self, socket: &Self::Socket, into: &mut [u8]) -> Result<Received, HostError> {
        let fd = socket.fd();
        let mut state = socket.state.borrow_mut();
        let state = &mut *state;
        loop {
            let next = state.errors.next(fd, &self.shortfalls);
            socket.note_kept_entries(state);
            let entry = match next {
                Ok(entry) => entry,
                Err(HostError::WouldBlock) => break,
                Err(error) => return Err(error),
            };
            let received = state.errors.received(
                &self.shortfalls,
                &entry,
                into,
                |into, copied, ancillary, error, offender| {
                    let written =
                        socket.icmp_error(into, copied, ancillary, error, offender, entry.source);
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
        let mut control = ControlBuffer::new();
        // Linux cuts a ping socket's packet to the buffers given and tells only what it copied.
        let mut tail = [MaybeUninit::<u8>::uninit(); LARGEST_MESSAGE];
        let tail = &mut tail[..LARGEST_MESSAGE.saturating_sub(into.len())];
        let message = receive::receive_with_tail(fd, into, tail, &mut control, false)?;
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
        let mut fields = ancillary.packet_fields(headers.len() + message.length, drops);
        // Linux prepares no preferred source for a ping socket and gives the unspecified address in its place.
        if fields.specific_destination == Some(Ipv4Addr::UNSPECIFIED) {
            fields.specific_destination = None;
        }
        self.fields_not_given.add();
        Ok(Received::Packet(fields))
    }

    fn count_unsent_bytes(&self, socket: &Self::Socket) -> Result<ByteCount, HostError> {
        unsent_bytes(socket.fd())
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
