//! The tests every implementation of wcontainers-tcp-sockets runs, written against its traits alone.
//!
//! Each check makes its own listener on a loopback address with a port the kernel chooses, so the checks run side by
//! side. Calls other than the waits answer at once, so an answer of `WouldBlock` is tried again until [`DEADLINE`]; the
//! waits are polled by hand with a waker that counts its wakes.
//!
//! The checks watch the wire through the host's packet and raw sockets, so they run as root on a Linux host, in a
//! network namespace of their own whose loopback is up with the `fq` queueing discipline (which honours a transmit
//! delay), with `net.ipv4.tcp_fastopen` at 3 (fast open for clients and listeners), `net.ipv6.flowlabel_consistency`
//! at 0 (which flow label reflection needs), `net.ipv4.tcp_fwmark_accept` at 1 (an accepted socket takes its SYN's
//! mark), `net.ipv6.conf.lo.forwarding` at 1 (loopback may then join any anycast address, not only those its
//! prefixes hold), the `dctcp`, `vegas` and `bbr` congestion controls available and a local route for 127.0.0.7 with the `tcp_usec_ts`
//! feature (TCP timestamps in microseconds).

use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use core::num::NonZeroU32;
use core::pin::{Pin, pin};
use core::task::{Context, Poll, Waker};
use core::time::Duration;
use std::future::Future;
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Wake;
use std::thread::{JoinHandle, Thread};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use etherparse::{
    Icmpv4Header, Icmpv4Slice, Icmpv4Type, Icmpv6Header, Icmpv6Slice, Icmpv6Type, IpDscp, IpEcn,
    IpNumber, Ipv4HeaderSlice, Ipv4Options, Ipv6FlowLabel, Ipv6Header, Ipv6HeaderSlice,
    Ipv6RawExtHeader, Ipv6RawExtHeaderSlice, TcpHeaderSlice, TcpOptionElement, TcpOptionReadError,
    icmpv4, icmpv6, tcp_option,
};
use linux_raw_sys::if_tun;
use nix::net::if_::InterfaceFlags;
use rustix::net::{AddressFamily, RecvFlags, SendFlags, SocketType, ipproto, sockopt};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    AddressSharing, AnycastAddress, BindAddress, ByteCount, BytesPerSecond, FilterInstruction,
    FilterJump, FilterOperand, FilterOperation, FlowLabelChoice, HopLimit, InterfaceIndex,
    IpPacketSetting, IpPrefix, IpSocketSetting, IpVersion, Ipv6Families, LocalBinding, LocalPort,
    Membership, MobileAddressPreference, MulticastGroup, MulticastMembership, NonlocalAddresses,
    PacketMark, PacketPriority, PathMtuDiscovery, Port, PortChoice, PortRange, PortSetting,
    PortSharing, PrefixLength, ProcessorIndex, ReceivedPacketFields, Report, RouteScope,
    SocketFilter, SourceFilter, SourcePreferences, TemporaryAddressPreference, TimestampSource,
    Timestamping, TransmitEvent, TransmitReportKey, TransmitReportRequest, TransmitStage,
};
use wcontainers_kernel::UserId;
use wcontainers_kernel::errno::Errno;
use wcontainers_readiness::{Readiness, ReadyFor};
use wcontainers_readiness_conformance::Waiting;
use wcontainers_tcp_sockets::authentication::{
    SequenceNumberExtension, TcpAuthenticationCounters, TcpAuthenticationIcmpErrors,
    TcpAuthenticationKeyAddition, TcpAuthenticationKeyIdentifier, TcpAuthenticationKeyIdentity,
    TcpAuthenticationKeyRemoval, TcpAuthenticationKeyUse, TcpAuthenticationMacLength,
    TcpAuthenticationOptionCoverage, TcpAuthenticationOptions, TcpAuthenticationRemovalTiming,
    TcpAuthenticationRepair, TcpAuthenticationRequirement, TcpAuthenticationStatus,
    TcpMd5KeyChange, TcpMd5Peer,
};
use wcontainers_tcp_sockets::multipath::{MultipathInformation, MultipathSubflowPosition};
use wcontainers_tcp_sockets::repair::{
    TcpRepair, TcpRepairOption, TcpRepairQueue, TcpRepairState, TcpTimestamp, TcpTimestampClock,
    TcpTimestampUnit,
};
use wcontainers_tcp_sockets::settings::{
    BufferLocks, BufferTuning, FAST_OPEN_KEY_LENGTH, FastOpenKey, FastOpenKeys, MaximumSegmentSize,
    ProbeCount, RetryCount, SegmentCount, TcpFinWait2, TcpListenerSetting, TcpQuickAcknowledgement,
    TcpSavedSynchronize, TcpSequenceNumber, TcpSetting, TransmitRehash, WindowSize,
};
use wcontainers_tcp_sockets::state::{
    ByteTotal, CongestionControlInformation, DATA_CENTER_FRACTION_WHOLE, DeliveryRateLimit,
    TcpCongestionAvoidanceState, TcpErrorQueueEntry, TcpInformation, TcpListenerQuery, TcpQuery,
    TcpQueues, TcpReading, TcpSavedSynchronizeHeader, TcpState, TcpTuned, UrgentMark,
};
use wcontainers_tcp_sockets::{
    ListenBacklog, TcpClose, TcpConnectStart, TcpContinuation, TcpFastOpen, TcpListening,
    TcpProtocol, TcpPushOverride, TcpReceiveRequest, TcpReceived, TcpRecordBoundary,
    TcpSendRequest, TcpShutdown, TcpSockets, TcpUrgency,
};

pub use wcontainers_configurable_conformance;
pub use wcontainers_negative_route_advisable_conformance;
pub use wcontainers_readiness_conformance;

/// The byte the first fast open key is made of.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const FIRST_FAST_OPEN_KEY: u8 = 1;

/// The byte the second fast open key is made of, unlike the first.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const SECOND_FAST_OPEN_KEY: u8 = 2;

/// The byte the third fast open key is made of, unlike the two before it.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const THIRD_FAST_OPEN_KEY: u8 = 3;

/// Peers of the address sharing check: two for the sharing connections, one for the connection that does not share.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const SHARING_PEERS: usize = 3;

/// Connections the backlog check makes, more than the smallest backlog holds.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const BACKLOG_OVERFLOW: usize = 3;

/// The names of the keep-alive check's two intervals, for its messages.
const PROBE: &str = "the probe";
const RESET: &str = "the reset";

/// How long a check waits for something the network does.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const DEADLINE: Duration = Duration::from_secs(10);

/// How long data held back must stay unseen by the peer, well inside the 200 ms ceiling of a cork.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const HELD_FOR: Duration = Duration::from_millis(30);

/// How soon pushed data must arrive, under the cork's 200 ms ceiling so that the push and not the ceiling sent it.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const PUSHED_WITHIN: Duration = Duration::from_millis(150);

/// The owner of a container's first process, root.
///
/// https://man7.org/linux/man-pages/man7/capabilities.7.html
const OWNER: UserId = UserId(0);

/// The backlog of the listeners of most checks.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const BACKLOG: ListenBacklog = ListenBacklog(16);

/// The bytes most checks send.
const MESSAGE: &[u8] = b"conformance";

/// The bytes sent back the other way.
const REPLY: &[u8] = b"reply";

/// The urgent byte of the urgent data checks.
const URGENT: &[u8] = b"!";

/// How far into the unread bytes a peek at an offset starts.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const PEEK_OFFSET: usize = 2;

/// How many bytes a read takes when the rest must stay unread.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const READ_PART: usize = 4;

/// Room for the IP and TCP headers of a SYN.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const HEADER_ROOM: usize = 256;

/// The room a SYN's header is cut to.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const CUT_HEADER: usize = 3;

/// Room for the packet of an error queue entry.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const ENTRY_ROOM: usize = 64;

/// Room for an ICMP error whole from its IP header, or a transmit report's packet from its link header.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const PACKET_ROOM: usize = 512;

/// How long loopback takes at most to deliver what was sent.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const SETTLE: Duration = Duration::from_millis(50);

/// A loopback address that connections to it do not leave from, so that their two ends differ.
///
/// https://www.rfc-editor.org/rfc/rfc6890
const OTHER_LOOPBACK: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 2);

/// The hop limit of the IPv6 header an ICMPv6 error quotes, Linux's default.
///
/// https://docs.kernel.org/networking/ip-sysctl.html
const QUOTED_HOP_LIMIT: u8 = 64;

/// The bytes the order check sends one way.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const TRANSFER: usize = 1 << 20;

/// The key the transmit report checks name their sends by.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const KEY: TransmitReportKey = TransmitReportKey(7);

/// A maximum segment size well below loopback's.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const SEGMENT: MaximumSegmentSize = MaximumSegmentSize(1000);

/// The first port of the range the exit node chooses ports from.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const CHOICE_FIRST: Port = Port(41000);
/// The last port of the reserved part at the range's start.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const RESERVED_LAST: Port = Port(41010);
/// The last port of the range the exit node chooses ports from.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const CHOICE_LAST: Port = Port(41020);

/// The first port of a range above Linux's default local port range, which ends at 60999.
///
/// https://docs.kernel.org/networking/ip-sysctl.html
const HIGH_CHOICE_FIRST: Port = Port(61000);
/// The last port of the reserved part at the high range's start.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const HIGH_RESERVED_LAST: Port = Port(61010);
/// The last port of the high range.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const HIGH_CHOICE_LAST: Port = Port(61020);

/// The fast open queue a listener takes.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const FAST_OPEN_QUEUE: Option<NonZeroU32> = NonZeroU32::new(8);

/// A time every setting of a time accepts.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const SETTING_TIME: Duration = Duration::from_secs(10);
/// A time under a second that the settings of a time in milliseconds accept.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const SHORT_SETTING_TIME: Duration = Duration::from_millis(40);
/// A count every setting of a count accepts.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const SETTING_COUNT: u8 = 4;
/// A size every setting of a size accepts.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const SETTING_BYTES: usize = 1 << 16;

/// A congestion control algorithm every Linux has.
const CONGESTION_CONTROL: &str = "reno";

/// Classic BPF's return of its operand, which ends a filter.
const RETURN_OPERAND: FilterOperation = FilterOperation((if_tun::BPF_RET | if_tun::BPF_K) as u16);

/// A filter that keeps every packet whole.
static ACCEPT_ALL: [FilterInstruction; 1] = [FilterInstruction {
    operation: RETURN_OPERAND,
    jump_if_true: FilterJump(0),
    jump_if_false: FilterJump(0),
    operand: FilterOperand(u32::MAX),
}];

/// A filter that drops every packet.
static DROP_ALL: [FilterInstruction; 1] = [FilterInstruction {
    operation: RETURN_OPERAND,
    jump_if_true: FilterJump(0),
    jump_if_false: FilterJump(0),
    operand: FilterOperand(0),
}];

/// How long a fill waits for a send to take anything before it counts the connection full.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const FILLED_AFTER: Duration = Duration::from_millis(100);

/// How many times less a setting that bounds a buffer lets through than the default buffers.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const BOUND_RATIO: usize = 4;

/// A receive low water mark that widens the receive buffer to hold it.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const WIDE_LOW_WATER: usize = 1 << 20;

/// How far from its time a timer of a second or so may fire on a loaded machine.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const TIMER_SLACK: Duration = Duration::from_millis(400);

/// How long the keep-alive checks' connections stay idle before the first probe.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const KEEP_ALIVE_IDLE: Duration = Duration::from_secs(2);
/// The probe counts the keep-alive check tries: the least, and one that shows each probe counted.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const KEEP_ALIVE_PROBE_COUNTS: &[u8] = &[1, 2];

/// How long apart the keep-alive checks' probes go.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(1);

/// The longest a quick acknowledgement takes on loopback, below the shortest delay of a delayed one.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const QUICK_WITHIN: Duration = Duration::from_millis(20);

/// The longest an acknowledgement may wait in the delayed acknowledgement check.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const DELAYED_ACKNOWLEDGEMENT_MAXIMUM: Duration = Duration::from_millis(5);

/// The longest retransmission timeout of the check of the maximum.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const RETRANSMISSION_CEILING: Duration = Duration::from_secs(1);
/// How long the checks of retransmission timeouts watch retransmissions.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const RETRANSMISSIONS_WATCHED: Duration = Duration::from_millis(3200);

/// How many times a segment leaves within the given time when its timeouts do not grow, more than when they double.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const LINEAR_TRANSMISSIONS: usize = 6;
/// The time the linear transmissions are counted in.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const LINEAR_TRANSMISSIONS_WITHIN: Duration = Duration::from_millis(1200);

/// How long sent data may stay unacknowledged before the user timeout check's connection ends.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const USER_TIMEOUT: Duration = Duration::from_millis(500);

/// How long the FIN_WAIT2 check's connection stays in that state.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const FIN_WAIT_2_LASTS: Duration = Duration::from_secs(1);

/// A congestion control that marks what it sends as capable of explicit congestion notification.
const NOTIFYING_CONGESTION_CONTROL: &str = "dctcp";

/// A pacing rate that spreads the paced bytes over more than the given time.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const PACING_RATE: BytesPerSecond = BytesPerSecond(1 << 20);
/// The bytes the pacing check sends.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const PACED_BYTES: usize = 1 << 20;
/// The least time the paced bytes take.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const PACED_AT_LEAST: Duration = Duration::from_millis(400);

/// The delay the transmit delay check gives each segment.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const TRANSMIT_DELAY: Duration = Duration::from_millis(50);

/// An address of the documentation range, which no interface holds.
///
/// https://www.rfc-editor.org/rfc/rfc5737
const NONLOCAL: Ipv4Addr = Ipv4Addr::new(192, 0, 2, 1);

/// The loopback address of the check of fast open with data.
///
/// https://www.rfc-editor.org/rfc/rfc6890
const FAST_OPEN_WITH_DATA: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 3);
/// The loopback address of the check of fast open until the first send.
///
/// https://www.rfc-editor.org/rfc/rfc6890
const FAST_OPEN_UNTIL_FIRST_SEND: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 4);
/// The loopback address of the check of fast open without a cookie.
///
/// https://www.rfc-editor.org/rfc/rfc6890
const FAST_OPEN_WITHOUT_COOKIE: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 5);
/// The loopback address of the check of fast open keys.
///
/// https://www.rfc-editor.org/rfc/rfc6890
const FAST_OPEN_KEYS: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 6);

/// A TCP timestamp clock reading, in milliseconds.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const CLOCK_READING: u32 = 1 << 30;

/// How long a timestamp clock runs between being set and the segment that shows it.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const CLOCK_RUNS: Duration = Duration::from_millis(50);

/// A loopback address whose local route has the `tcp_usec_ts` feature: its connections' segments carry microseconds.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const MICROSECOND_CLOCK: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 7);

/// How long a check sleeps before it tries again a call that would block.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const RETRY_INTERVAL: Duration = Duration::from_millis(1);

/// The count of a waker woken once.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const ONE_WAKE: usize = 1;

/// The port that asks the host to choose one.
const ANY_PORT: Port = Port(u16::MIN);

/// A length or offset of no bytes.
const NO_BYTES: ByteCount = ByteCount(usize::MIN);

/// The room of an accept that keeps no SYN header.
const NO_HEADER_ROOM: usize = usize::MIN;

/// The backlog Linux raises to its smallest.
const SMALLEST_BACKLOG: ListenBacklog = ListenBacklog(u32::MIN);

/// The flow label of a segment that carries none.
const NO_FLOW_LABEL: u32 = u32::MIN;

/// The users of a group or address one socket joined, however often.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const ONE_MEMBER: usize = 1;

/// The SYN retries of the check that a connect ends unanswered, the fewest that retry at all.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const ONE_SYNCHRONIZE_RETRY: RetryCount = RetryCount(1);

/// How far a timestamp may stray: one tick of rounding, and the low bit the unit takes.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const TIMESTAMP_SLACK: u128 = 2;

/// The smallest receive low water mark, Linux's default.
///
/// https://man7.org/linux/man-pages/man7/socket.7.html
const SMALLEST_LOW_WATER: ByteCount = ByteCount(1);

/// The smallest minimum hop limit that turns the check on.
///
/// https://man7.org/linux/man-pages/man7/ip.7.html
const SMALLEST_HOP_LIMIT: HopLimit = HopLimit(1);

fn errno(errno: Errno) -> HostError {
    HostError::Errno(errno as i32)
}

/// Tries the operation again while it answers `WouldBlock`, until the deadline.
fn retry<T>(mut operation: impl FnMut() -> Result<T, HostError>) -> Result<T, HostError> {
    let start = Instant::now();
    loop {
        match operation() {
            Err(HostError::WouldBlock) if start.elapsed() < DEADLINE => {
                std::thread::sleep(RETRY_INTERVAL)
            }
            outcome => return outcome,
        }
    }
}

fn bind_any_port(address: BindAddress) -> LocalBinding<'static> {
    LocalBinding {
        address,
        port: LocalPort::Requested(ANY_PORT),
    }
}

fn ipv4_loopback() -> BindAddress {
    BindAddress::V4(Ipv4Addr::LOCALHOST)
}

fn ipv4_any() -> BindAddress {
    BindAddress::V4(Ipv4Addr::UNSPECIFIED)
}

fn no_reports() -> TransmitReportRequest {
    TransmitReportRequest {
        key: TransmitReportKey(u32::MIN),
        scheduled: Report::NotRequested,
        software: Report::NotRequested,
        hardware: Report::NotRequested,
        acknowledgement: Report::NotRequested,
        completion: Report::NotRequested,
        wireless_status: Report::NotRequested,
        packet: Report::NotRequested,
    }
}

fn plain() -> TcpSendRequest {
    TcpSendRequest {
        urgency: TcpUrgency::Normal,
        continuation: TcpContinuation::Push,
        record: TcpRecordBoundary::Open,
        transmit_reports: no_reports(),
        transmit_statistics: Report::NotRequested,
    }
}

/// A listener, a connection to it and the connection it accepted.
struct Pair<S: TcpSockets> {
    listener: S::Listener,
    local: SocketAddr,
    client: S::Connection,
    server: S::Connection,
}

fn listen<S: TcpSockets>(
    sockets: &S,
    address: BindAddress,
    settings: &[TcpListenerSetting<'_>],
) -> (S::Listener, SocketAddr) {
    let listening = sockets
        .listen(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(address),
            BACKLOG,
            settings,
        )
        .expect("listen on loopback");
    (listening.listener, listening.local)
}

fn accept<S: TcpSockets>(sockets: &S, listener: &S::Listener) -> S::Connection {
    let mut header = [u8::MIN; 0];
    retry(|| {
        sockets
            .accept(listener, &mut header)
            .map(|accepted| accepted.connection)
    })
    .expect("accept the connection")
}

/// What an accept gave, the SYN's header copied out of the buffer it was cut to.
struct Accepted<C> {
    connection: C,
    peer: SocketAddr,
    local: SocketAddr,
    synchronize: ReceivedPacketFields,
    synchronize_header: Vec<u8>,
}

fn accept_with_header<S: TcpSockets>(
    sockets: &S,
    listener: &S::Listener,
    room: usize,
) -> Accepted<S::Connection> {
    let mut header = vec![u8::MIN; room];
    retry(|| {
        sockets
            .accept(listener, &mut header)
            .map(|accepted| Accepted {
                connection: accepted.connection,
                peer: accepted.peer,
                local: accepted.local,
                synchronize: accepted.synchronize,
                synchronize_header: accepted.synchronize_header.to_vec(),
            })
    })
    .expect("accept")
}

/// Sends the bytes from one connection while reading them at the other, returning what arrived.
fn transfer<S: TcpSockets>(
    sockets: &S,
    from: &S::Connection,
    to: &S::Connection,
    bytes: &[u8],
) -> Vec<u8> {
    let mut sent = usize::MIN;
    let mut received = vec![u8::MIN; bytes.len()];
    let mut length = usize::MIN;
    let start = Instant::now();
    while length < bytes.len() && start.elapsed() < DEADLINE {
        if sent < bytes.len() {
            match sockets.send(from, plain(), &bytes[sent..]) {
                Ok(taken) => sent += taken.0,
                Err(HostError::WouldBlock) => {}
                Err(error) => panic!("send: {error:?}"),
            }
        }
        match sockets.receive(to, TcpReceiveRequest::Read, &mut received[length..]) {
            Ok(got) if got.length.0 == 0 => break,
            Ok(got) => length += got.length.0,
            Err(HostError::WouldBlock) => {}
            Err(error) => panic!("receive: {error:?}"),
        }
    }
    received.truncate(length);
    received
}

fn pair_with<S: TcpSockets>(
    sockets: &S,
    client_settings: &[TcpSetting<'_>],
    listener_settings: &[TcpListenerSetting<'_>],
) -> Pair<S> {
    pair_at(sockets, ipv4_loopback(), client_settings, listener_settings)
}

fn pair_at<S: TcpSockets>(
    sockets: &S,
    address: BindAddress,
    client_settings: &[TcpSetting<'_>],
    listener_settings: &[TcpListenerSetting<'_>],
) -> Pair<S> {
    let (listener, local) = listen(sockets, address, listener_settings);
    let client = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(any_of(address)),
            local,
            client_settings,
            TcpFastOpen::Off,
        )
        .expect("connect to the listener")
        .connection;
    let server = accept(sockets, &listener);
    Pair {
        listener,
        local,
        client,
        server,
    }
}

fn pair<S: TcpSockets>(sockets: &S) -> Pair<S> {
    pair_with(sockets, &[], &[])
}

fn close<S: TcpSockets>(sockets: &S, pair: Pair<S>) {
    sockets.close(pair.client, TcpClose::Orderly);
    sockets.close(pair.server, TcpClose::Orderly);
    sockets.stop_listening(pair.listener);
}

fn send_all<S: TcpSockets>(
    sockets: &S,
    connection: &S::Connection,
    request: TcpSendRequest,
    bytes: &[u8],
) {
    let mut sent = usize::MIN;
    while sent < bytes.len() {
        let taken = retry(|| sockets.send(connection, request, &bytes[sent..])).expect("send");
        sent += taken.0;
    }
}

/// Reads until the given number of bytes or the end of the stream.
fn receive_up_to<S: TcpSockets>(sockets: &S, connection: &S::Connection, wanted: usize) -> Vec<u8> {
    let mut received = vec![u8::MIN; wanted];
    let mut length = usize::MIN;
    while length < wanted {
        let got =
            retry(|| sockets.receive(connection, TcpReceiveRequest::Read, &mut received[length..]))
                .expect("receive");
        if got.length.0 == 0 {
            break;
        }
        length += got.length.0;
    }
    received.truncate(length);
    received
}

/// Peeks until the given number of bytes wait to be read.
fn wait_for_unread<S: TcpSockets>(sockets: &S, connection: &S::Connection, wanted: usize) {
    let mut buffer = vec![u8::MIN; wanted];
    let start = Instant::now();
    loop {
        match sockets.receive(
            connection,
            TcpReceiveRequest::Peek { offset: NO_BYTES },
            &mut buffer,
        ) {
            Ok(got) if got.length.0 >= wanted => return,
            Ok(_) | Err(HostError::WouldBlock) if start.elapsed() < DEADLINE => {
                std::thread::sleep(RETRY_INTERVAL)
            }
            outcome => panic!("waiting for {wanted} unread bytes: {outcome:?}"),
        }
    }
}

/// Asserts that nothing arrives on the connection for the given time.
fn assert_silent<S: TcpSockets>(sockets: &S, connection: &S::Connection, period: Duration) {
    let start = Instant::now();
    let mut buffer = [u8::MIN; 1];
    while start.elapsed() < period {
        let outcome = sockets.receive(
            connection,
            TcpReceiveRequest::Peek { offset: NO_BYTES },
            &mut buffer,
        );
        assert_eq!(
            outcome.map(|got| got.length),
            Err(HostError::WouldBlock),
            "data arrived while held back"
        );
        std::thread::sleep(RETRY_INTERVAL);
    }
}

/// Asserts that the bytes arrive within the given time.
fn assert_arrives_within<S: TcpSockets>(
    sockets: &S,
    connection: &S::Connection,
    bytes: &[u8],
    within: Duration,
) {
    let start = Instant::now();
    let mut buffer = vec![u8::MIN; bytes.len()];
    let got = retry(|| sockets.receive(connection, TcpReceiveRequest::Read, &mut buffer))
        .expect("receive");
    assert!(start.elapsed() < within, "took {:?}", start.elapsed());
    assert_eq!(&buffer[..got.length.0], bytes);
}

fn pattern(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| (index % usize::from(u8::MAX)) as u8)
        .collect()
}

/// Sends a byte each way and checks that both arrive.
fn assert_carries_bytes<S: TcpSockets>(sockets: &S, pair: &Pair<S>) {
    send_all(sockets, &pair.client, plain(), MESSAGE);
    assert_eq!(receive_up_to(sockets, &pair.server, MESSAGE.len()), MESSAGE);
    send_all(sockets, &pair.server, plain(), REPLY);
    assert_eq!(receive_up_to(sockets, &pair.client, REPLY.len()), REPLY);
}

/// The TCP option kind of Multipath TCP (RFC 8684, section 3), which neither libc nor linux-raw-sys carries.
///
/// https://www.rfc-editor.org/rfc/rfc8684#section-3
const MULTIPATH_OPTION: u8 = 30;

/// The TCP option kind of an MD5 signature, which etherparse does not parse.
///
/// https://www.rfc-editor.org/rfc/rfc2385
const MD5_OPTION: u8 = 19;

/// The TCP option kind of the TCP Authentication Option (RFC 5925, section 2.2), which etherparse does not parse.
///
/// https://www.rfc-editor.org/rfc/rfc5925#section-2.2
const AUTHENTICATION_OPTION: u8 = 29;

/// The bytes of a TCP Authentication Option before its MAC: kind, length, key identifier and next key identifier.
///
/// https://www.rfc-editor.org/rfc/rfc5925#section-2.2
const AUTHENTICATION_OPTION_HEADER: u8 = 4;

/// The kind and length bytes every TCP option but the end and no-operation starts with.
///
/// https://www.rfc-editor.org/rfc/rfc9293#section-3.1
const OPTION_HEAD: u8 = 2;

/// A segment's TCP Authentication Option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AuthenticationOption {
    key: u8,
    next_key: u8,
    length: u8,
}

/// Gives the TCP Authentication Option among a segment's options, walked as RFC 9293 section 3.1 lays them out.
fn authentication_option(options: &[u8]) -> Option<AuthenticationOption> {
    let mut rest = options;
    loop {
        match *rest {
            [kind, ..] if kind == tcp_option::KIND_END => return None,
            [kind, ..] if kind == tcp_option::KIND_NOOP => rest = &rest[1..],
            [kind, length, key, next_key, ..]
                if kind == AUTHENTICATION_OPTION && length >= AUTHENTICATION_OPTION_HEADER =>
            {
                return Some(AuthenticationOption {
                    key,
                    next_key,
                    length,
                });
            }
            [_, length, ..] if length >= OPTION_HEAD => {
                rest = rest.get(usize::from(length)..)?;
            }
            _ => return None,
        }
    }
}

/// One TCP segment loopback carried, as the host's packet socket saw it.
#[derive(Clone, Debug)]
struct Segment {
    at: Instant,
    to_listener: bool,
    hop_limit: u8,
    /// The IPv4 header's DSCP and ECN fields.
    dscp: Option<IpDscp>,
    ecn: Option<IpEcn>,
    flow_label: Option<u32>,
    dont_fragment: bool,
    sequence: u32,
    acknowledgement: u32,
    window: u16,
    synchronize: bool,
    finish: bool,
    reset: bool,
    payload: usize,
    timestamp: Option<u32>,
    /// Whether the segment carries a Multipath TCP option.
    multipath: bool,
    /// Whether the segment carries an MD5 signature option.
    signed: bool,
    authentication: Option<AuthenticationOption>,
}

impl Segment {
    fn parse(packet: &[u8], listener: SocketAddr, at: Instant) -> Option<Self> {
        let (source, destination, hop_limit, dscp, ecn, flow_label, dont_fragment, transport) =
            if let Ok(header) = Ipv4HeaderSlice::from_slice(packet) {
                let end = usize::from(header.total_len()).min(packet.len());
                (header.protocol() == IpNumber::TCP).then_some((
                    IpAddr::V4(header.source_addr()),
                    IpAddr::V4(header.destination_addr()),
                    header.ttl(),
                    Some(header.dcp()),
                    Some(header.ecn()),
                    None,
                    header.dont_fragment(),
                    &packet[header.slice().len()..end],
                ))?
            } else {
                let header = Ipv6HeaderSlice::from_slice(packet).ok()?;
                let end =
                    (Ipv6Header::LEN + usize::from(header.payload_length())).min(packet.len());
                (header.next_header() == IpNumber::TCP).then_some((
                    IpAddr::V6(header.source_addr()),
                    IpAddr::V6(header.destination_addr()),
                    header.hop_limit(),
                    None,
                    None,
                    Some(header.flow_label().value()),
                    false,
                    &packet[Ipv6Header::LEN..end],
                ))?
            };
        let tcp = TcpHeaderSlice::from_slice(transport).ok()?;
        let to_listener = SocketAddr::new(destination, tcp.destination_port()) == listener;
        if !to_listener && SocketAddr::new(source, tcp.source_port()) != listener {
            return None;
        }
        Some(Segment {
            at,
            to_listener,
            hop_limit,
            dscp,
            ecn,
            flow_label,
            dont_fragment,
            sequence: tcp.sequence_number(),
            acknowledgement: tcp.acknowledgment_number(),
            window: tcp.window_size(),
            synchronize: tcp.syn(),
            finish: tcp.fin(),
            reset: tcp.rst(),
            payload: transport.len() - tcp.slice().len(),
            timestamp: tcp.options_iterator().find_map(|option| match option {
                Ok(TcpOptionElement::Timestamp(value, _)) => Some(value),
                _ => None,
            }),
            multipath: tcp
                .options_iterator()
                .any(|option| option == Err(TcpOptionReadError::UnknownId(MULTIPATH_OPTION))),
            signed: tcp
                .options_iterator()
                .any(|option| option == Err(TcpOptionReadError::UnknownId(MD5_OPTION))),
            authentication: authentication_option(tcp.options()),
        })
    }

    /// Gives the sequence number that follows this segment.
    fn end(&self) -> u32 {
        let flags = u32::from(self.synchronize) + u32::from(self.finish);
        self.sequence.wrapping_add(self.payload as u32 + flags)
    }

    fn is_data_to_listener(&self) -> bool {
        self.to_listener && self.payload > 0
    }
}

/// The segments to and from one listener, gathered by a thread reading the host's packet socket, which sees each loopback packet twice.
///
/// Once leaving and once arriving.
struct Capture {
    segments: Arc<Mutex<Vec<Segment>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// Opens the host's packet socket for every protocol, with a buffer of [`CAPTURE_BUFFER`].
#[cfg(target_os = "linux")]
fn packet_socket() -> OwnedFd {
    let socket = rustix::net::socket(
        AddressFamily::PACKET,
        SocketType::DGRAM,
        Some(rustix::net::eth::ALL),
    )
    .expect("a packet socket (the checks that watch the wire need root)");
    sockopt::set_socket_recv_buffer_size_force(&socket, CAPTURE_BUFFER).expect("a capture buffer");
    socket
}

/// Fails the check: only Linux has the packet socket the checks that watch the wire read.
#[cfg(not(target_os = "linux"))]
fn packet_socket() -> OwnedFd {
    panic!("the checks that watch the wire need the packet socket of a Linux host");
}

/// How long apart the two copies of one loopback packet may arrive at the packet socket.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const COPY_GAP: Duration = Duration::from_millis(5);

/// The buffer of a capture's packet socket, large enough for every check's traffic at once.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const CAPTURE_BUFFER: usize = 1 << 24;

impl Capture {
    fn start(listener: SocketAddr) -> Self {
        let socket = packet_socket();
        sockopt::set_socket_timeout(&socket, sockopt::Timeout::Recv, Some(COPY_GAP))
            .expect("a receive timeout");
        let segments = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = std::thread::spawn({
            let segments = segments.clone();
            let stop = stop.clone();
            move || {
                let mut buffer = vec![u8::MIN; usize::from(u16::MAX)];
                let mut recent: Vec<(Instant, Vec<u8>)> = Vec::new();
                while !stop.load(Ordering::Relaxed) {
                    let Ok((length, _)) =
                        rustix::net::recv(&socket, &mut buffer, RecvFlags::empty())
                    else {
                        continue;
                    };
                    let at = Instant::now();
                    let packet = &buffer[..length];
                    let Some(segment) = Segment::parse(packet, listener, at) else {
                        continue;
                    };
                    recent.retain(|(seen, _)| at.duration_since(*seen) < COPY_GAP);
                    if let Some(copy) = recent.iter().position(|(_, bytes)| bytes == packet) {
                        recent.swap_remove(copy);
                        continue;
                    }
                    recent.push((at, packet.to_vec()));
                    segments.lock().expect("the capture").push(segment);
                }
            }
        });
        Capture {
            segments,
            stop,
            thread: Some(thread),
        }
    }

    fn segments(&self) -> Vec<Segment> {
        self.segments.lock().expect("the capture").clone()
    }

    fn since(&self, start: Instant) -> Vec<Segment> {
        let mut segments = self.segments();
        segments.retain(|segment| segment.at >= start);
        segments
    }

    /// Gives the first segment since the start that meets the condition, waiting for it up to the given time.
    fn wait_for(
        &self,
        start: Instant,
        within: Duration,
        condition: impl Fn(&Segment) -> bool,
    ) -> Option<Segment> {
        loop {
            if let Some(found) = self.since(start).into_iter().find(&condition) {
                return Some(found);
            }
            if start.elapsed() > within {
                return None;
            }
            std::thread::sleep(RETRY_INTERVAL);
        }
    }

    /// Gives the first segment captured that meets the condition, waiting for it up to the deadline.
    fn first(&self, condition: impl Fn(&Segment) -> bool) -> Segment {
        let start = Instant::now();
        loop {
            if let Some(found) = self.segments().into_iter().find(&condition) {
                return found;
            }
            assert!(start.elapsed() < DEADLINE, "no such segment was captured");
            std::thread::sleep(RETRY_INTERVAL);
        }
    }

    /// Gives the sequence number the connection's next segment to the listener takes.
    fn next_to_listener(&self) -> u32 {
        self.first(|segment| segment.to_listener);
        self.segments()
            .iter()
            .filter(|segment| segment.to_listener)
            .map(Segment::end)
            .next_back()
            .expect("a segment to the listener")
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn any_of(address: BindAddress) -> BindAddress {
    match address {
        BindAddress::V4(_) => ipv4_any(),
        BindAddress::V6 { families, .. } => BindAddress::V6 {
            address: Ipv6Addr::UNSPECIFIED,
            families,
        },
    }
}

fn ipv6_loopback() -> BindAddress {
    BindAddress::V6 {
        address: Ipv6Addr::LOCALHOST,
        families: Ipv6Families::V6Only,
    }
}

/// Gives a pair on the given loopback address whose segments are captured from the SYN on.
fn watched_pair<S: TcpSockets>(
    sockets: &S,
    address: BindAddress,
    client_settings: &[TcpSetting<'_>],
    listener_settings: &[TcpListenerSetting<'_>],
) -> (Pair<S>, Capture) {
    let (listener, local) = listen(sockets, address, listener_settings);
    let capture = Capture::start(local);
    let client = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(any_of(address)),
            local,
            client_settings,
            TcpFastOpen::Off,
        )
        .expect("connect to the listener")
        .connection;
    let server = accept(sockets, &listener);
    let pair = Pair {
        listener,
        local,
        client,
        server,
    };
    (pair, capture)
}

/// Makes the connection drop every segment that arrives, by a minimum hop limit no packet meets.
fn drop_arrivals<S: TcpSockets>(sockets: &S, connection: &S::Connection, version: IpVersion) {
    sockets
        .configure(
            connection,
            TcpSetting::MinimumHopLimit {
                version,
                minimum: Some(HopLimit(u8::MAX)),
            },
        )
        .expect("a minimum hop limit");
}

/// Sends until the connection takes nothing more for a while, the peer not reading, giving the bytes taken.
fn fill<S: TcpSockets>(sockets: &S, connection: &S::Connection) -> usize {
    let chunk = pattern(SETTING_BYTES);
    let mut taken = usize::MIN;
    let mut last = Instant::now();
    while last.elapsed() < FILLED_AFTER {
        match sockets.send(connection, plain(), &chunk) {
            Ok(count) => {
                taken += count.0;
                last = Instant::now();
            }
            Err(HostError::WouldBlock) => std::thread::sleep(RETRY_INTERVAL),
            Err(error) => panic!("send: {error:?}"),
        }
    }
    taken
}

/// Gives the bytes waiting to be read on the connection.
fn unread<S: TcpSockets>(sockets: &S, connection: &S::Connection) -> usize {
    let mut buffer = vec![u8::MIN; CAPTURE_BUFFER];
    sockets
        .receive(
            connection,
            TcpReceiveRequest::Peek { offset: NO_BYTES },
            &mut buffer,
        )
        .map_or(usize::MIN, |got| got.length.0)
}

/// Gives how long after the send that started it a connection's first acknowledgement of data came.
fn acknowledgement_delay<S: TcpSockets>(
    sockets: &S,
    pair: &Pair<S>,
    capture: &Capture,
) -> Duration {
    let start = Instant::now();
    send_all(sockets, &pair.client, plain(), MESSAGE);
    let data = capture
        .wait_for(start, DEADLINE, Segment::is_data_to_listener)
        .expect("the data segment");
    let acknowledgement = capture
        .wait_for(start, DEADLINE, |segment| {
            !segment.to_listener && segment.acknowledgement == data.end()
        })
        .expect("its acknowledgement");
    acknowledgement.at - data.at
}

/// Waits until the receive answers something other than `WouldBlock`.
fn receive_outcome<S: TcpSockets>(
    sockets: &S,
    connection: &S::Connection,
    within: Duration,
) -> Result<TcpReceived, HostError> {
    let start = Instant::now();
    let mut buffer = [u8::MIN; ENTRY_ROOM];
    loop {
        match sockets.receive(connection, TcpReceiveRequest::Read, &mut buffer) {
            Err(HostError::WouldBlock) if start.elapsed() < within => {
                std::thread::sleep(RETRY_INTERVAL)
            }
            outcome => return outcome,
        }
    }
}

pub fn connect_and_accept_name_both_ends<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[],
            TcpFastOpen::Off,
        )
        .expect("connect");
    assert_eq!(
        connected.start,
        TcpConnectStart::SynchronizeSent {
            fast_open_data_taken: NO_BYTES
        }
    );
    let accepted = accept_with_header(&sockets, &listener, HEADER_ROOM);
    assert_eq!(accepted.peer, connected.local);
    assert_eq!(accepted.local, local);
    let ipv4 =
        Ipv4HeaderSlice::from_slice(&accepted.synchronize_header).expect("the SYN's IPv4 header");
    assert_eq!(ipv4.protocol(), IpNumber::TCP);
    assert_eq!(IpAddr::V4(ipv4.source_addr()), connected.local.ip());
    assert_eq!(accepted.synchronize.length, usize::from(ipv4.total_len()));
    if let Some(frame) = accepted.synchronize.link_layer_length {
        assert!(usize::try_from(frame.0).is_ok_and(|frame| frame >= accepted.synchronize.length));
    }
    let tcp = TcpHeaderSlice::from_slice(&accepted.synchronize_header[ipv4.slice().len()..])
        .expect("the SYN's TCP header");
    assert!(tcp.syn() && !tcp.ack());
    assert_eq!(tcp.source_port(), connected.local.port());
    assert_eq!(tcp.destination_port(), local.port());
    sockets.close(accepted.connection, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

pub fn accept_cuts_the_synchronize_header_to_the_buffer<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[],
            TcpFastOpen::Off,
        )
        .expect("connect");
    let accepted = accept_with_header(&sockets, &listener, CUT_HEADER);
    assert_eq!(accepted.synchronize_header.len(), CUT_HEADER);
    assert!(accepted.synchronize.length > CUT_HEADER);
    sockets.close(accepted.connection, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

pub fn connect_settings_apply_to_the_synchronize<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    let segment = SEGMENT;
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[TcpSetting::MaximumSegmentSize(Some(segment))],
            TcpFastOpen::Off,
        )
        .expect("connect");
    let accepted = accept_with_header(&sockets, &listener, HEADER_ROOM);
    let ipv4 = Ipv4HeaderSlice::from_slice(&accepted.synchronize_header).expect("IPv4 header");
    let tcp = TcpHeaderSlice::from_slice(&accepted.synchronize_header[ipv4.slice().len()..])
        .expect("TCP header");
    let announced = tcp.options_iterator().find_map(|option| match option {
        Ok(TcpOptionElement::MaximumSegmentSize(size)) => Some(size),
        _ => None,
    });
    assert_eq!(announced, Some(segment.0));
    sockets.close(accepted.connection, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

pub fn bytes_arrive_in_order_both_ways<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let bytes = pattern(TRANSFER);
    assert!(transfer(&sockets, &pair.client, &pair.server, &bytes) == bytes);
    send_all(&sockets, &pair.server, plain(), REPLY);
    assert_eq!(receive_up_to(&sockets, &pair.client, REPLY.len()), REPLY);
    close(&sockets, pair);
}

pub fn connects_over_ipv6<S: TcpSockets>(sockets: S) {
    let loopback = BindAddress::V6 {
        address: Ipv6Addr::LOCALHOST,
        families: Ipv6Families::V6Only,
    };
    let (listener, local) = listen(&sockets, loopback, &[]);
    let any = BindAddress::V6 {
        address: Ipv6Addr::UNSPECIFIED,
        families: Ipv6Families::V6Only,
    };
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(any),
            local,
            &[],
            TcpFastOpen::Off,
        )
        .expect("connect");
    let server = accept(&sockets, &listener);
    send_all(&sockets, &connected.connection, plain(), MESSAGE);
    assert_eq!(receive_up_to(&sockets, &server, MESSAGE.len()), MESSAGE);
    assert_eq!(connected.local.ip(), IpAddr::V6(Ipv6Addr::LOCALHOST));
    sockets.close(server, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

pub fn dual_stack_listener_accepts_ipv4<S: TcpSockets>(sockets: S) {
    let any = BindAddress::V6 {
        address: Ipv6Addr::UNSPECIFIED,
        families: Ipv6Families::DualStack,
    };
    let (listener, local) = listen(&sockets, any, &[]);
    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), local.port());
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            peer,
            &[],
            TcpFastOpen::Off,
        )
        .expect("connect");
    let accepted = accept_with_header(&sockets, &listener, NO_HEADER_ROOM);
    assert_eq!(
        accepted.peer.ip(),
        IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped())
    );
    assert_eq!(accepted.peer.port(), connected.local.port());
    sockets.close(accepted.connection, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

/// Tells whether the SYN and the SYN-ACK of the first handshake captured since the start offer Multipath TCP.
fn handshake_offers_multipath(capture: &Capture, start: Instant) -> (bool, bool) {
    let offers = |to_listener: bool| {
        capture
            .wait_for(start, DEADLINE, |segment| {
                segment.synchronize && segment.to_listener == to_listener
            })
            .expect("the handshake was captured")
            .multipath
    };
    (offers(true), offers(false))
}

pub fn multipath_is_negotiated_when_both_ends_ask<S: TcpSockets>(sockets: S) {
    let listen = |protocol| {
        sockets
            .listen(
                protocol,
                OWNER,
                bind_any_port(ipv4_loopback()),
                BACKLOG,
                &[],
            )
            .expect("listen")
    };
    let multipath = listen(TcpProtocol::Multipath);
    let single = listen(TcpProtocol::Tcp);
    let captures = [
        Capture::start(multipath.local),
        Capture::start(single.local),
    ];
    for (client, listening, capture, expected) in [
        (
            TcpProtocol::Multipath,
            &multipath,
            &captures[0],
            (true, true),
        ),
        (TcpProtocol::Tcp, &multipath, &captures[0], (false, false)),
        (TcpProtocol::Multipath, &single, &captures[1], (true, false)),
    ] {
        let start = Instant::now();
        let connected = sockets
            .connect(
                client,
                OWNER,
                bind_any_port(ipv4_any()),
                listening.local,
                &[],
                TcpFastOpen::Off,
            )
            .expect("connect");
        let server = accept(&sockets, &listening.listener);
        assert_eq!(
            handshake_offers_multipath(capture, start),
            expected,
            "the SYN's and the SYN-ACK's offers of a {client:?} client"
        );
        send_all(&sockets, &connected.connection, plain(), MESSAGE);
        assert_eq!(receive_up_to(&sockets, &server, MESSAGE.len()), MESSAGE);
        sockets.close(server, TcpClose::Orderly);
        sockets.close(connected.connection, TcpClose::Orderly);
    }
    sockets.stop_listening(multipath.listener);
    sockets.stop_listening(single.listener);
}

pub fn requested_port_is_bound<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    sockets.stop_listening(listener);
    let binding = LocalBinding {
        address: ipv4_loopback(),
        port: LocalPort::Requested(Port(local.port())),
    };
    let listening = sockets
        .listen(TcpProtocol::Tcp, OWNER, binding, BACKLOG, &[])
        .expect("listen on the port");
    assert_eq!(listening.local, local);
    sockets.stop_listening(listening.listener);
}

/// Checks that the exit node's port comes from its range outside the reserved part, also where the range lies above the host's own.
pub fn exit_node_chooses_a_port_outside_the_reserved<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    for (first, reserved_last, last) in [
        (CHOICE_FIRST, RESERVED_LAST, CHOICE_LAST),
        (HIGH_CHOICE_FIRST, HIGH_RESERVED_LAST, HIGH_CHOICE_LAST),
    ] {
        let reserved = [PortRange {
            first,
            last: reserved_last,
        }];
        let choice = PortChoice {
            range: PortRange { first, last },
            reserved: &reserved,
        };
        let allowed = reserved_last.0 + 1..=last.0;
        let connected = sockets
            .connect(
                TcpProtocol::Tcp,
                OWNER,
                LocalBinding {
                    address: ipv4_any(),
                    port: LocalPort::ChosenByExitNode(choice),
                },
                local,
                &[],
                TcpFastOpen::Off,
            )
            .expect("connect");
        let port = connected.local.port();
        assert!(allowed.contains(&port), "port {port}");
        let server = accept(&sockets, &listener);
        sockets.close(server, TcpClose::Orderly);
        sockets.close(connected.connection, TcpClose::Orderly);
        let listening = sockets
            .listen(
                TcpProtocol::Tcp,
                OWNER,
                LocalBinding {
                    address: ipv4_loopback(),
                    port: LocalPort::ChosenByExitNode(choice),
                },
                BACKLOG,
                &[],
            )
            .expect("listen");
        let port = listening.local.port();
        assert!(allowed.contains(&port), "port {port}");
        sockets.stop_listening(listening.listener);
    }
    sockets.stop_listening(listener);
}

pub fn stopped_listener_refuses_connections<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    sockets.stop_listening(listener);
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[],
            TcpFastOpen::Off,
        )
        .expect("start connecting");
    let mut buffer = [u8::MIN; 1];
    let outcome =
        retry(|| sockets.receive(&connected.connection, TcpReceiveRequest::Read, &mut buffer));
    assert_eq!(
        outcome.map(|got| got.length),
        Err(errno(Errno::ECONNREFUSED))
    );
    sockets.close(connected.connection, TcpClose::Orderly);
}

