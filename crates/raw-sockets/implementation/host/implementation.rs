#![cfg(target_os = "linux")]

use core::cell::RefCell;
use core::cmp::Ordering;
use core::num::NonZeroU32;
use std::net::{IpAddr, SocketAddr};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};

use etherparse::IpNumber;
use libc::c_int;
use linux_raw_sys::net as uapi;
use rustix::net::{Protocol, SocketType};
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    ByteCount, IcmpCode, IcmpType, IpHeaderFields, IpSendFields, IpSocketSetting, IpVersion,
    Ipv4HeaderFields, Ipv6HeaderFields, LocalFragmentation, MobilityHeaderType, PathMtuDiscovery,
    Received, Report, SendError, UpperLayer, UpperLayerSelector,
};
use wcontainers_internet_protocol_linux_socket_options as shared;
use wcontainers_kernel::UserId;
use wcontainers_kernel::signal_information::PollEvents;
use wcontainers_raw_sockets::{
    HeaderIncludedPacket, MulticastRouting, OutgoingReassembly, RawPacket, RawSocketSetting,
    RawSockets, RouterAlert, RouterAlertIsolation,
};

use shared::datagram::addresses::local_address;
use shared::datagram::exchange::{
    ErrorQueue, note_packet_wish, receive_in_order, reports_requested, send_message, unsent_bytes,
};
use shared::datagram::memberships::{AnycastMemberships, set_multicast_group};
use shared::datagram::receive::{self, Ancillary, ControlBuffer, Headers};
use shared::datagram::send::{Control, Refusal, SendState};
use shared::readiness::{NO_EVENTS, Readiness, Watched};
use shared::{Shortfalls, io_host_error, set_bytes, set_flag, set_int, sock};

/// Raw IP sockets of the Linux machine this runs on, with a count of each refusal by its reason.
pub struct HostRawSockets {
    shortfalls: Shortfalls,
    readiness: Readiness,
}

pub struct HostRawSocket {
    watched: Watched,
    ipv6: bool,
    protocol: IpNumber,
    state: RefCell<SocketState>,
}

struct SocketState {
    send: SendState,
    anycast: AnycastMemberships,
    errors: ErrorQueue,
    /// Whether the socket's readiness holds an error for the entries kept off the host's error queue.
    held_errors: bool,
    drops: u32,
    /// Whether `IP_HDRINCL` or `IPV6_HDRINCL` is on.
    header_included: bool,
    reassembly: OutgoingReassembly,
    router_alert: RouterAlert,
    multicast_routing: MulticastRouting,
}

pub const UPPER_LAYER: &str = "an upper layer other than the one linux routes a raw send by: the message's ICMP type and code or mobility header type, else none";
pub const OTHER_PROTOCOL: &str =
    "a payload under another protocol than its raw socket's, which linux sends under the socket's";
pub const FORWARDED_BY_ITS_BYTES: &str = "linux does not tell a packet it forwards to an ipv6 router \
     alert socket of ipproto_raw from a local one: taken as forwarded, and given whole, by its bytes";
pub const ICMP_ERROR_FIELDS_NOT_GIVEN: &str = "linux gives a raw socket an icmp error without the identification and \
     fragment flags of an ipv4 one, nor under a payload the quoted header's fields but its addresses: written as zero";

/// `MRT_INIT` of linux/mroute.h, and `MRT6_INIT` of linux/mroute6.h, which neither libc nor linux-raw-sys carries.
const MULTICAST_ROUTING_INIT: u32 = 200;
/// `MRT_DONE` and `MRT6_DONE`, the next request after the first.
const MULTICAST_ROUTING_DONE: u32 = MULTICAST_ROUTING_INIT + 1;

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
const SOCKET_RECEIVE_OPTIONS: [u32; 3] =
    [uapi::SO_RXQ_OVFL, uapi::SO_RCVMARK, uapi::SO_RCVPRIORITY];

impl HostRawSockets {
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

    fn refuse(&self, reason: &'static str) -> SendError {
        SendError::Host(self.shortfalls.refuse(reason))
    }
}

impl PartialEq for HostRawSocket {
    fn eq(&self, other: &Self) -> bool {
        self.fd().as_raw_fd() == other.fd().as_raw_fd()
    }
}

