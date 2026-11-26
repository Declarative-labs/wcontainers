//! Linux's socket option and socket ioctl calls made on the kernel the program runs on.
//!
//! Every `unsafe` of the host socket crates is here, each use marked with what makes it sound.
#![cfg(target_os = "linux")]

mod generated;
#[cfg(test)]
mod probe;

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
#[derive(Debug)]
pub struct DirectLinuxSocketOptions;

/// The options and requests that neither linux-raw-sys nor the uapi names, with where Linux defines each.
mod local {
    /// The multipath TCP option that reads the connection's `struct mptcp_info`.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
    pub(crate) const MPTCP_INFO: u32 = 1;
    /// The multipath TCP option that reads the connection's and its subflows' information at once.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/mptcp.h
    pub(crate) const MPTCP_FULL_INFO: u32 = 4;
    /// The UDP option that sends IPv6 datagrams with a zero checksum.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
    pub(crate) const UDP_NO_CHECK6_TX: u32 = libc::UDP_NO_CHECK6_TX as u32;
    /// The UDP option that takes IPv6 datagrams with a zero checksum.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
    pub(crate) const UDP_NO_CHECK6_RX: u32 = libc::UDP_NO_CHECK6_RX as u32;
    /// The level of UDP-Lite's own options, its protocol number.
    ///
    /// https://man7.org/linux/man-pages/man7/udplite.7.html
    pub(crate) const SOL_UDPLITE: u32 = super::uapi::IPPROTO_UDPLITE as u32;
    /// The UDP-Lite option that sets how much of each datagram sent the checksum covers.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
    pub(crate) const UDPLITE_SEND_CSCOV: u32 = 10;
    /// The UDP-Lite option that sets how much of each datagram taken the checksum must cover.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/udp.h
    pub(crate) const UDPLITE_RECV_CSCOV: u32 = 11;
    /// The request for the bytes sent and not yet acknowledged, which Linux defines as `TIOCOUTQ`.
    ///
    /// https://man7.org/linux/man-pages/man7/tcp.7.html
    pub(crate) const SIOCOUTQ: u32 = linux_raw_sys::ioctl::TIOCOUTQ;
    /// The request for the bytes not yet sent.
    ///
    /// https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/sockios.h
    pub(crate) const SIOCOUTQNSD: u32 = libc::SIOCOUTQNSD as u32;
    /// The request for whether the urgent mark is next to read.
    ///
    /// https://man7.org/linux/man-pages/man7/tcp.7.html
    pub(crate) const SIOCATMARK: u32 = linux_raw_sys::ioctl::SIOCATMARK;
}

/// A value whose bytes the kernel takes as an option's value: it holds no address the kernel follows.
///
/// Implemented only for bytes, runs of values, and the uapi structures `options.table` lists for sets; whether the
/// kernel may also write into it is the table's kernel column, which picks `set_value` or `set_value_written`.
trait Plain {}

impl Plain for u8 {}
impl<T: Plain> Plain for [T] {}

/// The canary bytes kept after every value or room of the implementation's own that the kernel is told of.
///
/// A kernel that wrote past the length it was told would change one, and the check after the call stops the program,
/// as a stack protector does, so that a wrong kernel column in `options.table` cannot go unseen.
const GUARD: usize = 64;

/// The first canary byte; each next one adds one, so no canary is zero and no two neighbors are equal.
const FIRST_CANARY: u8 = 0xa5;

/// The canaries of a guard, in order.
const CANARIES: [u8; GUARD] = canaries();

const fn canaries() -> [u8; GUARD] {
    let mut bytes = [FIRST_CANARY; GUARD];
    let mut index = 0;
    while index < GUARD {
        bytes[index] = FIRST_CANARY.wrapping_add(index as u8);
        index += 1;
    }
    bytes
}

/// A room of `ROOM` bytes the kernel is told of, followed by its guard.
#[repr(C)]
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable)]
struct Guarded<const ROOM: usize> {
    room: [u8; ROOM],
    guard: [u8; GUARD],
}

/// The room of a number option, the widest number a row reads or writes.
const NUMBER_ROOM: usize = size_of::<u64>();

/// Writes the canaries into `guard`, repeating them past the first `GUARD` bytes.
fn fill_canaries(guard: &mut [u8]) {
    for (byte, canary) in guard.iter_mut().zip(CANARIES.iter().cycle()) {
        *byte = *canary;
    }
}

