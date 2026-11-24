//! The state readings of a Linux TCP or MPTCP socket and the repair settings that write it back.

use std::ffi::c_int;
use std::mem::size_of;
use std::net::IpAddr;
use std::os::fd::BorrowedFd;
use std::sync::OnceLock;
use std::time::Duration;

use linux_raw_sys::net as uapi;
use rustix::net::{ipproto, sockopt};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    ByteCount, BytesPerSecond, IpVersion, MaximumTransmissionUnit, ProcessorIndex,
};
use wcontainers_internet_protocol_linux_socket_options::datagram::addresses::{
    StorageBytes, address_of_storage,
};
use wcontainers_internet_protocol_linux_socket_options::datagram::receive::Ancillary;
use wcontainers_internet_protocol_linux_socket_options::{
    self as options, NO_PROCESSOR, QueuedCount, Shortfalls, get_int, host_error, set_int,
    set_option,
};
use wcontainers_tcp_sockets::authentication::{
    SequenceNumberExtension, TcpAuthenticationCounters, TcpAuthenticationIcmpErrors,
    TcpAuthenticationKeyCounters, TcpAuthenticationKeyIdentifier, TcpAuthenticationKeyIdentity,
    TcpAuthenticationRepair, TcpAuthenticationRequirement, TcpAuthenticationStatus,
};
use wcontainers_tcp_sockets::multipath::{
    DataSequenceNumber, MultipathInformation, MultipathSubflow, MultipathSubflowIdentifier,
    MultipathSubflowPosition, MultipathToken, SubflowCount,
};
use wcontainers_tcp_sockets::repair::{
    TcpRepairOption, TcpRepairState, TcpRepairWindow, TcpTimestamp, TcpTimestampClock,
    TcpTimestampUnit, TcpWindowScales,
};
use wcontainers_tcp_sockets::settings::{
    MaximumSegmentSize, ProbeCount, RetryCount, SegmentCount, TcpSequenceNumber, WindowScale,
    WindowSize,
};
use wcontainers_tcp_sockets::state::{
    AccurateEcnFailures, AccurateEcnOptionSeen, ByteTotal, CongestionControlInformation,
    CongestionFraction, CongestionGain, DeliveryRateLimit, EcnByteCounts, EcnMode, EventCount,
    TcpCongestionAvoidanceState, TcpFastOpenClientFailure, TcpInformation, TcpNegotiatedOptions,
    TcpQueues, TcpReading, TcpState, TcpTuned, UrgentMark,
};
use zerocopy::{FromBytes, FromZeros, Immutable, IntoBytes, KnownLayout};

use super::{ONE_KEY, authentication_key_name, invalid, key_query};

/// The reasons a reading is incomplete or refused, each counted by [`Shortfalls`].
const SHORT_INFORMATION: &str =
    "the host's TCP_INFO is older than the contract's: the fields it lacks are given as zero";
const SHORT_MULTIPATH_INFORMATION: &str =
    "the host's MPTCP_INFO is older than the contract's: the fields it lacks are given as zero";
const UNKNOWN_STATE: &str =
    "the host reported a TCP state or congestion state the contract has no name for";
const NO_KERNEL_VERSION: &str = "the host's kernel version could not be read, which alone tells how TCP_INFO's last word is laid out";
const NO_ECN_MODE: &str = "Linux 6.18 and 6.19 report no ECN mode and flag Classic and Accurate ECN alike, so a connection using ECN cannot be read";
const UNKNOWN_CONGESTION_INFORMATION: &str =
    "the host reported congestion control information of a form the contract does not know";
const NO_RECORDED_IPV4_OPTIONS: &str = "Linux records no IPv4 options of the last packet a TCP socket received, so its header bytes are given as none";
const RECORDS_FILL_THE_ROOM: &str = "the records of the last packet a TCP socket received filled the room for them, so Linux may have left some out";
const HEADERS_CUT: &str = "the buffer given for the last packet's extension headers was shorter than the headers Linux recorded";
const SUBFLOWS_PAST_THE_ROOM: &str =
    "a subflow past those the in-kernel path manager allows is read through arrays on the heap";
const UNFILLED_INFORMATION: &str = "Linux fills neither tcpi_last_ack_sent nor tcpi_fackets: the time since the last acknowledgement sent and the forward-acknowledged count are given as zero";
const NO_MULTIPATH_URGENT_MARK: &str = "Linux keeps no urgent mark for a multipath socket, which takes no urgent data (SIOCATMARK is not an MPTCP ioctl): it reads not reached";
const NO_MULTIPATH_CLAMP: &str = "Linux reads no window clamp or quick acknowledgement mode of a multipath socket (TCP_WINDOW_CLAMP and TCP_QUICKACK answer EOPNOTSUPP), and the tuned values need both";
const NO_FULLMESH_LIMIT: &str = "the host's MPTCP_INFO gives the limit of fullmesh endpoints, which the contract's multipath information has no field for";

/// The TCP states in the order Linux numbers them from `TCP_ESTABLISHED`, which is one, less `TCP_NEW_SYN_RECV` and
/// `TCP_BOUND_INACTIVE`, which no socket a program holds is in.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/net/tcp_states.h
const STATES: [TcpState; 11] = [
    TcpState::Established,
    TcpState::SynchronizeSent,
    TcpState::SynchronizeReceived,
    TcpState::FinWait1,
    TcpState::FinWait2,
    TcpState::TimeWait,
    TcpState::Closed,
    TcpState::CloseWait,
    TcpState::LastAcknowledgement,
    TcpState::Listen,
    TcpState::Closing,
];

/// `TCP_ESTABLISHED`, the number of the first of [`STATES`].
///
/// https://github.com/torvalds/linux/blob/v6.18/include/net/tcp_states.h
const FIRST_STATE: u8 = 1;

/// The `tcpi_accecn_fail_mode` bit of an ACE field that failed in sending.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/net/tcp_ecn.h
const ACE_FAIL_SEND: u16 = 1 << 0;
/// The `tcpi_accecn_fail_mode` bit of an ACE field that failed in receiving.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/net/tcp_ecn.h
const ACE_FAIL_RECEIVE: u16 = 1 << 1;
/// The `tcpi_accecn_fail_mode` bit of an Accurate ECN option that failed in sending.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/net/tcp_ecn.h
const OPTION_FAIL_SEND: u16 = 1 << 2;
/// The `tcpi_accecn_fail_mode` bit of an Accurate ECN option that failed in receiving.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/net/tcp_ecn.h
const OPTION_FAIL_RECEIVE: u16 = 1 << 3;

/// The `tcpi_accecn_opt_seen` values in their order: none seen, an empty option, a counter, a failure.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/net/tcp_ecn.h
const OPTIONS_SEEN: [AccurateEcnOptionSeen; 4] = [
    AccurateEcnOptionSeen::NotSeen,
    AccurateEcnOptionSeen::EmptySeen,
    AccurateEcnOptionSeen::CounterSeen,
    AccurateEcnOptionSeen::FailSeen,
];

/// The fields `TCP_INFO` gives after `tcpi_total_rto_time`, which linux-raw-sys does not carry yet.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/tcp.h#L337-L346
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/tcp.h#L337-L347
#[repr(C)]
#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
struct AccurateEcnInformation {
    received_ce: u32,
    delivered_e1_bytes: u32,
    delivered_e0_bytes: u32,
    delivered_ce_bytes: u32,
    received_e1_bytes: u32,
    received_e0_bytes: u32,
    received_ce_bytes: u32,
    /// `tcpi_accecn_fail_mode` then `tcpi_accecn_opt_seen` as two `u16`s on Linux 6.18 and 6.19; from 7.0 one `u32` of
    /// bit fields from `tcpi_ecn_mode`, which [`AccurateEcnLayout`] tells apart.
    last: [u8; 4],
}

