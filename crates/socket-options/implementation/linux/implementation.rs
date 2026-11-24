//! The plain socket options and queue counts through Linux's `setsockopt`, `getsockopt` and `ioctl`.
#![cfg(target_os = "linux")]

#[cfg(not(any(kani, miri)))]
mod foreign {
    pub use libc::{getsockopt, ioctl, setsockopt};
}
/// Under Kani and Miri the calls reach a model of the kernel's copies, which they can check.
#[cfg(any(kani, miri))]
#[path = "model.rs"]
mod foreign;

#[cfg(kani)]
mod proofs;

use std::ffi::c_int;
use std::mem::offset_of;
use std::os::fd::{AsRawFd, BorrowedFd, RawFd};

use libc::socklen_t;
use linux_raw_sys::net as uapi;
use rustix::io::Errno;
use rustix::net::{AddressFamily, SocketType, ipproto, sockopt};
use wcontainers_socket_options::{
    AUTHENTICATION_KEY_QUERY, MPTCP_FULL_INFO, MultipathCounts, PlainOption, QueuedCount,
    SocketOptions, SourceFilterOptions, ValueLength,
};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[derive(Clone, Copy, Default, Debug)]
pub struct LinuxSocketOptions;

impl SocketOptions for LinuxSocketOptions {
    fn set_option(
        &self,
        socket: BorrowedFd<'_>,
        option: PlainOption,
        value: &[u8],
    ) -> Result<(), Errno> {
        internet_socket(socket)?;
        set(socket.as_raw_fd(), option, value)
    }

    fn get_option(
        &self,
        socket: BorrowedFd<'_>,
        option: PlainOption,
        into: &mut [u8],
    ) -> Result<usize, Errno> {
        internet_socket(socket)?;
        get(socket.as_raw_fd(), option, into)
    }

    fn count_queued(&self, socket: BorrowedFd<'_>, count: QueuedCount) -> Result<c_int, Errno> {
        match (internet_socket(socket)?, count) {
            (InternetSocket::Tcp, _)
            | (InternetSocket::Multipath, QueuedCount::SendQueue | QueuedCount::Unsent) => {}
            _ => return Err(Errno::NOTTY),
        }
        let request = match count {
            // SIOCOUTQ is TIOCOUTQ, as sockios.h defines it.
            QueuedCount::SendQueue => linux_raw_sys::ioctl::TIOCOUTQ,
            QueuedCount::Unsent => libc::SIOCOUTQNSD as u32,
            QueuedCount::AtUrgentMark => linux_raw_sys::ioctl::SIOCATMARK,
        };
        read_count(socket.as_raw_fd(), request)
    }

    fn get_authentication_key(
        &self,
        socket: BorrowedFd<'_>,
        query: &mut [u8; AUTHENTICATION_KEY_QUERY],
    ) -> Result<(), Errno> {
        internet_socket(socket)?;
        ask_for_one_key(query);
        let length = socklen_t::try_from(query.len()).map_err(|_| Errno::INVAL)?;
        let (level, name) = (uapi::SOL_TCP as c_int, uapi::TCP_AO_GET_KEYS as c_int);
        get_bytes(socket.as_raw_fd(), level, name, query, length).map(drop)
    }

    fn set_source_filter(
        &self,
        socket: BorrowedFd<'_>,
        filter: &SourceFilterOptions<'_>,
    ) -> Result<(), Errno> {
        internet_socket(socket)?;
        let mut request = source_filter_request(filter);
        let length = socklen_t::try_from(request.len()).map_err(|_| Errno::INVAL)?;
        let (level, name) = (filter.level.level() as c_int, uapi::MCAST_MSFILTER as c_int);
        set_bytes(socket.as_raw_fd(), level, name, &mut request, length)
    }

