//! The options and requests linux-raw-sys does not name, as Linux defines them.
//!
//! The direct crate and its generator test compile this one file, so a table name means one number in both.

/// The MPTCP option that reads `struct mptcp_info`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
pub(crate) const MPTCP_INFO: u32 = 1;
/// The MPTCP option that reads the connection's and its subflows' information.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
pub(crate) const MPTCP_FULL_INFO: u32 = 4;
/// The UDP option that sends IPv6 datagrams with a zero checksum.
pub(crate) const UDP_NO_CHECK6_TX: u32 = libc::UDP_NO_CHECK6_TX as u32;
/// The UDP option that takes IPv6 datagrams with a zero checksum.
pub(crate) const UDP_NO_CHECK6_RX: u32 = libc::UDP_NO_CHECK6_RX as u32;
/// The level of UDP-Lite's options, its protocol number.
pub(crate) const SOL_UDPLITE: u32 = linux_raw_sys::net::SOL_UDPLITE;
/// The UDP-Lite option for how much of a datagram sent the checksum covers.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub(crate) const UDPLITE_SEND_CSCOV: u32 = 10;
/// The UDP-Lite option for how much of a datagram taken the checksum must cover.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub(crate) const UDPLITE_RECV_CSCOV: u32 = 11;
/// The request for the bytes sent and not yet acknowledged, Linux's `TIOCOUTQ`.
pub(crate) const SIOCOUTQ: u32 = linux_raw_sys::ioctl::TIOCOUTQ;
/// The request for the bytes not yet sent.
pub(crate) const SIOCOUTQNSD: u32 = libc::SIOCOUTQNSD as u32;
/// The request for whether the urgent mark is next to read.
pub(crate) const SIOCATMARK: u32 = linux_raw_sys::ioctl::SIOCATMARK;
