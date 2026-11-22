//! TCP connections and listeners made of the sockets of the machine it runs on: on Linux every call, request and
//! setting, elsewhere what socket2 covers.
#![cfg(unix)]

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(target_os = "linux"))]
mod portable;

#[cfg(target_os = "linux")]
use std::collections::BTreeMap;
#[cfg(target_os = "linux")]
use std::net::{IpAddr, SocketAddr};
use std::os::fd::AsRawFd;
#[cfg(target_os = "linux")]
use std::os::fd::OwnedFd;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32};
#[cfg(target_os = "linux")]
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "linux")]
use etherparse::Ipv6FlowLabel;
use wcontainers_abi::{CounterReading, HostError};
use wcontainers_internet_protocol::IpVersion;
#[cfg(target_os = "linux")]
use wcontainers_internet_protocol::{InterfaceIndex, PtpHardwareClockIndex, TransmitReportKey};
use wcontainers_internet_protocol_linux_socket_options::Shortfalls;
#[cfg(target_os = "linux")]
use wcontainers_internet_protocol_linux_socket_options::datagram::memberships::AnycastMemberships;
use wcontainers_internet_protocol_linux_socket_options::readiness::{Watched, Watcher};
#[cfg(target_os = "linux")]
use wcontainers_tcp_sockets::TcpProtocol;

/// The host's TCP sockets as the exit node's TCP connections and listeners, with a count of each refusal by its reason.
pub struct HostTcpSockets {
    shortfalls: Shortfalls,
    #[cfg(target_os = "linux")]
    interfaces: InterfaceAddresses,
    readiness: Watcher,
}

pub struct HostTcpConnection {
    socket: Watched,
    family: IpVersion,
    /// The local and peer addresses, kept for the ICMP errors that come after the connection is gone.
    #[cfg(target_os = "linux")]
    addresses: OnceLock<(SocketAddr, SocketAddr)>,
    #[cfg(target_os = "linux")]
    receive_timestamping: AtomicU32,
    #[cfg(target_os = "linux")]
    unread_reports: AtomicBool,
    /// Whether the reports hold report flags, so that every send's bytes are counted.
    #[cfg(target_os = "linux")]
    reports_armed: AtomicBool,
    #[cfg(target_os = "linux")]
    reports: Mutex<TransmitReports>,
    /// The SYN's headers, kept when the listener was asked to save them, until they are taken.
    #[cfg(target_os = "linux")]
    saved_synchronize: Mutex<Option<Vec<u8>>>,
    /// Held by every receive, since a peek at an offset sets the socket-wide `SO_PEEK_OFF` for its call.
    #[cfg(target_os = "linux")]
    receiving: Mutex<()>,
    #[cfg(target_os = "linux")]
    anycast: Mutex<AnycastMemberships>,
    /// The IPv6 flow label the connection was given as it connected, which Linux keeps for good.
    #[cfg(target_os = "linux")]
    fixed_flow_label: OnceLock<Ipv6FlowLabel>,
}

pub struct HostTcpListener {
    socket: Watched,
    family: IpVersion,
    #[cfg(target_os = "linux")]
    protocol: TcpProtocol,
    #[cfg(target_os = "linux")]
    receive_timestamping: AtomicU32,
    #[cfg(target_os = "linux")]
    bound_clock: Mutex<Option<PtpHardwareClockIndex>>,
    /// The guest's `TCP_SAVE_SYN` value: whether accepted connections keep their SYN's headers, and from which header.
    #[cfg(target_os = "linux")]
    saves_synchronize: AtomicI32,
    /// Whether the guest asked the listener's sockets to record the last header they received.
    #[cfg(target_os = "linux")]
    records_last_received: AtomicBool,
    /// Whether the connections it accepts report the bytes left unread, as Linux's `TCP_INQ` is copied to them.
    #[cfg(target_os = "linux")]
    unread_reports: AtomicBool,
    #[cfg(target_os = "linux")]
    anycast: Mutex<AnycastMemberships>,
}

/// The transmit reports a connection asked for and the guest keys they are reported under.
#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy)]
struct ReportedSend {
    key: TransmitReportKey,
    remaining: u32,
    packet: bool,
    statistics: bool,
}

/// An interface address of the host and its prefix.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy)]
struct HeldAddress {
    address: IpAddr,
    netmask: Option<IpAddr>,
    interface: InterfaceIndex,
}

/// The host's interface addresses, read again only after the kernel announced a change of them.
#[cfg(target_os = "linux")]
#[derive(Debug, Default)]
struct InterfaceAddresses {
    /// A routing netlink socket that hears of every address change, none when the host refused one.
    changes: OnceLock<Option<OwnedFd>>,
    table: Mutex<Option<Vec<HeldAddress>>>,
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

impl HostTcpSockets {
    pub fn new() -> Result<Self, HostError> {
        Ok(Self {
            shortfalls: Shortfalls::default(),
            #[cfg(target_os = "linux")]
            interfaces: InterfaceAddresses::default(),
            readiness: Watcher::new()?,
        })
    }

    /// Reads how many requests each named reason has made this value refuse.
    pub fn counters(&self) -> Vec<CounterReading> {
        self.shortfalls.readings()
    }
}