    fn get_multipath_full_information<const CONNECTION: usize, Subflow, Information>(
        &self,
        socket: BorrowedFd<'_>,
        connection: &mut [u8; CONNECTION],
        subflows: &mut [Subflow],
        information: &mut [Information],
    ) -> Result<MultipathCounts, Errno>
    where
        Subflow: FromBytes,
        Information: FromBytes,
    {
        internet_socket(socket)?;
        let mut header = FullInformation::<CONNECTION>::new(subflows, information)?;
        get_full_information(socket.as_raw_fd(), &mut header, subflows, information)?;
        let FullInformation {
            sizes,
            connection: written,
            ..
        } = header;
        *connection = written;
        Ok(MultipathCounts {
            tcp_info_size: sizes[TCP_INFO_KERNEL],
            subflow_info_size: sizes[SUBFLOW_INFO_KERNEL],
            subflows: sizes[SUBFLOWS],
        })
    }
}

/// The sockets the kernel's handlers of the listed options and the queue counts were read for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InternetSocket {
    Tcp,
    Multipath,
    /// UDP, UDP-Lite, ICMP, or a raw socket of any protocol.
    Datagram,
}

/// What the socket is, refusing a descriptor that is not a socket (`ENOTSOCK`) and a socket of another family or
/// protocol (`ENOPROTOOPT`), on which a level and name, or a request number, may mean anything.
fn internet_socket(socket: BorrowedFd<'_>) -> Result<InternetSocket, Errno> {
    let family = sockopt::socket_domain(socket)?;
    if family != AddressFamily::INET && family != AddressFamily::INET6 {
        return Err(Errno::NOPROTOOPT);
    }
    let protocol = sockopt::socket_protocol(socket)?;
    if protocol == Some(ipproto::TCP) {
        Ok(InternetSocket::Tcp)
    } else if protocol == Some(ipproto::MPTCP) {
        Ok(InternetSocket::Multipath)
    } else if [ipproto::UDP, ipproto::UDPLITE, ipproto::ICMP, ipproto::ICMPV6]
        .map(Some)
        .contains(&protocol)
        || sockopt::socket_type(socket)? == SocketType::RAW
    {
        Ok(InternetSocket::Datagram)
    } else {
        Err(Errno::NOPROTOOPT)
    }
}

/// Sets the query's `nkeys` to one, so that the kernel writes at most the one `tcp_ao_getsockopt` it is given.
fn ask_for_one_key(query: &mut [u8; AUTHENTICATION_KEY_QUERY]) {
    let at = offset_of!(uapi::tcp_ao_getsockopt, nkeys);
    query[at..at + size_of::<u32>()].copy_from_slice(&KEYS_ASKED.to_ne_bytes());
}

/// The keys a `TCP_AO_GET_KEYS` read asks for: the one its query names.
const KEYS_ASKED: u32 = 1;

/// `struct group_filter` with `gf_numsrc` the number of sources, followed by them.
fn source_filter_request(filter: &SourceFilterOptions<'_>) -> Vec<u8> {
    type Header = uapi::group_filter__bindgen_ty_1__bindgen_ty_2;
    let start = offset_of!(Header, gf_slist_flex);
    let mut request = vec![u8::MIN; start + size_of_val(filter.sources)];
    let mut put = |at: usize, value: &[u8]| request[at..at + value.len()].copy_from_slice(value);
    put(offset_of!(Header, gf_interface), &filter.interface.to_ne_bytes());
    put(offset_of!(Header, gf_group), &filter.group);
    put(offset_of!(Header, gf_fmode), &filter.mode.to_ne_bytes());
    let count = u32::try_from(filter.sources.len()).unwrap_or(u32::MAX);
    put(offset_of!(Header, gf_numsrc), &count.to_ne_bytes());
    put(start, filter.sources.as_flattened());
    request
}

/// Refuses a value whose length is not the option's, and gives the length as the call takes it.
fn checked_length(option: PlainOption, given: usize) -> Result<socklen_t, Errno> {
    match option.value_length() {
        ValueLength::Exactly(length) if length != given => Err(Errno::INVAL),
        _ => socklen_t::try_from(given).map_err(|_| Errno::INVAL),
    }
}