/// Stops the program when a byte of `guard` lost its canary: the kernel wrote past what it was told.
fn check_canaries(guard: &[u8], told: usize, name: u32) {
    let held = guard
        .iter()
        .zip(CANARIES.iter().cycle())
        .all(|(byte, canary)| byte == canary);
    assert!(
        held,
        "canary changed: the kernel wrote past the {told} bytes it was told for option or request {name}, so its \
         kernel column in options.table is wrong"
    );
}

/// What a successful call answers.
///
/// https://man7.org/linux/man-pages/man2/setsockopt.2.html
const SUCCESS: c_int = 0;

/// The `nkeys` of a `TCP_AO_GET_KEYS` query that the room of one answer holds.
///
/// The kernel writes one answer per key asked for, whatever length it is given.
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
        reason = "every caller gives `length` bytes at `value` that it owns or borrows for the call, from an exclusive \
                  borrow wherever options.table says the kernel writes into the value (the probe checks each row on the \
                  running kernel), and holding no address the kernel follows except `set_program`'s `sock_fprog`, which \
                  points at `len` instructions held for the call; the descriptor is borrowed, so open"
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

/// Sets an option to `given`, copied to the start of `buffer` with canaries after it, and checks the kernel changed none.
fn set_guarded(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    given: &[u8],
    buffer: &mut [u8],
) -> Result<(), HostError> {
    let told = given.len();
    let (value, guard) = buffer.split_at_mut_checked(told).ok_or_else(invalid)?;
    value.copy_from_slice(given);
    fill_canaries(guard);
    let length = length_of(told)?;
    let result = set_bytes_at(
        socket,
        level,
        name,
        buffer.as_mut_ptr().cast_const().cast(),
        length,
    );
    assert!(
        buffer[..told] == *given,
        "the kernel wrote into the value of option {name}, which options.table says it only reads"
    );
    check_canaries(&buffer[told..], told, name);
    result
}

/// Sets an option whose value is a number or flag, copied into a guarded room of the implementation's own.
fn set_number<T: IntoBytes + Immutable>(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    value: &T,
) -> Result<(), HostError> {
    let mut buffer = Guarded::<NUMBER_ROOM>::new_zeroed();
    set_guarded(socket, level, name, value.as_bytes(), buffer.as_mut_bytes())
}

/// Sets an option to the bytes of a plain value, all of them, for an option whose value the kernel only reads.
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