pub fn accept_without_a_connection_would_block<S: TcpSockets>(sockets: S) {
    let (listener, _) = listen(&sockets, ipv4_loopback(), &[]);
    let mut header = [u8::MIN; 0];
    let outcome = sockets
        .accept(&listener, &mut header)
        .map(|accepted| accepted.peer);
    assert_eq!(outcome, Err(HostError::WouldBlock));
    sockets.stop_listening(listener);
}

pub fn write_shutdown_ends_the_peers_stream<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    sockets
        .shut_down(&pair.client, TcpShutdown::Write)
        .expect("shut down");
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len() + 1),
        MESSAGE,
        "then the end"
    );
    send_all(&sockets, &pair.server, plain(), REPLY);
    assert_eq!(receive_up_to(&sockets, &pair.client, REPLY.len()), REPLY);
    assert_eq!(
        sockets.send(&pair.client, plain(), MESSAGE),
        Err(errno(Errno::EPIPE))
    );
    close(&sockets, pair);
}

pub fn read_shutdown_ends_the_own_stream<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .shut_down(&pair.server, TcpShutdown::Read)
        .expect("shut down");
    let mut buffer = [u8::MIN; 1];
    let got = sockets
        .receive(&pair.server, TcpReceiveRequest::Read, &mut buffer)
        .expect("receive");
    assert_eq!(got.length, NO_BYTES);
    send_all(&sockets, &pair.server, plain(), REPLY);
    assert_eq!(receive_up_to(&sockets, &pair.client, REPLY.len()), REPLY);
    close(&sockets, pair);
}

pub fn shutdown_of_both_directions_ends_both<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .shut_down(&pair.client, TcpShutdown::ReadAndWrite)
        .expect("shut down");
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        Vec::<u8>::new()
    );
    assert_eq!(
        sockets.send(&pair.client, plain(), MESSAGE),
        Err(errno(Errno::EPIPE))
    );
    let mut buffer = [u8::MIN; 1];
    let got = sockets
        .receive(&pair.client, TcpReceiveRequest::Read, &mut buffer)
        .expect("the own stream has ended");
    assert_eq!(got.length, NO_BYTES);
    close(&sockets, pair);
}

pub fn orderly_close_ends_the_peers_stream<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    sockets.close(pair.client, TcpClose::Orderly);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len() + 1),
        MESSAGE
    );
    sockets.close(pair.server, TcpClose::Orderly);
    sockets.stop_listening(pair.listener);
}

pub fn abort_resets_the_peer<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets.close(pair.client, TcpClose::Abort);
    let mut buffer = [u8::MIN; 1];
    let outcome = retry(|| sockets.receive(&pair.server, TcpReceiveRequest::Read, &mut buffer));
    assert_eq!(outcome.map(|got| got.length), Err(errno(Errno::ECONNRESET)));
    sockets.close(pair.server, TcpClose::Orderly);
    sockets.stop_listening(pair.listener);
}

pub fn peek_leaves_the_bytes<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    wait_for_unread(&sockets, &pair.server, MESSAGE.len());
    let mut buffer = vec![u8::MIN; MESSAGE.len()];
    let got = sockets
        .receive(
            &pair.server,
            TcpReceiveRequest::Peek { offset: NO_BYTES },
            &mut buffer,
        )
        .expect("peek");
    assert_eq!(&buffer[..got.length.0], MESSAGE);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    close(&sockets, pair);
}

pub fn peek_at_an_offset_skips_bytes<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    wait_for_unread(&sockets, &pair.server, MESSAGE.len());
    let mut buffer = vec![u8::MIN; MESSAGE.len()];
    let got = sockets
        .receive(
            &pair.server,
            TcpReceiveRequest::Peek {
                offset: ByteCount(PEEK_OFFSET),
            },
            &mut buffer,
        )
        .expect("peek");
    assert_eq!(&buffer[..got.length.0], &MESSAGE[PEEK_OFFSET..]);
    let got = sockets
        .receive(
            &pair.server,
            TcpReceiveRequest::Peek { offset: NO_BYTES },
            &mut buffer,
        )
        .expect("peek");
    assert_eq!(&buffer[..got.length.0], MESSAGE);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    close(&sockets, pair);
}

fn urgent() -> TcpSendRequest {
    TcpSendRequest {
        urgency: TcpUrgency::Urgent,
        ..plain()
    }
}

pub fn urgent_byte_is_read_apart<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    send_all(&sockets, &pair.client, urgent(), URGENT);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    let mut byte = vec![u8::MIN; URGENT.len()];
    let peeked = retry(|| {
        match sockets.receive(&pair.server, TcpReceiveRequest::PeekUrgentByte, &mut byte) {
            // Linux answers EINVAL until the urgent byte has arrived.
            Err(HostError::Errno(code)) if code == Errno::EINVAL as i32 => {
                Err(HostError::WouldBlock)
            }
            outcome => outcome,
        }
    })
    .expect("peek the urgent byte");
    assert_eq!((peeked.length.0, &byte[..]), (URGENT.len(), URGENT));
    let read = sockets
        .receive(&pair.server, TcpReceiveRequest::ReadUrgentByte, &mut byte)
        .expect("read the urgent byte");
    assert_eq!((read.length.0, &byte[..]), (URGENT.len(), URGENT));
    let again = sockets
        .receive(&pair.server, TcpReceiveRequest::ReadUrgentByte, &mut byte)
        .map(|got| got.length);
    assert_eq!(again, Err(errno(Errno::EINVAL)));
    close(&sockets, pair);
}

pub fn urgent_inline_keeps_the_byte_in_the_stream<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .configure(&pair.server, TcpSetting::UrgentInline(true))
        .expect("urgent inline");
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    send_all(&sockets, &pair.client, urgent(), URGENT);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len() + URGENT.len()),
        [MESSAGE, URGENT].concat()
    );
    close(&sockets, pair);
}

pub fn cork_holds_data_until_uncorked<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .configure(&pair.client, TcpSetting::Cork(true))
        .expect("cork");
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    assert_silent(&sockets, &pair.server, HELD_FOR);
    sockets
        .configure(&pair.client, TcpSetting::Cork(false))
        .expect("uncork");
    assert_arrives_within(&sockets, &pair.server, MESSAGE, PUSHED_WITHIN);
    close(&sockets, pair);
}

pub fn more_to_come_holds_data_until_pushed<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let more = TcpSendRequest {
        continuation: TcpContinuation::MoreToCome,
        ..plain()
    };
    send_all(&sockets, &pair.client, more, MESSAGE);
    assert_silent(&sockets, &pair.server, HELD_FOR);
    sockets
        .push_queued_data(&pair.client, TcpPushOverride::CorkAndNagle)
        .expect("push");
    assert_arrives_within(&sockets, &pair.server, MESSAGE, PUSHED_WITHIN);
    close(&sockets, pair);
}

pub fn push_ignoring_neither_keeps_the_cork<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    sockets
        .configure(&pair.client, TcpSetting::Cork(true))
        .expect("cork");
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    sockets
        .push_queued_data(&pair.client, TcpPushOverride::Neither)
        .expect("push");
    assert_silent(&sockets, &pair.server, HELD_FOR);
    sockets
        .push_queued_data(&pair.client, TcpPushOverride::CorkAndNagle)
        .expect("push past the cork");
    assert_arrives_within(&sockets, &pair.server, MESSAGE, PUSHED_WITHIN);
    // The cork stays: later data is held again.
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    assert_silent(&sockets, &pair.server, HELD_FOR);
    close(&sockets, pair);
}

pub fn push_ignoring_neither_sends_data_held_for_more<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let more = TcpSendRequest {
        continuation: TcpContinuation::MoreToCome,
        ..plain()
    };
    send_all(&sockets, &pair.client, more, MESSAGE);
    assert_silent(&sockets, &pair.server, HELD_FOR);
    sockets
        .push_queued_data(&pair.client, TcpPushOverride::Neither)
        .expect("push");
    assert_arrives_within(&sockets, &pair.server, MESSAGE, PUSHED_WITHIN);
    close(&sockets, pair);
}

/// Checks that two sends under a cork, the first ending a record: they leave as two segments, not one.
pub fn record_end_starts_a_new_segment<S: TcpSockets>(sockets: S) {
    let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), &[], &[]);
    sockets
        .configure(&pair.client, TcpSetting::Cork(true))
        .expect("cork");
    let start = Instant::now();
    let ended = TcpSendRequest {
        record: TcpRecordBoundary::Ended,
        ..plain()
    };
    send_all(&sockets, &pair.client, ended, MESSAGE);
    send_all(&sockets, &pair.client, plain(), REPLY);
    sockets
        .configure(&pair.client, TcpSetting::Cork(false))
        .expect("uncork");
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len() + REPLY.len()),
        [MESSAGE, REPLY].concat()
    );
    // The capture thread may record the segments after the data was read, so this waits until it holds every byte.
    let data_lengths = || -> Vec<usize> {
        capture
            .since(start)
            .iter()
            .filter(|segment| segment.is_data_to_listener())
            .map(|segment| segment.payload)
            .collect()
    };
    let mut lengths = data_lengths();
    while lengths.iter().sum::<usize>() < MESSAGE.len() + REPLY.len() && start.elapsed() < DEADLINE
    {
        std::thread::sleep(RETRY_INTERVAL);
        lengths = data_lengths();
    }
    assert_eq!(lengths, [MESSAGE.len(), REPLY.len()]);
    close(&sockets, pair);
}

pub fn unread_byte_reports_count_what_is_left<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let mut buffer = [u8::MIN; READ_PART];
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    wait_for_unread(&sockets, &pair.server, MESSAGE.len());
    let got = sockets
        .receive(&pair.server, TcpReceiveRequest::Read, &mut buffer)
        .expect("receive");
    assert_eq!(got.unread_after, None, "not asked for");
    sockets
        .configure(
            &pair.server,
            TcpSetting::UnreadByteReports(Report::Requested),
        )
        .expect("unread reports");
    let got = sockets
        .receive(&pair.server, TcpReceiveRequest::Read, &mut buffer)
        .expect("receive");
    assert_eq!(
        (got.length, got.unread_after),
        (
            ByteCount(READ_PART),
            Some(ByteCount(MESSAGE.len() - 2 * READ_PART))
        )
    );
    close(&sockets, pair);
}

/// Checks that a listener asked for unread byte reports gives them to the connections it accepts.
pub fn listener_unread_byte_reports_reach_the_connections_it_accepts<S: TcpSockets>(sockets: S) {
    let pair = pair_with(
        &sockets,
        &[],
        &[TcpListenerSetting::Socket(TcpSetting::UnreadByteReports(
            Report::Requested,
        ))],
    );
    let mut buffer = [u8::MIN; READ_PART];
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    wait_for_unread(&sockets, &pair.server, MESSAGE.len());
    let got = sockets
        .receive(&pair.server, TcpReceiveRequest::Read, &mut buffer)
        .expect("receive");
    assert_eq!(
        (got.length, got.unread_after),
        (
            ByteCount(READ_PART),
            Some(ByteCount(MESSAGE.len() - READ_PART))
        )
    );
    close(&sockets, pair);
}

pub fn receive_software_timestamps_when_asked<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let mut buffer = [u8::MIN; 1];
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    let got = retry(|| sockets.receive(&pair.server, TcpReceiveRequest::Read, &mut buffer))
        .expect("receive");
    assert_eq!(got.timestamps.software, None, "not asked for");
    let timestamping = Timestamping {
        receive_software: Report::Requested,
        receive_hardware: Report::NotRequested,
        bound_clock: None,
    };
    sockets
        .configure(
            &pair.server,
            TcpSetting::Ip(IpSocketSetting::Timestamping(timestamping)),
        )
        .expect("timestamping");
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    let got = retry(|| sockets.receive(&pair.server, TcpReceiveRequest::Read, &mut buffer))
        .expect("receive");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is past the epoch");
    let stamp = got.timestamps.software.expect("a software timestamp");
    assert!(
        stamp <= now && now - stamp < DEADLINE,
        "stamp {stamp:?} now {now:?}"
    );
    assert_eq!(got.timestamps.hardware, None);
    close(&sockets, pair);
}

fn take_entry<S: TcpSockets>(sockets: &S, connection: &S::Connection) -> TcpErrorQueueEntry {
    let mut buffer = [u8::MIN; ENTRY_ROOM];
    retry(|| sockets.take_error_queue_entry(connection, &mut buffer)).expect("an error queue entry")
}

pub fn software_transmit_report_names_the_send<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let key = KEY;
    let reported = TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key,
            software: Report::Requested,
            ..no_reports()
        },
        ..plain()
    };
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is past the epoch");
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    send_all(&sockets, &pair.client, reported, MESSAGE);
    let TcpErrorQueueEntry::TransmitReport { report, .. } = take_entry(&sockets, &pair.client)
    else {
        panic!("not a transmit report");
    };
    assert_eq!(report.key, key);
    let TransmitEvent::Timestamp {
        stage,
        source,
        time,
    } = report.event
    else {
        panic!("not a timestamp: {:?}", report.event);
    };
    assert_eq!(
        (stage, source),
        (TransmitStage::Sent, TimestampSource::Software)
    );
    assert!(time >= before && time - before < DEADLINE);
    let mut buffer = [u8::MIN; ENTRY_ROOM];
    assert_eq!(
        sockets.take_error_queue_entry(&pair.client, &mut buffer),
        Err(HostError::WouldBlock)
    );
    close(&sockets, pair);
}

/// Sends that ask for no report, between two that do, leave each report naming its own send.
pub fn reports_name_their_sends_among_plain_ones<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let reported = |key| TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key,
            software: Report::Requested,
            ..no_reports()
        },
        ..plain()
    };
    let keys = [KEY, TransmitReportKey(KEY.0 + 1)];
    for key in keys {
        send_all(&sockets, &pair.client, reported(key), MESSAGE);
        send_all(&sockets, &pair.client, plain(), MESSAGE);
    }
    for key in keys {
        let TcpErrorQueueEntry::TransmitReport { report, .. } = take_entry(&sockets, &pair.client)
        else {
            panic!("not a transmit report");
        };
        assert_eq!(report.key, key);
    }
    close(&sockets, pair);
}

pub fn acknowledgement_report_follows_the_peers_acknowledgement<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let key = KEY;
    let reported = TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key,
            acknowledgement: Report::Requested,
            ..no_reports()
        },
        ..plain()
    };
    send_all(&sockets, &pair.client, reported, MESSAGE);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    let TcpErrorQueueEntry::TransmitReport { report, .. } = take_entry(&sockets, &pair.client)
    else {
        panic!("not a transmit report");
    };
    assert_eq!(report.key, key);
    assert!(matches!(
        report.event,
        TransmitEvent::Timestamp {
            stage: TransmitStage::Acknowledged,
            ..
        }
    ));
    close(&sockets, pair);
}

pub fn discarding_empties_the_error_queue<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let reported = TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key: KEY,
            software: Report::Requested,
            ..no_reports()
        },
        ..plain()
    };
    send_all(&sockets, &pair.client, reported, MESSAGE);
    send_all(&sockets, &pair.client, reported, MESSAGE);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, 2 * MESSAGE.len()),
        [MESSAGE, MESSAGE].concat()
    );
    sockets
        .discard_error_queue_entries(&pair.client)
        .expect("discard");
    let mut buffer = [u8::MIN; ENTRY_ROOM];
    assert_eq!(
        sockets.take_error_queue_entry(&pair.client, &mut buffer),
        Err(HostError::WouldBlock)
    );
    sockets
        .discard_error_queue_entries(&pair.client)
        .expect("discard an empty queue");
    close(&sockets, pair);
}

pub fn empty_error_queue_would_block<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let mut buffer = [u8::MIN; ENTRY_ROOM];
    assert_eq!(
        sockets.take_error_queue_entry(&pair.client, &mut buffer),
        Err(HostError::WouldBlock)
    );
    close(&sockets, pair);
}

fn take_entry_into<S: TcpSockets>(
    sockets: &S,
    connection: &S::Connection,
    buffer: &mut [u8],
) -> TcpErrorQueueEntry {
    retry(|| sockets.take_error_queue_entry(connection, buffer)).expect("an error queue entry")
}

/// Gives a raw socket of the host, which the ICMP checks use as the network between the connection and its peer.
fn raw_socket(version: IpVersion, protocol: rustix::net::Protocol) -> OwnedFd {
    let family = match version {
        IpVersion::V4 => AddressFamily::INET,
        IpVersion::V6 => AddressFamily::INET6,
    };
    let socket = rustix::net::socket(family, SocketType::RAW, Some(protocol))
        .expect("a raw socket (the ICMP checks need root)");
    sockopt::set_socket_timeout(&socket, sockopt::Timeout::Recv, Some(SETTLE))
        .expect("a receive timeout");
    socket
}

/// Waits on a raw TCP socket for the SYN from `local` to `peer`, giving what Linux delivered of it.
fn capture_synchronize(
    capture: &OwnedFd,
    version: IpVersion,
    local: SocketAddr,
    peer: SocketAddr,
) -> Vec<u8> {
    let start = Instant::now();
    let mut buffer = [u8::MIN; PACKET_ROOM];
    while start.elapsed() < DEADLINE {
        let Ok((length, _)) = rustix::net::recv(capture, &mut buffer, RecvFlags::empty()) else {
            continue;
        };
        let packet = &buffer[..length];
        let segment = match version {
            IpVersion::V4 => Ipv4HeaderSlice::from_slice(packet)
                .ok()
                .map(|header| &packet[header.slice().len()..]),
            IpVersion::V6 => Some(packet),
        };
        if let Some(tcp) = segment.and_then(|segment| TcpHeaderSlice::from_slice(segment).ok())
            && tcp.syn()
            && !tcp.ack()
            && tcp.source_port() == local.port()
            && tcp.destination_port() == peer.port()
        {
            return packet.to_vec();
        }
    }
    panic!("no SYN from {local} to {peer} arrived");
}

