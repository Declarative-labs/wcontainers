//! Linux's socket option and socket ioctl calls that no vetted crate wraps safely, one method each.
//!
//! The calls are setsockopt(2), getsockopt(2) and ioctl(2) on a socket. Made from `options.table` by the direct
//! implementation's `tests/generated.rs`; edit the table, not this file.
#![cfg(target_os = "linux")]

use std::ffi::c_int;
use std::mem::size_of;
use std::os::fd::BorrowedFd;

use linux_raw_sys::net as uapi;
use wcontainers_abi::HostError;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// The bytes a read wrote into a room of `ROOM` bytes, the rest zero, and how many it wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Written<const ROOM: usize> {
    pub bytes: [u8; ROOM],
    pub length: usize,
}

/// The length of the kernel's socket address storage, `struct __kernel_sockaddr_storage`.
pub const SOCKET_STORAGE_LENGTH: usize = size_of::<uapi::__kernel_sockaddr_storage>();

/// A socket address in the kernel's storage form, as bytes aligned as `struct __kernel_sockaddr_storage` is.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/socket.h
#[repr(C, align(8))]
#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct SocketStorage {
    pub bytes: [u8; SOCKET_STORAGE_LENGTH],
}

/// A multicast group's source filter, as `MCAST_MSFILTER` takes it in `struct group_filter`.
pub struct GroupFilter<'a> {
    pub interface: u32,
    pub group: SocketStorage,
    /// `MCAST_INCLUDE` or `MCAST_EXCLUDE`.
    pub mode: u32,
    pub sources: &'a [SocketStorage],
}

/// The length of `struct mptcp_info` as Linux 6.18 defines it, which `MPTCP_FULL_INFO` holds.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
pub const MPTCP_INFO_LENGTH: usize = 96;

/// The length of one `struct tcp_ao_getsockopt`, the query and answer of `TCP_AO_GET_KEYS`.
pub const KEY_QUERY_LENGTH: usize = size_of::<uapi::tcp_ao_getsockopt>();

/// What `MPTCP_FULL_INFO` gave: the kernel's lengths and counts, the connection's information and each subflow's.
///
/// Each subflow's `struct mptcp_subflow_info` and `struct tcp_info` sit in rooms of the sizes asked, as many as asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MptcpFullInformation<const SUBFLOW_ROOM: usize, const INFORMATION_ROOM: usize> {
    /// `size_tcpinfo_kernel`, the length of the kernel's `struct tcp_info`.
    pub tcp_information_length: u32,
    /// `size_sfinfo_kernel`, the length of the kernel's `struct mptcp_subflow_info`.
    pub subflow_information_length: u32,
    /// `num_subflows`, every subflow of the connection, also those past the rooms.
    pub subflows: u32,
    pub connection: [u8; MPTCP_INFO_LENGTH],
    pub subflow_information: Vec<[u8; SUBFLOW_ROOM]>,
    pub tcp_information: Vec<[u8; INFORMATION_ROOM]>,
}

