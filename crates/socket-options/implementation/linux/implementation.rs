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
mod scratch;

use std::ffi::c_int;
use std::mem::offset_of;
use std::os::fd::{AsRawFd, BorrowedFd, RawFd};

use libc::socklen_t;
use linux_raw_sys::net as uapi;
use rustix::io::Errno;
use rustix::net::{AddressFamily, SocketType, ipproto, sockopt};
use scratch::{Lent, lend};
use wcontainers_socket_options::{
    AUTHENTICATION_KEY_QUERY, MPTCP_FULL_INFO, MultipathCounts, PlainOption, QueuedCount,
    SocketOptions, SourceFilterOptions, Value,
};
use zerocopy::{FromBytes, FromZeros, Immutable, IntoBytes, KnownLayout};

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
        read_count(socket.as_raw_fd(), request(count))
    }

    fn get_authentication_key(
        &self,
        socket: BorrowedFd<'_>,
        query: &mut [u8; AUTHENTICATION_KEY_QUERY],
    ) -> Result<(), Errno> {
        internet_socket(socket)?;
        ask_for_one_key(query);
        let (level, name) = (uapi::SOL_TCP as c_int, uapi::TCP_AO_GET_KEYS as c_int);
        lend(query.len(), |lent| {
            lent.write(usize::MIN, query)?;
            get_through(socket.as_raw_fd(), level, name, lent)?;
            lent.read(usize::MIN, query)
        })
    }

    fn set_source_filter(
        &self,
        socket: BorrowedFd<'_>,
        filter: &SourceFilterOptions<'_>,
    ) -> Result<(), Errno> {
        internet_socket(socket)?;
        let request = source_filter_request(filter);
        let (level, name) = (filter.level.level() as c_int, uapi::MCAST_MSFILTER as c_int);
        lend(request.len(), |lent| {
            lent.write(usize::MIN, &request)?;
            set_through(socket.as_raw_fd(), level, name, lent)
        })
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
        let layout = FullLayout::new::<CONNECTION, Subflow, Information>(subflows.len(), information.len())?;
        lend(layout.length, |lent| {
            let header = FullInformation::<CONNECTION>::new(&layout, lent);
            lent.write(layout.header, header.as_bytes())?;
            get_through(socket.as_raw_fd(), SOL_MPTCP, MPTCP_FULL_INFO as c_int, &header_of(lent, &layout)?)?;
            let mut written = FullInformation::<CONNECTION>::new_zeroed();
            lent.read(layout.header, written.as_mut_bytes())?;
            fill(lent, layout.subflows, &mut subflows[..layout.count])?;
            fill(lent, layout.information, &mut information[..layout.count])?;
            *connection = written.connection;
            let sizes = written.sizes;
            Ok(MultipathCounts {
                tcp_info_size: sizes[TCP_INFO_KERNEL],
                subflow_info_size: sizes[SUBFLOW_INFO_KERNEL],
                subflows: sizes[SUBFLOWS],
            })
        })
    }
}