/// How the last word of a `TCP_INFO` with the Accurate ECN fields is laid out, which only the kernel's version tells.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AccurateEcnLayout {
    /// Linux 6.18 and 6.19: two `u16`s and no ECN mode.
    Halves,
    /// Linux 7.0 and later: bit fields, the ECN mode first.
    BitFields,
}

/// The first Linux whose last `TCP_INFO` word holds `tcpi_ecn_mode`, by major and minor version.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/tcp.h#L344-L347
const ECN_MODE_SINCE: (u32, u32) = (7, 0);

/// The bit fields of the last `TCP_INFO` word from Linux 7.0, in order: `tcpi_ecn_mode:2`, `tcpi_accecn_opt_seen:2` and
/// `tcpi_accecn_fail_mode:4`.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/tcp.h#L344-L347
const ECN_MODE_BITS: u32 = 2;
const OPTION_SEEN_BITS: u32 = 2;
const FAIL_MODE_BITS: u32 = 4;

/// The ECN modes in the order `TCPI_ECN_MODE_*` numbers them from zero.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/tcp.h#L229-L233
const ECN_MODES: [EcnMode; 4] = [
    EcnMode::Disabled,
    EcnMode::Classic,
    EcnMode::Accurate,
    EcnMode::Pending,
];

/// Gives the `bits` of a native-endian `word` after the first `shift`, counted as GCC lays out bit fields: from the
/// lowest bit on a little-endian host, from the highest on a big-endian one.
fn bit_field(word: u32, shift: u32, bits: u32) -> u32 {
    let from_lowest = if cfg!(target_endian = "big") {
        u32::BITS - shift - bits
    } else {
        shift
    };
    word >> from_lowest & ((1 << bits) - 1)
}

/// Tells how the running kernel lays out the last `TCP_INFO` word, none when its version cannot be read.
fn accurate_ecn_layout() -> Option<AccurateEcnLayout> {
    static LAYOUT: OnceLock<Option<AccurateEcnLayout>> = OnceLock::new();
    *LAYOUT.get_or_init(|| {
        let name = rustix::system::uname();
        let release = name.release().to_str().ok()?;
        let mut numbers = release
            .split(|character: char| !character.is_ascii_digit())
            .map(str::parse::<u32>);
        let version = (numbers.next()?.ok()?, numbers.next()?.ok()?);
        Some(if version >= ECN_MODE_SINCE {
            AccurateEcnLayout::BitFields
        } else {
            AccurateEcnLayout::Halves
        })
    })
}

options::uapi_mirror! {
    /// `struct tcp_info` as linux-raw-sys carries it.
    #[derive(IntoBytes)]
    struct TcpInfo mirrors uapi::tcp_info {
        tcpi_state: u8,
        tcpi_ca_state: u8,
        tcpi_retransmits: u8,
        tcpi_probes: u8,
        tcpi_backoff: u8,
        tcpi_options: u8,
        /// The window scales, the application limit and the fast open failure, read by [`TcpInfo::bit_field`].
        _bitfield_1: [u8; 2],
        tcpi_rto: u32,
        tcpi_ato: u32,
        tcpi_snd_mss: u32,
        tcpi_rcv_mss: u32,
        tcpi_unacked: u32,
        tcpi_sacked: u32,
        tcpi_lost: u32,
        tcpi_retrans: u32,
        tcpi_fackets: u32,
        tcpi_last_data_sent: u32,
        tcpi_last_ack_sent: u32,
        tcpi_last_data_recv: u32,
        tcpi_last_ack_recv: u32,
        tcpi_pmtu: u32,
        tcpi_rcv_ssthresh: u32,
        tcpi_rtt: u32,
        tcpi_rttvar: u32,
        tcpi_snd_ssthresh: u32,
        tcpi_snd_cwnd: u32,
        tcpi_advmss: u32,
        tcpi_reordering: u32,
        tcpi_rcv_rtt: u32,
        tcpi_rcv_space: u32,
        tcpi_total_retrans: u32,
        tcpi_pacing_rate: u64,
        tcpi_max_pacing_rate: u64,
        tcpi_bytes_acked: u64,
        tcpi_bytes_received: u64,
        tcpi_segs_out: u32,
        tcpi_segs_in: u32,
        tcpi_notsent_bytes: u32,
        tcpi_min_rtt: u32,
        tcpi_data_segs_in: u32,
        tcpi_data_segs_out: u32,
        tcpi_delivery_rate: u64,
        tcpi_busy_time: u64,
        tcpi_rwnd_limited: u64,
        tcpi_sndbuf_limited: u64,
        tcpi_delivered: u32,
        tcpi_delivered_ce: u32,
        tcpi_bytes_sent: u64,
        tcpi_bytes_retrans: u64,
        tcpi_dsack_dups: u32,
        tcpi_reord_seen: u32,
        tcpi_rcv_ooopack: u32,
        tcpi_snd_wnd: u32,
        tcpi_rcv_wnd: u32,
        tcpi_rehash: u32,
        tcpi_total_rto: u16,
        tcpi_total_rto_recoveries: u16,
        tcpi_total_rto_time: u32,
    }
}

options::uapi_mirror! {
    /// `struct tcp_ao_info_opt`, given as linux-raw-sys's by [`authentication_information`] for its bit field accessors.
    #[derive(IntoBytes)]
    pub(super) struct AuthenticationInformation mirrors uapi::tcp_ao_info_opt {
        _bitfield_1: [u8; 4],
        reserved2: u16,
        current_key: u8,
        rnext: u8,
        pkt_good: u64,
        pkt_bad: u64,
        pkt_key_not_found: u64,
        pkt_ao_required: u64,
        pkt_dropped_icmp: u64,
    }
}

impl From<&uapi::tcp_ao_info_opt> for AuthenticationInformation {
    fn from(held: &uapi::tcp_ao_info_opt) -> Self {
        Self {
            _bitfield_1: options::requests::bit_field_bytes(&held._bitfield_1),
            reserved2: held.reserved2,
            current_key: held.current_key,
            rnext: held.rnext,
            pkt_good: held.pkt_good,
            pkt_bad: held.pkt_bad,
            pkt_key_not_found: held.pkt_key_not_found,
            pkt_ao_required: held.pkt_ao_required,
            pkt_dropped_icmp: held.pkt_dropped_icmp,
        }
    }
}

/// Reads the socket's `TCP_AO_INFO`.
pub(super) fn authentication_information(
    socket: BorrowedFd<'_>,
) -> Result<uapi::tcp_ao_info_opt, HostError> {
    let found: AuthenticationInformation =
        options::get_option(socket, uapi::SOL_TCP, uapi::TCP_AO_INFO)?;
    Ok(uapi::tcp_ao_info_opt {
        _bitfield_align_1: [],
        _bitfield_1: uapi::__BindgenBitfieldUnit::new(found._bitfield_1),
        reserved2: found.reserved2,
        current_key: found.current_key,
        rnext: found.rnext,
        pkt_good: found.pkt_good,
        pkt_bad: found.pkt_bad,
        pkt_key_not_found: found.pkt_key_not_found,
        pkt_ao_required: found.pkt_ao_required,
        pkt_dropped_icmp: found.pkt_dropped_icmp,
    })
}

