//! The options and requests that neither linux-raw-sys nor the uapi names, with where Linux defines each.
//!
//! The generator test compiles this same file, so a name of `options.table` resolves to the number the methods use.

/// The multipath TCP option that reads the connection's `struct mptcp_info`.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
pub(crate) const MPTCP_INFO: u32 = 1;
/// The multipath TCP option that reads the connection's and its subflows' information at once.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
pub(crate) const MPTCP_FULL_INFO: u32 = 4;
/// The UDP option that sends IPv6 datagrams with a zero checksum.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub(crate) const UDP_NO_CHECK6_TX: u32 = libc::UDP_NO_CHECK6_TX as u32;
/// The UDP option that takes IPv6 datagrams with a zero checksum.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub(crate) const UDP_NO_CHECK6_RX: u32 = libc::UDP_NO_CHECK6_RX as u32;
/// The level of UDP-Lite's own options, its protocol number.
///
/// https://man7.org/linux/man-pages/man7/udplite.7.html
pub(crate) const SOL_UDPLITE: u32 = linux_raw_sys::net::SOL_UDPLITE;
/// The UDP-Lite option that sets how much of each datagram sent the checksum covers.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub(crate) const UDPLITE_SEND_CSCOV: u32 = 10;
/// The UDP-Lite option that sets how much of each datagram taken the checksum must cover.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
pub(crate) const UDPLITE_RECV_CSCOV: u32 = 11;
/// The request for the bytes sent and not yet acknowledged, which Linux defines as `TIOCOUTQ`.
///
/// https://man7.org/linux/man-pages/man7/tcp.7.html
pub(crate) const SIOCOUTQ: u32 = linux_raw_sys::ioctl::TIOCOUTQ;
/// The request for the bytes not yet sent.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/sockios.h
pub(crate) const SIOCOUTQNSD: u32 = libc::SIOCOUTQNSD as u32;
/// The request for whether the urgent mark is next to read.
///
/// https://man7.org/linux/man-pages/man7/tcp.7.html
pub(crate) const SIOCATMARK: u32 = linux_raw_sys::ioctl::SIOCATMARK;
