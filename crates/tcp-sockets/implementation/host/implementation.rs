//! TCP and MPTCP connections and listeners made of the Linux sockets of the machine it runs on.
#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::ffi::c_int;
use std::io::IoSlice;
use std::mem::{MaybeUninit, size_of};
use std::net::{IpAddr, SocketAddr};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use etherparse::IpNumber;
use linux_raw_sys::net as uapi;
use netlink_packet_core::{NlasIterator, parse_u8, parse_u16, parse_u32, parse_u64};
use rustix::io::Errno;
use rustix::net::{RecvFlags, SocketFlags, SocketType, ipproto};
use socket2::{MsgHdr, SockAddr, SockRef};
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    ByteCount, BytesPerSecond, DropCount, HopLimit, InterfaceIndex, IpSocketSetting, IpVersion,
    LocalBinding, LocalPort, PacketMark, PacketPriority, PtpHardwareClockIndex, ReceiveTimestamps,
    ReceivedPacketFields, Report, TransmitEvent, TransmitReport, TransmitReportKey,
    TransmitReportRequest, WirelessAcknowledgement,
};
use wcontainers_internet_protocol_linux_socket_options::datagram::receive::Ancillary;
use wcontainers_internet_protocol_linux_socket_options::datagram::send::Control;
use wcontainers_internet_protocol_linux_socket_options::icmp_error::write_icmp_error;
use wcontainers_internet_protocol_linux_socket_options::readiness::{Readiness, Watched};
use wcontainers_internet_protocol_linux_socket_options::{
    self as options, ControlBuffer, Shortfalls, bind, family_of, get_int, host_error, open_socket,
    peer_for, set_flag, set_int, set_option,
};
use wcontainers_kernel::UserId;
use wcontainers_kernel::signal_information::PollEvents;
use wcontainers_negative_route_advice::NegativeRouteAdvice;
use wcontainers_tcp_sockets::authentication::TcpAuthenticationCounters;
use wcontainers_tcp_sockets::repair::{
    TcpRepair, TcpRepairQueue, TcpTimestampClock, TcpTimestampUnit,
};
use wcontainers_tcp_sockets::settings::{
    BufferTuning, RetryCount, SegmentCount, TcpFinWait2, TcpListenerSetting,
    TcpQuickAcknowledgement, TcpSavedSynchronize, TcpSetting, TransmitRehash,
};
use wcontainers_tcp_sockets::state::{
    ByteTotal, DeliveryRateLimit, EventCount, TcpCongestionAvoidanceState, TcpErrorQueueEntry,
    TcpListenerQuery, TcpQuery, TcpReading, TcpSavedSynchronizeHeader, TcpTransmitStatistics,
};
use wcontainers_tcp_sockets::{
    ListenBacklog, TcpAccepted, TcpClose, TcpConnectStart, TcpConnected, TcpConnections,
    TcpContinuation, TcpFastOpen, TcpListeners, TcpListening, TcpProtocol, TcpPushOverride,
    TcpReceiveRequest, TcpReceived, TcpRecordBoundary, TcpSendRequest, TcpShutdown, TcpSockets,
    TcpUrgency,
};

/// The `SO_TIMESTAMPING` flags kept on a connection once it has asked for a transmit report: keys counted in bytes, software and hardware times.
const REPORT_FLAGS: u32 = uapi::SOF_TIMESTAMPING_OPT_ID as u32
    | uapi::SOF_TIMESTAMPING_OPT_ID_TCP as u32
    | uapi::SOF_TIMESTAMPING_SOFTWARE as u32
    | uapi::SOF_TIMESTAMPING_RAW_HARDWARE as u32;

/// Reports without the packet, which Linux needs for statistics.
const WITHOUT_PACKET: u32 = uapi::SOF_TIMESTAMPING_OPT_TSONLY as u32;
const WITH_STATISTICS: u32 = uapi::SOF_TIMESTAMPING_OPT_STATS as u32;

/// One more than the largest `TCP_NLA_*` attribute of transmit statistics.
const STATISTICS_ATTRIBUTES: usize = uapi::TCP_NLA_REHASH as usize + 1;

/// The interface index Linux gives when a report names no interface.
const NO_INTERFACE: InterfaceIndex = InterfaceIndex(0);

/// The room `TCP_SAVED_SYN` is first read with: the largest IPv4 and TCP headers after an Ethernet header.
const SAVED_SYNCHRONIZE_ROOM: usize = etherparse::Ethernet2Header::LEN
    + etherparse::Ipv4Header::MAX_LEN
    + etherparse::TcpHeader::MAX_LEN;

/// `TCP_SAVE_SYN`'s values, which Linux names none: off, from the network header, from the link header.
const SAVE_NONE: c_int = 0;
const SAVE_FROM_NETWORK_HEADER: c_int = 1;
const SAVE_FROM_LINK_HEADER: c_int = 2;

/// The options whose records `IP_PKTOPTIONS` gives a TCP socket: packet information, TTL and TOS.
const IPV4_RECORDS: [u32; 3] = [uapi::IP_PKTINFO, uapi::IP_RECVTTL, uapi::IP_RECVTOS];

/// The options whose records `IPV6_2292PKTOPTIONS` gives a TCP socket from the last packet that came in order.
const IPV6_RECORDS: [u32; 6] = [
    uapi::IPV6_RECVPKTINFO,
    uapi::IPV6_RECVHOPLIMIT,
    uapi::IPV6_RECVTCLASS,
    uapi::IPV6_RECVHOPOPTS,
    uapi::IPV6_RECVDSTOPTS,
    uapi::IPV6_RECVRTHDR,
];

/// The reasons the host could not honour a request, each counted by [`Shortfalls`].
const NO_AUTHENTICATION: &str = "TCP authentication is not mapped to the host yet";
const NO_REPAIR_OPTIONS: &str = "TCP repair options and windows are not mapped to the host yet";
const NO_SAVED_SYNCHRONIZE: &str = "the host kept no SYN headers for an accepted connection";
const ARRIVAL_INTERFACE_BY_ADDRESS: &str = "the SYN's arrival interface is taken as the host interface whose address or prefix holds the local address";
const SYNCHRONIZE_FIELDS_OF_THE_SOCKET: &str = "Linux reports no priority, timestamps or drops for a SYN, and its mark only as the accepted socket's: the accepted socket's mark and priority are given, with no timestamps or drops";
const LONG_SAVED_SYNCHRONIZE: &str = "a SYN's saved headers were longer than the room accept first reads them into, and were read again into a larger one";
const NO_NETWORK_HEADER: &str =
    "no IP header from the peer to the local address was found in a SYN's saved headers";
const PACKET_OR_STATISTICS: &str =
    "Linux gives a TCP socket's transmit reports either the packet or statistics, never both";
const NO_OTHER_ERROR_ENTRIES: &str = "error queue entries other than ICMP errors, transmit timestamps and wireless status have no TCP counterpart";
const NO_EXTENDED_ERROR: &str = "an error queue entry came without its extended error";
const UNWRITABLE_ICMP_ERROR: &str = "an ICMP error's addresses or lengths fit no IP header";
const CUT_REPORT_PACKET: &str = "a transmit report's packet was cut to the buffer";
const NO_STATISTICS: &str = "a transmit report came without the statistics its send asked for";
const CUT_CONTROL: &str = "the control messages of a receive or an error queue entry did not fit";
const UNASKED_REPORT: &str = "a transmit report for no send that asked for it";
const NO_CONNECTION_STATE: &str = "reading a TCP connection's state is not mapped to the host yet";
const ABORT_CLOSED_IN_ORDER: &str = "an abort closed in order: the host refused a linger of zero";
const CLOCK_UNIT_OF_THE_ROUTE: &str = "a TCP timestamp clock counts in the unit its route gives (the tcp_usec_ts route feature), which Linux cannot change";
const NO_LISTENER_STATE: &str = "reading a TCP listener's state is not mapped to the host yet";

/// Linux's TCP sockets as the exit node's TCP connections and listeners.
pub struct HostTcpSockets {
    shortfalls: Shortfalls,
    interfaces: InterfaceAddresses,
    readiness: Readiness,
}

impl HostTcpSockets {
    pub fn new() -> Result<Self, HostError> {
        Ok(Self {
            shortfalls: Shortfalls::default(),
            interfaces: InterfaceAddresses::default(),
            readiness: Readiness::new()?,
        })
    }

    /// The requests answered as unsupported so far, by reason.
    pub fn counters(&self) -> Vec<CounterReading> {
        self.shortfalls.readings()
    }

