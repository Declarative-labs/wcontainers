//! The Linux socket options whose values are plain bytes, and the queue counts of a socket, as one trait.

use std::ffi::c_int;
use std::mem::size_of;
use std::os::fd::BorrowedFd;

use linux_raw_sys::net as uapi;
use rustix::io::Errno;
use zerocopy::FromBytes;

/// What an option's value is, and so what the kernel can find in the bytes it is given.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Value {
    /// A 4-byte integer: too short to hold an address.
    Int,
    /// A 64-bit integer the kernel reads as a number, not an address.
    Number64,
    /// Exactly one uapi structure, by its name in linux-raw-sys, whose fields the suite's
    /// `no_listed_structure_holds_an_address` reads.
    Structure { size: usize, name: &'static str },
    /// A byte string the kernel parses, of any length, holding no address.
    Bytes,
    /// Read only: the kernel fills it and reads nothing from it; a set is refused.
    Filled,
}

impl Value {
    /// The one length the value takes, or None for any.
    pub const fn exact_length(self) -> Option<usize> {
        match self {
            Value::Int => Some(size_of::<c_int>()),
            Value::Number64 => Some(size_of::<u64>()),
            Value::Structure { size, .. } => Some(size),
            Value::Bytes | Value::Filled => None,
        }
    }
}

const INT: Value = Value::Int;
const NUMBER64: Value = Value::Number64;
const BYTES: Value = Value::Bytes;
const FILLED: Value = Value::Filled;