options::uapi_mirror! {
    /// `struct tcp_ao_repair`, its initial sequence numbers in network order.
    #[derive(IntoBytes)]
    struct AuthenticationRepair mirrors uapi::tcp_ao_repair {
        snt_isn: u32,
        rcv_isn: u32,
        snd_sne: u32,
        rcv_sne: u32,
    }
}

options::uapi_mirror! {
    /// `struct tcp_repair_window`.
    #[derive(IntoBytes)]
    struct RepairWindow mirrors uapi::tcp_repair_window {
        snd_wl1: u32,
        snd_wnd: u32,
        max_window: u32,
        rcv_wnd: u32,
        rcv_wup: u32,
    }
}

/// Tells whether the socket's TCP timestamps count microseconds, from its `TCP_INFO`.
pub(super) fn microsecond_timestamps(socket: BorrowedFd<'_>) -> Result<bool, HostError> {
    let information: TcpInfo = options::get_option(socket, uapi::SOL_TCP, uapi::TCP_INFO)?;
    Ok(u32::from(information.tcpi_options) & uapi::TCPI_OPT_USEC_TS != u32::MIN)
}

/// The bit fields of `tcp_info` by their first bit and width: `tcpi_snd_wscale:4`, `tcpi_rcv_wscale:4`,
/// `tcpi_delivery_rate_app_limited:1` and `tcpi_fastopen_client_fail:2`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/tcp.h#:~:text=tcpi_snd_wscale
const SEND_WINDOW_SCALE: (usize, u8) = (0, 4);
const RECEIVE_WINDOW_SCALE: (usize, u8) = (4, 4);
const DELIVERY_RATE_APPLICATION_LIMITED: (usize, u8) = (8, 1);
const FAST_OPEN_CLIENT_FAILURE: (usize, u8) = (9, 2);

impl TcpInfo {
    /// Reads one bit field of at most eight bits as linux-raw-sys's own accessors do.
    fn bit_field(&self, (first, width): (usize, u8)) -> u8 {
        uapi::__BindgenBitfieldUnit::new(self._bitfield_1).get(first, width) as u8
    }
}

/// The whole `TCP_INFO` the contract reads: linux-raw-sys's structure and the fields after `tcpi_total_rto_time` it lacks.
#[repr(C)]
#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
struct InformationBytes {
    info: TcpInfo,
    ecn: AccurateEcnInformation,
}

/// The length of the whole `TCP_INFO` the contract reads.
const INFORMATION_LENGTH: usize = size_of::<InformationBytes>();

/// The `TCP_CC_INFO` forms of `tcp_cc_info`: Vegas's (also Westwood's and Illinois'), DCTCP's and BBR's.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/inet_diag.h
#[repr(C)]
#[derive(Clone, Copy, FromBytes, KnownLayout, Immutable)]
struct VegasInformation {
    enabled: u32,
    round_trip_time_count: u32,
    round_trip_time: u32,
    minimum_round_trip_time: u32,
}

/// DCTCP's `tcp_dctcp_info`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/inet_diag.h
#[repr(C)]
#[derive(Clone, Copy, FromBytes, KnownLayout, Immutable)]
struct DataCenterInformation {
    enabled: u16,
    congestion_experienced: u16,
    alpha: u32,
    acknowledged_bytes_marked: u32,
    acknowledged_bytes_total: u32,
}

/// BBR's `tcp_bbr_info`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/inet_diag.h
#[repr(C)]
#[derive(Clone, Copy, FromBytes, KnownLayout, Immutable)]
struct BottleneckBandwidthInformation {
    bandwidth_low: u32,
    bandwidth_high: u32,
    minimum_round_trip_time: u32,
    pacing_gain: u32,
    congestion_window_gain: u32,
}

/// The room `TCP_CC_INFO` is read into: that of its longest form.
const CONGESTION_INFORMATION_ROOM: usize = size_of::<BottleneckBandwidthInformation>();

/// The congestion control whose information has DCTCP's form, which has the length of Vegas's.
const DATA_CENTER: &str = "dctcp";

/// How many subflows one `MPTCP_FULL_INFO` read on the stack holds: the first and the eight more the in-kernel path manager allows.
///
/// https://man7.org/linux/man-pages/man8/ip-mptcp.8.html
const SUBFLOW_ROOM: usize = 9;

/// The extension headers Linux records of one IPv6 packet, in RFC 8200's order: hop-by-hop, destination, routing, destination.
///
/// https://www.rfc-editor.org/rfc/rfc8200#section-4.1
const RECORDED_EXTENSION_HEADERS: usize = 4;
/// The integer records Linux gives a TCP socket of its last packet: hop limit or TTL, traffic class or TOS, flow label.
///
/// https://github.com/torvalds/linux/blob/v6.18/net/ipv6/datagram.c
const RECORDED_INTEGERS: usize = 3;

use options::control_space as space;

/// Room for every record of the last packet: its packet information, integer records and extension headers at their largest.
const LAST_HEADER_ROOM: usize = space(size_of::<uapi::in6_pktinfo>())
    + RECORDED_INTEGERS * space(size_of::<c_int>())
    + RECORDED_EXTENSION_HEADERS * space(etherparse::Ipv6RawExtHeader::MAX_LEN);

/// The room the last packet's records are read into, aligned as their control message headers are.
#[repr(C)]
struct LastHeaderRecords {
    _aligned: [libc::cmsghdr; usize::MIN],
    bytes: [u8; LAST_HEADER_ROOM],
}

/// The `mptcpi_flags` bit of a connection that fell back to TCP.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
const FELL_BACK: u32 = 1 << 0;
/// The `mptcpi_flags` bit of a connection that received its peer's key.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
const REMOTE_KEY_RECEIVED: u32 = 1 << 1;

/// `struct mptcp_info`, which neither libc nor linux-raw-sys carries.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
#[repr(C)]
#[derive(Clone, Copy, FromBytes, KnownLayout, Immutable)]
struct MptcpInfo {
    subflows: u8,
    add_addr_signal: u8,
    add_addr_accepted: u8,
    subflows_max: u8,
    add_addr_signal_max: u8,
    add_addr_accepted_max: u8,
    flags: u32,
    token: u32,
    write_seq: u64,
    snd_una: u64,
    rcv_nxt: u64,
    local_addr_used: u8,
    local_addr_max: u8,
    csum_enabled: u8,
    retransmits: u32,
    bytes_retrans: u64,
    bytes_sent: u64,
    bytes_received: u64,
    bytes_acked: u64,
    subflows_total: u8,
    endp_laminar_max: u8,
    /// `mptcpi_endp_fullmesh_max`, in Linux 6.19 and later, a reserved zero before.
    ///
    /// https://github.com/torvalds/linux/blob/v6.19/include/uapi/linux/mptcp.h
    endp_fullmesh_max: u8,
    reserved: u8,
    last_data_sent: u32,
    last_data_recv: u32,
    last_ack_recv: u32,
}

/// `struct mptcp_subflow_info`: a subflow's identifier and its local and remote addresses.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
#[repr(C)]
#[derive(Clone, Copy, FromBytes, KnownLayout, Immutable)]
struct MptcpSubflowInfo {
    id: u32,
    local: StorageBytes,
    remote: StorageBytes,
}