    /// The next entry of the error queue, and whether a packet its report carries was cut to the buffer.
    fn next_error_queue_entry(
        &self,
        connection: &HostTcpConnection,
        into: &mut [u8],
    ) -> Result<(TcpErrorQueueEntry, bool), HostError> {
        let mut control = ControlBuffer::default();
        let message = options::receive_message(
            connection.socket.fd(),
            into,
            libc::MSG_ERRQUEUE | libc::MSG_DONTWAIT,
            &mut control,
        )?;
        if message.flags & libc::MSG_CTRUNC != 0 {
            self.shortfalls.count(CUT_CONTROL);
        }
        let ancillary = Ancillary::parse(message.control);
        let Some((error, offender)) = ancillary.error else {
            return Err(self.shortfalls.refuse(NO_EXTENDED_ERROR));
        };
        let wireless = message.messages().find_map(|(level, kind, data)| {
            (level == libc::SOL_SOCKET && kind == uapi::SCM_WIFI_STATUS as c_int)
                .then(|| <[u8; size_of::<c_int>()]>::try_from(data).ok())
                .flatten()
                .map(c_int::from_ne_bytes)
        });
        let cut = message.flags & libc::MSG_TRUNC != 0;
        match error.ee_origin {
            libc::SO_EE_ORIGIN_ICMP | libc::SO_EE_ORIGIN_ICMP6 => {
                let (local, peer) = *connection
                    .addresses
                    .get()
                    .ok_or_else(|| self.shortfalls.refuse(UNWRITABLE_ICMP_ERROR))?;
                let length = write_icmp_error(
                    into,
                    message.length,
                    &ancillary,
                    &error,
                    offender,
                    local.ip(),
                    peer.ip(),
                    IpNumber::TCP,
                    &[],
                )
                .ok_or_else(|| self.shortfalls.refuse(UNWRITABLE_ICMP_ERROR))?;
                Ok((
                    TcpErrorQueueEntry::IcmpError(ancillary.packet_fields(length, u32::MIN)),
                    false,
                ))
            }
            libc::SO_EE_ORIGIN_TXSTATUS if wireless.is_some() => {
                let acknowledged = if wireless == Some(0) {
                    WirelessAcknowledgement::NotAcknowledged
                } else {
                    WirelessAcknowledgement::Acknowledged
                };
                // Linux names no send in a wireless status report: its key is the zero it gives.
                Ok((
                    TcpErrorQueueEntry::TransmitReport {
                        report: TransmitReport {
                            key: TransmitReportKey(error.ee_data),
                            interface: NO_INTERFACE,
                            event: TransmitEvent::WirelessStatus(acknowledged),
                            packet_length: Some(ByteCount(message.length)),
                        },
                        statistics: None,
                    },
                    cut,
                ))
            }
            libc::SO_EE_ORIGIN_TIMESTAMPING => {
                let Some(mut report) = ancillary.transmit_report(&error, None) else {
                    return Err(self.shortfalls.refuse(NO_OTHER_ERROR_ENTRIES));
                };
                let mut reports = connection.reports();
                let offset = error.ee_data;
                let send = match reports.keys.get_mut(&offset) {
                    Some(send) => {
                        let found = *send;
                        send.remaining = send.remaining.saturating_sub(1);
                        if send.remaining == 0 {
                            reports.keys.remove(&offset);
                        }
                        found
                    }
                    None => {
                        self.shortfalls.count(UNASKED_REPORT);
                        ReportedSend {
                            key: TransmitReportKey(offset),
                            remaining: 0,
                            packet: false,
                            statistics: false,
                        }
                    }
                };
                report.key = send.key;
                if send.packet {
                    report.packet_length = Some(ByteCount(message.length));
                }
                let statistics = if send.statistics {
                    let found = message.messages().find_map(|(level, kind, data)| {
                        (level == libc::SOL_SOCKET
                            && kind == uapi::SCM_TIMESTAMPING_OPT_STATS as c_int)
                            .then(|| transmit_statistics(data))
                            .flatten()
                    });
                    if found.is_none() {
                        self.shortfalls.count(NO_STATISTICS);
                    }
                    found
                } else {
                    None
                };
                Ok((
                    TcpErrorQueueEntry::TransmitReport { report, statistics },
                    send.packet && cut,
                ))
            }
            _ => Err(self.shortfalls.refuse(NO_OTHER_ERROR_ENTRIES)),
        }
    }
}

/// The transmit reports a connection asked for and the guest keys they are reported under.
#[derive(Debug, Default)]
struct TransmitReports {
    /// The `SO_TIMESTAMPING` report flags on the socket, none until a send asks for a report.
    flags: u32,
    wireless_status: bool,
    bound_clock: Option<PtpHardwareClockIndex>,
    /// Bytes taken by sends since the reports were armed, wrapping as the kernel's key does.
    sent: u32,
    /// Each send that asked for reports, by the offset of its last byte.
    keys: BTreeMap<u32, ReportedSend>,
}

#[derive(Debug, Clone, Copy)]
struct ReportedSend {
    key: TransmitReportKey,
    remaining: u32,
    packet: bool,
    statistics: bool,
}

pub struct HostTcpConnection {
    socket: Watched,
    family: IpVersion,
    /// The local and peer addresses, kept for the ICMP errors that come after the connection is gone.
    addresses: OnceLock<(SocketAddr, SocketAddr)>,
    receive_timestamping: AtomicU32,
    unread_reports: AtomicBool,
    /// Whether the reports hold report flags, so that every send's bytes are counted.
    reports_armed: AtomicBool,
    reports: Mutex<TransmitReports>,
    /// The SYN's headers, kept when the listener was asked to save them, until they are taken.
    saved_synchronize: Mutex<Option<Vec<u8>>>,
    /// Held by every receive, since a peek at an offset sets the socket-wide `SO_PEEK_OFF` for its call.
    receiving: Mutex<()>,
}

pub struct HostTcpListener {
    socket: Watched,
    family: IpVersion,
    protocol: TcpProtocol,
    receive_timestamping: AtomicU32,
    bound_clock: Mutex<Option<PtpHardwareClockIndex>>,
    /// The guest's `TCP_SAVE_SYN` value: whether accepted connections keep their SYN's headers to be taken, and from
    /// which header.
    saves_synchronize: AtomicI32,
    /// Whether the guest asked the listener's sockets to record the last header they received.
    records_last_received: AtomicBool,
}

macro_rules! ordered_by_descriptor {
    ($type:ty) => {
        impl PartialEq for $type {
            fn eq(&self, other: &Self) -> bool {
                self.socket.fd().as_raw_fd() == other.socket.fd().as_raw_fd()
            }
        }
        impl Eq for $type {}
        impl PartialOrd for $type {
            fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for $type {
            fn cmp(&self, other: &Self) -> core::cmp::Ordering {
                self.socket
                    .fd()
                    .as_raw_fd()
                    .cmp(&other.socket.fd().as_raw_fd())
            }
        }
    };
}
ordered_by_descriptor!(HostTcpConnection);
ordered_by_descriptor!(HostTcpListener);

impl HostTcpConnection {
    fn new(socket: Watched, family: IpVersion, receive_timestamping: u32) -> Self {
        Self {
            socket,
            family,
            addresses: OnceLock::new(),
            receive_timestamping: AtomicU32::new(receive_timestamping),
            unread_reports: AtomicBool::new(false),
            reports_armed: AtomicBool::new(false),
            reports: Mutex::default(),
            saved_synchronize: Mutex::default(),
            receiving: Mutex::default(),
        }
    }

    fn reports(&self) -> MutexGuard<'_, TransmitReports> {
        locked(&self.reports)
    }
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// An interface address of the host and its prefix.
#[derive(Debug, Clone, Copy)]
struct HeldAddress {
    address: IpAddr,
    netmask: Option<IpAddr>,
    interface: InterfaceIndex,
}

impl HeldAddress {
    fn holds(&self, address: IpAddr) -> bool {
        match (self.address, self.netmask, address) {
            (IpAddr::V4(held), Some(IpAddr::V4(mask)), IpAddr::V4(address)) => {
                (u32::from(held) ^ u32::from(address)) & u32::from(mask) == 0
            }
            (IpAddr::V6(held), Some(IpAddr::V6(mask)), IpAddr::V6(address)) => {
                (u128::from(held) ^ u128::from(address)) & u128::from(mask) == 0
            }
            _ => self.address == address,
        }
    }
}

/// The host's interface addresses, read again only after the kernel announced a change of them.
#[derive(Debug, Default)]
struct InterfaceAddresses {
    /// A routing netlink socket that hears of every address change, none when the host refused one.
    changes: OnceLock<Option<OwnedFd>>,
    table: Mutex<Option<Vec<HeldAddress>>>,
}

impl InterfaceAddresses {
    /// The interface that holds the address, or failing that the first whose prefix holds it.
    fn holding(&self, address: IpAddr) -> Option<InterfaceIndex> {
        // Subscribed before the first read, so that no change goes unheard.
        let changes = self.changes.get_or_init(address_changes);
        let mut table = locked(&self.table);
        if changes
            .as_ref()
            .is_none_or(|changes| changed(changes.as_fd()))
            || table.is_none()
        {
            *table = Some(interface_addresses());
        }
        let table = table.as_deref()?;
        table
            .iter()
            .find(|held| held.address == address)
            .or_else(|| table.iter().find(|held| held.holds(address)))
            .map(|held| held.interface)
    }
}

fn address_changes() -> Option<OwnedFd> {
    let socket = rustix::net::socket_with(
        rustix::net::AddressFamily::NETLINK,
        SocketType::RAW,
        SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
        // No protocol is NETLINK_ROUTE, which rustix cannot name: it is zero.
        None,
    )
    .ok()?;
    let groups =
        linux_raw_sys::netlink::RTMGRP_IPV4_IFADDR | linux_raw_sys::netlink::RTMGRP_IPV6_IFADDR;
    rustix::net::bind(
        &socket,
        &rustix::net::netlink::SocketAddrNetlink::new(0, groups),
    )
    .ok()?;
    Some(socket)
}

/// Whether the subscription heard of a change since it was last read, reading all it holds.
fn changed(changes: BorrowedFd<'_>) -> bool {
    let mut message = [0; NETLINK_MESSAGE_ROOM];
    let mut heard = false;
    loop {
        match rustix::net::recv(changes, &mut message, RecvFlags::DONTWAIT) {
            Ok(_) => heard = true,
            Err(rustix::io::Errno::AGAIN) => return heard,
            // An overrun loses changes: the table is read again.
            Err(_) => return true,
        }
    }
}

/// The room a change is read into; a longer one is cut, which loses nothing needed.
const NETLINK_MESSAGE_ROOM: usize = 1 << 12;

fn interface_addresses() -> Vec<HeldAddress> {
    let ip = |address: &nix::sys::socket::SockaddrStorage| {
        address
            .as_sockaddr_in()
            .map(|address| IpAddr::V4(address.ip()))
            .or_else(|| {
                address
                    .as_sockaddr_in6()
                    .map(|address| IpAddr::V6(address.ip()))
            })
    };
    let Ok(entries) = nix::ifaddrs::getifaddrs() else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| {
            Some(HeldAddress {
                address: ip(entry.address.as_ref()?)?,
                netmask: entry.netmask.as_ref().and_then(ip),
                interface: InterfaceIndex(
                    nix::net::if_::if_nametoindex(entry.interface_name.as_str()).ok()?,
                ),
            })
        })
        .collect()
}

