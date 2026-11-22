use core::net::SocketAddr;
use core::time::Duration;

use crate::settings::SegmentCount;
use crate::state::{ByteTotal, TcpInformation};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MultipathSubflowPosition(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubflowCount(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MultipathToken(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataSequenceNumber(pub u64);

/// The [`MPTCP_INFO`](https://docs.kernel.org/networking/mptcp.html#:~:text=MPTCP_INFO) of a multipath connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MultipathInformation {
    pub extra_subflows: SubflowCount,
    pub addresses_signalled: SubflowCount,
    pub addresses_accepted: SubflowCount,
    pub extra_subflows_limit: SubflowCount,
    pub signal_endpoints_limit: SubflowCount,
    pub accepted_addresses_limit: SubflowCount,
    pub fell_back_to_tcp: bool,
    pub remote_key_received: bool,
    pub token: MultipathToken,
    pub write_sequence: DataSequenceNumber,
    pub send_unacknowledged: DataSequenceNumber,
    pub receive_next: DataSequenceNumber,
    pub local_addresses_used: SubflowCount,
    pub subflow_endpoints_limit: SubflowCount,
    pub checksums: bool,
    pub retransmits: SegmentCount,
    pub bytes_retransmitted: ByteTotal,
    pub bytes_sent: ByteTotal,
    pub bytes_received: ByteTotal,
    pub bytes_acknowledged: ByteTotal,
    pub subflows_total: SubflowCount,
    pub laminar_endpoints_limit: SubflowCount,
    pub since_last_data_sent: Duration,
    pub since_last_data_received: Duration,
    pub since_last_acknowledgement_received: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MultipathSubflowIdentifier(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MultipathSubflow {
    pub identifier: MultipathSubflowIdentifier,
    pub local: SocketAddr,
    pub peer: SocketAddr,
    pub information: TcpInformation,
}