/// The shift of the receive scale in a `TCPOPT_WINDOW` repair option's value, the send scale being below it.
///
/// https://github.com/torvalds/linux/blob/v6.18/net/ipv4/tcp.c
const RECEIVE_SCALE_SHIFT: u32 = u16::BITS;

/// The value of a repair option that carries none: selective acknowledgements and timestamps.
///
/// https://github.com/torvalds/linux/blob/v6.18/net/ipv4/tcp.c
const NO_OPTION_VALUE: u32 = u32::MIN;

/// How many repair options one `TCP_REPAIR_OPTIONS` write gives, as many as there are kinds; Linux applies each on its own.
///
/// https://github.com/torvalds/linux/blob/v6.18/net/ipv4/tcp.c
const REPAIR_OPTIONS_AT_ONCE: usize = 4;

/// `SIOCATMARK`'s answer when the next byte to read is not the urgent mark.
///
/// https://man7.org/linux/man-pages/man7/tcp.7.html
const NOT_AT_MARK: c_int = 0;

fn overflow() -> HostError {
    HostError::Errno(libc::EOVERFLOW)
}

/// Gives a host value in the contract's type, or Linux's `EOVERFLOW` when it does not fit.
fn narrow<T: TryFrom<u64>>(value: impl TryInto<u64>) -> Result<T, HostError> {
    value
        .try_into()
        .ok()
        .and_then(|value| T::try_from(value).ok())
        .ok_or_else(overflow)
}

fn microseconds(value: impl Into<u64>) -> Duration {
    Duration::from_micros(value.into())
}

fn milliseconds(value: impl Into<u64>) -> Duration {
    Duration::from_millis(value.into())
}

fn congestion_state(state: u8) -> Option<TcpCongestionAvoidanceState> {
    use uapi::tcp_ca_state::{
        TCP_CA_CWR, TCP_CA_Disorder, TCP_CA_Loss, TCP_CA_Open, TCP_CA_Recovery,
    };
    [
        (TCP_CA_Open, TcpCongestionAvoidanceState::Open),
        (TCP_CA_Disorder, TcpCongestionAvoidanceState::Disorder),
        (
            TCP_CA_CWR,
            TcpCongestionAvoidanceState::CongestionWindowReduced,
        ),
        (TCP_CA_Recovery, TcpCongestionAvoidanceState::Recovery),
        (TCP_CA_Loss, TcpCongestionAvoidanceState::Loss),
    ]
    .into_iter()
    .find_map(|(linux, ours)| (linux as u32 == u32::from(state)).then_some(ours))
}

/// Reads `TCP_INFO` into bytes, counting a shorter one, which leaves the fields it lacks zero, and tells its layout.
///
/// One without the Accurate ECN fields is of a Linux before 6.18, which has no Accurate ECN.
fn information_bytes(
    socket: BorrowedFd<'_>,
    shortfalls: &Shortfalls,
) -> Result<(InformationBytes, Option<AccurateEcnLayout>), HostError> {
    let mut bytes = InformationBytes::new_zeroed();
    let length = options::get_bytes(socket, uapi::SOL_TCP, uapi::TCP_INFO, bytes.as_mut_bytes())?;
    if length < INFORMATION_LENGTH {
        shortfalls.count(SHORT_INFORMATION);
        return Ok((bytes, None));
    }
    let layout = accurate_ecn_layout().ok_or_else(|| shortfalls.refuse(NO_KERNEL_VERSION))?;
    Ok((bytes, Some(layout)))
}

