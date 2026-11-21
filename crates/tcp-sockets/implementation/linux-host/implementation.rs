//! TCP and MPTCP connections and listeners made of the Linux sockets of the machine it runs on.
#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::ffi::c_int;
use std::mem::size_of;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV6};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use linux_raw_sys::net as uapi;
use rustix::net::{
    AddressFamily, RecvFlags, SendFlags, Shutdown, SocketFlags, SocketType, ipproto, sockopt,
};
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BindAddress, ByteCount, DropCount, InterfaceIndex, IpSocketSetting, IpVersion, Ipv6Families,
    LocalBinding, LocalPort, PacketMark, PacketPriority, PortChoice, PortRange,
    PtpHardwareClockIndex, ReceiveTimestamps, ReceivedPacketFields, Report, TransmitReport,
    TransmitReportKey, TransmitReportRequest,
};
use wcontainers_internet_protocol_linux_socket_options::{
    self as options, ControlBuffer, Shortfalls, get_int, host_error, set_flag, set_int, set_option,
};
use wcontainers_kernel::UserId;
use wcontainers_negative_route_advice::NegativeRouteAdvice;
use wcontainers_tcp_sockets::authentication::TcpAuthenticationCounters;
use wcontainers_tcp_sockets::repair::{
    TcpRepair, TcpRepairQueue, TcpTimestampClock, TcpTimestampUnit,
};
use wcontainers_tcp_sockets::settings::{
    BufferTuning, TcpFinWait2, TcpListenerSetting, TcpQuickAcknowledgement, TcpSavedSynchronize,
    TcpSetting, TransmitRehash,
};
use wcontainers_tcp_sockets::state::{TcpErrorQueueEntry, TcpListenerQuery, TcpQuery, TcpReading};
use wcontainers_tcp_sockets::{
    ListenBacklog, TcpAccepted, TcpClose, TcpConnectStart, TcpConnected, TcpConnections,
    TcpContinuation, TcpFastOpen, TcpListeners, TcpListening, TcpProtocol, TcpPushOverride,
    TcpReceiveRequest, TcpReceived, TcpRecordBoundary, TcpSendRequest, TcpShutdown, TcpSockets,
    TcpUrgency,
};

/// The `SO_TIMESTAMPING` flags kept on a connection once it has asked for a transmit report: keys counted in bytes, reports without the packet, software and hardware times.
const REPORT_FLAGS: u32 = uapi::SOF_TIMESTAMPING_OPT_ID as u32
    | uapi::SOF_TIMESTAMPING_OPT_ID_TCP as u32
    | uapi::SOF_TIMESTAMPING_OPT_TSONLY as u32
    | uapi::SOF_TIMESTAMPING_SOFTWARE as u32
    | uapi::SOF_TIMESTAMPING_RAW_HARDWARE as u32;

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
const ARRIVAL_INTERFACE_BY_ADDRESS: &str =
    "the SYN's arrival interface is taken as the host interface holding the local address";
const NO_LINK_HEADERS: &str = "saving the link headers of SYNs is not mapped to the host yet";
const NO_REPORT_EXTRAS: &str = "transmit reports with the packet, wireless status or statistics are not mapped to the host yet";
const NO_OTHER_ERROR_ENTRIES: &str =
    "error queue entries other than transmit timestamps are not mapped to the host yet";
const UNASKED_REPORT: &str = "a transmit report for no send that asked for it";
const NO_CONNECTION_STATE: &str = "reading a TCP connection's state is not mapped to the host yet";
const ABORT_CLOSED_IN_ORDER: &str = "an abort closed in order: the host refused a linger of zero";
const NO_LISTENER_STATE: &str = "reading a TCP listener's state is not mapped to the host yet";