/// Connects to a full listener and answers the SYN with a destination unreachable error, checking the queued entry.
///
/// The full accept queue keeps the connection in SYN-SENT; the error queue entry must hold the error written whole.
fn icmp_error_reaches_a_synchronizing_connection<S: TcpSockets>(sockets: S, version: IpVersion) {
    let (address, listener_address, loopback) = match version {
        IpVersion::V4 => (
            ipv4_loopback(),
            BindAddress::V4(OTHER_LOOPBACK),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
        ),
        IpVersion::V6 => {
            let address = BindAddress::V6 {
                address: Ipv6Addr::LOCALHOST,
                families: Ipv6Families::V6Only,
            };
            (address, address, IpAddr::V6(Ipv6Addr::LOCALHOST))
        }
    };
    let listening = sockets
        .listen(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(listener_address),
            SMALLEST_BACKLOG,
            &[],
        )
        .expect("listen on loopback");
    let peer = listening.local;
    let connect = |settings: &[TcpSetting<'_>]| {
        sockets
            .connect(
                TcpProtocol::Tcp,
                OWNER,
                bind_any_port(address),
                peer,
                settings,
                TcpFastOpen::Off,
            )
            .expect("connect to the listener")
    };
    let queued = connect(&[]).connection;
    retry(|| sockets.send(&queued, plain(), URGENT)).expect("the first connection is established");
    std::thread::sleep(SETTLE);
    let capture = raw_socket(version, ipproto::TCP);
    let waiting = connect(&[TcpSetting::IcmpErrorReports {
        version,
        reports: Report::Requested,
    }]);
    let local = waiting.local;
    assert!(matches!(
        waiting.start,
        TcpConnectStart::SynchronizeSent { .. }
    ));
    let synchronize = capture_synchronize(&capture, version, local, peer);
    let (quoted, icmp) = match version {
        IpVersion::V4 => {
            let header = Ipv4HeaderSlice::from_slice(&synchronize).expect("an IPv4 header");
            let tcp = TcpHeaderSlice::from_slice(&synchronize[header.slice().len()..])
                .expect("a TCP header");
            let quoted = [header.slice(), tcp.slice()].concat();
            let icmp = Icmpv4Header::with_checksum(
                Icmpv4Type::DestinationUnreachable(icmpv4::DestUnreachableHeader::Host),
                &quoted,
            );
            (quoted, icmp.to_bytes().to_vec())
        }
        IpVersion::V6 => {
            let tcp = TcpHeaderSlice::from_slice(&synchronize).expect("a TCP header");
            let header = Ipv6Header {
                traffic_class: u8::MIN,
                flow_label: Ipv6FlowLabel::ZERO,
                payload_length: u16::try_from(synchronize.len()).expect("a short SYN"),
                next_header: IpNumber::TCP,
                hop_limit: QUOTED_HOP_LIMIT,
                source: Ipv6Addr::LOCALHOST.octets(),
                destination: Ipv6Addr::LOCALHOST.octets(),
            };
            let quoted = [&header.to_bytes()[..], tcp.slice()].concat();
            let icmp = Icmpv6Header::with_checksum(
                Icmpv6Type::DestinationUnreachable(icmpv6::DestUnreachableCode::Address),
                Ipv6Addr::LOCALHOST.octets(),
                Ipv6Addr::LOCALHOST.octets(),
                &quoted,
            )
            .expect("a short ICMPv6 error");
            (quoted, icmp.to_bytes().to_vec())
        }
    };
    let sender = raw_socket(
        version,
        match version {
            IpVersion::V4 => ipproto::ICMP,
            IpVersion::V6 => ipproto::ICMPV6,
        },
    );
    rustix::net::sendto(
        &sender,
        &[icmp, quoted].concat(),
        SendFlags::empty(),
        &SocketAddr::new(local.ip(), ANY_PORT.0),
    )
    .expect("send the ICMP error");
    let mut buffer = [u8::MIN; PACKET_ROOM];
    let TcpErrorQueueEntry::IcmpError(fields) =
        take_entry_into(&sockets, &waiting.connection, &mut buffer)
    else {
        panic!("not an ICMP error");
    };
    let packet = &buffer[..fields.length];
    let (icmp, inner) = match version {
        IpVersion::V4 => {
            let outer = Ipv4HeaderSlice::from_slice(packet).expect("an IPv4 header");
            assert_eq!(usize::from(outer.total_len()), fields.length);
            assert_eq!(outer.protocol(), IpNumber::ICMP);
            assert_eq!(IpAddr::V4(outer.source_addr()), loopback);
            assert_eq!(IpAddr::V4(outer.destination_addr()), local.ip());
            let icmp = Icmpv4Slice::from_slice(&packet[outer.slice().len()..])
                .expect("the ICMP error's header");
            assert_eq!(
                (icmp.type_u8(), icmp.code_u8()),
                (icmpv4::TYPE_DEST_UNREACH, icmpv4::CODE_DST_UNREACH_HOST)
            );
            assert_eq!(
                icmp.icmp_type().calc_checksum(icmp.payload()),
                icmp.checksum()
            );
            let inner = Ipv4HeaderSlice::from_slice(icmp.payload()).expect("the quoted header");
            assert_eq!(inner.protocol(), IpNumber::TCP);
            assert_eq!(IpAddr::V4(inner.source_addr()), local.ip());
            assert_eq!(IpAddr::V4(inner.destination_addr()), peer.ip());
            (icmp.payload(), inner.slice().len())
        }
        IpVersion::V6 => {
            let outer = Ipv6HeaderSlice::from_slice(packet).expect("an IPv6 header");
            assert_eq!(
                usize::from(outer.payload_length()) + Ipv6Header::LEN,
                fields.length
            );
            assert_eq!(outer.next_header(), IpNumber::IPV6_ICMP);
            assert_eq!(IpAddr::V6(outer.source_addr()), loopback);
            assert_eq!(IpAddr::V6(outer.destination_addr()), local.ip());
            let icmp = Icmpv6Slice::from_slice(&packet[Ipv6Header::LEN..])
                .expect("the ICMPv6 error's header");
            assert_eq!(
                (icmp.type_u8(), icmp.code_u8()),
                (icmpv6::TYPE_DST_UNREACH, icmpv6::CODE_DST_UNREACH_ADDR)
            );
            assert_eq!(
                icmp.icmp_type()
                    .calc_checksum(
                        outer.source_addr().octets(),
                        outer.destination_addr().octets(),
                        icmp.payload()
                    )
                    .expect("a checksum"),
                icmp.checksum()
            );
            let inner = Ipv6HeaderSlice::from_slice(icmp.payload()).expect("the quoted header");
            assert_eq!(inner.next_header(), IpNumber::TCP);
            assert_eq!(IpAddr::V6(inner.source_addr()), local.ip());
            assert_eq!(IpAddr::V6(inner.destination_addr()), peer.ip());
            (icmp.payload(), Ipv6Header::LEN)
        }
    };
    let tcp = TcpHeaderSlice::from_slice(&icmp[inner..]).expect("the quoted TCP header");
    let sent = TcpHeaderSlice::from_slice(match version {
        IpVersion::V4 => {
            &synchronize[Ipv4HeaderSlice::from_slice(&synchronize)
                .expect("an IPv4 header")
                .slice()
                .len()..]
        }
        IpVersion::V6 => &synchronize,
    })
    .expect("a TCP header");
    assert_eq!(
        (
            tcp.source_port(),
            tcp.destination_port(),
            tcp.sequence_number()
        ),
        (local.port(), peer.port(), sent.sequence_number())
    );
    sockets.close(waiting.connection, TcpClose::Abort);
    sockets.close(queued, TcpClose::Abort);
    sockets.stop_listening(listening.listener);
}

pub fn icmp_error_reaches_a_synchronizing_connection_over_ipv4<S: TcpSockets>(sockets: S) {
    icmp_error_reaches_a_synchronizing_connection(sockets, IpVersion::V4);
}

pub fn icmp_error_reaches_a_synchronizing_connection_over_ipv6<S: TcpSockets>(sockets: S) {
    icmp_error_reaches_a_synchronizing_connection(sockets, IpVersion::V6);
}

pub fn transmit_report_carries_the_packet_when_asked<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let reported = TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key: KEY,
            software: Report::Requested,
            packet: Report::Requested,
            ..no_reports()
        },
        ..plain()
    };
    send_all(&sockets, &pair.client, reported, MESSAGE);
    let mut buffer = [u8::MIN; PACKET_ROOM];
    let TcpErrorQueueEntry::TransmitReport { report, statistics } =
        take_entry_into(&sockets, &pair.client, &mut buffer)
    else {
        panic!("not a transmit report");
    };
    assert_eq!(report.key, KEY);
    assert_eq!(statistics, None);
    let length = report.packet_length.expect("the packet").0;
    assert!(length >= MESSAGE.len(), "a packet of {length} bytes");
    assert!(buffer[..length].ends_with(MESSAGE));
    close(&sockets, pair);
}

pub fn transmit_statistics_come_with_the_report_when_asked<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let reported = |statistics| TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key: KEY,
            software: Report::Requested,
            ..no_reports()
        },
        transmit_statistics: statistics,
        ..plain()
    };
    send_all(
        &sockets,
        &pair.client,
        reported(Report::NotRequested),
        MESSAGE,
    );
    let TcpErrorQueueEntry::TransmitReport { statistics, .. } = take_entry(&sockets, &pair.client)
    else {
        panic!("not a transmit report");
    };
    assert_eq!(statistics, None);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    send_all(&sockets, &pair.client, reported(Report::Requested), MESSAGE);
    let TcpErrorQueueEntry::TransmitReport { report, statistics } =
        take_entry(&sockets, &pair.client)
    else {
        panic!("not a transmit report");
    };
    assert_eq!(report.key, KEY);
    let statistics = statistics.expect("the statistics asked for");
    assert!(statistics.bytes_sent.0 >= MESSAGE.len() as u64);
    assert!(statistics.data_segments_out.0 >= 1);
    assert!(statistics.send_congestion_window.0 >= 1);
    close(&sockets, pair);
}

pub fn wireless_status_request_still_sends<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let reported = TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key: KEY,
            wireless_status: Report::Requested,
            ..no_reports()
        },
        ..plain()
    };
    send_all(&sockets, &pair.client, reported, MESSAGE);
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    // Loopback is no wireless link, so no status comes.
    let mut buffer = [u8::MIN; ENTRY_ROOM];
    assert_eq!(
        sockets.take_error_queue_entry(&pair.client, &mut buffer),
        Err(HostError::WouldBlock)
    );
    close(&sockets, pair);
}

/// What one connect with fast open showed on the wire.
struct FastOpenRound {
    start: TcpConnectStart,
    synchronize_data: usize,
    data_acknowledged: bool,
}

/// Connects with fast open, sends the message, and checks that the listener's connection receives it whole.
fn fast_open_round<S: TcpSockets>(
    sockets: &S,
    listener: &S::Listener,
    local: SocketAddr,
    capture: &Capture,
    settings: &[TcpSetting<'_>],
    fast_open: TcpFastOpen<'_>,
) -> FastOpenRound {
    let start = Instant::now();
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            settings,
            fast_open,
        )
        .expect("connect with fast open");
    let taken = match connected.start {
        TcpConnectStart::SynchronizeSent {
            fast_open_data_taken,
        } => fast_open_data_taken.0,
        _ => usize::MIN,
    };
    if connected.start == TcpConnectStart::DeferredUntilFirstSend {
        let early = capture.wait_for(start, SETTLE, |segment| {
            segment.to_listener && segment.synchronize
        });
        assert!(early.is_none(), "a deferred connect sent its SYN early");
    }
    send_all(sockets, &connected.connection, plain(), &MESSAGE[taken..]);
    let server = accept(sockets, listener);
    assert_eq!(receive_up_to(sockets, &server, MESSAGE.len()), MESSAGE);
    let synchronize = capture
        .wait_for(start, DEADLINE, |segment| {
            segment.to_listener && segment.synchronize
        })
        .expect("the SYN");
    let answer = capture
        .wait_for(start, DEADLINE, |segment| {
            !segment.to_listener && segment.synchronize
        })
        .expect("the SYN-ACK");
    sockets.close(server, TcpClose::Orderly);
    sockets.close(connected.connection, TcpClose::Orderly);
    FastOpenRound {
        start: connected.start,
        synchronize_data: synchronize.payload,
        data_acknowledged: synchronize.payload > 0 && answer.acknowledgement == synchronize.end(),
    }
}

/// Gives a fast open listener on its own loopback address, since a client keeps the cookies it is given per address.
fn fast_open_listener<S: TcpSockets>(
    sockets: &S,
    address: Ipv4Addr,
    settings: &[TcpListenerSetting<'_>],
) -> (S::Listener, SocketAddr, Capture) {
    let mut all = vec![TcpListenerSetting::FastOpenQueue(FAST_OPEN_QUEUE)];
    all.extend_from_slice(settings);
    let (listener, local) = listen(sockets, BindAddress::V4(address), &all);
    let capture = Capture::start(local);
    (listener, local, capture)
}

/// Checks that the first connect fetches a cookie; the second puts all its data in the SYN, which the listener takes.
pub fn fast_open_with_data_puts_it_in_the_synchronize<S: TcpSockets>(sockets: S) {
    let (listener, local, capture) = fast_open_listener(&sockets, FAST_OPEN_WITH_DATA, &[]);
    let fast_open = TcpFastOpen::WithData(MESSAGE);
    fast_open_round(&sockets, &listener, local, &capture, &[], fast_open);
    let round = fast_open_round(&sockets, &listener, local, &capture, &[], fast_open);
    assert_eq!(
        round.start,
        TcpConnectStart::SynchronizeSent {
            fast_open_data_taken: ByteCount(MESSAGE.len())
        }
    );
    assert_eq!(round.synchronize_data, MESSAGE.len());
    assert!(round.data_acknowledged);
    sockets.stop_listening(listener);
}

/// Checks that once a cookie is held, a connect until the first send sends nothing until that send, whose data goes in the SYN.
pub fn fast_open_until_first_send_defers_the_synchronize<S: TcpSockets>(sockets: S) {
    let (listener, local, capture) = fast_open_listener(&sockets, FAST_OPEN_UNTIL_FIRST_SEND, &[]);
    let fast_open = TcpFastOpen::UntilFirstSend;
    fast_open_round(&sockets, &listener, local, &capture, &[], fast_open);
    let round = fast_open_round(&sockets, &listener, local, &capture, &[], fast_open);
    assert_eq!(round.start, TcpConnectStart::DeferredUntilFirstSend);
    assert_eq!(round.synchronize_data, MESSAGE.len());
    assert!(round.data_acknowledged);
    sockets.stop_listening(listener);
}

/// Checks that both ends doing without cookies, the very first connect's data goes in the SYN.
pub fn fast_open_without_cookie_needs_no_first_connect<S: TcpSockets>(sockets: S) {
    let without = TcpSetting::FastOpenWithoutCookie(true);
    let (listener, local, capture) = fast_open_listener(
        &sockets,
        FAST_OPEN_WITHOUT_COOKIE,
        &[TcpListenerSetting::Socket(without.clone())],
    );
    let round = fast_open_round(
        &sockets,
        &listener,
        local,
        &capture,
        &[without],
        TcpFastOpen::WithData(MESSAGE),
    );
    assert_eq!(round.synchronize_data, MESSAGE.len());
    assert!(round.data_acknowledged);
    sockets.stop_listening(listener);
}

/// Checks that a listener takes SYN data only with a cookie of its primary or backup key, and only while its queue is on.
pub fn fast_open_keys_and_queue_decide_what_the_listener_takes<S: TcpSockets>(sockets: S) {
    let keys = |primary: u8, backup: Option<u8>| {
        TcpListenerSetting::FastOpenKeys(FastOpenKeys {
            primary: FastOpenKey([primary; FAST_OPEN_KEY_LENGTH]),
            backup: backup.map(|key| FastOpenKey([key; FAST_OPEN_KEY_LENGTH])),
        })
    };
    let (listener, local, capture) =
        fast_open_listener(&sockets, FAST_OPEN_KEYS, &[keys(FIRST_FAST_OPEN_KEY, None)]);
    let fast_open = TcpFastOpen::WithData(MESSAGE);
    let round = || fast_open_round(&sockets, &listener, local, &capture, &[], fast_open);
    round();
    assert!(round().data_acknowledged, "a cookie of the key");
    sockets
        .configure(&listener, keys(SECOND_FAST_OPEN_KEY, None))
        .expect("a new key");
    let stale = round();
    assert_eq!(stale.synchronize_data, MESSAGE.len());
    assert!(!stale.data_acknowledged, "a cookie of a key no longer held");
    sockets
        .configure(
            &listener,
            keys(THIRD_FAST_OPEN_KEY, Some(SECOND_FAST_OPEN_KEY)),
        )
        .expect("a new key, the last one kept as backup");
    assert!(round().data_acknowledged, "a cookie of the backup key");
    sockets
        .configure(&listener, TcpListenerSetting::FastOpenQueue(None))
        .expect("fast open off");
    let off = round();
    assert_eq!(off.synchronize_data, MESSAGE.len());
    assert!(!off.data_acknowledged, "fast open off");
    sockets.stop_listening(listener);
}

/// Gives a keep-alive probe: no data, one sequence number below what the connection sends next.
fn is_probe(segment: &Segment, next: u32) -> bool {
    segment.to_listener
        && segment.payload == 0
        && !segment.synchronize
        && !segment.reset
        && !segment.finish
        && segment.sequence == next.wrapping_sub(1)
}

fn assert_near(what: &str, measured: Duration, expected: Duration) {
    assert!(
        measured + TIMER_SLACK > expected && measured < expected + TIMER_SLACK,
        "{what}: {measured:?}, not {expected:?}"
    );
}

/// Checks that keep-alive probes come after the idle time, an interval apart, and end an unanswered connection.
///
/// The peer answers nothing; the connection probes as many times as asked and resets an interval after its last probe.
pub fn keep_alive_probes_end_an_unanswered_connection<S: TcpSockets>(sockets: S) {
    for &count in KEEP_ALIVE_PROBE_COUNTS {
        let (pair, capture) = watched_pair(
            &sockets,
            ipv4_loopback(),
            &[
                TcpSetting::KeepAlive(true),
                TcpSetting::KeepAliveIdle(KEEP_ALIVE_IDLE),
                TcpSetting::KeepAliveInterval(KEEP_ALIVE_INTERVAL),
                TcpSetting::KeepAliveProbes(ProbeCount(count)),
            ],
            &[],
        );
        let start = Instant::now();
        drop_arrivals(&sockets, &pair.server, IpVersion::V4);
        let next = capture.next_to_listener();
        let ending = KEEP_ALIVE_IDLE + KEEP_ALIVE_INTERVAL * u32::from(count) + TIMER_SLACK;
        let probe = capture
            .wait_for(start, ending, |segment| is_probe(segment, next))
            .expect("a keep-alive probe");
        assert_near(PROBE, probe.at - start, KEEP_ALIVE_IDLE);
        let reset = capture
            .wait_for(start, ending, |segment| {
                segment.to_listener && segment.reset
            })
            .expect("a reset after the last probe");
        let probes: Vec<_> = capture
            .segments()
            .into_iter()
            .filter(|segment| is_probe(segment, next))
            .collect();
        assert_eq!(probes.len(), usize::from(count), "{count} probes");
        for (earlier, later) in probes.iter().zip(&probes[1..]) {
            assert_near(PROBE, later.at - earlier.at, KEEP_ALIVE_INTERVAL);
        }
        let last = probes.last().expect("a probe");
        assert_near(RESET, reset.at - last.at, KEEP_ALIVE_INTERVAL);
        assert_eq!(
            receive_outcome(&sockets, &pair.client, DEADLINE).map(|got| got.length),
            Err(errno(Errno::ETIMEDOUT))
        );
        close(&sockets, pair);
    }
}

/// Checks that a probe answered by nothing is followed by the next only an interval later, unless the timer is rearmed.
///
/// Then the next is due at once, the idle time since the last arrival having passed.
pub fn rearming_the_keep_alive_timer_sends_a_due_probe<S: TcpSockets>(sockets: S) {
    let (pair, capture) = watched_pair(
        &sockets,
        ipv4_loopback(),
        &[
            TcpSetting::KeepAlive(true),
            TcpSetting::KeepAliveIdle(KEEP_ALIVE_INTERVAL),
            TcpSetting::KeepAliveInterval(SETTING_TIME),
        ],
        &[],
    );
    let start = Instant::now();
    drop_arrivals(&sockets, &pair.server, IpVersion::V4);
    let next = capture.next_to_listener();
    let first = capture
        .wait_for(start, KEEP_ALIVE_INTERVAL + TIMER_SLACK, |segment| {
            is_probe(segment, next)
        })
        .expect("a keep-alive probe");
    std::thread::sleep(TIMER_SLACK);
    let rearmed = Instant::now();
    sockets.rearm_keep_alive_timer(&pair.client).expect("rearm");
    let second = capture.wait_for(rearmed, TIMER_SLACK, |segment| {
        segment.at > first.at && is_probe(segment, next)
    });
    assert!(second.is_some(), "no probe after the rearm");
    close(&sockets, pair);
}

/// Checks that the peer answering nothing, a second small send waits for the first to be acknowledged, unless without delay.
pub fn no_delay_sends_small_segments_while_others_are_unacknowledged<S: TcpSockets>(sockets: S) {
    for (settings, pushed_first, all_sent) in [
        (vec![], false, false),
        (vec![TcpSetting::NoDelay(true)], false, true),
        (
            vec![TcpSetting::NoDelay(true), TcpSetting::NoDelay(false)],
            false,
            false,
        ),
        // A push past cork and Nagle's algorithm leaves the algorithm on for what follows.
        (vec![], true, false),
    ] {
        let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), &settings, &[]);
        drop_arrivals(&sockets, &pair.server, IpVersion::V4);
        if pushed_first {
            sockets
                .push_queued_data(&pair.client, TcpPushOverride::CorkAndNagle)
                .expect("push");
        }
        let start = Instant::now();
        let sends = [MESSAGE, REPLY];
        for bytes in sends {
            send_all(&sockets, &pair.client, plain(), bytes);
        }
        std::thread::sleep(HELD_FOR);
        let mut sequences: Vec<u32> = capture
            .since(start)
            .iter()
            .filter(|segment| segment.is_data_to_listener())
            .map(|segment| segment.sequence)
            .collect();
        sequences.dedup();
        // Held back, the second send waits for the first to be acknowledged, which never comes.
        let segments = if all_sent {
            sends.len()
        } else {
            sends.len() - 1
        };
        assert_eq!(
            sequences.len(),
            segments,
            "{settings:?}, pushed first: {pushed_first}"
        );
        close(&sockets, pair);
    }
}

/// Checks that delaying acknowledgements, the listener's connection acknowledges data late; quick again, at once; and with a maximum delay, within it.
pub fn quick_acknowledgement_settings_time_the_acknowledgement<S: TcpSockets>(sockets: S) {
    let delaying = TcpSetting::QuickAcknowledgement(TcpQuickAcknowledgement::Off);
    for (settings, delayed) in [
        (vec![delaying.clone()], true),
        (
            vec![
                delaying.clone(),
                TcpSetting::QuickAcknowledgement(TcpQuickAcknowledgement::On),
            ],
            false,
        ),
        (
            vec![
                delaying.clone(),
                TcpSetting::DelayedAcknowledgementMaximum(DELAYED_ACKNOWLEDGEMENT_MAXIMUM),
            ],
            false,
        ),
    ] {
        let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), &[], &[]);
        for setting in &settings {
            sockets
                .configure(&pair.server, setting.clone())
                .expect("an acknowledgement setting");
        }
        let delay = acknowledgement_delay(&sockets, &pair, &capture);
        assert_eq!(delay > QUICK_WITHIN, delayed, "{settings:?}: {delay:?}");
        close(&sockets, pair);
    }
}

/// Checks that a delayed acknowledgement of data read is sent on demand, and the mode asked for holds for the next data.
pub fn delayed_acknowledgement_is_sent_on_demand<S: TcpSockets>(sockets: S) {
    for afterwards in [TcpQuickAcknowledgement::On, TcpQuickAcknowledgement::Off] {
        let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), &[], &[]);
        sockets
            .configure(
                &pair.server,
                TcpSetting::QuickAcknowledgement(TcpQuickAcknowledgement::Off),
            )
            .expect("delay acknowledgements");
        let start = Instant::now();
        send_all(&sockets, &pair.client, plain(), MESSAGE);
        assert_eq!(
            receive_up_to(&sockets, &pair.server, MESSAGE.len()),
            MESSAGE
        );
        sockets
            .send_delayed_acknowledgement(&pair.server, afterwards)
            .expect("send it");
        let data = capture
            .wait_for(start, DEADLINE, Segment::is_data_to_listener)
            .expect("the data");
        let acknowledgement = capture
            .wait_for(start, DEADLINE, |segment| {
                !segment.to_listener && segment.acknowledgement == data.end()
            })
            .expect("its acknowledgement");
        let delay = acknowledgement.at - data.at;
        assert!(delay < QUICK_WITHIN, "{afterwards:?}: {delay:?}");
        let next = acknowledgement_delay(&sockets, &pair, &capture);
        assert_eq!(
            next > QUICK_WITHIN,
            afterwards == TcpQuickAcknowledgement::Off,
            "{afterwards:?}: {next:?}"
        );
        close(&sockets, pair);
    }
}

/// Checks that each setting bounds what a connection takes while its peer reads nothing, far below what it takes without.
pub fn send_settings_bound_what_a_connection_takes<S: TcpSockets>(sockets: S) {
    let unbounded = {
        let pair = pair(&sockets);
        let taken = fill(&sockets, &pair.client);
        close(&sockets, pair);
        taken
    };
    let low_water = TcpSetting::NotSentLowWater(Some(ByteCount(SETTING_BYTES / BOUND_RATIO)));
    for setting in [
        TcpSetting::Ip(IpSocketSetting::SendBuffer(ByteCount(SETTING_BYTES))),
        TcpSetting::BufferLocks(BufferLocks {
            send: BufferTuning::Locked,
            receive: BufferTuning::Automatic,
        }),
        low_water.clone(),
    ] {
        let pair = pair_with(&sockets, std::slice::from_ref(&setting), &[]);
        let taken = fill(&sockets, &pair.client);
        assert!(
            taken * BOUND_RATIO < unbounded,
            "{setting:?}: {taken} of {unbounded}"
        );
        close(&sockets, pair);
    }
    let pair = pair_with(&sockets, &[low_water], &[]);
    let bounded = fill(&sockets, &pair.client);
    sockets
        .configure(&pair.client, TcpSetting::NotSentLowWater(None))
        .expect("no low water");
    let more = fill(&sockets, &pair.client);
    assert!(
        (bounded + more) * BOUND_RATIO > unbounded,
        "{bounded} and {more} of {unbounded}"
    );
    close(&sockets, pair);
}

/// Checks that each setting of the receiving connection bounds what its peer sends while it reads nothing, or widens it.
///
/// A locked receive buffer keeps a low water mark from widening it.
pub fn receive_settings_bound_what_the_peer_sends<S: TcpSockets>(sockets: S) {
    let unread_with = |settings: &[TcpSetting<'_>]| {
        let pair = pair_with(&sockets, settings, &[]);
        fill(&sockets, &pair.server);
        let waiting = unread(&sockets, &pair.client);
        close(&sockets, pair);
        waiting
    };
    let unbounded = unread_with(&[]);
    let wide = TcpSetting::ReceiveLowWater(ByteCount(WIDE_LOW_WATER));
    let locked = TcpSetting::BufferLocks(BufferLocks {
        send: BufferTuning::Automatic,
        receive: BufferTuning::Locked,
    });
    let narrower = |waiting: usize| waiting * 2 < unbounded;
    let wider = |waiting: usize| waiting > unbounded * 2;
    let small = ByteCount(SETTING_BYTES / BOUND_RATIO);
    let narrowed = unread_with(&[TcpSetting::Ip(IpSocketSetting::ReceiveBuffer(small))]);
    assert!(narrower(narrowed), "{narrowed} of {unbounded}");
    let widened = unread_with(std::slice::from_ref(&wide));
    assert!(wider(widened), "{widened} of {unbounded}");
    let kept = unread_with(&[locked, wide]);
    assert!(!wider(kept), "{kept} of {unbounded}");
}

/// Checks that a window clamp given to connect bounds the window its SYN announces.
pub fn window_clamp_bounds_the_synchronize_window<S: TcpSockets>(sockets: S) {
    let clamp = WindowSize((SETTING_BYTES / BOUND_RATIO) as u32);
    let announced = |settings: &[TcpSetting<'_>]| {
        let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), settings, &[]);
        let window = capture
            .first(|segment| segment.to_listener && segment.synchronize)
            .window;
        close(&sockets, pair);
        u32::from(window)
    };
    let (unclamped, clamped) = (
        announced(&[]),
        announced(&[TcpSetting::WindowClamp(Some(clamp))]),
    );
    assert!(
        clamped <= clamp.0 && clamped < unclamped,
        "{clamped} of {unclamped}"
    );
}

/// Gives when a segment the peer never acknowledges left again.
fn transmissions(capture: &Capture, start: Instant) -> Vec<Duration> {
    capture
        .since(start)
        .iter()
        .filter(|segment| segment.is_data_to_listener())
        .map(|segment| segment.at - start)
        .collect()
}

/// Checks that the retransmission minimum, maximum and thin linear timeouts each time the retransmissions as asked.
///
/// The peer answering nothing, a short minimum makes the first retransmission early, a maximum caps how far the
/// timeout grows, and thin linear timeouts keep it from growing.
pub fn retransmission_settings_time_the_retransmissions<S: TcpSockets>(sockets: S) {
    let watched: Vec<_> = [
        TcpSetting::RetransmissionTimeoutMinimum(SHORT_SETTING_TIME),
        TcpSetting::RetransmissionTimeoutMaximum(RETRANSMISSION_CEILING),
        TcpSetting::ThinLinearTimeouts(true),
    ]
    .into_iter()
    .map(|setting| {
        let (pair, capture) = watched_pair(
            &sockets,
            ipv4_loopback(),
            std::slice::from_ref(&setting),
            &[],
        );
        drop_arrivals(&sockets, &pair.server, IpVersion::V4);
        (setting, pair, capture, Instant::now())
    })
    .collect();
    for (_, pair, _, _) in &watched {
        send_all(&sockets, &pair.client, plain(), MESSAGE);
    }
    std::thread::sleep(RETRANSMISSIONS_WATCHED);
    for (setting, pair, capture, start) in watched {
        let times = transmissions(&capture, start);
        let longest = times
            .iter()
            .zip(times.iter().skip(1))
            .map(|(earlier, later)| *later - *earlier)
            .max()
            .unwrap_or_default();
        match setting {
            TcpSetting::RetransmissionTimeoutMinimum(_) => assert!(
                times.len() > 1 && times[1] - times[0] < SHORT_SETTING_TIME + QUICK_WITHIN * 2,
                "{times:?}"
            ),
            TcpSetting::RetransmissionTimeoutMaximum(_) => assert!(
                times.last() > Some(&(RETRANSMISSION_CEILING * 2))
                    && longest < RETRANSMISSION_CEILING + TIMER_SLACK,
                "{times:?}"
            ),
            _ => assert!(
                times
                    .iter()
                    .filter(|time| **time < LINEAR_TRANSMISSIONS_WITHIN)
                    .count()
                    >= LINEAR_TRANSMISSIONS,
                "{times:?}"
            ),
        }
        close(&sockets, pair);
    }
}

/// Checks that the peer answering nothing, a user timeout ends the connection; lifted, the connection waits on.
pub fn user_timeout_ends_an_unacknowledged_connection<S: TcpSockets>(sockets: S) {
    let timeout = TcpSetting::UserTimeout(Some(USER_TIMEOUT));
    let timed = pair_with(&sockets, std::slice::from_ref(&timeout), &[]);
    let lifted = pair_with(&sockets, &[timeout, TcpSetting::UserTimeout(None)], &[]);
    let start = Instant::now();
    for pair in [&timed, &lifted] {
        drop_arrivals(&sockets, &pair.server, IpVersion::V4);
        send_all(&sockets, &pair.client, plain(), MESSAGE);
    }
    assert_eq!(
        receive_outcome(&sockets, &timed.client, DEADLINE).map(|got| got.length),
        Err(errno(Errno::ETIMEDOUT))
    );
    let elapsed = start.elapsed();
    assert!(
        elapsed >= USER_TIMEOUT && elapsed < USER_TIMEOUT * BOUND_RATIO as u32,
        "{elapsed:?}"
    );
    let mut buffer = [u8::MIN; 1];
    assert_eq!(
        sockets
            .receive(&lifted.client, TcpReceiveRequest::Read, &mut buffer)
            .map(|got| got.length),
        Err(HostError::WouldBlock)
    );
    close(&sockets, timed);
    close(&sockets, lifted);
}

