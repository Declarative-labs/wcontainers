//! What the kernel does with the memory of a value at each pair of level and option numbers a row may name.
//!
//! The kernel sees numbers, never names: an entry is keyed by the level number and the option or request number, each
//! taken from the kernel's own headers as linux-raw-sys, libc or the direct crate's `local.rs` give them (never typed
//! here), and a row's names resolve to numbers only through its level's list. Two names of one pair are one entry.
//! What an entry says comes from the manual page it names and the kernel handler it names, read, not copied.

#![allow(
    dead_code,
    reason = "each module of a test binary is compiled into it whole"
)]

use linux_raw_sys::net as uapi;

#[path = "../../local.rs"]
mod local;

/// A name a row may give, its path as the table spells it and the number the kernel's headers give it.
#[derive(Clone, Copy, Debug)]
pub struct Name {
    pub path: &'static str,
    pub number: u32,
}

/// Names a constant by its path, its number taken from that same constant.
macro_rules! name {
    ($space:ident :: $constant:ident) => {
        Name {
            path: concat!(stringify!($space), "::", stringify!($constant)),
            number: $space::$constant,
        }
    };
}

/// A level of `setsockopt(2)` and the names of the options in its own number space.
pub struct Level {
    pub name: Name,
    pub options: &'static [Name],
}