/// Where the SYN's IP header starts in the headers `TCP_SAVED_SYN` gave: the first place an IP header from the peer to
/// the local address holds a TCP header between their ports that ends the headers.
fn network_header_start(saved: &[u8], local: SocketAddr, peer: SocketAddr) -> Option<usize> {
    (0..saved.len()).find(|&start| {
        etherparse::LaxIpSlice::from_slice(&saved[start..]).is_ok_and(|(ip, _)| {
            let payload = ip.payload();
            ip.source_addr().to_canonical() == peer.ip().to_canonical()
                && ip.destination_addr().to_canonical() == local.ip().to_canonical()
                && payload.ip_number == IpNumber::TCP
                && etherparse::TcpSlice::from_slice(payload.payload).is_ok_and(|tcp| {
                    tcp.source_port() == peer.port()
                        && tcp.destination_port() == local.port()
                        && tcp.payload().is_empty()
                })
        })
    })
}

/// The room `IPV6_2292PKTOPTIONS` gives the SYN's packet information in: one control message, padded.
const PACKET_OPTIONS_ROOM: usize =
    size_of::<libc::cmsghdr>() + size_of::<uapi::in6_pktinfo>() + size_of::<usize>();

/// Control message room aligned as the messages' headers are.
#[repr(C)]
struct PacketOptions {
    _aligned: [libc::cmsghdr; 0],
    bytes: [u8; PACKET_OPTIONS_ROOM],
}

/// The host error of a failed socket2 call.
fn io_host_error(error: std::io::Error) -> HostError {
    host_error(Errno::from_io_error(&error).unwrap_or(Errno::IO))
}

/// The socket's address as socket2 gives it, which is always an internet one for these sockets.
fn internet_address(address: std::io::Result<SockAddr>) -> Result<SocketAddr, HostError> {
    address
        .map_err(io_host_error)?
        .as_socket()
        .ok_or(HostError::Errno(libc::EAFNOSUPPORT))
}

/// A receive buffer as socket2 takes it.
fn uninitialised(bytes: &mut [u8]) -> &mut [MaybeUninit<u8>] {
    // SAFETY: MaybeUninit<u8> has u8's layout, and socket2 writes only initialised bytes into it.
    unsafe { &mut *(std::ptr::from_mut(bytes) as *mut [MaybeUninit<u8>]) }
}

fn invalid() -> HostError {
    HostError::Errno(libc::EINVAL)
}

/// A duration as a whole number of the unit that fits a C int, or EINVAL.
fn whole(duration: Duration, unit: Duration) -> Result<c_int, HostError> {
    if !duration.as_nanos().is_multiple_of(unit.as_nanos()) {
        return Err(invalid());
    }
    c_int::try_from(duration.as_nanos() / unit.as_nanos()).map_err(|_| invalid())
}

/// The `TCP_QUICKACK` value (any even one but zero) that sends a scheduled acknowledgement, then delays if one was.
const QUICK_ACKNOWLEDGEMENT_THEN_DELAYING: c_int = 2;

const SECOND: Duration = Duration::from_secs(1);
const MILLISECOND: Duration = Duration::from_millis(1);
const MICROSECOND: Duration = Duration::from_micros(1);

fn count(value: usize) -> Result<c_int, HostError> {
    c_int::try_from(value).map_err(|_| invalid())
}

fn protocol_of(protocol: TcpProtocol) -> rustix::net::Protocol {
    match protocol {
        TcpProtocol::Tcp => ipproto::TCP,
        TcpProtocol::Multipath => ipproto::MPTCP,
    }
}

/// The `SO_TIMESTAMPING` record flags a send's report request asks for, and how many reports they bring.
fn record_flags(request: &TransmitReportRequest) -> (u32, u32) {
    let mut flags = 0;
    let mut reports = 0;
    for (report, flag) in [
        (request.scheduled, uapi::SOF_TIMESTAMPING_TX_SCHED as u32),
        (request.software, uapi::SOF_TIMESTAMPING_TX_SOFTWARE as u32),
        (request.hardware, uapi::SOF_TIMESTAMPING_TX_HARDWARE as u32),
        (
            request.acknowledgement,
            uapi::SOF_TIMESTAMPING_TX_ACK as u32,
        ),
        (
            request.completion,
            uapi::SOF_TIMESTAMPING_TX_COMPLETION as u32,
        ),
    ] {
        if report == Report::Requested {
            flags |= flag;
            reports += 1;
        }
    }
    (flags, reports)
}

/// One send with an `SO_TIMESTAMPING` control message carrying the record flags.
fn send_with_record_flags(
    socket: BorrowedFd<'_>,
    bytes: &[u8],
    flags: c_int,
    record: u32,
) -> Result<usize, HostError> {
    let mut control = Control::new();
    control.push(
        libc::SOL_SOCKET as u32,
        libc::SO_TIMESTAMPING as u32,
        &record,
    );
    let buffers = [IoSlice::new(bytes)];
    let message = MsgHdr::new()
        .with_buffers(&buffers)
        .with_control(control.bytes());
    SockRef::from(&socket)
        .sendmsg(&message, flags)
        .map_err(io_host_error)
}

/// The statistics of a transmit report from the `TCP_NLA_*` attributes of its `SCM_TIMESTAMPING_OPT_STATS`.
fn transmit_statistics(data: &[u8]) -> Option<TcpTransmitStatistics> {
    let mut values = [None; STATISTICS_ATTRIBUTES];
    for attribute in NlasIterator::new(data) {
        let attribute = attribute.ok()?;
        let value = attribute.value();
        // Each parser takes only a value of its own width.
        let number = parse_u64(value)
            .or_else(|_| parse_u32(value).map(u64::from))
            .or_else(|_| parse_u16(value).map(u64::from))
            .or_else(|_| parse_u8(value).map(u64::from));
        if let (Ok(number), Some(slot)) = (number, values.get_mut(usize::from(attribute.kind()))) {
            *slot = Some(number);
        }
    }
    let at = |kind: usize| values[kind];
    let microseconds = |kind: usize| at(kind).map(Duration::from_micros);
    let segments = |kind: usize| at(kind).map(SegmentCount);
    let events = |kind: usize| {
        at(kind)
            .and_then(|value| u32::try_from(value).ok())
            .map(EventCount)
    };
    let bytes = |kind: usize| {
        at(kind)
            .and_then(|value| usize::try_from(value).ok())
            .map(ByteCount)
    };
    let state = at(uapi::TCP_NLA_CA_STATE as usize)?;
    Some(TcpTransmitStatistics {
        busy: microseconds(uapi::TCP_NLA_BUSY as usize)?,
        receive_window_limited: microseconds(uapi::TCP_NLA_RWND_LIMITED as usize)?,
        send_buffer_limited: microseconds(uapi::TCP_NLA_SNDBUF_LIMITED as usize)?,
        data_segments_out: segments(uapi::TCP_NLA_DATA_SEGS_OUT as usize)?,
        total_retransmissions: segments(uapi::TCP_NLA_TOTAL_RETRANS as usize)?,
        pacing_rate: BytesPerSecond(at(uapi::TCP_NLA_PACING_RATE as usize)?),
        delivery_rate: BytesPerSecond(at(uapi::TCP_NLA_DELIVERY_RATE as usize)?),
        send_congestion_window: segments(uapi::TCP_NLA_SND_CWND as usize)?,
        reordering: segments(uapi::TCP_NLA_REORDERING as usize)?,
        minimum_round_trip_time: microseconds(uapi::TCP_NLA_MIN_RTT as usize)?,
        recurring_retransmissions: RetryCount(
            u8::try_from(at(uapi::TCP_NLA_RECUR_RETRANS as usize)?).ok()?,
        ),
        delivery_rate_limit: if at(uapi::TCP_NLA_DELIVERY_RATE_APP_LMT as usize)? == 0 {
            DeliveryRateLimit::Network
        } else {
            DeliveryRateLimit::Application
        },
        send_queue: bytes(uapi::TCP_NLA_SNDQ_SIZE as usize)?,
        congestion_avoidance_state: [
            (
                uapi::tcp_ca_state::TCP_CA_Open,
                TcpCongestionAvoidanceState::Open,
            ),
            (
                uapi::tcp_ca_state::TCP_CA_Disorder,
                TcpCongestionAvoidanceState::Disorder,
            ),
            (
                uapi::tcp_ca_state::TCP_CA_CWR,
                TcpCongestionAvoidanceState::CongestionWindowReduced,
            ),
            (
                uapi::tcp_ca_state::TCP_CA_Recovery,
                TcpCongestionAvoidanceState::Recovery,
            ),
            (
                uapi::tcp_ca_state::TCP_CA_Loss,
                TcpCongestionAvoidanceState::Loss,
            ),
        ]
        .into_iter()
        .find_map(|(linux, ours)| (linux as u64 == state).then_some(ours))?,
        send_slow_start_threshold: segments(uapi::TCP_NLA_SND_SSTHRESH as usize)?,
        delivered: segments(uapi::TCP_NLA_DELIVERED as usize)?,
        delivered_congestion_experienced: segments(uapi::TCP_NLA_DELIVERED_CE as usize)?,
        bytes_sent: ByteTotal(at(uapi::TCP_NLA_BYTES_SENT as usize)?),
        bytes_retransmitted: ByteTotal(at(uapi::TCP_NLA_BYTES_RETRANS as usize)?),
        duplicate_selective_acknowledgements: segments(uapi::TCP_NLA_DSACK_DUPS as usize)?,
        reordering_seen: events(uapi::TCP_NLA_REORD_SEEN as usize)?,
        smoothed_round_trip_time: microseconds(uapi::TCP_NLA_SRTT as usize)?,
        timeout_rehashes: events(uapi::TCP_NLA_TIMEOUT_REHASH as usize)?,
        not_sent: bytes(uapi::TCP_NLA_BYTES_NOTSENT as usize)?,
        earliest_departure: Duration::from_nanos(at(uapi::TCP_NLA_EDT as usize)?),
        // Linux gives the hop limit only in a report that an acknowledgement brought.
        received_hop_limit: HopLimit(
            at(uapi::TCP_NLA_TTL as usize)
                .map_or(Some(u8::MIN), |value| u8::try_from(value).ok())?,
        ),
        rehashes: events(uapi::TCP_NLA_REHASH as usize)?,
    })
}

