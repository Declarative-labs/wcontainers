//! Linux's socket option and socket ioctl calls made on the kernel the program runs on.
//!
//! Every `unsafe` of the host socket crates is here, each use marked with what makes it sound.
#![cfg(target_os = "linux")]

mod generated;

use std::ffi::{c_int, c_void};
use std::mem::{offset_of, size_of, size_of_val};
use std::os::fd::{AsRawFd, BorrowedFd};

use linux_raw_sys::net as uapi;
use wcontainers_abi::HostError;
use wcontainers_linux_socket_options::{
    GroupFilter, KEY_QUERY_LENGTH, MPTCP_INFO_LENGTH, MptcpFullInformation, SOCKET_STORAGE_LENGTH,
    Written,
};
use zerocopy::{FromBytes, FromZeros, Immutable, IntoBytes, KnownLayout};

/// The socket option and ioctl calls of the kernel the program runs on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirectLinuxSocketOptions;

/// The options and requests that neither linux-raw-sys nor the uapi names, with where Linux defines each.
mod local {
    /// `MPTCP_INFO`.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
    pub const MPTCP_INFO: u32 = 1;
    /// `MPTCP_FULL_INFO`.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
    pub const MPTCP_FULL_INFO: u32 = 4;
    /// `UDP_NO_CHECK6_TX`.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
    pub const UDP_NO_CHECK6_TX: u32 = libc::UDP_NO_CHECK6_TX as u32;
    /// `UDP_NO_CHECK6_RX`.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
    pub const UDP_NO_CHECK6_RX: u32 = libc::UDP_NO_CHECK6_RX as u32;
    /// The level of UDP-Lite's own options, its protocol number.
    ///
    /// https://man7.org/linux/man-pages/man7/udplite.7.html
    pub const SOL_UDPLITE: u32 = super::uapi::IPPROTO_UDPLITE as u32;
    /// `UDPLITE_SEND_CSCOV`.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
    pub const UDPLITE_SEND_CSCOV: u32 = 10;
    /// `UDPLITE_RECV_CSCOV`.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
    pub const UDPLITE_RECV_CSCOV: u32 = 11;
    /// `SIOCOUTQ`, which Linux defines as `TIOCOUTQ`.
    ///
    /// https://man7.org/linux/man-pages/man7/tcp.7.html
    pub const SIOCOUTQ: u32 = linux_raw_sys::ioctl::TIOCOUTQ;
    /// `SIOCOUTQNSD`.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/sockios.h
    pub const SIOCOUTQNSD: u32 = libc::SIOCOUTQNSD as u32;
    /// `SIOCATMARK`.
    ///
    /// https://man7.org/linux/man-pages/man7/tcp.7.html
    pub const SIOCATMARK: u32 = linux_raw_sys::ioctl::SIOCATMARK;
}

/// A value the kernel reads as an option's bytes and nothing more: it holds no address the kernel follows.
///
/// Implemented only for numbers, runs of them, and the uapi structures `options.table` lists for sets.
trait Plain {}

impl Plain for c_int {}
impl Plain for u32 {}
impl Plain for u64 {}
impl Plain for u8 {}
impl<T: Plain> Plain for [T] {}

/// What a successful call answers.
const SUCCESS: c_int = 0;

/// The `nkeys` of a `TCP_AO_GET_KEYS` query that the room of one answer holds: the kernel writes one answer per key
/// asked for, whatever length it is given.
///
/// https://docs.kernel.org/networking/tcp_ao.html
const KEYS_IN_ONE_ANSWER: u32 = 1;

fn invalid() -> HostError {
    HostError::Errno(libc::EINVAL)
}

/// Gives the error of the call that just failed on this thread, a would-block as the contract gives it.
fn last_error() -> HostError {
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::EAGAIN) => HostError::WouldBlock,
        Some(number) => HostError::Errno(number),
        None => HostError::Errno(libc::EIO),
    }
}

fn length_of(bytes: usize) -> Result<libc::socklen_t, HostError> {
    libc::socklen_t::try_from(bytes).map_err(|_| invalid())
}