/// Every level a row may name, each with the options of its own number space a row may name.
pub const LEVELS: [Level; 7] = [
    Level {
        name: name!(uapi::SOL_SOCKET),
        options: &[
            name!(uapi::SO_RCVMARK),
            name!(uapi::SO_RCVPRIORITY),
            name!(uapi::SO_ATTACH_REUSEPORT_CBPF),
            name!(uapi::SO_DETACH_REUSEPORT_BPF),
            name!(uapi::SO_MAX_PACING_RATE),
            name!(uapi::SO_RCVLOWAT),
            name!(uapi::SO_BUF_LOCK),
            name!(uapi::SO_TXREHASH),
            name!(uapi::SO_CNX_ADVICE),
            name!(uapi::SO_WIFI_STATUS),
            name!(uapi::SO_PEEK_OFF),
            name!(uapi::SO_MEMINFO),
            name!(uapi::SO_NO_CHECK),
            name!(uapi::SO_TIMESTAMPING),
            name!(uapi::SO_TIMESTAMPING_OLD),
            name!(uapi::SO_ATTACH_FILTER),
            name!(uapi::SO_GET_FILTER),
        ],
    },
    Level {
        name: name!(uapi::SOL_IP),
        options: &[
            name!(uapi::IP_OPTIONS),
            name!(uapi::IP_UNICAST_IF),
            name!(uapi::IP_LOCAL_PORT_RANGE),
            name!(uapi::IP_ROUTER_ALERT),
            name!(uapi::IP_NODEFRAG),
            name!(uapi::IP_RECVOPTS),
            name!(uapi::IP_RECVFRAGSIZE),
            name!(uapi::IP_MINTTL),
            name!(uapi::IP_PKTOPTIONS),
            name!(uapi::MCAST_JOIN_GROUP),
            name!(uapi::MCAST_LEAVE_GROUP),
            name!(uapi::MCAST_MSFILTER),
        ],
    },
    Level {
        name: name!(uapi::SOL_IPV6),
        options: &[
            name!(uapi::IPV6_HOPOPTS),
            name!(uapi::IPV6_RTHDRDSTOPTS),
            name!(uapi::IPV6_RTHDR),
            name!(uapi::IPV6_DSTOPTS),
            name!(uapi::IPV6_FLOWINFO_SEND),
            name!(uapi::IPV6_AUTOFLOWLABEL),
            name!(uapi::IPV6_FLOWLABEL_MGR),
            name!(uapi::IPV6_ADDR_PREFERENCES),
            name!(uapi::IPV6_JOIN_ANYCAST),
            name!(uapi::IPV6_LEAVE_ANYCAST),
            name!(uapi::IPV6_MTU),
            name!(uapi::IPV6_UNICAST_IF),
            name!(uapi::IPV6_ROUTER_ALERT),
            name!(uapi::IPV6_ROUTER_ALERT_ISOLATE),
            name!(uapi::IPV6_CHECKSUM),
            name!(uapi::IPV6_RECVHOPOPTS),
            name!(uapi::IPV6_RECVDSTOPTS),
            name!(uapi::IPV6_RECVRTHDR),
            name!(uapi::IPV6_FLOWINFO),
            name!(uapi::IPV6_RECVFRAGSIZE),
            name!(uapi::IPV6_MINHOPCOUNT),
            name!(uapi::IPV6_2292PKTOPTIONS),
            name!(uapi::MCAST_JOIN_GROUP),
            name!(uapi::MCAST_LEAVE_GROUP),
            name!(uapi::MCAST_MSFILTER),
        ],
    },
    Level {
        name: name!(uapi::SOL_TCP),
        options: &[
            name!(uapi::TCP_MD5SIG_EXT),
            name!(uapi::TCP_AO_ADD_KEY),
            name!(uapi::TCP_AO_GET_KEYS),
            name!(uapi::TCP_AO_DEL_KEY),
            name!(uapi::TCP_AO_INFO),
            name!(uapi::TCP_AO_REPAIR),
            name!(uapi::TCP_WINDOW_CLAMP),
            name!(uapi::TCP_SYNCNT),
            name!(uapi::TCP_DEFER_ACCEPT),
            name!(uapi::TCP_LINGER2),
            name!(uapi::TCP_RTO_MIN_US),
            name!(uapi::TCP_RTO_MAX_MS),
            name!(uapi::TCP_DELACK_MAX_US),
            name!(uapi::TCP_TX_DELAY),
            name!(uapi::TCP_FASTOPEN_NO_COOKIE),
            name!(uapi::TCP_INQ),
            name!(uapi::TCP_QUEUE_SEQ),
            name!(uapi::TCP_SAVED_SYN),
            name!(uapi::TCP_FASTOPEN),
            name!(uapi::TCP_FASTOPEN_KEY),
            name!(uapi::TCP_SAVE_SYN),
            name!(uapi::TCP_REPAIR_QUEUE),
            name!(uapi::TCP_TIMESTAMP),
            name!(uapi::TCP_QUICKACK),
            name!(uapi::TCP_INFO),
            name!(uapi::TCP_CC_INFO),
            name!(uapi::TCP_REPAIR_WINDOW),
            name!(uapi::TCP_REPAIR_OPTIONS),
            name!(uapi::TCP_ZEROCOPY_RECEIVE),
        ],
    },
    Level {
        name: name!(uapi::SOL_MPTCP),
        options: &[name!(local::MPTCP_INFO), name!(local::MPTCP_FULL_INFO)],
    },
    Level {
        name: name!(uapi::SOL_UDP),
        options: &[
            name!(local::UDP_NO_CHECK6_RX),
            name!(local::UDP_NO_CHECK6_TX),
        ],
    },
    Level {
        name: name!(local::SOL_UDPLITE),
        options: &[
            name!(local::UDPLITE_RECV_CSCOV),
            name!(local::UDPLITE_SEND_CSCOV),
        ],
    },
];

/// The ioctl requests a row may name, in the one number space of `ioctl(2)` on a socket.
pub const REQUESTS: [Name; 4] = [
    name!(local::SIOCOUTQ),
    name!(local::SIOCOUTQNSD),
    name!(local::SIOCATMARK),
    name!(uapi::SIOCGSTAMP_OLD),
];

