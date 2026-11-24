//! The Linux socket options whose values are plain bytes, and the queue counts of a socket, as one trait.

use std::ffi::c_int;
use std::mem::size_of;
use std::os::fd::BorrowedFd;

use linux_raw_sys::net as uapi;
use rustix::io::Errno;
use zerocopy::FromBytes;

/// How many bytes an option's value takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValueLength {
    /// Exactly the size of the uapi type the option takes.
    Exactly(usize),
    /// Any: a byte string, or a structure the kernel cuts to the length it is given.
    Any,
}

const INT: ValueLength = ValueLength::Exactly(size_of::<c_int>());
const U32: ValueLength = ValueLength::Exactly(size_of::<u32>());
const U64: ValueLength = ValueLength::Exactly(size_of::<u64>());
const ANY: ValueLength = ValueLength::Any;

const fn of<T>() -> ValueLength {
    ValueLength::Exactly(size_of::<T>())
}

/// `MPTCP_INFO`, which neither libc nor linux-raw-sys carries.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
pub const MPTCP_INFO: u32 = 1;
/// `MPTCP_FULL_INFO`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
pub const MPTCP_FULL_INFO: u32 = 4;
/// `UDPLITE_SEND_CSCOV`, which neither libc nor linux-raw-sys carries.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub const UDPLITE_SEND_CSCOV: u32 = 10;
/// `UDPLITE_RECV_CSCOV`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub const UDPLITE_RECV_CSCOV: u32 = 11;
/// `UDP_NO_CHECK6_TX`, which linux-raw-sys does not carry.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub const UDP_NO_CHECK6_TX: u32 = 101;
/// `UDP_NO_CHECK6_RX`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub const UDP_NO_CHECK6_RX: u32 = 102;
/// `SOL_UDP`, the level of UDP's own options.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/linux/socket.h
pub const SOL_UDP: u32 = uapi::IPPROTO_UDP as u32;
/// The level of UDP-Lite's own options, its protocol number.
///
/// https://github.com/torvalds/linux/blob/v6.18/net/ipv4/udplite.c
pub const SOL_UDPLITE: u32 = uapi::IPPROTO_UDPLITE as u32;

