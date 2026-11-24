//! Socket options by level and name: through socket2, rustix or nix where one has a typed call for the option, else
//! through the listed plain options of wcontainers-socket-options; any other is refused with `ENOPROTOOPT`.

use std::ffi::c_int;
use std::os::fd::BorrowedFd;
use std::time::Duration;

use linux_raw_sys::net as uapi;
use nix::sys::socket::{getsockopt, setsockopt, sockopt};
use rustix::io::Errno;
use rustix::net::sockopt as rustix_option;
use wcontainers_abi::HostError;
use wcontainers_socket_options::{
    AUTHENTICATION_KEY_QUERY, MulticastLevel, PlainOption, SocketOptions, SourceFilterOptions,
    StorageBytes,
};
use wcontainers_socket_options_linux::LinuxSocketOptions;
use zerocopy::{FromBytes, Immutable, IntoBytes};

use crate::{host_error, io_host_error, sock};

/// `UDP_GRO`, which linux-raw-sys does not carry.
const UDP_GRO: u32 = libc::UDP_GRO as u32;

fn listed(level: u32, name: u32) -> Result<PlainOption, HostError> {
    PlainOption::find(level, name).ok_or(host_error(Errno::NOPROTOOPT))
}

fn from_errno(errno: Errno) -> HostError {
    host_error(errno)
}

fn from_nix(errno: nix::errno::Errno) -> HostError {
    HostError::Errno(errno as i32)
}

/// Sets a listed socket option to the bytes of a value.
pub fn set_option<T: IntoBytes + Immutable + ?Sized>(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    value: &T,
) -> Result<(), HostError> {
    set_bytes(socket, level, name, value.as_bytes())
}

/// Sets a listed socket option to the bytes.
pub fn set_bytes(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    bytes: &[u8],
) -> Result<(), HostError> {
    LinuxSocketOptions
        .set_option(socket, listed(level, name)?, bytes)
        .map_err(from_errno)
}

/// Reads a listed socket option into the buffer, returning the length the kernel gave.
pub fn get_bytes(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    into: &mut [u8],
) -> Result<usize, HostError> {
    LinuxSocketOptions
        .get_option(socket, listed(level, name)?, into)
        .map_err(from_errno)
}

/// Reads a listed socket option whose value is plain data that every bit pattern is valid for.
pub fn get_option<T: FromBytes + IntoBytes>(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
) -> Result<T, HostError> {
    let mut value = T::new_zeroed();
    get_bytes(socket, level, name, value.as_mut_bytes())?;
    Ok(value)
}

pub fn set_flag(socket: BorrowedFd<'_>, level: u32, name: u32, on: bool) -> Result<(), HostError> {
    set_int(socket, level, name, c_int::from(on))
}

/// The value as the unsigned number a typed call takes, `EINVAL` for a negative one.
fn unsigned(value: c_int) -> Result<u32, HostError> {
    u32::try_from(value).map_err(|_| from_errno(Errno::INVAL))
}

fn seconds(value: c_int) -> Result<Duration, HostError> {
    unsigned(value).map(|seconds| Duration::from_secs(seconds.into()))
}

