//! Socket options of a number by level and name, each made through the typed call that covers it.
//!
//! That is socket2's, rustix's or nix's where one has it, else Linux's socket option calls in
//! wcontainers-linux-socket-options.
//!
//! An option no arm lists is refused with `ENOPROTOOPT`: every option the host crates set or read this way is listed.

use std::ffi::c_int;
use std::os::fd::BorrowedFd;
use std::time::Duration;

use linux_raw_sys::net as uapi;
use nix::sys::socket::{getsockopt, setsockopt, sockopt};
use rustix::io::Errno;
use rustix::net::sockopt as typed;
use wcontainers_abi::HostError;
use wcontainers_linux_socket_options::LinuxSocketOptions;
use wcontainers_linux_socket_options_direct::DirectLinuxSocketOptions;

use crate::datagram::sticky::UDP_LITE_RECEIVE_COVERAGE;
use crate::{host_error, io_host_error, sock};

/// The socket option calls of the running Linux, for the options no vetted crate covers.
pub const LINUX: DirectLinuxSocketOptions = DirectLinuxSocketOptions;

/// The level of UDP's own options, `SOL_UDP`.
const SOL_UDP: u32 = uapi::SOL_UDP;

/// The UDP option that coalesces received datagrams, which linux-raw-sys does not carry.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
const UDP_GRO: u32 = libc::UDP_GRO as u32;

/// The UDP option that takes IPv6 datagrams with a zero checksum, which linux-raw-sys does not carry.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
const UDP_NO_CHECK6_RX: u32 = libc::UDP_NO_CHECK6_RX as u32;

/// The level of UDP-Lite's own options, its protocol number.
///
/// https://man7.org/linux/man-pages/man7/udplite.7.html
const SOL_UDPLITE: u32 = uapi::IPPROTO_UDPLITE as u32;

fn refused() -> HostError {
    host_error(Errno::NOPROTOOPT)
}

fn from_nix(errno: nix::errno::Errno) -> HostError {
    host_error(Errno::from_raw_os_error(errno as c_int))
}

fn from_rustix(errno: Errno) -> HostError {
    host_error(errno)
}

fn from_io(error: std::io::Error) -> HostError {
    io_host_error(&error)
}

/// The value as the unsigned number a typed call takes, `EINVAL` for a negative one, as Linux answers those options.
fn unsigned(value: c_int) -> Result<u32, HostError> {
    u32::try_from(value).map_err(|_| host_error(Errno::INVAL))
}

fn seconds(value: c_int) -> Result<Duration, HostError> {
    unsigned(value).map(|seconds| Duration::from_secs(seconds.into()))
}

