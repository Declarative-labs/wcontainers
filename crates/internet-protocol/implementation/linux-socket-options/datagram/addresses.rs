//! Socket addresses between the standard library's and the uapi's forms.

use core::mem::{align_of, offset_of, size_of};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::fd::BorrowedFd;

use libc::c_int;
use linux_raw_sys::net as uapi;
use wcontainers_abi::HostError;
use zerocopy::{FromBytes, FromZeros, IntoBytes};

use crate::{io_host_error, sock, uapi_mirror};

uapi_mirror! {
    /// `struct sockaddr_in`, its port and address in network order.
    #[derive(IntoBytes)]
    struct Ipv4SocketAddress mirrors uapi::sockaddr_in {
        sin_family: u16,
        sin_port: [u8; size_of::<uapi::__be16>()],
        sin_addr: [u8; size_of::<uapi::in_addr>()],
        __pad: [u8; size_of::<uapi::sockaddr_in>() - offset_of!(uapi::sockaddr_in, __pad)],
    }
}

uapi_mirror! {
    /// `struct sockaddr_in6`, its port, flow information and address in network order.
    #[derive(IntoBytes)]
    struct Ipv6SocketAddress mirrors uapi::sockaddr_in6 {
        sin6_family: u16,
        sin6_port: [u8; size_of::<uapi::__be16>()],
        sin6_flowinfo: [u8; size_of::<uapi::__be32>()],
        sin6_addr: [u8; size_of::<uapi::in6_addr>()],
        sin6_scope_id: u32,
    }
}

uapi_mirror! {
    /// `struct in_pktinfo`, its addresses in network order.
    #[derive(IntoBytes)]
    pub(crate) struct Ipv4PacketInformation mirrors uapi::in_pktinfo {
        pub(crate) ipi_ifindex: c_int,
        pub(crate) ipi_spec_dst: [u8; size_of::<uapi::in_addr>()],
        pub(crate) ipi_addr: [u8; size_of::<uapi::in_addr>()],
    }
}

uapi_mirror! {
    /// `struct in6_pktinfo`, its address in network order.
    #[derive(IntoBytes)]
    pub(crate) struct Ipv6PacketInformation mirrors uapi::in6_pktinfo {
        pub(crate) ipi6_addr: [u8; size_of::<uapi::in6_addr>()],
        pub(crate) ipi6_ifindex: c_int,
    }
}

/// The bytes of a `__kernel_sockaddr_storage`, aligned as it is, as Linux's socket option calls take them.
pub use wcontainers_linux_socket_options::SocketStorage as StorageBytes;

const _: () = assert!(align_of::<StorageBytes>() == align_of::<uapi::__kernel_sockaddr_storage>());

/// A socket address in the kernel's storage form, as bytes.
pub fn storage_bytes(address: SocketAddr) -> StorageBytes {
    let mut storage = StorageBytes::new_zeroed();
    let mut write = |address: &[u8]| storage.bytes[..address.len()].copy_from_slice(address);
    match address {
        SocketAddr::V4(v4) => write(
            Ipv4SocketAddress {
                sin_family: uapi::AF_INET as u16,
                sin_port: v4.port().to_be_bytes(),
                sin_addr: v4.ip().octets(),
                __pad: Default::default(),
            }
            .as_bytes(),
        ),
        SocketAddr::V6(v6) => write(
            Ipv6SocketAddress {
                sin6_family: uapi::AF_INET6 as u16,
                sin6_port: v6.port().to_be_bytes(),
                sin6_flowinfo: v6.flowinfo().to_be_bytes(),
                sin6_addr: v6.ip().octets(),
                sin6_scope_id: v6.scope_id(),
            }
            .as_bytes(),
        ),
    }
    storage
}

/// The IPv4 or IPv6 socket address at the start of bytes in the kernel's socket address forms, IPv4-mapped ones as IPv4.
pub fn address_of_storage(bytes: &[u8]) -> Option<SocketAddr> {
    let (family, _) = u16::read_from_prefix(bytes).ok()?;
    let address = match u32::from(family) {
        uapi::AF_INET => {
            let (address, _) = Ipv4SocketAddress::read_from_prefix(bytes).ok()?;
            SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::from(address.sin_addr),
                u16::from_be_bytes(address.sin_port),
            ))
        }
        uapi::AF_INET6 => {
            let (address, _) = Ipv6SocketAddress::read_from_prefix(bytes).ok()?;
            SocketAddr::V6(SocketAddrV6::new(
                Ipv6Addr::from(address.sin6_addr),
                u16::from_be_bytes(address.sin6_port),
                u32::from_be_bytes(address.sin6_flowinfo),
                address.sin6_scope_id,
            ))
        }
        _ => return None,
    };
    Some(unmapped(address))
}

/// The kernel's storage form of a socket address from its bytes.
pub fn kernel_storage(storage: &StorageBytes) -> uapi::__kernel_sockaddr_storage {
    let (family, data) = storage.bytes.split_at(size_of::<u16>());
    uapi::__kernel_sockaddr_storage {
        __bindgen_anon_1: uapi::__kernel_sockaddr_storage__bindgen_ty_1 {
            __bindgen_anon_1: uapi::__kernel_sockaddr_storage__bindgen_ty_1__bindgen_ty_1 {
                ss_family: u16::from_ne_bytes([family[0], family[1]]),
                __data: core::array::from_fn(|index| data[index] as libc::c_char),
            },
        },
    }
}

pub fn local_address(socket: BorrowedFd<'_>) -> Result<SocketAddr, HostError> {
    sock(&socket)
        .local_addr()
        .map_err(|error| io_host_error(&error))?
        .as_socket()
        .ok_or(HostError::Errno(libc::EAFNOSUPPORT))
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

pub fn in6_addr(address: Ipv6Addr) -> uapi::in6_addr {
    uapi::in6_addr {
        in6_u: uapi::in6_addr__bindgen_ty_1 {
            u6_addr8: address.octets(),
        },
    }
}

pub fn from_in_addr(address: linux_raw_sys::net::in_addr) -> Ipv4Addr {
    Ipv4Addr::from(u32::from_be(address.s_addr))
}

/// A socket address in the kernel's storage form, as group requests carry it.
pub fn storage(address: IpAddr) -> uapi::__kernel_sockaddr_storage {
    kernel_storage(&storage_bytes(SocketAddr::new(address, u16::MIN)))
}
