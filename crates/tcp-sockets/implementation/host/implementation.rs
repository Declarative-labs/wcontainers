//! TCP and MPTCP connections and listeners made of the Linux sockets of the machine it runs on.
#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::ffi::c_int;
use std::mem::size_of;
use std::net::{IpAddr, SocketAddr};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use etherparse::IpNumber;
use linux_raw_sys::net as uapi;
use netlink_packet_core::{NlasIterator, parse_u8, parse_u16, parse_u32, parse_u64};
use rustix::net::{RecvFlags, SendFlags, Shutdown, SocketFlags, SocketType, ipproto, sockopt};
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    ByteCount, BytesPerSecond, DropCount, HopLimit, InterfaceIndex, IpSocketSetting, IpVersion,
    LocalBinding, LocalPort, PacketMark, PacketPriority, PtpHardwareClockIndex, ReceiveTimestamps,
    ReceivedPacketFields, Report, TransmitEvent, TransmitReport, TransmitReportKey,
    TransmitReportRequest, WirelessAcknowledgement,
};
use wcontainers_internet_protocol_linux_socket_options::datagram::receive::Ancillary;
use wcontainers_internet_protocol_linux_socket_options::icmp_error::write_icmp_error;
use wcontainers_internet_protocol_linux_socket_options::{
    self as options, ControlBuffer, Shortfalls, bind, family_of, get_int, host_error, open_socket,
    peer_for, set_flag, set_int, set_option, socket_address,
};
use wcontainers_kernel::UserId;
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

/// The room `TCP_SAVED_SYN` is first read with: the largest IPv4 and TCP headers.
const SAVED_SYNCHRONIZE_ROOM: usize =
    etherparse::Ipv4Header::MAX_LEN + etherparse::TcpHeader::MAX_LEN;

/// The reasons the host could not honour a request, each counted by [`Shortfalls`].
const NO_LAST_RECEIVED_HEADER: &str =
    "the last received header of a TCP socket is not mapped to the host yet";
const NO_AUTHENTICATION: &str = "TCP authentication is not mapped to the host yet";
const NO_REPAIR_OPTIONS: &str = "TCP repair options and windows are not mapped to the host yet";
const NO_SAVED_SYNCHRONIZE: &str = "the host kept no SYN headers for an accepted connection";
const ARRIVAL_INTERFACE_BY_ADDRESS: &str = "the SYN's arrival interface is taken as the host interface whose address or prefix holds the local address";
const SYNCHRONIZE_FIELDS_OF_THE_SOCKET: &str = "Linux reports no priority, timestamps or drops for a SYN, and its mark only as the accepted socket's: the accepted socket's mark and priority are given, with no timestamps or drops";
const NO_LINK_HEADERS: &str = "saving the link headers of SYNs is not mapped to the host yet";
const PACKET_OR_STATISTICS: &str =
    "Linux gives a TCP socket's transmit reports either the packet or statistics, never both";
const NO_OTHER_ERROR_ENTRIES: &str = "error queue entries other than ICMP errors, transmit timestamps and wireless status have no TCP counterpart";
const NO_EXTENDED_ERROR: &str = "an error queue entry came without its extended error";
const UNWRITABLE_ICMP_ERROR: &str = "an ICMP error's addresses or lengths fit no IP header";
const CUT_REPORT_PACKET: &str = "a transmit report's packet was cut to the buffer";
const NO_STATISTICS: &str = "a transmit report came without the statistics its send asked for";
const CUT_CONTROL: &str = "the control messages of an error queue entry did not fit";
const UNASKED_REPORT: &str = "a transmit report for no send that asked for it";
const NO_CONNECTION_STATE: &str = "reading a TCP connection's state is not mapped to the host yet";
const ABORT_CLOSED_IN_ORDER: &str = "an abort closed in order: the host refused a linger of zero";
const NO_LISTENER_STATE: &str = "reading a TCP listener's state is not mapped to the host yet";

/// Linux's TCP sockets as the exit node's TCP connections and listeners.
#[derive(Debug, Default)]
pub struct LinuxHostTcpSockets {
    shortfalls: Shortfalls,
    interfaces: InterfaceAddresses,
}

impl LinuxHostTcpSockets {
    pub fn new() -> Self {
        Self::default()
    }

