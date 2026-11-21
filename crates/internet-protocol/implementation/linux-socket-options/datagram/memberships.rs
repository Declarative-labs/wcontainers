//! Multicast group and anycast address memberships of a socket.

use core::mem::{offset_of, size_of};
use std::net::{IpAddr, Ipv6Addr};
use std::os::fd::{AsFd, BorrowedFd};

use libc::c_int;
use linux_raw_sys::net as uapi;
use socket2::{Domain, Socket, Type};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    AnycastAddress, Membership, MulticastGroup, MulticastMembership, SourceFilter,
};

use crate::datagram::addresses::{in6_addr, storage, zeroed};
use crate::{io_host_error, last_host_error, set_bytes, set_option};

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

/// The interface Linux joins a group on when none is given, found by joining it on a socket of its own and
/// leaving it on each interface until one holds it.
fn chosen_interface(level: u32, group: IpAddr) -> Result<u32, HostError> {
    let domain = match group {
        IpAddr::V4(_) => Domain::IPV4,
        IpAddr::V6(_) => Domain::IPV6,
    };
    let probe = Socket::new(domain, Type::DGRAM, None).map_err(|error| io_host_error(&error))?;
    let mut request = zeroed::<uapi::group_req>();
    request.gr_group = storage(group);
    set_option(probe.as_fd(), level, uapi::MCAST_JOIN_GROUP, &request)?;
    // SAFETY: if_nameindex gives an array ended by an entry of index 0, or null, freed below.
    let interfaces = unsafe { libc::if_nameindex() };
    if interfaces.is_null() {
        return Err(last_host_error());
    }
    let mut chosen = Err(HostError::Errno(libc::EADDRNOTAVAIL));
    // SAFETY: the array is read up to its ending entry and freed once.
    unsafe {
        let mut entry = interfaces;
        while (*entry).if_index != u32::MIN {
            request.gr_interface = (*entry).if_index;
            if set_option(probe.as_fd(), level, uapi::MCAST_LEAVE_GROUP, &request).is_ok() {
                chosen = Ok((*entry).if_index);
                break;
            }
            entry = entry.add(1);
        }
        libc::if_freenameindex(interfaces);
    }
    chosen
}

/// Joins a group and writes its whole source filter, or leaves it. Linux writes a filter only for a membership
/// named by its interface, so a join on no interface given is made on the one Linux would choose.
pub fn set_multicast_group<A: Copy>(
    fd: BorrowedFd<'_>,
    level: u32,
    group: IpAddr,
    setting: &MulticastGroup<'_, A>,
    address: impl Fn(&A) -> IpAddr,
) -> Result<(), HostError> {
    let mut request = zeroed::<uapi::group_req>();
    request.gr_group = storage(group);
    let filter = match setting.membership {
        MulticastMembership::Left => {
            request.gr_interface = setting.interface.map_or(u32::MIN, |index| index.0);
            return match set_option(fd, level, uapi::MCAST_LEAVE_GROUP, &request) {
                Err(HostError::Errno(libc::EADDRNOTAVAIL)) => Ok(()),
                other => other,
            };
        }
        MulticastMembership::Joined(filter) => filter,
    };
    let interface = match setting.interface {
        Some(index) => index.0,
        None => chosen_interface(level, group)?,
    };
    request.gr_interface = interface;
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