/// Checks that a listener dropping every SYN, a connect retries once and then fails.
pub fn synchronize_retries_end_an_unanswered_connect<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(
        &sockets,
        ipv4_loopback(),
        &[TcpListenerSetting::Socket(TcpSetting::MinimumHopLimit {
            version: IpVersion::V4,
            minimum: Some(HopLimit(u8::MAX)),
        })],
    );
    let capture = Capture::start(local);
    let client = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[TcpSetting::SynchronizeRetries(ONE_SYNCHRONIZE_RETRY)],
            TcpFastOpen::Off,
        )
        .expect("connect")
        .connection;
    // With the default retries the connect would last minutes.
    assert_eq!(
        receive_outcome(&sockets, &client, DEADLINE).map(|got| got.length),
        Err(errno(Errno::ETIMEDOUT))
    );
    let synchronizes = capture
        .segments()
        .iter()
        .filter(|segment| segment.to_listener && segment.synchronize)
        .count();
    assert!(synchronizes > 1, "{synchronizes} SYNs");
    sockets.close(client, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

/// Checks that a listener deferring accepts gives a connection only once data has come on it; with no deferral, at once.
pub fn defer_accept_holds_a_connection_until_data<S: TcpSockets>(sockets: S) {
    let deferring = TcpListenerSetting::Socket(TcpSetting::DeferAccept(Some(SETTING_TIME)));
    for (settings, deferred) in [
        (vec![deferring.clone()], true),
        (
            vec![
                deferring,
                TcpListenerSetting::Socket(TcpSetting::DeferAccept(None)),
            ],
            false,
        ),
    ] {
        let (listener, local) = listen(&sockets, ipv4_loopback(), &settings);
        let client = sockets
            .connect(
                TcpProtocol::Tcp,
                OWNER,
                bind_any_port(ipv4_any()),
                local,
                &[],
                TcpFastOpen::Off,
            )
            .expect("connect")
            .connection;
        if deferred {
            std::thread::sleep(SETTLE);
            let mut header = [u8::MIN; 0];
            assert_eq!(
                sockets.accept(&listener, &mut header).map(|_| ()),
                Err(HostError::WouldBlock)
            );
            send_all(&sockets, &client, plain(), MESSAGE);
        }
        let server = accept(&sockets, &listener);
        if deferred {
            assert_eq!(receive_up_to(&sockets, &server, MESSAGE.len()), MESSAGE);
        }
        sockets.close(client, TcpClose::Orderly);
        sockets.close(server, TcpClose::Orderly);
        sockets.stop_listening(listener);
    }
}

/// Checks that closed in order, a connection whose FIN-WAIT-2 resets ends with a reset.
///
/// One whose FIN-WAIT-2 lasts a second is gone after it, so the peer's FIN is answered by a reset; one with the system's default still answers it in order.
pub fn fin_wait_2_settings_decide_how_a_closed_connection_ends<S: TcpSockets>(sockets: S) {
    let resetting = TcpSetting::FinWait2(TcpFinWait2::Reset);
    let ends = [
        vec![resetting.clone()],
        vec![TcpSetting::FinWait2(TcpFinWait2::Lasts(FIN_WAIT_2_LASTS))],
        vec![resetting, TcpSetting::FinWait2(TcpFinWait2::SystemDefault)],
    ]
    .map(|settings| {
        let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), &settings, &[]);
        let start = Instant::now();
        sockets.close(pair.client, TcpClose::Orderly);
        assert_eq!(
            receive_outcome(&sockets, &pair.server, DEADLINE).map(|got| got.length),
            Ok(NO_BYTES)
        );
        (pair.listener, pair.server, capture, start)
    });
    let reset_after = |capture: &Capture, start: Instant, within: Duration| {
        capture
            .wait_for(start, within, |segment| {
                segment.to_listener && segment.reset
            })
            .is_some()
    };
    let [reset, lasting, default] = ends;
    assert!(reset_after(&reset.2, reset.3, PUSHED_WITHIN), "no reset");
    std::thread::sleep(FIN_WAIT_2_LASTS + TIMER_SLACK);
    let finished = Instant::now();
    for (_, server, _, _) in [&lasting, &default] {
        sockets
            .shut_down(server, TcpShutdown::Write)
            .expect("shut down");
    }
    assert!(
        reset_after(&lasting.2, finished, PUSHED_WITHIN),
        "no reset after FIN-WAIT-2 ended"
    );
    std::thread::sleep(PUSHED_WITHIN);
    assert!(
        !reset_after(&default.2, finished, Duration::ZERO),
        "a reset in the default FIN-WAIT-2"
    );
    for (listener, server, _, _) in [reset, lasting, default] {
        sockets.close(server, TcpClose::Orderly);
        sockets.stop_listening(listener);
    }
}

/// Checks that each setting that drops arrivals holds the peer's data back until it is lifted.
pub fn arrival_settings_drop_what_they_refuse<S: TcpSockets>(sockets: S) {
    for (address, refusing, lifting) in [
        (
            ipv4_loopback(),
            TcpSetting::Filter(Some(SocketFilter(&DROP_ALL))),
            TcpSetting::Filter(None),
        ),
        (
            ipv4_loopback(),
            TcpSetting::MinimumHopLimit {
                version: IpVersion::V4,
                minimum: Some(HopLimit(u8::MAX)),
            },
            TcpSetting::MinimumHopLimit {
                version: IpVersion::V4,
                minimum: None,
            },
        ),
        (
            ipv6_loopback(),
            TcpSetting::MinimumHopLimit {
                version: IpVersion::V6,
                minimum: Some(HopLimit(u8::MAX)),
            },
            TcpSetting::MinimumHopLimit {
                version: IpVersion::V6,
                minimum: None,
            },
        ),
    ] {
        let (pair, _capture) = watched_pair(&sockets, address, &[], &[]);
        sockets
            .configure(&pair.server, refusing.clone())
            .expect("refuse arrivals");
        send_all(&sockets, &pair.client, plain(), MESSAGE);
        assert_silent(&sockets, &pair.server, HELD_FOR);
        sockets
            .configure(&pair.server, lifting)
            .expect("take arrivals");
        assert_eq!(
            receive_up_to(&sockets, &pair.server, MESSAGE.len()),
            MESSAGE,
            "{refusing:?}"
        );
        close(&sockets, pair);
    }
    let pair = pair_with(&sockets, &[], &[]);
    sockets
        .configure(
            &pair.server,
            TcpSetting::Filter(Some(SocketFilter(&ACCEPT_ALL))),
        )
        .expect("a filter that keeps everything");
    assert_carries_bytes(&sockets, &pair);
    close(&sockets, pair);
}

/// Checks that the hop limit, traffic class, fragmenting and flow label settings show in the headers of what the connection sends.
pub fn header_settings_mark_each_segment<S: TcpSockets>(sockets: S) {
    let data_after = |pair: &Pair<S>, capture: &Capture| {
        let start = Instant::now();
        send_all(&sockets, &pair.client, plain(), MESSAGE);
        capture
            .wait_for(start, DEADLINE, Segment::is_data_to_listener)
            .expect("a data segment")
    };
    let limit = HopLimit(SETTING_COUNT);
    let (pair, capture) = watched_pair(
        &sockets,
        ipv4_loopback(),
        &[
            TcpSetting::Packet(IpPacketSetting::HopLimit {
                version: IpVersion::V4,
                limit: Some(limit),
            }),
            TcpSetting::Packet(IpPacketSetting::TrafficClass {
                version: IpVersion::V4,
                dscp: IpDscp::AF41,
                ecn: IpEcn::ZERO,
            }),
            TcpSetting::Packet(IpPacketSetting::PathMtuDiscovery {
                version: IpVersion::V4,
                mode: PathMtuDiscovery::Dont,
            }),
        ],
        &[],
    );
    let marked = data_after(&pair, &capture);
    assert_eq!(
        (marked.hop_limit, marked.dscp, marked.dont_fragment),
        (limit.0, Some(IpDscp::AF41), false)
    );
    for setting in [
        IpPacketSetting::HopLimit {
            version: IpVersion::V4,
            limit: None,
        },
        IpPacketSetting::PathMtuDiscovery {
            version: IpVersion::V4,
            mode: PathMtuDiscovery::Do,
        },
    ] {
        sockets
            .configure(&pair.client, TcpSetting::Packet(setting))
            .expect("a header setting");
    }
    let plain_segment = data_after(&pair, &capture);
    let listener_side = capture.first(|segment| !segment.to_listener);
    assert_eq!(
        (plain_segment.hop_limit, plain_segment.dont_fragment),
        (listener_side.hop_limit, true)
    );
    close(&sockets, pair);
    let (pair, capture) = watched_pair(
        &sockets,
        ipv6_loopback(),
        &[
            TcpSetting::Packet(IpPacketSetting::HopLimit {
                version: IpVersion::V6,
                limit: Some(limit),
            }),
            TcpSetting::Packet(IpPacketSetting::FlowLabel(FlowLabelChoice::Unlabelled)),
        ],
        &[],
    );
    let marked = data_after(&pair, &capture);
    assert_eq!(
        (marked.hop_limit, marked.flow_label),
        (limit.0, Some(NO_FLOW_LABEL))
    );
    sockets
        .configure(
            &pair.client,
            TcpSetting::Packet(IpPacketSetting::FlowLabel(FlowLabelChoice::Automatic)),
        )
        .expect("automatic flow labels");
    assert_ne!(data_after(&pair, &capture).flow_label, Some(NO_FLOW_LABEL));
    close(&sockets, pair);
}

/// A flow label a connection may carry for good: below 0x80000, which Linux keeps for the labels it makes itself.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const FIXED_FLOW_LABEL: u32 = 0x2468A;
/// A second flow label a connection may carry for good, unlike the first.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const OTHER_FIXED_FLOW_LABEL: u32 = 0x13579;

fn fixed_flow_label(label: u32) -> TcpSetting<'static> {
    TcpSetting::Packet(IpPacketSetting::FlowLabel(FlowLabelChoice::Label(
        Ipv6FlowLabel::try_new(label).expect("a flow label"),
    )))
}

/// Checks that a flow label given at connect marks every segment the connection sends, its SYN first.
///
/// Given again it changes nothing; another label afterwards is either refused, the first kept, or carried from then on.
pub fn fixed_flow_label_marks_every_segment<S: TcpSockets>(sockets: S) {
    let automatic = TcpSetting::Packet(IpPacketSetting::FlowLabel(FlowLabelChoice::Automatic));
    for settings in [
        vec![fixed_flow_label(FIXED_FLOW_LABEL)],
        vec![
            fixed_flow_label(FIXED_FLOW_LABEL),
            fixed_flow_label(FIXED_FLOW_LABEL),
        ],
        vec![automatic, fixed_flow_label(FIXED_FLOW_LABEL)],
        vec![
            fixed_flow_label(OTHER_FIXED_FLOW_LABEL),
            fixed_flow_label(FIXED_FLOW_LABEL),
        ],
    ] {
        let (pair, capture) = watched_pair(&sockets, ipv6_loopback(), &settings, &[]);
        assert_eq!(
            data_flow_label(&sockets, &pair, &capture),
            FIXED_FLOW_LABEL,
            "{settings:?}"
        );
        let sent: Vec<Segment> = capture
            .segments()
            .into_iter()
            .filter(|segment| segment.to_listener)
            .collect();
        assert!(sent.iter().any(|segment| segment.synchronize), "the SYN");
        assert!(
            sent.iter()
                .all(|segment| segment.flow_label == Some(FIXED_FLOW_LABEL)),
            "{settings:?}: {:?}",
            sent.iter()
                .map(|segment| segment.flow_label)
                .collect::<Vec<_>>()
        );
        sockets
            .configure(&pair.client, fixed_flow_label(FIXED_FLOW_LABEL))
            .expect("the label the connection has");
        assert_eq!(data_flow_label(&sockets, &pair, &capture), FIXED_FLOW_LABEL);
        let expected =
            match sockets.configure(&pair.client, fixed_flow_label(OTHER_FIXED_FLOW_LABEL)) {
                Ok(()) => OTHER_FIXED_FLOW_LABEL,
                Err(error) => {
                    assert_eq!(error, HostError::NotSupported);
                    FIXED_FLOW_LABEL
                }
            };
        assert_eq!(data_flow_label(&sockets, &pair, &capture), expected);
        close(&sockets, pair);
    }
}

/// An IPv4 group no other check joins, so that the host's tables show this check's membership alone.
///
/// https://www.rfc-editor.org/rfc/rfc5771
const MEMBERSHIP_V4: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);
/// An IPv6 group no other check joins.
///
/// https://www.rfc-editor.org/rfc/rfc4291
const MEMBERSHIP_V6: Ipv6Addr = Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0x123);

/// The host's tables of the multicast groups and anycast addresses each interface has joined.
const IPV4_GROUPS: &str = "/proc/net/igmp";
const IPV6_GROUPS: &str = "/proc/net/igmp6";
const IPV6_ANYCAST: &str = "/proc/net/anycast6";

/// Gives how many sockets of the host have joined the group on the interface, by its table; none when it is not listed.
fn group_users(group: IpAddr, interface: InterfaceIndex) -> Option<usize> {
    match group {
        // Device lines start with the interface's index, its groups follow indented, each the address's bytes as a
        // number of this host's byte order.
        IpAddr::V4(group) => {
            let wanted = format!("{:08X}", u32::from_ne_bytes(group.octets()));
            let table = std::fs::read_to_string(IPV4_GROUPS).expect("the IPv4 group table");
            let mut device = None;
            for line in table.lines().skip(1) {
                let mut fields = line.split_whitespace();
                if !line.starts_with(char::is_whitespace) {
                    device = fields.next().and_then(|index| index.parse().ok());
                } else if device == Some(interface.0) && fields.next() == Some(wanted.as_str()) {
                    return fields.next().and_then(|users| users.parse().ok());
                }
            }
            None
        }
        IpAddr::V6(group) => ipv6_address_users(IPV6_GROUPS, group, interface),
    }
}

/// Gives how many sockets [`IPV6_GROUPS`] or [`IPV6_ANYCAST`] counts for the address on the interface, if listed.
///
/// Each table has a line for each interface's address: index, name, address and users.
fn ipv6_address_users(table: &str, address: Ipv6Addr, interface: InterfaceIndex) -> Option<usize> {
    let wanted: String = address
        .octets()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    std::fs::read_to_string(table)
        .expect("the host's IPv6 address table")
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            let index = fields.next()?.parse::<u32>().ok()?;
            let _name = fields.next();
            (index == interface.0 && fields.next() == Some(wanted.as_str()))
                .then(|| fields.next()?.parse().ok())
                .flatten()
        })
}

/// An anycast address no other check joins.
///
/// https://www.rfc-editor.org/rfc/rfc4193
const ANYCAST_MEMBERSHIP: Ipv6Addr = Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 0x123);

/// Checks that a connection joins and leaves an anycast address, as the host's anycast table shows, twice as once.
pub fn anycast_addresses_are_joined_and_left<S: TcpSockets>(sockets: S) {
    let interface = loopback_interface();
    let pair = pair_at(&sockets, ipv6_loopback(), &[], &[]);
    let users = || ipv6_address_users(IPV6_ANYCAST, ANYCAST_MEMBERSHIP, interface);
    assert_eq!(users(), None, "before");
    for (membership, expected) in [
        (Membership::Joined, Some(ONE_MEMBER)),
        (Membership::Joined, Some(ONE_MEMBER)),
        (Membership::Left, None),
        (Membership::Left, None),
    ] {
        let setting = TcpSetting::Ip(IpSocketSetting::Anycast(AnycastAddress {
            address: ANYCAST_MEMBERSHIP,
            interface: Some(interface),
            membership,
        }));
        assert_eq!(
            sockets.configure(&pair.client, setting.clone()),
            Ok(()),
            "{setting:?}"
        );
        assert_eq!(users(), expected, "{setting:?}");
    }
    assert_carries_bytes(&sockets, &pair);
    close(&sockets, pair);
}

/// Checks that a connection joins and leaves a multicast group, as the host's group table shows, twice as once.
///
/// The table lists the group with one user after a join and not after a leave; the connection still carries bytes.
pub fn multicast_groups_are_joined_and_left<S: TcpSockets>(sockets: S) {
    let interface = loopback_interface();
    for (address, group, joined, left) in [
        (
            ipv4_loopback(),
            IpAddr::V4(MEMBERSHIP_V4),
            IpSocketSetting::MulticastV4(MulticastGroup {
                group: MEMBERSHIP_V4,
                interface: Some(interface),
                membership: MulticastMembership::Joined(SourceFilter::Exclude(&[])),
            }),
            IpSocketSetting::MulticastV4(MulticastGroup {
                group: MEMBERSHIP_V4,
                interface: Some(interface),
                membership: MulticastMembership::Left,
            }),
        ),
        (
            ipv6_loopback(),
            IpAddr::V6(MEMBERSHIP_V6),
            IpSocketSetting::MulticastV6(MulticastGroup {
                group: MEMBERSHIP_V6,
                interface: Some(interface),
                membership: MulticastMembership::Joined(SourceFilter::Exclude(&[])),
            }),
            IpSocketSetting::MulticastV6(MulticastGroup {
                group: MEMBERSHIP_V6,
                interface: Some(interface),
                membership: MulticastMembership::Left,
            }),
        ),
    ] {
        let pair = pair_at(&sockets, address, &[], &[]);
        assert_eq!(group_users(group, interface), None, "{group} before");
        for (setting, users) in [
            (&joined, Some(ONE_MEMBER)),
            (&joined, Some(ONE_MEMBER)),
            (&left, None),
            (&left, None),
        ] {
            assert_eq!(
                sockets.configure(&pair.client, TcpSetting::Ip(*setting)),
                Ok(()),
                "{setting:?}"
            );
            assert_eq!(group_users(group, interface), users, "{setting:?}");
        }
        assert_carries_bytes(&sockets, &pair);
        sockets
            .configure(&pair.client, TcpSetting::Ip(joined))
            .expect("joined again");
        close(&sockets, pair);
        assert_eq!(
            group_users(group, interface),
            None,
            "{group} after the close"
        );
    }
}

const MD5_KEY: &[u8] = b"conformance key";
const OTHER_MD5_KEY: &[u8] = b"another key";

/// Gives an MD5 key for the peers at 127.0.0.1 set, or with none removed.
fn md5_key(key: Option<&'static [u8]>) -> TcpSetting<'static> {
    let peer = TcpMd5Peer {
        prefix: IpPrefix {
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            length: PrefixLength(Ipv4Addr::BITS as u8),
        },
        interface: None,
    };
    TcpSetting::Md5Key(match key {
        Some(key) => TcpMd5KeyChange::Set { peer, key },
        None => TcpMd5KeyChange::Removed(peer),
    })
}

/// Checks that a listener with an MD5 key for its peers takes only connections signed with that key, whose every segment carries the signature.
///
/// A key set again replaces the last, and removed (once or twice) it bars no plain connection.
pub fn md5_keys_admit_only_connections_signed_with_them<S: TcpSockets>(sockets: S) {
    for (listener_keys, client_keys, admitted) in [
        (vec![Some(MD5_KEY)], vec![Some(MD5_KEY)], true),
        (
            vec![Some(MD5_KEY)],
            vec![Some(MD5_KEY), Some(MD5_KEY)],
            true,
        ),
        (
            vec![Some(OTHER_MD5_KEY), Some(MD5_KEY)],
            vec![Some(MD5_KEY)],
            true,
        ),
        (vec![Some(MD5_KEY)], vec![Some(OTHER_MD5_KEY)], false),
        (vec![Some(MD5_KEY)], vec![], false),
        (vec![Some(MD5_KEY)], vec![Some(MD5_KEY), None], false),
        (vec![Some(MD5_KEY), None, None], vec![], true),
    ] {
        let listener_settings: Vec<_> = listener_keys
            .iter()
            .map(|key| TcpListenerSetting::Socket(md5_key(*key)))
            .collect();
        let client_settings: Vec<_> = client_keys.iter().map(|key| md5_key(*key)).collect();
        let case = format!("listener {listener_keys:?}, client {client_keys:?}");
        let (listener, local) = listen(&sockets, ipv4_loopback(), &listener_settings);
        let capture = Capture::start(local);
        let client = sockets
            .connect(
                TcpProtocol::Tcp,
                OWNER,
                bind_any_port(ipv4_any()),
                local,
                &client_settings,
                TcpFastOpen::Off,
            )
            .expect("connect")
            .connection;
        let signed = client_keys.last().copied().flatten().is_some();
        let mut header = [u8::MIN; 0];
        if admitted {
            let server = retry(|| {
                sockets
                    .accept(&listener, &mut header)
                    .map(|taken| taken.connection)
            })
            .expect(&case);
            let pair = Pair {
                listener,
                local,
                client,
                server,
            };
            assert_carries_bytes(&sockets, &pair);
            std::thread::sleep(SETTLE);
            let segments = capture.segments();
            assert!(
                segments.iter().all(|segment| segment.signed == signed),
                "{case}"
            );
            close(&sockets, pair);
        } else {
            std::thread::sleep(SETTLE);
            assert!(
                matches!(
                    sockets.accept(&listener, &mut header),
                    Err(HostError::WouldBlock)
                ),
                "{case}"
            );
            let segments = capture.segments();
            let synchronize = segments
                .iter()
                .find(|segment| segment.to_listener && segment.synchronize)
                .expect("the SYN");
            assert_eq!(synchronize.signed, signed, "{case}");
            assert!(
                segments.iter().all(|segment| segment.to_listener),
                "{case}: answered"
            );
            sockets.close(client, TcpClose::Abort);
            sockets.stop_listening(listener);
        }
    }
}

const AUTHENTICATION_ALGORITHM: &str = "hmac(sha1)";
const AUTHENTICATION_SECRET: &[u8] = b"conformance secret";
const OTHER_AUTHENTICATION_SECRET: &[u8] = b"another secret";
/// A MAC length unlike Linux's default for the algorithm (12 bytes), so that the length given shows on the wire.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const AUTHENTICATION_MAC_LENGTH: TcpAuthenticationMacLength = TcpAuthenticationMacLength(16);
/// The first key identifier of the authentication checks.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const FIRST_KEY: TcpAuthenticationKeyIdentifier = TcpAuthenticationKeyIdentifier(1);
/// The second key identifier of the authentication checks.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const SECOND_KEY: TcpAuthenticationKeyIdentifier = TcpAuthenticationKeyIdentifier(2);
/// The third key identifier of the authentication checks.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const THIRD_AUTHENTICATION_KEY: TcpAuthenticationKeyIdentifier = TcpAuthenticationKeyIdentifier(3);

/// Gives a key for the peers at 127.0.0.1 that sends and receives with the one identifier.
fn authentication_identity(
    identifier: TcpAuthenticationKeyIdentifier,
) -> TcpAuthenticationKeyIdentity {
    authentication_identity_of(Ipv4Addr::LOCALHOST, identifier)
}

fn authentication_identity_of(
    peer: Ipv4Addr,
    identifier: TcpAuthenticationKeyIdentifier,
) -> TcpAuthenticationKeyIdentity {
    TcpAuthenticationKeyIdentity {
        peer: IpPrefix {
            address: IpAddr::V4(peer),
            length: PrefixLength(Ipv4Addr::BITS as u8),
        },
        interface: None,
        send_identifier: identifier,
        receive_identifier: identifier,
    }
}

fn authentication_key(
    identifier: TcpAuthenticationKeyIdentifier,
    secret: &'static [u8],
    other_options: TcpAuthenticationOptionCoverage,
    key_use: TcpAuthenticationKeyUse,
) -> TcpSetting<'static> {
    TcpSetting::AddAuthenticationKey(TcpAuthenticationKeyAddition {
        identity: authentication_identity(identifier),
        algorithm: AUTHENTICATION_ALGORITHM,
        mac_length: AUTHENTICATION_MAC_LENGTH,
        other_options,
        secret,
        key_use,
    })
}

/// Gives a key removed, with another made current and requested next.
fn authentication_key_removed(
    identifier: TcpAuthenticationKeyIdentifier,
    current: Option<TcpAuthenticationKeyIdentifier>,
) -> TcpSetting<'static> {
    TcpSetting::RemoveAuthenticationKey(TcpAuthenticationKeyRemoval {
        identity: authentication_identity(identifier),
        current,
        requested_next: current,
        timing: TcpAuthenticationRemovalTiming::Now,
    })
}

fn authentication_options(
    key: Option<TcpAuthenticationKeyIdentifier>,
    requirement: TcpAuthenticationRequirement,
) -> TcpSetting<'static> {
    TcpSetting::AuthenticationOptions(TcpAuthenticationOptions {
        current: key,
        requested_next: key,
        requirement,
        icmp_errors: TcpAuthenticationIcmpErrors::Ignored,
    })
}

/// Gives the option a segment signed with the key carries, asking for the same key next.
fn signed_with(key: TcpAuthenticationKeyIdentifier) -> Option<AuthenticationOption> {
    Some(AuthenticationOption {
        key: key.0,
        next_key: key.0,
        length: AUTHENTICATION_OPTION_HEADER + AUTHENTICATION_MAC_LENGTH.0,
    })
}