/// Gives the kernel a copy of the value, so that a write into it (`IPV6_FLOWLABEL_MGR` writes the label it chose)
/// never reaches the caller's shared bytes.
fn set(socket: RawFd, option: PlainOption, value: &[u8]) -> Result<(), Errno> {
    let length = checked_length(option, value.len())?;
    let (level, name) = option.level_and_name();
    set_bytes(socket, level as c_int, name as c_int, &mut value.to_vec(), length)
}

fn get(socket: RawFd, option: PlainOption, into: &mut [u8]) -> Result<usize, Errno> {
    let length = checked_length(option, into.len())?;
    let (level, name) = option.level_and_name();
    get_bytes(socket, level as c_int, name as c_int, into, length)
}

#[expect(
    unsafe_code,
    reason = "setsockopt has no safe form for these options: it reads, and may write, at most `length` bytes, the value's own length"
)]
fn set_bytes(
    socket: RawFd,
    level: c_int,
    name: c_int,
    value: &mut [u8],
    length: socklen_t,
) -> Result<(), Errno> {
    // SAFETY: the kernel reads, and for some options writes, at most `length` bytes from the start of `value`, which is
    // that long and borrowed only here; a listed option's value, and a source filter whose count `source_filter_request`
    // set, hold nothing that makes it go further.
    let result =
        unsafe { foreign::setsockopt(socket, level, name, value.as_mut_ptr().cast(), length) };
    answer(result).map(drop)
}

#[expect(
    unsafe_code,
    reason = "getsockopt has no safe form for these options: it writes at most `length` bytes, which `checked_length` made the buffer's own length"
)]
fn get_bytes(
    socket: RawFd,
    level: c_int,
    name: c_int,
    into: &mut [u8],
    length: socklen_t,
) -> Result<usize, Errno> {
    let mut written = length;
    // SAFETY: the kernel writes at most `written` bytes from the start of `into`, which is that long, and writes back
    // in `written`, a local, how many; a listed option's value, and a key query `ask_for_one_key` set to one key, hold
    // nothing that makes it go further.
    let result =
        unsafe { foreign::getsockopt(socket, level, name, into.as_mut_ptr().cast(), &mut written) };
    answer(result)?;
    Ok(written.min(length) as usize)
}

#[expect(
    unsafe_code,
    reason = "ioctl has no safe form for SIOCOUTQ, SIOCOUTQNSD and SIOCATMARK: on a socket whose protocol answers it, each writes one int"
)]
fn read_count(socket: RawFd, request: u32) -> Result<c_int, Errno> {
    let mut value = c_int::MIN;
    // SAFETY: `count_queued` passes only a request the socket's protocol answers, TCP all three and MPTCP the first two,
    // and each then writes one int, to `value`, a local; on another descriptor the number may mean any request.
    let result = unsafe { foreign::ioctl(socket, request as libc::Ioctl, &mut value) };
    answer(result)?;
    Ok(value)
}

/// The positions in [`FullInformation::sizes`] of `mptcp_full_info`'s first six fields.
const TCP_INFO_KERNEL: usize = 0;
const TCP_INFO_USER: usize = 1;
const SUBFLOW_INFO_KERNEL: usize = 2;
const SUBFLOW_INFO_USER: usize = 3;
const SUBFLOWS: usize = 4;
const ARRAYS_USER: usize = 5;
const SIZES: usize = ARRAYS_USER + 1;

/// `struct mptcp_full_info` with the connection's `mptcp_info` as bytes.
///
/// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable)]
struct FullInformation<const CONNECTION: usize> {
    sizes: [u32; SIZES],
    subflow_info: u64,
    tcp_info: u64,
    connection: [u8; CONNECTION],
}