/// Gives the contract's `TCP_INFO` from the bytes Linux wrote.
fn information_of(
    bytes: &InformationBytes,
    layout: Option<AccurateEcnLayout>,
    shortfalls: &Shortfalls,
) -> Result<TcpInformation, HostError> {
    let InformationBytes { info, ecn } = *bytes;
    shortfalls.count(UNFILLED_INFORMATION);
    let classic = u32::from(info.tcpi_options) & uapi::TCPI_OPT_ECN != u32::MIN;
    let word = u32::from_ne_bytes(ecn.last);
    let [first, second] = [[ecn.last[0], ecn.last[1]], [ecn.last[2], ecn.last[3]]];
    let (ecn_mode, fail_mode, option_seen) = match layout {
        None if classic => (EcnMode::Classic, u16::MIN, u16::MIN),
        None => (EcnMode::Disabled, u16::MIN, u16::MIN),
        Some(AccurateEcnLayout::BitFields) => (
            *ECN_MODES
                .get(bit_field(word, u32::MIN, ECN_MODE_BITS) as usize)
                .ok_or_else(|| shortfalls.refuse(UNKNOWN_STATE))?,
            bit_field(word, ECN_MODE_BITS + OPTION_SEEN_BITS, FAIL_MODE_BITS) as u16,
            bit_field(word, ECN_MODE_BITS, OPTION_SEEN_BITS) as u16,
        ),
        // `TCPI_OPT_ECN` is set for Classic and Accurate ECN alike there.
        Some(AccurateEcnLayout::Halves) if classic => {
            return Err(shortfalls.refuse(NO_ECN_MODE));
        }
        Some(AccurateEcnLayout::Halves) => (
            EcnMode::Disabled,
            u16::from_ne_bytes(first),
            u16::from_ne_bytes(second),
        ),
    };
    let state = info
        .tcpi_state
        .checked_sub(FIRST_STATE)
        .and_then(|index| STATES.get(usize::from(index)))
        .copied()
        .ok_or_else(|| shortfalls.refuse(UNKNOWN_STATE))?;
    let congestion_avoidance_state =
        congestion_state(info.tcpi_ca_state).ok_or_else(|| shortfalls.refuse(UNKNOWN_STATE))?;
    let option = |flag: u32| u32::from(info.tcpi_options) & flag != u32::MIN;
    let fast_open_client_failure = {
        use uapi::tcp_fastopen_client_fail::{
            TFO_COOKIE_UNAVAILABLE, TFO_DATA_NOT_ACKED, TFO_SYN_RETRANSMITTED,
        };
        match u32::from(info.bit_field(FAST_OPEN_CLIENT_FAILURE)) {
            failure if failure == TFO_COOKIE_UNAVAILABLE as u32 => {
                TcpFastOpenClientFailure::CookieUnavailable
            }
            failure if failure == TFO_DATA_NOT_ACKED as u32 => {
                TcpFastOpenClientFailure::DataNotAcknowledged
            }
            failure if failure == TFO_SYN_RETRANSMITTED as u32 => {
                TcpFastOpenClientFailure::SynchronizeRetransmitted
            }
            _ => TcpFastOpenClientFailure::Unspecified,
        }
    };
    let segments = |count: u32| SegmentCount(u64::from(count));
    let bytes_total = |count: u32| ByteTotal(u64::from(count));
    Ok(TcpInformation {
        state,
        congestion_avoidance_state,
        retransmits: RetryCount(info.tcpi_retransmits),
        probes: ProbeCount(info.tcpi_probes),
        backoff: RetryCount(info.tcpi_backoff),
        options: TcpNegotiatedOptions {
            timestamps: option(uapi::TCPI_OPT_TIMESTAMPS),
            selective_acknowledgements: option(uapi::TCPI_OPT_SACK),
            window_scaling: option(uapi::TCPI_OPT_WSCALE),
            explicit_congestion_notification: ecn_mode,
            explicit_congestion_notification_seen: option(uapi::TCPI_OPT_ECN_SEEN),
            synchronize_data_acknowledged: option(uapi::TCPI_OPT_SYN_DATA),
            microsecond_timestamps: option(uapi::TCPI_OPT_USEC_TS),
            fast_open_child: option(uapi::TCPI_OPT_TFO_CHILD),
        },
        window_scales: TcpWindowScales {
            send: WindowScale(info.bit_field(SEND_WINDOW_SCALE)),
            receive: WindowScale(info.bit_field(RECEIVE_WINDOW_SCALE)),
        },
        delivery_rate_limit: if info.bit_field(DELIVERY_RATE_APPLICATION_LIMITED) == u8::MIN {
            DeliveryRateLimit::Network
        } else {
            DeliveryRateLimit::Application
        },
        fast_open_client_failure,
        retransmission_timeout: microseconds(info.tcpi_rto),
        acknowledgement_timeout: microseconds(info.tcpi_ato),
        send_maximum_segment_size: MaximumSegmentSize(narrow(info.tcpi_snd_mss)?),
        receive_maximum_segment_size: MaximumSegmentSize(narrow(info.tcpi_rcv_mss)?),
        lost: segments(info.tcpi_lost),
        retransmitted: segments(info.tcpi_retrans),
        forward_acknowledged: segments(info.tcpi_fackets),
        since_last_data_sent: milliseconds(info.tcpi_last_data_sent),
        since_last_acknowledgement_sent: milliseconds(info.tcpi_last_ack_sent),
        since_last_data_received: milliseconds(info.tcpi_last_data_recv),
        since_last_acknowledgement_received: milliseconds(info.tcpi_last_ack_recv),
        path_mtu: MaximumTransmissionUnit(info.tcpi_pmtu),
        receive_slow_start_threshold: WindowSize(info.tcpi_rcv_ssthresh),
        round_trip_time: microseconds(info.tcpi_rtt),
        round_trip_time_variance: microseconds(info.tcpi_rttvar),
        send_slow_start_threshold: segments(info.tcpi_snd_ssthresh),
        send_congestion_window: segments(info.tcpi_snd_cwnd),
        advertised_maximum_segment_size: MaximumSegmentSize(narrow(info.tcpi_advmss)?),
        reordering: segments(info.tcpi_reordering),
        receive_round_trip_time: microseconds(info.tcpi_rcv_rtt),
        receive_space: ByteCount(narrow(info.tcpi_rcv_space)?),
        total_retransmissions: segments(info.tcpi_total_retrans),
        pacing_rate: BytesPerSecond(info.tcpi_pacing_rate),
        maximum_pacing_rate: BytesPerSecond(info.tcpi_max_pacing_rate),
        bytes_acknowledged: ByteTotal(info.tcpi_bytes_acked),
        bytes_received: ByteTotal(info.tcpi_bytes_received),
        segments_out: segments(info.tcpi_segs_out),
        segments_in: segments(info.tcpi_segs_in),
        not_sent: ByteCount(narrow(info.tcpi_notsent_bytes)?),
        minimum_round_trip_time: microseconds(info.tcpi_min_rtt),
        data_segments_in: segments(info.tcpi_data_segs_in),
        data_segments_out: segments(info.tcpi_data_segs_out),
        delivery_rate: BytesPerSecond(info.tcpi_delivery_rate),
        busy: microseconds(info.tcpi_busy_time),
        receive_window_limited: microseconds(info.tcpi_rwnd_limited),
        send_buffer_limited: microseconds(info.tcpi_sndbuf_limited),
        delivered: segments(info.tcpi_delivered),
        delivered_congestion_experienced: segments(info.tcpi_delivered_ce),
        bytes_sent: ByteTotal(info.tcpi_bytes_sent),
        bytes_retransmitted: ByteTotal(info.tcpi_bytes_retrans),
        duplicate_selective_acknowledgements: segments(info.tcpi_dsack_dups),
        reordering_seen: EventCount(info.tcpi_reord_seen),
        received_out_of_order: segments(info.tcpi_rcv_ooopack),
        send_window: WindowSize(info.tcpi_snd_wnd),
        receive_window: WindowSize(info.tcpi_rcv_wnd),
        rehashes: EventCount(info.tcpi_rehash),
        retransmission_timeouts: EventCount(u32::from(info.tcpi_total_rto)),
        retransmission_timeout_recoveries: EventCount(u32::from(info.tcpi_total_rto_recoveries)),
        in_retransmission_timeout_recovery: milliseconds(info.tcpi_total_rto_time),
        received_congestion_experienced: segments(ecn.received_ce),
        delivered_ecn_bytes: EcnByteCounts {
            capable_transport_1: bytes_total(ecn.delivered_e1_bytes),
            capable_transport_0: bytes_total(ecn.delivered_e0_bytes),
            congestion_experienced: bytes_total(ecn.delivered_ce_bytes),
        },
        received_ecn_bytes: EcnByteCounts {
            capable_transport_1: bytes_total(ecn.received_e1_bytes),
            capable_transport_0: bytes_total(ecn.received_e0_bytes),
            congestion_experienced: bytes_total(ecn.received_ce_bytes),
        },
        accurate_ecn_failures: AccurateEcnFailures {
            ace_field_sending: fail_mode & ACE_FAIL_SEND != u16::MIN,
            ace_field_receiving: fail_mode & ACE_FAIL_RECEIVE != u16::MIN,
            option_sending: fail_mode & OPTION_FAIL_SEND != u16::MIN,
            option_receiving: fail_mode & OPTION_FAIL_RECEIVE != u16::MIN,
        },
        accurate_ecn_option_seen: OPTIONS_SEEN
            .get(usize::from(option_seen))
            .copied()
            .ok_or_else(|| shortfalls.refuse(UNKNOWN_STATE))?,
    })
}

/// Reads the socket's `TCP_INFO`, with its `tcpi_unacked` and `tcpi_sacked`, which a listener gives its accept queue in.
pub(super) fn information(
    socket: BorrowedFd<'_>,
    shortfalls: &Shortfalls,
) -> Result<(TcpInformation, u32, u32), HostError> {
    let (bytes, layout) = information_bytes(socket, shortfalls)?;
    let info = bytes.info;
    Ok((
        information_of(&bytes, layout, shortfalls)?,
        info.tcpi_unacked,
        info.tcpi_sacked,
    ))
}