/// Sets an option to the bytes of a plain value, all of them, for an option whose value the kernel may write into.
fn set_value_written<T: Plain + ?Sized>(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    value: &mut T,
) -> Result<(), HostError> {
    let length = length_of(size_of_val(value))?;
    set_bytes_at(
        socket,
        level,
        name,
        std::ptr::from_mut(value).cast_const().cast(),
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
    let len = u16::try_from(program.len()).map_err(|_| invalid())?;
    let filter = program.as_ptr().cast_mut();
    let mut guarded = GuardedProgram {
        header: libc::sock_fprog { len, filter },
        guard: CANARIES,
    };
    let told = size_of::<libc::sock_fprog>();
    let result = set_bytes_at(
        socket,
        level,
        name,
        std::ptr::from_mut(&mut guarded).cast_const().cast(),
        length_of(told)?,
    );
    assert!(
        guarded.header.len == len && guarded.header.filter == filter,
        "the kernel wrote into the program header of option {name}, which options.table says it only reads"
    );
    check_canaries(&guarded.guard, told, name);
    result
}

/// A `struct sock_fprog` the kernel is told of, followed by its guard.
#[repr(C)]
struct GuardedProgram {
    header: libc::sock_fprog,
    guard: [u8; GUARD],
}

/// The start of `struct group_filter` before its sources, as linux-raw-sys names it.
type GroupFilterHeader = uapi::group_filter__bindgen_ty_1__bindgen_ty_2;

/// Sets `MCAST_MSFILTER` to a source filter written as `struct group_filter` bytes.
fn set_group_filter(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    filter: &GroupFilter<'_>,
) -> Result<(), HostError> {
    let given = group_filter_bytes(filter)?;
    let mut buffer = vec![u8::MIN; given.len() + GUARD];
    set_guarded(socket, level, name, &given, &mut buffer)
}

/// Writes a source filter as `struct group_filter` bytes, at least the structure's length.
fn group_filter_bytes(filter: &GroupFilter<'_>) -> Result<Vec<u8>, HostError> {
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
    Ok(bytes)
}

/// Calls `getsockopt` into the start of `buffer`, telling the kernel `told` bytes, every byte after them a canary.
///
/// Gives the length the kernel wrote, never more than `told`, and stops the program when a canary changed.
fn get_guarded(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    buffer: &mut [u8],
    told: usize,
) -> Result<usize, HostError> {
    fill_canaries(buffer.get_mut(told..).ok_or_else(invalid)?);
    let result = get_within(socket, level, name, buffer, told);
    check_canaries(&buffer[told..], told, name);
    let written = result?;
    Ok(usize::try_from(written).map_or(told, |written| written.min(told)))
}

/// Calls `getsockopt` at the start of `into`, telling the kernel it has `told` bytes, and gives the length it answers.
fn get_within(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    into: &mut [u8],
    told: usize,
) -> Result<libc::socklen_t, HostError> {
    if told > into.len() {
        return Err(invalid());
    }
    let mut length = length_of(told)?;
    #[expect(
        unsafe_code,
        reason = "the kernel writes only inside `into`: options.table marks every option read through here `within` \
                  (at most the `told` bytes, no more than `into` holds, as the probe checks each row on the running \
                  kernel and the canaries after `told` check on every call) except `TCP_AO_GET_KEYS`, marked `past`, \
                  which writes `told` bytes per key asked, and `get_keys` asks at most one; the generator refuses any \
                  other form for these columns; none reads an address from the value; the descriptor is borrowed, so open"
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
    Ok(length)
}

/// Reads an option whose value is a number, zero where the kernel wrote less.
fn get_value<T: FromBytes + IntoBytes>(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
) -> Result<T, HostError> {
    let told = size_of::<T>();
    if told > NUMBER_ROOM {
        return Err(invalid());
    }
    let mut buffer = Guarded::<NUMBER_ROOM>::new_zeroed();
    get_guarded(socket, level, name, buffer.as_mut_bytes(), told)?;
    T::read_from_prefix(&buffer.room)
        .map(|(value, _)| value)
        .map_err(|_| invalid())
}

/// Reads an option into a room of `ROOM` zero bytes.
fn get_written<const ROOM: usize>(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
) -> Result<Written<ROOM>, HostError> {
    let mut buffer = Guarded::<ROOM>::new_zeroed();
    let length = get_guarded(socket, level, name, buffer.as_mut_bytes(), ROOM)?;
    Ok(Written {
        bytes: buffer.room,
        length,
    })
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
    let mut buffer = Guarded::<KEY_ROOM>::new_zeroed();
    buffer.room[..KEY_QUERY_LENGTH].copy_from_slice(query);
    let length = get_guarded(socket, level, name, buffer.as_mut_bytes(), KEY_QUERY_LENGTH)?;
    let mut bytes = *query;
    bytes.copy_from_slice(&buffer.room[..KEY_QUERY_LENGTH]);
    Ok(Written { bytes, length })
}

/// The room of a key query: its answer, then a second answer's worth of canaries before the guard, so that a second
/// answer the kernel writes is caught whole rather than written past the guard.
const KEY_ROOM: usize = KEY_QUERY_LENGTH + KEY_QUERY_LENGTH;

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
    // One room more in each run, holding canaries, past the `subflows` the kernel is told of.
    let rooms = subflows.checked_add(1).ok_or_else(invalid)?;
    let mut answer = MptcpFullInformation {
        tcp_information_length: u32::MIN,
        subflow_information_length: u32::MIN,
        subflows: u32::MIN,
        connection: [u8::MIN; MPTCP_INFO_LENGTH],
        subflow_information: vec![[u8::MIN; SUBFLOW_ROOM]; rooms],
        tcp_information: vec![[u8::MIN; INFORMATION_ROOM]; rooms],
    };
    fill_canaries(&mut answer.subflow_information[subflows]);
    fill_canaries(&mut answer.tcp_information[subflows]);
    let mut guarded = GuardedHeader::new_zeroed();
    let result = full_info_into(
        socket,
        level,
        name,
        &mut guarded,
        &mut answer.subflow_information,
        &mut answer.tcp_information,
        subflows,
    );
    check_canaries(&guarded.guard, size_of::<FullInformationHeader>(), name);
    check_canaries(&answer.subflow_information[subflows], subflows, name);
    check_canaries(&answer.tcp_information[subflows], subflows, name);
    result?;
    answer.subflow_information.truncate(subflows);
    answer.tcp_information.truncate(subflows);
    let header = guarded.header;
    answer.tcp_information_length = header.size_tcpinfo_kernel;
    answer.subflow_information_length = header.size_sfinfo_kernel;
    answer.subflows = header.num_subflows;
    answer.connection = header.mptcp_info;
    Ok(answer)
}