impl<const CONNECTION: usize> FullInformation<CONNECTION> {
    /// A request whose two addresses are the arrays' starts, with their element sizes and the shorter one's length.
    fn new<Subflow, Information>(
        subflows: &mut [Subflow],
        information: &mut [Information],
    ) -> Result<Self, Errno> {
        let size = |element: usize| u32::try_from(element).map_err(|_| Errno::INVAL);
        let mut sizes = [u32::MIN; SIZES];
        sizes[TCP_INFO_USER] = size(size_of::<Information>())?;
        sizes[SUBFLOW_INFO_USER] = size(size_of::<Subflow>())?;
        sizes[ARRAYS_USER] =
            u32::try_from(subflows.len().min(information.len())).unwrap_or(u32::MAX);
        Ok(Self {
            sizes,
            subflow_info: subflows.as_mut_ptr().expose_provenance() as u64,
            tcp_info: information.as_mut_ptr().expose_provenance() as u64,
            connection: [u8::MIN; CONNECTION],
        })
    }
}

#[expect(
    unsafe_code,
    reason = "getsockopt has no safe form for MPTCP_FULL_INFO, whose request holds the addresses of the arrays the kernel fills"
)]
fn get_full_information<const CONNECTION: usize, Subflow, Information>(
    socket: RawFd,
    header: &mut FullInformation<CONNECTION>,
    _subflows: &mut [Subflow],
    _information: &mut [Information],
) -> Result<(), Errno> {
    let mut length =
        socklen_t::try_from(size_of::<FullInformation<CONNECTION>>()).map_err(|_| Errno::INVAL)?;
    // SAFETY: the kernel writes at most `length` bytes of the header, which is that long, and through the header's two
    // addresses at most `size_arrays_user` elements of each array, each element at most its own size apart and cut to
    // it; `FullInformation::new` made the addresses the starts of the two arrays, borrowed here for the call, and the
    // count the shorter one's length.
    let result = unsafe {
        foreign::getsockopt(
            socket,
            linux_raw_sys::net::SOL_MPTCP as c_int,
            MPTCP_FULL_INFO as c_int,
            header.as_mut_bytes().as_mut_ptr().cast(),
            &mut length,
        )
    };
    answer(result).map(drop)
}

/// The call's result, or the error number it set.
fn answer(result: c_int) -> Result<c_int, Errno> {
    if result < 0 {
        Err(Errno::from_io_error(&std::io::Error::last_os_error()).unwrap_or(Errno::IO))
    } else {
        Ok(result)
    }
}

#[cfg(all(test, miri))]
mod tests {
    use super::*;

    /// Lengths an option of any length is tried with.
    const ANY_LENGTHS: [usize; 3] = [0, 1, 64];

    fn lengths(option: PlainOption) -> Vec<(usize, bool)> {
        match option.value_length() {
            ValueLength::Exactly(length) => vec![
                (length, true),
                (length + 1, false),
                (length.saturating_sub(1), length == 0),
            ],
            ValueLength::Any => ANY_LENGTHS.iter().map(|&length| (length, true)).collect(),
        }
    }

    #[test]
    fn every_option_passes_only_its_own_bytes() {
        for &option in PlainOption::ALL {
            for (length, taken) in lengths(option) {
                let value = vec![u8::MAX; length];
                assert_eq!(set(0, option, &value).is_ok(), taken, "{option:?} {length}");
                let mut into = vec![u8::MIN; length];
                match get(0, option, &mut into) {
                    Ok(written) => assert!(taken && written <= length, "{option:?} {length}"),
                    Err(error) => assert!(!taken && error == Errno::INVAL, "{option:?} {length}"),
                }
            }
        }
    }

    #[test]
    fn every_count_writes_one_int() {
        for request in [
            linux_raw_sys::ioctl::TIOCOUTQ,
            libc::SIOCOUTQNSD as u32,
            linux_raw_sys::ioctl::SIOCATMARK,
        ] {
            assert!(read_count(0, request).is_ok());
        }
    }
}
