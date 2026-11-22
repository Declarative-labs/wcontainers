use core::time::Duration;

use wcontainers_internet_protocol::{
    ByteCount, BytesPerSecond, HopLimit, MaximumTransmissionUnit, ProcessorIndex,
    ReceivedPacketFields, TransmitReport,
};

use crate::authentication::{
    TcpAuthenticationKeyCounters, TcpAuthenticationKeyIdentity, TcpAuthenticationStatus,
};
use crate::multipath::{MultipathInformation, MultipathSubflow, MultipathSubflowPosition};
use crate::repair::{TcpRepairState, TcpTimestampClock, TcpWindowScales};
use crate::settings::{MaximumSegmentSize, ProbeCount, RetryCount, SegmentCount, WindowSize};

/// The BBR gain of one, [`BBR_UNIT`](https://github.com/torvalds/linux/blob/v6.18/net/ipv4/tcp_bbr.c#L78).
pub const BOTTLENECK_BANDWIDTH_GAIN_ONE: u32 = 256;

/// The whole of a DCTCP fraction, [`DCTCP_MAX_ALPHA`](https://github.com/torvalds/linux/blob/v6.18/net/ipv4/tcp_dctcp.c#L47).
pub const DATA_CENTER_FRACTION_WHOLE: u32 = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteTotal(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventCount(pub u32);

/// An entry of a connection's [error queue](https://man7.org/linux/man-pages/man2/IP_RECVERR.2const.html), an ICMP error whole from its IP header in the buffer or a transmit report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpErrorQueueEntry {
    IcmpError(ReceivedPacketFields),
    TransmitReport {
        report: TransmitReport,
        statistics: Option<TcpTransmitStatistics>,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum TcpQuery<'a> {
    Information,
    CongestionControl(&'a mut [u8]),
    Queues,
    Tuned,
    TakeSavedSynchronize(&'a mut [u8]),
    Timestamp,
    Repair,
    Authentication,
    AuthenticationKey(TcpAuthenticationKeyIdentity),
    Multipath,
    MultipathSubflow(MultipathSubflowPosition),
    LastReceivedHeader(&'a mut [u8]),
    IncomingProcessor,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TcpListenerQuery<'a> {
    Information,
    CongestionControl(&'a mut [u8]),
    Tuned,
    Timestamp,
    Authentication,
    AuthenticationKey(TcpAuthenticationKeyIdentity),
    IncomingProcessor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpReading<'a> {
    Information(TcpInformation),
    CongestionControl {
        name: &'a str,
        information: Option<CongestionControlInformation>,
    },
    Queues(TcpQueues),
    Tuned(TcpTuned),
    SavedSynchronize(TcpSavedSynchronizeHeader<'a>),
    Timestamp(TcpTimestampClock),
    Repair(TcpRepairState),
    Authentication(TcpAuthenticationStatus),
    AuthenticationKey(Option<TcpAuthenticationKeyCounters>),
    Multipath(MultipathInformation),
    MultipathSubflow(Option<MultipathSubflow>),
    LastReceivedHeader {
        fields: ReceivedPacketFields,
        header: &'a [u8],
    },
    IncomingProcessor(Option<ProcessorIndex>),
}

/// The outcome of taking the SYN headers that [`TCP_SAVED_SYN`](https://man7.org/linux/man-pages/man7/tcp.7.html#:~:text=TCP_SAVED_SYN%20%28since) holds: taken, kept as too long for the buffer, or none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpSavedSynchronizeHeader<'a> {
    Taken(&'a [u8]),
    Kept { length: ByteCount },
    Absent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrgentMark {
    NotReached,
    Reached,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpQueues {
    pub unread: ByteCount,
    pub unacknowledged: ByteCount,
    pub unsent: ByteCount,
    pub urgent_mark: UrgentMark,
}

/// The values a connection's stack chose or changed for settings it was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpTuned {
    pub send_buffer: ByteCount,
    pub receive_buffer: ByteCount,
    pub receive_low_water: ByteCount,
    pub maximum_segment_size: MaximumSegmentSize,
    pub window_clamp: WindowSize,
    pub path_mtu: MaximumTransmissionUnit,
    pub quick_acknowledgement: bool,
}

/// A BBR gain, [`BOTTLENECK_BANDWIDTH_GAIN_ONE`] being a gain of one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CongestionGain(pub u32);

/// A [DCTCP](https://www.rfc-editor.org/rfc/rfc8257) fraction, [`DATA_CENTER_FRACTION_WHOLE`] being the whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CongestionFraction(pub u32);

/// The state of a congestion control algorithm that `TCP_CC_INFO` reports, for the algorithms that report one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CongestionControlInformation {
    Vegas {
        enabled: bool,
        round_trip_time_count: EventCount,
        round_trip_time: Duration,
        minimum_round_trip_time: Duration,
    },
    DataCenter {
        enabled: bool,
        congestion_experienced: bool,
        alpha: CongestionFraction,
        acknowledged_bytes_marked: ByteTotal,
        acknowledged_bytes_total: ByteTotal,
    },
    BottleneckBandwidth {
        bandwidth: BytesPerSecond,
        minimum_round_trip_time: Duration,
        pacing_gain: CongestionGain,
        congestion_window_gain: CongestionGain,
    },
}

/// A [TCP state](https://www.rfc-editor.org/rfc/rfc9293#section-3.3.2), with Linux's own `NEW_SYN_RECV` and `BOUND_INACTIVE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpState {
    Established,
    SynchronizeSent,
    SynchronizeReceived,
    FinWait1,
    FinWait2,
    TimeWait,
    Closed,
    CloseWait,
    LastAcknowledgement,
    Listen,
    Closing,
    NewSynchronizeReceived,
    BoundInactive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpCongestionAvoidanceState {
    Open,
    Disorder,
    CongestionWindowReduced,
    Recovery,
    Loss,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpNegotiatedOptions {
    pub timestamps: bool,
    pub selective_acknowledgements: bool,
    pub window_scaling: bool,
    pub explicit_congestion_notification: bool,
    pub explicit_congestion_notification_seen: bool,
    pub synchronize_data_acknowledged: bool,
    pub microsecond_timestamps: bool,
    pub fast_open_child: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryRateLimit {
    Network,
    Application,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpFastOpenClientFailure {
    Unspecified,
    CookieUnavailable,
    DataNotAcknowledged,
    SynchronizeRetransmitted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EcnByteCounts {
    pub capable_transport_1: ByteTotal,
    pub capable_transport_0: ByteTotal,
    pub congestion_experienced: ByteTotal,
}

/// The [Accurate ECN](https://www.rfc-editor.org/rfc/rfc9768) failures a connection met, `tcpi_accecn_fail_mode`'s bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccurateEcnFailures {
    pub ace_field_sending: bool,
    pub ace_field_receiving: bool,
    pub option_sending: bool,
    pub option_receiving: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccurateEcnOptionSeen {
    NotSeen,
    EmptySeen,
    CounterSeen,
    FailSeen,
}

/// The [`TCP_INFO`](https://man7.org/linux/man-pages/man7/tcp.7.html#:~:text=TCP_INFO%20%28since) of a socket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpInformation {
    pub state: TcpState,
    pub congestion_avoidance_state: TcpCongestionAvoidanceState,
    pub retransmits: RetryCount,
    pub probes: ProbeCount,
    pub backoff: RetryCount,
    pub options: TcpNegotiatedOptions,
    pub window_scales: TcpWindowScales,
    pub delivery_rate_limit: DeliveryRateLimit,
    pub fast_open_client_failure: TcpFastOpenClientFailure,
    pub retransmission_timeout: Duration,
    pub acknowledgement_timeout: Duration,
    pub send_maximum_segment_size: MaximumSegmentSize,
    pub receive_maximum_segment_size: MaximumSegmentSize,
    pub unacknowledged: SegmentCount,
    pub selectively_acknowledged: SegmentCount,
    pub lost: SegmentCount,
    pub retransmitted: SegmentCount,
    pub forward_acknowledged: SegmentCount,
    pub since_last_data_sent: Duration,
    pub since_last_acknowledgement_sent: Duration,
    pub since_last_data_received: Duration,
    pub since_last_acknowledgement_received: Duration,
    pub path_mtu: MaximumTransmissionUnit,
    pub receive_slow_start_threshold: WindowSize,
    pub round_trip_time: Duration,
    pub round_trip_time_variance: Duration,
    pub send_slow_start_threshold: SegmentCount,
    pub send_congestion_window: SegmentCount,
    pub advertised_maximum_segment_size: MaximumSegmentSize,
    pub reordering: SegmentCount,
    pub receive_round_trip_time: Duration,
    pub receive_space: ByteCount,
    pub total_retransmissions: SegmentCount,
    pub pacing_rate: BytesPerSecond,
    pub maximum_pacing_rate: BytesPerSecond,
    pub bytes_acknowledged: ByteTotal,
    pub bytes_received: ByteTotal,
    pub segments_out: SegmentCount,
    pub segments_in: SegmentCount,
    pub not_sent: ByteCount,
    pub minimum_round_trip_time: Duration,
    pub data_segments_in: SegmentCount,
    pub data_segments_out: SegmentCount,
    pub delivery_rate: BytesPerSecond,
    pub busy: Duration,
    pub receive_window_limited: Duration,
    pub send_buffer_limited: Duration,
    pub delivered: SegmentCount,
    pub delivered_congestion_experienced: SegmentCount,
    pub bytes_sent: ByteTotal,
    pub bytes_retransmitted: ByteTotal,
    pub duplicate_selective_acknowledgements: SegmentCount,
    pub reordering_seen: EventCount,
    pub received_out_of_order: SegmentCount,
    pub send_window: WindowSize,
    pub receive_window: WindowSize,
    pub rehashes: EventCount,
    pub retransmission_timeouts: EventCount,
    pub retransmission_timeout_recoveries: EventCount,
    pub in_retransmission_timeout_recovery: Duration,
    pub received_congestion_experienced: SegmentCount,
    pub delivered_ecn_bytes: EcnByteCounts,
    pub received_ecn_bytes: EcnByteCounts,
    pub accurate_ecn_failures: AccurateEcnFailures,
    pub accurate_ecn_option_seen: AccurateEcnOptionSeen,
}

/// The [`SOF_TIMESTAMPING_OPT_STATS`](https://docs.kernel.org/networking/timestamping.html#:~:text=SOF_TIMESTAMPING_OPT_STATS) of a transmit report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpTransmitStatistics {
    pub busy: Duration,
    pub receive_window_limited: Duration,
    pub send_buffer_limited: Duration,
    pub data_segments_out: SegmentCount,
    pub total_retransmissions: SegmentCount,
    pub pacing_rate: BytesPerSecond,
    pub delivery_rate: BytesPerSecond,
    pub send_congestion_window: SegmentCount,
    pub reordering: SegmentCount,
    pub minimum_round_trip_time: Duration,
    pub recurring_retransmissions: RetryCount,
    pub delivery_rate_limit: DeliveryRateLimit,
    pub send_queue: ByteCount,
    pub congestion_avoidance_state: TcpCongestionAvoidanceState,
    pub send_slow_start_threshold: SegmentCount,
    pub delivered: SegmentCount,
    pub delivered_congestion_experienced: SegmentCount,
    pub bytes_sent: ByteTotal,
    pub bytes_retransmitted: ByteTotal,
    pub duplicate_selective_acknowledgements: SegmentCount,
    pub reordering_seen: EventCount,
    pub smoothed_round_trip_time: Duration,
    pub timeout_rehashes: EventCount,
    pub not_sent: ByteCount,
    /// The earliest departure time on the exit node's monotonic clock.
    pub earliest_departure: Duration,
    pub received_hop_limit: HopLimit,
    pub rehashes: EventCount,
}
