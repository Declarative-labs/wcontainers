//! Multicast group and anycast address memberships of a socket.

use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::os::fd::{AsFd, BorrowedFd};

use libc::c_int;
use linux_raw_sys::net as uapi;
use socket2::{Domain, Socket, Type};
use wcontainers_abi::HostError;
use wcontainers_internet_protocol::{
    AnycastAddress, Membership, MulticastGroup, MulticastMembership, SourceFilter,
};
use wcontainers_linux_socket_options::{GroupFilter, LinuxSocketOptions};

use crate::datagram::addresses::{StorageBytes, in6_addr, storage, storage_bytes};
use crate::{LINUX, io_host_error};

/// Joins a group at the level of its IP version, `MCAST_JOIN_GROUP` of `IPPROTO_IP` or `IPPROTO_IPV6`.
fn join_group(fd: BorrowedFd<'_>, level: u32, request: &uapi::group_req) -> Result<(), HostError> {
    if level == uapi::SOL_IPV6 {
        LINUX.set_ipv6_mcast_join_group(fd, request)
    } else {
        LINUX.set_ip_mcast_join_group(fd, request)
    }
}

/// Leaves a group at the level of its IP version, `MCAST_LEAVE_GROUP`.
fn leave_group(fd: BorrowedFd<'_>, level: u32, request: &uapi::group_req) -> Result<(), HostError> {
    if level == uapi::SOL_IPV6 {
        LINUX.set_ipv6_mcast_leave_group(fd, request)
    } else {
        LINUX.set_ip_mcast_leave_group(fd, request)
    }
}

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
                LINUX.set_ipv6_join_anycast(fd, &request)?;
                self.0.push(key);
            }
            Membership::Left if joined => {
                LINUX.set_ipv6_leave_anycast(fd, &request)?;
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
    join_group(probe.as_fd(), level, &request)?;
    let interfaces =
        nix::net::if_::if_nameindex().map_err(|error| HostError::Errno(error as i32))?;
    for interface in &interfaces {
        request.gr_interface = interface.index();
        if leave_group(probe.as_fd(), level, &request).is_ok() {
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
            return match leave_group(fd, level, &request) {
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
    match join_group(fd, level, &request) {
        Ok(()) | Err(HostError::Errno(libc::EADDRINUSE)) => {}
        Err(error) => return Err(error),
    }
    let (mode, sources) = match filter {
        SourceFilter::Include(sources) => (uapi::MCAST_INCLUDE, sources.as_slice()),
        SourceFilter::Exclude(sources) => (uapi::MCAST_EXCLUDE, sources),
    };
    let sources: Vec<StorageBytes> = sources
        .iter()
        .map(|source| storage_bytes(SocketAddr::new(address(source), u16::MIN)))
        .collect();
    let filter = GroupFilter {
        interface,
        group: storage_bytes(SocketAddr::new(group, u16::MIN)),
        mode,
        sources: &sources,
    };
    if level == uapi::SOL_IPV6 {
        LINUX.set_ipv6_mcast_msfilter(fd, &filter)
    } else {
        LINUX.set_ip_mcast_msfilter(fd, &filter)
    }
}