/// Linux's TCP sockets as the exit node's TCP connections and listeners.
#[derive(Debug, Default)]
pub struct LinuxHostTcpSockets {
    shortfalls: Shortfalls,
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
    armed: bool,
    bound_clock: Option<PtpHardwareClockIndex>,
    /// Bytes taken by sends since the reports were armed, wrapping as the kernel's key does.
    sent: u32,
    /// The guest key of each send that asked for reports, by the offset of its last byte, with the reports still to come.
    keys: BTreeMap<u32, (TransmitReportKey, u32)>,
}

#[derive(Debug)]
pub struct LinuxHostTcpConnection {
    socket: OwnedFd,
    family: IpVersion,
    receive_timestamping: AtomicU32,
    unread_reports: AtomicBool,
    reports: Mutex<TransmitReports>,
}

#[derive(Debug)]
pub struct LinuxHostTcpListener {
    socket: OwnedFd,
    family: IpVersion,
    protocol: TcpProtocol,
    receive_timestamping: AtomicU32,
    bound_clock: Mutex<Option<PtpHardwareClockIndex>>,
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
            receive_timestamping: AtomicU32::new(receive_timestamping),
            unread_reports: AtomicBool::new(false),
            reports: Mutex::default(),
        }
    }

    fn reports(&self) -> std::sync::MutexGuard<'_, TransmitReports> {
        self.reports
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
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

const SECOND: Duration = Duration::from_secs(1);
const MILLISECOND: Duration = Duration::from_millis(1);
const MICROSECOND: Duration = Duration::from_micros(1);

fn count(value: usize) -> Result<c_int, HostError> {
    c_int::try_from(value).map_err(|_| invalid())
}

fn family_of(address: &BindAddress) -> IpVersion {
    match address {
        BindAddress::V4(_) => IpVersion::V4,
        BindAddress::V6 { .. } => IpVersion::V6,
    }
}

fn socket_address(address: rustix::net::SocketAddrAny) -> Result<SocketAddr, HostError> {
    SocketAddr::try_from(address).map_err(host_error)
}

/// Makes a non-blocking stream socket owned by the given user, acting as that user for the call when the host's thread is another.
fn open_socket(
    family: IpVersion,
    protocol: TcpProtocol,
    owner: UserId,
) -> Result<OwnedFd, HostError> {
    let domain = match family {
        IpVersion::V4 => AddressFamily::INET,
        IpVersion::V6 => AddressFamily::INET6,
    };
    let protocol = match protocol {
        TcpProtocol::Tcp => ipproto::TCP,
        TcpProtocol::Multipath => ipproto::MPTCP,
    };
    let open = || {
        rustix::net::socket_with(
            domain,
            SocketType::STREAM,
            SocketFlags::NONBLOCK | SocketFlags::CLOEXEC,
            Some(protocol),
        )
        .map_err(host_error)
    };
    // An invalid id changes nothing and answers the thread's filesystem user.
    // SAFETY: setfsuid only changes this thread's filesystem user, which is restored before returning.
    let previous = unsafe { libc::setfsuid(libc::uid_t::MAX) } as libc::uid_t;
    if previous == owner.0 {
        return open();
    }
    // SAFETY: as above.
    let acting = unsafe {
        libc::setfsuid(owner.0);
        libc::setfsuid(libc::uid_t::MAX) as libc::uid_t
    };
    let socket = if acting == owner.0 {
        open()
    } else {
        Err(HostError::Errno(libc::EPERM))
    };
    // SAFETY: as above.
    unsafe { libc::setfsuid(previous) };
    socket
}

/// The ports of a choice outside its reserved ranges, as ranges.
fn allowed_ranges(choice: &PortChoice<'_>) -> Vec<PortRange> {
    let mut reserved: Vec<PortRange> = choice.reserved.to_vec();
    reserved.sort_by_key(|range| range.first.0);
    let mut allowed = Vec::new();
    let mut next = u32::from(choice.range.first.0);
    let last = u32::from(choice.range.last.0);
    for range in reserved {
        let (first, end) = (u32::from(range.first.0), u32::from(range.last.0));
        if first > next && next <= last {
            allowed.push((next, (first - 1).min(last)));
        }
        next = next.max(end + 1);
    }
    if next <= last {
        allowed.push((next, last));
    }
    allowed
        .into_iter()
        .map(|(first, last)| PortRange {
            first: wcontainers_internet_protocol::Port(first as u16),
            last: wcontainers_internet_protocol::Port(last as u16),
        })
        .collect()
}