impl HostTcpSockets {
    fn configure_socket(
        &self,
        socket: BorrowedFd<'_>,
        family: IpVersion,
        setting: &TcpSetting<'_>,
        report_flags: u32,
    ) -> Result<(), HostError> {
        let tcp = |name: u32, value: c_int| set_int(socket, uapi::SOL_TCP, name, value);
        let refuse = |reason| Err(self.shortfalls.refuse(reason));
        match *setting {
            TcpSetting::Ip(ref ip) => {
                options::set_ip_socket_setting(socket, family, ip, report_flags, &self.shortfalls)
            }
            TcpSetting::Packet(ref packet) => {
                options::set_packet_setting(socket, family, packet, &self.shortfalls)
            }
            TcpSetting::Port(ref port) => options::set_port_setting(socket, family, port),
            TcpSetting::BufferLocks(locks) => {
                let mut value = 0;
                if locks.send == BufferTuning::Locked {
                    value |= uapi::SOCK_SNDBUF_LOCK;
                }
                if locks.receive == BufferTuning::Locked {
                    value |= uapi::SOCK_RCVBUF_LOCK;
                }
                set_option(socket, uapi::SOL_SOCKET, uapi::SO_BUF_LOCK, &value)
            }
            TcpSetting::ReceiveLowWater(bytes) => {
                set_int(socket, uapi::SOL_SOCKET, uapi::SO_RCVLOWAT, count(bytes.0)?)
            }
            TcpSetting::Filter(filter) => options::set_filter(
                socket,
                uapi::SO_ATTACH_FILTER,
                uapi::SO_DETACH_FILTER,
                filter,
            ),
            TcpSetting::TransmitRehash(rehash) => {
                let value = match rehash {
                    // The option takes the default as the signed byte it is stored as.
                    TransmitRehash::SystemDefault => {
                        c_int::from(uapi::SOCK_TXREHASH_DEFAULT as u8 as i8)
                    }
                    TransmitRehash::Disabled => uapi::SOCK_TXREHASH_DISABLED as c_int,
                    TransmitRehash::Enabled => uapi::SOCK_TXREHASH_ENABLED as c_int,
                };
                set_int(socket, uapi::SOL_SOCKET, uapi::SO_TXREHASH, value)
            }
            TcpSetting::MinimumHopLimit { version, minimum } => {
                let minimum = minimum.map_or(0, |limit| c_int::from(limit.0));
                match version {
                    IpVersion::V4 => set_int(socket, uapi::SOL_IP, uapi::IP_MINTTL, minimum),
                    IpVersion::V6 => {
                        set_int(socket, uapi::SOL_IPV6, uapi::IPV6_MINHOPCOUNT, minimum)
                    }
                }
            }
            TcpSetting::RecordsLastReceivedHeader(report) => {
                let on = report == Report::Requested;
                for option in IPV4_RECORDS {
                    set_flag(socket, uapi::SOL_IP, option, on)?;
                }
                if family == IpVersion::V6 {
                    for option in IPV6_RECORDS {
                        set_flag(socket, uapi::SOL_IPV6, option, on)?;
                    }
                }
                Ok(())
            }
            TcpSetting::IcmpErrorReports { version, reports } => {
                let on = reports == Report::Requested;
                match version {
                    IpVersion::V4 => set_flag(socket, uapi::SOL_IP, uapi::IP_RECVERR, on),
                    IpVersion::V6 => set_flag(socket, uapi::SOL_IPV6, uapi::IPV6_RECVERR, on),
                }
            }
            TcpSetting::KeepAlive(on) => SockRef::from(&socket)
                .set_keepalive(on)
                .map_err(io_host_error),
            TcpSetting::KeepAliveIdle(idle) => tcp(uapi::TCP_KEEPIDLE, whole(idle, SECOND)?),
            TcpSetting::KeepAliveInterval(interval) => {
                tcp(uapi::TCP_KEEPINTVL, whole(interval, SECOND)?)
            }
            TcpSetting::KeepAliveProbes(probes) => tcp(uapi::TCP_KEEPCNT, c_int::from(probes.0)),
            TcpSetting::NoDelay(on) => SockRef::from(&socket)
                .set_tcp_nodelay(on)
                .map_err(io_host_error),
            TcpSetting::Cork(on) => SockRef::from(&socket)
                .set_tcp_cork(on)
                .map_err(io_host_error),
            TcpSetting::UrgentInline(on) => SockRef::from(&socket)
                .set_out_of_band_inline(on)
                .map_err(io_host_error),
            TcpSetting::QuickAcknowledgement(mode) => SockRef::from(&socket)
                .set_tcp_quickack(mode == TcpQuickAcknowledgement::On)
                .map_err(io_host_error),
            TcpSetting::MaximumSegmentSize(size) => SockRef::from(&socket)
                .set_tcp_mss(size.map_or(0, |size| u32::from(size.0)))
                .map_err(io_host_error),
            TcpSetting::WindowClamp(clamp) => tcp(
                uapi::TCP_WINDOW_CLAMP,
                match clamp {
                    Some(window) => c_int::try_from(window.0).map_err(|_| invalid())?,
                    None => 0,
                },
            ),
            TcpSetting::SynchronizeRetries(retries) => {
                tcp(uapi::TCP_SYNCNT, c_int::from(retries.0))
            }
            TcpSetting::UserTimeout(timeout) => tcp(
                uapi::TCP_USER_TIMEOUT,
                match timeout {
                    Some(timeout) => whole(timeout, MILLISECOND)?,
                    None => 0,
                },
            ),
            TcpSetting::DeferAccept(wait) => tcp(
                uapi::TCP_DEFER_ACCEPT,
                match wait {
                    Some(wait) => whole(wait, SECOND)?,
                    None => 0,
                },
            ),
            TcpSetting::FinWait2(fin_wait) => tcp(
                uapi::TCP_LINGER2,
                match fin_wait {
                    TcpFinWait2::SystemDefault => 0,
                    TcpFinWait2::Lasts(time) => whole(time, SECOND)?,
                    TcpFinWait2::Reset => -1,
                },
            ),
            TcpSetting::RetransmissionTimeoutMinimum(time) => {
                tcp(uapi::TCP_RTO_MIN_US, whole(time, MICROSECOND)?)
            }
            TcpSetting::RetransmissionTimeoutMaximum(time) => {
                tcp(uapi::TCP_RTO_MAX_MS, whole(time, MILLISECOND)?)
            }
            TcpSetting::DelayedAcknowledgementMaximum(time) => {
                tcp(uapi::TCP_DELACK_MAX_US, whole(time, MICROSECOND)?)
            }
            TcpSetting::TransmitDelay(time) => tcp(uapi::TCP_TX_DELAY, whole(time, MICROSECOND)?),
            TcpSetting::ThinLinearTimeouts(on) => SockRef::from(&socket)
                .set_tcp_thin_linear_timeouts(on)
                .map_err(io_host_error),
            TcpSetting::CongestionControl(name) => SockRef::from(&socket)
                .set_tcp_congestion(name.as_bytes())
                .map_err(io_host_error),
            TcpSetting::NotSentLowWater(low_water) => SockRef::from(&socket)
                .set_tcp_notsent_lowat(match low_water {
                    Some(bytes) => u32::try_from(bytes.0).map_err(|_| invalid())?,
                    None => u32::MAX,
                })
                .map_err(io_host_error),
            TcpSetting::FastOpenWithoutCookie(on) => {
                set_flag(socket, uapi::SOL_TCP, uapi::TCP_FASTOPEN_NO_COOKIE, on)
            }
            TcpSetting::UnreadByteReports(report) => set_flag(
                socket,
                uapi::SOL_TCP,
                uapi::TCP_INQ,
                report == Report::Requested,
            ),
            TcpSetting::Md5Key(_)
            | TcpSetting::AddAuthenticationKey(_)
            | TcpSetting::RemoveAuthenticationKey(_)
            | TcpSetting::AuthenticationOptions(_)
            | TcpSetting::InitialAuthenticationCounters(_)
            | TcpSetting::AuthenticationRepair(_) => refuse(NO_AUTHENTICATION),
            TcpSetting::Repair(mode) => tcp(
                uapi::TCP_REPAIR,
                match mode {
                    TcpRepair::Off => uapi::TCP_REPAIR_OFF,
                    TcpRepair::On => uapi::TCP_REPAIR_ON,
                } as c_int,
            ),
            TcpSetting::RepairQueue(queue) => tcp(
                uapi::TCP_REPAIR_QUEUE,
                match queue {
                    TcpRepairQueue::NoQueue => uapi::TCP_NO_QUEUE,
                    TcpRepairQueue::Receive => uapi::TCP_RECV_QUEUE,
                    TcpRepairQueue::Send => uapi::TCP_SEND_QUEUE,
                } as c_int,
            ),
            TcpSetting::QueueSequence(sequence) => {
                set_option(socket, uapi::SOL_TCP, uapi::TCP_QUEUE_SEQ, &sequence.0)
            }
            TcpSetting::RepairOptions(_) | TcpSetting::RepairWindow(_) => refuse(NO_REPAIR_OPTIONS),
        }
    }

