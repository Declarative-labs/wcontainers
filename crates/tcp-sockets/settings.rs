use core::num::NonZeroU32;
use core::time::Duration;

use wcontainers_internet_protocol::{
    ByteCount, HopLimit, IpPacketSetting, IpSocketSetting, IpVersion, PortSetting, Report,
    SocketFilter,
};

use crate::ListenBacklog;
use crate::authentication::{
    TcpAuthenticationCounters, TcpAuthenticationKeyAddition, TcpAuthenticationKeyRemoval,
    TcpAuthenticationOptions, TcpAuthenticationRepair, TcpMd5KeyChange,
};
use crate::repair::{TcpRepair, TcpRepairOption, TcpRepairQueue, TcpRepairWindow};

/// The length in bytes of one TCP Fast Open key, [`TCP_FASTOPEN_KEY_LENGTH`](https://github.com/torvalds/linux/blob/v6.18/include/net/tcp.h#L2038).
pub const FAST_OPEN_KEY_LENGTH: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowSize(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowScale(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaximumSegmentSize(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryCount(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeCount(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentCount(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpSequenceNumber(pub u32);

/// A setting of one TCP socket, the state that one [`setsockopt`](https://man7.org/linux/man-pages/man2/setsockopt.2.html) of it leaves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TcpSetting<'a> {
    Ip(IpSocketSetting<'a>),
    Packet(IpPacketSetting<'a>),
    Port(PortSetting<'a>),
    BufferLocks(BufferLocks),
    ReceiveLowWater(ByteCount),
    Filter(Option<SocketFilter<'a>>),
    TransmitRehash(TransmitRehash),
    MinimumHopLimit {
        version: IpVersion,
        minimum: Option<HopLimit>,
    },
    RecordsLastReceivedHeader(Report),
    IcmpErrorReports {
        version: IpVersion,
        reports: Report,
    },
    KeepAlive(bool),
    KeepAliveIdle(Duration),
    KeepAliveInterval(Duration),
    KeepAliveProbes(ProbeCount),
    NoDelay(bool),
    Cork(bool),
    UrgentInline(bool),
    QuickAcknowledgement(TcpQuickAcknowledgement),
    MaximumSegmentSize(Option<MaximumSegmentSize>),
    WindowClamp(Option<WindowSize>),
    SynchronizeRetries(RetryCount),
    UserTimeout(Option<Duration>),
    DeferAccept(Option<Duration>),
    FinWait2(TcpFinWait2),
    RetransmissionTimeoutMinimum(Duration),
    RetransmissionTimeoutMaximum(Duration),
    DelayedAcknowledgementMaximum(Duration),
    TransmitDelay(Duration),
    ThinLinearTimeouts(bool),
    CongestionControl(&'a str),
    NotSentLowWater(Option<ByteCount>),
    FastOpenWithoutCookie(bool),
    UnreadByteReports(Report),
    Md5Key(TcpMd5KeyChange<'a>),
    AddAuthenticationKey(TcpAuthenticationKeyAddition<'a>),
    RemoveAuthenticationKey(TcpAuthenticationKeyRemoval),
    AuthenticationOptions(TcpAuthenticationOptions),
    /// The [TCP Authentication Option](https://www.rfc-editor.org/rfc/rfc5925) counters the socket counts on from when it connects or listens, which can be set only before it does.
    InitialAuthenticationCounters(TcpAuthenticationCounters),
    Repair(TcpRepair),
    RepairQueue(TcpRepairQueue),
    QueueSequence(TcpSequenceNumber),
    RepairOptions(&'a [TcpRepairOption]),
    RepairWindow(TcpRepairWindow),
    AuthenticationRepair(TcpAuthenticationRepair),
}

/// A setting of a listening socket, its own or one it holds for itself and every connection it accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TcpListenerSetting<'a> {
    Socket(TcpSetting<'a>),
    Backlog(ListenBacklog),
    FastOpenQueue(Option<NonZeroU32>),
    FastOpenKeys(FastOpenKeys),
    SaveSynchronize(TcpSavedSynchronize),
    FlowLabelReflection(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferTuning {
    Automatic,
    Locked,
}

/// The `SO_BUF_LOCK` of a socket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferLocks {
    pub send: BufferTuning,
    pub receive: BufferTuning,
}

/// A `SO_TXREHASH` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransmitRehash {
    SystemDefault,
    Disabled,
    Enabled,
}

/// A [`TCP_QUICKACK`](https://man7.org/linux/man-pages/man7/tcp.7.html) mode, which the connection's own acknowledging later switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpQuickAcknowledgement {
    Off,
    On,
}

/// A [`TCP_LINGER2`](https://man7.org/linux/man-pages/man7/tcp.7.html) value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpFinWait2 {
    SystemDefault,
    Lasts(Duration),
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FastOpenKey(pub [u8; FAST_OPEN_KEY_LENGTH]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FastOpenKeys {
    pub primary: FastOpenKey,
    pub backup: Option<FastOpenKey>,
}

/// A [`TCP_SAVE_SYN`](https://man7.org/linux/man-pages/man7/tcp.7.html) value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpSavedSynchronize {
    Off,
    FromNetworkHeader,
    FromLinkHeader,
}