/// Sets an option that is on or off.
pub fn set_flag(socket: BorrowedFd<'_>, level: u32, name: u32, on: bool) -> Result<(), HostError> {
    let nix = |result: nix::Result<()>| result.map_err(from_nix);
    let io = |result: std::io::Result<()>| result.map_err(from_io);
    let socket2 = sock(&socket);
    match (level, name) {
        (uapi::SOL_SOCKET, uapi::SO_DONTROUTE) => nix(setsockopt(&socket, sockopt::DontRoute, &on)),
        (uapi::SOL_SOCKET, uapi::SO_RXQ_OVFL) => {
            nix(setsockopt(&socket, sockopt::RxqOvfl, &c_int::from(on)))
        }
        (uapi::SOL_SOCKET, uapi::SO_RCVMARK) => LINUX.set_so_rcvmark(socket, on),
        (uapi::SOL_SOCKET, uapi::SO_RCVPRIORITY) => LINUX.set_so_rcvpriority(socket, on),
        (uapi::SOL_SOCKET, uapi::SO_CNX_ADVICE) => LINUX.set_so_cnx_advice(socket, on),
        (uapi::SOL_SOCKET, uapi::SO_WIFI_STATUS) => LINUX.set_so_wifi_status(socket, on),
        (uapi::SOL_IP, uapi::IP_PKTINFO) => nix(setsockopt(&socket, sockopt::Ipv4PacketInfo, &on)),
        (uapi::SOL_IP, uapi::IP_RECVTTL) => nix(setsockopt(&socket, sockopt::Ipv4RecvTtl, &on)),
        (uapi::SOL_IP, uapi::IP_RECVERR) => nix(setsockopt(&socket, sockopt::Ipv4RecvErr, &on)),
        (uapi::SOL_IP, uapi::IP_RECVTOS) => io(socket2.set_recv_tos_v4(on)),
        (uapi::SOL_IP, uapi::IP_RECVORIGDSTADDR) => {
            nix(setsockopt(&socket, sockopt::Ipv4OrigDstAddr, &on))
        }
        (uapi::SOL_IP, uapi::IP_BIND_ADDRESS_NO_PORT) => {
            nix(setsockopt(&socket, sockopt::IpBindAddressNoPort, &on))
        }
        (uapi::SOL_IP, uapi::IP_RECVOPTS) => LINUX.set_ip_recvopts(socket, on),
        (uapi::SOL_IP, uapi::IP_RECVFRAGSIZE) => LINUX.set_ip_recvfragsize(socket, on),
        (uapi::SOL_IP, uapi::IP_ROUTER_ALERT) => LINUX.set_ip_router_alert(socket, on),
        (uapi::SOL_IP, uapi::IP_NODEFRAG) => LINUX.set_ip_nodefrag(socket, on),
        (uapi::SOL_IPV6, uapi::IPV6_RECVPKTINFO) => {
            nix(setsockopt(&socket, sockopt::Ipv6RecvPacketInfo, &on))
        }
        (uapi::SOL_IPV6, uapi::IPV6_RECVERR) => nix(setsockopt(&socket, sockopt::Ipv6RecvErr, &on)),
        (uapi::SOL_IPV6, uapi::IPV6_RECVORIGDSTADDR) => {
            nix(setsockopt(&socket, sockopt::Ipv6OrigDstAddr, &on))
        }
        (uapi::SOL_IPV6, uapi::IPV6_RECVHOPLIMIT) => io(socket2.set_recv_hoplimit_v6(on)),
        (uapi::SOL_IPV6, uapi::IPV6_RECVTCLASS) => io(socket2.set_recv_tclass_v6(on)),
        (uapi::SOL_IPV6, uapi::IPV6_RECVHOPOPTS) => LINUX.set_ipv6_recvhopopts(socket, on),
        (uapi::SOL_IPV6, uapi::IPV6_RECVDSTOPTS) => LINUX.set_ipv6_recvdstopts(socket, on),
        (uapi::SOL_IPV6, uapi::IPV6_RECVRTHDR) => LINUX.set_ipv6_recvrthdr(socket, on),
        (uapi::SOL_IPV6, uapi::IPV6_FLOWINFO) => LINUX.set_ipv6_flowinfo(socket, on),
        (uapi::SOL_IPV6, uapi::IPV6_RECVFRAGSIZE) => LINUX.set_ipv6_recvfragsize(socket, on),
        (uapi::SOL_IPV6, uapi::IPV6_FLOWINFO_SEND) => LINUX.set_ipv6_flowinfo_send(socket, on),
        (uapi::SOL_IPV6, uapi::IPV6_AUTOFLOWLABEL) => LINUX.set_ipv6_autoflowlabel(socket, on),
        (uapi::SOL_IPV6, uapi::IPV6_ROUTER_ALERT_ISOLATE) => {
            LINUX.set_ipv6_router_alert_isolate(socket, on)
        }
        (uapi::SOL_TCP, uapi::TCP_FASTOPEN_CONNECT) => {
            nix(setsockopt(&socket, sockopt::TcpFastOpenConnect, &on))
        }
        (uapi::SOL_TCP, uapi::TCP_FASTOPEN_NO_COOKIE) => {
            LINUX.set_tcp_fastopen_no_cookie(socket, on)
        }
        (uapi::SOL_TCP, uapi::TCP_INQ) => LINUX.set_tcp_inq(socket, on),
        (SOL_UDP, UDP_GRO) => nix(setsockopt(&socket, sockopt::UdpGroSegment, &on)),
        (SOL_UDP, UDP_NO_CHECK6_RX) => LINUX.set_udp_no_check6_rx(socket, on),
        _ => Err(refused()),
    }
}