/// Reads the congestion control's name, cut to the buffer as `TCP_CONGESTION` cuts it, and its `TCP_CC_INFO`.
pub(super) fn congestion_control<'a>(
    socket: BorrowedFd<'_>,
    into: &'a mut [u8],
    shortfalls: &Shortfalls,
) -> Result<(&'a str, Option<CongestionControlInformation>), HostError> {
    let name = socket2::SockRef::from(&socket)
        .tcp_congestion()
        .map_err(|error| options::io_host_error(&error))?;
    let length = name
        .iter()
        .position(|&byte| byte == u8::MIN)
        .unwrap_or(name.len());
    let mut bytes = [u8::MIN; CONGESTION_INFORMATION_ROOM];
    let size = options::get_bytes(socket, uapi::SOL_TCP, uapi::TCP_CC_INFO, &mut bytes)?;
    let information = match (&name[..length], size) {
        (_, usize::MIN) => None,
        (algorithm, _)
            if algorithm == DATA_CENTER.as_bytes()
                && size >= size_of::<DataCenterInformation>() =>
        {
            let (found, _) =
                DataCenterInformation::read_from_prefix(&bytes).map_err(|_| invalid())?;
            Some(CongestionControlInformation::DataCenter {
                enabled: found.enabled != u16::MIN,
                congestion_experienced: found.congestion_experienced != u16::MIN,
                alpha: CongestionFraction(found.alpha),
                acknowledged_bytes_marked: ByteTotal(u64::from(found.acknowledged_bytes_marked)),
                acknowledged_bytes_total: ByteTotal(u64::from(found.acknowledged_bytes_total)),
            })
        }
        // Only BBR's form has its length.
        (_, size) if size == size_of::<BottleneckBandwidthInformation>() => {
            let (found, _) =
                BottleneckBandwidthInformation::read_from_prefix(&bytes).map_err(|_| invalid())?;
            Some(CongestionControlInformation::BottleneckBandwidth {
                bandwidth: BytesPerSecond(
                    u64::from(found.bandwidth_high) << u32::BITS | u64::from(found.bandwidth_low),
                ),
                minimum_round_trip_time: microseconds(found.minimum_round_trip_time),
                pacing_gain: CongestionGain(found.pacing_gain),
                congestion_window_gain: CongestionGain(found.congestion_window_gain),
            })
        }
        (_, size) if size == size_of::<VegasInformation>() => {
            let (found, _) = VegasInformation::read_from_prefix(&bytes).map_err(|_| invalid())?;
            Some(CongestionControlInformation::Vegas {
                enabled: found.enabled != u32::MIN,
                round_trip_time_count: EventCount(found.round_trip_time_count),
                round_trip_time: microseconds(found.round_trip_time),
                minimum_round_trip_time: microseconds(found.minimum_round_trip_time),
            })
        }
        _ => return Err(shortfalls.refuse(UNKNOWN_CONGESTION_INFORMATION)),
    };
    let cut = length.min(into.len());
    into[..cut].copy_from_slice(&name[..cut]);
    let name = std::str::from_utf8(&into[..cut]).map_err(|_| invalid())?;
    Ok((name, information))
}

/// Reads the bytes unread, unacknowledged and unsent (`SIOCINQ`, `SIOCOUTQ`, `SIOCOUTQNSD`) and the urgent mark.
pub(super) fn queues(
    socket: BorrowedFd<'_>,
    shortfalls: &Shortfalls,
) -> Result<TcpQueues, HostError> {
    let count = |count| options::count_queued(socket, count).and_then(narrow::<usize>);
    let queued = count(QueuedCount::SendQueue)?;
    let unsent = count(QueuedCount::Unsent)?;
    let unread = rustix::io::ioctl_fionread(socket).map_err(host_error)?;
    Ok(TcpQueues {
        unread: ByteCount(narrow(unread)?),
        unacknowledged: ByteCount(queued.saturating_sub(unsent)),
        unsent: ByteCount(unsent),
        urgent_mark: match options::count_queued(socket, QueuedCount::AtUrgentMark) {
            Ok(NOT_AT_MARK) => UrgentMark::NotReached,
            Ok(_) => UrgentMark::Reached,
            Err(_) if is_multipath(socket) => {
                shortfalls.count(NO_MULTIPATH_URGENT_MARK);
                UrgentMark::NotReached
            }
            Err(error) => return Err(error),
        },
    })
}

/// Tells whether the socket is a multipath one, whose refusals of TCP's own options have reasons of their own.
fn is_multipath(socket: BorrowedFd<'_>) -> bool {
    sockopt::socket_protocol(socket).is_ok_and(|protocol| protocol == Some(ipproto::MPTCP))
}

/// Reads the values the stack chose for the socket's buffers, low water, segment size, clamp, path MTU and acknowledging.
pub(super) fn tuned(
    socket: BorrowedFd<'_>,
    shortfalls: &Shortfalls,
) -> Result<TcpTuned, HostError> {
    let int = |level, name| get_int(socket, level, name);
    let information = information_bytes(socket, shortfalls)?.0.info;
    Ok(TcpTuned {
        send_buffer: ByteCount(narrow(int(uapi::SOL_SOCKET, uapi::SO_SNDBUF)?)?),
        receive_buffer: ByteCount(narrow(int(uapi::SOL_SOCKET, uapi::SO_RCVBUF)?)?),
        receive_low_water: ByteCount(narrow(int(uapi::SOL_SOCKET, uapi::SO_RCVLOWAT)?)?),
        maximum_segment_size: MaximumSegmentSize(narrow(int(uapi::SOL_TCP, uapi::TCP_MAXSEG)?)?),
        window_clamp: WindowSize(narrow(
            int(uapi::SOL_TCP, uapi::TCP_WINDOW_CLAMP)
                .map_err(|error| multipath_refusal(socket, error, shortfalls))?,
        )?),
        path_mtu: MaximumTransmissionUnit(information.tcpi_pmtu),
        quick_acknowledgement: int(uapi::SOL_TCP, uapi::TCP_QUICKACK)
            .map_err(|error| multipath_refusal(socket, error, shortfalls))?
            != super::UNSET,
    })
}

/// Gives a multipath socket's refusal of the clamp or quick acknowledgement as the counted refusal of the tuned values.
fn multipath_refusal(
    socket: BorrowedFd<'_>,
    error: HostError,
    shortfalls: &Shortfalls,
) -> HostError {
    if is_multipath(socket) {
        shortfalls.refuse(NO_MULTIPATH_CLAMP)
    } else {
        error
    }
}

/// Reads the timestamp clock, in the unit it counts in.
pub(super) fn timestamp(
    socket: BorrowedFd<'_>,
    shortfalls: &Shortfalls,
) -> Result<TcpTimestampClock, HostError> {
    let value: u32 = options::get_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP)?;
    let information = information_bytes(socket, shortfalls)?.0.info;
    Ok(TcpTimestampClock {
        value: TcpTimestamp(value),
        unit: if u32::from(information.tcpi_options) & uapi::TCPI_OPT_USEC_TS != u32::MIN {
            TcpTimestampUnit::Microseconds
        } else {
            TcpTimestampUnit::Milliseconds
        },
    })
}

/// Reads a connection in repair mode: both sequence numbers through the repair queues, the window and the authentication values.
///
/// The repair queue chosen is put back; out of repair mode Linux refuses the reads.
pub(super) fn repair_state(socket: BorrowedFd<'_>) -> Result<TcpRepairState, HostError> {
    let window: RepairWindow = options::get_option(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_WINDOW)?;
    let queue = get_int(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE)?;
    let sequence_of = |chosen: c_int| -> Result<TcpSequenceNumber, HostError> {
        set_int(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE, chosen)?;
        options::get_option(socket, uapi::SOL_TCP, uapi::TCP_QUEUE_SEQ).map(TcpSequenceNumber)
    };
    let sequences = sequence_of(uapi::TCP_SEND_QUEUE as c_int)
        .and_then(|send| Ok((send, sequence_of(uapi::TCP_RECV_QUEUE as c_int)?)));
    set_int(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE, queue)?;
    let (send_sequence, receive_sequence) = sequences?;
    let authentication = match options::get_option::<AuthenticationRepair>(
        socket,
        uapi::SOL_TCP,
        uapi::TCP_AO_REPAIR,
    ) {
        Ok(found) => Some(TcpAuthenticationRepair {
            sent_initial_sequence: TcpSequenceNumber(u32::from_be(found.snt_isn)),
            received_initial_sequence: TcpSequenceNumber(u32::from_be(found.rcv_isn)),
            send_extension: SequenceNumberExtension(found.snd_sne),
            receive_extension: SequenceNumberExtension(found.rcv_sne),
        }),
        Err(HostError::Errno(libc::ENOENT)) => None,
        Err(error) => return Err(error),
    };
    Ok(TcpRepairState {
        send_sequence,
        receive_sequence,
        window: TcpRepairWindow {
            send_update_sequence: TcpSequenceNumber(window.snd_wl1),
            send_window: WindowSize(window.snd_wnd),
            maximum_window: WindowSize(window.max_window),
            receive_window: WindowSize(window.rcv_wnd),
            receive_update_sequence: TcpSequenceNumber(window.rcv_wup),
        },
        authentication,
    })
}