macro_rules! plain_options {
    ($($variant:ident = ($level:path, $name:path, $length:expr),)*) => {
        /// A socket option whose value the kernel reads and writes no further than the length given, whatever the value
        /// holds: no address, and no count, length or offset that sizes what the kernel copies; only these exist.
        ///
        /// The conformance suite's `no_listed_option_writes_past_its_length` tries every entry against the kernel.
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum PlainOption {
            $($variant,)*
        }

        impl PlainOption {
            /// Every option, in the order listed.
            pub const ALL: &[PlainOption] = &[$(PlainOption::$variant,)*];

            /// The option's level and name, as `setsockopt` takes them.
            pub const fn level_and_name(self) -> (u32, u32) {
                match self {
                    $(PlainOption::$variant => ($level, $name),)*
                }
            }

            pub const fn value_length(self) -> ValueLength {
                match self {
                    $(PlainOption::$variant => $length,)*
                }
            }

            /// The option of the level and name, or None for one not listed.
            pub const fn find(level: u32, name: u32) -> Option<PlainOption> {
                match (level, name) {
                    $(($level, $name) => Some(PlainOption::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

plain_options! {
    SoMaxPacingRate = (uapi::SOL_SOCKET, uapi::SO_MAX_PACING_RATE, U64),
    SoIncomingCpu = (uapi::SOL_SOCKET, uapi::SO_INCOMING_CPU, INT),
    SoTimestamping = (uapi::SOL_SOCKET, uapi::SO_TIMESTAMPING, of::<uapi::so_timestamping>()),
    SoDetachReuseportBpf = (uapi::SOL_SOCKET, uapi::SO_DETACH_REUSEPORT_BPF, INT),
    SoBufLock = (uapi::SOL_SOCKET, uapi::SO_BUF_LOCK, INT),
    SoRcvlowat = (uapi::SOL_SOCKET, uapi::SO_RCVLOWAT, INT),
    SoTxrehash = (uapi::SOL_SOCKET, uapi::SO_TXREHASH, INT),
    SoCnxAdvice = (uapi::SOL_SOCKET, uapi::SO_CNX_ADVICE, INT),
    SoWifiStatus = (uapi::SOL_SOCKET, uapi::SO_WIFI_STATUS, INT),
    SoPeekOff = (uapi::SOL_SOCKET, uapi::SO_PEEK_OFF, INT),
    SoMeminfo = (uapi::SOL_SOCKET, uapi::SO_MEMINFO, ANY),
    SoRcvmark = (uapi::SOL_SOCKET, uapi::SO_RCVMARK, INT),
    SoRcvpriority = (uapi::SOL_SOCKET, uapi::SO_RCVPRIORITY, INT),
    SoNoCheck = (uapi::SOL_SOCKET, uapi::SO_NO_CHECK, INT),
    SoTxtime = (uapi::SOL_SOCKET, uapi::SO_TXTIME, of::<uapi::sock_txtime>()),
    IpTtl = (uapi::SOL_IP, uapi::IP_TTL, INT),
    IpOptions = (uapi::SOL_IP, uapi::IP_OPTIONS, ANY),
    IpMinttl = (uapi::SOL_IP, uapi::IP_MINTTL, INT),
    IpPktoptions = (uapi::SOL_IP, uapi::IP_PKTOPTIONS, ANY),
    IpLocalPortRange = (uapi::SOL_IP, uapi::IP_LOCAL_PORT_RANGE, U32),
    IpUnicastIf = (uapi::SOL_IP, uapi::IP_UNICAST_IF, U32),
    IpRouterAlert = (uapi::SOL_IP, uapi::IP_ROUTER_ALERT, INT),
    IpNodefrag = (uapi::SOL_IP, uapi::IP_NODEFRAG, INT),
    IpRecvopts = (uapi::SOL_IP, uapi::IP_RECVOPTS, INT),
    IpRecvfragsize = (uapi::SOL_IP, uapi::IP_RECVFRAGSIZE, INT),
    IpMcastJoinGroup = (uapi::SOL_IP, uapi::MCAST_JOIN_GROUP, of::<uapi::group_req>()),
    IpMcastLeaveGroup = (uapi::SOL_IP, uapi::MCAST_LEAVE_GROUP, of::<uapi::group_req>()),
    Ipv6UnicastHops = (uapi::SOL_IPV6, uapi::IPV6_UNICAST_HOPS, INT),
    Ipv6Hopopts = (uapi::SOL_IPV6, uapi::IPV6_HOPOPTS, ANY),
    Ipv6Rthdrdstopts = (uapi::SOL_IPV6, uapi::IPV6_RTHDRDSTOPTS, ANY),
    Ipv6Rthdr = (uapi::SOL_IPV6, uapi::IPV6_RTHDR, ANY),
    Ipv6Dstopts = (uapi::SOL_IPV6, uapi::IPV6_DSTOPTS, ANY),
    Ipv6FlowinfoSend = (uapi::SOL_IPV6, uapi::IPV6_FLOWINFO_SEND, INT),
    Ipv6Autoflowlabel = (uapi::SOL_IPV6, uapi::IPV6_AUTOFLOWLABEL, INT),
    Ipv6AddrPreferences = (uapi::SOL_IPV6, uapi::IPV6_ADDR_PREFERENCES, U32),
    Ipv6FlowlabelMgr = (uapi::SOL_IPV6, uapi::IPV6_FLOWLABEL_MGR, of::<uapi::in6_flowlabel_req>()),
    Ipv6Recvhopopts = (uapi::SOL_IPV6, uapi::IPV6_RECVHOPOPTS, INT),
    Ipv6Recvdstopts = (uapi::SOL_IPV6, uapi::IPV6_RECVDSTOPTS, INT),
    Ipv6Recvrthdr = (uapi::SOL_IPV6, uapi::IPV6_RECVRTHDR, INT),
    Ipv6Flowinfo = (uapi::SOL_IPV6, uapi::IPV6_FLOWINFO, INT),
    Ipv6Recvfragsize = (uapi::SOL_IPV6, uapi::IPV6_RECVFRAGSIZE, INT),
    Ipv6Minhopcount = (uapi::SOL_IPV6, uapi::IPV6_MINHOPCOUNT, INT),
    Ipv62292pktoptions = (uapi::SOL_IPV6, uapi::IPV6_2292PKTOPTIONS, ANY),
    Ipv6JoinAnycast = (uapi::SOL_IPV6, uapi::IPV6_JOIN_ANYCAST, of::<uapi::ipv6_mreq>()),
    Ipv6LeaveAnycast = (uapi::SOL_IPV6, uapi::IPV6_LEAVE_ANYCAST, of::<uapi::ipv6_mreq>()),
    Ipv6Mtu = (uapi::SOL_IPV6, uapi::IPV6_MTU, INT),
    Ipv6UnicastIf = (uapi::SOL_IPV6, uapi::IPV6_UNICAST_IF, U32),
    Ipv6RouterAlert = (uapi::SOL_IPV6, uapi::IPV6_ROUTER_ALERT, INT),
    Ipv6RouterAlertIsolate = (uapi::SOL_IPV6, uapi::IPV6_ROUTER_ALERT_ISOLATE, INT),
    Ipv6Checksum = (uapi::SOL_IPV6, uapi::IPV6_CHECKSUM, INT),
    Ipv6McastJoinGroup = (uapi::SOL_IPV6, uapi::MCAST_JOIN_GROUP, of::<uapi::group_req>()),
    Ipv6McastLeaveGroup = (uapi::SOL_IPV6, uapi::MCAST_LEAVE_GROUP, of::<uapi::group_req>()),
    TcpMd5sigExt = (uapi::SOL_TCP, uapi::TCP_MD5SIG_EXT, of::<uapi::tcp_md5sig>()),
    TcpAoAddKey = (uapi::SOL_TCP, uapi::TCP_AO_ADD_KEY, of::<uapi::tcp_ao_add>()),
    TcpAoDelKey = (uapi::SOL_TCP, uapi::TCP_AO_DEL_KEY, of::<uapi::tcp_ao_del>()),
    TcpAoInfo = (uapi::SOL_TCP, uapi::TCP_AO_INFO, of::<uapi::tcp_ao_info_opt>()),
    TcpAoRepair = (uapi::SOL_TCP, uapi::TCP_AO_REPAIR, of::<uapi::tcp_ao_repair>()),
    TcpInq = (uapi::SOL_TCP, uapi::TCP_INQ, INT),
    TcpQueueSeq = (uapi::SOL_TCP, uapi::TCP_QUEUE_SEQ, U32),
    TcpSavedSyn = (uapi::SOL_TCP, uapi::TCP_SAVED_SYN, ANY),
    TcpFastopen = (uapi::SOL_TCP, uapi::TCP_FASTOPEN, INT),
    TcpFastopenKey = (uapi::SOL_TCP, uapi::TCP_FASTOPEN_KEY, ANY),
    TcpFastopenNoCookie = (uapi::SOL_TCP, uapi::TCP_FASTOPEN_NO_COOKIE, INT),
    TcpSaveSyn = (uapi::SOL_TCP, uapi::TCP_SAVE_SYN, INT),
    TcpRepair = (uapi::SOL_TCP, uapi::TCP_REPAIR, INT),
    TcpRepairQueue = (uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE, INT),
    TcpRepairWindow = (uapi::SOL_TCP, uapi::TCP_REPAIR_WINDOW, of::<uapi::tcp_repair_window>()),
    TcpRepairOptions = (uapi::SOL_TCP, uapi::TCP_REPAIR_OPTIONS, ANY),
    TcpTimestamp = (uapi::SOL_TCP, uapi::TCP_TIMESTAMP, U32),
    TcpInfo = (uapi::SOL_TCP, uapi::TCP_INFO, ANY),
    TcpCcInfo = (uapi::SOL_TCP, uapi::TCP_CC_INFO, ANY),
    TcpQuickack = (uapi::SOL_TCP, uapi::TCP_QUICKACK, INT),
    TcpWindowClamp = (uapi::SOL_TCP, uapi::TCP_WINDOW_CLAMP, INT),
    TcpSyncnt = (uapi::SOL_TCP, uapi::TCP_SYNCNT, INT),
    TcpDeferAccept = (uapi::SOL_TCP, uapi::TCP_DEFER_ACCEPT, INT),
    TcpLinger2 = (uapi::SOL_TCP, uapi::TCP_LINGER2, INT),
    TcpRtoMinUs = (uapi::SOL_TCP, uapi::TCP_RTO_MIN_US, INT),
    TcpRtoMaxMs = (uapi::SOL_TCP, uapi::TCP_RTO_MAX_MS, INT),
    TcpDelackMaxUs = (uapi::SOL_TCP, uapi::TCP_DELACK_MAX_US, INT),
    TcpTxDelay = (uapi::SOL_TCP, uapi::TCP_TX_DELAY, INT),
    MptcpInfo = (uapi::SOL_MPTCP, MPTCP_INFO, ANY),
    UdpNoCheck6Tx = (SOL_UDP, UDP_NO_CHECK6_TX, INT),
    UdpNoCheck6Rx = (SOL_UDP, UDP_NO_CHECK6_RX, INT),
    UdpliteSendCscov = (SOL_UDPLITE, UDPLITE_SEND_CSCOV, INT),
    UdpliteRecvCscov = (SOL_UDPLITE, UDPLITE_RECV_CSCOV, INT),
}

/// A count the kernel keeps of a socket's queues, read by `ioctl`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QueuedCount {
    /// `SIOCOUTQ`: the bytes sent and not yet acknowledged, unsent ones included.
    SendQueue,
    /// `SIOCOUTQNSD`: the bytes not yet sent.
    Unsent,
    /// `SIOCATMARK`: not zero when the next byte to read is the urgent mark.
    AtUrgentMark,
}

/// What an `MPTCP_FULL_INFO` read tells besides the arrays it fills.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MultipathCounts {
    /// The size of the kernel's `tcp_info`, which it cut to the array's element.
    pub tcp_info_size: u32,
    /// The size of the kernel's `mptcp_subflow_info`.
    pub subflow_info_size: u32,
    /// How many subflows the connection has, which may be more than the arrays hold.
    pub subflows: u32,
}

/// The length of `struct tcp_ao_getsockopt`, the one key a `TCP_AO_GET_KEYS` read names and is given.
pub const AUTHENTICATION_KEY_QUERY: usize = size_of::<uapi::tcp_ao_getsockopt>();

/// The bytes of a `struct __kernel_sockaddr_storage`.
pub type StorageBytes = [u8; size_of::<uapi::__kernel_sockaddr_storage>()];

/// The level a multicast setting is made at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MulticastLevel {
    Ipv4,
    Ipv6,
}

impl MulticastLevel {
    pub const fn level(self) -> u32 {
        match self {
            MulticastLevel::Ipv4 => uapi::SOL_IP,
            MulticastLevel::Ipv6 => uapi::SOL_IPV6,
        }
    }
}

/// An `MCAST_MSFILTER` request: a joined group's filter mode and sources, whose count the primitive sets from `sources`.
#[derive(Clone, Copy, Debug)]
pub struct SourceFilterOptions<'a> {
    pub level: MulticastLevel,
    pub interface: u32,
    pub group: StorageBytes,
    /// `MCAST_INCLUDE` or `MCAST_EXCLUDE`.
    pub mode: u32,
    pub sources: &'a [StorageBytes],
}

#[allow(
    reviewable::explicit_shared_error::Errno,
    reason = "each method is one call into the kernel and fails only with the error number it gives"
)]
/// Every method refuses a descriptor that is not an IPv4 or IPv6 socket of TCP, MPTCP, UDP, UDP-Lite, ICMP or a raw
/// one, the sockets the kernel's handlers of these options were read for.
pub trait SocketOptions {
    /// Sets the option to the value, refused with `EINVAL` when the value's length is not the option's.
    fn set_option(
        &self,
        socket: BorrowedFd<'_>,
        option: PlainOption,
        value: &[u8],
    ) -> Result<(), Errno>;

    /// Reads the option into the buffer and returns the length the kernel gave, refused with `EINVAL` when the buffer's length is not the option's.
    fn get_option(
        &self,
        socket: BorrowedFd<'_>,
        option: PlainOption,
        into: &mut [u8],
    ) -> Result<usize, Errno>;

    /// Refused with `ENOTTY` but on TCP, and on MPTCP for the urgent mark, which it does not answer.
    fn count_queued(&self, socket: BorrowedFd<'_>, count: QueuedCount) -> Result<c_int, Errno>;

    /// Reads `TCP_AO_GET_KEYS` for at most the one key the query names, whatever its `nkeys` held.
    fn get_authentication_key(
        &self,
        socket: BorrowedFd<'_>,
        query: &mut [u8; AUTHENTICATION_KEY_QUERY],
    ) -> Result<(), Errno>;

    fn set_source_filter(
        &self,
        socket: BorrowedFd<'_>,
        filter: &SourceFilterOptions<'_>,
    ) -> Result<(), Errno>;

    /// Reads `MPTCP_FULL_INFO`: the connection's `mptcp_info` into `connection`, and each subflow's `mptcp_subflow_info`
    /// and `tcp_info` into the two arrays, as many as the shorter holds, each cut to its element.
    fn get_multipath_full_information<const CONNECTION: usize, Subflow, Information>(
        &self,
        socket: BorrowedFd<'_>,
        connection: &mut [u8; CONNECTION],
        subflows: &mut [Subflow],
        information: &mut [Information],
    ) -> Result<MultipathCounts, Errno>
    where
        Subflow: FromBytes,
        Information: FromBytes;
}