/// Sets an option that is a number.
pub fn set_int(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    value: c_int,
) -> Result<(), HostError> {
    let rustix = |result: rustix::io::Result<()>| result.map_err(from_rustix);
    let nix = |result: nix::Result<()>| result.map_err(from_nix);
    match (level, name) {
        // Linux's -1 for no processor is the same 32 bits as the unsigned number rustix takes.
        (uapi::SOL_SOCKET, uapi::SO_INCOMING_CPU) => rustix(typed::set_socket_incoming_cpu(
            socket,
            value.cast_unsigned(),
        )),
        (uapi::SOL_SOCKET, uapi::SO_DETACH_FILTER) => {
            sock(&socket).detach_filter().map_err(from_io)
        }
        (uapi::SOL_SOCKET, uapi::SO_DETACH_REUSEPORT_BPF) => {
            LINUX.set_so_detach_reuseport_bpf(socket, value)
        }
        (uapi::SOL_SOCKET, uapi::SO_RCVLOWAT) => LINUX.set_so_rcvlowat(socket, value),
        (uapi::SOL_SOCKET, uapi::SO_TXREHASH) => LINUX.set_so_txrehash(socket, value),
        (uapi::SOL_SOCKET, uapi::SO_PEEK_OFF) => LINUX.set_so_peek_off(socket, value),
        (uapi::SOL_IP, uapi::IP_TTL) => nix(setsockopt(&socket, sockopt::Ipv4Ttl, &value)),
        (uapi::SOL_IP, uapi::IP_TOS) => sock(&socket)
            .set_tos_v4(value.cast_unsigned())
            .map_err(from_io),
        (uapi::SOL_IP, uapi::IP_MTU_DISCOVER) => rustix(typed::set_ip_mtu_discover(
            socket,
            typed::Ipv4PathMtuDiscovery::from_raw(value),
        )),
        (uapi::SOL_IP, uapi::IP_MINTTL) => LINUX.set_ip_minttl(socket, value),
        (uapi::SOL_IPV6, uapi::IPV6_UNICAST_HOPS) => {
            nix(setsockopt(&socket, sockopt::Ipv6Ttl, &value))
        }
        (uapi::SOL_IPV6, uapi::IPV6_TCLASS) => sock(&socket)
            .set_tclass_v6(value.cast_unsigned())
            .map_err(from_io),
        (uapi::SOL_IPV6, uapi::IPV6_MTU_DISCOVER) => rustix(typed::set_ipv6_mtu_discover(
            socket,
            typed::Ipv6PathMtuDiscovery::from_raw(value),
        )),
        (uapi::SOL_IPV6, uapi::IPV6_ROUTER_ALERT) => LINUX.set_ipv6_router_alert(socket, value),
        (uapi::SOL_IPV6, uapi::IPV6_CHECKSUM) => LINUX.set_ipv6_checksum(socket, value),
        (uapi::SOL_IPV6, uapi::IPV6_MINHOPCOUNT) => LINUX.set_ipv6_minhopcount(socket, value),
        (uapi::SOL_TCP, uapi::TCP_KEEPIDLE) => {
            rustix(typed::set_tcp_keepidle(socket, seconds(value)?))
        }
        (uapi::SOL_TCP, uapi::TCP_KEEPINTVL) => {
            rustix(typed::set_tcp_keepintvl(socket, seconds(value)?))
        }
        (uapi::SOL_TCP, uapi::TCP_KEEPCNT) => {
            rustix(typed::set_tcp_keepcnt(socket, unsigned(value)?))
        }
        (uapi::SOL_TCP, uapi::TCP_USER_TIMEOUT) => {
            rustix(typed::set_tcp_user_timeout(socket, unsigned(value)?))
        }
        // TCP_REPAIR_OFF_NO_WP's -1 is the same 32 bits as the unsigned number nix takes.
        (uapi::SOL_TCP, uapi::TCP_REPAIR) => nix(setsockopt(
            &socket,
            sockopt::TcpRepair,
            &value.cast_unsigned(),
        )),
        (uapi::SOL_TCP, uapi::TCP_WINDOW_CLAMP) => LINUX.set_tcp_window_clamp(socket, value),
        (uapi::SOL_TCP, uapi::TCP_SYNCNT) => LINUX.set_tcp_syncnt(socket, value),
        (uapi::SOL_TCP, uapi::TCP_DEFER_ACCEPT) => LINUX.set_tcp_defer_accept(socket, value),
        (uapi::SOL_TCP, uapi::TCP_LINGER2) => LINUX.set_tcp_linger2(socket, value),
        (uapi::SOL_TCP, uapi::TCP_RTO_MIN_US) => LINUX.set_tcp_rto_min_us(socket, value),
        (uapi::SOL_TCP, uapi::TCP_RTO_MAX_MS) => LINUX.set_tcp_rto_max_ms(socket, value),
        (uapi::SOL_TCP, uapi::TCP_DELACK_MAX_US) => LINUX.set_tcp_delack_max_us(socket, value),
        (uapi::SOL_TCP, uapi::TCP_TX_DELAY) => LINUX.set_tcp_tx_delay(socket, value),
        (uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE) => LINUX.set_tcp_repair_queue(socket, value),
        (uapi::SOL_TCP, uapi::TCP_SAVE_SYN) => LINUX.set_tcp_save_syn(socket, value),
        (uapi::SOL_TCP, uapi::TCP_QUICKACK) => LINUX.set_tcp_quickack(socket, value),
        (SOL_UDPLITE, UDP_LITE_RECEIVE_COVERAGE) => LINUX.set_udplite_recv_cscov(socket, value),
        _ => Err(refused()),
    }
}