    /// The SYN's headers that `TCP_SAVED_SYN` holds, read into the room given or, when longer, into `large`.
    fn take_saved_synchronize<'b>(
        &self,
        socket: BorrowedFd<'_>,
        room: &'b mut [u8],
        large: &'b mut Vec<u8>,
    ) -> Result<&'b [u8], HostError> {
        match options::get_bytes(socket, uapi::SOL_TCP, uapi::TCP_SAVED_SYN, room) {
            Ok(length) => Ok(&room[..length]),
            // Too small a buffer: the headers stay saved and can be read with room for any.
            Err(HostError::Errno(libc::EINVAL)) => {
                self.shortfalls.count(LONG_SAVED_SYNCHRONIZE);
                *large = vec![0; usize::from(u16::MAX)];
                let length = options::get_bytes(socket, uapi::SOL_TCP, uapi::TCP_SAVED_SYN, large)?;
                Ok(&large[..length])
            }
            Err(error) => Err(error),
        }
    }

    /// The interface an IPv6 SYN came in on, from the packet information the listener's `IPV6_RECVPKTINFO` kept,
    /// which the accepted connection goes on recording only when the guest asked for its records.
    fn ipv6_arrival_interface(
        &self,
        socket: BorrowedFd<'_>,
        records: bool,
    ) -> Option<InterfaceIndex> {
        let mut control = PacketOptions {
            _aligned: [],
            bytes: [0; PACKET_OPTIONS_ROOM],
        };
        let length = options::get_bytes(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_2292PKTOPTIONS,
            &mut control.bytes,
        )
        .ok()?;
        if !records {
            set_flag(socket, uapi::SOL_IPV6, uapi::IPV6_RECVPKTINFO, false).ok()?;
        }
        Ancillary::parse(&control.bytes[..length])
            .interface
            .map(InterfaceIndex)
    }

    fn synchronize_fields(
        &self,
        socket: BorrowedFd<'_>,
        header: &[u8],
        local: SocketAddr,
        records: bool,
    ) -> Result<ReceivedPacketFields, HostError> {
        let length = if let Ok(ipv4) = etherparse::Ipv4HeaderSlice::from_slice(header) {
            usize::from(ipv4.total_len())
        } else if let Ok(ipv6) = etherparse::Ipv6HeaderSlice::from_slice(header) {
            usize::from(ipv6.payload_length()) + etherparse::Ipv6Header::LEN
        } else {
            self.shortfalls.count(NO_SAVED_SYNCHRONIZE);
            0
        };
        let local = local.ip().to_canonical();
        let specific_destination = match local {
            IpAddr::V4(address) => Some(address),
            IpAddr::V6(_) => None,
        };
        let interface = match local {
            IpAddr::V6(_) => self.ipv6_arrival_interface(socket, records),
            IpAddr::V4(_) => None,
        }
        .or_else(|| {
            self.shortfalls.count(ARRIVAL_INTERFACE_BY_ADDRESS);
            self.interfaces.holding(local)
        })
        .unwrap_or(NO_INTERFACE);
        self.shortfalls.count(SYNCHRONIZE_FIELDS_OF_THE_SOCKET);
        Ok(ReceivedPacketFields {
            length,
            interface,
            specific_destination,
            largest_fragment: None,
            link_layer_length: None,
            timestamps: ReceiveTimestamps {
                software: None,
                hardware: None,
            },
            mark: PacketMark(options::get_option(
                socket,
                uapi::SOL_SOCKET,
                uapi::SO_MARK,
            )?),
            priority: PacketPriority(options::get_option(
                socket,
                uapi::SOL_SOCKET,
                uapi::SO_PRIORITY,
            )?),
            drops_since_previous: DropCount(0),
        })
    }

    /// Arms a connection's transmit reports with the packet or statistics a send asks for.
    fn arm_reports(
        &self,
        connection: &HostTcpConnection,
        reports: &mut TransmitReports,
        packet: bool,
        statistics: bool,
    ) -> Result<(), HostError> {
        let armed = reports.flags != 0;
        let with_packet = armed && reports.flags & WITHOUT_PACKET == 0;
        let with_statistics = reports.flags & WITH_STATISTICS != 0;
        if (packet && with_statistics) || (statistics && with_packet) {
            return Err(self.shortfalls.refuse(PACKET_OR_STATISTICS));
        }
        let mut flags = REPORT_FLAGS;
        if !(packet || with_packet) {
            flags |= WITHOUT_PACKET;
        }
        if statistics || with_statistics {
            flags |= WITH_STATISTICS;
        }
        if flags != reports.flags {
            options::set_timestamping(
                connection.socket.fd(),
                connection.receive_timestamping.load(Ordering::Relaxed) | flags,
                reports.bound_clock,
            )?;
            if !armed {
                reports.sent = 0;
            }
            reports.flags = flags;
            connection.reports_armed.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
}

impl<'a> Configurable<HostTcpConnection, TcpSetting<'a>> for HostTcpSockets {
    fn configure(
        &self,
        connection: &HostTcpConnection,
        setting: TcpSetting<'a>,
    ) -> Result<(), HostError> {
        let socket = connection.socket.fd();
        match setting {
            TcpSetting::Ip(IpSocketSetting::Timestamping(timestamping)) => {
                let mut reports = connection.reports();
                let receive = options::receive_timestamping_flags(&timestamping);
                options::set_timestamping(
                    socket,
                    receive | reports.flags,
                    timestamping.bound_clock,
                )?;
                connection
                    .receive_timestamping
                    .store(receive, Ordering::Relaxed);
                reports.bound_clock = timestamping.bound_clock;
                Ok(())
            }
            TcpSetting::UnreadByteReports(report) => {
                self.configure_socket(socket, connection.family, &setting, 0)?;
                connection
                    .unread_reports
                    .store(report == Report::Requested, Ordering::Relaxed);
                Ok(())
            }
            _ => self.configure_socket(socket, connection.family, &setting, 0),
        }
    }
}

impl<'a> Configurable<HostTcpListener, TcpListenerSetting<'a>> for HostTcpSockets {
    fn configure(
        &self,
        listener: &HostTcpListener,
        setting: TcpListenerSetting<'a>,
    ) -> Result<(), HostError> {
        let socket = listener.socket.fd();
        match setting {
            TcpListenerSetting::Socket(TcpSetting::Ip(IpSocketSetting::Timestamping(
                timestamping,
            ))) => {
                let receive = options::receive_timestamping_flags(&timestamping);
                options::set_timestamping(socket, receive, timestamping.bound_clock)?;
                listener
                    .receive_timestamping
                    .store(receive, Ordering::Relaxed);
                *listener
                    .bound_clock
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = timestamping.bound_clock;
                Ok(())
            }
            TcpListenerSetting::Socket(TcpSetting::RecordsLastReceivedHeader(report)) => {
                self.configure_socket(
                    socket,
                    listener.family,
                    &TcpSetting::RecordsLastReceivedHeader(report),
                    0,
                )?;
                // An IPv6 SYN's packet information still comes with each accepted connection, for its arrival interface.
                if listener.protocol == TcpProtocol::Tcp && listener.family == IpVersion::V6 {
                    set_flag(socket, uapi::SOL_IPV6, uapi::IPV6_RECVPKTINFO, true)?;
                }
                listener
                    .records_last_received
                    .store(report == Report::Requested, Ordering::Relaxed);
                Ok(())
            }
            TcpListenerSetting::Socket(setting) => {
                self.configure_socket(socket, listener.family, &setting, 0)
            }
            TcpListenerSetting::Backlog(backlog) => SockRef::from(&socket)
                .listen(i32::try_from(backlog.0).unwrap_or(i32::MAX))
                .map_err(io_host_error),
            TcpListenerSetting::FastOpenQueue(queue) => set_option(
                socket,
                uapi::SOL_TCP,
                uapi::TCP_FASTOPEN,
                &queue.map_or(0, |queue| queue.get()),
            ),
            TcpListenerSetting::FastOpenKeys(keys) => {
                let mut bytes = keys.primary.0.to_vec();
                if let Some(backup) = keys.backup {
                    bytes.extend_from_slice(&backup.0);
                }
                options::set_bytes(socket, uapi::SOL_TCP, uapi::TCP_FASTOPEN_KEY, &bytes)
            }
            // Every TCP listener saves at least its SYNs' network headers, which accept gives; the setting decides
            // whether the link header is saved too and whether the accepted connections keep them to be taken.
            TcpListenerSetting::SaveSynchronize(save) => {
                let value = match save {
                    TcpSavedSynchronize::Off => SAVE_NONE,
                    TcpSavedSynchronize::FromNetworkHeader => SAVE_FROM_NETWORK_HEADER,
                    TcpSavedSynchronize::FromLinkHeader => SAVE_FROM_LINK_HEADER,
                };
                let saved = match listener.protocol {
                    TcpProtocol::Tcp => value.max(SAVE_FROM_NETWORK_HEADER),
                    TcpProtocol::Multipath => value,
                };
                set_int(socket, uapi::SOL_TCP, uapi::TCP_SAVE_SYN, saved)?;
                listener.saves_synchronize.store(value, Ordering::Relaxed);
                Ok(())
            }
            TcpListenerSetting::FlowLabelReflection(on) => {
                // SAFETY: in6_flowlabel_req is plain data.
                let mut request: uapi::in6_flowlabel_req = unsafe { std::mem::zeroed() };
                request.flr_action = if on {
                    uapi::IPV6_FL_A_GET
                } else {
                    uapi::IPV6_FL_A_PUT
                } as u8;
                request.flr_flags = uapi::IPV6_FL_F_REFLECT as u16;
                match set_option(socket, uapi::SOL_IPV6, uapi::IPV6_FLOWLABEL_MGR, &request) {
                    // Turning off a reflection that is off.
                    Err(HostError::Errno(libc::ESRCH)) if !on => Ok(()),
                    other => other,
                }
            }
        }
    }
}

impl HostTcpSockets {
    /// Binds the socket as the binding asks, taking a port the exit node chooses at once: Linux chooses only inside
    /// the host's own port range, so the shared crate tries the rest of a range port by port.
    fn bind(&self, socket: BorrowedFd<'_>, local: &LocalBinding<'_>) -> Result<(), HostError> {
        match local.port {
            LocalPort::ChosenByExitNode(choice) => {
                let address = options::bind_address(socket, &local.address)?;
                options::bind_chosen_by_exit_node(socket, address, &choice, &self.shortfalls)
            }
            _ => bind(socket, local, false).map(drop),
        }
    }
}

impl NegativeRouteAdvice<HostTcpConnection> for HostTcpSockets {
    fn give_negative_route_advice(&self, connection: &HostTcpConnection) -> Result<(), HostError> {
        set_flag(
            connection.socket.fd(),
            uapi::SOL_SOCKET,
            uapi::SO_CNX_ADVICE,
            true,
        )
    }
}

impl TcpConnections for HostTcpSockets {
    type Connection = HostTcpConnection;

    fn connect(
        &self,
        protocol: TcpProtocol,
        owner: UserId,
        local: LocalBinding<'_>,
        peer: SocketAddr,
        settings: &[TcpSetting<'_>],
        fast_open: TcpFastOpen<'_>,
    ) -> Result<TcpConnected<Self::Connection>, HostError> {
        let family = family_of(&local.address);
        let connection = HostTcpConnection::new(
            self.readiness.watch(open_socket(
                family,
                SocketType::STREAM,
                protocol_of(protocol),
                owner,
            )?)?,
            family,
            0,
        );
        for setting in settings {
            self.configure(&connection, setting.clone())?;
        }
        let socket = connection.socket.fd();
        self.bind(socket, &local)?;
        let peer = peer_for(family, peer);
        let in_progress = |outcome: Result<usize, HostError>| match outcome {
            Err(HostError::Errno(libc::EINPROGRESS)) => Ok(None),
            other => other.map(Some),
        };
        let start = match fast_open {
            TcpFastOpen::Off | TcpFastOpen::UntilFirstSend => {
                if fast_open == TcpFastOpen::UntilFirstSend {
                    set_flag(socket, uapi::SOL_TCP, uapi::TCP_FASTOPEN_CONNECT, true)?;
                }
                let outcome = in_progress(
                    SockRef::from(&socket)
                        .connect(&SockAddr::from(peer))
                        .map(|()| 0)
                        .map_err(io_host_error),
                )?;
                match outcome {
                    None => TcpConnectStart::SynchronizeSent {
                        fast_open_data_taken: ByteCount(0),
                    },
                    Some(_) if fast_open == TcpFastOpen::UntilFirstSend => {
                        TcpConnectStart::DeferredUntilFirstSend
                    }
                    Some(_) => TcpConnectStart::EstablishedForRepair,
                }
            }
            TcpFastOpen::WithData(data) => {
                let flags = libc::MSG_FASTOPEN | libc::MSG_NOSIGNAL | libc::MSG_DONTWAIT;
                let taken = in_progress(
                    SockRef::from(&socket)
                        .send_to_with_flags(data, &SockAddr::from(peer), flags)
                        .map_err(io_host_error),
                )?;
                TcpConnectStart::SynchronizeSent {
                    fast_open_data_taken: ByteCount(taken.unwrap_or(0)),
                }
            }
        };
        let local = internet_address(SockRef::from(&socket).local_addr())?;
        let _ = connection.addresses.set((local, peer));
        Ok(TcpConnected {
            connection,
            local,
            start,
        })
    }

    fn send(
        &self,
        connection: &Self::Connection,
        request: TcpSendRequest,
        bytes: &[u8],
    ) -> Result<ByteCount, HostError> {
        let socket = connection.socket.fd();
        let mut flags = libc::MSG_NOSIGNAL | libc::MSG_DONTWAIT;
        if request.urgency == TcpUrgency::Urgent {
            flags |= libc::MSG_OOB;
        }
        if request.continuation == TcpContinuation::MoreToCome {
            flags |= libc::MSG_MORE;
        }
        if request.record == TcpRecordBoundary::Ended {
            flags |= libc::MSG_EOR;
        }
        let asked = request.transmit_reports;
        let packet = asked.packet == Report::Requested;
        let statistics = request.transmit_statistics == Report::Requested;
        let (record, expected) = record_flags(&asked);
        if record == 0
            && asked.wireless_status == Report::NotRequested
            && !connection.reports_armed.load(Ordering::Relaxed)
        {
            return SockRef::from(&socket)
                .send_with_flags(bytes, flags)
                .map(ByteCount)
                .map_err(io_host_error);
        }
        let mut reports = connection.reports();
        if asked.wireless_status == Report::Requested && !reports.wireless_status {
            set_flag(socket, uapi::SOL_SOCKET, uapi::SO_WIFI_STATUS, true)?;
            reports.wireless_status = true;
        }
        let taken = if record == 0 {
            SockRef::from(&socket)
                .send_with_flags(bytes, flags)
                .map_err(io_host_error)?
        } else {
            self.arm_reports(connection, &mut reports, packet, statistics)?;
            let taken = send_with_record_flags(socket, bytes, flags, record)?;
            if taken > 0 {
                let last = reports.sent.wrapping_add(taken as u32).wrapping_sub(1);
                reports.keys.insert(
                    last,
                    ReportedSend {
                        key: asked.key,
                        remaining: expected,
                        packet,
                        statistics,
                    },
                );
            }
            taken
        };
        if reports.flags != 0 {
            reports.sent = reports.sent.wrapping_add(taken as u32);
        }
        Ok(ByteCount(taken))
    }

    fn receive(
        &self,
        connection: &Self::Connection,
        request: TcpReceiveRequest,
        into: &mut [u8],
    ) -> Result<TcpReceived, HostError> {
        let _receiving = locked(&connection.receiving);
        let socket = connection.socket.fd();
        let mut flags = libc::MSG_DONTWAIT;
        let mut offset = 0;
        match request {
            TcpReceiveRequest::Read => {}
            TcpReceiveRequest::Peek { offset: from } => {
                flags |= libc::MSG_PEEK;
                offset = count(from.0)?;
            }
            TcpReceiveRequest::ReadUrgentByte => flags |= libc::MSG_OOB,
            TcpReceiveRequest::PeekUrgentByte => flags |= libc::MSG_OOB | libc::MSG_PEEK,
        }
        if offset != 0 {
            set_int(socket, uapi::SOL_SOCKET, uapi::SO_PEEK_OFF, offset)?;
        }
        let timestamping = connection.receive_timestamping.load(Ordering::Relaxed);
        let unread_reports = connection.unread_reports.load(Ordering::Relaxed);
        let outcome = if timestamping == 0 && !unread_reports {
            SockRef::from(&socket)
                .recv_with_flags(uninitialised(into), flags)
                .map(|length| TcpReceived {
                    length: ByteCount(length),
                    timestamps: ReceiveTimestamps {
                        software: None,
                        hardware: None,
                    },
                    unread_after: None,
                })
                .map_err(io_host_error)
        } else {
            let mut control = ControlBuffer::default();
            options::receive_message(socket, into, flags, &mut control).map(|message| {
                if message.flags & libc::MSG_CTRUNC != 0 {
                    self.shortfalls.count(CUT_CONTROL);
                }
                let mut received = TcpReceived {
                    length: ByteCount(message.length),
                    timestamps: ReceiveTimestamps {
                        software: None,
                        hardware: None,
                    },
                    unread_after: None,
                };
                for (level, kind, data) in message.messages() {
                    if level == libc::SOL_SOCKET && kind == libc::SCM_TIMESTAMPING {
                        let (software, hardware) = options::timestamps(data);
                        if timestamping & uapi::SOF_TIMESTAMPING_RX_SOFTWARE as u32 != 0 {
                            received.timestamps.software = software;
                        }
                        if timestamping & uapi::SOF_TIMESTAMPING_RX_HARDWARE as u32 != 0 {
                            received.timestamps.hardware = hardware;
                        }
                    } else if level == libc::SOL_TCP
                        && kind == uapi::TCP_CM_INQ as c_int
                        && let Ok(unread) = <[u8; size_of::<c_int>()]>::try_from(data)
                    {
                        received.unread_after = usize::try_from(c_int::from_ne_bytes(unread))
                            .ok()
                            .map(ByteCount);
                    }
                }
                received
            })
        };
        if offset != 0 {
            set_int(socket, uapi::SOL_SOCKET, uapi::SO_PEEK_OFF, -1)?;
        }
        outcome
    }

    fn take_error_queue_entry(
        &self,
        connection: &Self::Connection,
        into: &mut [u8],
    ) -> Result<TcpErrorQueueEntry, HostError> {
        let (entry, packet_cut) = self.next_error_queue_entry(connection, into)?;
        if packet_cut {
            self.shortfalls.count(CUT_REPORT_PACKET);
        }
        Ok(entry)
    }

    fn read_state<'a>(
        &self,
        connection: &Self::Connection,
        query: TcpQuery<'a>,
    ) -> Result<TcpReading<'a>, HostError> {
        match query {
            // As `TCP_SAVED_SYN` does: too small a buffer keeps the headers and tells their length.
            TcpQuery::TakeSavedSynchronize(into) => {
                let mut saved = locked(&connection.saved_synchronize);
                let header = match saved.as_deref() {
                    None => TcpSavedSynchronizeHeader::Absent,
                    Some(header) if header.len() > into.len() => TcpSavedSynchronizeHeader::Kept {
                        length: ByteCount(header.len()),
                    },
                    Some(header) => {
                        let length = header.len();
                        into[..length].copy_from_slice(header);
                        *saved = None;
                        TcpSavedSynchronizeHeader::Taken(&into[..length])
                    }
                };
                Ok(TcpReading::SavedSynchronize(header))
            }
            _ => Err(self.shortfalls.refuse(NO_CONNECTION_STATE)),
        }
    }

    fn shut_down(
        &self,
        connection: &Self::Connection,
        directions: TcpShutdown,
    ) -> Result<(), HostError> {
        let how = match directions {
            TcpShutdown::Read => std::net::Shutdown::Read,
            TcpShutdown::Write => std::net::Shutdown::Write,
            TcpShutdown::ReadAndWrite => std::net::Shutdown::Both,
        };
        SockRef::from(&connection.socket.fd())
            .shutdown(how)
            .map_err(io_host_error)
    }

    async fn wait_for_events(
        &self,
        connection: &Self::Connection,
        interest: PollEvents,
    ) -> Result<PollEvents, HostError> {
        connection.socket.wait_for_events(interest).await
    }

    fn close(&self, connection: Self::Connection, manner: TcpClose) {
        if manner == TcpClose::Abort {
            // A linger of zero makes the close send a reset; a failure leaves the orderly close.
            if SockRef::from(&connection.socket.fd())
                .set_linger(Some(Duration::ZERO))
                .is_err()
            {
                self.shortfalls.count(ABORT_CLOSED_IN_ORDER);
            }
        }
        drop(connection);
    }

    fn push_queued_data(
        &self,
        connection: &Self::Connection,
        ignoring: TcpPushOverride,
    ) -> Result<(), HostError> {
        let socket = connection.socket.fd();
        match ignoring {
            // Turning no-delay on pushes past the cork and Nagle's algorithm; it is then put back.
            TcpPushOverride::CorkAndNagle => {
                let socket = SockRef::from(&socket);
                let no_delay = socket.tcp_nodelay().map_err(io_host_error)?;
                socket.set_tcp_nodelay(true).map_err(io_host_error)?;
                if !no_delay {
                    socket.set_tcp_nodelay(false).map_err(io_host_error)?;
                }
                Ok(())
            }
            // Writing the cork as it is pushes when uncorked; under a cork everything a push may send is sent already.
            TcpPushOverride::Neither => {
                let socket = SockRef::from(&socket);
                let corked = socket.tcp_cork().map_err(io_host_error)?;
                socket.set_tcp_cork(corked).map_err(io_host_error)
            }
        }
    }

    fn send_delayed_acknowledgement(
        &self,
        connection: &Self::Connection,
        afterwards: TcpQuickAcknowledgement,
    ) -> Result<(), HostError> {
        let socket = connection.socket.fd();
        let quick = SockRef::from(&socket);
        let before = quick.tcp_quickack().map_err(io_host_error)?;
        // Quick mode off afterwards is the sign that one was scheduled.
        set_int(
            socket,
            uapi::SOL_TCP,
            uapi::TCP_QUICKACK,
            QUICK_ACKNOWLEDGEMENT_THEN_DELAYING,
        )?;
        let after = quick.tcp_quickack().map_err(io_host_error)?;
        let scheduled = !after;
        let wanted = if scheduled {
            afterwards == TcpQuickAcknowledgement::On
        } else {
            before
        };
        if wanted != after {
            quick.set_tcp_quickack(wanted).map_err(io_host_error)?;
        }
        Ok(())
    }

    fn rearm_keep_alive_timer(&self, connection: &Self::Connection) -> Result<(), HostError> {
        // Writing the idle time re-times the probe from the last data or acknowledgement received.
        let socket = connection.socket.fd();
        let idle = get_int(socket, uapi::SOL_TCP, uapi::TCP_KEEPIDLE)?;
        set_int(socket, uapi::SOL_TCP, uapi::TCP_KEEPIDLE, idle)
    }

    fn discard_error_queue_entries(&self, connection: &Self::Connection) -> Result<(), HostError> {
        let mut scratch = [0; 0];
        loop {
            match self.next_error_queue_entry(connection, &mut scratch) {
                Ok(_) | Err(HostError::NotSupported) => {}
                Err(HostError::WouldBlock) => return Ok(()),
                Err(error) => return Err(error),
            }
        }
    }

    fn send_window_probe(&self, connection: &Self::Connection) -> Result<(), HostError> {
        // Leaving repair mode with TCP_REPAIR_OFF sends a window probe; it also clears address sharing and the
        // repair mode itself, which are put back as they were.
        let socket = connection.socket.fd();
        let sharing = SockRef::from(&socket)
            .reuse_address()
            .map_err(io_host_error)?;
        let repair = |value: u32| set_int(socket, uapi::SOL_TCP, uapi::TCP_REPAIR, value as c_int);
        let repairing = get_int(socket, uapi::SOL_TCP, uapi::TCP_REPAIR)? != 0;
        let queue = if repairing {
            get_int(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE)?
        } else {
            repair(uapi::TCP_REPAIR_ON)?;
            uapi::TCP_NO_QUEUE as c_int
        };
        repair(uapi::TCP_REPAIR_OFF)?;
        if repairing {
            repair(uapi::TCP_REPAIR_ON)?;
            set_int(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE, queue)?;
        } else if sharing {
            SockRef::from(&socket)
                .set_reuse_address(true)
                .map_err(io_host_error)?;
        }
        Ok(())
    }

    fn set_timestamp_clock(
        &self,
        connection: &Self::Connection,
        reading: TcpTimestampClock,
    ) -> Result<(), HostError> {
        self.set_timestamp_clock(connection.socket.fd(), reading)
    }

    fn set_authentication_counters(
        &self,
        _connection: &Self::Connection,
        _values: TcpAuthenticationCounters,
    ) -> Result<(), HostError> {
        Err(self.shortfalls.refuse(NO_AUTHENTICATION))
    }
}

