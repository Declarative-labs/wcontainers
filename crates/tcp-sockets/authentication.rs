use wcontainers_internet_protocol::{InterfaceIndex, IpPrefix};

use crate::settings::{SegmentCount, TcpSequenceNumber};

/// The [VRF](https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/tcp.h#L399) whose connections a key is for: any, only the default one, or the one whose device is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpKeyVrf {
    Any,
    Default,
    Device(InterfaceIndex),
}

/// The peer of a [TCP MD5 signature](https://www.rfc-editor.org/rfc/rfc2385) key, `TCP_MD5SIG_EXT`'s prefix and VRF.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpMd5Peer {
    pub prefix: IpPrefix,
    pub vrf: TcpKeyVrf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpMd5KeyChange<'a> {
    Set { peer: TcpMd5Peer, key: &'a [u8] },
    Removed(TcpMd5Peer),
}

/// A key identifier of the [TCP Authentication Option](https://www.rfc-editor.org/rfc/rfc5925).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpAuthenticationKeyIdentifier(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpAuthenticationMacLength(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpAuthenticationKeyIdentity {
    pub peer: IpPrefix,
    pub vrf: TcpKeyVrf,
    pub send_identifier: TcpAuthenticationKeyIdentifier,
    pub receive_identifier: TcpAuthenticationKeyIdentifier,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpAuthenticationOptionCoverage {
    Included,
    Excluded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpAuthenticationKeyUse {
    Stored,
    Current,
    RequestedNext,
    CurrentAndRequestedNext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpAuthenticationKeyAddition<'a> {
    pub identity: TcpAuthenticationKeyIdentity,
    pub algorithm: &'a str,
    pub mac_length: TcpAuthenticationMacLength,
    pub other_options: TcpAuthenticationOptionCoverage,
    pub secret: &'a [u8],
    pub key_use: TcpAuthenticationKeyUse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpAuthenticationKeyRemoval {
    pub identity: TcpAuthenticationKeyIdentity,
    pub current: Option<TcpAuthenticationKeyIdentifier>,
    pub requested_next: Option<TcpAuthenticationKeyIdentifier>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpAuthenticationRequirement {
    Optional,
    Required,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcpAuthenticationIcmpErrors {
    Ignored,
    Accepted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpAuthenticationCounters {
    pub verified: SegmentCount,
    pub failed: SegmentCount,
    pub key_not_found: SegmentCount,
    pub signature_missing: SegmentCount,
    pub icmp_errors_ignored: SegmentCount,
}

/// The state of the [TCP Authentication Option](https://www.rfc-editor.org/rfc/rfc5925) that `TCP_AO_INFO` sets, each `None` leaving what is there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpAuthenticationOptions {
    pub current: Option<TcpAuthenticationKeyIdentifier>,
    pub requested_next: Option<TcpAuthenticationKeyIdentifier>,
    pub requirement: TcpAuthenticationRequirement,
    pub icmp_errors: TcpAuthenticationIcmpErrors,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpAuthenticationStatus {
    pub counters: TcpAuthenticationCounters,
    pub current: Option<TcpAuthenticationKeyIdentifier>,
    pub requested_next: Option<TcpAuthenticationKeyIdentifier>,
    pub requirement: TcpAuthenticationRequirement,
    pub icmp_errors: TcpAuthenticationIcmpErrors,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpAuthenticationKeyCounters {
    pub verified: SegmentCount,
    pub failed: SegmentCount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SequenceNumberExtension(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpAuthenticationRepair {
    pub sent_initial_sequence: TcpSequenceNumber,
    pub received_initial_sequence: TcpSequenceNumber,
    pub send_extension: SequenceNumberExtension,
    pub receive_extension: SequenceNumberExtension,
}