/// `MPTCP_FULL_INFO`'s header, followed by its guard.
#[repr(C)]
#[derive(FromBytes, IntoBytes, KnownLayout, Immutable)]
struct GuardedHeader {
    header: FullInformationHeader,
    guard: [u8; GUARD],
}

/// Reads `MPTCP_FULL_INFO` into the header, telling the kernel it has `told` entries at the start of each run of rooms.
fn full_info_into<const SUBFLOW_ROOM: usize, const INFORMATION_ROOM: usize>(
    socket: BorrowedFd<'_>,
    level: u32,
    name: u32,
    guarded: &mut GuardedHeader,
    subflow_rooms: &mut [[u8; SUBFLOW_ROOM]],
    information_rooms: &mut [[u8; INFORMATION_ROOM]],
    told: usize,
) -> Result<(), HostError> {
    if told > subflow_rooms.len() || told > information_rooms.len() {
        return Err(invalid());
    }
    let narrow = |count: usize| u32::try_from(count).map_err(|_| invalid());
    let address =
        |pointer: *mut u8| u64::try_from(pointer.expose_provenance()).map_err(|_| invalid());
    guarded.header = FullInformationHeader {
        size_tcpinfo_user: narrow(INFORMATION_ROOM)?,
        size_sfinfo_user: narrow(SUBFLOW_ROOM)?,
        size_arrays_user: narrow(told)?,
        subflow_info: address(subflow_rooms.as_mut_ptr().cast())?,
        tcp_info: address(information_rooms.as_mut_ptr().cast())?,
        ..FullInformationHeader::new_zeroed()
    };
    fill_canaries(&mut guarded.guard);
    let mut length = length_of(size_of::<FullInformationHeader>())?;
    #[expect(
        unsafe_code,
        reason = "the kernel writes at most `length` bytes of the header, and at most `size_arrays_user` entries of \
                  `size_sfinfo_user` and of `size_tcpinfo_user` bytes at `subflow_info` and `tcp_info`, which are two \
                  runs of rooms of exactly those sizes holding at least that many, alive and not moved during the call \
                  (options.table marks the option `through`, the probe checks it on the running kernel, and canaries \
                  after the header and after each run check every call); the descriptor is borrowed"
    )]
    let result = unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            level.cast_signed(),
            name.cast_signed(),
            guarded.as_mut_bytes().as_mut_ptr().cast(),
            &raw mut length,
        )
    };
    if result != SUCCESS {
        return Err(last_error());
    }
    Ok(())
}

/// Makes an ioctl request on the socket that writes one int.
fn ioctl_int(socket: BorrowedFd<'_>, request: u32) -> Result<c_int, HostError> {
    let mut buffer = Guarded::<{ size_of::<c_int>() }>::new_zeroed();
    fill_canaries(&mut buffer.guard);
    let result = ioctl_within(socket, request, buffer.as_mut_bytes());
    check_canaries(&buffer.guard, size_of::<c_int>(), request);
    result?;
    Ok(c_int::from_ne_bytes(buffer.room))
}

/// Makes an ioctl request on the socket that writes one int at the start of `into`.
fn ioctl_within(socket: BorrowedFd<'_>, request: u32, into: &mut [u8]) -> Result<(), HostError> {
    if into.len() < size_of::<c_int>() {
        return Err(invalid());
    }
    #[expect(
        unsafe_code,
        reason = "every request made through here (SIOCOUTQ, SIOCOUTQNSD, SIOCATMARK, marked `within` in \
                  options.table and checked by the probe) writes one int at the pointer, and `into` holds at least one; \
                  the descriptor is borrowed, so open"
    )]
    let result = unsafe {
        libc::ioctl(
            socket.as_raw_fd(),
            libc::Ioctl::from(request),
            into.as_mut_ptr(),
        )
    };
    if result < SUCCESS {
        return Err(last_error());
    }
    Ok(())
}