/// Reads an option that is a number, or on or off as 1 or 0.
pub fn get_int(socket: BorrowedFd<'_>, level: u32, name: u32) -> Result<c_int, HostError> {
    let nix_flag = |result: nix::Result<bool>| result.map(c_int::from).map_err(from_nix);
    let number = |value: usize| c_int::try_from(value).map_err(|_| host_error(Errno::OVERFLOW));
    let socket2 = sock(&socket);
    match (level, name) {
        (uapi::SOL_SOCKET, uapi::SO_SNDBUF) => number(socket2.send_buffer_size().map_err(from_io)?),
        (uapi::SOL_SOCKET, uapi::SO_RCVBUF) => number(socket2.recv_buffer_size().map_err(from_io)?),
        // Linux's -1 for no processor comes back as the same 32 bits.
        (uapi::SOL_SOCKET, uapi::SO_INCOMING_CPU) => typed::socket_incoming_cpu(socket)
            .map(u32::cast_signed)
            .map_err(from_rustix),
        (uapi::SOL_SOCKET, uapi::SO_RCVLOWAT) => LINUX.get_so_rcvlowat(socket),
        (uapi::SOL_IP, uapi::IP_RECVTTL) => nix_flag(getsockopt(&socket, sockopt::Ipv4RecvTtl)),
        (uapi::SOL_IP, uapi::IP_RECVERR) => nix_flag(getsockopt(&socket, sockopt::Ipv4RecvErr)),
        (uapi::SOL_IPV6, uapi::IPV6_RECVERR) => nix_flag(getsockopt(&socket, sockopt::Ipv6RecvErr)),
        (uapi::SOL_TCP, uapi::TCP_MAXSEG) => socket2
            .tcp_mss()
            .map_err(from_io)
            .and_then(|size| c_int::try_from(size).map_err(|_| host_error(Errno::OVERFLOW))),
        (uapi::SOL_TCP, uapi::TCP_QUICKACK) => {
            socket2.tcp_quickack().map(c_int::from).map_err(from_io)
        }
        (uapi::SOL_TCP, uapi::TCP_KEEPIDLE) => typed::tcp_keepidle(socket)
            .map_err(from_rustix)
            .and_then(|idle| {
                c_int::try_from(idle.as_secs()).map_err(|_| host_error(Errno::OVERFLOW))
            }),
        (uapi::SOL_TCP, uapi::TCP_REPAIR) => getsockopt(&socket, sockopt::TcpRepair)
            .map(u32::cast_signed)
            .map_err(from_nix),
        (uapi::SOL_TCP, uapi::TCP_WINDOW_CLAMP) => LINUX.get_tcp_window_clamp(socket),
        (uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE) => LINUX.get_tcp_repair_queue(socket),
        _ => Err(refused()),
    }
}