/// The request number of a count.
fn request(count: QueuedCount) -> u32 {
    match count {
        // SIOCOUTQ is TIOCOUTQ, as sockios.h defines it.
        QueuedCount::SendQueue => linux_raw_sys::ioctl::TIOCOUTQ,
        QueuedCount::Unsent => libc::SIOCOUTQNSD as u32,
        QueuedCount::AtUrgentMark => linux_raw_sys::ioctl::SIOCATMARK,
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

/// What the socket is, refusing a descriptor that is not a socket (`ENOTSOCK`) and a socket of another family, type or
/// protocol (`ENOPROTOOPT`). A courtesy, not the containment: the descriptor's number can name something else by the
/// time of the call.
fn internet_socket(socket: BorrowedFd<'_>) -> Result<InternetSocket, Errno> {
    let family = sockopt::socket_domain(socket)?;
    if family != AddressFamily::INET && family != AddressFamily::INET6 {
        return Err(Errno::NOPROTOOPT);
    }
    let kind = sockopt::socket_type(socket)?;
    let protocol = sockopt::socket_protocol(socket)?;
    if kind == SocketType::RAW {
        Ok(InternetSocket::Datagram)
    } else if kind == SocketType::STREAM && protocol == Some(ipproto::TCP) {
        Ok(InternetSocket::Tcp)
    } else if kind == SocketType::STREAM && protocol == Some(ipproto::MPTCP) {
        Ok(InternetSocket::Multipath)
    } else if kind == SocketType::DGRAM
        && [ipproto::UDP, ipproto::UDPLITE, ipproto::ICMP, ipproto::ICMPV6]
            .map(Some)
            .contains(&protocol)
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

/// Refuses a value whose length is not the option's.
fn checked_length(option: PlainOption, given: usize) -> Result<(), Errno> {
    match option.value().exact_length() {
        Some(length) if length != given => Err(Errno::INVAL),
        _ => Ok(()),
    }
}

/// Sets the option from a copy of the value in scratch, so that a write into it (`IPV6_FLOWLABEL_MGR` writes the label
/// it chose) never reaches the caller's shared bytes; an option that is only read is refused.
fn set(socket: RawFd, option: PlainOption, value: &[u8]) -> Result<(), Errno> {
    if option.value() == Value::Filled {
        return Err(Errno::NOPROTOOPT);
    }
    checked_length(option, value.len())?;
    let (level, name) = option.level_and_name();
    lend(value.len(), |lent| {
        lent.write(usize::MIN, value)?;
        set_through(socket, level as c_int, name as c_int, lent)
    })
}

/// Reads the option into zeroed scratch of the buffer's length, and copies what the kernel wrote into the buffer.
fn get(socket: RawFd, option: PlainOption, into: &mut [u8]) -> Result<usize, Errno> {
    checked_length(option, into.len())?;
    let (level, name) = option.level_and_name();
    lend(into.len(), |lent| {
        let written = get_through(socket, level as c_int, name as c_int, lent)?;
        lent.read(usize::MIN, &mut into[..written])?;
        Ok(written)
    })
}

/// Reads a count into four zeroed bytes of scratch.
fn read_count(socket: RawFd, request: u32) -> Result<c_int, Errno> {
    lend(size_of::<c_int>(), |lent| {
        ioctl_through(socket, request, lent)?;
        let mut value = [u8::MIN; size_of::<c_int>()];
        lent.read(usize::MIN, &mut value)?;
        Ok(c_int::from_ne_bytes(value))
    })
}

/// The length a call takes for a region.
fn length_of(lent: &Lent<'_>) -> Result<socklen_t, Errno> {
    socklen_t::try_from(lent.len()).map_err(|_| Errno::INVAL)
}

#[expect(
    unsafe_code,
    reason = "setsockopt has no safe form for these options: the kernel is given only a region of scratch that ends at unbacked guard pages"
)]
fn set_through(socket: RawFd, level: c_int, name: c_int, value: &Lent<'_>) -> Result<(), Errno> {
    let length = length_of(value)?;
    // SAFETY: the kernel is given one address, the start of `value`'s region of `length` bytes, which ends where the
    // scratch file ends; from there the guard is mapped with no page behind it, and before it lie 4 GiB of the same
    // file. So whatever the option means to whatever the descriptor is now, the kernel's reads and writes within 4 GiB
    // of that address reach only the scratch file, which no Rust reference covers, or fail with EFAULT. The bytes it
    // reads hold no address: they are a listed option's value of its kind, or a request the primitive built.
    // Assumed, and not checked: every handler a swapped descriptor can reach at this level and name reads the kind the
    // entry declares, not only the handler its `read_in` record names.
    let result = unsafe { foreign::setsockopt(socket, level, name, value.address(0), length) };
    answer(result).map(drop)
}

#[expect(
    unsafe_code,
    reason = "getsockopt has no safe form for these options: the kernel is given only a region of scratch that ends at unbacked guard pages"
)]
fn get_through(socket: RawFd, level: c_int, name: c_int, into: &Lent<'_>) -> Result<usize, Errno> {
    let length = length_of(into)?;
    let mut written = length;
    // SAFETY: as in `set_through`, the kernel's reads and writes from the region's address reach only the scratch file
    // or fail with EFAULT. It reads and writes `written`, a local `socklen_t`, as every socket's getsockopt does. The
    // region holds zeros, a key query, or a request whose two addresses `FullInformation::new` made point into the
    // same region.
    let result = unsafe { foreign::getsockopt(socket, level, name, into.address(0), &mut written) };
    answer(result)?;
    Ok(written.min(length) as usize)
}

#[expect(
    unsafe_code,
    reason = "ioctl has no safe form for SIOCOUTQ, SIOCOUTQNSD and SIOCATMARK: the kernel is given only four bytes of scratch that end at unbacked guard pages"
)]
fn ioctl_through(socket: RawFd, request: u32, into: &Lent<'_>) -> Result<(), Errno> {
    // SAFETY: as in `set_through`: whatever the request number means to whatever the descriptor is now, the kernel's
    // reads and writes from the address reach only the four zeroed bytes of scratch or fail with EFAULT.
    // Assumed, and not checked: every driver a swapped descriptor can reach treats the argument as user memory within
    // 4 GiB of the address, or as a plain number.
    let result = unsafe { foreign::ioctl(socket, request as libc::Ioctl, into.address(0)) };
    answer(result).map(drop)
}