/// Calls `setsockopt` with the bytes at `value`, `length` of them.
fn set_bytes_at(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    value: *const c_void,
    length: libc::socklen_t,
) -> Result<(), HostError> {
    #[expect(
        unsafe_code,
        reason = "every caller gives `length` readable bytes at `value` that hold no address the kernel follows, except \
                  `set_program`, whose `sock_fprog` points at `len` instructions it holds; the descriptor is borrowed, so open"
    )]
    let result = unsafe {
        libc::setsockopt(
            socket.as_raw_fd(),
            level.cast_signed(),
            name.cast_signed(),
            value,
            length,
        )
    };
    if result == SUCCESS {
        Ok(())
    } else {
        Err(last_error())
    }
}

/// Sets an option to the bytes of a plain value, all of them.
fn set_value<T: Plain + ?Sized>(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    value: &T,
) -> Result<(), HostError> {
    let length = length_of(size_of_val(value))?;
    set_bytes_at(
        socket,
        level,
        name,
        std::ptr::from_ref(value).cast(),
        length,
    )
}

/// Sets an option that takes a classic BPF program, through the `struct sock_fprog` that points at it.
fn set_program(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    program: &[libc::sock_filter],
) -> Result<(), HostError> {
    let header = libc::sock_fprog {
        len: u16::try_from(program.len()).map_err(|_| invalid())?,
        filter: program.as_ptr().cast_mut(),
    };
    let length = length_of(size_of::<libc::sock_fprog>())?;
    set_bytes_at(
        socket,
        level,
        name,
        std::ptr::from_ref(&header).cast(),
        length,
    )
}

/// The start of `struct group_filter` before its sources, as linux-raw-sys names it.
type GroupFilterHeader = uapi::group_filter__bindgen_ty_1__bindgen_ty_2;

/// Sets `MCAST_MSFILTER` to a source filter written as `struct group_filter` bytes, at least the structure's length.
fn set_group_filter(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    filter: &GroupFilter<'_>,
) -> Result<(), HostError> {
    let start = offset_of!(GroupFilterHeader, gf_slist_flex);
    let sources = filter
        .sources
        .len()
        .checked_mul(SOCKET_STORAGE_LENGTH)
        .and_then(|bytes| bytes.checked_add(start))
        .ok_or_else(invalid)?;
    let mut bytes = vec![u8::MIN; sources.max(size_of::<uapi::group_filter>())];
    let mut put = |at: usize, value: &[u8]| bytes[at..at + value.len()].copy_from_slice(value);
    put(
        offset_of!(GroupFilterHeader, gf_interface),
        &filter.interface.to_ne_bytes(),
    );
    put(
        offset_of!(GroupFilterHeader, gf_group),
        filter.group.as_bytes(),
    );
    put(
        offset_of!(GroupFilterHeader, gf_fmode),
        &filter.mode.to_ne_bytes(),
    );
    put(
        offset_of!(GroupFilterHeader, gf_numsrc),
        &u32::try_from(filter.sources.len())
            .map_err(|_| invalid())?
            .to_ne_bytes(),
    );
    for (index, source) in filter.sources.iter().enumerate() {
        put(start + index * SOCKET_STORAGE_LENGTH, source.as_bytes());
    }
    set_value(socket, level, name, bytes.as_slice())
}

/// Calls `getsockopt` into `into`, giving the length the kernel wrote, never more than `into` holds.
fn get_into(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    into: &mut [u8],
) -> Result<usize, HostError> {
    let mut length = length_of(into.len())?;
    #[expect(
        unsafe_code,
        reason = "the kernel writes at most `length` bytes at the pointer, the whole of `into`, for every option read \
                  through here: none reads an address from the value, and `TCP_AO_GET_KEYS` only after `get_keys` has \
                  checked that it asks for one answer; the descriptor is borrowed, so open"
    )]
    let result = unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            level.cast_signed(),
            name.cast_signed(),
            into.as_mut_ptr().cast(),
            &raw mut length,
        )
    };
    if result != SUCCESS {
        return Err(last_error());
    }
    Ok(usize::try_from(length).map_or(into.len(), |written| written.min(into.len())))
}

/// Reads an option whose value is a number, zero where the kernel wrote less.
fn get_value<T: FromBytes + IntoBytes>(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
) -> Result<T, HostError> {
    let mut value = T::new_zeroed();
    get_into(socket, level, name, value.as_mut_bytes())?;
    Ok(value)
}

/// Reads an option into a room of `ROOM` zero bytes.
fn get_written<const ROOM: usize>(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
) -> Result<Written<ROOM>, HostError> {
    let mut bytes = [u8::MIN; ROOM];
    let length = get_into(socket, level, name, &mut bytes)?;
    Ok(Written { bytes, length })
}

