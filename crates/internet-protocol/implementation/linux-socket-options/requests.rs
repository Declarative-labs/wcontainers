//! The uapi structures the IP options take, as copies zerocopy gives the bytes of.

use std::net::{IpAddr, Ipv6Addr, SocketAddr};

use libc::c_int;
use linux_raw_sys::net as uapi;

use crate::datagram::addresses::{StorageBytes, storage_bytes};
use crate::uapi_mirror;

uapi_mirror! {
    /// `struct in6_flowlabel_req`, as `IPV6_FLOWLABEL_MGR` takes it.
    #[derive(zerocopy::IntoBytes)]
    pub struct FlowLabelRequest mirrors uapi::in6_flowlabel_req {
        pub flr_dst: [u8; size_of::<Ipv6Addr>()],
        pub flr_label: u32,
        pub flr_action: u8,
        pub flr_share: u8,
        pub flr_flags: u16,
        pub flr_expires: u16,
        pub flr_linger: u16,
        pub __flr_pad: u32,
    }
}

impl FlowLabelRequest {
    /// A request for the label in network order to the destination, with no expiry or linger of its own.
    pub fn new(destination: Ipv6Addr, label: u32, action: u32, share: u32, flags: u32) -> Self {
        Self {
            flr_dst: destination.octets(),
            flr_label: label,
            flr_action: action as u8,
            flr_share: share as u8,
            flr_flags: flags as u16,
            flr_expires: u16::MIN,
            flr_linger: u16::MIN,
            __flr_pad: u32::MIN,
        }
    }
}

uapi_mirror! {
    /// `struct ipv6_mreq`, as `IPV6_JOIN_ANYCAST` and `IPV6_LEAVE_ANYCAST` take it.
    #[derive(zerocopy::IntoBytes)]
    pub struct AnycastRequest mirrors uapi::ipv6_mreq {
        pub ipv6mr_multiaddr: [u8; size_of::<Ipv6Addr>()],
        pub ipv6mr_ifindex: c_int,
    }
}

uapi_mirror! {
    /// `struct group_req`, as `MCAST_JOIN_GROUP` and `MCAST_LEAVE_GROUP` take it: the interface's index, then the
    /// padding before the aligned address as a second word of zero.
    #[derive(zerocopy::IntoBytes)]
    pub struct GroupRequest mirrors uapi::group_req {
        pub gr_interface: [u32; core::mem::offset_of!(uapi::group_req, gr_group) / size_of::<u32>()],
        pub gr_group: StorageBytes,
    }
}

impl GroupRequest {
    pub fn new(interface: u32, group: IpAddr) -> Self {
        let mut gr_interface =
            [u32::MIN; core::mem::offset_of!(uapi::group_req, gr_group) / size_of::<u32>()];
        gr_interface[usize::MIN] = interface;
        Self {
            gr_interface,
            gr_group: storage_bytes(SocketAddr::new(group, u16::MIN)),
        }
    }
}

/// The bytes of a bindgen bit field unit, each bit where bindgen's accessors put it.
pub fn bit_field_bytes<const BYTES: usize>(
    unit: &uapi::__BindgenBitfieldUnit<[u8; BYTES]>,
) -> [u8; BYTES] {
    let bits = u8::BITS as usize;
    let mut bytes = [u8::MIN; BYTES];
    for index in 0..BYTES * bits {
        let place = if cfg!(target_endian = "big") {
            bits - 1 - index % bits
        } else {
            index % bits
        };
        bytes[index / bits] |= u8::from(unit.get_bit(index)) << place;
    }
    bytes
}
