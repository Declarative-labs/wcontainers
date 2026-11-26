//! The trait's methods as the calls they name, made from the interface's `options.table`.

use std::ffi::c_int;
use std::os::fd::BorrowedFd;

use linux_raw_sys::net as uapi;
use wcontainers_abi::HostError;
use wcontainers_linux_socket_options::{
    GroupFilter, KEY_QUERY_LENGTH, LinuxSocketOptions, MptcpFullInformation, Written,
};

use crate::{
    DirectLinuxSocketOptions, Plain, get_full_info, get_keys, get_value, get_written, ioctl_int,
    local, set_group_filter, set_program, set_value, set_value_written,
};

impl Plain for uapi::group_req {}
impl Plain for uapi::in6_flowlabel_req {}
impl Plain for uapi::ipv6_mreq {}
impl Plain for uapi::so_timestamping {}
impl Plain for uapi::tcp_ao_add {}
impl Plain for uapi::tcp_ao_del {}
impl Plain for uapi::tcp_ao_info_opt {}
impl Plain for uapi::tcp_ao_repair {}
impl Plain for uapi::tcp_md5sig {}
impl Plain for uapi::tcp_repair_opt {}
impl Plain for uapi::tcp_repair_window {}

impl LinuxSocketOptions for DirectLinuxSocketOptions {
    fn set_so_rcvmark(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_SOCKET, uapi::SO_RCVMARK, &c_int::from(on))
    }

    fn set_so_rcvpriority(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_SOCKET,
            uapi::SO_RCVPRIORITY,
            &c_int::from(on),
        )
    }

    fn set_so_attach_reuseport_cbpf(
        &self,
        socket: BorrowedFd<'_>,
        program: &[libc::sock_filter],
    ) -> Result<(), HostError> {
        set_program(
            socket,
            uapi::SOL_SOCKET,
            uapi::SO_ATTACH_REUSEPORT_CBPF,
            program,
        )
    }

    fn set_so_detach_reuseport_bpf(
        &self,
        socket: BorrowedFd<'_>,
        value: c_int,
    ) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_SOCKET,
            uapi::SO_DETACH_REUSEPORT_BPF,
            &value,
        )
    }

    fn set_so_max_pacing_rate(&self, socket: BorrowedFd<'_>, value: u64) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_SOCKET, uapi::SO_MAX_PACING_RATE, &value)
    }

    fn set_so_rcvlowat(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_SOCKET, uapi::SO_RCVLOWAT, &value)
    }

    fn get_so_rcvlowat(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError> {
        get_value::<c_int>(socket, uapi::SOL_SOCKET, uapi::SO_RCVLOWAT)
    }

    fn set_so_buf_lock(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_SOCKET, uapi::SO_BUF_LOCK, &value)
    }

    fn set_so_txrehash(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_SOCKET, uapi::SO_TXREHASH, &value)
    }

    fn set_so_cnx_advice(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_SOCKET,
            uapi::SO_CNX_ADVICE,
            &c_int::from(on),
        )
    }

    fn set_so_wifi_status(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_SOCKET,
            uapi::SO_WIFI_STATUS,
            &c_int::from(on),
        )
    }

    fn set_so_peek_off(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_SOCKET, uapi::SO_PEEK_OFF, &value)
    }

    fn get_so_meminfo<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError> {
        get_written(socket, uapi::SOL_SOCKET, uapi::SO_MEMINFO)
    }

    fn set_so_no_check(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_SOCKET,
            uapi::SO_NO_CHECK,
            &c_int::from(on),
        )
    }

    fn set_so_timestamping(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::so_timestamping,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_SOCKET, uapi::SO_TIMESTAMPING, value)
    }

    fn set_ip_options(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IP, uapi::IP_OPTIONS, value)
    }

    fn set_ip_unicast_if(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IP, uapi::IP_UNICAST_IF, &value)
    }

    fn set_ip_local_port_range(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IP, uapi::IP_LOCAL_PORT_RANGE, &value)
    }

    fn set_ip_router_alert(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_IP,
            uapi::IP_ROUTER_ALERT,
            &c_int::from(on),
        )
    }

    fn set_ip_nodefrag(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IP, uapi::IP_NODEFRAG, &c_int::from(on))
    }

    fn set_ip_recvopts(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IP, uapi::IP_RECVOPTS, &c_int::from(on))
    }

    fn set_ip_recvfragsize(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_IP,
            uapi::IP_RECVFRAGSIZE,
            &c_int::from(on),
        )
    }

    fn set_ip_minttl(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IP, uapi::IP_MINTTL, &value)
    }

    fn get_ip_pktoptions<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError> {
        get_written(socket, uapi::SOL_IP, uapi::IP_PKTOPTIONS)
    }

    fn set_ip_mcast_join_group(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::group_req,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IP, uapi::MCAST_JOIN_GROUP, value)
    }

    fn set_ip_mcast_leave_group(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::group_req,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IP, uapi::MCAST_LEAVE_GROUP, value)
    }

    fn set_ip_mcast_msfilter(
        &self,
        socket: BorrowedFd<'_>,
        filter: &GroupFilter<'_>,
    ) -> Result<(), HostError> {
        set_group_filter(socket, uapi::SOL_IP, uapi::MCAST_MSFILTER, filter)
    }

    fn set_ipv6_hopopts(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::IPV6_HOPOPTS, value)
    }

    fn set_ipv6_rthdrdstopts(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::IPV6_RTHDRDSTOPTS, value)
    }

    fn set_ipv6_rthdr(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::IPV6_RTHDR, value)
    }

    fn set_ipv6_dstopts(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::IPV6_DSTOPTS, value)
    }

    fn set_ipv6_flowinfo_send(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_FLOWINFO_SEND,
            &c_int::from(on),
        )
    }

    fn set_ipv6_autoflowlabel(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_AUTOFLOWLABEL,
            &c_int::from(on),
        )
    }

    fn set_ipv6_flowlabel_mgr(
        &self,
        socket: BorrowedFd<'_>,
        value: &mut uapi::in6_flowlabel_req,
    ) -> Result<(), HostError> {
        set_value_written(socket, uapi::SOL_IPV6, uapi::IPV6_FLOWLABEL_MGR, value)
    }

    fn set_ipv6_addr_preferences(
        &self,
        socket: BorrowedFd<'_>,
        value: u32,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::IPV6_ADDR_PREFERENCES, &value)
    }

    fn set_ipv6_join_anycast(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::ipv6_mreq,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::IPV6_JOIN_ANYCAST, value)
    }

    fn set_ipv6_leave_anycast(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::ipv6_mreq,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::IPV6_LEAVE_ANYCAST, value)
    }

    fn set_ipv6_mtu(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::IPV6_MTU, &value)
    }

    fn set_ipv6_unicast_if(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::IPV6_UNICAST_IF, &value)
    }

    fn set_ipv6_router_alert(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::IPV6_ROUTER_ALERT, &value)
    }

    fn set_ipv6_router_alert_isolate(
        &self,
        socket: BorrowedFd<'_>,
        on: bool,
    ) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_ROUTER_ALERT_ISOLATE,
            &c_int::from(on),
        )
    }

    fn set_ipv6_checksum(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::IPV6_CHECKSUM, &value)
    }

    fn set_ipv6_recvhopopts(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_RECVHOPOPTS,
            &c_int::from(on),
        )
    }

    fn set_ipv6_recvdstopts(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_RECVDSTOPTS,
            &c_int::from(on),
        )
    }

    fn set_ipv6_recvrthdr(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_RECVRTHDR,
            &c_int::from(on),
        )
    }

    fn set_ipv6_flowinfo(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_FLOWINFO,
            &c_int::from(on),
        )
    }

    fn set_ipv6_recvfragsize(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_RECVFRAGSIZE,
            &c_int::from(on),
        )
    }

    fn set_ipv6_minhopcount(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::IPV6_MINHOPCOUNT, &value)
    }

    fn get_ipv6_2292pktoptions<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError> {
        get_written(socket, uapi::SOL_IPV6, uapi::IPV6_2292PKTOPTIONS)
    }

    fn set_ipv6_mcast_join_group(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::group_req,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::MCAST_JOIN_GROUP, value)
    }

    fn set_ipv6_mcast_leave_group(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::group_req,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_IPV6, uapi::MCAST_LEAVE_GROUP, value)
    }

    fn set_ipv6_mcast_msfilter(
        &self,
        socket: BorrowedFd<'_>,
        filter: &GroupFilter<'_>,
    ) -> Result<(), HostError> {
        set_group_filter(socket, uapi::SOL_IPV6, uapi::MCAST_MSFILTER, filter)
    }

    fn set_tcp_md5sig_ext(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_md5sig,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_MD5SIG_EXT, value)
    }

    fn set_tcp_ao_add_key(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_ao_add,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_AO_ADD_KEY, value)
    }

    fn get_tcp_ao_get_keys(
        &self,
        socket: BorrowedFd<'_>,
        query: &[u8; KEY_QUERY_LENGTH],
    ) -> Result<Written<KEY_QUERY_LENGTH>, HostError> {
        get_keys(socket, uapi::SOL_TCP, uapi::TCP_AO_GET_KEYS, query)
    }

    fn set_tcp_ao_del_key(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_ao_del,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_AO_DEL_KEY, value)
    }

    fn set_tcp_ao_info(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_ao_info_opt,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_AO_INFO, value)
    }

    fn get_tcp_ao_info<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError> {
        get_written(socket, uapi::SOL_TCP, uapi::TCP_AO_INFO)
    }

    fn set_tcp_ao_repair(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_ao_repair,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_AO_REPAIR, value)
    }

    fn get_tcp_ao_repair<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError> {
        get_written(socket, uapi::SOL_TCP, uapi::TCP_AO_REPAIR)
    }

    fn set_tcp_window_clamp(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_WINDOW_CLAMP, &value)
    }

    fn get_tcp_window_clamp(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError> {
        get_value::<c_int>(socket, uapi::SOL_TCP, uapi::TCP_WINDOW_CLAMP)
    }

    fn set_tcp_syncnt(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_SYNCNT, &value)
    }

    fn set_tcp_defer_accept(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_DEFER_ACCEPT, &value)
    }

    fn set_tcp_linger2(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_LINGER2, &value)
    }

    fn set_tcp_rto_min_us(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_RTO_MIN_US, &value)
    }

    fn set_tcp_rto_max_ms(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_RTO_MAX_MS, &value)
    }

    fn set_tcp_delack_max_us(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_DELACK_MAX_US, &value)
    }

    fn set_tcp_tx_delay(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_TX_DELAY, &value)
    }

    fn set_tcp_fastopen_no_cookie(
        &self,
        socket: BorrowedFd<'_>,
        on: bool,
    ) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_TCP,
            uapi::TCP_FASTOPEN_NO_COOKIE,
            &c_int::from(on),
        )
    }

    fn set_tcp_inq(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_INQ, &c_int::from(on))
    }

    fn set_tcp_queue_seq(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_QUEUE_SEQ, &value)
    }

    fn get_tcp_queue_seq(&self, socket: BorrowedFd<'_>) -> Result<u32, HostError> {
        get_value::<u32>(socket, uapi::SOL_TCP, uapi::TCP_QUEUE_SEQ)
    }

    fn get_tcp_saved_syn<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError> {
        get_written(socket, uapi::SOL_TCP, uapi::TCP_SAVED_SYN)
    }

    fn set_tcp_fastopen(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_FASTOPEN, &value)
    }

    fn set_tcp_fastopen_key(&self, socket: BorrowedFd<'_>, value: &[u8]) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_FASTOPEN_KEY, value)
    }

    fn set_tcp_save_syn(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_SAVE_SYN, &value)
    }

    fn set_tcp_repair_queue(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE, &value)
    }

    fn get_tcp_repair_queue(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError> {
        get_value::<c_int>(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE)
    }

    fn set_tcp_timestamp(&self, socket: BorrowedFd<'_>, value: u32) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP, &value)
    }

    fn get_tcp_timestamp(&self, socket: BorrowedFd<'_>) -> Result<u32, HostError> {
        get_value::<u32>(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP)
    }

    fn set_tcp_quickack(&self, socket: BorrowedFd<'_>, value: c_int) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_QUICKACK, &value)
    }

    fn get_tcp_info<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError> {
        get_written(socket, uapi::SOL_TCP, uapi::TCP_INFO)
    }

    fn get_tcp_cc_info<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError> {
        get_written(socket, uapi::SOL_TCP, uapi::TCP_CC_INFO)
    }

    fn set_tcp_repair_window(
        &self,
        socket: BorrowedFd<'_>,
        value: &uapi::tcp_repair_window,
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_WINDOW, value)
    }

    fn get_tcp_repair_window<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError> {
        get_written(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_WINDOW)
    }

    fn set_tcp_repair_options(
        &self,
        socket: BorrowedFd<'_>,
        value: &[uapi::tcp_repair_opt],
    ) -> Result<(), HostError> {
        set_value(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_OPTIONS, value)
    }

    fn get_mptcp_info<const ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
    ) -> Result<Written<ROOM>, HostError> {
        get_written(socket, uapi::SOL_MPTCP, local::MPTCP_INFO)
    }

    fn get_mptcp_full_info<const SUBFLOW_ROOM: usize, const INFORMATION_ROOM: usize>(
        &self,
        socket: BorrowedFd<'_>,
        subflows: usize,
    ) -> Result<MptcpFullInformation<SUBFLOW_ROOM, INFORMATION_ROOM>, HostError> {
        get_full_info(socket, uapi::SOL_MPTCP, local::MPTCP_FULL_INFO, subflows)
    }

    fn set_udp_no_check6_rx(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_UDP,
            local::UDP_NO_CHECK6_RX,
            &c_int::from(on),
        )
    }

    fn set_udp_no_check6_tx(&self, socket: BorrowedFd<'_>, on: bool) -> Result<(), HostError> {
        set_value(
            socket,
            uapi::SOL_UDP,
            local::UDP_NO_CHECK6_TX,
            &c_int::from(on),
        )
    }

    fn set_udplite_recv_cscov(
        &self,
        socket: BorrowedFd<'_>,
        value: c_int,
    ) -> Result<(), HostError> {
        set_value(
            socket,
            local::SOL_UDPLITE,
            local::UDPLITE_RECV_CSCOV,
            &value,
        )
    }

    fn set_udplite_send_cscov(
        &self,
        socket: BorrowedFd<'_>,
        value: c_int,
    ) -> Result<(), HostError> {
        set_value(
            socket,
            local::SOL_UDPLITE,
            local::UDPLITE_SEND_CSCOV,
            &value,
        )
    }

    fn get_siocoutq(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError> {
        ioctl_int(socket, local::SIOCOUTQ)
    }

    fn get_siocoutqnsd(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError> {
        ioctl_int(socket, local::SIOCOUTQNSD)
    }

    fn get_siocatmark(&self, socket: BorrowedFd<'_>) -> Result<c_int, HostError> {
        ioctl_int(socket, local::SIOCATMARK)
    }
}

/// The rows of `options.table`, which the probe calls each of.
#[cfg(test)]
pub(crate) const ROWS: [crate::probe::Row; 96] = [
    crate::probe::Row {
        method: "set_so_rcvmark",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_RCVMARK,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_so_rcvpriority",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_RCVPRIORITY,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_so_attach_reuseport_cbpf",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_ATTACH_REUSEPORT_CBPF,
        value: "program",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_so_detach_reuseport_bpf",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_DETACH_REUSEPORT_BPF,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_so_max_pacing_rate",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_MAX_PACING_RATE,
        value: "u64",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_so_rcvlowat",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_RCVLOWAT,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_so_rcvlowat",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_RCVLOWAT,
        value: "c_int",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "set_so_buf_lock",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_BUF_LOCK,
        value: "u32",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_so_txrehash",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_TXREHASH,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_so_cnx_advice",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_CNX_ADVICE,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_so_wifi_status",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_WIFI_STATUS,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_so_peek_off",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_PEEK_OFF,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_so_meminfo",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_MEMINFO,
        value: "room",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "set_so_no_check",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_NO_CHECK,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_so_timestamping",
        level: Some(uapi::SOL_SOCKET),
        option: uapi::SO_TIMESTAMPING,
        value: "struct:uapi::so_timestamping",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ip_options",
        level: Some(uapi::SOL_IP),
        option: uapi::IP_OPTIONS,
        value: "bytes",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ip_unicast_if",
        level: Some(uapi::SOL_IP),
        option: uapi::IP_UNICAST_IF,
        value: "u32",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ip_local_port_range",
        level: Some(uapi::SOL_IP),
        option: uapi::IP_LOCAL_PORT_RANGE,
        value: "u32",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ip_router_alert",
        level: Some(uapi::SOL_IP),
        option: uapi::IP_ROUTER_ALERT,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ip_nodefrag",
        level: Some(uapi::SOL_IP),
        option: uapi::IP_NODEFRAG,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ip_recvopts",
        level: Some(uapi::SOL_IP),
        option: uapi::IP_RECVOPTS,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ip_recvfragsize",
        level: Some(uapi::SOL_IP),
        option: uapi::IP_RECVFRAGSIZE,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ip_minttl",
        level: Some(uapi::SOL_IP),
        option: uapi::IP_MINTTL,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_ip_pktoptions",
        level: Some(uapi::SOL_IP),
        option: uapi::IP_PKTOPTIONS,
        value: "room",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "set_ip_mcast_join_group",
        level: Some(uapi::SOL_IP),
        option: uapi::MCAST_JOIN_GROUP,
        value: "struct:uapi::group_req",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ip_mcast_leave_group",
        level: Some(uapi::SOL_IP),
        option: uapi::MCAST_LEAVE_GROUP,
        value: "struct:uapi::group_req",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ip_mcast_msfilter",
        level: Some(uapi::SOL_IP),
        option: uapi::MCAST_MSFILTER,
        value: "group_filter",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_hopopts",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_HOPOPTS,
        value: "bytes",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_rthdrdstopts",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_RTHDRDSTOPTS,
        value: "bytes",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_rthdr",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_RTHDR,
        value: "bytes",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_dstopts",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_DSTOPTS,
        value: "bytes",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_flowinfo_send",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_FLOWINFO_SEND,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_autoflowlabel",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_AUTOFLOWLABEL,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_flowlabel_mgr",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_FLOWLABEL_MGR,
        value: "struct:uapi::in6_flowlabel_req",
        kernel: crate::probe::Kernel::WritesBack,
    },
    crate::probe::Row {
        method: "set_ipv6_addr_preferences",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_ADDR_PREFERENCES,
        value: "u32",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_join_anycast",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_JOIN_ANYCAST,
        value: "struct:uapi::ipv6_mreq",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_leave_anycast",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_LEAVE_ANYCAST,
        value: "struct:uapi::ipv6_mreq",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_mtu",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_MTU,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_unicast_if",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_UNICAST_IF,
        value: "u32",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_router_alert",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_ROUTER_ALERT,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_router_alert_isolate",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_ROUTER_ALERT_ISOLATE,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_checksum",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_CHECKSUM,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_recvhopopts",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_RECVHOPOPTS,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_recvdstopts",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_RECVDSTOPTS,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_recvrthdr",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_RECVRTHDR,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_flowinfo",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_FLOWINFO,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_recvfragsize",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_RECVFRAGSIZE,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_minhopcount",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_MINHOPCOUNT,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_ipv6_2292pktoptions",
        level: Some(uapi::SOL_IPV6),
        option: uapi::IPV6_2292PKTOPTIONS,
        value: "room",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "set_ipv6_mcast_join_group",
        level: Some(uapi::SOL_IPV6),
        option: uapi::MCAST_JOIN_GROUP,
        value: "struct:uapi::group_req",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_mcast_leave_group",
        level: Some(uapi::SOL_IPV6),
        option: uapi::MCAST_LEAVE_GROUP,
        value: "struct:uapi::group_req",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_ipv6_mcast_msfilter",
        level: Some(uapi::SOL_IPV6),
        option: uapi::MCAST_MSFILTER,
        value: "group_filter",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_md5sig_ext",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_MD5SIG_EXT,
        value: "struct:uapi::tcp_md5sig",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_ao_add_key",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_AO_ADD_KEY,
        value: "struct:uapi::tcp_ao_add",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_tcp_ao_get_keys",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_AO_GET_KEYS,
        value: "keys",
        kernel: crate::probe::Kernel::Past,
    },
    crate::probe::Row {
        method: "set_tcp_ao_del_key",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_AO_DEL_KEY,
        value: "struct:uapi::tcp_ao_del",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_ao_info",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_AO_INFO,
        value: "struct:uapi::tcp_ao_info_opt",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_tcp_ao_info",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_AO_INFO,
        value: "room",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "set_tcp_ao_repair",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_AO_REPAIR,
        value: "struct:uapi::tcp_ao_repair",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_tcp_ao_repair",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_AO_REPAIR,
        value: "room",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "set_tcp_window_clamp",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_WINDOW_CLAMP,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_tcp_window_clamp",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_WINDOW_CLAMP,
        value: "c_int",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "set_tcp_syncnt",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_SYNCNT,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_defer_accept",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_DEFER_ACCEPT,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_linger2",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_LINGER2,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_rto_min_us",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_RTO_MIN_US,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_rto_max_ms",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_RTO_MAX_MS,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_delack_max_us",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_DELACK_MAX_US,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_tx_delay",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_TX_DELAY,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_fastopen_no_cookie",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_FASTOPEN_NO_COOKIE,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_inq",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_INQ,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_queue_seq",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_QUEUE_SEQ,
        value: "u32",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_tcp_queue_seq",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_QUEUE_SEQ,
        value: "u32",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "get_tcp_saved_syn",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_SAVED_SYN,
        value: "room",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "set_tcp_fastopen",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_FASTOPEN,
        value: "u32",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_fastopen_key",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_FASTOPEN_KEY,
        value: "bytes",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_save_syn",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_SAVE_SYN,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_tcp_repair_queue",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_REPAIR_QUEUE,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_tcp_repair_queue",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_REPAIR_QUEUE,
        value: "c_int",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "set_tcp_timestamp",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_TIMESTAMP,
        value: "u32",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_tcp_timestamp",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_TIMESTAMP,
        value: "u32",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "set_tcp_quickack",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_QUICKACK,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_tcp_info",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_INFO,
        value: "room",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "get_tcp_cc_info",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_CC_INFO,
        value: "room",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "set_tcp_repair_window",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_REPAIR_WINDOW,
        value: "struct:uapi::tcp_repair_window",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_tcp_repair_window",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_REPAIR_WINDOW,
        value: "room",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "set_tcp_repair_options",
        level: Some(uapi::SOL_TCP),
        option: uapi::TCP_REPAIR_OPTIONS,
        value: "slice:uapi::tcp_repair_opt",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_mptcp_info",
        level: Some(uapi::SOL_MPTCP),
        option: local::MPTCP_INFO,
        value: "room",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "get_mptcp_full_info",
        level: Some(uapi::SOL_MPTCP),
        option: local::MPTCP_FULL_INFO,
        value: "full_info",
        kernel: crate::probe::Kernel::Through,
    },
    crate::probe::Row {
        method: "set_udp_no_check6_rx",
        level: Some(uapi::SOL_UDP),
        option: local::UDP_NO_CHECK6_RX,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_udp_no_check6_tx",
        level: Some(uapi::SOL_UDP),
        option: local::UDP_NO_CHECK6_TX,
        value: "bool",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_udplite_recv_cscov",
        level: Some(local::SOL_UDPLITE),
        option: local::UDPLITE_RECV_CSCOV,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "set_udplite_send_cscov",
        level: Some(local::SOL_UDPLITE),
        option: local::UDPLITE_SEND_CSCOV,
        value: "c_int",
        kernel: crate::probe::Kernel::Reads,
    },
    crate::probe::Row {
        method: "get_siocoutq",
        level: None,
        option: local::SIOCOUTQ,
        value: "ioctl",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "get_siocoutqnsd",
        level: None,
        option: local::SIOCOUTQNSD,
        value: "ioctl",
        kernel: crate::probe::Kernel::Within,
    },
    crate::probe::Row {
        method: "get_siocatmark",
        level: None,
        option: local::SIOCATMARK,
        value: "ioctl",
        kernel: crate::probe::Kernel::Within,
    },
];