impl Eq for HostRawSocket {}

impl PartialOrd for HostRawSocket {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HostRawSocket {
    fn cmp(&self, other: &Self) -> Ordering {
        self.fd().as_raw_fd().cmp(&other.fd().as_raw_fd())
    }
}

fn raw_protocol() -> IpNumber {
    IpNumber(uapi::IPPROTO_RAW as u8)
}

impl HostRawSocket {
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

    fn include_header(&self, state: &mut SocketState, on: bool) -> Result<(), HostError> {
        if state.header_included != on {
            let fd = self.fd();
            let socket2 = sock(&fd);
            if self.ipv6 {
                socket2.set_header_included_v6(on)
            } else {
                socket2.set_header_included_v4(on)
            }
            .map_err(|error| io_host_error(&error))?;
            state.header_included = on;
        }
        Ok(())
    }

    /// The protocol a payload of this upper layer goes out under: the socket's, or for IPv6 the one asked when the
    /// socket's is `IPPROTO_RAW`.
    fn payload_protocol(&self, upper_layer: &UpperLayer) -> Option<IpNumber> {
        if upper_layer.protocol == self.protocol || (self.ipv6 && self.protocol == raw_protocol()) {
            Some(upper_layer.protocol)
        } else {
            None
        }
    }

    /// The ICMP error of an error queue entry quoting `data`, written whole from its IP header.
    #[allow(clippy::too_many_arguments)]
    fn icmp_error(
        &self,
        header_included: bool,
        into: &mut [u8],
        data: &[u8],
        ancillary: &Ancillary<'_>,
        error: &libc::sock_extended_err,
        offender: Option<SocketAddr>,
        destination: Option<SocketAddr>,
    ) -> Option<usize> {
        let local = local_address(self.fd()).ok()?.ip();
        if header_included {
            // Linux gives a socket that writes its headers the quoted packet from its IP header on.
            shared::icmp_error::write_whole_icmp_error_quoting_header(
                into, data, ancillary, error, offender, local,
            )
        } else {
            shared::icmp_error::write_whole_icmp_error(
                into,
                data,
                ancillary,
                error,
                offender,
                local,
                destination?.ip(),
                self.protocol,
                &[],
            )
        }
    }
}

/// The selector Linux routes a payload by: its ICMP type and code, or for IPv6 its mobility header type, else none.
fn payload_selector(protocol: IpNumber, ipv6: bool, payload: &[u8]) -> Option<UpperLayerSelector> {
    /// The mobility header type follows its next header and length bytes.
    const MOBILITY_HEADER_TYPE: usize = 2;
    let icmp = if ipv6 {
        IpNumber::IPV6_ICMP
    } else {
        IpNumber::ICMP
    };
    if protocol == icmp {
        match payload {
            [message_type, code, ..] => Some(UpperLayerSelector::Icmp {
                message_type: IcmpType(*message_type),
                code: IcmpCode(*code),
            }),
            _ => None,
        }
    } else if ipv6 && protocol == IpNumber::MOBILITY_HEADER {
        payload
            .get(MOBILITY_HEADER_TYPE)
            .map(|kind| UpperLayerSelector::MobilityHeader(MobilityHeaderType(*kind)))
    } else {
        None
    }
}

impl RawSockets for HostRawSockets {
    type Socket = HostRawSocket;

    fn open(
        &self,
        version: IpVersion,
        protocol: IpNumber,
        owner: UserId,
    ) -> Result<Self::Socket, HostError> {
        let ipv6 = version == IpVersion::V6;
        // Linux makes no raw socket of protocol zero.
        let number = NonZeroU32::new(u32::from(protocol.0))
            .ok_or(HostError::Errno(libc::EPROTONOSUPPORT))?;
        let fd = shared::open_socket(version, SocketType::RAW, Protocol::from_raw(number), owner)?;
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
        for option in SOCKET_RECEIVE_OPTIONS {
            set_flag(borrowed, uapi::SOL_SOCKET, option, true)?;
        }
        let send = SendState::open(borrowed, ipv6)?;
        Ok(HostRawSocket {
            watched: self.readiness.watch(fd)?,
            ipv6,
            protocol,
            state: RefCell::new(SocketState {
                send,
                anycast: AnycastMemberships::default(),
                errors: ErrorQueue::default(),
                held_errors: false,
                drops: u32::MIN,
                // Linux writes no header for a socket of IPPROTO_RAW until told otherwise.
                header_included: protocol == raw_protocol(),
                reassembly: OutgoingReassembly::Allowed,
                router_alert: RouterAlert::Off,
                multicast_routing: MulticastRouting::Disabled,
            }),
        })
    }