#[allow(
    reviewable::explicit_shared_error::HostError,
    reason = "each method is one system call on the host's socket and fails only the ways that call does"
)]
pub trait LinuxSocketOptions {
    /// Sets [`SO_RCVMARK`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn set_so_rcvmark(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`SO_RCVPRIORITY`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn set_so_rcvpriority(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`SO_ATTACH_REUSEPORT_CBPF`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn set_so_attach_reuseport_cbpf(
        &self,
        socket: BorrowedFd<'_>,
        program: &[libc::sock_filter],
    ) -> Result<(), HostError>;

    /// Sets [`SO_DETACH_REUSEPORT_BPF`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn set_so_detach_reuseport_bpf(
        &self,
        socket: BorrowedFd<'_>,
        value: c_int,
    ) -> Result<(), HostError>;

    /// Sets [`SO_MAX_PACING_RATE`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn set_so_max_pacing_rate(&self, socket: BorrowedFd<'_>, value: u64) -> Result<(), HostError>;

    /// Sets [`SO_RCVLOWAT`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn set_so_rcvlowat(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads [`SO_RCVLOWAT`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn get_so_rcvlowat(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError>;

    /// Sets [`SO_BUF_LOCK`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn set_so_buf_lock(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Sets [`SO_TXREHASH`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn set_so_txrehash(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets [`SO_CNX_ADVICE`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn set_so_cnx_advice(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`SO_WIFI_STATUS`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn set_so_wifi_status(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`SO_PEEK_OFF`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn set_so_peek_off(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads [`SO_MEMINFO`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn get_so_meminfo<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets [`SO_NO_CHECK`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn set_so_no_check(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`SO_TIMESTAMPING`](https://man7.org/linux/man-pages/man7/socket.7.html).
    fn set_so_timestamping(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::so_timestamping,
    ) -> Result<(), HostError>;

    /// Sets [`IP_OPTIONS`](https://man7.org/linux/man-pages/man7/ip.7.html).
    fn set_ip_options(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError>;

    /// Sets [`IP_UNICAST_IF`](https://man7.org/linux/man-pages/man7/ip.7.html).
    fn set_ip_unicast_if(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Sets [`IP_LOCAL_PORT_RANGE`](https://man7.org/linux/man-pages/man7/ip.7.html).
    fn set_ip_local_port_range(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Sets [`IP_ROUTER_ALERT`](https://man7.org/linux/man-pages/man7/ip.7.html).
    fn set_ip_router_alert(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`IP_NODEFRAG`](https://man7.org/linux/man-pages/man7/ip.7.html).
    fn set_ip_nodefrag(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`IP_RECVOPTS`](https://man7.org/linux/man-pages/man7/ip.7.html).
    fn set_ip_recvopts(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`IP_RECVFRAGSIZE`](https://man7.org/linux/man-pages/man7/ip.7.html).
    fn set_ip_recvfragsize(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`IP_MINTTL`](https://man7.org/linux/man-pages/man7/ip.7.html).
    fn set_ip_minttl(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads [`IP_PKTOPTIONS`](https://man7.org/linux/man-pages/man7/ip.7.html).
    fn get_ip_pktoptions<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets [`MCAST_JOIN_GROUP`](https://man7.org/linux/man-pages/man7/ip.7.html).
    fn set_ip_mcast_join_group(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::group_req,
    ) -> Result<(), HostError>;

    /// Sets [`MCAST_LEAVE_GROUP`](https://man7.org/linux/man-pages/man7/ip.7.html).
    fn set_ip_mcast_leave_group(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::group_req,
    ) -> Result<(), HostError>;

    /// Sets [`MCAST_MSFILTER`](https://man7.org/linux/man-pages/man7/ip.7.html).
    fn set_ip_mcast_msfilter(
        &self,
        socket: BorrowedFd<'_>,
        filter: &GroupFilter<'_>,
    ) -> Result<(), HostError>;

    /// Sets [`IPV6_HOPOPTS`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_hopopts(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError>;

    /// Sets [`IPV6_RTHDRDSTOPTS`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_rthdrdstopts(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError>;

    /// Sets [`IPV6_RTHDR`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_rthdr(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError>;

    /// Sets [`IPV6_DSTOPTS`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_dstopts(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError>;

    /// Sets [`IPV6_FLOWINFO_SEND`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_flowinfo_send(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`IPV6_AUTOFLOWLABEL`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_autoflowlabel(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`IPV6_FLOWLABEL_MGR`](https://man7.org/linux/man-pages/man7/ipv6.7.html) to a value the kernel may write into.
    fn set_ipv6_flowlabel_mgr(
        &self,
        socket: BorrowedFd<'_>,
        value: &mut uapi::in6_flowlabel_req,
    ) -> Result<(), HostError>;

    /// Sets [`IPV6_ADDR_PREFERENCES`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_addr_preferences(
        &self,
        socket: BorrowedFd<'_>,
        value: u32,
    ) -> Result<(), HostError>;

    /// Sets [`IPV6_JOIN_ANYCAST`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_join_anycast(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::ipv6_mreq,
    ) -> Result<(), HostError>;

    /// Sets [`IPV6_LEAVE_ANYCAST`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_leave_anycast(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::ipv6_mreq,
    ) -> Result<(), HostError>;

    /// Sets [`IPV6_MTU`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_mtu(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets [`IPV6_UNICAST_IF`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_unicast_if(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Sets [`IPV6_ROUTER_ALERT`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_router_alert(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets [`IPV6_ROUTER_ALERT_ISOLATE`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_router_alert_isolate(
        &self,
        socket: BorrowedFd<'_>,
        on: bool,
    ) -> Result<(), HostError>;

    /// Sets [`IPV6_CHECKSUM`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_checksum(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets [`IPV6_RECVHOPOPTS`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_recvhopopts(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`IPV6_RECVDSTOPTS`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_recvdstopts(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`IPV6_RECVRTHDR`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_recvrthdr(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`IPV6_FLOWINFO`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_flowinfo(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`IPV6_RECVFRAGSIZE`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_recvfragsize(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`IPV6_MINHOPCOUNT`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_minhopcount(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads [`IPV6_2292PKTOPTIONS`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn get_ipv6_2292pktoptions<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets [`MCAST_JOIN_GROUP`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_mcast_join_group(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::group_req,
    ) -> Result<(), HostError>;

    /// Sets [`MCAST_LEAVE_GROUP`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_mcast_leave_group(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::group_req,
    ) -> Result<(), HostError>;

    /// Sets [`MCAST_MSFILTER`](https://man7.org/linux/man-pages/man7/ipv6.7.html).
    fn set_ipv6_mcast_msfilter(
        &self,
        socket: BorrowedFd<'_>,
        filter: &GroupFilter<'_>,
    ) -> Result<(), HostError>;

    /// Sets [`TCP_MD5SIG_EXT`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_md5sig_ext(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_md5sig,
    ) -> Result<(), HostError>;

    /// Sets [`TCP_AO_ADD_KEY`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_ao_add_key(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_ao_add,
    ) -> Result<(), HostError>;

    /// Reads [`TCP_AO_GET_KEYS`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn get_tcp_ao_get_keys(
        &self,
        socket: BorrowedFd<'_>,
        query: &[u8; KEY_QUERY_LENGTH],
    ) -> Result<Written<KEY_QUERY_LENGTH>, HostError>;

    /// Sets [`TCP_AO_DEL_KEY`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_ao_del_key(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_ao_del,
    ) -> Result<(), HostError>;

    /// Sets [`TCP_AO_INFO`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_ao_info(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_ao_info_opt,
    ) -> Result<(), HostError>;

    /// Reads [`TCP_AO_INFO`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn get_tcp_ao_info<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets [`TCP_AO_REPAIR`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_ao_repair(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_ao_repair,
    ) -> Result<(), HostError>;

    /// Reads [`TCP_AO_REPAIR`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn get_tcp_ao_repair<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets [`TCP_WINDOW_CLAMP`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_window_clamp(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads [`TCP_WINDOW_CLAMP`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn get_tcp_window_clamp(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError>;

    /// Sets [`TCP_SYNCNT`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_syncnt(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets [`TCP_DEFER_ACCEPT`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_defer_accept(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets [`TCP_LINGER2`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_linger2(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets [`TCP_RTO_MIN_US`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_rto_min_us(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets [`TCP_RTO_MAX_MS`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_rto_max_ms(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets [`TCP_DELACK_MAX_US`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_delack_max_us(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets [`TCP_TX_DELAY`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_tx_delay(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets [`TCP_FASTOPEN_NO_COOKIE`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_fastopen_no_cookie(&self, socket: BorrowedFd<'_>, on: bool)
    -> Result<(), HostError>;

    /// Sets [`TCP_INQ`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_inq(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`TCP_QUEUE_SEQ`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_queue_seq(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Reads [`TCP_QUEUE_SEQ`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn get_tcp_queue_seq(&self, socket: BorrowedFd<'_>) -> Result<u32, HostError>;

    /// Reads [`TCP_SAVED_SYN`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn get_tcp_saved_syn<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets [`TCP_FASTOPEN`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_fastopen(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Sets [`TCP_FASTOPEN_KEY`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_fastopen_key(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError>;

    /// Sets [`TCP_SAVE_SYN`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_save_syn(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets [`TCP_REPAIR_QUEUE`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_repair_queue(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads [`TCP_REPAIR_QUEUE`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn get_tcp_repair_queue(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError>;

    /// Sets [`TCP_TIMESTAMP`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_timestamp(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Reads [`TCP_TIMESTAMP`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn get_tcp_timestamp(&self, socket: BorrowedFd<'_>) -> Result<u32, HostError>;

    /// Sets [`TCP_QUICKACK`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_quickack(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads [`TCP_INFO`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn get_tcp_info<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Reads [`TCP_CC_INFO`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn get_tcp_cc_info<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets [`TCP_REPAIR_WINDOW`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_repair_window(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_repair_window,
    ) -> Result<(), HostError>;

    /// Reads [`TCP_REPAIR_WINDOW`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn get_tcp_repair_window<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets [`TCP_REPAIR_OPTIONS`](https://man7.org/linux/man-pages/man7/tcp.7.html).
    fn set_tcp_repair_options(
        &self,
        socket: BorrowedFd<'_>,
        value: &[uapi::tcp_repair_opt],
    ) -> Result<(), HostError>;

    /// Reads [`MPTCP_INFO`](https://man7.org/linux/man-pages/man7/mptcp.7.html).
    fn get_mptcp_info<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Reads [`MPTCP_FULL_INFO`](https://man7.org/linux/man-pages/man7/mptcp.7.html).
    fn get_mptcp_full_info<const SUBFLOW_ROOM: usize, const INFORMATION_ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
        subflows: usize,
    ) -> Result<MptcpFullInformation<SUBFLOW_ROOM, INFORMATION_ROOM>, HostError>;

    /// Sets [`UDP_NO_CHECK6_RX`](https://man7.org/linux/man-pages/man7/udp.7.html).
    fn set_udp_no_check6_rx(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`UDP_NO_CHECK6_TX`](https://man7.org/linux/man-pages/man7/udp.7.html).
    fn set_udp_no_check6_tx(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets [`UDPLITE_RECV_CSCOV`](https://man7.org/linux/man-pages/man7/udplite.7.html).
    fn set_udplite_recv_cscov(&self, socket: BorrowedFd<'_>, value: c_int)
    -> Result<(), HostError>;

    /// Sets [`UDPLITE_SEND_CSCOV`](https://man7.org/linux/man-pages/man7/udplite.7.html).
    fn set_udplite_send_cscov(&self, socket: BorrowedFd<'_>, value: c_int)
    -> Result<(), HostError>;

    /// Reads the int of the [`SIOCOUTQ`](https://man7.org/linux/man-pages/man7/tcp.7.html) request.
    fn get_siocoutq(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError>;

    /// Reads the int of the [`SIOCOUTQNSD`](https://man7.org/linux/man-pages/man7/tcp.7.html) request.
    fn get_siocoutqnsd(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError>;

    /// Reads the int of the [`SIOCATMARK`](https://man7.org/linux/man-pages/man7/tcp.7.html) request.
    fn get_siocatmark(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError>;
}
