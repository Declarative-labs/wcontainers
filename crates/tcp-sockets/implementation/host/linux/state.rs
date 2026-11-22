//! The state readings of a Linux TCP or MPTCP socket and the repair settings that write it back.

use std::ffi::c_int;
use std::mem::{MaybeUninit, offset_of, size_of};
use std::net::IpAddr;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::time::Duration;

use linux_raw_sys::ioctl as requests;
use linux_raw_sys::net as uapi;
use libc::socklen_t;
use rustix::net::addr::SocketAddrStorage;
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    ByteCount, BytesPerSecond, IpVersion, MaximumTransmissionUnit, ProcessorIndex,
};
use wcontainers_internet_protocol_linux_socket_options::datagram::addresses::read_address;
use wcontainers_internet_protocol_linux_socket_options::datagram::receive::Ancillary;
use wcontainers_internet_protocol_linux_socket_options::{
    self as options, Shortfalls, get_int, last_host_error, set_int, set_option,
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
    CongestionFraction, CongestionGain, DeliveryRateLimit, EcnByteCounts, EventCount,
    TcpCongestionAvoidanceState, TcpFastOpenClientFailure, TcpInformation, TcpNegotiatedOptions,
    TcpQueues, TcpReading, TcpState, TcpTuned, UrgentMark,
};

use super::{ONE_KEY, authentication_key_name, invalid};

/// The reasons a reading is incomplete or refused, each counted by [`Shortfalls`].
const SHORT_INFORMATION: &str =
    "the host's TCP_INFO is older than the contract's: the fields it lacks are given as zero";
const SHORT_MULTIPATH_INFORMATION: &str =
    "the host's MPTCP_INFO is older than the contract's: the fields it lacks are given as zero";
const UNKNOWN_STATE: &str =
    "the host reported a TCP state or congestion state the contract has no name for";
const UNKNOWN_CONGESTION_INFORMATION: &str =
    "the host reported congestion control information of a form the contract does not know";
const NO_RECORDED_LENGTH: &str = "Linux records no length and no fixed header of the last packet a TCP socket received, only its options and extension headers";

/// The TCP states in the order Linux numbers them from `TCP_ESTABLISHED`, which is one.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/net/tcp_states.h
const STATES: [TcpState; 13] = [
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
    TcpState::NewSynchronizeReceived,
    TcpState::BoundInactive,
];

/// `TCP_ESTABLISHED`, the number of the first of [`STATES`].
///
/// https://github.com/torvalds/linux/blob/v6.18/include/net/tcp_states.h
const FIRST_STATE: u8 = 1;

/// The `tcpi_accecn_fail_mode` bits: the ACE field failed sending and receiving, the option failed sending and receiving.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/net/tcp_ecn.h
const ACE_FAIL_SEND: u16 = 1 << 0;
/// https://github.com/torvalds/linux/blob/v6.18/include/net/tcp_ecn.h
const ACE_FAIL_RECEIVE: u16 = 1 << 1;
/// https://github.com/torvalds/linux/blob/v6.18/include/net/tcp_ecn.h
const OPTION_FAIL_SEND: u16 = 1 << 2;
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
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/tcp.h
#[repr(C)]
#[derive(Clone, Copy)]
struct AccurateEcnInformation {
    received_ce: u32,
    delivered_e1_bytes: u32,
    delivered_e0_bytes: u32,
    delivered_ce_bytes: u32,
    received_e1_bytes: u32,
    received_e0_bytes: u32,
    received_ce_bytes: u32,
    fail_mode: u16,
    option_seen: u16,
}

/// Where the fields linux-raw-sys lacks start in `TCP_INFO`, right after `tcpi_total_rto_time`.
const ACCURATE_ECN_AT: usize = offset_of!(uapi::tcp_info, tcpi_total_rto_time) + size_of::<u32>();
/// The length of the whole `TCP_INFO` the contract reads.
const INFORMATION_LENGTH: usize = ACCURATE_ECN_AT + size_of::<AccurateEcnInformation>();

/// The room `TCP_INFO` is read into, aligned for both views of it.
#[repr(C, align(8))]
#[derive(Clone, Copy)]
struct InformationBytes([u8; INFORMATION_LENGTH]);

/// The longest congestion control name, `TCP_CA_NAME_MAX`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/net/tcp.h
const CONGESTION_NAME_ROOM: usize = 16;