macro_rules! structure {
    ($name:ident) => {
        Value::Structure {
            size: size_of::<uapi::$name>(),
            name: stringify!($name),
        }
    };
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
    ($($variant:ident = ($level:path, $name:path, $value:expr),)*) => {
        /// A socket option a call site of the workspace sets or reads by its level and name, and no library wraps.
        ///
        /// The list is not what keeps the kernel inside the value: every call gives the kernel private memory that ends
        /// where unbacked guard pages begin, so an entry the kernel copies further than the length fails with `EFAULT`.
        /// What the list must still guarantee is that no value holds an address the kernel follows: a read gives the
        /// kernel zeros, and a set only bytes of its entry's [`Value`] kind; the conformance suite's
        /// `no_listed_structure_holds_an_address` checks each structure's fields, and `the_state_that_fills_each_option`
        /// names, in a `match` with no catch-all, the socket on which the kernel fills each entry.
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

            /// What the option's value is.
            pub const fn value(self) -> Value {
                match self {
                    $(PlainOption::$variant => $value,)*
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
    SoMaxPacingRate = (uapi::SOL_SOCKET, uapi::SO_MAX_PACING_RATE, NUMBER64),
    SoTimestamping = (uapi::SOL_SOCKET, uapi::SO_TIMESTAMPING, structure!(so_timestamping)),
    SoDetachReuseportBpf = (uapi::SOL_SOCKET, uapi::SO_DETACH_REUSEPORT_BPF, INT),
    SoBufLock = (uapi::SOL_SOCKET, uapi::SO_BUF_LOCK, INT),
    SoRcvlowat = (uapi::SOL_SOCKET, uapi::SO_RCVLOWAT, INT),
    SoTxrehash = (uapi::SOL_SOCKET, uapi::SO_TXREHASH, INT),
    SoCnxAdvice = (uapi::SOL_SOCKET, uapi::SO_CNX_ADVICE, INT),
    SoWifiStatus = (uapi::SOL_SOCKET, uapi::SO_WIFI_STATUS, INT),
    SoPeekOff = (uapi::SOL_SOCKET, uapi::SO_PEEK_OFF, INT),
    SoMeminfo = (uapi::SOL_SOCKET, uapi::SO_MEMINFO, FILLED),
    SoRcvmark = (uapi::SOL_SOCKET, uapi::SO_RCVMARK, INT),
    SoRcvpriority = (uapi::SOL_SOCKET, uapi::SO_RCVPRIORITY, INT),
    SoNoCheck = (uapi::SOL_SOCKET, uapi::SO_NO_CHECK, INT),
    SoTxtime = (uapi::SOL_SOCKET, uapi::SO_TXTIME, structure!(sock_txtime)),
    IpOptions = (uapi::SOL_IP, uapi::IP_OPTIONS, BYTES),
    IpMinttl = (uapi::SOL_IP, uapi::IP_MINTTL, INT),
    IpPktoptions = (uapi::SOL_IP, uapi::IP_PKTOPTIONS, FILLED),
    IpLocalPortRange = (uapi::SOL_IP, uapi::IP_LOCAL_PORT_RANGE, INT),
    IpUnicastIf = (uapi::SOL_IP, uapi::IP_UNICAST_IF, INT),
    IpRouterAlert = (uapi::SOL_IP, uapi::IP_ROUTER_ALERT, INT),
    IpNodefrag = (uapi::SOL_IP, uapi::IP_NODEFRAG, INT),
    IpRecvopts = (uapi::SOL_IP, uapi::IP_RECVOPTS, INT),
    IpRecvfragsize = (uapi::SOL_IP, uapi::IP_RECVFRAGSIZE, INT),
    IpMcastJoinGroup = (uapi::SOL_IP, uapi::MCAST_JOIN_GROUP, structure!(group_req)),
    IpMcastLeaveGroup = (uapi::SOL_IP, uapi::MCAST_LEAVE_GROUP, structure!(group_req)),
    Ipv6Hopopts = (uapi::SOL_IPV6, uapi::IPV6_HOPOPTS, BYTES),
    Ipv6Rthdrdstopts = (uapi::SOL_IPV6, uapi::IPV6_RTHDRDSTOPTS, BYTES),
    Ipv6Rthdr = (uapi::SOL_IPV6, uapi::IPV6_RTHDR, BYTES),
    Ipv6Dstopts = (uapi::SOL_IPV6, uapi::IPV6_DSTOPTS, BYTES),
    Ipv6FlowinfoSend = (uapi::SOL_IPV6, uapi::IPV6_FLOWINFO_SEND, INT),
    Ipv6Autoflowlabel = (uapi::SOL_IPV6, uapi::IPV6_AUTOFLOWLABEL, INT),
    Ipv6AddrPreferences = (uapi::SOL_IPV6, uapi::IPV6_ADDR_PREFERENCES, INT),
    Ipv6FlowlabelMgr = (uapi::SOL_IPV6, uapi::IPV6_FLOWLABEL_MGR, structure!(in6_flowlabel_req)),
    Ipv6Recvhopopts = (uapi::SOL_IPV6, uapi::IPV6_RECVHOPOPTS, INT),
    Ipv6Recvdstopts = (uapi::SOL_IPV6, uapi::IPV6_RECVDSTOPTS, INT),
    Ipv6Recvrthdr = (uapi::SOL_IPV6, uapi::IPV6_RECVRTHDR, INT),
    Ipv6Flowinfo = (uapi::SOL_IPV6, uapi::IPV6_FLOWINFO, INT),
    Ipv6Recvfragsize = (uapi::SOL_IPV6, uapi::IPV6_RECVFRAGSIZE, INT),
    Ipv6Minhopcount = (uapi::SOL_IPV6, uapi::IPV6_MINHOPCOUNT, INT),
    Ipv62292pktoptions = (uapi::SOL_IPV6, uapi::IPV6_2292PKTOPTIONS, FILLED),
    Ipv6JoinAnycast = (uapi::SOL_IPV6, uapi::IPV6_JOIN_ANYCAST, structure!(ipv6_mreq)),
    Ipv6LeaveAnycast = (uapi::SOL_IPV6, uapi::IPV6_LEAVE_ANYCAST, structure!(ipv6_mreq)),
    Ipv6Mtu = (uapi::SOL_IPV6, uapi::IPV6_MTU, INT),
    Ipv6UnicastIf = (uapi::SOL_IPV6, uapi::IPV6_UNICAST_IF, INT),
    Ipv6RouterAlert = (uapi::SOL_IPV6, uapi::IPV6_ROUTER_ALERT, INT),
    Ipv6RouterAlertIsolate = (uapi::SOL_IPV6, uapi::IPV6_ROUTER_ALERT_ISOLATE, INT),
    Ipv6Checksum = (uapi::SOL_IPV6, uapi::IPV6_CHECKSUM, INT),
    Ipv6McastJoinGroup = (uapi::SOL_IPV6, uapi::MCAST_JOIN_GROUP, structure!(group_req)),
    Ipv6McastLeaveGroup = (uapi::SOL_IPV6, uapi::MCAST_LEAVE_GROUP, structure!(group_req)),
    TcpMd5sigExt = (uapi::SOL_TCP, uapi::TCP_MD5SIG_EXT, structure!(tcp_md5sig)),
    TcpAoAddKey = (uapi::SOL_TCP, uapi::TCP_AO_ADD_KEY, structure!(tcp_ao_add)),
    TcpAoDelKey = (uapi::SOL_TCP, uapi::TCP_AO_DEL_KEY, structure!(tcp_ao_del)),
    TcpAoInfo = (uapi::SOL_TCP, uapi::TCP_AO_INFO, structure!(tcp_ao_info_opt)),
    TcpAoRepair = (uapi::SOL_TCP, uapi::TCP_AO_REPAIR, structure!(tcp_ao_repair)),
    TcpInq = (uapi::SOL_TCP, uapi::TCP_INQ, INT),
    TcpQueueSeq = (uapi::SOL_TCP, uapi::TCP_QUEUE_SEQ, INT),
    TcpSavedSyn = (uapi::SOL_TCP, uapi::TCP_SAVED_SYN, FILLED),
    TcpFastopen = (uapi::SOL_TCP, uapi::TCP_FASTOPEN, INT),
    TcpFastopenKey = (uapi::SOL_TCP, uapi::TCP_FASTOPEN_KEY, BYTES),
    TcpFastopenNoCookie = (uapi::SOL_TCP, uapi::TCP_FASTOPEN_NO_COOKIE, INT),
    TcpSaveSyn = (uapi::SOL_TCP, uapi::TCP_SAVE_SYN, INT),
    TcpRepairQueue = (uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE, INT),
    TcpRepairWindow = (uapi::SOL_TCP, uapi::TCP_REPAIR_WINDOW, structure!(tcp_repair_window)),
    TcpRepairOptions = (uapi::SOL_TCP, uapi::TCP_REPAIR_OPTIONS, BYTES),
    TcpTimestamp = (uapi::SOL_TCP, uapi::TCP_TIMESTAMP, INT),
    TcpInfo = (uapi::SOL_TCP, uapi::TCP_INFO, FILLED),
    TcpCcInfo = (uapi::SOL_TCP, uapi::TCP_CC_INFO, FILLED),
    TcpQuickack = (uapi::SOL_TCP, uapi::TCP_QUICKACK, INT),
    TcpWindowClamp = (uapi::SOL_TCP, uapi::TCP_WINDOW_CLAMP, INT),
    TcpSyncnt = (uapi::SOL_TCP, uapi::TCP_SYNCNT, INT),
    TcpDeferAccept = (uapi::SOL_TCP, uapi::TCP_DEFER_ACCEPT, INT),
    TcpLinger2 = (uapi::SOL_TCP, uapi::TCP_LINGER2, INT),
    TcpRtoMinUs = (uapi::SOL_TCP, uapi::TCP_RTO_MIN_US, INT),
    TcpRtoMaxMs = (uapi::SOL_TCP, uapi::TCP_RTO_MAX_MS, INT),
    TcpDelackMaxUs = (uapi::SOL_TCP, uapi::TCP_DELACK_MAX_US, INT),
    TcpTxDelay = (uapi::SOL_TCP, uapi::TCP_TX_DELAY, INT),
    MptcpInfo = (uapi::SOL_MPTCP, MPTCP_INFO, FILLED),
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
/// Every call gives the kernel private memory in place of the caller's bytes: the value's own length, ending where
/// unbacked guard pages begin, so that whatever the option or request means to whatever the descriptor is when the
/// call runs, the kernel's reads and writes through that memory stay in it or fail with `EFAULT`. That is the
/// containment. Before each call a method also refuses a descriptor that is not an IPv4 or IPv6 socket of TCP or MPTCP
/// (a stream), UDP, UDP-Lite or ICMP (a datagram) or a raw one; that check is a courtesy, not the containment, since
/// another thread can make the descriptor's number name something else between the check and the call.
pub trait SocketOptions {
    /// Sets the option to the value, refused with `EINVAL` when the value's length is not the option's, and with
    /// `ENOPROTOOPT` for an option that is only read.
    fn set_option(
        &self,
        socket: BorrowedFd<'_>,
        option: PlainOption,
        value: &[u8],
    ) -> Result<(), Errno>;

    /// Reads the option into the buffer and returns the length the kernel gave, refused with `EINVAL` when the buffer's
    /// length is not the option's; the kernel is given zeros, never the buffer's bytes.
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
    /// and `tcp_info` into the two arrays, as many as the shorter holds, each cut to its element; the bytes the kernel
    /// does not write are zero. The two addresses the request holds are the primitive's, into its private memory.
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