/// Sets an integer socket option through a typed call where socket2, rustix or nix has one, else as a listed option.
pub fn set_int(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    value: c_int,
) -> Result<(), HostError> {
    let on = value != c_int::default();
    let io = |result: std::io::Result<()>| result.map_err(|error| io_host_error(&error));
    let rustix = |result: rustix::io::Result<()>| result.map_err(from_errno);
    let nix = |result: nix::Result<()>| result.map_err(from_nix);
    let socket2 = sock(&socket);
    match (level, name) {
        (uapi::SOL_SOCKET, uapi::SO_MARK) => io(socket2.set_mark(unsigned(value)?)),
        (uapi::SOL_SOCKET, uapi::SO_PRIORITY) => io(socket2.set_priority(unsigned(value)?)),
        (uapi::SOL_SOCKET, uapi::SO_BROADCAST) => io(socket2.set_broadcast(on)),
        (uapi::SOL_SOCKET, uapi::SO_DONTROUTE) => nix(setsockopt(&socket, sockopt::DontRoute, &on)),
        (uapi::SOL_SOCKET, uapi::SO_RXQ_OVFL) => nix(setsockopt(&socket, sockopt::RxqOvfl, &value)),
        (uapi::SOL_IP, uapi::IP_TOS) => io(socket2.set_tos_v4(unsigned(value)?)),
        (uapi::SOL_IP, uapi::IP_MTU_DISCOVER) => rustix(rustix_option::set_ip_mtu_discover(
            socket,
            rustix_option::Ipv4PathMtuDiscovery::from_raw(value as _),
        )),
        (uapi::SOL_IP, uapi::IP_TRANSPARENT) => io(socket2.set_ip_transparent_v4(on)),
        (uapi::SOL_IP, uapi::IP_FREEBIND) => io(socket2.set_freebind_v4(on)),
        (uapi::SOL_IP, uapi::IP_RECVTOS) => io(socket2.set_recv_tos_v4(on)),
        (uapi::SOL_IP, uapi::IP_PKTINFO) => nix(setsockopt(&socket, sockopt::Ipv4PacketInfo, &on)),
        (uapi::SOL_IP, uapi::IP_RECVTTL) => nix(setsockopt(&socket, sockopt::Ipv4RecvTtl, &on)),
        (uapi::SOL_IP, uapi::IP_RECVERR) => nix(setsockopt(&socket, sockopt::Ipv4RecvErr, &on)),
        (uapi::SOL_IP, uapi::IP_RECVORIGDSTADDR) => {
            nix(setsockopt(&socket, sockopt::Ipv4OrigDstAddr, &on))
        }
        (uapi::SOL_IP, uapi::IP_BIND_ADDRESS_NO_PORT) => {
            nix(setsockopt(&socket, sockopt::IpBindAddressNoPort, &on))
        }
        (uapi::SOL_IPV6, uapi::IPV6_TCLASS) => io(socket2.set_tclass_v6(unsigned(value)?)),
        (uapi::SOL_IPV6, uapi::IPV6_MTU_DISCOVER) => rustix(rustix_option::set_ipv6_mtu_discover(
            socket,
            rustix_option::Ipv6PathMtuDiscovery::from_raw(value as _),
        )),
        (uapi::SOL_IPV6, uapi::IPV6_TRANSPARENT) => io(socket2.set_ip_transparent_v6(on)),
        (uapi::SOL_IPV6, uapi::IPV6_FREEBIND) => io(socket2.set_freebind_v6(on)),
        (uapi::SOL_IPV6, uapi::IPV6_RECVHOPLIMIT) => io(socket2.set_recv_hoplimit_v6(on)),
        (uapi::SOL_IPV6, uapi::IPV6_RECVTCLASS) => io(socket2.set_recv_tclass_v6(on)),
        (uapi::SOL_IPV6, uapi::IPV6_RECVPKTINFO) => {
            nix(setsockopt(&socket, sockopt::Ipv6RecvPacketInfo, &on))
        }
        (uapi::SOL_IPV6, uapi::IPV6_RECVERR) => nix(setsockopt(&socket, sockopt::Ipv6RecvErr, &on)),
        (uapi::SOL_IPV6, uapi::IPV6_RECVORIGDSTADDR) => {
            nix(setsockopt(&socket, sockopt::Ipv6OrigDstAddr, &on))
        }
        (uapi::SOL_IPV6, uapi::IPV6_DONTFRAG) => {
            nix(setsockopt(&socket, sockopt::Ipv6DontFrag, &on))
        }
        (uapi::SOL_TCP, uapi::TCP_KEEPIDLE) => {
            rustix(rustix_option::set_tcp_keepidle(socket, seconds(value)?))
        }
        (uapi::SOL_TCP, uapi::TCP_KEEPINTVL) => {
            rustix(rustix_option::set_tcp_keepintvl(socket, seconds(value)?))
        }
        (uapi::SOL_TCP, uapi::TCP_KEEPCNT) => {
            rustix(rustix_option::set_tcp_keepcnt(socket, unsigned(value)?))
        }
        (uapi::SOL_TCP, uapi::TCP_USER_TIMEOUT) => rustix(rustix_option::set_tcp_user_timeout(
            socket,
            unsigned(value)?,
        )),
        (uapi::SOL_TCP, uapi::TCP_FASTOPEN_CONNECT) => {
            nix(setsockopt(&socket, sockopt::TcpFastOpenConnect, &on))
        }
        (wcontainers_socket_options::SOL_UDP, UDP_GRO) => {
            nix(setsockopt(&socket, sockopt::UdpGroSegment, &on))
        }
        _ => set_bytes(socket, level, name, value.as_bytes()),
    }
}