/// The `TCP_CC_INFO` forms of `tcp_cc_info`: Vegas's (also Westwood's and Illinois'), DCTCP's and BBR's.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/inet_diag.h
#[repr(C)]
#[derive(Clone, Copy)]
struct VegasInformation {
    enabled: u32,
    round_trip_time_count: u32,
    round_trip_time: u32,
    minimum_round_trip_time: u32,
}

/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/inet_diag.h
#[repr(C)]
#[derive(Clone, Copy)]
struct DataCenterInformation {
    enabled: u16,
    congestion_experienced: u16,
    alpha: u32,
    acknowledged_bytes_marked: u32,
    acknowledged_bytes_total: u32,
}

/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/inet_diag.h
#[repr(C)]
#[derive(Clone, Copy)]
struct BottleneckBandwidthInformation {
    bandwidth_low: u32,
    bandwidth_high: u32,
    minimum_round_trip_time: u32,
    pacing_gain: u32,
    congestion_window_gain: u32,
}

/// The room `TCP_CC_INFO` is read into, aligned for each form.
#[repr(C, align(8))]
#[derive(Clone, Copy)]
struct CongestionBytes([u8; size_of::<BottleneckBandwidthInformation>()]);

/// The congestion control names whose information has DCTCP's and BBR's forms.
const DATA_CENTER: &str = "dctcp";
const BOTTLENECK_BANDWIDTH: &str = "bbr";

/// `MPTCP_INFO` and `MPTCP_FULL_INFO`, the options of level `SOL_MPTCP` that read a multipath socket.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
const MPTCP_INFO: u32 = 1;
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
const MPTCP_FULL_INFO: u32 = 4;

/// The `mptcpi_flags` bits: the connection fell back to TCP, the peer's key was received.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
const FELL_BACK: u32 = 1 << 0;
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
const REMOTE_KEY_RECEIVED: u32 = 1 << 1;

/// `struct mptcp_info`, which neither libc nor linux-raw-sys carries.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
#[repr(C)]
#[derive(Clone, Copy)]
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
    reserved: [u8; 2],
    last_data_sent: u32,
    last_data_recv: u32,
    last_ack_recv: u32,
}

/// `struct mptcp_subflow_info`: a subflow's identifier and its local and remote addresses.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
#[repr(C)]
#[derive(Clone, Copy)]
struct MptcpSubflowInfo {
    id: u32,
    local: SocketAddrStorage,
    remote: SocketAddrStorage,
}

/// `struct mptcp_full_info`, whose arrays of subflows and their `TCP_INFO` the caller gives.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
#[repr(C)]
#[derive(Clone, Copy)]
struct MptcpFullInfo {
    size_tcpinfo_kernel: u32,
    size_tcpinfo_user: u32,
    size_sfinfo_kernel: u32,
    size_sfinfo_user: u32,
    num_subflows: u32,
    size_arrays_user: u32,
    subflow_info: u64,
    tcp_info: u64,
    mptcp_info: MptcpInfo,
}

/// The shift of the receive scale in a `TCPOPT_WINDOW` repair option's value, the send scale being below it.
///
/// https://github.com/torvalds/linux/blob/v6.18/net/ipv4/tcp.c
const RECEIVE_SCALE_SHIFT: u32 = u16::BITS;

/// The value of a repair option that carries none: selective acknowledgements and timestamps.
///
/// https://github.com/torvalds/linux/blob/v6.18/net/ipv4/tcp.c
const NO_OPTION_VALUE: u32 = u32::MIN;

/// How many repair options one `TCP_REPAIR_OPTIONS` write gives; Linux applies each on its own.
const REPAIR_OPTIONS_AT_ONCE: usize = 4;

/// `SIOCATMARK`'s answer when the next byte to read is not the urgent mark.
///
/// https://man7.org/linux/man-pages/man7/tcp.7.html
const NOT_AT_MARK: c_int = 0;

/// `SO_INCOMING_CPU`'s answer when no processor is recorded.
///
/// https://man7.org/linux/man-pages/man7/socket.7.html
const NO_PROCESSOR: c_int = -1;

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

/// Reads a plain uapi structure from the start of bytes the kernel wrote.
fn view<T: Copy>(bytes: &[u8]) -> T {
    assert!(bytes.len() >= size_of::<T>());
    // SAFETY: the bytes hold at least a T, a plain uapi structure that any bytes make, read unaligned.
    unsafe { bytes.as_ptr().cast::<T>().read_unaligned() }
}

