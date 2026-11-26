//! Linux's socket option and socket ioctl calls (setsockopt(2), getsockopt(2), ioctl(2) on a socket), one method per
//! option and request the host socket crates use that no vetted crate wraps safely.
//!
//! Made from `options.table` by `tests/generated.rs`; edit the table, not this file.
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
#[repr(C)]
#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
pub struct SocketStorage {
    _aligned: [usize; 0],
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

/// What `MPTCP_FULL_INFO` gave: the kernel's lengths and counts, the connection's `struct mptcp_info`, and each subflow's
/// `struct mptcp_subflow_info` and `struct tcp_info` in rooms of the sizes asked, as many as asked for.
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
    /// Sets `SO_RCVMARK` of level `SOL_SOCKET` to on or off, as socket(7) defines it.
    fn set_so_rcvmark(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `SO_RCVPRIORITY` of level `SOL_SOCKET` to on or off, as socket(7) defines it.
    fn set_so_rcvpriority(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `SO_ATTACH_REUSEPORT_CBPF` of level `SOL_SOCKET` to a classic BPF program, as socket(7) defines it.
    fn set_so_attach_reuseport_cbpf(
        &self,
        socket: BorrowedFd<'_>,
        program: &[libc::sock_filter],
    ) -> Result<(), HostError>;

    /// Sets `SO_DETACH_REUSEPORT_BPF` of level `SOL_SOCKET` to a number, as socket(7) defines it.
    fn set_so_detach_reuseport_bpf(
        &self,
        socket: BorrowedFd<'_>,
        value: c_int,
    ) -> Result<(), HostError>;

    /// Sets `SO_MAX_PACING_RATE` of level `SOL_SOCKET` to a number, as socket(7) defines it.
    fn set_so_max_pacing_rate(&self, socket: BorrowedFd<'_>, value: u64) -> Result<(), HostError>;

    /// Sets `SO_RCVLOWAT` of level `SOL_SOCKET` to a number, as socket(7) defines it.
    fn set_so_rcvlowat(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads `SO_RCVLOWAT` of level `SOL_SOCKET`, as socket(7) defines it.
    fn get_so_rcvlowat(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError>;

    /// Sets `SO_BUF_LOCK` of level `SOL_SOCKET` to a number, as socket(7) defines it.
    fn set_so_buf_lock(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Sets `SO_TXREHASH` of level `SOL_SOCKET` to a number, as socket(7) defines it.
    fn set_so_txrehash(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets `SO_CNX_ADVICE` of level `SOL_SOCKET` to on or off, as socket(7) defines it.
    fn set_so_cnx_advice(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `SO_WIFI_STATUS` of level `SOL_SOCKET` to on or off, as socket(7) defines it.
    fn set_so_wifi_status(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `SO_PEEK_OFF` of level `SOL_SOCKET` to a number, as socket(7) defines it.
    fn set_so_peek_off(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads `SO_MEMINFO` of level `SOL_SOCKET`, as socket(7) defines it.
    fn get_so_meminfo<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets `SO_NO_CHECK` of level `SOL_SOCKET` to on or off, as socket(7) defines it.
    fn set_so_no_check(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `SO_TIMESTAMPING` of level `SOL_SOCKET` to a value, as socket(7) defines it.
    fn set_so_timestamping(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::so_timestamping,
    ) -> Result<(), HostError>;

    /// Sets `IP_OPTIONS` of level `SOL_IP` to a byte string, as ip(7) defines it.
    fn set_ip_options(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError>;

    /// Sets `IP_UNICAST_IF` of level `SOL_IP` to a number, as ip(7) defines it.
    fn set_ip_unicast_if(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Sets `IP_LOCAL_PORT_RANGE` of level `SOL_IP` to a number, as ip(7) defines it.
    fn set_ip_local_port_range(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Sets `IP_ROUTER_ALERT` of level `SOL_IP` to on or off, as ip(7) defines it.
    fn set_ip_router_alert(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `IP_NODEFRAG` of level `SOL_IP` to on or off, as ip(7) defines it.
    fn set_ip_nodefrag(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `IP_RECVOPTS` of level `SOL_IP` to on or off, as ip(7) defines it.
    fn set_ip_recvopts(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `IP_RECVFRAGSIZE` of level `SOL_IP` to on or off, as ip(7) defines it.
    fn set_ip_recvfragsize(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `IP_MINTTL` of level `SOL_IP` to a number, as ip(7) defines it.
    fn set_ip_minttl(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads `IP_PKTOPTIONS` of level `SOL_IP`, as ip(7) defines it.
    fn get_ip_pktoptions<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets `MCAST_JOIN_GROUP` of level `SOL_IP` to a value, as ip(7) defines it.
    fn set_ip_mcast_join_group(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::group_req,
    ) -> Result<(), HostError>;

    /// Sets `MCAST_LEAVE_GROUP` of level `SOL_IP` to a value, as ip(7) defines it.
    fn set_ip_mcast_leave_group(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::group_req,
    ) -> Result<(), HostError>;

    /// Sets `MCAST_MSFILTER` of level `SOL_IP` to a source filter, as ip(7) defines it.
    fn set_ip_mcast_msfilter(
        &self,
        socket: BorrowedFd<'_>,
        filter: &GroupFilter<'_>,
    ) -> Result<(), HostError>;

    /// Sets `IPV6_HOPOPTS` of level `SOL_IPV6` to a byte string, as ipv6(7) defines it.
    fn set_ipv6_hopopts(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError>;

    /// Sets `IPV6_RTHDRDSTOPTS` of level `SOL_IPV6` to a byte string, as ipv6(7) defines it.
    fn set_ipv6_rthdrdstopts(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError>;

    /// Sets `IPV6_RTHDR` of level `SOL_IPV6` to a byte string, as ipv6(7) defines it.
    fn set_ipv6_rthdr(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError>;

    /// Sets `IPV6_DSTOPTS` of level `SOL_IPV6` to a byte string, as ipv6(7) defines it.
    fn set_ipv6_dstopts(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError>;

    /// Sets `IPV6_FLOWINFO_SEND` of level `SOL_IPV6` to on or off, as ipv6(7) defines it.
    fn set_ipv6_flowinfo_send(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `IPV6_AUTOFLOWLABEL` of level `SOL_IPV6` to on or off, as ipv6(7) defines it.
    fn set_ipv6_autoflowlabel(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `IPV6_FLOWLABEL_MGR` of level `SOL_IPV6` to a value the kernel may write into, as ipv6(7) defines it.
    fn set_ipv6_flowlabel_mgr(
        &self,
        socket: BorrowedFd<'_>,
        value: &mut uapi::in6_flowlabel_req,
    ) -> Result<(), HostError>;

    /// Sets `IPV6_ADDR_PREFERENCES` of level `SOL_IPV6` to a number, as ipv6(7) defines it.
    fn set_ipv6_addr_preferences(
        &self,
        socket: BorrowedFd<'_>,
        value: u32,
    ) -> Result<(), HostError>;

    /// Sets `IPV6_JOIN_ANYCAST` of level `SOL_IPV6` to a value, as ipv6(7) defines it.
    fn set_ipv6_join_anycast(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::ipv6_mreq,
    ) -> Result<(), HostError>;

    /// Sets `IPV6_LEAVE_ANYCAST` of level `SOL_IPV6` to a value, as ipv6(7) defines it.
    fn set_ipv6_leave_anycast(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::ipv6_mreq,
    ) -> Result<(), HostError>;

    /// Sets `IPV6_MTU` of level `SOL_IPV6` to a number, as ipv6(7) defines it.
    fn set_ipv6_mtu(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets `IPV6_UNICAST_IF` of level `SOL_IPV6` to a number, as ipv6(7) defines it.
    fn set_ipv6_unicast_if(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Sets `IPV6_ROUTER_ALERT` of level `SOL_IPV6` to a number, as ipv6(7) defines it.
    fn set_ipv6_router_alert(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets `IPV6_ROUTER_ALERT_ISOLATE` of level `SOL_IPV6` to on or off, as ipv6(7) defines it.
    fn set_ipv6_router_alert_isolate(
        &self,
        socket: BorrowedFd<'_>,
        on: bool,
    ) -> Result<(), HostError>;

    /// Sets `IPV6_CHECKSUM` of level `SOL_IPV6` to a number, as ipv6(7) defines it.
    fn set_ipv6_checksum(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets `IPV6_RECVHOPOPTS` of level `SOL_IPV6` to on or off, as ipv6(7) defines it.
    fn set_ipv6_recvhopopts(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `IPV6_RECVDSTOPTS` of level `SOL_IPV6` to on or off, as ipv6(7) defines it.
    fn set_ipv6_recvdstopts(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `IPV6_RECVRTHDR` of level `SOL_IPV6` to on or off, as ipv6(7) defines it.
    fn set_ipv6_recvrthdr(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `IPV6_FLOWINFO` of level `SOL_IPV6` to on or off, as ipv6(7) defines it.
    fn set_ipv6_flowinfo(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `IPV6_RECVFRAGSIZE` of level `SOL_IPV6` to on or off, as ipv6(7) defines it.
    fn set_ipv6_recvfragsize(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `IPV6_MINHOPCOUNT` of level `SOL_IPV6` to a number, as ipv6(7) defines it.
    fn set_ipv6_minhopcount(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads `IPV6_2292PKTOPTIONS` of level `SOL_IPV6`, as ipv6(7) defines it.
    fn get_ipv6_2292pktoptions<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets `MCAST_JOIN_GROUP` of level `SOL_IPV6` to a value, as ipv6(7) defines it.
    fn set_ipv6_mcast_join_group(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::group_req,
    ) -> Result<(), HostError>;

    /// Sets `MCAST_LEAVE_GROUP` of level `SOL_IPV6` to a value, as ipv6(7) defines it.
    fn set_ipv6_mcast_leave_group(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::group_req,
    ) -> Result<(), HostError>;

    /// Sets `MCAST_MSFILTER` of level `SOL_IPV6` to a source filter, as ipv6(7) defines it.
    fn set_ipv6_mcast_msfilter(
        &self,
        socket: BorrowedFd<'_>,
        filter: &GroupFilter<'_>,
    ) -> Result<(), HostError>;

    /// Sets `TCP_MD5SIG_EXT` of level `SOL_TCP` to a value, as tcp(7) defines it.
    fn set_tcp_md5sig_ext(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_md5sig,
    ) -> Result<(), HostError>;

    /// Sets `TCP_AO_ADD_KEY` of level `SOL_TCP` to a value, as tcp(7) defines it.
    fn set_tcp_ao_add_key(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_ao_add,
    ) -> Result<(), HostError>;

    /// Reads `TCP_AO_GET_KEYS` of level `SOL_TCP`, as tcp(7) defines it.
    fn get_tcp_ao_get_keys(
        &self,
        socket: BorrowedFd<'_>,
        query: &[u8; KEY_QUERY_LENGTH],
    ) -> Result<Written<KEY_QUERY_LENGTH>, HostError>;

    /// Sets `TCP_AO_DEL_KEY` of level `SOL_TCP` to a value, as tcp(7) defines it.
    fn set_tcp_ao_del_key(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_ao_del,
    ) -> Result<(), HostError>;

    /// Sets `TCP_AO_INFO` of level `SOL_TCP` to a value, as tcp(7) defines it.
    fn set_tcp_ao_info(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_ao_info_opt,
    ) -> Result<(), HostError>;

    /// Reads `TCP_AO_INFO` of level `SOL_TCP`, as tcp(7) defines it.
    fn get_tcp_ao_info<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets `TCP_AO_REPAIR` of level `SOL_TCP` to a value, as tcp(7) defines it.
    fn set_tcp_ao_repair(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_ao_repair,
    ) -> Result<(), HostError>;

    /// Reads `TCP_AO_REPAIR` of level `SOL_TCP`, as tcp(7) defines it.
    fn get_tcp_ao_repair<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets `TCP_WINDOW_CLAMP` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_window_clamp(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads `TCP_WINDOW_CLAMP` of level `SOL_TCP`, as tcp(7) defines it.
    fn get_tcp_window_clamp(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError>;

    /// Sets `TCP_SYNCNT` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_syncnt(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets `TCP_DEFER_ACCEPT` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_defer_accept(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets `TCP_LINGER2` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_linger2(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets `TCP_RTO_MIN_US` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_rto_min_us(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets `TCP_RTO_MAX_MS` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_rto_max_ms(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets `TCP_DELACK_MAX_US` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_delack_max_us(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets `TCP_TX_DELAY` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_tx_delay(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets `TCP_FASTOPEN_NO_COOKIE` of level `SOL_TCP` to on or off, as tcp(7) defines it.
    fn set_tcp_fastopen_no_cookie(&self, socket: BorrowedFd<'_>, on: bool)
    -> Result<(), HostError>;

    /// Sets `TCP_INQ` of level `SOL_TCP` to on or off, as tcp(7) defines it.
    fn set_tcp_inq(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `TCP_QUEUE_SEQ` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_queue_seq(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Reads `TCP_QUEUE_SEQ` of level `SOL_TCP`, as tcp(7) defines it.
    fn get_tcp_queue_seq(&self, socket: BorrowedFd<'_>) -> Result<u32, HostError>;

    /// Reads `TCP_SAVED_SYN` of level `SOL_TCP`, as tcp(7) defines it.
    fn get_tcp_saved_syn<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets `TCP_FASTOPEN` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_fastopen(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Sets `TCP_FASTOPEN_KEY` of level `SOL_TCP` to a byte string, as tcp(7) defines it.
    fn set_tcp_fastopen_key(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError>;

    /// Sets `TCP_SAVE_SYN` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_save_syn(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Sets `TCP_REPAIR_QUEUE` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_repair_queue(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads `TCP_REPAIR_QUEUE` of level `SOL_TCP`, as tcp(7) defines it.
    fn get_tcp_repair_queue(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError>;

    /// Sets `TCP_TIMESTAMP` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_timestamp(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError>;

    /// Reads `TCP_TIMESTAMP` of level `SOL_TCP`, as tcp(7) defines it.
    fn get_tcp_timestamp(&self, socket: BorrowedFd<'_>) -> Result<u32, HostError>;

    /// Sets `TCP_QUICKACK` of level `SOL_TCP` to a number, as tcp(7) defines it.
    fn set_tcp_quickack(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError>;

    /// Reads `TCP_INFO` of level `SOL_TCP`, as tcp(7) defines it.
    fn get_tcp_info<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Reads `TCP_CC_INFO` of level `SOL_TCP`, as tcp(7) defines it.
    fn get_tcp_cc_info<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets `TCP_REPAIR_WINDOW` of level `SOL_TCP` to a value, as tcp(7) defines it.
    fn set_tcp_repair_window(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_repair_window,
    ) -> Result<(), HostError>;

    /// Reads `TCP_REPAIR_WINDOW` of level `SOL_TCP`, as tcp(7) defines it.
    fn get_tcp_repair_window<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Sets `TCP_REPAIR_OPTIONS` of level `SOL_TCP` to a run of values, as tcp(7) defines it.
    fn set_tcp_repair_options(
        &self,
        socket: BorrowedFd<'_>,
        value: &[uapi::tcp_repair_opt],
    ) -> Result<(), HostError>;

    /// Reads `MPTCP_INFO` of level `SOL_MPTCP`, as mptcp(7) defines it.
    fn get_mptcp_info<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError>;

    /// Reads `MPTCP_FULL_INFO` of level `SOL_MPTCP`, as mptcp(7) defines it.
    fn get_mptcp_full_info<const SUBFLOW_ROOM: usize, const INFORMATION_ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
        subflows: usize,
    ) -> Result<MptcpFullInformation<SUBFLOW_ROOM, INFORMATION_ROOM>, HostError>;

    /// Sets `UDP_NO_CHECK6_RX` of level `SOL_UDP` to on or off, as udp(7) defines it.
    fn set_udp_no_check6_rx(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `UDP_NO_CHECK6_TX` of level `SOL_UDP` to on or off, as udp(7) defines it.
    fn set_udp_no_check6_tx(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError>;

    /// Sets `UDPLITE_RECV_CSCOV` of level `SOL_UDPLITE` to a number, as udplite(7) defines it.
    fn set_udplite_recv_cscov(&self, socket: BorrowedFd<'_>, value: c_int)
    -> Result<(), HostError>;

    /// Sets `UDPLITE_SEND_CSCOV` of level `SOL_UDPLITE` to a number, as udplite(7) defines it.
    fn set_udplite_send_cscov(&self, socket: BorrowedFd<'_>, value: c_int)
    -> Result<(), HostError>;

    /// Reads the int the `SIOCOUTQ` ioctl request gives for the socket, as tcp(7) defines it.
    fn get_siocoutq(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError>;

    /// Reads the int the `SIOCOUTQNSD` ioctl request gives for the socket, as tcp(7) defines it.
    fn get_siocoutqnsd(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError>;

    /// Reads the int the `SIOCATMARK` ioctl request gives for the socket, as tcp(7) defines it.
    fn get_siocatmark(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError>;
}