    /// The requests answered as unsupported so far, by reason.
    pub fn counters(&self) -> Vec<CounterReading> {
        self.shortfalls.readings()
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

#[derive(Debug)]
pub struct LinuxHostTcpConnection {
    socket: OwnedFd,
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
}

#[derive(Debug)]
pub struct LinuxHostTcpListener {
    socket: OwnedFd,
    family: IpVersion,
    protocol: TcpProtocol,
    receive_timestamping: AtomicU32,
    bound_clock: Mutex<Option<PtpHardwareClockIndex>>,
    /// The guest's `TCP_SAVE_SYN`: whether accepted connections keep their SYN's headers to be taken.
    saves_synchronize: AtomicBool,
}

macro_rules! ordered_by_descriptor {
    ($type:ty) => {
        impl PartialEq for $type {
            fn eq(&self, other: &Self) -> bool {
                self.socket.as_raw_fd() == other.socket.as_raw_fd()
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
                self.socket.as_raw_fd().cmp(&other.socket.as_raw_fd())
            }
        }
    };
}
ordered_by_descriptor!(LinuxHostTcpConnection);
ordered_by_descriptor!(LinuxHostTcpListener);

impl LinuxHostTcpConnection {
    fn new(socket: OwnedFd, family: IpVersion, receive_timestamping: u32) -> Self {
        Self {
            socket,
            family,
            addresses: OnceLock::new(),
            receive_timestamping: AtomicU32::new(receive_timestamping),
            unread_reports: AtomicBool::new(false),
            reports_armed: AtomicBool::new(false),
            reports: Mutex::default(),
            saved_synchronize: Mutex::default(),
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

/// The room `IPV6_2292PKTOPTIONS` gives the SYN's packet information in: one control message, padded.
const PACKET_OPTIONS_ROOM: usize =
    size_of::<libc::cmsghdr>() + size_of::<uapi::in6_pktinfo>() + size_of::<usize>();

/// Control message room aligned as the messages' headers are.
#[repr(C, align(8))]
struct PacketOptions([u8; PACKET_OPTIONS_ROOM]);

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
    let mut control = ControlBuffer::default();
    let mut vector = libc::iovec {
        iov_base: bytes.as_ptr().cast_mut().cast(),
        iov_len: bytes.len(),
    };
    // SAFETY: msghdr is plain data; the fields set below are the only ones the call reads.
    let mut header: libc::msghdr = unsafe { std::mem::zeroed() };
    header.msg_iov = &mut vector;
    header.msg_iovlen = 1;
    // SAFETY: CMSG_SPACE only computes a size.
    header.msg_controllen = unsafe { libc::CMSG_SPACE(size_of::<u32>() as u32) } as _;
    header.msg_control = (&raw mut control).cast();
    // SAFETY: the control buffer holds one message of a u32, which is written within it; the kernel only reads the
    // bytes and the vector.
    let sent = unsafe {
        let message = libc::CMSG_FIRSTHDR(&header);
        (*message).cmsg_level = libc::SOL_SOCKET;
        (*message).cmsg_type = libc::SO_TIMESTAMPING;
        (*message).cmsg_len = libc::CMSG_LEN(size_of::<u32>() as u32) as _;
        libc::CMSG_DATA(message)
            .cast::<u32>()
            .write_unaligned(record);
        libc::sendmsg(socket.as_raw_fd(), &header, flags)
    };
    if sent < 0 {
        Err(options::last_host_error())
    } else {
        Ok(sent as usize)
    }
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

impl LinuxHostTcpSockets {
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
            TcpSetting::RecordsLastReceivedHeader(_) => refuse(NO_LAST_RECEIVED_HEADER),
            TcpSetting::IcmpErrorReports { version, reports } => {
                let on = reports == Report::Requested;
                match version {
                    IpVersion::V4 => set_flag(socket, uapi::SOL_IP, uapi::IP_RECVERR, on),
                    IpVersion::V6 => set_flag(socket, uapi::SOL_IPV6, uapi::IPV6_RECVERR, on),
                }
            }
            TcpSetting::KeepAlive(on) => {
                sockopt::set_socket_keepalive(socket, on).map_err(host_error)
            }
            TcpSetting::KeepAliveIdle(idle) => tcp(uapi::TCP_KEEPIDLE, whole(idle, SECOND)?),
            TcpSetting::KeepAliveInterval(interval) => {
                tcp(uapi::TCP_KEEPINTVL, whole(interval, SECOND)?)
            }
            TcpSetting::KeepAliveProbes(probes) => tcp(uapi::TCP_KEEPCNT, c_int::from(probes.0)),
            TcpSetting::NoDelay(on) => sockopt::set_tcp_nodelay(socket, on).map_err(host_error),
            TcpSetting::Cork(on) => sockopt::set_tcp_cork(socket, on).map_err(host_error),
            TcpSetting::UrgentInline(on) => {
                sockopt::set_socket_oobinline(socket, on).map_err(host_error)
            }
            TcpSetting::QuickAcknowledgement(mode) => {
                sockopt::set_tcp_quickack(socket, mode == TcpQuickAcknowledgement::On)
                    .map_err(host_error)
            }
            TcpSetting::MaximumSegmentSize(size) => {
                tcp(uapi::TCP_MAXSEG, size.map_or(0, |size| c_int::from(size.0)))
            }
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
            TcpSetting::ThinLinearTimeouts(on) => {
                sockopt::set_tcp_thin_linear_timeouts(socket, on).map_err(host_error)
            }
            TcpSetting::CongestionControl(name) => {
                sockopt::set_tcp_congestion(socket, name).map_err(host_error)
            }
            TcpSetting::NotSentLowWater(low_water) => set_option(
                socket,
                uapi::SOL_TCP,
                uapi::TCP_NOTSENT_LOWAT,
                &match low_water {
                    Some(bytes) => u32::try_from(bytes.0).map_err(|_| invalid())?,
                    None => u32::MAX,
                },
            ),
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
                *large = vec![0; usize::from(u16::MAX)];
                let length = options::get_bytes(socket, uapi::SOL_TCP, uapi::TCP_SAVED_SYN, large)?;
                Ok(&large[..length])
            }
            Err(error) => Err(error),
        }
    }

    /// The interface an IPv6 SYN came in on, from the packet information the listener's `IPV6_RECVPKTINFO` kept.
    fn ipv6_arrival_interface(&self, socket: BorrowedFd<'_>) -> Option<InterfaceIndex> {
        let mut control = PacketOptions([0; PACKET_OPTIONS_ROOM]);
        let length = options::get_bytes(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_2292PKTOPTIONS,
            &mut control.0,
        )
        .ok()?;
        // The accepted connection keeps no packet information for what arrives later.
        set_flag(socket, uapi::SOL_IPV6, uapi::IPV6_RECVPKTINFO, false).ok()?;
        Ancillary::parse(&control.0[..length])
            .interface
            .map(InterfaceIndex)
    }

    fn synchronize_fields(
        &self,
        socket: BorrowedFd<'_>,
        header: &[u8],
        local: SocketAddr,
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
            IpAddr::V6(_) => self.ipv6_arrival_interface(socket),
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
        connection: &LinuxHostTcpConnection,
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
                connection.socket.as_fd(),
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

impl<'a> Configurable<LinuxHostTcpConnection, TcpSetting<'a>> for LinuxHostTcpSockets {
    fn configure(
        &self,
        connection: &LinuxHostTcpConnection,
        setting: TcpSetting<'a>,
    ) -> Result<(), HostError> {
        let socket = connection.socket.as_fd();
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

impl<'a> Configurable<LinuxHostTcpListener, TcpListenerSetting<'a>> for LinuxHostTcpSockets {
    fn configure(
        &self,
        listener: &LinuxHostTcpListener,
        setting: TcpListenerSetting<'a>,
    ) -> Result<(), HostError> {
        let socket = listener.socket.as_fd();
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
            TcpListenerSetting::Socket(setting) => {
                self.configure_socket(socket, listener.family, &setting, 0)
            }
            TcpListenerSetting::Backlog(backlog) => {
                rustix::net::listen(socket, i32::try_from(backlog.0).unwrap_or(i32::MAX))
                    .map_err(host_error)
            }
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
            // Every TCP listener saves its SYNs' network headers, which accept gives; the setting decides whether
            // the accepted connections keep them to be taken.
            TcpListenerSetting::SaveSynchronize(
                save @ (TcpSavedSynchronize::Off | TcpSavedSynchronize::FromNetworkHeader),
            ) => {
                let on = save == TcpSavedSynchronize::FromNetworkHeader;
                if listener.protocol == TcpProtocol::Multipath {
                    set_flag(socket, uapi::SOL_TCP, uapi::TCP_SAVE_SYN, on)?;
                }
                listener.saves_synchronize.store(on, Ordering::Relaxed);
                Ok(())
            }
            TcpListenerSetting::SaveSynchronize(TcpSavedSynchronize::FromLinkHeader) => {
                Err(self.shortfalls.refuse(NO_LINK_HEADERS))
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

impl LinuxHostTcpSockets {
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

impl NegativeRouteAdvice<LinuxHostTcpConnection> for LinuxHostTcpSockets {
    fn give_negative_route_advice(
        &self,
        connection: &LinuxHostTcpConnection,
    ) -> Result<(), HostError> {
        set_flag(
            connection.socket.as_fd(),
            uapi::SOL_SOCKET,
            uapi::SO_CNX_ADVICE,
            true,
        )
    }
}

impl TcpConnections for LinuxHostTcpSockets {
    type Connection = LinuxHostTcpConnection;

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
        let connection = LinuxHostTcpConnection::new(
            open_socket(family, SocketType::STREAM, protocol_of(protocol), owner)?,
            family,
            0,
        );
        for setting in settings {
            self.configure(&connection, setting.clone())?;
        }
        let socket = connection.socket.as_fd();
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
                    rustix::net::connect(socket, &peer)
                        .map(|()| 0)
                        .map_err(host_error),
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
                let flags = SendFlags::from_bits_retain(libc::MSG_FASTOPEN as u32)
                    | SendFlags::NOSIGNAL
                    | SendFlags::DONTWAIT;
                let taken = in_progress(
                    rustix::net::sendto(socket, data, flags, &peer).map_err(host_error),
                )?;
                TcpConnectStart::SynchronizeSent {
                    fast_open_data_taken: ByteCount(taken.unwrap_or(0)),
                }
            }
        };
        let local = socket_address(rustix::net::getsockname(socket).map_err(host_error)?)?;
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
        let socket = connection.socket.as_fd();
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
            return rustix::net::send(socket, bytes, SendFlags::from_bits_retain(flags as u32))
                .map(ByteCount)
                .map_err(host_error);
        }
        let mut reports = connection.reports();
        if asked.wireless_status == Report::Requested && !reports.wireless_status {
            set_flag(socket, uapi::SOL_SOCKET, uapi::SO_WIFI_STATUS, true)?;
            reports.wireless_status = true;
        }
        let taken = if record == 0 {
            rustix::net::send(socket, bytes, SendFlags::from_bits_retain(flags as u32))
                .map_err(host_error)?
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
        let socket = connection.socket.as_fd();
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
            rustix::net::recv(
                socket,
                &mut *into,
                RecvFlags::from_bits_retain(flags as u32),
            )
            .map(|(length, _)| TcpReceived {
                length: ByteCount(length),
                timestamps: ReceiveTimestamps {
                    software: None,
                    hardware: None,
                },
                unread_after: None,
            })
            .map_err(host_error)
        } else {
            let mut control = ControlBuffer::default();
            options::receive_message(socket, into, flags, &mut control).map(|message| {
                let mut received = TcpReceived {
                    length: ByteCount(message.length),
                    timestamps: ReceiveTimestamps {
                        software: None,
                        hardware: None,
                    },
                    unread_after: None,
                };
                for &(level, kind, data) in &message.control {
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
        let mut control = ControlBuffer::default();
        let message = options::receive_message(
            connection.socket.as_fd(),
            into,
            libc::MSG_ERRQUEUE | libc::MSG_DONTWAIT,
            &mut control,
        )?;
        if message.flags & libc::MSG_CTRUNC != 0 {
            self.shortfalls.count(CUT_CONTROL);
        }
        let ancillary = Ancillary::parse(message.control_bytes);
        let Some((error, offender)) = ancillary.error else {
            return Err(self.shortfalls.refuse(NO_EXTENDED_ERROR));
        };
        let wireless = message.control.iter().find_map(|&(level, kind, data)| {
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
                Ok(TcpErrorQueueEntry::IcmpError(
                    ancillary.packet_fields(length, u32::MIN),
                ))
            }
            libc::SO_EE_ORIGIN_TXSTATUS if wireless.is_some() => {
                if cut {
                    self.shortfalls.count(CUT_REPORT_PACKET);
                }
                let acknowledged = if wireless == Some(0) {
                    WirelessAcknowledgement::NotAcknowledged
                } else {
                    WirelessAcknowledgement::Acknowledged
                };
                // Linux names no send in a wireless status report: its key is the zero it gives.
                Ok(TcpErrorQueueEntry::TransmitReport {
                    report: TransmitReport {
                        key: TransmitReportKey(error.ee_data),
                        interface: NO_INTERFACE,
                        event: TransmitEvent::WirelessStatus(acknowledged),
                        packet_length: Some(ByteCount(message.length)),
                    },
                    statistics: None,
                })
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
                    if cut {
                        self.shortfalls.count(CUT_REPORT_PACKET);
                    }
                    report.packet_length = Some(ByteCount(message.length));
                }
                let statistics = if send.statistics {
                    let found = message.control.iter().find_map(|&(level, kind, data)| {
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
                Ok(TcpErrorQueueEntry::TransmitReport { report, statistics })
            }
            _ => Err(self.shortfalls.refuse(NO_OTHER_ERROR_ENTRIES)),
        }
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
            TcpShutdown::Read => Shutdown::Read,
            TcpShutdown::Write => Shutdown::Write,
            TcpShutdown::ReadAndWrite => Shutdown::Both,
        };
        rustix::net::shutdown(connection.socket.as_fd(), how).map_err(host_error)
    }

    fn close(&self, connection: Self::Connection, manner: TcpClose) {
        if manner == TcpClose::Abort {
            // A linger of zero makes the close send a reset; a failure leaves the orderly close.
            if sockopt::set_socket_linger(connection.socket.as_fd(), Some(Duration::ZERO)).is_err()
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
        let socket = connection.socket.as_fd();
        match ignoring {
            // Turning no-delay on pushes past the cork and Nagle's algorithm; it is then put back.
            TcpPushOverride::CorkAndNagle => {
                let no_delay = sockopt::tcp_nodelay(socket).map_err(host_error)?;
                sockopt::set_tcp_nodelay(socket, true).map_err(host_error)?;
                if !no_delay {
                    sockopt::set_tcp_nodelay(socket, false).map_err(host_error)?;
                }
                Ok(())
            }
            // Writing the cork as it is pushes when uncorked; under a cork everything a push may send is sent already.
            TcpPushOverride::Neither => {
                let corked = sockopt::tcp_cork(socket).map_err(host_error)?;
                sockopt::set_tcp_cork(socket, corked).map_err(host_error)
            }
        }
    }

    fn send_delayed_acknowledgement(
        &self,
        connection: &Self::Connection,
        afterwards: TcpQuickAcknowledgement,
    ) -> Result<(), HostError> {
        let socket = connection.socket.as_fd();
        let before = sockopt::tcp_quickack(socket).map_err(host_error)?;
        // Quick mode off afterwards is the sign that one was scheduled.
        set_int(
            socket,
            uapi::SOL_TCP,
            uapi::TCP_QUICKACK,
            QUICK_ACKNOWLEDGEMENT_THEN_DELAYING,
        )?;
        let after = sockopt::tcp_quickack(socket).map_err(host_error)?;
        let scheduled = !after;
        let wanted = if scheduled {
            afterwards == TcpQuickAcknowledgement::On
        } else {
            before
        };
        if wanted != after {
            sockopt::set_tcp_quickack(socket, wanted).map_err(host_error)?;
        }
        Ok(())
    }

    fn rearm_keep_alive_timer(&self, connection: &Self::Connection) -> Result<(), HostError> {
        // Writing the idle time re-times the probe from the last data or acknowledgement received.
        let socket = connection.socket.as_fd();
        let idle = get_int(socket, uapi::SOL_TCP, uapi::TCP_KEEPIDLE)?;
        set_int(socket, uapi::SOL_TCP, uapi::TCP_KEEPIDLE, idle)
    }

    fn discard_error_queue_entries(&self, connection: &Self::Connection) -> Result<(), HostError> {
        let mut scratch = [0; 0];
        loop {
            match self.take_error_queue_entry(connection, &mut scratch) {
                Ok(_) | Err(HostError::NotSupported) => {}
                Err(HostError::WouldBlock) => return Ok(()),
                Err(error) => return Err(error),
            }
        }
    }

    fn send_window_probe(&self, connection: &Self::Connection) -> Result<(), HostError> {
        // Leaving repair mode with TCP_REPAIR_OFF sends a window probe; it also clears address sharing and the
        // repair mode itself, which are put back as they were.
        let socket = connection.socket.as_fd();
        let sharing = sockopt::socket_reuseaddr(socket).map_err(host_error)?;
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
            sockopt::set_socket_reuseaddr(socket, true).map_err(host_error)?;
        }
        Ok(())
    }

    fn set_timestamp_clock(
        &self,
        connection: &Self::Connection,
        reading: TcpTimestampClock,
    ) -> Result<(), HostError> {
        set_timestamp_clock(connection.socket.as_fd(), reading)
    }

    fn set_authentication_counters(
        &self,
        _connection: &Self::Connection,
        _values: TcpAuthenticationCounters,
    ) -> Result<(), HostError> {
        Err(self.shortfalls.refuse(NO_AUTHENTICATION))
    }
}

fn set_timestamp_clock(
    socket: BorrowedFd<'_>,
    reading: TcpTimestampClock,
) -> Result<(), HostError> {
    let unit = u32::from(reading.unit == TcpTimestampUnit::Microseconds);
    let value = reading.value.0 & !1 | unit;
    set_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP, &value)
}

impl TcpListeners for LinuxHostTcpSockets {
    type Listener = LinuxHostTcpListener;

    fn listen(
        &self,
        protocol: TcpProtocol,
        owner: UserId,
        local: LocalBinding<'_>,
        backlog: ListenBacklog,
        settings: &[TcpListenerSetting<'_>],
    ) -> Result<TcpListening<Self::Listener>, HostError> {
        let family = family_of(&local.address);
        let listener = LinuxHostTcpListener {
            socket: open_socket(family, SocketType::STREAM, protocol_of(protocol), owner)?,
            family,
            protocol,
            receive_timestamping: AtomicU32::new(0),
            bound_clock: Mutex::new(None),
            saves_synchronize: AtomicBool::new(false),
        };
        let socket = listener.socket.as_fd();
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
        rustix::net::listen(socket, i32::try_from(backlog.0).unwrap_or(i32::MAX))
            .map_err(host_error)?;
        // A backlog among the settings is listening again with it.
        for setting in backlogs {
            self.configure(&listener, setting.clone())?;
        }
        let local = socket_address(rustix::net::getsockname(socket).map_err(host_error)?)?;
        Ok(TcpListening { listener, local })
    }

    fn accept<'a>(
        &self,
        listener: &Self::Listener,
        synchronize_header: &'a mut [u8],
    ) -> Result<TcpAccepted<'a, Self::Connection>, HostError> {
        let socket = rustix::net::accept_with(
            listener.socket.as_fd(),
            SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
        )
        .map_err(host_error)?;
        let connection = LinuxHostTcpConnection::new(
            socket,
            listener.family,
            listener.receive_timestamping.load(Ordering::Relaxed),
        );
        let bound_clock = *locked(&listener.bound_clock);
        if bound_clock.is_some() {
            connection.reports().bound_clock = bound_clock;
        }
        let socket = connection.socket.as_fd();
        let peer = socket_address(
            rustix::net::getpeername(socket)
                .map_err(host_error)?
                .ok_or(HostError::Errno(libc::ENOTCONN))?,
        )?;
        let local = socket_address(rustix::net::getsockname(socket).map_err(host_error)?)?;
        let _ = connection.addresses.set((local, peer));
        let mut room = [0; SAVED_SYNCHRONIZE_ROOM];
        let mut large = Vec::new();
        let saved = match listener.protocol {
            TcpProtocol::Tcp => self.take_saved_synchronize(socket, &mut room, &mut large)?,
            TcpProtocol::Multipath => &[],
        };
        if listener.saves_synchronize.load(Ordering::Relaxed) && !saved.is_empty() {
            *locked(&connection.saved_synchronize) = Some(saved.to_vec());
        }
        let synchronize = self.synchronize_fields(socket, saved, local)?;
        let length = saved.len().min(synchronize_header.len());
        synchronize_header[..length].copy_from_slice(&saved[..length]);
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
        set_timestamp_clock(listener.socket.as_fd(), reading)
    }

    fn stop_listening(&self, listener: Self::Listener) {
        drop(listener);
    }
}

impl TcpSockets for LinuxHostTcpSockets {}

#[cfg(test)]
mod tests {
    wcontainers_tcp_sockets_conformance::tests!(super::LinuxHostTcpSockets::new());
}