fn set_port_range(socket: BorrowedFd<'_>, range: PortRange) -> Result<(), HostError> {
    let value = u32::from(range.last.0) << u16::BITS | u32::from(range.first.0);
    set_option(socket, uapi::SOL_IP, uapi::IP_LOCAL_PORT_RANGE, &value)
}

/// Runs an operation that takes a local port once per range the exit node may choose from, until one has a free port.
fn in_port_ranges<T>(
    socket: BorrowedFd<'_>,
    choice: Option<&PortChoice<'_>>,
    mut operation: impl FnMut() -> Result<T, HostError>,
) -> Result<T, HostError> {
    let Some(choice) = choice else {
        return operation();
    };
    let mut outcome = Err(HostError::Errno(libc::EADDRNOTAVAIL));
    for range in allowed_ranges(choice) {
        set_port_range(socket, range)?;
        outcome = operation();
        match outcome {
            Err(HostError::Errno(libc::EADDRNOTAVAIL | libc::EADDRINUSE)) => continue,
            _ => break,
        }
    }
    outcome
}

/// Binds the socket as the binding asks, returning the choice the exit node makes its port from, if it does.
fn bind<'a>(
    socket: BorrowedFd<'_>,
    local: &LocalBinding<'a>,
    listening: bool,
) -> Result<Option<PortChoice<'a>>, HostError> {
    let address = match local.address {
        BindAddress::V4(address) => IpAddr::V4(address),
        BindAddress::V6 { address, families } => {
            sockopt::set_ipv6_v6only(socket, families == Ipv6Families::V6Only)
                .map_err(host_error)?;
            IpAddr::V6(address)
        }
    };
    match local.port {
        LocalPort::Requested(port) | LocalPort::ChosenByKernel(port) => {
            rustix::net::bind(socket, &SocketAddr::new(address, port.0)).map_err(host_error)?;
            Ok(None)
        }
        LocalPort::ChosenByExitNode(choice) => {
            let any = SocketAddr::new(address, 0);
            if listening {
                in_port_ranges(socket, Some(&choice), || {
                    rustix::net::bind(socket, &any).map_err(host_error)
                })?;
            } else if !address.is_unspecified() {
                set_flag(socket, uapi::SOL_IP, uapi::IP_BIND_ADDRESS_NO_PORT, true)?;
                rustix::net::bind(socket, &any).map_err(host_error)?;
            }
            Ok(Some(choice))
        }
    }
}

/// The peer as the socket's family takes it, an IPv4 peer of an IPv6 socket mapped into IPv6.
fn peer_for(family: IpVersion, peer: SocketAddr) -> SocketAddr {
    match (family, peer) {
        (IpVersion::V6, SocketAddr::V4(peer)) => SocketAddr::V6(SocketAddrV6::new(
            peer.ip().to_ipv6_mapped(),
            peer.port(),
            0,
            0,
        )),
        _ => peer,
    }
}