/// The size of one `tcp_repair_opt`, a code and a value.
const REPAIR_OPTION_SIZE: usize = size_of::<uapi::tcp_repair_opt>();

/// Writes repair options through `TCP_REPAIR_OPTIONS`, a few at a time on the stack.
pub(super) fn set_repair_options(
    socket: BorrowedFd<'_>,
    repair: &[TcpRepairOption],
) -> Result<(), HostError> {
    use etherparse::tcp_option::{
        KIND_MAXIMUM_SEGMENT_SIZE, KIND_SELECTIVE_ACK_PERMITTED, KIND_TIMESTAMP, KIND_WINDOW_SCALE,
    };
    for chunk in repair.chunks(REPAIR_OPTIONS_AT_ONCE) {
        let mut written = [u8::MIN; REPAIR_OPTION_SIZE * REPAIR_OPTIONS_AT_ONCE];
        for (to, option) in written
            .as_chunks_mut::<REPAIR_OPTION_SIZE>()
            .0
            .iter_mut()
            .zip(chunk)
        {
            let (kind, value) = match *option {
                TcpRepairOption::MaximumSegmentSize(size) => {
                    (KIND_MAXIMUM_SEGMENT_SIZE, u32::from(size.0))
                }
                TcpRepairOption::WindowScales(scales) => (
                    KIND_WINDOW_SCALE,
                    u32::from(scales.send.0) | u32::from(scales.receive.0) << RECEIVE_SCALE_SHIFT,
                ),
                TcpRepairOption::SelectiveAcknowledgements => {
                    (KIND_SELECTIVE_ACK_PERMITTED, NO_OPTION_VALUE)
                }
                TcpRepairOption::Timestamps => (KIND_TIMESTAMP, NO_OPTION_VALUE),
            };
            let code = std::mem::offset_of!(uapi::tcp_repair_opt, opt_code);
            let at = std::mem::offset_of!(uapi::tcp_repair_opt, opt_val);
            to[code..code + size_of::<u32>()].copy_from_slice(&u32::from(kind).to_ne_bytes());
            to[at..at + size_of::<u32>()].copy_from_slice(&value.to_ne_bytes());
        }
        let used = &written[..chunk.len() * REPAIR_OPTION_SIZE];
        options::set_bytes(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_OPTIONS, used)?;
    }
    Ok(())
}

pub(super) fn set_repair_window(
    socket: BorrowedFd<'_>,
    window: &TcpRepairWindow,
) -> Result<(), HostError> {
    let written = RepairWindow {
        snd_wl1: window.send_update_sequence.0,
        snd_wnd: window.send_window.0,
        max_window: window.maximum_window.0,
        rcv_wnd: window.receive_window.0,
        rcv_wup: window.receive_update_sequence.0,
    };
    set_option(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_WINDOW, &written)
}

/// Writes the TCP Authentication Option's repair values, its initial sequence numbers in network order as Linux keeps them.
pub(super) fn set_authentication_repair(
    socket: BorrowedFd<'_>,
    repair: &TcpAuthenticationRepair,
) -> Result<(), HostError> {
    let written = AuthenticationRepair {
        snt_isn: repair.sent_initial_sequence.0.to_be(),
        rcv_isn: repair.received_initial_sequence.0.to_be(),
        snd_sne: repair.send_extension.0,
        rcv_sne: repair.receive_extension.0,
    };
    set_option(socket, uapi::SOL_TCP, uapi::TCP_AO_REPAIR, &written)
}

/// Reads the TCP Authentication Option's state from `TCP_AO_INFO`; a socket without it answers Linux's `ENOENT`.
pub(super) fn authentication(socket: BorrowedFd<'_>) -> Result<TcpAuthenticationStatus, HostError> {
    let found = authentication_information(socket)?;
    let identifier =
        |set: u32, key: u8| (set != u32::MIN).then_some(TcpAuthenticationKeyIdentifier(key));
    Ok(TcpAuthenticationStatus {
        counters: TcpAuthenticationCounters {
            verified: SegmentCount(found.pkt_good),
            failed: SegmentCount(found.pkt_bad),
            key_not_found: SegmentCount(found.pkt_key_not_found),
            signature_missing: SegmentCount(found.pkt_ao_required),
            icmp_errors_ignored: SegmentCount(found.pkt_dropped_icmp),
        },
        current: identifier(found.set_current(), found.current_key),
        requested_next: identifier(found.set_rnext(), found.rnext),
        requirement: if found.ao_required() == u32::MIN {
            TcpAuthenticationRequirement::Optional
        } else {
            TcpAuthenticationRequirement::Required
        },
        icmp_errors: if found.accept_icmps() == u32::MIN {
            TcpAuthenticationIcmpErrors::Ignored
        } else {
            TcpAuthenticationIcmpErrors::Accepted
        },
    })
}

/// Reads one key's counters through `TCP_AO_GET_KEYS`, none when the socket holds no such key.
pub(super) fn authentication_key(
    socket: BorrowedFd<'_>,
    family: IpVersion,
    identity: &TcpAuthenticationKeyIdentity,
) -> Result<Option<TcpAuthenticationKeyCounters>, HostError> {
    let name = authentication_key_name(family, identity)?;
    let mut asked = key_query(&name);
    match options::get_bytes(
        socket,
        uapi::SOL_TCP,
        uapi::TCP_AO_GET_KEYS,
        asked.as_mut_bytes(),
    ) {
        Ok(_) if asked.nkeys == ONE_KEY => Ok(Some(TcpAuthenticationKeyCounters {
            verified: SegmentCount(asked.pkt_good),
            failed: SegmentCount(asked.pkt_bad),
        })),
        Ok(_) | Err(HostError::Errno(libc::ENOENT)) => Ok(None),
        Err(error) => Err(error),
    }
}

fn multipath_of(found: &MptcpInfo) -> MultipathInformation {
    MultipathInformation {
        extra_subflows: SubflowCount(found.subflows),
        addresses_signalled: SubflowCount(found.add_addr_signal),
        addresses_accepted: SubflowCount(found.add_addr_accepted),
        extra_subflows_limit: SubflowCount(found.subflows_max),
        signal_endpoints_limit: SubflowCount(found.add_addr_signal_max),
        accepted_addresses_limit: SubflowCount(found.add_addr_accepted_max),
        fell_back_to_tcp: found.flags & FELL_BACK != u32::MIN,
        remote_key_received: found.flags & REMOTE_KEY_RECEIVED != u32::MIN,
        token: MultipathToken(found.token),
        write_sequence: DataSequenceNumber(found.write_seq),
        send_unacknowledged: DataSequenceNumber(found.snd_una),
        receive_next: DataSequenceNumber(found.rcv_nxt),
        local_addresses_used: SubflowCount(found.local_addr_used),
        subflow_endpoints_limit: SubflowCount(found.local_addr_max),
        checksums: found.csum_enabled != u8::MIN,
        retransmits: SegmentCount(u64::from(found.retransmits)),
        bytes_retransmitted: ByteTotal(found.bytes_retrans),
        bytes_sent: ByteTotal(found.bytes_sent),
        bytes_received: ByteTotal(found.bytes_received),
        bytes_acknowledged: ByteTotal(found.bytes_acked),
        subflows_total: SubflowCount(found.subflows_total),
        laminar_endpoints_limit: SubflowCount(found.endp_laminar_max),
        since_last_data_sent: milliseconds(found.last_data_sent),
        since_last_data_received: milliseconds(found.last_data_recv),
        since_last_acknowledgement_received: milliseconds(found.last_ack_recv),
    }
}

