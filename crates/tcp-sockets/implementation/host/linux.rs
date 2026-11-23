//! The Linux host's TCP and MPTCP sockets, every call, request and setting the traits name.

use std::ffi::c_int;
use std::io::IoSlice;
use std::mem::size_of;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::num::NonZeroU32;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use etherparse::{IpNumber, Ipv6FlowLabel};
use linux_raw_sys::net as uapi;
use netlink_packet_core::{NlasIterator, parse_u8, parse_u16, parse_u32, parse_u64};
use rustix::io::Errno;
use rustix::net::{RecvFlags, SocketFlags, SocketType, ipproto};
use socket2::{MsgHdr, SockAddr, SockRef};
use wcontainers_abi::HostError;
use wcontainers_configurable::Configurable;
use wcontainers_internet_protocol::{
    BindingSetting, ByteCount, BytesPerSecond, DropCount, FlowLabelChoice, FrameLength, HopLimit,
    InterfaceIndex, IpPacketSetting, IpSocketSetting, IpVersion, LocalBinding, LocalPort,
    PacketMark, PacketPriority, Port, ReceiveTimestamps, ReceivedPacketFields, Report,
    TransmitEvent, TransmitReport, TransmitReportKey, TransmitReportRequest,
    WirelessAcknowledgement,
};
use wcontainers_internet_protocol_linux_socket_options::datagram::addresses::{
    StorageBytes, in6_addr, kernel_storage, storage, storage_bytes,
};
use wcontainers_internet_protocol_linux_socket_options::datagram::exchange::NO_ARRIVAL_INTERFACE;
use wcontainers_internet_protocol_linux_socket_options::datagram::memberships::{
    AnycastMemberships, set_multicast_group,
};
use wcontainers_internet_protocol_linux_socket_options::datagram::receive::Ancillary;
use wcontainers_internet_protocol_linux_socket_options::datagram::send::Control;
use wcontainers_internet_protocol_linux_socket_options::errno::errno_of;
use wcontainers_internet_protocol_linux_socket_options::icmp_error::write_icmp_error;
use wcontainers_internet_protocol_linux_socket_options::readiness::Watched;
use wcontainers_internet_protocol_linux_socket_options::{
    self as options, ControlBuffer, bind, family_of, get_int, host_error, open_socket, peer_for,
    set_flag, set_int, set_option,
};
use wcontainers_kernel::UserId;
use wcontainers_kernel::errno::Errno as KernelErrno;
use wcontainers_negative_route_advisable::NegativeRouteAdvisable;
use wcontainers_readiness::{Readiness, ReadinessReport, ReadyFor};
use wcontainers_tcp_sockets::authentication::{
    TcpAuthenticationCounters, TcpAuthenticationIcmpErrors, TcpAuthenticationKeyAddition,
    TcpAuthenticationKeyIdentity, TcpAuthenticationKeyRemoval, TcpAuthenticationKeyUse,
    TcpAuthenticationOptionCoverage, TcpAuthenticationOptions, TcpAuthenticationRequirement,
    TcpKeyVrf, TcpMd5KeyChange,
};
use wcontainers_tcp_sockets::repair::{
    TcpRepair, TcpRepairQueue, TcpTimestampClock, TcpTimestampUnit,
};
use wcontainers_tcp_sockets::settings::{
    BufferTuning, RetryCount, SegmentCount, TcpFinWait2, TcpListenerSetting,
    TcpQuickAcknowledgement, TcpSavedSynchronize, TcpSetting, TransmitRehash,
};
use wcontainers_tcp_sockets::state::{
    ByteTotal, ConnectionCount, DeliveryRateLimit, EventCount, TcpCongestionAvoidanceState,
    TcpErrorQueueEntry, TcpListenerQuery, TcpListenerReading, TcpQuery, TcpReading,
    TcpSavedSynchronizeHeader, TcpTransmitStatistics,
};
use wcontainers_tcp_sockets::{
    ListenBacklog, TcpAccepted, TcpBound, TcpClose, TcpConnectStart, TcpConnected, TcpConnections,
    TcpContinuation, TcpDisconnected, TcpFastOpen, TcpListeners, TcpListening, TcpLocalEnd,
    TcpLocalEndError, TcpProtocol, TcpPushOverride, TcpReceiveRequest, TcpReceived,
    TcpRecordBoundary, TcpSendRequest, TcpShutdown, TcpSockets, TcpUrgency,
};
use zerocopy::{FromZeros, IntoBytes};

mod state;