/// Reads an integer socket option through a typed call where socket2, rustix or nix has one, else as a listed option.
pub fn get_int(socket: BorrowedFd<'_>, level: u32, name: u32) -> Result<c_int, HostError> {
    let io = |error: std::io::Error| io_host_error(&error);
    let number = |value: usize| c_int::try_from(value).map_err(|_| from_errno(Errno::OVERFLOW));
    let socket2 = sock(&socket);
    match (level, name) {
        (uapi::SOL_SOCKET, uapi::SO_SNDBUF) => number(socket2.send_buffer_size().map_err(io)?),
        (uapi::SOL_SOCKET, uapi::SO_RCVBUF) => number(socket2.recv_buffer_size().map_err(io)?),
        (uapi::SOL_SOCKET, uapi::SO_INCOMING_CPU) => rustix_option::socket_incoming_cpu(socket)
            .map(|processor| processor as c_int)
            .map_err(from_errno),
        (uapi::SOL_IP, uapi::IP_RECVTTL) => getsockopt(&socket, sockopt::Ipv4RecvTtl)
            .map(c_int::from)
            .map_err(from_nix),
        (uapi::SOL_IP, uapi::IP_RECVERR) => getsockopt(&socket, sockopt::Ipv4RecvErr)
            .map(c_int::from)
            .map_err(from_nix),
        (uapi::SOL_IPV6, uapi::IPV6_RECVERR) => getsockopt(&socket, sockopt::Ipv6RecvErr)
            .map(c_int::from)
            .map_err(from_nix),
        (uapi::SOL_TCP, uapi::TCP_MAXSEG) => number(socket2.tcp_mss().map_err(io)? as usize),
        (uapi::SOL_TCP, uapi::TCP_QUICKACK) => socket2.tcp_quickack().map(c_int::from).map_err(io),
        (uapi::SOL_TCP, uapi::TCP_KEEPIDLE) => number(
            rustix_option::tcp_keepidle(socket)
                .map_err(from_errno)?
                .as_secs() as usize,
        ),
        (uapi::SOL_TCP, uapi::TCP_REPAIR) => getsockopt(&socket, sockopt::TcpRepair)
            .map(|value| value as c_int)
            .map_err(from_nix),
        _ => get_option(socket, level, name),
    }
}

pub use wcontainers_socket_options::{MPTCP_INFO, MultipathCounts, QueuedCount};

/// Reads `TCP_AO_GET_KEYS` for the one key a `struct tcp_ao_getsockopt` names, into it.
pub fn get_authentication_key<T: FromBytes + IntoBytes>(
    socket: BorrowedFd<'_>,
    query: &mut T,
) -> Result<(), HostError> {
    let query = <&mut [u8; AUTHENTICATION_KEY_QUERY]>::try_from(query.as_mut_bytes())
        .map_err(|_| from_errno(Errno::INVAL))?;
    LinuxSocketOptions
        .get_authentication_key(socket, query)
        .map_err(from_errno)
}

/// Writes the source filter of a group the socket joined on the interface, through `MCAST_MSFILTER`.
pub fn set_source_filter(
    socket: BorrowedFd<'_>,
    level: u32,
    interface: u32,
    group: StorageBytes,
    mode: u32,
    sources: &[StorageBytes],
) -> Result<(), HostError> {
    let level = match level {
        uapi::SOL_IP => MulticastLevel::Ipv4,
        uapi::SOL_IPV6 => MulticastLevel::Ipv6,
        _ => return Err(from_errno(Errno::NOPROTOOPT)),
    };
    let filter = SourceFilterOptions {
        level,
        interface,
        group,
        mode,
        sources,
    };
    LinuxSocketOptions
        .set_source_filter(socket, &filter)
        .map_err(from_errno)
}

/// Reads one of the kernel's counts of the socket's queues.
pub fn count_queued(socket: BorrowedFd<'_>, count: QueuedCount) -> Result<c_int, HostError> {
    LinuxSocketOptions
        .count_queued(socket, count)
        .map_err(from_errno)
}

/// Reads `MPTCP_FULL_INFO`: the connection's `mptcp_info` and each subflow's `mptcp_subflow_info` and `tcp_info`.
pub fn get_multipath_full_information<const CONNECTION: usize, Subflow, Information>(
    socket: BorrowedFd<'_>,
    connection: &mut [u8; CONNECTION],
    subflows: &mut [Subflow],
    information: &mut [Information],
) -> Result<MultipathCounts, HostError>
where
    Subflow: FromBytes,
    Information: FromBytes,
{
    LinuxSocketOptions
        .get_multipath_full_information(socket, connection, subflows, information)
        .map_err(from_errno)
}