/// What the kernel does with the memory of a value, for one direction of one pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Does {
    /// Not known here: a row of this direction is refused.
    Unknown,
    /// A set that reads an int, or a 64-bit number, at most the length it is told.
    ReadsNumber,
    /// A set that reads an opaque run of bytes holding no address, at most the length it is told.
    ReadsBytes,
    /// A set that reads one value of the named uapi type, at most the length it is told.
    ReadsStructure(&'static str),
    /// A set that reads a run of values of the named uapi type, at most the length it is told.
    ReadsRun(&'static str),
    /// A set that reads `struct group_filter` and the sources after it, at most the length it is told.
    ReadsGroupFilter,
    /// A set that reads one value of the named uapi type and may write into it, within the length it is told.
    WritesBack(&'static str),
    /// A set that reads `struct sock_fprog` and then `len` instructions at the address it holds.
    FollowsProgram,
    /// A call that reads or writes at an address its value holds, in a way no form of the generator handles.
    Follows,
    /// A get that writes at most the length it is told, and follows no address in its value.
    WritesWithin,
    /// A get that writes one answer of the length it is told per key its value asks for.
    WritesPerKey,
    /// A get that writes past the length it is told, the length counting something other than bytes.
    WritesPast,
    /// A get that reads `struct mptcp_full_info` and writes the two runs at the addresses it holds.
    WritesThroughFullInfo,
    /// An ioctl request that writes one int at its pointer.
    WritesInt,
    /// An ioctl request that writes one value of the named type at its pointer, whatever room is behind it.
    WritesStructure(&'static str),
}

/// What the kernel does at one pair, for a set and for a get, and where that is written.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    /// The level number, or none for an ioctl request.
    pub level: Option<u32>,
    pub option: u32,
    pub set: Does,
    pub get: Does,
    /// The manual page and the kernel handler the entry was read from.
    pub source: &'static str,
}

use Does::{
    Follows, FollowsProgram, ReadsBytes, ReadsGroupFilter, ReadsNumber, ReadsRun, ReadsStructure,
    Unknown, WritesBack, WritesInt, WritesPast, WritesPerKey, WritesStructure,
    WritesThroughFullInfo, WritesWithin,
};

/// The socket(7) handler both directions of the `SOL_SOCKET` options go through.
const SOCKET: &str = "socket(7); net/core/sock.c sk_setsockopt and sk_getsockopt";
const IP: &str = "ip(7); net/ipv4/ip_sockglue.c do_ip_setsockopt and do_ip_getsockopt";
const IPV6: &str = "ipv6(7); net/ipv6/ipv6_sockglue.c do_ipv6_setsockopt and do_ipv6_getsockopt";
const TCP: &str = "tcp(7); net/ipv4/tcp.c do_tcp_setsockopt and do_tcp_getsockopt";
const TCP_AO: &str = "tcp(7), docs.kernel.org networking/tcp_ao; net/ipv4/tcp_ao.c";
const MPTCP: &str = "mptcp(7); net/mptcp/sockopt.c mptcp_getsockopt_sol_mptcp";
const UDP: &str = "udp(7), udplite(7); net/ipv4/udp.c udp_lib_setsockopt";
const SOCKET_IOCTL: &str =
    "tcp(7), sockios.h; net/ipv4/tcp.c tcp_ioctl and net/core/sock.c sock_gettstamp";

/// Gives the entry of a set-and-get option of `SOL_SOCKET`.
const fn socket(option: u32, set: Does, get: Does) -> Entry {
    Entry {
        level: Some(uapi::SOL_SOCKET),
        option,
        set,
        get,
        source: SOCKET,
    }
}

const fn ip(option: u32, set: Does, get: Does) -> Entry {
    Entry {
        level: Some(uapi::SOL_IP),
        option,
        set,
        get,
        source: IP,
    }
}

const fn ipv6(option: u32, set: Does, get: Does) -> Entry {
    Entry {
        level: Some(uapi::SOL_IPV6),
        option,
        set,
        get,
        source: IPV6,
    }
}

const fn tcp(option: u32, set: Does, get: Does, source: &'static str) -> Entry {
    Entry {
        level: Some(uapi::SOL_TCP),
        option,
        set,
        get,
        source,
    }
}

const fn request(option: u32, does: Does) -> Entry {
    Entry {
        level: None,
        option,
        set: Unknown,
        get: does,
        source: SOCKET_IOCTL,
    }
}

/// Every pair a row may resolve to: a direction marked `Unknown` is one no row may take.
pub const ENTRIES: [Entry; 91] = [
    socket(uapi::SO_RCVMARK, ReadsNumber, Unknown),
    socket(uapi::SO_RCVPRIORITY, ReadsNumber, Unknown),
    socket(uapi::SO_ATTACH_REUSEPORT_CBPF, FollowsProgram, Unknown),
    socket(uapi::SO_DETACH_REUSEPORT_BPF, ReadsNumber, Unknown),
    socket(uapi::SO_MAX_PACING_RATE, ReadsNumber, WritesWithin),
    socket(uapi::SO_RCVLOWAT, ReadsNumber, WritesWithin),
    socket(uapi::SO_BUF_LOCK, ReadsNumber, Unknown),
    socket(uapi::SO_TXREHASH, ReadsNumber, Unknown),
    socket(uapi::SO_CNX_ADVICE, ReadsNumber, Unknown),
    socket(uapi::SO_WIFI_STATUS, ReadsNumber, Unknown),
    socket(uapi::SO_PEEK_OFF, ReadsNumber, Unknown),
    socket(uapi::SO_MEMINFO, Unknown, WritesWithin),
    socket(uapi::SO_NO_CHECK, ReadsNumber, Unknown),
    socket(
        uapi::SO_TIMESTAMPING,
        ReadsStructure("uapi::so_timestamping"),
        Unknown,
    ),
    // SO_GET_FILTER is SO_ATTACH_FILTER's number: a get takes the length as a count of instructions (sk_get_filter).
    socket(uapi::SO_ATTACH_FILTER, FollowsProgram, WritesPast),
    ip(uapi::IP_OPTIONS, ReadsBytes, Unknown),
    ip(uapi::IP_UNICAST_IF, ReadsNumber, Unknown),
    ip(uapi::IP_LOCAL_PORT_RANGE, ReadsNumber, Unknown),
    ip(uapi::IP_ROUTER_ALERT, ReadsNumber, Unknown),
    ip(uapi::IP_NODEFRAG, ReadsNumber, Unknown),
    ip(uapi::IP_RECVOPTS, ReadsNumber, Unknown),
    ip(uapi::IP_RECVFRAGSIZE, ReadsNumber, Unknown),
    ip(uapi::IP_MINTTL, ReadsNumber, Unknown),
    // The control messages are put through `put_cmsg`, which stops at the length told (MSG_CTRUNC).
    ip(uapi::IP_PKTOPTIONS, Unknown, WritesWithin),
    ip(
        uapi::MCAST_JOIN_GROUP,
        ReadsStructure("uapi::group_req"),
        Unknown,
    ),
    ip(
        uapi::MCAST_LEAVE_GROUP,
        ReadsStructure("uapi::group_req"),
        Unknown,
    ),
    ip(uapi::MCAST_MSFILTER, ReadsGroupFilter, Unknown),
    ipv6(uapi::IPV6_HOPOPTS, ReadsBytes, Unknown),
    ipv6(uapi::IPV6_RTHDRDSTOPTS, ReadsBytes, Unknown),
    ipv6(uapi::IPV6_RTHDR, ReadsBytes, Unknown),
    ipv6(uapi::IPV6_DSTOPTS, ReadsBytes, Unknown),
    ipv6(uapi::IPV6_FLOWINFO_SEND, ReadsNumber, Unknown),
    ipv6(uapi::IPV6_AUTOFLOWLABEL, ReadsNumber, Unknown),
    // A get-and-create request with label 0 is answered with the label chosen, written into the request.
    ipv6(
        uapi::IPV6_FLOWLABEL_MGR,
        WritesBack("uapi::in6_flowlabel_req"),
        Unknown,
    ),
    ipv6(uapi::IPV6_ADDR_PREFERENCES, ReadsNumber, Unknown),
    ipv6(
        uapi::IPV6_JOIN_ANYCAST,
        ReadsStructure("uapi::ipv6_mreq"),
        Unknown,
    ),
    ipv6(
        uapi::IPV6_LEAVE_ANYCAST,
        ReadsStructure("uapi::ipv6_mreq"),
        Unknown,
    ),
    ipv6(uapi::IPV6_MTU, ReadsNumber, Unknown),
    ipv6(uapi::IPV6_UNICAST_IF, ReadsNumber, Unknown),
    ipv6(uapi::IPV6_ROUTER_ALERT, ReadsNumber, Unknown),
    ipv6(uapi::IPV6_ROUTER_ALERT_ISOLATE, ReadsNumber, Unknown),
    ipv6(uapi::IPV6_CHECKSUM, ReadsNumber, Unknown),
    ipv6(uapi::IPV6_RECVHOPOPTS, ReadsNumber, Unknown),
    ipv6(uapi::IPV6_RECVDSTOPTS, ReadsNumber, Unknown),
    ipv6(uapi::IPV6_RECVRTHDR, ReadsNumber, Unknown),
    ipv6(uapi::IPV6_FLOWINFO, ReadsNumber, Unknown),
    ipv6(uapi::IPV6_RECVFRAGSIZE, ReadsNumber, Unknown),
    ipv6(uapi::IPV6_MINHOPCOUNT, ReadsNumber, Unknown),
    ipv6(uapi::IPV6_2292PKTOPTIONS, Unknown, WritesWithin),
    ipv6(
        uapi::MCAST_JOIN_GROUP,
        ReadsStructure("uapi::group_req"),
        Unknown,
    ),
    ipv6(
        uapi::MCAST_LEAVE_GROUP,
        ReadsStructure("uapi::group_req"),
        Unknown,
    ),
    ipv6(uapi::MCAST_MSFILTER, ReadsGroupFilter, Unknown),
    tcp(
        uapi::TCP_MD5SIG_EXT,
        ReadsStructure("uapi::tcp_md5sig"),
        Unknown,
        TCP,
    ),
    tcp(
        uapi::TCP_AO_ADD_KEY,
        ReadsStructure("uapi::tcp_ao_add"),
        Unknown,
        TCP_AO,
    ),
    // tcp_ao_get_mkts writes one answer per key matched, up to the `nkeys` asked, each at the length told.
    tcp(uapi::TCP_AO_GET_KEYS, Unknown, WritesPerKey, TCP_AO),
    tcp(
        uapi::TCP_AO_DEL_KEY,
        ReadsStructure("uapi::tcp_ao_del"),
        Unknown,
        TCP_AO,
    ),
    tcp(
        uapi::TCP_AO_INFO,
        ReadsStructure("uapi::tcp_ao_info_opt"),
        WritesWithin,
        TCP_AO,
    ),
    tcp(
        uapi::TCP_AO_REPAIR,
        ReadsStructure("uapi::tcp_ao_repair"),
        WritesWithin,
        TCP_AO,
    ),
    tcp(uapi::TCP_WINDOW_CLAMP, ReadsNumber, WritesWithin, TCP),
    tcp(uapi::TCP_SYNCNT, ReadsNumber, Unknown, TCP),
    tcp(uapi::TCP_DEFER_ACCEPT, ReadsNumber, Unknown, TCP),
    tcp(uapi::TCP_LINGER2, ReadsNumber, Unknown, TCP),
    tcp(uapi::TCP_RTO_MIN_US, ReadsNumber, Unknown, TCP),
    tcp(uapi::TCP_RTO_MAX_MS, ReadsNumber, Unknown, TCP),
    tcp(uapi::TCP_DELACK_MAX_US, ReadsNumber, Unknown, TCP),
    tcp(uapi::TCP_TX_DELAY, ReadsNumber, Unknown, TCP),
    tcp(uapi::TCP_FASTOPEN_NO_COOKIE, ReadsNumber, Unknown, TCP),
    tcp(uapi::TCP_INQ, ReadsNumber, Unknown, TCP),
    tcp(uapi::TCP_QUEUE_SEQ, ReadsNumber, WritesWithin, TCP),
    // A room shorter than the saved headers is refused with EINVAL and nothing written.
    tcp(uapi::TCP_SAVED_SYN, Unknown, WritesWithin, TCP),
    tcp(uapi::TCP_FASTOPEN, ReadsNumber, Unknown, TCP),
    tcp(uapi::TCP_FASTOPEN_KEY, ReadsBytes, Unknown, TCP),
    tcp(uapi::TCP_SAVE_SYN, ReadsNumber, Unknown, TCP),
    tcp(uapi::TCP_REPAIR_QUEUE, ReadsNumber, WritesWithin, TCP),
    tcp(uapi::TCP_TIMESTAMP, ReadsNumber, WritesWithin, TCP),
    tcp(uapi::TCP_QUICKACK, ReadsNumber, Unknown, TCP),
    tcp(uapi::TCP_INFO, Unknown, WritesWithin, TCP),
    tcp(uapi::TCP_CC_INFO, Unknown, WritesWithin, TCP),
    tcp(
        uapi::TCP_REPAIR_WINDOW,
        ReadsStructure("uapi::tcp_repair_window"),
        WritesWithin,
        TCP,
    ),
    tcp(
        uapi::TCP_REPAIR_OPTIONS,
        ReadsRun("uapi::tcp_repair_opt"),
        Unknown,
        TCP,
    ),
    // `struct tcp_zerocopy_receive` holds the address the kernel maps the received pages at.
    tcp(uapi::TCP_ZEROCOPY_RECEIVE, Unknown, Follows, TCP),
    Entry {
        level: Some(uapi::SOL_MPTCP),
        option: local::MPTCP_INFO,
        set: Unknown,
        get: WritesWithin,
        source: MPTCP,
    },
    Entry {
        level: Some(uapi::SOL_MPTCP),
        option: local::MPTCP_FULL_INFO,
        set: Unknown,
        get: WritesThroughFullInfo,
        source: MPTCP,
    },
    Entry {
        level: Some(uapi::SOL_UDP),
        option: local::UDP_NO_CHECK6_RX,
        set: ReadsNumber,
        get: Unknown,
        source: UDP,
    },
    Entry {
        level: Some(uapi::SOL_UDP),
        option: local::UDP_NO_CHECK6_TX,
        set: ReadsNumber,
        get: Unknown,
        source: UDP,
    },
    Entry {
        level: Some(local::SOL_UDPLITE),
        option: local::UDPLITE_RECV_CSCOV,
        set: ReadsNumber,
        get: Unknown,
        source: UDP,
    },
    Entry {
        level: Some(local::SOL_UDPLITE),
        option: local::UDPLITE_SEND_CSCOV,
        set: ReadsNumber,
        get: Unknown,
        source: UDP,
    },
    request(local::SIOCOUTQ, WritesInt),
    request(local::SIOCOUTQNSD, WritesInt),
    request(local::SIOCATMARK, WritesInt),
    // The old form writes `struct __kernel_old_timeval`, sixteen bytes on a 64-bit machine.
    request(
        uapi::SIOCGSTAMP_OLD,
        WritesStructure("__kernel_old_timeval"),
    ),
];

/// Gives the number a level's name resolves to, if it is a level a row may name.
pub fn level(path: &str) -> Option<&'static Level> {
    LEVELS.iter().find(|level| level.name.path == path)
}

/// Gives the entry a row's level and option names resolve to, or why they resolve to none.
///
/// `level` is none for an ioctl request. The option is looked up only among its own level's names.
pub fn resolve(level_path: Option<&str>, option_path: &str) -> Result<&'static Entry, String> {
    let (level_number, names): (Option<u32>, &[Name]) = match level_path {
        None => (None, &REQUESTS),
        Some(path) => {
            let found =
                level(path).ok_or_else(|| format!("{path} is not a level a row may name"))?;
            (Some(found.name.number), found.options)
        }
    };
    let option = names
        .iter()
        .find(|name| name.path == option_path)
        .ok_or_else(|| {
            format!(
                "{option_path} is not a name of the number space of {}",
                level_path.unwrap_or("ioctl requests")
            )
        })?;
    ENTRIES
        .iter()
        .find(|entry| entry.level == level_number && entry.option == option.number)
        .ok_or_else(|| {
            format!(
                "no kernel entry for level {level_number:?} option {} ({option_path})",
                option.number
            )
        })
}
