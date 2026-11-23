//! Multicast group and anycast address memberships of a socket.

use core::mem::{offset_of, size_of};
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::os::fd::{AsFd, BorrowedFd};

use libc::c_int;
use linux_raw_sys::net as uapi;
use socket2::{Domain, Socket, Type};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    AnycastAddress, Membership, MulticastGroup, MulticastMembership, SourceFilter,
};
use zerocopy::IntoBytes;

use crate::datagram::addresses::{StorageBytes, in6_addr, storage, storage_bytes};
use crate::{io_host_error, set_bytes, set_option};

/// The anycast addresses a socket joined, which Linux counts per join and cannot be asked back.
#[derive(Default)]
pub struct AnycastMemberships(Vec<(Ipv6Addr, u32)>);

impl AnycastMemberships {
    /// Joins or leaves an anycast address, doing nothing when the socket already is as asked.
    pub fn set(&mut self, fd: BorrowedFd<'_>, anycast: &AnycastAddress) -> Result<(), HostError> {
        let interface = anycast.interface.map_or(u32::MIN, |index| index.0.get());
        let key = (anycast.address, interface);
        let joined = self.0.contains(&key);
        let request = uapi::ipv6_mreq {
            ipv6mr_multiaddr: in6_addr(anycast.address),
            ipv6mr_ifindex: interface as c_int,
        };
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
    let mut request = uapi::group_req {
        gr_interface: u32::MIN,
        gr_group: storage(group),
    };
    set_option(probe.as_fd(), level, uapi::MCAST_JOIN_GROUP, &request)?;
    let interfaces =
        nix::net::if_::if_nameindex().map_err(|error| HostError::Errno(error as i32))?;
    for interface in &interfaces {
        request.gr_interface = interface.index();
        if set_option(probe.as_fd(), level, uapi::MCAST_LEAVE_GROUP, &request).is_ok() {
            return Ok(interface.index());
        }
    }
    Err(HostError::Errno(libc::EADDRNOTAVAIL))
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
    let mut request = uapi::group_req {
        gr_interface: u32::MIN,
        gr_group: storage(group),
    };
    let filter = match setting.membership {
        MulticastMembership::Left => {
            request.gr_interface = setting.interface.map_or(u32::MIN, |index| index.0.get());
            return match set_option(fd, level, uapi::MCAST_LEAVE_GROUP, &request) {
                Err(HostError::Errno(libc::EADDRNOTAVAIL)) => Ok(()),
                other => other,
            };
        }
        MulticastMembership::Joined(filter) => filter,
    };
    let interface = match setting.interface {
        Some(index) => index.0.get(),
        None => chosen_interface(level, group)?,
    };
    request.gr_interface = interface;
    match set_option(fd, level, uapi::MCAST_JOIN_GROUP, &request) {
        Ok(()) | Err(HostError::Errno(libc::EADDRINUSE)) => {}
        Err(error) => return Err(error),
    }
    let (mode, sources) = match filter {
        SourceFilter::Include(sources) => (uapi::MCAST_INCLUDE, sources.as_slice()),
        SourceFilter::Exclude(sources) => (uapi::MCAST_EXCLUDE, sources),
    };
    type Header = uapi::group_filter__bindgen_ty_1__bindgen_ty_2;
    let start = offset_of!(Header, gf_slist_flex);
    let room = size_of::<StorageBytes>();
    let length = (start + sources.len() * room).max(size_of::<uapi::group_filter>());
    let mut bytes = vec![u8::MIN; length];
    let mut put = |at: usize, value: &[u8]| bytes[at..at + value.len()].copy_from_slice(value);
    put(offset_of!(Header, gf_interface), &interface.to_ne_bytes());
    put(
        offset_of!(Header, gf_group),
        storage_bytes(SocketAddr::new(group, u16::MIN)).as_bytes(),
    );
    put(offset_of!(Header, gf_fmode), &mode.to_ne_bytes());
    put(
        offset_of!(Header, gf_numsrc),
        &(sources.len() as u32).to_ne_bytes(),
    );
    for (index, source) in sources.iter().enumerate() {
        let source = storage_bytes(SocketAddr::new(address(source), u16::MIN));
        put(start + index * room, source.as_bytes());
    }
    set_bytes(fd, level, uapi::MCAST_MSFILTER, &bytes)
}