/// Tells whether a listener with the settings admits a client with its own, and the option the client's SYN carries.
///
/// For a connection made, every segment both ways is signed when the SYN is.
fn authentication_round<S: TcpSockets>(
    sockets: &S,
    listener_settings: &[TcpSetting<'static>],
    client_settings: &[TcpSetting<'static>],
) -> (Option<Pair<S>>, Option<AuthenticationOption>, Capture) {
    let listener_settings: Vec<_> = listener_settings
        .iter()
        .map(|setting| TcpListenerSetting::Socket(setting.clone()))
        .collect();
    let (listener, local) = listen(sockets, ipv4_loopback(), &listener_settings);
    let capture = Capture::start(local);
    let client = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            client_settings,
            TcpFastOpen::Off,
        )
        .expect("connect")
        .connection;
    std::thread::sleep(SETTLE);
    let mut header = [u8::MIN; 0];
    let synchronize = capture
        .segments()
        .iter()
        .find(|segment| segment.to_listener && segment.synchronize)
        .expect("the SYN")
        .authentication;
    match sockets.accept(&listener, &mut header) {
        Ok(accepted) => {
            let pair = Pair {
                listener,
                local,
                client,
                server: accepted.connection,
            };
            assert_carries_bytes(sockets, &pair);
            std::thread::sleep(SETTLE);
            for segment in capture.segments() {
                assert_eq!(
                    segment.authentication.is_some(),
                    synchronize.is_some(),
                    "{segment:?}"
                );
                if let Some(option) = segment.authentication {
                    assert_eq!(option.length, synchronize.expect("signed").length);
                }
            }
            (Some(pair), synchronize, capture)
        }
        Err(HostError::WouldBlock) => {
            assert!(
                capture.segments().iter().all(|segment| segment.to_listener),
                "answered"
            );
            sockets.close(client, TcpClose::Abort);
            sockets.stop_listening(listener);
            (None, synchronize, capture)
        }
        Err(error) => panic!("accept: {error:?}"),
    }
}

/// Checks that a listener with TCP Authentication Option keys takes only connections signed with one of them.
///
/// Signed by the key, MAC length and option coverage given; keys added twice are added once, the key asked to be
/// current signs the SYN, a key removed signs nothing, and a listener that requires authentication takes no plain
/// client until it no longer does.
pub fn authentication_keys_admit_only_connections_signed_with_them<S: TcpSockets>(sockets: S) {
    use TcpAuthenticationKeyUse::{CurrentAndRequestedNext, Stored};
    use TcpAuthenticationOptionCoverage::{Excluded, Included};
    use TcpAuthenticationRequirement::{Optional, Required};
    let first = authentication_key(FIRST_KEY, AUTHENTICATION_SECRET, Included, Stored);
    let second = authentication_key(SECOND_KEY, AUTHENTICATION_SECRET, Included, Stored);
    let second_current = authentication_key(
        SECOND_KEY,
        AUTHENTICATION_SECRET,
        Included,
        CurrentAndRequestedNext,
    );
    let other = authentication_key(FIRST_KEY, OTHER_AUTHENTICATION_SECRET, Included, Stored);
    let excluding = authentication_key(FIRST_KEY, AUTHENTICATION_SECRET, Excluded, Stored);
    let first_removed = authentication_key_removed(FIRST_KEY, None);
    let third = authentication_key(
        THIRD_AUTHENTICATION_KEY,
        AUTHENTICATION_SECRET,
        Included,
        Stored,
    );
    let first_removed_for_second = authentication_key_removed(FIRST_KEY, Some(SECOND_KEY));
    for (listener_settings, client_settings, admitted, synchronize) in [
        (
            vec![first.clone()],
            vec![first.clone()],
            true,
            signed_with(FIRST_KEY),
        ),
        (
            vec![first.clone()],
            vec![first.clone(), first.clone()],
            true,
            signed_with(FIRST_KEY),
        ),
        // The current key added before and after another, so that one of them is not Linux's choice without it.
        (
            vec![first.clone(), second.clone()],
            vec![first.clone(), second_current.clone()],
            true,
            signed_with(SECOND_KEY),
        ),
        (
            vec![first.clone(), second.clone()],
            vec![second_current.clone(), first.clone()],
            true,
            signed_with(SECOND_KEY),
        ),
        // A key held already and added again to be current.
        (
            vec![first.clone(), second.clone()],
            vec![second.clone(), first.clone(), second_current.clone()],
            true,
            signed_with(SECOND_KEY),
        ),
        (
            vec![first.clone(), second.clone()],
            vec![first.clone(), second.clone(), second_current.clone()],
            true,
            signed_with(SECOND_KEY),
        ),
        (
            vec![first.clone(), second.clone(), third.clone()],
            vec![
                third.clone(),
                second.clone(),
                first.clone(),
                first_removed_for_second.clone(),
                first_removed_for_second.clone(),
            ],
            true,
            signed_with(SECOND_KEY),
        ),
        (
            vec![first.clone(), second.clone(), third.clone()],
            vec![
                second.clone(),
                third,
                first.clone(),
                first_removed_for_second,
            ],
            true,
            signed_with(SECOND_KEY),
        ),
        (
            vec![first.clone()],
            vec![other],
            false,
            signed_with(FIRST_KEY),
        ),
        (vec![first.clone()], vec![], false, None),
        (
            vec![first.clone(), first_removed.clone(), first_removed],
            vec![],
            true,
            None,
        ),
        (
            vec![authentication_options(None, Required)],
            vec![],
            false,
            None,
        ),
        (
            vec![
                authentication_options(None, Required),
                authentication_options(None, Optional),
            ],
            vec![],
            true,
            None,
        ),
        (
            vec![excluding.clone()],
            vec![first],
            false,
            signed_with(FIRST_KEY),
        ),
        (
            vec![excluding.clone()],
            vec![excluding],
            true,
            signed_with(FIRST_KEY),
        ),
    ] {
        let case = format!("listener {listener_settings:?}, client {client_settings:?}");
        let (pair, signed, _) =
            authentication_round(&sockets, &listener_settings, &client_settings);
        assert_eq!(signed, synchronize, "{case}");
        assert_eq!(pair.is_some(), admitted, "{case}");
        if let Some(pair) = pair {
            close(&sockets, pair);
        }
    }
}

/// Checks that a connection's authentication options choose the key it signs with and the one it asks its peer for, set twice the same as once.
pub fn authentication_options_choose_the_keys_in_use<S: TcpSockets>(sockets: S) {
    use TcpAuthenticationKeyUse::{CurrentAndRequestedNext, Stored};
    use TcpAuthenticationOptionCoverage::Included;
    let keys = [
        authentication_key(FIRST_KEY, AUTHENTICATION_SECRET, Included, Stored),
        authentication_key(SECOND_KEY, AUTHENTICATION_SECRET, Included, Stored),
    ];
    let client_keys = [
        authentication_key(
            SECOND_KEY,
            AUTHENTICATION_SECRET,
            Included,
            CurrentAndRequestedNext,
        ),
        keys[0].clone(),
    ];
    let (pair, signed, capture) = authentication_round(&sockets, &keys, &client_keys);
    assert_eq!(signed, signed_with(SECOND_KEY));
    let pair = pair.expect("admitted");
    let switched = authentication_options(Some(FIRST_KEY), TcpAuthenticationRequirement::Optional);
    for _ in 0..2 {
        assert_eq!(sockets.configure(&pair.client, switched.clone()), Ok(()));
    }
    let start = Instant::now();
    assert_carries_bytes(&sockets, &pair);
    std::thread::sleep(SETTLE);
    let sent: Vec<_> = capture
        .since(start)
        .into_iter()
        .filter(Segment::is_data_to_listener)
        .collect();
    assert!(!sent.is_empty(), "the client's data");
    for segment in sent {
        assert_eq!(
            segment.authentication,
            signed_with(FIRST_KEY),
            "{segment:?}"
        );
    }
    close(&sockets, pair);
}

/// Checks that a congestion control that needs explicit congestion notification marks the data it sends as capable of it.
pub fn congestion_control_is_the_one_named<S: TcpSockets>(sockets: S) {
    let control = TcpSetting::CongestionControl(NOTIFYING_CONGESTION_CONTROL);
    let (pair, capture) = watched_pair(
        &sockets,
        ipv4_loopback(),
        std::slice::from_ref(&control),
        &[TcpListenerSetting::Socket(control.clone())],
    );
    let start = Instant::now();
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    let data = capture
        .wait_for(start, DEADLINE, Segment::is_data_to_listener)
        .expect("a data segment");
    assert_eq!(data.ecn, Some(IpEcn::TWO));
    close(&sockets, pair);
}

/// Checks that a pacing rate spreads a transfer out over time; lifted, the transfer is quick.
pub fn pacing_rate_spreads_what_is_sent<S: TcpSockets>(sockets: S) {
    let pacing = |rate| TcpSetting::Ip(IpSocketSetting::MaximumPacingRate(rate));
    let bytes = pattern(PACED_BYTES);
    for (settings, paced) in [
        (vec![pacing(Some(PACING_RATE))], true),
        (vec![pacing(Some(PACING_RATE)), pacing(None)], false),
    ] {
        let pair = pair_with(&sockets, &settings, &[]);
        let start = Instant::now();
        assert_eq!(
            transfer(&sockets, &pair.client, &pair.server, &bytes),
            bytes
        );
        let elapsed = start.elapsed();
        assert_eq!(elapsed > PACED_AT_LEAST, paced, "{settings:?}: {elapsed:?}");
        close(&sockets, pair);
    }
}

/// Checks that a transmit delay holds each segment back by that long; set to nothing, segments leave at once.
pub fn transmit_delay_holds_each_segment<S: TcpSockets>(sockets: S) {
    let pair = pair_with(&sockets, &[TcpSetting::TransmitDelay(TRANSMIT_DELAY)], &[]);
    let timed = |pair: &Pair<S>| {
        let start = Instant::now();
        send_all(&sockets, &pair.client, plain(), MESSAGE);
        assert_eq!(
            receive_up_to(&sockets, &pair.server, MESSAGE.len()),
            MESSAGE
        );
        start.elapsed()
    };
    let delayed = timed(&pair);
    assert!(delayed > TRANSMIT_DELAY - QUICK_WITHIN, "{delayed:?}");
    sockets
        .configure(&pair.client, TcpSetting::TransmitDelay(Duration::ZERO))
        .expect("no delay");
    let quick = timed(&pair);
    assert!(quick < TRANSMIT_DELAY - QUICK_WITHIN, "{quick:?}");
    close(&sockets, pair);
}

/// Connections sharing their address leave from one port to different peers; one not sharing cannot take it.
pub fn address_sharing_lets_connections_leave_from_one_port<S: TcpSockets>(sockets: S) {
    let shared = [TcpSetting::Port(PortSetting::AddressSharing(
        AddressSharing::Shared,
    ))];
    let listeners = [(); SHARING_PEERS].map(|_| listen(&sockets, ipv4_loopback(), &[]));
    let connect = |port: Port, peer: SocketAddr, settings: &[TcpSetting<'_>]| {
        sockets.connect(
            TcpProtocol::Tcp,
            OWNER,
            LocalBinding {
                address: ipv4_loopback(),
                port: LocalPort::Requested(port),
            },
            peer,
            settings,
            TcpFastOpen::Off,
        )
    };
    let first = connect(ANY_PORT, listeners[0].1, &shared).expect("the first connection");
    let port = Port(first.local.port());
    let second = connect(port, listeners[1].1, &shared).expect("a second from the port");
    assert_eq!(second.local, first.local);
    assert_eq!(
        connect(port, listeners[2].1, &[]).map(|connected| connected.local),
        Err(errno(Errno::EADDRINUSE))
    );
    sockets.close(first.connection, TcpClose::Orderly);
    sockets.close(second.connection, TcpClose::Orderly);
    for (listener, _) in listeners {
        sockets.stop_listening(listener);
    }
}

fn listen_at<S: TcpSockets>(
    sockets: &S,
    address: BindAddress,
    port: Port,
    settings: &[TcpListenerSetting<'_>],
) -> Result<TcpListening<S::Listener>, HostError> {
    sockets.listen(
        TcpProtocol::Tcp,
        OWNER,
        LocalBinding {
            address,
            port: LocalPort::Requested(port),
        },
        BACKLOG,
        settings,
    )
}

/// Listeners sharing their port listen on one; one not sharing cannot.
pub fn port_sharing_lets_listeners_share_a_port<S: TcpSockets>(sockets: S) {
    let shared = [TcpListenerSetting::Socket(TcpSetting::Port(
        PortSetting::PortSharing(PortSharing::Shared),
    ))];
    let first = listen_at(&sockets, ipv4_loopback(), ANY_PORT, &shared).expect("listen");
    let port = Port(first.local.port());
    let second = listen_at(&sockets, ipv4_loopback(), port, &shared).expect("listen on the port");
    assert_eq!(
        listen_at(&sockets, ipv4_loopback(), port, &[]).map(|listening| listening.local),
        Err(errno(Errno::EADDRINUSE))
    );
    sockets.stop_listening(first.listener);
    sockets.stop_listening(second.listener);
}

/// Checks that an address no interface holds can be bound only while nonlocal addresses are bindable.
pub fn nonlocal_addresses_are_bound_only_when_bindable<S: TcpSockets>(sockets: S) {
    let address = BindAddress::V4(NONLOCAL);
    let setting = |addresses| {
        TcpListenerSetting::Socket(TcpSetting::Port(PortSetting::NonlocalAddresses(addresses)))
    };
    let bound = listen_at(
        &sockets,
        address,
        ANY_PORT,
        &[setting(NonlocalAddresses::Bindable)],
    )
    .expect("listen on a nonlocal address");
    sockets.stop_listening(bound.listener);
    for settings in [
        vec![],
        vec![
            setting(NonlocalAddresses::Bindable),
            setting(NonlocalAddresses::Refused),
        ],
    ] {
        assert_eq!(
            listen_at(&sockets, address, ANY_PORT, &settings).map(|listening| listening.local),
            Err(errno(Errno::EADDRNOTAVAIL))
        );
    }
}

/// Where the backlog check gives a listener its backlog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BacklogGiven {
    ToListen,
    AsListenSetting,
    Later,
}

/// Checks that a listener of backlog 0, given to listen, as its setting or set later, holds one connection, not three.
pub fn listener_backlog_bounds_the_accept_queue<S: TcpSockets>(sockets: S) {
    let smallest = TcpListenerSetting::Backlog(SMALLEST_BACKLOG);
    for (given, backlog, settings) in [
        (BacklogGiven::ToListen, SMALLEST_BACKLOG, vec![]),
        (
            BacklogGiven::AsListenSetting,
            BACKLOG,
            vec![smallest.clone()],
        ),
        (BacklogGiven::Later, BACKLOG, vec![]),
    ] {
        let listening = sockets
            .listen(
                TcpProtocol::Tcp,
                OWNER,
                bind_any_port(ipv4_loopback()),
                backlog,
                &settings,
            )
            .expect("listen on loopback");
        let (listener, local) = (listening.listener, listening.local);
        if given == BacklogGiven::Later {
            sockets
                .configure(&listener, smallest.clone())
                .expect("a smaller backlog");
        }
        let connect = || {
            sockets
                .connect(
                    TcpProtocol::Tcp,
                    OWNER,
                    bind_any_port(ipv4_any()),
                    local,
                    &[],
                    TcpFastOpen::Off,
                )
                .expect("connect")
                .connection
        };
        // The first connection fills the queue before the others come, so that none of theirs can race it: a full
        // queue drops their SYNs.
        let first = connect();
        {
            let (_, waker) = counting_waker();
            let mut queued = pin!(sockets.wait_until_ready(&listener, ReadyFor::READABLE));
            let start = Instant::now();
            while poll_once(queued.as_mut(), &waker).is_pending() {
                assert!(
                    start.elapsed() < DEADLINE,
                    "the first connection was never queued"
                );
                std::thread::sleep(RETRY_INTERVAL);
            }
        }
        let clients: Vec<_> = std::iter::once(first)
            .chain((1..BACKLOG_OVERFLOW).map(|_| connect()))
            .collect();
        std::thread::sleep(SETTLE);
        let mut header = [u8::MIN; 0];
        let mut accepted = vec![
            retry(|| {
                sockets
                    .accept(&listener, &mut header)
                    .map(|taken| taken.connection)
            })
            .expect("the one connection the queue holds"),
        ];
        // The others wait for a retransmitted SYN (a second at the least), so none comes while this is watched; the
        // last try is made after the period, however late this thread ran.
        let start = Instant::now();
        loop {
            let done = start.elapsed() >= HELD_FOR;
            if let Ok(taken) = sockets.accept(&listener, &mut header) {
                accepted.push(taken.connection);
            }
            if done {
                break;
            }
        }
        assert_eq!(accepted.len(), 1, "{given:?}");
        for connection in accepted.into_iter().chain(clients) {
            sockets.close(connection, TcpClose::Orderly);
        }
        sockets.stop_listening(listener);
    }
}

/// Gives the flow label of the next data segment, which IPv6 connections carry.
fn data_flow_label<S: TcpSockets>(sockets: &S, pair: &Pair<S>, capture: &Capture) -> u32 {
    let start = Instant::now();
    send_all(sockets, &pair.client, plain(), MESSAGE);
    capture
        .wait_for(start, DEADLINE, Segment::is_data_to_listener)
        .and_then(|segment| segment.flow_label)
        .expect("an IPv6 data segment")
}

/// Checks that negative advice makes a connection pick a new flow, which shows in its flow label, unless rehashing is off.
pub fn negative_route_advice_picks_a_new_flow<S: TcpSockets>(sockets: S) {
    let rehash = TcpSetting::TransmitRehash;
    for (settings, rehashed) in [
        (vec![], true),
        (vec![rehash(TransmitRehash::Disabled)], false),
        (
            vec![
                rehash(TransmitRehash::Disabled),
                rehash(TransmitRehash::Enabled),
            ],
            true,
        ),
        (
            vec![
                rehash(TransmitRehash::Disabled),
                rehash(TransmitRehash::SystemDefault),
            ],
            true,
        ),
    ] {
        let (pair, capture) = watched_pair(&sockets, ipv6_loopback(), &settings, &[]);
        let before = data_flow_label(&sockets, &pair, &capture);
        sockets
            .give_negative_route_advice(&pair.client)
            .expect("advice");
        let after = data_flow_label(&sockets, &pair, &capture);
        assert_eq!(before != after, rehashed, "{settings:?}");
        close(&sockets, pair);
    }
}

/// Checks that a listener reflecting flow labels answers a SYN with the SYN's own flow label.
pub fn flow_label_reflection_answers_with_the_peers_label<S: TcpSockets>(sockets: S) {
    let reflecting = TcpListenerSetting::FlowLabelReflection(true);
    for (settings, reflected) in [
        (vec![reflecting.clone()], true),
        (
            vec![reflecting, TcpListenerSetting::FlowLabelReflection(false)],
            false,
        ),
    ] {
        let (pair, capture) = watched_pair(&sockets, ipv6_loopback(), &[], &settings);
        let synchronizing = |to_listener: bool| {
            capture.first(|segment| segment.to_listener == to_listener && segment.synchronize)
        };
        let (synchronize, answer) = (synchronizing(true), synchronizing(false));
        assert_eq!(
            answer.flow_label == synchronize.flow_label,
            reflected,
            "{settings:?}"
        );
        close(&sockets, pair);
    }
}

/// Checks that a window probe goes out one sequence number below the next and is answered.
///
/// The connection keeps sharing its address and, when in repair mode, stays in it.
pub fn window_probe_is_answered_and_changes_no_setting<S: TcpSockets>(sockets: S) {
    let shared = [TcpSetting::Port(PortSetting::AddressSharing(
        AddressSharing::Shared,
    ))];
    let from_port = |port: u16| LocalBinding {
        address: ipv4_loopback(),
        port: LocalPort::Requested(Port(port)),
    };
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    let capture = Capture::start(local);
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            from_port(ANY_PORT.0),
            local,
            &shared,
            TcpFastOpen::Off,
        )
        .expect("connect");
    let server = accept(&sockets, &listener);
    let pair = Pair {
        listener,
        local,
        client: connected.connection,
        server,
    };
    assert_carries_bytes(&sockets, &pair);
    std::thread::sleep(SETTLE);
    let next = capture.next_to_listener();
    let start = Instant::now();
    sockets
        .send_window_probe(&pair.client)
        .expect("window probe");
    let probe = capture
        .wait_for(start, PUSHED_WITHIN, |segment| is_probe(segment, next))
        .expect("the probe");
    capture
        .wait_for(start, PUSHED_WITHIN, |segment| {
            !segment.to_listener && segment.at >= probe.at && segment.acknowledgement == next
        })
        .expect("its answer");
    let (other, peer) = listen(&sockets, ipv4_loopback(), &[]);
    let second = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            from_port(connected.local.port()),
            peer,
            &shared,
            TcpFastOpen::Off,
        )
        .expect("a second connection from the port the probing one shares");
    sockets.close(second.connection, TcpClose::Abort);
    sockets.stop_listening(other);
    let clock = TcpTimestampClock {
        value: TcpTimestamp(CLOCK_READING),
        unit: TcpTimestampUnit::Milliseconds,
    };
    sockets
        .configure(&pair.client, TcpSetting::Repair(TcpRepair::On))
        .expect("repair mode");
    sockets
        .configure(
            &pair.client,
            TcpSetting::RepairQueue(TcpRepairQueue::Receive),
        )
        .expect("the receive queue");
    sockets
        .send_window_probe(&pair.client)
        .expect("window probe in repair mode");
    // In repair mode a send fills the queue chosen and a peek reads it; with no queue chosen, both are refused.
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    let mut queued = vec![u8::MIN; MESSAGE.len()];
    let peeked = sockets
        .receive(
            &pair.client,
            TcpReceiveRequest::Peek { offset: NO_BYTES },
            &mut queued,
        )
        .expect("a peek at the receive queue still chosen");
    assert_eq!(&queued[..peeked.length.0], MESSAGE);
    assert_eq!(sockets.set_timestamp_clock(&pair.client, clock), Ok(()));
    sockets
        .configure(&pair.client, TcpSetting::Repair(TcpRepair::Off))
        .expect("out of repair mode");
    close(&sockets, pair);
}

/// Gives how far past [`CLOCK_READING`] the next segment's TSval reads, and when it was seen, after a clock setting.
///
/// The connection's clock is set in repair mode to the reading in the unit and then runs for [`CLOCK_RUNS`].
fn clock_run<S: TcpSockets>(
    sockets: &S,
    address: BindAddress,
    unit: TcpTimestampUnit,
) -> Result<(u32, Duration), HostError> {
    let (pair, capture) = watched_pair(sockets, address, &[], &[]);
    let clock = TcpTimestampClock {
        value: TcpTimestamp(CLOCK_READING),
        unit,
    };
    sockets
        .configure(&pair.client, TcpSetting::Repair(TcpRepair::On))
        .expect("repair mode");
    let set = Instant::now();
    let outcome = sockets.set_timestamp_clock(&pair.client, clock);
    std::thread::sleep(CLOCK_RUNS);
    sockets
        .configure(&pair.client, TcpSetting::Repair(TcpRepair::Off))
        .expect("out of repair mode");
    let run = outcome.map(|()| {
        send_all(sockets, &pair.client, plain(), MESSAGE);
        let data = capture
            .wait_for(set, DEADLINE, Segment::is_data_to_listener)
            .expect("a data segment");
        let since = data
            .timestamp
            .expect("a timestamp")
            .wrapping_sub(CLOCK_READING);
        (since, data.at.duration_since(set))
    });
    close(sockets, pair);
    run
}

/// Asserts that a clock set to [`CLOCK_READING`] counted in the unit over at least [`CLOCK_RUNS`] and at most the time it ran.
fn assert_counts_in(unit: TcpTimestampUnit, (since, ran): (u32, Duration)) {
    let (at_least, at_most) = match unit {
        TcpTimestampUnit::Milliseconds => (CLOCK_RUNS.as_millis(), ran.as_millis()),
        TcpTimestampUnit::Microseconds => (CLOCK_RUNS.as_micros(), ran.as_micros()),
    };
    let slack = TIMESTAMP_SLACK;
    assert!(
        (at_least - slack..=at_most + slack).contains(&u128::from(since)),
        "{since} {unit:?} after the reading, over {ran:?}"
    );
}

/// Checks that outside repair mode the timestamp clock cannot be set.
///
/// In it, the value set is what the next segments carry, counted in milliseconds or, on a route whose segments carry microseconds, in microseconds.
pub fn timestamp_clock_set_in_repair_mode_stamps_segments<S: TcpSockets>(sockets: S) {
    let (pair, _capture) = watched_pair(&sockets, ipv4_loopback(), &[], &[]);
    let clock = TcpTimestampClock {
        value: TcpTimestamp(CLOCK_READING),
        unit: TcpTimestampUnit::Milliseconds,
    };
    // The contract names no error for a socket out of repair mode, so any refusal holds.
    assert!(sockets.set_timestamp_clock(&pair.client, clock).is_err());
    assert!(
        sockets
            .set_listener_timestamp_clock(&pair.listener, clock)
            .is_err()
    );
    // A listener taken into repair mode takes the clock; one that refuses repair mode refuses the clock too.
    let repairing = sockets.configure(
        &pair.listener,
        TcpListenerSetting::Socket(TcpSetting::Repair(TcpRepair::On)),
    );
    let set = sockets.set_listener_timestamp_clock(&pair.listener, clock);
    match repairing {
        Ok(()) => {
            set.expect("set the listener's clock");
            match sockets.read_listener_state(&pair.listener, TcpListenerQuery::Timestamp) {
                Ok(TcpReading::Timestamp(read)) => assert!(
                    read.value.0.wrapping_sub(CLOCK_READING) < CLOCK_READING,
                    "{read:?}"
                ),
                other => panic!("the listener's clock read {other:?}"),
            }
        }
        Err(_) => assert!(set.is_err(), "{set:?}"),
    }
    close(&sockets, pair);
    for (address, unit) in [
        (ipv4_loopback(), TcpTimestampUnit::Milliseconds),
        (
            BindAddress::V4(MICROSECOND_CLOCK),
            TcpTimestampUnit::Microseconds,
        ),
    ] {
        let run = clock_run(&sockets, address, unit).expect("set the clock");
        assert_counts_in(unit, run);
    }
}

/// Checks that a clock asked to count in the other unit than its route's segments carry either does so or is refused as not supported.
pub fn timestamp_clock_in_the_other_unit_counts_in_it_or_is_refused<S: TcpSockets>(sockets: S) {
    for (address, unit) in [
        (ipv4_loopback(), TcpTimestampUnit::Microseconds),
        (
            BindAddress::V4(MICROSECOND_CLOCK),
            TcpTimestampUnit::Milliseconds,
        ),
    ] {
        match clock_run(&sockets, address, unit) {
            Ok(run) => assert_counts_in(unit, run),
            Err(error) => assert_eq!(error, HostError::NotSupported, "{unit:?}"),
        }
    }
}

/// Rounds of one segment each that the statistics check sends.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const STATISTICS_ROUNDS: usize = 10;

/// The hop limit of the peer's acknowledgements in the statistics check.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const STATISTICS_HOP_LIMIT: HopLimit = HopLimit(33);

/// Checks that the statistics of each round's acknowledgement report agree with what the connection did.
///
/// Rounds of one small segment, acknowledged late by a peer with a transmit delay and a hop limit of its own.
pub fn transmit_statistics_describe_the_connection<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    for setting in [
        TcpSetting::TransmitDelay(TRANSMIT_DELAY),
        TcpSetting::Packet(IpPacketSetting::HopLimit {
            version: IpVersion::V4,
            limit: Some(STATISTICS_HOP_LIMIT),
        }),
    ] {
        sockets
            .configure(&pair.server, setting)
            .expect("a setting of the peer");
    }
    let reported = TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key: KEY,
            acknowledgement: Report::Requested,
            ..no_reports()
        },
        transmit_statistics: Report::Requested,
        ..plain()
    };
    let mut last = None;
    for _ in 0..STATISTICS_ROUNDS {
        send_all(&sockets, &pair.client, reported, MESSAGE);
        assert_eq!(
            receive_up_to(&sockets, &pair.server, MESSAGE.len()),
            MESSAGE
        );
        let TcpErrorQueueEntry::TransmitReport { report, statistics } =
            take_entry(&sockets, &pair.client)
        else {
            panic!("not a transmit report");
        };
        assert_eq!(report.key, KEY);
        last = statistics;
    }
    let statistics = last.expect("the statistics asked for");
    let rounds = STATISTICS_ROUNDS as u64;
    assert!(
        statistics.bytes_sent.0 >= rounds * MESSAGE.len() as u64,
        "{statistics:?}"
    );
    assert!(statistics.data_segments_out.0 >= rounds, "{statistics:?}");
    assert!(statistics.delivered.0 >= rounds, "{statistics:?}");
    assert_eq!(statistics.delivered_congestion_experienced.0, 0);
    assert_eq!(
        statistics.total_retransmissions.0 == 0,
        statistics.bytes_retransmitted.0 == 0,
        "{statistics:?}"
    );
    assert!(
        statistics.minimum_round_trip_time < TRANSMIT_DELAY,
        "the handshake's: {statistics:?}"
    );
    assert!(
        statistics.smoothed_round_trip_time > TRANSMIT_DELAY / 2
            && statistics.smoothed_round_trip_time > statistics.minimum_round_trip_time,
        "{statistics:?}"
    );
    assert!(statistics.busy >= TRANSMIT_DELAY, "{statistics:?}");
    assert!(
        statistics.receive_window_limited < TRANSMIT_DELAY
            && statistics.send_buffer_limited < TRANSMIT_DELAY,
        "{statistics:?}"
    );
    assert!(
        statistics.pacing_rate.0 > 0 && statistics.delivery_rate.0 > 0,
        "{statistics:?}"
    );
    assert_eq!(
        statistics.delivery_rate_limit,
        DeliveryRateLimit::Application
    );
    assert!(
        statistics.send_slow_start_threshold.0 > statistics.send_congestion_window.0
            && statistics.send_congestion_window.0 >= 1,
        "still in slow start: {statistics:?}"
    );
    assert_eq!(
        statistics.congestion_avoidance_state,
        TcpCongestionAvoidanceState::Open
    );
    assert_eq!(statistics.received_hop_limit, STATISTICS_HOP_LIMIT);
    assert_eq!(statistics.not_sent, NO_BYTES);
    assert!(
        statistics.earliest_departure > Duration::ZERO,
        "{statistics:?}"
    );
    assert_eq!(statistics.recurring_retransmissions, RetryCount(u8::MIN));
    assert_eq!(statistics.duplicate_selective_acknowledgements.0, 0);
    assert_eq!(statistics.reordering_seen.0, 0);
    assert_eq!(statistics.timeout_rehashes.0, 0);
    assert_eq!(statistics.rehashes.0, 0);
    close(&sockets, pair);
}

/// Checks that asked for with none scheduled, the delayed acknowledgement leaves the mode as it was.
pub fn delayed_acknowledgement_leaves_the_mode_when_none_is_scheduled<S: TcpSockets>(sockets: S) {
    let (pair, capture) = watched_pair(&sockets, ipv4_loopback(), &[], &[]);
    sockets
        .configure(
            &pair.server,
            TcpSetting::QuickAcknowledgement(TcpQuickAcknowledgement::Off),
        )
        .expect("delay acknowledgements");
    sockets
        .send_delayed_acknowledgement(&pair.server, TcpQuickAcknowledgement::On)
        .expect("nothing to send");
    let delay = acknowledgement_delay(&sockets, &pair, &capture);
    assert!(delay > QUICK_WITHIN, "{delay:?}");
    close(&sockets, pair);
}

/// Checks that accept names the interface a SYN came in on, the address to reply from and the sender's mark.
///
/// The interface also for an address loopback's prefix holds and for multipath, the address it was sent to as the one
/// to reply from (IPv4 only), and the mark its sender gave it (TCP only).
pub fn accept_names_where_the_synchronize_arrived<S: TcpSockets>(sockets: S) {
    let interface = loopback_interface();
    for protocol in [TcpProtocol::Tcp, TcpProtocol::Multipath] {
        for (address, specific_destination) in [
            (BindAddress::V4(OTHER_LOOPBACK), Some(OTHER_LOOPBACK)),
            (ipv6_loopback(), None),
        ] {
            let case = format!("{protocol:?} to {address:?}");
            let listening = sockets
                .listen(protocol, OWNER, bind_any_port(address), BACKLOG, &[])
                .expect("listen");
            let connected = sockets
                .connect(
                    protocol,
                    OWNER,
                    bind_any_port(any_of(address)),
                    listening.local,
                    &[TcpSetting::Packet(IpPacketSetting::Mark(MARK))],
                    TcpFastOpen::Off,
                )
                .expect("connect");
            let accepted = accept_with_header(&sockets, &listening.listener, NO_HEADER_ROOM);
            let fields = accepted.synchronize;
            assert_eq!(fields.interface, interface, "{case}: {fields:?}");
            assert_eq!(
                fields.specific_destination, specific_destination,
                "{case}: {fields:?}"
            );
            // Linux gives a SYN's mark only to a TCP socket it accepts, not to a multipath one.
            if protocol == TcpProtocol::Tcp {
                assert_eq!(fields.mark, MARK, "{case}: {fields:?}");
            }
            sockets.close(accepted.connection, TcpClose::Orderly);
            sockets.close(connected.connection, TcpClose::Orderly);
            sockets.stop_listening(listening.listener);
        }
    }
}

/// The owner of the connection of the owner check, other than [`OWNER`].
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const CONNECTION_OWNER: UserId = UserId(1001);
/// The owner of the listener of the owner check, other than [`OWNER`].
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const LISTENER_OWNER: UserId = UserId(1002);

/// The host's tables of TCP sockets, a line for each.
///
/// Slot, local and peer address (each the address and port in hexadecimal after a colon), state, queues, timer, retransmissions, user and more.
const IPV4_TCP_SOCKETS: &str = "/proc/net/tcp";
const IPV6_TCP_SOCKETS: &str = "/proc/net/tcp6";
const ADDRESS_PORT_SEPARATOR: char = ':';
/// The tables write addresses and ports in hexadecimal.
///
/// https://docs.kernel.org/networking/proc_net_tcp.html
const TABLE_NUMBER_RADIX: u32 = 16;
/// The column of the tables that holds a socket's user.
///
/// https://docs.kernel.org/networking/proc_net_tcp.html
const USER_COLUMN: usize = 7;

/// Gives the user the host's table of TCP sockets names for the socket between the ports, a listener's peer port being 0.
fn socket_owner(version: IpVersion, local: Port, peer: Port) -> Option<u32> {
    let table = match version {
        IpVersion::V4 => IPV4_TCP_SOCKETS,
        IpVersion::V6 => IPV6_TCP_SOCKETS,
    };
    let port = |address: &str| {
        address
            .rsplit(ADDRESS_PORT_SEPARATOR)
            .next()
            .and_then(|port| u16::from_str_radix(port, TABLE_NUMBER_RADIX).ok())
    };
    std::fs::read_to_string(table)
        .expect("the host's TCP sockets")
        .lines()
        .skip(1)
        .find_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            let wanted =
                port(fields.get(1)?) == Some(local.0) && port(fields.get(2)?) == Some(peer.0);
            wanted
                .then(|| fields.get(USER_COLUMN)?.parse().ok())
                .flatten()
        })
}

/// Checks that a connection and a listener belong to the owners they were made for, over IPv4 and IPv6.
pub fn connect_and_listen_make_sockets_of_their_owner<S: TcpSockets>(sockets: S) {
    for (address, version) in [
        (ipv4_loopback(), IpVersion::V4),
        (ipv6_loopback(), IpVersion::V6),
    ] {
        let listening = sockets
            .listen(
                TcpProtocol::Tcp,
                LISTENER_OWNER,
                bind_any_port(address),
                BACKLOG,
                &[],
            )
            .expect("listen");
        let connected = sockets
            .connect(
                TcpProtocol::Tcp,
                CONNECTION_OWNER,
                bind_any_port(any_of(address)),
                listening.local,
                &[],
                TcpFastOpen::Off,
            )
            .expect("connect");
        let server = accept(&sockets, &listening.listener);
        let listener_port = Port(listening.local.port());
        assert_eq!(
            socket_owner(version, listener_port, ANY_PORT),
            Some(LISTENER_OWNER.0),
            "the listener over {version:?}"
        );
        assert_eq!(
            socket_owner(version, Port(connected.local.port()), listener_port),
            Some(CONNECTION_OWNER.0),
            "the connection over {version:?}"
        );
        close(
            &sockets,
            Pair {
                listener: listening.listener,
                local: listening.local,
                client: connected.connection,
                server,
            },
        );
    }
}

/// Checks that a connect in repair mode is established at once and sends nothing.
pub fn connect_in_repair_mode_is_established_at_once<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[TcpSetting::Repair(TcpRepair::On)],
            TcpFastOpen::Off,
        )
        .expect("connect in repair mode");
    assert_eq!(connected.start, TcpConnectStart::EstablishedForRepair);
    std::thread::sleep(SETTLE);
    let mut header = [u8::MIN; 0];
    assert_eq!(
        sockets.accept(&listener, &mut header).map(drop),
        Err(HostError::WouldBlock)
    );
    sockets.close(connected.connection, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

/// Checks that a listener's receive timestamping holds for the connections it accepts.
pub fn listener_timestamping_reaches_the_connections_it_accepts<S: TcpSockets>(sockets: S) {
    let timestamping = Timestamping {
        receive_software: Report::Requested,
        receive_hardware: Report::NotRequested,
        bound_clock: None,
    };
    let pair = pair_with(
        &sockets,
        &[],
        &[TcpListenerSetting::Socket(TcpSetting::Ip(
            IpSocketSetting::Timestamping(timestamping),
        ))],
    );
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    let mut buffer = [u8::MIN; 1];
    let got = retry(|| sockets.receive(&pair.server, TcpReceiveRequest::Read, &mut buffer))
        .expect("receive");
    assert!(got.timestamps.software.is_some(), "{got:?}");
    close(&sockets, pair);
}

/// Checks that a listener asked to save SYNs leaves each accepted connection its SYN's headers, to be taken once.
///
/// From the IP header or from the link header before it as asked, kept while the buffer is too small for them; a
/// listener not asked, or asked to stop, leaves none. Accept gives the IP header whichever is saved. A multipath
/// listener may refuse to save them, and then leaves none either.
pub fn saved_synchronize_is_taken_once<S: TcpSockets>(sockets: S) {
    let save = TcpListenerSetting::SaveSynchronize(TcpSavedSynchronize::FromNetworkHeader);
    let stop = TcpListenerSetting::SaveSynchronize(TcpSavedSynchronize::Off);
    let link = TcpListenerSetting::SaveSynchronize(TcpSavedSynchronize::FromLinkHeader);
    for protocol in [TcpProtocol::Tcp, TcpProtocol::Multipath] {
        for settings in [
            vec![],
            vec![save.clone()],
            vec![save.clone(), stop.clone()],
            vec![link.clone()],
            vec![link.clone(), save.clone()],
            vec![link.clone(), stop.clone()],
        ] {
            let listening = sockets
                .listen(
                    protocol,
                    OWNER,
                    bind_any_port(ipv4_loopback()),
                    BACKLOG,
                    &[],
                )
                .expect("listen");
            let mut saving = TcpSavedSynchronize::Off;
            for setting in &settings {
                match sockets.configure(&listening.listener, setting.clone()) {
                    Ok(()) => {
                        if let TcpListenerSetting::SaveSynchronize(asked) = setting {
                            saving = *asked;
                        }
                    }
                    Err(error) => assert!(
                        protocol == TcpProtocol::Multipath,
                        "{protocol:?} {setting:?}: {error:?}"
                    ),
                }
            }
            let connected = sockets
                .connect(
                    protocol,
                    OWNER,
                    bind_any_port(ipv4_any()),
                    listening.local,
                    &[],
                    TcpFastOpen::Off,
                )
                .expect("connect");
            let accepted = accept_with_header(&sockets, &listening.listener, HEADER_ROOM);
            let header = &accepted.synchronize_header;
            if protocol == TcpProtocol::Tcp {
                let ip = Ipv4HeaderSlice::from_slice(header).expect("the SYN's IP header");
                assert_eq!(
                    (ip.source_addr(), ip.destination_addr()),
                    (Ipv4Addr::LOCALHOST, Ipv4Addr::LOCALHOST),
                    "{settings:?}"
                );
            }
            let mut small = [u8::MIN; CUT_HEADER];
            let mut room = [u8::MIN; HEADER_ROOM];
            if saving != TcpSavedSynchronize::Off {
                let length = match sockets.read_state(
                    &accepted.connection,
                    TcpQuery::TakeSavedSynchronize(&mut small),
                ) {
                    Ok(TcpReading::SavedSynchronize(TcpSavedSynchronizeHeader::Kept {
                        length,
                    })) => length,
                    other => panic!("{protocol:?} {settings:?}: {other:?}"),
                };
                let taken = match sockets.read_state(
                    &accepted.connection,
                    TcpQuery::TakeSavedSynchronize(&mut room),
                ) {
                    Ok(TcpReading::SavedSynchronize(TcpSavedSynchronizeHeader::Taken(taken))) => {
                        taken.to_vec()
                    }
                    other => panic!("{protocol:?} {settings:?}: {other:?}"),
                };
                assert_eq!(length, ByteCount(taken.len()), "{settings:?}");
                if saving == TcpSavedSynchronize::FromLinkHeader {
                    assert!(
                        taken.len() > header.len() && taken.ends_with(header),
                        "{settings:?}: {taken:?} does not end with {header:?}"
                    );
                } else {
                    assert_eq!(&taken, header, "{settings:?}");
                }
            }
            assert_eq!(
                sockets.read_state(
                    &accepted.connection,
                    TcpQuery::TakeSavedSynchronize(&mut room)
                ),
                Ok(TcpReading::SavedSynchronize(
                    TcpSavedSynchronizeHeader::Absent
                )),
                "{protocol:?} {settings:?}"
            );
            sockets.close(accepted.connection, TcpClose::Orderly);
            sockets.close(connected.connection, TcpClose::Orderly);
            sockets.stop_listening(listening.listener);
        }
    }
}

/// Counters an authentication check sets, each unlike the others.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const AUTHENTICATION_COUNTERS: TcpAuthenticationCounters = TcpAuthenticationCounters {
    verified: SegmentCount(1),
    failed: SegmentCount(2),
    key_not_found: SegmentCount(3),
    signature_missing: SegmentCount(4),
    icmp_errors_ignored: SegmentCount(5),
};

