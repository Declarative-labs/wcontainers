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
use std::os::fd::{AsRawFd, BorrowedFd, RawFd};

use libc::socklen_t;
use rustix::io::Errno;
use wcontainers_socket_options::{
    MPTCP_FULL_INFO, MultipathCounts, PlainOption, QueuedCount, SocketOptions, ValueLength,
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
        set(socket.as_raw_fd(), option, value)
    }

    fn get_option(
        &self,
        socket: BorrowedFd<'_>,
        option: PlainOption,
        into: &mut [u8],
    ) -> Result<usize, Errno> {
        get(socket.as_raw_fd(), option, into)
    }

    fn count_queued(&self, socket: BorrowedFd<'_>, count: QueuedCount) -> Result<c_int, Errno> {
        let request = match count {
            // SIOCOUTQ is TIOCOUTQ, as sockios.h defines it.
            QueuedCount::SendQueue => linux_raw_sys::ioctl::TIOCOUTQ,
            QueuedCount::Unsent => libc::SIOCOUTQNSD as u32,
            QueuedCount::AtUrgentMark => linux_raw_sys::ioctl::SIOCATMARK,
        };
        read_count(socket.as_raw_fd(), request)
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

/// Refuses a value whose length is not the option's, and gives the length as the call takes it.
fn checked_length(option: PlainOption, given: usize) -> Result<socklen_t, Errno> {
    match option.value_length() {
        ValueLength::Exactly(length) if length != given => Err(Errno::INVAL),
        _ => socklen_t::try_from(given).map_err(|_| Errno::INVAL),
    }
}

fn set(socket: RawFd, option: PlainOption, value: &[u8]) -> Result<(), Errno> {
    let length = checked_length(option, value.len())?;
    let (level, name) = option.level_and_name();
    set_bytes(socket, level as c_int, name as c_int, value, length)
}

fn get(socket: RawFd, option: PlainOption, into: &mut [u8]) -> Result<usize, Errno> {
    let length = checked_length(option, into.len())?;
    let (level, name) = option.level_and_name();
    get_bytes(socket, level as c_int, name as c_int, into, length)
}

#[expect(
    unsafe_code,
    reason = "setsockopt has no safe form for these options: it reads `length` bytes, which `checked_length` made the value's own length"
)]
fn set_bytes(
    socket: RawFd,
    level: c_int,
    name: c_int,
    value: &[u8],
    length: socklen_t,
) -> Result<(), Errno> {
    // SAFETY: the kernel reads at most `length` bytes from the start of `value`, which is that long, and a listed
    // option's value holds no address the kernel follows.
    let result = unsafe { foreign::setsockopt(socket, level, name, value.as_ptr().cast(), length) };
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
    // in `written`, a local, how many; a listed option's value holds no address the kernel follows.
    let result =
        unsafe { foreign::getsockopt(socket, level, name, into.as_mut_ptr().cast(), &mut written) };
    answer(result)?;
    Ok(written.min(length) as usize)
}

#[expect(
    unsafe_code,
    reason = "ioctl has no safe form for SIOCOUTQ, SIOCOUTQNSD and SIOCATMARK: each writes one int"
)]
fn read_count(socket: RawFd, request: u32) -> Result<c_int, Errno> {
    let mut value = c_int::MIN;
    // SAFETY: each request `count_queued` passes writes one int, to `value`, a local.
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