use super::{
    ABORT_CLOSED_IN_ORDER, HeldAddress, HostTcpBoundSocket, HostTcpConnection, HostTcpListener,
    HostTcpSockets, InterfaceAddresses, ReportedSend, TransmitReports,
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

/// The index a key flagged for a VRF names the default VRF with.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/tcp.h#L399
const DEFAULT_VRF_INDEX: c_int = 0;

/// The room `TCP_SAVED_SYN` is first read with: the largest IPv4 and TCP headers after an Ethernet header.
const SAVED_SYNCHRONIZE_ROOM: usize = etherparse::Ethernet2Header::LEN
    + etherparse::Ipv4Header::MAX_LEN
    + etherparse::TcpHeader::MAX_LEN;

/// `TCP_SAVE_SYN`'s value for saving no headers, which Linux does not name.
///
/// https://github.com/torvalds/linux/blob/master/net/ipv4/tcp.c
const SAVE_NONE: c_int = 0;
/// `TCP_SAVE_SYN`'s value for saving from the network header.
///
/// https://github.com/torvalds/linux/blob/master/net/ipv4/tcp.c
const SAVE_FROM_NETWORK_HEADER: c_int = 1;
/// `TCP_SAVE_SYN`'s value for saving from the link header.
///
/// https://github.com/torvalds/linux/blob/master/net/ipv4/tcp.c
const SAVE_FROM_LINK_HEADER: c_int = 2;

/// The options whose records `IP_PKTOPTIONS` gives a TCP socket: packet information, TTL and TOS.
const IPV4_RECORDS: &[u32] = &[uapi::IP_PKTINFO, uapi::IP_RECVTTL, uapi::IP_RECVTOS];

/// The options whose records `IPV6_2292PKTOPTIONS` gives a TCP socket from the last packet that came in order.
const IPV6_RECORDS: &[u32] = &[
    uapi::IPV6_RECVPKTINFO,
    uapi::IPV6_RECVHOPLIMIT,
    uapi::IPV6_RECVTCLASS,
    uapi::IPV6_RECVHOPOPTS,
    uapi::IPV6_RECVDSTOPTS,
    uapi::IPV6_RECVRTHDR,
];

/// The reasons the host could not honour a request, each counted by [`Shortfalls`].
const NO_SAVED_SYNCHRONIZE: &str = "the host kept no SYN headers for an accepted connection";
const ARRIVAL_INTERFACE_BY_ADDRESS: &str = "the SYN's arrival interface is taken as the host interface whose address or prefix holds the local address";
const ARRIVAL_INTERFACE_OF_THE_HANDSHAKE: &str = "Linux keeps for an IPv4 connection the interface of the segment that completed its handshake, which is given as its SYN's";
const SYNCHRONIZE_FIELDS_OF_THE_SOCKET: &str = "Linux reports no priority, timestamps or drops for a SYN, and its mark only as the accepted socket's: the accepted socket's mark and priority are given, with no timestamps or drops";
const NO_SYNCHRONIZE_INTERFACE: &str =
    "Linux named no arrival interface for a SYN and no host interface holds its local address";
const NO_SYNCHRONIZE_FRAGMENT_SIZE: &str = "Linux keeps no largest fragment size for a SYN, which is given as a packet that was not reassembled";
const NO_SYNCHRONIZE_FRAME: &str =
    "no link header was found before a SYN's network header, so its frame length is not given";
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
const DISCONNECT_REFUSED: &str = "the host refused a connect to AF_UNSPEC, which Linux answers with success: the socket was closed";
const UNNAMED_ERROR: &str =
    "a socket's pending error is a number the kernel interface does not name";
const NO_MICROSECOND_CLOCK: &str =
    "the host's TCP timestamp clock counts only milliseconds (Linux before 6.7 has no tcp_usec_ts)";
const FLOW_LABEL_FIXED_AT_CONNECT: &str = "Linux fixes a TCP socket's IPv6 flow label only as it connects: a listener, an accepted connection or a connected one takes no other";

impl HostTcpSockets {
    /// Gives the next entry of the error queue, and whether a packet its report carries was cut to the buffer.
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
                let fields = ancillary
                    .packet_fields(length, u32::MIN)
                    .map_err(|_| self.shortfalls.refuse(NO_ARRIVAL_INTERFACE))?;
                Ok((TcpErrorQueueEntry::IcmpError(fields), false))
            }
            libc::SO_EE_ORIGIN_TXSTATUS if wireless.is_some() => {
                let acknowledged = if wireless.is_some_and(|value| value != 0) {
                    WirelessAcknowledgement::Acknowledged
                } else {
                    WirelessAcknowledgement::NotAcknowledged
                };
                // Linux names no send in a wireless status report: its key is the zero it gives.
                Ok((
                    TcpErrorQueueEntry::TransmitReport {
                        report: TransmitReport {
                            key: TransmitReportKey(error.ee_data),
                            interface: None,
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
                            remaining: u32::MIN,
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

impl HostTcpConnection {
    fn new(
        socket: Watched,
        family: IpVersion,
        protocol: TcpProtocol,
        bound_by_number: bool,
        receive_timestamping: u32,
    ) -> Self {
        Self {
            socket,
            family,
            protocol,
            bound_by_number,
            addresses: OnceLock::new(),
            receive_timestamping: AtomicU32::new(receive_timestamping),
            unread_reports: AtomicBool::new(false),
            reports_armed: AtomicBool::new(false),
            reports: Mutex::default(),
            saved_synchronize: Mutex::default(),
            receiving: Mutex::default(),
            anycast: Mutex::default(),
            fixed_flow_label: OnceLock::new(),
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

impl InterfaceAddresses {
    /// Gives the interface that holds the address, or failing that the first whose prefix holds it.
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
        &rustix::net::netlink::SocketAddrNetlink::new(KERNEL_CHOOSES_PORT_ID, groups),
    )
    .ok()?;
    Some(socket)
}

/// Tells whether the subscription heard of a change since it was last read, reading all it holds.
fn changed(changes: BorrowedFd<'_>) -> bool {
    let mut message = [u8::MIN; NETLINK_MESSAGE_ROOM];
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

/// The room a change is read into, a page as netlink(7) advises; a longer one is cut, which loses nothing needed.
///
/// https://man7.org/linux/man-pages/man7/netlink.7.html
const NETLINK_MESSAGE_ROOM: usize = 4096;

/// The port id a netlink socket binds to for the kernel to choose one.
///
/// https://man7.org/linux/man-pages/man7/netlink.7.html
const KERNEL_CHOOSES_PORT_ID: u32 = 0;

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
                interface: InterfaceIndex(NonZeroU32::new(
                    nix::net::if_::if_nametoindex(entry.interface_name.as_str()).ok()?,
                )?),
            })
        })
        .collect()
}

/// Gives where the SYN's IP header starts in the headers `TCP_SAVED_SYN` gave.
///
/// The first place an IP header from the peer to the local address holds a TCP header between their ports that ends the headers.
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

/// Room for the packet information of `IP_PKTOPTIONS` or `IPV6_2292PKTOPTIONS`, which Linux writes before other records.
const PACKET_OPTIONS_ROOM: usize =
    size_of::<libc::cmsghdr>() + size_of::<uapi::in6_pktinfo>() + size_of::<usize>();

/// Control message room aligned as the messages' headers are.
#[repr(C)]
struct PacketOptions {
    _aligned: [libc::cmsghdr; usize::MIN],
    bytes: [u8; PACKET_OPTIONS_ROOM],
}

/// Gives the host error of a failed socket2 call.
fn io_host_error(error: std::io::Error) -> HostError {
    host_error(Errno::from_io_error(&error).unwrap_or(Errno::IO))
}

/// Gives the socket's address as socket2 gives it, which is always an internet one for these sockets.
fn internet_address(address: std::io::Result<SockAddr>) -> Result<SocketAddr, HostError> {
    address
        .map_err(io_host_error)?
        .as_socket()
        .ok_or(HostError::Errno(libc::EAFNOSUPPORT))
}

fn invalid() -> HostError {
    HostError::Errno(libc::EINVAL)
}

/// Gives a duration as a whole number of the unit that fits a C int, or EINVAL.
fn whole(duration: Duration, unit: Duration) -> Result<c_int, HostError> {
    if !duration.as_nanos().is_multiple_of(unit.as_nanos()) {
        return Err(invalid());
    }
    c_int::try_from(duration.as_nanos() / unit.as_nanos()).map_err(|_| invalid())
}

/// The `TCP_QUICKACK` value (any even one but zero) that sends a scheduled acknowledgement, then delays if one was.
///
/// https://github.com/torvalds/linux/blob/master/net/ipv4/tcp.c
const QUICK_ACKNOWLEDGEMENT_THEN_DELAYING: c_int = 2;

/// The units tcp(7) gives its times in.
///
/// https://man7.org/linux/man-pages/man7/tcp.7.html
const SECOND: Duration = Duration::from_secs(1);
/// A millisecond, the unit of `TCP_USER_TIMEOUT` and `TCP_RTO_MAX_MS`.
///
/// https://man7.org/linux/man-pages/man7/tcp.7.html
const MILLISECOND: Duration = Duration::from_millis(1);
/// A microsecond, the unit of `TCP_RTO_MIN_US`.
///
/// https://docs.kernel.org/networking/ip-sysctl.html
const MICROSECOND: Duration = Duration::from_micros(1);

/// The zero tcp(7) and ip(7) take for none or the system's choice: no clamp, timeout, deferral, minimum hop limit or segment size.
///
/// https://man7.org/linux/man-pages/man7/tcp.7.html
const UNSET: c_int = 0;

/// The `TCP_LINGER2` value that resets in place of `FIN_WAIT2`.
///
/// https://man7.org/linux/man-pages/man7/tcp.7.html
const FIN_WAIT2_RESET: c_int = -1;

/// The `SO_PEEK_OFF` value that turns peeking at an offset off.
///
/// https://man7.org/linux/man-pages/man7/socket.7.html
const NO_PEEK_OFFSET: c_int = -1;

/// The low bit of `TCP_TIMESTAMP`'s value, which says that the clock counts microseconds.
///
/// https://github.com/torvalds/linux/blob/master/net/ipv4/tcp.c
const TIMESTAMP_IN_MICROSECONDS: u32 = 1;

/// The `SO_TIMESTAMPING` flags of a socket that asked for no reports.
const NO_REPORT_FLAGS: u32 = u32::MIN;

/// How many keys `TCP_AO_GET_KEYS` is asked for to find one.
///
/// https://docs.kernel.org/networking/tcp_ao.html
const ONE_KEY: u32 = 1;

/// The receive timestamping flags of a socket that asked for none.
const NO_RECEIVE_TIMESTAMPING: u32 = u32::MIN;

/// Turns the packet information on while the socket records the last header or reports ICMP errors, and off otherwise.
///
/// An ICMP error names its arrival interface only in packet information; `IP_RECVTTL` stands for every record.
fn match_packet_information(socket: BorrowedFd<'_>, family: IpVersion) -> Result<(), HostError> {
    let on = |level: u32, option: u32| get_int(socket, level, option).map(|value| value != UNSET);
    let records = on(uapi::SOL_IP, uapi::IP_RECVTTL)?;
    set_flag(
        socket,
        uapi::SOL_IP,
        uapi::IP_PKTINFO,
        records || on(uapi::SOL_IP, uapi::IP_RECVERR)?,
    )?;
    if family == IpVersion::V6 {
        set_flag(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_RECVPKTINFO,
            records || on(uapi::SOL_IPV6, uapi::IPV6_RECVERR)?,
        )?;
    }
    Ok(())
}

fn count(value: usize) -> Result<c_int, HostError> {
    c_int::try_from(value).map_err(|_| invalid())
}

fn protocol_of(protocol: TcpProtocol) -> rustix::net::Protocol {
    match protocol {
        TcpProtocol::Tcp => ipproto::TCP,
        TcpProtocol::Multipath => ipproto::MPTCP,
    }
}

/// Makes the socket hold a flow label to the peer, shared with this process's other sockets, and send the label its connect address carries.
///
/// Linux connects with a label only when the socket holds it.
fn hold_flow_label(
    socket: BorrowedFd<'_>,
    peer: SocketAddr,
    label: Ipv6FlowLabel,
) -> Result<(), HostError> {
    let destination = match peer.ip() {
        IpAddr::V4(address) => address.to_ipv6_mapped(),
        IpAddr::V6(address) => address,
    };
    let request = flow_label_request(
        destination,
        label.value().to_be(),
        uapi::IPV6_FL_A_GET,
        uapi::IPV6_FL_S_PROCESS,
        uapi::IPV6_FL_F_CREATE,
    );
    set_option(socket, uapi::SOL_IPV6, uapi::IPV6_FLOWLABEL_MGR, &request)?;
    set_flag(socket, uapi::SOL_IPV6, uapi::IPV6_FLOWINFO_SEND, true)
}

/// An `IPV6_FLOWLABEL_MGR` request, with no expiry or linger of its own.
fn flow_label_request(
    destination: Ipv6Addr,
    label: u32,
    action: u32,
    share: u32,
    flags: u32,
) -> uapi::in6_flowlabel_req {
    uapi::in6_flowlabel_req {
        flr_dst: in6_addr(destination),
        flr_label: label,
        flr_action: action as u8,
        flr_share: share as u8,
        flr_flags: flags as u16,
        flr_expires: u16::MIN,
        flr_linger: u16::MIN,
        __flr_pad: u32::MIN,
    }
}

/// Sets or removes the MD5 signature key of the peers a prefix holds, through `TCP_MD5SIG_EXT`.
///
/// An IPv4 prefix of an IPv6 socket is given mapped, as Linux takes it. Removing a key that is not there leaves the socket as asked.
fn set_md5_key(
    socket: BorrowedFd<'_>,
    family: IpVersion,
    change: TcpMd5KeyChange<'_>,
) -> Result<(), HostError> {
    let (peer, key) = match change {
        TcpMd5KeyChange::Set { peer, key } => (peer, key),
        TcpMd5KeyChange::Removed(peer) => (peer, &[][..]),
    };
    let mut request = uapi::tcp_md5sig {
        tcpm_addr: storage(key_address(family, peer.prefix.address)),
        tcpm_flags: uapi::TCP_MD5SIG_FLAG_PREFIX as u8,
        tcpm_prefixlen: peer.prefix.length.0,
        tcpm_keylen: u16::MIN,
        tcpm_ifindex: UNSET,
        tcpm_key: std::array::from_fn(|_| u8::MIN),
    };
    if let Some(index) = vrf_index(peer.vrf)? {
        request.tcpm_flags |= uapi::TCP_MD5SIG_FLAG_IFINDEX as u8;
        request.tcpm_ifindex = index;
    }
    request
        .tcpm_key
        .get_mut(..key.len())
        .ok_or_else(invalid)?
        .copy_from_slice(key);
    request.tcpm_keylen = key.len() as u16;
    match set_option(socket, uapi::SOL_TCP, uapi::TCP_MD5SIG_EXT, &request) {
        Err(HostError::Errno(libc::ENOENT)) if key.is_empty() => Ok(()),
        other => other,
    }
}

/// Gives the `SO_TIMESTAMPING` record flags a send's report request asks for, and how many reports they bring.
///
/// A key's peer address as Linux takes it from a socket of the family: an IPv4 one of an IPv6 socket mapped.
fn key_address(family: IpVersion, address: IpAddr) -> IpAddr {
    match (family, address) {
        (IpVersion::V6, IpAddr::V4(address)) => IpAddr::V6(address.to_ipv6_mapped()),
        (_, address) => address,
    }
}

/// Gives the index a key's VRF is flagged with, none for a key of any VRF.
fn vrf_index(vrf: TcpKeyVrf) -> Result<Option<c_int>, HostError> {
    match vrf {
        TcpKeyVrf::Any => Ok(None),
        TcpKeyVrf::Default => Ok(Some(DEFAULT_VRF_INDEX)),
        TcpKeyVrf::Device(index) => c_int::try_from(index.0.get())
            .map(Some)
            .map_err(|_| invalid()),
    }
}

/// The `TCP_AO_*` fields that name a key: its peer, prefix, interface flag and index, and identifiers.
struct AuthenticationKeyName {
    address: StorageBytes,
    prefix: u8,
    flags: u8,
    interface: i32,
    send: u8,
    receive: u8,
}

options::uapi_mirror! {
    /// `struct tcp_ao_getsockopt`, which `TCP_AO_GET_KEYS` reads a key's name from and writes the key into.
    #[derive(IntoBytes)]
    struct KeyQuery mirrors uapi::tcp_ao_getsockopt {
        addr: StorageBytes,
        alg_name: [libc::c_char; 64],
        key: [u8; 80],
        nkeys: u32,
        /// `is_current`, `is_rnext` and `get_all`, left clear to ask for the named key alone.
        _bitfield_1: [u8; 2],
        sndid: u8,
        rcvid: u8,
        prefix: u8,
        maclen: u8,
        keyflags: u8,
        keylen: u8,
        ifindex: i32,
        pkt_good: u64,
        pkt_bad: u64,
    }
}

/// Asks `TCP_AO_GET_KEYS` for the one key of the name.
fn key_query(name: &AuthenticationKeyName) -> KeyQuery {
    KeyQuery {
        addr: name.address,
        prefix: name.prefix,
        keyflags: name.flags,
        ifindex: name.interface,
        sndid: name.send,
        rcvid: name.receive,
        nkeys: ONE_KEY,
        ..KeyQuery::new_zeroed()
    }
}

fn authentication_key_name(
    family: IpVersion,
    identity: &TcpAuthenticationKeyIdentity,
) -> Result<AuthenticationKeyName, HostError> {
    let (flags, interface) = match vrf_index(identity.vrf)? {
        Some(index) => (uapi::TCP_AO_KEYF_IFINDEX as u8, index),
        None => (u8::MIN, DEFAULT_VRF_INDEX),
    };
    Ok(AuthenticationKeyName {
        address: storage_bytes(SocketAddr::new(
            key_address(family, identity.peer.address),
            u16::MIN,
        )),
        prefix: identity.peer.length.0,
        flags,
        interface,
        send: identity.send_identifier.0,
        receive: identity.receive_identifier.0,
    })
}

/// Adds a TCP Authentication Option key through `TCP_AO_ADD_KEY`.
///
/// Linux refuses a key whose identifiers a held key of the peer has (`EEXIST`): when the held key is this one, the socket is as asked, with the key put to its use.
fn add_authentication_key(
    socket: BorrowedFd<'_>,
    family: IpVersion,
    addition: &TcpAuthenticationKeyAddition<'_>,
) -> Result<(), HostError> {
    let name = authentication_key_name(family, &addition.identity)?;
    let mut request = uapi::tcp_ao_add {
        addr: kernel_storage(&name.address),
        alg_name: std::array::from_fn(|_| libc::c_char::default()),
        ifindex: name.interface,
        _bitfield_align_1: [],
        _bitfield_1: Default::default(),
        reserved2: u16::MIN,
        prefix: name.prefix,
        sndid: name.send,
        rcvid: name.receive,
        maclen: addition.mac_length.0,
        keyflags: name.flags,
        keylen: u8::MIN,
        key: std::array::from_fn(|_| u8::MIN),
    };
    if addition.other_options == TcpAuthenticationOptionCoverage::Excluded {
        request.keyflags |= uapi::TCP_AO_KEYF_EXCLUDE_OPT as u8;
    }
    // The algorithm's name is a C string, so one with a NUL or no room for the terminating one is invalid.
    let algorithm = addition.algorithm.as_bytes();
    if algorithm.contains(&u8::MIN) || algorithm.len() >= request.alg_name.len() {
        return Err(invalid());
    }
    for (to, from) in request.alg_name.iter_mut().zip(algorithm) {
        *to = *from as libc::c_char;
    }
    request
        .key
        .get_mut(..addition.secret.len())
        .ok_or_else(invalid)?
        .copy_from_slice(addition.secret);
    request.keylen = addition.secret.len() as u8;
    let (current, next) = match addition.key_use {
        TcpAuthenticationKeyUse::Stored => (false, false),
        TcpAuthenticationKeyUse::Current => (true, false),
        TcpAuthenticationKeyUse::RequestedNext => (false, true),
        TcpAuthenticationKeyUse::CurrentAndRequestedNext => (true, true),
    };
    request.set_set_current(u32::from(current));
    request.set_set_rnext(u32::from(next));
    match set_option(socket, uapi::SOL_TCP, uapi::TCP_AO_ADD_KEY, &request) {
        Err(HostError::Errno(libc::EEXIST))
            if holds_authentication_key(socket, &name, &request) =>
        {
            if current || next {
                change_authentication_options(socket, |options| {
                    options.set_set_current(u32::from(current));
                    options.current_key = request.sndid;
                    options.set_set_rnext(u32::from(next));
                    options.rnext = request.rcvid;
                })
            } else {
                Ok(())
            }
        }
        other => other,
    }
}

/// Tells whether the socket holds the key the request adds, as `TCP_AO_GET_KEYS` gives the key with its name.
fn holds_authentication_key(
    socket: BorrowedFd<'_>,
    name: &AuthenticationKeyName,
    request: &uapi::tcp_ao_add,
) -> bool {
    let mut held = key_query(name);
    options::get_bytes(
        socket,
        uapi::SOL_TCP,
        uapi::TCP_AO_GET_KEYS,
        held.as_mut_bytes(),
    )
    .is_ok()
        && held.nkeys == ONE_KEY
        && held.alg_name == request.alg_name
        && held.maclen == request.maclen
        && held.keyflags == request.keyflags
        && held.key[..usize::from(held.keylen)] == request.key[..usize::from(request.keylen)]
}

/// Removes a TCP Authentication Option key through `TCP_AO_DEL_KEY`, at once or, on a listener, asynchronously.
///
/// Removing one that is not there leaves the socket as asked, with the keys named to be used next put to use.
fn remove_authentication_key(
    socket: BorrowedFd<'_>,
    family: IpVersion,
    removal: &TcpAuthenticationKeyRemoval,
    asynchronously: bool,
) -> Result<(), HostError> {
    let name = authentication_key_name(family, &removal.identity)?;
    let mut request = uapi::tcp_ao_del {
        addr: kernel_storage(&name.address),
        ifindex: name.interface,
        _bitfield_align_1: [],
        _bitfield_1: Default::default(),
        reserved2: u16::MIN,
        prefix: name.prefix,
        sndid: name.send,
        rcvid: name.receive,
        current_key: u8::MIN,
        rnext: u8::MIN,
        keyflags: name.flags,
    };
    if let Some(key) = removal.current {
        request.set_set_current(u32::from(true));
        request.current_key = key.0;
    }
    if let Some(key) = removal.requested_next {
        request.set_set_rnext(u32::from(true));
        request.rnext = key.0;
    }
    request.set_del_async(u32::from(asynchronously));
    match set_option(socket, uapi::SOL_TCP, uapi::TCP_AO_DEL_KEY, &request) {
        Err(HostError::Errno(libc::ENOENT)) => {
            if removal.current.is_none() && removal.requested_next.is_none() {
                return Ok(());
            }
            change_authentication_options(socket, |options| {
                options.set_set_current(request.set_current());
                options.current_key = request.current_key;
                options.set_set_rnext(request.set_rnext());
                options.rnext = request.rnext;
            })
        }
        other => other,
    }
}

/// Writes `TCP_AO_INFO` as the socket holds it, with the change.
///
/// Linux sets the requirement and the ICMP choice at every write, so they are read first (none held is Linux's defaults, zero).
fn change_authentication_options(
    socket: BorrowedFd<'_>,
    change: impl FnOnce(&mut uapi::tcp_ao_info_opt),
) -> Result<(), HostError> {
    let mut options = match options::get_option::<uapi::tcp_ao_info_opt>(
        socket,
        uapi::SOL_TCP,
        uapi::TCP_AO_INFO,
    ) {
        Ok(held) => held,
        Err(HostError::Errno(libc::ENOENT)) => uapi::tcp_ao_info_opt {
            _bitfield_align_1: [],
            _bitfield_1: Default::default(),
            reserved2: u16::MIN,
            current_key: u8::MIN,
            rnext: u8::MIN,
            pkt_good: u64::MIN,
            pkt_bad: u64::MIN,
            pkt_key_not_found: u64::MIN,
            pkt_ao_required: u64::MIN,
            pkt_dropped_icmp: u64::MIN,
        },
        Err(error) => return Err(error),
    };
    options.set_set_current(u32::from(false));
    options.set_set_rnext(u32::from(false));
    options.set_set_counters(u32::from(false));
    change(&mut options);
    set_option(socket, uapi::SOL_TCP, uapi::TCP_AO_INFO, &options)
}

fn set_authentication_options(
    socket: BorrowedFd<'_>,
    choice: &TcpAuthenticationOptions,
) -> Result<(), HostError> {
    change_authentication_options(socket, |options| {
        if let Some(key) = choice.current {
            options.set_set_current(u32::from(true));
            options.current_key = key.0;
        }
        if let Some(key) = choice.requested_next {
            options.set_set_rnext(u32::from(true));
            options.rnext = key.0;
        }
        options.set_ao_required(u32::from(
            choice.requirement == TcpAuthenticationRequirement::Required,
        ));
        options.set_accept_icmps(u32::from(
            choice.icmp_errors == TcpAuthenticationIcmpErrors::Accepted,
        ));
    })
}

fn set_authentication_counters(
    socket: BorrowedFd<'_>,
    counters: &TcpAuthenticationCounters,
) -> Result<(), HostError> {
    change_authentication_options(socket, |options| {
        options.set_set_counters(u32::from(true));
        options.pkt_good = counters.verified.0;
        options.pkt_bad = counters.failed.0;
        options.pkt_key_not_found = counters.key_not_found.0;
        options.pkt_ao_required = counters.signature_missing.0;
        options.pkt_dropped_icmp = counters.icmp_errors_ignored.0;
    })
}

fn record_flags(request: &TransmitReportRequest) -> (u32, u32) {
    let mut flags = u32::MIN;
    let mut reports = u32::MIN;
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

/// Gives one send with an `SO_TIMESTAMPING` control message carrying the record flags.
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

/// Gives the statistics of a transmit report from the `TCP_NLA_*` attributes of its `SCM_TIMESTAMPING_OPT_STATS`.
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
        anycast: &Mutex<AnycastMemberships>,
    ) -> Result<(), HostError> {
        let tcp = |name: u32, value: c_int| set_int(socket, uapi::SOL_TCP, name, value);
        let refuse = |reason| Err(self.shortfalls.refuse(reason));
        match *setting {
            // Linux takes memberships on TCP sockets through `MCAST_JOIN_GROUP` (`IP_ADD_MEMBERSHIP` is EPROTO).
            TcpSetting::Ip(IpSocketSetting::MulticastV4(ref group)) => set_multicast_group(
                socket,
                uapi::SOL_IP,
                IpAddr::V4(group.group),
                group,
                |source| IpAddr::V4(*source),
            ),
            TcpSetting::Ip(IpSocketSetting::MulticastV6(ref group)) => set_multicast_group(
                socket,
                uapi::SOL_IPV6,
                IpAddr::V6(group.group),
                group,
                |source| IpAddr::V6(*source),
            ),
            TcpSetting::Ip(IpSocketSetting::Anycast(ref address)) => {
                locked(anycast).set(socket, address)
            }
            TcpSetting::Packet(IpPacketSetting::FlowLabel(FlowLabelChoice::Label(_))) => {
                refuse(FLOW_LABEL_FIXED_AT_CONNECT)
            }
            TcpSetting::Ip(ref ip) => {
                options::set_ip_socket_setting(socket, family, ip, report_flags, &self.shortfalls)
            }
            TcpSetting::Packet(ref packet) => {
                options::set_packet_setting(socket, packet, &self.shortfalls)
            }
            TcpSetting::Binding(ref binding) => {
                options::set_binding_setting(socket, family, binding)
            }
            TcpSetting::BufferLocks(locks) => {
                let mut value = u32::MIN;
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
                let minimum = minimum.map_or(UNSET, |limit| c_int::from(limit.0));
                match version {
                    IpVersion::V4 => set_int(socket, uapi::SOL_IP, uapi::IP_MINTTL, minimum),
                    IpVersion::V6 => {
                        set_int(socket, uapi::SOL_IPV6, uapi::IPV6_MINHOPCOUNT, minimum)
                    }
                }
            }
            TcpSetting::RecordsLastReceivedHeader(report) => {
                let on = report == Report::Requested;
                for &option in IPV4_RECORDS {
                    set_flag(socket, uapi::SOL_IP, option, on)?;
                }
                if family == IpVersion::V6 {
                    for &option in IPV6_RECORDS {
                        set_flag(socket, uapi::SOL_IPV6, option, on)?;
                    }
                }
                match_packet_information(socket, family)
            }
            TcpSetting::IcmpErrorReports { version, reports } => {
                let on = reports == Report::Requested;
                match version {
                    IpVersion::V4 => set_flag(socket, uapi::SOL_IP, uapi::IP_RECVERR, on),
                    IpVersion::V6 => set_flag(socket, uapi::SOL_IPV6, uapi::IPV6_RECVERR, on),
                }?;
                match_packet_information(socket, family)
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
                .set_tcp_mss(size.map_or(UNSET.unsigned_abs(), |size| u32::from(size.0)))
                .map_err(io_host_error),
            TcpSetting::WindowClamp(clamp) => tcp(
                uapi::TCP_WINDOW_CLAMP,
                match clamp {
                    Some(window) => c_int::try_from(window.0).map_err(|_| invalid())?,
                    None => UNSET,
                },
            ),
            TcpSetting::SynchronizeRetries(retries) => {
                tcp(uapi::TCP_SYNCNT, c_int::from(retries.0))
            }
            TcpSetting::UserTimeout(timeout) => tcp(
                uapi::TCP_USER_TIMEOUT,
                match timeout {
                    Some(timeout) => whole(timeout, MILLISECOND)?,
                    None => UNSET,
                },
            ),
            TcpSetting::DeferAccept(wait) => tcp(
                uapi::TCP_DEFER_ACCEPT,
                match wait {
                    Some(wait) => whole(wait, SECOND)?,
                    None => UNSET,
                },
            ),
            TcpSetting::FinWait2(fin_wait) => tcp(
                uapi::TCP_LINGER2,
                match fin_wait {
                    TcpFinWait2::SystemDefault => UNSET,
                    TcpFinWait2::Lasts(time) => whole(time, SECOND)?,
                    TcpFinWait2::Reset => FIN_WAIT2_RESET,
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
            TcpSetting::Md5Key(change) => set_md5_key(socket, family, change),
            TcpSetting::AddAuthenticationKey(ref addition) => {
                add_authentication_key(socket, family, addition)
            }
            TcpSetting::RemoveAuthenticationKey(ref removal) => {
                remove_authentication_key(socket, family, removal, false)
            }
            TcpSetting::AuthenticationOptions(ref choice) => {
                set_authentication_options(socket, choice)
            }
            TcpSetting::InitialAuthenticationCounters(ref counters) => {
                set_authentication_counters(socket, counters)
            }
            TcpSetting::AuthenticationRepair(ref repair) => {
                state::set_authentication_repair(socket, repair)
            }
            TcpSetting::Repair(mode) => tcp(
                uapi::TCP_REPAIR,
                match mode {
                    TcpRepair::Off => uapi::TCP_REPAIR_OFF as c_int,
                    TcpRepair::OffWithoutWindowProbe => uapi::TCP_REPAIR_OFF_NO_WP as c_int,
                    TcpRepair::On => uapi::TCP_REPAIR_ON as c_int,
                },
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
            TcpSetting::RepairOptions(repair) => state::set_repair_options(socket, repair),
            TcpSetting::RepairWindow(ref window) => state::set_repair_window(socket, window),
        }
    }

    /// Gives the SYN's headers that `TCP_SAVED_SYN` holds, read into the room given or, when longer, into `large`.
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
                *large = vec![u8::MIN; usize::from(u16::MAX)];
                let length = options::get_bytes(socket, uapi::SOL_TCP, uapi::TCP_SAVED_SYN, large)?;
                Ok(&large[..length])
            }
            Err(error) => Err(error),
        }
    }

    /// Gives the interface the SYN came in on, from the packet information the listener's `IP_PKTINFO` kept.
    ///
    /// Or `IPV6_RECVPKTINFO`; the accepted connection goes on with it only as [`match_packet_information`] says.
    fn arrival_interface(
        &self,
        socket: BorrowedFd<'_>,
        family: IpVersion,
        local: IpAddr,
    ) -> Option<InterfaceIndex> {
        let mut control = PacketOptions {
            _aligned: [],
            bytes: [u8::MIN; PACKET_OPTIONS_ROOM],
        };
        let length = match local {
            IpAddr::V4(_) => {
                self.shortfalls.count(ARRIVAL_INTERFACE_OF_THE_HANDSHAKE);
                options::get_bytes(
                    socket,
                    uapi::SOL_IP,
                    uapi::IP_PKTOPTIONS,
                    &mut control.bytes,
                )
            }
            IpAddr::V6(_) => options::get_bytes(
                socket,
                uapi::SOL_IPV6,
                uapi::IPV6_2292PKTOPTIONS,
                &mut control.bytes,
            ),
        }
        .ok()?;
        match_packet_information(socket, family).ok()?;
        Ancillary::parse(&control.bytes[..length]).interface()
    }

    fn synchronize_fields(
        &self,
        socket: BorrowedFd<'_>,
        header: &[u8],
        link_header: Option<usize>,
        local: SocketAddr,
        family: IpVersion,
    ) -> Result<ReceivedPacketFields, HostError> {
        let length = if let Ok(ipv4) = etherparse::Ipv4HeaderSlice::from_slice(header) {
            usize::from(ipv4.total_len())
        } else if let Ok(ipv6) = etherparse::Ipv6HeaderSlice::from_slice(header) {
            usize::from(ipv6.payload_length()) + etherparse::Ipv6Header::LEN
        } else {
            self.shortfalls.count(NO_SAVED_SYNCHRONIZE);
            usize::MIN
        };
        let local = local.ip().to_canonical();
        let specific_destination = match local {
            IpAddr::V4(address) => Some(address),
            IpAddr::V6(_) => None,
        };
        let interface = self
            .arrival_interface(socket, family, local)
            .or_else(|| {
                self.shortfalls.count(ARRIVAL_INTERFACE_BY_ADDRESS);
                self.interfaces.holding(local)
            })
            .ok_or_else(|| self.shortfalls.refuse(NO_SYNCHRONIZE_INTERFACE))?;
        let link_layer_length = link_header
            .filter(|_| length > usize::MIN)
            .and_then(|link| u32::try_from(link + length).ok())
            .map(FrameLength);
        if link_layer_length.is_none() {
            self.shortfalls.count(NO_SYNCHRONIZE_FRAME);
        }
        self.shortfalls.count(SYNCHRONIZE_FIELDS_OF_THE_SOCKET);
        self.shortfalls.count(NO_SYNCHRONIZE_FRAGMENT_SIZE);
        Ok(ReceivedPacketFields {
            length,
            interface,
            specific_destination,
            largest_fragment: None,
            link_layer_length,
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
            drops_since_previous: DropCount(u32::MIN),
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
                reports.sent = u32::MIN;
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
                self.configure_socket(
                    socket,
                    connection.family,
                    &setting,
                    NO_REPORT_FLAGS,
                    &connection.anycast,
                )?;
                connection
                    .unread_reports
                    .store(report == Report::Requested, Ordering::Relaxed);
                Ok(())
            }
            // The label a connection connected with is the one it keeps; asking for it again changes nothing.
            TcpSetting::Packet(IpPacketSetting::FlowLabel(choice))
                if connection.fixed_flow_label.get().is_some() =>
            {
                if connection
                    .fixed_flow_label
                    .get()
                    .copied()
                    .map(FlowLabelChoice::Label)
                    == Some(choice)
                {
                    Ok(())
                } else {
                    Err(self.shortfalls.refuse(FLOW_LABEL_FIXED_AT_CONNECT))
                }
            }
            _ => self.configure_socket(
                socket,
                connection.family,
                &setting,
                NO_REPORT_FLAGS,
                &connection.anycast,
            ),
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
            // A TCP listener keeps its packet information whatever these say, for its SYNs' arrival interfaces.
            TcpListenerSetting::Socket(
                ref setting @ (TcpSetting::RecordsLastReceivedHeader(_)
                | TcpSetting::IcmpErrorReports { .. }),
            ) => {
                self.configure_socket(
                    socket,
                    listener.family,
                    setting,
                    NO_REPORT_FLAGS,
                    &listener.anycast,
                )?;
                if listener.protocol == TcpProtocol::Tcp {
                    self.record_arrival_interfaces(socket, listener.family)?;
                }
                Ok(())
            }
            TcpListenerSetting::Socket(TcpSetting::UnreadByteReports(report)) => {
                self.configure_socket(
                    socket,
                    listener.family,
                    &TcpSetting::UnreadByteReports(report),
                    NO_REPORT_FLAGS,
                    &listener.anycast,
                )?;
                listener
                    .unread_reports
                    .store(report == Report::Requested, Ordering::Relaxed);
                Ok(())
            }
            TcpListenerSetting::Socket(setting) => self.configure_socket(
                socket,
                listener.family,
                &setting,
                NO_REPORT_FLAGS,
                &listener.anycast,
            ),
            TcpListenerSetting::Backlog(backlog) => SockRef::from(&socket)
                .listen(i32::try_from(backlog.0).unwrap_or(i32::MAX))
                .map_err(io_host_error),
            TcpListenerSetting::FastOpenQueue(queue) => set_option(
                socket,
                uapi::SOL_TCP,
                uapi::TCP_FASTOPEN,
                &queue.map_or(u32::MIN, NonZeroU32::get),
            ),
            TcpListenerSetting::FastOpenKeys(keys) => {
                let mut bytes = keys.primary.0.to_vec();
                if let Some(backup) = keys.backup {
                    bytes.extend_from_slice(&backup.0);
                }
                options::set_bytes(socket, uapi::SOL_TCP, uapi::TCP_FASTOPEN_KEY, &bytes)
            }
            // Every TCP listener saves its SYNs' headers from the link header, which accept gives and measures the frame
            // by; the setting decides which of them the accepted connections keep to be taken.
            TcpListenerSetting::SaveSynchronize(save) => {
                let value = match save {
                    TcpSavedSynchronize::Off => SAVE_NONE,
                    TcpSavedSynchronize::FromNetworkHeader => SAVE_FROM_NETWORK_HEADER,
                    TcpSavedSynchronize::FromLinkHeader => SAVE_FROM_LINK_HEADER,
                };
                let saved = match listener.protocol {
                    TcpProtocol::Tcp => SAVE_FROM_LINK_HEADER,
                    TcpProtocol::Multipath => value,
                };
                set_int(socket, uapi::SOL_TCP, uapi::TCP_SAVE_SYN, saved)?;
                listener.saves_synchronize.store(value, Ordering::Relaxed);
                Ok(())
            }
            TcpListenerSetting::RemoveAuthenticationKeyAsynchronously(identity) => {
                remove_authentication_key(
                    socket,
                    listener.family,
                    &TcpAuthenticationKeyRemoval {
                        identity,
                        current: None,
                        requested_next: None,
                    },
                    true,
                )
            }
            TcpListenerSetting::FlowLabelReflection(on) => {
                let action = if on {
                    uapi::IPV6_FL_A_GET
                } else {
                    uapi::IPV6_FL_A_PUT
                };
                let request = flow_label_request(
                    Ipv6Addr::UNSPECIFIED,
                    u32::MIN,
                    action,
                    u32::MIN,
                    uapi::IPV6_FL_F_REFLECT,
                );
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
    /// Makes a TCP listener keep the packet information, with the arrival interface, of what each accepted connection came with.
    fn record_arrival_interfaces(
        &self,
        socket: BorrowedFd<'_>,
        family: IpVersion,
    ) -> Result<(), HostError> {
        set_flag(socket, uapi::SOL_IP, uapi::IP_PKTINFO, true)?;
        if family == IpVersion::V6 {
            set_flag(socket, uapi::SOL_IPV6, uapi::IPV6_RECVPKTINFO, true)?;
        }
        Ok(())
    }

    /// Binds the socket as the binding asks, taking a port the exit node chooses at once.
    ///
    /// Linux chooses only inside the host's own port range, so the shared crate tries the rest of a range port by port.
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

impl NegativeRouteAdvisable<HostTcpConnection> for HostTcpSockets {
    fn give_negative_route_advice(&self, connection: &HostTcpConnection) -> Result<(), HostError> {
        set_flag(
            connection.socket.fd(),
            uapi::SOL_SOCKET,
            uapi::SO_CNX_ADVICE,
            true,
        )
    }
}

impl HostTcpSockets {
    /// Opens the socket a connect or listen starts from, unbound with the binding it is to take, or as it was bound.
    fn local_end<'a>(
        &self,
        local: TcpLocalEnd<'a, HostTcpBoundSocket>,
    ) -> Result<(HostTcpBoundSocket, Option<LocalBinding<'a>>), TcpLocalEndError<HostTcpBoundSocket>>
    {
        match local {
            TcpLocalEnd::Bound(bound) => Ok((bound, None)),
            TcpLocalEnd::Unbound {
                protocol,
                owner,
                local,
            } => {
                let family = family_of(&local.address);
                let socket = open_socket(family, SocketType::STREAM, protocol_of(protocol), owner)
                    .map_err(TcpLocalEndError::Unbound)?;
                Ok((
                    HostTcpBoundSocket {
                        socket,
                        family,
                        protocol,
                        bound_by_number: bound_by_number(&local),
                    },
                    Some(local),
                ))
            }
        }
    }

    /// Starts connecting, binding first when a binding is given, and gives the local address and how it started.
    fn start_connection(
        &self,
        connection: &HostTcpConnection,
        local: Option<&LocalBinding<'_>>,
        peer: SocketAddr,
        settings: &[TcpSetting<'_>],
        fast_open: TcpFastOpen<'_>,
    ) -> Result<(SocketAddr, TcpConnectStart), Failure> {
        let fixed_flow_label = settings
            .iter()
            .rev()
            .find_map(|setting| match setting {
                TcpSetting::Packet(IpPacketSetting::FlowLabel(choice)) => Some(*choice),
                _ => None,
            })
            .and_then(|choice| match choice {
                FlowLabelChoice::Label(label) => Some(label),
                _ => None,
            });
        for setting in settings {
            // A fixed label is given as the connection connects.
            if !matches!(
                setting,
                TcpSetting::Packet(IpPacketSetting::FlowLabel(FlowLabelChoice::Label(_)))
            ) {
                self.configure(connection, setting.clone())
                    .map_err(Failure::Before)?;
            }
        }
        let socket = connection.socket.fd();
        if let Some(local) = local {
            self.bind(socket, local).map_err(Failure::Before)?;
        }
        let peer = peer_for(connection.family, peer);
        let mut destination = peer;
        if let Some(label) = fixed_flow_label {
            hold_flow_label(socket, peer, label).map_err(Failure::Before)?;
            // The standard library keeps `sin6_flowinfo` as it is, which Linux reads in network order.
            if let SocketAddr::V6(ref mut address) = destination {
                address.set_flowinfo(label.value().to_be());
            }
            let _ = connection.fixed_flow_label.set(label);
        }
        let in_progress = |outcome: Result<usize, HostError>| match outcome {
            Err(HostError::Errno(libc::EINPROGRESS)) => Ok(None),
            other => other.map(Some),
        };
        let start = match fast_open {
            TcpFastOpen::Off | TcpFastOpen::UntilFirstSend => {
                if fast_open == TcpFastOpen::UntilFirstSend {
                    set_flag(socket, uapi::SOL_TCP, uapi::TCP_FASTOPEN_CONNECT, true)
                        .map_err(Failure::Before)?;
                }
                let outcome = in_progress(
                    SockRef::from(&socket)
                        .connect(&SockAddr::from(destination))
                        .map(|()| usize::MIN)
                        .map_err(io_host_error),
                )
                .map_err(Failure::Before)?;
                match outcome {
                    None => TcpConnectStart::SynchronizeSent {
                        fast_open_data_taken: ByteCount(usize::MIN),
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
                        .send_to_with_flags(data, &SockAddr::from(destination), flags)
                        .map_err(io_host_error),
                )
                .map_err(Failure::Before)?;
                TcpConnectStart::SynchronizeSent {
                    fast_open_data_taken: ByteCount(taken.unwrap_or(usize::MIN)),
                }
            }
        };
        let local =
            internet_address(SockRef::from(&socket).local_addr()).map_err(Failure::After)?;
        let _ = connection.addresses.set((local, peer));
        Ok((local, start))
    }

    /// Ends a socket whose connect or listen failed after it started, giving it back where its port stays held.
    fn end_after_failure(
        &self,
        bound: HostTcpBoundSocket,
        error: HostError,
    ) -> TcpLocalEndError<HostTcpBoundSocket> {
        match self.disconnected(bound).0 {
            Some(bound_socket) => TcpLocalEndError::Bound {
                bound_socket,
                error,
            },
            None => TcpLocalEndError::Unbound(error),
        }
    }

    /// Disconnects the socket as a connect to `AF_UNSPEC` does, giving it back if its port was bound by number, and its pending error.
    fn disconnected(
        &self,
        bound: HostTcpBoundSocket,
    ) -> (Option<HostTcpBoundSocket>, Option<KernelErrno>) {
        let unspecified = rustix::net::connect_unspec(&bound.socket);
        let pending_error = self
            .take_pending_error(bound.socket.as_fd())
            .unwrap_or(None);
        // Linux answers it with success in every state.
        if unspecified.is_err() {
            self.shortfalls.count(DISCONNECT_REFUSED);
            return (None, pending_error);
        }
        (bound.bound_by_number.then_some(bound), pending_error)
    }

    /// Takes the socket's `SO_ERROR`, which Linux clears as it reads it.
    fn take_pending_error(&self, socket: BorrowedFd<'_>) -> Result<Option<KernelErrno>, HostError> {
        match SockRef::from(&socket).take_error().map_err(io_host_error)? {
            None => Ok(None),
            Some(error) => error
                .raw_os_error()
                .and_then(errno_of)
                .map(Some)
                .ok_or_else(|| self.shortfalls.refuse(UNNAMED_ERROR)),
        }
    }
}

/// Where a connect or listen failed: before the socket started connecting or listening, or after.
enum Failure {
    Before(HostError),
    After(HostError),
}

/// Tells whether a binding binds its port by number, which Linux keeps held where it frees a port bound as 0.
fn bound_by_number(local: &LocalBinding<'_>) -> bool {
    match local.port {
        LocalPort::Requested(port) | LocalPort::ChosenByKernel(port) => port != ANY_PORT,
        LocalPort::ChosenByExitNode(_) => false,
    }
}

/// The port a bind asks the host to choose with, which Linux binds as 0.
///
/// https://man7.org/linux/man-pages/man7/ip.7.html#:~:text=If%20the%20port%20is%20zero
const ANY_PORT: Port = Port(u16::MIN);

impl HostTcpConnection {
    /// Gives the connection's socket back as only bound, no longer watched.
    fn into_bound(self) -> HostTcpBoundSocket {
        HostTcpBoundSocket {
            family: self.family,
            protocol: self.protocol,
            bound_by_number: self.bound_by_number,
            socket: self.socket.unwatch(),
        }
    }
}

impl TcpConnections for HostTcpSockets {
    type Connection = HostTcpConnection;
    type BoundSocket = HostTcpBoundSocket;

    fn bind(
        &self,
        protocol: TcpProtocol,
        owner: UserId,
        local: LocalBinding<'_>,
        settings: &[BindingSetting<'_>],
    ) -> Result<TcpBound<Self::BoundSocket>, HostError> {
        let family = family_of(&local.address);
        let socket = open_socket(family, SocketType::STREAM, protocol_of(protocol), owner)?;
        for setting in settings {
            options::set_binding_setting(socket.as_fd(), family, setting)?;
        }
        self.bind(socket.as_fd(), &local)?;
        let local_address = internet_address(SockRef::from(&socket).local_addr())?;
        Ok(TcpBound {
            bound_socket: HostTcpBoundSocket {
                socket,
                family,
                protocol,
                bound_by_number: bound_by_number(&local),
            },
            local: local_address,
        })
    }

    fn connect(
        &self,
        local: TcpLocalEnd<'_, Self::BoundSocket>,
        peer: SocketAddr,
        settings: &[TcpSetting<'_>],
        fast_open: TcpFastOpen<'_>,
    ) -> Result<TcpConnected<Self::Connection>, TcpLocalEndError<Self::BoundSocket>> {
        let (bound, binding) = self.local_end(local)?;
        let HostTcpBoundSocket {
            socket,
            family,
            protocol,
            bound_by_number,
        } = bound;
        let connection = HostTcpConnection::new(
            self.readiness
                .watch(socket, ReadyFor::all())
                .map_err(TcpLocalEndError::Unbound)?,
            family,
            protocol,
            bound_by_number,
            NO_RECEIVE_TIMESTAMPING,
        );
        match self.start_connection(&connection, binding.as_ref(), peer, settings, fast_open) {
            Ok((local, start)) => Ok(TcpConnected {
                connection,
                local,
                start,
            }),
            Err(Failure::Before(error)) => {
                // A clash of the four addresses and ports frees a port bound as 0.
                let lost = error == HostError::Errno(libc::EADDRNOTAVAIL) && !bound_by_number;
                Err(match binding {
                    Some(_) => TcpLocalEndError::Unbound(error),
                    None if lost => TcpLocalEndError::Unbound(error),
                    None => TcpLocalEndError::Bound {
                        bound_socket: connection.into_bound(),
                        error,
                    },
                })
            }
            Err(Failure::After(error)) => Err(match binding {
                Some(_) => TcpLocalEndError::Unbound(error),
                None => self.end_after_failure(connection.into_bound(), error),
            }),
        }
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
        let mut offset = UNSET;
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
            rustix::net::recv(socket, into, RecvFlags::from_bits_retain(flags as u32))
                .map(|(_, length)| TcpReceived {
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
            set_int(socket, uapi::SOL_SOCKET, uapi::SO_PEEK_OFF, NO_PEEK_OFFSET)?;
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
        let socket = connection.socket.fd();
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
            TcpQuery::LastReceivedHeader(into) => {
                let peer = connection.addresses.get().map_or(
                    IpAddr::from(std::net::Ipv4Addr::UNSPECIFIED),
                    |(_, peer)| peer.ip(),
                );
                state::last_received_header(socket, peer, into, &self.shortfalls)
            }
            TcpQuery::Queues => state::queues(socket, &self.shortfalls).map(TcpReading::Queues),
            TcpQuery::Repair => state::repair_state(socket).map(TcpReading::Repair),
            TcpQuery::Multipath => {
                state::multipath(socket, &self.shortfalls).map(TcpReading::Multipath)
            }
            TcpQuery::MultipathSubflow(position) => {
                state::multipath_subflow(socket, position, &self.shortfalls)
                    .map(TcpReading::MultipathSubflow)
            }
            TcpQuery::Information => state::information(socket, &self.shortfalls).map(
                |(information, unacknowledged, selectively_acknowledged)| TcpReading::Information {
                    information,
                    unacknowledged: SegmentCount(u64::from(unacknowledged)),
                    selectively_acknowledged: SegmentCount(u64::from(selectively_acknowledged)),
                },
            ),
            TcpQuery::CongestionControl(into) => {
                state::congestion_control(socket, into, &self.shortfalls)
                    .map(|(name, information)| TcpReading::CongestionControl { name, information })
            }
            TcpQuery::Tuned => state::tuned(socket, &self.shortfalls).map(TcpReading::Tuned),
            TcpQuery::Timestamp => {
                state::timestamp(socket, &self.shortfalls).map(TcpReading::Timestamp)
            }
            TcpQuery::Authentication => {
                state::authentication(socket).map(TcpReading::Authentication)
            }
            TcpQuery::AuthenticationKey(identity) => {
                state::authentication_key(socket, connection.family, &identity)
                    .map(TcpReading::AuthenticationKey)
            }
            TcpQuery::IncomingProcessor => {
                state::incoming_processor(socket).map(TcpReading::IncomingProcessor)
            }
            TcpQuery::TakePendingError => self
                .take_pending_error(socket)
                .map(TcpReading::PendingError),
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

    fn disconnect(&self, connection: Self::Connection) -> TcpDisconnected<Self::BoundSocket> {
        // An accepted connection's port is the listener's, which a disconnect never keeps for it.
        let (bound_socket, pending_error) = self.disconnected(connection.into_bound());
        TcpDisconnected {
            bound_socket,
            pending_error,
        }
    }

    fn release(&self, bound_socket: Self::BoundSocket) {
        drop(bound_socket);
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
        loop {
            match self.next_error_queue_entry(connection, &mut []) {
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
        connection: &Self::Connection,
        values: TcpAuthenticationCounters,
    ) -> Result<(), HostError> {
        set_authentication_counters(connection.socket.fd(), &values)
    }
}

impl HostTcpSockets {
    /// Sets the clock's value and, from the value's low bit, the unit it counts in from now on, as Linux 6.7 and later do.
    ///
    /// An older kernel counts milliseconds whatever the bit: a clock asked for microseconds there is put back and refused.
    fn set_timestamp_clock(
        &self,
        socket: BorrowedFd<'_>,
        reading: TcpTimestampClock,
    ) -> Result<(), HostError> {
        let microseconds = reading.unit == TcpTimestampUnit::Microseconds;
        let unit = if microseconds {
            TIMESTAMP_IN_MICROSECONDS
        } else {
            u32::MIN
        };
        let value = reading.value.0 & !TIMESTAMP_IN_MICROSECONDS | unit;
        if !microseconds {
            return set_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP, &value);
        }
        let before: u32 = options::get_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP)?;
        set_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP, &value)?;
        let information: uapi::tcp_info =
            options::get_option(socket, uapi::SOL_TCP, uapi::TCP_INFO)?;
        if u32::from(information.tcpi_options) & uapi::TCPI_OPT_USEC_TS == u32::MIN {
            set_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP, &before)?;
            return Err(self.shortfalls.refuse(NO_MICROSECOND_CLOCK));
        }
        Ok(())
    }
}

impl HostTcpSockets {
    /// Starts listening, binding first when a binding is given, and gives the local address.
    ///
    /// The backlog is the last one among the settings, else the one given, so that the socket listens once.
    fn start_listening(
        &self,
        listener: &HostTcpListener,
        local: Option<&LocalBinding<'_>>,
        backlog: ListenBacklog,
        settings: &[TcpListenerSetting<'_>],
    ) -> Result<SocketAddr, Failure> {
        let socket = listener.socket.fd();
        // Multipath sockets have no TCP_SAVE_SYN.
        if listener.protocol == TcpProtocol::Tcp {
            set_int(
                socket,
                uapi::SOL_TCP,
                uapi::TCP_SAVE_SYN,
                SAVE_FROM_LINK_HEADER,
            )
            .and_then(|()| self.record_arrival_interfaces(socket, listener.family))
            .map_err(Failure::Before)?;
        }
        let mut backlog = backlog;
        for setting in settings {
            match setting {
                TcpListenerSetting::Backlog(last) => backlog = *last,
                other => self
                    .configure(listener, other.clone())
                    .map_err(Failure::Before)?,
            }
        }
        if let Some(local) = local {
            self.bind(socket, local).map_err(Failure::Before)?;
        }
        SockRef::from(&socket)
            .listen(i32::try_from(backlog.0).unwrap_or(i32::MAX))
            .map_err(|error| Failure::Before(io_host_error(error)))?;
        internet_address(SockRef::from(&socket).local_addr()).map_err(Failure::After)
    }
}

impl TcpListeners for HostTcpSockets {
    type Listener = HostTcpListener;

    fn listen(
        &self,
        local: TcpLocalEnd<'_, Self::BoundSocket>,
        backlog: ListenBacklog,
        settings: &[TcpListenerSetting<'_>],
    ) -> Result<TcpListening<Self::Listener>, TcpLocalEndError<Self::BoundSocket>> {
        let (bound, binding) = self.local_end(local)?;
        let HostTcpBoundSocket {
            socket,
            family,
            protocol,
            bound_by_number,
        } = bound;
        let listener = HostTcpListener {
            socket: self
                .readiness
                .watch(socket, ReadyFor::all())
                .map_err(TcpLocalEndError::Unbound)?,
            family,
            protocol,
            receive_timestamping: AtomicU32::new(NO_RECEIVE_TIMESTAMPING),
            bound_clock: Mutex::new(None),
            saves_synchronize: AtomicI32::new(SAVE_NONE),
            unread_reports: AtomicBool::new(false),
            anycast: Mutex::default(),
        };
        let give_back = |listener: HostTcpListener| HostTcpBoundSocket {
            socket: listener.socket.unwatch(),
            family,
            protocol,
            bound_by_number,
        };
        match self.start_listening(&listener, binding.as_ref(), backlog, settings) {
            Ok(local) => Ok(TcpListening { listener, local }),
            // A listen that fails keeps the port, even one bound as 0.
            Err(Failure::Before(error)) => Err(match binding {
                Some(_) => TcpLocalEndError::Unbound(error),
                None => TcpLocalEndError::Bound {
                    bound_socket: give_back(listener),
                    error,
                },
            }),
            Err(Failure::After(error)) => Err(match binding {
                Some(_) => TcpLocalEndError::Unbound(error),
                None => self.end_after_failure(give_back(listener), error),
            }),
        }
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
            self.readiness
                .watch(OwnedFd::from(socket), ReadyFor::all())?,
            listener.family,
            listener.protocol,
            false,
            listener.receive_timestamping.load(Ordering::Relaxed),
        );
        connection.unread_reports.store(
            listener.unread_reports.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
        let bound_clock = *locked(&listener.bound_clock);
        if bound_clock.is_some() {
            connection.reports().bound_clock = bound_clock;
        }
        let socket = connection.socket.fd();
        let local = internet_address(SockRef::from(&socket).local_addr())?;
        let _ = connection.addresses.set((local, peer));
        let mut room = [u8::MIN; SAVED_SYNCHRONIZE_ROOM];
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
        let link_header = (!network.is_empty()).then(|| saved.len() - network.len());
        let synchronize =
            self.synchronize_fields(socket, network, link_header, local, listener.family)?;
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
        listener: &Self::Listener,
        query: TcpListenerQuery<'a>,
    ) -> Result<TcpListenerReading<'a>, HostError> {
        let socket = listener.socket.fd();
        let shortfalls = &self.shortfalls;
        match query {
            // A listener's `tcpi_unacked` is its accept queue's length and `tcpi_sacked` its backlog.
            TcpListenerQuery::Information => {
                state::information(socket, shortfalls).map(|(information, waiting, backlog)| {
                    TcpListenerReading::Information {
                        information,
                        waiting_to_be_accepted: ConnectionCount(waiting),
                        backlog: ListenBacklog(backlog),
                    }
                })
            }
            TcpListenerQuery::CongestionControl(into) => {
                state::congestion_control(socket, into, shortfalls).map(|(name, information)| {
                    TcpListenerReading::CongestionControl { name, information }
                })
            }
            TcpListenerQuery::Tuned => {
                state::tuned(socket, shortfalls).map(TcpListenerReading::Tuned)
            }
            TcpListenerQuery::Timestamp => {
                state::timestamp(socket, shortfalls).map(TcpListenerReading::Timestamp)
            }
            TcpListenerQuery::Authentication => {
                state::authentication(socket).map(TcpListenerReading::Authentication)
            }
            TcpListenerQuery::AuthenticationKey(identity) => {
                state::authentication_key(socket, listener.family, &identity)
                    .map(TcpListenerReading::AuthenticationKey)
            }
            TcpListenerQuery::IncomingProcessor => {
                state::incoming_processor(socket).map(TcpListenerReading::IncomingProcessor)
            }
            TcpListenerQuery::TakePendingError => self
                .take_pending_error(socket)
                .map(TcpListenerReading::PendingError),
        }
    }

    fn set_listener_authentication_counters(
        &self,
        listener: &Self::Listener,
        values: TcpAuthenticationCounters,
    ) -> Result<(), HostError> {
        set_authentication_counters(listener.socket.fd(), &values)
    }

    fn set_listener_timestamp_clock(
        &self,
        listener: &Self::Listener,
        reading: TcpTimestampClock,
    ) -> Result<(), HostError> {
        self.set_timestamp_clock(listener.socket.fd(), reading)
    }

    fn stop_listening(&self, listener: Self::Listener) {
        drop(listener);
    }
}

impl Readiness<HostTcpConnection> for HostTcpSockets {
    async fn wait_until_ready(
        &self,
        connection: &HostTcpConnection,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        connection.socket.wait_until_ready(interest).await
    }
}

impl Readiness<HostTcpListener> for HostTcpSockets {
    async fn wait_until_ready(
        &self,
        listener: &HostTcpListener,
        interest: ReadyFor,
    ) -> Result<ReadinessReport, HostError> {
        listener.socket.wait_until_ready(interest).await
    }
}

impl TcpSockets for HostTcpSockets {}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::pin::pin;
    use std::sync::Arc;
    use std::task::{Context, Wake, Waker};
    use std::thread::Thread;

    use wcontainers_internet_protocol::{BindAddress, Port};

    use super::*;

    struct Unparking(Thread);

    impl Wake for Unparking {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    /// How long a wait may take to be woken once its data is sent.
    const DEADLINE: Duration = Duration::from_secs(10);

    /// Gives a listener and the two ends of a connection to it, on IPv4 loopback.
    fn connected(
        sockets: &HostTcpSockets,
    ) -> (HostTcpListener, HostTcpConnection, HostTcpConnection) {
        let loopback = LocalBinding {
            address: BindAddress::V4(Ipv4Addr::LOCALHOST),
            port: LocalPort::Requested(Port(0)),
        };
        let local = || TcpLocalEnd::Unbound {
            protocol: TcpProtocol::Tcp,
            owner: UserId(0),
            local: loopback,
        };
        let failed = |error: TcpLocalEndError<HostTcpBoundSocket>| match error {
            TcpLocalEndError::Bound { error, .. } | TcpLocalEndError::Unbound(error) => error,
        };
        let listening = sockets
            .listen(local(), ListenBacklog(1), &[])
            .map_err(failed)
            .expect("listen");
        let client = sockets
            .connect(local(), listening.local, &[], TcpFastOpen::Off)
            .map_err(failed)
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

    /// Waits on the connection until one of the facts holds.
    fn wait(
        sockets: &HostTcpSockets,
        connection: &HostTcpConnection,
        interest: ReadyFor,
        waker: &Waker,
    ) {
        let mut context = Context::from_waker(waker);
        let mut wait = pin!(sockets.wait_until_ready(connection, interest));
        let start = std::time::Instant::now();
        while wait.as_mut().poll(&mut context).is_pending() {
            assert!(start.elapsed() < DEADLINE, "the wait was not woken");
            std::thread::park_timeout(DEADLINE);
        }
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
        wait(&sockets, &server, ReadyFor::READABLE, &waker);
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

    /// Checks that discarding a transmit report that carries its packet counts no packet cut to a buffer.
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
        wait(&sockets, &client, ReadyFor::empty(), &waker);
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