/// Asks `TCP_AO_GET_KEYS` the query in place, refusing one that asks for more answers than the room holds.
fn get_keys(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    query: &[u8; KEY_QUERY_LENGTH],
) -> Result<Written<KEY_QUERY_LENGTH>, HostError> {
    let at = offset_of!(uapi::tcp_ao_getsockopt, nkeys);
    let asked = query
        .get(at..at + size_of::<u32>())
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_ne_bytes)
        .ok_or_else(invalid)?;
    if asked > KEYS_IN_ONE_ANSWER {
        return Err(invalid());
    }
    let mut bytes = *query;
    let length = get_into(socket, level, name, &mut bytes)?;
    Ok(Written { bytes, length })
}

/// `struct mptcp_full_info` with the connection's `struct mptcp_info` as Linux 6.18 defines it.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
#[repr(C)]
#[derive(Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable)]
struct FullInformationHeader {
    size_tcpinfo_kernel: u32,
    size_tcpinfo_user: u32,
    size_sfinfo_kernel: u32,
    size_sfinfo_user: u32,
    num_subflows: u32,
    size_arrays_user: u32,
    subflow_info: u64,
    tcp_info: u64,
    mptcp_info: [u8; MPTCP_INFO_LENGTH],
}

/// Reads `MPTCP_FULL_INFO` with room for `subflows` subflows, the kernel writing each one's values into the answer.
fn get_full_info<const SUBFLOW_ROOM: usize, const INFORMATION_ROOM: usize>(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    subflows: usize,
) -> Result<MptcpFullInformation<SUBFLOW_ROOM, INFORMATION_ROOM>, HostError> {
    let narrow = |count: usize| u32::try_from(count).map_err(|_| invalid());
    let address =
        |pointer: *mut u8| u64::try_from(pointer.expose_provenance()).map_err(|_| invalid());
    let mut answer = MptcpFullInformation {
        tcp_information_length: u32::MIN,
        subflow_information_length: u32::MIN,
        subflows: u32::MIN,
        connection: [u8::MIN; MPTCP_INFO_LENGTH],
        subflow_information: vec![[u8::MIN; SUBFLOW_ROOM]; subflows],
        tcp_information: vec![[u8::MIN; INFORMATION_ROOM]; subflows],
    };
    let mut header = FullInformationHeader {
        size_tcpinfo_user: narrow(INFORMATION_ROOM)?,
        size_sfinfo_user: narrow(SUBFLOW_ROOM)?,
        size_arrays_user: narrow(subflows)?,
        subflow_info: address(answer.subflow_information.as_mut_ptr().cast())?,
        tcp_info: address(answer.tcp_information.as_mut_ptr().cast())?,
        ..FullInformationHeader::new_zeroed()
    };
    let mut length = length_of(size_of::<FullInformationHeader>())?;
    #[expect(
        unsafe_code,
        reason = "the kernel writes at most `length` bytes of the header, and at most `size_arrays_user` entries of \
                  `size_sfinfo_user` and of `size_tcpinfo_user` bytes at `subflow_info` and `tcp_info`, which are the \
                  answer's two runs of exactly those counts and sizes, alive and not moved during the call; the \
                  descriptor is borrowed"
    )]
    let result = unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            level.cast_signed(),
            name.cast_signed(),
            header.as_mut_bytes().as_mut_ptr().cast(),
            &raw mut length,
        )
    };
    if result != SUCCESS {
        return Err(last_error());
    }
    answer.tcp_information_length = header.size_tcpinfo_kernel;
    answer.subflow_information_length = header.size_sfinfo_kernel;
    answer.subflows = header.num_subflows;
    answer.connection = header.mptcp_info;
    Ok(answer)
}

/// Makes an ioctl request on the socket that writes one int.
fn ioctl_int(socket: BorrowedFd<'_>, request: u32) -> Result<c_int, HostError> {
    let mut value = c_int::MIN;
    #[expect(
        unsafe_code,
        reason = "every request made through here (SIOCOUTQ, SIOCOUTQNSD, SIOCATMARK) writes one int at the pointer, \
                  which is `value`; the descriptor is borrowed, so open"
    )]
    let result = unsafe {
        libc::ioctl(
            socket.as_raw_fd(),
            libc::Ioctl::from(request),
            &raw mut value,
        )
    };
    if result < SUCCESS {
        return Err(last_error());
    }
    Ok(value)
}
