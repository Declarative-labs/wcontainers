use crate::authentication::TcpAuthenticationRepair;
use crate::settings::{MaximumSegmentSize, TcpSequenceNumber, WindowScale, WindowSize};

/// A `TCP_REPAIR` mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpRepair {
    Off,
    OffWithoutWindowProbe,
    On,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpRepairQueue {
    NoQueue,
    Receive,
    Send,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpRepairOption {
    MaximumSegmentSize(MaximumSegmentSize),
    WindowScales(TcpWindowScales),
    SelectiveAcknowledgements,
    Timestamps,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpWindowScales {
    pub send: WindowScale,
    pub receive: WindowScale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpRepairWindow {
    pub send_update_sequence: TcpSequenceNumber,
    pub send_window: WindowSize,
    pub maximum_window: WindowSize,
    pub receive_window: WindowSize,
    pub receive_update_sequence: TcpSequenceNumber,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpTimestamp(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpTimestampUnit {
    Milliseconds,
    Microseconds,
}

/// A `TCP_TIMESTAMP` value, its low bit given as the unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpTimestampClock {
    pub value: TcpTimestamp,
    pub unit: TcpTimestampUnit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpRepairState {
    pub send_sequence: TcpSequenceNumber,
    pub receive_sequence: TcpSequenceNumber,
    pub window: TcpRepairWindow,
    pub authentication: Option<TcpAuthenticationRepair>,
}