    fn send_to(
        &self,
        socket: &Self::Socket,
        packet: RawPacket<'_>,
        fields: IpSendFields,
    ) -> Result<(), SendError> {
        let fd = socket.fd();
        let mut state = socket.state.borrow_mut();
        let state = &mut *state;
        let mut control = Control::new();
        let invalid = SendError::Host(HostError::Errno(libc::EINVAL));
        let (destination, bytes) = match packet {
            RawPacket::Payload { header, payload } => {
                let upper_layer = match &header {
                    IpHeaderFields::V4(header) => header.route.upper_layer,
                    IpHeaderFields::V6(header) => header.route.upper_layer,
                };
                let Some(protocol) = socket.payload_protocol(&upper_layer) else {
                    return Err(self.refuse(OTHER_PROTOCOL));
                };
                if upper_layer.selector != payload_selector(protocol, socket.ipv6, payload) {
                    return Err(self.refuse(UPPER_LAYER));
                }
                socket
                    .include_header(state, false)
                    .map_err(SendError::Host)?;
                // An IPv6 raw socket takes the protocol of a send in its address's port.
                let port = if socket.ipv6 {
                    u16::from(protocol.0)
                } else {
                    u16::MIN
                };
                let destination = state
                    .send
                    .apply_header(fd, &mut control, &header, port, socket.ipv6)
                    .map_err(SendError::Host)?;
                (destination, payload)
            }
            RawPacket::HeaderIncluded(HeaderIncludedPacket::V4 {
                route,
                reassembly,
                packet,
            }) => {
                if socket.ipv6 {
                    return Err(invalid);
                }
                if route.upper_layer.selector.is_some() {
                    return Err(self.refuse(UPPER_LAYER));
                }
                socket
                    .include_header(state, true)
                    .map_err(SendError::Host)?;
                if state.reassembly != reassembly {
                    set_flag(
                        fd,
                        uapi::SOL_IP,
                        uapi::IP_NODEFRAG,
                        reassembly == OutgoingReassembly::Disabled,
                    )
                    .map_err(SendError::Host)?;
                    state.reassembly = reassembly;
                }
                // The kernel takes neither a TTL nor a path MTU mode for a packet with its own header.
                let header = IpHeaderFields::V4(Ipv4HeaderFields {
                    route,
                    time_to_live: None,
                    path_mtu_discovery: PathMtuDiscovery::Want,
                });
                let destination = state
                    .send
                    .apply_header(fd, &mut control, &header, u16::MIN, false)
                    .map_err(SendError::Host)?;
                control.push(
                    uapi::SOL_IP,
                    uapi::IP_PROTOCOL,
                    &c_int::from(route.upper_layer.protocol.0),
                );
                (destination, packet)
            }
            RawPacket::HeaderIncluded(HeaderIncludedPacket::V6 { route, packet }) => {
                if !socket.ipv6 {
                    return Err(invalid);
                }
                if route.upper_layer.selector.is_some() {
                    return Err(self.refuse(UPPER_LAYER));
                }
                socket
                    .include_header(state, true)
                    .map_err(SendError::Host)?;
                let port = u16::from(route.upper_layer.protocol.0);
                // The kernel takes only the route of these for a packet with its own header.
                let header = IpHeaderFields::V6(Ipv6HeaderFields {
                    route,
                    hop_limit: None,
                    path_mtu_discovery: PathMtuDiscovery::Want,
                    local_fragmentation: LocalFragmentation::Allowed,
                    fragment_mtu: None,
                    hop_by_hop_options: None,
                    routing_destination_options: None,
                    destination_options: None,
                });
                let destination = state
                    .send
                    .apply_header(fd, &mut control, &header, port, true)
                    .map_err(SendError::Host)?;
                (destination, packet)
            }
        };
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
        let sent = send_message(fd, Some(destination), bytes, control.bytes(), flags);
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

    fn receive(&self, socket: &Self::Socket, into: &mut [u8]) -> Result<Received, HostError> {
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

impl HostRawSockets {
    /// The next ICMP error or transmit report of the error queue, kept entries first, past entries that give none.
    fn take_error_entry(
        &self,
        socket: &HostRawSocket,
        state: &mut SocketState,
        into: &mut [u8],
    ) -> Result<Received, HostError> {
        let fd = socket.fd();
        let header_included = state.header_included;
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
                        header_included,
                        into,
                        &entry.data,
                        ancillary,
                        error,
                        offender,
                        entry.source,
                    );
                    // Only an IPv6 error quoting a packet with its own header is given whole.
                    if written.is_some()
                        && (!header_included || error.ee_origin == libc::SO_EE_ORIGIN_ICMP)
                    {
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

    /// The next packet, IPv6 with its headers written in front.
    fn receive_packet(
        &self,
        socket: &HostRawSocket,
        state: &mut SocketState,
        into: &mut [u8],
    ) -> Result<Received, HostError> {
        let fd = socket.fd();
        let mut control = ControlBuffer::new();
        let message = receive::receive(fd, into, &mut control, false)?;
        if message.control_truncated {
            self.shortfalls.count(receive::CONTROL_TRUNCATED);
        }
        let ancillary = Ancillary::parse(message.control);
        let given = &into[..message.length.min(into.len())];
        let forwarded = socket.ipv6
            && socket.is_forwarded_whole(state.router_alert, given, &message, &ancillary);
        if forwarded {
            self.shortfalls.count(FORWARDED_BY_ITS_BYTES);
        }
        let length = if socket.ipv6 && !forwarded {
            // Linux gives an IPv6 raw socket the packet after its headers, which are written here in front.
            let invalid = HostError::Errno(libc::EINVAL);
            let IpAddr::V6(source) = message.source.ok_or(invalid)?.ip() else {
                return Err(invalid);
            };
            let Some(IpAddr::V6(destination)) = ancillary.header_destination else {
                return Err(invalid);
            };
            let mut headers = Headers::new();
            receive::push_ipv6_headers(
                &mut headers,
                source,
                destination,
                socket.protocol,
                message.length,
                &ancillary,
            )
            .ok_or(HostError::Errno(libc::EMSGSIZE))?;
            headers.place(into, message.length.min(into.len()));
            headers.len() + message.length
        } else {
            message.length
        };
        let drops = match ancillary.drops {
            Some(total) => {
                let since = total.wrapping_sub(state.drops);
                state.drops = total;
                since
            }
            None => u32::MIN,
        };
        Ok(Received::Packet(ancillary.packet_fields(length, drops)))
    }
}

impl HostRawSocket {
    /// Answers whether Linux gave this packet from its IPv6 header on, as it gives a packet it was forwarding to a
    /// router alert socket of `IPPROTO_RAW`: the socket takes IPv6 router alerts, and the bytes are an IPv6 header
    /// from the sender, as long as the packet, then the hop-by-hop header Linux reports beside them.
    fn is_forwarded_whole(
        &self,
        router_alert: RouterAlert,
        bytes: &[u8],
        message: &receive::Message<'_>,
        ancillary: &Ancillary<'_>,
    ) -> bool {
        if self.protocol != raw_protocol() || !matches!(router_alert, RouterAlert::Ipv6 { .. }) {
            return false;
        }
        let Ok(header) = etherparse::Ipv6HeaderSlice::from_slice(bytes) else {
            return false;
        };
        let reported = ancillary
            .extension_headers
            .iter()
            .flatten()
            .find(|(kind, _)| *kind == IpNumber::IPV6_HEADER_HOP_BY_HOP);
        Some(IpAddr::V6(header.source_addr())) == message.source.map(|source| source.ip())
            && usize::from(header.payload_length()) + etherparse::Ipv6Header::LEN == message.length
            && header.next_header() == IpNumber::IPV6_HEADER_HOP_BY_HOP
            && reported.is_some_and(|(_, options)| {
                bytes[etherparse::Ipv6Header::LEN..].starts_with(options)
            })
    }

    fn set_router_alert(
        &self,
        state: &mut SocketState,
        wanted: RouterAlert,
    ) -> Result<(), HostError> {
        // Linux answers a second request for what a socket has with EADDRINUSE, so only a change is written.
        if state.router_alert == wanted {
            return Ok(());
        }
        let fd = self.fd();
        match state.router_alert {
            RouterAlert::Off => {}
            RouterAlert::Ipv4 => set_flag(fd, uapi::SOL_IP, uapi::IP_ROUTER_ALERT, false)?,
            RouterAlert::Ipv6 { .. } => {
                // A negative value takes the socket off the IPv6 router alert chain.
                set_int(fd, uapi::SOL_IPV6, uapi::IPV6_ROUTER_ALERT, -1)?
            }
        }
        state.router_alert = RouterAlert::Off;
        match wanted {
            RouterAlert::Off => {}
            RouterAlert::Ipv4 => set_flag(fd, uapi::SOL_IP, uapi::IP_ROUTER_ALERT, true)?,
            RouterAlert::Ipv6 { value, isolation } => {
                set_flag(
                    fd,
                    uapi::SOL_IPV6,
                    uapi::IPV6_ROUTER_ALERT_ISOLATE,
                    isolation == RouterAlertIsolation::OwnNamespace,
                )?;
                set_int(
                    fd,
                    uapi::SOL_IPV6,
                    uapi::IPV6_ROUTER_ALERT,
                    c_int::from(value.0),
                )?;
            }
        }
        state.router_alert = wanted;
        Ok(())
    }

    fn set_multicast_routing(
        &self,
        state: &mut SocketState,
        wanted: MulticastRouting,
    ) -> Result<(), HostError> {
        // Linux answers a second MRT_INIT of a socket with EADDRINUSE, so only a change is written.
        if state.multicast_routing == wanted {
            return Ok(());
        }
        let level = if self.ipv6 {
            uapi::SOL_IPV6
        } else {
            uapi::SOL_IP
        };
        match wanted {
            MulticastRouting::Enabled => set_flag(self.fd(), level, MULTICAST_ROUTING_INIT, true)?,
            MulticastRouting::Disabled => set_bytes(self.fd(), level, MULTICAST_ROUTING_DONE, &[])?,
        }
        state.multicast_routing = wanted;
        Ok(())
    }
}

impl<'a> Configurable<HostRawSocket, RawSocketSetting<'a>> for HostRawSockets {
    fn configure(
        &self,
        socket: &HostRawSocket,
        setting: RawSocketSetting<'a>,
    ) -> Result<(), HostError> {
        let fd = socket.fd();
        let mut state = socket.state.borrow_mut();
        let state = &mut *state;
        match setting {
            RawSocketSetting::Ip(IpSocketSetting::MulticastV4(group)) => set_multicast_group(
                fd,
                uapi::SOL_IP,
                IpAddr::V4(group.group),
                &group,
                |source| IpAddr::V4(*source),
            ),
            RawSocketSetting::Ip(IpSocketSetting::MulticastV6(group)) => set_multicast_group(
                fd,
                uapi::SOL_IPV6,
                IpAddr::V6(group.group),
                &group,
                |source| IpAddr::V6(*source),
            ),
            RawSocketSetting::Ip(IpSocketSetting::Anycast(anycast)) => {
                state.anycast.set(fd, &anycast)
            }
            RawSocketSetting::Ip(IpSocketSetting::Timestamping(timestamping)) => {
                state.send.set_receive_timestamps(fd, &timestamping)
            }
            RawSocketSetting::Ip(setting) => shared::set_ip_socket_setting(
                fd,
                socket.version(),
                &setting,
                u32::MIN,
                &self.shortfalls,
            ),
            // An offset of -1 is no checksum; an IPv4 socket answers as Linux does, that it has no such option.
            RawSocketSetting::Checksum(offset) => set_int(
                fd,
                uapi::SOL_IPV6,
                uapi::IPV6_CHECKSUM,
                match offset {
                    Some(offset) => {
                        c_int::try_from(offset.0).map_err(|_| HostError::Errno(libc::EINVAL))?
                    }
                    None => -1,
                },
            ),
            RawSocketSetting::RouterAlert(alert) => socket.set_router_alert(state, alert),
            RawSocketSetting::MulticastRouting(routing) => {
                socket.set_multicast_routing(state, routing)
            }
        }
    }
}