/// Checks that a connection made without authentication has no authentication counters to set, nor a status to read.
///
/// A listener's counters, set, are what its state reads.
pub fn authentication_counters_are_set_only_where_authentication_is<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let refused = sockets.set_authentication_counters(&pair.client, AUTHENTICATION_COUNTERS);
    assert!(refused.is_err(), "{refused:?}");
    let status = authentication_status(&sockets, &pair.client);
    assert!(status.is_err(), "{status:?}");
    sockets
        .set_listener_authentication_counters(&pair.listener, AUTHENTICATION_COUNTERS)
        .expect("set the listener's counters");
    assert_eq!(
        listener_authentication_status(&sockets, &pair.listener).map(|status| status.counters),
        Ok(AUTHENTICATION_COUNTERS)
    );
    close(&sockets, pair);
}

/// Checks that a listener's state reads it listening with the congestion control it was given.
pub fn listener_state_reads_a_listener<S: TcpSockets>(sockets: S) {
    let control = TcpSetting::CongestionControl(CONGESTION_CONTROL);
    let (listener, _) = listen(
        &sockets,
        ipv4_loopback(),
        &[TcpListenerSetting::Socket(control)],
    );
    match sockets.read_listener_state(&listener, TcpListenerQuery::Information) {
        Ok(TcpReading::Information(information)) => {
            assert_eq!(information.state, TcpState::Listen)
        }
        other => panic!("information read {other:?}"),
    }
    let mut name = [u8::MIN; HEADER_ROOM];
    match sockets.read_listener_state(&listener, TcpListenerQuery::CongestionControl(&mut name)) {
        Ok(TcpReading::CongestionControl { name, information }) => {
            assert_eq!((name, information), (CONGESTION_CONTROL, None))
        }
        other => panic!("congestion control read {other:?}"),
    }
    sockets.stop_listening(listener);
}

/// Checks that iPv4 options and IPv6 extension headers set before connect go out in the SYN, as its saved headers show.
pub fn header_options_go_out_in_the_synchronize<S: TcpSockets>(sockets: S) {
    let options = Ipv4Options::try_from(&IPV4_OPTIONS[..]).expect("IPv4 options");
    let synchronize_header = |address: BindAddress, setting: IpPacketSetting<'_>| {
        let (listener, local) = listen(&sockets, address, &[]);
        let connected = sockets
            .connect(
                TcpProtocol::Tcp,
                OWNER,
                bind_any_port(any_of(address)),
                local,
                &[TcpSetting::Packet(setting)],
                TcpFastOpen::Off,
            )
            .expect("connect");
        let accepted = accept_with_header(&sockets, &listener, HEADER_ROOM);
        sockets.close(accepted.connection, TcpClose::Orderly);
        sockets.close(connected.connection, TcpClose::Orderly);
        sockets.stop_listening(listener);
        accepted.synchronize_header
    };
    let header = synchronize_header(ipv4_loopback(), IpPacketSetting::Ipv4Options(&options));
    let ipv4 = Ipv4HeaderSlice::from_slice(&header).expect("the SYN's IPv4 header");
    assert_eq!(ipv4.options(), IPV4_OPTIONS);
    for (setting, number) in [
        (
            IpPacketSetting::HopByHopOptions(padding_header()),
            IpNumber::IPV6_HEADER_HOP_BY_HOP,
        ),
        (
            IpPacketSetting::DestinationOptions(padding_header()),
            IpNumber::IPV6_DESTINATION_OPTIONS,
        ),
    ] {
        let header = synchronize_header(ipv6_loopback(), setting);
        let ipv6 = Ipv6HeaderSlice::from_slice(&header).expect("the SYN's IPv6 header");
        assert_eq!(ipv6.next_header(), number);
        let extension = Ipv6RawExtHeaderSlice::from_slice(&header[Ipv6Header::LEN..])
            .expect("the extension header");
        assert_eq!(extension.next_header(), IpNumber::TCP);
        assert_eq!(extension.slice().len(), PADDING_HEADER.len());
        assert!(extension.payload().iter().all(|&byte| byte == 0));
    }
}

/// A header word of IPv4 options that do nothing.
const IPV4_OPTIONS: [u8; size_of::<u32>()] =
    [linux_raw_sys::net::IPOPT_NOP as u8; size_of::<u32>()];

/// The smallest IPv6 extension header, its options all one-byte padding (Pad1 is option type 0).
const PADDING_HEADER: [u8; Ipv6RawExtHeader::MIN_LEN] = [0; Ipv6RawExtHeader::MIN_LEN];

/// A link-local IPv4 multicast group.
///
/// https://www.rfc-editor.org/rfc/rfc5771
const MULTICAST_V4: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
/// A link-local IPv6 multicast group.
///
/// https://www.rfc-editor.org/rfc/rfc4291
const MULTICAST_V6: Ipv6Addr = Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0xfb);
/// A unique local IPv6 address.
///
/// https://www.rfc-editor.org/rfc/rfc4193
const ANYCAST_V6: Ipv6Addr = Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1);
/// A flow label of the setting checks.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const FLOW_LABEL: u32 = 0xABCDE;
/// A packet mark of the setting checks.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const MARK: PacketMark = PacketMark(7);

/// Gives the index of the host's loopback interface, the one that says it is loopback.
fn loopback_interface() -> InterfaceIndex {
    let loopback = nix::ifaddrs::getifaddrs()
        .expect("the host's interfaces")
        .find(|entry| entry.flags.contains(InterfaceFlags::IFF_LOOPBACK))
        .expect("a loopback interface");
    InterfaceIndex(
        nix::net::if_::if_nametoindex(loopback.interface_name.as_str())
            .expect("the loopback interface's index"),
    )
}

fn padding_header() -> Option<Ipv6RawExtHeaderSlice<'static>> {
    Some(Ipv6RawExtHeaderSlice::from_slice(&PADDING_HEADER).expect("an extension header"))
}

/// Gives the settings of an IPv4 connection checked for being set idempotently, each taking no privilege.
fn connection_settings(options: &Ipv4Options, interface: InterfaceIndex) -> Vec<TcpSetting<'_>> {
    vec![
        TcpSetting::Ip(IpSocketSetting::SendBuffer(ByteCount(SETTING_BYTES))),
        TcpSetting::Ip(IpSocketSetting::ReceiveBuffer(ByteCount(SETTING_BYTES))),
        TcpSetting::Ip(IpSocketSetting::MaximumPacingRate(Some(BytesPerSecond(
            SETTING_BYTES as u64,
        )))),
        TcpSetting::Ip(IpSocketSetting::MaximumPacingRate(None)),
        TcpSetting::Ip(IpSocketSetting::MulticastLoopback(true)),
        TcpSetting::Ip(IpSocketSetting::MulticastAllGroups {
            version: IpVersion::V4,
            enabled: false,
        }),
        TcpSetting::Ip(IpSocketSetting::Timestamping(Timestamping {
            receive_software: Report::Requested,
            receive_hardware: Report::NotRequested,
            bound_clock: None,
        })),
        TcpSetting::Packet(IpPacketSetting::HopLimit {
            version: IpVersion::V4,
            limit: Some(HopLimit(SETTING_COUNT)),
        }),
        TcpSetting::Packet(IpPacketSetting::HopLimit {
            version: IpVersion::V4,
            limit: None,
        }),
        TcpSetting::Packet(IpPacketSetting::TrafficClass {
            version: IpVersion::V4,
            dscp: IpDscp::AF41,
            ecn: IpEcn::ZERO,
        }),
        TcpSetting::Packet(IpPacketSetting::PathMtuDiscovery {
            version: IpVersion::V4,
            mode: PathMtuDiscovery::Do,
        }),
        TcpSetting::Packet(IpPacketSetting::Ipv4Options(options)),
        TcpSetting::Packet(IpPacketSetting::Priority(PacketPriority(u32::from(
            SETTING_COUNT,
        )))),
        TcpSetting::Packet(IpPacketSetting::UnicastInterface(Some(interface))),
        TcpSetting::Packet(IpPacketSetting::UnicastInterface(None)),
        TcpSetting::Packet(IpPacketSetting::RouteScope(RouteScope::Universe)),
        TcpSetting::Port(PortSetting::AddressSharing(AddressSharing::Shared)),
        TcpSetting::Port(PortSetting::PortSharing(PortSharing::Shared)),
        TcpSetting::Port(PortSetting::NonlocalAddresses(NonlocalAddresses::Bindable)),
        TcpSetting::Port(PortSetting::NonlocalAddresses(NonlocalAddresses::Refused)),
        TcpSetting::Port(PortSetting::BoundInterface(Some(interface))),
        TcpSetting::Port(PortSetting::BoundInterface(None)),
        TcpSetting::Port(PortSetting::IncomingProcessor(None)),
        TcpSetting::BufferLocks(BufferLocks {
            send: BufferTuning::Locked,
            receive: BufferTuning::Automatic,
        }),
        TcpSetting::ReceiveLowWater(SMALLEST_LOW_WATER),
        TcpSetting::Filter(Some(SocketFilter(&ACCEPT_ALL))),
        TcpSetting::Filter(None),
        TcpSetting::TransmitRehash(TransmitRehash::Enabled),
        TcpSetting::TransmitRehash(TransmitRehash::SystemDefault),
        TcpSetting::MinimumHopLimit {
            version: IpVersion::V4,
            minimum: Some(SMALLEST_HOP_LIMIT),
        },
        TcpSetting::MinimumHopLimit {
            version: IpVersion::V4,
            minimum: None,
        },
        TcpSetting::IcmpErrorReports {
            version: IpVersion::V4,
            reports: Report::Requested,
        },
        TcpSetting::KeepAlive(true),
        TcpSetting::KeepAliveIdle(SETTING_TIME),
        TcpSetting::KeepAliveInterval(SETTING_TIME),
        TcpSetting::KeepAliveProbes(ProbeCount(SETTING_COUNT)),
        TcpSetting::NoDelay(true),
        TcpSetting::Cork(false),
        TcpSetting::UrgentInline(true),
        TcpSetting::QuickAcknowledgement(TcpQuickAcknowledgement::On),
        TcpSetting::MaximumSegmentSize(Some(SEGMENT)),
        TcpSetting::WindowClamp(Some(WindowSize(SETTING_BYTES as u32))),
        TcpSetting::SynchronizeRetries(RetryCount(SETTING_COUNT)),
        TcpSetting::UserTimeout(Some(SETTING_TIME)),
        TcpSetting::UserTimeout(None),
        TcpSetting::DeferAccept(None),
        TcpSetting::FinWait2(TcpFinWait2::Lasts(SETTING_TIME)),
        TcpSetting::FinWait2(TcpFinWait2::Reset),
        TcpSetting::FinWait2(TcpFinWait2::SystemDefault),
        TcpSetting::RetransmissionTimeoutMinimum(SHORT_SETTING_TIME),
        TcpSetting::RetransmissionTimeoutMaximum(SETTING_TIME),
        TcpSetting::DelayedAcknowledgementMaximum(SHORT_SETTING_TIME),
        TcpSetting::TransmitDelay(SHORT_SETTING_TIME),
        TcpSetting::ThinLinearTimeouts(true),
        TcpSetting::CongestionControl(CONGESTION_CONTROL),
        TcpSetting::NotSentLowWater(Some(ByteCount(SETTING_BYTES))),
        TcpSetting::NotSentLowWater(None),
        TcpSetting::FastOpenWithoutCookie(true),
        TcpSetting::UnreadByteReports(Report::Requested),
        TcpSetting::RecordsLastReceivedHeader(Report::Requested),
        TcpSetting::RecordsLastReceivedHeader(Report::NotRequested),
    ]
}

/// Gives the settings of an IPv6 connection checked for being set idempotently: those only IPv6 has, and the IPv6 variants of those both versions have.
fn ipv6_connection_settings(interface: InterfaceIndex) -> Vec<TcpSetting<'static>> {
    vec![
        TcpSetting::Ip(IpSocketSetting::MulticastAllGroups {
            version: IpVersion::V6,
            enabled: false,
        }),
        TcpSetting::Packet(IpPacketSetting::HopLimit {
            version: IpVersion::V6,
            limit: Some(HopLimit(SETTING_COUNT)),
        }),
        TcpSetting::Packet(IpPacketSetting::HopLimit {
            version: IpVersion::V6,
            limit: None,
        }),
        TcpSetting::Packet(IpPacketSetting::TrafficClass {
            version: IpVersion::V6,
            dscp: IpDscp::AF41,
            ecn: IpEcn::ZERO,
        }),
        TcpSetting::Packet(IpPacketSetting::PathMtuDiscovery {
            version: IpVersion::V6,
            mode: PathMtuDiscovery::Do,
        }),
        TcpSetting::Packet(IpPacketSetting::HopByHopOptions(padding_header())),
        TcpSetting::Packet(IpPacketSetting::HopByHopOptions(None)),
        TcpSetting::Packet(IpPacketSetting::RoutingDestinationOptions(None)),
        TcpSetting::Packet(IpPacketSetting::RoutingHeader(None)),
        TcpSetting::Packet(IpPacketSetting::DestinationOptions(padding_header())),
        TcpSetting::Packet(IpPacketSetting::DestinationOptions(None)),
        TcpSetting::Packet(IpPacketSetting::FlowLabel(FlowLabelChoice::Unlabelled)),
        TcpSetting::Packet(IpPacketSetting::FlowLabel(FlowLabelChoice::Automatic)),
        TcpSetting::Packet(IpPacketSetting::UnicastInterface(Some(interface))),
        TcpSetting::Packet(IpPacketSetting::UnicastInterface(None)),
        TcpSetting::Packet(IpPacketSetting::SourcePreferences(SourcePreferences {
            temporary: TemporaryAddressPreference::Public,
            mobile: MobileAddressPreference::Home,
        })),
        TcpSetting::MinimumHopLimit {
            version: IpVersion::V6,
            minimum: Some(SMALLEST_HOP_LIMIT),
        },
        TcpSetting::MinimumHopLimit {
            version: IpVersion::V6,
            minimum: None,
        },
        TcpSetting::IcmpErrorReports {
            version: IpVersion::V6,
            reports: Report::Requested,
        },
        TcpSetting::RecordsLastReceivedHeader(Report::Requested),
        TcpSetting::RecordsLastReceivedHeader(Report::NotRequested),
    ]
}

/// Gives each setting given to connect once and twice, where each must be taken, then set once and twice on the established connections.
///
/// The outcomes agree (Linux takes some settings only before a connection starts) and the connections carry bytes.
fn settings_set_twice_equal_set_once<S: TcpSockets>(
    sockets: &S,
    address: BindAddress,
    settings: &[TcpSetting<'_>],
) {
    for setting in settings {
        let once = pair_at(sockets, address, std::slice::from_ref(setting), &[]);
        let twice = pair_at(sockets, address, &[setting.clone(), setting.clone()], &[]);
        assert_carries_bytes(sockets, &once);
        assert_carries_bytes(sockets, &twice);
        assert_eq!(
            observed(sockets, &once.client),
            observed(sockets, &twice.client),
            "{setting:?} given to connect"
        );
        let first = sockets.configure(&once.client, setting.clone());
        let outcomes = [
            first,
            sockets.configure(&twice.client, setting.clone()),
            sockets.configure(&twice.client, setting.clone()),
        ];
        assert!(
            outcomes.iter().all(|outcome| *outcome == first),
            "{setting:?}: {outcomes:?}"
        );
        assert_eq!(
            observed(sockets, &once.client),
            observed(sockets, &twice.client),
            "{setting:?} set on the connection"
        );
        assert_carries_bytes(sockets, &once);
        assert_carries_bytes(sockets, &twice);
        close(sockets, once);
        close(sockets, twice);
    }
}

pub fn connection_settings_set_twice_equal_set_once<S: TcpSockets>(sockets: S) {
    let options = Ipv4Options::try_from(&IPV4_OPTIONS[..]).expect("IPv4 options");
    let settings = connection_settings(&options, loopback_interface());
    settings_set_twice_equal_set_once(&sockets, ipv4_loopback(), &settings);
}

pub fn ipv6_connection_settings_set_twice_equal_set_once<S: TcpSockets>(sockets: S) {
    let settings = ipv6_connection_settings(loopback_interface());
    settings_set_twice_equal_set_once(&sockets, ipv6_loopback(), &settings);
}

/// Checks that each listener setting set once and twice on fresh listeners.
///
/// The outcomes agree, every setting an implementation must have succeeds, and the listeners still accept connections that carry bytes.
pub fn listener_settings_set_twice_equal_set_once<S: TcpSockets>(sockets: S) {
    let key = FastOpenKey([u8::MAX; FAST_OPEN_KEY_LENGTH]);
    // Authentication for a peer other than the clients, so that the listener still takes them unsigned.
    let distant = authentication_identity_of(NONLOCAL, FIRST_KEY);
    let settings = [
        (
            TcpListenerSetting::Socket(TcpSetting::AddAuthenticationKey(
                TcpAuthenticationKeyAddition {
                    identity: distant,
                    algorithm: AUTHENTICATION_ALGORITHM,
                    mac_length: AUTHENTICATION_MAC_LENGTH,
                    other_options: TcpAuthenticationOptionCoverage::Included,
                    secret: AUTHENTICATION_SECRET,
                    key_use: TcpAuthenticationKeyUse::Stored,
                },
            )),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::Socket(TcpSetting::RemoveAuthenticationKey(
                TcpAuthenticationKeyRemoval {
                    identity: distant,
                    current: None,
                    requested_next: None,
                    timing: TcpAuthenticationRemovalTiming::WhenUnused,
                },
            )),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::Socket(TcpSetting::AuthenticationOptions(
                TcpAuthenticationOptions {
                    current: None,
                    requested_next: None,
                    requirement: TcpAuthenticationRequirement::Optional,
                    icmp_errors: TcpAuthenticationIcmpErrors::Accepted,
                },
            )),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::Socket(TcpSetting::InitialAuthenticationCounters(
                AUTHENTICATION_COUNTERS,
            )),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::Backlog(ListenBacklog(u32::from(SETTING_COUNT))),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::FastOpenQueue(FAST_OPEN_QUEUE),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::FastOpenQueue(None),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::FastOpenKeys(FastOpenKeys {
                primary: key,
                backup: Some(key),
            }),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::SaveSynchronize(TcpSavedSynchronize::FromNetworkHeader),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::SaveSynchronize(TcpSavedSynchronize::Off),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::SaveSynchronize(TcpSavedSynchronize::FromLinkHeader),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::Socket(TcpSetting::RecordsLastReceivedHeader(Report::Requested)),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::Socket(TcpSetting::RecordsLastReceivedHeader(Report::Requested)),
            true,
            ipv6_loopback(),
        ),
        (
            TcpListenerSetting::Socket(TcpSetting::RecordsLastReceivedHeader(Report::NotRequested)),
            true,
            ipv6_loopback(),
        ),
        (
            TcpListenerSetting::FlowLabelReflection(true),
            true,
            ipv6_loopback(),
        ),
        (
            TcpListenerSetting::FlowLabelReflection(false),
            true,
            ipv6_loopback(),
        ),
        (
            TcpListenerSetting::Socket(TcpSetting::DeferAccept(None)),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::Socket(TcpSetting::NoDelay(true)),
            true,
            ipv4_loopback(),
        ),
        (
            TcpListenerSetting::Socket(TcpSetting::Ip(IpSocketSetting::Timestamping(
                Timestamping {
                    receive_software: Report::Requested,
                    receive_hardware: Report::NotRequested,
                    bound_clock: None,
                },
            ))),
            true,
            ipv4_loopback(),
        ),
    ];
    for (setting, required, address) in settings {
        let once = pair_at(&sockets, address, &[], &[]);
        let first = sockets.configure(&once.listener, setting.clone());
        let twice = pair_at(&sockets, address, &[], &[]);
        let outcomes = [
            first,
            sockets.configure(&twice.listener, setting.clone()),
            sockets.configure(&twice.listener, setting.clone()),
        ];
        assert!(
            outcomes.iter().all(|outcome| *outcome == first),
            "{setting:?}: {outcomes:?}"
        );
        if required {
            assert_eq!(first, Ok(()), "{setting:?}");
        }
        for pair in [once, twice] {
            let client = sockets
                .connect(
                    TcpProtocol::Tcp,
                    OWNER,
                    bind_any_port(any_of(address)),
                    pair.local,
                    &[],
                    TcpFastOpen::Off,
                )
                .expect("connect after the setting")
                .connection;
            let server = accept(&sockets, &pair.listener);
            send_all(&sockets, &client, plain(), MESSAGE);
            assert_eq!(receive_up_to(&sockets, &server, MESSAGE.len()), MESSAGE);
            sockets.close(client, TcpClose::Orderly);
            sockets.close(server, TcpClose::Orderly);
            close(&sockets, pair);
        }
    }
}

/// Checks that settings an implementation may refuse, set once and twice on established connections, agree.
///
/// Refused or taken, and the connections still carry bytes, but for routing destination options without a routing
/// header, which Linux counts in each packet's length and never sends.
pub fn refusable_connection_settings_set_twice_equal_set_once<S: TcpSockets>(sockets: S) {
    let interface = loopback_interface();
    let ipv4 = [
        TcpSetting::Packet(IpPacketSetting::Mark(MARK)),
        // Linux adds the first authentication key only before a connection starts.
        authentication_key(
            FIRST_KEY,
            AUTHENTICATION_SECRET,
            TcpAuthenticationOptionCoverage::Included,
            TcpAuthenticationKeyUse::Stored,
        ),
        TcpSetting::InitialAuthenticationCounters(AUTHENTICATION_COUNTERS),
        TcpSetting::Port(PortSetting::GroupFilter(Some(SocketFilter(&ACCEPT_ALL)))),
        TcpSetting::Port(PortSetting::GroupFilter(None)),
        TcpSetting::Ip(IpSocketSetting::MulticastV4(MulticastGroup {
            group: MULTICAST_V4,
            interface: Some(interface),
            membership: MulticastMembership::Joined(SourceFilter::Exclude(&[])),
        })),
        TcpSetting::Ip(IpSocketSetting::MulticastV4(MulticastGroup {
            group: MULTICAST_V4,
            interface: Some(interface),
            membership: MulticastMembership::Left,
        })),
    ];
    let ipv6 = [
        TcpSetting::Packet(IpPacketSetting::FlowLabel(FlowLabelChoice::Label(
            Ipv6FlowLabel::try_new(FLOW_LABEL).expect("a flow label"),
        ))),
        TcpSetting::Ip(IpSocketSetting::MulticastV6(MulticastGroup {
            group: MULTICAST_V6,
            interface: Some(interface),
            membership: MulticastMembership::Joined(SourceFilter::Exclude(&[])),
        })),
        TcpSetting::Ip(IpSocketSetting::Anycast(AnycastAddress {
            address: ANYCAST_V6,
            interface: Some(interface),
            membership: Membership::Joined,
        })),
        TcpSetting::Ip(IpSocketSetting::Anycast(AnycastAddress {
            address: ANYCAST_V6,
            interface: Some(interface),
            membership: Membership::Left,
        })),
    ];
    let unsendable = [TcpSetting::Packet(
        IpPacketSetting::RoutingDestinationOptions(padding_header()),
    )];
    for (address, settings, carries) in [
        (ipv4_loopback(), &ipv4[..], true),
        (ipv6_loopback(), &ipv6[..], true),
        (ipv6_loopback(), &unsendable[..], false),
    ] {
        for setting in settings {
            let once = pair_at(&sockets, address, &[], &[]);
            let twice = pair_at(&sockets, address, &[], &[]);
            let first = sockets.configure(&once.client, setting.clone());
            let outcomes = [
                first,
                sockets.configure(&twice.client, setting.clone()),
                sockets.configure(&twice.client, setting.clone()),
            ];
            assert!(
                outcomes.iter().all(|outcome| *outcome == first),
                "{setting:?}: {outcomes:?}"
            );
            assert_eq!(
                observed(&sockets, &once.client),
                observed(&sockets, &twice.client),
                "{setting:?}"
            );
            if carries {
                assert_carries_bytes(&sockets, &once);
                assert_carries_bytes(&sockets, &twice);
            }
            close(&sockets, once);
            close(&sockets, twice);
        }
    }
}

/// Reads the connection's `TCP_INFO`, failing the check on any other answer.
fn information<S: TcpSockets>(sockets: &S, connection: &S::Connection) -> TcpInformation {
    match sockets.read_state(connection, TcpQuery::Information) {
        Ok(TcpReading::Information(information)) => information,
        other => panic!("information read {other:?}"),
    }
}

fn queues<S: TcpSockets>(sockets: &S, connection: &S::Connection) -> TcpQueues {
    match sockets.read_state(connection, TcpQuery::Queues) {
        Ok(TcpReading::Queues(queues)) => queues,
        other => panic!("queues read {other:?}"),
    }
}

fn tuned<S: TcpSockets>(sockets: &S, connection: &S::Connection) -> TcpTuned {
    match sockets.read_state(connection, TcpQuery::Tuned) {
        Ok(TcpReading::Tuned(tuned)) => tuned,
        other => panic!("tuned values read {other:?}"),
    }
}

fn timestamp<S: TcpSockets>(sockets: &S, connection: &S::Connection) -> TcpTimestampClock {
    match sockets.read_state(connection, TcpQuery::Timestamp) {
        Ok(TcpReading::Timestamp(clock)) => clock,
        other => panic!("timestamp read {other:?}"),
    }
}

/// Reads the connection's repair state, the refusal included.
fn repair_state<S: TcpSockets>(
    sockets: &S,
    connection: &S::Connection,
) -> Result<TcpRepairState, HostError> {
    sockets
        .read_state(connection, TcpQuery::Repair)
        .map(|reading| match reading {
            TcpReading::Repair(state) => state,
            other => panic!("repair state read {other:?}"),
        })
}

fn authentication_status<S: TcpSockets>(
    sockets: &S,
    connection: &S::Connection,
) -> Result<TcpAuthenticationStatus, HostError> {
    sockets
        .read_state(connection, TcpQuery::Authentication)
        .map(|reading| match reading {
            TcpReading::Authentication(status) => status,
            other => panic!("authentication read {other:?}"),
        })
}

fn listener_authentication_status<S: TcpSockets>(
    sockets: &S,
    listener: &S::Listener,
) -> Result<TcpAuthenticationStatus, HostError> {
    sockets
        .read_listener_state(listener, TcpListenerQuery::Authentication)
        .map(|reading| match reading {
            TcpReading::Authentication(status) => status,
            other => panic!("authentication read {other:?}"),
        })
}

/// Reads what the connection recorded of the last packet it received: the fields and the header bytes.
fn last_received_header<S: TcpSockets>(
    sockets: &S,
    connection: &S::Connection,
) -> (ReceivedPacketFields, Vec<u8>) {
    let mut room = [u8::MIN; HEADER_ROOM];
    match sockets.read_state(connection, TcpQuery::LastReceivedHeader(&mut room)) {
        Ok(TcpReading::LastReceivedHeader { fields, header }) => (fields, header.to_vec()),
        other => panic!("last received header read {other:?}"),
    }
}

fn multipath<S: TcpSockets>(sockets: &S, connection: &S::Connection) -> MultipathInformation {
    match sockets.read_state(connection, TcpQuery::Multipath) {
        Ok(TcpReading::Multipath(information)) => information,
        other => panic!("multipath read {other:?}"),
    }
}

/// What the readings show of a connection's settings, compared between settings given once and twice.
///
/// Without what traffic alone changes: the authentication's verified count and the acknowledging mode.
#[derive(Debug, PartialEq)]
struct Observed {
    tuned: TcpTuned,
    authentication: Result<TcpAuthenticationStatus, HostError>,
    last_received: (ReceivedPacketFields, Vec<u8>),
}

fn observed<S: TcpSockets>(sockets: &S, connection: &S::Connection) -> Observed {
    let mut tuned = tuned(sockets, connection);
    tuned.quick_acknowledgement = false;
    let mut authentication = authentication_status(sockets, connection);
    if let Ok(status) = authentication.as_mut() {
        status.counters.verified = SegmentCount(u64::MIN);
    }
    Observed {
        tuned,
        authentication,
        last_received: last_received_header(sockets, connection),
    }
}

/// What part of the first round trip measured RFC 6298's first variance estimate is.
///
/// https://www.rfc-editor.org/rfc/rfc6298#section-2
const ROUND_TRIP_VARIANCE_SHARE: u128 = 2;

/// Checks that a connection's information reads its state and what it carried, each end agreeing with the other.
///
/// The window scales and options each end negotiated, the bytes sent, acknowledged and received, and the states a
/// shutdown leads to; a listener reads as listening.
pub fn information_reads_what_the_connection_did<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let (client, server) = (
        information(&sockets, &pair.client),
        information(&sockets, &pair.server),
    );
    assert_eq!(
        (client.state, server.state),
        (TcpState::Established, TcpState::Established)
    );
    // The client has measured one round trip, its SYN's, so RFC 6298's first estimate holds: the measurement and its half.
    assert_eq!(client.round_trip_time, client.minimum_round_trip_time);
    assert_eq!(
        client.round_trip_time_variance.as_micros(),
        client.round_trip_time.as_micros() / ROUND_TRIP_VARIANCE_SHARE,
        "{client:?}"
    );
    assert_eq!(client.window_scales.send, server.window_scales.receive);
    assert_eq!(client.window_scales.receive, server.window_scales.send);
    assert_eq!(client.options.timestamps, server.options.timestamps);
    assert_eq!(
        client.options.selective_acknowledgements,
        server.options.selective_acknowledgements
    );
    let bytes = pattern(TRANSFER);
    assert_eq!(
        transfer(&sockets, &pair.client, &pair.server, &bytes),
        bytes
    );
    std::thread::sleep(SETTLE);
    let (sent, received) = (
        information(&sockets, &pair.client),
        information(&sockets, &pair.server),
    );
    let transferred = TRANSFER as u64;
    // RFC 4898's count of the bytes acknowledged may count the SYN's sequence number.
    assert!(
        (transferred..=transferred + u64::from(true)).contains(&sent.bytes_acknowledged.0),
        "{sent:?}"
    );
    assert_eq!(received.bytes_received, ByteTotal(transferred));
    assert!(sent.bytes_sent.0 >= transferred, "{sent:?}");
    assert_eq!(sent.not_sent, NO_BYTES);
    assert_eq!(sent.unacknowledged, SegmentCount(u64::MIN));
    let segment = u64::from(sent.send_maximum_segment_size.0);
    assert!(
        sent.data_segments_out.0 >= transferred.div_ceil(segment),
        "{sent:?}"
    );
    assert!(
        (u64::from(true)..=sent.data_segments_out.0).contains(&received.data_segments_in.0),
        "{received:?}"
    );
    assert!(sent.segments_out.0 >= sent.data_segments_out.0);
    assert!(received.since_last_data_received < DEADLINE);
    sockets
        .shut_down(&pair.server, TcpShutdown::Write)
        .expect("shut down");
    for (end, state) in [
        (&pair.client, TcpState::CloseWait),
        (&pair.server, TcpState::FinWait2),
    ] {
        let start = Instant::now();
        while information(&sockets, end).state != state {
            assert!(start.elapsed() < DEADLINE, "never reached {state:?}");
            std::thread::sleep(RETRY_INTERVAL);
        }
    }
    close(&sockets, pair);
}

/// The congestion control whose information has Vegas's form.
const VEGAS: &str = "vegas";
/// The congestion control whose information has BBR's form.
const BOTTLENECK_BANDWIDTH: &str = "bbr";