/// Gives a structure's bytes for the kernel to read and write.
fn bytes_of<T: Copy>(value: &mut T) -> &mut [u8] {
    // SAFETY: T is a plain uapi structure, so its bytes may be read and written as such.
    unsafe {
        std::slice::from_raw_parts_mut(std::ptr::from_mut(value).cast::<u8>(), size_of::<T>())
    }
}

fn zeroed<T: Copy>() -> T {
    // SAFETY: only used with plain uapi structures, for which all zero bytes are a value.
    unsafe { MaybeUninit::<T>::zeroed().assume_init() }
}

/// Reads one of the socket's ioctl counts.
fn ioctl_count(socket: BorrowedFd<'_>, request: u32) -> Result<c_int, HostError> {
    let mut value = c_int::MIN;
    // SAFETY: each request used writes one int.
    if unsafe { libc::ioctl(socket.as_raw_fd(), request as _, &mut value) } < 0 {
        return Err(last_host_error());
    }
    Ok(value)
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

/// Reads `TCP_INFO` into bytes, counting a shorter one, which leaves the fields it lacks zero.
fn information_bytes(
    socket: BorrowedFd<'_>,
    shortfalls: &Shortfalls,
) -> Result<InformationBytes, HostError> {
    let mut bytes = InformationBytes([u8::MIN; INFORMATION_LENGTH]);
    let length = options::get_bytes(socket, uapi::SOL_TCP, uapi::TCP_INFO, &mut bytes.0)?;
    if length < INFORMATION_LENGTH {
        shortfalls.count(SHORT_INFORMATION);
    }
    Ok(bytes)
}

/// Gives the contract's `TCP_INFO` from the bytes Linux wrote.
fn information_of(
    bytes: &InformationBytes,
    shortfalls: &Shortfalls,
) -> Result<TcpInformation, HostError> {
    let info: uapi::tcp_info = view(&bytes.0);
    let ecn: AccurateEcnInformation = view(&bytes.0[ACCURATE_ECN_AT..]);
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
        match u32::from(info.tcpi_fastopen_client_fail()) {
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
            explicit_congestion_notification: option(uapi::TCPI_OPT_ECN),
            explicit_congestion_notification_seen: option(uapi::TCPI_OPT_ECN_SEEN),
            synchronize_data_acknowledged: option(uapi::TCPI_OPT_SYN_DATA),
            microsecond_timestamps: option(uapi::TCPI_OPT_USEC_TS),
            fast_open_child: option(uapi::TCPI_OPT_TFO_CHILD),
        },
        window_scales: TcpWindowScales {
            send: WindowScale(info.tcpi_snd_wscale()),
            receive: WindowScale(info.tcpi_rcv_wscale()),
        },
        delivery_rate_limit: if info.tcpi_delivery_rate_app_limited() == u8::MIN {
            DeliveryRateLimit::Network
        } else {
            DeliveryRateLimit::Application
        },
        fast_open_client_failure,
        retransmission_timeout: microseconds(info.tcpi_rto),
        acknowledgement_timeout: microseconds(info.tcpi_ato),
        send_maximum_segment_size: MaximumSegmentSize(narrow(info.tcpi_snd_mss)?),
        receive_maximum_segment_size: MaximumSegmentSize(narrow(info.tcpi_rcv_mss)?),
        unacknowledged: segments(info.tcpi_unacked),
        selectively_acknowledged: segments(info.tcpi_sacked),
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
            ace_field_sending: ecn.fail_mode & ACE_FAIL_SEND != u16::MIN,
            ace_field_receiving: ecn.fail_mode & ACE_FAIL_RECEIVE != u16::MIN,
            option_sending: ecn.fail_mode & OPTION_FAIL_SEND != u16::MIN,
            option_receiving: ecn.fail_mode & OPTION_FAIL_RECEIVE != u16::MIN,
        },
        accurate_ecn_option_seen: OPTIONS_SEEN
            .get(usize::from(ecn.option_seen))
            .copied()
            .ok_or_else(|| shortfalls.refuse(UNKNOWN_STATE))?,
    })
}