/// `SOL_MPTCP`.
const SOL_MPTCP: c_int = uapi::SOL_MPTCP as c_int;

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

/// Where an `MPTCP_FULL_INFO` read keeps its parts in one region: the subflow array, the information array, and the
/// request last, so that the kernel's writes past any of them stay in the region or reach the guard.
struct FullLayout {
    /// How many elements of each array the request names: the shorter array's length.
    count: usize,
    subflow_size: usize,
    information_size: usize,
    subflows: usize,
    information: usize,
    header: usize,
    length: usize,
}

impl FullLayout {
    fn new<const CONNECTION: usize, Subflow, Information>(
        subflows: usize,
        information: usize,
    ) -> Result<Self, Errno> {
        let count = subflows.min(information).min(u32::MAX as usize);
        let (subflow_size, information_size) = (size_of::<Subflow>(), size_of::<Information>());
        let bytes = |size: usize| size.checked_mul(count).ok_or(Errno::INVAL);
        let information_at = bytes(subflow_size)?;
        let header = information_at.checked_add(bytes(information_size)?).ok_or(Errno::INVAL)?;
        let length = header.checked_add(size_of::<FullInformation<CONNECTION>>()).ok_or(Errno::INVAL)?;
        Ok(Self {
            count,
            subflow_size,
            information_size,
            subflows: usize::MIN,
            information: information_at,
            header,
            length,
        })
    }
}

impl<const CONNECTION: usize> FullInformation<CONNECTION> {
    /// A request whose two addresses are the arrays' places in the region, with their element sizes and count.
    fn new(layout: &FullLayout, lent: &Lent<'_>) -> Self {
        let mut sizes = [u32::MIN; SIZES];
        sizes[TCP_INFO_USER] = layout.information_size as u32;
        sizes[SUBFLOW_INFO_USER] = layout.subflow_size as u32;
        sizes[ARRAYS_USER] = layout.count as u32;
        Self {
            sizes,
            subflow_info: lent.address(layout.subflows).expose_provenance() as u64,
            tcp_info: lent.address(layout.information).expose_provenance() as u64,
            connection: [u8::MIN; CONNECTION],
        }
    }
}

/// The request's own place in the region: the region from the request on, ending where the region ends.
fn header_of<'a>(lent: &Lent<'a>, layout: &FullLayout) -> Result<Lent<'a>, Errno> {
    lent.from(layout.header)
}

/// Reads each element of the array from the region at `at`, one element apart.
fn fill<Element: FromBytes>(lent: &Lent<'_>, at: usize, array: &mut [Element]) -> Result<(), Errno> {
    let size = size_of::<Element>();
    if size == usize::MIN {
        return Ok(());
    }
    let mut bytes = vec![u8::MIN; size_of_val(array)];
    lent.read(at, &mut bytes)?;
    for (element, bytes) in array.iter_mut().zip(bytes.chunks_exact(size)) {
        *element = Element::read_from_bytes(bytes).map_err(|_| Errno::INVAL)?;
    }
    Ok(())
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
        match option.value().exact_length() {
            Some(length) => vec![
                (length, true),
                (length + 1, false),
                (length.saturating_sub(1), length == 0),
            ],
            None => ANY_LENGTHS.iter().map(|&length| (length, true)).collect(),
        }
    }

    #[test]
    fn every_option_passes_only_its_own_bytes() {
        for &option in PlainOption::ALL {
            let settable = option.value() != Value::Filled;
            for (length, taken) in lengths(option) {
                let value = vec![u8::MAX; length];
                assert_eq!(set(0, option, &value).is_ok(), taken && settable, "{option:?} {length}");
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
        for count in [QueuedCount::SendQueue, QueuedCount::Unsent, QueuedCount::AtUrgentMark] {
            assert!(read_count(0, request(count)).is_ok());
        }
    }

    #[test]
    fn a_full_information_read_stays_in_its_region() {
        let mut connection = [u8::MIN; 8];
        let mut subflows = [[u8::MIN; 3]; 2];
        let mut information = [u64::MIN; 3];
        let layout = FullLayout::new::<8, [u8; 3], u64>(subflows.len(), information.len()).unwrap();
        let counts = lend(layout.length, |lent| {
            let header = FullInformation::<8>::new(&layout, lent);
            lent.write(layout.header, header.as_bytes())?;
            get_through(0, SOL_MPTCP, MPTCP_FULL_INFO as c_int, &header_of(lent, &layout)?)?;
            fill(lent, layout.subflows, &mut subflows[..layout.count])?;
            fill(lent, layout.information, &mut information[..layout.count])?;
            let _ = &mut connection;
            Ok(layout.count)
        });
        assert_eq!(counts, Ok(2));
    }
}