impl HostTcpSockets {
    /// Sets the clock, which counts in the unit its route gave the socket (`tcp_usec_ts`): Linux cannot change it.
    fn set_timestamp_clock(
        &self,
        socket: BorrowedFd<'_>,
        reading: TcpTimestampClock,
    ) -> Result<(), HostError> {
        let information: uapi::tcp_info =
            options::get_option(socket, uapi::SOL_TCP, uapi::TCP_INFO)?;
        let microseconds = u32::from(information.tcpi_options) & uapi::TCPI_OPT_USEC_TS != 0;
        if microseconds != (reading.unit == TcpTimestampUnit::Microseconds) {
            return Err(self.shortfalls.refuse(CLOCK_UNIT_OF_THE_ROUTE));
        }
        let value = reading.value.0 & !1 | u32::from(microseconds);
        set_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP, &value)
    }
}

impl TcpListeners for HostTcpSockets {
    type Listener = HostTcpListener;

    fn listen(
        &self,
        protocol: TcpProtocol,
        owner: UserId,
        local: LocalBinding<'_>,
        backlog: ListenBacklog,
        settings: &[TcpListenerSetting<'_>],
    ) -> Result<TcpListening<Self::Listener>, HostError> {
        let family = family_of(&local.address);
        let listener = HostTcpListener {
            socket: self.readiness.watch(open_socket(
                family,
                SocketType::STREAM,
                protocol_of(protocol),
                owner,
            )?)?,
            family,
            protocol,
            receive_timestamping: AtomicU32::new(0),
            bound_clock: Mutex::new(None),
            saves_synchronize: AtomicI32::new(SAVE_NONE),
            records_last_received: AtomicBool::new(false),
        };
        let socket = listener.socket.fd();
        // Multipath sockets have no TCP_SAVE_SYN.
        if protocol == TcpProtocol::Tcp {
            set_flag(socket, uapi::SOL_TCP, uapi::TCP_SAVE_SYN, true)?;
            // An IPv6 SYN's packet information, with its arrival interface, then comes with each accepted connection.
            if family == IpVersion::V6 {
                set_flag(socket, uapi::SOL_IPV6, uapi::IPV6_RECVPKTINFO, true)?;
            }
        }
        let (backlogs, others): (Vec<_>, Vec<_>) = settings
            .iter()
            .partition(|setting| matches!(setting, TcpListenerSetting::Backlog(_)));
        for setting in others {
            self.configure(&listener, setting.clone())?;
        }
        self.bind(socket, &local)?;
        SockRef::from(&socket)
            .listen(i32::try_from(backlog.0).unwrap_or(i32::MAX))
            .map_err(io_host_error)?;
        // A backlog among the settings is listening again with it.
        for setting in backlogs {
            self.configure(&listener, setting.clone())?;
        }
        let local = internet_address(SockRef::from(&socket).local_addr())?;
        Ok(TcpListening { listener, local })
    }

    fn accept<'a>(
        &self,
        listener: &Self::Listener,
        synchronize_header: &'a mut [u8],
    ) -> Result<TcpAccepted<'a, Self::Connection>, HostError> {
        let (socket, peer) = SockRef::from(&listener.socket.fd())
            .accept4(libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC)
            .map_err(io_host_error)?;
        let peer = internet_address(Ok(peer))?;
        let connection = HostTcpConnection::new(
            self.readiness.watch(OwnedFd::from(socket))?,
            listener.family,
            listener.receive_timestamping.load(Ordering::Relaxed),
        );
        let bound_clock = *locked(&listener.bound_clock);
        if bound_clock.is_some() {
            connection.reports().bound_clock = bound_clock;
        }
        let socket = connection.socket.fd();
        let local = internet_address(SockRef::from(&socket).local_addr())?;
        let _ = connection.addresses.set((local, peer));
        let mut room = [0; SAVED_SYNCHRONIZE_ROOM];
        let mut large = Vec::new();
        let saved = match listener.protocol {
            TcpProtocol::Tcp => self.take_saved_synchronize(socket, &mut room, &mut large)?,
            TcpProtocol::Multipath => &[],
        };
        // The headers may start with the link header, as the listener saved them when the SYN came.
        let network = if saved.is_empty() {
            saved
        } else if let Some(start) = network_header_start(saved, local, peer) {
            &saved[start..]
        } else {
            self.shortfalls.count(NO_NETWORK_HEADER);
            &[]
        };
        let kept = match listener.saves_synchronize.load(Ordering::Relaxed) {
            SAVE_NONE => None,
            SAVE_FROM_NETWORK_HEADER => Some(network),
            _ => Some(saved),
        };
        if let Some(kept) = kept.filter(|kept| !kept.is_empty()) {
            *locked(&connection.saved_synchronize) = Some(kept.to_vec());
        }
        let records = listener.records_last_received.load(Ordering::Relaxed);
        let synchronize = self.synchronize_fields(socket, network, local, records)?;
        let length = network.len().min(synchronize_header.len());
        synchronize_header[..length].copy_from_slice(&network[..length]);
        Ok(TcpAccepted {
            connection,
            peer,
            local,
            synchronize,
            synchronize_header: &synchronize_header[..length],
        })
    }

    fn read_listener_state<'a>(
        &self,
        _listener: &Self::Listener,
        _query: TcpListenerQuery<'a>,
    ) -> Result<TcpReading<'a>, HostError> {
        Err(self.shortfalls.refuse(NO_LISTENER_STATE))
    }

    fn set_listener_authentication_counters(
        &self,
        _listener: &Self::Listener,
        _values: TcpAuthenticationCounters,
    ) -> Result<(), HostError> {
        Err(self.shortfalls.refuse(NO_AUTHENTICATION))
    }

    fn set_listener_timestamp_clock(
        &self,
        listener: &Self::Listener,
        reading: TcpTimestampClock,
    ) -> Result<(), HostError> {
        self.set_timestamp_clock(listener.socket.fd(), reading)
    }

    async fn wait_for_listener_events(
        &self,
        listener: &Self::Listener,
        interest: PollEvents,
    ) -> Result<PollEvents, HostError> {
        listener.socket.wait_for_events(interest).await
    }

    fn stop_listening(&self, listener: Self::Listener) {
        drop(listener);
    }
}