/// Reads the socket's `TCP_INFO`.
pub(super) fn information(
    socket: BorrowedFd<'_>,
    shortfalls: &Shortfalls,
) -> Result<TcpInformation, HostError> {
    information_of(&information_bytes(socket, shortfalls)?, shortfalls)
}

/// Reads the congestion control's name, cut to the buffer as `TCP_CONGESTION` cuts it, and its `TCP_CC_INFO`.
pub(super) fn congestion_control<'a>(
    socket: BorrowedFd<'_>,
    into: &'a mut [u8],
    shortfalls: &Shortfalls,
) -> Result<TcpReading<'a>, HostError> {
    let mut name = [u8::MIN; CONGESTION_NAME_ROOM];
    let length = options::get_bytes(socket, uapi::SOL_TCP, uapi::TCP_CONGESTION, &mut name)?;
    let length = name[..length]
        .iter()
        .position(|&byte| byte == u8::MIN)
        .unwrap_or(length);
    let mut bytes = CongestionBytes([u8::MIN; size_of::<BottleneckBandwidthInformation>()]);
    let size = options::get_bytes(socket, uapi::SOL_TCP, uapi::TCP_CC_INFO, &mut bytes.0)?;
    let information = match (&name[..length], size) {
        (_, usize::MIN) => None,
        (algorithm, _)
            if algorithm == DATA_CENTER.as_bytes()
                && size >= size_of::<DataCenterInformation>() =>
        {
            let found: DataCenterInformation = view(&bytes.0);
            Some(CongestionControlInformation::DataCenter {
                enabled: found.enabled != u16::MIN,
                congestion_experienced: found.congestion_experienced != u16::MIN,
                alpha: CongestionFraction(found.alpha),
                acknowledged_bytes_marked: ByteTotal(u64::from(found.acknowledged_bytes_marked)),
                acknowledged_bytes_total: ByteTotal(u64::from(found.acknowledged_bytes_total)),
            })
        }
        (algorithm, _)
            if algorithm == BOTTLENECK_BANDWIDTH.as_bytes()
                && size >= size_of::<BottleneckBandwidthInformation>() =>
        {
            let found: BottleneckBandwidthInformation = view(&bytes.0);
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
            let found: VegasInformation = view(&bytes.0);
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
    Ok(TcpReading::CongestionControl { name, information })
}

/// Reads the bytes unread, unacknowledged and unsent (`SIOCINQ`, `SIOCOUTQ`, `SIOCOUTQNSD`) and the urgent mark.
pub(super) fn queues(socket: BorrowedFd<'_>) -> Result<TcpQueues, HostError> {
    let count = |request| ioctl_count(socket, request).and_then(narrow::<usize>);
    let queued = count(requests::TIOCOUTQ)?;
    let unsent = count(libc::SIOCOUTQNSD as u32)?;
    Ok(TcpQueues {
        unread: ByteCount(count(requests::FIONREAD)?),
        unacknowledged: ByteCount(queued.saturating_sub(unsent)),
        unsent: ByteCount(unsent),
        urgent_mark: if ioctl_count(socket, requests::SIOCATMARK)? == NOT_AT_MARK {
            UrgentMark::NotReached
        } else {
            UrgentMark::Reached
        },
    })
}

/// Reads the values the stack chose for the socket's buffers, low water, segment size, clamp, path MTU and acknowledging.
pub(super) fn tuned(
    socket: BorrowedFd<'_>,
    shortfalls: &Shortfalls,
) -> Result<TcpTuned, HostError> {
    let int = |level, name| get_int(socket, level, name);
    let information: uapi::tcp_info = view(&information_bytes(socket, shortfalls)?.0);
    Ok(TcpTuned {
        send_buffer: ByteCount(narrow(int(uapi::SOL_SOCKET, uapi::SO_SNDBUF)?)?),
        receive_buffer: ByteCount(narrow(int(uapi::SOL_SOCKET, uapi::SO_RCVBUF)?)?),
        receive_low_water: ByteCount(narrow(int(uapi::SOL_SOCKET, uapi::SO_RCVLOWAT)?)?),
        maximum_segment_size: MaximumSegmentSize(narrow(int(uapi::SOL_TCP, uapi::TCP_MAXSEG)?)?),
        window_clamp: WindowSize(narrow(int(uapi::SOL_TCP, uapi::TCP_WINDOW_CLAMP)?)?),
        path_mtu: MaximumTransmissionUnit(information.tcpi_pmtu),
        quick_acknowledgement: int(uapi::SOL_TCP, uapi::TCP_QUICKACK)? != super::UNSET,
    })
}

/// Reads the timestamp clock, in the unit its route gave the socket.
pub(super) fn timestamp(
    socket: BorrowedFd<'_>,
    shortfalls: &Shortfalls,
) -> Result<TcpTimestampClock, HostError> {
    let value: u32 = options::get_option(socket, uapi::SOL_TCP, uapi::TCP_TIMESTAMP)?;
    let information: uapi::tcp_info = view(&information_bytes(socket, shortfalls)?.0);
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
    let window: uapi::tcp_repair_window =
        options::get_option(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_WINDOW)?;
    let queue = get_int(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE)?;
    let sequence_of = |chosen: c_int| -> Result<TcpSequenceNumber, HostError> {
        set_int(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE, chosen)?;
        options::get_option(socket, uapi::SOL_TCP, uapi::TCP_QUEUE_SEQ).map(TcpSequenceNumber)
    };
    let sequences = sequence_of(uapi::TCP_SEND_QUEUE as c_int)
        .and_then(|send| Ok((send, sequence_of(uapi::TCP_RECV_QUEUE as c_int)?)));
    set_int(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_QUEUE, queue)?;
    let (send_sequence, receive_sequence) = sequences?;
    let authentication = match options::get_option::<uapi::tcp_ao_repair>(
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

/// Writes repair options through `TCP_REPAIR_OPTIONS`, a few at a time on the stack.
pub(super) fn set_repair_options(
    socket: BorrowedFd<'_>,
    repair: &[TcpRepairOption],
) -> Result<(), HostError> {
    use etherparse::tcp_option::{
        KIND_MAXIMUM_SEGMENT_SIZE, KIND_SELECTIVE_ACK_PERMITTED, KIND_TIMESTAMP, KIND_WINDOW_SCALE,
    };
    for chunk in repair.chunks(REPAIR_OPTIONS_AT_ONCE) {
        let mut written = [zeroed::<uapi::tcp_repair_opt>(); REPAIR_OPTIONS_AT_ONCE];
        for (to, option) in written.iter_mut().zip(chunk) {
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
            to.opt_code = u32::from(kind);
            to.opt_val = value;
        }
        let used = &written[..chunk.len()];
        // SAFETY: the options are plain data, given as their bytes.
        let bytes =
            unsafe { std::slice::from_raw_parts(used.as_ptr().cast::<u8>(), size_of_val(used)) };
        options::set_bytes(socket, uapi::SOL_TCP, uapi::TCP_REPAIR_OPTIONS, bytes)?;
    }
    Ok(())
}

pub(super) fn set_repair_window(
    socket: BorrowedFd<'_>,
    window: &TcpRepairWindow,
) -> Result<(), HostError> {
    let written = uapi::tcp_repair_window {
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
    let written = uapi::tcp_ao_repair {
        snt_isn: repair.sent_initial_sequence.0.to_be(),
        rcv_isn: repair.received_initial_sequence.0.to_be(),
        snd_sne: repair.send_extension.0,
        rcv_sne: repair.receive_extension.0,
    };
    set_option(socket, uapi::SOL_TCP, uapi::TCP_AO_REPAIR, &written)
}

/// Reads the TCP Authentication Option's state from `TCP_AO_INFO`; a socket without it answers Linux's `ENOENT`.
pub(super) fn authentication(socket: BorrowedFd<'_>) -> Result<TcpAuthenticationStatus, HostError> {
    let found: uapi::tcp_ao_info_opt =
        options::get_option(socket, uapi::SOL_TCP, uapi::TCP_AO_INFO)?;
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
    let mut asked: uapi::tcp_ao_getsockopt = zeroed();
    asked.addr = name.address;
    asked.prefix = name.prefix;
    asked.keyflags = name.flags;
    asked.ifindex = name.interface;
    asked.sndid = name.send;
    asked.rcvid = name.receive;
    asked.nkeys = ONE_KEY;
    match options::get_bytes(
        socket,
        uapi::SOL_TCP,
        uapi::TCP_AO_GET_KEYS,
        bytes_of(&mut asked),
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
    let mut found: MptcpInfo = zeroed();
    let length = options::get_bytes(socket, uapi::SOL_MPTCP, MPTCP_INFO, bytes_of(&mut found))?;
    if length < size_of::<MptcpInfo>() {
        shortfalls.count(SHORT_MULTIPATH_INFORMATION);
    }
    Ok(multipath_of(&found))
}

/// Reads `MPTCP_FULL_INFO` with arrays of the given length, giving how many subflows the connection has.
fn full_information(
    socket: BorrowedFd<'_>,
    subflows: &mut [MptcpSubflowInfo],
    information: &mut [InformationBytes],
) -> Result<u32, HostError> {
    let mut full: MptcpFullInfo = zeroed();
    full.size_tcpinfo_user = narrow(size_of::<InformationBytes>() as u64)?;
    full.size_sfinfo_user = narrow(size_of::<MptcpSubflowInfo>() as u64)?;
    full.size_arrays_user = narrow(subflows.len() as u64)?;
    full.subflow_info = subflows.as_mut_ptr() as u64;
    full.tcp_info = information.as_mut_ptr() as u64;
    options::get_bytes(
        socket,
        uapi::SOL_MPTCP,
        MPTCP_FULL_INFO,
        bytes_of(&mut full),
    )?;
    Ok(full.num_subflows)
}

/// Reads the subflow at the position through `MPTCP_FULL_INFO`, none past the last.
pub(super) fn multipath_subflow(
    socket: BorrowedFd<'_>,
    position: MultipathSubflowPosition,
    shortfalls: &Shortfalls,
) -> Result<Option<MultipathSubflow>, HostError> {
    let count = full_information(socket, &mut [], &mut [])?;
    if position.0 >= count {
        return Ok(None);
    }
    let length = usize::try_from(count).map_err(|_| overflow())?;
    let mut subflows = vec![zeroed::<MptcpSubflowInfo>(); length];
    let mut information = vec![InformationBytes([u8::MIN; INFORMATION_LENGTH]); length];
    let filled = full_information(socket, &mut subflows, &mut information)?;
    let index = usize::try_from(position.0).map_err(|_| overflow())?;
    if position.0 >= filled.min(count) {
        return Ok(None);
    }
    let subflow = subflows[index];
    let address = |storage: SocketAddrStorage| {
        read_address(
            &MaybeUninit::new(storage),
            size_of::<SocketAddrStorage>() as socklen_t,
        )
        .ok_or(HostError::Errno(libc::EAFNOSUPPORT))
    };
    Ok(Some(MultipathSubflow {
        identifier: MultipathSubflowIdentifier(subflow.id),
        local: address(subflow.local)?,
        peer: address(subflow.remote)?,
        information: information_of(&information[index], shortfalls)?,
    }))
}

/// Reads what the socket recorded of the last packet it received in order: its options' records and extension headers.
///
/// IPv4's `IP_PKTOPTIONS` or IPv6's `IPV6_2292PKTOPTIONS` by the peer's family; Linux keeps no length and no fixed header.
pub(super) fn last_received_header<'a>(
    socket: BorrowedFd<'_>,
    peer: IpAddr,
    into: &'a mut [u8],
    shortfalls: &Shortfalls,
) -> Result<TcpReading<'a>, HostError> {
    let mut control = super::PacketOptions {
        _aligned: [],
        bytes: [u8::MIN; super::PACKET_OPTIONS_ROOM],
    };
    let length = match peer.to_canonical() {
        IpAddr::V4(_) => options::get_bytes(
            socket,
            uapi::SOL_IP,
            uapi::IP_PKTOPTIONS,
            &mut control.bytes,
        ),
        IpAddr::V6(_) => options::get_bytes(
            socket,
            uapi::SOL_IPV6,
            uapi::IPV6_2292PKTOPTIONS,
            &mut control.bytes,
        ),
    }?;
    shortfalls.count(NO_RECORDED_LENGTH);
    let ancillary = Ancillary::parse(&control.bytes[..length]);
    let mut written = usize::MIN;
    let headers = std::iter::once(ancillary.ipv4_options).chain(
        ancillary
            .extension_headers
            .iter()
            .flatten()
            .map(|(_, header)| *header),
    );
    for header in headers {
        let room = into.len() - written;
        let taken = header.len().min(room);
        into[written..written + taken].copy_from_slice(&header[..taken]);
        written += taken;
    }
    Ok(TcpReading::LastReceivedHeader {
        fields: ancillary.packet_fields(usize::MIN, u32::MIN),
        header: &into[..written],
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