/// The index of the host's interface that holds the address.
fn interface_holding(address: IpAddr) -> Option<InterfaceIndex> {
    let address = address.to_canonical();
    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs fills the list, walked read-only below and freed once.
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return None;
    }
    let mut found = None;
    let mut entry = list;
    while !entry.is_null() && found.is_none() {
        // SAFETY: every entry of the list and its address stay valid until freeifaddrs.
        unsafe {
            let held = (*entry).ifa_addr;
            let matches = !held.is_null()
                && match c_int::from((*held).sa_family) {
                    libc::AF_INET => {
                        let held = held.cast::<libc::sockaddr_in>().read_unaligned();
                        IpAddr::from(Ipv4Addr::from(u32::from_be(held.sin_addr.s_addr))) == address
                    }
                    libc::AF_INET6 => {
                        let held = held.cast::<libc::sockaddr_in6>().read_unaligned();
                        IpAddr::from(held.sin6_addr.s6_addr) == address
                    }
                    _ => false,
                };
            if matches {
                let index = libc::if_nametoindex((*entry).ifa_name);
                found = (index != 0).then_some(InterfaceIndex(index));
            }
            entry = (*entry).ifa_next;
        }
    }
    // SAFETY: the list came from getifaddrs and is not used after this.
    unsafe { libc::freeifaddrs(list) };
    found
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

    fn take_saved_synchronize(&self, socket: BorrowedFd<'_>) -> Result<Vec<u8>, HostError> {
        let mut saved = vec![0; SAVED_SYNCHRONIZE_ROOM];
        let length =
            match options::get_bytes(socket, uapi::SOL_TCP, uapi::TCP_SAVED_SYN, &mut saved) {
                // Too small a buffer: the headers stay saved and can be read with room for any.
                Err(HostError::Errno(libc::EINVAL)) => {
                    saved = vec![0; usize::from(u16::MAX)];
                    options::get_bytes(socket, uapi::SOL_TCP, uapi::TCP_SAVED_SYN, &mut saved)?
                }
                other => other?,
            };
        saved.truncate(length);
        Ok(saved)
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
        self.shortfalls.count(ARRIVAL_INTERFACE_BY_ADDRESS);
        let interface = interface_holding(local.ip()).unwrap_or(NO_INTERFACE);
        let specific_destination = match local.ip().to_canonical() {
            IpAddr::V4(address) => Some(address),
            IpAddr::V6(_) => None,
        };
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

    /// Arms a connection's transmit reports at its first send that asks for one.
    fn arm_reports(
        &self,
        connection: &LinuxHostTcpConnection,
        reports: &mut TransmitReports,
    ) -> Result<(), HostError> {
        if !reports.armed {
            options::set_timestamping(
                connection.socket.as_fd(),
                connection.receive_timestamping.load(Ordering::Relaxed) | REPORT_FLAGS,
                reports.bound_clock,
            )?;
            reports.armed = true;
            reports.sent = 0;
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
                let kept = if reports.armed { REPORT_FLAGS } else { 0 };
                options::set_timestamping(socket, receive | kept, timestamping.bound_clock)?;
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
            // Every listener saves the network headers of its SYNs, which accept gives.
            TcpListenerSetting::SaveSynchronize(
                TcpSavedSynchronize::Off | TcpSavedSynchronize::FromNetworkHeader,
            ) => Ok(()),
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
        let connection =
            LinuxHostTcpConnection::new(open_socket(family, protocol, owner)?, family, 0);
        for setting in settings {
            self.configure(&connection, setting.clone())?;
        }
        let socket = connection.socket.as_fd();
        let choice = bind(socket, &local, false)?;
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
                let outcome = in_progress(in_port_ranges(socket, choice.as_ref(), || {
                    rustix::net::connect(socket, &peer)
                        .map(|()| 0)
                        .map_err(host_error)
                }))?;
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
                let taken = in_progress(in_port_ranges(socket, choice.as_ref(), || {
                    rustix::net::sendto(socket, data, flags, &peer).map_err(host_error)
                }))?;
                TcpConnectStart::SynchronizeSent {
                    fast_open_data_taken: ByteCount(taken.unwrap_or(0)),
                }
            }
        };
        let local = socket_address(rustix::net::getsockname(socket).map_err(host_error)?)?;
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
        if asked.packet == Report::Requested
            || asked.wireless_status == Report::Requested
            || request.transmit_statistics == Report::Requested
        {
            return Err(self.shortfalls.refuse(NO_REPORT_EXTRAS));
        }
        let (record, expected) = record_flags(&asked);
        let mut reports = connection.reports();
        let taken = if record == 0 {
            rustix::net::send(socket, bytes, SendFlags::from_bits_retain(flags as u32))
                .map_err(host_error)?
        } else {
            self.arm_reports(connection, &mut reports)?;
            let taken = send_with_record_flags(socket, bytes, flags, record)?;
            if taken > 0 {
                let last = reports.sent.wrapping_add(taken as u32).wrapping_sub(1);
                reports.keys.insert(last, (asked.key, expected));
            }
            taken
        };
        if reports.armed {
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
        let entry = options::take_error_queue_message(connection.socket.as_fd(), into)?;
        let Some(event) = options::transmit_event(&entry) else {
            return Err(self.shortfalls.refuse(NO_OTHER_ERROR_ENTRIES));
        };
        let mut reports = connection.reports();
        let offset = entry.error.ee_data;
        let key = match reports.keys.get_mut(&offset) {
            Some((key, remaining)) => {
                let key = *key;
                *remaining = remaining.saturating_sub(1);
                if *remaining == 0 {
                    reports.keys.remove(&offset);
                }
                key
            }
            None => {
                self.shortfalls.count(UNASKED_REPORT);
                TransmitReportKey(offset)
            }
        };
        Ok(TcpErrorQueueEntry::TransmitReport {
            report: TransmitReport {
                key,
                interface: NO_INTERFACE,
                event,
                packet_length: None,
            },
            statistics: None,
        })
    }

    fn read_state<'a>(
        &self,
        _connection: &Self::Connection,
        _query: TcpQuery<'a>,
    ) -> Result<TcpReading<'a>, HostError> {
        Err(self.shortfalls.refuse(NO_CONNECTION_STATE))
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
        // Two sends a scheduled acknowledgement and then leaves quick mode, which it enters first: quick mode off
        // afterwards is the sign that one was scheduled.
        set_int(socket, uapi::SOL_TCP, uapi::TCP_QUICKACK, 2)?;
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
        // Leaving repair mode with TCP_REPAIR_OFF sends a window probe.
        let socket = connection.socket.as_fd();
        set_int(
            socket,
            uapi::SOL_TCP,
            uapi::TCP_REPAIR,
            uapi::TCP_REPAIR_ON as c_int,
        )?;
        set_int(
            socket,
            uapi::SOL_TCP,
            uapi::TCP_REPAIR,
            uapi::TCP_REPAIR_OFF as c_int,
        )
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
            socket: open_socket(family, protocol, owner)?,
            family,
            protocol,
            receive_timestamping: AtomicU32::new(0),
            bound_clock: Mutex::new(None),
        };
        let socket = listener.socket.as_fd();
        // Multipath sockets have no TCP_SAVE_SYN.
        if protocol == TcpProtocol::Tcp {
            set_flag(socket, uapi::SOL_TCP, uapi::TCP_SAVE_SYN, true)?;
        }
        for setting in settings {
            if !matches!(setting, TcpListenerSetting::Backlog(_)) {
                self.configure(&listener, setting.clone())?;
            }
        }
        bind(socket, &local, true)?;
        rustix::net::listen(socket, i32::try_from(backlog.0).unwrap_or(i32::MAX))
            .map_err(host_error)?;
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
        connection.reports().bound_clock = *listener
            .bound_clock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let socket = connection.socket.as_fd();
        let peer = socket_address(
            rustix::net::getpeername(socket)
                .map_err(host_error)?
                .ok_or(HostError::Errno(libc::ENOTCONN))?,
        )?;
        let local = socket_address(rustix::net::getsockname(socket).map_err(host_error)?)?;
        let saved = match listener.protocol {
            TcpProtocol::Tcp => self.take_saved_synchronize(socket)?,
            TcpProtocol::Multipath => Vec::new(),
        };
        let synchronize = self.synchronize_fields(socket, &saved, local)?;
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