/// Reads a multipath socket's `MPTCP_INFO`; Linux answers `EOPNOTSUPP` for a TCP socket and one that fell back to TCP.
pub(super) fn multipath(
    socket: BorrowedFd<'_>,
    shortfalls: &Shortfalls,
) -> Result<MultipathInformation, HostError> {
    let mut bytes = [u8::MIN; size_of::<MptcpInfo>()];
    let length = options::get_bytes(socket, uapi::SOL_MPTCP, options::MPTCP_INFO, &mut bytes)?;
    let found = MptcpInfo::read_from_bytes(&bytes).map_err(|_| invalid())?;
    if length < size_of::<MptcpInfo>() {
        shortfalls.count(SHORT_MULTIPATH_INFORMATION);
    }
    if found.endp_fullmesh_max != u8::MIN {
        shortfalls.count(NO_FULLMESH_LIMIT);
    }
    Ok(multipath_of(&found))
}

/// Reads `MPTCP_FULL_INFO` with arrays of the given length, giving how many subflows the connection has.
///
/// A host whose subflow `TCP_INFO` is shorter than the room is counted, as a plain `TCP_INFO` read is.
fn full_information(
    socket: BorrowedFd<'_>,
    subflows: &mut [MptcpSubflowInfo],
    information: &mut [InformationBytes],
    shortfalls: &Shortfalls,
) -> Result<(u32, Option<AccurateEcnLayout>), HostError> {
    let mut connection = [u8::MIN; size_of::<MptcpInfo>()];
    let full =
        options::get_multipath_full_information(socket, &mut connection, subflows, information)?;
    if usize::try_from(full.tcp_info_size).map_err(|_| overflow())? < INFORMATION_LENGTH {
        shortfalls.count(SHORT_INFORMATION);
        return Ok((full.subflows, None));
    }
    let layout = accurate_ecn_layout().ok_or_else(|| shortfalls.refuse(NO_KERNEL_VERSION))?;
    Ok((full.subflows, Some(layout)))
}

/// Reads the subflow at the position through `MPTCP_FULL_INFO`, none past the last.
pub(super) fn multipath_subflow(
    socket: BorrowedFd<'_>,
    position: MultipathSubflowPosition,
    shortfalls: &Shortfalls,
) -> Result<Option<MultipathSubflow>, HostError> {
    let empty = InformationBytes::new_zeroed();
    let mut subflows = [MptcpSubflowInfo::new_zeroed(); SUBFLOW_ROOM];
    let mut information = [empty; SUBFLOW_ROOM];
    let (count, layout) = full_information(socket, &mut subflows, &mut information, shortfalls)?;
    let index = usize::try_from(position.0).map_err(|_| overflow())?;
    if position.0 >= count {
        return Ok(None);
    }
    if index < SUBFLOW_ROOM {
        return subflow_of(subflows[index], &information[index], layout, shortfalls).map(Some);
    }
    shortfalls.count(SUBFLOWS_PAST_THE_ROOM);
    let length = usize::try_from(count).map_err(|_| overflow())?;
    let mut subflows = vec![MptcpSubflowInfo::new_zeroed(); length];
    let mut information = vec![empty; length];
    let (filled, layout) = full_information(socket, &mut subflows, &mut information, shortfalls)?;
    if position.0 >= filled.min(count) {
        return Ok(None);
    }
    subflow_of(subflows[index], &information[index], layout, shortfalls).map(Some)
}

/// Gives one subflow of an `MPTCP_FULL_INFO` read with its `TCP_INFO`.
fn subflow_of(
    subflow: MptcpSubflowInfo,
    information: &InformationBytes,
    layout: Option<AccurateEcnLayout>,
    shortfalls: &Shortfalls,
) -> Result<MultipathSubflow, HostError> {
    let address = |storage: StorageBytes| {
        address_of_storage(&storage.bytes).ok_or(HostError::Errno(libc::EAFNOSUPPORT))
    };
    Ok(MultipathSubflow {
        identifier: MultipathSubflowIdentifier(subflow.id),
        local: address(subflow.local)?,
        peer: address(subflow.remote)?,
        information: information_of(information, layout, shortfalls)?,
    })
}

/// Reads what the socket recorded of the last packet it received in order: its options' records and extension headers.
///
/// IPv4's `IP_PKTOPTIONS` or IPv6's `IPV6_2292PKTOPTIONS` by the peer's family; Linux keeps no fixed header, and for IPv4
/// no options.
pub(super) fn last_received_header<'a>(
    socket: BorrowedFd<'_>,
    peer: IpAddr,
    into: &'a mut [u8],
    shortfalls: &Shortfalls,
) -> Result<TcpReading<'a>, HostError> {
    let mut control = LastHeaderRecords {
        _aligned: [],
        bytes: [u8::MIN; LAST_HEADER_ROOM],
    };
    let length = match peer.to_canonical() {
        IpAddr::V4(_) => {
            shortfalls.count(NO_RECORDED_IPV4_OPTIONS);
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
    }?;
    // Linux stops writing at the first record that does not fit, and says nothing of it.
    if LAST_HEADER_ROOM - length < size_of::<libc::cmsghdr>() {
        shortfalls.count(RECORDS_FILL_THE_ROOM);
    }
    let records = &control.bytes[..length];
    let mut written = usize::MIN;
    for header in extension_headers(records) {
        let room = into.len() - written;
        if header.len() > room {
            shortfalls.count(HEADERS_CUT);
        }
        let taken = header.len().min(room);
        into[written..written + taken].copy_from_slice(&header[..taken]);
        written += taken;
    }
    Ok(TcpReading::LastReceivedHeader {
        interface: Ancillary::parse(records).interface(),
        header: &into[..written],
    })
}

/// Gives the bytes of each IPv6 extension header among control messages, in the order Linux wrote them.
fn extension_headers(control: &[u8]) -> impl Iterator<Item = &[u8]> {
    options::ControlMessages::new(control).filter_map(|(level, kind, data)| {
        (level as u32 == uapi::SOL_IPV6
            && matches!(
                kind as u32,
                uapi::IPV6_HOPOPTS | uapi::IPV6_DSTOPTS | uapi::IPV6_RTHDR
            ))
        .then_some(data)
    })
}

/// Reads the processor the socket's packets are received on (`SO_INCOMING_CPU`), none when none is recorded.
pub(super) fn incoming_processor(
    socket: BorrowedFd<'_>,
) -> Result<Option<ProcessorIndex>, HostError> {
    match get_int(socket, uapi::SOL_SOCKET, uapi::SO_INCOMING_CPU)? {
        NO_PROCESSOR => Ok(None),
        processor => u32::try_from(processor)
            .map(|index| Some(ProcessorIndex(index)))
            .map_err(|_| invalid()),
    }
}
