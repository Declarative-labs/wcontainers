//! Multicast group and anycast address memberships of a socket.

use core::mem::{offset_of, size_of};
use std::net::{IpAddr, Ipv6Addr};
use std::os::fd::BorrowedFd;

use libc::c_int;
use linux_raw_sys::net as uapi;
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    AnycastAddress, Membership, MulticastGroup, MulticastMembership, SourceFilter,
};

use crate::datagram::addresses::{in6_addr, storage, zeroed};
use crate::{set_bytes, set_option};

/// The anycast addresses a socket joined, which Linux counts per join and cannot be asked back.
#[derive(Default)]
pub struct AnycastMemberships(Vec<(Ipv6Addr, u32)>);

impl AnycastMemberships {
    /// Joins or leaves an anycast address, doing nothing when the socket already is as asked.
    pub fn set(&mut self, fd: BorrowedFd<'_>, anycast: &AnycastAddress) -> Result<(), HostError> {
        let interface = anycast.interface.map_or(u32::MIN, |index| index.0);
        let key = (anycast.address, interface);
        let joined = self.0.contains(&key);
        let mut request = zeroed::<uapi::ipv6_mreq>();
        request.ipv6mr_multiaddr = in6_addr(anycast.address);
        request.ipv6mr_ifindex = interface as c_int;
        match anycast.membership {
            Membership::Joined if !joined => {
                set_option(fd, uapi::SOL_IPV6, uapi::IPV6_JOIN_ANYCAST, &request)?;
                self.0.push(key);
            }
            Membership::Left if joined => {
                set_option(fd, uapi::SOL_IPV6, uapi::IPV6_LEAVE_ANYCAST, &request)?;
                self.0.retain(|joined| *joined != key);
            }
            _ => {}
        }
        Ok(())
    }
}

/// Joins a group and writes its whole source filter, or leaves it.
pub fn set_multicast_group<A: Copy>(
    fd: BorrowedFd<'_>,
    level: u32,
    group: IpAddr,
    setting: &MulticastGroup<'_, A>,
    address: impl Fn(&A) -> IpAddr,
) -> Result<(), HostError> {
    let interface = setting.interface.map_or(u32::MIN, |index| index.0);
    let mut request = zeroed::<uapi::group_req>();
    request.gr_interface = interface;
    request.gr_group = storage(group);
    let filter = match setting.membership {
        MulticastMembership::Left => {
            return match set_option(fd, level, uapi::MCAST_LEAVE_GROUP, &request) {
                Err(HostError::Errno(libc::EADDRNOTAVAIL)) => Ok(()),
                other => other,
            };
        }
        MulticastMembership::Joined(filter) => filter,
    };
    match set_option(fd, level, uapi::MCAST_JOIN_GROUP, &request) {
        Ok(()) | Err(HostError::Errno(libc::EADDRINUSE)) => {}
        Err(error) => return Err(error),
    }
    let (mode, sources) = match filter {
        SourceFilter::Include(sources) => (uapi::MCAST_INCLUDE, sources),
        SourceFilter::Exclude(sources) => (uapi::MCAST_EXCLUDE, sources),
    };
    type Header = uapi::group_filter__bindgen_ty_1__bindgen_ty_2;
    type Storage = uapi::__kernel_sockaddr_storage;
    let start = offset_of!(Header, gf_slist_flex);
    let length =
        (start + sources.len() * size_of::<Storage>()).max(size_of::<uapi::group_filter>());
    let mut words = vec![u64::MIN; length.div_ceil(size_of::<u64>())];
    // SAFETY: the buffer is large enough and aligned for the header and its sources.
    unsafe {
        let header = words.as_mut_ptr().cast::<Header>();
        (*header).gf_interface = interface;
        (*header).gf_group = storage(group);
        (*header).gf_fmode = mode;
        (*header).gf_numsrc = sources.len() as u32;
        let list = words.as_mut_ptr().cast::<u8>().add(start).cast::<Storage>();
        for (index, source) in sources.iter().enumerate() {
            list.add(index).write_unaligned(storage(address(source)));
        }
    }
    // SAFETY: the words are plain bytes of the filter.
    let bytes = unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u8>(), length) };
    set_bytes(fd, level, uapi::MCAST_MSFILTER, bytes)
}