/// Checks that the congestion control state names the algorithm and gives the information of those that keep one.
///
/// Reno keeps none; DCTCP, Vegas and BBR give their own forms after a transfer, each describing a working connection.
pub fn congestion_control_state_names_the_algorithm<S: TcpSockets>(sockets: S) {
    for algorithm in [
        CONGESTION_CONTROL,
        NOTIFYING_CONGESTION_CONTROL,
        VEGAS,
        BOTTLENECK_BANDWIDTH,
    ] {
        // Both ends, since DCTCP falls back to Reno without the ECN a peer must agree to.
        let control = TcpSetting::CongestionControl(algorithm);
        let pair = pair_with(
            &sockets,
            std::slice::from_ref(&control),
            &[TcpListenerSetting::Socket(control.clone())],
        );
        let bytes = pattern(TRANSFER);
        assert_eq!(
            transfer(&sockets, &pair.client, &pair.server, &bytes),
            bytes
        );
        let mut room = [u8::MIN; HEADER_ROOM];
        let information =
            match sockets.read_state(&pair.client, TcpQuery::CongestionControl(&mut room)) {
                Ok(TcpReading::CongestionControl { name, information }) => {
                    assert_eq!(name, algorithm);
                    information
                }
                other => panic!("{algorithm}: {other:?}"),
            };
        match (algorithm, information) {
            (CONGESTION_CONTROL, None) => {}
            (
                NOTIFYING_CONGESTION_CONTROL,
                Some(CongestionControlInformation::DataCenter { enabled, alpha, .. }),
            ) => {
                assert!(enabled, "{information:?}");
                assert!(alpha.0 <= DATA_CENTER_FRACTION_WHOLE, "{information:?}");
            }
            (
                VEGAS,
                Some(CongestionControlInformation::Vegas {
                    enabled,
                    round_trip_time,
                    ..
                }),
            ) => {
                // A round trip over loopback was measured.
                assert!(enabled, "{information:?}");
                assert!(
                    round_trip_time > Duration::ZERO && round_trip_time < SETTLE,
                    "{information:?}"
                );
            }
            (
                BOTTLENECK_BANDWIDTH,
                Some(CongestionControlInformation::BottleneckBandwidth {
                    bandwidth,
                    pacing_gain,
                    congestion_window_gain,
                    ..
                }),
            ) => {
                assert!(bandwidth.0 > u64::MIN, "{information:?}");
                // No delivery rate is measured over less than the smallest round trip, so none is above every
                // segment sent delivered in one.
                let carried = self::information(&sockets, &pair.client);
                let most = (u128::from(carried.segments_out.0)
                    * u128::from(carried.send_maximum_segment_size.0)
                    * Duration::from_secs(u64::from(true)).as_nanos())
                .checked_div(carried.minimum_round_trip_time.as_nanos())
                .expect("a smallest round trip measured");
                assert!(
                    u128::from(bandwidth.0) <= most,
                    "{information:?} above {most} bytes a second"
                );
                assert!(pacing_gain.0 > u32::MIN, "{information:?}");
                assert!(congestion_window_gain.0 > u32::MIN, "{information:?}");
            }
            other => panic!("{algorithm}: {other:?}"),
        }
        close(&sockets, pair);
    }
}

/// Checks that the queues read the bytes unsent under a cork, unacknowledged by a peer that drops them, and unread by the peer.
///
/// The urgent mark is reached once the bytes before it are read.
pub fn queues_count_unread_unacknowledged_and_unsent<S: TcpSockets>(sockets: S) {
    let pair = pair_with(&sockets, &[TcpSetting::Cork(true)], &[]);
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    std::thread::sleep(SETTLE);
    assert_eq!(
        queues(&sockets, &pair.client),
        TcpQueues {
            unread: NO_BYTES,
            unacknowledged: NO_BYTES,
            unsent: ByteCount(MESSAGE.len()),
            urgent_mark: UrgentMark::NotReached,
        }
    );
    sockets
        .configure(&pair.client, TcpSetting::Cork(false))
        .expect("uncork");
    wait_for_unread(&sockets, &pair.server, MESSAGE.len());
    std::thread::sleep(SETTLE);
    assert_eq!(
        queues(&sockets, &pair.server).unread,
        ByteCount(MESSAGE.len())
    );
    let sender = queues(&sockets, &pair.client);
    assert_eq!((sender.unacknowledged, sender.unsent), (NO_BYTES, NO_BYTES));
    drop_arrivals(&sockets, &pair.server, IpVersion::V4);
    send_all(&sockets, &pair.client, plain(), REPLY);
    std::thread::sleep(SETTLE);
    let sender = queues(&sockets, &pair.client);
    assert_eq!(
        (sender.unacknowledged, sender.unsent),
        (ByteCount(REPLY.len()), NO_BYTES)
    );
    sockets.close(pair.client, TcpClose::Abort);
    sockets.close(pair.server, TcpClose::Abort);
    sockets.stop_listening(pair.listener);
    let pair = self::pair(&sockets);
    send_all(&sockets, &pair.client, plain(), MESSAGE);
    send_all(&sockets, &pair.client, urgent(), URGENT);
    wait_for_unread(&sockets, &pair.server, MESSAGE.len());
    std::thread::sleep(SETTLE);
    assert_eq!(
        queues(&sockets, &pair.server).urgent_mark,
        UrgentMark::NotReached
    );
    assert_eq!(
        receive_up_to(&sockets, &pair.server, MESSAGE.len()),
        MESSAGE
    );
    assert_eq!(
        queues(&sockets, &pair.server).urgent_mark,
        UrgentMark::Reached
    );
    close(&sockets, pair);
}

/// The smallest link MTU IPv6 allows, below any path MTU loopback can have.
///
/// https://www.rfc-editor.org/rfc/rfc8200#section-5
const IPV6_MINIMUM_MTU: u32 = 1280;

/// How many times the send buffer's size a tuned check asks of the receive buffer, more than Linux's doubling.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const LARGER_BUFFER: usize = 4;

/// Checks that the tuned values follow the settings given: buffers at least as asked, the low water and clamp as asked.
///
/// The segment size within the one given before connect, quick acknowledgement as last set, a path MTU loopback
/// carries; a listener reads its own.
pub fn tuned_values_follow_the_settings<S: TcpSockets>(sockets: S) {
    let pair = pair_with(
        &sockets,
        &[TcpSetting::MaximumSegmentSize(Some(SEGMENT))],
        &[TcpListenerSetting::Socket(TcpSetting::Ip(
            IpSocketSetting::SendBuffer(ByteCount(SETTING_BYTES)),
        ))],
    );
    let clamp = WindowSize(SETTING_BYTES as u32);
    // Larger than the send buffer doubled, so that a receive buffer read from the send buffer is seen.
    let receive_buffer = SETTING_BYTES * LARGER_BUFFER;
    for setting in [
        TcpSetting::Ip(IpSocketSetting::SendBuffer(ByteCount(SETTING_BYTES))),
        TcpSetting::Ip(IpSocketSetting::ReceiveBuffer(ByteCount(receive_buffer))),
        TcpSetting::ReceiveLowWater(ByteCount(READ_PART)),
        TcpSetting::WindowClamp(Some(clamp)),
        TcpSetting::QuickAcknowledgement(TcpQuickAcknowledgement::On),
    ] {
        sockets
            .configure(&pair.client, setting.clone())
            .unwrap_or_else(|error| panic!("{setting:?}: {error:?}"));
    }
    let read = tuned(&sockets, &pair.client);
    assert!(read.send_buffer.0 >= SETTING_BYTES, "{read:?}");
    assert!(read.receive_buffer.0 >= receive_buffer, "{read:?}");
    assert_eq!(read.receive_low_water, ByteCount(READ_PART));
    assert_eq!(read.window_clamp, clamp);
    assert!(read.quick_acknowledgement, "{read:?}");
    assert!(read.maximum_segment_size.0 <= SEGMENT.0, "{read:?}");
    assert!(read.path_mtu.0 >= IPV6_MINIMUM_MTU, "{read:?}");
    assert_eq!(read.path_mtu, information(&sockets, &pair.client).path_mtu);
    sockets
        .configure(
            &pair.client,
            TcpSetting::QuickAcknowledgement(TcpQuickAcknowledgement::Off),
        )
        .expect("quick acknowledgement off");
    assert!(!tuned(&sockets, &pair.client).quick_acknowledgement);
    match sockets.read_listener_state(&pair.listener, TcpListenerQuery::Tuned) {
        Ok(TcpReading::Tuned(listener)) => {
            assert!(listener.send_buffer.0 >= SETTING_BYTES, "{listener:?}")
        }
        other => panic!("the listener's tuned values read {other:?}"),
    }
    close(&sockets, pair);
}

/// Checks that the timestamp clock reads in the unit of its route and advances as time does, between two readings.
///
/// A clock set in repair mode reads from the value set on; a listener's clock counts milliseconds.
pub fn timestamp_reads_the_clock_in_its_unit<S: TcpSockets>(sockets: S) {
    for (address, unit) in [
        (ipv4_loopback(), TcpTimestampUnit::Milliseconds),
        (
            BindAddress::V4(MICROSECOND_CLOCK),
            TcpTimestampUnit::Microseconds,
        ),
    ] {
        let pair = pair_at(&sockets, address, &[], &[]);
        let first = timestamp(&sockets, &pair.client);
        let start = Instant::now();
        std::thread::sleep(CLOCK_RUNS);
        let second = timestamp(&sockets, &pair.client);
        let ran = start.elapsed();
        assert_eq!((first.unit, second.unit), (unit, unit));
        assert_counts_in(unit, (second.value.0.wrapping_sub(first.value.0), ran));
        sockets
            .configure(&pair.client, TcpSetting::Repair(TcpRepair::On))
            .expect("repair mode");
        let set = Instant::now();
        sockets
            .set_timestamp_clock(
                &pair.client,
                TcpTimestampClock {
                    value: TcpTimestamp(CLOCK_READING),
                    unit,
                },
            )
            .expect("set the clock");
        std::thread::sleep(CLOCK_RUNS);
        let after = timestamp(&sockets, &pair.client);
        assert_counts_in(
            unit,
            (after.value.0.wrapping_sub(CLOCK_READING), set.elapsed()),
        );
        sockets
            .configure(&pair.client, TcpSetting::Repair(TcpRepair::Off))
            .expect("out of repair mode");
        close(&sockets, pair);
    }
    let (listener, _) = listen(&sockets, ipv4_loopback(), &[]);
    let read = |listener| match sockets.read_listener_state(listener, TcpListenerQuery::Timestamp) {
        Ok(TcpReading::Timestamp(clock)) => clock,
        other => panic!("the listener's timestamp read {other:?}"),
    };
    let first = read(&listener);
    let start = Instant::now();
    std::thread::sleep(CLOCK_RUNS);
    let second = read(&listener);
    assert_eq!(first.unit, TcpTimestampUnit::Milliseconds);
    assert_counts_in(
        TcpTimestampUnit::Milliseconds,
        (second.value.0.wrapping_sub(first.value.0), start.elapsed()),
    );
    sockets.stop_listening(listener);
}

/// Checks that a connection read in repair mode and written back into a new socket goes on where the first left off.
///
/// The repair state reads each end's sequence numbers as the other's, and is refused out of repair mode; the new socket
/// takes the sequence numbers, options, window and clock, reads them back, and then carries bytes both ways with the
/// first one's peer and its segment size.
pub fn repaired_connection_continues_where_the_original_left_off<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    // A small receive buffer on the client's side, so that its window and the server's differ; a small window on the
    // server's side would bound the segment size the repaired client sends.
    let connected = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_loopback()),
            local,
            &[TcpSetting::Ip(IpSocketSetting::ReceiveBuffer(ByteCount(
                SMALL_RECEIVE_BUFFER,
            )))],
            TcpFastOpen::Off,
        )
        .expect("connect");
    let server = accept(&sockets, &listener);
    let pair = Pair {
        listener,
        local,
        client: connected.connection,
        server,
    };
    assert_carries_bytes(&sockets, &pair);
    std::thread::sleep(SETTLE);
    assert!(repair_state(&sockets, &pair.client).is_err());
    for end in [&pair.client, &pair.server] {
        sockets
            .configure(end, TcpSetting::Repair(TcpRepair::On))
            .expect("repair mode");
    }
    let original = repair_state(&sockets, &pair.client).expect("the client's repair state");
    let peer = repair_state(&sockets, &pair.server).expect("the server's repair state");
    assert_eq!(original.send_sequence, peer.receive_sequence);
    assert_eq!(original.receive_sequence, peer.send_sequence);
    // Each end's send window is the window the other offered, the client's the smaller.
    assert_eq!(
        (original.window.send_window, original.window.receive_window),
        (peer.window.receive_window, peer.window.send_window)
    );
    assert!(
        original.window.receive_window.0 < original.window.send_window.0,
        "{original:?}"
    );
    assert_eq!(original.authentication, None);
    sockets
        .configure(&pair.server, TcpSetting::Repair(TcpRepair::Off))
        .expect("the server out of repair mode");
    let options = information(&sockets, &pair.client);
    let segment = tuned(&sockets, &pair.client).maximum_segment_size;
    let clock = timestamp(&sockets, &pair.client);
    let read_at = Instant::now();
    sockets.close(pair.client, TcpClose::Orderly);
    let mut repair_options = vec![
        TcpRepairOption::MaximumSegmentSize(segment),
        TcpRepairOption::WindowScales(options.window_scales),
    ];
    if options.options.selective_acknowledgements {
        repair_options.push(TcpRepairOption::SelectiveAcknowledgements);
    }
    if options.options.timestamps {
        repair_options.push(TcpRepairOption::Timestamps);
    }
    let restored = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            LocalBinding {
                address: ipv4_loopback(),
                port: LocalPort::Requested(Port(connected.local.port())),
            },
            local,
            &[
                TcpSetting::Port(PortSetting::AddressSharing(AddressSharing::Shared)),
                TcpSetting::Repair(TcpRepair::On),
                TcpSetting::RepairQueue(TcpRepairQueue::Send),
                TcpSetting::QueueSequence(original.send_sequence),
                TcpSetting::RepairQueue(TcpRepairQueue::Receive),
                TcpSetting::QueueSequence(original.receive_sequence),
                TcpSetting::RepairQueue(TcpRepairQueue::NoQueue),
            ],
            TcpFastOpen::Off,
        )
        .expect("connect in repair mode");
    assert_eq!(restored.start, TcpConnectStart::EstablishedForRepair);
    let restored = restored.connection;
    for setting in [
        TcpSetting::RepairOptions(&repair_options),
        TcpSetting::RepairWindow(original.window),
    ] {
        sockets
            .configure(&restored, setting.clone())
            .unwrap_or_else(|error| panic!("{setting:?}: {error:?}"));
    }
    // The peer drops segments stamped earlier than the last it saw, so the clock goes on from the reading.
    let elapsed = read_at.elapsed().as_millis() as u32;
    sockets
        .set_timestamp_clock(
            &restored,
            TcpTimestampClock {
                value: TcpTimestamp(clock.value.0.wrapping_add(elapsed)),
                unit: clock.unit,
            },
        )
        .expect("set the clock");
    assert_eq!(
        repair_state(&sockets, &restored),
        Ok(TcpRepairState {
            authentication: None,
            ..original
        })
    );
    let restored_options = information(&sockets, &restored);
    assert_eq!(restored_options.window_scales, options.window_scales);
    assert_eq!(
        restored_options.options.timestamps,
        options.options.timestamps
    );
    assert_eq!(
        restored_options.options.selective_acknowledgements,
        options.options.selective_acknowledgements
    );
    sockets
        .configure(&restored, TcpSetting::Repair(TcpRepair::Off))
        .expect("out of repair mode");
    let restored_pair = Pair {
        listener: pair.listener,
        local,
        client: restored,
        server: pair.server,
    };
    assert_carries_bytes(&sockets, &restored_pair);
    let bytes = pattern(TRANSFER);
    assert_eq!(
        transfer(
            &sockets,
            &restored_pair.client,
            &restored_pair.server,
            &bytes
        ),
        bytes
    );
    // Above the default a socket without the option keeps, within the one given.
    let restored_segment = information(&sockets, &restored_pair.client).send_maximum_segment_size;
    assert!(
        (DEFAULT_SEND_SEGMENT + 1..=segment.0).contains(&restored_segment.0),
        "{restored_segment:?} of at most {segment:?}"
    );
    close(&sockets, restored_pair);
}

/// A receive buffer well below the window a connection offers by default, for a check whose two ends' windows differ.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const SMALL_RECEIVE_BUFFER: usize = 1 << 13;

/// The send segment size a TCP connection assumes when no maximum segment size option came.
///
/// https://www.rfc-editor.org/rfc/rfc9293#section-3.7.1
const DEFAULT_SEND_SEGMENT: u16 = 536;

/// Counters an authentication check sets after the first ones, each unlike every other.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const LATER_AUTHENTICATION_COUNTERS: TcpAuthenticationCounters = TcpAuthenticationCounters {
    verified: SegmentCount(11),
    failed: SegmentCount(12),
    key_not_found: SegmentCount(13),
    signature_missing: SegmentCount(14),
    icmp_errors_ignored: SegmentCount(15),
};

/// Gives an authenticated pair whose ends hold the first key, current and requested next, and the initial counters.
///
/// With the capture of its segments.
fn authenticated_pair<S: TcpSockets>(sockets: &S) -> (Pair<S>, Capture) {
    let key = authentication_key(
        FIRST_KEY,
        AUTHENTICATION_SECRET,
        TcpAuthenticationOptionCoverage::Included,
        TcpAuthenticationKeyUse::CurrentAndRequestedNext,
    );
    let initial = TcpSetting::InitialAuthenticationCounters(AUTHENTICATION_COUNTERS);
    let (pair, synchronize, capture) =
        authentication_round(sockets, &[key.clone(), initial.clone()], &[key, initial]);
    assert_eq!(synchronize, signed_with(FIRST_KEY));
    (
        pair.expect("the keyed listener takes the keyed client"),
        capture,
    )
}

/// Checks that the authentication state reads the keys in use and counters that go on from those given before connect.
///
/// A key held reads its counters and one not held none; counters set are what is read next, for a connection and its
/// listener.
pub fn authentication_state_reads_the_keys_and_counters<S: TcpSockets>(sockets: S) {
    let (pair, _capture) = authenticated_pair(&sockets);
    std::thread::sleep(SETTLE);
    let status = authentication_status(&sockets, &pair.client).expect("the client's status");
    assert_eq!(
        (
            status.current,
            status.requested_next,
            status.requirement,
            status.icmp_errors
        ),
        (
            Some(FIRST_KEY),
            Some(FIRST_KEY),
            TcpAuthenticationRequirement::Optional,
            TcpAuthenticationIcmpErrors::Ignored
        )
    );
    let counters = status.counters;
    assert!(
        counters.verified.0 > AUTHENTICATION_COUNTERS.verified.0,
        "{counters:?}"
    );
    assert_eq!(
        TcpAuthenticationCounters {
            verified: AUTHENTICATION_COUNTERS.verified,
            ..counters
        },
        AUTHENTICATION_COUNTERS
    );
    let key = |identifier| TcpQuery::AuthenticationKey(authentication_identity(identifier));
    match sockets.read_state(&pair.client, key(FIRST_KEY)) {
        Ok(TcpReading::AuthenticationKey(Some(held))) => {
            assert!(held.verified.0 > u64::MIN, "{held:?}");
            assert_eq!(held.failed, SegmentCount(u64::MIN));
        }
        other => panic!("the first key read {other:?}"),
    }
    assert_eq!(
        sockets.read_state(&pair.client, key(SECOND_KEY)),
        Ok(TcpReading::AuthenticationKey(None))
    );
    sockets
        .set_authentication_counters(&pair.client, LATER_AUTHENTICATION_COUNTERS)
        .expect("set the counters");
    assert_eq!(
        authentication_status(&sockets, &pair.client).map(|status| status.counters),
        Ok(LATER_AUTHENTICATION_COUNTERS)
    );
    let listener = listener_authentication_status(&sockets, &pair.listener)
        .expect("the listener's status")
        .counters;
    assert!(
        listener.verified.0 >= AUTHENTICATION_COUNTERS.verified.0,
        "{listener:?}"
    );
    assert!(matches!(
        sockets.read_listener_state(
            &pair.listener,
            TcpListenerQuery::AuthenticationKey(authentication_identity(FIRST_KEY))
        ),
        Ok(TcpReading::AuthenticationKey(Some(_)))
    ));
    sockets
        .set_listener_authentication_counters(&pair.listener, LATER_AUTHENTICATION_COUNTERS)
        .expect("set the listener's counters");
    assert_eq!(
        listener_authentication_status(&sockets, &pair.listener).map(|status| status.counters),
        Ok(LATER_AUTHENTICATION_COUNTERS)
    );
    close(&sockets, pair);
}

/// The send sequence number extension an authentication repair check writes, unlike a new connection's zero.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const SEND_EXTENSION: SequenceNumberExtension = SequenceNumberExtension(1);
/// The receive sequence number extension an authentication repair check writes, unlike the send one.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const RECEIVE_EXTENSION: SequenceNumberExtension = SequenceNumberExtension(2);

/// Checks that in repair mode an authenticated connection's initial sequence numbers are those its SYN and SYN-ACK carried.
///
/// Its extensions start at zero, and repair values written are what is read back.
pub fn authentication_repair_values_are_read_and_written<S: TcpSockets>(sockets: S) {
    let (pair, capture) = authenticated_pair(&sockets);
    let initial_sequence = |to_listener| {
        capture
            .segments()
            .iter()
            .find(|segment| segment.synchronize && segment.to_listener == to_listener)
            .map(|segment| TcpSequenceNumber(segment.sequence))
            .expect("the SYN and SYN-ACK")
    };
    for end in [&pair.client, &pair.server] {
        sockets
            .configure(end, TcpSetting::Repair(TcpRepair::On))
            .expect("repair mode");
    }
    let client = repair_state(&sockets, &pair.client)
        .expect("the client's repair state")
        .authentication
        .expect("the client's authentication repair values");
    let server = repair_state(&sockets, &pair.server)
        .expect("the server's repair state")
        .authentication
        .expect("the server's authentication repair values");
    let (sent, received) = (initial_sequence(true), initial_sequence(false));
    assert_eq!(
        (
            client.sent_initial_sequence,
            client.received_initial_sequence
        ),
        (sent, received)
    );
    assert_eq!(
        (
            server.sent_initial_sequence,
            server.received_initial_sequence
        ),
        (received, sent)
    );
    let fresh = SequenceNumberExtension(u32::MIN);
    assert_eq!(
        (client.send_extension, client.receive_extension),
        (fresh, fresh)
    );
    let written = TcpAuthenticationRepair {
        send_extension: SEND_EXTENSION,
        receive_extension: RECEIVE_EXTENSION,
        ..client
    };
    sockets
        .configure(&pair.client, TcpSetting::AuthenticationRepair(written))
        .expect("write the repair values");
    assert_eq!(
        repair_state(&sockets, &pair.client).map(|state| state.authentication),
        Ok(Some(written))
    );
    sockets.close(pair.client, TcpClose::Abort);
    sockets.close(pair.server, TcpClose::Abort);
    sockets.stop_listening(pair.listener);
}

/// Checks that a multipath connection's state reads what it carried, its data sequence numbers agreeing with its peer's.
///
/// Its first subflow reads the connection's addresses and none follows it; a TCP connection has no multipath state.
pub fn multipath_state_describes_the_connection<S: TcpSockets>(sockets: S) {
    let listening = sockets
        .listen(
            TcpProtocol::Multipath,
            OWNER,
            bind_any_port(ipv4_loopback()),
            BACKLOG,
            &[],
        )
        .expect("listen");
    let connected = sockets
        .connect(
            TcpProtocol::Multipath,
            OWNER,
            bind_any_port(ipv4_loopback()),
            listening.local,
            &[],
            TcpFastOpen::Off,
        )
        .expect("connect");
    let server = accept(&sockets, &listening.listener);
    let pair = Pair {
        listener: listening.listener,
        local: listening.local,
        client: connected.connection,
        server,
    };
    assert_carries_bytes(&sockets, &pair);
    std::thread::sleep(SETTLE);
    let (client, server) = (
        multipath(&sockets, &pair.client),
        multipath(&sockets, &pair.server),
    );
    assert!(
        !client.fell_back_to_tcp && !server.fell_back_to_tcp,
        "{client:?}"
    );
    assert!(
        client.remote_key_received && server.remote_key_received,
        "{client:?}"
    );
    let (message, reply) = (MESSAGE.len() as u64, REPLY.len() as u64);
    assert_eq!(
        (
            client.bytes_sent,
            client.bytes_acknowledged,
            client.bytes_received
        ),
        (ByteTotal(message), ByteTotal(message), ByteTotal(reply))
    );
    assert_eq!(server.bytes_received, ByteTotal(message));
    // A token names its connection among those of its host, so the two ends of one on the same host differ.
    assert_ne!(client.token, server.token);
    assert_eq!(client.write_sequence, client.send_unacknowledged);
    assert_eq!(client.receive_next, server.write_sequence);
    assert_eq!(server.receive_next, client.write_sequence);
    assert!(client.subflows_total.0 >= u8::from(true), "{client:?}");
    match sockets.read_state(
        &pair.client,
        TcpQuery::MultipathSubflow(MultipathSubflowPosition(u32::MIN)),
    ) {
        Ok(TcpReading::MultipathSubflow(Some(subflow))) => {
            assert_eq!(
                (subflow.local, subflow.peer),
                (connected.local, listening.local)
            );
            // The one subflow carried every byte, as the connection's own information reads.
            let carried = subflow.information;
            assert_eq!(carried.state, TcpState::Established);
            assert_eq!(
                (
                    carried.bytes_received,
                    carried.bytes_acknowledged.0 >= message
                ),
                (ByteTotal(reply), true),
                "{carried:?}"
            );
            assert!(
                carried.send_maximum_segment_size.0 > u16::MIN,
                "{carried:?}"
            );
        }
        other => panic!("the first subflow read {other:?}"),
    }
    assert_eq!(
        sockets.read_state(
            &pair.client,
            TcpQuery::MultipathSubflow(MultipathSubflowPosition(u32::from(
                client.subflows_total.0
            )))
        ),
        Ok(TcpReading::MultipathSubflow(None))
    );
    close(&sockets, pair);
    let pair = self::pair(&sockets);
    assert!(
        sockets
            .read_state(&pair.client, TcpQuery::Multipath)
            .is_err()
    );
    assert!(
        sockets
            .read_state(
                &pair.client,
                TcpQuery::MultipathSubflow(MultipathSubflowPosition(u32::MIN))
            )
            .is_err()
    );
    close(&sockets, pair);
}

/// The interface index that names none.
///
/// https://man7.org/linux/man-pages/man3/if_nametoindex.3.html
const NO_INTERFACE: InterfaceIndex = InterfaceIndex(u32::MIN);

/// Checks that a connection asked to record the last header it received reads the interface and address it came to.
///
/// And over IPv6 the hop-by-hop header its peer sent; one not asked, or no longer asked, reads none, over IPv4 and IPv6.
pub fn last_received_header_records_where_the_last_packet_came<S: TcpSockets>(sockets: S) {
    let records = |report| TcpSetting::RecordsLastReceivedHeader(report);
    let hop_by_hop = [TcpSetting::Packet(IpPacketSetting::HopByHopOptions(
        padding_header(),
    ))];
    // IPv4 carries no options here; the IPv6 client sends a hop-by-hop header with every packet.
    for (address, destination, client_settings) in [
        (ipv4_loopback(), Some(Ipv4Addr::LOCALHOST), &[][..]),
        (ipv6_loopback(), None, &hop_by_hop[..]),
    ] {
        let recorded = pair_at(
            &sockets,
            address,
            client_settings,
            &[TcpListenerSetting::Socket(records(Report::Requested))],
        );
        let plain_pair = pair_at(&sockets, address, &[], &[]);
        assert_carries_bytes(&sockets, &recorded);
        assert_carries_bytes(&sockets, &plain_pair);
        let (fields, recorded_header) = last_received_header(&sockets, &recorded.server);
        assert_eq!(
            (fields.interface, fields.specific_destination),
            (loopback_interface(), destination),
            "{address:?}"
        );
        if client_settings.is_empty() {
            assert!(recorded_header.is_empty(), "{recorded_header:?}");
        } else {
            // The header as sent, with the next header its sender filled in.
            let extension = Ipv6RawExtHeaderSlice::from_slice(&recorded_header)
                .expect("the recorded hop-by-hop header");
            assert_eq!(
                (
                    extension.next_header(),
                    extension.slice().len(),
                    recorded_header.len()
                ),
                (IpNumber::TCP, PADDING_HEADER.len(), PADDING_HEADER.len())
            );
            assert!(extension.payload().iter().all(|&byte| byte == u8::MIN));
        }
        let (fields, header) = last_received_header(&sockets, &plain_pair.server);
        assert_eq!((fields.interface, header.len()), (NO_INTERFACE, usize::MIN));
        sockets
            .configure(&recorded.server, records(Report::NotRequested))
            .expect("records off");
        assert_eq!(
            last_received_header(&sockets, &recorded.server).0.interface,
            NO_INTERFACE
        );
        close(&sockets, recorded);
        close(&sockets, plain_pair);
    }
}

/// A processor a listener is told to receive on, not the first, so that a reading of the first alone is seen.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const SET_PROCESSOR: ProcessorIndex = ProcessorIndex(3);

/// Checks that a connection that received reads a processor it was received on, and a listener the one set or none.
pub fn incoming_processor_is_read_as_received_or_set<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    assert_carries_bytes(&sockets, &pair);
    let processors = std::thread::available_parallelism().map_or(u32::MAX, |count| {
        u32::try_from(count.get()).unwrap_or(u32::MAX)
    });
    match sockets.read_state(&pair.server, TcpQuery::IncomingProcessor) {
        Ok(TcpReading::IncomingProcessor(Some(ProcessorIndex(index)))) => {
            assert!(index < processors, "processor {index} of {processors}")
        }
        other => panic!("the connection's incoming processor read {other:?}"),
    }
    let read =
        |listener| sockets.read_listener_state(listener, TcpListenerQuery::IncomingProcessor);
    let (listener, _) = listen(&sockets, ipv4_loopback(), &[]);
    assert_eq!(read(&listener), Ok(TcpReading::IncomingProcessor(None)));
    for processor in [Some(ProcessorIndex(u32::MIN)), Some(SET_PROCESSOR), None] {
        sockets
            .configure(
                &listener,
                TcpListenerSetting::Socket(TcpSetting::Port(PortSetting::IncomingProcessor(
                    processor,
                ))),
            )
            .expect("set the incoming processor");
        assert_eq!(
            read(&listener),
            Ok(TcpReading::IncomingProcessor(processor))
        );
    }
    sockets.stop_listening(listener);
    close(&sockets, pair);
}

/// Makes a connection and a setting it takes, for the suite of `Configurable`.
pub fn configured_connection<S: TcpSockets>(sockets: S) -> (S, S::Connection, TcpSetting<'static>) {
    let pair = pair(&sockets);
    (sockets, pair.client, TcpSetting::NoDelay(true))
}

/// Makes a listener and a setting it takes, for the suite of `Configurable`.
pub fn configured_listener<S: TcpSockets>(
    sockets: S,
) -> (S, S::Listener, TcpListenerSetting<'static>) {
    let (listener, _) = listen(&sockets, ipv4_loopback(), &[]);
    (sockets, listener, TcpListenerSetting::Backlog(BACKLOG))
}

/// Makes a connection to advise, for the suite of `NegativeRouteAdvisable`.
pub fn advised_connection<S: TcpSockets>(sockets: S) -> (S, S::Connection) {
    let pair = pair(&sockets);
    (sockets, pair.client)
}

/// A waker that counts its wakes and unparks the thread that made it.
struct CountingWaker {
    wakes: AtomicUsize,
    thread: Thread,
}

impl Wake for CountingWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
        self.thread.unpark();
    }
}

fn counting_waker() -> (Arc<CountingWaker>, Waker) {
    let counter = Arc::new(CountingWaker {
        wakes: AtomicUsize::new(usize::MIN),
        thread: std::thread::current(),
    });
    (Arc::clone(&counter), Waker::from(counter))
}

fn wakes(counter: &CountingWaker) -> usize {
    counter.wakes.load(Ordering::SeqCst)
}

fn poll_once<F: Future>(future: Pin<&mut F>, waker: &Waker) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(waker))
}

/// Parks until the waker has been woken the given number of times, or the time is up.
fn woken_within(counter: &CountingWaker, times: usize, within: Duration) -> bool {
    let start = Instant::now();
    while wakes(counter) < times {
        let left = within.saturating_sub(start.elapsed());
        if left.is_zero() {
            return false;
        }
        std::thread::park_timeout(left);
    }
    true
}

/// Polls a wait each time it is woken until it finishes, within the deadline.
fn finish<F: Future>(mut future: Pin<&mut F>, counter: &CountingWaker, waker: &Waker) -> F::Output {
    let start = Instant::now();
    loop {
        let seen = wakes(counter);
        if let Poll::Ready(output) = poll_once(future.as_mut(), waker) {
            return output;
        }
        assert!(
            woken_within(counter, seen + 1, DEADLINE.saturating_sub(start.elapsed())),
            "the wait was not woken within the deadline"
        );
    }
}

