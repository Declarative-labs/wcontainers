//! Options and requests linux-raw-sys does not name; the direct crate and the generator compile this one file.

// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
pub(crate) const MPTCP_INFO: u32 = 1;
// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
pub(crate) const MPTCP_FULL_INFO: u32 = 4;
pub(crate) const UDP_NO_CHECK6_TX: u32 = libc::UDP_NO_CHECK6_TX as u32;
pub(crate) const UDP_NO_CHECK6_RX: u32 = libc::UDP_NO_CHECK6_RX as u32;
pub(crate) const SOL_UDPLITE: u32 = linux_raw_sys::net::SOL_UDPLITE;
// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub(crate) const UDPLITE_SEND_CSCOV: u32 = 10;
// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub(crate) const UDPLITE_RECV_CSCOV: u32 = 11;
/// Linux's `TIOCOUTQ`, the bytes sent and not yet acknowledged.
pub(crate) const SIOCOUTQ: u32 = linux_raw_sys::ioctl::TIOCOUTQ;
pub(crate) const SIOCOUTQNSD: u32 = libc::SIOCOUTQNSD as u32;
pub(crate) const SIOCATMARK: u32 = linux_raw_sys::ioctl::SIOCATMARK;
