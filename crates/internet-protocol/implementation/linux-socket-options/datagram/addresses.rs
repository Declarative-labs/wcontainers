//! Socket addresses between the standard library's and the uapi's forms.

use core::mem::{MaybeUninit, size_of};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::fd::BorrowedFd;

use libc::socklen_t;
use rustix::net::SocketAddrAny;
use rustix::net::addr::SocketAddrStorage;
use wcontainers_abi::HostError;

use crate::host_error;

pub fn local_address(socket: BorrowedFd<'_>) -> Result<SocketAddr, HostError> {
    let address = rustix::net::getsockname(socket).map_err(host_error)?;
    SocketAddr::try_from(address).map_err(|error| HostError::Errno(error.raw_os_error()))
}

pub fn as_bytes<T: Copy>(value: &T) -> &[u8] {
    // SAFETY: `T` is plain data of the uapi, read as its bytes for its own size.
    unsafe { core::slice::from_raw_parts((value as *const T).cast::<u8>(), size_of::<T>()) }
}

pub fn zeroed<T: Copy>() -> T {
    // SAFETY: only used for the uapi's plain data structures, for which all zero bytes are valid.
    unsafe { MaybeUninit::<T>::zeroed().assume_init() }
}

/// The address the kernel wrote into a `recvmsg` name buffer, if it wrote one.
pub fn read_address(
    storage: &MaybeUninit<SocketAddrStorage>,
    length: socklen_t,
) -> Option<SocketAddr> {
    if length == 0 {
        return None;
    }
    // SAFETY: the kernel initialised `length` bytes of the storage with a socket address.
    let any = unsafe { SocketAddrAny::read(storage.as_ptr(), length) };
    SocketAddr::try_from(any).ok().map(unmapped)
}

/// An IPv4-mapped IPv6 socket address as the IPv4 one it carries.
pub fn unmapped(address: SocketAddr) -> SocketAddr {
    match address {
        SocketAddr::V6(v6) => match v6.ip().to_ipv4_mapped() {
            Some(v4) => SocketAddr::V4(SocketAddrV4::new(v4, v6.port())),
            None => address,
        },
        v4 => v4,
    }
}

/// An address in the form a socket of the given family takes.
pub fn for_family(address: SocketAddr, ipv6_socket: bool) -> SocketAddr {
    match address {
        SocketAddr::V4(v4) if ipv6_socket => SocketAddr::V6(SocketAddrV6::new(
            v4.ip().to_ipv6_mapped(),
            v4.port(),
            u32::MIN,
            u32::MIN,
        )),
        other => other,
    }
}

pub fn in_addr(address: Ipv4Addr) -> linux_raw_sys::net::in_addr {
    linux_raw_sys::net::in_addr {
        s_addr: u32::from(address).to_be(),
    }
}

pub fn in6_addr(address: Ipv6Addr) -> linux_raw_sys::net::in6_addr {
    let mut value = zeroed::<linux_raw_sys::net::in6_addr>();
    value.in6_u.u6_addr8 = address.octets();
    value
}

pub fn from_in_addr(address: linux_raw_sys::net::in_addr) -> Ipv4Addr {
    Ipv4Addr::from(u32::from_be(address.s_addr))
}

pub fn from_in6_addr(address: linux_raw_sys::net::in6_addr) -> Ipv6Addr {
    // SAFETY: every byte pattern of the union is a valid array of bytes.
    Ipv6Addr::from(unsafe { address.in6_u.u6_addr8 })
}

/// A socket address in the kernel's storage form, as group requests carry it.
pub fn storage(address: IpAddr) -> linux_raw_sys::net::__kernel_sockaddr_storage {
    let mut value = zeroed::<linux_raw_sys::net::__kernel_sockaddr_storage>();
    let any = SocketAddr::new(address, u16::MIN);
    // SAFETY: rustix gives a pointer to `length` bytes of a socket address, at most the storage's size.
    unsafe {
        rustix::net::addr::SocketAddrArg::with_sockaddr(&any, |pointer, length| {
            core::ptr::copy_nonoverlapping(
                pointer.cast::<u8>(),
                (&mut value as *mut linux_raw_sys::net::__kernel_sockaddr_storage).cast::<u8>(),
                length as usize,
            );
        });
    }
    value
}