/// How many connections the check of waits on many connections watches at once.
///
/// https://github.com/Declarative-labs/wcontainers/blob/main/docs/plan-host-tcp-sockets.md#the-suites-own-choices
const WATCHED_CONNECTIONS: usize = 16;

/// Gives a connection waited on for data, made ready by data its peer sends, for the readiness suite.
pub fn waiting_for_data<S: TcpSockets>(
    sockets: S,
) -> Waiting<S, S::Connection, impl FnOnce(&S, &S::Connection)> {
    let Pair {
        listener,
        client,
        server,
        ..
    } = pair(&sockets);
    Waiting {
        readiness: sockets,
        handle: server,
        interest: ReadyFor::READABLE,
        make_ready: move |sockets: &S, _: &S::Connection| {
            send_all(sockets, &client, plain(), MESSAGE);
            sockets.close(client, TcpClose::Orderly);
            sockets.stop_listening(listener);
        },
    }
}

/// Gives a send that asks for a software transmit report, which queues an entry on the sender's error queue.
fn software_reported() -> TcpSendRequest {
    TcpSendRequest {
        transmit_reports: TransmitReportRequest {
            key: KEY,
            software: Report::Requested,
            ..no_reports()
        },
        ..plain()
    }
}

/// Gives a connection waited on for urgent data, which a transmit report on its error queue fails with no hang-up.
pub fn waiting_for_urgent_data_until_a_report<S: TcpSockets>(
    sockets: S,
) -> Waiting<S, S::Connection, impl FnOnce(&S, &S::Connection)> {
    let Pair {
        listener,
        client,
        server,
        ..
    } = pair(&sockets);
    Waiting {
        readiness: sockets,
        handle: client,
        interest: ReadyFor::URGENT_DATA,
        make_ready: move |sockets: &S, client: &S::Connection| {
            send_all(sockets, client, software_reported(), MESSAGE);
            // Read before the close, which would otherwise reset the connection and hang it up.
            receive_up_to(sockets, &server, MESSAGE.len());
            sockets.close(server, TcpClose::Orderly);
            sockets.stop_listening(listener);
        },
    }
}

/// Gives a connection that has shut down writing, waited on for urgent data, which its peer's shutdown hangs up with no error.
pub fn waiting_for_urgent_data_until_both_ways_are_shut<S: TcpSockets>(
    sockets: S,
) -> Waiting<S, S::Connection, impl FnOnce(&S, &S::Connection)> {
    let Pair {
        listener,
        client,
        server,
        ..
    } = pair(&sockets);
    sockets
        .shut_down(&server, TcpShutdown::Write)
        .expect("shut down");
    Waiting {
        readiness: sockets,
        handle: server,
        interest: ReadyFor::URGENT_DATA,
        make_ready: move |sockets: &S, _: &S::Connection| {
            sockets
                .shut_down(&client, TcpShutdown::Write)
                .expect("shut down");
            sockets.close(client, TcpClose::Orderly);
            sockets.stop_listening(listener);
        },
    }
}

/// Gives a listener waited on for a connection to accept, made ready by a connection to it, for the readiness suite.
pub fn waiting_for_a_connection<S: TcpSockets>(
    sockets: S,
) -> Waiting<S, S::Listener, impl FnOnce(&S, &S::Listener)> {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    Waiting {
        readiness: sockets,
        handle: listener,
        interest: ReadyFor::READABLE,
        make_ready: move |sockets: &S, _: &S::Listener| {
            let client = sockets
                .connect(
                    TcpProtocol::Tcp,
                    OWNER,
                    bind_any_port(ipv4_any()),
                    local,
                    &[],
                    TcpFastOpen::Off,
                )
                .expect("connect")
                .connection;
            // A send is taken only once the handshake is done, so the connection is queued before the client closes.
            send_all(sockets, &client, plain(), MESSAGE);
            sockets.close(client, TcpClose::Orderly);
        },
    }
}

/// Checks that a wait on each of many connections is woken once when its own connection's data comes, and the silent ones not at all.
pub fn waits_on_many_connections_each_wake_once<S: TcpSockets>(sockets: S) {
    let pairs: Vec<_> = (0..WATCHED_CONNECTIONS).map(|_| pair(&sockets)).collect();
    let mut waits: Vec<_> = pairs
        .iter()
        .map(|pair| {
            let (counter, waker) = counting_waker();
            (
                Box::pin(sockets.wait_until_ready(&pair.server, ReadyFor::READABLE)),
                counter,
                waker,
            )
        })
        .collect();
    for (wait, _, waker) in &mut waits {
        assert!(poll_once(wait.as_mut(), waker).is_pending());
    }
    // Every other connection gets data; the rest stay silent.
    for (_, pair) in pairs.iter().enumerate().filter(|(index, _)| index % 2 == 0) {
        send_all(&sockets, &pair.client, plain(), MESSAGE);
    }
    for (index, (wait, counter, waker)) in waits
        .iter_mut()
        .enumerate()
        .filter(|(index, _)| index % 2 == 0)
    {
        assert!(
            woken_within(counter, ONE_WAKE, DEADLINE),
            "wait {index} was not woken"
        );
        let facts = poll_once(wait.as_mut(), waker);
        assert!(matches!(facts, Poll::Ready(Ok(facts)) if facts.contains(ReadyFor::READABLE)));
    }
    std::thread::sleep(SETTLE);
    for (index, (_, counter, _)) in waits.iter().enumerate() {
        let fed = index % 2 == 0;
        assert_eq!(wakes(counter), usize::from(fed), "wait {index}");
    }
    drop(waits);
    for pair in pairs {
        close(&sockets, pair);
    }
}

/// Checks that a wait for room to send finishes when the peer has read enough of what fills the connection.
pub fn wait_for_room_finishes_when_the_peer_reads<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let chunk = vec![u8::MIN; SETTING_BYTES];
    let start = Instant::now();
    loop {
        match sockets.send(&pair.client, plain(), &chunk) {
            Ok(_) => assert!(start.elapsed() < DEADLINE, "the connection never filled"),
            Err(HostError::WouldBlock) => break,
            Err(error) => panic!("send: {error:?}"),
        }
    }
    let (counter, waker) = counting_waker();
    {
        let mut wait = pin!(sockets.wait_until_ready(&pair.client, ReadyFor::WRITABLE));
        assert!(poll_once(wait.as_mut(), &waker).is_pending());
        assert!(
            !woken_within(&counter, ONE_WAKE, SETTLE),
            "woken while the connection was full"
        );
        let mut sink = vec![u8::MIN; SETTING_BYTES];
        while wakes(&counter) == 0 {
            assert!(
                start.elapsed() < DEADLINE,
                "never woken while the peer read"
            );
            match sockets.receive(&pair.server, TcpReceiveRequest::Read, &mut sink) {
                Ok(_) | Err(HostError::WouldBlock) => {}
                Err(error) => panic!("receive: {error:?}"),
            }
        }
        let facts = finish(wait, &counter, &waker).expect("wait for room");
        assert!(facts.contains(ReadyFor::WRITABLE), "{facts:?}");
    }
    close(&sockets, pair);
}

/// Gives a wait on the server, after its own step, finishes when the client's act brings a fact asked for, and not before.
fn wait_finishes_when_the_peer_acts<S: TcpSockets>(
    sockets: &S,
    interest: ReadyFor,
    own: impl FnOnce(&S, &S::Connection),
    act: impl FnOnce(&S, &S::Connection),
    given: impl Fn(ReadyFor) -> bool,
) {
    let pair = pair(sockets);
    own(sockets, &pair.server);
    let (counter, waker) = counting_waker();
    {
        let mut wait = pin!(sockets.wait_until_ready(&pair.server, interest));
        assert!(poll_once(wait.as_mut(), &waker).is_pending());
        assert!(
            !woken_within(&counter, ONE_WAKE, SETTLE),
            "woken before the peer acted"
        );
        assert!(poll_once(wait.as_mut(), &waker).is_pending());
        act(sockets, &pair.client);
        assert!(
            woken_within(&counter, ONE_WAKE, DEADLINE),
            "not woken when the peer acted"
        );
        let facts = finish(wait, &counter, &waker).expect("wait");
        assert!(given(facts), "{facts:?}");
    }
    close(sockets, pair);
}

/// Checks that a wait for urgent data finishes when the urgent byte arrives.
pub fn wait_for_urgent_data_finishes_when_it_arrives<S: TcpSockets>(sockets: S) {
    wait_finishes_when_the_peer_acts(
        &sockets,
        ReadyFor::URGENT_DATA,
        |_, _| {},
        |sockets, client| send_all(sockets, client, urgent(), URGENT),
        |facts| facts.contains(ReadyFor::URGENT_DATA),
    );
}

/// Checks that a wait for the peer to finish sending finishes when it shuts down writing, with no hang-up while this end still writes.
pub fn wait_for_the_peers_shutdown_finishes_when_it_comes<S: TcpSockets>(sockets: S) {
    wait_finishes_when_the_peer_acts(
        &sockets,
        ReadyFor::PEER_FINISHED_SENDING,
        |_, _| {},
        |sockets, client| {
            sockets
                .shut_down(client, TcpShutdown::Write)
                .expect("shut down");
        },
        |facts| {
            facts.contains(ReadyFor::PEER_FINISHED_SENDING) && !facts.contains(ReadyFor::HANG_UP)
        },
    );
}

/// Checks that a wait for a hang-up finishes, with no error, when the peer shuts down writing after this end has.
pub fn wait_for_a_hang_up_finishes_when_both_ways_are_shut<S: TcpSockets>(sockets: S) {
    let shut_down_writing = |sockets: &S, connection: &S::Connection| {
        sockets
            .shut_down(connection, TcpShutdown::Write)
            .expect("shut down");
    };
    wait_finishes_when_the_peer_acts(
        &sockets,
        ReadyFor::HANG_UP,
        shut_down_writing,
        shut_down_writing,
        |facts| facts.contains(ReadyFor::HANG_UP) && !facts.contains(ReadyFor::ERROR),
    );
}

/// Gives the facts a wait on the handle gives once it finishes.
fn facts_once_ready<R: Readiness<H>, H>(readiness: &R, handle: &H, interest: ReadyFor) -> ReadyFor {
    let (counter, waker) = counting_waker();
    finish(
        pin!(readiness.wait_until_ready(handle, interest)),
        &counter,
        &waker,
    )
    .expect("wait")
}

/// Checks that a wait for every fact gives exactly the facts the connection has, in each state that brings a different set.
pub fn a_wait_gives_the_facts_the_connection_has_and_no_others<S: TcpSockets>(sockets: S) {
    let every =
        |connection: &S::Connection| facts_once_ready(&sockets, connection, ReadyFor::all());
    let readable_and_writable = ReadyFor::READABLE | ReadyFor::WRITABLE;
    let data = pair(&sockets);
    assert_eq!(every(&data.server), ReadyFor::WRITABLE, "nothing to read");
    send_all(&sockets, &data.client, plain(), MESSAGE);
    facts_once_ready(&sockets, &data.server, ReadyFor::READABLE);
    assert_eq!(every(&data.server), readable_and_writable, "data to read");
    send_all(&sockets, &data.client, urgent(), URGENT);
    facts_once_ready(&sockets, &data.server, ReadyFor::URGENT_DATA);
    assert_eq!(
        every(&data.server),
        readable_and_writable | ReadyFor::URGENT_DATA,
        "urgent data behind data"
    );
    close(&sockets, data);

    let shut = pair(&sockets);
    sockets
        .shut_down(&shut.client, TcpShutdown::Write)
        .expect("shut down");
    facts_once_ready(&sockets, &shut.server, ReadyFor::PEER_FINISHED_SENDING);
    assert_eq!(
        every(&shut.server),
        readable_and_writable | ReadyFor::PEER_FINISHED_SENDING,
        "the peer finished sending"
    );
    sockets
        .shut_down(&shut.server, TcpShutdown::Write)
        .expect("shut down");
    facts_once_ready(&sockets, &shut.client, ReadyFor::HANG_UP);
    assert_eq!(
        every(&shut.client),
        readable_and_writable | ReadyFor::PEER_FINISHED_SENDING | ReadyFor::HANG_UP,
        "both ways shut"
    );
    close(&sockets, shut);

    let reported = pair(&sockets);
    send_all(&sockets, &reported.client, software_reported(), MESSAGE);
    facts_once_ready(&sockets, &reported.client, ReadyFor::ERROR);
    assert_eq!(
        every(&reported.client),
        ReadyFor::WRITABLE | ReadyFor::ERROR,
        "a transmit report queued"
    );
    assert_eq!(
        receive_up_to(&sockets, &reported.server, MESSAGE.len()),
        MESSAGE
    );
    close(&sockets, reported);
}

/// Checks that a wait for every fact on a listener waits while none is queued, and then gives only that it is readable.
pub fn a_wait_gives_the_facts_the_listener_has_and_no_others<S: TcpSockets>(sockets: S) {
    let (listener, local) = listen(&sockets, ipv4_loopback(), &[]);
    let (counter, waker) = counting_waker();
    {
        let mut wait = pin!(sockets.wait_until_ready(&listener, ReadyFor::all()));
        assert!(
            poll_once(wait.as_mut(), &waker).is_pending(),
            "nothing to accept"
        );
        assert!(
            !woken_within(&counter, ONE_WAKE, SETTLE),
            "nothing to accept"
        );
    }
    let client = sockets
        .connect(
            TcpProtocol::Tcp,
            OWNER,
            bind_any_port(ipv4_any()),
            local,
            &[],
            TcpFastOpen::Off,
        )
        .expect("connect")
        .connection;
    facts_once_ready(&sockets, &listener, ReadyFor::READABLE);
    assert_eq!(
        facts_once_ready(&sockets, &listener, ReadyFor::all()),
        ReadyFor::READABLE,
        "a connection to accept"
    );
    sockets.close(client, TcpClose::Orderly);
    sockets.stop_listening(listener);
}

/// Checks that a wait for an error finishes when the peer resets the connection, and gives the error with the hang-up.
pub fn wait_for_an_error_finishes_when_the_peer_resets<S: TcpSockets>(sockets: S) {
    let pair = pair(&sockets);
    let (counter, waker) = counting_waker();
    {
        let mut wait = pin!(sockets.wait_until_ready(&pair.client, ReadyFor::ERROR));
        assert!(poll_once(wait.as_mut(), &waker).is_pending());
        assert!(
            !woken_within(&counter, ONE_WAKE, SETTLE),
            "woken before the reset"
        );
        sockets.close(pair.server, TcpClose::Abort);
        let facts = finish(wait, &counter, &waker).expect("wait across the reset");
        assert!(
            facts.contains(ReadyFor::ERROR | ReadyFor::HANG_UP),
            "{facts:?}"
        );
    }
    sockets.close(pair.client, TcpClose::Orderly);
    sockets.stop_listening(pair.listener);
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn connect_and_accept_name_both_ends() {
            $crate::connect_and_accept_name_both_ends($value);
        }

        #[test]
        fn accept_cuts_the_synchronize_header_to_the_buffer() {
            $crate::accept_cuts_the_synchronize_header_to_the_buffer($value);
        }

        #[test]
        fn connect_settings_apply_to_the_synchronize() {
            $crate::connect_settings_apply_to_the_synchronize($value);
        }

        #[test]
        fn bytes_arrive_in_order_both_ways() {
            $crate::bytes_arrive_in_order_both_ways($value);
        }

        #[test]
        fn connects_over_ipv6() {
            $crate::connects_over_ipv6($value);
        }

        #[test]
        fn dual_stack_listener_accepts_ipv4() {
            $crate::dual_stack_listener_accepts_ipv4($value);
        }

        #[test]
        fn multipath_is_negotiated_when_both_ends_ask() {
            $crate::multipath_is_negotiated_when_both_ends_ask($value);
        }

        #[test]
        fn requested_port_is_bound() {
            $crate::requested_port_is_bound($value);
        }

        #[test]
        fn exit_node_chooses_a_port_outside_the_reserved() {
            $crate::exit_node_chooses_a_port_outside_the_reserved($value);
        }

        #[test]
        fn stopped_listener_refuses_connections() {
            $crate::stopped_listener_refuses_connections($value);
        }

        #[test]
        fn accept_without_a_connection_would_block() {
            $crate::accept_without_a_connection_would_block($value);
        }

        #[test]
        fn write_shutdown_ends_the_peers_stream() {
            $crate::write_shutdown_ends_the_peers_stream($value);
        }

        #[test]
        fn read_shutdown_ends_the_own_stream() {
            $crate::read_shutdown_ends_the_own_stream($value);
        }

        #[test]
        fn shutdown_of_both_directions_ends_both() {
            $crate::shutdown_of_both_directions_ends_both($value);
        }

        #[test]
        fn orderly_close_ends_the_peers_stream() {
            $crate::orderly_close_ends_the_peers_stream($value);
        }

        #[test]
        fn abort_resets_the_peer() {
            $crate::abort_resets_the_peer($value);
        }

        #[test]
        fn peek_leaves_the_bytes() {
            $crate::peek_leaves_the_bytes($value);
        }

        #[test]
        fn peek_at_an_offset_skips_bytes() {
            $crate::peek_at_an_offset_skips_bytes($value);
        }

        #[test]
        fn urgent_byte_is_read_apart() {
            $crate::urgent_byte_is_read_apart($value);
        }

        #[test]
        fn urgent_inline_keeps_the_byte_in_the_stream() {
            $crate::urgent_inline_keeps_the_byte_in_the_stream($value);
        }

        #[test]
        fn cork_holds_data_until_uncorked() {
            $crate::cork_holds_data_until_uncorked($value);
        }

        #[test]
        fn more_to_come_holds_data_until_pushed() {
            $crate::more_to_come_holds_data_until_pushed($value);
        }

        #[test]
        fn push_ignoring_neither_keeps_the_cork() {
            $crate::push_ignoring_neither_keeps_the_cork($value);
        }

        #[test]
        fn push_ignoring_neither_sends_data_held_for_more() {
            $crate::push_ignoring_neither_sends_data_held_for_more($value);
        }

        #[test]
        fn record_end_starts_a_new_segment() {
            $crate::record_end_starts_a_new_segment($value);
        }

        #[test]
        fn unread_byte_reports_count_what_is_left() {
            $crate::unread_byte_reports_count_what_is_left($value);
        }

        #[test]
        fn listener_unread_byte_reports_reach_the_connections_it_accepts() {
            $crate::listener_unread_byte_reports_reach_the_connections_it_accepts($value);
        }

        #[test]
        fn receive_software_timestamps_when_asked() {
            $crate::receive_software_timestamps_when_asked($value);
        }

        #[test]
        fn software_transmit_report_names_the_send() {
            $crate::software_transmit_report_names_the_send($value);
        }

        #[test]
        fn reports_name_their_sends_among_plain_ones() {
            $crate::reports_name_their_sends_among_plain_ones($value);
        }

        #[test]
        fn acknowledgement_report_follows_the_peers_acknowledgement() {
            $crate::acknowledgement_report_follows_the_peers_acknowledgement($value);
        }

        #[test]
        fn discarding_empties_the_error_queue() {
            $crate::discarding_empties_the_error_queue($value);
        }

        #[test]
        fn empty_error_queue_would_block() {
            $crate::empty_error_queue_would_block($value);
        }

        #[test]
        fn icmp_error_reaches_a_synchronizing_connection_over_ipv4() {
            $crate::icmp_error_reaches_a_synchronizing_connection_over_ipv4($value);
        }

        #[test]
        fn icmp_error_reaches_a_synchronizing_connection_over_ipv6() {
            $crate::icmp_error_reaches_a_synchronizing_connection_over_ipv6($value);
        }

        #[test]
        fn transmit_report_carries_the_packet_when_asked() {
            $crate::transmit_report_carries_the_packet_when_asked($value);
        }

        #[test]
        fn transmit_statistics_come_with_the_report_when_asked() {
            $crate::transmit_statistics_come_with_the_report_when_asked($value);
        }

        #[test]
        fn wireless_status_request_still_sends() {
            $crate::wireless_status_request_still_sends($value);
        }

        #[test]
        fn fast_open_with_data_puts_it_in_the_synchronize() {
            $crate::fast_open_with_data_puts_it_in_the_synchronize($value);
        }

        #[test]
        fn fast_open_until_first_send_defers_the_synchronize() {
            $crate::fast_open_until_first_send_defers_the_synchronize($value);
        }

        #[test]
        fn fast_open_without_cookie_needs_no_first_connect() {
            $crate::fast_open_without_cookie_needs_no_first_connect($value);
        }

        #[test]
        fn fast_open_keys_and_queue_decide_what_the_listener_takes() {
            $crate::fast_open_keys_and_queue_decide_what_the_listener_takes($value);
        }

        #[test]
        fn keep_alive_probes_end_an_unanswered_connection() {
            $crate::keep_alive_probes_end_an_unanswered_connection($value);
        }

        #[test]
        fn rearming_the_keep_alive_timer_sends_a_due_probe() {
            $crate::rearming_the_keep_alive_timer_sends_a_due_probe($value);
        }

        #[test]
        fn no_delay_sends_small_segments_while_others_are_unacknowledged() {
            $crate::no_delay_sends_small_segments_while_others_are_unacknowledged($value);
        }

        #[test]
        fn quick_acknowledgement_settings_time_the_acknowledgement() {
            $crate::quick_acknowledgement_settings_time_the_acknowledgement($value);
        }

        #[test]
        fn delayed_acknowledgement_is_sent_on_demand() {
            $crate::delayed_acknowledgement_is_sent_on_demand($value);
        }

        #[test]
        fn send_settings_bound_what_a_connection_takes() {
            $crate::send_settings_bound_what_a_connection_takes($value);
        }

        #[test]
        fn receive_settings_bound_what_the_peer_sends() {
            $crate::receive_settings_bound_what_the_peer_sends($value);
        }

        #[test]
        fn window_clamp_bounds_the_synchronize_window() {
            $crate::window_clamp_bounds_the_synchronize_window($value);
        }

        #[test]
        fn retransmission_settings_time_the_retransmissions() {
            $crate::retransmission_settings_time_the_retransmissions($value);
        }

        #[test]
        fn user_timeout_ends_an_unacknowledged_connection() {
            $crate::user_timeout_ends_an_unacknowledged_connection($value);
        }

        #[test]
        fn synchronize_retries_end_an_unanswered_connect() {
            $crate::synchronize_retries_end_an_unanswered_connect($value);
        }

        #[test]
        fn defer_accept_holds_a_connection_until_data() {
            $crate::defer_accept_holds_a_connection_until_data($value);
        }

        #[test]
        fn fin_wait_2_settings_decide_how_a_closed_connection_ends() {
            $crate::fin_wait_2_settings_decide_how_a_closed_connection_ends($value);
        }

        #[test]
        fn arrival_settings_drop_what_they_refuse() {
            $crate::arrival_settings_drop_what_they_refuse($value);
        }

        #[test]
        fn header_settings_mark_each_segment() {
            $crate::header_settings_mark_each_segment($value);
        }

        #[test]
        fn fixed_flow_label_marks_every_segment() {
            $crate::fixed_flow_label_marks_every_segment($value);
        }

        #[test]
        fn anycast_addresses_are_joined_and_left() {
            $crate::anycast_addresses_are_joined_and_left($value);
        }

        #[test]
        fn multicast_groups_are_joined_and_left() {
            $crate::multicast_groups_are_joined_and_left($value);
        }

        #[test]
        fn md5_keys_admit_only_connections_signed_with_them() {
            $crate::md5_keys_admit_only_connections_signed_with_them($value);
        }

        #[test]
        fn authentication_keys_admit_only_connections_signed_with_them() {
            $crate::authentication_keys_admit_only_connections_signed_with_them($value);
        }

        #[test]
        fn authentication_options_choose_the_keys_in_use() {
            $crate::authentication_options_choose_the_keys_in_use($value);
        }

        #[test]
        fn congestion_control_is_the_one_named() {
            $crate::congestion_control_is_the_one_named($value);
        }

        #[test]
        fn pacing_rate_spreads_what_is_sent() {
            $crate::pacing_rate_spreads_what_is_sent($value);
        }

        #[test]
        fn transmit_delay_holds_each_segment() {
            $crate::transmit_delay_holds_each_segment($value);
        }

        #[test]
        fn address_sharing_lets_connections_leave_from_one_port() {
            $crate::address_sharing_lets_connections_leave_from_one_port($value);
        }

        #[test]
        fn port_sharing_lets_listeners_share_a_port() {
            $crate::port_sharing_lets_listeners_share_a_port($value);
        }

        #[test]
        fn nonlocal_addresses_are_bound_only_when_bindable() {
            $crate::nonlocal_addresses_are_bound_only_when_bindable($value);
        }

        #[test]
        fn listener_backlog_bounds_the_accept_queue() {
            $crate::listener_backlog_bounds_the_accept_queue($value);
        }

        #[test]
        fn negative_route_advice_picks_a_new_flow() {
            $crate::negative_route_advice_picks_a_new_flow($value);
        }

        #[test]
        fn flow_label_reflection_answers_with_the_peers_label() {
            $crate::flow_label_reflection_answers_with_the_peers_label($value);
        }

        #[test]
        fn window_probe_is_answered_and_changes_no_setting() {
            $crate::window_probe_is_answered_and_changes_no_setting($value);
        }

        #[test]
        fn timestamp_clock_set_in_repair_mode_stamps_segments() {
            $crate::timestamp_clock_set_in_repair_mode_stamps_segments($value);
        }

        #[test]
        fn timestamp_clock_in_the_other_unit_counts_in_it_or_is_refused() {
            $crate::timestamp_clock_in_the_other_unit_counts_in_it_or_is_refused($value);
        }

        #[test]
        fn transmit_statistics_describe_the_connection() {
            $crate::transmit_statistics_describe_the_connection($value);
        }

        #[test]
        fn delayed_acknowledgement_leaves_the_mode_when_none_is_scheduled() {
            $crate::delayed_acknowledgement_leaves_the_mode_when_none_is_scheduled($value);
        }

        #[test]
        fn accept_names_where_the_synchronize_arrived() {
            $crate::accept_names_where_the_synchronize_arrived($value);
        }

        #[test]
        fn connect_and_listen_make_sockets_of_their_owner() {
            $crate::connect_and_listen_make_sockets_of_their_owner($value);
        }

        #[test]
        fn connect_in_repair_mode_is_established_at_once() {
            $crate::connect_in_repair_mode_is_established_at_once($value);
        }

        #[test]
        fn listener_timestamping_reaches_the_connections_it_accepts() {
            $crate::listener_timestamping_reaches_the_connections_it_accepts($value);
        }

        #[test]
        fn saved_synchronize_is_taken_once() {
            $crate::saved_synchronize_is_taken_once($value);
        }

        #[test]
        fn authentication_counters_are_set_only_where_authentication_is() {
            $crate::authentication_counters_are_set_only_where_authentication_is($value);
        }

        #[test]
        fn listener_state_reads_a_listener() {
            $crate::listener_state_reads_a_listener($value);
        }

        #[test]
        fn information_reads_what_the_connection_did() {
            $crate::information_reads_what_the_connection_did($value);
        }

        #[test]
        fn congestion_control_state_names_the_algorithm() {
            $crate::congestion_control_state_names_the_algorithm($value);
        }

        #[test]
        fn queues_count_unread_unacknowledged_and_unsent() {
            $crate::queues_count_unread_unacknowledged_and_unsent($value);
        }

        #[test]
        fn tuned_values_follow_the_settings() {
            $crate::tuned_values_follow_the_settings($value);
        }

        #[test]
        fn timestamp_reads_the_clock_in_its_unit() {
            $crate::timestamp_reads_the_clock_in_its_unit($value);
        }

        #[test]
        fn repaired_connection_continues_where_the_original_left_off() {
            $crate::repaired_connection_continues_where_the_original_left_off($value);
        }

        #[test]
        fn authentication_state_reads_the_keys_and_counters() {
            $crate::authentication_state_reads_the_keys_and_counters($value);
        }

        #[test]
        fn authentication_repair_values_are_read_and_written() {
            $crate::authentication_repair_values_are_read_and_written($value);
        }

        #[test]
        fn multipath_state_describes_the_connection() {
            $crate::multipath_state_describes_the_connection($value);
        }

        #[test]
        fn last_received_header_records_where_the_last_packet_came() {
            $crate::last_received_header_records_where_the_last_packet_came($value);
        }

        #[test]
        fn incoming_processor_is_read_as_received_or_set() {
            $crate::incoming_processor_is_read_as_received_or_set($value);
        }

        #[test]
        fn header_options_go_out_in_the_synchronize() {
            $crate::header_options_go_out_in_the_synchronize($value);
        }

        #[test]
        fn connection_settings_set_twice_equal_set_once() {
            $crate::connection_settings_set_twice_equal_set_once($value);
        }

        #[test]
        fn ipv6_connection_settings_set_twice_equal_set_once() {
            $crate::ipv6_connection_settings_set_twice_equal_set_once($value);
        }

        #[test]
        fn listener_settings_set_twice_equal_set_once() {
            $crate::listener_settings_set_twice_equal_set_once($value);
        }

        #[test]
        fn refusable_connection_settings_set_twice_equal_set_once() {
            $crate::refusable_connection_settings_set_twice_equal_set_once($value);
        }

        #[test]
        fn waits_on_many_connections_each_wake_once() {
            $crate::waits_on_many_connections_each_wake_once($value);
        }

        #[test]
        fn wait_for_room_finishes_when_the_peer_reads() {
            $crate::wait_for_room_finishes_when_the_peer_reads($value);
        }

        #[test]
        fn wait_for_urgent_data_finishes_when_it_arrives() {
            $crate::wait_for_urgent_data_finishes_when_it_arrives($value);
        }

        #[test]
        fn wait_for_the_peers_shutdown_finishes_when_it_comes() {
            $crate::wait_for_the_peers_shutdown_finishes_when_it_comes($value);
        }

        #[test]
        fn wait_for_a_hang_up_finishes_when_both_ways_are_shut() {
            $crate::wait_for_a_hang_up_finishes_when_both_ways_are_shut($value);
        }

        #[test]
        fn a_wait_gives_the_facts_the_connection_has_and_no_others() {
            $crate::a_wait_gives_the_facts_the_connection_has_and_no_others($value);
        }

        #[test]
        fn a_wait_gives_the_facts_the_listener_has_and_no_others() {
            $crate::a_wait_gives_the_facts_the_listener_has_and_no_others($value);
        }

        #[test]
        fn wait_for_an_error_finishes_when_the_peer_resets() {
            $crate::wait_for_an_error_finishes_when_the_peer_resets($value);
        }

        mod configurable_tcp_connections {
            $crate::wcontainers_configurable_conformance::tests!($crate::configured_connection($value));
        }

        mod configurable_tcp_listeners {
            $crate::wcontainers_configurable_conformance::tests!($crate::configured_listener($value));
        }

        mod negative_route_advice {
            $crate::wcontainers_negative_route_advisable_conformance::tests!($crate::advised_connection($value));
        }

        $crate::wcontainers_readiness_conformance::tests!(
            readiness_tcp_connections,
            ready: $crate::waiting_for_data($value),
            failing: $crate::waiting_for_urgent_data_until_a_report($value)
        );

        // The failure above is an error alone; this one is a hang-up alone, so each ends a wait by itself.
        mod readiness_tcp_connections_hanging_up {
            #[test]
            fn a_failure_ends_a_wait_that_did_not_ask_for_it() {
                $crate::wcontainers_readiness_conformance::a_failure_ends_a_wait_that_did_not_ask_for_it(
                    $crate::waiting_for_urgent_data_until_both_ways_are_shut($value),
                );
            }
        }

        // No call of the traits makes a listener fail, so its readiness runs every check of the suite but the failure's.
        mod readiness_tcp_listeners {
            #[test]
            fn a_wait_does_not_finish_before_the_handle_is_ready() {
                $crate::wcontainers_readiness_conformance::a_wait_does_not_finish_before_the_handle_is_ready($crate::waiting_for_a_connection($value));
            }

            #[test]
            fn a_wait_finishes_when_the_handle_becomes_ready() {
                $crate::wcontainers_readiness_conformance::a_wait_finishes_when_the_handle_becomes_ready($crate::waiting_for_a_connection($value));
            }

            #[test]
            fn a_wait_made_when_ready_finishes_at_once() {
                $crate::wcontainers_readiness_conformance::a_wait_made_when_ready_finishes_at_once($crate::waiting_for_a_connection($value));
            }

            #[test]
            fn a_dropped_wait_is_withdrawn() {
                $crate::wcontainers_readiness_conformance::a_dropped_wait_is_withdrawn($crate::waiting_for_a_connection($value));
            }

            #[test]
            fn many_waits_each_wake_once() {
                $crate::wcontainers_readiness_conformance::many_waits_each_wake_once($crate::waiting_for_a_connection($value));
            }
        }
    };
}