impl TcpSockets for HostTcpSockets {}

#[cfg(test)]
mod tests {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::net::Ipv4Addr;
    use std::pin::pin;
    use std::sync::Arc;
    use std::task::{Context, Wake, Waker};
    use std::thread::Thread;

    use wcontainers_internet_protocol::{BindAddress, Port, Timestamping};

    use super::*;

    wcontainers_tcp_sockets_conformance::tests!(
        super::HostTcpSockets::new().expect("the host's readiness mechanism starts")
    );

    /// The system's allocator, counting the allocations each thread makes.
    struct Counting;

    thread_local! {
        static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    }

    // SAFETY: every call goes straight to the system's allocator.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
            // SAFETY: as the caller promised.
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            // SAFETY: as the caller promised.
            unsafe { System.dealloc(pointer, layout) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: Counting = Counting;

    fn allocations() -> usize {
        ALLOCATIONS.with(Cell::get)
    }

    struct Unparking(Thread);

    impl Wake for Unparking {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    /// The cycles each allocation check measures, after one to warm up.
    const MEASURED_CYCLES: usize = 100;

    /// How long a wait may take to be woken once its data is sent.
    const DEADLINE: Duration = Duration::from_secs(10);

    /// A listener and the two ends of a connection to it, on IPv4 loopback.
    fn connected(
        sockets: &HostTcpSockets,
    ) -> (HostTcpListener, HostTcpConnection, HostTcpConnection) {
        let loopback = LocalBinding {
            address: BindAddress::V4(Ipv4Addr::LOCALHOST),
            port: LocalPort::Requested(Port(0)),
        };
        let listening = sockets
            .listen(TcpProtocol::Tcp, UserId(0), loopback, ListenBacklog(1), &[])
            .expect("listen");
        let client = sockets
            .connect(
                TcpProtocol::Tcp,
                UserId(0),
                loopback,
                listening.local,
                &[],
                TcpFastOpen::Off,
            )
            .expect("connect")
            .connection;
        let mut header = [0; 0];
        let server = loop {
            match sockets.accept(&listening.listener, &mut header) {
                Ok(accepted) => break accepted.connection,
                Err(HostError::WouldBlock) => std::thread::yield_now(),
                Err(error) => panic!("accept: {error:?}"),
            }
        };
        (listening.listener, client, server)
    }

    fn events(data_to_read: bool, error_condition: bool) -> PollEvents {
        PollEvents {
            data_to_read,
            exceptional_condition: false,
            writing_possible: false,
            error_condition,
            hang_up: false,
            descriptor_not_open: false,
            normal_data_to_read: false,
            priority_band_data_to_read: false,
            normal_data_writable: false,
            priority_data_writable: false,
            message_available: false,
            peer_shut_down_writing: false,
        }
    }

    fn reports(software: Report, packet: Report) -> TcpSendRequest {
        TcpSendRequest {
            urgency: TcpUrgency::Normal,
            continuation: TcpContinuation::Push,
            record: TcpRecordBoundary::Open,
            transmit_reports: TransmitReportRequest {
                key: TransmitReportKey(0),
                scheduled: Report::NotRequested,
                software,
                hardware: Report::NotRequested,
                acknowledgement: Report::NotRequested,
                completion: Report::NotRequested,
                wireless_status: Report::NotRequested,
                packet,
            },
            transmit_statistics: Report::NotRequested,
        }
    }

    fn plain() -> TcpSendRequest {
        reports(Report::NotRequested, Report::NotRequested)
    }

    /// Waits on the connection until one of the events comes.
    fn wait(
        sockets: &HostTcpSockets,
        connection: &HostTcpConnection,
        interest: PollEvents,
        waker: &Waker,
    ) {
        let mut context = Context::from_waker(waker);
        let mut wait = pin!(sockets.wait_for_events(connection, interest));
        let start = std::time::Instant::now();
        while wait.as_mut().poll(&mut context).is_pending() {
            assert!(start.elapsed() < DEADLINE, "the wait was not woken");
            std::thread::park_timeout(DEADLINE);
        }
    }

    /// Once a connection has waited, its waits that have to wait allocate nothing on the waiting thread.
    #[test]
    fn a_wait_allocates_nothing_once_warm() {
        let sockets = HostTcpSockets::new().expect("readiness");
        let (_listener, client, server) = connected(&sockets);
        let waker = Waker::from(Arc::new(Unparking(std::thread::current())));
        let interest = events(true, false);
        let mut byte = [0; 1];
        let mut cycle = || {
            let mut wait = pin!(sockets.wait_for_events(&server, interest));
            let mut context = Context::from_waker(&waker);
            assert!(wait.as_mut().poll(&mut context).is_pending());
            sockets.send(&client, plain(), &byte).expect("send");
            let start = std::time::Instant::now();
            while wait.as_mut().poll(&mut context).is_pending() {
                assert!(start.elapsed() < DEADLINE, "the wait was not woken");
                std::thread::park_timeout(DEADLINE);
            }
            sockets
                .receive(&server, TcpReceiveRequest::Read, &mut byte)
                .expect("receive");
        };
        cycle();
        let before = allocations();
        for _ in 0..MEASURED_CYCLES {
            cycle();
        }
        assert_eq!(allocations() - before, 0);
        let counted = Box::new(0_u8);
        assert_eq!(allocations() - before, 1, "the allocator counts");
        drop(counted);
    }

    /// A receive that brings control messages, a timestamp and the unread byte count, allocates nothing.
    #[test]
    fn a_receive_with_control_messages_allocates_nothing() {
        let sockets = HostTcpSockets::new().expect("readiness");
        let (_listener, client, server) = connected(&sockets);
        let timestamping = Timestamping {
            receive_software: Report::Requested,
            receive_hardware: Report::NotRequested,
            bound_clock: None,
        };
        sockets
            .configure(
                &server,
                TcpSetting::Ip(IpSocketSetting::Timestamping(timestamping)),
            )
            .expect("timestamping");
        sockets
            .configure(&server, TcpSetting::UnreadByteReports(Report::Requested))
            .expect("unread byte reports");
        let waker = Waker::from(Arc::new(Unparking(std::thread::current())));
        let mut byte = [0; 1];
        let mut cycle = || {
            sockets.send(&client, plain(), &byte).expect("send");
            wait(&sockets, &server, events(true, false), &waker);
            let received = sockets
                .receive(&server, TcpReceiveRequest::Read, &mut byte)
                .expect("receive");
            assert!(received.timestamps.software.is_some());
            assert_eq!(received.unread_after, Some(ByteCount(0)));
        };
        cycle();
        let before = allocations();
        for _ in 0..MEASURED_CYCLES {
            cycle();
        }
        assert_eq!(allocations() - before, 0);
    }

    /// The peeks of each thread in the two-thread peek check.
    const PEEK_ROUNDS: usize = 20_000;

    /// Peeks at an offset on one thread leave a plain peek on another starting at the first unread byte.
    #[test]
    fn peeks_from_two_threads_each_start_where_asked() {
        let sockets = HostTcpSockets::new().expect("readiness");
        let (_listener, client, server) = connected(&sockets);
        let message = b"two threads";
        let offset = message.len() / 2;
        sockets.send(&client, plain(), message).expect("send");
        let waker = Waker::from(Arc::new(Unparking(std::thread::current())));
        wait(&sockets, &server, events(true, false), &waker);
        let peek = |from: usize| {
            let mut buffer = [0; 16];
            sockets
                .receive(
                    &server,
                    TcpReceiveRequest::Peek {
                        offset: ByteCount(from),
                    },
                    &mut buffer,
                )
                .map(|got| buffer[..got.length.0].to_vec())
        };
        let start = std::time::Instant::now();
        while peek(0).map_or(true, |bytes| bytes != message) {
            assert!(start.elapsed() < DEADLINE, "the message never arrived");
            std::thread::yield_now();
        }
        std::thread::scope(|scope| {
            for from in [offset, 0] {
                scope.spawn(move || {
                    for _ in 0..PEEK_ROUNDS {
                        assert_eq!(peek(from), Ok(message[from..].to_vec()), "peek at {from}");
                    }
                });
            }
        });
    }

    /// Discarding a transmit report that carries its packet counts no packet cut to a buffer.
    #[test]
    fn discarding_cuts_no_packet() {
        let sockets = HostTcpSockets::new().expect("readiness");
        let (_listener, client, _server) = connected(&sockets);
        sockets
            .send(
                &client,
                reports(Report::Requested, Report::Requested),
                b"reported",
            )
            .expect("send");
        let waker = Waker::from(Arc::new(Unparking(std::thread::current())));
        wait(&sockets, &client, events(false, true), &waker);
        sockets
            .discard_error_queue_entries(&client)
            .expect("discard");
        let mut room = [0; 0];
        assert_eq!(
            sockets.take_error_queue_entry(&client, &mut room).err(),
            Some(HostError::WouldBlock)
        );
        let counters = sockets.counters();
        assert!(
            counters
                .iter()
                .all(|counter| counter.reason != CUT_REPORT_PACKET),
            "{counters:?}"
        );
    }
}
